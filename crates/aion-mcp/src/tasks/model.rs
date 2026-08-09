//! The `io.modelcontextprotocol/tasks` task model.
//!
//! Pinned to `modelcontextprotocol/ext-tasks` @
//! `2c1425d9a288b9b1f489430fe1e00bb392b47e48`, `specification/draft/tasks.md`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// A task's lifecycle status.
///
/// `working` and `input_required` interconvert; the other three are terminal
/// and, once reached, never change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Executing.
    Working,
    /// Blocked on outstanding server-to-client input requests.
    InputRequired,
    /// Terminal: finished, carrying the result the underlying request would
    /// have returned.
    Completed,
    /// Terminal: cancelled.
    Cancelled,
    /// Terminal: the underlying request produced a JSON-RPC error.
    Failed,
}

impl TaskStatus {
    /// Whether this status is terminal.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed)
    }
}

/// The flat task record returned by `CreateTaskResult` and embedded in every
/// detailed variant.
#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    /// Stable identifier. Generated with cryptographic entropy: it doubles as a
    /// bearer token for the stored state behind it, so it must be neither
    /// enumerable nor guessable.
    pub task_id: String,
    /// Current status.
    pub status: TaskStatus,
    /// Optional human/model-readable status detail.
    pub status_message: Option<String>,
    /// Creation instant.
    pub created_at: DateTime<Utc>,
    /// Last status change.
    pub last_updated_at: DateTime<Utc>,
    /// Milliseconds from creation after which the server may fail and delete
    /// the task. `None` means unlimited.
    ///
    /// This is NOT the `CacheableResult.ttlMs` freshness hint — a different
    /// field with different semantics that happens to share a name.
    pub ttl_ms: Option<u64>,
    /// The interval a client should poll at.
    pub poll_interval_ms: u64,
}

impl Task {
    /// Encode the flat task fields into a result object.
    pub fn write_into(&self, result: &mut Map<String, Value>) {
        result.insert("taskId".to_owned(), json!(self.task_id));
        result.insert("status".to_owned(), json!(self.status));
        if let Some(message) = &self.status_message {
            result.insert("statusMessage".to_owned(), json!(message));
        }
        result.insert("createdAt".to_owned(), json!(self.created_at.to_rfc3339()));
        result.insert(
            "lastUpdatedAt".to_owned(),
            json!(self.last_updated_at.to_rfc3339()),
        );
        // `ttlMs` is REQUIRED and nullable: `null` is the wire spelling of
        // "unlimited", so the key is always written.
        result.insert("ttlMs".to_owned(), json!(self.ttl_ms));
        result.insert("pollIntervalMs".to_owned(), json!(self.poll_interval_ms));
    }
}

/// A task plus the payload its status requires.
///
/// The variants enforce the per-status obligations structurally: a completed
/// task cannot be built without its result, and a failed task cannot be built
/// without its JSON-RPC error.
#[derive(Clone, Debug, PartialEq)]
pub enum DetailedTask {
    /// Executing; no extra payload.
    Working(Task),
    /// Terminal success. `result` is EXACTLY what the underlying request would
    /// have returned — including a `CallToolResult` carrying `isError: true`,
    /// which is a completed task and never a failed one.
    Completed {
        /// The task record.
        task: Task,
        /// The embedded result object.
        result: Value,
    },
    /// Terminal failure. `error` is the JSON-RPC error the underlying request
    /// would have returned. A tool result with `isError: true` MUST NOT be
    /// represented here.
    Failed {
        /// The task record.
        task: Task,
        /// The embedded JSON-RPC error object.
        error: Value,
    },
    /// Terminal cancellation.
    Cancelled(Task),
}

impl DetailedTask {
    /// The flat task record.
    #[must_use]
    pub const fn task(&self) -> &Task {
        match self {
            Self::Working(task)
            | Self::Completed { task, .. }
            | Self::Failed { task, .. }
            | Self::Cancelled(task) => task,
        }
    }

    /// Encode the detailed task into a result object.
    pub fn write_into(&self, result: &mut Map<String, Value>) {
        self.task().write_into(result);
        match self {
            Self::Completed {
                result: embedded, ..
            } => {
                result.insert("result".to_owned(), embedded.clone());
            }
            Self::Failed { error, .. } => {
                result.insert("error".to_owned(), error.clone());
            }
            Self::Working(_) | Self::Cancelled(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use serde_json::{Map, json};

    use super::{DetailedTask, Task, TaskStatus};

    fn task(status: TaskStatus, ttl_ms: Option<u64>) -> Task {
        Task {
            task_id: "t-1".to_owned(),
            status,
            status_message: None,
            created_at: Utc
                .timestamp_opt(1_700_000_000, 0)
                .single()
                .unwrap_or_default(),
            last_updated_at: Utc
                .timestamp_opt(1_700_000_100, 0)
                .single()
                .unwrap_or_default(),
            ttl_ms,
            poll_interval_ms: 500,
        }
    }

    #[test]
    fn terminal_statuses_are_exactly_the_three() {
        assert!(TaskStatus::Completed.is_terminal());
        assert!(TaskStatus::Failed.is_terminal());
        assert!(TaskStatus::Cancelled.is_terminal());
        assert!(!TaskStatus::Working.is_terminal());
        assert!(!TaskStatus::InputRequired.is_terminal());
    }

    #[test]
    fn an_unlimited_ttl_is_written_as_an_explicit_null() {
        let mut encoded = Map::new();
        task(TaskStatus::Working, None).write_into(&mut encoded);
        assert_eq!(encoded.get("ttlMs"), Some(&json!(null)));
        assert_eq!(encoded["status"], "working");
    }

    #[test]
    fn a_completed_task_embeds_its_result_and_a_failed_one_embeds_its_error() {
        let mut completed = Map::new();
        DetailedTask::Completed {
            task: task(TaskStatus::Completed, Some(60_000)),
            result: json!({ "content": [], "isError": true }),
        }
        .write_into(&mut completed);
        assert_eq!(completed["status"], "completed");
        assert_eq!(completed["result"]["isError"], true);
        assert!(completed.get("error").is_none());

        let mut failed = Map::new();
        DetailedTask::Failed {
            task: task(TaskStatus::Failed, None),
            error: json!({ "code": -32603, "message": "boom" }),
        }
        .write_into(&mut failed);
        assert_eq!(failed["error"]["code"], -32603);
        assert!(failed.get("result").is_none());
    }
}
