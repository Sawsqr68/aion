//! Shipping-binary e2e for post-attach child transcript visibility.

use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use aion_core::{
    ActivityEvent, ActivityEventKind, ActivityId, Event, EventEnvelope, PackageVersion, Payload,
    RunId, WorkflowId, WorkflowSummary,
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

const PARENT_ID: u128 = 0x2440;
const PARENT_RUN: u128 = 0x2441;
const CHILD_ID: u128 = 0x2442;
const CHILD_RUN: u128 = 0x2443;

#[derive(Clone)]
struct HttpFixture {
    parent_attached: Arc<Notify>,
    emit_fanout: Arc<Notify>,
    transcript_attached: Arc<Notify>,
    emit_live: Arc<Notify>,
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
        emit_live: Arc::new(Notify::new()),
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
    assert_eq!(
        String::from_utf8(output.stdout)?,
        "",
        "tail owns its transcript rendering and must not append a JSON result"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn tail_follow_prints_child_events_emitted_after_attach()
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
    let emit_live = Arc::new(Notify::new());
    let http_state = HttpFixture {
        parent_attached: parent_attached.clone(),
        emit_fanout: emit_fanout.clone(),
        transcript_attached: transcript_attached.clone(),
        emit_live: emit_live.clone(),
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
    let (line_sender, line_receiver) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(line) if line.contains("post-attach-live-leg") => {
                    return line_sender.send(Ok(line));
                }
                Ok(_) => {}
                Err(error) => return line_sender.send(Err(error.to_string())),
            }
        }
        line_sender.send(Err(
            "tail exited before printing the live leg line".to_owned()
        ))
    });

    tokio::time::timeout(Duration::from_secs(10), parent_attached.notified())
        .await
        .map_err(|_| "tail did not attach its parent workflow socket")?;
    emit_fanout.notify_waiters();
    tokio::time::timeout(Duration::from_secs(10), transcript_attached.notified())
        .await
        .map_err(|_| "tail did not attach its transcript socket")?;
    emit_live.notify_waiters();
    let observed = line_receiver.recv_timeout(Duration::from_secs(5));

    child.kill()?;
    let status = child.wait()?;
    let mut stderr_text = String::new();
    BufReader::new(stderr).read_to_string(&mut stderr_text)?;
    reader.join().map_err(|_| "stdout reader panicked")??;
    http_task.abort();
    grpc_task.abort();

    let line = observed
        .map_err(|error| {
            format!(
                "post-attach leg line never appeared ({error}); tail status {status}; stderr: {stderr_text}"
            )
        })?
        .map_err(|error| format!("tail output ended without the live leg line: {error}"))?;
    assert!(line.contains("Builder"), "leg label missing from: {line}");
    assert!(
        line.contains("activity:7 attempt:1"),
        "attempt identity missing from: {line}"
    );
    Ok(())
}

async fn children() -> Json<Value> {
    Json(json!({"children": []}))
}

async fn transcripts(Json(request): Json<Value>) -> Json<Value> {
    drop(request);
    Json(json!({"streams": []}))
}

async fn transcript() -> Json<Value> {
    Json(json!({"events": [], "head_seq": 0}))
}

async fn stream(ws: WebSocketUpgrade, State(state): State<HttpFixture>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| serve_socket(socket, state))
}

async fn serve_socket(mut socket: WebSocket, state: HttpFixture) {
    let Some(Ok(Message::Text(subscription))) = socket.next().await else {
        return;
    };
    let Ok(request) = serde_json::from_str::<Value>(&subscription) else {
        return;
    };
    if request.get("transcript").is_some() {
        serve_transcript(socket, state).await;
        return;
    }
    let workflow_id = request
        .pointer("/per_workflow/workflow_id/uuid")
        .and_then(Value::as_str);
    let parent_id = WorkflowId::new(uuid::Uuid::from_u128(PARENT_ID)).to_string();
    let child_id = WorkflowId::new(uuid::Uuid::from_u128(CHILD_ID)).to_string();
    if workflow_id == Some(parent_id.as_str()) {
        if request.pointer("/per_workflow/resume_from_seq") != Some(&json!(1)) {
            return;
        }
        state.parent_attached.notify_waiters();
        state.emit_fanout.notified().await;
        let Some(event) = child_started() else {
            return;
        };
        if !send_workflow_event(&mut socket, event).await {
            return;
        }
    } else if workflow_id == Some(child_id.as_str()) {
        let Some(started) = child_run_started() else {
            return;
        };
        for event in [started, child_activity_started()] {
            if !send_workflow_event(&mut socket, event).await {
                return;
            }
        }
    }
    while socket.next().await.is_some() {}
}

async fn serve_transcript(mut socket: WebSocket, state: HttpFixture) {
    state.transcript_attached.notify_waiters();
    state.emit_live.notified().await;
    let Ok(frame) = serde_json::to_string(&StreamedActivityEvent::new(live_event())) else {
        return;
    };
    if socket.send(Message::Text(frame.into())).await.is_err() {
        return;
    }
    while socket.next().await.is_some() {}
}

async fn send_workflow_event(socket: &mut WebSocket, event: Event) -> bool {
    let Ok(encoded) = encode_streamed_event("default", None, &event) else {
        return false;
    };
    let Ok(frame) = serde_json::to_string(&encoded) else {
        return false;
    };
    socket.send(Message::Text(frame.into())).await.is_ok()
}

fn child_started() -> Option<Event> {
    Some(Event::ChildWorkflowStarted {
        envelope: event_envelope(2, PARENT_ID),
        child_workflow_id: WorkflowId::new(uuid::Uuid::from_u128(CHILD_ID)),
        workflow_type: "Builder".to_owned(),
        input: Payload::from_json(&json!({})).ok()?,
        package_version: PackageVersion::new("a".repeat(64)),
    })
}

fn child_run_started() -> Option<Event> {
    Some(Event::WorkflowStarted {
        envelope: event_envelope(1, CHILD_ID),
        workflow_type: "Builder".to_owned(),
        input: Payload::from_json(&json!({})).ok()?,
        run_id: RunId::new(uuid::Uuid::from_u128(CHILD_RUN)),
        parent_run_id: Some(RunId::new(uuid::Uuid::from_u128(PARENT_RUN))),
        package_version: PackageVersion::new("a".repeat(64)),
    })
}

fn child_activity_started() -> Event {
    Event::ActivityStarted {
        envelope: event_envelope(2, CHILD_ID),
        activity_id: ActivityId::from_sequence_position(7),
        attempt: 1,
    }
}

fn event_envelope(seq: u64, workflow_id: u128) -> EventEnvelope {
    EventEnvelope {
        seq,
        recorded_at: DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap_or_default(),
        workflow_id: WorkflowId::new(uuid::Uuid::from_u128(workflow_id)),
    }
}

fn live_event() -> ActivityEvent {
    ActivityEvent {
        workflow_id: WorkflowId::new(uuid::Uuid::from_u128(CHILD_ID)),
        run_id: RunId::new(uuid::Uuid::from_u128(CHILD_RUN)),
        activity_id: ActivityId::from_sequence_position(7),
        attempt: 1,
        agent_id: uuid::Uuid::from_u128(0x2444),
        agent_role: "builder".to_owned(),
        emitted_at: DateTime::<Utc>::from_timestamp(1_700_000_001, 0).unwrap_or_default(),
        worker_seq: 1,
        store_seq: Some(1),
        ephemeral: false,
        kind: ActivityEventKind::Raw {
            source: "post-attach-live-leg".to_owned(),
            value: json!({"line": "visible only through the live socket"}),
        },
    }
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

fn generated_envelope(value: aion_proto::WireEnvelope) -> generated::WireEnvelope {
    generated::WireEnvelope {
        namespace: value.namespace,
        request_id: value.request_id,
        payload: value.payload.map(|payload| generated::Payload {
            content_type: payload.content_type,
            bytes: payload.bytes,
        }),
    }
}
