//! Fast unit tests of the session against an in-memory stdout reader — no real process. Canned
//! stdout lines feed the REAL pump + demux, so the session's demux-to-event path is exercised
//! end-to-end without spawning anything.

use super::*;
use aion_core::{
    ActivityEventKind, ActivityId, ContentType, InterventionKind, MessageRole, RunId, WorkflowId,
};
use chrono::Utc;
use futures::StreamExt;
use serde_json::json;
use uuid::Uuid;

fn identity() -> EventIdentity {
    EventIdentity {
        workflow_id: WorkflowId::new(Uuid::nil()),
        run_id: RunId::new(Uuid::from_u128(0x11)),
        activity_id: ActivityId::from_sequence_position(2),
        attempt: 1,
    }
}

/// The derived session handle the record must carry: the identity quadruple, colon-joined —
/// computed from the SAME identity the session was started with, never hand-transcribed.
fn expected_session_id() -> String {
    let id = identity();
    format!(
        "{}:{}:{}:{}",
        id.workflow_id, id.run_id, id.activity_id, id.attempt
    )
}

/// Build a session over an in-memory stdout containing `text` (no child to await, so the exit
/// code is `None` — a clean-exit result).
fn session_over(text: &str) -> CliSession {
    let stdout = std::io::Cursor::new(text.as_bytes().to_vec());
    CliSession::start(stdout, identity(), None)
}

#[tokio::test]
async fn capabilities_are_always_empty() {
    let session = session_over("");
    assert!(
        session.capabilities().is_empty(),
        "the observability-only session advertises no interventions"
    );
}

#[tokio::test]
async fn stdout_lines_demux_into_a_live_event_stream() {
    let stdout = "\
[info] starting\n\
{\"type\":\"message\",\"text\":\"hello\"}\n\
{\"type\":\"stop\",\"reason\":\"end_turn\"}\n";
    let mut session = session_over(stdout);
    let events: Vec<ActivityEvent> = session.events().collect().await;

    assert_eq!(events.len(), 3, "every stdout line becomes an event");
    // A plain log line is Raw; the message and stop lines are mapped.
    assert!(matches!(events[0].kind, ActivityEventKind::Raw { .. }));
    match &events[1].kind {
        ActivityEventKind::Message { role, text } => {
            assert_eq!(*role, MessageRole::Assistant);
            assert_eq!(text, "hello");
        }
        other => panic!("expected Message, got {other:?}"),
    }
    assert!(matches!(events[2].kind, ActivityEventKind::Stop { .. }));
    // Ordering is stamped monotonically.
    assert_eq!(events[0].worker_seq, 0);
    assert_eq!(events[2].worker_seq, 2);
}

#[tokio::test]
async fn intervene_rejects_every_command_as_capability_not_supported() {
    let session = session_over("{\"type\":\"stop\",\"reason\":\"end_turn\"}\n");
    for kind in [
        InterventionKind::InjectMessage {
            text: "steer".to_owned(),
            priority: aion_core::InjectPriority::Interrupt,
        },
        InterventionKind::Cancel {
            reason: "stop".to_owned(),
        },
    ] {
        let cmd = InterventionCommand {
            workflow_id: WorkflowId::new(Uuid::nil()),
            run_id: RunId::new_v4(),
            activity_id: ActivityId::from_sequence_position(2),
            attempt: 1,
            issued_by: None,
            issued_at: Utc::now(),
            kind,
        };
        let error = session.intervene(cmd).await.unwrap_err();
        assert!(
            matches!(error, HarnessError::CapabilityNotSupported { .. }),
            "an observability-only session rejects every command, got {error:?}"
        );
    }
}

#[tokio::test]
async fn a_run_that_announced_no_stop_falls_back_to_end_turn() {
    // The fallback pin: NO `{"type":"stop"}` line anywhere in this stdout, so `end_turn`
    // here is the documented ended-normally-saying-nothing fallback, not a retained reason.
    let stdout = "\
{\"type\":\"message\",\"text\":\"thinking\"}\n\
the final answer is 42\n";
    let mut session = session_over(stdout);
    // Drain events so the pump reaches end-of-stdout and captures the terminal result.
    let _events: Vec<ActivityEvent> = session.events().collect().await;
    let payload = session.wait_result().await.unwrap();
    assert_eq!(payload.content_type(), &ContentType::Json);
    let decoded = payload.to_json().unwrap();
    assert_eq!(
        decoded,
        json!({
            "text": "the final answer is 42",
            "final_message": "the final answer is 42",
            "stop_reason": "end_turn",
            "session_id": expected_session_id(),
        }),
        "with no announced stop, a cleanly-ended run reports the end_turn fallback \
             and the identity-derived session handle"
    );
}

#[tokio::test]
async fn the_announced_stop_reason_reaches_the_outcome_record() {
    // The discriminating case: the agent announces a NON-end_turn stop, and that exact
    // reason must reach the record — a hardcoded `end_turn` fails here.
    let stdout = "\
partial work\n\
{\"type\":\"stop\",\"reason\":\"limit_reached\"}\n";
    let mut session = session_over(stdout);
    let _events: Vec<ActivityEvent> = session.events().collect().await;
    let decoded = session.wait_result().await.unwrap().to_json().unwrap();
    assert_eq!(
        decoded["stop_reason"],
        json!("limit_reached"),
        "the demuxed stop reason is the record's stop_reason, never re-derived: {decoded}"
    );
}

#[tokio::test]
async fn the_last_announced_stop_wins() {
    // A run that stops for tools and then ends its turn reports the LAST announcement.
    let stdout = "\
{\"type\":\"stop\",\"reason\":\"tool_use\"}\n\
{\"type\":\"message\",\"text\":\"done now\"}\n\
{\"type\":\"stop\",\"reason\":\"cancelled\"}\n";
    let mut session = session_over(stdout);
    let _events: Vec<ActivityEvent> = session.events().collect().await;
    let decoded = session.wait_result().await.unwrap().to_json().unwrap();
    assert_eq!(decoded["stop_reason"], json!("cancelled"));
}

#[tokio::test]
async fn an_event_is_never_the_result_the_two_are_separate_channels() {
    // The events stream and the terminal result are distinct: a message event whose text looks
    // like an answer is still only an event, and the result is captured separately.
    let mut session = session_over("{\"type\":\"message\",\"text\":\"not the result\"}\n");
    let events: Vec<ActivityEvent> = session.events().collect().await;
    assert_eq!(events.len(), 1);
    let payload = session.wait_result().await.unwrap();
    // The result carries the final LINE, which is the message JSON line, not the extracted text.
    let decoded = payload.to_json().unwrap();
    assert_eq!(decoded["stop_reason"], json!("end_turn"));
    assert_eq!(
        decoded["text"],
        json!("{\"type\":\"message\",\"text\":\"not the result\"}")
    );
}
