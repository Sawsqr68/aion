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
    /// One-shot slot for deferred startup recovery (#266). `NotDeferred` on a
    /// default build; `Pending` until [`Engine::run_startup_recovery`]
    /// consumes it.
    pub(super) deferred_startup_recovery:
        std::sync::Mutex<super::startup_deferred::DeferredRecoverySlot>,
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
    /// Closes the engine-task epoch with `EngineTaskRuntime::shutdown`, whose
    /// runtime drop is isolated on a plain joiner thread. That makes joined
    /// cleanup safe even when this `Drop` runs inside a host async context: the
    /// epoch is gated, every task is aborted, and the executor's I/O driver is
    /// released before `Drop` returns. The gate remains load-bearing for an
    /// attempt already past an await boundary: its append boundary reads
    /// `is_epoch_open` and refuses.
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
    /// the `visibility_reconciliation_task.abort()` above. It therefore remains
    /// safe before the joined engine-task shutdown below.
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
    /// Trading a scheduler leak for a use-after-clear is the wrong direction,
    /// so that leak stands and is named: **an engine released without explicit
    /// `shutdown` still holds its scheduler and installed seams.** The dedicated
    /// engine-task executor is different: it is joined below so its I/O driver
    /// cannot accumulate process descriptors. What this drop guarantees for
    /// durability is still narrower — **no durable writer this drop can reach
    /// keeps writing, and no writer it cannot reach can end a run.** The first
    /// clause covers FOUR BACKGROUND writers, stopped in two different ways:
    ///
    /// 1. anything armed on the **engine-task epoch** — `shutdown()` below;
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
    ///    the engine-task epoch closure is what silences it, one indirection away.
    ///
    /// # 🔴 AND THERE IS A FIFTH, WHICH IS NOT A BACKGROUND WRITER AT ALL
    ///
    /// The four above are things the engine spawned; this drop stops them
    /// because it can reach them. The fifth is the **workflow process itself**,
    /// and this drop deliberately does not stop it — it leaves the beamr
    /// scheduler running and the NIF seams installed, which is exactly what the
    /// section above says it is trading for. A still-runnable workflow process
    /// therefore keeps calling NIFs after the `Engine` is gone, and **13 of the
    /// 24 registered engine NIFs perform durable writes** — `dispatch_activity`,
    /// `dispatch_activity_in_vm`, `await_activity_result`, `sleep`,
    /// `start_timer`, `cancel_timer`, `with_timeout`, `continue_as_new`,
    /// `send_signal`, `spawn_child`, `collect_all`, `collect_race`,
    /// `collect_map`. The other 11 read or reply and record nothing. The
    /// registration table is `runtime::engine_nifs::engine_nif_entries`, whose
    /// own test asserts the total, so both halves of that split are checkable
    /// against a closed set rather than taken on trust — which is the point,
    /// since the first draft of this paragraph carried a transposed count.
    /// None of the 13 consults the engine-task epoch, and
    /// nothing in the append path does either: `NifContext::block_on_recorder`
    /// takes the recorder lock and nothing else, and `Recorder::append_one`
    /// goes straight to `store.append`.
    ///
    /// An earlier revision of this doc said "there are FOUR" full stop, and was
    /// wrong in the way that matters most: it did not omit an obscure writer, it
    /// omitted **the one that executes user code**.
    ///
    /// What has been closed is the part that can END A RUN.
    /// `WorkflowContinuedAsNew` is a TERMINAL, it was the ONE terminal this
    /// fifth writer could still record, and it is now refused off the same epoch
    /// (`runtime::nif_continue_as_new::record_continuation`). The reason it had
    /// to be, in one line: **the successor run that terminal obliges was already
    /// refused** at `completion::start_continuation_replacement`, so the two
    /// halves of one transition disagreed and the run was left terminal with no
    /// continuation. Every other terminal reachable from workflow code was
    /// already gated — process exit at the completion append boundary,
    /// `WorkflowTimedOut` off the timer bridge's stand-down latch.
    ///
    /// **And the refusal ENDS THE PROCESS, which is the half that makes it a
    /// gain rather than a trade.** Before the gate, the recorder call either
    /// succeeded or aborted the NIF, and the success path always reached
    /// `cancel_pid` — that instruction is where this fifth writer died. A
    /// refusal that merely returned early would have removed it, leaving the
    /// process runnable and free to make every ungated write listed below. So
    /// `runtime::nif_continue_as_new` terminates on the epoch refusal too — and,
    /// of the refusals, on that one ONLY. A pre-terminal store fault is an
    /// ordinary error workflow code may handle, and killing a process for it
    /// would turn a transient blip into a dead run; an already-terminal run is
    /// spared for a different reason — its terminal was recorded by a seam
    /// that owns its own teardown, and of those owners some end the pid (a
    /// second `cancel_pid` from here would race them) while some only
    /// deregister (a kill from here would usurp them). The predicate's doc
    /// carries that split; the "Five ordinary terminal paths" paragraph in
    /// `lifecycle/completion.rs` carries the one enumeration of the owners.
    /// It ALSO terminates whenever the terminal actually landed, including
    /// the half-completed case where the terminal is durable but the deadline
    /// retirement that follows it failed — because the question that decides
    /// this is "did the terminal land", not "was there an error". The
    /// predicate is `outcome_must_end_the_process`, pinned by a test with both
    /// negative controls.
    ///
    /// The cost, stated because it is not zero: the refusal returns before
    /// `retire_run_deadline`, so the predecessor's deadline row stays armed. A
    /// restart gap longer than the run's remaining budget times the run out
    /// instead of continuing it. That is the same exposure every other in-flight
    /// run already carries across an outage; the old path escaped it only by
    /// recording a terminal for a transition that never completed.
    ///
    /// # 🔴 WHAT IS STILL OPEN, AND WHY IT IS NOT CLOSED HERE
    ///
    /// A workflow process refused by the EPOCH gate is now stopped, so the
    /// writes below are not reachable from that path. Say "the epoch gate" and
    /// not "was refused": the other refusals deliberately leave the process
    /// alive, so a reader who takes this sentence at its widest reading would
    /// believe an exposure is closed that is open by design.
    ///
    /// They remain fully open on every other path — a process that never calls
    /// `continue_as_new` is untouched by any of this and keeps writing.
    ///
    /// The fifth writer's NON-terminal durable writes are ungated and remain so:
    /// `TimerStarted` plus a durable timer row (`sleep`, `start_timer`,
    /// `with_timeout` — `TimerService::schedule` writes the row and only then
    /// arms, so the wheel's refusal lands after both), activity schedule/start
    /// and completion records, `spawn_child`'s whole child-start chain, and
    /// `send_signal`, which writes into a THIRD workflow's history.
    ///
    /// Two things bound that, and neither is what a reader might assume:
    /// - `WriteToken` fences NOTHING. It is a zero-sized marker with a public
    ///   `recorder()` constructor and no engine, epoch, lease or node identity;
    ///   two engines over one store both mint valid ones. Its own doc says so —
    ///   it exists to stop an `Arc<dyn EventStore>` alone being write authority.
    /// - `SequenceConflict` catches only the LOSER of a head race, and a
    ///   released engine is structurally positioned to be the winner: its
    ///   Recorder is the one already at the current head, because it is the one
    ///   that has been appending. If it writes first, its write succeeds and the
    ///   SUCCESSOR takes the conflict.
    ///
    /// So the remaining exposure is real and is stated rather than denied. It is
    /// not closed here because **no flag in this crate distinguishes "released"
    /// from "shutting down"** — `begin_close` sets one bit and both `Engine::drop`
    /// and `Engine::shutdown` set it. A gate on that bit at a workflow-process
    /// write path would therefore also fire during an ORDINARY graceful
    /// shutdown, for the whole unbounded span between `begin_close()` and
    /// `runtime.shutdown()` further down this file, and there the failure is an
    /// `{error, _}` returned INSIDE running workflow code — a failed `sleep`, a
    /// failed `spawn_child` — on runs the shutdown was trying to leave intact.
    /// The terminal was worth that trade because its successor was already
    /// refused at `start_continuation_replacement`: recording it could only
    /// produce a run that is terminal with no continuation.
    ///
    /// ⚠️ **Refusing it is not free, and an earlier revision of this sentence
    /// said it was.** It read "refusing cost nothing that was not already lost",
    /// which is the exact claim `runtime::nif_continue_as_new`'s own
    /// documentation exists to retract — and which the "cost, stated because it
    /// is not zero" paragraph above already contradicts. The price is stated
    /// there and holds here: the refusal returns before `retire_run_deadline`,
    /// so the predecessor's deadline stays armed and a long enough outage
    /// times the run out instead of continuing it. What makes the trade worth
    /// taking is not that it is free but that the alternative bought its
    /// exemption with a false terminal.
    ///
    /// Refusing ordinary progress is a different bargain and needs a latch that
    /// means what it says. Do not add one of these gates without adding that
    /// latch.
    ///
    /// 🔴 THAT LIST IS A CLAIM ABOUT DURABLE WRITERS AND IT IS ONLY AS GOOD AS
    /// ITS ENUMERATION — four times proven. An earlier revision named two and
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
    /// see, because the list looked complete.
    ///
    /// And the FOURTH time is the section above: every revision so far had
    /// enumerated only what this drop *reaches*, and then written a guarantee
    /// over every writer that *exists*. The workflow process is not on any list
    /// of things a `Recorder` grep or a `spawn` grep produces, because nobody
    /// spawned it here and it holds no handle this file can see — it is reached
    /// through an installed NIF seam by code the operator wrote. **A search
    /// shaped like the mechanism you already know will not find the writer you
    /// do not.** That is why the method below now starts from the NIF
    /// registration table, which is a closed set that something asserts the size
    /// of, rather than from a grep whose completeness nothing checks.
    ///
    /// The way to check this list is: take
    /// `runtime::engine_nifs::engine_nif_entries` and account for every entry;
    /// grep the crate for every construction of a `Recorder` handle and every
    /// detached `spawn`; and then, for each writer either search yields, follow
    /// the ACTUAL route from the wake to the append and confirm the named gate
    /// sits on it. Not to re-read this sentence and find it plausible.
    fn drop(&mut self) {
        if let Some(task) = &self.visibility_reconciliation_task {
            task.abort();
        }
        self.runtime.nif_state().shutdown_timer_wheel();
        self.runtime.engine_tasks().shutdown();
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
    /// `Some` when the builder deferred startup recovery (#266): the stowed
    /// recovery inputs [`Engine::run_startup_recovery`] consumes. `None` when
    /// `build()` ran recovery itself, as it does by default.
    pub(crate) deferred_startup_recovery: Option<super::startup_deferred::DeferredStartupRecovery>,
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
            deferred_startup_recovery,
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
            deferred_startup_recovery: super::startup_deferred::DeferredRecoverySlot::from_build(
                deferred_startup_recovery,
            ),
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
mod api_tests;
