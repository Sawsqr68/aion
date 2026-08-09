//! A contained command's output, streamed into the activity transcript.
//!
//! [`CommandTranscript`] is the bridge between the two halves this build joins:
//! [`run_cancellable_command`](crate::process::run_cancellable_command) now
//! delivers a command's output line by line as it is written, and
//! [`ActivityContext::emit_event`] is the seam an activity publishes live
//! transcript events on. Wiring the first into the second is what makes a
//! command step readable WHILE it runs, through exactly the machinery an agent
//! step already uses — the same [`ActivityEvent`] envelope, the same
//! `(workflow, run, activity, attempt)` stream, the same durable append and
//! cursor reads on the server side. Nothing downstream had to learn what a
//! command is.
//!
//! # What one line becomes
//!
//! A line becomes a [`ActivityEventKind::Message`] attributed to
//! [`MessageRole::Tool`] — a non-conversational participant's output, which is
//! what a command's writing is. The stream it came from is carried by the
//! event's `agent_role` (the envelope's producer label), so stdout and stderr
//! stay distinguishable without inventing a persisted event kind for something
//! the locked set already models. `agent_id` is the nil UUID: a declared command
//! is not an agent and has no sub-identity to attribute, and nil is what this
//! codebase already writes where agent attribution is absent.
//!
//! # Size
//!
//! No byte ceiling is applied here, deliberately. The server's transcript
//! publisher bounds every event it persists against the operator's
//! `observability.max_event_bytes`, truncating on a `char` boundary with an
//! explicit marker; a second ceiling on this side would be the same rule known
//! in two places, free to drift from the one that is actually enforced. Nor does
//! streaming add an unbounded buffer: a line is a borrowed slice of the capture
//! the command's completion already retains in full, and the event carries only
//! that one line.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use aion_core::{ActivityEvent, ActivityEventKind, MessageRole};
use chrono::Utc;
use uuid::Uuid;

use crate::context::ActivityContext;
use crate::process::{CommandOutputObserver, CommandStream};

/// The producer label carried by a line the command wrote to standard output.
const STDOUT_PRODUCER: &str = "command stdout";

/// The producer label carried by a line the command wrote to standard error.
const STDERR_PRODUCER: &str = "command stderr";

/// The producer label for one of a command's output streams.
const fn producer_label(stream: CommandStream) -> &'static str {
    match stream {
        CommandStream::Stdout => STDOUT_PRODUCER,
        CommandStream::Stderr => STDERR_PRODUCER,
    }
}

/// Publishes a contained command's output lines onto an activity's transcript.
///
/// Built for one command execution and passed to
/// [`run_cancellable_command`](crate::process::run_cancellable_command) as its
/// observer. On a context with no live transcript seam — every isolated unit
/// test, and any activity whose host installed none — emitting is the documented
/// no-op that [`ActivityContext::emit_event`] already is, so a command runs
/// exactly as it did before.
#[derive(Debug)]
pub struct CommandTranscript<'context> {
    context: &'context ActivityContext,
    /// Worker-local monotonic sequence, shared across both streams so the
    /// numbering follows the order lines were actually observed.
    next_seq: AtomicU64,
    /// Whether the seam-closed diagnostic has already been emitted for this
    /// command, so a closed seam costs one log line and not one per line of
    /// output.
    seam_failure_reported: AtomicBool,
}

impl<'context> CommandTranscript<'context> {
    /// Build a transcript publisher for the command `context` is executing.
    #[must_use]
    pub fn new(context: &'context ActivityContext) -> Self {
        Self {
            context,
            next_seq: AtomicU64::new(0),
            seam_failure_reported: AtomicBool::new(false),
        }
    }

    /// The transcript event one observed line becomes. The full stream key —
    /// `(workflow, run, activity, attempt)` — is read off the context, which
    /// carries it by construction; a line is never attributed to an invented or
    /// partial identity.
    fn event(&self, stream: CommandStream, line: &str) -> ActivityEvent {
        ActivityEvent {
            workflow_id: self.context.workflow_id().clone(),
            run_id: self.context.run_id().clone(),
            activity_id: self.context.activity_id().clone(),
            attempt: self.context.attempt(),
            agent_id: Uuid::nil(),
            agent_role: producer_label(stream).to_owned(),
            // Producer-clock emission time, exactly as a harness adapter stamps
            // it. This is an observability record and never enters replay, so it
            // is not a determinism-boundary clock read.
            emitted_at: Utc::now(),
            worker_seq: self.next_seq.fetch_add(1, Ordering::Relaxed),
            // Assigned by the server's sequencer at durable-commit time; a
            // producer never mints one.
            store_seq: None,
            // Command output is retained transcript, not a token delta.
            ephemeral: false,
            kind: ActivityEventKind::Message {
                role: MessageRole::Tool,
                text: line.to_owned(),
            },
        }
    }
}

impl CommandOutputObserver for CommandTranscript<'_> {
    fn on_line(&self, stream: CommandStream, line: &str) {
        let event = self.event(stream, line);
        if let Err(error) = self.context.emit_event(event) {
            // The seam closed mid-command (its drain was dropped). The command
            // keeps running and its result is unaffected; the loss is named once
            // rather than once per line.
            if !self.seam_failure_reported.swap(true, Ordering::Relaxed) {
                tracing::warn!(
                    %error,
                    stream = stream.name(),
                    activity_id = %self.context.activity_id(),
                    attempt = self.context.attempt(),
                    "command transcript: the activity's event seam is closed; this command's \
                     remaining output is not streamed to the transcript"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use aion_core::{ActivityEventKind, ActivityId, MessageRole, RunId, WorkflowId};
    use tokio::sync::mpsc;
    use uuid::Uuid;

    use super::{CommandTranscript, STDERR_PRODUCER, STDOUT_PRODUCER};
    use crate::context::ActivityContext;
    use crate::process::{CommandOutputObserver, CommandStream};

    /// What a test returns. Every fallible step is carried rather than
    /// unwrapped, because the workspace denies panicking accessors in test code
    /// as firmly as in library code.
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn workflow_id() -> WorkflowId {
        WorkflowId::new(Uuid::from_u128(0x5eed))
    }

    fn run_id() -> RunId {
        RunId::new(Uuid::from_u128(0x5eed_0001))
    }

    /// The whole event contract for one observed line, asserted field by field:
    /// the stream key comes from the context, the line is a `Tool` message, the
    /// producing stream is named, and the sequence is the producer's own.
    #[tokio::test]
    async fn an_observed_line_becomes_a_transcript_message_keyed_to_the_activity() -> TestResult {
        let (sender, mut events) = mpsc::unbounded_channel();
        let (context, cancellation) = ActivityContext::with_transcript(
            workflow_id(),
            run_id(),
            ActivityId::from_sequence_position(4),
            2,
            sender,
        );
        drop(cancellation);
        let transcript = CommandTranscript::new(&context);

        transcript.on_line(CommandStream::Stdout, "building");

        let event = events.recv().await.ok_or("the line must be emitted")?;
        assert_eq!(event.workflow_id, workflow_id());
        assert_eq!(event.run_id, run_id());
        assert_eq!(event.activity_id, ActivityId::from_sequence_position(4));
        assert_eq!(event.attempt, 2);
        assert_eq!(event.agent_role, STDOUT_PRODUCER);
        assert_eq!(event.agent_id, Uuid::nil());
        assert_eq!(event.worker_seq, 0);
        assert_eq!(event.store_seq, None);
        assert!(
            !event.ephemeral,
            "command output is retained transcript, never a WS-only delta"
        );
        let ActivityEventKind::Message { role, text } = event.kind else {
            return Err("an output line must be a Message".into());
        };
        assert_eq!(role, MessageRole::Tool);
        assert_eq!(text, "building");
        Ok(())
    }

    /// Both streams reach the same transcript, stay distinguishable by their
    /// producer label, and share one monotonic sequence that follows arrival
    /// order.
    #[tokio::test]
    async fn both_streams_are_labelled_and_share_one_monotonic_sequence() -> TestResult {
        let (sender, mut events) = mpsc::unbounded_channel();
        let (context, cancellation) = ActivityContext::with_transcript(
            workflow_id(),
            run_id(),
            ActivityId::from_sequence_position(1),
            0,
            sender,
        );
        drop(cancellation);
        let transcript = CommandTranscript::new(&context);

        transcript.on_line(CommandStream::Stdout, "out");
        transcript.on_line(CommandStream::Stderr, "err");
        transcript.on_line(CommandStream::Stdout, "out again");

        let mut observed = Vec::new();
        for _ in 0..3u8 {
            let event = events.recv().await.ok_or("every line must be emitted")?;
            let ActivityEventKind::Message { text, .. } = event.kind else {
                return Err("an output line must be a Message".into());
            };
            observed.push((event.agent_role, text, event.worker_seq));
        }
        assert_eq!(
            observed,
            vec![
                (STDOUT_PRODUCER.to_owned(), "out".to_owned(), 0),
                (STDERR_PRODUCER.to_owned(), "err".to_owned(), 1),
                (STDOUT_PRODUCER.to_owned(), "out again".to_owned(), 2),
            ]
        );
        Ok(())
    }

    /// A context with no transcript seam installed — the isolated unit-test
    /// shape — publishes nothing and does not fail the command. Identity is
    /// always present (it is required at construction); absence of the SEAM is
    /// the only no-op condition left.
    #[tokio::test]
    async fn a_context_without_a_seam_publishes_nothing() {
        let (context, cancellation) = ActivityContext::new(
            workflow_id(),
            run_id(),
            ActivityId::from_sequence_position(1),
            0,
        );
        drop(cancellation);
        let transcript = CommandTranscript::new(&context);
        transcript.on_line(CommandStream::Stdout, "ignored");
    }

    /// A seam whose receiving end has been dropped mid-command does not fail the
    /// command: the remaining lines are dropped and the loss is reported, not
    /// escalated.
    #[tokio::test]
    async fn a_closed_seam_does_not_fail_the_command() -> TestResult {
        let (sender, events) = mpsc::unbounded_channel();
        let (context, cancellation) = ActivityContext::with_transcript(
            workflow_id(),
            run_id(),
            ActivityId::from_sequence_position(1),
            0,
            sender,
        );
        drop(cancellation);
        drop(events);
        let transcript = CommandTranscript::new(&context);

        transcript.on_line(CommandStream::Stdout, "into the void");
        transcript.on_line(CommandStream::Stderr, "also lost");
        Ok(())
    }
}
