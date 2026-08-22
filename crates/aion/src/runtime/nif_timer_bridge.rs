//! Engine-seam bridge backing the timer NIFs.
//!
//! The bridge adapts the engine's registry, event store, and tokio runtime to
//! the [`EngineHandle`] seam consumed by [`TimerService`], and owns the live
//! timer wheel (armed tokio sleep tasks keyed per process and timer id).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use aion_core::{Event, TimerCancelCause, TimerId, WorkflowFilter, WorkflowId, WorkflowSummary};
use aion_store::{EventStore, ReadableEventStore, RunSummary, StoreError, TimerEntry};
use chrono::{DateTime, Utc};
use dashmap::{DashMap, DashSet};
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

use crate::engine_seam::{
    ChildWorkflowSpawnRequest, ChildWorkflowSpawnResult, EngineHandle, EngineSeamError,
    RecordOutcome, TimerWheelEntry, WorkflowMailboxMessage, WorkflowProcessHandle,
    WorkflowResidency,
};
use crate::registry::Registry;
use crate::runtime::nif_state::EngineNifState;
use crate::runtime::nif_timer::NifTimerError;
use crate::runtime::nif_timer_fire::{
    RefusedAppend, TimerAppendError, active_run_has_terminal, event_kind, fire_wheel_timer,
    wheel_torn_down_before_arming,
};
use crate::time::timer_service::{TimerDisposition, timer_disposition_in_active_segment};
use crate::time::{DeadlineHandler, TimerService};

pub(super) struct TimerNifBridge {
    pub(super) registry: Arc<Registry>,
    pub(super) store: Arc<dyn ReadableEventStore>,
    pub(super) tokio_handle: Handle,
    /// Builder-supplied bound for the registry-registration birth wait.
    pub(super) birth_wait: crate::runtime::SignalDeliveryConfig,
    pending_timers: DashMap<(WorkflowProcessHandle, TimerId), PendingTimerTask>,
    next_timer_generation: AtomicU64,
    /// Set by [`Self::shutdown_timer_wheel`] BEFORE it drains, and never
    /// cleared: this engine's wheel is closed for good once teardown begins.
    ///
    /// 🔴 A DRAIN IS NOT A GATE, and until 2026-08-07 this bridge had only the
    /// drain. `shutdown_timer_wheel` removed and aborted every task pending at
    /// one instant, while [`Self::arm_timer`] spawned unconditionally — so a
    /// workflow process still runnable on the beamr scheduler could reach
    /// `sleep` after the drain and arm a fresh durable `TimerFired` writer.
    /// `Engine::shutdown` was not exposed to it (it stops the scheduler before
    /// draining), but `Drop for Engine` deliberately leaves the scheduler and
    /// the seams alive, so the window it opened was unbounded — and what
    /// escapes through it is precisely the #119 failover race this drain
    /// exists to prevent: a released engine recording a `TimerFired` for a run
    /// a successor has already adopted, which is a second writer for that run.
    /// `Arc` so the append boundary can read it from inside the blocking future:
    /// [`Self::record_workflow_event`] hands a clone there, which is the only
    /// place a check can catch a task already past its `abort` point at all.
    ///
    /// 🔴 THAT IS REACH, NOT DECISIVENESS, and three comments in this crate
    /// said "decisive" before one reviewer checked. Nothing that sets this flag
    /// takes a lock any reader shares, so every read of it anywhere is
    /// check-then-act. What is decisive is the arm/insert composition on
    /// [`Self::shutdown_timer_wheel`] — two `SeqCst` reads around an insert —
    /// and that guarantee belongs to the composition, not to any single gate.
    shut_down: Arc<AtomicBool>,
    /// Test-only interleaving seam, fired inside [`Self::arm_timer`] after the
    /// pre-arm gate and the spawn and immediately BEFORE the insert.
    ///
    /// It exists because the window this fix closes is a genuine two-thread
    /// race with no natural trigger: a test that merely calls
    /// `shutdown_timer_wheel` and then `arm_timer` exercises the pre-arm gate,
    /// which was already correct, and would stay green with the post-insert
    /// re-read deleted. Firing the teardown from exactly inside the window is
    /// the only way to put a test on the path of the code being fixed.
    #[cfg(test)]
    arm_interleave: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    // Weak: the engine state owns this bridge through its timer slot.
    nif_state: Weak<EngineNifState>,
    /// Engine-registered handler for reserved `deadline:{run_id}` fires,
    /// installed by [`register_deadline_handler`] after engine seams are wired
    /// and before startup timer recovery runs. Every [`Self::service`] hands it
    /// to the `TimerService` it constructs, so both the live wheel and
    /// `recover_due` route a deadline fire to the engine instead of recording a
    /// generic `TimerFired`.
    deadline_handler: Mutex<Option<Arc<dyn DeadlineHandler>>>,
    /// The single per-timer first-recorded-wins coordinator shared by every
    /// [`TimerService`] this bridge constructs, so a cancel obtained from one
    /// service instance and a fire obtained from another mutually exclude per
    /// timer. Owning it here (not per service) is what makes the guard real
    /// across the separately-obtained services the live wheel and `Engine::cancel`
    /// use.
    terminal_updates: Arc<DashSet<(WorkflowId, TimerId)>>,
}

struct PendingTimerTask {
    generation: u64,
    handle: JoinHandle<()>,
}

struct ReadableEventStoreAdapter {
    store: Arc<dyn EventStore>,
}

#[async_trait::async_trait]
impl ReadableEventStore for ReadableEventStoreAdapter {
    async fn read_history(&self, workflow_id: &WorkflowId) -> Result<Vec<Event>, StoreError> {
        self.store.read_history(workflow_id).await
    }

    async fn read_history_from(
        &self,
        workflow_id: &WorkflowId,
        from_seq: u64,
    ) -> Result<Vec<Event>, StoreError> {
        self.store.read_history_from(workflow_id, from_seq).await
    }

    async fn read_run_chain(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<RunSummary>, StoreError> {
        self.store.read_run_chain(workflow_id).await
    }

    async fn list_active(&self) -> Result<Vec<WorkflowId>, StoreError> {
        self.store.list_active().await
    }

    async fn list_paused(&self) -> Result<Vec<WorkflowId>, StoreError> {
        self.store.list_paused().await
    }

    async fn list_workflow_ids(&self) -> Result<Vec<WorkflowId>, StoreError> {
        self.store.list_workflow_ids().await
    }

    async fn query(&self, filter: &WorkflowFilter) -> Result<Vec<WorkflowSummary>, StoreError> {
        self.store.query(filter).await
    }

    async fn schedule_timer(
        &self,
        workflow_id: &WorkflowId,
        timer_id: &TimerId,
        fire_at: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.store
            .schedule_timer(workflow_id, timer_id, fire_at)
            .await
    }

    async fn expired_timers(&self, as_of: DateTime<Utc>) -> Result<Vec<TimerEntry>, StoreError> {
        self.store.expired_timers(as_of).await
    }
}

impl TimerNifBridge {
    pub(super) fn service(self: &Arc<Self>) -> TimerService {
        let engine: Arc<dyn EngineHandle> = self.clone();
        let store: Arc<dyn ReadableEventStore> = self.store.clone();
        let service = TimerService::new(engine, store)
            .with_terminal_updates(Arc::clone(&self.terminal_updates));
        match self.deadline_handler() {
            Some(handler) => service.with_deadline_handler(handler),
            None => service,
        }
    }

    /// The engine-registered deadline handler, if one has been installed.
    fn deadline_handler(&self) -> Option<Arc<dyn DeadlineHandler>> {
        match self.deadline_handler.lock() {
            Ok(handler) => handler.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    /// Abort every armed live-wheel timer task this engine owns.
    ///
    /// Called from BOTH of this engine's teardown paths — `Engine::shutdown`
    /// (`engine/api.rs`) and `impl Drop for Engine` — so a timer this engine
    /// armed cannot fire AFTER it has stopped owning the workflow.
    ///
    /// 🔴 THE `Drop` CALLER IS THE ONE THAT MAKES THE GATE LOAD-BEARING, and an
    /// earlier revision of this line named neither. It read "engine shutdown
    /// (and shard relinquishment)": the parenthetical named a caller that has
    /// never existed — `relinquish` appears nowhere in this crate outside doc
    /// prose — while the caller that actually needs the gate went unmentioned.
    /// `Engine::shutdown` stops the beamr scheduler, so after it no workflow
    /// process can reach `arm_timer` at all and a pure drain would have done;
    /// `Drop` deliberately leaves the scheduler and the seams alive, so there
    /// the flag has to keep refusing long after the drain. A reader who
    /// believed the old line would have taken the drain for the whole
    /// mechanism.
    ///
    /// The armed tasks run on the tokio runtime, not the beamr scheduler, so
    /// `RuntimeHandle::shutdown` (which stops the scheduler and the
    /// wake-confirmation worker) does NOT reach them: without this, a still
    /// pending wheel task fires `TimerService::fire_timer` against the
    /// torn-down engine — recording `TimerFired` for a process that no longer
    /// runs here and (post-shutdown) with no wake-confirmation ladder to heal a
    /// lost wake. That is the #119 failover race: the dead owner's orphaned
    /// timer task races the survivor's adoption-armed timer, and when the dead
    /// owner wins it records the one durable `TimerFired` first, so the
    /// survivor's wheel fire sees the timer already retired and never wakes the
    /// adopted, resident sleeper — which then parks forever. Aborting here
    /// hands the fire cleanly to the new owner; exactly-once is preserved
    /// because the durable `TimerFired` is still recorded exactly once, by
    /// whichever engine actually owns the workflow when the timer elapses.
    pub(super) fn shutdown_timer_wheel(&self) {
        // GATE FIRST, then drain — but the gate is only half the rule, and the
        // half that lives here is not the load-bearing one.
        //
        // 🔴 OBSERVING THE FLAG AS UNSET DOES NOT PUT AN ENTRY IN THE MAP.
        // `arm_timer` reads the flag, then does registry work, then spawns,
        // and only THEN inserts — so an arm that passed the gate before this
        // store can still be spawning while the snapshot below is taken, and
        // its entry lands in a map already drained. That task would outlive
        // the engine holding a live `Weak<EngineNifState>` (this `Drop`
        // deliberately leaves the seams installed) and record a durable
        // `TimerFired` for a run a successor may already own.
        //
        // What closes it is the SECOND read, in `arm_timer` after its insert.
        // With `SeqCst` on this store and on both of that function's loads,
        // the two are totally ordered: either this snapshot sees the entry and
        // aborts it, or that second load sees the flag and retracts the arm.
        // Neither side can be skipped, and the composition — not either half —
        // is the guarantee.
        self.shut_down.store(true, Ordering::SeqCst);
        let keys: Vec<(WorkflowProcessHandle, TimerId)> = self
            .pending_timers
            .iter()
            .map(|entry| entry.key().clone())
            .collect();
        for key in keys {
            if let Some((_, pending)) = self.pending_timers.remove(&key) {
                pending.handle.abort();
            }
        }
    }

    /// Install the interleaving seam described on `arm_interleave`.
    #[cfg(test)]
    pub(super) fn set_arm_interleave(&self, hook: Arc<dyn Fn() + Send + Sync>) {
        match self.arm_interleave.lock() {
            Ok(mut slot) => *slot = Some(hook),
            Err(poisoned) => *poisoned.into_inner() = Some(hook),
        }
    }

    /// Fire the interleaving seam, if one is installed.
    ///
    /// The hook is cloned OUT of the lock before it runs: it calls back into
    /// this bridge, and holding the lock across that call would deadlock a
    /// hook that installs another.
    #[cfg(test)]
    fn fire_arm_interleave(&self) {
        let hook = match self.arm_interleave.lock() {
            Ok(slot) => slot.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        };
        if let Some(hook) = hook {
            hook();
        }
    }

    /// How many wheel timer tasks are armed right now.
    ///
    /// See [`crate::runtime::nif_state::EngineNifState::armed_wheel_timers`] for
    /// why this is read through the NIF state rather than off an `Engine`.
    #[cfg(test)]
    pub(super) fn armed_wheel_timers(&self) -> usize {
        self.pending_timers.len()
    }

    fn workflow_id_for_process(
        &self,
        process: WorkflowProcessHandle,
    ) -> Result<WorkflowId, EngineSeamError> {
        self.registry
            .list()
            .map_err(|error| EngineSeamError::TimerWheel {
                reason: error.to_string(),
            })?
            .into_iter()
            .find(|handle| handle.pid() == process.pid())
            .map(|handle| handle.workflow_id().clone())
            .ok_or_else(|| EngineSeamError::TimerWheel {
                reason: format!("unknown workflow process {}", process.pid()),
            })
    }
}

enum TimerOutcome {
    Fired(TimerId),
    Cancelled(TimerId, TimerCancelCause),
}

impl EngineHandle for TimerNifBridge {
    fn resolve_workflow(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<WorkflowResidency, EngineSeamError> {
        let handle = self
            .registry
            .list()
            .map_err(|error| EngineSeamError::Delivery {
                reason: error.to_string(),
            })?
            .into_iter()
            .find(|handle| handle.workflow_id() == workflow_id);
        Ok(match handle {
            Some(handle) if handle.residency() == crate::HandleResidency::Resident => {
                WorkflowResidency::Resident(WorkflowProcessHandle::new(handle.pid()))
            }
            Some(_) => WorkflowResidency::NonResident,
            None => WorkflowResidency::Unknown,
        })
    }

    fn deliver_workflow_message(
        &self,
        process: WorkflowProcessHandle,
        message: WorkflowMailboxMessage,
    ) -> Result<(), EngineSeamError> {
        match message {
            WorkflowMailboxMessage::TimerFired { .. } => {
                // The fired terminal is already durably recorded
                // (record-before-deliver in `TimerService::fire_timer`), so
                // delivery is a pure wake: the suspended await re-runs its
                // two-phase resolution and reads the outcome from history.
                let nif_state =
                    self.nif_state
                        .upgrade()
                        .ok_or_else(|| EngineSeamError::Delivery {
                            reason: "engine NIF state has been dropped".to_owned(),
                        })?;
                let runtime =
                    super::nif_activity::runtime_context(&nif_state).map_err(|error| {
                        EngineSeamError::Delivery {
                            reason: error.to_string(),
                        }
                    })?;
                runtime
                    .runtime
                    .wake_workflow(process.pid())
                    .map_err(|error| EngineSeamError::Delivery {
                        reason: error.to_string(),
                    })
            }
            other => Err(EngineSeamError::Delivery {
                reason: format!("unsupported timer NIF bridge mailbox message: {other:?}"),
            }),
        }
    }

    fn spawn_child_workflow(
        &self,
        request: ChildWorkflowSpawnRequest,
    ) -> Result<ChildWorkflowSpawnResult, EngineSeamError> {
        let _ = request;
        Err(EngineSeamError::ChildSpawn {
            reason: "timer NIF bridge does not spawn child workflows".to_owned(),
        })
    }

    fn terminate_linked_child_workflow(
        &self,
        parent_workflow_id: &WorkflowId,
        child_process: WorkflowProcessHandle,
        correlation: u64,
    ) -> Result<(), EngineSeamError> {
        let _ = (parent_workflow_id, child_process, correlation);
        Err(EngineSeamError::ChildTermination {
            reason: "timer NIF bridge does not terminate child workflows".to_owned(),
        })
    }

    fn terminate_linked_activity(
        &self,
        parent_workflow_id: &WorkflowId,
        activity_process: crate::Pid,
        correlation: u64,
    ) -> Result<(), EngineSeamError> {
        let _ = (parent_workflow_id, activity_process, correlation);
        Err(EngineSeamError::ChildTermination {
            reason: "timer NIF bridge does not terminate activities".to_owned(),
        })
    }

    fn arm_timer(&self, entry: TimerWheelEntry) -> Result<(), EngineSeamError> {
        // 🔴 REFUSE ONCE THIS ENGINE'S WHEEL HAS BEEN TORN DOWN. Arming spawns
        // a task that appends a durable `TimerFired`, so an arm after teardown
        // is a second writer for a run a successor engine may already own.
        // Refusing is the safe direction: the timer is still durable in
        // history, and the owning engine's startup recovery re-arms it from
        // there. See `shut_down` on this struct for the mechanism.
        if self.shut_down.load(Ordering::SeqCst) {
            return Err(wheel_torn_down_before_arming(&entry.timer_id));
        }
        let workflow_id = self.workflow_id_for_process(entry.process)?;
        let key = (entry.process, entry.timer_id.clone());
        if let Some((_, previous)) = self.pending_timers.remove(&key) {
            previous.handle.abort();
        }

        let fire_at = entry.fire_at;
        let timer_id = entry.timer_id.clone();
        let task_key = key.clone();
        let generation = self.next_timer_generation.fetch_add(1, Ordering::Relaxed);
        let delay = match (fire_at - Utc::now()).to_std() {
            Ok(delay) => delay,
            Err(_) => Duration::ZERO,
        };
        let nif_state = Weak::clone(&self.nif_state);
        let handle = self.tokio_handle.spawn(async move {
            tokio::time::sleep(delay).await;
            fire_wheel_timer(&nif_state, &workflow_id, &timer_id, fire_at).await;
            if let Some(bridge) = nif_state
                .upgrade()
                .and_then(|state| timer_bridge(&state).ok())
            {
                if bridge
                    .pending_timers
                    .get(&task_key)
                    .is_some_and(|pending| pending.generation == generation)
                {
                    bridge.pending_timers.remove(&task_key);
                }
            }
        });
        // The window this function's second gate closes, made reachable by a
        // test. Fired here — after the spawn, before the insert — because that
        // is exactly where a concurrent `shutdown_timer_wheel` does its damage.
        #[cfg(test)]
        self.fire_arm_interleave();
        let undo_key = key.clone();
        self.pending_timers
            .insert(key, PendingTimerTask { generation, handle });
        // 🔴 RE-READ THE GATE AFTER THE INSERT. This is the half of the rule
        // that actually closes the window; the check at the top of this
        // function only saves the work of arming when teardown is already
        // known. `shutdown_timer_wheel` can set the flag and take its snapshot
        // anywhere between that check and this insert — across a registry
        // read, a `remove`, and a `spawn` — and this entry would then land in a
        // map that has already been drained, leaving a live task holding a
        // `Weak<EngineNifState>` the engine's `Drop` deliberately keeps
        // upgradable. It would record a durable `TimerFired` for a run a
        // successor may already own: the #119 second-writer breach, and a
        // violation of the single-writer invariant.
        //
        // `SeqCst` on the store and on both loads totally orders the two, so
        // either the drain's snapshot contains this entry or this load sees
        // the flag. There is no interleaving in which neither happens.
        //
        // 🔴 Retract by GENERATION IDENTITY, never by key alone. Removing by
        // key retracts whatever occupies it now, which need not be this arm:
        // this task could have completed and released its own entry, and a
        // later arm taken the key, which we would then abort under the later
        // arm's honest success. `EngineTaskRuntime::arm` takes the same care
        // for the same reason.
        if self.shut_down.load(Ordering::SeqCst) {
            if let Some((_, pending)) = self
                .pending_timers
                .remove_if(&undo_key, |_, task| task.generation == generation)
            {
                pending.handle.abort();
            }
            return Err(wheel_torn_down_before_arming(&entry.timer_id));
        }
        Ok(())
    }

    fn disarm_timer(
        &self,
        process: WorkflowProcessHandle,
        timer_id: &TimerId,
    ) -> Result<(), EngineSeamError> {
        if let Some((_, pending)) = self.pending_timers.remove(&(process, timer_id.clone())) {
            pending.handle.abort();
        }
        Ok(())
    }

    fn record_workflow_event(
        &self,
        workflow_id: &WorkflowId,
        event: Event,
    ) -> Result<RecordOutcome, EngineSeamError> {
        let recorded_at = *event.recorded_at();
        let outcome = match event {
            Event::TimerFired { timer_id, .. } => TimerOutcome::Fired(timer_id),
            Event::TimerCancelled {
                timer_id, cause, ..
            } => TimerOutcome::Cancelled(timer_id, cause),
            other => {
                return Err(EngineSeamError::Recorder {
                    reason: format!("timer NIF bridge cannot record {}", event_kind(&other)),
                });
            }
        };
        let handle = self
            .registry
            .list()
            .map_err(|error| EngineSeamError::Recorder {
                reason: error.to_string(),
            })?
            .into_iter()
            .find(|handle| handle.workflow_id() == workflow_id)
            .ok_or_else(|| EngineSeamError::UnknownWorkflow {
                workflow_id: workflow_id.clone(),
            })?;
        let recorder = handle.recorder();
        let store = Arc::clone(&self.store);
        let workflow_id = workflow_id.clone();
        let shut_down = Arc::clone(&self.shut_down);
        // Carry the OUTCOME across the boundary, not just the id. A refused fire
        // and a refused cancel are the same cause with opposite consequences for
        // the run, and collapsing them here is exactly what let one sentence be
        // raised for both — see `TimerAppendError::WheelTornDown`.
        //
        // 🔴 THE CAUSE TRAVELS TOO, and dropping it with a `_` was this same
        // defect one level down: a `CancelTeardown` cancel got the sentence
        // written for a `WorkflowIntent` one, telling an operator to wait for
        // the run to reissue a cancellation when the run is being cancelled and
        // will never re-execute.
        let (torn_down_timer, torn_down_refused) = match &outcome {
            TimerOutcome::Fired(timer_id) => (timer_id.clone(), RefusedAppend::Fire),
            TimerOutcome::Cancelled(timer_id, cause) => {
                (timer_id.clone(), RefusedAppend::Cancel(*cause))
            }
        };
        run_blocking(&self.tokio_handle, async move {
            let mut recorder = recorder.lock().await;
            // 🔴 REFUSE THE APPEND IF THIS ENGINE'S WHEEL IS TORN DOWN. The
            // arming gate cannot cover this: `shutdown_timer_wheel` aborts the
            // armed tasks, but `JoinHandle::abort` does not stop a task that
            // has already entered a poll, so a fire that reached this point
            // before the abort landed would complete its append regardless.
            // Read INSIDE the recorder lock, and immediately before the append
            // — but that is NARROWING, not closing, and an earlier version of
            // this comment called it "decisive rather than another
            // check-then-act", which was false. `shutdown_timer_wheel` sets the
            // flag with an atomic store and a map drain and takes no recorder
            // lock, so this lock excludes other recorder writers and excludes
            // nothing about the flag. What actually covers the arm/insert race
            // is the composition documented on `shutdown_timer_wheel` — the
            // second read after the insert — not this gate wearing a lock.
            //
            // The refusal leaves the timer durable in history for whichever
            // engine owns the run, which costs a FIRE nothing and costs a
            // CANCEL a deferral, so the outcome travels with the refusal rather
            // than being flattened out of it.
            //
            // Without this the wheel is the one durable writer in the crate
            // with no boundary refusal at all, which is precisely the shape
            // that produced the #119 failover race.
            if shut_down.load(Ordering::SeqCst) {
                return Err(TimerAppendError::WheelTornDown {
                    timer_id: torn_down_timer,
                    refused: torn_down_refused,
                });
            }
            // Late-append refusal under the SAME recorder lock that records the
            // timer event: if the active run already recorded a terminal, refuse
            // ALL late timer appends (fire OR cancel) cleanly — no post-terminal
            // `TimerFired`, no wake. A parked sleep that elapses in the
            // post-terminal window is thereby refused rather than mutating a
            // terminal history, closing the whole late-timer class, not only the
            // deadline case.
            let history = store
                .read_history(&workflow_id)
                .await
                .map_err(TimerAppendError::append)?;
            if active_run_has_terminal(&history) {
                // Late fire/cancel after the run terminated: append nothing and
                // report the refusal so the caller withholds the mailbox wake.
                return Ok(RecordOutcome::RefusedTerminal);
            }
            match outcome {
                TimerOutcome::Fired(timer_id) => {
                    // #145 ack-loss reconciliation, under the SAME recorder lock
                    // and against the SAME history read as the terminal refusal
                    // above. If the active segment already shows `TimerFired` as
                    // this timer's last event, that event can only be this
                    // recorder's own earlier append whose acknowledgement was
                    // lost (single writer per workflow; the timer service's
                    // fire guard never sends a fire here for a timer another
                    // path retired). Appending again would double-record the
                    // fire, and appending anything ELSE would die on
                    // `SequenceConflict`, because the un-acked append left the
                    // recorder's tracked head one below the durable head. So:
                    // reconcile the tracked head FORWARD to the durable head and
                    // report `AlreadyRecorded`, appending nothing — the caller
                    // still delivers the owed wake. The timer service performs
                    // the same inspection before calling in (it keeps genuine
                    // no-ops out of this seam); THIS check, under the lock, is
                    // the authoritative one. Reconciliation happens ONLY on this
                    // branch — never in response to a `SequenceConflict`, which
                    // must keep surfacing un-resynced as the double-writer alarm
                    // it is.
                    if matches!(
                        timer_disposition_in_active_segment(&history, &timer_id),
                        TimerDisposition::Fired
                    ) {
                        let durable_head = history.iter().map(Event::seq).max().unwrap_or_default();
                        recorder.reconcile_head_forward(durable_head);
                        return Ok(RecordOutcome::AlreadyRecorded);
                    }
                    recorder
                        .record_timer_fired(recorded_at, timer_id)
                        .await
                        .map_err(TimerAppendError::append)?;
                }
                TimerOutcome::Cancelled(timer_id, cause) => {
                    recorder
                        .record_timer_cancelled(recorded_at, timer_id, cause)
                        .await
                        .map_err(TimerAppendError::append)?;
                }
            }
            Ok(RecordOutcome::Recorded)
        })
        .map_err(TimerAppendError::into_seam_error)
    }
}

/// Install the engine-scoped timer bridge used by raw NIF function pointers.
pub(crate) fn install_timer_nif_bridge(
    state: &Arc<EngineNifState>,
    registry: Arc<Registry>,
    store: Arc<dyn EventStore>,
    tokio_handle: Handle,
    birth_wait: crate::runtime::SignalDeliveryConfig,
) {
    let store: Arc<dyn ReadableEventStore> = Arc::new(ReadableEventStoreAdapter { store });
    let bridge = Arc::new(TimerNifBridge {
        registry,
        store,
        tokio_handle,
        birth_wait,
        pending_timers: DashMap::new(),
        next_timer_generation: AtomicU64::new(0),
        shut_down: Arc::new(AtomicBool::new(false)),
        #[cfg(test)]
        arm_interleave: Mutex::new(None),
        nif_state: Arc::downgrade(state),
        deadline_handler: Mutex::new(None),
        terminal_updates: Arc::new(DashSet::new()),
    });
    match state.timer_bridge.lock() {
        Ok(mut installed) => *installed = Some(bridge),
        Err(poisoned) => *poisoned.into_inner() = Some(bridge),
    }
}

/// Register the engine-side deadline handler on the installed timer bridge.
///
/// Must run after [`install_timer_nif_bridge`] and before startup timer recovery
/// (`recover_timers_on_startup`), so an already-due deadline recovered at boot
/// routes to the engine rather than failing as an unhandled reserved fire.
///
/// # Errors
///
/// Returns the bridge-resolution error string when no timer bridge is installed.
///
/// 🔴 THE HANDLER IS BUILT HERE, FROM THIS BRIDGE'S OWN STAND-DOWN LATCH, and
/// that is why this takes a constructor rather than a finished handler. The
/// deadline handler is the one durable writer the wheel's append boundary never
/// sees: `TimerService::fire_timer_guarded` demuxes a reserved `deadline:{run}`
/// fire straight to it, ahead of the generic record-then-deliver path that
/// [`TimerNifBridge::record_workflow_event`] guards. So the handler must read
/// the same `shut_down` flag [`TimerNifBridge::shutdown_timer_wheel`] sets, or
/// a deadline task already past its `abort` point records a durable
/// `WorkflowTimedOut` for a run a successor engine owns.
///
/// Handing `build` the latch instead of accepting one alongside the handler is
/// SUBTRACTION, not ceremony: a signature that took both would let a caller
/// supply a freshly-constructed flag, which compiles, passes every test that
/// sets that flag itself, and gates nothing in production — a fact known in two
/// places with nothing forcing them to agree. Here there is only one place the
/// flag can come from.
pub(crate) fn register_deadline_handler<F>(state: &EngineNifState, build: F) -> Result<(), String>
where
    F: FnOnce(Arc<AtomicBool>) -> Arc<dyn DeadlineHandler>,
{
    let bridge = timer_bridge(state).map_err(|error| error.to_string())?;
    let handler = build(Arc::clone(&bridge.shut_down));
    match bridge.deadline_handler.lock() {
        Ok(mut slot) => *slot = Some(handler),
        Err(poisoned) => *poisoned.into_inner() = Some(handler),
    }
    Ok(())
}

pub(crate) fn installed_timer_service(state: &EngineNifState) -> Result<Arc<TimerService>, String> {
    timer_bridge(state)
        .map(|bridge| Arc::new(bridge.service()))
        .map_err(|error| error.to_string())
}

pub(super) fn timer_bridge(state: &EngineNifState) -> Result<Arc<TimerNifBridge>, NifTimerError> {
    state
        .timer_bridge
        .lock()
        .map_err(|_| NifTimerError::Context("timer bridge lock is poisoned".to_owned()))?
        .clone()
        .ok_or_else(|| NifTimerError::Context("timer bridge is not configured".to_owned()))
}

/// Drive a future to completion from synchronous bridge code.
///
/// Bridge methods are called both from dirty NIF threads (no ambient tokio
/// runtime — `block_on` directly) and from tasks spawned on the engine
/// runtime itself (the armed-timer fire path), where `Handle::block_on`
/// panics with "Cannot start a runtime from within a runtime". In that case
/// the wait moves to a scoped helper thread so the runtime stays free to
/// drive the future.
pub(super) fn run_blocking<T, F>(handle: &Handle, future: F) -> T
where
    T: Send,
    F: std::future::Future<Output = T> + Send,
{
    if Handle::try_current().is_err() {
        return handle.block_on(future);
    }
    std::thread::scope(
        |scope| match scope.spawn(|| handle.block_on(future)).join() {
            Ok(value) => value,
            Err(panic) => std::panic::resume_unwind(panic),
        },
    )
}

#[cfg(test)]
#[path = "nif_timer_bridge_tests.rs"]
mod tests;
