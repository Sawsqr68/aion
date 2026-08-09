//! `tasks/get`, `tasks/update`, and `tasks/cancel`.
//!
//! All three are gated on the request declaring the tasks extension: a client
//! that has not declared it gets `-32021` with `data.requiredCapabilities`,
//! never a task.
//!
//! There is deliberately no `tasks/list`. Its absence is the reason a server
//! cannot leak the existence of one caller's tasks to another, and adding one
//! would undo that.

use serde_json::{Map, Value};

use crate::protocol::error::ProtocolError;
use crate::protocol::meta::RequestMeta;
use crate::protocol::result::{RESULT_TYPE_COMPLETE, result_envelope};
use crate::protocol::version::TASKS_EXTENSION;
use crate::tasks::store::{TaskStore, TaskStoreError};

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
/// [`ProtocolError::InvalidParams`] for an unknown or expired task.
pub fn tasks_get(
    config: &McpServerConfig,
    tasks: &TaskStore,
    meta: &RequestMeta,
    params: &Map<String, Value>,
) -> Result<Value, ProtocolError> {
    let task_id = required_task_id(meta, params)?;
    let detail = tasks
        .get(&task_id, chrono::Utc::now())
        .map_err(map_store_error)?;
    let mut result = result_envelope(RESULT_TYPE_COMPLETE, &config.server_info);
    detail.write_into(&mut result);
    Ok(Value::Object(result))
}

/// `tasks/cancel` — an empty acknowledgement.
///
/// Cancellation is cooperative and eventually consistent: the server is
/// obligated to acknowledge, not to stop the work. The task may still be
/// `working` when the next `tasks/get` lands and may still reach a terminal
/// status other than `cancelled`.
///
/// `notifications/cancelled` is NOT used for this and is not accepted.
///
/// # Errors
///
/// The extension gate and `taskId` errors from [`required_task_id`], plus
/// [`ProtocolError::InvalidParams`] for an unknown task.
pub fn tasks_cancel(
    config: &McpServerConfig,
    tasks: &TaskStore,
    meta: &RequestMeta,
    params: &Map<String, Value>,
) -> Result<Value, ProtocolError> {
    let task_id = required_task_id(meta, params)?;
    tasks.request_cancel(&task_id).map_err(map_store_error)?;
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
/// The extension gate and `taskId` errors from [`required_task_id`],
/// [`ProtocolError::InvalidParams`] for an unknown task or a missing/malformed
/// `inputResponses` object.
pub fn tasks_update(
    config: &McpServerConfig,
    tasks: &TaskStore,
    meta: &RequestMeta,
    params: &Map<String, Value>,
) -> Result<Value, ProtocolError> {
    let task_id = required_task_id(meta, params)?;
    if !params.get("inputResponses").is_some_and(Value::is_object) {
        return Err(ProtocolError::InvalidParams {
            detail: "params.inputResponses is required and must be an object".to_owned(),
        });
    }
    if !tasks.contains(&task_id).map_err(map_store_error)? {
        return Err(map_store_error(TaskStoreError::Unknown { task_id }));
    }
    Ok(Value::Object(result_envelope(
        RESULT_TYPE_COMPLETE,
        &config.server_info,
    )))
}

/// Map a store failure onto the protocol error the extension prescribes.
///
/// An unknown or expired task is `-32602`, with a message that says which of
/// the two it was — "expired" and "never existed" are different operational
/// facts and a caller deserves to be told which one it hit.
fn map_store_error(error: TaskStoreError) -> ProtocolError {
    match error {
        unknown @ (TaskStoreError::Unknown { .. } | TaskStoreError::Expired { .. }) => {
            ProtocolError::InvalidParams {
                detail: unknown.to_string(),
            }
        }
        poisoned @ TaskStoreError::LockPoisoned => ProtocolError::Internal {
            detail: poisoned.to_string(),
        },
    }
}
