//! Envelope-delta persistence for repeated provider response envelopes.
//!
//! # The defect this addresses
//!
//! A provider streaming one LLM turn emits several lifecycle frames that each carry the *whole*
//! response envelope — `response.created`, `response.in_progress`, `response.completed` — and the
//! envelope carries the turn's entire instructions block every time. Measured on a real run, the
//! three copies are ~82 KB each and account for ~91% of a transcript's bytes before any storage
//! amplifier is considered. The adapter passes those frames through verbatim as
//! [`ActivityEventKind::Raw`], so all three copies become permanent store.
//!
//! # The shape of the fix
//!
//! Events stay events. What changes is what the emitter puts *in* one: the first frame of a turn
//! is persisted verbatim and remembered as that turn's **base**; every later frame of the same
//! turn is persisted as a **delta** against it — the fields that actually changed, plus the paths
//! that went away. The instructions block, the tool list, and every other unchanged field are
//! stored once per turn instead of once per frame.
//!
//! # Lossless by construction, not by assertion
//!
//! [`EnvelopeDeltaEncoder`] never emits a delta it has not itself proved reversible: it applies
//! the delta back to the base and compares the reconstruction to the original frame **byte for
//! byte**. A frame whose delta does not reproduce it exactly (or whose delta is not smaller than
//! the frame) is persisted in full, exactly as before this module existed. Compaction is therefore
//! incapable of changing what a reader sees; the worst case is that it saves nothing.
//!
//! [`EnvelopeDeltaDecoder`] is the inverse, and it is equally unwilling to guess. A delta whose
//! base it has not seen — trimmed by retention, or simply before the window the reader asked for —
//! is reported as [`ResolvedEnvelope::Unresolved`] naming the base and the reason. It is never
//! silently rendered as though it were an envelope, and the delta document itself is never
//! discarded, so nothing is lost even when reconstruction is impossible.
//!
//! # Where this runs
//!
//! The encoder runs in the harness adapter, on the way out — the only place that sees a provider
//! frame before it is persisted. The decoder runs at the server's transcript **serving** boundary,
//! so every downstream consumer (the CLI's `tail`, the ops console, the MCP transcript tool, the
//! live-describe projection) keeps receiving exactly the full envelopes it received before this
//! module existed. Reconstruction happens once, in one place, rather than once per consumer.
//!
//! # Mixed history is permanent
//!
//! Streams written before this module exists carry only full envelopes; streams written after
//! carry a base plus deltas; a stream that spans the change carries both. All three shapes are
//! first-class here — a full envelope is recorded as a base and passed through, so an old-format
//! region needs no special case and a delta that follows old-format frames resolves against them.

/// Reconstructing a persisted delta back into the envelope it was diffed from.
pub mod decode;
/// Compacting a repeated provider envelope into a delta against its turn's base.
pub mod encode;
/// The structural JSON diff and its inverse.
mod patch;
/// The identity a turn's base envelope is remembered under.
mod slot;
/// The persisted delta document and the keys that name it on the wire.
pub mod wire;

pub use decode::{EnvelopeDeltaDecoder, ResolvedEnvelope, UnresolvedDelta, UnresolvedReason};
pub use encode::{Compaction, EnvelopeDeltaEncoder, FullRetainedReason};
pub use wire::{ENVELOPE_DELTA_KEY, ENVELOPE_DELTA_VERSION, EnvelopeDelta};

#[cfg(test)]
mod tests;
