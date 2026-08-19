//! `tools/list` and `tools/call`.

use serde_json::{Map, Value, json};

use crate::protocol::error::ProtocolError;
use crate::protocol::meta::RequestMeta;
use crate::protocol::result::{RESULT_TYPE_COMPLETE, RESULT_TYPE_TASK, result_envelope};
use crate::protocol::version::TASKS_EXTENSION;
use crate::tools::service::{ToolCall, ToolService, ToolTaskDecision, validate_against};

use super::call_result::{call_tool_failure_result, call_tool_success_result};
use super::config::McpServerConfig;
use super::tasks_methods::resolve_detailed;

/// Build the `tools/list` result.
///
/// The catalog is a fixed set in a fixed order, so the result is deterministic
/// across requests and cannot vary as a side effect of anything that happened
/// on the connection — there is no connection state to vary with.
///
/// No pagination cursor is emitted: the catalog is deliberately small and is
/// returned whole, so there is no page for a `cacheScope` to disagree across.
#[must_use]
pub fn tools_list_result<S: ToolService>(service: &S, config: &McpServerConfig) -> Value {
    let mut result = result_envelope(RESULT_TYPE_COMPLETE, &config.server_info);
    result.insert("tools".to_owned(), json!(service.catalog().tools()));
    config.tools_list_cache.apply(&mut result);
    Value::Object(result)
}

/// Decode a `tools/call` request's parameters.
///
/// # Errors
///
/// [`ProtocolError::InvalidParams`] when `name` is absent or not a string, when
/// `arguments` is present but is not an object, or when the named tool is not
/// published.
pub fn decode_call<S: ToolService>(
    service: &S,
    params: &Map<String, Value>,
) -> Result<ToolCall, ProtocolError> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| ProtocolError::InvalidParams {
            detail: "params.name is required and must be a string".to_owned(),
        })?
        .to_owned();
    let arguments = match params.get("arguments") {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(arguments)) => arguments.clone(),
        Some(_) => {
            return Err(ProtocolError::InvalidParams {
                detail: "params.arguments must be an object".to_owned(),
            });
        }
    };
    let Some(tool) = service.catalog().find(&name) else {
        return Err(ProtocolError::InvalidParams {
            detail: format!("Unknown tool: {name}"),
        });
    };
    // Input validation happens HERE, before the host's tool sees a byte: the
    // catalog published a contract and the boundary is where it is enforced.
    validate_against(&tool.input_schema, &Value::Object(arguments.clone())).map_err(
        |violations| ProtocolError::InvalidParams {
            detail: format!("arguments for tool `{name}` are invalid: {violations}"),
        },
    )?;
    Ok(ToolCall { name, arguments })
}

/// Execute a decoded call, inline or as a task.
///
/// # Errors
///
/// * [`ProtocolError::MissingClientCapability`] when the server decided this
///   call must run as a task and the request did not declare the tasks
///   extension. The call is refused rather than silently downgraded to an
///   inline call that would block the client for as long as the work takes.
/// * [`ProtocolError::Internal`] when a tool violates its own published
///   `outputSchema`, or when the host cannot name the work a task-shaped call
///   just started.
pub async fn execute_call<S: ToolService>(
    service: &S,
    config: &McpServerConfig,
    meta: &RequestMeta,
    caller: &S::Caller,
    call: ToolCall,
) -> Result<Value, ProtocolError> {
    let decision = service.task_decision(&call);
    if decision == ToolTaskDecision::Task && !meta.client_capabilities.declares_tasks() {
        return Err(ProtocolError::MissingClientCapability {
            required: vec![format!("extensions.{TASKS_EXTENSION}")],
        });
    }
    // The call is performed the same way either way: the work is really done
    // and confirmed before this returns. A task-shaped call differs only in
    // what it is ANSWERED with — a handle onto durable work instead of that
    // work's finished result.
    let outcome = match service.call(caller, call.clone()).await {
        Ok(outcome) => outcome,
        // A failed call is answered INLINE even when the decision was `Task`. A
        // task pointing at work that never started could never resolve, and an
        // unresolvable handle is worse than the error it would be hiding.
        Err(failure) => {
            let mut result = result_envelope(RESULT_TYPE_COMPLETE, &config.server_info);
            merge(&mut result, call_tool_failure_result(&failure));
            return Ok(Value::Object(result));
        }
    };
    match decision {
        ToolTaskDecision::Task => create_task(service, config, caller, &call, &outcome).await,
        ToolTaskDecision::Inline => {
            let mut result = result_envelope(RESULT_TYPE_COMPLETE, &config.server_info);
            let body = call_tool_success_result(service.catalog().find(&call.name), &outcome)
                .map_err(|detail| ProtocolError::Internal { detail })?;
            merge(&mut result, body);
            Ok(Value::Object(result))
        }
    }
}

/// Answer a completed task-shaped call with a handle onto the work it started.
///
/// The creation result is built by RESOLVING that handle rather than by
/// describing what was just done, so the answer comes from the same projection
/// every later `tasks/get` reads. Work that was already finished by the time
/// the handle was minted is reported as finished here, immediately — there is
/// no moment where the server claims `working` about work that has stopped.
async fn create_task<S: ToolService>(
    service: &S,
    config: &McpServerConfig,
    caller: &S::Caller,
    call: &ToolCall,
    outcome: &crate::tools::service::ToolOutcome,
) -> Result<Value, ProtocolError> {
    let task_id = service
        .task_handle(call, outcome)
        .ok_or_else(|| ProtocolError::Internal {
            detail: format!(
                "`{}` was answered as a task but the server could not name the work it started",
                call.name
            ),
        })?;
    let detail = resolve_detailed(service, config, caller, &task_id).await?;
    // A `CreateTaskResult` carries the FLAT task record. The status-specific
    // payload is what `tasks/get` is for.
    let mut result = result_envelope(RESULT_TYPE_TASK, &config.server_info);
    detail.task().write_into(&mut result);
    Ok(Value::Object(result))
}

fn merge(target: &mut Map<String, Value>, source: Value) {
    if let Value::Object(source) = source {
        for (key, value) in source {
            target.insert(key, value);
        }
    }
}
