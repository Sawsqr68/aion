//! `tasks/get`, `tasks/update`, and `tasks/cancel`.
//!
//! All three are gated on the request declaring the tasks extension: a client
//! that has not declared it gets `-32021` with `data.requiredCapabilities`,
//! never a task.
//!
//! All three also RESOLVE the task before answering, under the caller's own
//! identity. That resolution is the authorization: a handle names durable work
//! the host holds, it is derived rather than secret, and holding one is
//! therefore not evidence of anything. A method that acknowledged without
//! resolving would be answering for work it never checked the caller could see.
//!
//! There is deliberately no `tasks/list`. Its absence is the reason a server
//! cannot leak the existence of one caller's tasks to another, and adding one
//! would undo that.

use serde_json::{Map, Value};

use crate::protocol::error::ProtocolError;
use crate::protocol::meta::RequestMeta;
use crate::protocol::result::{RESULT_TYPE_COMPLETE, result_envelope};
use crate::protocol::version::TASKS_EXTENSION;
use crate::tasks::model::DetailedTask;
use crate::tools::service::ToolService;

use super::config::McpServerConfig;

/// Read `params.taskId`, enforcing the extension gate first.
///
/// # Errors
///
/// [`ProtocolError::MissingClientCapability`] when the request did not declare
/// the tasks extension, and [`ProtocolError::InvalidParams`] when `taskId` is
/// absent or is not a string.
pub fn required_task_id(
    meta: &RequestMeta,
    params: &Map<String, Value>,
) -> Result<String, ProtocolError> {
    if !meta.client_capabilities.declares_tasks() {
        return Err(ProtocolError::MissingClientCapability {
            required: vec![format!("extensions.{TASKS_EXTENSION}")],
        });
    }
    Ok(params
        .get("taskId")
        .and_then(Value::as_str)
        .ok_or_else(|| ProtocolError::InvalidParams {
            detail: "params.taskId is required and must be a string".to_owned(),
        })?
        .to_owned())
}

/// Resolve a task under the caller's authorization, as a detailed task.
///
/// This is the single place a handle becomes a task, used by `tasks/get` and by
/// the creation path alike, so the state a client is told at creation and the
/// state it reads a moment later come from ONE projection rather than two.
///
/// # Errors
///
/// Whatever [`ToolService::resolve_task`] refuses, mapped onto its protocol
/// error: `-32602` for a handle that names nothing this caller can read,
/// `-32603` when the durable work could not be read at all.
pub async fn resolve_detailed<S: ToolService>(
    service: &S,
    config: &McpServerConfig,
    caller: &S::Caller,
    task_id: &str,
) -> Result<DetailedTask, ProtocolError> {
    let projection = service
        .resolve_task(caller, task_id)
        .await
        .map_err(|error| error.to_protocol_error())?;
    Ok(projection.into_detailed(task_id, config.task_poll_interval_ms, service.catalog()))
}

/// `tasks/get`.
///
/// `resultType` is `"complete"`, not `"task"`: `"task"` marks a
/// `CreateTaskResult` and nothing else. The status-appropriate payload rides
/// alongside the flat task fields — `result` on a completed task (which is
/// exactly what the underlying call would have returned, including an
/// `isError: true` tool result), `error` on a failed one.
///
/// # Errors
///
/// The extension gate and `taskId` errors from [`required_task_id`], plus
/// whatever resolving the handle refuses.
pub async fn tasks_get<S: ToolService>(
    service: &S,
    config: &McpServerConfig,
    caller: &S::Caller,
    meta: &RequestMeta,
    params: &Map<String, Value>,
) -> Result<Value, ProtocolError> {
    let task_id = required_task_id(meta, params)?;
    let detail = resolve_detailed(service, config, caller, &task_id).await?;
    let mut result = result_envelope(RESULT_TYPE_COMPLETE, &config.server_info);
    detail.write_into(&mut result);
    Ok(Value::Object(result))
}

/// `tasks/cancel` — an empty acknowledgement.
///
/// Cancellation is cooperative and eventually consistent: the server is
/// obligated to acknowledge, not to stop the work. The extension says so in as
/// many words — the observable status MAY remain `working` after the ack and
/// MAY reach a terminal status other than `cancelled` — and it defines no
/// `cancel_requested` status, so there is nowhere a "cancellation was asked
/// for" fact could even be shown. Nothing is recorded here, therefore, and
/// nothing needs to be.
///
/// This host's answer to "whether and when to honor it" is *never*: the work
/// behind a task is a durable run, and a durable run is not something an MCP
/// client's disinterest is allowed to kill. The task's `statusMessage` says so
/// on every read and names the tool that DOES stop a run.
///
/// The task is still resolved first, so a handle naming nothing this caller can
/// read is refused instead of being cheerfully acknowledged.
///
/// `notifications/cancelled` is NOT used for this and is not accepted.
///
/// # Errors
///
/// The extension gate and `taskId` errors from [`required_task_id`], plus
/// whatever resolving the handle refuses.
pub async fn tasks_cancel<S: ToolService>(
    service: &S,
    config: &McpServerConfig,
    caller: &S::Caller,
    meta: &RequestMeta,
    params: &Map<String, Value>,
) -> Result<Value, ProtocolError> {
    let task_id = required_task_id(meta, params)?;
    resolve_detailed(service, config, caller, &task_id).await?;
    Ok(Value::Object(result_envelope(
        RESULT_TYPE_COMPLETE,
        &config.server_info,
    )))
}

/// `tasks/update` — deliver responses to a task's outstanding input requests.
///
/// This server never puts a task into `input_required`: it opens no elicitation,
/// no sampling, and no roots request, so a task it created has no outstanding
/// input request key at any point in its life. Every `inputResponses` key is
/// therefore one that was never issued, and the extension's rule for such a key
/// is to IGNORE it. The acknowledgement is the honest answer — inventing a
/// failure would tell a client its perfectly legal request was malformed.
///
/// # Errors
///
/// The extension gate and `taskId` errors from [`required_task_id`], whatever
/// resolving the handle refuses, and [`ProtocolError::InvalidParams`] for a
/// missing or malformed `inputResponses` object.
pub async fn tasks_update<S: ToolService>(
    service: &S,
    config: &McpServerConfig,
    caller: &S::Caller,
    meta: &RequestMeta,
    params: &Map<String, Value>,
) -> Result<Value, ProtocolError> {
    let task_id = required_task_id(meta, params)?;
    // Authorization BEFORE body validation: a caller who may not read this task
    // learns nothing about it from how well-formed its request happened to be.
    resolve_detailed(service, config, caller, &task_id).await?;
    if !params.get("inputResponses").is_some_and(Value::is_object) {
        return Err(ProtocolError::InvalidParams {
            detail: "params.inputResponses is required and must be an object".to_owned(),
        });
    }
    Ok(Value::Object(result_envelope(
        RESULT_TYPE_COMPLETE,
        &config.server_info,
    )))
}
