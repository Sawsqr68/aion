//! `tools/list` and `tools/call`.

use std::sync::Arc;

use serde_json::{Map, Value, json};

use crate::protocol::error::ProtocolError;
use crate::protocol::meta::RequestMeta;
use crate::protocol::result::{RESULT_TYPE_COMPLETE, RESULT_TYPE_TASK, result_envelope};
use crate::protocol::version::TASKS_EXTENSION;
use crate::tasks::store::TaskStore;
use crate::tools::service::{ToolCall, ToolService, ToolTaskDecision, validate_against};

use super::call_result::{call_tool_failure_result, call_tool_success_result};
use super::config::McpServerConfig;

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
/// * [`ProtocolError::Internal`] when the task store cannot record the task, or
///   when a tool violates its own published `outputSchema`.
pub async fn execute_call<S: ToolService>(
    service: &Arc<S>,
    config: &McpServerConfig,
    tasks: &TaskStore,
    meta: &RequestMeta,
    caller: &S::Caller,
    call: ToolCall,
) -> Result<Value, ProtocolError> {
    match service.task_decision(&call) {
        ToolTaskDecision::Task => {
            if !meta.client_capabilities.declares_tasks() {
                return Err(ProtocolError::MissingClientCapability {
                    required: vec![format!("extensions.{TASKS_EXTENSION}")],
                });
            }
            create_task(service, config, tasks, caller, call)
        }
        ToolTaskDecision::Inline => {
            let outcome = service.call(caller, call.clone()).await;
            let mut result = result_envelope(RESULT_TYPE_COMPLETE, &config.server_info);
            let body = match outcome {
                Ok(outcome) => {
                    call_tool_success_result(service.catalog().find(&call.name), &outcome)
                        .map_err(|detail| ProtocolError::Internal { detail })?
                }
                Err(failure) => call_tool_failure_result(&failure),
            };
            merge(&mut result, body);
            Ok(Value::Object(result))
        }
    }
}

fn create_task<S: ToolService>(
    service: &Arc<S>,
    config: &McpServerConfig,
    tasks: &TaskStore,
    caller: &S::Caller,
    call: ToolCall,
) -> Result<Value, ProtocolError> {
    let now = chrono::Utc::now();
    let (task, cancel) = tasks
        .create(
            now,
            config.task_ttl_ms,
            config.task_poll_interval_ms,
            Some(format!("`{}` is running", call.name)),
        )
        .map_err(|error| ProtocolError::Internal {
            detail: format!("the task could not be created: {error}"),
        })?;
    // The task is in the store BEFORE the executor is spawned and before this
    // returns, so a `tasks/get` issued the instant the client reads the
    // creation result already resolves.
    tokio::spawn(crate::tasks::runner::drive(
        Arc::clone(service),
        tasks.clone(),
        task.task_id.clone(),
        caller.clone(),
        call,
        cancel,
        config.task_ttl_ms.map(std::time::Duration::from_millis),
    ));
    let mut result = result_envelope(RESULT_TYPE_TASK, &config.server_info);
    task.write_into(&mut result);
    Ok(Value::Object(result))
}

fn merge(target: &mut Map<String, Value>, source: Value) {
    if let Value::Object(source) = source {
        for (key, value) in source {
            target.insert(key, value);
        }
    }
}
