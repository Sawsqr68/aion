//! `Engine` start, cancel, result, list, and shutdown support.

use std::sync::Arc;

use aion_core::{Event, RunId, SearchAttributeSchema, WorkflowId};
use tokio::sync::Mutex as AsyncMutex;
use tokio::task::JoinHandle;

use crate::durability::Recorder;
use crate::schedule::ScheduleEvaluator;
use aion_store::EventStore;
use aion_store::visibility::VisibilityStore;

use crate::registry::TerminalOutcome;
use crate::{
    EngineError, Registry, RuntimeHandle, SupervisionTree, WorkflowCatalog,
    signal::SignalResumeHandoff,
};

use super::api_schedule::{
    ScheduleRuntimeDeps, default_schedule_evaluator, schedule_coordinator_workflow_id,
};
use super::delegated::DelegatedSeams;
use super::shutdown_gate::ShutdownGate;

/// Live embedded workflow engine assembled by [`crate::EngineBuilder`].
pub struct Engine {
    pub(super) store: Arc<dyn EventStore>,
    pub(super) visibility_store: Arc<dyn VisibilityStore>,
    pub(super) schedule_recorder: Arc<AsyncMutex<Recorder>>,
    pub(super) schedule_evaluator: Arc<AsyncMutex<ScheduleEvaluator>>,
    pub(super) schedule_coordinator_workflow_id: WorkflowId,
    pub(super) runtime: Arc<RuntimeHandle>,
    pub(super) catalog: Arc<WorkflowCatalog>,
    pub(super) registry: Arc<Registry>,
    pub(super) supervision: Arc<SupervisionTree>,
    delegated: DelegatedSeams,
    pub(super) signal_handoff: Arc<SignalResumeHandoff>,
    pub(super) search_attribute_schema: Arc<SearchAttributeSchema>,
    pub(super) shutdown_gate: ShutdownGate,
    /// Serializes the deploy mutations (load / route / unload) end-to-end
    /// across BOTH the catalog commit and its store persistence write, so
    /// the persisted package set and route pointers can never disagree with
    /// the catalog through interleaving (for example a concurrent re-deploy
    /// re-persisting a version an unload just deleted). Workflow dispatch
    /// never takes this lock.
    pub(super) deploy_mutations: AsyncMutex<()>,
    visibility_reconciliation_task: Option<JoinHandle<()>>,
    /// Shared dispatch-hold set for durable pause (#204): the workflow ids whose
    /// outbox rows are held `Pending` while paused. Mutated by pause/resume/cancel
    /// and rebuilt from [`EventStore::list_paused`] at startup/adoption; read by
    /// the outbox dispatcher at claim time.
    pub(super) paused_runs: crate::lifecycle::PausedRuns,
}

impl Drop for Engine {
    /// Close the engine-task epoch when the engine is released, whether or not
    /// [`Engine::shutdown`] was ever called or ever succeeded.
    ///
    /// Without this, an engine dropped without a successful shutdown left
    /// completion retries armed and appending terminal events. They could not
    /// be stopped by `EngineTaskRuntime::drop` either: an attempt in flight
    /// upgrades its weak reference and holds the `RuntimeHandle` strongly for
    /// the length of the attempt, so the refcount never reaches zero and that
    /// backstop is unreachable for precisely the span of the append it exists
    /// to stop. This drop runs before the engine's own fields are released, so
    /// it does not depend on that refcount at all.
    ///
    /// Uses the non-blocking `EngineTaskRuntime::begin_close` rather than the
    /// full shutdown:
    /// a `Drop` may run inside a host async context, where a blocking join
    /// panics. It therefore **gates and aborts, it does not await** — an
    /// attempt already past the append boundary is cancelled at its next await
    /// point. The gate is the load-bearing half: the append boundary reads it
    /// through `is_epoch_open` and refuses.
    ///
    /// # The visibility reconciliation task is aborted here for the same reason
    ///
    /// It runs on the HOST runtime, not the engine-task executor, so the epoch
    /// gate does not reach it — and dropping its `JoinHandle` detaches rather
    /// than cancels. It is an unbounded loop holding the event store and the
    /// visibility store, and `reconcile_visibility` WRITES. Left detached, an
    /// engine released without `shutdown` went on upserting visibility rows for
    /// the life of the process, against a store a successor engine may already
    /// own. `Engine::shutdown` aborts it as its first act; this does the same,
    /// so the two paths agree.
    ///
    /// # The live timer wheel is disarmed here for the third time, same reason
    ///
    /// 🔴 THIS WAS MISSING, AND IT LEFT A DURABLE WRITER ARMED. Live-wheel
    /// timer tasks are `tokio::spawn`ed on the HOST runtime
    /// (`runtime/nif_timer_bridge.rs`), so — exactly like the reconciliation
    /// loop — the engine-task epoch gate does not reach them. Their body is
    /// `fire_wheel_timer`, which records a durable `TimerFired`. They hold a
    /// `Weak<EngineNifState>`, and this drop deliberately does NOT clear the
    /// seams (see below), so that upgrade succeeds and the fire proceeds.
    ///
    /// An engine released without `shutdown` therefore kept a durable-append
    /// path armed for the life of the process. `Engine::shutdown` names the
    /// consequence precisely: across a failover, the dead owner's orphaned
    /// wheel task races the survivor's adoption-armed timer and can record the
    /// one durable `TimerFired` first, leaving the survivor's resident sleeper
    /// parked forever. That is the single-writer invariant, and nothing about
    /// it cares whether the engine was shut down or dropped.
    ///
    /// Safe in a `Drop`: `shutdown_timer_wheel` sets a flag and then performs a
    /// `DashMap` drain plus `abort()` — non-blocking, structurally identical to
    /// the `visibility_reconciliation_task.abort()` above. The async-context
    /// objection that justifies `begin_close` over the full shutdown does not
    /// apply to it.
    ///
    /// 🔴 AND IT IS A GATE, NOT ONLY A DRAIN — which it had to become for this
    /// `Drop` to be worth anything. A drain closes the set of timers armed at
    /// one instant; this `Drop` deliberately leaves the beamr scheduler and the
    /// engine seams alive, so a workflow process still runnable could reach
    /// `sleep` a moment later and arm a fresh durable `TimerFired` writer
    /// through a wheel this drop believed it had emptied. `arm_timer` now
    /// refuses once the flag is set (`nif_timer_bridge.rs`, `shut_down`), so
    /// the guarantee below is a property of the wheel from here on rather than
    /// of one instant.
    ///
    /// # 🔴 WHAT THIS DOES NOT DO, STATED SO NOBODY READS MORE INTO IT
    ///
    /// It does not clear the engine NIF seams. Those hold `Arc`s back to the
    /// `RuntimeHandle`, so until `clear_engine_seams` runs the handle, its
    /// beamr scheduler and every store clone they reach outlive this drop.
    /// `Engine::shutdown` clears them only after the scheduler has stopped and
    /// the child-task and timer-wheel epochs have closed; none of that has
    /// happened here, and a NIF could still read a slot this drop cleared.
    /// Trading a leak for a use-after-clear is the wrong direction, so the leak
    /// stands and is named: **an engine released without `shutdown` still holds
    /// its runtime.** What this drop guarantees is narrower and is the part
    /// that matters for durability — **no durable writer this process can still
    /// reach keeps writing.** There are FOUR, and they are stopped in two
    /// different ways:
    ///
    /// 1. anything armed on the **engine-task epoch** — `begin_close()` below;
    /// 2. the **visibility reconciliation loop** — `abort()` below;
    /// 3. the **live timer wheel** — `shutdown_timer_wheel()` below, which
    ///    gates and drains, *and* refuses at the point of writing, because
    ///    `abort` cannot stop a task already inside a poll. That refusal is in
    ///    TWO places, not one, and the second is easy to miss: an ordinary timer
    ///    is refused at the bridge's append boundary
    ///    (`nif_timer_bridge.rs`, `record_workflow_event`), but a reserved
    ///    `deadline:{run}` fire never reaches that boundary — `fire_timer_guarded`
    ///    demuxes it to the deadline handler first — so it is refused inside
    ///    [`crate::lifecycle::deadline::WorkflowDeadlineHandler`] instead, off
    ///    the same latch;
    /// 4. the **activity completion / retry task**
    ///    ([`crate::runtime::nif_activity_retry_dispatch::spawn_completion_task`]),
    ///    which this drop **cannot reach at all**: its `JoinHandle` is
    ///    discarded, so it is detached on the host runtime and nothing here
    ///    registers or aborts it. It is stopped instead at its append boundary,
    ///    which reads `is_epoch_open()` under the recorder lock — so step 1's
    ///    `begin_close()` is what silences it, one indirection away.
    ///
    /// 🔴 THAT LIST IS A CLAIM ABOUT DURABLE WRITERS AND IT IS ONLY AS GOOD AS
    /// ITS ENUMERATION — three times proven. An earlier revision named two and
    /// was wrong: the timer wheel was the third, and it was armed. The revision
    /// after that named three and was also wrong: the completion task was the
    /// fourth, it had no epoch check of any kind, and it sleeps an
    /// SDK-declared backoff with no ceiling between attempts. And the revision
    /// after THAT — the one that added the wheel's append-boundary refusal —
    /// wrote entry 3 as though that boundary covered the whole wheel, when the
    /// deadline path is demuxed away before it and had no refusal at all: an
    /// engine released without `shutdown` could still record a durable
    /// `WorkflowTimedOut` and tear a run down. **The enumeration was right and
    /// the mechanism named under it was not**, which is the harder failure to
    /// see, because the list looked complete. The way to check it is to
    /// enumerate what this process can still append with — grep the crate for
    /// every construction of a `Recorder` handle and every detached `spawn` —
    /// and then, for each entry, follow the ACTUAL route from the wake to the
    /// append and confirm the named gate sits on it. Not to re-read this
    /// sentence and find it plausible.
    fn drop(&mut self) {
        if let Some(task) = &self.visibility_reconciliation_task {
            task.abort();
        }
        self.runtime.nif_state().shutdown_timer_wheel();
        self.runtime.engine_tasks().begin_close();
    }
}

/// Components required to construct an [`Engine`].
pub(crate) struct EngineComponents {
    pub(crate) store: Arc<dyn EventStore>,
    pub(crate) visibility_store: Arc<dyn VisibilityStore>,
    pub(crate) runtime: Arc<RuntimeHandle>,
    pub(crate) catalog: Arc<WorkflowCatalog>,
    pub(crate) registry: Arc<Registry>,
    pub(crate) supervision: Arc<SupervisionTree>,
    pub(crate) delegated: DelegatedSeams,
    pub(crate) signal_handoff: Arc<SignalResumeHandoff>,
    pub(crate) search_attribute_schema: Arc<SearchAttributeSchema>,
    pub(crate) visibility_reconciliation_task: Option<JoinHandle<()>>,
}

impl Engine {
    /// Construct an engine from already-assembled components.
    #[must_use]
    pub(crate) fn new(components: EngineComponents) -> Self {
        let EngineComponents {
            store,
            visibility_store,
            runtime,
            catalog,
            registry,
            supervision,
            delegated,
            signal_handoff,
            search_attribute_schema,
            visibility_reconciliation_task,
        } = components;
        let schedule_coordinator_workflow_id = schedule_coordinator_workflow_id();
        let schedule_recorder = Arc::new(AsyncMutex::new(Recorder::new(
            schedule_coordinator_workflow_id.clone(),
            Arc::clone(&store),
        )));
        let runtime_arc = runtime;
        let registry_arc = registry;
        let supervision_arc = supervision;
        let schedule_evaluator = Arc::new(AsyncMutex::new(default_schedule_evaluator(
            schedule_coordinator_workflow_id.clone(),
            Arc::clone(&schedule_recorder),
            ScheduleRuntimeDeps {
                store: Arc::clone(&store),
                visibility_store: Arc::clone(&visibility_store),
                runtime: Arc::clone(&runtime_arc),
                catalog: Arc::clone(&catalog),
                registry: Arc::clone(&registry_arc),
                supervision: Arc::clone(&supervision_arc),
                search_attribute_schema: Arc::clone(&search_attribute_schema),
            },
        )));
        Self {
            store,
            visibility_store,
            schedule_recorder,
            schedule_evaluator,
            schedule_coordinator_workflow_id,
            runtime: runtime_arc,
            catalog,
            registry: registry_arc,
            supervision: supervision_arc,
            delegated,
            signal_handoff,
            search_attribute_schema,
            shutdown_gate: ShutdownGate::default(),
            deploy_mutations: AsyncMutex::new(()),
            visibility_reconciliation_task,
            paused_runs: crate::lifecycle::PausedRuns::default(),
        }
    }

    /// Advance the schedule coordinator's recorder head to match persisted
    /// events so that a rebuilt engine resumes appending at the correct
    /// sequence rather than conflicting at head 0.
    ///
    /// # Errors
    ///
    /// Returns store read errors.
    pub(crate) async fn catchup_schedule_coordinator(&self) -> Result<(), EngineError> {
        let history = self
            .store
            .read_history(&self.schedule_coordinator_workflow_id)
            .await?;
        let head = u64::try_from(history.len()).unwrap_or(u64::MAX);
        if head > 0 {
            let mut recorder = self.schedule_recorder.lock().await;
            *recorder = Recorder::resume_at(
                self.schedule_coordinator_workflow_id.clone(),
                Arc::clone(&self.store),
                head,
            );
        }
        Ok(())
    }

    /// Event store used by lifecycle and delegated AD/AT operations.
    #[must_use]
    pub fn store(&self) -> Arc<dyn EventStore> {
        Arc::clone(&self.store)
    }

    /// Visibility store used for workflow summary projections.
    #[must_use]
    pub fn visibility_store(&self) -> Arc<dyn VisibilityStore> {
        Arc::clone(&self.visibility_store)
    }

    /// Runtime boundary assembled for this engine.
    #[must_use]
    pub fn runtime(&self) -> &RuntimeHandle {
        &self.runtime
    }

    /// Shared workflow package catalog: loaded versions and routing.
    #[must_use]
    pub fn workflow_catalog(&self) -> &Arc<WorkflowCatalog> {
        &self.catalog
    }

    /// Active execution registry.
    #[must_use]
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Supervision tree snapshot/model.
    #[must_use]
    pub fn supervision(&self) -> &SupervisionTree {
        &self.supervision
    }

    /// Delegated signal/query/subscribe seams installed for AT/AD integration.
    #[must_use]
    pub const fn delegated(&self) -> &DelegatedSeams {
        &self.delegated
    }

    /// Shared in-memory handoff for already-recorded non-resident signals.
    #[must_use]
    pub fn signal_handoff(&self) -> Arc<SignalResumeHandoff> {
        Arc::clone(&self.signal_handoff)
    }

    /// Absorb a dead peer's distribution shards into this LIVE engine and resume
    /// their orphaned workflows — the SS-5 failover entry point.
    ///
    /// This is the production failover step a cluster supervisor invokes when it
    /// observes a peer gone (membership loss). It is the post-boot counterpart to
    /// the boot path's `EngineBuilder::owned_shards` election + recovery, run
    /// against an already-running engine:
    ///
    /// 1. **Elect + union-merge.** `acquire_owned_shards` wins the per-shard
    ///    election for each `shards` entry (fencing the dead owner) and
    ///    `become_live` union-merges that shard's committed history locally, so
    ///    every event the dead node had quorum-committed is now present on this
    ///    node. The election is blocking and runs off the tokio runtime inside the
    ///    store seam, honouring haematite's no-blocking-election-in-async
    ///    constraint, so this `async` method may call it directly.
    /// 2. **Widen the scope.** `extend_owned_shards` unions `shards` into this
    ///    node's owned-enumeration set so the adopted workflows, timers, and
    ///    outbox rows become visible to enumeration WITHOUT dropping this node's
    ///    own shards.
    /// 3. **Publish ownership.** `publish_shard_owner` records this node as each
    ///    adopted shard's current owner in the cluster's quorum-replicated
    ///    shard-owner directory (SS-3), so a request reaching a DIFFERENT survivor
    ///    routes to this adopter rather than mis-resolving to the dead declared
    ///    owner. The publish is fenced by the election just won, so only the true
    ///    adopter writes it; a non-distributed store no-ops it.
    /// 4. **Re-resident.** Re-run the idempotent active-workflow recovery and
    ///    timer recovery, which re-spawn every adopted workflow from the
    ///    union-merged history through the same production recovery seam the boot
    ///    path uses, skipping the workflows this node already owns.
    ///
    /// Detection of the peer's death is the CALLER's responsibility (a cluster
    /// supervisor / membership-loss trigger); this method performs the
    /// re-acquisition and resume once that decision is made. It is idempotent:
    /// adopting a shard this node already serves re-acquires (a no-op on the
    /// fence it already holds) and recovers nothing new.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::ShuttingDown`] after shutdown begins, store errors
    /// from the election / union-merge ([`EngineError::Durability`]), and any
    /// typed recovery error from re-residenting an adopted workflow.
    pub async fn adopt_shards(&self, shards: &[usize]) -> Result<(), EngineError> {
        let operation = self.shutdown_gate.begin_start()?;
        let result = self.adopt_shards_inner(shards).await;
        drop(operation);
        result
    }

    /// Body of [`Self::adopt_shards`]: acquire+publish each shard as a UNIT under
    /// the double-adoption fence (ADR-021 clean-partial), then widen scope and
    /// recover over EXACTLY the shards that survived BOTH steps.
    ///
    /// ## Ordering invariant (the fix)
    ///
    /// For each shard the publish-fence happens BEFORE the shard contributes to
    /// `extend_owned_shards` AND before it is recovered. The pre-fix order
    /// (extend → publish) let a survivor that won the election but was then
    /// deposed at publish-time still widen its scope and recover the shard, so two
    /// survivors could both execute its workflows. Here, a `NotOwner` from EITHER
    /// `acquire_owned_shard` OR `publish_shard_owner` DROPS that shard: it never
    /// reaches `extend_owned_shards`, is never recovered, and is NEVER a hard
    /// `Durability` error. A deposed survivor therefore leaves ZERO widened
    /// owned-shards scope and recovers nothing.
    async fn adopt_shards_inner(&self, shards: &[usize]) -> Result<(), EngineError> {
        // 1-3. Drive the double-adoption fence in the FIXED order (acquire →
        //      publish per shard as a UNIT, then re-assert ownership and widen the
        //      enumeration scope ONCE) and learn which shards survived it. A shard
        //      deposed at acquire OR publish (or in the residual window) is dropped
        //      cleanly — never extended, never recovered, never a hard error. The
        //      planner GUARANTEES each survivor's publish-fence precedes both the
        //      scope widening and (below) recovery. A single-node store no-ops
        //      every step, so this path stays byte-identical there.
        // The returned survivor set is already reflected in the store's widened
        // owned-shard scope (the planner's single `extend`), which is what recovery
        // enumerates over; the value is bound only to make that contract explicit.
        let _recoverable = super::fence::plan_adopted_shards(
            &super::fence::StoreFenceSeam {
                store: &*self.store,
            },
            shards,
        )?;
        // 3b. Rebuild the pause dispatch-hold for the newly-adopted shards (#204).
        //     The fence above widened the owned-shard scope, so `list_paused` now
        //     sees the adopted shards' durably-`Paused` runs. `extend` (not replace)
        //     preserves the holds for shards this node already owned. A run paused on
        //     an adopted shard keeps its outbox rows held after failover; without this
        //     the adopting node's dispatcher would claim and dispatch them. A store
        //     error is logged, not fatal: the adoption itself is durable and the next
        //     startup/rebuild repopulates the hold.
        match self.store.list_paused().await {
            Ok(paused) => self.paused_runs.extend(paused),
            Err(error) => {
                tracing::warn!(%error, "failed to rebuild paused-runs dispatch hold at shard adoption");
            }
        }
        // 4. Re-resident the adopted workflows through the production recovery
        //    seam (idempotent: this node's own workflows are skipped). Recovery
        //    enumerates over the owned scope, which now contains only shards that
        //    survived the fence.
        super::startup::recover_adopted_shards(super::startup::StartupRecoveryContext {
            store: Arc::clone(&self.store),
            visibility_store: Arc::clone(&self.visibility_store),
            runtime: Arc::clone(&self.runtime),
            catalog: Arc::clone(&self.catalog),
            registry: Arc::clone(&self.registry),
            supervision: Arc::clone(&self.supervision),
            recovery: None,
            search_attribute_schema: Arc::clone(&self.search_attribute_schema),
            bootstrap_schedule_coordinator: false,
        })
        .await?;
        // 5. Re-arm durable timers for the adopted workflows — the SAME step the
        //    boot path runs after `recover_active_workflows_on_startup` (see
        //    `EngineBuilder::build`). This is LOAD-BEARING for a workflow PARKED on
        //    a durable timer (#119): step 4 replays it and re-parks it, but the
        //    replay of a not-yet-fired sleep does NOT re-arm the live wheel (only a
        //    first, non-replay arrival does — see `nif_timer::sleep`'s `ResumeLive`
        //    branch). Without this call the adopted workflow stays parked forever:
        //    `recover_due` fires already-expired timers and
        //    `rearm_future_from_active_histories` re-arms still-future ones onto the
        //    now-resident process. Removing it reproduces the #119 symptom (a
        //    survivor adopts the shard but the parked timer never reaches the
        //    resumed workflow). Guarded by `tests/adoption_parked_timer_e2e.rs`
        //    (single-process) and `tests/adoption_parked_timer_xnode_e2e.rs`
        //    (real cross-node failover).
        super::startup::recover_timers_on_startup(self.runtime.nif_state(), Arc::clone(&self.store))
            .await
    }

    /// Gracefully stop accepting new starts and shut down the embedded runtime.
    ///
    /// # Errors
    ///
    /// Returns registry poison or runtime shutdown failures as typed errors.
    pub fn shutdown(&self) -> Result<(), EngineError> {
        if let Some(task) = &self.visibility_reconciliation_task {
            task.abort();
        }
        // 🔴 THE EPOCH CLOSES FIRST, BEFORE ANY WAIT.
        //
        // The first cut put the unconditional close in `RuntimeHandle::shutdown`
        // — one level BELOW the call the shipped server actually makes — and
        // left this function short-circuiting above it. Two ways that lost the
        // property it was written to guarantee:
        //
        //   1. `close_and_wait` returns `Err` on registry poison, so `?` here
        //      returned before the epoch was ever gated and completion retries
        //      stayed armed.
        //   2. `close_and_wait` is a condvar wait with NO timeout. A lifecycle
        //      operation stuck on a degraded store — precisely the condition
        //      that arms completion retries in the first place — blocks this
        //      function indefinitely, and the operator reasonably concludes the
        //      node is wedged and brings up a successor while this process is
        //      still appending terminals.
        //
        // Gating costs nothing, cannot fail, and is idempotent. Doing it first
        // means no path through this function leaves retries armed. Everything
        // after is teardown that still needs to run.
        //
        // 🔴 WHAT THIS ORDERING COSTS, STATED WHERE THE ORDERING IS CHOSEN.
        // Process-exit callbacks are still admitted for the whole span between
        // this line and `process_exits.begin_shutdown()` below, and the
        // completion path refuses every one of them because the epoch is
        // already closed. A run exiting in that window records no terminal and
        // stays `Running` in the store, with one `error!` line naming it. The
        // span is UNBOUNDED — `close_and_wait` is a condvar wait with no
        // timeout — and it is longest under exactly the degraded-store
        // condition the completion retries exist for. That window is the price
        // of the two properties above and is argued in full at the refusal site
        // (`lifecycle::completion`, at `refuse_if_epoch_closed`); it is
        // repeated here because a reader deciding to move this line would
        // otherwise not know a cost had been accepted.
        self.runtime.engine_tasks().begin_close();
        // Every step below runs on every path, and the FIRST error is returned
        // at the end. A `?` here would skip the timer-wheel shutdown and the
        // seam clearing, whose consequences are spelled out at their own call
        // sites — an orphaned wheel task racing a survivor's adoption timer, and
        // a durable backend's writer lock held past shutdown. Neither is
        // something to trade for reporting an earlier error sooner.
        //
        // 🔴 THE SEAM CLEARING IS THE HALF WITH NO BACKSTOP, AND THAT IS THE
        // WHOLE REASON. An earlier revision said `Drop for Engine` "backstops
        // neither", which stopped being true in this same file when `Drop`
        // gained `shutdown_timer_wheel` (see it above) — so the wheel half IS
        // backstopped, and a reader checking only that half would conclude the
        // `?` costs nothing. It does: `clear_engine_seams` runs from
        // `Engine::shutdown` and NOWHERE else, by design — it may only run once
        // the scheduler has stopped and both epochs are closed, which `Drop`
        // cannot establish. Skip it and the `RuntimeHandle` ↔ `EngineNifState`
        // cycle is never broken, so every store clone reached through the seams
        // outlives the process's interest in them and a durable backend's
        // cross-process writer lock is held until exit.
        //
        // 🔴 THE THIRD COST, STATED BECAUSE EVERY OTHER ONE IN THIS FUNCTION IS.
        // `ShutdownGate::close_and_wait` returns `Err` on exactly one condition
        // — mutex poison — and continuing past it means the gate's DRAIN
        // guarantee is skipped, not merely its error deferred: a lifecycle
        // operation admitted before the poison may still be in flight when
        // `runtime.shutdown()` stops the scheduler and `clear_engine_seams()`
        // nulls the seam slots. That is not a memory hazard (the slots are
        // `Option`-shaped and a NIF reading a cleared one gets a typed error),
        // and no NEW operation can be admitted either, because `begin_start`
        // and `begin_operation` share the same poisoned `state()`. What is lost
        // is the promise that nothing was still running when teardown began.
        // Accepted for the same reason as the rest: the alternative is skipping
        // the seam clearing, which is unbacked-up and permanent.
        let mut first_error: Option<EngineError> = None;
        // Scoped so the closure's unique borrow of `first_error` visibly ends
        // before the value is read. (`drop(closure)` would end it just as
        // surely — this crate is edition 2024, and under NLL a borrow ends at
        // its last use — so this is a readability choice, not a soundness one.
        // An earlier revision of this comment argued the opposite and was
        // describing pre-NLL lexical scoping.)
        {
            // 🔴 THE SECOND ERROR IS REPORTED, NOT DISCARDED. Only one
            // `EngineError` can be returned, but accumulate-and-continue means
            // more than one step can fail — and at HEAD that could not happen
            // at all, because `?` meant a later step never ran. Keeping only
            // the first and dropping the rest would trade a skipped teardown
            // for a swallowed failure, which is the same defect wearing the
            // other hat: an operator seeing `RegistryPoisoned` would have no
            // signal that the runtime teardown ALSO failed. Each subsequent
            // failure is emitted at `error` level with the position that made
            // it subsequent, so the log carries what the return value cannot.
            let mut failed_steps = 0_u32;
            let mut keep = |step: &'static str, result: Result<(), EngineError>| {
                if let Err(error) = result {
                    failed_steps += 1;
                    if first_error.is_none() {
                        first_error = Some(error);
                    } else {
                        tracing::error!(
                            step,
                            failed_steps,
                            error = %error,
                            "a further engine-shutdown step failed after an earlier one; only \
                             the first failure can be returned, so this one is reported here"
                        );
                    }
                }
            };
            keep(
                "shutdown_gate.close_and_wait",
                self.shutdown_gate.close_and_wait(),
            );
            // Epoch close for engine background tasks (F4): every watcher,
            // spawn-recovery task and process-exit completion retry is aborted AND
            // awaited to quiescence — a task still mid-record after shutdown could
            // double-write a history a successor engine over the same store also
            // records into. Arming is additionally gated inside the task registry
            // the moment shutdown begins.
            //
            // `runtime.shutdown()` performs that close itself, because the
            // completion retry is a core lifecycle path and its epoch close must not
            // depend on whether an optional bridge was installed. The bridge call
            // that follows is idempotent and kept only so an installed bridge
            // participates explicitly.
            keep("runtime.shutdown", self.runtime.shutdown());
        }
        self.runtime.nif_state().shutdown_engine_tasks();
        // Abort armed live-wheel timer tasks (#119): they run on the tokio
        // runtime, not the beamr scheduler, so `runtime.shutdown()` does not
        // reach them. A timer this engine armed must NOT fire after the engine
        // has stopped owning the workflow — otherwise, across a failover, the
        // dead owner's orphaned wheel task races the survivor's adoption-armed
        // timer and can record the one durable `TimerFired` first, leaving the
        // survivor's resident sleeper parked forever.
        self.runtime.nif_state().shutdown_timer_wheel();
        // Break the RuntimeHandle <-> EngineNifState reference cycle (see
        // EngineNifState::clear_engine_seams). The engine-scoped NIF seams each
        // hold an Arc back to the runtime and/or clones of the event store and
        // registry; without releasing them here the runtime, its NIF state, and
        // every store clone they reach would outlive the dropped Engine
        // forever, keeping a durable backend's writer lock held past shutdown.
        // Safe now: the scheduler has stopped and the child-task and timer-wheel
        // epochs have closed, so no NIF or background task can still read a slot.
        self.runtime.nif_state().clear_engine_seams();
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

pub(crate) fn terminal_outcome_from_history(events: &[Event]) -> Option<TerminalOutcome> {
    // Reset-aware via the shared single-source predicate: the current lease's
    // terminal event, where a reopen (WorkflowReopened) supersedes any earlier
    // terminal.
    match aion_core::current_lease_terminal(events)? {
        Event::WorkflowCompleted { result, .. } => Some(TerminalOutcome::Completed(result.clone())),
        Event::WorkflowFailed { error, .. } => Some(TerminalOutcome::Failed(error.clone())),
        Event::WorkflowCancelled { reason, .. } => Some(TerminalOutcome::Cancelled(reason.clone())),
        Event::WorkflowTimedOut { timeout, .. } => Some(TerminalOutcome::TimedOut(timeout.clone())),
        Event::WorkflowContinuedAsNew {
            input,
            workflow_type,
            parent_run_id,
            ..
        } => Some(TerminalOutcome::ContinuedAsNew {
            input: input.clone(),
            workflow_type: workflow_type.clone(),
            parent_run_id: parent_run_id.clone(),
        }),
        _ => None,
    }
}

pub(crate) fn workflow_not_found(id: &WorkflowId, run: &RunId) -> EngineError {
    EngineError::WorkflowNotFound {
        workflow_type: format!("{id}/{run}"),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use aion_core::{
        Event, EventEnvelope, PackageVersion, Payload, RunId, SearchAttributeSchema,
        TimerCancelCause, TimerId, WorkflowFilter, WorkflowId, WorkflowStatus,
    };
    use aion_package::ContentHash;
    use aion_store::visibility::VisibilityStore;
    use aion_store::{EventStore, InMemoryStore, ReadableEventStore};
    use serde_json::json;

    use std::collections::HashMap;

    use super::{DelegatedSeams, Engine, EngineComponents};
    use crate::durability::Recorder;
    use crate::lifecycle::terminate::{self, TerminateWorkflowContext};
    use crate::registry::{CompletionNotifier, HandleResidency, WorkflowHandleParts};
    use crate::time::TimerRecovery;
    use crate::time::timer_service::live_timers_in_active_segment;
    use crate::{
        EngineError, Registry, RuntimeConfig, RuntimeHandle, SupervisionTree, WorkflowCatalog,
        WorkflowHandle,
    };

    fn payload(label: &str) -> Result<Payload, aion_core::PayloadError> {
        Payload::from_json(&json!({ "label": label }))
    }

    fn workflow_error(message: &str) -> aion_core::WorkflowError {
        aion_core::WorkflowError {
            message: message.to_owned(),
            details: None,
        }
    }

    fn workflow_catalog(workflow_type: &str, deployed_module: &str) -> Arc<WorkflowCatalog> {
        let catalog = Arc::new(WorkflowCatalog::new());
        catalog.note_loaded_workflow_for_test(
            workflow_type,
            deployed_module,
            "run",
            ContentHash::from_bytes([5; 32]),
        );
        catalog
    }

    fn engine_with_loaded_workflow(
        store: Arc<dyn EventStore>,
        workflow_type: &str,
        deployed_module: &str,
    ) -> Result<Engine, EngineError> {
        let runtime = RuntimeHandle::new(RuntimeConfig::new(Some(1)))?;
        runtime.register_waiting_test_module(deployed_module, "run");
        let visibility_store: Arc<dyn VisibilityStore> = Arc::new(InMemoryStore::default());
        Ok(Engine::new(EngineComponents {
            store,
            visibility_store,
            runtime: Arc::new(runtime),
            catalog: workflow_catalog(workflow_type, deployed_module),
            registry: Arc::new(Registry::default()),
            supervision: Arc::new(SupervisionTree::new()),
            delegated: DelegatedSeams::default(),
            signal_handoff: Arc::new(crate::signal::SignalResumeHandoff::new()),
            search_attribute_schema: Arc::new(SearchAttributeSchema::new()),
            visibility_reconciliation_task: None,
        }))
    }

    fn termination_context(engine: &Engine) -> TerminateWorkflowContext<'_> {
        TerminateWorkflowContext {
            runtime: engine.runtime(),
            store: engine.store(),
            visibility_store: engine.visibility_store(),
            registry: engine.registry(),
            catalog: engine.workflow_catalog(),
        }
    }

    async fn insert_active_handle(
        engine: &Engine,
        store: Arc<dyn EventStore>,
        workflow_type: &str,
    ) -> Result<WorkflowHandle, Box<dyn std::error::Error>> {
        let workflow_id = aion_core::WorkflowId::new_v4();
        let run_id = aion_core::RunId::new_v4();
        let mut recorder = Recorder::new(workflow_id.clone(), store);
        recorder
            .record_workflow_started(
                chrono::Utc::now(),
                crate::durability::WorkflowStartRecord {
                    workflow_type: workflow_type.to_owned(),
                    input: payload("input")?,
                    run_id: run_id.clone(),
                    parent_run_id: None,
                    package_version: aion_core::PackageVersion::new("a".repeat(64)),
                },
            )
            .await?;
        let pid = engine.runtime().spawn_test_process_with_trap_exit(true)?;
        let handle = WorkflowHandle::new(WorkflowHandleParts {
            workflow_id: workflow_id.clone(),
            run_id: run_id.clone(),
            pid,
            workflow_type: workflow_type.to_owned(),
            namespace: String::from("default"),
            loaded_version: ContentHash::from_bytes([9; 32]),
            cached_status: WorkflowStatus::Running,
            residency: HandleResidency::Resident,
            recorder,
            completion: CompletionNotifier::new(),
        });
        engine
            .registry()
            .insert((workflow_id, run_id), handle.clone())?;
        Ok(handle)
    }

    #[tokio::test]
    async fn start_then_cancel_records_started_then_cancelled()
    -> Result<(), Box<dyn std::error::Error>> {
        let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
        let engine =
            engine_with_loaded_workflow(Arc::clone(&store), "checkout", "checkout_deployed")?;
        let handle = engine
            .start_workflow(
                "checkout",
                payload("input")?,
                HashMap::new(),
                String::from("default"),
            )
            .await?;

        engine
            .cancel(
                handle.workflow_id(),
                handle.run_id(),
                "caller requested cancellation",
            )
            .await?;

        let history = store.read_history(handle.workflow_id()).await?;
        match history.as_slice() {
            [
                Event::WorkflowStarted { .. },
                Event::WorkflowCancelled { reason, .. },
            ] => {
                assert_eq!(reason, "caller requested cancellation");
            }
            other => return Err(format!("expected started then cancelled, found {other:?}").into()),
        }
        engine.shutdown()?;
        Ok(())
    }

    fn test_envelope(workflow_id: &WorkflowId, seq: u64) -> EventEnvelope {
        EventEnvelope {
            seq,
            recorded_at: chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap_or_default(),
            workflow_id: workflow_id.clone(),
        }
    }

    fn started_event(workflow_id: &WorkflowId, seq: u64) -> Event {
        Event::WorkflowStarted {
            envelope: test_envelope(workflow_id, seq),
            workflow_type: String::from("checkout"),
            input: Payload::new(aion_core::ContentType::Json, b"{}".to_vec()),
            run_id: RunId::new_v4(),
            parent_run_id: None,
            package_version: PackageVersion::new("a".repeat(64)),
        }
    }

    fn timer_started_event(workflow_id: &WorkflowId, seq: u64, timer_id: &TimerId) -> Event {
        Event::TimerStarted {
            envelope: test_envelope(workflow_id, seq),
            timer_id: timer_id.clone(),
            fire_at: chrono::DateTime::from_timestamp(1_700_000_500, 0).unwrap_or_default(),
        }
    }

    fn timer_fired_event(workflow_id: &WorkflowId, seq: u64, timer_id: &TimerId) -> Event {
        Event::TimerFired {
            envelope: test_envelope(workflow_id, seq),
            timer_id: timer_id.clone(),
        }
    }

    fn timer_cancelled_event(workflow_id: &WorkflowId, seq: u64, timer_id: &TimerId) -> Event {
        Event::TimerCancelled {
            envelope: test_envelope(workflow_id, seq),
            timer_id: timer_id.clone(),
            cause: TimerCancelCause::WorkflowIntent,
        }
    }

    #[test]
    fn live_timers_lists_started_and_unterminated() {
        let workflow_id = WorkflowId::new_v4();
        let first = TimerId::anonymous(0);
        let second = TimerId::anonymous(1);
        let history = vec![
            started_event(&workflow_id, 0),
            timer_started_event(&workflow_id, 1, &first),
            timer_started_event(&workflow_id, 2, &second),
        ];
        assert_eq!(
            live_timers_in_active_segment(&history),
            vec![first, second],
            "both started, unterminated timers should be live, in start order"
        );
    }

    #[test]
    fn live_timers_excludes_fired_and_cancelled() {
        let workflow_id = WorkflowId::new_v4();
        let fired = TimerId::anonymous(0);
        let cancelled = TimerId::anonymous(1);
        let live = TimerId::anonymous(2);
        let history = vec![
            started_event(&workflow_id, 0),
            timer_started_event(&workflow_id, 1, &fired),
            timer_started_event(&workflow_id, 2, &cancelled),
            timer_started_event(&workflow_id, 3, &live),
            timer_fired_event(&workflow_id, 4, &fired),
            timer_cancelled_event(&workflow_id, 5, &cancelled),
        ];
        assert_eq!(
            live_timers_in_active_segment(&history),
            vec![live],
            "only the timer with no terminal event remains live"
        );
    }

    #[test]
    fn live_timers_dedups_repeated_start() {
        let workflow_id = WorkflowId::new_v4();
        let timer = TimerId::anonymous(0);
        let history = vec![
            started_event(&workflow_id, 0),
            timer_started_event(&workflow_id, 1, &timer),
            timer_started_event(&workflow_id, 2, &timer),
        ];
        assert_eq!(live_timers_in_active_segment(&history), vec![timer]);
    }

    #[test]
    fn live_timers_scopes_to_active_run_segment() {
        // A timer started in a prior run (before a continue-as-new
        // `WorkflowStarted`) must not be surfaced for the replacement run.
        let workflow_id = WorkflowId::new_v4();
        let prior_run = TimerId::anonymous(0);
        let current_run = TimerId::anonymous(0);
        let history = vec![
            started_event(&workflow_id, 0),
            timer_started_event(&workflow_id, 1, &prior_run),
            started_event(&workflow_id, 2),
            timer_started_event(&workflow_id, 3, &current_run),
        ];
        assert_eq!(
            live_timers_in_active_segment(&history),
            vec![current_run],
            "only timers from the latest WorkflowStarted segment are live"
        );
    }

    #[test]
    fn live_timers_empty_history_is_empty() {
        assert!(live_timers_in_active_segment(&[]).is_empty());
    }

    /// Build an engine whose runtime has the production timer NIF bridge
    /// installed against the given store + registry, so `Engine::cancel`'s timer
    /// cleanup exercises the real `TimerService` path (not a fake). Must be
    /// called from within a tokio runtime (`Handle::current()`).
    fn engine_with_timer_bridge(
        store: Arc<dyn EventStore>,
        registry: Arc<Registry>,
    ) -> Result<Engine, EngineError> {
        let runtime = RuntimeHandle::new(RuntimeConfig::new(Some(1)))?;
        runtime.register_waiting_test_module("checkout_deployed", "run");
        crate::runtime::nif_timer_bridge::install_timer_nif_bridge(
            runtime.nif_state(),
            Arc::clone(&registry),
            Arc::clone(&store),
            tokio::runtime::Handle::current(),
            crate::runtime::SignalDeliveryConfig::default(),
        );
        let visibility_store: Arc<dyn VisibilityStore> = Arc::new(InMemoryStore::default());
        Ok(Engine::new(EngineComponents {
            store,
            visibility_store,
            runtime: Arc::new(runtime),
            catalog: workflow_catalog("checkout", "checkout_deployed"),
            registry,
            supervision: Arc::new(SupervisionTree::new()),
            delegated: DelegatedSeams::default(),
            signal_handoff: Arc::new(crate::signal::SignalResumeHandoff::new()),
            search_attribute_schema: Arc::new(SearchAttributeSchema::new()),
            visibility_reconciliation_task: None,
        }))
    }

    /// Root-cause regression: cancelling a workflow with a live durable timer
    /// must record `TimerCancelled` (before the terminal `WorkflowCancelled`),
    /// so the timer is dead in history and recovery never fires it as an
    /// orphan. Drives the real `Engine::cancel` against a runtime with the
    /// production timer bridge installed.
    #[tokio::test(flavor = "multi_thread")]
    async fn cancel_records_timer_cancelled_before_workflow_cancelled()
    -> Result<(), Box<dyn std::error::Error>> {
        let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
        let registry = Arc::new(Registry::default());
        let engine = engine_with_timer_bridge(Arc::clone(&store), Arc::clone(&registry))?;

        let handle = engine
            .start_workflow(
                "checkout",
                payload("input")?,
                HashMap::new(),
                String::from("default"),
            )
            .await?;

        // Arm a live durable timer for the resident run and record its
        // `TimerStarted`, exactly as the resume-live handoff would in production.
        let timer_id = TimerId::anonymous(0);
        let fire_at = chrono::Utc::now() + chrono::Duration::hours(1);
        handle
            .recorder()
            .lock()
            .await
            .record_timer_started(chrono::Utc::now(), timer_id.clone(), fire_at)
            .await?;
        let timer_service =
            crate::runtime::nif_timer_bridge::installed_timer_service(engine.runtime().nif_state())
                .map_err(|error| format!("timer service unavailable: {error}"))?;
        timer_service
            .schedule(handle.workflow_id().clone(), timer_id.clone(), fire_at)
            .await?;

        engine
            .cancel(
                handle.workflow_id(),
                handle.run_id(),
                "caller requested cancellation",
            )
            .await?;

        let history = store.read_history(handle.workflow_id()).await?;
        match history.as_slice() {
            [
                Event::WorkflowStarted { .. },
                Event::TimerStarted {
                    timer_id: started, ..
                },
                Event::TimerCancelled {
                    timer_id: cancelled,
                    ..
                },
                Event::WorkflowCancelled { reason, .. },
            ] => {
                assert_eq!(started, &timer_id);
                assert_eq!(cancelled, &timer_id, "the live timer must be cancelled");
                assert_eq!(reason, "caller requested cancellation");
            }
            other => {
                return Err(format!(
                    "expected [started, timer-started, timer-cancelled, cancelled], found {other:?}"
                )
                .into());
            }
        }
        engine.shutdown()?;
        Ok(())
    }

    /// All live timers (not just one) are cancelled, in start order, before the
    /// terminal `WorkflowCancelled`.
    #[tokio::test(flavor = "multi_thread")]
    async fn cancel_cancels_multiple_live_timers() -> Result<(), Box<dyn std::error::Error>> {
        let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
        let registry = Arc::new(Registry::default());
        let engine = engine_with_timer_bridge(Arc::clone(&store), Arc::clone(&registry))?;
        let handle = engine
            .start_workflow(
                "checkout",
                payload("input")?,
                HashMap::new(),
                String::from("default"),
            )
            .await?;

        let first = TimerId::anonymous(0);
        let second = TimerId::anonymous(1);
        let fire_at = chrono::Utc::now() + chrono::Duration::hours(1);
        {
            let recorder = handle.recorder();
            let mut recorder = recorder.lock().await;
            recorder
                .record_timer_started(chrono::Utc::now(), first.clone(), fire_at)
                .await?;
            recorder
                .record_timer_started(chrono::Utc::now(), second.clone(), fire_at)
                .await?;
        }

        engine
            .cancel(handle.workflow_id(), handle.run_id(), "stop")
            .await?;

        let history = store.read_history(handle.workflow_id()).await?;
        match history.as_slice() {
            [
                Event::WorkflowStarted { .. },
                Event::TimerStarted {
                    timer_id: started_first,
                    ..
                },
                Event::TimerStarted {
                    timer_id: started_second,
                    ..
                },
                Event::TimerCancelled {
                    timer_id: cancelled_first,
                    ..
                },
                Event::TimerCancelled {
                    timer_id: cancelled_second,
                    ..
                },
                Event::WorkflowCancelled { .. },
            ] => {
                assert_eq!(started_first, &first);
                assert_eq!(started_second, &second);
                assert_eq!(cancelled_first, &first, "first live timer cancelled first");
                assert_eq!(
                    cancelled_second, &second,
                    "second live timer cancelled second"
                );
            }
            other => {
                return Err(format!(
                    "expected two timer-cancels before workflow-cancel, found {other:?}"
                )
                .into());
            }
        }
        engine.shutdown()?;
        Ok(())
    }

    /// End-to-end source-of-bug proof: a cancelled workflow leaves no orphan for
    /// startup recovery. With a past-due durable timer row (the exact shape that
    /// bricked startup before the fix), recovery surfaces no `UnknownWorkflow`
    /// and fires nothing — because cancel recorded `TimerCancelled`, so the
    /// timer is dead in history. Complements the committed `recover_due` defense
    /// test by proving the orphan is gone *at the source*.
    #[tokio::test(flavor = "multi_thread")]
    async fn cancelled_workflow_leaves_no_orphan_for_recovery()
    -> Result<(), Box<dyn std::error::Error>> {
        let concrete: Arc<InMemoryStore> = Arc::new(InMemoryStore::default());
        let store: Arc<dyn EventStore> = concrete.clone();
        let registry = Arc::new(Registry::default());
        let engine = engine_with_timer_bridge(Arc::clone(&store), Arc::clone(&registry))?;
        let handle = engine
            .start_workflow(
                "checkout",
                payload("input")?,
                HashMap::new(),
                String::from("default"),
            )
            .await?;
        let workflow_id = handle.workflow_id().clone();

        // A live timer whose durable row is already past-due, inserted directly
        // (no wheel arm, so nothing races the cancel).
        let timer_id = TimerId::anonymous(0);
        let fire_at = chrono::Utc::now() - chrono::Duration::hours(1);
        handle
            .recorder()
            .lock()
            .await
            .record_timer_started(chrono::Utc::now(), timer_id.clone(), fire_at)
            .await?;
        concrete
            .schedule_timer(&workflow_id, &timer_id, fire_at)
            .await?;

        let timer_service =
            crate::runtime::nif_timer_bridge::installed_timer_service(engine.runtime().nif_state())
                .map_err(|error| format!("timer service unavailable: {error}"))?;

        engine.cancel(&workflow_id, handle.run_id(), "stop").await?;

        // Cancel removed the workflow from the registry and the durable row is
        // now past-due — exactly the orphan scenario. Recovery must handle it
        // cleanly: the recorded `TimerCancelled` makes `fire_timer` a no-op, so
        // no `TimerFired` and (critically) no `UnknownWorkflow`.
        let readable: Arc<dyn ReadableEventStore> = concrete.clone();
        TimerRecovery::new(readable, timer_service, Duration::ZERO)
            .recover_on_startup(chrono::Utc::now())
            .await?;

        let history = concrete.read_history(&workflow_id).await?;
        assert!(
            !history
                .iter()
                .any(|event| matches!(event, Event::TimerFired { .. })),
            "no timer should fire for a cancelled workflow during recovery"
        );
        assert!(
            history
                .iter()
                .any(|event| matches!(event, Event::TimerCancelled { .. })),
            "cancel must have recorded TimerCancelled at the source"
        );
        engine.shutdown()?;
        Ok(())
    }

    #[tokio::test]
    async fn result_returns_completed_payload() -> Result<(), Box<dyn std::error::Error>> {
        let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
        let engine =
            engine_with_loaded_workflow(Arc::clone(&store), "checkout", "checkout_deployed")?;
        let handle = engine
            .start_workflow(
                "checkout",
                payload("input")?,
                HashMap::new(),
                String::from("default"),
            )
            .await?;
        let result_payload = payload("result")?;

        terminate::complete(
            termination_context(&engine),
            handle.workflow_id(),
            handle.run_id(),
            result_payload.clone(),
        )
        .await?;

        assert_eq!(
            engine.result(handle.workflow_id(), handle.run_id()).await?,
            Ok(result_payload)
        );
        engine.shutdown()?;
        Ok(())
    }

    #[tokio::test]
    async fn result_returns_failed_workflow_error() -> Result<(), Box<dyn std::error::Error>> {
        let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
        let engine =
            engine_with_loaded_workflow(Arc::clone(&store), "checkout", "checkout_deployed")?;
        let handle = engine
            .start_workflow(
                "checkout",
                payload("input")?,
                HashMap::new(),
                String::from("default"),
            )
            .await?;
        let error = workflow_error("workflow failed");

        terminate::fail(
            termination_context(&engine),
            handle.workflow_id(),
            handle.run_id(),
            error.clone(),
        )
        .await?;

        assert_eq!(
            engine.result(handle.workflow_id(), handle.run_id()).await?,
            Err(error)
        );
        engine.shutdown()?;
        Ok(())
    }

    #[tokio::test]
    async fn result_unknown_workflow_returns_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
        let engine = engine_with_loaded_workflow(store, "checkout", "checkout_deployed")?;
        let workflow_id = aion_core::WorkflowId::new_v4();
        let run_id = aion_core::RunId::new_v4();

        let result = engine.result(&workflow_id, &run_id).await;

        assert!(matches!(result, Err(EngineError::WorkflowNotFound { .. })));
        engine.shutdown()?;
        Ok(())
    }

    /// F5: `Engine::shutdown`'s SECOND failing step must be reported.
    ///
    /// Its two fallible steps both go through `keep`, which returns the first
    /// and emits every later one at `error` level. That `else` arm IS the fix
    /// for the swallowed-second-error defect, and nothing asserted on it: the
    /// sibling below arms only the drain, which fails the SECOND step, so
    /// `first_error` is still `None` when `keep` sees it and the `else` is
    /// never taken.
    ///
    /// Reaching it needs BOTH steps to fail, which is why `ShutdownGate` gained
    /// its own injection seam. The gate's only real failure is mutex poison, so
    /// the injected error is `RegistryPoisoned` — a fault wearing the label its
    /// injection point can actually issue.
    ///
    /// Killing mutation: replace the `else` body inside `keep` with `{}`. No
    /// `error!` is emitted and the capture assertion fails.
    #[tokio::test]
    async fn a_second_failing_engine_shutdown_step_is_reported_not_swallowed()
    -> Result<(), Box<dyn std::error::Error>> {
        let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
        let engine = engine_with_loaded_workflow(store, "checkout", "checkout_deployed")?;

        // Both steps fail: the gate first (so it is the returned error), the
        // runtime drain second (so it lands in the `else`).
        engine.shutdown_gate.force_close_failure();
        engine.runtime().force_process_exit_drain_failure();

        let (captured, subscriber) = crate::log_capture::LogCapture::new()?;
        let returned = {
            let _installed = tracing::subscriber::set_default(subscriber);
            engine.shutdown()
        };

        let error = returned
            .err()
            .ok_or("both steps failed, so shutdown must not return Ok")?;
        assert!(
            matches!(error, EngineError::RegistryPoisoned),
            "control: the FIRST failure is the one returned, and it is the gate's: {error:?}"
        );

        let reported: Vec<_> = captured
            .at_level("ERROR")?
            .into_iter()
            .filter(|event| event.mentions("a further engine-shutdown step failed"))
            .collect();
        assert!(
            !reported.is_empty(),
            "the second failing step must be reported — a teardown failure with no trace at all \
             is a swallowed Result, which this codebase forbids outright"
        );
        assert!(
            reported
                .iter()
                .any(|event| event.field("step") == Some("runtime.shutdown")),
            "the report must NAME the step, or the operator cannot tell which half failed: \
             {reported:?}"
        );
        Ok(())
    }

    /// 🔴 A FAILING TEARDOWN STEP DOES NOT CANCEL THE STEPS AFTER IT.
    ///
    /// `shutdown` used to be a chain of `?`, so the first step that failed
    /// returned and every later step — the runtime drain, and the three
    /// `nif_state` teardowns that release the engine's installed seams — simply
    /// never ran. The process then exited with a catalog still installed and an
    /// engine reference still reachable from the NIF table: the exact leak the
    /// function exists to prevent, produced by the error path of the function
    /// itself.
    ///
    /// The reason this went unmeasured is that no drain failure in here can be
    /// produced on demand, so no test ever took the error path at all.
    ///
    /// 🔴 WHY THAT SET IS UNREACHABLE IS STATED IN EXACTLY ONE PLACE, AND IT IS
    /// NOT HERE. See [`crate::RuntimeHandle::shutdown`].
    ///
    /// This comment has now been wrong twice about it, in two different ways —
    /// first "every one is timeout-shaped", then "the shared PRECONDITION: each
    /// needs a worker thread or a beamr publisher in a state no test can
    /// arrange". The second is false for `ProcessExitOutcomeMissingAfterEvent`,
    /// which is a beamr contract breach surfaced through `registry.process_event`
    /// and needs neither. It also reasons from a shared property, which is the
    /// move `RuntimeHandle::shutdown` explicitly forbids — the set is OPEN, so
    /// no property shared by today's members is safe to state about it.
    ///
    /// Two wrong answers in two revisions is what a rule known in two places
    /// does, and the cure is subtraction rather than a third attempt: the
    /// characterisation lives at the one site that owns the drain, and this one
    /// points at it. All that is needed locally is that
    /// `force_process_exit_drain_failure` is the named `#[cfg(test)]` seam that
    /// makes the path reachable at all, and that it cannot reach a shipped
    /// binary.
    ///
    /// **The decisive observable is (c), not (a).** That the error still reaches
    /// the caller is true of the old chain too — it is what the old chain did
    /// *instead of* finishing. Only `installed_workflow_catalog() == None` can
    /// tell "the later steps ran" from "the function returned early", because
    /// `clear_engine_seams` is ordered after the failing step. A test asserting
    /// only the error would pass against the defect.
    #[tokio::test]
    async fn a_failing_teardown_step_does_not_skip_the_ones_after_it()
    -> Result<(), Box<dyn std::error::Error>> {
        let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
        let engine = engine_with_loaded_workflow(store, "checkout", "checkout_deployed")?;

        // Installed explicitly, because `engine_with_loaded_workflow` calls
        // `Engine::new` directly and only `EngineBuilder::build` installs the NIF
        // seams. Without this the assertion below would hold on an engine that
        // never had a catalog to clear — a pass measuring nothing. The control
        // that follows is what caught exactly that on the first cut of this test.
        engine
            .runtime()
            .nif_state()
            .set_workflow_catalog(Arc::clone(engine.workflow_catalog()));

        // Control: the seam under (c) is genuinely installed before the call, so
        // a `None` afterwards is the teardown's doing and not the absence of
        // anything to tear down.
        assert!(
            engine
                .runtime()
                .nif_state()
                .installed_workflow_catalog()
                .is_some(),
            "control: the catalog must be installed before shutdown, or asserting it is \
             cleared afterwards measures nothing"
        );

        engine.runtime().force_process_exit_drain_failure();
        let error = engine
            .shutdown()
            .err()
            .ok_or("an injected drain failure must be reported, not swallowed")?;

        assert!(
            matches!(error, EngineError::ProcessExitRegistryPoisoned),
            "(a) the failure must reach the caller as itself: {error:?}"
        );
        assert!(
            !engine.runtime().engine_tasks().is_epoch_open(),
            "(b) the epoch must be closed — it is closed FIRST, so a shutdown that failed \
             later must still leave it shut"
        );
        assert!(
            engine
                .runtime()
                .nif_state()
                .installed_workflow_catalog()
                .is_none(),
            "(c) THE DECISIVE ONE: `clear_engine_seams` is ordered AFTER the step that \
             failed, so a still-installed catalog means the failure returned early and \
             the engine leaked its seams"
        );
        Ok(())
    }

    #[tokio::test]
    async fn continue_as_new_unknown_workflow_returns_not_found()
    -> Result<(), Box<dyn std::error::Error>> {
        let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
        let engine = engine_with_loaded_workflow(store, "checkout", "checkout_deployed")?;
        let workflow_id = aion_core::WorkflowId::new_v4();
        let run_id = aion_core::RunId::new_v4();

        let result = engine
            .continue_as_new(&workflow_id, &run_id, payload("next")?, None)
            .await;

        assert!(matches!(result, Err(EngineError::WorkflowNotFound { .. })));
        engine.shutdown()?;
        Ok(())
    }

    #[tokio::test]
    async fn list_workflows_merges_live_and_terminal_without_duplicates()
    -> Result<(), Box<dyn std::error::Error>> {
        let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
        let engine =
            engine_with_loaded_workflow(Arc::clone(&store), "checkout", "checkout_deployed")?;
        let running = insert_active_handle(&engine, Arc::clone(&store), "checkout").await?;
        let completed = engine
            .start_workflow(
                "checkout",
                payload("input")?,
                HashMap::new(),
                String::from("default"),
            )
            .await?;
        terminate::complete(
            termination_context(&engine),
            completed.workflow_id(),
            completed.run_id(),
            payload("result")?,
        )
        .await?;

        let summaries = engine.list_workflows(WorkflowFilter::default()).await?;
        assert_eq!(summaries.len(), 2);
        assert!(summaries.iter().any(|summary| {
            &summary.workflow_id == running.workflow_id()
                && summary.status == WorkflowStatus::Running
        }));
        assert!(summaries.iter().any(|summary| {
            &summary.workflow_id == completed.workflow_id()
                && summary.status == WorkflowStatus::Completed
        }));

        let completed_only = engine
            .list_workflows(WorkflowFilter {
                status: Some(WorkflowStatus::Completed),
                ..WorkflowFilter::default()
            })
            .await?;
        assert_eq!(completed_only.len(), 1);
        assert_eq!(&completed_only[0].workflow_id, completed.workflow_id());
        engine.shutdown()?;
        Ok(())
    }

    #[tokio::test]
    async fn shutdown_rejects_subsequent_starts() -> Result<(), Box<dyn std::error::Error>> {
        let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
        let engine =
            engine_with_loaded_workflow(Arc::clone(&store), "checkout", "checkout_deployed")?;
        let handle = engine
            .start_workflow(
                "checkout",
                payload("input")?,
                HashMap::new(),
                String::from("default"),
            )
            .await?;
        terminate::complete(
            termination_context(&engine),
            handle.workflow_id(),
            handle.run_id(),
            payload("result")?,
        )
        .await?;

        engine.shutdown()?;
        let result = engine
            .start_workflow(
                "checkout",
                payload("after-shutdown")?,
                HashMap::new(),
                String::from("default"),
            )
            .await;

        assert!(matches!(result, Err(EngineError::ShuttingDown)));
        Ok(())
    }

    #[tokio::test]
    async fn shutdown_is_idempotent() -> Result<(), Box<dyn std::error::Error>> {
        let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
        let engine =
            engine_with_loaded_workflow(Arc::clone(&store), "checkout", "checkout_deployed")?;
        let handle = engine
            .start_workflow(
                "checkout",
                payload("input")?,
                HashMap::new(),
                String::from("default"),
            )
            .await?;
        terminate::complete(
            termination_context(&engine),
            handle.workflow_id(),
            handle.run_id(),
            payload("result")?,
        )
        .await?;

        engine.shutdown()?;
        let second = engine.shutdown();

        assert!(
            second.is_ok(),
            "double shutdown should succeed; got {second:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn shutdown_rejects_schedule_creation() -> Result<(), Box<dyn std::error::Error>> {
        let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
        let engine =
            engine_with_loaded_workflow(Arc::clone(&store), "checkout", "checkout_deployed")?;
        let handle = engine
            .start_workflow(
                "checkout",
                payload("input")?,
                HashMap::new(),
                String::from("default"),
            )
            .await?;
        terminate::complete(
            termination_context(&engine),
            handle.workflow_id(),
            handle.run_id(),
            payload("result")?,
        )
        .await?;
        engine.shutdown()?;

        let config = aion_core::ScheduleConfig {
            trigger: aion_core::TriggerSpec::Interval {
                period: Duration::from_secs(60),
            },
            overlap_policy: aion_core::OverlapPolicy::Skip,
            catch_up_policy: aion_core::CatchUpPolicy::Skip,
            workflow_type: String::from("checkout"),
            input: payload("scheduled")?,
            search_attributes: HashMap::new(),
        };
        let result = engine.create_schedule(config).await;

        assert!(
            matches!(result, Err(EngineError::ShuttingDown)),
            "create_schedule after shutdown should return ShuttingDown; got {result:?}"
        );
        Ok(())
    }
}
