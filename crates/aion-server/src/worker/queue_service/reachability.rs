//! Whether an in-flight activity can still reach a worker.
//!
//! `ActivityStarted` is recorded by the engine at DISPATCH time, atomically with
//! its `ActivityScheduled` and before any worker has leased the work (see
//! `aion::durability::recorder::fan_out` and the single-dispatch seam in
//! `aion::runtime::nif_activity_dispatch`). So a started, unterminated activity
//! means "the engine handed this to the fleet", not "someone is working on it",
//! and a dispatch to a task queue nobody serves is byte-for-byte
//! indistinguishable in history from one a worker is executing right now. The
//! projected status is `Running` for both, correctly — status is a projection of
//! history, and history genuinely contains no terminal event.
//!
//! What history alone cannot say, the live fleet can. This module joins the two:
//! the run's own recorded dispatch addresses against the SAME poller census and
//! the SAME [`classify`] verdict the dispatcher's selection wait uses
//! ([`super::wait`]). There is no second notion of "is anyone serving this" here
//! — a disagreement between what an operator reads and what a dispatch does
//! would be worse than the silence it replaces.
//!
//! The healthy case is silent by construction: [`classify`] returns `None` for
//! an address with a live compatible worker, so a run whose activity is genuinely
//! being executed produces no entry at all.

use aion_core::{ActivityId, Event, StepState, UnservedActivity, WorkflowId};
use chrono::{DateTime, Utc};

use super::census::classify;
use super::declarations::QueueDeclarationSource;
use super::state::QueueServiceState;
use super::taxonomy::ServiceAddress;
use crate::error::ServerError;
use crate::worker::registry::ConnectedWorkerRegistry;

/// One activity the active run segment records as dispatched and unterminated.
///
/// The address fields are the ones the engine STAMPED on the dispatch
/// (`ActivityScheduled` carries the resolved `task_queue` and `node`), so this is
/// the address the dispatch really went to, not a re-resolution that could
/// disagree with it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenActivity {
    /// Activity ordinal recorded in history.
    pub activity_id: ActivityId,
    /// Activity type the dispatch needs served.
    pub activity_type: String,
    /// Task queue stamped on the recorded `ActivityScheduled`.
    pub task_queue: String,
    /// Node affinity stamped on the recorded `ActivityScheduled`, if any.
    pub node: Option<String>,
    /// One-based delivery attempt stamped on the recorded `ActivityStarted`.
    pub attempt: u32,
    /// `recorded_at` of the `ActivityStarted` — when the ENGINE dispatched.
    pub dispatched_at: DateTime<Utc>,
}

/// The activities the run's active segment records as dispatched and not yet
/// terminated, in the order the ordinals were opened.
///
/// This is the DISPATCHED slice of [`aion_core::open_steps`] — the single fold
/// that answers "what is this run's history holding open", shared with the live
/// describe join rather than reimplemented here. Keeping one fold is the point:
/// two scans of the same events, each with its own idea of what a terminal is,
/// would sooner or later disagree, and an operator would be reading whichever
/// one happened to be wired to the surface they opened.
///
/// A step the fold reports as `Scheduled` or `Reopened` is deliberately NOT
/// here. Nothing has been dispatched for it, so there is no delivery for the
/// fleet to fail to serve, and classifying one would report a queue problem
/// against work that has not been handed to a queue.
#[must_use]
pub fn open_activities_in_active_segment(history: &[Event]) -> Vec<OpenActivity> {
    aion_core::open_steps(history)
        .into_iter()
        .filter_map(|step| match step.state {
            StepState::Dispatched {
                attempt,
                dispatched_at,
            } => Some(OpenActivity {
                activity_id: step.activity_id,
                activity_type: step.activity_type,
                task_queue: step.task_queue,
                node: step.node,
                attempt,
                dispatched_at,
            }),
            StepState::Scheduled | StepState::Reopened { .. } => None,
        })
        .collect()
}

/// The live fleet seams an in-flight activity's reachability is judged against.
///
/// Exactly the three the dispatcher's selection wait holds: the connected-worker
/// registry (the census), the deployed queue declarations (the structural
/// verdict), and the parked-dispatch state (whether this process is waiting).
pub struct ActivityReachability<'a> {
    /// Live connected-worker registry.
    pub registry: &'a ConnectedWorkerRegistry,
    /// Deployed queue declarations.
    pub declarations: &'a QueueDeclarationSource,
    /// Live parked-dispatch state.
    pub state: &'a QueueServiceState,
}

impl ActivityReachability<'_> {
    /// Every in-flight activity in `history` the fleet cannot currently serve.
    ///
    /// Returns an EMPTY vector for a run whose in-flight activities all have a
    /// live compatible worker, and for a run with no in-flight activity at all.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] when the connected-worker registry or the parked
    /// dispatch state cannot be read. Never guesses: an unreadable fleet is
    /// reported, not rendered as "everything is fine".
    pub fn unserved(
        &self,
        namespace: &str,
        workflow_id: &WorkflowId,
        history: &[Event],
    ) -> Result<Vec<UnservedActivity>, ServerError> {
        let mut unserved = Vec::new();
        for open in open_activities_in_active_segment(history) {
            let address = ServiceAddress {
                namespace: namespace.to_owned(),
                task_queue: open.task_queue,
                activity_type: open.activity_type,
                node: open.node,
            };
            let census = self.registry.pool_census(
                &address.namespace,
                &address.task_queue,
                &address.activity_type,
                address.node.as_deref(),
            )?;
            let declaration = self.declarations.declaration_for(&address.task_queue);
            // `None` is the healthy answer: a live compatible worker can take
            // this dispatch, so there is nothing to report and nothing is
            // reported.
            let Some(reason) = classify(declaration, &census) else {
                continue;
            };
            let dispatch_parked = self.state.is_unserved(workflow_id, &open.activity_id)?;
            unserved.push(UnservedActivity {
                activity_id: open.activity_id,
                reason: reason.as_str().to_owned(),
                detail: reason.explain(&address),
                activity_type: address.activity_type,
                task_queue: address.task_queue,
                node: address.node,
                attempt: open.attempt,
                dispatched_at: open.dispatched_at,
                workers_in_pool: count(census.workers_in_pool),
                workers_serving_activity: count(census.workers_serving_activity),
                compatible_workers: count(census.compatible_workers),
                dispatch_parked,
            });
        }
        Ok(unserved)
    }
}

/// Widen a census count to the wire width, saturating rather than wrapping.
fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "reachability_tests.rs"]
mod tests;
