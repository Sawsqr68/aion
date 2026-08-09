//! Task execution.
//!
//! A task-shaped `tools/call` runs on a detached tokio task under the SAME
//! caller identity that was authorized when the task was created — never a
//! re-derived one — and races the tool future against the task's cancellation
//! token. If the token wins, the tool future is dropped and the task settles as
//! `cancelled`.
//!
//! Dropping the tool future stops THIS server from waiting; it does not cancel
//! the workflow the tool started. That is deliberate and is what "cooperative"
//! means here: a durable run is not something an MCP client's disinterest is
//! allowed to kill. A caller that wants the run stopped calls the `cancel`
//! tool, which is a different, explicitly destructive verb.

use std::sync::Arc;

use serde_json::Value;

use crate::server::call_result::{call_tool_failure_result, call_tool_success_result};
use crate::tools::service::{ToolCall, ToolService};

use super::store::{TaskStore, TerminalOutcome};

/// Drive one task-shaped call to a terminal state.
///
/// Never panics and never leaves a task `working`: every arm — success,
/// tool-level failure, output-schema violation, cancellation, and TTL
/// expiry — writes a terminal outcome before returning.
///
/// `ttl` is the same lifetime the created task advertises. Racing it here is
/// what makes the advertised TTL true even when nobody ever polls: without it,
/// a task whose client walked away would keep its executor alive forever, and
/// the expiry would only be noticed by a `tasks/get` that never comes. `None`
/// is an unlimited lifetime and adds no deadline.
pub async fn drive<S>(
    service: Arc<S>,
    store: TaskStore,
    task_id: String,
    caller: S::Caller,
    call: ToolCall,
    cancel: tokio_util::sync::CancellationToken,
    ttl: Option<std::time::Duration>,
) where
    S: ToolService,
{
    let expiry = async {
        match ttl {
            Some(ttl) => tokio::time::sleep(ttl).await,
            None => std::future::pending().await,
        }
    };
    let outcome = tokio::select! {
        biased;
        () = cancel.cancelled() => TerminalOutcome::Cancelled {
            message: Some(
                "the task was cancelled; any workflow it started is still running and \
                 is reachable through the handles the task creation returned"
                    .to_owned(),
            ),
        },
        result = service.call(&caller, call.clone()) => terminal_from(service.as_ref(), &call, result),
        () = expiry => TerminalOutcome::Failed {
            error: internal_error_object(
                "the task exceeded its advertised lifetime before the work completed",
            ),
            message: Some(
                "the task exceeded its advertised ttlMs; any workflow it started is still \
                 running and is reachable through the handles the task creation returned"
                    .to_owned(),
            ),
        },
    };
    if let Err(error) = store.finish(&task_id, chrono::Utc::now(), outcome) {
        tracing::error!(
            target: "aion_mcp::tasks",
            task_id = %task_id,
            "task completed but its terminal state could not be recorded: {error}"
        );
    }
}

/// Map a tool's own outcome onto a terminal task state.
///
/// A tool-level failure is a **completed** task carrying a `CallToolResult`
/// with `isError: true` — never a `failed` task. `failed` is reserved for a
/// JSON-RPC error, and a tool that ran and reported a problem did not produce
/// one.
fn terminal_from<S>(
    service: &S,
    call: &ToolCall,
    result: Result<crate::tools::service::ToolOutcome, crate::tools::service::ToolFailure>,
) -> TerminalOutcome
where
    S: ToolService,
{
    match result {
        Ok(outcome) => {
            let tool = service.catalog().find(&call.name);
            match call_tool_success_result(tool, &outcome) {
                Ok(result) => TerminalOutcome::Completed {
                    result,
                    message: None,
                },
                // The tool produced structured content that violates the
                // schema the server itself published. That is a server defect,
                // not a tool-level failure the model could correct, so it is a
                // JSON-RPC internal error and the task is `failed`.
                Err(detail) => TerminalOutcome::Failed {
                    error: internal_error_object(&detail),
                    message: Some(detail),
                },
            }
        }
        Err(failure) => TerminalOutcome::Completed {
            result: call_tool_failure_result(&failure),
            message: Some(failure.message.clone()),
        },
    }
}

fn internal_error_object(detail: &str) -> Value {
    serde_json::json!({
        "code": crate::protocol::error::error_codes::INTERNAL_ERROR,
        "message": detail,
    })
}
