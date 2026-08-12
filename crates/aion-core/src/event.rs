//! Workflow history events and their deterministic recording envelope.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    ActivityError, ActivityId, PackageVersion, Payload, RunId, ScheduleConfig, ScheduleId,
    SearchAttributeValue, TimerId, WorkflowError, WorkflowId,
};

/// Metadata recorded with every workflow history event.
#[derive(Serialize, Deserialize, ts_rs::TS, Clone, Debug, PartialEq, Eq)]
pub struct EventEnvelope {
    /// Monotonic sequence number within the owning workflow history.
    pub seq: u64,
    /// Recorded UTC timestamp for this event.
    ///
    /// This timestamp is the determinism source for `workflow.now`; replay must use the recorded
    /// value rather than consulting wall-clock time.
    pub recorded_at: DateTime<Utc>,
    /// Workflow history that owns this event.
    pub workflow_id: WorkflowId,
}

/// The named default task queue: the single sanctioned fallback when no explicit task queue was
/// selected (no SDK-level selection exists yet — that is NSTQ-4) and the replay-safe decode value
/// for [`Event::ActivityScheduled`] events recorded before the `task_queue` field existed.
///
/// This is the canonical task-queue default for the whole workspace. `aion_store::DEFAULT_OUTBOX_ROUTE`
/// and `aion_server::worker::registry::DEFAULT_TASK_QUEUE` both alias/re-export this constant rather
/// than redeclaring the literal, so a history-derived task queue and an outbox-row-derived task queue
/// cannot drift.
pub const DEFAULT_TASK_QUEUE: &str = "default";

/// serde default for [`Event::ActivityScheduled::task_queue`]: the named [`DEFAULT_TASK_QUEUE`].
///
/// Used by `#[serde(default = ...)]` so an old recorded history that has no `task_queue` on its
/// `ActivityScheduled` events decodes deterministically to `"default"`.
fn default_task_queue() -> String {
    String::from(DEFAULT_TASK_QUEUE)
}

/// Sentinel `attempt` value for activity lifecycle events decoded from a history recorded BEFORE the
/// `attempt` field existed on [`Event::ActivityStarted`] / [`Event::ActivityCompleted`] /
/// [`Event::ActivityCancelled`] (NOI-0).
///
/// Activity attempts are **one-based** everywhere they are produced (see [`Event::ActivityFailed`]'s
/// `attempt`, which is documented "One-based activity attempt number", and the engine's
/// `FIRST_DELIVERY_ATTEMPT = 1`). A real attempt is therefore always `>= 1`, so `0` can never collide
/// with a genuine attempt: it is a distinguishable "legacy / unknown attempt" marker. Old histories
/// that predate the field decode to this sentinel via `#[serde(default = "legacy_activity_attempt")]`
/// — deterministically, never panicking, never differing run-to-run — while the compiler still forces
/// every LIVE construction site to supply the genuine one-based attempt (there is no blanket
/// `Default` on the variant).
const LEGACY_ACTIVITY_ATTEMPT: u32 = 0;

/// serde default for the `attempt` field on the activity lifecycle events that gained it in NOI-0.
///
/// Returns [`LEGACY_ACTIVITY_ATTEMPT`] (`0`) so a history recorded before the field existed decodes
/// deterministically to the legacy/unknown sentinel rather than failing. See
/// [`LEGACY_ACTIVITY_ATTEMPT`] for why `0` is a safe distinguishable value under one-based attempts.
fn legacy_activity_attempt() -> u32 {
    LEGACY_ACTIVITY_ATTEMPT
}

/// Search attribute name that records the task queue a workflow was STARTED on.
///
/// The server stamps this attribute durably in the SAME atomic append as
/// [`Event::WorkflowStarted`] (via [`Event::SearchAttributesUpdated`]) when the
/// start request selected a task queue — mirroring the `aion.namespace`
/// attribute that records the owning namespace. It is therefore part of
/// RECORDED HISTORY: recovery/replay re-derive the identical value, so an
/// activity that falls back to its workflow's start-time queue (#144) resolves
/// to the same queue on every replay. The attribute is absent when the start
/// did not select a queue (the legacy / "no selection anywhere" case).
///
/// This is the canonical name for the whole workspace;
/// `aion_server::TASK_QUEUE_ATTRIBUTE` re-exports it rather than redeclaring the
/// literal, so a history-derived start-time queue and the server's recorded
/// attribute cannot drift.
pub const START_TIME_TASK_QUEUE_ATTRIBUTE: &str = "aion.task_queue";

/// The task queue a workflow was STARTED on, projected from recorded history.
///
/// Reads the [`START_TIME_TASK_QUEUE_ATTRIBUTE`] search attribute folded from
/// the run's [`Event::SearchAttributesUpdated`] events (the server records it in
/// the same append as [`Event::WorkflowStarted`]). Returns `None` when the start
/// recorded no task-queue selection — a legacy history, or a start that left the
/// queue unset — so callers fall back to the named [`DEFAULT_TASK_QUEUE`].
///
/// Because the value is read purely from recorded history (never from live or
/// wall-clock state), it is replay-deterministic: the same history always
/// projects the same start-time queue.
#[must_use]
pub fn start_time_task_queue(events: &[Event]) -> Option<String> {
    let attributes = crate::search_attributes_from_events(events);
    match attributes.get(START_TIME_TASK_QUEUE_ATTRIBUTE) {
        Some(crate::SearchAttributeValue::String(queue)) => Some(queue.clone()),
        _ => None,
    }
}

/// Search attribute name that carries a workflow's operator-facing display
/// name (#211).
///
/// The display name is a LABEL over the workflow's UUID identity, never an
/// address. Stated exactly, because the label rule is about ACTS and not about
/// whether a name may appear on the wire:
///
/// 1. **No operation acts on a name.** Every act-verb — start, signal, query,
///    cancel, pause, resume, reopen, rename, intervene — takes a `WorkflowId`
///    (and optionally a `RunId`). None of them accepts a name as its target.
/// 2. **Name-shaped FILTERING returns lists and acts on nothing.** This
///    attribute is an ordinary search attribute, so it is filterable like any
///    other: a visibility predicate on it yields candidate summaries. That is
///    the first half of a picker (name in, candidates out, an operator
///    chooses, a UUID acts), and it is deliberately supported.
/// 3. **The composition is prohibited.** No SDK, CLI verb, MCP tool, or
///    convenience function may compose name-filter → take-first → act. That
///    composition re-imports addressing one layer up: the system, not the
///    caller, would be choosing the referent. The CHOOSING STEP STAYS WITH THE
///    CALLER, BY CONTRACT. A future "find by name" helper RETURNS CANDIDATES
///    AND STOPS.
///
/// The sweep predicate, written down so the next audit checks the right thing
/// rather than re-deriving it: *for every verb, HTTP route, RPC, MCP tool, and
/// SDK function that CHANGES state, the target is identified by a UUID; a
/// display name appears only as a value being SET, never as the selector, and
/// no single call turns a name into an act.* The MCP tool catalogue enforces
/// this mechanically (`aion_server::mcp::catalog` — see its
/// `name_is_not_an_address_tests`); the other surfaces are Rust types rather
/// than data and are checked by reading against that predicate.
///
/// It is recorded durably as an ordinary [`Event::SearchAttributesUpdated`]
/// search attribute — stamped in the same atomic append as
/// [`Event::WorkflowStarted`] when the start named one, and re-recorded by
/// every rename — so history keeps every name the workflow has worn and
/// replay/failover re-derive the current one.
///
/// The recorded attribute carries NO run id. A rename is addressed to a
/// workflow AND a run, but what it records is a workflow-level attribute, and
/// every reader folds it over the whole history (see [`display_name`]). So a
/// continue-as-new successor inherits the name its predecessor was wearing, and
/// describing any run of the workflow reads back the same, current name.
///
/// This is the canonical name for the whole workspace;
/// `aion_server::DISPLAY_NAME_ATTRIBUTE` re-exports it rather than redeclaring
/// the literal, so the projector below and the server's recorded attribute
/// cannot drift.
pub const DISPLAY_NAME_ATTRIBUTE: &str = "aion.display_name";

/// The workflow's current display name, projected from recorded history.
///
/// Reads the [`DISPLAY_NAME_ATTRIBUTE`] search attribute folded over EVERY
/// [`Event::SearchAttributesUpdated`] in `events`, last write wins — so a
/// rename's later update supersedes the start-time name while history keeps
/// both. Returns `None` when no name was ever recorded (the unnamed workflow
/// renders as its bare UUID).
///
/// The fold is per WORKFLOW, not per run: it is not run-scoped the way
/// [`crate::failure_projection`] and [`crate::current_lease_terminal`] are, and
/// the recorded attribute carries no run id to scope it by. Hand it a whole
/// workflow history and the newest recorded name wins whichever run recorded
/// it — so a continue-as-new successor reads back the name its predecessor was
/// wearing until something renames it.
///
/// Because the value is read purely from recorded history (never from live or
/// wall-clock state), it is replay-deterministic: the same history always
/// projects the same display name.
#[must_use]
pub fn display_name(events: &[Event]) -> Option<String> {
    display_name_from_attributes(&crate::search_attributes_from_events(events))
}

/// The display name carried by an already-folded search-attribute map.
///
/// The same projection as [`display_name`], for readers that hold the folded
/// map rather than the events — the visibility projection's
/// `search_attributes`, which the store keeps refreshed on every append. Both
/// entry points share this one lookup so the attribute name and the
/// String-typed expectation cannot drift between the history reader and the
/// visibility reader. The map is itself folded over a whole workflow history,
/// so this reads the same per-workflow label with the same last-write-wins
/// rule.
#[must_use]
pub fn display_name_from_attributes<S: std::hash::BuildHasher>(
    attributes: &std::collections::HashMap<String, crate::SearchAttributeValue, S>,
) -> Option<String> {
    match attributes.get(DISPLAY_NAME_ATTRIBUTE) {
        Some(crate::SearchAttributeValue::String(name)) => Some(name.clone()),
        _ => None,
    }
}

/// A recorded workflow history event.
///
/// User data is carried as opaque [`Payload`] values, while failures use the closed workflow and
/// activity error types from this crate.
#[derive(Serialize, Deserialize, ts_rs::TS, Clone, Debug, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum Event {
    /// A workflow execution started with a type name and input payload.
    WorkflowStarted {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Workflow type selected by the caller.
        workflow_type: String,
        /// Opaque workflow input payload.
        input: Payload,
        /// Concrete run identifier started by this event.
        run_id: RunId,
        /// Parent run that continued as this run, when this start is part of a
        /// continue-as-new chain.
        parent_run_id: Option<RunId>,
        /// Package version this run was resolved against at record time.
        ///
        /// Recovery and replay resolve workflow code from this recorded
        /// version; they never re-resolve a "latest" version.
        package_version: PackageVersion,
    },
    /// A workflow execution completed successfully; this terminal event projects to Completed.
    WorkflowCompleted {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Opaque workflow result payload.
        result: Payload,
    },
    /// A workflow execution failed terminally; this terminal event projects to Failed.
    WorkflowFailed {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Terminal workflow failure.
        error: WorkflowError,
    },
    /// A workflow execution was cancelled; this terminal event projects to Cancelled.
    WorkflowCancelled {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Human-readable cancellation reason.
        reason: String,
    },
    /// A workflow execution timed out; this terminal event projects to `TimedOut`.
    WorkflowTimedOut {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Descriptor identifying the timeout that elapsed.
        ///
        /// Intentionally stringly-typed: the closed set of timeout kinds is defined by cluster AT
        /// (timers and signals), not by the core event model.
        timeout: String,
    },
    /// A workflow execution continued as a new run; this terminal event projects to
    /// `ContinuedAsNew`.
    WorkflowContinuedAsNew {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Opaque workflow input payload carried into the new run.
        input: Payload,
        /// Workflow type override for the new run, when migration changes the workflow type.
        ///
        /// When absent, the new run uses the current workflow type.
        workflow_type: Option<String>,
        /// Run identifier for the current run that is being continued.
        parent_run_id: RunId,
    },
    /// A failed run was reopened.
    ///
    /// Engine-internal — never authored by workflow or SDK code. This is the
    /// compensating event that reconciles reopen with the status-is-a-projection
    /// invariant: under the last-lifecycle-event-wins scan it supersedes the
    /// run's prior terminal event and returns the run to Running, exactly as a
    /// replacement [`Event::WorkflowStarted`] does for continue-as-new. Terminal
    /// detection is scoped to "since the last reopen point", so a run holds
    /// exactly one terminal event per lease.
    WorkflowReopened {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Run being reopened — the run that recorded the superseded terminal
        /// event and that the reopened execution continues.
        run_id: RunId,
        /// Activities to re-dispatch on replay: those that ended in a terminal
        /// failure in this run with no later successful attempt. The history
        /// cursor treats each as a reset point so the recorded failure is
        /// superseded and the activity resolves to live re-dispatch.
        reopened: Vec<ActivityId>,
    },
    /// A running workflow was paused by an operator.
    ///
    /// Engine-internal — never authored by workflow or SDK code. A NON-terminal
    /// lifecycle marker: under the last-lifecycle-event-wins scan it projects the
    /// run to [`crate::WorkflowStatus::Paused`], holding new activity dispatch at
    /// the outbox while every durable record path (timer fires, signal receipts,
    /// drained completions) keeps recording. It is invisible to the replay cursor
    /// (it is neither a terminal nor a run-start reset), so a paused-then-resumed
    /// history replays byte-identically to one that was never paused.
    WorkflowPaused {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Run being paused — the live, non-terminal run the operator held.
        run_id: RunId,
        /// Optional operator-supplied pause reason.
        reason: Option<String>,
        /// Optional identity of the operator who issued the pause.
        operator: Option<String>,
    },
    /// A paused workflow was resumed by an operator.
    ///
    /// Engine-internal — never authored by workflow or SDK code. Supersedes the
    /// run's prior [`Event::WorkflowPaused`] under the last-lifecycle-event-wins
    /// scan, returning the run to [`crate::WorkflowStatus::Running`], and — like
    /// [`Event::WorkflowPaused`] — is invisible to the replay cursor.
    WorkflowResumed {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Run being resumed.
        run_id: RunId,
        /// Optional identity of the operator who issued the resume.
        operator: Option<String>,
    },
    /// Workflow search attributes were updated for visibility and query projection.
    SearchAttributesUpdated {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Workflow whose search attributes changed.
        workflow_id: WorkflowId,
        /// Updated search attributes keyed by attribute name.
        attributes: HashMap<String, SearchAttributeValue>,
    },
    /// An activity was scheduled by workflow code.
    ActivityScheduled {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Deterministic activity identifier derived from the scheduling sequence position.
        activity_id: ActivityId,
        /// Activity type selected by workflow code.
        activity_type: String,
        /// Opaque activity input payload.
        input: Payload,
        /// Pool/flavour selector this activity dispatches to within the workflow's namespace
        /// (NSTQ-3). This is the durable source-of-truth for re-targeting the **same** task queue
        /// on reopen/recovery, mirroring how the namespace is recovered from history but recorded
        /// **per-activity** rather than as a workflow-level search attribute.
        ///
        /// Replay-safety: histories recorded before this field existed have no `task_queue` on
        /// their `ActivityScheduled` events. Decode defaults the missing value to
        /// [`DEFAULT_TASK_QUEUE`] (`"default"`) via `#[serde(default = ...)]`, so an old history
        /// deterministically re-derives `task_queue = "default"` — never panics, never differs
        /// run-to-run. The encoding of the existing fields is untouched.
        #[serde(default = "default_task_queue")]
        task_queue: String,
        /// OPTIONAL node affinity this activity dispatches to (NODE-3). `None` = no affinity (the
        /// genuine current value; SDK-level node selection is NODE-4). This is the durable
        /// source-of-truth for re-targeting the **same** node on reopen/recovery, recorded
        /// **per-activity** alongside `task_queue`.
        ///
        /// Replay-safety: histories recorded before this field existed have no `node` key on their
        /// `ActivityScheduled` events. serde's `Option` default is `None`, so `#[serde(default)]`
        /// decodes a missing `node` deterministically to `None` — never a sentinel, never panics,
        /// never differs run-to-run. The encoding of the existing fields is untouched.
        #[serde(default)]
        node: Option<String>,
    },
    /// The engine DISPATCHED an activity attempt to its task queue.
    ///
    /// **This does NOT mean a worker has taken the work.** The event is written
    /// by the engine at dispatch time, in the SAME atomic append as its
    /// [`Event::ActivityScheduled`] and under the same `recorded_at`, before any
    /// worker has been selected — let alone leased the attempt. Worker selection
    /// happens afterwards, in the server, and may wait indefinitely: a dispatch
    /// to a task queue nobody serves records this event and then nothing, so a
    /// run parked forever and a run a worker is actively executing have
    /// identical history shapes and both project
    /// [`WorkflowStatus::Running`](crate::WorkflowStatus::Running). That is not
    /// a projection bug — history genuinely holds no terminal event — but it
    /// means this event alone can never answer "is anyone working on it".
    ///
    /// Every producing seam behaves this way: the single-dispatch and in-VM
    /// seams (`aion::runtime::nif_activity_dispatch`,
    /// `aion::runtime::nif_activity_in_vm`), the retry delivery
    /// (`aion::runtime::nif_activity_retry_dispatch`, which records the next
    /// attempt's start "before it goes on the wire"), and the fan-out batch
    /// (`aion::durability::recorder::fan_out`).
    ///
    /// The name is therefore wrong and the behaviour is not. A rename to
    /// `ActivityDispatched` is PROPOSED and PENDING an owner ruling, together
    /// with the question of whether a separate lease-time event should exist;
    /// see `docs/design/aion-authoring/ACTIVITY-STARTED-SEMANTICS-DECISION.md`.
    /// Deferring THIS event to lease time is recommended against there, because
    /// it would silently change what every already-recorded `ActivityStarted`
    /// meant.
    ///
    /// To ask whether an in-flight activity can still reach a worker, join its
    /// recorded address to the live fleet — `aion_server`'s
    /// `worker::ActivityReachability`, surfaced on `POST /workflows/describe`.
    ActivityStarted {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Activity that was dispatched.
        activity_id: ActivityId,
        /// One-based activity attempt number this start belongs to (NOI-0).
        ///
        /// Matches the `attempt` on the [`Event::ActivityFailed`] / [`Event::ActivityCompleted`] /
        /// [`Event::ActivityCancelled`] that terminates the SAME attempt, so
        /// `(workflow, activity, attempt)` is a stable identity across the whole lifecycle — the key
        /// the NOI dedupe/guard/session-id design is built on.
        ///
        /// Replay-safety: histories recorded before this field existed have no `attempt` key on their
        /// `ActivityStarted` events. Decode defaults the missing value to
        /// [`LEGACY_ACTIVITY_ATTEMPT`] (`0`) via `#[serde(default = ...)]` — never panics, never
        /// differs run-to-run. Because real attempts are one-based, `0` is a distinguishable
        /// legacy/unknown sentinel, never a genuine attempt. The encoding of the existing fields is
        /// untouched.
        #[serde(default = "legacy_activity_attempt")]
        attempt: u32,
    },
    /// An activity completed successfully.
    ActivityCompleted {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Activity that produced the result.
        activity_id: ActivityId,
        /// Opaque activity result payload.
        result: Payload,
        /// One-based activity attempt number that produced this completion (NOI-0).
        ///
        /// Matches the `attempt` on the [`Event::ActivityStarted`] of the SAME attempt, so a
        /// completed activity carries one consistent `attempt` readable off both its start and its
        /// terminal — the negative-control invariant NOI-0 gates on.
        ///
        /// Replay-safety: histories recorded before this field existed have no `attempt` key on their
        /// `ActivityCompleted` events. Decode defaults the missing value to
        /// [`LEGACY_ACTIVITY_ATTEMPT`] (`0`) via `#[serde(default = ...)]` — never panics, never
        /// differs run-to-run. The encoding of the existing fields is untouched.
        #[serde(default = "legacy_activity_attempt")]
        attempt: u32,
    },
    /// An activity attempt failed.
    ///
    /// The `attempt` field together with [`ActivityError`]'s retryable or terminal classification
    /// lets replay distinguish a retryable interim failure from a terminal one for the same
    /// [`ActivityId`].
    ActivityFailed {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Activity whose attempt failed.
        activity_id: ActivityId,
        /// Classified activity failure.
        error: ActivityError,
        /// One-based activity attempt number that produced this failure.
        attempt: u32,
    },
    /// An ADVISORY activity spent its whole attempt budget and failed for good
    /// (RUNTIME-OPERATIONS.md R5).
    ///
    /// The warning the class promises: an advisory action is a side channel
    /// (a heartbeat, a notification), so its exhaustion never faults the
    /// calling step — but it must never be silent either. This event is that
    /// visibility. It ACCOMPANIES the activity's honest terminal
    /// [`Event::ActivityFailed`]; it never replaces it, because the activity
    /// really did fail and history says so.
    ///
    /// Non-terminal for the workflow: it says nothing about the run's
    /// outcome, so status projection deliberately ignores it.
    ActivityAdvisoryExhausted {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Advisory activity whose attempt budget was spent.
        activity_id: ActivityId,
        /// Activity type of the exhausted advisory activity.
        activity_type: String,
        /// The last attempt's failure reason, verbatim — the same string the
        /// accompanying terminal [`Event::ActivityFailed`] carries.
        reason: String,
        /// One-based number of the attempt that spent the budget.
        attempt: u32,
    },
    /// An activity was cancelled as an explicit cancellation outcome.
    ActivityCancelled {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Activity that was cancelled.
        activity_id: ActivityId,
        /// One-based activity attempt number that was cancelled (NOI-0).
        ///
        /// Matches the `attempt` on the [`Event::ActivityStarted`] of the SAME attempt, so the
        /// cancellation terminal is attributable to a specific attempt exactly like
        /// [`Event::ActivityFailed`] is.
        ///
        /// Replay-safety: histories recorded before this field existed have no `attempt` key on their
        /// `ActivityCancelled` events. Decode defaults the missing value to
        /// [`LEGACY_ACTIVITY_ATTEMPT`] (`0`) via `#[serde(default = ...)]` — never panics, never
        /// differs run-to-run. The encoding of the existing fields is untouched.
        #[serde(default = "legacy_activity_attempt")]
        attempt: u32,
    },
    /// A timer was scheduled to fire at a deterministic timestamp.
    TimerStarted {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Timer selected by workflow code or assigned by the engine.
        timer_id: TimerId,
        /// UTC timestamp at which the timer becomes eligible to fire.
        fire_at: DateTime<Utc>,
    },
    /// A timer fired.
    TimerFired {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Timer that fired.
        timer_id: TimerId,
    },
    /// A timer was cancelled as an explicit cancellation outcome.
    TimerCancelled {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Timer that was cancelled.
        timer_id: TimerId,
        /// Who retired the timer. Decides reopen behavior: a
        /// [`TimerCancelCause::CancelTeardown`] cancellation is re-armed when the
        /// run is reopened; a [`TimerCancelCause::WorkflowIntent`] cancellation is
        /// permanent.
        ///
        /// Replay-safety: histories recorded before this field existed have no
        /// `cause` key. Decode defaults the missing value to
        /// [`TimerCancelCause::WorkflowIntent`] via `#[serde(default)]` — the
        /// pre-field behavior (never resurrected), never panics, never differs
        /// run-to-run. The encoding of the existing fields is untouched.
        #[serde(default)]
        cause: TimerCancelCause,
    },
    /// A `with_timeout` operation reached a durable terminal outcome.
    WithTimeoutCompleted {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Timer that bounded the operation.
        timer_id: TimerId,
        /// Recorded timeout outcome.
        outcome: WithTimeoutOutcome,
        /// JSON-encoded BEAM term payload for completed operation results.
        result: Option<Payload>,
    },
    /// A signal was delivered to the workflow.
    SignalReceived {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Signal name selected by the sender.
        name: String,
        /// Opaque signal payload.
        payload: Payload,
    },
    /// A signal was sent by this workflow to another workflow.
    SignalSent {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Target workflow identifier selected by workflow code.
        target_workflow_id: WorkflowId,
        /// Signal name selected by workflow code.
        name: String,
        /// Opaque signal payload.
        payload: Payload,
    },
    /// A child workflow was started.
    ChildWorkflowStarted {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Child workflow identifier.
        child_workflow_id: WorkflowId,
        /// Child workflow type selected by the parent.
        workflow_type: String,
        /// Opaque child workflow input payload.
        input: Payload,
        /// Package version resolved for the child at record time.
        ///
        /// The crash-repair sweep and the child's own start use exactly this
        /// recorded version, so the crash path resolves identically to the
        /// crash-free path.
        package_version: PackageVersion,
    },
    /// A child workflow completed successfully.
    ChildWorkflowCompleted {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Child workflow that produced the result.
        child_workflow_id: WorkflowId,
        /// Opaque child workflow result payload.
        result: Payload,
    },
    /// A child workflow failed terminally.
    ChildWorkflowFailed {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Child workflow that failed.
        child_workflow_id: WorkflowId,
        /// Terminal child workflow failure.
        error: WorkflowError,
    },
    /// A child workflow was cancelled as an explicit cancellation outcome.
    ChildWorkflowCancelled {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Child workflow that was cancelled.
        child_workflow_id: WorkflowId,
    },
    /// A schedule resource was created.
    ScheduleCreated {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Schedule resource that was created.
        schedule_id: ScheduleId,
        /// Persisted schedule configuration.
        config: ScheduleConfig,
    },
    /// A schedule resource was updated.
    ScheduleUpdated {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Schedule resource that was updated.
        schedule_id: ScheduleId,
        /// Updated schedule configuration.
        config: ScheduleConfig,
    },
    /// A schedule resource was paused.
    SchedulePaused {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Schedule resource that was paused.
        schedule_id: ScheduleId,
    },
    /// A paused schedule resource was resumed.
    ScheduleResumed {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Schedule resource that was resumed.
        schedule_id: ScheduleId,
    },
    /// A schedule resource was deleted.
    ScheduleDeleted {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Schedule resource that was deleted.
        schedule_id: ScheduleId,
    },
    /// A schedule tick started a workflow execution.
    ScheduleTriggered {
        /// Recording metadata for this event.
        envelope: EventEnvelope,
        /// Schedule resource that fired.
        schedule_id: ScheduleId,
        /// Workflow execution started by the schedule tick.
        workflow_id: WorkflowId,
        /// Run started by the schedule tick.
        run_id: RunId,
    },
}

/// Durable terminal outcome for a `with_timeout` operation.
#[derive(Serialize, Deserialize, ts_rs::TS, Clone, Debug, PartialEq, Eq)]
pub enum WithTimeoutOutcome {
    /// The operation closure returned before the deadline.
    OperationCompleted,
    /// The deadline fired before the operation completed.
    TimedOut,
}

/// Who retired a durable timer, recorded on [`Event::TimerCancelled`].
///
/// The distinction decides reopen semantics. A timer the WORKFLOW retired —
/// an SDK `cancel_timer` call or a `with_timeout` scope settling because the
/// racing operation won — is a business fact: reopen must never resurrect it.
/// A timer the ENGINE retired while tearing down a cancelled run
/// (`Engine::cancel`'s in-flight timer cleanup) is bookkeeping: the deadline
/// itself was never reached or waived, so reopening the run re-arms it at its
/// original `fire_at`.
#[derive(Serialize, Deserialize, ts_rs::TS, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimerCancelCause {
    /// Workflow code retired the timer (SDK cancel or a settled timeout scope).
    ///
    /// The serde default: histories recorded before this field existed decode
    /// as workflow intent, preserving their pre-field never-resurrected
    /// behavior.
    #[default]
    WorkflowIntent,
    /// The engine retired the timer while cancelling its workflow run.
    CancelTeardown,
}

impl Event {
    /// Returns the envelope recorded with this event.
    #[must_use]
    pub const fn envelope(&self) -> &EventEnvelope {
        match self {
            Self::WorkflowStarted { envelope, .. }
            | Self::WorkflowCompleted { envelope, .. }
            | Self::WorkflowFailed { envelope, .. }
            | Self::WorkflowCancelled { envelope, .. }
            | Self::WorkflowTimedOut { envelope, .. }
            | Self::WorkflowContinuedAsNew { envelope, .. }
            | Self::WorkflowReopened { envelope, .. }
            | Self::WorkflowPaused { envelope, .. }
            | Self::WorkflowResumed { envelope, .. }
            | Self::SearchAttributesUpdated { envelope, .. }
            | Self::ActivityScheduled { envelope, .. }
            | Self::ActivityStarted { envelope, .. }
            | Self::ActivityCompleted { envelope, .. }
            | Self::ActivityFailed { envelope, .. }
            | Self::ActivityAdvisoryExhausted { envelope, .. }
            | Self::ActivityCancelled { envelope, .. }
            | Self::TimerStarted { envelope, .. }
            | Self::TimerFired { envelope, .. }
            | Self::TimerCancelled { envelope, .. }
            | Self::WithTimeoutCompleted { envelope, .. }
            | Self::SignalReceived { envelope, .. }
            | Self::SignalSent { envelope, .. }
            | Self::ChildWorkflowStarted { envelope, .. }
            | Self::ChildWorkflowCompleted { envelope, .. }
            | Self::ChildWorkflowFailed { envelope, .. }
            | Self::ChildWorkflowCancelled { envelope, .. }
            | Self::ScheduleCreated { envelope, .. }
            | Self::ScheduleUpdated { envelope, .. }
            | Self::SchedulePaused { envelope, .. }
            | Self::ScheduleResumed { envelope, .. }
            | Self::ScheduleDeleted { envelope, .. }
            | Self::ScheduleTriggered { envelope, .. } => envelope,
        }
    }

    /// Returns the monotonic sequence number recorded for this event.
    #[must_use]
    pub const fn seq(&self) -> u64 {
        self.envelope().seq
    }

    /// Returns the deterministic recorded timestamp for this event.
    #[must_use]
    pub const fn recorded_at(&self) -> &DateTime<Utc> {
        &self.envelope().recorded_at
    }

    /// Returns the workflow history that owns this event.
    #[must_use]
    pub const fn workflow_id(&self) -> &WorkflowId {
        &self.envelope().workflow_id
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::{DateTime, Utc};
    use serde_json::json;

    use super::{
        DEFAULT_TASK_QUEUE, Event, EventEnvelope, LEGACY_ACTIVITY_ATTEMPT, TimerCancelCause,
    };
    use crate::{
        ActivityError, ActivityErrorKind, ActivityId, CatchUpPolicy, OverlapPolicy, PackageVersion,
        Payload, RunId, ScheduleConfig, ScheduleId, SearchAttributeValue, TimerId, TriggerSpec,
        WorkflowError, WorkflowId,
    };

    fn package_version() -> PackageVersion {
        PackageVersion::new("a".repeat(64))
    }

    fn recorded_at() -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000, 123_000_000).unwrap_or_default()
    }

    fn envelope(seq: u64) -> EventEnvelope {
        EventEnvelope {
            seq,
            recorded_at: recorded_at(),
            workflow_id: WorkflowId::new(uuid::Uuid::nil()),
        }
    }

    fn payload(label: &str) -> Result<Payload, crate::PayloadError> {
        Payload::from_json(&json!({ "label": label }))
    }

    fn schedule_config(label: &str) -> Result<ScheduleConfig, crate::PayloadError> {
        Ok(ScheduleConfig {
            trigger: TriggerSpec::Cron {
                expression: String::from("0 0 * * *"),
            },
            overlap_policy: OverlapPolicy::Skip,
            catch_up_policy: CatchUpPolicy::One,
            workflow_type: String::from("checkout"),
            input: payload(label)?,
            search_attributes: HashMap::from([(
                String::from("aion.namespace"),
                crate::SearchAttributeValue::String(String::from("tenant-a")),
            )]),
        })
    }

    fn workflow_error(message: &str) -> WorkflowError {
        WorkflowError {
            message: String::from(message),
            details: None,
        }
    }

    fn activity_error(kind: ActivityErrorKind, message: &str) -> ActivityError {
        ActivityError {
            kind,
            message: String::from(message),
            details: None,
        }
    }

    fn round_trip(event: &Event) -> Result<(), serde_json::Error> {
        let json = serde_json::to_string(event)?;
        let decoded = serde_json::from_str::<Event>(&json)?;
        assert_eq!(*event, decoded);
        Ok(())
    }

    /// NSTQ-3: a recorded `ActivityScheduled` carries its `task_queue` through the durable JSON
    /// wire so reopen/recovery can re-target the same pool.
    #[test]
    fn activity_scheduled_records_and_reads_back_its_task_queue()
    -> Result<(), Box<dyn std::error::Error>> {
        let event = Event::ActivityScheduled {
            envelope: envelope(6),
            activity_id: ActivityId::from_sequence_position(6),
            activity_type: String::from("charge-card"),
            input: payload("activity-input")?,
            task_queue: String::from("claude"),
            node: None,
        };

        let json = serde_json::to_string(&event)?;
        let decoded = serde_json::from_str::<Event>(&json)?;

        match decoded {
            Event::ActivityScheduled { task_queue, .. } => {
                assert_eq!(
                    task_queue, "claude",
                    "the recorded task queue must survive the round-trip"
                );
            }
            other => return Err(format!("expected ActivityScheduled, got {other:?}").into()),
        }
        Ok(())
    }

    /// NSTQ-3 replay-safety (the load-bearing test): an OLD recorded history that has no
    /// `task_queue` key on its `ActivityScheduled` events MUST still decode, defaulting the missing
    /// value to the named `"default"` task queue, deterministically — never panic, never differ
    /// run-to-run. The old wire form is the exact pre-field bytes: the current serialization with
    /// the `task_queue` key removed.
    #[test]
    fn activity_scheduled_decodes_old_history_without_task_queue_as_default()
    -> Result<(), Box<dyn std::error::Error>> {
        // Build a current event, serialize, then strip the `task_queue` key to reconstruct exactly
        // what a history recorded before the field existed looks like on the wire.
        let current = Event::ActivityScheduled {
            envelope: envelope(6),
            activity_id: ActivityId::from_sequence_position(6),
            activity_type: String::from("charge-card"),
            input: payload("activity-input")?,
            task_queue: String::from("ignored-when-stripped"),
            node: Some(String::from("ignored-when-stripped")),
        };
        let mut value = serde_json::to_value(&current)?;
        let data = value
            .get_mut("data")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or("ActivityScheduled must serialize to a tagged object with a `data` map")?;
        assert!(
            data.remove("task_queue").is_some(),
            "the current wire form must contain task_queue before we strip it"
        );

        // Decode the stripped (old-shape) wire form repeatedly: it must succeed and always read
        // back the named default, deterministically.
        let old_wire = serde_json::to_string(&value)?;
        for _ in 0..4 {
            let decoded = serde_json::from_str::<Event>(&old_wire)?;
            match &decoded {
                Event::ActivityScheduled { task_queue, .. } => {
                    assert_eq!(
                        task_queue, DEFAULT_TASK_QUEUE,
                        "a missing task_queue must default to the named default queue"
                    );
                    assert_eq!(task_queue, "default");
                }
                other => return Err(format!("expected ActivityScheduled, got {other:?}").into()),
            }
        }
        Ok(())
    }

    /// NODE-3: a recorded `ActivityScheduled` carries its OPTIONAL `node` affinity through the
    /// durable JSON wire so reopen/recovery can re-target the same node.
    #[test]
    fn activity_scheduled_records_and_reads_back_its_node() -> Result<(), Box<dyn std::error::Error>>
    {
        let event = Event::ActivityScheduled {
            envelope: envelope(6),
            activity_id: ActivityId::from_sequence_position(6),
            activity_type: String::from("charge-card"),
            input: payload("activity-input")?,
            task_queue: String::from("claude"),
            node: Some(String::from("box-7")),
        };

        let json = serde_json::to_string(&event)?;
        let decoded = serde_json::from_str::<Event>(&json)?;

        match decoded {
            Event::ActivityScheduled { node, .. } => {
                assert_eq!(
                    node.as_deref(),
                    Some("box-7"),
                    "the recorded node affinity must survive the round-trip"
                );
            }
            other => return Err(format!("expected ActivityScheduled, got {other:?}").into()),
        }
        Ok(())
    }

    /// NODE-3 replay-safety (the load-bearing test): an OLD recorded history that has no `node` key
    /// on its `ActivityScheduled` events MUST still decode, defaulting the missing value to `None`
    /// (no affinity) deterministically — never a sentinel, never panic, never differ run-to-run.
    /// The old wire form is the exact pre-field bytes: the current serialization with the `node`
    /// key removed.
    #[test]
    fn activity_scheduled_decodes_old_history_without_node_as_none()
    -> Result<(), Box<dyn std::error::Error>> {
        // Build a current event with a node set, serialize, then strip the `node` key to
        // reconstruct exactly what a history recorded before the field existed looks like on the
        // wire.
        let current = Event::ActivityScheduled {
            envelope: envelope(6),
            activity_id: ActivityId::from_sequence_position(6),
            activity_type: String::from("charge-card"),
            input: payload("activity-input")?,
            task_queue: String::from("default"),
            node: Some(String::from("ignored-when-stripped")),
        };
        let mut value = serde_json::to_value(&current)?;
        let data = value
            .get_mut("data")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or("ActivityScheduled must serialize to a tagged object with a `data` map")?;
        assert!(
            data.remove("node").is_some(),
            "the current wire form must contain node before we strip it"
        );

        // Decode the stripped (old-shape) wire form repeatedly: it must succeed and always read
        // back `None`, deterministically.
        let old_wire = serde_json::to_string(&value)?;
        for _ in 0..4 {
            let decoded = serde_json::from_str::<Event>(&old_wire)?;
            match &decoded {
                Event::ActivityScheduled { node, .. } => {
                    assert_eq!(
                        *node, None,
                        "a missing node must default to None (no affinity)"
                    );
                }
                other => return Err(format!("expected ActivityScheduled, got {other:?}").into()),
            }
        }
        Ok(())
    }

    /// NOI-0 positive round-trip: `ActivityStarted`, `ActivityCompleted`, and `ActivityCancelled`
    /// each carry a genuine one-based `attempt` through the durable JSON wire, so replay reads back
    /// the same attempt that was recorded — a completed activity has one consistent attempt readable
    /// off BOTH its start and its terminal (the invariant the NOI design keys on).
    #[test]
    fn activity_lifecycle_records_and_reads_back_its_attempt()
    -> Result<(), Box<dyn std::error::Error>> {
        let started = Event::ActivityStarted {
            envelope: envelope(7),
            activity_id: ActivityId::from_sequence_position(6),
            attempt: 3,
        };
        let completed = Event::ActivityCompleted {
            envelope: envelope(8),
            activity_id: ActivityId::from_sequence_position(6),
            result: payload("activity-result")?,
            attempt: 3,
        };
        let cancelled = Event::ActivityCancelled {
            envelope: envelope(9),
            activity_id: ActivityId::from_sequence_position(6),
            attempt: 3,
        };

        for event in [&started, &completed, &cancelled] {
            round_trip(event)?;
        }

        // Read the attempt back off each decoded terminal — it must be the recorded value, not the
        // legacy sentinel.
        match serde_json::from_str::<Event>(&serde_json::to_string(&started)?)? {
            Event::ActivityStarted { attempt, .. } => assert_eq!(attempt, 3),
            other => return Err(format!("expected ActivityStarted, got {other:?}").into()),
        }
        match serde_json::from_str::<Event>(&serde_json::to_string(&completed)?)? {
            Event::ActivityCompleted { attempt, .. } => assert_eq!(attempt, 3),
            other => return Err(format!("expected ActivityCompleted, got {other:?}").into()),
        }
        match serde_json::from_str::<Event>(&serde_json::to_string(&cancelled)?)? {
            Event::ActivityCancelled { attempt, .. } => assert_eq!(attempt, 3),
            other => return Err(format!("expected ActivityCancelled, got {other:?}").into()),
        }
        Ok(())
    }

    /// NOI-0 replay-safety (the load-bearing negative control): an OLD recorded history that has no
    /// `attempt` key on its `ActivityStarted` / `ActivityCompleted` / `ActivityCancelled` events MUST
    /// still decode without panic, defaulting the missing value to the legacy sentinel
    /// [`LEGACY_ACTIVITY_ATTEMPT`] (`0`) deterministically — never differ run-to-run. Because real
    /// attempts are one-based, `0` can never collide with a genuine attempt. The old wire form is the
    /// exact pre-field bytes: the current serialization with the `attempt` key removed.
    #[test]
    fn activity_lifecycle_decodes_old_history_without_attempt_as_legacy_sentinel()
    -> Result<(), Box<dyn std::error::Error>> {
        // One current event per variant, each with a NON-sentinel attempt so we can prove the strip
        // (not the value) is what drives the default on decode.
        let started = Event::ActivityStarted {
            envelope: envelope(7),
            activity_id: ActivityId::from_sequence_position(6),
            attempt: 5,
        };
        let completed = Event::ActivityCompleted {
            envelope: envelope(8),
            activity_id: ActivityId::from_sequence_position(6),
            result: payload("activity-result")?,
            attempt: 5,
        };
        let cancelled = Event::ActivityCancelled {
            envelope: envelope(9),
            activity_id: ActivityId::from_sequence_position(6),
            attempt: 5,
        };

        // Strip the `attempt` key from each to reconstruct exactly what a pre-NOI-0 history looks
        // like on the wire, then decode the stripped form repeatedly: it must succeed and always read
        // back the legacy sentinel, deterministically.
        for current in [&started, &completed, &cancelled] {
            let mut value = serde_json::to_value(current)?;
            let data = value
                .get_mut("data")
                .and_then(serde_json::Value::as_object_mut)
                .ok_or("activity lifecycle event must serialize to a tagged object with `data`")?;
            assert!(
                data.remove("attempt").is_some(),
                "the current wire form must contain attempt before we strip it"
            );
            let old_wire = serde_json::to_string(&value)?;
            for _ in 0..4 {
                let decoded = serde_json::from_str::<Event>(&old_wire)?;
                let attempt = match &decoded {
                    Event::ActivityStarted { attempt, .. }
                    | Event::ActivityCompleted { attempt, .. }
                    | Event::ActivityCancelled { attempt, .. } => *attempt,
                    other => {
                        return Err(
                            format!("expected an activity lifecycle event, got {other:?}").into(),
                        );
                    }
                };
                assert_eq!(
                    attempt, LEGACY_ACTIVITY_ATTEMPT,
                    "a missing attempt must default to the legacy sentinel (0)"
                );
                assert_eq!(attempt, 0);
            }
        }
        Ok(())
    }

    /// Replay-safety proof for the `cause` field on `TimerCancelled` (#222):
    /// a history recorded BEFORE the field existed has no `cause` key and MUST
    /// decode without panic, defaulting to `WorkflowIntent` — the pre-field
    /// behavior (a reopen never resurrects it) — deterministically. The old
    /// wire form is the exact pre-field bytes: the current serialization with
    /// the `cause` key removed.
    #[test]
    fn timer_cancelled_decodes_old_history_without_cause_as_workflow_intent()
    -> Result<(), Box<dyn std::error::Error>> {
        // A NON-default cause proves the strip (not the value) drives the default.
        let cancelled = Event::TimerCancelled {
            envelope: envelope(7),
            timer_id: TimerId::named("deadline")?,
            cause: TimerCancelCause::CancelTeardown,
        };

        let mut value = serde_json::to_value(&cancelled)?;
        let data = value
            .get_mut("data")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or("TimerCancelled must serialize to a tagged object with `data`")?;
        assert!(
            data.remove("cause").is_some(),
            "the current wire form must contain cause before we strip it"
        );
        let old_wire = serde_json::to_string(&value)?;
        for _ in 0..4 {
            let decoded = serde_json::from_str::<Event>(&old_wire)?;
            match &decoded {
                Event::TimerCancelled { cause, .. } => assert_eq!(
                    *cause,
                    TimerCancelCause::WorkflowIntent,
                    "a missing cause must default to WorkflowIntent (never resurrected)"
                ),
                other => {
                    return Err(format!("expected TimerCancelled, got {other:?}").into());
                }
            }
        }
        Ok(())
    }

    /// Pause/resume (#204) round-trip: the two new NON-terminal lifecycle markers
    /// carry plain fields and survive the durable JSON wire unchanged.
    #[test]
    fn pause_resume_events_round_trip_through_json() -> Result<(), Box<dyn std::error::Error>> {
        let events = vec![
            Event::WorkflowPaused {
                envelope: envelope(2),
                run_id: RunId::new(uuid::Uuid::from_u128(1)),
                reason: Some(String::from("operator hold")),
                operator: Some(String::from("tom")),
            },
            Event::WorkflowPaused {
                envelope: envelope(3),
                run_id: RunId::new(uuid::Uuid::from_u128(1)),
                reason: None,
                operator: None,
            },
            Event::WorkflowResumed {
                envelope: envelope(4),
                run_id: RunId::new(uuid::Uuid::from_u128(1)),
                operator: Some(String::from("tom")),
            },
        ];
        for event in &events {
            round_trip(event)?;
        }
        Ok(())
    }

    /// GATE-6 back-compat: an OLD history serialized before pause/resume existed
    /// decodes byte-identically — it simply never contains the new variants. We
    /// prove the whole event enum still decodes an old-shape history with no new
    /// variants present (the decode round-trip test the brief requires), and that
    /// adding the variants did not change the encoding of any existing variant.
    #[test]
    fn old_history_without_pause_resume_decodes_unchanged() -> Result<(), Box<dyn std::error::Error>>
    {
        let started = Event::WorkflowStarted {
            envelope: envelope(1),
            workflow_type: String::from("checkout"),
            input: payload("input")?,
            run_id: RunId::new(uuid::Uuid::from_u128(1)),
            parent_run_id: None,
            package_version: package_version(),
        };
        let completed = Event::WorkflowCompleted {
            envelope: envelope(2),
            result: payload("result")?,
        };
        // Serialize an old-shape history and decode it back: no new variant is
        // present, and every existing variant round-trips exactly.
        let history = vec![started, completed];
        let json = serde_json::to_string(&history)?;
        let decoded = serde_json::from_str::<Vec<Event>>(&json)?;
        assert_eq!(history, decoded);
        Ok(())
    }

    /// #144: the start-time task queue projects from the `aion.task_queue`
    /// search attribute recorded by `SearchAttributesUpdated`, mirroring the
    /// `aion.namespace` projection. A later update overrides an earlier value.
    #[test]
    fn start_time_task_queue_projects_from_recorded_attribute()
    -> Result<(), Box<dyn std::error::Error>> {
        use super::{START_TIME_TASK_QUEUE_ATTRIBUTE, start_time_task_queue};
        use crate::SearchAttributeValue;

        let events = vec![
            Event::WorkflowStarted {
                envelope: envelope(1),
                workflow_type: String::from("checkout"),
                input: payload("input")?,
                run_id: RunId::new(uuid::Uuid::from_u128(1)),
                parent_run_id: None,
                package_version: package_version(),
            },
            Event::SearchAttributesUpdated {
                envelope: envelope(2),
                workflow_id: WorkflowId::new(uuid::Uuid::nil()),
                attributes: HashMap::from([(
                    START_TIME_TASK_QUEUE_ATTRIBUTE.to_owned(),
                    SearchAttributeValue::String(String::from("gpu")),
                )]),
            },
        ];

        assert_eq!(start_time_task_queue(&events).as_deref(), Some("gpu"));
        Ok(())
    }

    /// #144 back-compat: a history with no recorded `aion.task_queue` attribute
    /// projects `None`, so callers fall back to the named default.
    #[test]
    fn start_time_task_queue_is_none_without_the_attribute()
    -> Result<(), Box<dyn std::error::Error>> {
        use super::start_time_task_queue;

        let events = vec![Event::WorkflowStarted {
            envelope: envelope(1),
            workflow_type: String::from("checkout"),
            input: payload("input")?,
            run_id: RunId::new(uuid::Uuid::from_u128(1)),
            parent_run_id: None,
            package_version: package_version(),
        }];

        assert_eq!(start_time_task_queue(&events), None);
        Ok(())
    }

    /// #211: the display name projects from the `aion.display_name` search
    /// attribute recorded by `SearchAttributesUpdated`, mirroring the
    /// `aion.task_queue` projection.
    #[test]
    fn display_name_projects_from_recorded_attribute() -> Result<(), Box<dyn std::error::Error>> {
        use super::{DISPLAY_NAME_ATTRIBUTE, display_name};
        use crate::SearchAttributeValue;

        let events = vec![
            Event::WorkflowStarted {
                envelope: envelope(1),
                workflow_type: String::from("checkout"),
                input: payload("input")?,
                run_id: RunId::new(uuid::Uuid::from_u128(1)),
                parent_run_id: None,
                package_version: package_version(),
            },
            Event::SearchAttributesUpdated {
                envelope: envelope(2),
                workflow_id: WorkflowId::new(uuid::Uuid::nil()),
                attributes: HashMap::from([(
                    DISPLAY_NAME_ATTRIBUTE.to_owned(),
                    SearchAttributeValue::String(String::from("Nightly settlement")),
                )]),
            },
        ];

        assert_eq!(display_name(&events).as_deref(), Some("Nightly settlement"));
        Ok(())
    }

    /// #211 back-compat: a history with no recorded `aion.display_name`
    /// attribute projects `None`, so the unnamed run renders as its bare UUID.
    #[test]
    fn display_name_is_none_without_the_attribute() -> Result<(), Box<dyn std::error::Error>> {
        use super::display_name;

        let events = vec![Event::WorkflowStarted {
            envelope: envelope(1),
            workflow_type: String::from("checkout"),
            input: payload("input")?,
            run_id: RunId::new(uuid::Uuid::from_u128(1)),
            parent_run_id: None,
            package_version: package_version(),
        }];

        assert_eq!(display_name(&events), None);
        Ok(())
    }

    /// #211 rename invariant: a later `SearchAttributesUpdated` overrides the
    /// earlier name (last write wins) while BOTH remain in history — a rename
    /// is a recorded event, never an overwrite of the past.
    #[test]
    fn display_name_later_update_overrides_earlier() -> Result<(), Box<dyn std::error::Error>> {
        use super::{DISPLAY_NAME_ATTRIBUTE, display_name};
        use crate::SearchAttributeValue;

        let name_event = |seq: u64, name: &str| Event::SearchAttributesUpdated {
            envelope: envelope(seq),
            workflow_id: WorkflowId::new(uuid::Uuid::nil()),
            attributes: HashMap::from([(
                DISPLAY_NAME_ATTRIBUTE.to_owned(),
                SearchAttributeValue::String(String::from(name)),
            )]),
        };
        let events = vec![
            Event::WorkflowStarted {
                envelope: envelope(1),
                workflow_type: String::from("checkout"),
                input: payload("input")?,
                run_id: RunId::new(uuid::Uuid::from_u128(1)),
                parent_run_id: None,
                package_version: package_version(),
            },
            name_event(2, "first name"),
            name_event(3, "second name"),
        ];

        assert_eq!(display_name(&events).as_deref(), Some("second name"));
        // NOTE: the "history keeps both names" half of this invariant is NOT
        // asserted here. `events` is this test's own literal, so counting it
        // would measure the `vec!` rather than any behaviour. It is proven
        // where it can be — against events a real recorder appended — by
        // `aion/tests/rename_e2e.rs`
        // `renaming_twice_supersedes_while_history_keeps_both_names`.
        Ok(())
    }

    #[test]
    fn event_accessors_return_envelope_fields() -> Result<(), Box<dyn std::error::Error>> {
        let workflow_id = WorkflowId::new_v4();
        let recorded_at = recorded_at();
        let envelope = EventEnvelope {
            seq: 17,
            recorded_at,
            workflow_id: workflow_id.clone(),
        };
        let event = Event::WorkflowStarted {
            envelope,
            workflow_type: String::from("checkout"),
            input: payload("input")?,
            run_id: RunId::new(uuid::Uuid::from_u128(1)),
            parent_run_id: None,
            package_version: package_version(),
        };

        assert_eq!(event.seq(), 17);
        assert_eq!(event.recorded_at(), &recorded_at);
        assert_eq!(event.workflow_id(), &workflow_id);
        Ok(())
    }

    #[test]
    fn events_round_trip_through_json() -> Result<(), Box<dyn std::error::Error>> {
        let fire_at = DateTime::from_timestamp(1_700_000_100, 0).unwrap_or_default();
        let events = vec![
            Event::WorkflowStarted {
                envelope: envelope(1),
                workflow_type: String::from("checkout"),
                input: payload("workflow-input")?,
                run_id: RunId::new(uuid::Uuid::from_u128(1)),
                parent_run_id: None,
                package_version: package_version(),
            },
            Event::WorkflowCompleted {
                envelope: envelope(2),
                result: payload("workflow-result")?,
            },
            Event::WorkflowFailed {
                envelope: envelope(3),
                error: workflow_error("workflow failed"),
            },
            Event::WorkflowCancelled {
                envelope: envelope(4),
                reason: String::from("caller requested cancellation"),
            },
            Event::WorkflowTimedOut {
                envelope: envelope(5),
                timeout: String::from("execution"),
            },
            Event::ActivityScheduled {
                envelope: envelope(6),
                activity_id: ActivityId::from_sequence_position(6),
                activity_type: String::from("charge-card"),
                input: payload("activity-input")?,
                task_queue: String::from("claude"),
                node: Some(String::from("box-7")),
            },
            Event::ActivityStarted {
                envelope: envelope(7),
                activity_id: ActivityId::from_sequence_position(6),
                attempt: 1,
            },
            Event::ActivityCompleted {
                envelope: envelope(8),
                activity_id: ActivityId::from_sequence_position(6),
                result: payload("activity-result")?,
                attempt: 1,
            },
            Event::ActivityFailed {
                envelope: envelope(9),
                activity_id: ActivityId::from_sequence_position(6),
                error: activity_error(ActivityErrorKind::Retryable, "temporary outage"),
                attempt: 1,
            },
            Event::ActivityCancelled {
                envelope: envelope(10),
                activity_id: ActivityId::from_sequence_position(6),
                attempt: 1,
            },
            Event::TimerStarted {
                envelope: envelope(11),
                timer_id: TimerId::anonymous(11),
                fire_at,
            },
            Event::TimerFired {
                envelope: envelope(12),
                timer_id: TimerId::anonymous(11),
            },
            Event::TimerCancelled {
                envelope: envelope(13),
                timer_id: TimerId::named("reminder")?,
                cause: TimerCancelCause::WorkflowIntent,
            },
            Event::SignalReceived {
                envelope: envelope(14),
                name: String::from("approve"),
                payload: payload("signal")?,
            },
            Event::SignalSent {
                envelope: envelope(15),
                target_workflow_id: WorkflowId::new(uuid::Uuid::from_u128(5)),
                name: String::from("approve"),
                payload: payload("signal-sent")?,
            },
        ];

        for event in events {
            round_trip(&event)?;
        }
        Ok(())
    }

    #[test]
    fn child_events_round_trip_through_json() -> Result<(), Box<dyn std::error::Error>> {
        let child_workflow_id = WorkflowId::new(uuid::Uuid::from_u128(1));
        let events = vec![
            Event::ChildWorkflowStarted {
                envelope: envelope(16),
                child_workflow_id: child_workflow_id.clone(),
                workflow_type: String::from("fulfillment"),
                input: payload("child-input")?,
                package_version: package_version(),
            },
            Event::ChildWorkflowCompleted {
                envelope: envelope(16),
                child_workflow_id: child_workflow_id.clone(),
                result: payload("child-result")?,
            },
            Event::ChildWorkflowFailed {
                envelope: envelope(17),
                child_workflow_id: child_workflow_id.clone(),
                error: workflow_error("child failed"),
            },
            Event::ChildWorkflowCancelled {
                envelope: envelope(18),
                child_workflow_id,
            },
        ];

        for event in events {
            round_trip(&event)?;
        }
        Ok(())
    }

    #[test]
    fn extended_events_round_trip_through_json() -> Result<(), Box<dyn std::error::Error>> {
        let schedule_id = ScheduleId::new(uuid::Uuid::from_u128(2));
        let triggered_workflow_id = WorkflowId::new(uuid::Uuid::from_u128(3));
        let triggered_run_id = RunId::new(uuid::Uuid::from_u128(4));
        let events = vec![
            Event::WorkflowContinuedAsNew {
                envelope: envelope(19),
                input: payload("continued-input")?,
                workflow_type: Some(String::from("checkout-v2")),
                parent_run_id: RunId::new(uuid::Uuid::from_u128(2)),
            },
            Event::SearchAttributesUpdated {
                envelope: envelope(20),
                workflow_id: WorkflowId::new(uuid::Uuid::nil()),
                attributes: HashMap::from([(
                    String::from("customer_id"),
                    SearchAttributeValue::String(String::from("cust-123")),
                )]),
            },
            Event::ScheduleCreated {
                envelope: envelope(20),
                schedule_id: schedule_id.clone(),
                config: schedule_config("schedule-created")?,
            },
            Event::ScheduleUpdated {
                envelope: envelope(21),
                schedule_id: schedule_id.clone(),
                config: schedule_config("schedule-updated")?,
            },
            Event::SchedulePaused {
                envelope: envelope(22),
                schedule_id: schedule_id.clone(),
            },
            Event::ScheduleResumed {
                envelope: envelope(23),
                schedule_id: schedule_id.clone(),
            },
            Event::ScheduleDeleted {
                envelope: envelope(24),
                schedule_id: schedule_id.clone(),
            },
            Event::ScheduleTriggered {
                envelope: envelope(25),
                schedule_id,
                workflow_id: triggered_workflow_id,
                run_id: triggered_run_id,
            },
        ];

        for event in events {
            round_trip(&event)?;
        }
        Ok(())
    }
}
