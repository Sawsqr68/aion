//! Reconstructing a persisted delta back into the envelope it was diffed from.

use std::collections::HashMap;
use std::fmt;

use aion_core::{ActivityEvent, ActivityEventKind};
use serde_json::Value;

use super::patch::{self, Patch};
use super::slot::{RecordedBase, StreamSlot};
use super::wire::{self, ENVELOPE_DELTA_VERSION, EnvelopeDelta};

/// What a reader should use in place of one event's persisted raw value.
#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedEnvelope {
    /// The event carries no delta. Its persisted value stands as-is — this is every non-`Raw`
    /// event, every pre-change full envelope, and every turn's base frame.
    Verbatim,
    /// A delta resolved against its base. The carried value is byte-identical to the envelope the
    /// frame would have carried had it never been compacted.
    Reconstructed(Value),
    /// A delta that could not be resolved, and why. The event's persisted value is untouched and
    /// still carries every field that actually changed, so a reader can show the delta itself
    /// rather than nothing.
    Unresolved(UnresolvedDelta),
}

/// A delta the reader declined to reconstruct.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnresolvedDelta {
    /// The provider response identifier of the turn, when the document named one.
    pub base: String,
    /// The `worker_seq` of the base record, when the document named one.
    pub base_worker_seq: Option<u64>,
    /// Why reconstruction was declined.
    pub reason: UnresolvedReason,
}

impl fmt::Display for UnresolvedDelta {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "envelope delta for turn {:?}", self.base)?;
        if let Some(worker_seq) = self.base_worker_seq {
            write!(formatter, " (base at worker_seq {worker_seq})")?;
        }
        write!(formatter, " could not be resolved: {}", self.reason)
    }
}

/// Why one delta could not be turned back into an envelope.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum UnresolvedReason {
    /// The base record is not in what this reader has seen — it fell outside the requested window,
    /// or retention trimmed it. Nothing is wrong; the reader simply does not hold the bytes.
    #[error(
        "its base record was not in the range this reader has seen (trimmed by retention, or before the requested window)"
    )]
    BaseNotSeen,
    /// A base for this turn was seen, but its bytes are not the bytes the delta was built against
    /// — for instance because the base event was truncated by the server's per-event size ceiling
    /// after the encoder read it.
    #[error(
        "the base record present here has digest {observed}, but the delta was built against {expected}"
    )]
    BaseDigestMismatch {
        /// The digest the delta names.
        expected: String,
        /// The digest of the base this reader holds.
        observed: String,
    },
    /// The document names a format version this reader does not implement.
    #[error("it declares delta format version {version}, and this reader implements {supported}")]
    UnsupportedVersion {
        /// The version the document declares.
        version: u64,
        /// The version this reader implements.
        supported: u64,
    },
    /// The document is not a well-formed delta.
    #[error("the delta document is malformed: {detail}")]
    Malformed {
        /// What was wrong with it.
        detail: String,
    },
    /// The delta did not apply cleanly to the base, which means it does not belong to it.
    #[error("the delta did not apply to the base: {detail}")]
    PatchFailed {
        /// The patch error, rendered.
        detail: String,
    },
}

/// Reconstructs compacted provider envelopes as a transcript is read back.
///
/// Hold one per served stream and feed it every event **in the order the reader will present
/// them**: a base is only available to the deltas that follow it. It remembers a single base per
/// `(stream, agent)`, mirroring [`EnvelopeDeltaEncoder`](super::EnvelopeDeltaEncoder) exactly.
#[derive(Debug, Default)]
pub struct EnvelopeDeltaDecoder {
    bases: HashMap<StreamSlot, RecordedBase>,
}

impl EnvelopeDeltaDecoder {
    /// A decoder holding no bases.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolves one event, recording it as a base when it is one.
    pub fn resolve(&mut self, event: &ActivityEvent) -> ResolvedEnvelope {
        let slot = StreamSlot::of(event);
        let ActivityEventKind::Raw { value, .. } = &event.kind else {
            return ResolvedEnvelope::Verbatim;
        };
        if let Some(document) = wire::delta_document(value) {
            return self.resolve_delta(&slot, document);
        }
        let Some(response_id) = wire::envelope_response_id(value) else {
            return ResolvedEnvelope::Verbatim;
        };
        // An ephemeral frame is never persisted, so the encoder never recorded it as a base and a
        // durable delta can never refer to one. Recording it here would break that symmetry: an
        // ephemeral frame of a DIFFERENT turn would displace the base a durable delta still needs.
        // The two halves must agree on what a base is, exactly.
        if event.ephemeral {
            return ResolvedEnvelope::Verbatim;
        }
        self.record_base(slot, response_id, event.worker_seq, value);
        ResolvedEnvelope::Verbatim
    }

    /// Returns a copy of the event with any delta replaced by the envelope it reconstructs to.
    ///
    /// This is the form the server's transcript serving boundary uses: downstream consumers keep
    /// receiving exactly the full envelopes they received before compaction existed. An
    /// unresolvable delta is left in place — never fabricated — and reported to the caller so it
    /// can be logged where a delta going unresolved is worth knowing about.
    pub fn resolve_event(
        &mut self,
        event: &ActivityEvent,
    ) -> (ActivityEvent, Option<UnresolvedDelta>) {
        match self.resolve(event) {
            ResolvedEnvelope::Verbatim => (event.clone(), None),
            ResolvedEnvelope::Reconstructed(envelope) => {
                let mut resolved = event.clone();
                if let ActivityEventKind::Raw { value, .. } = &mut resolved.kind {
                    *value = envelope;
                }
                (resolved, None)
            }
            ResolvedEnvelope::Unresolved(unresolved) => (event.clone(), Some(unresolved)),
        }
    }

    /// Remembers a full envelope as its turn's base.
    ///
    /// A base already held for the same turn is kept rather than overwritten, because the encoder
    /// diffs every one of a turn's frames against the turn's *first* frame. A frame bearing a new
    /// turn displaces the old base, which is what bounds what a decoder retains.
    fn record_base(&mut self, slot: StreamSlot, response_id: &str, worker_seq: u64, value: &Value) {
        if self
            .bases
            .get(&slot)
            .is_some_and(|held| held.response_id == response_id)
        {
            return;
        }
        if let Some(base) = RecordedBase::record_parts(response_id, worker_seq, value) {
            self.bases.insert(slot, base);
        }
    }

    /// Reconstructs one delta document, or says why it could not.
    fn resolve_delta(&self, slot: &StreamSlot, document: &Value) -> ResolvedEnvelope {
        let delta: EnvelopeDelta = match serde_json::from_value(document.clone()) {
            Ok(delta) => delta,
            Err(error) => {
                return unresolved(
                    document
                        .get("base")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                    document.get("base_worker_seq").and_then(Value::as_u64),
                    UnresolvedReason::Malformed {
                        detail: error.to_string(),
                    },
                );
            }
        };
        if delta.version != ENVELOPE_DELTA_VERSION {
            return unresolved(
                &delta.base,
                Some(delta.base_worker_seq),
                UnresolvedReason::UnsupportedVersion {
                    version: delta.version,
                    supported: ENVELOPE_DELTA_VERSION,
                },
            );
        }
        let Some(base) = self
            .bases
            .get(slot)
            .filter(|held| held.response_id == delta.base)
        else {
            return unresolved(
                &delta.base,
                Some(delta.base_worker_seq),
                UnresolvedReason::BaseNotSeen,
            );
        };
        if base.digest != delta.base_digest {
            return unresolved(
                &delta.base,
                Some(delta.base_worker_seq),
                UnresolvedReason::BaseDigestMismatch {
                    expected: delta.base_digest,
                    observed: base.digest.clone(),
                },
            );
        }
        let reconstruction = patch::apply(
            &base.value,
            &Patch {
                set: delta.set,
                unset: delta.unset,
            },
        );
        match reconstruction {
            Ok(envelope) => ResolvedEnvelope::Reconstructed(envelope),
            Err(error) => unresolved(
                &delta.base,
                Some(delta.base_worker_seq),
                UnresolvedReason::PatchFailed {
                    detail: error.to_string(),
                },
            ),
        }
    }
}

/// Builds the unresolved arm.
fn unresolved(
    base: &str,
    base_worker_seq: Option<u64>,
    reason: UnresolvedReason,
) -> ResolvedEnvelope {
    ResolvedEnvelope::Unresolved(UnresolvedDelta {
        base: base.to_owned(),
        base_worker_seq,
        reason,
    })
}
