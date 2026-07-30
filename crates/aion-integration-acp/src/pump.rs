//! The reader pump: the one task that owns the connection's reader and handles every inbound frame.
//!
//! - a `session/update` **notification** is translated ([`crate::translate::update_to_kinds`]) and
//!   forwarded to the session's event stream — it has no `id`, so it can NEVER be captured as the
//!   result,
//! - the **Response** whose `id` matches the outstanding `session/prompt` is the terminal result, and
//!   nothing else can be,
//! - a `session/request_permission` **request** is recorded, answered from the
//!   [`PermissionPolicy`], and the decision recorded — a permission request left unanswered would
//!   block the agent until the activity timed out,
//! - any other peer **request** (`fs/*`, `terminal/*`, an unknown method) is answered with `-32601`
//!   and recorded. **Every** peer request gets an answer; silence wedges the agent.
//!
//! # The event stream ending is the end-of-run signal
//!
//! The platform's agent driver (`aion-worker`'s `spawn_agent`) pumps `events.next()` and takes the
//! terminal result only when the stream yields `None`. So the transcript sender is **dropped the
//! moment the terminal Response has been handled** — after the accumulator is flushed, the frame is
//! recorded, the `Stop` event is emitted and the outcome is sent. A real ACP agent is a persistent
//! stdio server: it answers `session/prompt` and keeps running, so waiting for the child to exit
//! before closing the stream would hang every successful turn forever.
//!
//! The loop itself keeps reading after that, so a peer request that was already on the wire when the
//! Response arrived still gets an answer. Be honest about how wide that window is: in the driver
//! path it is very narrow — the driver breaks out as soon as the stream ends, `wait_result` consumes
//! the session, and closing the session ends the child. It is a courtesy for a frame already in
//! flight, not a guaranteed window; a frame arriving after the stream closed is logged (it can no
//! longer reach the transcript) rather than silently discarded.
//!
//! # Where the loop stops, and where it does not
//!
//! It stops at **end-of-stream** or on a **transport** error, and reports which one to
//! `wait_result` through `PumpEnd` (see [`crate::outcome`]). A **protocol** error — a line that is not valid JSON-RPC — is
//! recoverable and does NOT stop it: the reader consumed exactly one line, so the framing is still
//! aligned, and a `node`-launched agent printing `Debugger listening on ws://…` between two frames
//! must not be able to destroy a turn whose answer has already been received.
//!
//! [`PermissionPolicy`]: crate::permission::PermissionPolicy

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use agent_client_protocol_schema::v1::{
    PromptResponse, RequestPermissionOutcome, RequestPermissionRequest, SessionId,
    SessionNotification,
};
use aion_core::{ActivityEvent, ActivityEventKind, StopKind};
use aion_integrations::HarnessError;
use aion_integrations::jsonrpc::{
    IncomingMessage, JsonRpcConnection, JsonRpcError, JsonRpcId, JsonRpcNotification,
    JsonRpcRequest, JsonRpcResponse,
};
use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinSet;

use crate::messages::MessageAccumulator;
use crate::outcome::{PromptOutcome, PumpEnd};
use crate::permission::PermissionPolicy;
use crate::protocol;
use crate::translate::{self, EventIdentity, EventKind};

/// The `Raw` source label for a line the transport could not read as a JSON-RPC frame.
///
/// Not a `session/update` source: the frame never got as far as being classified, so labelling it
/// under the update namespace would claim more about it than is known.
const UNREADABLE_FRAME_SOURCE: &str = "acp/unreadable_frame";

/// The immutable context one pump task holds for the life of a turn.
///
/// Grouped into a struct rather than positional arguments so a future field cannot be wired into the
/// wrong parameter slot. Two things are deliberately NOT here: the terminal-result sender and the
/// transcript sender. Both are consumed — the first exactly once, the second at the terminal
/// Response — so they live in the mutable [`PumpState`] where dropping them is expressible.
pub(crate) struct PumpContext<R, W> {
    pub(crate) connection: Arc<JsonRpcConnection<R, W>>,
    pub(crate) session_id: SessionId,
    pub(crate) prompt_id: JsonRpcId,
    pub(crate) policy: PermissionPolicy,
    pub(crate) cancelled: Arc<AtomicBool>,
    pub(crate) identity: EventIdentity,
}

/// The mutable per-turn state the pump threads through every frame.
struct PumpState {
    /// Best-effort worker-local event sequence.
    worker_seq: u64,
    /// Streamed-message buffer: chunks in, deltas and completed messages out.
    accumulator: MessageAccumulator,
    /// The terminal-result channel, taken when the turn ends by any of the three routes.
    outcome_tx: Option<oneshot::Sender<PumpEnd>>,
    /// The transcript channel. Dropped at the terminal Response: the driver's end-of-run signal is
    /// this stream ending, so holding it past the answer hangs the run.
    event_tx: Option<mpsc::UnboundedSender<ActivityEvent>>,
    /// Peer-request answers currently being written, each on a task of its own.
    ///
    /// Owned here so they die with the pump: a session that is dropped must not leave a task
    /// writing into a child that is being killed. See [`queue_response`] for why the write cannot
    /// happen inline.
    writers: JoinSet<()>,
}

/// The reader pump: handles every inbound frame until end-of-stream.
///
/// `deferred` carries the notifications that arrived during the pre-prompt handshake, before this
/// pump existed; they are replayed first so the transcript starts with the frames the agent actually
/// sent first.
///
/// Every exit reports a [`PumpEnd`] to `wait_result` if the prompt is still unanswered, so a caller
/// never waits on a channel nobody will write to and never reads a cause that did not happen.
pub(crate) async fn pump_loop<R, W>(
    context: PumpContext<R, W>,
    deferred: Vec<JsonRpcNotification>,
    event_tx: mpsc::UnboundedSender<ActivityEvent>,
    outcome_tx: oneshot::Sender<PumpEnd>,
) where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let mut state = PumpState {
        worker_seq: 0,
        accumulator: MessageAccumulator::new(),
        outcome_tx: Some(outcome_tx),
        event_tx: Some(event_tx),
        writers: JoinSet::new(),
    };
    for note in deferred {
        handle_notification(&context, &mut state, note);
    }
    loop {
        match context.connection.recv().await {
            Ok(Some(IncomingMessage::Notification(note))) => {
                handle_notification(&context, &mut state, note);
            }
            Ok(Some(IncomingMessage::Response(response))) => {
                handle_response(&context, &mut state, response);
            }
            Ok(Some(IncomingMessage::Request(request))) => {
                handle_peer_request(&context, &mut state, request);
            }
            Ok(None) => {
                report_end(&mut state, PumpEnd::StreamEnded);
                return;
            }
            Err(HarnessError::Protocol { detail }) => {
                record_unreadable_frame(&context, &mut state, &detail);
            }
            Err(error) => {
                let detail = error.to_string();
                tracing::warn!(%detail, "acp adapter: reader pump ended on a transport error");
                report_end(&mut state, PumpEnd::TransportFailed { detail });
                return;
            }
        }
    }
}

/// Records a line the transport could not read as a JSON-RPC frame, and keeps pumping.
///
/// This is a **recoverable** condition, not the end of the run. The reader is line-oriented and has
/// already consumed the offending line, so the framing is still aligned; an agent launched through
/// `node` or `npx` that prints `Debugger listening on ws://127.0.0.1:9229` to stdout must not be
/// able to throw away an answer that has already arrived.
///
/// **Known seam limitation:** the offending line's own bytes are not recoverable here. The shared
/// [`JsonRpcConnection::recv`] reports the rejection as a [`HarnessError::Protocol`] carrying serde's
/// message (which names a position, not the content) and does not surface the line itself, so what
/// reaches the transcript is the rejection verbatim rather than the text. Recording the rejection is
/// what is available; inventing a line would be worse than recording none.
fn record_unreadable_frame<R, W>(context: &PumpContext<R, W>, state: &mut PumpState, detail: &str) {
    tracing::warn!(
        %detail,
        "acp adapter: a line on the agent's stdout is not a JSON-RPC frame; recorded and skipped"
    );
    let kind = translate::raw_kind(
        UNREADABLE_FRAME_SOURCE,
        &json!({
            "error": detail,
            "recovered": true,
            "note": "the line was skipped and the turn continued; the transport does not surface \
                     the line's own bytes",
        }),
    );
    emit(context, state, vec![kind]);
}

/// Reports how the turn ended, if it has not already been reported.
///
/// A dropped receiver means the caller abandoned `wait_result`; that is not an error worth
/// surfacing, and it is the only reason the send can fail.
fn report_end(state: &mut PumpState, end: PumpEnd) {
    if let Some(outcome_tx) = state.outcome_tx.take() {
        let _ = outcome_tx.send(end);
    }
}

/// Handles one inbound notification: `session/update` becomes transcript events, anything else is
/// recorded verbatim.
fn handle_notification<R, W>(
    context: &PumpContext<R, W>,
    state: &mut PumpState,
    note: JsonRpcNotification,
) {
    let params = note.params.unwrap_or(Value::Null);
    if note.method == protocol::METHOD_SESSION_UPDATE {
        emit_session_update(context, state, &params);
        return;
    }
    if note.method == protocol::METHOD_CANCEL_REQUEST {
        // Every inbound peer request is answered by this pump, so there is never one pending for the
        // agent to cancel. It is recorded rather than acted on.
        tracing::debug!(
            method = %note.method,
            "acp adapter: agent cancelled a request; this client holds none pending"
        );
    } else {
        tracing::warn!(method = %note.method, "acp adapter: unrecognised notification method");
    }
    emit(
        context,
        state,
        vec![translate::raw_kind(note.method.as_str(), &params)],
    );
}

/// Decodes a `session/update` notification and forwards the events it produces.
///
/// A frame that does not decode as a [`SessionNotification`] is a protocol violation on the agent's
/// side; it is recorded verbatim rather than dropped, because an operator watching a stalled run
/// needs to see the malformed frame.
fn emit_session_update<R, W>(context: &PumpContext<R, W>, state: &mut PumpState, params: &Value) {
    let notification: SessionNotification = match serde_json::from_value(params.clone()) {
        Ok(notification) => notification,
        Err(source) => {
            tracing::warn!(%source, "acp adapter: undecodable session/update params");
            let kind = translate::raw_kind(
                format!("{}/undecodable", translate::UPDATE_SOURCE_PREFIX),
                &json!({ "error": source.to_string(), "params": params }),
            );
            emit(context, state, vec![kind]);
            return;
        }
    };
    if notification.session_id != context.session_id {
        // One session per run: an update for another session cannot be attributed to this attempt's
        // transcript, but it is still evidence of an agent bug, so it is recorded verbatim.
        tracing::warn!(
            received = %notification.session_id,
            expected = %context.session_id,
            "acp adapter: session/update for an unexpected session id"
        );
        let kind = translate::raw_kind(
            format!("{}/foreign_session", translate::UPDATE_SOURCE_PREFIX),
            &notification,
        );
        emit(context, state, vec![kind]);
        return;
    }
    let kinds = translate::update_to_kinds(&notification.update, &mut state.accumulator);
    emit(context, state, kinds);
}

/// Handles one inbound Response: the id-matched `session/prompt` reply is the terminal result.
///
/// In order: flush the open messages (so the last durable message precedes the boundary), record the
/// frame verbatim, emit the terminal [`ActivityEventKind::Stop`], hand the response plus the
/// accumulated text to `wait_result`, and then **drop the transcript sender** so the events stream
/// ends — which is the driver's end-of-run signal.
///
/// A Response with any other id answers a request this adapter never sent.
fn handle_response<R, W>(
    context: &PumpContext<R, W>,
    state: &mut PumpState,
    response: JsonRpcResponse,
) {
    if response.id != context.prompt_id {
        tracing::warn!(
            received = ?response.id,
            expected = ?context.prompt_id,
            "acp adapter: response for an id this client never requested; ignoring the frame"
        );
        return;
    }
    let Some(outcome_tx) = state.outcome_tx.take() else {
        tracing::warn!(
            id = ?response.id,
            "acp adapter: a second response for the prompt id; ignoring the frame"
        );
        return;
    };
    let flushed = state.accumulator.finish();
    emit(context, state, flushed);
    // Every other frame gets a verbatim twin, and this one decides the activity's result.
    emit(
        context,
        state,
        vec![translate::prompt_response_kind(&response)],
    );
    emit(context, state, vec![stop_event_kind(&response)]);
    let outcome = PromptOutcome {
        response,
        text: state.accumulator.completed_text().to_owned(),
    };
    // A dropped receiver means the caller abandoned `wait_result`; not an error worth surfacing.
    let _ = outcome_tx.send(PumpEnd::Answered(Box::new(outcome)));
    state.event_tx = None;
}

/// The terminal [`ActivityEventKind::Stop`] for a `session/prompt` Response.
///
/// **Always emitted.** A transcript that just stops mid-stream with no terminal boundary is not an
/// honest record of a failed turn, and there is nothing to invent: `aion_core`'s
/// [`StopKind::Error`] exists for exactly the case where a run stopped because of an error. The
/// three non-success shapes each get one carrying the reason, and the frame itself is on the
/// transcript verbatim beside it ([`translate::prompt_response_kind`]).
///
/// `error` is checked before `result` so the boundary agrees with the terminal interpretation in
/// [`crate::outcome`], which reports an error Response as the failure it is regardless of what else
/// the frame carries.
fn stop_event_kind(response: &JsonRpcResponse) -> EventKind {
    let reason = if let Some(error) = response.error.as_ref() {
        StopKind::Error {
            message: format!(
                "session/prompt failed (code {}): {}",
                error.code, error.message
            ),
        }
    } else if let Some(result) = response.result.as_ref() {
        match serde_json::from_value::<PromptResponse>(result.clone()) {
            Ok(prompt) => translate::stop_kind(prompt.stop_reason),
            Err(source) => {
                tracing::warn!(%source, "acp adapter: undecodable session/prompt result");
                StopKind::Error {
                    message: format!(
                        "the session/prompt result is not an ACP PromptResponse ({source}): {result}"
                    ),
                }
            }
        }
    } else {
        StopKind::Error {
            message: "the session/prompt response carried neither result nor error".to_owned(),
        }
    };
    (ActivityEventKind::Stop { reason }, false)
}

/// Handles one peer-initiated request. Every path queues exactly one response.
fn handle_peer_request<R, W>(
    context: &PumpContext<R, W>,
    state: &mut PumpState,
    request: JsonRpcRequest,
) where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    if request.method == protocol::METHOD_SESSION_REQUEST_PERMISSION {
        answer_permission_request(context, state, request);
        return;
    }
    // Anything else is a method this client did not advertise (`fs/*`, `terminal/*`) or does not
    // know. It is refused with an answer, never with silence.
    tracing::warn!(
        method = %request.method,
        "acp adapter: refusing a peer request for an unadvertised capability"
    );
    emit(
        context,
        state,
        vec![translate::refused_request_kind(
            &request.method,
            request.params.as_ref(),
        )],
    );
    let refusal = protocol::unadvertised_method_response(request.id, &request.method);
    queue_response(context, state, refusal);
}

/// Records, decides, records again, and answers one `session/request_permission`.
///
/// The order is deliberate: the ask reaches the transcript before the answer is written, so a run
/// that dies mid-decision still shows what was asked. A request that cannot be decoded is refused
/// with `-32602` rather than left unanswered — an undecodable request is still a request the agent is
/// blocked on.
fn answer_permission_request<R, W>(
    context: &PumpContext<R, W>,
    state: &mut PumpState,
    request: JsonRpcRequest,
) where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let params = request.params.clone().unwrap_or(Value::Null);
    let decoded: RequestPermissionRequest = match serde_json::from_value(params.clone()) {
        Ok(decoded) => decoded,
        Err(source) => {
            tracing::warn!(%source, "acp adapter: undecodable session/request_permission params");
            emit(
                context,
                state,
                vec![translate::raw_kind(
                    format!(
                        "{}/undecodable",
                        protocol::METHOD_SESSION_REQUEST_PERMISSION
                    ),
                    &json!({ "error": source.to_string(), "params": params }),
                )],
            );
            let refusal = JsonRpcResponse::failure(
                request.id,
                JsonRpcError::new(
                    aion_integrations::jsonrpc::error_codes::INVALID_PARAMS,
                    format!("session/request_permission params are not decodable: {source}"),
                ),
            );
            queue_response(context, state, refusal);
            return;
        }
    };

    emit(
        context,
        state,
        vec![translate::permission_request_kind(&decoded)],
    );

    // A client that has cancelled MUST answer every permission request `cancelled`, whatever the
    // configured policy would otherwise have chosen.
    let outcome = if context.cancelled.load(Ordering::SeqCst) {
        RequestPermissionOutcome::Cancelled
    } else {
        context.policy.decide(&decoded.options)
    };
    emit(
        context,
        state,
        vec![translate::permission_decision_kind(
            context.policy,
            &decoded.tool_call.tool_call_id.to_string(),
            &decoded.options,
            &outcome,
        )],
    );

    let answer = match serde_json::to_value(RequestPermissionResponseShim::of(&outcome)) {
        Ok(result) => JsonRpcResponse::success(request.id, result),
        Err(source) => {
            // Unreachable for the two outcome shapes, but an unanswered request is unacceptable, so
            // the failure is still an ANSWER rather than a dropped frame.
            tracing::error!(%source, "acp adapter: could not encode a permission outcome");
            JsonRpcResponse::failure(
                request.id,
                JsonRpcError::new(
                    aion_integrations::jsonrpc::error_codes::INTERNAL_ERROR,
                    format!("could not encode the permission outcome: {source}"),
                ),
            )
        }
    };
    queue_response(context, state, answer);
}

/// The `session/request_permission` result envelope: `{ "outcome": <outcome> }`.
///
/// ACP wraps the outcome in an object because a JSON-RPC result must be one, and the schema's own
/// [`agent_client_protocol_schema::v1::RequestPermissionResponse`] is `#[non_exhaustive]` — it cannot
/// be constructed field-by-field from outside its crate, and its `new` takes ownership. This borrows
/// the outcome instead so the same value can be recorded on the transcript and sent on the wire
/// without a clone diverging between the two.
#[derive(serde::Serialize)]
struct RequestPermissionResponseShim<'a> {
    outcome: &'a RequestPermissionOutcome,
}

impl<'a> RequestPermissionResponseShim<'a> {
    /// Wraps one outcome for the wire.
    const fn of(outcome: &'a RequestPermissionOutcome) -> Self {
        Self { outcome }
    }
}

/// Queues one response for writing on a task of its own, and reaps the finished writers.
///
/// **The write must not be awaited in the read loop.** The shared transport holds the connection's
/// writer mutex across `write_all` + `flush`, so a child whose stdin buffer is full blocks that write
/// until it reads — and it cannot read while this loop has stopped draining its stdout. Awaiting
/// inline is therefore a real deadlock with no timeout (observed at 729 answered permission requests
/// against an agent that did not read its stdin), and one that `intervene` could not rescue, because
/// cancelling needs the same mutex. Writing off the loop keeps stdout draining, which is what lets
/// the child make progress at all.
///
/// The "exactly one answer per peer request, never two, never a mismatched id" property is
/// untouched: the response is built from that request's own id before it is queued, and it is queued
/// once. Only WHEN the bytes leave changes — never how many, nor under which id. Two answers may
/// leave in a different order than their requests arrived, which JSON-RPC allows (responses are
/// id-correlated, not positional), and the transport's writer mutex still makes each frame atomic.
///
/// Residual honesty: a child that never reads its stdin AT ALL cannot be written to by anyone — the
/// pipe is full, mutex or no mutex — so `intervene` can still block on such an agent. That is a
/// property of the pipe rather than of this adapter's locking, and the read loop no longer stalls
/// with it, so the turn's terminal result still arrives.
fn queue_response<R, W>(
    context: &PumpContext<R, W>,
    state: &mut PumpState,
    response: JsonRpcResponse,
) where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    // Reap finished writers, so the set holds only writes genuinely still in flight (bounded by the
    // agent's own outstanding-request count) rather than growing for the life of the turn.
    while let Some(joined) = state.writers.try_join_next() {
        if let Err(error) = joined {
            tracing::error!(%error, "acp adapter: a peer-request answer task did not complete");
        }
    }
    let connection = Arc::clone(&context.connection);
    state.writers.spawn(async move {
        // A failed write means the agent's stdin is gone: the run is over and the pump's next `recv`
        // will end the loop. It is logged rather than propagated because the pump has no error
        // channel — and it must not be silent.
        if let Err(error) = connection.send_response(&response).await {
            tracing::warn!(%error, "acp adapter: could not answer a peer request");
        }
    });
}

/// Stamps and forwards translated kinds, advancing the worker sequence.
///
/// A send failure means the consumer dropped the events stream; the pump keeps going so the terminal
/// result and the peer-request answers still happen. That is not an error.
///
/// Once the transcript sender has been dropped at the terminal Response, a later frame has nowhere
/// durable to go. It is logged at `warn` with its translated kind rather than discarded quietly: a
/// frame arriving after the turn's answer is an agent-conformance problem, and silence about it
/// would leave an operator with no trace of it at all.
fn emit<R, W>(context: &PumpContext<R, W>, state: &mut PumpState, kinds: Vec<EventKind>) {
    for kind in kinds {
        let event = context.identity.stamp(state.worker_seq, kind);
        state.worker_seq = state.worker_seq.saturating_add(1);
        if let Some(sender) = state.event_tx.as_ref() {
            let _ = sender.send(event);
        } else {
            tracing::warn!(
                kind = ?event.kind,
                "acp adapter: a frame arrived after the terminal session/prompt response; the \
                 transcript stream is already closed"
            );
        }
    }
}
