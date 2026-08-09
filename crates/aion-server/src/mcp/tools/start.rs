//! `start_run`, including the awaited (task-shaped) variant.

use std::time::Duration;

use aion_core::{RunId, WorkflowId, WorkflowStatus};
use aion_mcp::tools::service::{ToolCall, ToolFailure, ToolOutcome};
use aion_proto::{ProtoStartWorkflowRequest, WireError};
use serde_json::json;

use crate::mcp::args::{optional_bool, optional_json, optional_str, required_str};
use crate::{CallerIdentity, ServerState};

use super::errors::tool_failure;
use super::run_view::RunView;

/// Whether this `start_run` call asked to be awaited.
///
/// This is the SERVER's task decision in the extension's sense: the tool
/// argument says what work the caller wants done ("start it and tell me how it
/// ended"), and the server is what decides that such work can only be served as
/// a task. A client cannot ask for a task-shaped response to work that finishes
/// immediately, and cannot refuse one for work that does not.
pub(crate) fn awaits_completion(call: &ToolCall) -> bool {
    call.name == "start_run" && optional_bool(call, "await_completion")
}

/// Run `start_run`.
///
/// # Errors
///
/// [`ToolFailure`] when the arguments are malformed, the caller does not hold
/// the namespace, the workflow type is not deployed, the server is draining, or
/// the engine start fails. When awaiting, also when the run's terminal status
/// cannot be read back.
pub(crate) async fn start_run(
    state: &ServerState,
    caller: &CallerIdentity,
    call: &ToolCall,
    await_poll_interval: Duration,
) -> Result<ToolOutcome, ToolFailure> {
    if state.drain_state().is_draining() {
        return Err(ToolFailure::new(
            "this server is draining and is not accepting new workflow starts",
            json!({ "code": "draining" }),
        ));
    }
    let namespace = required_str(call, "namespace")?;
    let workflow_type = required_str(call, "workflow_type")?;
    let input = optional_json(call, "input")
        .map(|value| {
            aion_core::Payload::from_json(&value).map_err(|error| {
                tool_failure(&WireError::invalid_input(format!(
                    "the workflow input could not be encoded: {error}"
                )))
            })
        })
        .transpose()?;
    let minter = state.namespace_minter();
    let response = crate::api::handlers::start_with_placement(
        state.namespace_guard(),
        caller,
        ProtoStartWorkflowRequest {
            namespace: namespace.clone(),
            workflow_type: workflow_type.clone(),
            input: input.map(Into::into),
            routing_key: optional_str(call, "routing_key"),
            task_queue: optional_str(call, "task_queue"),
        },
        None,
        Some(&minter),
    )
    .await
    .map_err(|error| tool_failure(&error))?;

    let workflow_id: WorkflowId = response
        .workflow_id
        .ok_or_else(|| {
            tool_failure(&WireError::backend(
                "the start response carried no workflow id",
            ))
        })?
        .try_into()
        .map_err(|error| tool_failure(&error))?;
    let run_id: RunId = response
        .run_id
        .ok_or_else(|| tool_failure(&WireError::backend("the start response carried no run id")))?
        .try_into()
        .map_err(|error| tool_failure(&error))?;

    if !awaits_completion(call) {
        return Ok(ToolOutcome {
            summary: format!(
                "started {workflow_type} as workflow {workflow_id} run {run_id}; \
                 call describe_run with those handles to follow it"
            ),
            structured: json!({
                "namespace": namespace,
                "workflow_id": workflow_id.to_string(),
                "run_id": run_id.to_string(),
                "workflow_type": workflow_type,
                "status": WorkflowStatus::Running,
                "awaited": false,
            }),
        });
    }

    let status =
        await_terminal(state, caller, &namespace, &workflow_id, await_poll_interval).await?;
    Ok(ToolOutcome {
        summary: format!("{workflow_type} workflow {workflow_id} finished as {status:?}"),
        structured: json!({
            "namespace": namespace,
            "workflow_id": workflow_id.to_string(),
            "run_id": run_id.to_string(),
            "workflow_type": workflow_type,
            "status": status,
            "awaited": true,
        }),
    })
}

/// Poll a workflow until its projected status is terminal.
///
/// The loop has no iteration cap and no deadline of its own, deliberately: a
/// durable run may legitimately take three months, and a cap here would be a
/// lie about when the answer arrives. The bounds that DO apply are the task's
/// own — the client's `tasks/cancel` and the configured task TTL — both of
/// which drop this future from the outside. That keeps exactly one authority
/// over how long a wait may last, and it is the operator's configuration
/// rather than a number chosen here.
///
/// `ContinuedAsNew` is terminal for THIS run and is reported as such; the chain
/// carries on under a new run, which `describe_run` will show.
async fn await_terminal(
    state: &ServerState,
    caller: &CallerIdentity,
    namespace: &str,
    workflow_id: &WorkflowId,
    poll_interval: Duration,
) -> Result<WorkflowStatus, ToolFailure> {
    loop {
        let view = RunView::read_parts(state, caller, namespace, workflow_id, None).await?;
        if view.summary.status.is_terminal() {
            return Ok(view.summary.status);
        }
        tokio::time::sleep(poll_interval).await;
    }
}

#[cfg(test)]
mod tests {
    use aion_mcp::tools::service::ToolCall;
    use serde_json::{Map, Value, json};

    use super::awaits_completion;

    fn call(name: &str, arguments: &Value) -> Result<ToolCall, serde_json::Error> {
        Ok(ToolCall {
            name: name.to_owned(),
            arguments: serde_json::from_value::<Map<String, Value>>(arguments.clone())?,
        })
    }

    #[test]
    fn only_start_run_with_await_completion_is_task_shaped() -> Result<(), serde_json::Error> {
        assert!(awaits_completion(&call(
            "start_run",
            &json!({ "await_completion": true })
        )?));
        assert!(!awaits_completion(&call(
            "start_run",
            &json!({ "await_completion": false })
        )?));
        assert!(!awaits_completion(&call("start_run", &json!({}))?));
        // No other tool is ever task-shaped, whatever arguments it carries.
        assert!(!awaits_completion(&call(
            "describe_run",
            &json!({ "await_completion": true })
        )?));
        Ok(())
    }
}
