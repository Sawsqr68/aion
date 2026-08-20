//! NIF bridge dispatcher that routes `run_activity` calls to connected workers.
//!
//! `WorkerActivityDispatcher` implements `aion::ActivityDispatcher` so the
//! engine's activity NIFs can synchronously dispatch to a remote worker and
//! block until the result comes back.
//!
//! # Threading contract
//!
//! The engine invokes [`aion::ActivityDispatcher::dispatch`] from two kinds of
//! threads: beamr scheduler threads (concurrency combinators) and spawned
//! tokio tasks (the two-phase `dispatch_activity` completion task). The task
//! send uses `try_send()` (non-blocking channel push) and the response wait
//! blocks on `std::sync::mpsc::Receiver::recv`.
//!
//! Blocking is harmless on a beamr thread, but on a tokio runtime worker it
//! must be wrapped in `tokio::task::block_in_place`: the `try_send` wakes the
//! per-worker gRPC stream forwarder task, and tokio schedules a task woken
//! from task context into the *current* worker's LIFO slot, which no other
//! runtime worker can steal. Without the `block_in_place` core handoff the
//! forwarder sits trapped in that slot while this thread blocks, so the queued
//! `ActivityTask` is never flushed to the worker even though the worker is
//! healthy. `block_in_place` moves the worker's scheduler core (LIFO slot
//! included) to another thread before the wait begins, so dispatch-to-delivery
//! stays in the millisecond range and the runtime keeps full parallelism.
//!
//! # Wait termination
//!
//! The engine imposes no activity timeout of its own: agent-style activities
//! legitimately run for over an hour, so the completion wait is unbounded.
//! The blocking `recv` terminates on exactly one of:
//!
//! - **Completion** — the worker reports a result and the stream handler
//!   delivers it through [`ActivityCompletionSink::complete_activity`].
//! - **Worker loss** — the worker's gRPC stream ends (process death,
//!   disconnect, expired token); the stream teardown sweeps the worker's
//!   in-flight tasks through the same sink as every other TRANSPORT loss
//!   ([`HeartbeatTracker::fail_disconnected_worker`]). A liminal-delivered
//!   worker's loss is observed by its reply router thread instead (the
//!   correlated-reply awaiter wakes the moment the connection closes) and
//!   resolves the dispatch with the same transport-loss class. Both are
//!   classified by [`transport_loss`](crate::worker::transport_loss): `lost:`
//!   while the transport still has budget to deliver the activity (the engine
//!   re-dispatches the SAME attempt, recording nothing), `transport-exhausted:`
//!   once that budget is spent. Neither ever wears the action's retry
//!   vocabulary — the activity never ran.
//! - **Graceful-drain park (#207)** — during a drain, a worker stream ending
//!   (or the drain-timeout backstop) PARKS the worker's in-flight tasks
//!   through [`ActivityCompletionSink::park_activity`]: the waiter resolves
//!   with the ephemeral parked sentinel
//!   ([`aion::PARKED_ACTIVITY_REASON`]), nothing is recorded or delivered,
//!   and restart recovery re-dispatches the dangling ordinal — kill -9
//!   convergence.
//! - **Drain timeout at shutdown** — the shutdown coordinator parks all
//!   remaining in-flight tasks through the sink
//!   (`HeartbeatTracker::park_all_in_flight_workers`).
//! - **Channel teardown** — every sender for the pending entry is dropped
//!   (a cleanup path removed the entry without completing it); surfaced as
//!   a channel-closed dispatch error, never a hang.
//!
//! An activity's duration is bounded only by the workflow's own
//! `timeout_seconds` and by worker liveness — never by an engine constant.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use aion::{ActivityDispatch, ActivityDispatcher};
use aion_core::{ActivityId, ContentType, Payload, RunId, WorkflowId};
use aion_proto::{ProtoActivityId, ProtoActivityTask, ProtoPayload, ProtoWorkflowId};
use dashmap::DashMap;
use dashmap::mapref::entry::Entry;

use super::dispatch::{ActivityCompletion, ActivityCompletionOutcome, ActivityCompletionSink};
use super::envelope::{CompletionFences, CompletionToken, idempotency_key};
use super::heartbeat::{HeartbeatTracker, InFlightActivity};
use super::queue_service::{
    DeliveryRefusal, ExpiredClock, PARK_POLL_INTERVAL, PoolCensus, QueueDeclarationSource,
    QueueServiceConfig, QueueServiceReason, QueueServiceState, SelectionRefusal, ServiceAddress,
    ServiceWait, WorkerUnavailable, deliver_within_schedule_to_start, select_worker_or_refuse,
};
use super::registry::{
    ConnectedWorkerRegistry, WorkerDelivery, WorkerHandle, WorkerId, WorkerMessage,
};
use crate::error::ServerError;
use crate::shutdown::DrainState;
use tracing::info_span;

type SyncSender = std::sync::mpsc::SyncSender<Result<String, String>>;
type SyncReceiver = std::sync::mpsc::Receiver<Result<String, String>>;

/// Execution-scoped key for an in-flight activity dispatch.
///
/// The engine seam ([`ActivityDispatch`]) carries the *real* workflow id and
/// the *real* per-workflow activity ordinal recorded in history, so this pair
/// uniquely and stably identifies one execution. Keying by bare [`ActivityId`]
/// would be unsafe across server restarts — a stale result re-reported from a
/// worker's previous session could complete a *different* post-restart
/// dispatch reusing the same ordinal — but pairing it with the real workflow
/// id closes that race: two different workflow executions never share a
/// workflow id, so a stale `(workflow_id, activity_id)` from a previous server
/// life can only ever match the exact execution it belongs to.
///
/// The wire (`ActivityResult`) carries both ids, plus an attempt discriminator
/// (`ActivityTask.attempt`). The pending key stays attempt-free for now: a
/// retry re-dispatches under the same `(workflow_id, activity_id)` and the
/// outstanding entry is the one awaiting completion. Redelivery bookkeeping
/// can widen this key with the wire attempt later — no protocol change needed.
type PendingActivityKey = (WorkflowId, ActivityId);

/// Routes an unmatched durable-outbox completion into the live workflow.
///
/// When the outbox is ON a worker completion can arrive at the sink with no
/// pending oneshot (the dispatch was non-blocking fan-out, or the original
/// waiter was lost). Rather than dropping it, [`PendingActivities::complete`]
/// hands it to this callback, which resolves the workflow to its live engine
/// process and delivers the terminal into its mailbox. The callback is only
/// installed when the outbox is enabled, so flag-off the unmatched branch
/// stays a silent drop.
pub trait OutboxDeliveryCallback: Send + Sync {
    /// Deliver a successful completion to the live workflow.
    ///
    /// Returns `Ok(true)` when delivered to a live workflow and `Ok(false)`
    /// when no run is currently live (the expected stale-completion case that
    /// recovery re-arms).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] when the engine rejects the delivery.
    fn deliver_completion(
        &self,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
        run_id: Option<&RunId>,
        result: String,
    ) -> Result<bool, ServerError>;

    /// Deliver a failure to the live workflow. Same `bool`/error contract as
    /// [`Self::deliver_completion`].
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] when the engine rejects the delivery.
    fn deliver_failure(
        &self,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
        run_id: Option<&RunId>,
        reason: String,
    ) -> Result<bool, ServerError>;
}

/// Tracks in-flight activity dispatches waiting for worker results.
///
/// When the server's worker stream handler receives an `ActivityResult`, it
/// calls [`complete_activity`](ActivityCompletionSink::complete_activity) to
/// deliver the result to the blocked NIF thread. Entries are keyed by
/// [`PendingActivityKey`] so a stale result from a previous server life can
/// never be matched to a different execution (#59).
///
/// Clones share both the pending map and the outbox-delivery callback through
/// `Arc`, so [`set_outbox_delivery`](Self::set_outbox_delivery) called once on
/// any clone after construction is visible to the clone the dispatcher holds.
#[derive(Clone)]
pub struct PendingActivities {
    pending: Arc<DashMap<PendingActivityKey, SyncSender>>,
    completion_fences: CompletionFences,
    outbox_delivery: Arc<OnceLock<Arc<dyn OutboxDeliveryCallback>>>,
    /// The transport's OWN re-dispatch budget for activities whose worker died
    /// before reporting (see [`transport_loss`](crate::worker::transport_loss)).
    /// Shared across clones, so every loss for one execution site lands in one
    /// budget. There is no default: the window is required at construction, so
    /// a zero budget cannot be inherited by omission.
    transport_losses: super::transport_loss::TransportLossLedger,
}

impl std::fmt::Debug for PendingActivities {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PendingActivities")
            .field("pending", &self.pending.len())
            .field("completion_fences", &self.completion_fences)
            .field(
                "outbox_delivery_installed",
                &self.outbox_delivery.get().is_some(),
            )
            .field("transport_losses", &self.transport_losses)
            .finish()
    }
}

impl PendingActivities {
    fn insert(
        &self,
        workflow_id: WorkflowId,
        activity_id: ActivityId,
    ) -> Result<(CompletionToken, SyncReceiver), ServerError> {
        let completion_token = self.completion_fences.issue(&workflow_id, &activity_id)?;
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let key = (workflow_id, activity_id);
        match self.pending.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(tx);
                Ok((completion_token, rx))
            }
            Entry::Occupied(entry) => {
                let (workflow_id, activity_id) = entry.key();
                Err(ServerError::PendingActivityCollision {
                    workflow_id: workflow_id.clone(),
                    activity_id: activity_id.clone(),
                })
            }
        }
    }

    /// Share the generation registry with non-blocking outbox dispatch.
    #[must_use]
    pub fn completion_fences(&self) -> CompletionFences {
        self.completion_fences.clone()
    }

    /// Test seam: register a pending waiter exactly as a live dispatch does,
    /// so crate-internal tests outside this module (the stream-teardown
    /// park-vs-fail pins) can observe how a sweep resolves it.
    #[cfg(test)]
    pub(crate) fn insert_for_test(
        &self,
        workflow_id: WorkflowId,
        activity_id: ActivityId,
    ) -> Result<(CompletionToken, SyncReceiver), ServerError> {
        self.insert(workflow_id, activity_id)
    }

    /// Install the unmatched-completion delivery callback (idempotent).
    ///
    /// Set once, after construction, when the durable outbox is enabled. A
    /// second set is ignored and logged: the callback is process-wide and must
    /// not silently change identity.
    pub fn set_outbox_delivery(&self, callback: Arc<dyn OutboxDeliveryCallback>) {
        if self.outbox_delivery.set(callback).is_err() {
            tracing::warn!("outbox delivery callback already installed; ignoring duplicate set");
        }
    }

    /// Complete a pending dispatch, or route an unmatched completion to the
    /// outbox delivery callback when one is installed.
    ///
    /// A matched entry delivers to its waiting oneshot exactly as before. An
    /// unmatched completion is dropped silently when no callback is installed
    /// (outbox OFF — byte-identical to the prior behaviour); with a callback
    /// installed (outbox ON) it is routed into the live workflow's mailbox.
    fn complete(
        &self,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
        run_id: Option<&RunId>,
        result: Result<String, String>,
    ) -> bool {
        // Take and drop the DashMap guard before any callback runs: the engine
        // delivery the callback invokes must never execute under a shard lock.
        let matched = self
            .pending
            .remove(&(workflow_id.clone(), activity_id.clone()));
        if let Some((_, sender)) = matched {
            return sender.send(result).is_ok();
        }
        let Some(callback) = self.outbox_delivery.get() else {
            // Outbox OFF: silent drop, byte-identical to the prior behaviour.
            return false;
        };
        let outcome = match result {
            Ok(payload) => callback.deliver_completion(workflow_id, activity_id, run_id, payload),
            Err(reason) => callback.deliver_failure(workflow_id, activity_id, run_id, reason),
        };
        match outcome {
            Ok(true) => true,
            Ok(false) => {
                // Not live: the expected stale-completion case recovery re-arms.
                tracing::debug!(
                    workflow_id = %workflow_id,
                    activity_id = %activity_id,
                    "unmatched outbox completion for a workflow that is not currently live; \
                     recovery will re-arm it"
                );
                false
            }
            Err(error) => {
                tracing::warn!(
                    workflow_id = %workflow_id,
                    activity_id = %activity_id,
                    %error,
                    "failed to deliver unmatched outbox completion to the live workflow"
                );
                false
            }
        }
    }

    /// Atomically consume `completion_token` and only then resolve its waiter or
    /// durable-outbox callback.
    fn complete_fenced(
        &self,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
        run_id: Option<&RunId>,
        completion_token: &CompletionToken,
        result: Result<String, String>,
    ) -> Result<bool, ServerError> {
        self.completion_fences
            .accept(workflow_id, activity_id, completion_token)
            .inspect_err(|error| {
                tracing::warn!(
                    workflow_id = %workflow_id,
                    activity_id = %activity_id,
                    %error,
                    "activity completion rejected by execution-generation fence"
                );
            })?;
        // A resolution OUTSIDE the transport domain — a real result, or a real
        // action failure — retires this execution site's transport-loss budget,
        // so a run that survived one blip does not carry it forward. A
        // transport-domain resolution deliberately does not: the running budget
        // is exactly what bounds a flapping link.
        let transport_domain = result
            .as_ref()
            .err()
            .is_some_and(|reason| super::transport_loss::is_transport_domain_reason(reason));
        if !transport_domain {
            if let Err(error) = self.transport_losses.clear(workflow_id, activity_id) {
                tracing::warn!(
                    workflow_id = %workflow_id,
                    activity_id = %activity_id,
                    %error,
                    "failed to retire the transport-loss budget for a resolved activity"
                );
            }
        }
        Ok(self.complete(workflow_id, activity_id, run_id, result))
    }

    /// Build a sink whose transport-loss ledger derives its budget from the
    /// operator's heartbeat window (see
    /// [`transport_loss`](crate::worker::transport_loss)).
    ///
    /// The window is a CONSTRUCTOR parameter rather than a builder step, and
    /// that is the point: there is no way to obtain a `PendingActivities`
    /// without naming the window, so a wiring cannot supply everything else and
    /// silently inherit a zero budget. The previous shape —
    /// `default().with_heartbeat_window(w)` — was two calls where the second was
    /// omissible, and nothing in the test suite could tell the wired world from
    /// the unwired one.
    #[must_use]
    pub fn new(heartbeat_window: std::time::Duration) -> Self {
        Self {
            pending: Arc::default(),
            completion_fences: CompletionFences::default(),
            outbox_delivery: Arc::default(),
            transport_losses: super::transport_loss::TransportLossLedger::new(heartbeat_window),
        }
    }

    /// The transport-loss ledger this sink classifies worker deaths through.
    #[must_use]
    pub const fn transport_losses(&self) -> &super::transport_loss::TransportLossLedger {
        &self.transport_losses
    }

    /// Classify one worker loss for `(workflow_id, activity_id)` into the
    /// transport-domain reason the engine seam consumes.
    ///
    /// A ledger failure (poisoned lock) is reported as transport exhaustion
    /// rather than as a re-dispatchable loss: with no trustworthy budget the
    /// only safe answer is the one that terminates, because an unbounded
    /// re-dispatch is the failure mode the budget exists to prevent.
    fn classify_worker_loss(
        &self,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
        worker_id: crate::worker::registry::WorkerId,
    ) -> String {
        let detail = super::transport_loss::worker_lost_detail(worker_id);
        match self
            .transport_losses
            .record_loss(workflow_id, activity_id, &detail)
        {
            Ok(verdict) => {
                if verdict.exhausted {
                    tracing::error!(
                        operation = "activity_complete",
                        workflow_id = %workflow_id,
                        activity_id = %activity_id,
                        worker_id = ?worker_id,
                        error_type = "TransportExhausted",
                        losses = verdict.losses,
                        budget_ms = self.transport_losses.budget().as_millis(),
                        "activity abandoned: the transport kept losing its worker past the \
                         transport-loss budget"
                    );
                } else {
                    tracing::warn!(
                        operation = "activity_complete",
                        workflow_id = %workflow_id,
                        activity_id = %activity_id,
                        worker_id = ?worker_id,
                        error_type = "WorkerLost",
                        losses = verdict.losses,
                        budget_ms = self.transport_losses.budget().as_millis(),
                        "worker lost before reporting an activity result; the activity never ran \
                         and will be re-dispatched attempt-neutrally"
                    );
                }
                verdict.reason
            }
            Err(error) => {
                tracing::error!(
                    workflow_id = %workflow_id,
                    activity_id = %activity_id,
                    %error,
                    "transport-loss ledger is unreadable; abandoning the activity rather than \
                     re-dispatching it without a budget"
                );
                format!(
                    "{}{detail} (transport-loss budget unreadable: {error})",
                    super::transport_loss::TRANSPORT_EXHAUSTED_REASON_PREFIX
                )
            }
        }
    }
}

impl ActivityCompletionSink for PendingActivities {
    /// Park one in-flight dispatch for restart recovery (#207): resolve the
    /// matched waiter with the ephemeral parked sentinel, and nothing else.
    ///
    /// This resolution is MANDATORY, not an optimization: the default
    /// `ActivityDispatcher::dispatch_async` runs the dispatcher's blocking
    /// `std::sync::mpsc::recv()` on tokio's blocking pool, and tokio `Runtime`
    /// drop joins blocking threads — an unresolved waiter would wedge process
    /// exit indefinitely. An unmatched dispatch (already resolved by another
    /// path) is a no-op: a park is NEVER routed to the outbox delivery
    /// callback, because it is not a failure and must never reach a workflow.
    fn park_activity(
        &self,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
    ) -> Result<(), ServerError> {
        self.completion_fences
            .revoke_current(workflow_id, activity_id)?;
        let matched = self
            .pending
            .remove(&(workflow_id.clone(), activity_id.clone()));
        if let Some((_, sender)) = matched {
            // A send failure means the waiter side already dropped its
            // receiver (the dispatch is being cleaned up concurrently) —
            // benign: there is no thread left to unblock.
            let _ = sender.send(Err(aion::PARKED_ACTIVITY_REASON.to_owned()));
        }
        Ok(())
    }

    fn complete_activity(&self, completion: ActivityCompletion) -> Result<(), ServerError> {
        let result = match completion.outcome {
            ActivityCompletionOutcome::Succeeded(payload) => {
                payload_to_string(&payload).map_err(|reason| {
                    tracing::error!(
                        operation = "activity_complete",
                        workflow_id = %completion.workflow_id,
                        activity_id = %completion.activity_id,
                        error_type = "ActivityResultDecode",
                        %reason,
                        "activity completion failed"
                    );
                    ServerError::worker_dispatch("", "", format!("payload decode: {reason}"))
                })?
            }
            ActivityCompletionOutcome::Failed(error) => {
                let prefix = if error.is_retryable() {
                    "retryable"
                } else {
                    "terminal"
                };
                tracing::error!(
                    operation = "activity_complete",
                    workflow_id = %completion.workflow_id,
                    activity_id = %completion.activity_id,
                    error_type = "ActivityFailed",
                    error_kind = prefix,
                    reason = %error.message,
                    "activity completion failed"
                );
                Err(format!("{prefix}:{}", error.message))
            }
            // A TRANSPORT-domain loss: the activity never executed to a result,
            // so it is classified by the transport's own ledger (re-dispatchable
            // while its budget holds, transport-exhausted once it is spent) and
            // NEVER by the action's retry vocabulary.
            ActivityCompletionOutcome::WorkerLost { worker_id } => Err(self.classify_worker_loss(
                &completion.workflow_id,
                &completion.activity_id,
                worker_id,
            )),
        };
        self.complete_fenced(
            &completion.workflow_id,
            &completion.activity_id,
            completion.run_id.as_ref(),
            &completion.completion_token,
            result,
        )?;
        Ok(())
    }
}

fn payload_to_string(payload: &Payload) -> Result<Result<String, String>, String> {
    match payload.content_type() {
        ContentType::Json => String::from_utf8(payload.bytes().to_vec())
            .map(Ok)
            .map_err(|_| "activity result payload is not valid UTF-8".to_owned()),
    }
}

/// Dispatcher that routes `run_activity` NIF calls to connected workers.
///
/// Synchronous interface — uses `try_send` for the task channel and
/// `std::sync::mpsc::Receiver::recv` for the response. Callers on a
/// multi-thread tokio runtime are detected and moved into
/// `tokio::task::block_in_place` so the blocking wait never starves the
/// runtime tasks that flush the worker stream (see the module docs).
pub struct WorkerActivityDispatcher {
    registry: ConnectedWorkerRegistry,
    namespace: String,
    pending: PendingActivities,
    heartbeat_tracker: HeartbeatTracker,
    drain_state: DrainState,
    tokio_handle: Option<tokio::runtime::Handle>,
    /// NOI-6 attempt→owner back-index. When installed, each liminal-delivered
    /// dispatch binds its `(workflow, activity, attempt)` to the owning worker
    /// for the dispatch's lifetime, so the intervention router (and the ops
    /// console's live-attempts enumeration) can see and target it. `None`
    /// (isolated tests) binds nothing.
    attempt_owners: Option<super::intervention::AttemptOwnerIndex>,
    /// R1 service policies and the two service clocks. Default: `strict` with
    /// no clocks — refuse the structurally unservable, wait (loudly) for
    /// everything else, and invent no deadline the operator never wrote.
    queue_service: QueueServiceConfig,
    /// Deployed queue declarations, installed once the engine exists. Answers
    /// `Unknown` until then, and `Unknown` never refuses anything.
    queue_declarations: QueueDeclarationSource,
    /// Live unserved-queue state a parked dispatch publishes itself into.
    queue_state: QueueServiceState,
    /// Cluster-event publisher an unbounded park announces itself on (#266
    /// T4). `None` (isolated tests) loses only the pushed echo; the WARN and
    /// the queryable state above cannot be switched off by wiring.
    cluster_publisher: Option<crate::cluster_publisher::ClusterEventPublisher>,
}

impl std::fmt::Debug for WorkerActivityDispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerActivityDispatcher")
            .field("namespace", &self.namespace)
            .finish_non_exhaustive()
    }
}

impl WorkerActivityDispatcher {
    /// Build a dispatcher for the given namespace, worker registry, and
    /// liveness tracker.
    ///
    /// The tracker must be the same instance the worker stream handler and
    /// shutdown coordinator share: the unbounded completion wait relies on
    /// stream teardown sweeping this tracker's in-flight entries to fail
    /// dispatches whose worker was lost.
    #[must_use]
    pub fn new(
        registry: ConnectedWorkerRegistry,
        namespace: impl Into<String>,
        heartbeat_tracker: HeartbeatTracker,
    ) -> Self {
        Self {
            registry,
            namespace: namespace.into(),
            // Derived from the tracker this constructor is already handed,
            // which already carries the operator's window: the dispatcher
            // cannot be given a heartbeat window and a mismatched budget.
            pending: PendingActivities::new(heartbeat_tracker.heartbeat_window()),
            heartbeat_tracker,
            drain_state: DrainState::default(),
            tokio_handle: None,
            attempt_owners: None,
            queue_service: QueueServiceConfig::default(),
            queue_declarations: QueueDeclarationSource::default(),
            queue_state: QueueServiceState::default(),
            cluster_publisher: None,
        }
    }

    /// Share the deployment-global cluster-event publisher so a dispatch
    /// parked with no availability deadline is announced on the operator's
    /// real-time channel, not only in the log (#266 T4).
    #[must_use]
    pub fn with_cluster_publisher(
        mut self,
        cluster_publisher: crate::cluster_publisher::ClusterEventPublisher,
    ) -> Self {
        self.cluster_publisher = Some(cluster_publisher);
        self
    }

    /// Share the operator's R1 queue-service settings: the default policy, the
    /// written per-queue `durable_pending` opt-ins, and the two clocks.
    #[must_use]
    pub fn with_queue_service(mut self, queue_service: QueueServiceConfig) -> Self {
        self.queue_service = queue_service;
        self
    }

    /// Share the queue-declaration source the boot path fills in once the
    /// engine exists (the handle is cloneable; installing on any clone is
    /// visible here).
    #[must_use]
    pub fn with_queue_declarations(mut self, queue_declarations: QueueDeclarationSource) -> Self {
        self.queue_declarations = queue_declarations;
        self
    }

    /// Share the queue-service state so the server can read which addresses are
    /// unserved and which runs are parked on them.
    #[must_use]
    pub fn with_queue_state(mut self, queue_state: QueueServiceState) -> Self {
        self.queue_state = queue_state;
        self
    }

    /// Share the server's NOI-6 attempt→owner back-index so liminal-delivered
    /// dispatches are visible (and targetable) to the intervention router for
    /// exactly as long as they are in flight. The production boot passes
    /// `ServerState`'s index — the SAME instance `intervenable_attempts` and
    /// `intervene` read — or the console's live-attempts list stays empty for
    /// every bridge-dispatched agent step.
    #[must_use]
    pub fn with_attempt_owners(
        mut self,
        attempt_owners: super::intervention::AttemptOwnerIndex,
    ) -> Self {
        self.attempt_owners = Some(attempt_owners);
        self
    }

    /// The completion sink this dispatcher classifies worker deaths through.
    ///
    /// Exposed so the transport-loss budget a wiring actually ends up with is
    /// observable, rather than being a property only prose asserts.
    #[must_use]
    pub const fn pending(&self) -> &PendingActivities {
        &self.pending
    }

    /// Share a caller-supplied pending-activities tracker.
    #[must_use]
    pub fn with_pending(mut self, pending: PendingActivities) -> Self {
        self.pending = pending;
        self
    }

    /// Share the server drain gate.
    #[must_use]
    pub fn with_drain_state(mut self, drain_state: DrainState) -> Self {
        self.drain_state = drain_state;
        self
    }

    /// Share the server runtime handle for sync history writes from dirty NIF threads.
    #[must_use]
    pub fn with_tokio_handle(mut self, tokio_handle: tokio::runtime::Handle) -> Self {
        self.tokio_handle = Some(tokio_handle);
        self
    }
}

impl WorkerActivityDispatcher {
    /// The drain gate for one dispatch, and the single place a draining server's
    /// refusal is turned into a reason the engine acts on.
    ///
    /// The reason handed back is the #207 parked sentinel
    /// ([`aion::PARKED_ACTIVITY_REASON`]), NOT the gate's own message. A
    /// dispatch the drain refused never reached a worker and never ran, which is
    /// precisely the state that sentinel was minted for: record nothing, deliver
    /// nothing, and let post-restart replay re-dispatch the dangling ordinal —
    /// the kill -9 convergence `shutdown::ShutdownOutcome::Parked` documents.
    ///
    /// Both alternatives are wrong here, and neither is a style preference:
    ///
    /// - the *unprefixed* gate message this used to return is classified as a
    ///   terminal action failure, so a routine deploy records `ActivityFailed`
    ///   (and, with no authored retry, `WorkflowFailed`) for work nobody
    ///   attempted — the synthesized failure #207 exists to abolish;
    /// - the TRANSPORT-loss class (`lost:`) is attempt-neutral but re-dispatches
    ///   the SAME attempt live and in-process, which on a draining server means
    ///   refuse → re-dispatch → refuse, spinning against the very gate that
    ///   refused it while the process is trying to exit.
    ///
    /// The operator-facing detail is not lost: the gate's own message is what is
    /// logged here. Only the engine-facing classification is the sentinel.
    fn ensure_accepting(
        &self,
        namespace: &str,
        activity_type: &str,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
        worker_id: Option<WorkerId>,
    ) -> Result<(), String> {
        self.drain_state
            .ensure_accepting(namespace, activity_type)
            .map_err(|error| {
                log_worker_error(
                    "WorkerDispatch",
                    namespace,
                    activity_type,
                    workflow_id,
                    activity_id,
                    worker_id,
                    &error.to_string(),
                );
                aion::PARKED_ACTIVITY_REASON.to_owned()
            })
    }

    /// Select a worker for the address, or refuse with a typed R1 reason.
    ///
    /// The wait itself lives in [`super::queue_service::wait`]: every selection
    /// miss is classified against the deployed contract records and the live
    /// poller census, published to the queue-service state, and stated at WARN.
    /// This method owns only the two things the seam cannot delegate — the
    /// drain gate and the runtime plumbing of one bounded park.
    fn select_worker_or_wait(
        &self,
        address: &ServiceAddress,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
    ) -> Result<WorkerHandle, String> {
        let wait = ServiceWait {
            registry: &self.registry,
            declarations: &self.queue_declarations,
            config: &self.queue_service,
            state: &self.queue_state,
            address,
            workflow_id,
            activity_id,
            publisher: self.cluster_publisher.as_ref(),
        };
        let mut accepting = || {
            self.ensure_accepting(
                &address.namespace,
                &address.activity_type,
                workflow_id,
                activity_id,
                None,
            )
        };
        let mut park = |budget: Option<Duration>| self.park_for_worker(budget);
        select_worker_or_refuse(&wait, &mut accepting, &mut park).map_err(|refusal| {
            let reason = refusal.reason_string();
            // The drain gate already logged its own refusal at its own site;
            // logging it twice would double-count the incident.
            if !matches!(refusal, SelectionRefusal::NotAccepting { .. }) {
                let error_type = match refusal {
                    SelectionRefusal::Unavailable(_) => "WorkerUnavailable",
                    _ => "WorkerRegistry",
                };
                log_worker_error(
                    error_type,
                    &address.namespace,
                    &address.activity_type,
                    workflow_id,
                    activity_id,
                    None,
                    &reason,
                );
            }
            reason
        })
    }

    /// Wait once for a worker arrival — bounded by `budget` when one is given,
    /// and bounded in BOTH arms by the server's own shutdown.
    ///
    /// The arrival wait races the drain latch, and that race is the whole point.
    /// Without it a `None` budget — the default, because no schedule-to-start
    /// deadline is configured unless an operator writes one — parked this
    /// dispatch on a `spawn_blocking` thread with nothing left that could ever
    /// wake it: the drain gate is consulted once per selection iteration, and
    /// the iteration was stuck inside this wait. Tokio's runtime `Drop` joins
    /// the blocking pool, so `aion server` could not exit even after `main`
    /// returned an exit code — it kept the store's writer lock and blocked the
    /// next deploy into the same data directory. The captured stack of a real
    /// one is `docs/evidence/samples/aion-server-hang-after-drain-15202.sample`.
    /// The `Some(budget)` arm carried the same defect at the scale of the
    /// budget: exit was delayed by up to the whole availability deadline.
    ///
    /// Racing the latch is the fix rather than capping the wait, because the
    /// wait is legitimately unbounded while the server runs: a queue no worker
    /// serves yet is a fleet condition an operator resolves by starting a
    /// worker, and a deadline invented here would refuse work nobody asked to
    /// have refused.
    ///
    /// Waking on the latch decides nothing by itself. The selection loop
    /// re-selects, misses, and consults the SAME drain gate it always
    /// consulted — [`Self::ensure_accepting`] — which is where the refusal and
    /// its classification are produced. This seam gains no second opinion about
    /// draining; it only stops being unwakeable.
    fn park_for_worker(&self, budget: Option<Duration>) {
        let handle = self
            .tokio_handle
            .clone()
            .or_else(|| tokio::runtime::Handle::try_current().ok());
        let Some(handle) = handle else {
            // No runtime in reach (a plain OS thread in an isolated test):
            // sleep the seam's park interval, clamped to the remaining budget.
            // Bounded by construction, so the loop re-consults the drain gate
            // within one interval and needs no signal to race.
            std::thread::sleep(
                budget.map_or(PARK_POLL_INTERVAL, |budget| budget.min(PARK_POLL_INTERVAL)),
            );
            return;
        };
        handle.block_on(async {
            let arrival = async {
                tokio::select! {
                    () = self.registry.wait_for_worker() => {}
                    () = self.drain_state.wait_for_drain() => {}
                }
            };
            match budget {
                None => arrival.await,
                Some(budget) => {
                    // Timeout is not failure here: the caller re-classifies and
                    // decides whether the clock has run out.
                    drop(tokio::time::timeout(budget, arrival).await);
                }
            }
        });
    }

    fn track_worker_task(
        &self,
        worker_id: WorkerId,
        activity_type: &str,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
        attempt: u32,
        completion_token: CompletionToken,
    ) -> Result<(), String> {
        self.heartbeat_tracker
            .track_task(
                worker_id,
                InFlightActivity {
                    workflow_id: workflow_id.clone(),
                    activity_id: activity_id.clone(),
                    // The engine-provided attempt this delivery carries — the
                    // same one stamped on the wire task and bound into the
                    // NOI-6 attempt→owner index, so the tracker, the wire, and
                    // the owner index all name one attempt.
                    attempt,
                    completion_token,
                },
                Instant::now(),
            )
            .map_err(|error| {
                let reason = error.to_string();
                log_worker_error(
                    "WorkerHeartbeatTracker",
                    &self.namespace,
                    activity_type,
                    workflow_id,
                    activity_id,
                    Some(worker_id),
                    &reason,
                );
                reason
            })
    }

    fn cleanup_activity(
        &self,
        worker_id: WorkerId,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
        completion_token: &CompletionToken,
    ) {
        self.pending
            .pending
            .remove(&(workflow_id.clone(), activity_id.clone()));
        if let Err(error) =
            self.pending
                .completion_fences
                .revoke(workflow_id, activity_id, completion_token)
        {
            tracing::error!(
                workflow_id = %workflow_id,
                activity_id = %activity_id,
                %error,
                "failed to revoke undelivered activity generation"
            );
        }
        let _ = self
            .heartbeat_tracker
            .complete_task(worker_id, workflow_id, activity_id);
        self.drain_state.notify_activity_drained();
    }

    /// Deliver one dispatched task to the selected worker over ITS transport.
    ///
    /// The bridge is transport-agnostic at this seam: selection
    /// ([`Self::select_worker_or_wait`]) already treats every registry member
    /// identically, and this match delivers on whichever [`WorkerDelivery`] leg
    /// the worker registered with — the gRPC stream `mpsc` push, or the liminal
    /// server-push on the worker's existing connection. Both legs resolve
    /// through the SAME pending map, so `await_activity_result` is oblivious to
    /// the transport.
    fn send_activity_task(
        &self,
        worker: &WorkerHandle,
        task: ProtoActivityTask,
        address: &ServiceAddress,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
        completion_token: &CompletionToken,
    ) -> Result<(), String> {
        match worker.delivery() {
            WorkerDelivery::Grpc(sender) => {
                let worker_id = worker.id();
                let mut accepting = || {
                    self.ensure_accepting(
                        &address.namespace,
                        &address.activity_type,
                        workflow_id,
                        activity_id,
                        Some(worker_id),
                    )
                };
                let handed_over = deliver_within_schedule_to_start(
                    sender,
                    WorkerMessage::ActivityTask(Box::new(task)),
                    self.queue_service.schedule_to_start_timeout,
                    &mut accepting,
                );
                let Err(refusal) = handed_over else {
                    return Ok(());
                };
                self.cleanup_activity(worker_id, workflow_id, activity_id, completion_token);
                let (error_type, reason) = self.hand_off_failure(&refusal, address);
                if !matches!(refusal, DeliveryRefusal::NotAccepting { .. }) {
                    log_worker_error(
                        error_type,
                        &address.namespace,
                        &address.activity_type,
                        workflow_id,
                        activity_id,
                        Some(worker_id),
                        &reason,
                    );
                }
                Err(reason)
            }
            // The liminal leg has no bounded intake queue to saturate: the push
            // either enqueues on the worker's live connection or fails
            // synchronously because that connection is already gone. There is
            // therefore no schedule-to-start window to apply here, and reporting
            // `SATURATED` for a push failure would name a condition that did not
            // happen.
            #[cfg(feature = "liminal-transport")]
            WorkerDelivery::Liminal(delivery) => self.send_liminal_activity_task(
                worker.id(),
                delivery,
                task,
                &address.activity_type,
                workflow_id,
                activity_id,
            ),
        }
    }

    /// Render one hand-off refusal into its log class and failure reason.
    ///
    /// `SATURATED` is the only one that becomes a typed [`WorkerUnavailable`]:
    /// a compatible worker WAS live and would not take the task inside the
    /// schedule-to-start clock. A full intake with no clock configured, and a
    /// closed transport, keep the failure they always had.
    fn hand_off_failure(
        &self,
        refusal: &DeliveryRefusal,
        address: &ServiceAddress,
    ) -> (&'static str, String) {
        match refusal {
            DeliveryRefusal::Saturated { waited } => {
                // A census failure must not swallow the refusal: report the
                // saturation with an empty census and say why it is empty.
                let census = self
                    .registry
                    .pool_census(
                        &address.namespace,
                        &address.task_queue,
                        &address.activity_type,
                        address.node.as_deref(),
                    )
                    .unwrap_or_else(|error| {
                        tracing::error!(
                            namespace = %address.namespace,
                            task_queue = %address.task_queue,
                            activity_type = %address.activity_type,
                            %error,
                            "poller census failed while reporting a saturated queue; \
                             the refusal carries an empty census"
                        );
                        PoolCensus::default()
                    });
                let unavailable = WorkerUnavailable {
                    reason: QueueServiceReason::Saturated,
                    clock: Some(ExpiredClock::ScheduleToStart),
                    waited: *waited,
                    address: address.clone(),
                    census,
                };
                ("WorkerUnavailable", unavailable.reason_string())
            }
            DeliveryRefusal::Full => (
                "WorkerChannelClosed",
                "worker task channel full or closed: no available capacity".to_owned(),
            ),
            DeliveryRefusal::Closed => (
                "WorkerChannelClosed",
                "worker task channel full or closed: channel closed".to_owned(),
            ),
            DeliveryRefusal::NotAccepting { reason } => ("WorkerDispatch", reason.clone()),
        }
    }

    /// Deliver one dispatched task to a liminal-connected worker: push the SAME
    /// wire frame the outbox liminal path pushes (a
    /// [`DispatchRequest`](super::liminal_transport::DispatchRequest) — the
    /// worker's serve loop cannot tell a bridge dispatch from an outbox row) and
    /// hand the correlated-reply awaiter to a dedicated router thread that
    /// resolves this dispatch's pending entry exactly like a gRPC completion.
    ///
    /// The wire carries the SAME engine-provided `attempt` and `labels` the gRPC
    /// arm's `ActivityTask` carries (a retry over liminal executes with the real
    /// attempt, not a re-stamped first delivery), plus the server's heartbeat
    /// window so the worker's automatic liveness pump keeps this TRACKED
    /// dispatch alive under the #176 expiry sweeper. It carries the engine's
    /// concrete run identity, generation proof, and stable effect key exactly
    /// like the gRPC bridge task.
    ///
    /// A successful push also binds the attempt into the NOI-6 attempt→owner
    /// back-index (when installed) with the SAME `(workflow, activity, attempt)`
    /// key the worker stamps its intervention session with, exactly as the
    /// outbox liminal arm binds each row dispatch — so the ops console can
    /// enumerate this live attempt and route interventions to its worker. The
    /// binding is released when the reply router exits (reply, abandonment, or
    /// disconnect — every path). The gRPC arm carries no bind because the agent
    /// harness seam exists only on the liminal worker transport.
    #[cfg(feature = "liminal-transport")]
    fn send_liminal_activity_task(
        &self,
        worker_id: WorkerId,
        delivery: &super::liminal_transport::LiminalWorkerDelivery,
        task: ProtoActivityTask,
        activity_type: &str,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
    ) -> Result<(), String> {
        let completion_token =
            CompletionToken::from_wire(workflow_id, activity_id, task.completion_token.clone())
                .map_err(|error| error.to_string())?;
        let heartbeat_window_ms =
            u64::try_from(self.heartbeat_tracker.heartbeat_window().as_millis())
                .unwrap_or(u64::MAX);
        let attempt = task.attempt;
        // The run is resolved BEFORE the dispatch is built, because everything
        // downstream is keyed on it: the worker refuses a run-less dispatch
        // envelope outright, the attempt-owner binding below needs it to name a
        // generation, and the attempt's transcript is written under it. Refusing
        // here reports the fault against the dispatch that has it, rather than
        // shipping a frame whose only possible outcome is the worker's rejection.
        let run_id = task
            .run_id
            .map(RunId::try_from)
            .transpose()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| {
                "activity task run id is missing; refusing to dispatch an unidentified run"
                    .to_owned()
            })?;
        let request = super::liminal_transport::DispatchRequest {
            activity_type: activity_type.to_owned(),
            workflow_id: workflow_id.clone(),
            ordinal: activity_id.sequence_position(),
            run_id: Some(run_id.clone()),
            attempt,
            completion_token: task.completion_token,
            idempotency_key: task.idempotency_key,
            labels: task.labels.into_iter().collect(),
            heartbeat_window_ms,
            input: task.input.map(|payload| payload.bytes).unwrap_or_default(),
        };
        // A push-enqueue failure means the worker's connection was already gone
        // at push time — the same synchronous-failure contract as a closed gRPC
        // stream channel above.
        let awaiter = match delivery.push_dispatch(&request) {
            Ok(awaiter) => awaiter,
            Err(error) => {
                let reason = format!("worker liminal push failed: {error}");
                self.cleanup_activity(worker_id, workflow_id, activity_id, &completion_token);
                log_worker_error(
                    "WorkerChannelClosed",
                    &self.namespace,
                    activity_type,
                    workflow_id,
                    activity_id,
                    Some(worker_id),
                    &reason,
                );
                return Err(reason);
            }
        };
        // NOI-6: the attempt is live on `worker_id` from this push until the
        // router resolves it — bind it for exactly that window (the guard is
        // dropped when the router thread exits).
        let owner_binding = self.attempt_owners.as_ref().map(|owners| {
            super::liminal_transport::AttemptOwnerGuard::bind(
                owners.clone(),
                super::intervention::AttemptKey::new(
                    workflow_id.clone(),
                    run_id.clone(),
                    activity_id.clone(),
                    attempt,
                ),
                worker_id,
            )
        });
        self.spawn_liminal_reply_router(
            worker_id,
            awaiter,
            workflow_id,
            activity_id,
            &completion_token,
            owner_binding,
        );
        Ok(())
    }

    /// Waits (on a dedicated router thread, bounded by the dispatch's own
    /// lifetime) for the worker's correlated
    /// [`DispatchResponse`](super::liminal_transport::DispatchResponse) and
    /// re-enters it through the SAME completion bookkeeping the gRPC inbound
    /// stream applies (`process_inbound` in `worker_grpc.rs`): clear the
    /// in-flight liveness entry, wake any drain waiter, then resolve the
    /// bridge's pending map — result, failure, and retryable classification
    /// identical (the worker encodes the `retryable:`/`terminal:` reason
    /// vocabulary on the wire). An unmatched (already-resolved) REAL reply
    /// routes through the outbox delivery callback exactly like a late gRPC
    /// result.
    ///
    /// The #176 heartbeat sweeper covers this dispatch exactly as it covers a
    /// gRPC one: the dispatch is tracked in the shared [`HeartbeatTracker`] and
    /// the worker's runtime pumps automatic liveness beats over the reserved
    /// liminal channel (`WORKER_LIVENESS_CHANNEL`), so a healthy worker running
    /// an over-window activity is never falsely expired while a wedged one
    /// still is. Prompt worker-DEATH detection additionally rides the
    /// connection itself — the awaiter wakes with the typed Disconnected error
    /// the moment the connection closes, resolving the SAME retryable
    /// lost-worker failure the gRPC stream-teardown sweep reports.
    ///
    /// Two structural guards mirror the gRPC arm's tracker gating:
    ///
    /// - A SYNTHESIZED failure (disconnect / receive fault) is delivered only
    ///   when this router's own `complete_task` actually retired the tracked
    ///   entry — the same "fail only still-tracked tasks" gate
    ///   `remove_worker_tasks` gives the gRPC sweeps — so a dispatch already
    ///   resolved elsewhere (expiry sweep, shutdown drain, deregistered
    ///   fast path) never has a spurious failure injected for an ordinal whose
    ///   retry may be live on another worker.
    /// - The wait itself ends one reply-poll after the tracked entry
    ///   disappears, so an abandoned dispatch never parks this thread for the
    ///   remaining life of the worker's connection. A real reply arriving
    ///   AFTER that exit is dropped (the resolving path owns the ordinal — its
    ///   retry re-executes); this is the one deliberate divergence from the
    ///   gRPC arm, whose shared stream task routes any late result to the
    ///   outbox callback, and it is the safer half of the trade because a
    ///   stale attempt's result can never resolve a newer attempt's entry.
    #[cfg(feature = "liminal-transport")]
    fn spawn_liminal_reply_router(
        &self,
        worker_id: WorkerId,
        awaiter: liminal_server::server::connection::PushReplyAwaiter,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
        completion_token: &CompletionToken,
        owner_binding: Option<super::liminal_transport::AttemptOwnerGuard>,
    ) {
        let pending = self.pending.clone();
        let heartbeat_tracker = self.heartbeat_tracker.clone();
        let drain_state = self.drain_state.clone();
        let workflow_id = workflow_id.clone();
        let activity_id = activity_id.clone();
        let completion_token = completion_token.clone();
        std::thread::spawn(move || {
            // Owns the NOI-6 attempt binding for the dispatch's lifetime: it
            // drops (releasing the back-index entry) when this router exits,
            // on every path — reply, abandonment, disconnect, or panic.
            let _owner_binding = owner_binding;
            route_liminal_reply(
                &pending,
                &heartbeat_tracker,
                &drain_state,
                &awaiter,
                (worker_id, &workflow_id, &activity_id, &completion_token),
            );
        });
    }

    /// Block until the dispatch terminates (see the module docs for the
    /// exhaustive termination list). The wait is deliberately unbounded:
    /// the engine imposes no activity timeout of its own.
    fn await_activity_result(
        &self,
        context: &ActivityDispatchContext<'_>,
        rx: &SyncReceiver,
    ) -> Result<String, String> {
        // Close the dispatch/disconnect race before blocking. A worker whose
        // stream tore down *before* this dispatch tracked its task was swept
        // without this entry, so nothing would ever deliver through `rx`.
        // `fail_lost_worker` deregisters before it collects tasks, and this
        // dispatch tracked its task before sending, so: if the worker is
        // still registered here, any later sweep is guaranteed to include
        // this task and unblock the `recv` below.
        match self.registry.is_registered(context.worker_id) {
            Ok(true) => {}
            Ok(false) => {
                // A sweep that did include this task may have delivered
                // already; prefer its verdict (or a genuine result that
                // raced the disconnect) over fabricating one.
                if let Ok(result) = rx.try_recv() {
                    return self.deliver_result(context, result);
                }
                self.cleanup_activity(
                    context.worker_id,
                    context.workflow_id,
                    context.activity_id,
                    &context.completion_token,
                );
                // TRANSPORT domain, classified through the shared ledger: the
                // activity never executed, so the failure never wears the
                // action's retry vocabulary and is re-dispatched
                // attempt-neutrally until the transport's own budget is spent.
                let reason = self.pending.classify_worker_loss(
                    context.workflow_id,
                    context.activity_id,
                    context.worker_id,
                );
                log_worker_error(
                    "WorkerLost",
                    &self.namespace,
                    context.activity_type,
                    context.workflow_id,
                    context.activity_id,
                    Some(context.worker_id),
                    &reason,
                );
                return Err(reason);
            }
            Err(error) => {
                self.cleanup_activity(
                    context.worker_id,
                    context.workflow_id,
                    context.activity_id,
                    &context.completion_token,
                );
                let reason = format!("worker registry inspection failed: {error}");
                log_worker_error(
                    "WorkerRegistry",
                    &self.namespace,
                    context.activity_type,
                    context.workflow_id,
                    context.activity_id,
                    Some(context.worker_id),
                    &reason,
                );
                return Err(reason);
            }
        }
        if let Ok(result) = rx.recv() {
            return self.deliver_result(context, result);
        }
        // Every sender was dropped without completing: a cleanup path
        // removed the pending entry. Surface it instead of hanging.
        self.cleanup_activity(
            context.worker_id,
            context.workflow_id,
            context.activity_id,
            &context.completion_token,
        );
        let reason = "activity response channel dropped".to_owned();
        log_worker_error(
            "WorkerChannelClosed",
            &self.namespace,
            context.activity_type,
            context.workflow_id,
            context.activity_id,
            Some(context.worker_id),
            &reason,
        );
        Err(reason)
    }

    fn deliver_result(
        &self,
        context: &ActivityDispatchContext<'_>,
        result: Result<String, String>,
    ) -> Result<String, String> {
        self.pending
            .pending
            .remove(&(context.workflow_id.clone(), context.activity_id.clone()));
        // A parked dispatch (#207) is not a failure: the server is draining and
        // restart recovery re-dispatches the ordinal. Info, never error — a
        // routine deploy must not emit an ActivityFailed log per in-flight
        // dispatch (the incident's alarm noise).
        if let Err(reason) = &result
            && aion::is_parked_reason(reason)
        {
            tracing::info!(
                operation = "activity_dispatch",
                namespace = %self.namespace,
                workflow_id = %context.workflow_id,
                activity_id = %context.activity_id,
                activity_type = context.activity_type,
                worker_id = ?context.worker_id,
                "activity parked for restart recovery"
            );
            return result;
        }
        log_activity_completion(context, result.is_ok());
        result.inspect_err(|reason| {
            log_worker_error(
                "ActivityFailed",
                &self.namespace,
                context.activity_type,
                context.workflow_id,
                context.activity_id,
                Some(context.worker_id),
                reason,
            );
        })
    }
}

impl ActivityDispatcher for WorkerActivityDispatcher {
    fn dispatch(&self, request: ActivityDispatch) -> Result<String, String> {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => match handle.runtime_flavor() {
                tokio::runtime::RuntimeFlavor::MultiThread => {
                    // We are inside a tokio runtime (the engine spawns the
                    // sync dispatch onto its handle). Hand this worker's
                    // scheduler core to another thread before blocking so the
                    // stream forwarder woken by our `try_send` can actually
                    // run — otherwise it is trapped in this worker's
                    // non-stealable LIFO slot for as long as we block.
                    tokio::task::block_in_place(|| self.dispatch_blocking(request))
                }
                flavor => Err(format!(
                    "activity dispatch blocks the calling thread until the worker responds; \
                     a {flavor:?} tokio runtime cannot host that wait because the worker \
                     stream forwarder shares its only executor thread and the task could \
                     never be delivered — run the engine on a multi-thread tokio runtime"
                )),
            },
            // No tokio context: a beamr scheduler thread or other plain OS
            // thread. Blocking here is the designed contract and cannot starve
            // the server runtime.
            Err(_) => self.dispatch_blocking(request),
        }
    }
}

impl WorkerActivityDispatcher {
    /// Dispatch the activity and block the calling thread until the worker
    /// responds, the worker is declared lost, or the server drains (see the
    /// module docs for the exhaustive termination list).
    ///
    /// The request carries the *real* workflow and activity ids the engine
    /// recorded in history, so the worker logs, the pending-completion key,
    /// and the heartbeat tracker all correlate directly against the event
    /// store. `config` is forwarded by the engine seam but not yet consumed
    /// here (the retry executor that reads it is unbuilt).
    ///
    /// Must never run while the calling thread still owns a tokio scheduler
    /// core: the response can only arrive after the runtime's stream
    /// forwarder flushes the queued [`WorkerMessage::ActivityTask`] to the
    /// worker, so the thread blocking here must not be the one responsible
    /// for polling that forwarder. [`ActivityDispatcher::dispatch`] enforces
    /// this with `tokio::task::block_in_place`.
    fn dispatch_blocking(&self, request: ActivityDispatch) -> Result<String, String> {
        let ActivityDispatch {
            namespace,
            task_queue,
            // OPTIONAL within-pool node affinity (NODE-4): `Some(n)` pins this
            // dispatch to workers advertising node `n` (require semantics);
            // `None` is unpinned and reaches any worker in the pool.
            node,
            workflow_id,
            run_id,
            activity_id,
            name,
            input,
            config: _,
            attempt,
            labels,
            // R5 advisory class: engine-side only. The wire carries no such
            // field and a worker's behaviour is identical either way — the
            // class governs how the ENGINE treats the failure, never how the
            // work is done.
            advisory: _,
        } = request;
        let started_at = Instant::now();
        self.ensure_accepting(&namespace, &name, &workflow_id, &activity_id, None)?;
        let address = ServiceAddress {
            namespace: namespace.clone(),
            task_queue: task_queue.clone(),
            activity_type: name.clone(),
            node: node.clone(),
        };
        let worker = self.select_worker_or_wait(&address, &workflow_id, &activity_id)?;
        let worker_id = worker.id();
        let span = info_span!(
            "activity_dispatch",
            operation = "activity_dispatch",
            namespace = %namespace,
            task_queue = %task_queue,
            node = node.as_deref(),
            workflow_id = %workflow_id,
            activity_id = %activity_id,
            activity_type = %name,
            worker_id = ?worker_id,
        );
        let _span_guard = span.enter();
        self.ensure_accepting(
            &namespace,
            &name,
            &workflow_id,
            &activity_id,
            Some(worker_id),
        )?;

        let (completion_token, rx) = self
            .pending
            .insert(workflow_id.clone(), activity_id.clone())
            .map_err(|error| error.to_string())?;
        let task = activity_task(
            &name,
            &input,
            (&workflow_id, &run_id, &activity_id),
            attempt,
            labels,
            &completion_token,
        );
        if let Err(error) = self.track_worker_task(
            worker_id,
            &name,
            &workflow_id,
            &activity_id,
            attempt,
            completion_token.clone(),
        ) {
            self.cleanup_activity(worker_id, &workflow_id, &activity_id, &completion_token);
            return Err(error);
        }
        self.send_activity_task(
            &worker,
            task,
            &address,
            &workflow_id,
            &activity_id,
            &completion_token,
        )?;
        let context = ActivityDispatchContext {
            namespace: &namespace,
            activity_type: &name,
            worker_id,
            workflow_id: &workflow_id,
            activity_id: &activity_id,
            completion_token,
            started_at,
        };
        self.await_activity_result(&context, &rx)
    }
}

/// Body of one liminal reply-router thread (see
/// [`WorkerActivityDispatcher::spawn_liminal_reply_router`] for the contract).
///
/// Resolves by the key THIS push dispatched (the awaiter is already
/// correlation-scoped to it), never by the reply's echoed ids: a buggy echo
/// must not cross executions.
#[cfg(feature = "liminal-transport")]
fn route_liminal_reply(
    pending: &PendingActivities,
    heartbeat_tracker: &HeartbeatTracker,
    drain_state: &DrainState,
    awaiter: &liminal_server::server::connection::PushReplyAwaiter,
    execution: (WorkerId, &WorkflowId, &ActivityId, &CompletionToken),
) {
    let (worker_id, workflow_id, activity_id, current_token) = execution;
    // The wait re-arms only while this dispatch is still tracked in-flight, so
    // a dispatch resolved elsewhere (expiry sweep, shutdown drain, cleanup)
    // releases this thread within one reply poll instead of parking it for the
    // remaining life of the worker's connection.
    let waited = super::liminal_transport::receive_bridge_reply(awaiter, || {
        heartbeat_tracker
            .is_tracked(worker_id, workflow_id, activity_id)
            .unwrap_or(false)
    });
    // `synthesized` marks a failure this router FABRICATED (disconnect or
    // receive fault) as opposed to a real worker reply: only fabricated
    // failures are gated on the tracker below.
    let (run_id, submitted_token, outcome, synthesized) = match waited {
        Ok(Some(response)) => {
            let submitted_token = match CompletionToken::from_wire(
                workflow_id,
                activity_id,
                response.completion_token,
            ) {
                Ok(token) => token,
                Err(error) => {
                    tracing::warn!(
                        worker_id = ?worker_id,
                        workflow_id = %workflow_id,
                        activity_id = %activity_id,
                        %error,
                        "liminal activity completion omitted its generation proof"
                    );
                    return;
                }
            };
            (response.run_id, submitted_token, response.outcome, false)
        }
        Ok(None) => {
            tracing::debug!(
                worker_id = ?worker_id,
                workflow_id = %workflow_id,
                activity_id = %activity_id,
                "liminal dispatch resolved by another path; abandoning reply wait"
            );
            return;
        }
        Err(error) if error.is_worker_connection_lost() => (
            None,
            current_token.clone(),
            // TRANSPORT domain (the worker's connection closed before it
            // replied): classified through the shared ledger, never as an
            // action failure.
            Err(pending.classify_worker_loss(workflow_id, activity_id, worker_id)),
            true,
        ),
        Err(error) => (
            None,
            current_token.clone(),
            Err(format!("retryable:worker liminal reply failed: {error}")),
            true,
        ),
    };
    // The gRPC sweeps fail only still-tracked tasks (`remove_worker_tasks`);
    // this is the same structural gate: `complete_task` reports whether THIS
    // call retired the tracked entry. A poisoned tracker fails open (deliver)
    // so the blocked dispatch thread is never left hanging on a broken lock.
    if synthesized {
        let was_tracked =
            complete_liminal_tracking(heartbeat_tracker, worker_id, workflow_id, activity_id);
        if !was_tracked {
            // Another path already resolved this dispatch (and notified drain):
            // injecting the fabricated lost-worker failure now could reach a retry.
            tracing::debug!(
                worker_id = ?worker_id,
                workflow_id = %workflow_id,
                activity_id = %activity_id,
                "liminal dispatch already resolved; dropping synthesized lost-worker failure"
            );
            return;
        }
    }
    if let Err(error) = pending.complete_fenced(
        workflow_id,
        activity_id,
        run_id.as_ref(),
        &submitted_token,
        outcome,
    ) {
        tracing::warn!(
            worker_id = ?worker_id,
            workflow_id = %workflow_id,
            activity_id = %activity_id,
            %error,
            "liminal activity completion handoff rejected"
        );
        return;
    }
    if !synthesized
        && let Err(error) = heartbeat_tracker.complete_task(worker_id, workflow_id, activity_id)
    {
        tracing::error!(
            worker_id = ?worker_id,
            workflow_id = %workflow_id,
            activity_id = %activity_id,
            %error,
            "failed to clear in-flight tracking for completed liminal activity"
        );
    }
    drain_state.notify_activity_drained();
}

#[cfg(feature = "liminal-transport")]
fn complete_liminal_tracking(
    heartbeat_tracker: &HeartbeatTracker,
    worker_id: WorkerId,
    workflow_id: &WorkflowId,
    activity_id: &ActivityId,
) -> bool {
    heartbeat_tracker
        .complete_task(worker_id, workflow_id, activity_id)
        .unwrap_or_else(|error| {
            tracing::error!(
                worker_id = ?worker_id,
                workflow_id = %workflow_id,
                activity_id = %activity_id,
                %error,
                "failed to clear in-flight tracking for completed liminal activity"
            );
            true
        })
}

struct ActivityDispatchContext<'a> {
    namespace: &'a str,
    activity_type: &'a str,
    worker_id: WorkerId,
    workflow_id: &'a WorkflowId,
    activity_id: &'a ActivityId,
    completion_token: CompletionToken,
    started_at: Instant,
}

fn activity_task(
    activity_type: &str,
    input: &str,
    execution: (&WorkflowId, &RunId, &ActivityId),
    attempt: u32,
    labels: BTreeMap<String, String>,
    completion_token: &CompletionToken,
) -> ProtoActivityTask {
    let (workflow_id, run_id, activity_id) = execution;
    ProtoActivityTask {
        workflow_id: Some(ProtoWorkflowId::from(workflow_id.clone())),
        activity_id: Some(ProtoActivityId::from(activity_id.clone())),
        activity_type: activity_type.to_owned(),
        input: Some(ProtoPayload {
            content_type: String::from("application/json"),
            bytes: input.as_bytes().to_vec(),
        }),
        attempt,
        labels: labels.into_iter().collect(),
        run_id: Some(run_id.clone().into()),
        completion_token: completion_token.as_str().to_owned(),
        idempotency_key: idempotency_key(workflow_id, run_id, activity_id),
    }
}

fn log_activity_completion(context: &ActivityDispatchContext<'_>, succeeded: bool) {
    let duration_ms = duration_ms(context.started_at.elapsed());
    tracing::info!(
        operation = "activity_complete",
        namespace = context.namespace,
        workflow_id = %context.workflow_id,
        activity_id = %context.activity_id,
        activity_type = context.activity_type,
        worker_id = ?context.worker_id,
        duration_ms,
        outcome = if succeeded { "succeeded" } else { "failed" },
        "activity completed"
    );
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn log_worker_error(
    error_type: &'static str,
    namespace: &str,
    activity_type: &str,
    workflow_id: &WorkflowId,
    activity_id: &ActivityId,
    worker_id: Option<super::registry::WorkerId>,
    reason: &str,
) {
    tracing::error!(
        operation = "activity_dispatch",
        namespace,
        workflow_id = %workflow_id,
        activity_id = %activity_id,
        activity_type,
        worker_id = ?worker_id,
        error_type,
        reason,
        "worker interaction failed"
    );
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use aion_core::{ActivityError, ActivityErrorKind, ContentType, Payload};

    use super::*;

    fn activity_id(pos: u64) -> ActivityId {
        ActivityId::from_sequence_position(pos)
    }

    #[test]
    fn pending_insert_and_complete_delivers_result() -> Result<(), ServerError> {
        let pending = PendingActivities::new(TEST_HEARTBEAT_WINDOW);
        let workflow_id = WorkflowId::new_v4();
        let id = activity_id(1);
        let rx = pending.insert(workflow_id.clone(), id.clone())?.1;

        assert!(pending.complete(&workflow_id, &id, None, Ok("done".to_owned())));
        assert_eq!(
            rx.recv_timeout(Duration::from_millis(50)),
            Ok(Ok("done".to_owned()))
        );
        Ok(())
    }

    /// #246 / #254 red: retrying an activity while the same worker still holds
    /// the prior attempt must never replace its responder silently. Today's
    /// attempt-free `DashMap::insert` drops the first sender, producing the
    /// terminal "activity response channel dropped" specimen.
    #[test]
    fn same_execution_collision_is_a_named_typed_refusal() -> Result<(), ServerError> {
        let pending = PendingActivities::new(TEST_HEARTBEAT_WINDOW);
        let workflow_id = WorkflowId::new_v4();
        let id = activity_id(2);
        let _first = pending.insert(workflow_id.clone(), id.clone())?;

        let error = pending.insert(workflow_id, id).err().ok_or_else(|| {
            ServerError::worker_dispatch(
                "test",
                "same-worker-collision",
                "a live responder collision was silently replaced",
            )
        })?;

        assert!(
            error.to_string().contains("pending activity collision"),
            "{error}"
        );
        Ok(())
    }

    #[test]
    fn pending_complete_unknown_returns_false() {
        let pending = PendingActivities::new(TEST_HEARTBEAT_WINDOW);
        assert!(!pending.complete(
            &WorkflowId::new_v4(),
            &activity_id(99),
            None,
            Ok("orphan".to_owned())
        ));
    }

    #[derive(Default)]
    struct RecordingOutboxCallback {
        completions: Mutex<Vec<(WorkflowId, ActivityId, String)>>,
        failures: Mutex<Vec<(WorkflowId, ActivityId, String)>>,
        live: bool,
    }

    impl OutboxDeliveryCallback for RecordingOutboxCallback {
        fn deliver_completion(
            &self,
            workflow_id: &WorkflowId,
            activity_id: &ActivityId,
            run_id: Option<&RunId>,
            result: String,
        ) -> Result<bool, ServerError> {
            let _ = run_id;
            self.completions
                .lock()
                .map_err(|_| ServerError::lock_poisoned("recording outbox callback"))?
                .push((workflow_id.clone(), activity_id.clone(), result));
            Ok(self.live)
        }

        fn deliver_failure(
            &self,
            workflow_id: &WorkflowId,
            activity_id: &ActivityId,
            run_id: Option<&RunId>,
            reason: String,
        ) -> Result<bool, ServerError> {
            let _ = run_id;
            self.failures
                .lock()
                .map_err(|_| ServerError::lock_poisoned("recording outbox callback"))?
                .push((workflow_id.clone(), activity_id.clone(), reason));
            Ok(self.live)
        }
    }

    #[test]
    fn unmatched_completion_routes_to_outbox_callback_when_installed() -> Result<(), ServerError> {
        let pending = PendingActivities::new(TEST_HEARTBEAT_WINDOW);
        let callback = Arc::new(RecordingOutboxCallback {
            live: true,
            ..RecordingOutboxCallback::default()
        });
        // Install on one clone; the wiring must be visible to every clone.
        pending.clone().set_outbox_delivery(callback.clone());

        let workflow_id = WorkflowId::new_v4();
        let id = activity_id(7);

        // No pending entry: the completion is unmatched and must route to the
        // callback rather than being dropped. A live workflow reports true.
        assert!(pending.complete(&workflow_id, &id, None, Ok("done".to_owned())));
        let completions = callback
            .completions
            .lock()
            .map_err(|_| ServerError::lock_poisoned("recording outbox callback"))?;
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].0, workflow_id);
        assert_eq!(completions[0].1, id);
        assert_eq!(completions[0].2, "done");
        Ok(())
    }

    #[test]
    fn unmatched_failure_routes_to_outbox_callback_and_not_live_reports_false()
    -> Result<(), ServerError> {
        let pending = PendingActivities::new(TEST_HEARTBEAT_WINDOW);
        // live = false models the expected stale-completion case.
        let callback = Arc::new(RecordingOutboxCallback::default());
        pending.set_outbox_delivery(callback.clone());

        let workflow_id = WorkflowId::new_v4();
        let id = activity_id(8);

        assert!(!pending.complete(&workflow_id, &id, None, Err("retryable:boom".to_owned())));
        let failures = callback
            .failures
            .lock()
            .map_err(|_| ServerError::lock_poisoned("recording outbox callback"))?;
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].2, "retryable:boom");
        Ok(())
    }

    #[test]
    fn unmatched_completion_is_silent_drop_when_no_callback_installed() {
        // Flag-off byte-identical behaviour: no callback, unmatched returns
        // false (silent drop) exactly as before.
        let pending = PendingActivities::new(TEST_HEARTBEAT_WINDOW);
        assert!(!pending.complete(
            &WorkflowId::new_v4(),
            &activity_id(9),
            None,
            Ok("x".to_owned())
        ));
    }

    #[test]
    fn matched_completion_never_reaches_outbox_callback() -> Result<(), ServerError> {
        let pending = PendingActivities::new(TEST_HEARTBEAT_WINDOW);
        let callback = Arc::new(RecordingOutboxCallback {
            live: true,
            ..RecordingOutboxCallback::default()
        });
        pending.set_outbox_delivery(callback.clone());

        let workflow_id = WorkflowId::new_v4();
        let id = activity_id(10);
        let rx = pending.insert(workflow_id.clone(), id.clone())?.1;

        assert!(pending.complete(&workflow_id, &id, None, Ok("matched".to_owned())));
        assert_eq!(
            rx.recv_timeout(Duration::from_millis(50)),
            Ok(Ok("matched".to_owned()))
        );
        assert!(
            callback
                .completions
                .lock()
                .map_err(|_| ServerError::lock_poisoned("recording outbox callback"))?
                .is_empty(),
            "a matched completion must deliver to its waiter, not the outbox callback"
        );
        Ok(())
    }

    /// #207: parking resolves the matched waiter with the ephemeral parked
    /// sentinel — the exact string the engine's retry loop classifies as
    /// `Parked` — and nothing else.
    #[test]
    fn park_activity_resolves_matched_waiter_with_the_parked_sentinel() -> Result<(), ServerError> {
        let pending = PendingActivities::new(TEST_HEARTBEAT_WINDOW);
        let workflow_id = WorkflowId::new_v4();
        let id = activity_id(11);
        let rx = pending.insert(workflow_id.clone(), id.clone())?.1;

        pending.park_activity(&workflow_id, &id)?;
        let result = rx
            .recv_timeout(Duration::from_millis(50))
            .map_err(|e| ServerError::worker_dispatch("", "", format!("channel: {e}")))?;
        assert_eq!(result, Err(aion::PARKED_ACTIVITY_REASON.to_owned()));
        Ok(())
    }

    /// #207: an unmatched park is a no-op and is NEVER routed to the outbox
    /// delivery callback — a park is not a failure and must never reach a
    /// workflow.
    #[test]
    fn unmatched_park_is_a_noop_and_never_reaches_the_outbox_callback() -> Result<(), ServerError> {
        let pending = PendingActivities::new(TEST_HEARTBEAT_WINDOW);
        let callback = Arc::new(RecordingOutboxCallback {
            live: true,
            ..RecordingOutboxCallback::default()
        });
        pending.set_outbox_delivery(callback.clone());

        pending.park_activity(&WorkflowId::new_v4(), &activity_id(12))?;

        assert!(
            callback
                .failures
                .lock()
                .map_err(|_| ServerError::lock_poisoned("recording outbox callback"))?
                .is_empty(),
            "a park must never be delivered as an outbox failure"
        );
        assert!(
            callback
                .completions
                .lock()
                .map_err(|_| ServerError::lock_poisoned("recording outbox callback"))?
                .is_empty(),
            "a park must never be delivered as an outbox completion"
        );
        Ok(())
    }

    #[test]
    fn completion_sink_routes_success() -> Result<(), ServerError> {
        let pending = PendingActivities::new(TEST_HEARTBEAT_WINDOW);
        let workflow_id = WorkflowId::new_v4();
        let id = activity_id(2);
        let (completion_token, rx) = pending.insert(workflow_id.clone(), id.clone())?;
        let payload = Payload::new(ContentType::Json, br#"{"greeting":"hi"}"#.to_vec());

        pending.complete_activity(ActivityCompletion {
            workflow_id,
            activity_id: id,
            run_id: None,
            completion_token,
            outcome: ActivityCompletionOutcome::Succeeded(payload),
        })?;

        let result = rx
            .recv_timeout(Duration::from_millis(50))
            .map_err(|e| ServerError::worker_dispatch("", "", format!("channel: {e}")))?;
        assert_eq!(result, Ok(r#"{"greeting":"hi"}"#.to_owned()));
        Ok(())
    }

    #[test]
    fn malformed_payload_does_not_consume_the_current_generation() -> Result<(), ServerError> {
        let pending = PendingActivities::new(TEST_HEARTBEAT_WINDOW);
        let workflow_id = WorkflowId::new_v4();
        let id = activity_id(12);
        let (completion_token, rx) = pending.insert(workflow_id.clone(), id.clone())?;

        let malformed = pending.complete_activity(ActivityCompletion {
            workflow_id: workflow_id.clone(),
            activity_id: id.clone(),
            run_id: None,
            completion_token: completion_token.clone(),
            outcome: ActivityCompletionOutcome::Succeeded(Payload::new(
                ContentType::Json,
                vec![0xff],
            )),
        });
        assert!(matches!(malformed, Err(ServerError::WorkerDispatch { .. })));
        assert!(
            rx.try_recv().is_err(),
            "an invalid result must leave the waiter unresolved"
        );

        pending.complete_activity(ActivityCompletion {
            workflow_id,
            activity_id: id,
            run_id: None,
            completion_token,
            outcome: ActivityCompletionOutcome::Succeeded(Payload::new(
                ContentType::Json,
                br#""valid""#.to_vec(),
            )),
        })?;
        let result = rx
            .recv_timeout(Duration::from_millis(50))
            .map_err(|error| ServerError::worker_dispatch("", "", format!("channel: {error}")))?;
        assert_eq!(result, Ok(r#""valid""#.to_owned()));
        Ok(())
    }

    #[test]
    fn completion_sink_routes_retryable_error() -> Result<(), ServerError> {
        let pending = PendingActivities::new(TEST_HEARTBEAT_WINDOW);
        let workflow_id = WorkflowId::new_v4();
        let id = activity_id(3);
        let (completion_token, rx) = pending.insert(workflow_id.clone(), id.clone())?;

        pending.complete_activity(ActivityCompletion {
            workflow_id,
            activity_id: id,
            run_id: None,
            completion_token,
            outcome: ActivityCompletionOutcome::Failed(ActivityError {
                kind: ActivityErrorKind::Retryable,
                message: "temporary".to_owned(),
                details: None,
            }),
        })?;

        let result = rx
            .recv_timeout(Duration::from_millis(50))
            .map_err(|e| ServerError::worker_dispatch("", "", format!("channel: {e}")))?;
        assert_eq!(result, Err("retryable:temporary".to_owned()));
        Ok(())
    }

    /// Regression test (#59, brief D12): pending tracking must be keyed by
    /// the full `(WorkflowId, ActivityId)` pair. The dispatcher fabricates
    /// activity ids from a process-local counter that resets on server
    /// restart, so a stale result re-reported from a worker's previous
    /// session carries the same bare `ActivityId` as a fresh post-restart
    /// dispatch. Under bare-`ActivityId` keying the stale result completed
    /// the wrong execution; with pair keying it is dropped and the genuine
    /// result still completes.
    #[test]
    fn stale_result_for_other_workflow_does_not_complete_pending_dispatch()
    -> Result<(), ServerError> {
        let pending = PendingActivities::new(TEST_HEARTBEAT_WINDOW);
        let post_restart_workflow = WorkflowId::new_v4();
        let pre_restart_workflow = WorkflowId::new_v4();
        // Counter resets to the same sequence position after restart.
        let id = activity_id(1);
        let (completion_token, rx) = pending.insert(post_restart_workflow.clone(), id.clone())?;

        // Stale pre-restart result: same activity id, different workflow.
        let rejected = pending.complete_activity(ActivityCompletion {
            workflow_id: pre_restart_workflow,
            activity_id: id.clone(),
            run_id: None,
            completion_token: CompletionToken::for_test(),
            outcome: ActivityCompletionOutcome::Succeeded(Payload::new(
                ContentType::Json,
                br#""stale""#.to_vec(),
            )),
        });
        assert!(matches!(
            rejected,
            Err(ServerError::ActivityCompletionRejected { .. })
        ));
        assert!(
            rx.try_recv().is_err(),
            "stale result for a different workflow must not complete this dispatch"
        );

        // The genuine result for the pending execution still completes.
        pending.complete_activity(ActivityCompletion {
            workflow_id: post_restart_workflow,
            activity_id: id,
            run_id: None,
            completion_token,
            outcome: ActivityCompletionOutcome::Succeeded(Payload::new(
                ContentType::Json,
                br#""fresh""#.to_vec(),
            )),
        })?;
        let result = rx
            .recv_timeout(Duration::from_millis(50))
            .map_err(|e| ServerError::worker_dispatch("", "", format!("channel: {e}")))?;
        assert_eq!(result, Ok(r#""fresh""#.to_owned()));
        Ok(())
    }

    /// Liveness tracker for dispatcher unit tests; the window only matters
    /// to expiry checks, which nothing in these tests drives.
    /// The heartbeat window these unit tests wire, named once so the
    /// transport-loss budget they run under is explicit rather than inherited.
    /// It is the same window [`test_tracker`] uses, so a sink and a tracker
    /// built side by side in a test agree exactly as they do in production.
    const TEST_HEARTBEAT_WINDOW: Duration = Duration::from_secs(5);

    /// The budget a sink ends up with is DERIVED from the operator's window,
    /// and this is the arithmetic — not prose about it.
    ///
    /// Before #100 the wiring was `default().with_heartbeat_window(w)`: two
    /// calls where the second was omissible, and deleting it from all five
    /// production sites reddened nothing, because every test ran the zero-budget
    /// default. This pin and its sibling below are what make that deletion
    /// impossible to perform silently.
    #[test]
    fn a_sink_derives_its_transport_budget_from_the_operator_window() {
        let window = Duration::from_secs(7);
        let sink = PendingActivities::new(window);

        assert_eq!(
            sink.transport_losses().budget(),
            window * super::super::transport_loss::TRANSPORT_LOSS_BUDGET_WINDOWS,
            "the transport-loss budget must be the operator's heartbeat window times \
             TRANSPORT_LOSS_BUDGET_WINDOWS; a sink carrying any other budget is one no \
             operator declared"
        );
    }

    /// The wiring itself, which is where the defect actually lived: a dispatcher
    /// is handed a `HeartbeatTracker` and must not be able to carry a transport
    /// budget that disagrees with it.
    ///
    /// This asserts on the dispatcher a caller really builds, so it sits on the
    /// path a forgetful wiring would take rather than on the constructor in
    /// isolation.
    #[test]
    fn a_dispatcher_carries_the_transport_budget_of_the_tracker_it_was_given() {
        let window = Duration::from_secs(11);
        let registry = ConnectedWorkerRegistry::default();
        let dispatcher =
            WorkerActivityDispatcher::new(registry, "default", HeartbeatTracker::new(window));

        assert_eq!(
            dispatcher.pending().transport_losses().budget(),
            window * super::super::transport_loss::TRANSPORT_LOSS_BUDGET_WINDOWS,
            "the dispatcher's transport budget must be derived from the heartbeat window it \
             was constructed with; a zero or mismatched budget here means a worker loss is \
             declared transport-exhausted after ONE re-dispatch instead of after the \
             operator's window"
        );
        assert_ne!(
            dispatcher.pending().transport_losses().budget(),
            Duration::ZERO,
            "a ZERO budget is the specific regression this pin exists for: it grants exactly \
             one re-dispatchable loss and then declares the infrastructure flapping"
        );
    }

    fn test_tracker() -> HeartbeatTracker {
        HeartbeatTracker::new(TEST_HEARTBEAT_WINDOW)
    }

    /// A `greet` dispatch request carrying real (test-synthesized) ids, the
    /// engine-seam shape `WorkerActivityDispatcher::dispatch` now consumes.
    fn greet_request() -> ActivityDispatch {
        ActivityDispatch {
            namespace: "default".to_owned(),
            task_queue: "default".to_owned(),
            node: None,
            workflow_id: WorkflowId::new_v4(),
            run_id: RunId::new_v4(),
            activity_id: ActivityId::from_sequence_position(0),
            name: "greet".to_owned(),
            input: "{}".to_owned(),
            config: "{}".to_owned(),
            attempt: 1,
            labels: std::collections::BTreeMap::new(),
            advisory: false,
        }
    }

    #[test]
    fn dispatcher_fails_immediately_when_draining_without_workers() {
        let registry = ConnectedWorkerRegistry::default();
        let drain = DrainState::default();
        let dispatcher = WorkerActivityDispatcher::new(registry, "default", test_tracker())
            .with_drain_state(drain.clone());

        let _ = drain.begin();

        let result = dispatcher.dispatch(greet_request());

        assert!(result.is_err());
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains("drain"),
            "expected drain rejection, got: {err}"
        );
    }

    /// Regression test for the production stall where every remote activity
    /// timed out: the engine invoked the sync `dispatch` from inside a
    /// spawned tokio task (`futures::future::lazy` polled on a runtime
    /// worker), and the woken stream-consumer task landed in that blocked
    /// worker's non-stealable LIFO slot, so the queued `ActivityTask` was
    /// only delivered when the then-extant 30s dispatch timeout fired (the
    /// dispatch wait is unbounded today; the stall would now be a hang).
    ///
    /// Mirrors the real wiring minus tonic: the real registry channel that
    /// the gRPC stream forwarder drains, a worker task awaiting that channel
    /// on the same runtime, completion through the production
    /// `ActivityCompletionSink`, and the sync dispatch invoked from a
    /// runtime worker task — the worst case the `block_in_place` guard in
    /// `dispatch` defends against (the engine itself now routes through
    /// `dispatch_async`, off the async workers).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dispatch_inside_runtime_task_delivers_promptly_and_round_trips()
    -> Result<(), Box<dyn std::error::Error>> {
        let registry = ConnectedWorkerRegistry::default();
        let pending = PendingActivities::new(TEST_HEARTBEAT_WINDOW);
        let (worker_tx, mut worker_rx) = tokio::sync::mpsc::channel(32);
        let activity_types = [String::from("greet")];
        let registration = registry.register("default", activity_types.iter(), worker_tx)?;

        let sink = pending.clone();
        let echo_worker = tokio::spawn(async move {
            let Some(WorkerMessage::ActivityTask(task)) = worker_rx.recv().await else {
                return Err("expected an activity task on the worker channel".to_owned());
            };
            let workflow_id = task
                .workflow_id
                .ok_or("task missing workflow id")
                .and_then(|id| WorkflowId::try_from(id).map_err(|_| "bad workflow id"))?;
            let activity_id = task
                .activity_id
                .map(ActivityId::from)
                .ok_or("task missing activity id")?;
            let completion_token =
                CompletionToken::from_wire(&workflow_id, &activity_id, task.completion_token)
                    .map_err(|error| error.to_string())?;
            sink.complete_activity(ActivityCompletion {
                workflow_id,
                activity_id,
                run_id: None,
                completion_token,
                outcome: ActivityCompletionOutcome::Succeeded(Payload::new(
                    ContentType::Json,
                    br#"{"greeting":"hello"}"#.to_vec(),
                )),
            })
            .map_err(|error| error.to_string())
        });

        let dispatcher = Arc::new(
            WorkerActivityDispatcher::new(registry, "default", test_tracker())
                .with_pending(pending),
        );
        let started = Instant::now();
        // Invoke the sync dispatch inside the first poll of a spawned task:
        // the worst-case calling context for the `block_in_place` guard.
        let dispatch_task = tokio::spawn(futures::future::lazy(move |_| {
            dispatcher.dispatch(greet_request())
        }));
        let result = dispatch_task.await.map_err(|error| error.to_string())?;
        let elapsed = started.elapsed();

        assert_eq!(result, Ok(r#"{"greeting":"hello"}"#.to_owned()));
        assert!(
            elapsed < Duration::from_secs(5),
            "dispatch round trip took {elapsed:?}; task delivery must not \
             depend on the blocked dispatch thread"
        );
        echo_worker.await.map_err(|error| error.to_string())??;
        registration.deregister()?;
        Ok(())
    }

    /// A current-thread runtime cannot host the blocking wait (the stream
    /// forwarder would share its only executor thread), so dispatch must
    /// fail fast with a precise error instead of blocking forever.
    #[tokio::test]
    async fn dispatch_on_current_thread_runtime_fails_fast()
    -> Result<(), Box<dyn std::error::Error>> {
        let registry = ConnectedWorkerRegistry::default();
        let (worker_tx, _worker_rx) = tokio::sync::mpsc::channel(32);
        let activity_types = [String::from("greet")];
        let registration = registry.register("default", activity_types.iter(), worker_tx)?;
        let dispatcher = WorkerActivityDispatcher::new(registry, "default", test_tracker());

        let started = Instant::now();
        let result = dispatcher.dispatch(greet_request());
        let elapsed = started.elapsed();

        let err = result.err().ok_or("expected dispatch to fail")?;
        assert!(
            err.contains("multi-thread tokio runtime"),
            "unexpected error: {err}"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "fail-fast path took {elapsed:?}"
        );
        registration.deregister()?;
        Ok(())
    }

    /// Bridge-level mirror of the e2e node-pin proof: two workers share the
    /// `(namespace, task_queue)` pool but advertise different nodes; an
    /// `ActivityDispatch` pinned to one node must reach ONLY the worker on that
    /// node through the live engine-seam `WorkerActivityDispatcher`. This is the
    /// regression guard for the bridge discarding the dispatch's `node`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dispatch_pinned_to_node_reaches_only_that_node()
    -> Result<(), Box<dyn std::error::Error>> {
        let registry = ConnectedWorkerRegistry::default();
        let pending = PendingActivities::new(TEST_HEARTBEAT_WINDOW);
        let activity_types = [String::from("greet")];
        let (n1_tx, mut n1_rx) = tokio::sync::mpsc::channel(32);
        let (n2_tx, mut n2_rx) = tokio::sync::mpsc::channel(32);
        // Register the DECOY (n2) FIRST so it owns the lowest worker id. The
        // bridge's `select_worker` picks the lowest-id matching worker, so a
        // bridge that DISCARDED the node would route to n2 (the decoy) here —
        // the n1 echo would never fire and the round trip would time out. With
        // the node threaded through, selection is filtered to n1.
        let on_n2 = registry.register_namespaces(
            [String::from("default")],
            "default",
            Some(String::from("n2")),
            activity_types.iter(),
            n2_tx,
        )?;
        let on_n1 = registry.register_namespaces(
            [String::from("default")],
            "default",
            Some(String::from("n1")),
            activity_types.iter(),
            n1_tx,
        )?;

        // Echo only on the n1 channel: the dispatch can only complete if the
        // task was routed to n1. If it leaked to n2, the n1 wait would stall and
        // the round trip below would time out instead.
        let sink = pending.clone();
        let echo_n1 = tokio::spawn(async move {
            let Some(WorkerMessage::ActivityTask(task)) = n1_rx.recv().await else {
                return Err("expected an activity task on the n1 worker channel".to_owned());
            };
            let workflow_id = task
                .workflow_id
                .ok_or("task missing workflow id")
                .and_then(|id| WorkflowId::try_from(id).map_err(|_| "bad workflow id"))?;
            let activity_id = task
                .activity_id
                .map(ActivityId::from)
                .ok_or("task missing activity id")?;
            let completion_token =
                CompletionToken::from_wire(&workflow_id, &activity_id, task.completion_token)
                    .map_err(|error| error.to_string())?;
            sink.complete_activity(ActivityCompletion {
                workflow_id,
                activity_id,
                run_id: None,
                completion_token,
                outcome: ActivityCompletionOutcome::Succeeded(Payload::new(
                    ContentType::Json,
                    br#"{"greeting":"hello"}"#.to_vec(),
                )),
            })
            .map_err(|error| error.to_string())
        });

        let dispatcher = Arc::new(
            WorkerActivityDispatcher::new(registry.clone(), "default", test_tracker())
                .with_pending(pending),
        );

        let pinned = ActivityDispatch {
            node: Some(String::from("n1")),
            ..greet_request()
        };
        let started = Instant::now();
        let result = tokio::spawn(futures::future::lazy(move |_| dispatcher.dispatch(pinned)))
            .await
            .map_err(|error| error.to_string())?;
        let elapsed = started.elapsed();

        assert_eq!(result, Ok(r#"{"greeting":"hello"}"#.to_owned()));
        assert!(
            elapsed < Duration::from_secs(5),
            "pinned dispatch round trip took {elapsed:?}; the task must route to n1"
        );
        echo_n1.await.map_err(|error| error.to_string())??;

        // The n2 worker (wrong node) must never have been handed the task.
        assert!(
            n2_rx.try_recv().is_err(),
            "node=Some(\"n1\") dispatch must not reach the n2 worker"
        );

        on_n1.deregister()?;
        on_n2.deregister()?;
        Ok(())
    }

    /// An unpinned (`node = None`) dispatch is byte-identical to today: it
    /// reaches a worker in the pool regardless of the worker's advertised node.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unpinned_dispatch_reaches_a_pooled_worker_regardless_of_node()
    -> Result<(), Box<dyn std::error::Error>> {
        let registry = ConnectedWorkerRegistry::default();
        let pending = PendingActivities::new(TEST_HEARTBEAT_WINDOW);
        let activity_types = [String::from("greet")];
        let (n1_tx, mut n1_rx) = tokio::sync::mpsc::channel(32);
        let on_n1 = registry.register_namespaces(
            [String::from("default")],
            "default",
            Some(String::from("n1")),
            activity_types.iter(),
            n1_tx,
        )?;

        let sink = pending.clone();
        let echo = tokio::spawn(async move {
            let Some(WorkerMessage::ActivityTask(task)) = n1_rx.recv().await else {
                return Err("expected an activity task on the worker channel".to_owned());
            };
            let workflow_id = task
                .workflow_id
                .ok_or("task missing workflow id")
                .and_then(|id| WorkflowId::try_from(id).map_err(|_| "bad workflow id"))?;
            let activity_id = task
                .activity_id
                .map(ActivityId::from)
                .ok_or("task missing activity id")?;
            let completion_token =
                CompletionToken::from_wire(&workflow_id, &activity_id, task.completion_token)
                    .map_err(|error| error.to_string())?;
            sink.complete_activity(ActivityCompletion {
                workflow_id,
                activity_id,
                run_id: None,
                completion_token,
                outcome: ActivityCompletionOutcome::Succeeded(Payload::new(
                    ContentType::Json,
                    br#"{"greeting":"hello"}"#.to_vec(),
                )),
            })
            .map_err(|error| error.to_string())
        });

        let dispatcher = Arc::new(
            WorkerActivityDispatcher::new(registry.clone(), "default", test_tracker())
                .with_pending(pending),
        );

        // greet_request() carries node: None — the unpinned path.
        let result = tokio::spawn(futures::future::lazy(move |_| {
            dispatcher.dispatch(greet_request())
        }))
        .await
        .map_err(|error| error.to_string())?;

        assert_eq!(result, Ok(r#"{"greeting":"hello"}"#.to_owned()));
        echo.await.map_err(|error| error.to_string())??;
        on_n1.deregister()?;
        Ok(())
    }
}
