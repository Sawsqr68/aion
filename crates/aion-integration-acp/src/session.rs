//! [`AcpSession`] — a live ACP prompt turn behind the [`AgentSession`] seam.
//!
//! The session owns three things and delegates the rest:
//!
//! - the **negotiated capability set** ([`AgentSession::capabilities`]),
//! - the **event stream** out ([`AgentSession::events`]) and the **command channel** in
//!   ([`AgentSession::intervene`]),
//! - the **terminal result** ([`AgentSession::wait_result`]), and the orderly shutdown of the agent
//!   process that follows it.
//!
//! The inbound half of the conversation lives in [`crate::pump`] — one background task that owns the
//! connection's reader, translates `session/update` into transcript events, answers every peer
//! request, and captures the `session/prompt` Response as the result. The interpretation of that
//! Response lives in [`crate::outcome`].
//!
//! # The events stream ends at the answer, not at the process
//!
//! The platform's agent driver treats the event stream ending as end-of-run and only then takes the
//! result. A real ACP agent is a persistent stdio server that keeps running after it answers
//! `session/prompt`, so the pump drops its transcript sender as soon as the terminal Response has
//! been handled rather than waiting for the child's stdout to close. See [`crate::pump`].
//!
//! # Cancellation
//!
//! [`AgentSession::intervene`] with [`aion_core::InterventionKind::Cancel`] sends the
//! `session/cancel` notification. The spec also requires that a client which cancels answers every
//! pending `session/request_permission` with the `cancelled` outcome, so the session raises a
//! cancelled flag BEFORE writing the notification: from that moment the pump answers every
//! permission request `cancelled` rather than consulting the policy. A request the pump had already
//! answered from the policy before the flag was raised stays answered as it was — it happened before
//! the cancellation, and rewriting history would be the dishonest option.
//!
//! [`AcpSession`]: crate::session::AcpSession
//! [`AgentSession`]: aion_integrations::AgentSession
//! [`AgentSession::capabilities`]: aion_integrations::AgentSession::capabilities
//! [`AgentSession::events`]: aion_integrations::AgentSession::events
//! [`AgentSession::intervene`]: aion_integrations::AgentSession::intervene
//! [`AgentSession::wait_result`]: aion_integrations::AgentSession::wait_result

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use agent_client_protocol_schema::v1::SessionId;
use aion_core::{
    ActivityEvent, InterventionCapabilities, InterventionCommand, InterventionKind, Payload,
};
use aion_integrations::HarnessError;
use aion_integrations::contract::AgentSession;
use aion_integrations::jsonrpc::{
    JsonRpcConnection, JsonRpcId, JsonRpcNotification, JsonRpcRequest,
};
use async_trait::async_trait;
use futures::stream::BoxStream;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::process::Child;
use tokio::sync::{mpsc, oneshot};

use crate::outcome::PumpEnd;
use crate::permission::PermissionPolicy;
use crate::protocol;
use crate::pump::{PumpContext, pump_loop};
use crate::translate::{self, EventIdentity};

/// Everything one live session needs at construction.
///
/// Grouped into a struct rather than eight positional arguments so a future field cannot be wired
/// into the wrong parameter slot — the same reason [`PumpContext`] is a struct.
pub(crate) struct SessionStart<R, W> {
    /// The connection whose handshake is done and whose `session/prompt` is already in flight.
    pub(crate) connection: Arc<JsonRpcConnection<R, W>>,
    /// The session `session/new` minted.
    pub(crate) session_id: SessionId,
    /// The id of the outstanding `session/prompt`: its Response is the terminal result.
    pub(crate) prompt_id: JsonRpcId,
    /// How `session/request_permission` is answered.
    pub(crate) policy: PermissionPolicy,
    /// The `(workflow, activity, attempt)` identity stamped on every event.
    pub(crate) identity: EventIdentity,
    /// Notifications that arrived during the pre-prompt handshake, replayed onto the transcript
    /// first so nothing read before the pump existed is lost.
    pub(crate) deferred: Vec<JsonRpcNotification>,
    /// The spawned agent. `None` for the in-memory test path, which drives a duplex.
    pub(crate) child: Option<Child>,
    /// How long the agent may take to exit on its own after its stdin closes. See
    /// [`crate::AcpHarness::new`].
    pub(crate) exit_grace: Duration,
}

/// A live ACP prompt turn for one activity attempt.
///
/// Generic over the child's read/write halves so tests can drive a real [`JsonRpcConnection`] over an
/// in-memory duplex; in production the halves are the agent subprocess's stdout/stdin.
pub struct AcpSession<R, W> {
    /// The connection, held in an [`Option`] because closing the agent's stdin means dropping the
    /// LAST reference to it — that is the only in-band way to tell an ACP agent the conversation is
    /// over, and it cannot be done while the field still owns one.
    connection: Option<Arc<JsonRpcConnection<R, W>>>,
    capabilities: InterventionCapabilities,
    session_id: SessionId,
    events: Option<mpsc::UnboundedReceiver<ActivityEvent>>,
    outcome: Option<oneshot::Receiver<PumpEnd>>,
    /// Raised before `session/cancel` is written; makes the pump answer later permission requests
    /// with the `cancelled` outcome, as the spec requires of a client that cancels.
    cancelled: Arc<AtomicBool>,
    pump: Option<tokio::task::JoinHandle<()>>,
    /// The spawned agent, shut down when the result is taken or the session is dropped. `None` for
    /// the in-memory test path.
    child: Option<Child>,
    /// The grace the agent gets to exit on its own once its stdin has closed.
    exit_grace: Duration,
}

impl<R, W> AcpSession<R, W>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    /// Builds a session over a connection whose handshake is already done and whose
    /// `session/prompt` request is already in flight, and starts the reader pump.
    ///
    /// The pump starts here so events stream from the moment the session exists.
    pub(crate) fn start(start: SessionStart<R, W>) -> Self {
        let SessionStart {
            connection,
            session_id,
            prompt_id,
            policy,
            identity,
            deferred,
            child,
            exit_grace,
        } = start;
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (outcome_tx, outcome_rx) = oneshot::channel();
        let cancelled = Arc::new(AtomicBool::new(false));

        let pump = tokio::spawn(pump_loop(
            PumpContext {
                connection: Arc::clone(&connection),
                session_id: session_id.clone(),
                prompt_id,
                policy,
                cancelled: Arc::clone(&cancelled),
                identity,
            },
            deferred,
            event_tx,
            outcome_tx,
        ));

        Self {
            connection: Some(connection),
            capabilities: translate::negotiated_capabilities(),
            session_id,
            events: Some(event_rx),
            outcome: Some(outcome_rx),
            cancelled,
            pump: Some(pump),
            child,
            exit_grace,
        }
    }

    /// Awaits the pump's single terminal report and interprets it.
    ///
    /// Split from [`AgentSession::wait_result`] so the agent process is shut down on **every** path,
    /// including the failing ones.
    async fn terminal_result(&mut self) -> Result<Payload, HarnessError> {
        let receiver = self
            .outcome
            .take()
            .ok_or_else(|| HarnessError::protocol("the prompt result was already awaited"))?;
        // A dropped sender means the pump task itself ended without reporting — it panicked, or it
        // was aborted. The message says exactly that and asserts nothing about the channel, which
        // may well still be open with the agent's answer unread.
        let end = receiver.await.map_err(|_recv| {
            HarnessError::transport(
                "the ACP reader pump ended without reporting a terminal outcome: the pump task did \
                 not complete (it panicked, or the session was shut down), so no session/prompt \
                 answer was ever interpreted",
            )
        })?;
        end.into_payload(&self.session_id)
    }

    /// Stops the pump, closes the agent's stdin, and gives it the configured grace to exit before
    /// killing it.
    ///
    /// A SUCCESSFUL turn ends here too, which is why the grace exists: an agent that is
    /// `SIGKILL`ed the instant it answers gets no chance to flush a log, close a file it wrote, or
    /// release a lock. The order is the only one that works:
    ///
    /// 1. **stop the pump** — it holds a second reference to the connection, and the in-flight
    ///    peer-request answers it owns die with it (the turn is over and the process is going),
    /// 2. **drop the last connection reference** — that drops the child's stdin, and stdin EOF is
    ///    the only in-band signal ACP has for "the conversation is over",
    /// 3. **wait, bounded by the configured grace** — a worker must not be hung by an agent that
    ///    ignores EOF,
    /// 4. **kill and reap** whatever is still alive.
    ///
    /// One honest caveat on step 2: the pump's answer-writer tasks each hold a reference too, and
    /// aborting them releases those references when the scheduler drops the tasks rather than
    /// synchronously here. In the ordinary case there are none in flight and the stdin close is
    /// immediate; where one is, the close lands a scheduler tick later, which the grace absorbs
    /// because the wait is on the child's exit and not on a deadline for the close.
    async fn shut_down_agent(&mut self) {
        if let Some(pump) = self.pump.take() {
            pump.abort();
            match pump.await {
                Ok(()) => {}
                // The expected outcome of the abort above.
                Err(error) if error.is_cancelled() => {}
                Err(error) => {
                    tracing::error!(%error, "acp adapter: the reader pump task panicked");
                }
            }
        }
        drop(self.connection.take());
        let Some(mut child) = self.child.take() else {
            return;
        };
        if self.exit_grace.is_zero() {
            kill_and_reap(&mut child, "no exit grace is configured for this harness").await;
            return;
        }
        match tokio::time::timeout(self.exit_grace, child.wait()).await {
            Ok(Ok(status)) => tracing::debug!(
                ?status,
                "acp adapter: the agent exited on its own after its stdin closed"
            ),
            Ok(Err(source)) => tracing::warn!(
                %source,
                "acp adapter: could not wait for the agent to exit; it may still be running"
            ),
            Err(_elapsed) => {
                kill_and_reap(
                    &mut child,
                    "the agent did not exit within the configured grace after its stdin closed",
                )
                .await;
            }
        }
    }
}

#[async_trait]
impl<R, W> AgentSession for AcpSession<R, W>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    fn capabilities(&self) -> &InterventionCapabilities {
        &self.capabilities
    }

    fn events(&mut self) -> BoxStream<'static, ActivityEvent> {
        // Owned by the returned stream, so taking it never borrows the session: the driver pumps
        // events while also calling `intervene` and `wait_result`. Taken once; a second call yields
        // an empty stream rather than panicking.
        match self.events.take() {
            Some(receiver) => Box::pin(futures::stream::unfold(
                receiver,
                |mut receiver| async move { receiver.recv().await.map(|event| (event, receiver)) },
            )),
            None => Box::pin(futures::stream::empty()),
        }
    }

    async fn intervene(&self, cmd: InterventionCommand) -> Result<(), HarnessError> {
        // Capability-gate FIRST, against the set this session ADVERTISED: an unadvertised primitive
        // is rejected before anything is written, so the four primitives ACP cannot deliver never
        // reach the wire even if a caller ignores the advertisement.
        if !self.capabilities.supports(&cmd.kind) {
            return Err(HarnessError::capability_not_supported(
                translate::primitive_label(cmd.kind.primitive()),
            ));
        }
        // `Cancel` carries an obligation beyond its frame: from the moment a client cancels it must
        // answer every pending `session/request_permission` with `cancelled`, so the flag is raised
        // BEFORE the notification is written. It is done here, at the decision, rather than inside a
        // send helper, so the ordering the spec requires is visible where the choice is made.
        if let InterventionKind::Cancel { reason } = &cmd.kind {
            self.cancelled.store(true, Ordering::SeqCst);
            tracing::info!(
                session = %self.session_id,
                reason,
                "acp adapter: cancelling the prompt turn"
            );
        }
        // The translation is the single authority on what this adapter can put on the wire, so a
        // primitive with no ACP mechanism is refused BY it, by name — there is no second list here
        // that could drift from the one the published capability set is pinned against.
        let notification = translate::intervention_to_notification(&self.session_id, &cmd.kind)?;
        self.send(notification).await
    }

    async fn wait_result(mut self) -> Result<Payload, HarnessError> {
        let result = self.terminal_result().await;
        // On EVERY path, including a failure: the agent gets its orderly shutdown, and the process
        // is reaped rather than left for `Drop` to signal blindly.
        self.shut_down_agent().await;
        result
    }
}

impl<R, W> AcpSession<R, W>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    /// Writes one translated intervention frame to the agent.
    ///
    /// Intervening on a session whose result has already been taken is a stale target, not a
    /// transport fault: the turn is over and the connection was deliberately released.
    async fn send(&self, notification: translate::AcpNotification) -> Result<(), HarnessError> {
        let connection = self.connection.as_ref().ok_or_else(|| {
            HarnessError::stale_target(
                "the ACP session has already been shut down; there is no turn left to intervene in",
            )
        })?;
        let frame = JsonRpcNotification::new(notification.method, Some(notification.params));
        connection.send_notification(&frame).await
    }
}

/// Kills the agent and reaps it, saying why.
///
/// [`Child::kill`] is `start_kill` **plus** the wait that reaps the zombie, which is what makes this
/// a shutdown rather than a signal. A failure here is the only evidence that the kill did not land,
/// so it is logged rather than discarded.
async fn kill_and_reap(child: &mut Child, why: &str) {
    if let Err(source) = child.kill().await {
        tracing::warn!(
            %source,
            why,
            "acp adapter: could not kill and reap the agent process"
        );
        return;
    }
    tracing::debug!(why, "acp adapter: the agent process was killed and reaped");
}

impl<R, W> Drop for AcpSession<R, W> {
    fn drop(&mut self) {
        // The orderly shutdown lives in `shut_down_agent`, which needs to await and so cannot run
        // here. This is the abandoned path — a session dropped without its result being taken — and
        // its only job is to leave no task and no process running.
        if let Some(pump) = self.pump.take() {
            pump.abort();
        }
        if let Some(mut child) = self.child.take() {
            // `start_kill` INITIATES a kill; it does not reap. The reap is tokio's own: the command
            // was built with `kill_on_drop(true)`, so dropping `child` at the end of this scope
            // performs it. A failure here means the signal did not reach the process at all (a
            // child that has already exited reports success), which must not be silent.
            if let Err(source) = child.start_kill() {
                tracing::warn!(
                    %source,
                    "acp adapter: could not signal the agent process while dropping the session"
                );
            }
        }
    }
}

/// Answers one peer request received OUTSIDE the pump — during the pre-prompt handshake — with the
/// same `-32601` refusal the pump uses.
///
/// The handshake reads frames before a session exists, and a conforming agent sends no request
/// there. A non-conforming one must still get an answer: an unanswered request would block it before
/// the run had even started.
///
/// # Errors
///
/// Returns the transport error when the refusal cannot be written.
pub(crate) async fn refuse_peer_request<R, W>(
    connection: &JsonRpcConnection<R, W>,
    request: JsonRpcRequest,
) -> Result<(), HarnessError>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    tracing::warn!(
        method = %request.method,
        "acp adapter: refusing a peer request received during the handshake"
    );
    let refusal = protocol::unadvertised_method_response(request.id, &request.method);
    connection.send_response(&refusal).await
}
