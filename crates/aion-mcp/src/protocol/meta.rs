//! The per-request `_meta` contract.
//!
//! Statelessness is what makes this load-bearing: there is no handshake and no
//! session, so every single request re-supplies the protocol version and the
//! client's capabilities. A request missing either is malformed and is refused
//! with `-32602` / HTTP 400 — never defaulted, because defaulting would invent
//! a capability the client never claimed.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::error::ProtocolError;
use super::version::{
    META_CLIENT_CAPABILITIES, META_CLIENT_INFO, META_PROTOCOL_VERSION, TASKS_EXTENSION,
    is_supported,
};

/// A self-reported implementation identity (`name` + `version`).
///
/// Self-reported and unverified: this server never reads it to change behaviour
/// and never uses it for a security decision.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Implementation {
    /// Programmatic name.
    pub name: String,
    /// Implementation version.
    pub version: String,
    /// Optional human display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

impl Implementation {
    /// Build an implementation identity.
    #[must_use]
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            title: None,
        }
    }
}

/// The capabilities a client declared on **this** request.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClientCapabilities {
    /// The raw declared object, preserved verbatim so an unrecognised
    /// capability is never silently dropped from a diagnostic.
    raw: Map<String, Value>,
}

impl ClientCapabilities {
    /// Wrap a declared capabilities object.
    #[must_use]
    pub const fn from_object(raw: Map<String, Value>) -> Self {
        Self { raw }
    }

    /// Whether the request declared support for `extension`.
    ///
    /// A declaration is the presence of the extension identifier as a key under
    /// `extensions` whose value is an object. A key mapped to a non-object (or
    /// to `null`) is not a declaration: the extension's settings type is
    /// `Record<string, never>`, so anything else is a malformed claim and is
    /// treated as absent rather than generously accepted.
    #[must_use]
    pub fn declares_extension(&self, extension: &str) -> bool {
        self.raw
            .get("extensions")
            .and_then(Value::as_object)
            .and_then(|extensions| extensions.get(extension))
            .is_some_and(Value::is_object)
    }

    /// Whether the request declared the tasks extension.
    #[must_use]
    pub fn declares_tasks(&self) -> bool {
        self.declares_extension(TASKS_EXTENSION)
    }

    /// The declared capabilities as the raw object.
    #[must_use]
    pub const fn as_object(&self) -> &Map<String, Value> {
        &self.raw
    }
}

/// The validated `_meta` of one request.
#[derive(Clone, Debug)]
pub struct RequestMeta {
    /// The protocol version this request is speaking. Always supported: an
    /// unsupported value is refused during parsing.
    pub protocol_version: String,
    /// The capabilities declared on this request.
    pub client_capabilities: ClientCapabilities,
    /// The client's self-reported identity, when it sent one.
    pub client_info: Option<Implementation>,
    /// The client's progress token, when it opted in.
    ///
    /// Read so the value is not silently discarded from diagnostics; this
    /// server emits no progress notifications, which the progress pattern
    /// explicitly permits.
    pub progress_token: Option<Value>,
}

impl RequestMeta {
    /// Parse and validate the `_meta` object of a request's `params`.
    ///
    /// # Errors
    ///
    /// * [`ProtocolError::InvalidParams`] when `_meta` is absent or is not an
    ///   object, when `io.modelcontextprotocol/protocolVersion` is absent or is
    ///   not a string, or when `io.modelcontextprotocol/clientCapabilities` is
    ///   absent or is not an object.
    /// * [`ProtocolError::UnsupportedProtocolVersion`] when the declared
    ///   version is not one this server implements.
    pub fn parse(params: &Map<String, Value>) -> Result<Self, ProtocolError> {
        let meta = params
            .get("_meta")
            .ok_or_else(|| ProtocolError::InvalidParams {
                detail: "params._meta is required on every request".to_owned(),
            })?
            .as_object()
            .ok_or_else(|| ProtocolError::InvalidParams {
                detail: "params._meta must be an object".to_owned(),
            })?;
        let protocol_version = meta
            .get(META_PROTOCOL_VERSION)
            .and_then(Value::as_str)
            .ok_or_else(|| ProtocolError::InvalidParams {
                detail: format!("params._meta[\"{META_PROTOCOL_VERSION}\"] is required"),
            })?
            .to_owned();
        // The version check runs BEFORE the capability check so a client on an
        // unimplemented revision learns which revisions exist, rather than
        // being told its (differently-shaped) capabilities object is malformed.
        if !is_supported(&protocol_version) {
            return Err(ProtocolError::UnsupportedProtocolVersion {
                requested: protocol_version,
            });
        }
        let client_capabilities = meta
            .get(META_CLIENT_CAPABILITIES)
            .ok_or_else(|| ProtocolError::InvalidParams {
                detail: format!("params._meta[\"{META_CLIENT_CAPABILITIES}\"] is required"),
            })?
            .as_object()
            .ok_or_else(|| ProtocolError::InvalidParams {
                detail: format!("params._meta[\"{META_CLIENT_CAPABILITIES}\"] must be an object"),
            })?
            .clone();
        let client_info = match meta.get(META_CLIENT_INFO) {
            None | Some(Value::Null) => None,
            Some(value) => Some(serde_json::from_value(value.clone()).map_err(|error| {
                ProtocolError::InvalidParams {
                    detail: format!(
                        "params._meta[\"{META_CLIENT_INFO}\"] is not an Implementation: {error}"
                    ),
                }
            })?),
        };
        let progress_token = match meta.get("progressToken") {
            None | Some(Value::Null) => None,
            Some(token @ (Value::String(_) | Value::Number(_))) => Some(token.clone()),
            Some(_) => {
                return Err(ProtocolError::InvalidParams {
                    detail: "params._meta.progressToken must be a string or a number".to_owned(),
                });
            }
        };
        Ok(Self {
            protocol_version,
            client_capabilities: ClientCapabilities::from_object(client_capabilities),
            client_info,
            progress_token,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value, json};

    use super::{ClientCapabilities, ProtocolError, RequestMeta};
    use crate::protocol::version::{PROTOCOL_VERSION, TASKS_EXTENSION};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn params(meta: &Value) -> Result<Map<String, Value>, serde_json::Error> {
        serde_json::from_value(json!({ "_meta": meta }))
    }

    #[test]
    fn a_complete_meta_parses() -> TestResult {
        let params = params(&json!({
            "io.modelcontextprotocol/protocolVersion": PROTOCOL_VERSION,
            "io.modelcontextprotocol/clientCapabilities": { "extensions": { TASKS_EXTENSION: {} } },
            "io.modelcontextprotocol/clientInfo": { "name": "aion-assistant", "version": "0.12.0" },
        }))?;
        let meta = RequestMeta::parse(&params)?;
        assert_eq!(meta.protocol_version, PROTOCOL_VERSION);
        assert!(meta.client_capabilities.declares_tasks());
        assert_eq!(
            meta.client_info.map(|info| info.name),
            Some("aion-assistant".to_owned())
        );
        Ok(())
    }

    #[test]
    fn missing_capabilities_is_invalid_params_not_an_empty_default() -> TestResult {
        let params = params(&json!({
            "io.modelcontextprotocol/protocolVersion": PROTOCOL_VERSION,
        }))?;
        assert!(matches!(
            RequestMeta::parse(&params),
            Err(ProtocolError::InvalidParams { .. })
        ));
        Ok(())
    }

    #[test]
    fn missing_meta_entirely_is_invalid_params() {
        assert!(matches!(
            RequestMeta::parse(&Map::new()),
            Err(ProtocolError::InvalidParams { .. })
        ));
    }

    #[test]
    fn an_unimplemented_version_is_refused_before_the_capability_check() -> TestResult {
        let params = params(&json!({
            "io.modelcontextprotocol/protocolVersion": "2025-11-25",
        }))?;
        assert!(matches!(
            RequestMeta::parse(&params),
            Err(ProtocolError::UnsupportedProtocolVersion { .. })
        ));
        Ok(())
    }

    #[test]
    fn a_non_object_extension_value_is_not_a_declaration() -> TestResult {
        let declared: Map<String, Value> =
            serde_json::from_value(json!({ "extensions": { TASKS_EXTENSION: true } }))?;
        assert!(!ClientCapabilities::from_object(declared).declares_tasks());
        let null: Map<String, Value> =
            serde_json::from_value(json!({ "extensions": { TASKS_EXTENSION: Value::Null } }))?;
        assert!(!ClientCapabilities::from_object(null).declares_tasks());
        Ok(())
    }
}
