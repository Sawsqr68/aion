//! Unit proofs for the envelope-delta codec.
//!
//! The load-bearing claim is that compaction cannot change what a reader sees. These tests attack
//! that from both ends: round-trip equality on the shapes a provider actually emits, and explicit
//! refusals on every shape the codec must not guess at.

use aion_core::{ActivityEvent, ActivityEventKind, ActivityId, MessageRole, RunId, WorkflowId};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use super::patch::{self, Patch, PatchError};
use super::{
    Compaction, ENVELOPE_DELTA_KEY, EnvelopeDeltaDecoder, EnvelopeDeltaEncoder, FullRetainedReason,
    ResolvedEnvelope, UnresolvedReason,
};

fn fixed_time() -> DateTime<Utc> {
    DateTime::from_timestamp(1_786_895_017, 0).unwrap_or_default()
}

fn raw_event(worker_seq: u64, value: Value) -> ActivityEvent {
    ActivityEvent {
        workflow_id: WorkflowId::new(Uuid::from_u128(0x11)),
        run_id: RunId::new(Uuid::from_u128(0x22)),
        activity_id: ActivityId::from_sequence_position(7),
        attempt: 2,
        agent_id: Uuid::from_u128(0x33),
        agent_role: "orchestrator".to_owned(),
        emitted_at: fixed_time(),
        worker_seq,
        store_seq: Some(worker_seq),
        ephemeral: false,
        kind: ActivityEventKind::Raw {
            source: "event/raw".to_owned(),
            value,
        },
    }
}

/// The instructions block a turn's every frame repeats verbatim.
///
/// Sized to dominate the envelope the way the measured ones do (the forensics put a real frame at
/// ~82 KB, overwhelmingly this block), because a fixture whose invariant part is smaller than the
/// delta's own header would prove the opposite of what it claims.
fn instructions() -> String {
    "You are the implementation agent for an isolated repository lane. Inspect the repository \
     before editing. Preserve unrelated changes. Implement the bounded capability end to end, \
     add focused regression tests, run the required verification battery, and report only \
     evidence you directly observed. "
        .repeat(24)
}

fn frame(kind: &str, sequence_number: u64, status: &str, output: &Value, usage: &Value) -> Value {
    json!({
        "type": kind,
        "sequence_number": sequence_number,
        "response": {
            "id": "resp_turn_01",
            "object": "response",
            "created_at": 1_786_895_017_u64,
            "status": status,
            "model": "gpt-5.2-codex",
            "instructions": instructions(),
            "output": output,
            "parallel_tool_calls": true,
            "temperature": 1.0,
            "tool_choice": "auto",
            "tools": [{"name": "read", "type": "function"}, {"name": "bash", "type": "function"}],
            "usage": usage,
        },
        "agent_id": "00000000-0000-0000-0000-000000000033",
        "agent_role": "orchestrator",
    })
}

fn turn() -> [Value; 3] {
    [
        frame("response.created", 0, "queued", &json!([]), &Value::Null),
        frame(
            "response.in_progress",
            1,
            "in_progress",
            &json!([]),
            &Value::Null,
        ),
        frame(
            "response.completed",
            8,
            "completed",
            &json!([{"id": "msg_01", "type": "message", "role": "assistant"}]),
            &json!({"input_tokens": 1200, "output_tokens": 7}),
        ),
    ]
}

/// The fallback a non-`Raw` event yields: every caller here builds a `Raw` event, so this is
/// unreachable, and returning it fails the caller's assertion instead of aborting the run.
static NOT_RAW: Value = Value::Null;

fn raw_value(event: &ActivityEvent) -> &Value {
    match &event.kind {
        ActivityEventKind::Raw { value, .. } => value,
        _ => &NOT_RAW,
    }
}

// ---------------------------------------------------------------------------
// Round trip: what the writer emits, the reader restores byte for byte.
// ---------------------------------------------------------------------------

#[test]
fn a_turns_later_frames_compact_and_restore_to_the_exact_original_bytes() {
    let mut encoder = EnvelopeDeltaEncoder::new();
    let mut decoder = EnvelopeDeltaDecoder::new();
    let original = turn();

    for (index, frame) in original.iter().enumerate() {
        let worker_seq = 100 + u64::try_from(index).unwrap_or_default();
        let mut event = raw_event(worker_seq, frame.clone());
        let compaction = encoder.compact(&mut event);

        if index == 0 {
            assert_eq!(
                compaction,
                Compaction::BaseRecorded {
                    base: "resp_turn_01".to_owned()
                }
            );
            assert_eq!(raw_value(&event), frame, "the base is persisted verbatim");
        } else {
            let Compaction::Compacted {
                base,
                full_bytes,
                delta_bytes,
            } = compaction
            else {
                unreachable!("frame {index} of a turn must compact: got {compaction:?}")
            };
            assert_eq!(base, "resp_turn_01");
            assert!(
                delta_bytes < full_bytes,
                "frame {index}: delta {delta_bytes} B must be smaller than full {full_bytes} B"
            );
            assert!(
                raw_value(&event).get(ENVELOPE_DELTA_KEY).is_some(),
                "frame {index} must be persisted as a delta document"
            );
        }

        let (resolved, unresolved) = decoder.resolve_event(&event);
        assert_eq!(unresolved, None, "frame {index} must resolve");
        assert_eq!(
            serde_json::to_vec(raw_value(&resolved)).unwrap_or_default(),
            serde_json::to_vec(frame).unwrap_or_default(),
            "frame {index} must restore to the exact bytes it was emitted with"
        );
    }
}

#[test]
fn the_repeated_instructions_block_is_persisted_once_per_turn() {
    let mut encoder = EnvelopeDeltaEncoder::new();
    let mut persisted = 0_usize;
    let mut carrying_instructions = 0_usize;

    for (index, frame) in turn().iter().enumerate() {
        let mut event = raw_event(
            100 + u64::try_from(index).unwrap_or_default(),
            frame.clone(),
        );
        encoder.compact(&mut event);
        let bytes = serde_json::to_vec(raw_value(&event)).unwrap_or_default();
        persisted += bytes.len();
        if String::from_utf8_lossy(&bytes).contains(&instructions()) {
            carrying_instructions += 1;
        }
    }

    assert_eq!(
        carrying_instructions, 1,
        "the instructions block belongs in exactly one persisted frame per turn"
    );
    let uncompacted: usize = turn()
        .iter()
        .map(|frame| serde_json::to_vec(frame).unwrap_or_default().len())
        .sum();
    assert!(
        persisted * 2 < uncompacted,
        "compacted turn ({persisted} B) must be well under half the uncompacted turn ({uncompacted} B)"
    );
}

#[test]
fn a_new_turn_displaces_the_previous_turns_base_rather_than_accumulating() {
    let mut encoder = EnvelopeDeltaEncoder::new();
    let mut decoder = EnvelopeDeltaDecoder::new();
    let mut events = Vec::new();

    for (turn_index, response_id) in ["resp_a", "resp_b"].iter().enumerate() {
        for (frame_index, frame) in turn().iter().enumerate() {
            let mut value = frame.clone();
            if let Some(response) = value.get_mut("response").and_then(Value::as_object_mut) {
                response.insert("id".to_owned(), json!(response_id));
            }
            let seq = u64::try_from(turn_index * 10 + frame_index).unwrap_or_default();
            let mut event = raw_event(seq, value.clone());
            encoder.compact(&mut event);
            events.push((value, event));
        }
    }

    for (original, event) in &events {
        let (resolved, unresolved) = decoder.resolve_event(event);
        assert_eq!(unresolved, None);
        assert_eq!(raw_value(&resolved), original);
    }
}

// ---------------------------------------------------------------------------
// Mixed history: old full envelopes and deltas in one stream.
// ---------------------------------------------------------------------------

#[test]
fn a_delta_resolves_against_a_base_that_was_written_in_the_old_format() {
    let frames = turn();
    let mut decoder = EnvelopeDeltaDecoder::new();

    // The old-format region: a full envelope, persisted by an emitter that knew nothing of deltas.
    let old = raw_event(100, frames[0].clone());
    assert_eq!(decoder.resolve(&old), ResolvedEnvelope::Verbatim);

    // The new-format region, compacted by an encoder primed with the same base.
    let mut encoder = EnvelopeDeltaEncoder::new();
    let mut base_event = raw_event(100, frames[0].clone());
    encoder.compact(&mut base_event);
    let mut delta_event = raw_event(101, frames[1].clone());
    encoder.compact(&mut delta_event);

    let (resolved, unresolved) = decoder.resolve_event(&delta_event);
    assert_eq!(unresolved, None);
    assert_eq!(raw_value(&resolved), &frames[1]);
}

// ---------------------------------------------------------------------------
// Refusals: every shape the codec must decline rather than guess at.
// ---------------------------------------------------------------------------

#[test]
fn a_delta_whose_base_was_never_seen_is_reported_and_never_guessed() {
    let frames = turn();
    let mut encoder = EnvelopeDeltaEncoder::new();
    let mut base_event = raw_event(100, frames[0].clone());
    encoder.compact(&mut base_event);
    let mut delta_event = raw_event(101, frames[1].clone());
    encoder.compact(&mut delta_event);

    // A reader that joined the stream after the base — a window that opened mid-turn, or a base
    // that retention trimmed.
    let mut decoder = EnvelopeDeltaDecoder::new();
    let (resolved, unresolved) = decoder.resolve_event(&delta_event);

    let Some(unresolved) = unresolved else {
        unreachable!("a delta with no base must be reported, not resolved")
    };
    assert_eq!(unresolved.reason, UnresolvedReason::BaseNotSeen);
    assert_eq!(unresolved.base, "resp_turn_01");
    assert_eq!(unresolved.base_worker_seq, Some(100));
    assert_eq!(
        raw_value(&resolved),
        raw_value(&delta_event),
        "the delta document itself is preserved, so the changed fields are never lost"
    );
    assert!(
        unresolved.to_string().contains("worker_seq 100"),
        "the report names the missing record: {unresolved}"
    );
}

#[test]
fn a_base_that_was_altered_after_the_delta_was_built_is_refused_not_patched() {
    let frames = turn();
    let mut encoder = EnvelopeDeltaEncoder::new();
    let mut base_event = raw_event(100, frames[0].clone());
    encoder.compact(&mut base_event);
    let mut delta_event = raw_event(101, frames[1].clone());
    encoder.compact(&mut delta_event);

    // The server's per-event ceiling replaced the base's payload with a truncation stub after the
    // encoder had already diffed against the real bytes.
    let truncated = raw_event(
        100,
        json!({
            "response": {"id": "resp_turn_01"},
            "truncated": true,
            "reason": "observability.max_event_bytes",
        }),
    );

    let mut decoder = EnvelopeDeltaDecoder::new();
    assert_eq!(decoder.resolve(&truncated), ResolvedEnvelope::Verbatim);
    let ResolvedEnvelope::Unresolved(unresolved) = decoder.resolve(&delta_event) else {
        unreachable!("a delta must not be applied to a base it was not built against")
    };
    assert!(
        matches!(
            unresolved.reason,
            UnresolvedReason::BaseDigestMismatch { .. }
        ),
        "expected a digest mismatch, got {:?}",
        unresolved.reason
    );
}

#[test]
fn a_delta_from_a_future_format_version_is_refused_by_name() {
    let mut decoder = EnvelopeDeltaDecoder::new();
    let event = raw_event(
        7,
        json!({ENVELOPE_DELTA_KEY: {
            "v": 999,
            "base": "resp_turn_01",
            "base_worker_seq": 100,
            "base_digest": "00",
            "set": {},
            "unset": [],
        }}),
    );
    let ResolvedEnvelope::Unresolved(unresolved) = decoder.resolve(&event) else {
        unreachable!("an unknown format version must be refused")
    };
    assert_eq!(
        unresolved.reason,
        UnresolvedReason::UnsupportedVersion {
            version: 999,
            supported: super::ENVELOPE_DELTA_VERSION,
        }
    );
}

#[test]
fn a_malformed_delta_document_is_refused_and_still_names_its_turn() {
    let mut decoder = EnvelopeDeltaDecoder::new();
    let event = raw_event(
        7,
        json!({ENVELOPE_DELTA_KEY: {"base": "resp_turn_01", "base_worker_seq": 100}}),
    );
    let ResolvedEnvelope::Unresolved(unresolved) = decoder.resolve(&event) else {
        unreachable!("a malformed delta document must be refused")
    };
    assert!(matches!(
        unresolved.reason,
        UnresolvedReason::Malformed { .. }
    ));
    assert_eq!(unresolved.base, "resp_turn_01");
    assert_eq!(unresolved.base_worker_seq, Some(100));
}

#[test]
fn an_ephemeral_frame_is_never_compacted_because_it_is_never_durable() {
    let mut encoder = EnvelopeDeltaEncoder::new();
    let frames = turn();
    let mut base_event = raw_event(100, frames[0].clone());
    encoder.compact(&mut base_event);

    let mut ephemeral = raw_event(101, frames[1].clone());
    ephemeral.ephemeral = true;
    assert_eq!(encoder.compact(&mut ephemeral), Compaction::NotApplicable);
    assert_eq!(raw_value(&ephemeral), &frames[1]);
}

#[test]
fn a_non_envelope_raw_value_and_a_non_raw_kind_are_both_left_alone() {
    let mut encoder = EnvelopeDeltaEncoder::new();
    let mut decoder = EnvelopeDeltaDecoder::new();

    let mut line = raw_event(1, json!({"line": "plain adapter passthrough"}));
    assert_eq!(encoder.compact(&mut line), Compaction::NotApplicable);
    assert_eq!(decoder.resolve(&line), ResolvedEnvelope::Verbatim);

    let mut message = raw_event(2, Value::Null);
    message.kind = ActivityEventKind::Message {
        role: MessageRole::Assistant,
        text: "hello".to_owned(),
    };
    assert_eq!(encoder.compact(&mut message), Compaction::NotApplicable);
    assert_eq!(decoder.resolve(&message), ResolvedEnvelope::Verbatim);
}

#[test]
fn a_frame_whose_delta_would_not_be_smaller_is_persisted_in_full() {
    let mut encoder = EnvelopeDeltaEncoder::new();
    // A tiny envelope: the delta's own header (version, base id, worker seq, digest) costs more
    // than the whole frame, so compacting it would grow the store.
    let base = json!({"response": {"id": "r"}, "n": 1});
    let next = json!({"response": {"id": "r"}, "n": 2});
    let mut base_event = raw_event(1, base);
    assert_eq!(
        encoder.compact(&mut base_event),
        Compaction::BaseRecorded {
            base: "r".to_owned()
        }
    );
    let mut next_event = raw_event(2, next.clone());
    let compaction = encoder.compact(&mut next_event);
    assert!(
        matches!(
            compaction,
            Compaction::FullRetained {
                reason: FullRetainedReason::NotSmaller { .. },
                ..
            }
        ),
        "expected the frame to be kept whole, got {compaction:?}"
    );
    assert_eq!(raw_value(&next_event), &next);
}

#[test]
fn two_agents_in_one_attempt_do_not_displace_each_others_bases() {
    let mut encoder = EnvelopeDeltaEncoder::new();
    let mut decoder = EnvelopeDeltaDecoder::new();
    let frames = turn();

    let mut first_base = raw_event(1, frames[0].clone());
    let mut second_base = raw_event(2, frames[0].clone());
    second_base.agent_id = Uuid::from_u128(0x44);
    encoder.compact(&mut first_base);
    encoder.compact(&mut second_base);

    let mut first_delta = raw_event(3, frames[2].clone());
    let mut second_delta = raw_event(4, frames[1].clone());
    second_delta.agent_id = Uuid::from_u128(0x44);
    encoder.compact(&mut first_delta);
    encoder.compact(&mut second_delta);

    for event in [&first_base, &second_base, &first_delta, &second_delta] {
        let (_, unresolved) = decoder.resolve_event(event);
        assert_eq!(unresolved, None, "every frame must resolve in its own slot");
    }
    let (first, _) = decoder.resolve_event(&first_delta);
    let (second, _) = decoder.resolve_event(&second_delta);
    assert_eq!(raw_value(&first), &frames[2]);
    assert_eq!(raw_value(&second), &frames[1]);
}

// ---------------------------------------------------------------------------
// The pure diff/apply layer.
// ---------------------------------------------------------------------------

#[test]
fn a_field_set_to_null_round_trips_as_null_rather_than_being_removed() {
    let base = json!({"a": 1, "b": {"c": 2}});
    let next = json!({"a": Value::Null, "b": {"c": 2}});
    let (Value::Object(base_object), Value::Object(next_object)) = (&base, &next) else {
        unreachable!("both fixtures are objects")
    };
    let diff = patch::diff(base_object, next_object);
    assert!(diff.unset.is_empty(), "setting null is not a removal");
    assert_eq!(patch::apply(&base, &diff), Ok(next));
}

#[test]
fn a_removed_field_is_carried_as_a_pointer_and_restored_by_removal() {
    let base = json!({"a": 1, "b": {"c": 2, "d": 3}});
    let next = json!({"a": 1, "b": {"c": 2}});
    let (Value::Object(base_object), Value::Object(next_object)) = (&base, &next) else {
        unreachable!("both fixtures are objects")
    };
    let diff = patch::diff(base_object, next_object);
    assert_eq!(diff.unset, vec!["/b/d".to_owned()]);
    assert_eq!(patch::apply(&base, &diff), Ok(next));
}

#[test]
fn keys_containing_pointer_metacharacters_survive_the_round_trip() {
    let base = json!({"a/b": {"c~d": 1, "gone": 2}});
    let next = json!({"a/b": {"c~d": 9}});
    let (Value::Object(base_object), Value::Object(next_object)) = (&base, &next) else {
        unreachable!("both fixtures are objects")
    };
    let diff = patch::diff(base_object, next_object);
    assert_eq!(diff.unset, vec!["/a~1b/gone".to_owned()]);
    assert_eq!(patch::apply(&base, &diff), Ok(next));
}

#[test]
fn a_field_that_changed_type_is_replaced_wholesale_not_merged() {
    let base = json!({"usage": Value::Null});
    let next = json!({"usage": {"input_tokens": 1}});
    let (Value::Object(base_object), Value::Object(next_object)) = (&base, &next) else {
        unreachable!("both fixtures are objects")
    };
    let diff = patch::diff(base_object, next_object);
    assert_eq!(patch::apply(&base, &diff), Ok(next));
}

#[test]
fn applying_a_patch_to_the_wrong_base_errors_instead_of_half_patching() {
    let wrong_base = json!({"a": 1});
    let patch = Patch {
        set: json!({"a": 2}).as_object().cloned().unwrap_or_default(),
        unset: vec!["/missing".to_owned()],
    };
    assert_eq!(
        patch::apply(&wrong_base, &patch),
        Err(PatchError::PointerMissing {
            pointer: "/missing".to_owned()
        })
    );
    assert_eq!(
        patch::apply(&json!([1, 2]), &Patch::default()),
        Err(PatchError::BaseNotAnObject)
    );
}
