//! Describe-workflow response projection.
//!
//! The ops console's `POST /workflows/describe` read consumes exactly this shape:
//! a workflow [`WorkflowSummary`] projection plus the run's event [`Event`]
//! history as plain JSON. Defining it here lets the same type be exported to
//! TypeScript (so the generated bindings match the wire by construction) and be
//! produced directly by the HTTP handler at the transport boundary.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{ActivityId, Event, WorkflowSummary};

/// Response to a describe-workflow request.
///
/// `history` is the run's events as plain serialized [`Event`] values (never a
/// protobuf-derived envelope), so the ops console decodes each entry directly.
/// When `include_history` is false the server returns an empty `history`.
///
/// `unserved` is computed independently of `include_history`: it is the answer
/// to "can anything still advance this run", and an operator who asked for the
/// summary alone still needs it.
#[derive(Serialize, Deserialize, ts_rs::TS, Clone, Debug, PartialEq)]
pub struct DescribeWorkflowResponse {
    /// Workflow summary projected from authoritative history, when the workflow
    /// exists.
    pub summary: Option<WorkflowSummary>,
    /// The run's event history as plain serialized events.
    pub history: Vec<Event>,
    /// Every activity this run's history records as dispatched and unterminated
    /// that the fleet cannot currently serve.
    ///
    /// EMPTY is the healthy answer, and it is the only healthy answer: an
    /// activity a live compatible worker could take is never listed here. A
    /// non-empty list is the difference between "a worker is working on it" and
    /// "it is parked with nobody to take it", which the projected
    /// [`crate::WorkflowStatus`] alone cannot express — `Running` covers both.
    pub unserved: Vec<UnservedActivity>,
}

/// One in-flight activity whose recorded dispatch address is not being served.
///
/// The activity's identity, address, and dispatch instant come from the run's
/// own recorded history; the verdict and the counts come from the live
/// connected-worker fleet at read time. Both halves are named so an operator can
/// see which is which.
#[derive(Serialize, Deserialize, ts_rs::TS, Clone, Debug, PartialEq, Eq)]
pub struct UnservedActivity {
    /// Activity ordinal recorded in history.
    pub activity_id: ActivityId,
    /// Activity type the dispatch needs served.
    pub activity_type: String,
    /// Task queue the dispatch was recorded against.
    pub task_queue: String,
    /// Node affinity recorded on the dispatch, if any.
    pub node: Option<String>,
    /// One-based delivery attempt recorded on the `ActivityStarted`.
    pub attempt: u32,
    /// When the engine recorded the dispatch — NOT when a worker took it, which
    /// is precisely the thing that never happened.
    pub dispatched_at: DateTime<Utc>,
    /// Canonical queue-service reason (`NO_QUEUE_DECLARATION`,
    /// `NO_LIVE_POLLERS`, `POLLERS_INCOMPATIBLE`), in the same vocabulary the
    /// dispatch refusals and the server logs use.
    pub reason: String,
    /// One sentence naming what an operator has to fix.
    pub detail: String,
    /// Workers connected for the `(namespace, task_queue)` pool, whatever they
    /// serve.
    pub workers_in_pool: u64,
    /// Of those, workers advertising this activity type.
    pub workers_serving_activity: u64,
    /// Of those, workers that also satisfy the dispatch's node pin.
    pub compatible_workers: u64,
    /// Whether THIS server process is currently holding the dispatch in its
    /// selection wait.
    ///
    /// False with a non-empty verdict is its own signal: history says the
    /// activity is in flight, the fleet cannot serve it, and no selection in
    /// this process is even waiting for it — the shape a run left behind by a
    /// restart takes.
    pub dispatch_parked: bool,
}
