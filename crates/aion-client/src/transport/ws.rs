//! WebSocket event-stream transport.
//!
//! Speaks the cross-SDK subscription protocol against the server's
//! `/events/stream` endpoint:
//!
//! - caller identity (`authorization`, `x-aion-subject`, `x-aion-namespaces`)
//!   travels as headers on the upgrade request;
//! - the first client frame is the JSON `SubscriptionRequest` in the
//!   documented hand-written shape (`{"per_workflow": ...}`, `{"filtered":
//!   ...}`, `{"firehose": ...}`), with `resume_from_seq` (the FIRST sequence
//!   number wanted, `last delivered + 1`) riding inside the per-workflow
//!   variant on resume;
//! - server frames are `StreamedEvent` JSON, except terminal
//!   `{"error": <WireError>}` frames, which are mapped through the shared
//!   taxonomy (`lagged` — including the `SequenceContiguityViolation`
//!   discriminator — becomes [`ClientError::Unavailable`] so the resume loop
//!   reconnects with its cursor; `namespace_denied` / `not_found` /
//!   `invalid_input` are terminal);
//! - a normal close (code 1000) ends the stream; any abnormal close or socket
//!   failure surfaces one `Err(`[`ClientError::Unavailable`]`)` item so the
//!   resume loop reconnects.

use std::sync::Arc;

use aion_core::Event;
use aion_proto::{StreamedEvent, SubscriptionRequest, WireError, subscription_request};
use futures::stream::BoxStream;
use futures::{SinkExt, StreamExt, stream};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::{self, Message};
use tokio_tungstenite::{Connector, MaybeTlsStream, WebSocketStream};

use crate::client::{ClientConfig, TlsOptions};
use crate::error::ClientError;
use crate::transport::contract::SubscriptionAttempt;

/// Path of the server's WebSocket event stream.
pub const EVENT_STREAM_PATH: &str = "/events/stream";

pub(crate) type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// Opens one WebSocket subscription attempt for `request`.
///
/// `resume_from_sequence`, when supplied by the resume loop, is written into
/// the per-workflow subscription's `resume_from_seq` field; filtered and
/// firehose subscriptions are live-only and reject a cursor.
///
/// # Errors
///
/// Returns [`ClientError::InvalidArgument`] when no stream endpoint is
/// configured, the endpoint URL is unusable, or a cursor is supplied for a
/// live-only subscription; [`ClientError::Unauthenticated`] when the upgrade
/// is rejected with HTTP 401; and [`ClientError::Unavailable`] when the
/// connection cannot be established.
pub async fn open_subscription(
    config: &ClientConfig,
    request: SubscriptionRequest,
    resume_from_sequence: Option<u64>,
) -> Result<SubscriptionAttempt, ClientError> {
    // Validate the endpoint and build the first frame before opening a
    // socket, so invalid input never costs a connection.
    let url = stream_url(config)?;
    let frame = subscription_frame(request, resume_from_sequence)?;

    let mut upgrade = url.as_str().into_client_request().map_err(|source| {
        ClientError::invalid_argument(format!(
            "stream endpoint {url} is not a valid websocket URL: {source}"
        ))
    })?;
    apply_headers(&mut upgrade, config)?;
    let connector = tls_connector(config.tls.as_ref())?;

    let (mut socket, _response) =
        tokio_tungstenite::connect_async_tls_with_config(upgrade, None, false, connector)
            .await
            .map_err(map_connect_error)?;
    socket
        .send(Message::Text(frame.into()))
        .await
        .map_err(|source| {
            ClientError::unavailable(format!(
                "websocket subscription frame send failed: {source}"
            ))
        })?;

    Ok(SubscriptionAttempt::new(socket_events(socket)))
}

/// Resolves the configured stream endpoint into a `ws://`/`wss://` URL.
///
/// `ws`/`wss` endpoints pass through unchanged; `http`/`https` endpoints are
/// protocol-mapped (the same listener serves `/events/stream`, so this is
/// scheme mapping, never an invented address). There is NO default: the
/// gRPC and HTTP/WebSocket listeners are separate addresses, so deriving one
/// from the other would be an assumed default.
pub(crate) fn stream_url(config: &ClientConfig) -> Result<String, ClientError> {
    let Some(endpoint) = config.stream_endpoint.as_deref() else {
        return Err(ClientError::invalid_argument(format!(
            "no stream endpoint is configured; event subscriptions require \
             ClientBuilder::with_stream_endpoint pointing at the server's \
             {EVENT_STREAM_PATH} WebSocket URL (the HTTP/WebSocket listener \
             is a separate address from the gRPC endpoint)"
        )));
    };
    let Some((scheme, rest)) = endpoint.split_once("://") else {
        return Err(ClientError::invalid_argument(format!(
            "stream endpoint {endpoint} is not an absolute URL; expected a \
             ws://, wss://, http://, or https:// address"
        )));
    };
    match scheme {
        "ws" | "wss" => Ok(endpoint.to_owned()),
        "http" => Ok(format!("ws://{rest}")),
        "https" => Ok(format!("wss://{rest}")),
        other => Err(ClientError::invalid_argument(format!(
            "cannot derive a websocket stream URL from a {other}:// endpoint; \
             expected ws://, wss://, http://, or https://"
        ))),
    }
}

/// Builds the `wss://` TLS connector from the client's [`TlsOptions`]: the
/// webpki trust roots plus every caller-supplied CA certificate from
/// `ca_certificate_pem` — the same custom-CA material the gRPC channel
/// trusts, so a deployment behind a private CA streams events over the same
/// trust configuration it uses for unary calls.
///
/// Returns `None` when the client has no TLS options, so tokio-tungstenite's
/// built-in webpki-roots default applies unchanged. The TLS server name is
/// always the stream URL's host; `TlsOptions::with_domain_name` overrides
/// verification for the gRPC channel only.
///
/// # Errors
///
/// Returns [`ClientError::InvalidArgument`] when `ca_certificate_pem` is not
/// parseable PEM, contains no certificate, or holds a certificate the trust
/// store rejects.
pub(crate) fn tls_connector(tls: Option<&TlsOptions>) -> Result<Option<Connector>, ClientError> {
    let Some(tls) = tls else {
        return Ok(None);
    };
    let mut roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    if let Some(pem) = &tls.ca_certificate_pem {
        let mut added = 0_usize;
        for certificate in rustls_pemfile::certs(&mut pem.as_slice()) {
            let certificate = certificate.map_err(|source| {
                ClientError::invalid_argument(format!(
                    "TLS ca_certificate_pem is not parseable PEM: {source}"
                ))
            })?;
            roots.add(certificate).map_err(|source| {
                ClientError::invalid_argument(format!(
                    "TLS ca_certificate_pem holds a certificate the trust store rejects: {source}"
                ))
            })?;
            added += 1;
        }
        if added == 0 {
            return Err(ClientError::invalid_argument(
                "TLS ca_certificate_pem contains no CA certificate",
            ));
        }
    }
    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(Some(Connector::Rustls(Arc::new(tls_config))))
}

/// Builds the first client frame: the JSON `SubscriptionRequest` in the
/// documented hand-written shape, with the resume cursor written into the
/// per-workflow variant.
pub(crate) fn subscription_frame(
    request: SubscriptionRequest,
    resume_from_sequence: Option<u64>,
) -> Result<String, ClientError> {
    let (key, subscription) = match request.subscription {
        Some(subscription_request::Subscription::PerWorkflow(mut per_workflow)) => {
            if let Some(cursor) = resume_from_sequence {
                if cursor == 0 {
                    return Err(ClientError::invalid_argument(
                        "resume_from_seq must be >= 1 (the first sequence number wanted)",
                    ));
                }
                per_workflow.resume_from_seq = Some(cursor);
            }
            ("per_workflow", encode_subscription(&per_workflow)?)
        }
        Some(subscription_request::Subscription::Filtered(filtered)) => {
            reject_live_only_cursor("filtered", resume_from_sequence)?;
            ("filtered", encode_subscription(&filtered)?)
        }
        Some(subscription_request::Subscription::Firehose(firehose)) => {
            reject_live_only_cursor("firehose", resume_from_sequence)?;
            ("firehose", encode_subscription(&firehose)?)
        }
        Some(subscription_request::Subscription::Cluster(cluster)) => {
            // The WS3 cluster channel carries its own `after_seq` cursor in the
            // subscription body; it does not use the per-workflow resume-cursor
            // argument, which is per-workflow-seq-only.
            reject_live_only_cursor("cluster", resume_from_sequence)?;
            ("cluster", encode_subscription(&cluster)?)
        }
        Some(subscription_request::Subscription::Transcript(transcript)) => {
            // The NOI-5b transcript channel likewise carries its own `after_seq`
            // resume cursor (keyed on the commit-allocated `store_seq`) in the
            // subscription body, so it does not use the per-workflow-seq resume
            // argument.
            reject_live_only_cursor("transcript", resume_from_sequence)?;
            ("transcript", encode_subscription(&transcript)?)
        }
        None => {
            return Err(ClientError::invalid_argument(
                "subscription request is missing its subscription variant",
            ));
        }
    };
    serde_json::to_string(&serde_json::json!({ key: subscription })).map_err(|source| {
        ClientError::invalid_argument(format!("failed to encode subscription request: {source}"))
    })
}

fn encode_subscription<T: serde::Serialize>(value: &T) -> Result<serde_json::Value, ClientError> {
    serde_json::to_value(value).map_err(|source| {
        ClientError::invalid_argument(format!("failed to encode subscription request: {source}"))
    })
}

fn reject_live_only_cursor(kind: &str, cursor: Option<u64>) -> Result<(), ClientError> {
    if cursor.is_some() {
        return Err(ClientError::invalid_argument(format!(
            "{kind} event streams are live-only by design; resume cursors are \
             valid for per-workflow subscriptions only"
        )));
    }
    Ok(())
}

/// Forwards the caller identity headers the server's caller extraction reads.
pub(crate) fn apply_headers(
    upgrade: &mut tungstenite::handshake::client::Request,
    config: &ClientConfig,
) -> Result<(), ClientError> {
    let headers = upgrade.headers_mut();
    if let Some(auth) = &config.auth {
        let value = HeaderValue::from_str(&format!("Bearer {}", auth.token()))
            .map_err(|_| ClientError::invalid_argument("auth token is not a valid header value"))?;
        headers.insert("authorization", value);
    }
    if let Some(subject) = &config.subject {
        let value = HeaderValue::from_str(subject).map_err(|_| {
            ClientError::invalid_argument("subject is not a valid x-aion-subject header value")
        })?;
        headers.insert("x-aion-subject", value);
    }
    if !config.authorized_namespaces.is_empty() {
        let value =
            HeaderValue::from_str(&config.authorized_namespaces.join(",")).map_err(|_| {
                ClientError::invalid_argument(
                    "authorized namespaces are not a valid x-aion-namespaces header value",
                )
            })?;
        headers.insert("x-aion-namespaces", value);
    }
    Ok(())
}

pub(crate) fn map_connect_error(error: tungstenite::Error) -> ClientError {
    match error {
        // The server rejects bad credentials on the upgrade with HTTP 401.
        tungstenite::Error::Http(response)
            if response.status() == tungstenite::http::StatusCode::UNAUTHORIZED =>
        {
            ClientError::unauthenticated("websocket upgrade was rejected with HTTP 401")
        }
        other => ClientError::unavailable(format!("websocket connect failed: {other}")),
    }
}

/// Adapts one connected socket into the per-attempt event stream.
///
/// The stream is fused after any error item: a terminal `{"error": ...}`
/// frame or a transport failure ends the attempt, and the surrounding resume
/// loop decides whether to reconnect (`Unavailable`) or surface the error.
fn socket_events(socket: WsStream) -> BoxStream<'static, Result<Event, ClientError>> {
    stream::unfold(Some(socket), |state| async move {
        let mut socket = state?;
        loop {
            return match socket.next().await {
                None | Some(Err(tungstenite::Error::ConnectionClosed)) => None,
                Some(Ok(Message::Text(text))) => match decode_frame(text.as_bytes()) {
                    Ok(event) => Some((Ok(event), Some(socket))),
                    Err(error) => Some((Err(error), None)),
                },
                Some(Ok(Message::Binary(bytes))) => match decode_frame(&bytes) {
                    Ok(event) => Some((Ok(event), Some(socket))),
                    Err(error) => Some((Err(error), None)),
                },
                Some(Ok(Message::Close(frame))) => match frame {
                    // A normal closure ends the stream; anything else is a
                    // transient drop the resume loop recovers from.
                    Some(frame) if frame.code == CloseCode::Normal => None,
                    Some(frame) => Some((
                        Err(ClientError::unavailable(format!(
                            "websocket closed abnormally ({} {})",
                            frame.code, frame.reason
                        ))),
                        None,
                    )),
                    None => Some((
                        Err(ClientError::unavailable(
                            "websocket closed without a close frame",
                        )),
                        None,
                    )),
                },
                Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => continue,
                Some(Err(source)) => Some((
                    Err(ClientError::unavailable(format!(
                        "websocket transport failed: {source}"
                    ))),
                    None,
                )),
            };
        }
    })
    .boxed()
}

/// Decodes one server frame: a `StreamedEvent` yields its core event, a
/// `{"error": <WireError>}` frame yields the mapped taxonomy error.
fn decode_frame(bytes: &[u8]) -> Result<Event, ClientError> {
    #[derive(serde::Deserialize)]
    struct ErrorFrame {
        error: WireError,
    }
    if let Ok(frame) = serde_json::from_slice::<ErrorFrame>(bytes) {
        return Err(ClientError::from_wire_error(frame.error));
    }
    let streamed = serde_json::from_slice::<StreamedEvent>(bytes).map_err(|source| {
        ClientError::server(format!(
            "event stream frame is neither a StreamedEvent nor an error frame: {source}"
        ))
    })?;
    streamed
        .decode_event()
        .map_err(ClientError::from_wire_error)
}

#[cfg(test)]
#[path = "ws_socket_tests.rs"]
mod socket_tests;
#[cfg(test)]
#[path = "ws_unit_tests.rs"]
mod unit_tests;
