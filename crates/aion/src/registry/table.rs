//! Active-execution registry keyed by workflow and run identifiers.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use aion_core::{Event, RunId, WorkflowId, run_segment, status_from_events};
use aion_store::EventStore;

use crate::EngineError;

use super::handle::{Residency, WorkflowHandle};
use super::terminal_writer::TerminalWriterReservation;
use super::unrecoverable::UnrecoverableRuns;

type RegistryKey = (WorkflowId, RunId);
type HandleMap = HashMap<RegistryKey, WorkflowHandle>;

/// Everything that can hold a writer for a workflow, behind ONE lock.
///
/// The two maps are deliberately not two mutexes. A live handle and a
/// terminal-writer reservation (#117(c)) are both writers for the same durable
/// event stream, and exactly one of them may exist for a given workflow at a
/// time (invariant #3). Putting them in separate locks would make that
/// exclusion a rule two independent critical sections have to co-operate to
/// keep; putting them in one makes it a property of every operation that can
/// create either, because no path can observe or mutate one without holding the
/// other's lock too.
#[derive(Debug, Default)]
struct Slots {
    handles: HandleMap,
    /// Workflows currently held by a terminal-writer reservation, and the run
    /// each reservation names. At most one per workflow, keyed by workflow
    /// rather than by `(workflow, run)` because a `Recorder` writes the
    /// WORKFLOW's event stream: a reservation on one run excludes a handle on
    /// any run of the same workflow, and vice versa.
    terminal_writers: HashMap<WorkflowId, RunId>,
}

impl Slots {
    /// The run of the only live handle for `workflow_id`, if any.
    ///
    /// Scans the handle map rather than consulting the `index`, and must: the
    /// index is a `WorkflowId -> (RunId, pid)` map with ONE slot per workflow,
    /// so a continue-as-new window that registers two runs and then removes the
    /// newer leaves the older handle live with no index entry at all. The index
    /// answers "which run is current"; only the handle map answers "does any
    /// handle exist". Cancelling a never-alive run is a rare operator action, so
    /// the linear scan buys exactness at a cost nothing is measuring.
    fn any_handle_for(&self, workflow_id: &WorkflowId) -> Option<RunId> {
        self.handles
            .keys()
            .find(|(id, _)| id == workflow_id)
            .map(|(_, run)| run.clone())
    }
}

/// Secondary index mapping a workflow to its single live run.
///
/// Values are `(RunId, pid)`, not a bare pid: the [`RunId`] lets [`remove`]
/// compare-and-delete so a stale run's removal never evicts a newer run that
/// already overwrote the entry during the continue-as-new window.
///
/// [`remove`]: Registry::remove
type LivePidIndex = HashMap<WorkflowId, (RunId, u64)>;

/// The outcome of an identity-checked handle removal.
///
/// Reopen admission needs three answers, not two. A caller that read history,
/// found a reopenable terminal, and then observed a registered handle must know
/// whether the entry it is about to clear is still the one it looked at — a
/// concurrent reopen can append `WorkflowReopened` and register a fresh handle
/// in that window, and clearing by key would evict the winner.
#[derive(Debug)]
pub enum HandleRemoval {
    /// The observed handle was still registered and has been removed.
    Removed(WorkflowHandle),
    /// No handle is registered for the pair. Another caller in the same
    /// pre-append window already cleared it; nothing has been disturbed.
    Absent,
    /// A different handle occupies the slot. It is left untouched, and returned
    /// so the caller can name it in its refusal.
    Replaced(WorkflowHandle),
}

/// Concurrency-safe registry of live workflow process handles.
///
/// The `index` is a `WorkflowId -> (RunId, pid)` secondary index maintained
/// alongside `handles` so the unmatched outbox-completion path can resolve a
/// workflow id to its live pid in O(1) without scanning the handle map.
///
/// # Lock ordering
///
/// `slots` is always locked before `index`; never the reverse. The
/// read-only [`Registry::live_pid`] lookup locks `index` alone. This fixed
/// order rules out lock-order inversion between the two mutexes. The
/// `unrecoverable` set is a third, independent mutex that is never held
/// together with either.
#[derive(Debug, Default)]
pub struct Registry {
    slots: Mutex<Slots>,
    index: Mutex<LivePidIndex>,
    /// Runs this process could not make resident (#117). Lives here because
    /// residency is what the registry is about, and this is the negative half
    /// of it: the runs that have no handle and never will under this build.
    /// It carries its OWN mutex and is never locked while `handles` or `index`
    /// is held, so it sits outside the ordering below rather than extending it.
    unrecoverable: UnrecoverableRuns,
}

impl Registry {
    /// Inserts or replaces the handle for a workflow run.
    ///
    /// Returns the previously registered handle for the same workflow/run, if any.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::RegistryPoisoned`] if the registry lock was
    /// poisoned, and [`EngineError::TerminalWriterHeld`] when a terminal-writer
    /// reservation holds this workflow's writer slot (#117(c)). The refusal is
    /// the other half of that exclusion: a reservation lives only across one
    /// terminal transition, so the caller's run is momentarily un-registerable
    /// rather than permanently so.
    pub fn insert(
        &self,
        key: (WorkflowId, RunId),
        handle: WorkflowHandle,
    ) -> Result<Option<WorkflowHandle>, EngineError> {
        // Lock ordering: slots first, then index. Never the reverse.
        let mut slots = self.slots()?;
        refuse_if_terminal_writer_held(&slots, &key)?;
        let pid = handle.pid();
        let previous = slots.handles.insert(key.clone(), handle);
        // Upsert the live-pid index: the newest run for a workflow id wins,
        // so a continue-as-new replacement points at the new run immediately.
        self.index()?.insert(key.0, (key.1, pid));
        Ok(previous)
    }

    /// Atomically inserts `handle` for `key` only if no handle is already
    /// registered for that exact `(workflow, run)`.
    ///
    /// Returns the handle now registered for `key`: the freshly inserted one when
    /// the slot was empty, or the pre-existing one when it was occupied (leaving
    /// that occupant untouched). The whole check-and-insert runs under the single
    /// `handles` lock, so two concurrent reopens of the same terminal workflow
    /// cannot both insert — exactly one wins the slot and the other observes the
    /// winner's handle. This is the per-workflow serialization point the reopen
    /// operation relies on (invariant #3): the winner's recorder is the sole
    /// writer.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::RegistryPoisoned`] if the registry lock was
    /// poisoned, and [`EngineError::TerminalWriterHeld`] when a terminal-writer
    /// reservation holds this workflow's writer slot (see [`Self::insert`]).
    pub fn insert_if_absent(
        &self,
        key: (WorkflowId, RunId),
        handle: WorkflowHandle,
    ) -> Result<WorkflowHandle, EngineError> {
        // Lock ordering: slots first, then index. Never the reverse.
        let mut slots = self.slots()?;
        refuse_if_terminal_writer_held(&slots, &key)?;
        if let Some(existing) = slots.handles.get(&key) {
            return Ok(existing.clone());
        }
        let pid = handle.pid();
        slots.handles.insert(key.clone(), handle.clone());
        self.index()?.insert(key.0, (key.1, pid));
        Ok(handle)
    }

    /// Looks up a live workflow run handle.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::RegistryPoisoned`] if the registry lock was poisoned.
    pub fn get(&self, id: &WorkflowId, run: &RunId) -> Result<Option<WorkflowHandle>, EngineError> {
        let slots = self.slots()?;
        Ok(slots.handles.get(&(id.clone(), run.clone())).cloned())
    }

    /// Removes a live workflow run handle.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::RegistryPoisoned`] if the registry lock was poisoned.
    pub fn remove(
        &self,
        id: &WorkflowId,
        run: &RunId,
    ) -> Result<Option<WorkflowHandle>, EngineError> {
        // Lock ordering: slots first, then index. Never the reverse.
        let mut slots = self.slots()?;
        let removed = slots.handles.remove(&(id.clone(), run.clone()));
        drop(slots);
        self.forget_live_index_entry(id, run)?;
        Ok(removed)
    }

    /// Removes the handle for `(id, run)` only if it is still the instance the
    /// caller observed, identified by `expected_pid`.
    ///
    /// The compare and the delete happen under one `slots` lock, which is what
    /// distinguishes this from a `get` followed by a `remove`: between those two
    /// calls a concurrent reopen can register its own handle, and removing by
    /// key would evict it. See [`HandleRemoval`].
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::RegistryPoisoned`] if the registry lock was poisoned.
    pub fn remove_if_pid(
        &self,
        id: &WorkflowId,
        run: &RunId,
        expected_pid: u64,
    ) -> Result<HandleRemoval, EngineError> {
        // Lock ordering: slots first, then index. Never the reverse.
        let mut slots = self.slots()?;
        let key = (id.clone(), run.clone());
        let Some(occupant) = slots.handles.get(&key) else {
            return Ok(HandleRemoval::Absent);
        };
        if occupant.pid() != expected_pid {
            return Ok(HandleRemoval::Replaced(occupant.clone()));
        }
        let removed = slots.handles.remove(&key);
        drop(slots);
        self.forget_live_index_entry(id, run)?;
        match removed {
            Some(handle) => Ok(HandleRemoval::Removed(handle)),
            // Unreachable: the occupant was observed under the lock just held.
            // Reported rather than asserted — no `unwrap`/`expect` in library code.
            None => Ok(HandleRemoval::Absent),
        }
    }

    /// Drops the live-pid index entry for `id`, but only while it still points at
    /// `run`. During continue-as-new a newer run has already overwritten the
    /// entry, and that newer run must survive this removal.
    fn forget_live_index_entry(&self, id: &WorkflowId, run: &RunId) -> Result<(), EngineError> {
        let mut index = self.index()?;
        if let std::collections::hash_map::Entry::Occupied(entry) = index.entry(id.clone()) {
            if entry.get().0 == *run {
                entry.remove();
            }
        }
        Ok(())
    }

    /// Returns a snapshot of all live handles without holding the registry lock.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::RegistryPoisoned`] if the registry lock was poisoned.
    pub fn list(&self) -> Result<Vec<WorkflowHandle>, EngineError> {
        let slots = self.slots()?;
        Ok(slots.handles.values().cloned().collect())
    }

    /// Updates only the engine-internal residency for a live workflow run.
    ///
    /// The projected workflow status is not read or changed. If the workflow run
    /// is not registered, no cache is updated and `Ok(None)` is returned.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::RegistryPoisoned`] if the registry lock was poisoned.
    pub fn replace_residency(
        &self,
        id: &WorkflowId,
        run: &RunId,
        residency: Residency,
    ) -> Result<Option<WorkflowHandle>, EngineError> {
        let mut slots = self.slots()?;
        let Some(handle) = slots.handles.get_mut(&(id.clone(), run.clone())) else {
            return Ok(None);
        };

        handle.replace_residency(residency);
        Ok(Some(handle.clone()))
    }

    /// Reconciles a cached handle status against the core event projection
    /// **of the named run**.
    ///
    /// The projected status always wins. If the workflow run is not registered,
    /// no cache is updated and `Ok(None)` is returned. Residency is not read or changed.
    ///
    /// 🔴 THE PROJECTION IS SCOPED TO `run`, AND THAT IS LOAD-BEARING (aion#94).
    /// It used to be `status_from_events(events)` over the caller's whole slice,
    /// and every production caller passes the ENTIRE history for the workflow id.
    /// `status_from_events` is last-lifecycle-event-wins, so after a
    /// continue-as-new — one workflow id, two runs — the last lifecycle event
    /// belongs to the SUCCESSOR, and the predecessor's handle was cached as
    /// `Running` when it was terminal.
    ///
    /// That was not a race. `Engine::list_workflows` reconciles every registered
    /// handle against full history, so a read-only listing wrote it, and the
    /// nif-driven continue-as-new path never removes the predecessor's handle —
    /// so it stood for the life of the engine and was rewritten on every list.
    ///
    /// Taking a `RunId` and then not scoping by it made every call site correct
    /// only under an invariant stated nowhere: that the handle is the workflow's
    /// latest run. Scoping here means no caller has to know that.
    ///
    /// For a workflow's latest run this changes no answer — its segment runs to
    /// the end of history, which is where the backwards scan starts.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::RegistryPoisoned`] if the registry lock was poisoned.
    ///
    /// Returns [`EngineError::RunNotInHistory`] if the registered run has no
    /// `WorkflowStarted` in `events`. The alternative is worse than an error:
    /// an empty slice projects `Running` by default, so the reconciliation whose
    /// purpose is to stop terminal runs being cached as live would itself cache
    /// one as live.
    pub fn reconcile(
        &self,
        id: &WorkflowId,
        run: &RunId,
        events: &[Event],
    ) -> Result<Option<WorkflowHandle>, EngineError> {
        let mut slots = self.slots()?;
        // The registration check comes first deliberately: a run this registry
        // does not track is nothing to reconcile, and reporting a history
        // inconsistency about it would be an error about someone else's run.
        let Some(handle) = slots.handles.get_mut(&(id.clone(), run.clone())) else {
            return Ok(None);
        };

        let segment = run_segment(events, run);
        if segment.is_empty() {
            return Err(EngineError::RunNotInHistory {
                workflow_id: id.clone(),
                run_id: run.clone(),
            });
        }

        handle.replace_projected_status(status_from_events(segment));
        Ok(Some(handle.clone()))
    }

    /// Resolves a workflow id to the pid of its single live run, if any.
    ///
    /// Reads only the secondary index, so it never contends the handle map.
    /// Returns `Ok(None)` when no run for the workflow is currently live — the
    /// expected stale-completion case after a crash or eviction, before
    /// recovery re-arms the run.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::RegistryPoisoned`] if the index lock was poisoned.
    pub fn live_pid(&self, workflow_id: &WorkflowId) -> Result<Option<u64>, EngineError> {
        let index = self.index()?;
        Ok(index.get(workflow_id).map(|(_, pid)| *pid))
    }

    /// Resolves a workflow id to its single live run and that run's pid.
    ///
    /// Run-aware twin of [`Self::live_pid`]: the returned [`RunId`] lets the
    /// caller reject a completion that belongs to a superseded run (e.g. a
    /// prior run after continue-as-new) so the new run's reused ordinal space
    /// is never resolved by a dead run's late activity completion (OBX-011).
    ///
    /// Reads only the secondary index, so it never contends the handle map.
    /// Returns `Ok(None)` when no run for the workflow is currently live.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::RegistryPoisoned`] if the index lock was poisoned.
    pub fn live_run_pid(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Option<(RunId, u64)>, EngineError> {
        let index = self.index()?;
        Ok(index.get(workflow_id).cloned())
    }

    /// The runs this process could not make resident, and why (#117).
    ///
    /// Exposed by reference rather than cloned: the set is the engine's single
    /// live answer to "why is this run not running", and a caller holding a
    /// stale copy is precisely the failure this exists to prevent.
    pub fn unrecoverable(&self) -> &UnrecoverableRuns {
        &self.unrecoverable
    }

    /// Takes the sole terminal-writer reservation for a run that holds no handle
    /// and can never obtain one (#117(c)).
    ///
    /// This call IS the under-lock proof of the non-residency precondition:
    /// nothing is cited from an earlier observation. Under the one `slots` lock
    /// it establishes, atomically, that the workflow has no live handle for ANY
    /// run and no other reservation — and hands back a guard that keeps both
    /// true for as long as it lives. That is why the exclusion is a property of
    /// the registry rather than a check a caller performs and then hopes holds:
    /// between a caller's check and its append there is a window, and between
    /// these two statements there is not.
    ///
    /// The reservation is invisible to [`Self::live_pid`], [`Self::get`], and
    /// [`Self::list`]: it is not a process, and nothing that looks for one may
    /// find it.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::RegistryPoisoned`] if the registry lock was
    /// poisoned, and [`EngineError::TerminalWriterUnavailable`] when the
    /// workflow already has a writer — either a live handle (in which case the
    /// ordinary cancel path applies and this extraordinary one must not) or an
    /// existing reservation.
    pub fn reserve_terminal_writer(
        &self,
        id: &WorkflowId,
        run: &RunId,
        store: Arc<dyn EventStore>,
    ) -> Result<TerminalWriterReservation<'_>, EngineError> {
        let mut slots = self.slots()?;
        if let Some(resident_run) = slots.any_handle_for(id) {
            return Err(EngineError::TerminalWriterUnavailable {
                workflow_id: id.to_string(),
                run_id: run.to_string(),
                holder: format!("run {resident_run} holds a live handle for this workflow"),
            });
        }
        if let Some(held_run) = slots.terminal_writers.get(id) {
            return Err(EngineError::TerminalWriterUnavailable {
                workflow_id: id.to_string(),
                run_id: run.to_string(),
                holder: format!("run {held_run} already holds a terminal-writer reservation"),
            });
        }
        slots.terminal_writers.insert(id.clone(), run.clone());
        drop(slots);
        Ok(TerminalWriterReservation::new(
            self,
            id.clone(),
            run.clone(),
            store,
        ))
    }

    /// Releases a terminal-writer reservation, returning whether one was held.
    ///
    /// Called only by [`TerminalWriterReservation`]'s `Drop`, which is why it is
    /// not public: release is not a decision any caller makes. The compare
    /// against `run` is the same compare-and-delete discipline [`Self::remove`]
    /// uses on the index — a reservation for a different run of this workflow
    /// must survive a stale release.
    pub(super) fn release_terminal_writer(
        &self,
        id: &WorkflowId,
        run: &RunId,
    ) -> Result<bool, EngineError> {
        let mut slots = self.slots()?;
        if let std::collections::hash_map::Entry::Occupied(entry) =
            slots.terminal_writers.entry(id.clone())
        {
            if entry.get() == run {
                entry.remove();
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Whether a terminal-writer reservation is currently held for `id`, and for
    /// which run.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::RegistryPoisoned`] if the registry lock was poisoned.
    pub fn terminal_writer_run(&self, id: &WorkflowId) -> Result<Option<RunId>, EngineError> {
        Ok(self.slots()?.terminal_writers.get(id).cloned())
    }

    fn slots(&self) -> Result<MutexGuard<'_, Slots>, EngineError> {
        self.slots.lock().map_err(|_| EngineError::RegistryPoisoned)
    }

    fn index(&self) -> Result<MutexGuard<'_, LivePidIndex>, EngineError> {
        self.index.lock().map_err(|_| EngineError::RegistryPoisoned)
    }
}

/// The handle-side half of the terminal-writer exclusion.
///
/// Both `insert` paths route through this while holding the same `slots` lock a
/// reservation is taken under, so the two writers can never be created for one
/// workflow whichever order the calls arrive in.
fn refuse_if_terminal_writer_held(slots: &Slots, key: &RegistryKey) -> Result<(), EngineError> {
    if let Some(held_run) = slots.terminal_writers.get(&key.0) {
        return Err(EngineError::TerminalWriterHeld {
            workflow_id: key.0.to_string(),
            run_id: held_run.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use aion_core::{Event, EventEnvelope, Payload, PayloadError, WorkflowId, WorkflowStatus};
    use aion_package::ContentHash;
    use chrono::Utc;
    use serde_json::json;

    use crate::EngineError;
    use crate::registry::handle::{
        CompletionNotifier, HandleResidency, WorkflowHandle, WorkflowHandleParts,
    };

    use super::{HandleRemoval, Registry};

    type TestResult = Result<(), TestError>;

    #[derive(Debug)]
    enum TestError {
        Engine(EngineError),
        Payload(PayloadError),
    }

    impl std::fmt::Display for TestError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Engine(error) => write!(formatter, "{error}"),
                Self::Payload(error) => write!(formatter, "{error}"),
            }
        }
    }

    impl std::error::Error for TestError {}

    impl From<EngineError> for TestError {
        fn from(error: EngineError) -> Self {
            Self::Engine(error)
        }
    }

    impl From<PayloadError> for TestError {
        fn from(error: PayloadError) -> Self {
            Self::Payload(error)
        }
    }

    fn assert_send_sync<T: Send + Sync>() {}

    fn hash(byte: u8) -> ContentHash {
        ContentHash::from_bytes([byte; 32])
    }

    fn handle(pid: u64, version_byte: u8, status: WorkflowStatus) -> WorkflowHandle {
        let workflow_id = WorkflowId::new_v4();
        let run_id = aion_core::RunId::new_v4();
        let store = Arc::new(aion_store::InMemoryStore::default());
        let recorder = crate::durability::Recorder::new(workflow_id.clone(), store);
        WorkflowHandle::new(WorkflowHandleParts {
            workflow_id,
            run_id,
            pid,
            workflow_type: "checkout".to_owned(),
            namespace: String::from("default"),
            loaded_version: hash(version_byte),
            cached_status: status,
            residency: HandleResidency::Resident,
            recorder,
            completion: CompletionNotifier::new(),
        })
    }

    fn envelope(workflow_id: &aion_core::WorkflowId, seq: u64) -> EventEnvelope {
        EventEnvelope {
            seq,
            recorded_at: Utc::now(),
            workflow_id: workflow_id.clone(),
        }
    }

    fn payload(label: &str) -> Result<Payload, aion_core::PayloadError> {
        Payload::from_json(&json!({ "label": label }))
    }

    fn completed(workflow_id: &aion_core::WorkflowId) -> Result<Event, aion_core::PayloadError> {
        Ok(Event::WorkflowCompleted {
            envelope: envelope(workflow_id, 2),
            result: payload("result")?,
        })
    }

    fn cancelled(workflow_id: &aion_core::WorkflowId) -> Event {
        Event::WorkflowCancelled {
            envelope: envelope(workflow_id, 2),
            reason: String::from("caller requested cancellation"),
        }
    }

    #[test]
    fn registry_is_send_sync() {
        assert_send_sync::<Registry>();
    }

    #[test]
    fn stores_two_runs_for_the_same_workflow_without_shadowing() -> Result<(), EngineError> {
        let registry = Registry::default();
        let workflow_id = aion_core::WorkflowId::new_v4();
        let first_run = aion_core::RunId::new_v4();
        let second_run = aion_core::RunId::new_v4();
        let first = handle(1, 1, WorkflowStatus::Running);
        let second = handle(2, 2, WorkflowStatus::Completed);

        assert!(
            registry
                .insert((workflow_id.clone(), first_run.clone()), first.clone())?
                .is_none()
        );
        assert!(
            registry
                .insert((workflow_id.clone(), second_run.clone()), second.clone())?
                .is_none()
        );

        assert_eq!(registry.get(&workflow_id, &first_run)?, Some(first));
        assert_eq!(registry.get(&workflow_id, &second_run)?, Some(second));

        let stale_run = aion_core::RunId::new_v4();
        assert_eq!(registry.get(&workflow_id, &stale_run)?, None);
        Ok(())
    }

    #[test]
    fn remove_deletes_only_the_requested_run() -> Result<(), EngineError> {
        let registry = Registry::default();
        let workflow_id = aion_core::WorkflowId::new_v4();
        let first_run = aion_core::RunId::new_v4();
        let second_run = aion_core::RunId::new_v4();
        let first = handle(1, 1, WorkflowStatus::Running);
        let second = handle(2, 2, WorkflowStatus::Running);

        registry.insert((workflow_id.clone(), first_run.clone()), first.clone())?;
        registry.insert((workflow_id.clone(), second_run.clone()), second.clone())?;

        assert_eq!(registry.remove(&workflow_id, &first_run)?, Some(first));
        assert_eq!(registry.get(&workflow_id, &first_run)?, None);
        assert_eq!(registry.get(&workflow_id, &second_run)?, Some(second));
        Ok(())
    }

    #[test]
    fn remove_if_pid_removes_matching_handle_and_clears_index() -> Result<(), EngineError> {
        let registry = Registry::default();
        let workflow_id = WorkflowId::new_v4();
        let run_id = aion_core::RunId::new_v4();
        let registered = handle(51, 1, WorkflowStatus::Failed);
        registry.insert((workflow_id.clone(), run_id.clone()), registered.clone())?;

        let removed = registry.remove_if_pid(&workflow_id, &run_id, 51)?;
        assert!(
            matches!(removed, HandleRemoval::Removed(handle) if handle.pid() == 51),
            "a matching pid removes and returns the observed handle"
        );
        assert_eq!(registry.get(&workflow_id, &run_id)?, None);
        assert_eq!(registry.live_pid(&workflow_id)?, None);
        Ok(())
    }

    #[test]
    fn remove_if_pid_reports_absent_for_an_empty_slot() -> Result<(), EngineError> {
        let registry = Registry::default();
        let workflow_id = WorkflowId::new_v4();
        let run_id = aion_core::RunId::new_v4();

        assert!(matches!(
            registry.remove_if_pid(&workflow_id, &run_id, 51)?,
            HandleRemoval::Absent
        ));
        Ok(())
    }

    #[test]
    fn remove_if_pid_returns_replacement_without_disturbing_it_or_the_index()
    -> Result<(), EngineError> {
        let registry = Registry::default();
        let workflow_id = WorkflowId::new_v4();
        let run_id = aion_core::RunId::new_v4();
        let replacement = handle(52, 2, WorkflowStatus::Running);
        registry.insert((workflow_id.clone(), run_id.clone()), replacement.clone())?;

        let outcome = registry.remove_if_pid(&workflow_id, &run_id, 51)?;
        assert!(
            matches!(outcome, HandleRemoval::Replaced(handle) if handle.pid() == 52),
            "a different occupant is returned rather than evicted"
        );
        assert_eq!(
            registry.get(&workflow_id, &run_id)?,
            Some(replacement),
            "the replacement remains registered"
        );
        assert_eq!(
            registry.live_pid(&workflow_id)?,
            Some(52),
            "a replacement refusal preserves the live-pid index"
        );
        Ok(())
    }

    #[test]
    fn live_pid_tracks_newest_run_across_continue_as_new() -> Result<(), EngineError> {
        let registry = Registry::default();
        let workflow_id = aion_core::WorkflowId::new_v4();
        let first_run = aion_core::RunId::new_v4();
        let second_run = aion_core::RunId::new_v4();

        // An unknown workflow has no live pid.
        assert_eq!(registry.live_pid(&workflow_id)?, None);

        // Insert resolves the workflow to its run's pid.
        registry.insert(
            (workflow_id.clone(), first_run.clone()),
            handle(1, 1, WorkflowStatus::Running),
        )?;
        assert_eq!(registry.live_pid(&workflow_id)?, Some(1));

        // Continue-as-new inserts the new run, then removes the old one. The
        // index must track the newest run, and the stale removal must not
        // evict it (compare-and-delete on RunId).
        registry.insert(
            (workflow_id.clone(), second_run.clone()),
            handle(2, 2, WorkflowStatus::Running),
        )?;
        assert_eq!(registry.live_pid(&workflow_id)?, Some(2));
        registry.remove(&workflow_id, &first_run)?;
        assert_eq!(registry.live_pid(&workflow_id)?, Some(2));

        // Removing the live run clears the index entry.
        registry.remove(&workflow_id, &second_run)?;
        assert_eq!(registry.live_pid(&workflow_id)?, None);
        Ok(())
    }

    #[test]
    fn insert_if_absent_wins_the_slot_and_rejects_the_racer() -> Result<(), EngineError> {
        let registry = Registry::default();
        let workflow_id = aion_core::WorkflowId::new_v4();
        let run = aion_core::RunId::new_v4();
        let first = handle(1, 1, WorkflowStatus::Running);
        let second = handle(2, 2, WorkflowStatus::Running);

        // The empty slot accepts the first handle.
        let registered =
            registry.insert_if_absent((workflow_id.clone(), run.clone()), first.clone())?;
        assert_eq!(registered.pid(), 1);
        assert_eq!(registry.live_pid(&workflow_id)?, Some(1));

        // A second insert_if_absent observes the existing occupant untouched.
        let observed = registry.insert_if_absent((workflow_id.clone(), run.clone()), second)?;
        assert_eq!(
            observed.pid(),
            1,
            "the second insert must return the winner's handle, not overwrite it"
        );
        assert_eq!(registry.get(&workflow_id, &run)?, Some(first));
        Ok(())
    }

    #[test]
    fn list_returns_snapshot_handles() -> Result<(), EngineError> {
        let registry = Registry::default();
        let workflow_id = aion_core::WorkflowId::new_v4();
        let first_run = aion_core::RunId::new_v4();
        let second_run = aion_core::RunId::new_v4();

        registry.insert(
            (workflow_id.clone(), first_run),
            handle(1, 1, WorkflowStatus::Running),
        )?;
        registry.insert(
            (workflow_id, second_run),
            handle(2, 2, WorkflowStatus::Running),
        )?;

        let mut pids = registry
            .list()?
            .into_iter()
            .map(|handle| handle.pid())
            .collect::<Vec<_>>();
        pids.sort_unstable();

        assert_eq!(pids, vec![1, 2]);
        Ok(())
    }

    #[test]
    fn poisoned_lock_returns_typed_registry_error() {
        let registry = Arc::new(Registry::default());
        let poisoner_registry = Arc::clone(&registry);
        let poisoner = std::thread::spawn(move || {
            let guard = poisoner_registry.slots.lock();
            assert!(guard.is_ok());
            std::panic::resume_unwind(Box::new("poison registry lock"));
        });

        assert!(poisoner.join().is_err());
        assert!(matches!(
            registry.list(),
            Err(EngineError::RegistryPoisoned)
        ));
    }

    #[test]
    fn reconcile_updates_completed_projection() -> TestResult {
        let registry = Registry::default();
        let workflow_id = aion_core::WorkflowId::new_v4();
        let run_id = aion_core::RunId::new_v4();
        registry.insert(
            (workflow_id.clone(), run_id.clone()),
            handle(1, 1, WorkflowStatus::Running),
        )?;
        let events = vec![
            started_run(&workflow_id, &run_id, 1, None)?,
            completed(&workflow_id)?,
        ];

        let reconciled = registry.reconcile(&workflow_id, &run_id, &events)?;

        assert_eq!(
            reconciled.map(|handle| handle.cached_status()),
            Some(WorkflowStatus::Completed)
        );
        assert_eq!(
            registry
                .get(&workflow_id, &run_id)?
                .map(|handle| handle.cached_status()),
            Some(WorkflowStatus::Completed)
        );
        Ok(())
    }

    #[test]
    fn reconcile_updates_cancelled_projection() -> TestResult {
        let registry = Registry::default();
        let workflow_id = aion_core::WorkflowId::new_v4();
        let run_id = aion_core::RunId::new_v4();
        registry.insert(
            (workflow_id.clone(), run_id.clone()),
            handle(1, 1, WorkflowStatus::Running),
        )?;
        let events = vec![
            started_run(&workflow_id, &run_id, 1, None)?,
            cancelled(&workflow_id),
        ];

        let reconciled = registry.reconcile(&workflow_id, &run_id, &events)?;

        assert_eq!(
            reconciled.map(|handle| handle.cached_status()),
            Some(WorkflowStatus::Cancelled)
        );
        Ok(())
    }

    #[test]
    fn reconcile_projection_wins_over_disagreeing_cache() -> TestResult {
        let registry = Registry::default();
        let workflow_id = aion_core::WorkflowId::new_v4();
        let run_id = aion_core::RunId::new_v4();
        registry.insert(
            (workflow_id.clone(), run_id.clone()),
            handle(1, 1, WorkflowStatus::Failed),
        )?;
        let events = vec![started_run(&workflow_id, &run_id, 1, None)?];

        let reconciled = registry.reconcile(&workflow_id, &run_id, &events)?;

        assert_eq!(
            reconciled.map(|handle| handle.cached_status()),
            Some(WorkflowStatus::Running)
        );
        Ok(())
    }

    fn store() -> Arc<dyn aion_store::EventStore> {
        Arc::new(aion_store::InMemoryStore::default())
    }

    /// Requirement (1). The registry — not a caller's check — is what makes a
    /// handle and a terminal-writer reservation mutually exclusive, and it holds
    /// in BOTH directions and from EITHER order of arrival.
    #[test]
    fn a_reservation_and_a_handle_are_mutually_exclusive() -> Result<(), EngineError> {
        let registry = Registry::default();
        let workflow_id = aion_core::WorkflowId::new_v4();
        let run = aion_core::RunId::new_v4();

        // Reservation first: the handle is refused while it is held.
        let reservation = registry.reserve_terminal_writer(&workflow_id, &run, store())?;
        assert!(
            matches!(
                registry.insert(
                    (workflow_id.clone(), run.clone()),
                    handle(1, 1, WorkflowStatus::Running)
                ),
                Err(EngineError::TerminalWriterHeld { .. })
            ),
            "a handle must not be registrable for a workflow whose writer is reserved"
        );
        assert!(
            matches!(
                registry.insert_if_absent(
                    (workflow_id.clone(), run.clone()),
                    handle(1, 1, WorkflowStatus::Running)
                ),
                Err(EngineError::TerminalWriterHeld { .. })
            ),
            "insert_if_absent is the other door into the same map and must refuse too"
        );
        drop(reservation);

        // Handle first: the reservation is refused while it is held.
        registry.insert(
            (workflow_id.clone(), run.clone()),
            handle(1, 1, WorkflowStatus::Running),
        )?;
        assert!(
            matches!(
                registry.reserve_terminal_writer(&workflow_id, &run, store()),
                Err(EngineError::TerminalWriterUnavailable { .. })
            ),
            "a resident run has a writer already and must take the ordinary cancel path"
        );
        Ok(())
    }

    /// The exclusion is WORKFLOW-scoped, not `(workflow, run)`-scoped, because a
    /// `Recorder` writes the workflow's event stream. A handle on a different run
    /// of the same workflow is still a second writer.
    ///
    /// This is the case a `(workflow, run)`-keyed check would wave through, and
    /// it is reachable: the continue-as-new window registers two runs at once.
    #[test]
    fn a_handle_on_another_run_of_the_workflow_blocks_the_reservation() -> Result<(), EngineError> {
        let registry = Registry::default();
        let workflow_id = aion_core::WorkflowId::new_v4();
        let live_run = aion_core::RunId::new_v4();
        let dead_run = aion_core::RunId::new_v4();

        registry.insert(
            (workflow_id.clone(), live_run),
            handle(1, 1, WorkflowStatus::Running),
        )?;

        assert!(
            matches!(
                registry.reserve_terminal_writer(&workflow_id, &dead_run, store()),
                Err(EngineError::TerminalWriterUnavailable { .. })
            ),
            "another run's live handle is still a writer for this workflow's stream"
        );

        // The control: a DIFFERENT workflow is unaffected. Without this, a
        // `reserve` that refused everything would pass the assertion above.
        let other_workflow = aion_core::WorkflowId::new_v4();
        assert!(
            registry
                .reserve_terminal_writer(&other_workflow, &dead_run, store())
                .is_ok(),
            "the refusal must be scoped to the workflow that has a writer, not global"
        );
        Ok(())
    }

    /// Only one reservation at a time — the guard is the writer, so two guards
    /// would be two writers.
    #[test]
    fn a_second_reservation_for_the_same_workflow_is_refused() -> Result<(), EngineError> {
        let registry = Registry::default();
        let workflow_id = aion_core::WorkflowId::new_v4();
        let first = aion_core::RunId::new_v4();
        let second = aion_core::RunId::new_v4();

        let held = registry.reserve_terminal_writer(&workflow_id, &first, store())?;
        assert!(matches!(
            registry.reserve_terminal_writer(&workflow_id, &second, store()),
            Err(EngineError::TerminalWriterUnavailable { .. })
        ));
        drop(held);
        Ok(())
    }

    /// The anti-wedge property, and the reason the reservation is an RAII guard
    /// rather than a pair of calls: a reservation that could be taken and never
    /// released would lock the workflow out of every future writer for the life
    /// of the process — the defect this whole path exists to fix, one level up.
    #[test]
    fn dropping_a_reservation_releases_the_slot() -> Result<(), EngineError> {
        let registry = Registry::default();
        let workflow_id = aion_core::WorkflowId::new_v4();
        let run = aion_core::RunId::new_v4();

        {
            let _reservation = registry.reserve_terminal_writer(&workflow_id, &run, store())?;
            assert_eq!(
                registry.terminal_writer_run(&workflow_id)?,
                Some(run.clone()),
                "the slot must be held while the guard is alive, or the test below proves nothing"
            );
        }

        assert_eq!(
            registry.terminal_writer_run(&workflow_id)?,
            None,
            "the slot must be free once the guard goes out of scope"
        );
        registry.insert(
            (workflow_id.clone(), run.clone()),
            handle(1, 1, WorkflowStatus::Running),
        )?;
        assert!(
            registry.get(&workflow_id, &run)?.is_some(),
            "and a handle must be registrable again afterwards"
        );
        Ok(())
    }

    /// A reservation is not a process. Every read that exists to find a live
    /// process must miss it — a reservation that showed up in `live_pid` would
    /// make the startup recovery loop skip the run permanently, and would make
    /// delivery believe there is somewhere to route to.
    #[test]
    fn a_reservation_is_invisible_to_every_live_process_lookup() -> Result<(), EngineError> {
        let registry = Registry::default();
        let workflow_id = aion_core::WorkflowId::new_v4();
        let run = aion_core::RunId::new_v4();
        let _reservation = registry.reserve_terminal_writer(&workflow_id, &run, store())?;

        assert_eq!(registry.live_pid(&workflow_id)?, None);
        assert_eq!(registry.live_run_pid(&workflow_id)?, None);
        assert_eq!(registry.get(&workflow_id, &run)?, None);
        assert!(registry.list()?.is_empty());
        Ok(())
    }

    #[test]
    fn reconcile_missing_handle_is_noop() -> TestResult {
        let registry = Registry::default();
        let workflow_id = aion_core::WorkflowId::new_v4();
        let run_id = aion_core::RunId::new_v4();
        let events = vec![started_run(&workflow_id, &run_id, 1, None)?];

        assert_eq!(registry.reconcile(&workflow_id, &run_id, &events)?, None);
        Ok(())
    }

    // --- reconcile must scope its projection to the run it names (aion#94) ----
    //
    // 🔴 The helpers above cannot see this defect, and that is worth saying out
    // loud: `started()` stamps a hardcoded `run_id` unrelated to the registry key
    // it is reconciled against, so every existing `reconcile` test asserts on a
    // history that does not belong to the run under test — and they all pass,
    // because the projection ignores the `run` argument entirely. A test that
    // cannot tell the two apart cannot detect the confusion between them.
    //
    // These two build the one history shape where the distinction is load-bearing:
    // a continue-as-new, where ONE workflow id owns TWO runs, the predecessor is
    // terminal, and the successor is running.

    fn started_run(
        workflow_id: &aion_core::WorkflowId,
        run_id: &aion_core::RunId,
        seq: u64,
        parent_run_id: Option<aion_core::RunId>,
    ) -> Result<Event, aion_core::PayloadError> {
        Ok(Event::WorkflowStarted {
            envelope: envelope(workflow_id, seq),
            workflow_type: String::from("checkout"),
            input: payload("input")?,
            run_id: run_id.clone(),
            parent_run_id,
            // A continue-as-new chain, not a child spawn: one workflow id owns
            // both runs, so there is no parent WORKFLOW to wear (aion#77).
            parent_workflow_id: None,
            package_version: aion_core::PackageVersion::new("a".repeat(64)),
        })
    }

    fn continued_as_new(
        workflow_id: &aion_core::WorkflowId,
        parent_run_id: &aion_core::RunId,
        seq: u64,
    ) -> Result<Event, aion_core::PayloadError> {
        Ok(Event::WorkflowContinuedAsNew {
            envelope: envelope(workflow_id, seq),
            input: payload("carried")?,
            workflow_type: None,
            parent_run_id: parent_run_id.clone(),
        })
    }

    /// A predecessor run stays `ContinuedAsNew` when its successor is running.
    ///
    /// Today this FAILS with `Running`: `status_from_events` is
    /// last-lifecycle-event-wins over whatever slice it is handed, every
    /// production caller hands it the whole history for the workflow id, and
    /// after a continue-as-new the last lifecycle event belongs to the SUCCESSOR.
    /// So the predecessor's handle is told it is running.
    ///
    /// It is reachable from a read-only API: `Engine::list_workflows` iterates
    /// every registered handle and reconciles each against full history, and the
    /// nif-driven continue-as-new path never removes the predecessor's handle.
    #[test]
    fn reconcile_scopes_the_projection_to_the_named_run() -> TestResult {
        let registry = Registry::default();
        let workflow_id = aion_core::WorkflowId::new_v4();
        let predecessor = aion_core::RunId::new_v4();
        let successor = aion_core::RunId::new_v4();

        registry.insert(
            (workflow_id.clone(), predecessor.clone()),
            handle(1, 1, WorkflowStatus::Running),
        )?;
        registry.insert(
            (workflow_id.clone(), successor.clone()),
            handle(2, 1, WorkflowStatus::Running),
        )?;

        let events = vec![
            started_run(&workflow_id, &predecessor, 1, None)?,
            continued_as_new(&workflow_id, &predecessor, 2)?,
            started_run(&workflow_id, &successor, 3, Some(predecessor.clone()))?,
        ];

        // Fixture control: the whole-history projection really is `Running`, so a
        // failure below is the scoping defect and not a malformed history.
        assert_eq!(
            aion_core::status_from_events(&events),
            WorkflowStatus::Running,
            "fixture control: the successor's start is the last lifecycle event"
        );

        let reconciled = registry.reconcile(&workflow_id, &predecessor, &events)?;

        assert_eq!(
            reconciled.map(|handle| handle.cached_status()),
            Some(WorkflowStatus::ContinuedAsNew),
            "the predecessor run is terminal; only the successor is running"
        );
        Ok(())
    }

    /// The twin, and it is not decoration: without it the assertion above is
    /// satisfied by anything that reports `ContinuedAsNew` unconditionally.
    ///
    /// Measured 2026-08-20: this test is deliberately NOT red under the
    /// mutation that reddens its twin. Reverting `reconcile` to the
    /// whole-history projection leaves this one green, because on this history
    /// both readings answer `Running` for the successor. That is the OTHER
    /// direction, not a vacuous pass — its job is to prove the run-scoping does
    /// not over-correct by making the LIVE run look terminal, which a scoping
    /// bug that took the wrong segment would do. A fix has two ways to be
    /// wrong, and one red twin measures only one of them.
    #[test]
    fn reconcile_still_sees_the_successor_as_running() -> TestResult {
        let registry = Registry::default();
        let workflow_id = aion_core::WorkflowId::new_v4();
        let predecessor = aion_core::RunId::new_v4();
        let successor = aion_core::RunId::new_v4();

        registry.insert(
            (workflow_id.clone(), successor.clone()),
            handle(2, 1, WorkflowStatus::Completed),
        )?;

        let events = vec![
            started_run(&workflow_id, &predecessor, 1, None)?,
            continued_as_new(&workflow_id, &predecessor, 2)?,
            started_run(&workflow_id, &successor, 3, Some(predecessor.clone()))?,
        ];

        let reconciled = registry.reconcile(&workflow_id, &successor, &events)?;

        assert_eq!(
            reconciled.map(|handle| handle.cached_status()),
            Some(WorkflowStatus::Running),
            "the successor is the live run and the projection must still say so"
        );
        Ok(())
    }

    /// A registered run absent from the history it is projected against is an
    /// ERROR, and that is this branch's own stated point: `status_from_events`
    /// answers `Running` for an empty slice, so absorbing the case would leave
    /// the reconciliation whose whole purpose is to stop terminal runs being
    /// cached as live caching one as live itself — silently, and precisely for
    /// the run whose history had gone missing.
    ///
    /// 🔴 This test was missing when the fix was written. The variant, its raise
    /// site, and both `aion-server` mappings shipped together with nothing
    /// exercising any of them, so reverting the `Err` to the old default passed
    /// the entire tree. A guard is not a guard until something has watched it
    /// refuse.
    #[test]
    fn reconcile_refuses_a_run_absent_from_the_history() -> TestResult {
        let registry = Registry::default();
        let workflow_id = aion_core::WorkflowId::new_v4();
        let registered = aion_core::RunId::new_v4();
        let stranger = aion_core::RunId::new_v4();

        // The handle must really be registered: the missing-handle path returns
        // `Ok(None)` BEFORE the segment is ever taken, so without this insert
        // the call would leave through a different branch and assert nothing.
        registry.insert(
            (workflow_id.clone(), registered.clone()),
            handle(1, 1, WorkflowStatus::Running),
        )?;

        // A history for this workflow id that belongs entirely to another run.
        let events = vec![
            started_run(&workflow_id, &stranger, 1, None)?,
            continued_as_new(&workflow_id, &stranger, 2)?,
        ];

        // Fixture control, and it is the whole reason the error exists: the
        // empty segment this produces would itself project `Running`.
        assert_eq!(
            aion_core::status_from_events(&[]),
            WorkflowStatus::Running,
            "fixture control: an empty slice defaults to Running"
        );

        let refusal = registry.reconcile(&workflow_id, &registered, &events).err();

        assert!(
            matches!(
                &refusal,
                Some(EngineError::RunNotInHistory {
                    workflow_id: named_workflow,
                    run_id: named_run,
                }) if named_workflow == &workflow_id && named_run == &registered
            ),
            "the refusal must name the run that is missing, not the one the \
             history happens to hold; got: {refusal:?}"
        );

        // The handle must be untouched. A refusal that had already written
        // would leave the cache holding the very status it declined to vouch
        // for, which is worse than either answering or erroring cleanly.
        assert_eq!(
            registry
                .get(&workflow_id, &registered)?
                .map(|handle| handle.cached_status()),
            Some(WorkflowStatus::Running),
            "a refused reconciliation must not have mutated the handle"
        );
        Ok(())
    }
}
