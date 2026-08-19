//! Resolving a task handle into the task the extension describes.
//!
//! There is no task store. A task here is a **projection of the durable work
//! its handle names**, read fresh on every request — the same discipline the
//! host applies one layer down, where a workflow's status is projected from its
//! event history rather than kept as a mutable field beside it.
//!
//! Two consequences follow, and both are the point:
//!
//! * **Nothing expires.** A projection has no lifetime of its own, so there is
//!   no TTL to configure and no default to guess. `ttlMs` is always `null`,
//!   which is already the extension's spelling of "unlimited".
//! * **Nothing is held.** An abandoned task costs no thread, no memory, and no
//!   entry, because there was never anything to abandon.
//!
//! The handle's format is the host's business; this module only carries it.

use chrono::{DateTime, Utc};
use serde_json::{Value, json};

use crate::protocol::error::ProtocolError;
use crate::server::call_result::call_tool_success_result;
use crate::tools::service::{ToolCatalog, ToolOutcome};

use super::model::{DetailedTask, Task, TaskStatus};

/// What the durable work behind a handle is doing right now.
///
/// `Cancelled` has no representative here, deliberately: this crate's task is
/// the work's own truth, and a host that declines to stop the work on
/// `tasks/cancel` — which the extension explicitly permits, cancellation being
/// cooperative — never has a task that reaches `cancelled`.
#[derive(Clone, Debug, PartialEq)]
pub enum TaskState {
    /// Still running.
    Working,
    /// Finished. `outcome` is what the tool would have returned had the call
    /// been made inline, and it is rendered here through the SAME function the
    /// inline path uses, so the two forms cannot drift apart.
    ///
    /// A finished-and-failed piece of work is still `Finished`: the tool
    /// succeeded at its job of reporting how the work ended, and the ending
    /// rides in the structured content. `failed` in the extension's sense is
    /// reserved for a JSON-RPC error, which is why the only way to reach it
    /// from here is a server that violated its own published output schema.
    Finished {
        /// The tool whose result this is, used to find the output schema the
        /// rendered result must satisfy.
        tool_name: String,
        /// The tool's result.
        outcome: ToolOutcome,
    },
}

/// One resolution of a task handle: everything the extension's task record
/// needs, read from the durable work itself.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskProjection {
    /// What the work is doing.
    pub state: TaskState,
    /// Human/model-readable detail, carried on every read rather than stored
    /// once at creation — a projection has one code path, so the message a
    /// client sees on `tasks/get` is the message it saw on creation.
    pub status_message: Option<String>,
    /// When the work started.
    pub created_at: DateTime<Utc>,
    /// When its status last changed.
    pub last_updated_at: DateTime<Utc>,
}

impl TaskProjection {
    /// Render this projection as the extension's detailed task.
    ///
    /// `ttl_ms` is always `None`: there is no stored state to expire, so there
    /// is no lifetime to advertise and no number to have chosen.
    #[must_use]
    pub fn into_detailed(
        self,
        task_id: &str,
        poll_interval_ms: u64,
        catalog: &ToolCatalog,
    ) -> DetailedTask {
        let task = |status: TaskStatus| Task {
            task_id: task_id.to_owned(),
            status,
            status_message: self.status_message.clone(),
            created_at: self.created_at,
            last_updated_at: self.last_updated_at,
            ttl_ms: None,
            poll_interval_ms,
        };
        match self.state {
            TaskState::Working => DetailedTask::Working(task(TaskStatus::Working)),
            TaskState::Finished { tool_name, outcome } => {
                match call_tool_success_result(catalog.find(&tool_name), &outcome) {
                    Ok(result) => DetailedTask::Completed {
                        task: task(TaskStatus::Completed),
                        result,
                    },
                    // The server published an output schema and then broke it.
                    // That is a JSON-RPC-error-shaped failure of the underlying
                    // request, which is exactly what `failed` means — and the
                    // error object is built from the same `ProtocolError` an
                    // inline call would have answered with.
                    Err(detail) => DetailedTask::Failed {
                        task: task(TaskStatus::Failed),
                        error: error_object(&ProtocolError::Internal { detail }),
                    },
                }
            }
        }
    }
}

/// Render a protocol error as the JSON-RPC error object it would be sent as.
fn error_object(error: &ProtocolError) -> Value {
    let mut object = json!({ "code": error.code(), "message": error.message() });
    if let (Some(data), Value::Object(fields)) = (error.data(), &mut object) {
        fields.insert("data".to_owned(), data);
    }
    object
}

/// Why a task handle could not be resolved.
#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum TaskResolveError {
    /// The string is not a handle this server issues.
    #[error("{detail}")]
    Malformed {
        /// What was wrong with it, phrased for the caller that sent it.
        detail: String,
    },
    /// The handle is well-formed but names no work — or names work this caller
    /// is not entitled to see.
    ///
    /// The two are ONE answer on purpose. A handle is derived from identifiers
    /// the caller may already hold, so it is constructible rather than secret;
    /// distinguishing "no such task" from "not yours" would turn that
    /// constructibility into a way to probe for the existence of another
    /// caller's work.
    #[error("task {task_id} is not a task this caller can read")]
    Unknown {
        /// The handle that was presented.
        task_id: String,
    },
    /// The durable work could not be read at all.
    #[error("the task could not be resolved: {detail}")]
    Backend {
        /// The underlying reason.
        detail: String,
    },
}

impl TaskResolveError {
    /// The protocol error this failure is reported as.
    ///
    /// A malformed or unresolvable handle is `-32602`, which is what the
    /// extension prescribes for a task id the server cannot honour. A backend
    /// failure is `-32603`: the request was fine and the server could not
    /// answer it, and telling the caller to fix its parameters would be a lie.
    #[must_use]
    pub fn to_protocol_error(&self) -> ProtocolError {
        match self {
            Self::Malformed { .. } | Self::Unknown { .. } => ProtocolError::InvalidParams {
                detail: self.to_string(),
            },
            Self::Backend { detail } => ProtocolError::Internal {
                detail: detail.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    use super::{TaskProjection, TaskResolveError, TaskState};
    use crate::protocol::error::error_codes;
    use crate::tools::descriptor::{Tool, ToolAnnotations, WorldScope};
    use crate::tools::service::{CatalogError, ToolCatalog, ToolOutcome};

    fn catalog() -> Result<ToolCatalog, CatalogError> {
        ToolCatalog::new(vec![Tool {
            name: "start_run".to_owned(),
            title: None,
            description: None,
            input_schema: json!({ "type": "object" }),
            output_schema: Some(json!({
                "type": "object",
                "properties": { "status": { "type": "string" } },
                "required": ["status"],
            })),
            annotations: ToolAnnotations::read_only(WorldScope::Closed),
        }])
    }

    fn projection(state: TaskState) -> TaskProjection {
        TaskProjection {
            state,
            status_message: Some("still going".to_owned()),
            created_at: Utc
                .timestamp_opt(1_700_000_000, 0)
                .single()
                .unwrap_or_default(),
            last_updated_at: Utc
                .timestamp_opt(1_700_000_100, 0)
                .single()
                .unwrap_or_default(),
        }
    }

    #[test]
    fn a_projection_never_advertises_a_lifetime() -> Result<(), CatalogError> {
        let catalog = catalog()?;
        let detailed = projection(TaskState::Working).into_detailed("t-1", 1_000, &catalog);
        assert_eq!(detailed.task().ttl_ms, None);
        assert_eq!(detailed.task().poll_interval_ms, 1_000);
        assert_eq!(
            detailed.task().status_message.as_deref(),
            Some("still going")
        );
        Ok(())
    }

    #[test]
    fn finished_work_completes_the_task_and_carries_the_inline_result() -> Result<(), CatalogError>
    {
        let catalog = catalog()?;
        let detailed = projection(TaskState::Finished {
            tool_name: "start_run".to_owned(),
            outcome: ToolOutcome {
                structured: json!({ "status": "Failed" }),
                summary: "it failed".to_owned(),
            },
        })
        .into_detailed("t-1", 1_000, &catalog);
        // A run that ended FAILED is a COMPLETED task: the tool succeeded at
        // reporting how the run ended.
        let mut encoded = serde_json::Map::new();
        detailed.write_into(&mut encoded);
        assert_eq!(encoded["status"], "completed");
        assert_eq!(encoded["result"]["isError"], false);
        assert_eq!(encoded["result"]["structuredContent"]["status"], "Failed");
        assert_eq!(encoded["ttlMs"], json!(null));
        Ok(())
    }

    /// The only route to `failed` is the server breaking its own published
    /// output schema, and it must be reported as a JSON-RPC error rather than
    /// shipped to the model as a result.
    #[test]
    fn a_result_violating_the_published_schema_fails_the_task() -> Result<(), CatalogError> {
        let catalog = catalog()?;
        let detailed = projection(TaskState::Finished {
            tool_name: "start_run".to_owned(),
            outcome: ToolOutcome {
                structured: json!({ "status": 7 }),
                summary: "wrong".to_owned(),
            },
        })
        .into_detailed("t-1", 1_000, &catalog);
        let mut encoded = serde_json::Map::new();
        detailed.write_into(&mut encoded);
        assert_eq!(encoded["status"], "failed");
        assert_eq!(encoded["error"]["code"], error_codes::INTERNAL_ERROR);
        assert!(encoded.get("result").is_none());
        Ok(())
    }

    #[test]
    fn an_unreadable_handle_is_invalid_params_and_a_backend_failure_is_internal() {
        let unknown = TaskResolveError::Unknown {
            task_id: "run.v1:x".to_owned(),
        };
        assert_eq!(
            unknown.to_protocol_error().code(),
            error_codes::INVALID_PARAMS
        );
        // "no such task" and "not yours" are ONE answer, so the message must
        // not name which of the two it was.
        let message = unknown.to_string();
        assert!(!message.contains("does not exist"), "{message}");

        assert_eq!(
            TaskResolveError::Backend {
                detail: "the store is unreachable".to_owned(),
            }
            .to_protocol_error()
            .code(),
            error_codes::INTERNAL_ERROR
        );
    }
}
