//! Deferred startup recovery (#266): the seam that lets an embedding host
//! install the engine-backed collaborators its activity dispatcher consults
//! BEFORE recovery replay re-dispatches in-flight work.
//!
//! By default [`crate::EngineBuilder::build`] performs the four startup
//! recovery steps itself, exactly as it always has. A host that decorates the
//! activity dispatcher with seams it can only fill once the engine exists
//! (the server's declared-body source is the motivating case) calls
//! [`crate::EngineBuilder::defer_startup_recovery`]; `build()` then skips the
//! four steps and stows what they need in a one-shot slot, and the host runs
//! [`Engine::run_startup_recovery`] after its seams are installed. The four
//! steps run in the same order either way.

use std::sync::{Arc, Mutex};

use chrono::Utc;

use crate::EngineError;
use crate::durability::ActiveWorkflowRecoverySeam;

use super::api::Engine;
use super::startup::{
    StartupRecoveryContext, recover_active_workflows_on_startup, recover_timers_on_startup,
};

/// What `build()` stows when startup recovery is deferred: the two recovery
/// inputs the constructed [`Engine`] does not itself hold.
pub(crate) struct DeferredStartupRecovery {
    /// The active-workflow recovery seam override configured on the builder.
    pub(crate) recovery: Option<Arc<dyn ActiveWorkflowRecoverySeam>>,
    /// Whether this node seeds the schedule coordinator's history.
    pub(crate) bootstrap_schedule_coordinator: bool,
}

/// The one-shot slot the staged startup-recovery entry points consume.
pub(super) enum DeferredRecoverySlot {
    /// `build()` ran recovery itself (the default); nothing is owed.
    NotDeferred,
    /// Recovery was deferred and has not run yet.
    Pending(DeferredStartupRecovery),
    /// [`Engine::recover_workflows_on_startup`] ran (or is running); the
    /// catch-up legs — timer recovery, schedule-coordinator catch-up,
    /// schedule recovery — are still owed to
    /// [`Engine::run_startup_catchup`].
    WorkflowsRecovered,
    /// Every recovery leg already ran (or is running).
    Completed,
}

impl DeferredRecoverySlot {
    /// The slot state `build()` hands [`Engine::new`].
    pub(super) fn from_build(deferred: Option<DeferredStartupRecovery>) -> Mutex<Self> {
        Mutex::new(deferred.map_or(Self::NotDeferred, Self::Pending))
    }
}

impl Engine {
    /// Run the startup recovery steps a deferred build skipped, in the same
    /// order `build()` runs them: active-workflow recovery replay, timer
    /// recovery, schedule-coordinator catch-up, schedule recovery.
    ///
    /// Call this exactly once, after every seam the activity dispatcher
    /// consults is installed — recovery replay re-dispatches in-flight
    /// activities through the dispatcher immediately.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::StartupRecoveryNotDeferred`] when the engine was
    /// built without [`crate::EngineBuilder::defer_startup_recovery`] (its
    /// build already ran recovery), [`EngineError::StartupRecoveryAlreadyRan`]
    /// on a second call, [`EngineError::StartupRecoverySlotPoisoned`] when the
    /// slot lock was poisoned, and any recovery-step error the deferred steps
    /// themselves surface.
    pub async fn run_startup_recovery(&self) -> Result<(), EngineError> {
        self.recover_workflows_on_startup().await?;
        self.run_startup_catchup().await
    }

    /// Run ONLY the active-workflow recovery leg of a deferred build: every
    /// durably-active workflow is replayed to residency and registered, so
    /// signals, queries, and cancels answer correctly the moment transports
    /// serve. The catch-up legs — owed timer fires, schedule-coordinator
    /// catch-up, schedule recovery — remain owed to
    /// [`Engine::run_startup_catchup`], which a host may run behind already-
    /// open doors: an owed-fire backlog has no upper bound, and every fire it
    /// delivers goes through the same idempotent record-once path the live
    /// timer wheel uses, so serving during catch-up is the steady-state
    /// contract, not a special mode.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::StartupRecoveryNotDeferred`] when the engine was
    /// built without [`crate::EngineBuilder::defer_startup_recovery`],
    /// [`EngineError::StartupRecoveryAlreadyRan`] on a second call,
    /// [`EngineError::StartupRecoverySlotPoisoned`] when the slot lock was
    /// poisoned, and any error the workflow-recovery leg itself surfaces.
    pub async fn recover_workflows_on_startup(&self) -> Result<(), EngineError> {
        let deferred = {
            let mut slot = self
                .deferred_startup_recovery
                .lock()
                .map_err(|_| EngineError::StartupRecoverySlotPoisoned)?;
            match std::mem::replace(&mut *slot, DeferredRecoverySlot::WorkflowsRecovered) {
                DeferredRecoverySlot::Pending(deferred) => deferred,
                DeferredRecoverySlot::NotDeferred => {
                    *slot = DeferredRecoverySlot::NotDeferred;
                    return Err(EngineError::StartupRecoveryNotDeferred);
                }
                DeferredRecoverySlot::WorkflowsRecovered => {
                    *slot = DeferredRecoverySlot::WorkflowsRecovered;
                    return Err(EngineError::StartupRecoveryAlreadyRan);
                }
                DeferredRecoverySlot::Completed => {
                    *slot = DeferredRecoverySlot::Completed;
                    return Err(EngineError::StartupRecoveryAlreadyRan);
                }
            }
        };
        recover_active_workflows_on_startup(StartupRecoveryContext {
            store: Arc::clone(&self.store),
            visibility_store: Arc::clone(&self.visibility_store),
            runtime: Arc::clone(&self.runtime),
            catalog: Arc::clone(&self.catalog),
            registry: Arc::clone(&self.registry),
            supervision: Arc::clone(&self.supervision),
            recovery: deferred.recovery,
            search_attribute_schema: Arc::clone(&self.search_attribute_schema),
            bootstrap_schedule_coordinator: deferred.bootstrap_schedule_coordinator,
        })
        .await
    }

    /// Run the catch-up legs a [`Engine::recover_workflows_on_startup`] call
    /// left owed, in the order `build()` runs them: timer recovery (owed
    /// fires and future re-arms), schedule-coordinator catch-up, schedule
    /// recovery. Safe to run while the host is serving — every leg is the
    /// same idempotent machinery the live paths run against a serving engine.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::StartupCatchupBeforeWorkflowRecovery`] when the
    /// workflow-recovery leg has not run,
    /// [`EngineError::StartupRecoveryNotDeferred`] when the build was not
    /// deferred, [`EngineError::StartupRecoveryAlreadyRan`] on a second call,
    /// [`EngineError::StartupRecoverySlotPoisoned`] when the slot lock was
    /// poisoned, and any error the catch-up legs themselves surface.
    pub async fn run_startup_catchup(&self) -> Result<(), EngineError> {
        {
            let mut slot = self
                .deferred_startup_recovery
                .lock()
                .map_err(|_| EngineError::StartupRecoverySlotPoisoned)?;
            match std::mem::replace(&mut *slot, DeferredRecoverySlot::Completed) {
                DeferredRecoverySlot::WorkflowsRecovered => {}
                DeferredRecoverySlot::Pending(deferred) => {
                    *slot = DeferredRecoverySlot::Pending(deferred);
                    return Err(EngineError::StartupCatchupBeforeWorkflowRecovery);
                }
                DeferredRecoverySlot::NotDeferred => {
                    *slot = DeferredRecoverySlot::NotDeferred;
                    return Err(EngineError::StartupRecoveryNotDeferred);
                }
                DeferredRecoverySlot::Completed => {
                    return Err(EngineError::StartupRecoveryAlreadyRan);
                }
            }
        }
        recover_timers_on_startup(self.runtime.nif_state(), Arc::clone(&self.store)).await?;
        self.catchup_schedule_coordinator().await?;
        self.recover_schedules_on_startup(Utc::now()).await?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "startup_deferred_tests.rs"]
mod tests;
