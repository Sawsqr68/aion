//! Shipping-binary e2e for post-attach child transcript visibility.

#[path = "tail_follow_e2e/fixtures.rs"]
mod tail_follow_fixtures;

use std::io::{BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use aion_core::{
    Event, EventEnvelope, PackageVersion, Payload, RunId, WorkflowId, WorkflowSummary,
};
use aion_proto::generated;
use aion_proto::{StreamedActivityEvent, encode_streamed_event};
use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Json, State};
use axum::response::IntoResponse;
use axum::routing::{any, post};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Request, Response, Status};

use tail_follow_fixtures::{
    assert_replay_outcome, backfill_event, child_run_events, generated_envelope, live_event,
    replayed_parent_events, selected_parent_live_event, spawn_line_reader,
};

const PARENT_ID: u128 = 0x2440;
const PARENT_RUN: u128 = 0x2441;
const CHILD_ID: u128 = 0x2442;
const CHILD_RUN: u128 = 0x2443;
const PRIOR_PARENT_RUN: u128 = 0x2431;
const PRIOR_CHILD_ID: u128 = 0x2432;

#[derive(Clone)]
struct HttpFixture {
    parent_attached: Arc<Notify>,
    emit_fanout: Arc<Notify>,
    transcript_attached: Arc<Notify>,
    selected_parent_transcript_attached: Arc<Notify>,
    selected_child_attached: Arc<Notify>,
    emit_live: Arc<Notify>,
    prior_child_attached: Arc<AtomicBool>,
}

#[derive(Clone)]
struct DescribeFixture {
    response: generated::DescribeWorkflowResponse,
}

macro_rules! impl_describe_fixture {
    ($(($name:ident, $request:ty, $response:ty)),+ $(,)?) => {
        #[tonic::async_trait]
        impl generated::workflow_service_server::WorkflowService for DescribeFixture {
            $(
                async fn $name(
                    &self,
                    request: Request<$request>,
                ) -> Result<Response<$response>, Status> {
                    drop(request);
                    Err(Status::unimplemented(stringify!($name)))
                }
            )+

            async fn describe_workflow(
                &self,
                request: Request<generated::DescribeWorkflowRequest>,
            ) -> Result<Response<generated::DescribeWorkflowResponse>, Status> {
                drop(request);
                Ok(Response::new(self.response.clone()))
            }
        }
    };
}

impl_describe_fixture!(
    (
        start_workflow,
        generated::StartWorkflowRequest,
        generated::StartWorkflowResponse
    ),
    (signal, generated::SignalRequest, generated::SignalResponse),
    (query, generated::QueryRequest, generated::QueryResponse),
    (cancel, generated::CancelRequest, generated::CancelResponse),
    (reopen, generated::ReopenRequest, generated::ReopenResponse),
    (pause, generated::PauseRequest, generated::PauseResponse),
    (resume, generated::ResumeRequest, generated::ResumeResponse),
    (rename, generated::RenameRequest, generated::RenameResponse),
    (
        list_workflows,
        generated::ListWorkflowsRequest,
        generated::ListWorkflowsResponse
    ),
    (
        count_workflows,
        generated::CountWorkflowsRequest,
        generated::CountWorkflowsResponse
    ),
    (
        create_schedule,
        generated::CreateScheduleRequest,
        generated::CreateScheduleResponse
    ),
    (
        update_schedule,
        generated::UpdateScheduleRequest,
        generated::UpdateScheduleResponse
    ),
    (
        pause_schedule,
        generated::ScheduleIdRequest,
        generated::PauseScheduleResponse
    ),
    (
        resume_schedule,
        generated::ScheduleIdRequest,
        generated::ResumeScheduleResponse
    ),
    (
        delete_schedule,
        generated::ScheduleIdRequest,
        generated::DeleteScheduleResponse
    ),
    (
        list_schedules,
        generated::ListSchedulesRequest,
        generated::ListSchedulesResponse
    ),
    (
        describe_schedule,
        generated::ScheduleIdRequest,
        generated::DescribeScheduleResponse
    ),
    (
        mint_namespace,
        generated::MintNamespaceRequest,
        generated::MintNamespaceResponse
    ),
);

#[tokio::test(flavor = "multi_thread")]
async fn tail_no_follow_does_not_append_json_null_to_its_transcript_output()
-> Result<(), Box<dyn std::error::Error>> {
    let grpc_listener = TcpListener::bind("127.0.0.1:0").await?;
    let grpc_address = grpc_listener.local_addr()?;
    let grpc = tonic::transport::Server::builder()
        .add_service(
            generated::workflow_service_server::WorkflowServiceServer::new(DescribeFixture {
                response: describe_response()?,
            }),
        )
        .serve_with_incoming(TcpListenerStream::new(grpc_listener));
    let grpc_task = tokio::spawn(grpc);

    let http_state = HttpFixture {
        parent_attached: Arc::new(Notify::new()),
        emit_fanout: Arc::new(Notify::new()),
        transcript_attached: Arc::new(Notify::new()),
        selected_parent_transcript_attached: Arc::new(Notify::new()),
        selected_child_attached: Arc::new(Notify::new()),
        emit_live: Arc::new(Notify::new()),
        prior_child_attached: Arc::new(AtomicBool::new(false)),
    };
    let app = Router::new()
        .route("/workflows/children", post(children))
        .route("/workflows/transcripts", post(transcripts))
        .route("/workflows/transcript", post(transcript))
        .route("/events/stream", any(stream))
        .with_state(http_state);
    let http_listener = TcpListener::bind("127.0.0.1:0").await?;
    let http_address = http_listener.local_addr()?;
    let http_task = tokio::spawn(axum::serve(http_listener, app).into_future());

    let output = Command::new(env!("CARGO_BIN_EXE_aion"))
        .args([
            "--endpoint",
            &format!("http://{grpc_address}"),
            "tail",
            &WorkflowId::new(uuid::Uuid::from_u128(PARENT_ID)).to_string(),
            "--run-id",
            &RunId::new(uuid::Uuid::from_u128(PARENT_RUN)).to_string(),
            "--http-endpoint",
            &format!("http://{http_address}"),
            "--no-follow",
        ])
        .output()?;
    http_task.abort();
    grpc_task.abort();

    assert!(output.status.success(), "tail failed: {output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("retained-backfill-parent"), "{stdout}");
    assert!(
        !stdout.lines().any(|line| line == "null"),
        "tail owns its transcript rendering and must not append a JSON result: {stdout}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn tail_follow_attributes_replay_only_after_the_selected_generation_starts()
-> Result<(), Box<dyn std::error::Error>> {
    let grpc_listener = TcpListener::bind("127.0.0.1:0").await?;
    let grpc_address = grpc_listener.local_addr()?;
    let grpc = tonic::transport::Server::builder()
        .add_service(
            generated::workflow_service_server::WorkflowServiceServer::new(DescribeFixture {
                response: describe_response()?,
            }),
        )
        .serve_with_incoming(TcpListenerStream::new(grpc_listener));
    let grpc_task = tokio::spawn(grpc);

    let parent_attached = Arc::new(Notify::new());
    let emit_fanout = Arc::new(Notify::new());
    let transcript_attached = Arc::new(Notify::new());
    let selected_parent_transcript_attached = Arc::new(Notify::new());
    let selected_child_attached = Arc::new(Notify::new());
    let emit_live = Arc::new(Notify::new());
    let prior_child_attached = Arc::new(AtomicBool::new(false));
    let http_state = HttpFixture {
        parent_attached: parent_attached.clone(),
        emit_fanout: emit_fanout.clone(),
        transcript_attached: transcript_attached.clone(),
        selected_parent_transcript_attached: selected_parent_transcript_attached.clone(),
        selected_child_attached: selected_child_attached.clone(),
        emit_live: emit_live.clone(),
        prior_child_attached: prior_child_attached.clone(),
    };
    let app = Router::new()
        .route("/workflows/children", post(children))
        .route("/workflows/transcripts", post(transcripts))
        .route("/workflows/transcript", post(transcript))
        .route("/events/stream", any(stream))
        .with_state(http_state);
    let http_listener = TcpListener::bind("127.0.0.1:0").await?;
    let http_address = http_listener.local_addr()?;
    let http_task = tokio::spawn(axum::serve(http_listener, app).into_future());

    let mut command = Command::new(env!("CARGO_BIN_EXE_aion"));
    command
        .args([
            "--endpoint",
            &format!("http://{grpc_address}"),
            "tail",
            &WorkflowId::new(uuid::Uuid::from_u128(PARENT_ID)).to_string(),
            "--run-id",
            &RunId::new(uuid::Uuid::from_u128(PARENT_RUN)).to_string(),
            "--http-endpoint",
            &format!("http://{http_address}"),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let stdout = child.stdout.take().ok_or("tail stdout was not piped")?;
    let stderr = child.stderr.take().ok_or("tail stderr was not piped")?;
    let (line_receiver, reader) = spawn_line_reader(stdout);

    tokio::time::timeout(Duration::from_secs(10), parent_attached.notified())
        .await
        .map_err(|_| "tail did not attach its parent workflow socket")?;
    emit_fanout.notify_waiters();
    let selected_child_followed =
        tokio::time::timeout(Duration::from_secs(5), selected_child_attached.notified())
            .await
            .is_ok();
    let parent_attempt_attached = tokio::time::timeout(
        Duration::from_secs(5),
        selected_parent_transcript_attached.notified(),
    )
    .await
    .is_ok();
    if parent_attempt_attached {
        emit_live.notify_waiters();
    }
    let mut lines = Vec::new();
    while let Ok(line) = line_receiver.recv_timeout(Duration::from_secs(1)) {
        let line = line.map_err(|error| format!("tail stdout read failed: {error}"))?;
        lines.push(line);
        if lines
            .iter()
            .any(|line| line.contains("selected-run-live-parent"))
        {
            break;
        }
    }

    child.kill()?;
    let status = child.wait()?;
    let mut stderr_text = String::new();
    BufReader::new(stderr).read_to_string(&mut stderr_text)?;
    reader.join().map_err(|_| "stdout reader panicked")?;
    http_task.abort();
    grpc_task.abort();

    assert_replay_outcome(
        prior_child_attached.load(Ordering::SeqCst),
        &lines,
        parent_attempt_attached,
        selected_child_followed,
        status,
        &stderr_text,
    );
    Ok(())
}

async fn children(Json(request): Json<Value>) -> Json<Value> {
    assert_eq!(
        request,
        json!({
            "namespace": "default",
            "workflow_id": WorkflowId::new(uuid::Uuid::from_u128(PARENT_ID)),
            "run_id": RunId::new(uuid::Uuid::from_u128(PARENT_RUN)),
        })
    );
    Json(json!({"children": []}))
}

async fn transcripts(Json(request): Json<Value>) -> Json<Value> {
    assert_eq!(
        request,
        json!({
            "namespace": "default",
            "workflow_id": WorkflowId::new(uuid::Uuid::from_u128(PARENT_ID)),
            "run_id": RunId::new(uuid::Uuid::from_u128(PARENT_RUN)),
        })
    );
    Json(json!({
        "streams": [{"activity_id": 7, "attempt": 1}]
    }))
}

async fn transcript(Json(request): Json<Value>) -> Json<Value> {
    assert_eq!(
        request,
        json!({
            "namespace": "default",
            "workflow_id": WorkflowId::new(uuid::Uuid::from_u128(PARENT_ID)),
            "run_id": RunId::new(uuid::Uuid::from_u128(PARENT_RUN)),
            "activity_id": 7,
            "attempt": 1,
            "last": 2_000,
        })
    );
    Json(json!({"events": [backfill_event()], "head_seq": 2}))
}

async fn stream(ws: WebSocketUpgrade, State(state): State<HttpFixture>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| serve_socket(socket, state))
}

async fn serve_socket(mut socket: WebSocket, state: HttpFixture) {
    let Some(Ok(Message::Text(subscription))) = socket.next().await else {
        return;
    };
    let request = match serde_json::from_str::<Value>(&subscription) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("tail fixture could not decode subscription JSON: {error}");
            return;
        }
    };
    if request
        .get("transcript")
        .is_some_and(|transcript| !transcript.is_null())
    {
        let workflow_id = request
            .pointer("/transcript/workflow_id/uuid")
            .and_then(Value::as_str);
        let child_id = WorkflowId::new(uuid::Uuid::from_u128(CHILD_ID)).to_string();
        let parent_id = WorkflowId::new(uuid::Uuid::from_u128(PARENT_ID)).to_string();
        let attempt = request
            .pointer("/transcript/attempt")
            .and_then(Value::as_u64);
        if workflow_id == Some(child_id.as_str()) {
            serve_transcript(
                socket,
                state.transcript_attached,
                state.emit_live,
                live_event(),
            )
            .await;
        } else if workflow_id == Some(parent_id.as_str()) && attempt == Some(2) {
            serve_transcript(
                socket,
                state.selected_parent_transcript_attached,
                state.emit_live,
                selected_parent_live_event(),
            )
            .await;
        } else {
            while socket.next().await.is_some() {}
        }
        return;
    }
    let workflow_id = request
        .pointer("/per_workflow/workflow_id/uuid")
        .and_then(Value::as_str);
    let parent_id = WorkflowId::new(uuid::Uuid::from_u128(PARENT_ID)).to_string();
    let child_id = WorkflowId::new(uuid::Uuid::from_u128(CHILD_ID)).to_string();
    let prior_child_id = WorkflowId::new(uuid::Uuid::from_u128(PRIOR_CHILD_ID)).to_string();
    if workflow_id == Some(parent_id.as_str()) {
        if request.pointer("/per_workflow/resume_from_seq") != Some(&json!(1)) {
            return;
        }
        state.parent_attached.notify_waiters();
        state.emit_fanout.notified().await;
        let Some(events) = replayed_parent_events() else {
            return;
        };
        for event in events {
            if !send_workflow_event(&mut socket, event).await {
                return;
            }
        }
    } else if workflow_id == Some(child_id.as_str()) {
        state.selected_child_attached.notify_one();
        let Some(events) = child_run_events() else {
            return;
        };
        for event in events {
            if !send_workflow_event(&mut socket, event).await {
                return;
            }
        }
    } else if workflow_id == Some(prior_child_id.as_str()) {
        state.prior_child_attached.store(true, Ordering::SeqCst);
    }
    while socket.next().await.is_some() {}
}

async fn serve_transcript(
    mut socket: WebSocket,
    attached: Arc<Notify>,
    emit_live: Arc<Notify>,
    event: aion_core::ActivityEvent,
) {
    attached.notify_one();
    emit_live.notified().await;
    let frame = match serde_json::to_string(&StreamedActivityEvent::new(event)) {
        Ok(frame) => frame,
        Err(error) => {
            eprintln!("tail fixture could not encode a live transcript frame: {error}");
            return;
        }
    };
    if socket.send(Message::Text(frame.into())).await.is_err() {
        return;
    }
    while socket.next().await.is_some() {}
}

async fn send_workflow_event(socket: &mut WebSocket, event: Event) -> bool {
    let encoded = match encode_streamed_event("default", None, &event) {
        Ok(encoded) => encoded,
        Err(error) => {
            eprintln!("tail fixture could not encode a workflow event: {error}");
            return false;
        }
    };
    let frame = match serde_json::to_string(&encoded) {
        Ok(frame) => frame,
        Err(error) => {
            eprintln!("tail fixture could not serialize a workflow frame: {error}");
            return false;
        }
    };
    socket.send(Message::Text(frame.into())).await.is_ok()
}

fn describe_response() -> Result<generated::DescribeWorkflowResponse, Box<dyn std::error::Error>> {
    let workflow_id = WorkflowId::new(uuid::Uuid::from_u128(PARENT_ID));
    let started_at = DateTime::<Utc>::from_timestamp(1_700_000_000, 0)
        .ok_or("test timestamp must be representable")?;
    let started = Event::WorkflowStarted {
        envelope: EventEnvelope {
            seq: 1,
            recorded_at: started_at,
            workflow_id,
        },
        workflow_type: "parent".to_owned(),
        input: Payload::from_json(&json!({}))?,
        run_id: RunId::new(uuid::Uuid::from_u128(PARENT_RUN)),
        parent_run_id: None,
        package_version: PackageVersion::new("a".repeat(64)),
    };
    let summary = WorkflowSummary::from_history(std::slice::from_ref(&started))
        .ok_or("started history must produce a summary")?;
    Ok(generated::DescribeWorkflowResponse {
        summary: Some(generated_envelope(aion_proto::encode_core_value(
            "default", None, &summary,
        )?)),
        history: vec![generated_envelope(aion_proto::encode_event(
            "default", None, &started,
        )?)],
    })
}
