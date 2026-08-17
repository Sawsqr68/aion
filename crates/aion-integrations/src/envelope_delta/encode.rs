//! Compacting a repeated provider envelope into a delta against its turn's base.

use std::collections::HashMap;

use aion_core::{ActivityEvent, ActivityEventKind};
use serde_json::{Value, json};

use super::patch;
use super::slot::{RecordedBase, StreamSlot};
use super::wire::{self, ENVELOPE_DELTA_KEY, ENVELOPE_DELTA_VERSION};

/// What the encoder did with one event, so the caller can record it.
///
/// Every arm is reported, including the ones where nothing was saved. A compaction pass that
/// quietly declined to compact would be indistinguishable from one that was never reached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Compaction {
    /// The event is not a provider response envelope (or is ephemeral, and so is never
    /// persisted): untouched.
    NotApplicable,
    /// The first frame of a turn. Persisted in full and remembered as that turn's base.
    BaseRecorded {
        /// The provider response identifier the base was filed under.
        base: String,
    },
    /// A later frame of a turn, persisted as a delta against the base.
    Compacted {
        /// The provider response identifier the delta refers to.
        base: String,
        /// Serialized bytes the frame would have cost persisted in full.
        full_bytes: usize,
        /// Serialized bytes the delta document costs instead.
        delta_bytes: usize,
    },
    /// A later frame of a turn that was persisted in full anyway, and why.
    ///
    /// This is the safe outcome, not the failure outcome: the stream is exactly what it would have
    /// been without this module.
    FullRetained {
        /// The provider response identifier the frame belongs to.
        base: String,
        /// Why the delta was declined.
        reason: FullRetainedReason,
    },
}

/// Why an eligible frame was persisted in full rather than as a delta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FullRetainedReason {
    /// Applying the delta back to the base did not reproduce the frame byte for byte. The encoder
    /// refuses to persist a delta it cannot prove reversible, whatever the saving would have been.
    NotByteReversible,
    /// The delta document is not smaller than the frame it would replace, so compacting it would
    /// cost bytes rather than save them.
    NotSmaller {
        /// Serialized bytes of the frame.
        full_bytes: usize,
        /// Serialized bytes the delta document would have cost.
        delta_bytes: usize,
    },
    /// The frame or its base could not be serialized, so neither the digest nor the reversibility
    /// check could be performed.
    NotSerializable,
}

/// Compacts a harness adapter's outgoing provider envelopes, one stream's turns at a time.
///
/// Hold one per session. It remembers a single base envelope per `(stream, agent)`; see
/// [`RecordedBase`] for why that is both sufficient and bounded.
#[derive(Debug, Default)]
pub struct EnvelopeDeltaEncoder {
    bases: HashMap<StreamSlot, RecordedBase>,
}

impl EnvelopeDeltaEncoder {
    /// A encoder holding no bases.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Rewrites `event`'s raw value in place when it is a compactible provider envelope frame.
    ///
    /// The returned [`Compaction`] says what happened; the event is modified only in the
    /// [`Compaction::Compacted`] case.
    pub fn compact(&mut self, event: &mut ActivityEvent) -> Compaction {
        // Ephemeral events are forwarded live and never persisted, so compacting one would trade
        // nothing for the risk of a reader meeting a delta whose base was never durable.
        if event.ephemeral {
            return Compaction::NotApplicable;
        }
        let slot = StreamSlot::of(event);
        let worker_seq = event.worker_seq;
        let ActivityEventKind::Raw { value, .. } = &mut event.kind else {
            return Compaction::NotApplicable;
        };
        // A value that is already a delta document is not re-compacted; the encoder is the only
        // writer of them, so this can only mean the same event was passed twice.
        if wire::delta_document(value).is_some() {
            return Compaction::NotApplicable;
        }
        let Some(response_id) = wire::envelope_response_id(value) else {
            return Compaction::NotApplicable;
        };
        let response_id = response_id.to_owned();

        match self.bases.get(&slot) {
            Some(base) if base.response_id == response_id => match build_delta(base, value) {
                Ok(delta) => {
                    *value = delta.document;
                    Compaction::Compacted {
                        base: response_id,
                        full_bytes: delta.full_bytes,
                        delta_bytes: delta.delta_bytes,
                    }
                }
                Err(reason) => Compaction::FullRetained {
                    base: response_id,
                    reason,
                },
            },
            // Either the first frame this stream has produced, or the first frame of a new turn:
            // the previous turn's base is dropped by the same insert.
            _ => match RecordedBase::record_parts(&response_id, worker_seq, value) {
                Some(base) => {
                    self.bases.insert(slot, base);
                    Compaction::BaseRecorded { base: response_id }
                }
                None => Compaction::FullRetained {
                    base: response_id,
                    reason: FullRetainedReason::NotSerializable,
                },
            },
        }
    }
}

/// A delta the encoder has proved reversible, with the byte counts that justify it.
struct BuiltDelta {
    document: Value,
    full_bytes: usize,
    delta_bytes: usize,
}

/// Builds the delta document for `frame` against `base`, refusing anything it cannot prove.
fn build_delta(base: &RecordedBase, frame: &Value) -> Result<BuiltDelta, FullRetainedReason> {
    let (Value::Object(base_object), Value::Object(frame_object)) = (&base.value, frame) else {
        return Err(FullRetainedReason::NotByteReversible);
    };
    let diff = patch::diff(base_object, frame_object);
    let document = json!({
        ENVELOPE_DELTA_KEY: {
            "v": ENVELOPE_DELTA_VERSION,
            "base": base.response_id,
            "base_worker_seq": base.worker_seq,
            "base_digest": base.digest,
            "set": Value::Object(diff.set.clone()),
            "unset": diff.unset.clone(),
        }
    });

    // The reversibility proof. `Value` equality ignores object key order, so this compares the
    // serialized bytes — the thing a reader will actually render — and not merely the structures.
    let reconstructed =
        patch::apply(&base.value, &diff).map_err(|_| FullRetainedReason::NotByteReversible)?;
    let (Ok(frame_bytes), Ok(reconstructed_bytes)) = (
        serde_json::to_vec(frame),
        serde_json::to_vec(&reconstructed),
    ) else {
        return Err(FullRetainedReason::NotSerializable);
    };
    if frame_bytes != reconstructed_bytes {
        return Err(FullRetainedReason::NotByteReversible);
    }

    let Ok(document_bytes) = serde_json::to_vec(&document) else {
        return Err(FullRetainedReason::NotSerializable);
    };
    let full_bytes = frame_bytes.len();
    let delta_bytes = document_bytes.len();
    if delta_bytes >= full_bytes {
        return Err(FullRetainedReason::NotSmaller {
            full_bytes,
            delta_bytes,
        });
    }
    Ok(BuiltDelta {
        document,
        full_bytes,
        delta_bytes,
    })
}
