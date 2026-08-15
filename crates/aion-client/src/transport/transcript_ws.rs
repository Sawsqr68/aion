//! Strict WebSocket transport for activity-attempt transcript subscriptions.

use aion_core::ActivityEvent;
use aion_proto::{
    StreamedActivityEvent, SubscriptionRequest, TranscriptSubscription, WireError, WireErrorCode,
    subscription_request,
};
use futures::{SinkExt, StreamExt, stream};
use serde::Deserialize;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::{self, Message};

use super::ws::{
    WsStream, apply_headers, map_connect_error, stream_url, subscription_frame, tls_connector,
};
use crate::client::ClientConfig;
use crate::error::ClientError;
use crate::transcript::{TranscriptStream, TranscriptStreamItem};

/// Open one transcript subscription against the configured stream endpoint.
pub(crate) async fn open(
    config: &ClientConfig,
    transcript: TranscriptSubscription,
) -> Result<TranscriptStream, ClientError> {
    let url = stream_url(config)?;
    let request = SubscriptionRequest {
        subscription: Some(subscription_request::Subscription::Transcript(transcript)),
    };
    let frame = subscription_frame(request, None)?;
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
                "websocket transcript subscription frame send failed: {source}"
            ))
        })?;
    Ok(transcript_events(socket))
}

fn transcript_events(socket: WsStream) -> TranscriptStream {
    stream::unfold(Some(socket), |state| async move {
        let mut socket = state?;
        loop {
            return match socket.next().await {
                None | Some(Err(tungstenite::Error::ConnectionClosed)) => None,
                Some(Ok(Message::Text(text))) => Some(decoded_item(text.as_bytes(), socket)),
                Some(Ok(Message::Binary(bytes))) => Some(decoded_item(&bytes, socket)),
                Some(Ok(Message::Close(frame))) => match frame {
                    Some(frame) if frame.code == CloseCode::Normal => None,
                    Some(frame) => Some((
                        Err(ClientError::unavailable(format!(
                            "transcript websocket closed abnormally ({} {})",
                            frame.code, frame.reason
                        ))),
                        None,
                    )),
                    None => Some((
                        Err(ClientError::unavailable(
                            "transcript websocket closed without a close frame",
                        )),
                        None,
                    )),
                },
                Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => continue,
                Some(Err(source)) => Some((
                    Err(ClientError::unavailable(format!(
                        "transcript websocket transport failed: {source}"
                    ))),
                    None,
                )),
            };
        }
    })
    .boxed()
}

fn decoded_item(
    bytes: &[u8],
    socket: WsStream,
) -> (Result<TranscriptStreamItem, ClientError>, Option<WsStream>) {
    match decode_frame(bytes) {
        Ok(item @ TranscriptStreamItem::Event(_)) => (Ok(item), Some(socket)),
        Ok(item @ TranscriptStreamItem::Lagged { .. }) => (Ok(item), None),
        Err(error) => (Err(error), None),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StrictActivityFrame {
    kind: String,
    event: ActivityEvent,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LagFrame {
    error: StrictLag,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StrictLag {
    code: String,
    skipped: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ErrorFrame {
    error: StrictWireError,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StrictWireError {
    code: WireErrorCode,
    message: String,
    error_type: Option<String>,
}

fn decode_frame(bytes: &[u8]) -> Result<TranscriptStreamItem, ClientError> {
    if let Ok(frame) = serde_json::from_slice::<LagFrame>(bytes) {
        if frame.error.code != "transcript_lagged" {
            return Err(ClientError::server(format!(
                "unknown transcript error frame code `{}`",
                frame.error.code
            )));
        }
        return Ok(TranscriptStreamItem::Lagged {
            skipped: frame.error.skipped,
        });
    }
    if let Ok(frame) = serde_json::from_slice::<ErrorFrame>(bytes) {
        return Err(ClientError::from_wire_error(WireError {
            code: frame.error.code,
            message: frame.error.message,
            error_type: frame.error.error_type,
        }));
    }
    let frame = serde_json::from_slice::<StrictActivityFrame>(bytes).map_err(|source| {
        ClientError::server(format!(
            "transcript frame is neither a strict activity_event, transcript_lagged, nor wire-error frame: {source}"
        ))
    })?;
    if frame.kind != StreamedActivityEvent::KIND {
        return Err(ClientError::server(format!(
            "unknown transcript frame kind `{}`",
            frame.kind
        )));
    }
    Ok(TranscriptStreamItem::Event(Box::new(frame.event)))
}

#[cfg(test)]
mod tests {
    use aion_core::{ActivityEventKind, ActivityId, RunId, WorkflowId};
    use aion_proto::StreamedActivityEvent;
    use chrono::{DateTime, Utc};
    use serde_json::json;

    use super::*;

    fn event() -> Result<ActivityEvent, Box<dyn std::error::Error>> {
        Ok(ActivityEvent {
            workflow_id: WorkflowId::new(uuid::Uuid::from_u128(1)),
            run_id: RunId::new(uuid::Uuid::from_u128(2)),
            activity_id: ActivityId::from_sequence_position(3),
            attempt: 1,
            agent_id: uuid::Uuid::from_u128(4),
            agent_role: "builder".to_owned(),
            emitted_at: DateTime::<Utc>::from_timestamp(1_700_000_000, 0)
                .ok_or("test timestamp must be representable")?,
            worker_seq: 5,
            store_seq: Some(6),
            ephemeral: false,
            kind: ActivityEventKind::Raw {
                source: "decoder-red".to_owned(),
                value: json!({"line": "live leg output"}),
            },
        })
    }

    #[test]
    fn decodes_frame_from_canonical_server_serializer() -> Result<(), Box<dyn std::error::Error>> {
        let event = event()?;
        let bytes = serde_json::to_vec(&StreamedActivityEvent::new(event.clone()))?;
        assert_eq!(
            decode_frame(&bytes)?,
            TranscriptStreamItem::Event(Box::new(event))
        );
        Ok(())
    }

    #[test]
    fn rejects_unknown_kind_and_fields_loudly() -> Result<(), Box<dyn std::error::Error>> {
        let event = event()?;
        for frame in [
            json!({"kind": "future_event", "event": event}),
            json!({"kind": "activity_event", "event": event, "extra": true}),
        ] {
            let error = decode_frame(&serde_json::to_vec(&frame)?).err();
            assert!(matches!(error, Some(ClientError::Server { .. })));
        }
        Ok(())
    }

    #[test]
    fn lag_is_a_recoverable_typed_item() -> Result<(), Box<dyn std::error::Error>> {
        let item = decode_frame(br#"{"error":{"code":"transcript_lagged","skipped":12}}"#)?;
        assert_eq!(item, TranscriptStreamItem::Lagged { skipped: 12 });
        Ok(())
    }
}
