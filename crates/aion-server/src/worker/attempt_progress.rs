//! Reading the volatile worker progress note of one attempt — honestly.
//!
//! A worker heartbeat may carry an opaque progress payload ("what I am doing
//! right now"). The server holds the most recent one per in-flight attempt in
//! [`HeartbeatTracker`], in memory, and writes it nowhere. That makes the read
//! path a measurement problem rather than a lookup: the absence of a note has
//! two completely different causes, and collapsing them would tell an operator
//! that a busy worker has gone silent every time the server was restarted.
//!
//! So the read returns [`AttemptProgress`], which never says "no note" without
//! saying which kind of no:
//!
//! - [`AttemptProgress::Reported`] — the tracker holds a note for this attempt.
//! - [`AttemptProgress::NoneSent`] — the tracker holds the ATTEMPT and no note
//!   on it. This is a measurement: the worker really has reported nothing.
//! - [`AttemptProgress::Untracked`] — the tracker does not hold the attempt, so
//!   there is nothing to measure. A restart is the usual cause (the notes were
//!   in a process that no longer exists), and the other causes — the owner was
//!   swept off the tracker, or nothing has leased the dispatch — are equally
//!   unmeasurable from here. Every one of them is this variant; none of them is
//!   ever reported as silence.

use aion_core::{ActivityId, Payload, WorkflowId};
use chrono::{DateTime, Utc};

use super::heartbeat::{HeartbeatTracker, TaskLiveness};
use crate::error::ServerError;

/// What this server process can say about one attempt's progress note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttemptProgress {
    /// The worker serving this attempt reported this note, and this process
    /// received it at `reported_at`.
    Reported {
        /// The worker's opaque progress payload, exactly as it was sent.
        payload: Payload,
        /// Wall-clock instant this process received the reporting heartbeat.
        reported_at: DateTime<Utc>,
    },
    /// This process is tracking the attempt and no note has been reported on it.
    NoneSent,
    /// This process is not tracking the attempt, so it cannot say whether a
    /// note was reported.
    Untracked,
}

/// The progress note this process holds for `(workflow, activity, attempt)`.
///
/// When a within-attempt failover has both a dying owner and its adopter
/// tracked, the entry with the most recent heartbeat wins: it is the one still
/// being fed, and taking the other would report a note the live worker has
/// already superseded. `last_heartbeat_at` is a monotonic [`std::time::Instant`]
/// minted by this process, so comparing entries is meaningful even though the
/// value itself would be meaningless to report.
///
/// # Errors
///
/// Returns [`ServerError::LockPoisoned`] when the tracker state cannot be read.
/// An unreadable tracker is reported, never rendered as "no note" — that would
/// be the exact confusion this module exists to prevent.
pub fn attempt_progress(
    tracker: &HeartbeatTracker,
    workflow_id: &WorkflowId,
    activity_id: &ActivityId,
    attempt: u32,
) -> Result<AttemptProgress, ServerError> {
    let entries = tracker.attempt_entries(workflow_id, activity_id, attempt)?;
    let Some(freshest) = freshest(entries) else {
        return Ok(AttemptProgress::Untracked);
    };
    match (freshest.last_progress, freshest.last_progress_at) {
        (Some(payload), Some(reported_at)) => Ok(AttemptProgress::Reported {
            payload,
            reported_at,
        }),
        // A note is stamped with its receive instant in the same assignment, so
        // a note without a stamp is unreachable. It is handled as UNTRACKED
        // rather than as `NoneSent` because a note that exists without a receive
        // instant is a note this process cannot describe — reporting it as
        // "nothing was sent" would be the one lie the type exists to prevent.
        (Some(_payload), None) => Ok(AttemptProgress::Untracked),
        (None, _) => Ok(AttemptProgress::NoneSent),
    }
}

/// The tracked entry with the most recent heartbeat.
fn freshest(entries: Vec<TaskLiveness>) -> Option<TaskLiveness> {
    entries
        .into_iter()
        .max_by_key(|liveness| liveness.last_heartbeat_at)
}

#[cfg(test)]
#[path = "attempt_progress_tests.rs"]
mod tests;
