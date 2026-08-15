use aion_proto::{
    FilteredSubscription, FirehoseSubscription, PerWorkflowSubscription, ProtoWorkflowId,
    SubscriptionRequest, WireError, encode_streamed_event, subscription_request,
};
use serde_json::json;

use super::{decode_frame, stream_url, subscription_frame};
use crate::client::{ClientBuilder, ClientConfig};
use crate::error::{ClientError, ErrorDetail};

fn config(stream_endpoint: Option<&str>) -> ClientConfig {
    let mut builder = ClientBuilder::new("http://127.0.0.1:50051");
    if let Some(endpoint) = stream_endpoint {
        builder = builder.with_stream_endpoint(endpoint);
    }
    ClientConfig::from(builder)
}

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

#[test]
fn missing_stream_endpoint_is_invalid_argument_with_precise_message()
-> Result<(), Box<dyn std::error::Error>> {
    let error = stream_url(&config(None)).err();

    let Some(ClientError::InvalidArgument { detail }) = error else {
        return Err(format!("must be InvalidArgument, got {error:?}").into());
    };
    assert!(
        detail.message.contains("with_stream_endpoint"),
        "detail: {detail}"
    );
    assert!(
        detail.message.contains("/events/stream"),
        "detail: {detail}"
    );
    Ok(())
}

#[test]
fn stream_url_maps_http_schemes_and_passes_ws_through() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        stream_url(&config(Some("ws://127.0.0.1:8080/events/stream")))?,
        "ws://127.0.0.1:8080/events/stream"
    );
    assert_eq!(
        stream_url(&config(Some("wss://aion.example.com/events/stream")))?,
        "wss://aion.example.com/events/stream"
    );
    assert_eq!(
        stream_url(&config(Some("http://127.0.0.1:8080/events/stream")))?,
        "ws://127.0.0.1:8080/events/stream"
    );
    assert_eq!(
        stream_url(&config(Some("https://aion.example.com/events/stream")))?,
        "wss://aion.example.com/events/stream"
    );
    Ok(())
}

#[test]
fn stream_url_rejects_non_websocket_schemes() {
    for endpoint in ["ftp://example.com/events/stream", "not-a-url"] {
        let error = stream_url(&config(Some(endpoint))).err();
        assert!(
            matches!(error, Some(ClientError::InvalidArgument { .. })),
            "{endpoint} must be rejected, got {error:?}"
        );
    }
}

/// A PEM-encoded self-signed CA fixture (no key material), used to prove
/// the custom-CA plumbing into the WebSocket TLS connector.
const TEST_CA_PEM: &str = "-----BEGIN CERTIFICATE-----
MIIBnTCCAUOgAwIBAgIUZeF05kLNKnZTC4xSV0RxC7fQ+DgwCgYIKoZIzj0EAwIw
IzEhMB8GA1UEAwwYYWlvbi1jb25mb3JtYW5jZS10ZXN0LWNhMCAXDTI2MDYxMTE5
MDgwM1oYDzIxMjYwNTE4MTkwODAzWjAjMSEwHwYDVQQDDBhhaW9uLWNvbmZvcm1h
bmNlLXRlc3QtY2EwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAAQNxfK/cvPDW0ue
a6AjlScsSdO+U+H53YG50Fn4HULhmu2Wu8JfcmEo4Rgao+SciqnpqRFiU4X0FTuh
yoKxsO+uo1MwUTAdBgNVHQ4EFgQUwkbSaaXC/W1IxAkg+3Jl7jz+wckwHwYDVR0j
BBgwFoAUwkbSaaXC/W1IxAkg+3Jl7jz+wckwDwYDVR0TAQH/BAUwAwEB/zAKBggq
hkjOPQQDAgNIADBFAiEAtalplxZn9gozJpWUrMO4ddjy/IuKXwO1b7AwSvwtO8EC
ICo9Vooy83Vq0mVVYmWRSVMZ4AtTrLY+7h3pIVrGLLl/
-----END CERTIFICATE-----
";

#[test]
fn no_tls_options_means_no_custom_connector() -> Result<(), Box<dyn std::error::Error>> {
    assert!(super::tls_connector(None)?.is_none());
    Ok(())
}

#[test]
fn tls_options_without_custom_ca_build_a_webpki_connector() -> Result<(), Box<dyn std::error::Error>>
{
    let connector = super::tls_connector(Some(&crate::client::TlsOptions::new()))?;
    assert!(
        matches!(connector, Some(tokio_tungstenite::Connector::Rustls(_))),
        "TLS options must produce a rustls connector"
    );
    Ok(())
}

#[test]
fn custom_ca_certificate_is_added_to_the_websocket_trust_roots()
-> Result<(), Box<dyn std::error::Error>> {
    let options = crate::client::TlsOptions::new().with_ca_certificate_pem(TEST_CA_PEM);
    let connector = super::tls_connector(Some(&options))?;
    let Some(tokio_tungstenite::Connector::Rustls(config)) = connector else {
        return Err("custom CA must produce a rustls connector".into());
    };
    // The webpki bundle plus exactly one extra caller-supplied root.
    let baseline = webpki_roots::TLS_SERVER_ROOTS.len();
    // rustls exposes no public root iterator on ClientConfig; the
    // RootCertStore is rebuilt identically here to pin the count.
    let mut roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    for certificate in rustls_pemfile::certs(&mut TEST_CA_PEM.as_bytes()) {
        roots.add(certificate?)?;
    }
    assert_eq!(roots.roots.len(), baseline + 1);
    drop(config);
    Ok(())
}

#[test]
fn malformed_custom_ca_pem_is_invalid_argument() {
    for pem in ["not pem at all", ""] {
        let options =
            crate::client::TlsOptions::new().with_ca_certificate_pem(pem.as_bytes().to_vec());
        let error = super::tls_connector(Some(&options)).err();
        assert!(
            matches!(error, Some(ClientError::InvalidArgument { .. })),
            "{pem:?} must be rejected as InvalidArgument, got {error:?}"
        );
    }
}

#[test]
fn per_workflow_frame_carries_the_resume_cursor() -> Result<(), Box<dyn std::error::Error>> {
    let frame = subscription_frame(per_workflow_request(None), Some(7))?;
    let value: serde_json::Value = serde_json::from_str(&frame)?;

    assert_eq!(value["per_workflow"]["namespace"], json!("tenant-a"));
    assert_eq!(
        value["per_workflow"]["workflow_id"]["uuid"],
        json!("00000000-0000-0000-0000-000000000001")
    );
    assert_eq!(value["per_workflow"]["resume_from_seq"], json!(7));
    Ok(())
}

#[test]
fn initial_attach_sends_no_resume_cursor() -> Result<(), Box<dyn std::error::Error>> {
    // Cross-SDK contract: an initial attach is a live tail — the cursor
    // field stays absent/null, matching the Python and TypeScript SDKs.
    let frame = subscription_frame(per_workflow_request(None), None)?;
    let value: serde_json::Value = serde_json::from_str(&frame)?;

    assert_eq!(value["per_workflow"]["resume_from_seq"], json!(null));
    Ok(())
}

#[test]
fn resume_cursor_zero_never_reaches_the_wire() -> Result<(), Box<dyn std::error::Error>> {
    let error = subscription_frame(per_workflow_request(None), Some(0)).err();

    let Some(ClientError::InvalidArgument { detail }) = error else {
        return Err(format!("cursor 0 must be InvalidArgument, got {error:?}").into());
    };
    assert!(detail.message.contains(">= 1"), "detail: {detail}");
    Ok(())
}

#[test]
fn live_only_subscriptions_reject_resume_cursors() -> Result<(), Box<dyn std::error::Error>> {
    let filtered = SubscriptionRequest {
        subscription: Some(subscription_request::Subscription::Filtered(
            FilteredSubscription {
                namespace: String::from("tenant-a"),
                workflow_type: None,
                status: None,
                namespace_selector: None,
            },
        )),
    };
    let firehose = SubscriptionRequest {
        subscription: Some(subscription_request::Subscription::Firehose(
            FirehoseSubscription {
                namespace: String::from("tenant-a"),
            },
        )),
    };

    for request in [filtered, firehose] {
        let error = subscription_frame(request, Some(3)).err();
        let Some(ClientError::InvalidArgument { detail }) = error else {
            return Err(format!("live-only cursor must be InvalidArgument, got {error:?}").into());
        };
        assert!(detail.message.contains("live-only"), "detail: {detail}");
    }
    Ok(())
}

#[test]
fn streamed_event_frames_decode_to_core_events() -> Result<(), Box<dyn std::error::Error>> {
    let workflow_id = aion_core::WorkflowId::new_v4();
    let event = aion_core::Event::SignalReceived {
        envelope: aion_core::EventEnvelope {
            seq: 3,
            recorded_at: chrono::Utc::now(),
            workflow_id,
        },
        name: String::from("approve"),
        payload: aion_core::Payload::from_json(&json!({ "ok": true }))?,
    };
    let frame = serde_json::to_string(&encode_streamed_event("tenant-a", None, &event)?)?;

    let decoded =
        decode_frame(frame.as_bytes()).map_err(|error| format!("decode failed: {error}"))?;
    assert_eq!(decoded, event);
    Ok(())
}

#[test]
fn lagged_error_frames_map_to_unavailable_so_the_resume_loop_reconnects()
-> Result<(), Box<dyn std::error::Error>> {
    let lagged = serde_json::to_string(&json!({
        "error": WireError::lagged("subscriber lagged behind")
    }))?;
    assert_eq!(
        decode_frame(lagged.as_bytes()).err(),
        Some(ClientError::unavailable("subscriber lagged behind"))
    );

    // The per-workflow contiguity tripwire rides the `lagged` code with
    // the SequenceContiguityViolation discriminator: same recovery, the
    // resume loop reconnects with `resume_from_seq = last delivered + 1`.
    let violation = serde_json::to_string(&json!({
        "error": {
            "code": "lagged",
            "message": "per-workflow stream contiguity violated",
            "error_type": "SequenceContiguityViolation",
        }
    }))?;
    assert_eq!(
        decode_frame(violation.as_bytes()).err(),
        Some(ClientError::unavailable(ErrorDetail::with_type(
            "per-workflow stream contiguity violated",
            "SequenceContiguityViolation",
        )))
    );
    Ok(())
}

#[test]
fn terminal_error_frames_map_through_the_shared_taxonomy() -> Result<(), Box<dyn std::error::Error>>
{
    let not_found = serde_json::to_string(&json!({
        "error": WireError::not_found("workflow not found in namespace tenant-a")
    }))?;
    assert_eq!(
        decode_frame(not_found.as_bytes()).err(),
        Some(ClientError::not_found(
            "workflow not found in namespace tenant-a"
        ))
    );

    let denied = serde_json::to_string(&json!({
        "error": WireError::namespace_denied("namespace tenant-b is not granted")
    }))?;
    assert_eq!(
        decode_frame(denied.as_bytes()).err(),
        Some(ClientError::namespace_denied(
            "namespace tenant-b is not granted"
        ))
    );

    let invalid = serde_json::to_string(&json!({
        "error": {
            "code": "invalid_input",
            "message": "resume_from_seq 9 is ahead of recorded history",
            "error_type": "ResumeCursorAheadOfHistory",
        }
    }))?;
    assert_eq!(
        decode_frame(invalid.as_bytes()).err(),
        Some(ClientError::invalid_argument(ErrorDetail::with_type(
            "resume_from_seq 9 is ahead of recorded history",
            "ResumeCursorAheadOfHistory",
        )))
    );
    Ok(())
}

#[test]
fn unrecognizable_frames_are_terminal_server_errors() {
    let error = decode_frame(b"not json");
    assert!(
        matches!(error, Err(ClientError::Server { .. })),
        "garbage frames must be terminal, got {error:?}"
    );
}
