//! The transcript serving boundary's envelope reconstruction.
//!
//! Provider response envelopes are persisted compacted: a turn's first frame in full, its later
//! frames as deltas against it (see `aion_integrations::envelope_delta` for why, and for the
//! measurements that motivated it). That is a **storage** representation. Every surface that
//! serves a transcript — the WebSocket stream, the REST fetch, the MCP transcript tool, the
//! live-describe projection — reconstructs the full envelopes here, so no consumer downstream of
//! this boundary can tell a compacted stream from an uncompacted one.
//!
//! Reconstructing in one place, at the edge, is deliberate. The alternative is every consumer
//! learning the delta format — the CLI, the ops console's TypeScript, the MCP tool, and anything
//! written later — which is four implementations of one format and four chances for them to drift.
//!
//! # Order matters, and it is the caller's job
//!
//! A delta resolves only against a base the decoder has already been shown. Feed a decoder the
//! records of ONE stream in ascending `store_seq`, which is the order every read path here already
//! produces, and resolve BEFORE windowing rather than after: a window that opens mid-turn would
//! otherwise discard the very base its first delta needs.
//!
//! # What cannot be reconstructed is said, never guessed
//!
//! A window that genuinely begins after its turn's base — because retention trimmed the base, or
//! because the caller resumed from a cursor inside a turn — yields a delta this boundary cannot
//! resolve. Those are returned to the caller, which logs them; the event is served with its delta
//! document intact, carrying every field that actually changed. Nothing is fabricated, and nothing
//! is dropped.

use aion_core::ActivityEvent;
use aion_integrations::envelope_delta::{EnvelopeDeltaDecoder, ResolvedEnvelope, UnresolvedDelta};
use aion_store::ActivityRecord;

/// Reconstructs one event in place, returning the report when a delta could not be resolved.
///
/// For the streaming paths, which see one event at a time and must hold their own decoder for the
/// life of the socket so a live frame can resolve against a base delivered during replay.
pub(crate) fn resolve_event(
    decoder: &mut EnvelopeDeltaDecoder,
    event: &mut ActivityEvent,
) -> Option<UnresolvedDelta> {
    match decoder.resolve(event) {
        ResolvedEnvelope::Verbatim => None,
        ResolvedEnvelope::Reconstructed(envelope) => {
            if let aion_core::ActivityEventKind::Raw { value, .. } = &mut event.kind {
                *value = envelope;
            }
            None
        }
        ResolvedEnvelope::Unresolved(unresolved) => Some(unresolved),
    }
}

/// Reconstructs a whole read of one stream in place, returning every delta it could not resolve.
///
/// For the windowed paths, which read a range and then narrow it. Call this on the records as
/// read, before any narrowing.
pub(crate) fn resolve_records(records: &mut [ActivityRecord]) -> Vec<UnresolvedDelta> {
    let mut decoder = EnvelopeDeltaDecoder::new();
    let mut unresolved = Vec::new();
    for record in records {
        if let Some(report) = resolve_event(&mut decoder, &mut record.event) {
            unresolved.push(report);
        }
    }
    unresolved
}

/// Records the deltas a read could not reconstruct.
///
/// A window opening inside a turn is ordinary and expected, so this is a debug-level note naming
/// the stream and each unresolvable base — enough to explain an operator's "why is this frame a
/// delta document?" without turning a routine window boundary into a warning.
pub(crate) fn note_unresolved(surface: &str, unresolved: &[UnresolvedDelta]) {
    for report in unresolved {
        tracing::debug!(
            surface,
            base = %report.base,
            base_worker_seq = ?report.base_worker_seq,
            "{report}"
        );
    }
}

#[cfg(test)]
mod tests;
