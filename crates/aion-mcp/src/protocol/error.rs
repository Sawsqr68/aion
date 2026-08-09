//! Error codes and the typed protocol failure.
//!
//! Every variant carries the HTTP status the transport must answer with, so the
//! status and the JSON-RPC code can never drift apart: they are produced from
//! one value.

use serde_json::{Value, json};

use super::version::SUPPORTED_PROTOCOL_VERSIONS;

/// The JSON-RPC and MCP error codes this server emits or accepts.
pub mod error_codes {
    /// JSON-RPC: invalid JSON was received.
    pub const PARSE_ERROR: i32 = -32700;
    /// JSON-RPC: the payload is not a valid Request object.
    pub const INVALID_REQUEST: i32 = -32600;
    /// JSON-RPC: the method does not exist.
    pub const METHOD_NOT_FOUND: i32 = -32601;
    /// JSON-RPC: invalid method parameters.
    pub const INVALID_PARAMS: i32 = -32602;
    /// JSON-RPC: internal server error.
    pub const INTERNAL_ERROR: i32 = -32603;
    /// MCP: a required standard header is missing, or disagrees with the body.
    pub const HEADER_MISMATCH: i32 = -32020;
    /// MCP: the server needs a client capability the request did not declare.
    pub const MISSING_REQUIRED_CLIENT_CAPABILITY: i32 = -32021;
    /// MCP: the requested protocol version is not implemented.
    pub const UNSUPPORTED_PROTOCOL_VERSION: i32 = -32022;
    /// The tasks draft's stale allocation for a missing client capability.
    ///
    /// Accept-only. The core revision reserves `-32000`…`-32019` as the legacy
    /// sub-range in which new codes MUST NOT be allocated and receivers MUST NOT
    /// assume a meaning, so this server never emits it; it exists as a named
    /// constant purely so the drift is recorded in code rather than in a
    /// comment. `-32021` is emitted instead.
    pub const TASKS_DRAFT_MISSING_CAPABILITY: i32 = -32003;
}

/// A protocol-level failure: a JSON-RPC error plus the HTTP status that must
/// carry it.
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    /// The request body was not valid JSON.
    #[error("parse error: {detail}")]
    Parse {
        /// What failed to parse.
        detail: String,
    },
    /// The request body was valid JSON but not a valid JSON-RPC request.
    #[error("invalid request: {detail}")]
    InvalidRequest {
        /// What was wrong with the envelope.
        detail: String,
    },
    /// The method is not implemented by this server.
    #[error("method not found: {method}")]
    MethodNotFound {
        /// The method that was asked for.
        method: String,
    },
    /// Parameters were missing, malformed, or named something that does not
    /// exist (an unknown tool or an unknown task).
    #[error("invalid params: {detail}")]
    InvalidParams {
        /// What was wrong with the parameters.
        detail: String,
    },
    /// A required standard header is absent, disagrees with the body value, or
    /// contains characters a header value may not carry.
    #[error("header mismatch: {detail}")]
    HeaderMismatch {
        /// Which header, and how it disagreed.
        detail: String,
    },
    /// The server cannot service this request without a client capability the
    /// request did not declare.
    #[error("missing required client capability: {}", required.join(", "))]
    MissingClientCapability {
        /// The capabilities the request must declare, in the dotted form the
        /// spec's `data.requiredCapabilities` uses.
        required: Vec<String>,
    },
    /// The requested protocol version is not implemented.
    #[error("unsupported protocol version: {requested}")]
    UnsupportedProtocolVersion {
        /// The version the request asked for.
        requested: String,
    },
    /// The server failed while executing an otherwise well-formed request.
    #[error("internal error: {detail}")]
    Internal {
        /// What failed.
        detail: String,
    },
}

impl ProtocolError {
    /// The JSON-RPC error code for this failure.
    #[must_use]
    pub const fn code(&self) -> i32 {
        match self {
            Self::Parse { .. } => error_codes::PARSE_ERROR,
            Self::InvalidRequest { .. } => error_codes::INVALID_REQUEST,
            Self::MethodNotFound { .. } => error_codes::METHOD_NOT_FOUND,
            Self::InvalidParams { .. } => error_codes::INVALID_PARAMS,
            Self::HeaderMismatch { .. } => error_codes::HEADER_MISMATCH,
            Self::MissingClientCapability { .. } => error_codes::MISSING_REQUIRED_CLIENT_CAPABILITY,
            Self::UnsupportedProtocolVersion { .. } => error_codes::UNSUPPORTED_PROTOCOL_VERSION,
            Self::Internal { .. } => error_codes::INTERNAL_ERROR,
        }
    }

    /// The HTTP status the transport must answer this failure with.
    ///
    /// `404` for an unimplemented method is what distinguishes it from a legacy
    /// HTTP+SSE 404: the JSON-RPC `-32601` body rides inside it.
    #[must_use]
    pub const fn http_status(&self) -> u16 {
        match self {
            Self::MethodNotFound { .. } => 404,
            Self::Internal { .. } => 500,
            Self::Parse { .. }
            | Self::InvalidRequest { .. }
            | Self::InvalidParams { .. }
            | Self::HeaderMismatch { .. }
            | Self::MissingClientCapability { .. }
            | Self::UnsupportedProtocolVersion { .. } => 400,
        }
    }

    /// The JSON-RPC `error.data` object, when this failure carries one.
    #[must_use]
    pub fn data(&self) -> Option<Value> {
        match self {
            Self::MissingClientCapability { required } => {
                Some(json!({ "requiredCapabilities": required }))
            }
            Self::UnsupportedProtocolVersion { requested } => Some(json!({
                "supported": SUPPORTED_PROTOCOL_VERSIONS,
                "requested": requested,
            })),
            _ => None,
        }
    }

    /// The `message` string carried in the JSON-RPC error object.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::MissingClientCapability { .. } => "Missing required client capability".to_owned(),
            Self::UnsupportedProtocolVersion { .. } => "Unsupported protocol version".to_owned(),
            other => other.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ProtocolError, error_codes};

    #[test]
    fn unsupported_version_carries_supported_and_requested() {
        let error = ProtocolError::UnsupportedProtocolVersion {
            requested: "1900-01-01".to_owned(),
        };
        assert_eq!(error.code(), error_codes::UNSUPPORTED_PROTOCOL_VERSION);
        assert_eq!(error.http_status(), 400);
        let data = error.data().unwrap_or_default();
        assert_eq!(data["requested"], "1900-01-01");
        assert_eq!(
            data["supported"][0],
            super::super::version::PROTOCOL_VERSION
        );
    }

    #[test]
    fn missing_capability_emits_the_core_code_not_the_tasks_draft_code() {
        let error = ProtocolError::MissingClientCapability {
            required: vec!["extensions.io.modelcontextprotocol/tasks".to_owned()],
        };
        assert_eq!(
            error.code(),
            error_codes::MISSING_REQUIRED_CLIENT_CAPABILITY
        );
        assert_ne!(error.code(), error_codes::TASKS_DRAFT_MISSING_CAPABILITY);
        let data = error.data().unwrap_or_default();
        assert_eq!(
            data["requiredCapabilities"][0],
            "extensions.io.modelcontextprotocol/tasks"
        );
    }

    #[test]
    fn an_unimplemented_method_is_a_404_carrying_the_json_rpc_code() {
        let error = ProtocolError::MethodNotFound {
            method: "subscriptions/listen".to_owned(),
        };
        assert_eq!(error.http_status(), 404);
        assert_eq!(error.code(), error_codes::METHOD_NOT_FOUND);
    }
}
