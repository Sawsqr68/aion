use aion_proto::{
    PerWorkflowSubscription, ProtoWorkflowId, SubscriptionRequest, WireError,
    encode_streamed_event, subscription_request,
};
use serde_json::json;

use crate::client::{ClientAuth, ClientBuilder, ClientConfig};
use crate::error::ClientError;

fn per_workflow_request(resume_from_seq: Option<u64>) -> SubscriptionRequest {
    SubscriptionRequest {
        subscription: Some(subscription_request::Subscription::PerWorkflow(
            PerWorkflowSubscription {
                namespace: String::from("tenant-a"),
                workflow_id: Some(ProtoWorkflowId {
                    uuid: String::from("00000000-0000-0000-0000-000000000001"),
                }),
                resume_from_seq,
            },
        )),
    }
}

use std::collections::HashMap;
use std::sync::Arc;

use futures::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

/// What one server-side accept observed: the upgrade-request headers and
/// the decoded first (subscription) frame.
struct CapturedAttempt {
    headers: HashMap<String, String>,
    subscription: serde_json::Value,
}

enum AttemptEnd {
    CleanClose,
    Drop,
}

/// Handshake callback capturing request headers into the shared map; a
/// named `Callback` impl because the trait fixes the (large)
/// `ErrorResponse` error type in its signature.
struct CaptureHeaders(Arc<std::sync::Mutex<HashMap<String, String>>>);

impl tokio_tungstenite::tungstenite::handshake::server::Callback for CaptureHeaders {
    fn on_request(
        self,
        request: &tokio_tungstenite::tungstenite::handshake::server::Request,
        response: tokio_tungstenite::tungstenite::handshake::server::Response,
    ) -> Result<
        tokio_tungstenite::tungstenite::handshake::server::Response,
        tokio_tungstenite::tungstenite::handshake::server::ErrorResponse,
    > {
        if let Ok(mut headers) = self.0.lock() {
            for (name, value) in request.headers() {
                if let Ok(value) = value.to_str() {
                    headers.insert(name.as_str().to_owned(), value.to_owned());
                }
            }
        }
        Ok(response)
    }
}

async fn accept_one(
    listener: &TcpListener,
    responses: Vec<Message>,
    end: AttemptEnd,
) -> Result<CapturedAttempt, Box<dyn std::error::Error + Send + Sync>> {
    let (stream, _) = listener.accept().await?;
    let captured: Arc<std::sync::Mutex<HashMap<String, String>>> =
        Arc::new(std::sync::Mutex::new(HashMap::new()));
    let sink = Arc::clone(&captured);
    let mut socket = tokio_tungstenite::accept_hdr_async(stream, CaptureHeaders(sink)).await?;
    let first = socket
        .next()
        .await
        .ok_or("client sent no subscription frame")??;
    let Message::Text(text) = first else {
        return Err(format!("expected a text subscription frame, got {first:?}").into());
    };
    let subscription: serde_json::Value = serde_json::from_str(text.as_str())?;
    for frame in responses {
        socket.send(frame).await?;
    }
    match end {
        AttemptEnd::CleanClose => {
            socket
                .send(Message::Close(Some(CloseFrame {
                    code: CloseCode::Normal,
                    reason: "".into(),
                })))
                .await?;
            // Drain until the close handshake completes.
            while let Some(message) = socket.next().await {
                drop(message);
            }
        }
        AttemptEnd::Drop => drop(socket),
    }
    let headers = captured
        .lock()
        .map_err(|_| "captured-header mutex poisoned")?
        .clone();
    Ok(CapturedAttempt {
        headers,
        subscription,
    })
}

fn event_frame(
    seq: u64,
    workflow_id: &aion_core::WorkflowId,
) -> Result<Message, Box<dyn std::error::Error + Send + Sync>> {
    let event = aion_core::Event::SignalReceived {
        envelope: aion_core::EventEnvelope {
            seq,
            recorded_at: chrono::Utc::now(),
            workflow_id: workflow_id.clone(),
        },
        name: format!("signal-{seq}"),
        payload: aion_core::Payload::from_json(&json!({ "seq": seq }))?,
    };
    let frame = serde_json::to_string(&encode_streamed_event("tenant-a", None, &event)?)?;
    Ok(Message::Text(frame.into()))
}

fn live_config(port: u16) -> ClientConfig {
    ClientConfig::from(
        ClientBuilder::new("http://127.0.0.1:50051")
            .with_stream_endpoint(format!("ws://127.0.0.1:{port}/events/stream"))
            .with_auth(ClientAuth::bearer("secret-token"))
            .with_subject("alice")
            .with_namespace("tenant-a")
            .with_authorized_namespaces(["tenant-a", "tenant-b"]),
    )
}

#[tokio::test]
async fn open_subscription_streams_events_and_forwards_identity_headers()
-> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let workflow_id = aion_core::WorkflowId::new_v4();
    let server = tokio::spawn(async move {
        accept_one(
            &listener,
            vec![event_frame(5, &workflow_id)?, event_frame(6, &workflow_id)?],
            AttemptEnd::CleanClose,
        )
        .await
    });

    let attempt = super::open_subscription(&live_config(port), per_workflow_request(None), Some(5))
        .await
        .map_err(|error| format!("open_subscription failed: {error}"))?;
    let delivered: Vec<_> = attempt.events.collect().await;
    let captured = tokio::time::timeout(std::time::Duration::from_secs(5), server)
        .await??
        .map_err(|error| format!("server side failed: {error}"))?;

    // The upgrade request carried the caller identity headers.
    assert_eq!(
        captured.headers.get("authorization").map(String::as_str),
        Some("Bearer secret-token")
    );
    assert_eq!(
        captured.headers.get("x-aion-subject").map(String::as_str),
        Some("alice")
    );
    assert_eq!(
        captured
            .headers
            .get("x-aion-namespaces")
            .map(String::as_str),
        Some("tenant-a,tenant-b")
    );
    // The first frame is the per-workflow subscription with the cursor.
    assert_eq!(
        captured.subscription["per_workflow"]["resume_from_seq"],
        json!(5)
    );
    assert_eq!(
        captured.subscription["per_workflow"]["namespace"],
        json!("tenant-a")
    );
    // Both events decoded; the clean close ended the stream.
    let seqs = delivered
        .into_iter()
        .map(|item| item.map(|event| event.seq()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("stream item failed: {error}"))?;
    assert_eq!(seqs, vec![5, 6]);
    Ok(())
}

#[tokio::test]
async fn abrupt_socket_drop_surfaces_one_unavailable_item() -> Result<(), Box<dyn std::error::Error>>
{
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let workflow_id = aion_core::WorkflowId::new_v4();
    let server = tokio::spawn(async move {
        accept_one(
            &listener,
            vec![event_frame(1, &workflow_id)?],
            AttemptEnd::Drop,
        )
        .await
    });

    let attempt = super::open_subscription(&live_config(port), per_workflow_request(None), None)
        .await
        .map_err(|error| format!("open_subscription failed: {error}"))?;
    let delivered: Vec<_> = attempt.events.collect().await;
    tokio::time::timeout(std::time::Duration::from_secs(5), server)
        .await??
        .map_err(|error| format!("server side failed: {error}"))?;

    assert_eq!(delivered.len(), 2, "one event then the transient error");
    assert!(matches!(&delivered[0], Ok(event) if event.seq() == 1));
    assert!(
        matches!(
            delivered[1].as_ref().err(),
            Some(ClientError::Unavailable { .. })
        ),
        "an abrupt drop must surface retryable Unavailable, got {:?}",
        delivered[1]
    );
    Ok(())
}

#[tokio::test]
async fn terminal_error_frame_ends_the_attempt_with_the_mapped_error()
-> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let error_frame = serde_json::to_string(&json!({
        "error": WireError::not_found("workflow not found in namespace tenant-a")
    }))?;
    let server = tokio::spawn(async move {
        accept_one(
            &listener,
            vec![Message::Text(error_frame.into())],
            AttemptEnd::CleanClose,
        )
        .await
    });

    let attempt = super::open_subscription(&live_config(port), per_workflow_request(None), None)
        .await
        .map_err(|error| format!("open_subscription failed: {error}"))?;
    let delivered: Vec<_> = attempt.events.collect().await;
    tokio::time::timeout(std::time::Duration::from_secs(5), server)
        .await??
        .map_err(|error| format!("server side failed: {error}"))?;

    assert_eq!(
        delivered,
        vec![Err(ClientError::not_found(
            "workflow not found in namespace tenant-a"
        ))]
    );
    Ok(())
}

/// Full resume-loop protocol flow over real sockets: attempt one delivers
/// events 1-2 and drops; the reconnect must carry `resume_from_seq = 3`
/// and splice the remainder without gaps or duplicates.
#[tokio::test]
async fn resume_loop_reconnects_with_the_cursor_over_a_real_socket()
-> Result<(), Box<dyn std::error::Error>> {
    use crate::stream::{ResumingEventStream, SubscribeTarget};
    use crate::transport::grpc::GrpcWorkflowTransport;

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let workflow_id = aion_core::WorkflowId::new_v4();
    let server_workflow = workflow_id.clone();
    let server = tokio::spawn(async move {
        let first = accept_one(
            &listener,
            vec![
                event_frame(1, &server_workflow)?,
                event_frame(2, &server_workflow)?,
            ],
            AttemptEnd::Drop,
        )
        .await?;
        let second = accept_one(
            &listener,
            vec![event_frame(3, &server_workflow)?],
            AttemptEnd::CleanClose,
        )
        .await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>((first, second))
    });

    // A lazy channel never dials: only the WebSocket side is exercised.
    let channel = tonic::transport::Endpoint::from_static("http://127.0.0.1:1").connect_lazy();
    let transport = Arc::new(GrpcWorkflowTransport::from_channel(
        live_config(port),
        channel,
    ));
    let mut events = ResumingEventStream::new(
        transport,
        "tenant-a",
        SubscribeTarget::Workflow { workflow_id },
    );

    let mut seqs = Vec::new();
    while let Some(item) = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        futures::StreamExt::next(&mut events),
    )
    .await?
    {
        seqs.push(item.map(|event| event.seq()));
    }
    let (first, second) = tokio::time::timeout(std::time::Duration::from_secs(5), server)
        .await??
        .map_err(|error| format!("server side failed: {error}"))?;

    let seqs = seqs
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("stream item failed: {error}"))?;
    assert_eq!(seqs, vec![1, 2, 3], "gap-free and duplicate-free delivery");
    assert_eq!(
        first.subscription["per_workflow"]["resume_from_seq"],
        json!(null),
        "the initial attach is a live tail"
    );
    assert_eq!(
        second.subscription["per_workflow"]["resume_from_seq"],
        json!(3),
        "the reconnect must resume from last delivered + 1"
    );
    Ok(())
}
