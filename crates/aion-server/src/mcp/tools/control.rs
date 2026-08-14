//! `signal`, `query`, and `cancel`.
//!
//! All three go through the same shared handlers the HTTP and gRPC surfaces
//! use — `handlers::signal`, `handlers::query`, `handlers::cancel` — as
//! in-process calls. There is no HTTP self-call and no second copy of the
//! namespace gate: the MCP surface is another mouth on the same handler, which
//! is the only arrangement in which the two surfaces cannot disagree about who
//! may do what.

use aion_mcp::tools::service::{ToolCall, ToolFailure, ToolOutcome};
use aion_proto::{
    ProtoCancelRequest, ProtoQueryRequest, ProtoSignalRequest, ProtoWorkflowId, WireError,
    proto_query_response,
};
use serde_json::json;

use crate::mcp::args::{
    optional_json, optional_run_id, optional_str, required_str, required_workflow_id,
};
use crate::{CallerIdentity, ServerState, api::handlers};

use super::errors::tool_failure;

/// Run `signal`.
///
/// # Errors
///
/// [`ToolFailure`] when the arguments are malformed, the caller does not hold
/// the namespace, the run is terminal, or the engine call fails.
pub(crate) async fn signal(
    state: &ServerState,
    caller: &CallerIdentity,
    call: &ToolCall,
) -> Result<ToolOutcome, ToolFailure> {
    let namespace = required_str(call, "namespace")?;
    let workflow_id = required_workflow_id(call, "workflow_id")?;
    let run_id = optional_run_id(call, "run_id")?;
    let signal_name = required_str(call, "signal_name")?;
    let payload = optional_json(call, "payload")
        .map(|value| {
            aion_core::Payload::from_json(&value).map_err(|error| {
                tool_failure(&WireError::invalid_input(format!(
                    "the signal payload could not be encoded: {error}"
                )))
            })
        })
        .transpose()?;

    handlers::signal(
        state.namespace_guard(),
        caller,
        ProtoSignalRequest {
            namespace,
            workflow_id: Some(ProtoWorkflowId {
                uuid: workflow_id.to_string(),
            }),
            run_id: run_id.map(Into::into),
            signal_name: signal_name.clone(),
            payload: payload.map(Into::into),
        },
    )
    .await
    .map_err(|error| tool_failure(&error))?;

    Ok(ToolOutcome {
        summary: format!("signal `{signal_name}` delivered to workflow {workflow_id}"),
        structured: json!({
            "workflow_id": workflow_id.to_string(),
            "signal_name": signal_name,
            "delivered": true,
        }),
    })
}

/// Run `query`.
///
/// A query whose handler ran and reported a failure is a TOOL failure, not a
/// success carrying an error field: the model asked a question and did not get
/// an answer, and dressing that as a result would invite it to reason over the
/// error text as if it were the state.
///
/// # Errors
///
/// [`ToolFailure`] when the arguments are malformed, the caller does not hold
/// the namespace, the query name is unregistered, the query times out, or the
/// handler reports a failure.
pub(crate) async fn query(
    state: &ServerState,
    caller: &CallerIdentity,
    call: &ToolCall,
) -> Result<ToolOutcome, ToolFailure> {
    let namespace = required_str(call, "namespace")?;
    let workflow_id = required_workflow_id(call, "workflow_id")?;
    let run_id = optional_run_id(call, "run_id")?;
    let query_name = required_str(call, "query_name")?;
    // Absent arguments stay absent on the wire: the server materializes the
    // canonical JSON `null` document, so the handler always receives one
    // well-formed input without this surface inventing a second default.
    let arguments = optional_json(call, "arguments")
        .map(|value| {
            aion_core::Payload::from_json(&value).map_err(|error| {
                tool_failure(&WireError::invalid_input(format!(
                    "the query arguments could not be encoded: {error}"
                )))
            })
        })
        .transpose()?;

    let response = handlers::query(
        state.namespace_guard(),
        caller,
        ProtoQueryRequest {
            namespace,
            workflow_id: Some(ProtoWorkflowId {
                uuid: workflow_id.to_string(),
            }),
            run_id: run_id.map(Into::into),
            query_name: query_name.clone(),
            arguments: arguments.map(Into::into),
        },
    )
    .await
    .map_err(|error| tool_failure(&error))?;

    let result = match response.outcome {
        Some(proto_query_response::Outcome::Result(payload)) => {
            let payload =
                aion_core::Payload::try_from(payload).map_err(|error| tool_failure(&error))?;
            payload.to_json().map_err(|error| {
                tool_failure(&WireError::backend(format!(
                    "the query result is not JSON: {error}"
                )))
            })?
        }
        Some(proto_query_response::Outcome::Error(error)) => {
            let error = WireError::try_from(error).map_err(|error| tool_failure(&error))?;
            return Err(tool_failure(&error));
        }
        None => {
            return Err(tool_failure(&WireError::backend(
                "the query response carried no outcome",
            )));
        }
    };

    Ok(ToolOutcome {
        summary: format!("query `{query_name}` answered by workflow {workflow_id}"),
        structured: json!({
            "workflow_id": workflow_id.to_string(),
            "query_name": query_name,
            "result": result,
        }),
    })
}

/// Run `cancel`.
///
/// # Errors
///
/// [`ToolFailure`] when the arguments are malformed, the caller does not hold
/// the namespace, the run is already terminal, or the engine call fails.
pub(crate) async fn cancel(
    state: &ServerState,
    caller: &CallerIdentity,
    call: &ToolCall,
) -> Result<ToolOutcome, ToolFailure> {
    let namespace = required_str(call, "namespace")?;
    let workflow_id = required_workflow_id(call, "workflow_id")?;
    let run_id = optional_run_id(call, "run_id")?;
    // An empty reason is what the engine records when none is given; it is not
    // a sentinel and is not substituted for.
    let reason = optional_str(call, "reason").unwrap_or_default();

    handlers::cancel(
        state,
        state.namespace_guard(),
        caller,
        ProtoCancelRequest {
            namespace,
            workflow_id: Some(ProtoWorkflowId {
                uuid: workflow_id.to_string(),
            }),
            run_id: run_id.map(Into::into),
            reason: reason.clone(),
        },
    )
    .await
    .map_err(|error| tool_failure(&error))?;

    Ok(ToolOutcome {
        summary: format!("workflow {workflow_id} cancelled"),
        structured: json!({
            "workflow_id": workflow_id.to_string(),
            "cancelled": true,
            "reason": reason,
        }),
    })
}
