//! JSON-RPC 2.0 framing for the stateless MCP revision.
//!
//! Only two inbound shapes exist here: a **request** (carries an `id`) and a
//! **notification** (carries none). A client MUST NOT send JSON-RPC responses,
//! so an inbound object carrying `result` or `error` is rejected as an invalid
//! request rather than silently ignored.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::error::ProtocolError;

/// A JSON-RPC request identifier: a string or a number, never null.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    /// String identifier.
    Text(String),
    /// Integer identifier.
    Number(i64),
}

/// A parsed inbound JSON-RPC message.
#[derive(Clone, Debug)]
pub struct IncomingMessage {
    /// `None` for a notification.
    pub id: Option<RequestId>,
    /// The method name. Case-sensitive.
    pub method: String,
    /// The `params` object, or an empty object when absent.
    pub params: Map<String, Value>,
}

impl IncomingMessage {
    /// Whether this message is a notification (no `id`), which MUST NOT be
    /// answered with a JSON-RPC response.
    #[must_use]
    pub const fn is_notification(&self) -> bool {
        self.id.is_none()
    }

    /// Parse a request body into a message.
    ///
    /// # Errors
    ///
    /// [`ProtocolError::Parse`] when the body is not JSON, and
    /// [`ProtocolError::InvalidRequest`] when it is JSON but not a JSON-RPC
    /// request or notification — including the explicitly-forbidden case of a
    /// client-sent JSON-RPC *response*, and the case of a null or
    /// wrongly-typed `id`.
    pub fn parse(body: &[u8]) -> Result<Self, ProtocolError> {
        let value: Value = serde_json::from_slice(body).map_err(|error| ProtocolError::Parse {
            detail: error.to_string(),
        })?;
        let Value::Object(object) = value else {
            return Err(ProtocolError::InvalidRequest {
                detail: "the request body must be a single JSON-RPC object".to_owned(),
            });
        };
        match object.get("jsonrpc").and_then(Value::as_str) {
            Some("2.0") => {}
            Some(other) => {
                return Err(ProtocolError::InvalidRequest {
                    detail: format!("jsonrpc must be \"2.0\", not \"{other}\""),
                });
            }
            None => {
                return Err(ProtocolError::InvalidRequest {
                    detail: "jsonrpc version field is missing".to_owned(),
                });
            }
        }
        if object.contains_key("result") || object.contains_key("error") {
            return Err(ProtocolError::InvalidRequest {
                detail: "a client must not send JSON-RPC responses".to_owned(),
            });
        }
        let method = object
            .get("method")
            .and_then(Value::as_str)
            .ok_or_else(|| ProtocolError::InvalidRequest {
                detail: "method is missing or is not a string".to_owned(),
            })?
            .to_owned();
        let id = match object.get("id") {
            None => None,
            Some(Value::String(text)) => Some(RequestId::Text(text.clone())),
            Some(Value::Number(number)) => {
                Some(RequestId::Number(number.as_i64().ok_or_else(|| {
                    ProtocolError::InvalidRequest {
                        detail: "id must be a string or an integer".to_owned(),
                    }
                })?))
            }
            Some(Value::Null) => {
                return Err(ProtocolError::InvalidRequest {
                    detail: "id must not be null".to_owned(),
                });
            }
            Some(_) => {
                return Err(ProtocolError::InvalidRequest {
                    detail: "id must be a string or an integer".to_owned(),
                });
            }
        };
        let params = match object.get("params") {
            None => Map::new(),
            Some(Value::Object(params)) => params.clone(),
            Some(_) => {
                return Err(ProtocolError::InvalidRequest {
                    detail: "params must be an object".to_owned(),
                });
            }
        };
        Ok(Self { id, method, params })
    }
}

/// A JSON-RPC error object.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// The error code.
    pub code: i32,
    /// A human-readable description.
    pub message: String,
    /// Structured detail, when the code defines one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// A JSON-RPC response: exactly one of `result` or `error`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// Always `"2.0"`.
    pub jsonrpc: String,
    /// The originating request's id. `None` only on an error produced before an
    /// id could be read, which the spec permits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<RequestId>,
    /// The successful result object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// The failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    /// A successful response carrying `result`.
    #[must_use]
    pub fn success(id: Option<RequestId>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// A failure response built from a typed protocol error.
    #[must_use]
    pub fn failure(id: Option<RequestId>, error: &ProtocolError) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code: error.code(),
                message: error.message(),
                data: error.data(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{IncomingMessage, ProtocolError, RequestId};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn a_request_and_a_notification_are_distinguished_by_the_id() -> TestResult {
        let request = IncomingMessage::parse(
            br#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#,
        )?;
        assert_eq!(request.id, Some(RequestId::Number(1)));
        assert!(!request.is_notification());

        let notification =
            IncomingMessage::parse(br#"{"jsonrpc":"2.0","method":"notifications/ping"}"#)?;
        assert!(notification.is_notification());
        Ok(())
    }

    #[test]
    fn a_client_sent_response_is_refused() {
        let outcome = IncomingMessage::parse(br#"{"jsonrpc":"2.0","id":1,"result":{}}"#);
        assert!(matches!(outcome, Err(ProtocolError::InvalidRequest { .. })));
    }

    #[test]
    fn a_null_id_is_refused() {
        let outcome =
            IncomingMessage::parse(br#"{"jsonrpc":"2.0","id":null,"method":"tools/list"}"#);
        assert!(matches!(outcome, Err(ProtocolError::InvalidRequest { .. })));
    }

    #[test]
    fn a_non_json_body_is_a_parse_error() {
        assert!(matches!(
            IncomingMessage::parse(b"{not json"),
            Err(ProtocolError::Parse { .. })
        ));
    }
}
