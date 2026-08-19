//! Method routing over a validated request.
//!
//! The order of checks is the contract, and it is not arbitrary:
//!
//! 1. `_meta` — version first (so an unimplemented revision is told which
//!    revisions exist), then the required capabilities object.
//! 2. Standard headers — proven against the body values they mirror.
//! 3. The method itself.
//!
//! Anything unrecognised is `404` + `-32601`, carrying the JSON-RPC body that
//! distinguishes it from a legacy HTTP+SSE 404.

use std::sync::Arc;

use serde_json::{Map, Value};

use crate::protocol::error::ProtocolError;
use crate::protocol::headers::{
    HEADER_METHOD, HEADER_NAME, HEADER_PROTOCOL_VERSION, HeaderView, reject_unexpected,
    require_matching, require_matching_param,
};
use crate::protocol::jsonrpc::IncomingMessage;
use crate::protocol::meta::RequestMeta;
use crate::tools::service::{ToolCatalog, ToolService};

use super::config::McpServerConfig;
use super::discover::discover_result;
use super::tasks_methods::{tasks_cancel, tasks_get, tasks_update};
use super::tools_methods::{decode_call, execute_call, tools_list_result};

/// Every method this server implements.
pub const IMPLEMENTED_METHODS: &[&str] = &[
    "server/discover",
    "tools/list",
    "tools/call",
    "tasks/get",
    "tasks/update",
    "tasks/cancel",
];

/// Validate the standard headers of one request against its body.
///
/// # Errors
///
/// [`ProtocolError::HeaderMismatch`] for a missing, mismatched, unexpected, or
/// invalidly-encoded header.
pub fn validate_headers(
    headers: &HeaderView,
    message: &IncomingMessage,
    meta: &RequestMeta,
    catalog: &ToolCatalog,
) -> Result<(), ProtocolError> {
    require_matching(
        headers,
        HEADER_PROTOCOL_VERSION,
        "MCP-Protocol-Version",
        &meta.protocol_version,
    )?;
    require_matching(headers, HEADER_METHOD, "Mcp-Method", &message.method)?;
    match mcp_name_source(&message.method, &message.params) {
        Some(expected) => require_matching(headers, HEADER_NAME, "Mcp-Name", &expected)?,
        // The body carries no `Mcp-Name` source for this method, so the header
        // must not be expected — and must not be accepted either: it would be
        // asserting agreement with a value that does not exist.
        None => reject_unexpected(headers, HEADER_NAME, "Mcp-Name")?,
    }
    validate_param_headers(headers, message, catalog)
}

/// Validate every `Mcp-Param-{Name}` header the called tool RECOGNISES.
///
/// The three-armed matrix, in full:
///
/// * the body carries a value and the header is present → they must agree,
///   after Base64-sentinel decoding, with integers compared numerically;
/// * the body carries a value and the header is absent → reject: the client
///   dropped half of a contract the tool declared;
/// * the body carries no value (absent, or explicitly `null`) → the header must
///   NOT be sent, because there is nothing it could be agreeing with.
///
/// Headers whose suffix matches no annotation of the called tool are
/// unrecognised and are forwarded-and-ignored, which is what RFC 9110 requires
/// of an intermediary and is the only behaviour that lets a proxy add its own
/// headers without breaking every server it fronts.
fn validate_param_headers(
    headers: &HeaderView,
    message: &IncomingMessage,
    catalog: &ToolCatalog,
) -> Result<(), ProtocolError> {
    if message.method != "tools/call" {
        return Ok(());
    }
    let Some(name) = message.params.get("name").and_then(Value::as_str) else {
        return Ok(());
    };
    let arguments = message
        .params
        .get("arguments")
        .cloned()
        .unwrap_or(Value::Null);
    for binding in catalog.bindings_of(name) {
        let header = binding.full_header();
        let display = format!("Mcp-Param-{}", binding.header_name);
        match binding.resolve(&arguments) {
            Some(expected) => require_matching_param(headers, &header, &display, expected)?,
            None => reject_unexpected(headers, &header, &display)?,
        }
    }
    Ok(())
}

/// The body value `Mcp-Name` must mirror for `method`, if any.
///
/// `tools/call` takes it from `params.name`. The tasks methods take it from
/// `params.taskId` — a requirement that originates in the extension, not in the
/// core header table, and which the core's own header-agreement rule then makes
/// enforceable.
fn mcp_name_source(method: &str, params: &Map<String, Value>) -> Option<String> {
    let key = match method {
        "tools/call" => "name",
        "tasks/get" | "tasks/update" | "tasks/cancel" => "taskId",
        _ => return None,
    };
    params.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// Route one validated request to its handler.
///
/// # Errors
///
/// [`ProtocolError::MethodNotFound`] for an unimplemented method, and whatever
/// the handler itself returns.
pub async fn route<S: ToolService>(
    service: &Arc<S>,
    config: &McpServerConfig,
    caller: &S::Caller,
    message: &IncomingMessage,
    meta: &RequestMeta,
) -> Result<Value, ProtocolError> {
    let service = service.as_ref();
    match message.method.as_str() {
        "server/discover" => Ok(discover_result(
            config,
            &meta.client_capabilities,
            service.instructions(),
        )),
        "tools/list" => Ok(tools_list_result(service, config)),
        "tools/call" => {
            let call = decode_call(service, &message.params)?;
            execute_call(service, config, meta, caller, call).await
        }
        // Every task method carries the caller: the handle is constructible, so
        // possession of it authorizes nothing on its own.
        "tasks/get" => tasks_get(service, config, caller, meta, &message.params).await,
        "tasks/update" => tasks_update(service, config, caller, meta, &message.params).await,
        "tasks/cancel" => tasks_cancel(service, config, caller, meta, &message.params).await,
        other => Err(ProtocolError::MethodNotFound {
            method: other.to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value, json};

    use super::{IMPLEMENTED_METHODS, mcp_name_source};

    #[test]
    fn mcp_name_is_sourced_from_the_right_body_field_per_method() -> Result<(), serde_json::Error> {
        let call: Map<String, Value> = serde_json::from_value(json!({ "name": "read_history" }))?;
        assert_eq!(
            mcp_name_source("tools/call", &call),
            Some("read_history".to_owned())
        );
        let task: Map<String, Value> = serde_json::from_value(json!({ "taskId": "t-1" }))?;
        assert_eq!(mcp_name_source("tasks/get", &task), Some("t-1".to_owned()));
        assert_eq!(
            mcp_name_source("tasks/cancel", &task),
            Some("t-1".to_owned())
        );
        assert_eq!(mcp_name_source("server/discover", &Map::new()), None);
        assert_eq!(mcp_name_source("tools/list", &Map::new()), None);
        Ok(())
    }

    #[test]
    fn subscriptions_listen_is_not_implemented() {
        assert!(!IMPLEMENTED_METHODS.contains(&"subscriptions/listen"));
        assert!(!IMPLEMENTED_METHODS.contains(&"initialize"));
        assert!(!IMPLEMENTED_METHODS.contains(&"tasks/list"));
    }
}
