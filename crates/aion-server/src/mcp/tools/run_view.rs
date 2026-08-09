//! The shared run read.
//!
//! Every tool that needs to know something about a run gets it from HERE, from
//! ONE namespace-gated history read. Two reads would be two answers that could
//! disagree — and one of them would be the one the agent acted on.

use aion_core::{Event, RunId, WorkflowId, WorkflowSummary};
use aion_mcp::tools::service::{ToolCall, ToolFailure};
use aion_proto::{ProtoDescribeWorkflowRequest, ProtoWorkflowId, WireError};
use serde_json::json;

use crate::mcp::args::{optional_run_id, required_str, required_workflow_id};
use crate::mcp::run_scope::RunScope;
use crate::{CallerIdentity, ServerState, api::handlers};

use super::errors::tool_failure;

/// One namespace-gated read of a workflow, projected for the tools.
pub(crate) struct RunView {
    /// The namespace the caller was scoped to.
    pub(crate) namespace: String,
    /// The workflow that was read.
    pub(crate) workflow_id: WorkflowId,
    /// The run this view is about: the caller's, or the latest of the chain
    /// when the caller named none.
    pub(crate) run_id: RunId,
    /// The workflow's complete history, across every run of the chain.
    pub(crate) history: Vec<Event>,
    /// The status projection over that history.
    pub(crate) summary: WorkflowSummary,
    /// Which run of the chain dispatched which attempt.
    pub(crate) scope: RunScope,
}

impl RunView {
    /// Read a workflow under the caller's authorization.
    ///
    /// The `run_id` argument is resolved but NOT trusted: a run that is not one
    /// of this workflow's own runs is refused here rather than silently read as
    /// though it were the latest. Naming another workflow's run and being
    /// served this workflow's history is exactly the confusion the required
    /// run handle exists to prevent.
    ///
    /// # Errors
    ///
    /// [`ToolFailure`] when the arguments are malformed, the caller does not
    /// hold the namespace, the workflow does not exist, or the named run is not
    /// this workflow's.
    pub(crate) async fn read(
        state: &ServerState,
        caller: &CallerIdentity,
        call: &ToolCall,
    ) -> Result<Self, ToolFailure> {
        let namespace = required_str(call, "namespace")?;
        let workflow_id = required_workflow_id(call, "workflow_id")?;
        let requested_run = optional_run_id(call, "run_id")?;
        Self::read_parts(
            state,
            caller,
            &namespace,
            &workflow_id,
            requested_run.as_ref(),
        )
        .await
    }

    /// The same read from already-extracted parts.
    ///
    /// # Errors
    ///
    /// As [`RunView::read`].
    pub(crate) async fn read_parts(
        state: &ServerState,
        caller: &CallerIdentity,
        namespace: &str,
        workflow_id: &WorkflowId,
        requested_run: Option<&RunId>,
    ) -> Result<Self, ToolFailure> {
        let request = ProtoDescribeWorkflowRequest {
            namespace: namespace.to_owned(),
            workflow_id: Some(ProtoWorkflowId {
                uuid: workflow_id.to_string(),
            }),
            run_id: requested_run.map(|run| run.clone().into()),
            include_history: false,
        };
        let outcome = handlers::describe(state.namespace_guard(), caller, request)
            .await
            .map_err(|error| tool_failure(&error))?;
        let summary = WorkflowSummary::from_history(&outcome.history).ok_or_else(|| {
            tool_failure(&WireError::not_found(format!(
                "workflow {workflow_id} has no recorded start event"
            )))
        })?;
        let scope = RunScope::from_history(&outcome.history);
        let run_id = match requested_run {
            Some(run) => {
                if !scope.knows_run(run) {
                    return Err(ToolFailure::new(
                        format!(
                            "run {run} is not a run of workflow {workflow_id}. Call describe_run \
                             without a run_id to get this workflow's current run, and never \
                             construct a run id."
                        ),
                        json!({
                            "code": "unknown_run",
                            "workflow_id": workflow_id.to_string(),
                            "run_id": run.to_string(),
                        }),
                    ));
                }
                run.clone()
            }
            None => latest_run(&outcome.history).ok_or_else(|| {
                tool_failure(&WireError::not_found(format!(
                    "workflow {workflow_id} has no recorded run"
                )))
            })?,
        };
        Ok(Self {
            namespace: outcome.namespace,
            workflow_id: outcome.workflow_id,
            run_id,
            history: outcome.history,
            summary,
            scope,
        })
    }

    /// The head sequence of the workflow's history: the seq of its last event.
    pub(crate) fn history_head_seq(&self) -> u64 {
        self.history.last().map_or(0, Event::seq)
    }

    /// The events belonging to this view's run alone.
    pub(crate) fn run_segment(&self) -> &[Event] {
        aion_core::run_segment(&self.history, &self.run_id)
    }
}

/// The last run opened in a history — the newest generation of the chain.
fn latest_run(history: &[Event]) -> Option<RunId> {
    history.iter().rev().find_map(|event| match event {
        Event::WorkflowStarted { run_id, .. } => Some(run_id.clone()),
        _ => None,
    })
}
