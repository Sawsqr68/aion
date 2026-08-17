//! Fast unit tests of the session against an IN-TEST FAKE PEER over an in-memory duplex — no
//! real process. The fake peer feeds canned JSON-RPC frames (notifications + the id-matched
//! run/execute Response + intervene acks) using the same `aion-integrations` jsonrpc helper the
//! adapter itself uses, so the pump's demux is exercised end-to-end without spawning `norn`.

use super::*;
use aion_core::{
    ActivityEventKind, ActivityId, InjectPriority, InterventionCapabilities, InterventionKind,
    InterventionPrimitive, RunId, WorkflowId,
};
use aion_integrations::ContentType;
use aion_integrations::jsonrpc::{JsonRpcNotification, JsonRpcResponse};
use chrono::Utc;
use futures::StreamExt;
use serde_json::json;
use tokio::io::{DuplexStream, ReadHalf, WriteHalf, duplex, split};
use uuid::Uuid;

/// Failures are reported as errors naming what was expected, so no assertion needs `unwrap`.
type TestResult = Result<(), Box<dyn std::error::Error>>;

type SessionUnderTest = NornSession<ReadHalf<DuplexStream>, WriteHalf<DuplexStream>>;
/// The fake peer's own framed connection: it writes notifications/responses the session reads,
/// and reads the session's outbound intervene requests.
type PeerConnection = JsonRpcConnection<ReadHalf<DuplexStream>, WriteHalf<DuplexStream>>;

fn identity() -> EventIdentity {
    EventIdentity {
        workflow_id: WorkflowId::new(Uuid::nil()),
        run_id: RunId::new(Uuid::from_u128(0x11)),
        activity_id: ActivityId::from_sequence_position(2),
        attempt: 1,
    }
}

fn caps() -> InterventionCapabilities {
    InterventionCapabilities::from_primitives([
        InterventionPrimitive::InjectMessage,
        InterventionPrimitive::Cancel,
    ])
}

/// Wire a session to a fake peer over a loopback duplex. The session reads what the peer writes
/// and vice versa. The run/execute id is fixed so the peer can id-match the result Response.
fn wire() -> (SessionUnderTest, PeerConnection, JsonRpcId) {
    let (session_io, peer_io) = duplex(8192);
    let (s_read, s_write) = split(session_io);
    let (p_read, p_write) = split(peer_io);
    let connection = Arc::new(JsonRpcConnection::new(s_read, s_write));
    let run_id = JsonRpcId::number(1);
    let session = NornSession::start(connection, caps(), run_id.clone(), identity(), None);
    let peer = JsonRpcConnection::new(p_read, p_write);
    (session, peer, run_id)
}

fn text_note(text: &str) -> JsonRpcNotification {
    JsonRpcNotification::new(
        "event/message",
        Some(json!({
            "type": "text",
            "text": text,
            "agent_id": Uuid::nil().to_string(),
            "agent_role": "root",
        })),
    )
}

/// A `run/execute` success result in the versioned stop-envelope shape a completed run
/// carries (the fields beyond `envelope_version`/`stop`/`output` are realistic passengers).
fn completed_envelope(output: &serde_json::Value) -> serde_json::Value {
    json!({
        "envelope_version": 1,
        "stop": { "reason": "completed" },
        "output": output,
        "usage": { "input_tokens": 12, "output_tokens": 3 },
        "model": "mock-model",
        "session_id": "step-042",
        "events": [],
        "diagnostics": [],
    })
}

/// A non-completed stop envelope with the given internally-tagged `stop` object and `output`
/// (`null` for the reasons that carry none; the partial for those that may).
fn stopped_envelope(stop: &serde_json::Value, output: &serde_json::Value) -> serde_json::Value {
    json!({
        "envelope_version": 1,
        "stop": stop,
        "output": output,
        "model": "mock-model",
        "session_id": "step-042",
    })
}

/// Runs [`response_into_payload`] on a success Response carrying `result`.
fn payload_of(result: serde_json::Value) -> Result<Payload, HarnessError> {
    response_into_payload(JsonRpcResponse::success(JsonRpcId::number(1), result))
}

fn inject_command() -> InterventionCommand {
    InterventionCommand {
        workflow_id: WorkflowId::new(Uuid::nil()),
        run_id: RunId::new_v4(),
        activity_id: ActivityId::from_sequence_position(2),
        attempt: 1,
        issued_by: Some("operator".to_owned()),
        issued_at: Utc::now(),
        kind: InterventionKind::InjectMessage {
            text: "steer now".to_owned(),
            priority: InjectPriority::Interrupt,
        },
    }
}

#[tokio::test]
async fn capabilities_are_the_negotiated_set() {
    let (session, _peer, _id) = wire();
    assert!(
        session
            .capabilities()
            .supports_primitive(InterventionPrimitive::InjectMessage)
    );
    assert!(
        session
            .capabilities()
            .supports_primitive(InterventionPrimitive::Cancel)
    );
}

#[tokio::test]
async fn live_events_stream_from_peer_notifications() -> TestResult {
    let (mut session, peer, run_id) = wire();
    let mut events = session.events();

    // The peer streams two live event/* notifications, then the id-matched result.
    peer.send_notification(&text_note("working"))
        .await
        .map_err(|error| format!("the peer must send the first notification: {error}"))?;
    peer.send_notification(&text_note("still working"))
        .await
        .map_err(|error| format!("the peer must send the second notification: {error}"))?;

    let Some(first) = events.next().await else {
        return Err("the first event must stream live to the session".into());
    };
    assert_eq!(first.attempt, 1);
    match first.kind {
        ActivityEventKind::Message { text, .. } => assert_eq!(text, "working"),
        other => return Err(format!("expected a Message event, got {other:?}").into()),
    }
    let Some(second) = events.next().await else {
        return Err("the second event must stream live to the session".into());
    };
    match second.kind {
        ActivityEventKind::Message { text, .. } => assert_eq!(text, "still working"),
        other => return Err(format!("expected a Message event, got {other:?}").into()),
    }

    // Close the run so wait_result completes and the stream ends.
    peer.send_response(&JsonRpcResponse::success(
        run_id,
        completed_envelope(&json!("all done")),
    ))
    .await
    .map_err(|error| format!("the peer must send the run/execute result: {error}"))?;
    let payload = session
        .wait_result()
        .await
        .map_err(|error| format!("the completed run must yield a payload: {error}"))?;
    assert_eq!(payload.content_type(), &ContentType::Json);
    Ok(())
}

#[tokio::test]
async fn wait_result_returns_the_id_matched_run_execute_response_payload() -> TestResult {
    let (session, peer, run_id) = wire();
    // A stray notification and a non-matching Response must NOT be captured as the result.
    peer.send_notification(&text_note("noise"))
        .await
        .map_err(|error| format!("the peer must send the stray notification: {error}"))?;
    peer.send_response(&JsonRpcResponse::success(
        JsonRpcId::number(999),
        json!({ "not": "the result" }),
    ))
    .await
    .map_err(|error| format!("the peer must send the non-matching response: {error}"))?;
    // Only the id-matched run/execute Response is the result.
    peer.send_response(&JsonRpcResponse::success(
        run_id,
        completed_envelope(&json!("the id-matched answer")),
    ))
    .await
    .map_err(|error| format!("the peer must send the id-matched result: {error}"))?;

    let payload = session
        .wait_result()
        .await
        .map_err(|error| format!("the id-matched result must yield a payload: {error}"))?;
    let decoded = payload
        .to_json()
        .map_err(|error| format!("the result payload must be JSON: {error}"))?;
    assert_eq!(
        decoded,
        json!({ "text": "the id-matched answer", "final_message": "the id-matched answer", "stop_reason": "end_turn", "session_id": "step-042" })
    );
    Ok(())
}

#[tokio::test]
async fn intervene_sends_request_and_awaits_the_peer_ack() -> TestResult {
    let (session, peer, _run_id) = wire();

    // Drive intervene concurrently: the peer must read the request and ack it.
    let intervene = tokio::spawn(async move { session.intervene(inject_command()).await });

    // The peer reads the outbound intervene/injectMessage request and acks its id.
    let received = peer
        .recv()
        .await
        .map_err(|error| format!("the peer must read the intervene request: {error}"))?;
    let Some(message) = received else {
        return Err("an intervene request must arrive at the peer, got EOF".into());
    };
    let request = match message {
        IncomingMessage::Request(request) => request,
        other => return Err(format!("expected a request frame, got {other:?}").into()),
    };
    assert_eq!(request.method, "intervene/injectMessage");
    let Some(params) = request.params.as_ref() else {
        return Err("the intervene request must carry params".into());
    };
    assert_eq!(params["text"], json!("steer now"));
    assert_eq!(params["priority"], json!("interrupt"));
    peer.send_response(&JsonRpcResponse::success(
        request.id,
        json!({ "status": "injected" }),
    ))
    .await
    .map_err(|error| format!("the peer must ack the intervene request: {error}"))?;

    intervene
        .await
        .map_err(|error| format!("the intervene task must join cleanly: {error}"))?
        .map_err(|error| format!("the peer ack must resolve the intervene: {error}"))?;
    Ok(())
}

#[tokio::test]
async fn intervene_rejects_an_unsupported_primitive_without_sending() -> TestResult {
    let (session, peer, _run_id) = wire();
    let mut cmd = inject_command();
    cmd.kind = InterventionKind::PauseResume { paused: true };

    let Err(error) = session.intervene(cmd).await else {
        return Err("an unadvertised primitive must be refused, not accepted".into());
    };
    assert!(
        matches!(error, HarnessError::CapabilityNotSupported { .. }),
        "an unadvertised primitive is capability-gated, got {error:?}"
    );
    // Nothing was sent: the peer sees no frame before EOF (drop the session to close it).
    drop(session);
    let next = peer
        .recv()
        .await
        .map_err(|error| format!("the peer must read to EOF without error: {error}"))?;
    assert!(
        next.is_none(),
        "no request must reach the peer for a gated primitive"
    );
    Ok(())
}

#[tokio::test]
async fn intervene_surfaces_a_peer_error_ack() -> TestResult {
    let (session, peer, _run_id) = wire();
    let intervene = tokio::spawn(async move { session.intervene(inject_command()).await });

    let received = peer
        .recv()
        .await
        .map_err(|error| format!("the peer must read the intervene request: {error}"))?;
    let Some(message) = received else {
        return Err("an intervene request must arrive at the peer, got EOF".into());
    };
    let request = match message {
        IncomingMessage::Request(request) => request,
        other => return Err(format!("expected a request frame, got {other:?}").into()),
    };
    peer.send_response(&JsonRpcResponse::failure(
        request.id,
        aion_integrations::jsonrpc::JsonRpcError::new(-32603, "inbound channel full"),
    ))
    .await
    .map_err(|error| format!("the peer must send the error ack: {error}"))?;

    let outcome = intervene
        .await
        .map_err(|error| format!("the intervene task must join cleanly: {error}"))?;
    let Err(error) = outcome else {
        return Err("a rejected intervene must not resolve successfully".into());
    };
    assert!(
        matches!(error, HarnessError::Harness { .. }),
        "a rejected intervene surfaces as a harness error, got {error:?}"
    );
    Ok(())
}

#[tokio::test]
async fn a_notification_is_never_captured_as_the_result() -> TestResult {
    // The negative control: even an event/* notification whose params look result-shaped can
    // never fill the result slot, because it carries no id.
    let (session, peer, run_id) = wire();
    peer.send_notification(&JsonRpcNotification::new(
        "event/stop",
        Some(json!({
            "type": "done", "stop_reason": "end_turn",
            "agent_id": Uuid::nil().to_string(), "agent_role": "root",
        })),
    ))
    .await
    .map_err(|error| format!("the peer must send the result-shaped notification: {error}"))?;
    // Now the real result.
    peer.send_response(&JsonRpcResponse::success(
        run_id,
        completed_envelope(&json!("the real result")),
    ))
    .await
    .map_err(|error| format!("the peer must send the real result: {error}"))?;
    let payload = session
        .wait_result()
        .await
        .map_err(|error| format!("the id-matched result must yield a payload: {error}"))?;
    let decoded = payload
        .to_json()
        .map_err(|error| format!("the result payload must be JSON: {error}"))?;
    assert_eq!(
        decoded,
        json!({ "text": "the real result", "final_message": "the real result", "stop_reason": "end_turn", "session_id": "step-042" })
    );
    Ok(())
}

// --- response_into_payload: the stop-envelope branches ---

/// What one LLM turn costs to persist, measured on the pump.
mod envelope_compaction;
mod outcomes;
