//! Describe-workflow handler.

use aion_core::{Event, WorkflowSummary};
use aion_proto::{
    ProtoDescribeWorkflowRequest, ProtoDescribeWorkflowResponse, WireError,
    convert::encode_workflow_summary,
};

use super::error::workflow_not_found_error;
use super::payload::{encode_history, required_workflow_id};
use super::runs::resolve_run_id;
use crate::{CallerIdentity, NamespaceGuard, NamespaceOperation, ServerError, WorkflowTarget};

/// The describe read's full result: the wire response every transport returns,
/// plus the two inputs a transport may need to project further.
///
/// The scoped `namespace` and the decoded `history` are handed back rather than
/// dropped because the HTTP surface projects an unserved-activity verdict over
/// them ([`crate::worker::ActivityReachability`]). Recomputing either at the
/// transport would mean a second namespace scope and a second `read_history` for
/// one request, and — worse — a second answer that could disagree with the one
/// the summary was built from.
#[derive(Debug)]
pub struct DescribeOutcome {
    /// Wire response: the encoded summary and (optionally) the encoded history.
    pub response: ProtoDescribeWorkflowResponse,
    /// The namespace the caller was scoped to.
    pub namespace: String,
    /// The workflow the read resolved to.
    pub workflow_id: aion_core::WorkflowId,
    /// The run's decoded history, read in full regardless of `include_history`.
    pub history: Vec<Event>,
}

/// Handles a decoded describe-workflow request.
///
/// # Errors
///
/// Returns a stable [`WireError`] when IDs are missing or malformed, namespace scoping fails, store
/// history reading fails, the workflow has no summary, or response envelopes cannot be encoded.
pub async fn describe(
    guard: &NamespaceGuard,
    caller: &CallerIdentity,
    request: ProtoDescribeWorkflowRequest,
) -> Result<DescribeOutcome, WireError> {
    let workflow_id = required_workflow_id(request.workflow_id.clone())?;
    let target = WorkflowTarget::workflow(&workflow_id);
    let scoped = guard
        .scope(caller, &NamespaceOperation::describe(&request, target))
        .await
        .map_err(|error| error.to_wire_error())?;
    let engine = scoped.engine().map_err(|error| error.to_wire_error())?;
    resolve_run_id(engine.as_ref(), &workflow_id, request.run_id.clone()).await?;

    let history = engine
        .store()
        .read_history(&workflow_id)
        .await
        .map_err(|error| ServerError::from(error).to_wire_error())?;
    let summary = WorkflowSummary::from_history(&history)
        .ok_or_else(|| workflow_not_found_error(&workflow_id))?;
    let namespace = scoped.namespace().to_owned();
    let summary = encode_workflow_summary(namespace.clone(), None, &summary)?;
    let encoded_history = encode_history(request.include_history, &namespace, &history)?;

    Ok(DescribeOutcome {
        response: ProtoDescribeWorkflowResponse {
            summary: Some(summary),
            history: encoded_history,
        },
        namespace,
        workflow_id,
        history,
    })
}

#[cfg(test)]
mod tests {
    use aion_core::Event;
    use aion_proto::{
        WireError,
        convert::{decode_event, decode_workflow_summary},
    };

    use super::super::test_support::{
        NAMESPACE, append_started, assert_workflow_not_found, context, describe_request, run_id,
        workflow_id,
    };
    use super::describe;

    #[tokio::test]
    async fn describe_handler_scopes_then_reads_summary_and_optional_history()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        context.ownership.record(workflow_id(), NAMESPACE)?;
        append_started(context.store.as_ref()).await?;

        let outcome = describe(
            &context.guard,
            &context.caller,
            describe_request(true, None),
        )
        .await?;
        // The decoded history and the scoped namespace come back for the
        // transport's own projection, and they are the SAME read the summary was
        // built from — not a second one that could disagree.
        assert_eq!(outcome.namespace, NAMESPACE);
        assert_eq!(outcome.history.len(), 1);
        let response = outcome.response;

        let summary = response
            .summary
            .as_ref()
            .map(decode_workflow_summary)
            .transpose()?
            .ok_or_else(|| WireError::backend("summary missing"))?;
        assert_eq!(summary.workflow_id, workflow_id());
        assert_eq!(response.history.len(), 1);
        assert!(matches!(
            decode_event(&response.history[0])?,
            Event::WorkflowStarted { .. }
        ));
        Ok(())
    }

    #[tokio::test]
    async fn describe_handler_maps_omitted_run_missing_workflow_to_not_found()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        context.ownership.record(workflow_id(), NAMESPACE)?;

        let error = describe(
            &context.guard,
            &context.caller,
            describe_request(false, None),
        )
        .await;

        assert_workflow_not_found(error)?;
        Ok(())
    }

    #[tokio::test]
    async fn describe_handler_maps_empty_history_to_not_found()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = context().await?;
        context.ownership.record(workflow_id(), NAMESPACE)?;

        let error = describe(
            &context.guard,
            &context.caller,
            describe_request(false, Some(run_id())),
        )
        .await;

        assert_workflow_not_found(error)?;
        Ok(())
    }
}
