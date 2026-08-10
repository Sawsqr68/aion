//! [`CliSession`] — a live plain-stdout CLI agent run behind the [`AgentSession`] seam.
//!
//! This is the **observability-only** case (case (b), §3A.1): the agent has no structured control
//! channel at all. One background **pump task** owns the child's stdout, reads it line by line,
//! demuxes each line into a neutral [`ActivityEvent`] ([`crate::demux`]), and forwards it to the
//! [`AgentSession::events`] stream. When stdout closes (end of run) it captures the terminal result
//! (the final stdout line, how the child ended, and the last stop reason the agent announced) and
//! hands it to [`AgentSession::wait_result`].
//!
//! The capability set is **always empty**: [`AgentSession::capabilities`] returns
//! [`InterventionCapabilities::none`] and [`AgentSession::intervene`] rejects **every** command with
//! [`HarnessError::CapabilityNotSupported`]. There is no control channel to route a command onto —
//! the empty advertisement is a first-class tier, not a degenerate one, and the server never routes
//! a command to a session that advertises nothing.

use std::sync::Arc;

use aion_core::{
    ActivityEvent, AgentOutcome, InterventionCapabilities, InterventionCommand, Payload, StopKind,
};
use aion_integrations::contract::AgentSession;
use aion_integrations::error::HarnessError;
use async_trait::async_trait;
use futures::stream::BoxStream;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader, Lines};
use tokio::sync::{Mutex, mpsc, oneshot};

use crate::demux::{self, EventIdentity};

/// How the child process ended, kept honest by naming all three endings.
///
/// A flattened `Option<i32>` conflated two endings that must not share a fate:
/// `None` used to mean BOTH "no child at all" (the in-memory reader path) and
/// "terminated by a signal" — and a signal-killed agent fell through the
/// non-zero-exit refusal into a result claiming the agent finished its turn.
enum ChildEnd {
    /// No child process exists — the session runs over a bare reader (tests,
    /// in-memory drivers). End of stream is the only ending observable.
    NoChild,
    /// The child exited on its own, with this exit code.
    Exited(i32),
    /// The child was terminated by a signal: it did not end by its own choice.
    Signalled,
    /// The child's exit status could not be read at all (an OS-level `wait`
    /// failure) — an ending that cannot honestly be classified as any of the
    /// other three, carried so it refuses loudly instead of guessing.
    StatusUnreadable {
        /// The wait error as the OS reported it.
        detail: String,
    },
}

/// The terminal outcome the pump captures when stdout closes: the final line,
/// how the child ended, and the last stop reason the agent itself announced.
struct TerminalOutcome {
    /// The last non-empty stdout line the agent printed, if any — its "final answer".
    final_line: Option<String>,
    /// How the child process ended.
    end: ChildEnd,
    /// The reason from the LAST `{"type":"stop",...}` line the demux produced,
    /// if the run announced one — the agent's own account of why it stopped,
    /// retained so the outcome record reports it instead of inventing one.
    announced_stop: Option<StopKind>,
}

/// A live plain-stdout CLI agent run for one activity attempt.
///
/// Generic over the child's stdout read half so tests can drive it over an in-memory reader; in
/// production it is the child process's stdout. The child handle is held so it is reaped on drop.
pub struct CliSession {
    capabilities: InterventionCapabilities,
    events: Option<mpsc::UnboundedReceiver<ActivityEvent>>,
    result: Option<oneshot::Receiver<TerminalOutcome>>,
    pump: Option<tokio::task::JoinHandle<()>>,
    /// The spawned child, reaped when the session is dropped. `None` for the in-memory test path.
    child: Arc<Mutex<Option<tokio::process::Child>>>,
}

impl CliSession {
    /// Builds a session over a child's stdout read half and starts the reader pump.
    ///
    /// `identity` stamps the run key onto every demuxed event. `child` (when present) is awaited by
    /// the pump for the exit status and reaped on drop. The pump starts here so events stream from
    /// the moment the session exists.
    pub(crate) fn start<R>(
        stdout: R,
        identity: EventIdentity,
        child: Option<tokio::process::Child>,
    ) -> Self
    where
        R: AsyncRead + Unpin + Send + 'static,
    {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (result_tx, result_rx) = oneshot::channel();
        let child = Arc::new(Mutex::new(child));

        let pump = tokio::spawn(pump_loop(
            BufReader::new(stdout).lines(),
            identity,
            event_tx,
            result_tx,
            Arc::clone(&child),
        ));

        Self {
            capabilities: InterventionCapabilities::none(),
            events: Some(event_rx),
            result: Some(result_rx),
            pump: Some(pump),
            child,
        }
    }
}

/// The reader pump: demuxes every stdout line into an event until stdout closes, then captures the
/// terminal outcome (final line + exit status) and sends it to the result waiter.
///
/// A send failure on the event channel means the consumer dropped the stream; the pump keeps
/// reading so the terminal result is still captured, it just stops forwarding events.
async fn pump_loop<R>(
    mut lines: Lines<BufReader<R>>,
    identity: EventIdentity,
    event_tx: mpsc::UnboundedSender<ActivityEvent>,
    result_tx: oneshot::Sender<TerminalOutcome>,
    child: Arc<Mutex<Option<tokio::process::Child>>>,
) where
    R: AsyncRead + Unpin + Send,
{
    let mut worker_seq: u64 = 0;
    let mut final_line: Option<String> = None;
    let mut announced_stop: Option<StopKind> = None;
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                if !line.trim().is_empty() {
                    final_line = Some(line.clone());
                }
                let event = demux::line_to_event(&identity, worker_seq, &line);
                worker_seq = worker_seq.saturating_add(1);
                // The agent's own stop announcement is part of the terminal
                // truth, not just transcript colour: the LAST one is retained
                // so the outcome record can report the reason the demux
                // already produced instead of inventing `end_turn`.
                if let aion_core::ActivityEventKind::Stop { reason } = &event.kind {
                    announced_stop = Some(reason.clone());
                }
                let _ = event_tx.send(event);
            }
            // End of stdout == end of run: capture the terminal outcome and stop.
            Ok(None) => break,
            Err(error) => {
                tracing::debug!(%error, "cli adapter: reader pump ended on a stdout read error");
                break;
            }
        }
    }

    let end = wait_child_end(&child).await;
    // A signal death is an ending the agent could not announce, so the pump
    // announces it: a synthesized terminal Stop event puts the detail
    // ("terminated by signal") on the transcript — the one channel whose
    // `StopKind::Other` carries a reason verbatim, where the outcome record's
    // canonical vocabulary collapses it to `"other"`.
    if matches!(end, ChildEnd::Signalled) {
        let stop = demux::synthesized_stop_event(
            &identity,
            worker_seq,
            StopKind::Other {
                reason: "terminated by signal".to_owned(),
            },
        );
        let _ = event_tx.send(stop);
    }
    // A dropped result receiver (the caller gave up on wait_result) is benign.
    let _ = result_tx.send(TerminalOutcome {
        final_line,
        end,
        announced_stop,
    });
}

/// Awaits the child's exit and reports how it ended — exit code, signal
/// termination, or no child at all (the in-memory reader path).
async fn wait_child_end(child: &Arc<Mutex<Option<tokio::process::Child>>>) -> ChildEnd {
    let mut guard = child.lock().await;
    let Some(child) = guard.as_mut() else {
        return ChildEnd::NoChild;
    };
    match child.wait().await {
        Ok(status) => match status.code() {
            Some(code) => ChildEnd::Exited(code),
            // On Unix an exit status without a code is a signal termination;
            // it must never be mistaken for a clean exit.
            None => ChildEnd::Signalled,
        },
        Err(error) => ChildEnd::StatusUnreadable {
            detail: error.to_string(),
        },
    }
}

#[async_trait]
impl AgentSession for CliSession {
    fn capabilities(&self) -> &InterventionCapabilities {
        // Always the empty set: observability-only, no control channel.
        &self.capabilities
    }

    fn events(&mut self) -> BoxStream<'static, ActivityEvent> {
        // The stream is taken once; a second call yields an empty stream rather than panicking.
        match self.events.take() {
            Some(receiver) => Box::pin(tokio_stream::wrappers::UnboundedReceiverStream::new(
                receiver,
            )),
            None => Box::pin(futures::stream::empty()),
        }
    }

    async fn intervene(&self, cmd: InterventionCommand) -> Result<(), HarnessError> {
        // The empty capability set rejects EVERY command: there is no control channel at all. This
        // is the first-class observability-only rejection, not an internal failure.
        Err(HarnessError::capability_not_supported(format!(
            "{:?}",
            cmd.kind.primitive()
        )))
    }

    async fn wait_result(mut self) -> Result<Payload, HarnessError> {
        let receiver = self
            .result
            .take()
            .ok_or_else(|| HarnessError::protocol("run result already awaited"))?;
        let outcome = receiver.await.map_err(|_recv| {
            HarnessError::transport("cli agent result channel closed before the run finished")
        })?;
        terminal_outcome_into_payload(outcome)
    }
}

/// Interprets the captured terminal outcome as the replay-authoritative [`Payload`].
///
/// A non-zero exit code is an application-level failure ([`HarnessError::Harness`]), and an
/// unreadable exit status refuses rather than guesses. Every other ending yields the canonical
/// [`AgentOutcome`] record — the same JSON result shape the worker captures from EVERY harness.
/// `text` is the final stdout line (the agent's "answer"); a run that printed nothing yields the
/// honest empty string, never an invented answer. `stop_reason` is the run's own account, through
/// the crate's demux vocabulary:
///
/// - the LAST `{"type":"stop","reason":...}` line the agent printed, when it printed one — the
///   reason the demux already produced, never re-derived;
/// - [`StopKind::EndTurn`] ONLY when a cleanly-ended run announced no stop line at all
///   (end-of-stdout with a zero exit, or the in-memory no-child path);
/// - a signal-killed child is [`StopKind::Other`] (`terminated by signal`): the completion policy
///   is unchanged — such a run completed before this and still does — but the record stops
///   claiming the agent chose to end its turn, and it overrides any earlier announced stop
///   because a stop line followed by a signal death was a mid-run announcement, not the ending.
fn terminal_outcome_into_payload(outcome: TerminalOutcome) -> Result<Payload, HarnessError> {
    let TerminalOutcome {
        final_line,
        end,
        announced_stop,
    } = outcome;
    let stop = match end {
        ChildEnd::Exited(code) if code != 0 => {
            return Err(HarnessError::harness(format!(
                "cli agent exited with a non-zero status: {code}"
            )));
        }
        ChildEnd::StatusUnreadable { detail } => {
            return Err(HarnessError::transport(format!(
                "the cli agent's exit status could not be read, so the run has no honest \
                 terminal classification: {detail}"
            )));
        }
        ChildEnd::Signalled => StopKind::Other {
            reason: "terminated by signal".to_owned(),
        },
        ChildEnd::Exited(_) | ChildEnd::NoChild => announced_stop.unwrap_or(StopKind::EndTurn),
    };
    AgentOutcome::new(final_line.unwrap_or_default(), &stop)
        .into_payload()
        .map_err(|source| {
            HarnessError::protocol(format!("cli agent result is not encodable: {source}"))
        })
}

impl Drop for CliSession {
    fn drop(&mut self) {
        // Abort the pump and best-effort kill the child so a dropped session leaks neither a task
        // nor a process.
        if let Some(pump) = self.pump.take() {
            pump.abort();
        }
        if let Ok(mut guard) = self.child.try_lock()
            && let Some(child) = guard.as_mut()
        {
            let _ = child.start_kill();
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    //! Fast unit tests of the session against an in-memory stdout reader — no real process. Canned
    //! stdout lines feed the REAL pump + demux, so the session's demux-to-event path is exercised
    //! end-to-end without spawning anything.

    use super::*;
    use aion_core::{
        ActivityEventKind, ActivityId, ContentType, InterventionKind, MessageRole, RunId,
        WorkflowId,
    };
    use chrono::Utc;
    use futures::StreamExt;
    use serde_json::json;
    use uuid::Uuid;

    fn identity() -> EventIdentity {
        EventIdentity {
            workflow_id: WorkflowId::new(Uuid::nil()),
            run_id: RunId::new(Uuid::from_u128(0x11)),
            activity_id: ActivityId::from_sequence_position(2),
            attempt: 1,
        }
    }

    /// Build a session over an in-memory stdout containing `text` (no child to await, so the exit
    /// code is `None` — a clean-exit result).
    fn session_over(text: &str) -> CliSession {
        let stdout = std::io::Cursor::new(text.as_bytes().to_vec());
        CliSession::start(stdout, identity(), None)
    }

    #[tokio::test]
    async fn capabilities_are_always_empty() {
        let session = session_over("");
        assert!(
            session.capabilities().is_empty(),
            "the observability-only session advertises no interventions"
        );
    }

    #[tokio::test]
    async fn stdout_lines_demux_into_a_live_event_stream() {
        let stdout = "\
[info] starting\n\
{\"type\":\"message\",\"text\":\"hello\"}\n\
{\"type\":\"stop\",\"reason\":\"end_turn\"}\n";
        let mut session = session_over(stdout);
        let events: Vec<ActivityEvent> = session.events().collect().await;

        assert_eq!(events.len(), 3, "every stdout line becomes an event");
        // A plain log line is Raw; the message and stop lines are mapped.
        assert!(matches!(events[0].kind, ActivityEventKind::Raw { .. }));
        match &events[1].kind {
            ActivityEventKind::Message { role, text } => {
                assert_eq!(*role, MessageRole::Assistant);
                assert_eq!(text, "hello");
            }
            other => panic!("expected Message, got {other:?}"),
        }
        assert!(matches!(events[2].kind, ActivityEventKind::Stop { .. }));
        // Ordering is stamped monotonically.
        assert_eq!(events[0].worker_seq, 0);
        assert_eq!(events[2].worker_seq, 2);
    }

    #[tokio::test]
    async fn intervene_rejects_every_command_as_capability_not_supported() {
        let session = session_over("{\"type\":\"stop\",\"reason\":\"end_turn\"}\n");
        for kind in [
            InterventionKind::InjectMessage {
                text: "steer".to_owned(),
                priority: aion_core::InjectPriority::Interrupt,
            },
            InterventionKind::Cancel {
                reason: "stop".to_owned(),
            },
        ] {
            let cmd = InterventionCommand {
                workflow_id: WorkflowId::new(Uuid::nil()),
                run_id: RunId::new_v4(),
                activity_id: ActivityId::from_sequence_position(2),
                attempt: 1,
                issued_by: None,
                issued_at: Utc::now(),
                kind,
            };
            let error = session.intervene(cmd).await.unwrap_err();
            assert!(
                matches!(error, HarnessError::CapabilityNotSupported { .. }),
                "an observability-only session rejects every command, got {error:?}"
            );
        }
    }

    #[tokio::test]
    async fn a_run_that_announced_no_stop_falls_back_to_end_turn() {
        // The fallback pin: NO `{"type":"stop"}` line anywhere in this stdout, so `end_turn`
        // here is the documented ended-normally-saying-nothing fallback, not a retained reason.
        let stdout = "\
{\"type\":\"message\",\"text\":\"thinking\"}\n\
the final answer is 42\n";
        let mut session = session_over(stdout);
        // Drain events so the pump reaches end-of-stdout and captures the terminal result.
        let _events: Vec<ActivityEvent> = session.events().collect().await;
        let payload = session.wait_result().await.unwrap();
        assert_eq!(payload.content_type(), &ContentType::Json);
        let decoded = payload.to_json().unwrap();
        assert_eq!(
            decoded,
            json!({ "text": "the final answer is 42", "stop_reason": "end_turn" }),
            "with no announced stop, a cleanly-ended run reports the end_turn fallback"
        );
    }

    #[tokio::test]
    async fn the_announced_stop_reason_reaches_the_outcome_record() {
        // The discriminating case: the agent announces a NON-end_turn stop, and that exact
        // reason must reach the record — a hardcoded `end_turn` fails here.
        let stdout = "\
partial work\n\
{\"type\":\"stop\",\"reason\":\"limit_reached\"}\n";
        let mut session = session_over(stdout);
        let _events: Vec<ActivityEvent> = session.events().collect().await;
        let decoded = session.wait_result().await.unwrap().to_json().unwrap();
        assert_eq!(
            decoded["stop_reason"],
            json!("limit_reached"),
            "the demuxed stop reason is the record's stop_reason, never re-derived: {decoded}"
        );
    }

    #[tokio::test]
    async fn the_last_announced_stop_wins() {
        // A run that stops for tools and then ends its turn reports the LAST announcement.
        let stdout = "\
{\"type\":\"stop\",\"reason\":\"tool_use\"}\n\
{\"type\":\"message\",\"text\":\"done now\"}\n\
{\"type\":\"stop\",\"reason\":\"cancelled\"}\n";
        let mut session = session_over(stdout);
        let _events: Vec<ActivityEvent> = session.events().collect().await;
        let decoded = session.wait_result().await.unwrap().to_json().unwrap();
        assert_eq!(decoded["stop_reason"], json!("cancelled"));
    }

    #[tokio::test]
    async fn an_event_is_never_the_result_the_two_are_separate_channels() {
        // The events stream and the terminal result are distinct: a message event whose text looks
        // like an answer is still only an event, and the result is captured separately.
        let mut session = session_over("{\"type\":\"message\",\"text\":\"not the result\"}\n");
        let events: Vec<ActivityEvent> = session.events().collect().await;
        assert_eq!(events.len(), 1);
        let payload = session.wait_result().await.unwrap();
        // The result carries the final LINE, which is the message JSON line, not the extracted text.
        let decoded = payload.to_json().unwrap();
        assert_eq!(decoded["stop_reason"], json!("end_turn"));
        assert_eq!(
            decoded["text"],
            json!("{\"type\":\"message\",\"text\":\"not the result\"}")
        );
    }
}
