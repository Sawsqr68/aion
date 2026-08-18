//! `POST /workflows/describe-live` end to end.
//!
//! The join is only worth having if each source is really consulted, so every
//! test drives ONE source into a specific state and asserts the response
//! changed with it — paired, wherever the answer is a verdict, with the control
//! that produces the opposite answer from the same request.

use std::sync::Arc;
use std::time::{Duration, Instant};

use aion_core::{
    ActivityEvent, ActivityEventKind, ActivityId, Event, EventEnvelope, InterventionCapabilities,
    InterventionPrimitive, MessageRole, Payload,
};
use aion_store::WriteToken;
use axum::{Router, http::StatusCode};
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::json;
use tower::ServiceExt;

use super::super::router::workflow_router;
use super::super::test_support::{
    NAMESPACE, json_request, read_json, runtime_config, server_state, shared_engine, workflow_id,
};
use crate::worker::{AttemptKey, InFlightActivity, WorkerRegistration};
use crate::{
    NamespaceResolver, ServerState, StaticScheduleNamespaces, StaticWorkflowNamespaces,
    config::NamespaceMode,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const ACTIVITY: u64 = 4;
const ATTEMPT: u32 = 1;

fn activity_id() -> ActivityId {
    ActivityId::from_sequence_position(ACTIVITY)
}

/// The one run every fixture in this file lives in: the history's
/// `WorkflowStarted`, the published transcript events, and the attempt-owner
/// bind all carry it, exactly as one live generation would.
fn run_id() -> aion_core::RunId {
    aion_core::RunId::new(uuid::Uuid::from_u128(10))
}

fn envelope(seq: u64, recorded_at: DateTime<Utc>) -> EventEnvelope {
    EventEnvelope {
        seq,
        recorded_at,
        workflow_id: workflow_id(),
    }
}

fn payload() -> Result<Payload, aion_core::PayloadError> {
    Payload::from_json(&json!({ "fixture": true }))
}

fn started(at: DateTime<Utc>) -> Result<Event, aion_core::PayloadError> {
    Ok(Event::WorkflowStarted {
        envelope: envelope(1, at),
        workflow_type: "fixture".to_owned(),
        input: payload()?,
        run_id: run_id(),
        parent_run_id: None,
        parent_workflow_id: None,
        package_version: aion_core::PackageVersion::new("a".repeat(64)),
    })
}

fn scheduled(at: DateTime<Utc>) -> Result<Event, aion_core::PayloadError> {
    Ok(Event::ActivityScheduled {
        envelope: envelope(2, at),
        activity_id: activity_id(),
        activity_type: "review".to_owned(),
        input: payload()?,
        task_queue: "agents".to_owned(),
        node: None,
    })
}

fn dispatched(at: DateTime<Utc>) -> Event {
    Event::ActivityStarted {
        envelope: envelope(3, at),
        activity_id: activity_id(),
        attempt: ATTEMPT,
    }
}

/// The history of a run parked inside its dispatched step, with the dispatch
/// recorded at `dispatched_at`.
fn in_flight_history(dispatched_at: DateTime<Utc>) -> Result<Vec<Event>, aion_core::PayloadError> {
    Ok(vec![
        started(dispatched_at)?,
        scheduled(dispatched_at)?,
        dispatched(dispatched_at),
    ])
}

/// A server state over a shared in-memory engine, seeded with `history`.
async fn state_with(
    history: &[Event],
    max_stream_events: u64,
) -> Result<ServerState, Box<dyn std::error::Error>> {
    let (engine, store, _visibility) = shared_engine().await?;
    if !history.is_empty() {
        store
            .append(WriteToken::recorder(), &workflow_id(), history, 0)
            .await?;
    }
    let ownership = StaticWorkflowNamespaces::default();
    ownership.record(workflow_id(), NAMESPACE)?;
    let resolver = NamespaceResolver::from_parts(
        NamespaceMode::SharedEngine,
        Some(engine),
        Arc::new(ownership),
        Arc::new(StaticScheduleNamespaces::default()),
    );
    let mut runtime = runtime_config();
    runtime.observability.max_stream_events = max_stream_events;
    server_state(resolver, runtime).await
}

/// Publish `count` transcript records onto the current attempt's stream.
async fn publish_transcript(
    state: &ServerState,
    count: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    for worker_seq in 0..count {
        state
            .transcript_publisher()
            .publish(&ActivityEvent {
                workflow_id: workflow_id(),
                run_id: run_id(),
                activity_id: activity_id(),
                attempt: ATTEMPT,
                agent_id: uuid::Uuid::from_u128(42),
                agent_role: "orchestrator".to_owned(),
                emitted_at: DateTime::<Utc>::UNIX_EPOCH,
                worker_seq,
                store_seq: None,
                ephemeral: false,
                kind: ActivityEventKind::Message {
                    role: MessageRole::Assistant,
                    text: format!("line-{worker_seq}"),
                },
            })
            .await?;
    }
    Ok(())
}

/// Register a worker and make it the tracked owner of the current attempt.
///
/// Returns the registration guard: dropping it deregisters the worker, so the
/// caller must hold it for as long as the worker is meant to be live.
fn own_the_attempt(
    state: &ServerState,
    capabilities: InterventionCapabilities,
) -> Result<WorkerRegistration, Box<dyn std::error::Error>> {
    let (sender, _receiver) = tokio::sync::mpsc::channel(1);
    let activity_types = [String::from("review")];
    let registration = state
        .worker_registry()
        .register_delivery_with_capabilities(
            [NAMESPACE.to_owned()],
            aion_core::DEFAULT_TASK_QUEUE,
            None,
            activity_types.iter(),
            crate::worker::WorkerDelivery::Grpc(sender),
            capabilities,
        )?;
    let worker_id = registration
        .worker_id()
        .ok_or("registration did not assign a worker id")?;
    state.attempt_owners().bind(
        AttemptKey::new(workflow_id(), run_id(), activity_id(), ATTEMPT),
        worker_id,
    );
    state.heartbeat_tracker().track_task(
        worker_id,
        InFlightActivity {
            workflow_id: workflow_id(),
            activity_id: activity_id(),
            attempt: ATTEMPT,
            completion_token: crate::worker::CompletionToken::for_test(),
        },
        Instant::now(),
    )?;
    Ok(registration)
}

/// Record a progress note on the current attempt from its owning worker.
fn report_note(
    state: &ServerState,
    registration: &WorkerRegistration,
    note: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let worker_id = registration
        .worker_id()
        .ok_or("registration did not assign a worker id")?;
    state.heartbeat_tracker().record_heartbeat(
        worker_id,
        aion_proto::ProtoHeartbeat {
            workflow_id: Some(aion_proto::ProtoWorkflowId::from(workflow_id())),
            activity_id: Some(aion_proto::ProtoActivityId::from(activity_id())),
            progress: Some(aion_proto::ProtoPayload::from(Payload::from_json(note)?)),
        },
        Instant::now(),
    )?;
    Ok(())
}

async fn describe_live(
    router: &Router,
    fields: serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut request = match fields {
        serde_json::Value::Object(request) => request,
        _other => return Err("describe-live request fields must be an object".into()),
    };
    request.insert("namespace".to_owned(), json!(NAMESPACE));
    request.insert("workflow_id".to_owned(), json!(workflow_id().to_string()));
    let response = router
        .clone()
        .oneshot(json_request("/workflows/describe-live", &request)?)
        .await?;
    if response.status() != StatusCode::OK {
        return Err(format!("describe-live failed with {}", response.status()).into());
    }
    read_json(response).await
}

/// The join composes: summary + status, the folded current step with its
/// stamped address, and the open-step list — all from one call.
#[tokio::test]
async fn the_join_reports_the_summary_and_the_folded_current_step() -> TestResult {
    let state = state_with(&in_flight_history(Utc::now())?, 20_000).await?;
    let router = workflow_router(state);

    let body = describe_live(&router, json!({})).await?;
    assert_eq!(body["summary"]["workflow_id"], workflow_id().to_string());
    assert_eq!(body["summary"]["status"], "Running");
    assert_eq!(body["current_step"]["step"]["activity_type"], "review");
    assert_eq!(body["current_step"]["step"]["task_queue"], "agents");
    assert_eq!(body["current_step"]["step"]["state"]["state"], "Dispatched");
    assert_eq!(body["current_step"]["step"]["state"]["attempt"], 1);
    assert_eq!(body["current_step"]["attempt"]["attempt"], 1);
    assert_eq!(
        body["open_steps"]
            .as_array()
            .ok_or("open_steps missing")?
            .len(),
        1
    );
    Ok(())
}

/// A run with nothing open answers `null` — the run is genuinely not inside an
/// activity, and the response says so rather than omitting the field.
#[tokio::test]
async fn a_run_inside_no_activity_reports_no_current_step() -> TestResult {
    let state = state_with(&[started(Utc::now())?], 20_000).await?;
    let router = workflow_router(state);

    let body = describe_live(&router, json!({})).await?;
    assert_eq!(body["current_step"], serde_json::Value::Null);
    assert!(
        body["open_steps"]
            .as_array()
            .ok_or("open_steps")?
            .is_empty()
    );
    assert!(
        body["unserved"].as_array().ok_or("unserved")?.is_empty(),
        "a run with no dispatch has nothing unserved"
    );
    Ok(())
}

/// A step scheduled but not dispatched has no attempt at all, so liveness, note
/// and transcript are structurally absent rather than reported as empty.
#[tokio::test]
async fn a_scheduled_step_has_no_attempt_to_report() -> TestResult {
    let now = Utc::now();
    let state = state_with(&[started(now)?, scheduled(now)?], 20_000).await?;
    let router = workflow_router(state);

    let body = describe_live(&router, json!({ "tail": 5 })).await?;
    assert_eq!(body["current_step"]["step"]["state"]["state"], "Scheduled");
    assert_eq!(body["current_step"]["attempt"], serde_json::Value::Null);
    Ok(())
}

/// NOTE STATE 1 — reported: the owning worker's note comes back verbatim.
#[tokio::test]
async fn a_reported_note_is_returned_with_its_payload() -> TestResult {
    let state = state_with(&in_flight_history(Utc::now())?, 20_000).await?;
    let registration = own_the_attempt(&state, InterventionCapabilities::none())?;
    report_note(
        &state,
        &registration,
        &json!({ "doing": "reading the brief" }),
    )?;
    let router = workflow_router(state);

    let body = describe_live(&router, json!({})).await?;
    let note = &body["current_step"]["attempt"]["note"];
    assert_eq!(note["note"], "Reported");
    let payload: Payload = serde_json::from_value(note["payload"].clone())?;
    assert_eq!(payload.to_json()?["doing"], "reading the brief");
    assert!(
        note["reported_at"].is_string(),
        "a reported note carries when this process received it"
    );
    drop(registration);
    Ok(())
}

/// NOTE STATE 2 — none sent: the attempt IS tracked here and the worker has
/// reported nothing. This is a measurement, and the response says so.
#[tokio::test]
async fn a_tracked_attempt_with_no_note_reports_silence() -> TestResult {
    let state = state_with(&in_flight_history(Utc::now())?, 20_000).await?;
    let registration = own_the_attempt(&state, InterventionCapabilities::none())?;
    let router = workflow_router(state);

    let body = describe_live(&router, json!({})).await?;
    assert_eq!(body["current_step"]["attempt"]["note"]["note"], "NoneSent");
    drop(registration);
    Ok(())
}

/// NOTE STATE 3 — unavailable, with the restart named: the attempt was
/// dispatched BEFORE this process began holding notes, so the absence of a note
/// says nothing about the worker. Nothing is tracked, exactly as after a
/// restart.
#[tokio::test]
async fn an_attempt_older_than_this_process_reports_unavailable_notes() -> TestResult {
    let long_ago = Utc::now()
        .checked_sub_signed(TimeDelta::hours(1))
        .ok_or("clock underflow")?;
    let state = state_with(&in_flight_history(long_ago)?, 20_000).await?;
    let held_since = state.heartbeat_tracker().notes_held_since();
    let router = workflow_router(state);

    let body = describe_live(&router, json!({})).await?;
    let note = &body["current_step"]["attempt"]["note"];
    assert_eq!(
        note["note"], "Unavailable",
        "a cold store must never be reported as a silent worker"
    );
    assert_eq!(note["restarted_since_attempt_began"], true);
    assert_eq!(note["notes_held_since"], json!(held_since));
    Ok(())
}

/// The restart flag is a MEASUREMENT, not a constant: an attempt dispatched
/// after this process began holding notes, but not tracked by it, is still
/// unavailable — and says a restart does NOT explain it.
#[tokio::test]
async fn an_untracked_recent_attempt_is_unavailable_without_blaming_a_restart() -> TestResult {
    // The dispatch instant is stamped when the state (and its tracker) already
    // exists, so it is strictly after `notes_held_since`.
    let state = state_with(&[], 20_000).await?;
    let dispatched_at = Utc::now();
    let engine = state.engine()?;
    engine
        .store()
        .append(
            WriteToken::recorder(),
            &workflow_id(),
            &in_flight_history(dispatched_at)?,
            0,
        )
        .await?;
    let held_since = state.heartbeat_tracker().notes_held_since();
    assert!(
        dispatched_at > held_since,
        "the fixture must dispatch after the tracker existed"
    );
    let router = workflow_router(state);

    let body = describe_live(&router, json!({})).await?;
    let note = &body["current_step"]["attempt"]["note"];
    assert_eq!(note["note"], "Unavailable");
    assert_eq!(
        note["restarted_since_attempt_began"], false,
        "no restart happened, so none is claimed"
    );
    Ok(())
}

/// Liveness comes from the live attempt→owner index, and reports the owning
/// worker's advertised primitives.
#[tokio::test]
async fn a_live_owner_is_reported_with_its_advertised_capabilities() -> TestResult {
    let state = state_with(&in_flight_history(Utc::now())?, 20_000).await?;
    let registration = own_the_attempt(
        &state,
        InterventionCapabilities::from_primitives([InterventionPrimitive::InjectMessage]),
    )?;
    let router = workflow_router(state);

    let body = describe_live(&router, json!({})).await?;
    let liveness = &body["current_step"]["attempt"]["liveness"];
    assert_eq!(liveness["liveness"], "Live");
    assert_eq!(
        liveness["capabilities"]["supported"][0]["primitive"],
        "InjectMessage"
    );
    drop(registration);
    Ok(())
}

/// The control for the above: with nothing owning the attempt, the same request
/// reports no live owner rather than an empty capability set.
#[tokio::test]
async fn an_unowned_attempt_reports_no_live_owner() -> TestResult {
    let state = state_with(&in_flight_history(Utc::now())?, 20_000).await?;
    let router = workflow_router(state);

    let body = describe_live(&router, json!({})).await?;
    assert_eq!(
        body["current_step"]["attempt"]["liveness"]["liveness"],
        "NoLiveOwner"
    );
    Ok(())
}

/// An omitted `tail` reads no transcript and claims nothing about the stream.
#[tokio::test]
async fn an_omitted_tail_claims_nothing_about_the_transcript() -> TestResult {
    let state = state_with(&in_flight_history(Utc::now())?, 20_000).await?;
    publish_transcript(&state, 3).await?;
    let router = workflow_router(state);

    let body = describe_live(&router, json!({})).await?;
    assert_eq!(
        body["current_step"]["attempt"]["transcript"]["transcript"],
        "NotRequested"
    );
    // The stream heads are still reported: enumeration is not the tail.
    assert_eq!(body["transcript_streams"][0]["head_seq"], 3);
    Ok(())
}

/// A requested tail returns the LAST records and says how many it left behind.
#[tokio::test]
async fn a_requested_tail_returns_the_last_records_and_names_what_it_omitted() -> TestResult {
    let state = state_with(&in_flight_history(Utc::now())?, 20_000).await?;
    publish_transcript(&state, 5).await?;
    let router = workflow_router(state);

    let body = describe_live(&router, json!({ "tail": 2 })).await?;
    let transcript = &body["current_step"]["attempt"]["transcript"];
    assert_eq!(transcript["transcript"], "Window");
    let events = transcript["events"].as_array().ok_or("events missing")?;
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["store_seq"], 3);
    assert_eq!(events[1]["store_seq"], 4);
    assert_eq!(transcript["head_seq"], 5);
    assert_eq!(transcript["omitted_before"], 3);
    assert_eq!(
        transcript["retention_truncated"], false,
        "a stream well inside the cap is not truncated"
    );
    Ok(())
}

/// TRUNCATION HONESTY: with the retention cap crossed, both the tail and the
/// stream head SAY the stream was truncated. The events after the cap exist
/// only on the live socket, and a reader must not read the last retained record
/// as the agent falling silent.
#[tokio::test]
async fn a_capped_stream_says_truncated_rather_than_showing_silence() -> TestResult {
    let state = state_with(&in_flight_history(Utc::now())?, 3).await?;
    // Five events against a cap of three: three are retained, the fourth writes
    // the cap marker, and everything after is live-only.
    publish_transcript(&state, 5).await?;
    let router = workflow_router(state);

    let body = describe_live(&router, json!({ "tail": 10 })).await?;
    let transcript = &body["current_step"]["attempt"]["transcript"];
    assert_eq!(
        transcript["retention_truncated"], true,
        "the tail must say the retention stopped, not imply the agent did"
    );
    assert_eq!(transcript["head_seq"], 4, "three records plus the marker");
    assert_eq!(
        body["transcript_streams"][0]["retention_truncated"], true,
        "the stream enumeration says it too"
    );
    // The marker itself is retained and names the cap, so the last record a
    // reader sees explains the ending.
    let events = transcript["events"].as_array().ok_or("events missing")?;
    let last = events.last().ok_or("the marker is retained")?;
    assert!(
        last["kind"]["detail"]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("retention cap"),
        "the last retained record explains the ending: {last}"
    );
    Ok(())
}

/// Stream enumeration is annotated: each stream names its activity type and
/// whether it is the current attempt's — the two facts a bare `(activity,
/// attempt, head)` list leaves a reader to guess.
#[tokio::test]
async fn transcript_streams_name_their_activity_and_mark_the_current_one() -> TestResult {
    let now = Utc::now();
    // An earlier step with its own retained stream, then the current one. The
    // envelope sequence is contiguous because the store enforces it.
    let history = vec![
        started(now)?,
        Event::ActivityScheduled {
            envelope: envelope(2, now),
            activity_id: ActivityId::from_sequence_position(1),
            activity_type: "plan".to_owned(),
            input: payload()?,
            task_queue: "agents".to_owned(),
            node: None,
        },
        Event::ActivityScheduled {
            envelope: envelope(3, now),
            activity_id: activity_id(),
            activity_type: "review".to_owned(),
            input: payload()?,
            task_queue: "agents".to_owned(),
            node: None,
        },
        Event::ActivityStarted {
            envelope: envelope(4, now),
            activity_id: activity_id(),
            attempt: ATTEMPT,
        },
    ];
    let state = state_with(&history, 20_000).await?;
    publish_transcript(&state, 2).await?;
    state
        .transcript_publisher()
        .publish(&ActivityEvent {
            workflow_id: workflow_id(),
            run_id: run_id(),
            activity_id: ActivityId::from_sequence_position(1),
            attempt: ATTEMPT,
            agent_id: uuid::Uuid::from_u128(42),
            agent_role: "orchestrator".to_owned(),
            emitted_at: DateTime::<Utc>::UNIX_EPOCH,
            worker_seq: 0,
            store_seq: None,
            ephemeral: false,
            kind: ActivityEventKind::Message {
                role: MessageRole::Assistant,
                text: "planning".to_owned(),
            },
        })
        .await?;
    let router = workflow_router(state);

    let body = describe_live(&router, json!({})).await?;
    let streams = body["transcript_streams"]
        .as_array()
        .ok_or("transcript_streams missing")?;
    assert_eq!(streams.len(), 2);
    let plan = streams
        .iter()
        .find(|stream| stream["activity_id"] == json!(1))
        .ok_or("the earlier step's stream is enumerated")?;
    assert_eq!(plan["activity_type"], "plan");
    assert_eq!(plan["current"], false);
    let review = streams
        .iter()
        .find(|stream| stream["activity_id"] == json!(ACTIVITY))
        .ok_or("the current step's stream is enumerated")?;
    assert_eq!(review["activity_type"], "review");
    assert_eq!(
        review["current"], true,
        "the current attempt's stream is marked as such"
    );
    Ok(())
}

/// A caller scoped to another namespace gets the guard's anti-leak answer, not
/// a step, a note, or a transcript.
#[tokio::test]
async fn a_foreign_workflow_is_refused_before_anything_is_read() -> TestResult {
    let state = state_with(&in_flight_history(Utc::now())?, 20_000).await?;
    let router = workflow_router(state);

    let response = router
        .oneshot(json_request(
            "/workflows/describe-live",
            &json!({
                "namespace": "tenant-b",
                "workflow_id": workflow_id().to_string(),
            }),
        )?)
        .await?;
    assert_ne!(
        response.status(),
        StatusCode::OK,
        "a foreign namespace must not receive a live view"
    );
    Ok(())
}

/// A malformed workflow id is a typed input error, not a 500.
#[tokio::test]
async fn a_malformed_workflow_id_is_a_typed_input_error() -> TestResult {
    let state = state_with(&in_flight_history(Utc::now())?, 20_000).await?;
    let router = workflow_router(state);

    let response = router
        .oneshot(json_request(
            "/workflows/describe-live",
            &json!({ "namespace": NAMESPACE, "workflow_id": "not-a-uuid" }),
        )?)
        .await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error: aion_proto::WireError = read_json(response).await?;
    assert_eq!(error.code, aion_proto::WireErrorCode::InvalidInput);
    Ok(())
}

/// The heartbeat window the tracker was built with is irrelevant to the note
/// read, but the fixture pins it so a future change to the default cannot
/// silently expire the tracked attempt mid-test.
#[tokio::test]
async fn the_tracked_attempt_survives_the_read() -> TestResult {
    let state = state_with(&in_flight_history(Utc::now())?, 20_000).await?;
    assert_eq!(
        state.runtime_config().worker.heartbeat_window,
        Duration::from_secs(30)
    );
    let registration = own_the_attempt(&state, InterventionCapabilities::none())?;
    report_note(&state, &registration, &json!({ "doing": "work" }))?;
    let router = workflow_router(state);

    let first = describe_live(&router, json!({})).await?;
    let second = describe_live(&router, json!({})).await?;
    assert_eq!(
        first["current_step"]["attempt"]["note"], second["current_step"]["attempt"]["note"],
        "reading the note must not consume it"
    );
    drop(registration);
    Ok(())
}
