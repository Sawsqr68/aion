//! The server handle: the one object a host constructs and calls into.

use std::sync::Arc;

use serde_json::{Value, json};

use crate::http::{HttpRequest, HttpResponse};
use crate::protocol::error::ProtocolError;
use crate::protocol::jsonrpc::{IncomingMessage, JsonRpcResponse, RequestId};
use crate::protocol::meta::RequestMeta;
use crate::tools::service::ToolService;

use super::config::McpServerConfig;
use super::dispatch::{route, validate_headers};

/// A stateless MCP server over one host tool service.
///
/// "Stateless" is literal, and now without qualification: this object holds a
/// service and a configuration, and nothing else. Nothing is keyed by
/// connection, and nothing spans requests — not even a task, which is a
/// projection of durable work the HOST stores and is re-read on each call
/// through the explicit handle the client was handed.
pub struct McpServer<S: ToolService> {
    service: Arc<S>,
    config: McpServerConfig,
}

impl<S: ToolService> McpServer<S> {
    /// Build a server over a tool service.
    #[must_use]
    pub const fn new(service: Arc<S>, config: McpServerConfig) -> Self {
        Self { service, config }
    }

    /// The configuration this server runs under.
    #[must_use]
    pub const fn config(&self) -> &McpServerConfig {
        &self.config
    }

    /// Handle one HTTP request to the MCP endpoint.
    ///
    /// `caller` is the identity the host already authenticated. The MCP layer
    /// authenticates nothing and authorizes nothing — it carries the host's
    /// caller through to the tools AND to the task methods, which apply the
    /// host's own model. That is what "auth is inherited" means concretely:
    /// there is no second auth path here to drift from the first, and no
    /// request that reaches the host's data without carrying its caller.
    pub async fn handle_http(&self, request: &HttpRequest, caller: &S::Caller) -> HttpResponse {
        // Origin first: a DNS-rebinding attempt must be refused before its body
        // is parsed, let alone dispatched.
        if let Some(origin) = request.origin()
            && !self.config.origin_allowed(origin)
        {
            return HttpResponse::json(
                403,
                &json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": crate::protocol::error::error_codes::INVALID_REQUEST,
                        "message": "Origin is not permitted to reach this MCP endpoint",
                    }
                }),
            );
        }
        if request.method != "POST" {
            return HttpResponse::method_not_allowed();
        }
        let message = match IncomingMessage::parse(&request.body) {
            Ok(message) => message,
            Err(error) => return failure_response(None, &error),
        };
        // A notification is acknowledged with 202 and no body, and is never
        // answered with a JSON-RPC response. This server defines no inbound
        // notification, so nothing acts on it — but acknowledging it is still
        // the correct transport behaviour and is not the same as ignoring it.
        if message.is_notification() {
            return HttpResponse::empty(202);
        }
        let id = message.id.clone();
        match self.dispatch(&message, request, caller).await {
            Ok(result) => HttpResponse::json(200, &JsonRpcResponse::success(id, result).to_value()),
            Err(error) => failure_response(id, &error),
        }
    }

    async fn dispatch(
        &self,
        message: &IncomingMessage,
        request: &HttpRequest,
        caller: &S::Caller,
    ) -> Result<Value, ProtocolError> {
        let meta = RequestMeta::parse(&message.params)?;
        validate_headers(&request.headers, message, &meta, self.service.catalog())?;
        route(&self.service, &self.config, caller, message, &meta).await
    }
}

impl JsonRpcResponse {
    /// Render this response as a JSON value.
    ///
    /// Serialization of a response cannot fail (every field is a plain JSON
    /// type), but the fallible API is honoured rather than unwrapped: a
    /// serializer failure degrades to an internal-error body carrying the same
    /// request id instead of a panic.
    #[must_use]
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|error| {
            json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": crate::protocol::error::error_codes::INTERNAL_ERROR,
                    "message": format!("the MCP response could not be encoded: {error}"),
                }
            })
        })
    }
}

fn failure_response(id: Option<RequestId>, error: &ProtocolError) -> HttpResponse {
    HttpResponse::json(
        error.http_status(),
        &JsonRpcResponse::failure(id, error).to_value(),
    )
}
