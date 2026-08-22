//! [`NornSession`] — a live Norn `--protocol jsonrpc` run behind the [`AgentSession`] seam.
//!
//! One background **pump task** owns the connection's single reader and demultiplexes every
//! inbound frame off the shared JSON-RPC channel:
//!
//! - a `event/*` **notification** is translated ([`crate::translate::notification_to_event`]) and
//!   forwarded to the [`AgentSession::events`] stream — it has no `id`, so it can NEVER be
//!   captured as the result,
//! - a **Response** is routed by its `id` to the one waiter that sent that request — the
//!   `run/execute` waiter (the terminal result) or an `intervene/*` waiter (its ack),
//! - a child-initiated **Request** (e.g. a future `approval/*`) is outside NOI-4 scope and is
//!   traced and dropped rather than answered.
//!
//! This id-routing is what makes the result/event split structural (§4.1): the terminal result is
//! captured ONLY as the Response whose `id` matches the `run/execute` request the harness sent.

use std::collections::HashMap;
use std::sync::Arc;

use aion_core::{ActivityEvent, InterventionCapabilities, InterventionCommand, Payload};
use aion_integrations::contract::AgentSession;
use aion_integrations::envelope_delta::{Compaction, EnvelopeDeltaEncoder};
use aion_integrations::jsonrpc::{
    IncomingMessage, JsonRpcConnection, JsonRpcId, JsonRpcRequest, JsonRpcResponse,
};
use aion_integrations::{HarnessError, PolicyRefusalDetection};
use async_trait::async_trait;
use futures::stream::BoxStream;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{Mutex, mpsc, oneshot};

use crate::translate::{self, EventIdentity};

/// The set of request-id → response-waiter registrations the pump routes Responses to.
type Waiters = Arc<Mutex<HashMap<JsonRpcId, oneshot::Sender<JsonRpcResponse>>>>;

/// A live Norn run for one activity attempt.
///
/// Generic over the child's read/write halves so tests can drive it over an in-memory duplex; in
/// production the halves are the child process's stdout/stdin.
pub struct NornSession<R, W> {
    connection: Arc<JsonRpcConnection<R, W>>,
    capabilities: InterventionCapabilities,
    events: Option<mpsc::UnboundedReceiver<ActivityEvent>>,
    waiters: Waiters,
    run_id: JsonRpcId,
    run_result: Option<oneshot::Receiver<JsonRpcResponse>>,
    pump: Option<tokio::task::JoinHandle<()>>,
    /// The spawned child, reaped when the session is dropped. `None` for the in-memory test path.
    child: Option<tokio::process::Child>,
}

impl<R, W> NornSession<R, W>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    /// Builds a session over an already-handshaked connection and starts the reader pump.
    ///
    /// `run_id` is the id of the outstanding `run/execute` request whose Response is the result.
    /// `identity` stamps the run key onto every translated event. The pump is started here so
    /// events stream from the moment the session exists.
    pub(crate) fn start(
        connection: Arc<JsonRpcConnection<R, W>>,
        capabilities: InterventionCapabilities,
        run_id: JsonRpcId,
        identity: EventIdentity,
        child: Option<tokio::process::Child>,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (result_tx, result_rx) = oneshot::channel();

        // Register the run/execute waiter into the map BEFORE the pump can observe its Response, by
        // building the map with the entry already present — no lock, no `.await` in this sync ctor.
        let mut initial = HashMap::new();
        initial.insert(run_id.clone(), result_tx);
        let waiters: Waiters = Arc::new(Mutex::new(initial));

        let pump = tokio::spawn(pump_loop(
            Arc::clone(&connection),
            Arc::clone(&waiters),
            event_tx,
            identity,
        ));

        Self {
            connection,
            capabilities,
            events: Some(event_rx),
            waiters,
            run_id,
            run_result: Some(result_rx),
            pump: Some(pump),
            child,
        }
    }
}

/// The reader pump: demultiplexes every inbound frame until end-of-stream.
///
/// Notifications become events; Responses are routed to their id's waiter; child-initiated
/// Requests are traced and dropped (out of NOI-4 scope). On a transport error or EOF the loop
/// ends, the event channel closes (ending the events stream), and any un-answered waiter observes
/// a dropped sender (a `RecvError` its awaiter maps to a transport error).
async fn pump_loop<R, W>(
    connection: Arc<JsonRpcConnection<R, W>>,
    waiters: Waiters,
    event_tx: mpsc::UnboundedSender<ActivityEvent>,
    identity: EventIdentity,
) where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    let mut worker_seq: u64 = 0;
    // One encoder for the life of the pump. A provider repeats the whole response envelope —
    // instructions block and all — in every lifecycle frame of a turn; this persists it once per
    // turn and the rest as deltas against it. It never emits a delta it has not proved reproduces
    // the frame byte for byte, so a frame it cannot compact is passed through exactly as before.
    let mut envelopes = EnvelopeDeltaEncoder::new();
    loop {
        match connection.recv().await {
            Ok(Some(IncomingMessage::Notification(note))) => {
                let params = note.params.unwrap_or(serde_json::Value::Null);
                let mut event =
                    translate::notification_to_event(&identity, worker_seq, &note.method, &params);
                record_compaction(&note.method, &envelopes.compact(&mut event));
                worker_seq = worker_seq.saturating_add(1);
                // A send failure means the consumer dropped the events stream; keep pumping so the
                // result and acks still route, just stop forwarding events. Not an error.
                let _ = event_tx.send(event);
            }
            Ok(Some(IncomingMessage::Response(response))) => {
                if let Some(sender) = waiters.lock().await.remove(&response.id) {
                    // A dropped receiver (the waiter gave up) is not an error worth surfacing.
                    let _ = sender.send(response);
                }
            }
            Ok(Some(IncomingMessage::Request(request))) => {
                tracing::debug!(
                    method = %request.method,
                    "norn adapter: ignoring child-initiated request (out of NOI-4 scope)",
                );
            }
            Ok(None) => return,
            Err(error) => {
                tracing::debug!(%error, "norn adapter: reader pump ended on transport error");
                return;
            }
        }
    }
}

/// Records what compaction did with one frame.
///
/// A frame kept whole is the safe outcome, not the failure outcome — the stream is exactly what it
/// would have been without compaction — but it is the outcome worth being able to see, because a
/// provider shape that stopped compacting would otherwise be invisible.
fn record_compaction(method: &str, compaction: &Compaction) {
    match compaction {
        Compaction::NotApplicable => {}
        Compaction::BaseRecorded { base } => {
            tracing::trace!(method, %base, "norn adapter: recorded a turn's base envelope");
        }
        Compaction::Compacted {
            base,
            full_bytes,
            delta_bytes,
        } => {
            tracing::trace!(
                method,
                %base,
                full_bytes,
                delta_bytes,
                "norn adapter: persisted a response frame as an envelope delta",
            );
        }
        Compaction::FullRetained { base, reason } => {
            tracing::debug!(
                method,
                %base,
                ?reason,
                "norn adapter: kept a response frame whole rather than compacting it",
            );
        }
    }
}

#[async_trait]
impl<R, W> AgentSession for NornSession<R, W>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    fn capabilities(&self) -> &InterventionCapabilities {
        &self.capabilities
    }

    fn events(&mut self) -> BoxStream<'static, ActivityEvent> {
        // The stream is taken once; a second call yields an empty stream rather than panicking.
        match self.events.take() {
            Some(receiver) => Box::pin(futures::stream::unfold(
                receiver,
                |mut receiver| async move { receiver.recv().await.map(|event| (event, receiver)) },
            )),
            None => Box::pin(futures::stream::empty()),
        }
    }

    async fn intervene(&self, cmd: InterventionCommand) -> Result<(), HarnessError> {
        // Capability-gate FIRST: an unadvertised primitive is rejected before any request is sent,
        // and the three unsupported primitives never even build a Norn request.
        if !self.capabilities.supports(&cmd.kind) {
            return Err(HarnessError::capability_not_supported(format!(
                "{:?}",
                cmd.kind.primitive()
            )));
        }
        let request = translate::intervention_to_request(&cmd.kind)?;

        let id = self.connection.next_request_id();
        let (ack_tx, ack_rx) = oneshot::channel();
        self.waiters.lock().await.insert(id.clone(), ack_tx);

        let frame = JsonRpcRequest::new(id.clone(), request.method, Some(request.params));
        if let Err(error) = self.connection.send_request(&frame).await {
            self.waiters.lock().await.remove(&id);
            return Err(error);
        }

        let response = ack_rx.await.map_err(|_recv| {
            HarnessError::transport("intervene ack channel closed before reply")
        })?;
        response_into_ack(response)
    }

    async fn wait_result(mut self) -> Result<Payload, HarnessError> {
        let receiver = self
            .run_result
            .take()
            .ok_or_else(|| HarnessError::protocol("run result already awaited"))?;
        let response = receiver.await.map_err(|_recv| {
            HarnessError::transport("run/execute response channel closed before reply")
        })?;
        debug_assert_eq!(
            response.id, self.run_id,
            "pump routed the run/execute response"
        );
        response_into_payload(response)
    }
}

/// Interprets an `intervene/*` Response as an ack: success is `Ok(())`, an error object surfaces.
fn response_into_ack(response: JsonRpcResponse) -> Result<(), HarnessError> {
    if let Some(error) = response.error {
        return Err(HarnessError::harness(format!(
            "intervene rejected (code {}): {}",
            error.code, error.message
        )));
    }
    Ok(())
}

/// Interprets a `run/execute` Response as the terminal [`Payload`].
///
/// The success `result` is Norn's versioned stop envelope (`envelope_version: 1`), interpreted by
/// [`translate::run_result_to_outcome`]:
///
/// - `stop.reason == "completed"` with a String `output` → the neutral [`Payload`] is the
///   canonical agent-outcome record
///   `{"text": <the output string>, "final_message": <the same final output string>,
///   "stop_reason": "end_turn", "session_id": <the envelope's own id>}` (the envelope protocol
///   defines `output` as the final value; its `stop.reason` is mapped through the neutral vocabulary),
/// - `stop.reason == "completed"` with a non-String `output` → [`HarnessError::Contract`]
///   naming the canonical agent-outcome contract and what was found — a typed, DETERMINISTIC
///   contract refusal at this boundary (terminal in the worker's mapping), never a
///   workflow-side serde decode error,
/// - any other `stop.reason` → [`HarnessError::Harness`] carrying the whole `stop` object
///   (reason plus per-variant detail) verbatim — and the partial `output`, when the envelope
///   carried one — so the caller can judge retry (or accept the partial),
/// - a `null` result (a prompt that resolved entirely to a local slash command) →
///   [`HarnessError::Harness`] — an agent activity must produce output,
/// - a result-less, error-less Response (a broken peer) → [`HarnessError::Protocol`],
/// - a non-envelope result → [`HarnessError::Protocol`] naming what was missing.
///
/// A provider policy refusal is recognised here, while the JSON-RPC error is
/// still structured. The exact `/error/code` data path is preferred; because
/// today's norn output drops that provider code, the measured adapter-owned
/// message phrase is the deliberately narrow textual fallback. Other error
/// objects surface as [`HarnessError::Harness`].
fn response_into_payload(response: JsonRpcResponse) -> Result<Payload, HarnessError> {
    if let Some(error) = response.error {
        let detail = format!(
            "run/execute failed (code {}): {}",
            error.code, error.message
        );
        if let Some(detection) = policy_refusal_detection(&error) {
            return Err(HarnessError::policy_refused(detail, detection));
        }
        return Err(HarnessError::harness(detail));
    }
    // The envelope type keeps key presence: `None` means the frame carried NO `result` key — with
    // `error` also absent that is a broken peer, not a payload. A PRESENT `"result": null`
    // (`Some(Value::Null)`) is the contract's answer for a prompt that resolved entirely to a
    // local slash command.
    let result = match response.result {
        None => {
            return Err(HarnessError::protocol(
                "run/execute response carried neither result nor error",
            ));
        }
        Some(serde_json::Value::Null) => {
            return Err(HarnessError::harness(
                "run resolved to a local slash command; no output",
            ));
        }
        Some(result) => result,
    };
    translate::run_result_to_outcome(&result)?
        .into_payload()
        .map_err(|source| HarnessError::protocol(format!("run output is not encodable: {source}")))
}

const POLICY_REFUSAL_CODE: &str = "cyber_policy";
const POLICY_REFUSAL_MESSAGE: &str = "provider rejected the request under its cybersecurity policy";

fn policy_refusal_detection(
    error: &aion_integrations::jsonrpc::JsonRpcError,
) -> Option<PolicyRefusalDetection> {
    let structured = error.data.as_ref().and_then(|data| {
        data.pointer("/error/code")
            .and_then(serde_json::Value::as_str)
    });
    if structured == Some(POLICY_REFUSAL_CODE) {
        Some(PolicyRefusalDetection::Structured)
    } else if error.message.contains(POLICY_REFUSAL_MESSAGE) {
        Some(PolicyRefusalDetection::Textual)
    } else {
        None
    }
}

impl<R, W> Drop for NornSession<R, W> {
    fn drop(&mut self) {
        // Abort the pump and reap the child so a dropped session never leaks a task or a process.
        if let Some(pump) = self.pump.take() {
            pump.abort();
        }
        if let Some(mut child) = self.child.take() {
            // A best-effort kill; the child may already have exited.
            let _ = child.start_kill();
        }
    }
}

#[cfg(test)]
mod tests;
