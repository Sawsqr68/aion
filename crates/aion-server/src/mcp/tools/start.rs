//! `start_run`, including the awaited (task-shaped) variant.

use std::time::Duration;

use aion_core::{RunId, WorkflowId, WorkflowStatus};
use aion_mcp::tools::service::{ToolCall, ToolFailure, ToolOutcome};
use aion_proto::{ProtoStartWorkflowRequest, WireError};
use serde_json::json;

use crate::mcp::args::{
    optional_bool, optional_json, optional_non_blank_str, optional_str, required_str,
};
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
    let request = start_request(call)?;
    let namespace = request.namespace.clone();
    let workflow_type = request.workflow_type.clone();
    let minter = state.namespace_minter();
    let response = crate::api::handlers::start_with_placement(
        state.namespace_guard(),
        caller,
        request,
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

/// Build the start request from the tool call's arguments.
///
/// Extraction is separated from the engine call so the argument reading — the
/// part that decides what the caller actually asked for — is exercised by unit
/// tests directly, rather than only through a live server.
///
/// # Errors
///
/// [`ToolFailure`] when a required argument is missing, the input cannot be
/// encoded as a payload, or `display_name` is present but blank.
fn start_request(call: &ToolCall) -> Result<ProtoStartWorkflowRequest, ToolFailure> {
    let input = optional_json(call, "input")
        .map(|value| {
            aion_core::Payload::from_json(&value).map_err(|error| {
                tool_failure(&WireError::invalid_input(format!(
                    "the workflow input could not be encoded: {error}"
                )))
            })
        })
        .transpose()?;
    Ok(ProtoStartWorkflowRequest {
        namespace: required_str(call, "namespace")?,
        workflow_type: required_str(call, "workflow_type")?,
        input: input.map(Into::into),
        routing_key: optional_str(call, "routing_key"),
        task_queue: optional_str(call, "task_queue"),
        // #211: a LABEL the caller may set on the run it is starting. This is
        // not a by-name lookup — no tool takes a name as a target.
        //
        // Read with `optional_non_blank_str`, NOT `optional_str`: a blank name
        // read as absent would start the run unnamed and answer success,
        // dropping a name the caller asked for. Absent stays absent and starts
        // the run unnamed, which is what "no name" means.
        display_name: optional_non_blank_str(call, "display_name")?,
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

    use super::{awaits_completion, start_request};

    fn call(name: &str, arguments: &Value) -> Result<ToolCall, serde_json::Error> {
        Ok(ToolCall {
            name: name.to_owned(),
            arguments: serde_json::from_value::<Map<String, Value>>(arguments.clone())?,
        })
    }

    fn start_call(display_name: Option<&str>) -> Result<ToolCall, serde_json::Error> {
        let mut arguments = json!({ "namespace": "ops", "workflow_type": "settlement" });
        if let Some(display_name) = display_name {
            arguments["display_name"] = json!(display_name);
        }
        call("start_run", &arguments)
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

    /// #211: `start_run` must not reach the engine with a blank name silently
    /// turned into "unnamed". This exercises the tool's own argument reading,
    /// so swapping `optional_non_blank_str` back to `optional_str` here fails
    /// even though the helper's own tests would still pass.
    #[test]
    fn a_blank_display_name_never_reaches_the_start_request()
    -> Result<(), Box<dyn std::error::Error>> {
        for blank in ["", "   ", "\t\n "] {
            let failure = start_request(&start_call(Some(blank))?)
                .err()
                .ok_or_else(|| format!("start_run must refuse a blank display_name {blank:?}"))?;
            assert!(
                failure.message.contains("display_name"),
                "the refusal must name the argument, got {}",
                failure.message
            );
            assert_eq!(failure.detail["code"], json!("invalid_argument"));
        }
        Ok(())
    }

    /// An OMITTED name is not a dropped name: the run starts, unnamed. A name
    /// that was given is carried through untrimmed, leaving the start handler
    /// as the single authority on the recorded form.
    #[test]
    fn an_omitted_display_name_starts_unnamed_and_a_given_one_is_carried()
    -> Result<(), Box<dyn std::error::Error>> {
        let unnamed = start_request(&start_call(None)?)?;
        assert_eq!(unnamed.display_name, None);
        assert_eq!(unnamed.namespace, "ops");
        assert_eq!(unnamed.workflow_type, "settlement");

        let named = start_request(&start_call(Some("  Nightly settlement  "))?)?;
        assert_eq!(
            named.display_name.as_deref(),
            Some("  Nightly settlement  ")
        );
        Ok(())
    }
}
