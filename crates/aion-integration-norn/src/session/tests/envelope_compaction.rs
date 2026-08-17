//! What one LLM turn costs to persist, measured on the real emitter path.
//!
//! The forensics of 2026-08-17 measured a provider turn arriving as three `event/raw`
//! notifications — `response.created`, `response.in_progress`, `response.completed` — each
//! carrying the entire response envelope including an instructions block identical across all
//! three. At ~82 KB per frame that is ~91% of a transcript's bytes, before any storage amplifier.
//!
//! This drives the session's own pump over an in-memory duplex, exactly as the other session
//! tests do, and measures the events the pump actually emits. It is not a synthetic call to the
//! codec: if the adapter stops compacting, or compacts somewhere the pump does not reach, these
//! numbers move.

use super::*;
use aion_integrations::envelope_delta::ENVELOPE_DELTA_KEY;

/// The turn-invariant block a provider repeats in every lifecycle frame of one turn.
fn instructions() -> String {
    "You are the implementation agent for an isolated repository lane. Inspect the repository \
     before editing. Preserve unrelated changes. Implement the bounded capability end to end, \
     add focused regression tests, run the required verification battery, and report only \
     evidence you directly observed. "
        .repeat(32)
}

/// One `event/raw` notification carrying a full provider response envelope.
fn envelope_note(kind: &str, status: &str, usage: &serde_json::Value) -> JsonRpcNotification {
    JsonRpcNotification::new(
        "event/raw",
        Some(json!({
            "type": kind,
            "response": {
                "id": "resp_measured_turn",
                "object": "response",
                "created_at": 1_786_895_017_u64,
                "status": status,
                "model": "gpt-5.2-codex",
                "instructions": instructions(),
                "tools": [{"name": "read", "type": "function"}, {"name": "bash", "type": "function"}],
                "parallel_tool_calls": true,
                "usage": usage,
            },
            "agent_id": Uuid::nil().to_string(),
            "agent_role": "root",
        })),
    )
}

/// The three frames of one turn, in the order a provider streams them.
fn turn() -> [JsonRpcNotification; 3] {
    [
        envelope_note("response.created", "queued", &serde_json::Value::Null),
        envelope_note(
            "response.in_progress",
            "in_progress",
            &serde_json::Value::Null,
        ),
        envelope_note(
            "response.completed",
            "completed",
            &json!({"input_tokens": 1200, "output_tokens": 7, "total_tokens": 1207}),
        ),
    ]
}

/// Drives one turn through the session pump and returns the events it emitted, in order.
async fn emitted_turn() -> Result<Vec<ActivityEvent>, Box<dyn std::error::Error>> {
    let (mut session, peer, run_id) = wire();
    let mut events = session.events();
    let mut emitted = Vec::new();

    for note in turn() {
        peer.send_notification(&note)
            .await
            .map_err(|error| format!("the peer must send a turn frame: {error}"))?;
        let Some(event) = events.next().await else {
            return Err("every turn frame must reach the events stream".into());
        };
        emitted.push(event);
    }

    peer.send_response(&JsonRpcResponse::success(
        run_id,
        completed_envelope(&json!("done")),
    ))
    .await
    .map_err(|error| format!("the peer must close the run: {error}"))?;
    session
        .wait_result()
        .await
        .map_err(|error| format!("the run must yield a payload: {error}"))?;
    Ok(emitted)
}

/// The serialized bytes of the events that would be persisted (ephemeral ones never are).
fn persisted_bytes(events: &[ActivityEvent]) -> Result<usize, serde_json::Error> {
    let mut total = 0;
    for event in events.iter().filter(|event| !event.ephemeral) {
        total += serde_json::to_vec(event)?.len();
    }
    Ok(total)
}

/// How many persisted frames carry the turn-invariant instructions block.
fn frames_carrying_instructions(
    events: &[ActivityEvent],
) -> Result<usize, Box<dyn std::error::Error>> {
    let block = instructions();
    let mut count = 0;
    for event in events.iter().filter(|event| !event.ephemeral) {
        if String::from_utf8_lossy(&serde_json::to_vec(&event.kind)?).contains(&block) {
            count += 1;
        }
    }
    Ok(count)
}

/// THE DEFECT, MEASURED. A turn's instructions block must be persisted once, not once per frame.
///
/// This is the red-first assertion for the writer arm: before the emitter compacts, all three
/// frames carry the block and the turn costs three full envelopes.
#[tokio::test]
async fn one_turn_persists_its_instructions_block_exactly_once() -> TestResult {
    let events = emitted_turn().await?;
    assert_eq!(events.len(), 3, "one turn is three lifecycle frames");

    assert_eq!(
        frames_carrying_instructions(&events)?,
        1,
        "the turn-invariant instructions block belongs in ONE persisted frame per turn, \
         not in every lifecycle frame of it"
    );
    Ok(())
}

/// The byte consequence of the same defect, stated against the frame the turn is diffed from.
///
/// A turn's cost must be about one envelope plus two small deltas, so it must come in under twice
/// the first frame's size. Three full copies cannot.
#[tokio::test]
async fn one_turn_costs_about_one_envelope_not_three() -> TestResult {
    let events = emitted_turn().await?;
    let base_bytes = serde_json::to_vec(&events[0])?.len();
    let turn_bytes = persisted_bytes(&events)?;
    // Printed so the measurement is in the record on every run, not only when it regresses.
    println!(
        "one turn: {turn_bytes} B persisted; base frame {base_bytes} B; \
         three full copies would be ~{} B",
        base_bytes * 3
    );

    assert!(
        turn_bytes < base_bytes * 2,
        "a turn is one envelope plus its deltas: {turn_bytes} B against a {base_bytes} B \
         first frame (three full copies would be about {} B)",
        base_bytes * 3
    );
    Ok(())
}

/// The frames after a turn's first must be persisted as deltas naming their base.
#[tokio::test]
async fn the_frames_after_a_turns_first_are_persisted_as_deltas() -> TestResult {
    let events = emitted_turn().await?;
    let compacted = events
        .iter()
        .filter(|event| match &event.kind {
            ActivityEventKind::Raw { value, .. } => value.get(ENVELOPE_DELTA_KEY).is_some(),
            _ => false,
        })
        .count();
    assert_eq!(
        compacted, 2,
        "the base is persisted whole and the other two frames as deltas against it"
    );
    Ok(())
}

/// Compaction must not disturb the ordering the transcript is keyed on.
#[tokio::test]
async fn compaction_leaves_the_worker_sequence_and_identity_untouched() -> TestResult {
    let events = emitted_turn().await?;
    assert_eq!(
        events
            .iter()
            .map(|event| event.worker_seq)
            .collect::<Vec<_>>(),
        vec![0, 1, 2],
        "the pump's monotonic worker sequence is unchanged by compaction"
    );
    assert!(
        events
            .iter()
            .all(|event| event.attempt == 1 && !event.ephemeral),
        "identity and durability flags are unchanged by compaction"
    );
    Ok(())
}
