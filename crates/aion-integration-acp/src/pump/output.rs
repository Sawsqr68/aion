//! Non-blocking peer-response writes and transcript event emission.

use std::sync::Arc;

use crate::translate::EventKind;
use tokio::io::{AsyncRead, AsyncWrite};

use super::{JsonRpcResponse, PumpContext, PumpState};

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
pub(super) fn queue_response<R, W>(
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
pub(super) fn emit<R, W>(
    context: &PumpContext<R, W>,
    state: &mut PumpState,
    kinds: Vec<EventKind>,
) {
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
