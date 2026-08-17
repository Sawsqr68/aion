//! Proofs for the transcript serving boundary's reconstruction.
//!
//! The boundary's contract is that no consumer downstream of it can tell a compacted stream from
//! an uncompacted one. These tests state that as byte equality against the uncompacted stream, on
//! each of the three shapes a reader meets: pure old format, pure delta, and one stream carrying
//! both.

use aion_core::{ActivityEvent, ActivityEventKind, ActivityId, RunId, WorkflowId};
use aion_integrations::envelope_delta::{ENVELOPE_DELTA_KEY, EnvelopeDeltaEncoder};
use aion_store::ActivityRecord;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{resolve_event, resolve_records};

fn fixed_time() -> DateTime<Utc> {
    DateTime::from_timestamp(1_786_895_017, 0).unwrap_or_default()
}

/// The turn-invariant block that is the whole point: a provider repeats it in every lifecycle
/// frame of a turn, and compaction persists it once.
fn instructions() -> String {
    "Inspect the repository before editing. Preserve unrelated changes. Implement the bounded \
     capability end to end, run the required verification battery, and report only evidence you \
     directly observed. "
        .repeat(16)
}

fn frame(response_id: &str, kind: &str, status: &str, usage: &Value) -> Value {
    json!({
        "type": kind,
        "response": {
            "id": response_id,
            "object": "response",
            "status": status,
            "model": "gpt-5.2-codex",
            "instructions": instructions(),
            "tools": [{"name": "read", "type": "function"}],
            "usage": usage,
        },
    })
}

/// One turn's three lifecycle frames, the shape the forensics measured.
fn turn(response_id: &str) -> Vec<Value> {
    vec![
        frame(response_id, "response.created", "queued", &Value::Null),
        frame(
            response_id,
            "response.in_progress",
            "in_progress",
            &Value::Null,
        ),
        frame(
            response_id,
            "response.completed",
            "completed",
            &json!({"input_tokens": 1200, "output_tokens": 7}),
        ),
    ]
}

fn record(store_seq: u64, value: Value) -> ActivityRecord {
    ActivityRecord {
        store_seq,
        event: ActivityEvent {
            workflow_id: WorkflowId::new(Uuid::from_u128(0x11)),
            run_id: RunId::new(Uuid::from_u128(0x22)),
            activity_id: ActivityId::from_sequence_position(7),
            attempt: 2,
            agent_id: Uuid::from_u128(0x33),
            agent_role: "orchestrator".to_owned(),
            emitted_at: fixed_time(),
            worker_seq: store_seq,
            store_seq: Some(store_seq),
            ephemeral: false,
            kind: ActivityEventKind::Raw {
                source: "event/raw".to_owned(),
                value,
            },
        },
    }
}

/// Two turns, every frame in full — what a stream written before compaction contains.
fn old_format() -> Vec<ActivityRecord> {
    let mut records = Vec::new();
    for (index, value) in turn("resp_a").into_iter().chain(turn("resp_b")).enumerate() {
        records.push(record(u64::try_from(index).unwrap_or_default(), value));
    }
    records
}

/// Runs the emitter's compaction over a range of records, in place.
fn compact(records: &mut [ActivityRecord], range: std::ops::Range<usize>) {
    let mut encoder = EnvelopeDeltaEncoder::new();
    for record in &mut records[range] {
        encoder.compact(&mut record.event);
    }
}

fn compacted_count(records: &[ActivityRecord]) -> usize {
    records
        .iter()
        .filter(|record| match &record.event.kind {
            ActivityEventKind::Raw { value, .. } => value.get(ENVELOPE_DELTA_KEY).is_some(),
            _ => false,
        })
        .count()
}

/// The events of a run of records, which is what a consumer actually receives (an
/// `ActivityRecord` is a store-side pairing and is not itself serializable).
fn events(records: &[ActivityRecord]) -> Vec<ActivityEvent> {
    records.iter().map(|record| record.event.clone()).collect()
}

/// The serialized bytes of a run of records' events, for byte-equality assertions.
fn wire(records: &[ActivityRecord]) -> Vec<u8> {
    serde_json::to_vec(&events(records)).unwrap_or_default()
}

fn bytes(records: &[ActivityRecord]) -> usize {
    records
        .iter()
        .map(|record| serde_json::to_vec(&record.event).map_or(0, |bytes| bytes.len()))
        .sum()
}

#[test]
fn an_old_format_stream_passes_through_the_boundary_untouched() {
    let old = old_format();
    let mut served = old.clone();
    let unresolved = resolve_records(&mut served);

    assert!(unresolved.is_empty(), "old format carries no deltas");
    assert_eq!(
        wire(&served),
        wire(&old),
        "a stream with nothing to reconstruct must be returned byte-identical"
    );
}

#[test]
fn a_fully_compacted_stream_is_served_byte_identically_to_the_old_format() {
    let old = old_format();
    let mut stored = old.clone();
    compact(&mut stored, 0..6);
    assert_eq!(compacted_count(&stored), 4, "each turn keeps one base");
    assert!(
        bytes(&stored) * 2 < bytes(&old),
        "the point of the exercise: {} B stored against {} B",
        bytes(&stored),
        bytes(&old)
    );

    let mut served = stored;
    let unresolved = resolve_records(&mut served);
    assert!(unresolved.is_empty(), "every base is inside this read");
    assert_eq!(
        wire(&served),
        wire(&old),
        "a consumer must not be able to tell the stored stream was compacted"
    );
}

#[test]
fn a_mixed_stream_of_old_and_compacted_turns_is_served_byte_identically() {
    let old = old_format();
    let mut stored = old.clone();
    // The first turn predates compaction; the second was written after it. This is the shape a
    // stream has forever once the emitter changes, and it is never migrated away.
    compact(&mut stored, 3..6);
    assert_eq!(compacted_count(&stored), 2);

    let mut served = stored;
    let unresolved = resolve_records(&mut served);
    assert!(unresolved.is_empty());
    assert_eq!(wire(&served), wire(&old),);
}

#[test]
fn a_read_that_opens_after_a_base_reports_the_loss_and_fabricates_nothing() {
    let old = old_format();
    let mut stored = old.clone();
    compact(&mut stored, 0..6);
    // A window that begins inside the first turn — retention trimmed the base, or the caller
    // resumed from a cursor past it.
    let mut served = stored[1..].to_vec();
    let unresolved = resolve_records(&mut served);

    assert_eq!(
        unresolved.len(),
        2,
        "the first turn's two deltas are orphaned"
    );
    assert!(
        unresolved.iter().all(|report| report.base == "resp_a"),
        "the report names the turn that cannot be rebuilt: {unresolved:?}"
    );
    assert_eq!(
        compacted_count(&served[..2]),
        2,
        "the orphans keep their delta documents"
    );
    assert_eq!(
        wire(&served[2..]),
        wire(&old[3..]),
        "the second turn is unaffected by the first turn's missing base"
    );
}

#[test]
fn the_streaming_path_resolves_a_live_frame_against_a_base_delivered_during_replay() {
    let old = old_format();
    let mut stored = old.clone();
    compact(&mut stored, 0..6);

    // The socket's decoder spans the replay/live splice; the base arrives in replay and the
    // delta arrives live.
    let mut decoder = aion_integrations::envelope_delta::EnvelopeDeltaDecoder::new();
    let mut replayed = stored[0].event.clone();
    assert!(resolve_event(&mut decoder, &mut replayed).is_none());

    let mut live = stored[1].event.clone();
    assert!(
        resolve_event(&mut decoder, &mut live).is_none(),
        "a live delta must resolve against a base seen during replay"
    );
    assert_eq!(
        serde_json::to_vec(&live).unwrap_or_default(),
        serde_json::to_vec(&old[1].event).unwrap_or_default(),
    );
}
