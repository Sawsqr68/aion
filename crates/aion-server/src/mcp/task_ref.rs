//! The handle an MCP task is named by, and the projection behind it.
//!
//! An MCP task on this server is **a run, seen through the extension's task
//! model**. Nothing is stored to make that true: the handle names a run, and
//! every read of the task is a fresh namespace-gated read of that run's
//! history, exactly as `describe_run` does. That is load-bearing invariant #4
//! — status is a projection, never a stored mutable field — applied one layer
//! up, where the protocol layer had been keeping a second, divergent copy.
//!
//! Two properties follow directly, and both are why the design was chosen:
//!
//! * **A task survives a restart**, because the run does. No persistence code
//!   exists here; the durability used is the engine's own.
//! * **A task that nobody reads costs nothing** — no thread, no memory, no
//!   entry — because there is nothing to abandon.

use aion_core::{RunId, WorkflowId, WorkflowSummary};
use aion_mcp::tasks::resolve::{TaskProjection, TaskResolveError, TaskState};
use aion_mcp::tools::service::{ToolCall, ToolOutcome};
use serde_json::Value;
use uuid::Uuid;

use crate::{CallerIdentity, ServerState};

use super::tools::run_view::RunView;
use super::tools::start::{START_RUN, awaited_outcome, awaits_completion};

/// The prefix every handle this server issues begins with.
///
/// Versioned because the format is a wire contract with clients that hold
/// handles across deploys: a future format is a new prefix, and an old handle
/// keeps resolving rather than being silently misread as the new shape.
const HANDLE_PREFIX: &str = "run.v1";

/// The message a task carries on every read while its run is still going.
///
/// It lives here rather than being recorded once at creation because a
/// projection has no place to record anything — and that is the better answer:
/// a client reading the task an hour later is told the same thing the creator
/// was, and cannot be told something stale.
const RUNNING_MESSAGE: &str = "the run is still going; cancelling this task does not stop it — \
                               use the `cancel` tool to stop the run";

/// The message a task carries once its run has ended.
const ENDED_MESSAGE: &str = "the run has ended; its terminal status is in the result";

/// A parsed task handle: the run it names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TaskRef {
    /// The namespace the run lives in.
    pub(crate) namespace: String,
    /// The workflow the run belongs to.
    pub(crate) workflow_id: WorkflowId,
    /// The run itself.
    pub(crate) run_id: RunId,
}

impl TaskRef {
    /// Render this reference as the handle a client is given.
    ///
    /// Plain text rather than an opaque blob, deliberately. The identifiers in
    /// it were just handed to the caller by `start_run`, so encoding buys no
    /// secrecy — and a handle an operator can read in a log is worth more than
    /// one that only looks secure. Opacity is not the security property here;
    /// the namespace guard on every read is.
    pub(crate) fn to_handle(&self) -> String {
        format!(
            "{HANDLE_PREFIX}:{}:{}:{}",
            self.workflow_id, self.run_id, self.namespace
        )
    }

    /// Parse a handle a client presented.
    ///
    /// The namespace takes the whole remainder, so a namespace containing a
    /// colon round-trips rather than being truncated into a different one.
    ///
    /// # Errors
    ///
    /// [`TaskResolveError::Malformed`] when the string is not a handle this
    /// server issues.
    pub(crate) fn parse(handle: &str) -> Result<Self, TaskResolveError> {
        let malformed = |detail: String| TaskResolveError::Malformed { detail };
        let mut parts = handle.splitn(4, ':');
        let (Some(prefix), Some(workflow), Some(run), Some(namespace)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(malformed(format!(
                "`{handle}` is not a task id this server issued; a task id comes from an awaited \
                 start_run and is never constructed"
            )));
        };
        if prefix != HANDLE_PREFIX {
            return Err(malformed(format!(
                "`{handle}` is not a task id this server issued (unknown handle kind `{prefix}`)"
            )));
        }
        if namespace.is_empty() {
            return Err(malformed(format!("`{handle}` names no namespace")));
        }
        Ok(Self {
            workflow_id: Uuid::parse_str(workflow)
                .map(WorkflowId::new)
                .map_err(|error| {
                    malformed(format!("`{handle}` does not name a workflow: {error}"))
                })?,
            run_id: Uuid::parse_str(run)
                .map(RunId::new)
                .map_err(|error| malformed(format!("`{handle}` does not name a run: {error}")))?,
            namespace: namespace.to_owned(),
        })
    }
}

/// Mint the handle for a task-shaped call that has just been performed.
///
/// Returns `None` for any call this server does not answer as a task, and for
/// an outcome that does not carry the handles a run is named by. Both are
/// server defects if they ever fire — the same service decided the call was
/// task-shaped — and the caller reports them as such rather than handing back a
/// task nothing can resolve.
pub(crate) fn task_handle(call: &ToolCall, outcome: &ToolOutcome) -> Option<String> {
    if !awaits_completion(call) {
        return None;
    }
    let field = |key: &str| outcome.structured.get(key).and_then(Value::as_str);
    Some(
        TaskRef {
            namespace: field("namespace")?.to_owned(),
            workflow_id: Uuid::parse_str(field("workflow_id")?)
                .map(WorkflowId::new)
                .ok()?,
            run_id: Uuid::parse_str(field("run_id")?).map(RunId::new).ok()?,
        }
        .to_handle(),
    )
}

/// Resolve a handle by reading the run it names, under the caller's own
/// authorization.
///
/// The read is [`RunView::read_parts`], which routes through
/// `handlers::describe` and its namespace guard — the SAME guard `describe_run`
/// passes. There is no second authorization path here to drift from that one,
/// and no way to reach a run through a task that could not be reached through
/// `describe_run`.
///
/// A run refused by that guard, a workflow that does not exist, and a run id
/// that is not one of that workflow's runs all resolve to the same
/// [`TaskResolveError::Unknown`]. Collapsing them is deliberate: a handle is
/// constructible from identifiers a caller may hold, so a distinguishable
/// refusal would be a way to probe for another tenant's work.
///
/// # Errors
///
/// [`TaskResolveError`] when the handle is malformed, names work this caller
/// cannot read, or names a run whose history cannot be projected.
pub(crate) async fn resolve_task(
    state: &ServerState,
    caller: &CallerIdentity,
    task_id: &str,
) -> Result<TaskProjection, TaskResolveError> {
    let reference = TaskRef::parse(task_id)?;
    let view = RunView::read_parts(
        state,
        caller,
        &reference.namespace,
        &reference.workflow_id,
        Some(&reference.run_id),
    )
    .await
    .map_err(|_refusal| TaskResolveError::Unknown {
        task_id: task_id.to_owned(),
    })?;

    // Project THIS run, not the chain. The task names one run, and a run that
    // continued-as-new has genuinely ended even though its successor carries on
    // — reporting the chain's status here would leave a task `working` forever
    // for a run that finished.
    let summary = WorkflowSummary::from_history(view.run_segment()).ok_or_else(|| {
        TaskResolveError::Backend {
            detail: format!(
                "run {} of workflow {} has no recorded start event",
                reference.run_id, reference.workflow_id
            ),
        }
    })?;

    let ended = summary.status.is_terminal();
    Ok(TaskProjection {
        state: if ended {
            TaskState::Finished {
                tool_name: START_RUN.to_owned(),
                outcome: awaited_outcome(&reference.namespace, &reference.run_id, &summary),
            }
        } else {
            TaskState::Working
        },
        status_message: Some(
            if ended {
                ENDED_MESSAGE
            } else {
                RUNNING_MESSAGE
            }
            .to_owned(),
        ),
        // The status changes exactly twice — when the run starts, and when it
        // ends — so the recorded timestamps ARE the extension's "last status
        // change". No extra history read is needed to find it.
        created_at: summary.started_at,
        last_updated_at: summary.ended_at.unwrap_or(summary.started_at),
    })
}

#[cfg(test)]
mod tests {
    use aion_core::{RunId, WorkflowId};
    use aion_mcp::tools::service::{ToolCall, ToolOutcome};
    use serde_json::{Map, Value, json};
    use uuid::Uuid;

    use super::{TaskRef, task_handle};

    fn outcome(structured: Value) -> ToolOutcome {
        ToolOutcome {
            structured,
            summary: "started".to_owned(),
        }
    }

    fn call(arguments: &Value) -> Result<ToolCall, serde_json::Error> {
        Ok(ToolCall {
            name: "start_run".to_owned(),
            arguments: serde_json::from_value::<Map<String, Value>>(arguments.clone())?,
        })
    }

    #[test]
    fn a_handle_round_trips_through_a_namespace_containing_colons()
    -> Result<(), Box<dyn std::error::Error>> {
        let reference = TaskRef {
            namespace: "tenant:eu:prod".to_owned(),
            workflow_id: WorkflowId::new(Uuid::parse_str("5f1d2b90-1d3a-4a1e-9c2b-000000000001")?),
            run_id: RunId::new(Uuid::parse_str("5f1d2b90-1d3a-4a1e-9c2b-000000000002")?),
        };
        let handle = reference.to_handle();
        assert_eq!(TaskRef::parse(&handle)?, reference);
        Ok(())
    }

    #[test]
    fn a_string_this_server_never_issued_is_malformed_not_unknown() {
        for bogus in [
            "no-such-task",
            "run.v2:a:b:c",
            "run.v1:not-a-uuid:also-not:ns",
            "run.v1:5f1d2b90-1d3a-4a1e-9c2b-000000000001:5f1d2b90-1d3a-4a1e-9c2b-000000000002:",
            "",
        ] {
            let error = TaskRef::parse(bogus).err();
            assert!(
                matches!(
                    error,
                    Some(aion_mcp::tasks::resolve::TaskResolveError::Malformed { .. })
                ),
                "{bogus} must be refused as malformed, got {error:?}"
            );
        }
    }

    /// A plain start is not task-shaped, so it must never mint a handle — a
    /// task nobody asked for is a task nobody will ever read.
    #[test]
    fn only_an_awaited_start_mints_a_handle() -> Result<(), Box<dyn std::error::Error>> {
        let structured = json!({
            "namespace": "ops",
            "workflow_id": "5f1d2b90-1d3a-4a1e-9c2b-000000000001",
            "run_id": "5f1d2b90-1d3a-4a1e-9c2b-000000000002",
        });
        assert_eq!(
            task_handle(&call(&json!({}))?, &outcome(structured.clone())),
            None
        );
        let handle = task_handle(
            &call(&json!({ "await_completion": true }))?,
            &outcome(structured),
        )
        .ok_or("an awaited start must mint a handle")?;
        let reference = TaskRef::parse(&handle)?;
        assert_eq!(reference.namespace, "ops");
        Ok(())
    }

    /// An outcome missing the handles cannot name a run, and saying so is the
    /// difference between an internal error and a task that can never resolve.
    #[test]
    fn an_outcome_without_run_handles_mints_nothing() -> Result<(), serde_json::Error> {
        let awaited = call(&json!({ "await_completion": true }))?;
        assert_eq!(
            task_handle(&awaited, &outcome(json!({ "namespace": "ops" }))),
            None
        );
        Ok(())
    }
}
