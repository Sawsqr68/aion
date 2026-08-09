//! The task store.
//!
//! Process-local and in-memory by construction, and the TTL says so: a task
//! records what one in-flight MCP call is doing, and an MCP call does not
//! survive the server that is running it. The DURABLE thing a task points at is
//! the workflow run, which is event-sourced in the engine and is recovered by
//! `describe_run` on the handles the task's own creation result returned. That
//! is the honest split — the task is a handle to durable state, not a second
//! copy of it.
//!
//! `tasks/get` on an unknown identifier is `-32602`, and there is no
//! `tasks/list`: a caller can only reach a task whose identifier it already
//! holds, so one caller's tasks can never be enumerated by another.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use chrono::{DateTime, TimeDelta, Utc};
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::model::{DetailedTask, Task, TaskStatus};

/// Why a task operation failed.
#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum TaskStoreError {
    /// No task with that identifier exists — never created, or expired and
    /// deleted.
    #[error("Failed to retrieve task: no task with id `{task_id}` exists")]
    Unknown {
        /// The identifier that was asked for.
        task_id: String,
    },
    /// The task existed but its TTL has elapsed.
    #[error("Failed to retrieve task: Task has expired")]
    Expired {
        /// The identifier that was asked for.
        task_id: String,
    },
    /// The store's lock was poisoned by a panicking holder.
    #[error("task store lock was poisoned")]
    LockPoisoned,
}

struct Entry {
    detail: DetailedTask,
    cancel: CancellationToken,
}

/// An in-memory task store.
#[derive(Clone, Default)]
pub struct TaskStore {
    entries: Arc<Mutex<HashMap<String, Entry>>>,
}

impl std::fmt::Debug for TaskStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("TaskStore")
    }
}

impl TaskStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a `working` task and return it together with the cancellation
    /// token its executor must observe.
    ///
    /// The task is inserted BEFORE this returns, so a `tasks/get` issued the
    /// instant the creation result reaches the client already resolves.
    ///
    /// # Errors
    ///
    /// [`TaskStoreError::LockPoisoned`] when a previous holder panicked.
    pub fn create(
        &self,
        now: DateTime<Utc>,
        ttl_ms: Option<u64>,
        poll_interval_ms: u64,
        status_message: Option<String>,
    ) -> Result<(Task, CancellationToken), TaskStoreError> {
        // A v4 UUID is 122 bits from the platform CSPRNG: not enumerable and
        // not guessable, which is what the extension requires of an identifier
        // that doubles as a bearer token.
        let task = Task {
            task_id: Uuid::new_v4().to_string(),
            status: TaskStatus::Working,
            status_message,
            created_at: now,
            last_updated_at: now,
            ttl_ms,
            poll_interval_ms,
        };
        let cancel = CancellationToken::new();
        let mut entries = self.lock()?;
        entries.insert(
            task.task_id.clone(),
            Entry {
                detail: DetailedTask::Working(task.clone()),
                cancel: cancel.clone(),
            },
        );
        Ok((task, cancel))
    }

    /// Read a task's current detail.
    ///
    /// A task whose TTL has elapsed is reported as expired and removed, rather
    /// than served stale.
    ///
    /// # Errors
    ///
    /// [`TaskStoreError::Unknown`] / [`TaskStoreError::Expired`] per above, and
    /// [`TaskStoreError::LockPoisoned`].
    pub fn get(&self, task_id: &str, now: DateTime<Utc>) -> Result<DetailedTask, TaskStoreError> {
        let mut entries = self.lock()?;
        let entry = entries
            .get(task_id)
            .ok_or_else(|| TaskStoreError::Unknown {
                task_id: task_id.to_owned(),
            })?;
        if is_expired(entry.detail.task(), now) {
            entry.cancel.cancel();
            drop(entries.remove(task_id));
            return Err(TaskStoreError::Expired {
                task_id: task_id.to_owned(),
            });
        }
        Ok(entry.detail.clone())
    }

    /// Move a task to a terminal state.
    ///
    /// Terminal states never change: a transition attempted against a task that
    /// already reached one is dropped, so a cancellation racing a completion
    /// cannot rewrite the answer a client has already read.
    ///
    /// # Errors
    ///
    /// [`TaskStoreError::LockPoisoned`].
    pub fn finish(
        &self,
        task_id: &str,
        now: DateTime<Utc>,
        outcome: TerminalOutcome,
    ) -> Result<(), TaskStoreError> {
        let mut entries = self.lock()?;
        let Some(entry) = entries.get_mut(task_id) else {
            return Ok(());
        };
        if entry.detail.task().status.is_terminal() {
            return Ok(());
        }
        let mut task = entry.detail.task().clone();
        task.last_updated_at = now;
        entry.detail = match outcome {
            TerminalOutcome::Completed { result, message } => {
                task.status = TaskStatus::Completed;
                task.status_message = message;
                DetailedTask::Completed { task, result }
            }
            TerminalOutcome::Failed { error, message } => {
                task.status = TaskStatus::Failed;
                task.status_message = message;
                DetailedTask::Failed { task, error }
            }
            TerminalOutcome::Cancelled { message } => {
                task.status = TaskStatus::Cancelled;
                task.status_message = message;
                DetailedTask::Cancelled(task)
            }
        };
        Ok(())
    }

    /// Request cooperative cancellation.
    ///
    /// Fires the executor's token and returns. The status is NOT forced here:
    /// cancellation is cooperative and eventually consistent, so the task may
    /// legitimately remain `working` after the acknowledgement and may still
    /// reach a terminal status other than `cancelled`.
    ///
    /// # Errors
    ///
    /// [`TaskStoreError::Unknown`] when no such task exists, and
    /// [`TaskStoreError::LockPoisoned`].
    pub fn request_cancel(&self, task_id: &str) -> Result<(), TaskStoreError> {
        let entries = self.lock()?;
        let entry = entries
            .get(task_id)
            .ok_or_else(|| TaskStoreError::Unknown {
                task_id: task_id.to_owned(),
            })?;
        entry.cancel.cancel();
        Ok(())
    }

    /// Whether a task with this identifier is present (expiry not considered).
    ///
    /// # Errors
    ///
    /// [`TaskStoreError::LockPoisoned`].
    pub fn contains(&self, task_id: &str) -> Result<bool, TaskStoreError> {
        Ok(self.lock()?.contains_key(task_id))
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, HashMap<String, Entry>>, TaskStoreError> {
        self.entries.lock().map_err(|error: PoisonError<_>| {
            tracing::error!(
                target: "aion_mcp::tasks",
                "task store mutex was poisoned: {error}"
            );
            TaskStoreError::LockPoisoned
        })
    }
}

/// The terminal state a task is being moved to.
#[derive(Clone, Debug)]
pub enum TerminalOutcome {
    /// The underlying request produced a result.
    Completed {
        /// Exactly what the underlying request would have returned.
        result: Value,
        /// Optional diagnostic message.
        message: Option<String>,
    },
    /// The underlying request produced a JSON-RPC error.
    Failed {
        /// The JSON-RPC error object.
        error: Value,
        /// Diagnostic message. The extension says a failed task SHOULD carry
        /// one, so every internal failure path fills it.
        message: Option<String>,
    },
    /// The task was cancelled before producing a result.
    Cancelled {
        /// Optional diagnostic message.
        message: Option<String>,
    },
}

fn is_expired(task: &Task, now: DateTime<Utc>) -> bool {
    let Some(ttl_ms) = task.ttl_ms else {
        return false;
    };
    let Ok(ttl_ms) = i64::try_from(ttl_ms) else {
        // A TTL past `i64::MAX` milliseconds is ~292 million years; treating it
        // as unlimited is the only honest reading and never expires a live task.
        return false;
    };
    let Some(delta) = TimeDelta::try_milliseconds(ttl_ms) else {
        return false;
    };
    now.signed_duration_since(task.created_at) > delta
}

#[cfg(test)]
mod tests {
    use chrono::{TimeDelta, Utc};
    use serde_json::json;

    use super::{DetailedTask, TaskStatus, TaskStore, TaskStoreError, TerminalOutcome};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn a_created_task_resolves_immediately_and_is_working() -> TestResult {
        let store = TaskStore::new();
        let now = Utc::now();
        let (task, cancel) = store.create(now, Some(60_000), 500, None)?;
        assert!(!cancel.is_cancelled());
        assert!(store.contains(&task.task_id)?);
        assert!(matches!(
            store.get(&task.task_id, now)?,
            DetailedTask::Working(_)
        ));
        Ok(())
    }

    #[test]
    fn task_ids_do_not_repeat() -> TestResult {
        let store = TaskStore::new();
        let now = Utc::now();
        let (first, first_cancel) = store.create(now, None, 500, None)?;
        let (second, second_cancel) = store.create(now, None, 500, None)?;
        // Two tasks are two independent cancellation scopes, not one shared one.
        first_cancel.cancel();
        assert!(!second_cancel.is_cancelled());
        assert_ne!(first.task_id, second.task_id);
        assert_eq!(first.task_id.len(), 36);
        Ok(())
    }

    #[test]
    fn an_unknown_task_is_reported_as_unknown() {
        let store = TaskStore::new();
        assert_eq!(
            store.get("missing", Utc::now()),
            Err(TaskStoreError::Unknown {
                task_id: "missing".to_owned()
            })
        );
        assert_eq!(
            store.request_cancel("missing"),
            Err(TaskStoreError::Unknown {
                task_id: "missing".to_owned()
            })
        );
    }

    #[test]
    fn a_terminal_state_is_never_rewritten() -> TestResult {
        let store = TaskStore::new();
        let now = Utc::now();
        let (task, cancel) = store.create(now, None, 500, None)?;
        assert!(!cancel.is_cancelled());
        store.finish(
            &task.task_id,
            now,
            TerminalOutcome::Completed {
                result: json!({ "content": [] }),
                message: None,
            },
        )?;
        store.finish(
            &task.task_id,
            now,
            TerminalOutcome::Cancelled { message: None },
        )?;
        assert!(matches!(
            store.get(&task.task_id, now)?,
            DetailedTask::Completed { .. }
        ));
        Ok(())
    }

    #[test]
    fn an_elapsed_ttl_expires_the_task_and_removes_it() -> TestResult {
        let store = TaskStore::new();
        let now = Utc::now();
        let (task, cancel) = store.create(now, Some(1_000), 500, None)?;
        let later = now
            .checked_add_signed(TimeDelta::try_seconds(5).unwrap_or_default())
            .unwrap_or(now);
        assert_eq!(
            store.get(&task.task_id, later),
            Err(TaskStoreError::Expired {
                task_id: task.task_id.clone()
            })
        );
        assert!(cancel.is_cancelled());
        assert!(!store.contains(&task.task_id)?);
        Ok(())
    }

    #[test]
    fn an_unlimited_ttl_never_expires() -> TestResult {
        let store = TaskStore::new();
        let now = Utc::now();
        let (task, cancel) = store.create(now, None, 500, None)?;
        assert!(!cancel.is_cancelled());
        let much_later = now
            .checked_add_signed(TimeDelta::try_days(3_650).unwrap_or_default())
            .unwrap_or(now);
        assert!(matches!(
            store.get(&task.task_id, much_later)?,
            DetailedTask::Working(_)
        ));
        Ok(())
    }

    #[test]
    fn cancellation_fires_the_token_without_forcing_the_status() -> TestResult {
        let store = TaskStore::new();
        let now = Utc::now();
        let (task, cancel) = store.create(now, None, 500, None)?;
        store.request_cancel(&task.task_id)?;
        assert!(cancel.is_cancelled());
        assert_eq!(
            store.get(&task.task_id, now)?.task().status,
            TaskStatus::Working
        );
        Ok(())
    }
}
