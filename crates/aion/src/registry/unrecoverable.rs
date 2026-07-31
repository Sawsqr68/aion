//! Runs that startup recovery could not make resident, retained so the fact can
//! be ASKED FOR rather than only witnessed.
//!
//! # Why this exists (#117)
//!
//! A run whose pinned package version cannot be loaded is skipped by
//! `engine::startup::repopulate_active_workflows` and never becomes resident.
//! Until this module existed, the typed error was formatted into a single boot
//! ERROR line and dropped on `continue`. The fact then existed exactly once, in
//! a stream nobody could interrogate — and the operator who needs it is
//! precisely the one who was not watching stdout at boot.
//!
//! That gap has real consequences, measured on run `0756ecd5`: the read plane
//! and the control plane disagree about whether the run exists at all.
//!
//! ```text
//! describe -> rc=0, full history, Running     (read plane FOUND it)
//! resume   -> rc=1 "is Running, not Paused"   (also found it, read its status)
//! cancel   -> WorkflowNotFound                (control plane: does not exist)
//! ```
//!
//! Both planes are behaving correctly. `cancel` opens by taking the run's
//! `Recorder`, and a non-resident run has none — that is load-bearing invariant
//! #3 (single writer per workflow) working as designed. What was missing is any
//! way to learn WHY, after the boot log has scrolled away.
//!
//! # What this is NOT
//!
//! **It is not a status.** `WorkflowStatus` is a projection of event history
//! (invariant #4) and must stay one. Being unrecoverable is a property of THIS
//! ENGINE PROCESS's attempt to make the run resident — engine-internal
//! residency, orthogonal to status, exactly as `Resident` / `Suspended` already
//! are. A run recorded here still projects whatever its history says.
//!
//! **It is not durable.** The entries are per-process and rebuilt on every boot,
//! because the condition itself is per-process: a run unrecoverable under this
//! build may recover cleanly under the next one, which is the entire point of
//! the `redeploy` remedy. Persisting it would create a fact that outlives its
//! own truth.
//!
//! **It adds no writer.** This module only retains and reports something the
//! engine already computed. The cancellation append for a non-resident run is a
//! separate question that needs a ruling, and is deliberately not built here.
//!
//! # Staleness is the failure mode that matters
//!
//! A degraded flag that outlives the degradation is worse than no flag: it sends
//! an operator to a redeploy for a run that is running fine. So the entry is
//! cleared the moment the engine observes that run resident — see
//! [`UnrecoverableRuns::clear`] and its callers in `engine::startup`.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use chrono::{DateTime, Utc};

use aion_core::WorkflowId;

use crate::EngineError;

/// Why one run could not be made resident, and when this engine observed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnrecoverableRun {
    /// The run's workflow type, as read from its own `WorkflowStarted` event.
    pub workflow_type: String,
    /// The typed recovery error, rendered. Carries the operator-facing cause —
    /// for the identity-domain case, the pinned version that could not load.
    pub reason: String,
    /// When this engine process observed the failure. Wall-clock is correct
    /// here: this is an engine-operational observation, not workflow-visible
    /// state, so the determinism boundary (invariant #2) does not apply.
    pub observed_at: DateTime<Utc>,
}

/// Per-process set of runs that startup recovery could not make resident.
///
/// # Lock ordering
///
/// This holds its own mutex and is never locked while `Registry`'s `handles` or
/// `index` mutexes are held — every method here takes its lock, does one map
/// operation, and drops it. That keeps it outside the existing
/// `handles`-before-`index` ordering rather than extending it, so no new
/// lock-order inversion is possible.
#[derive(Debug, Default)]
pub struct UnrecoverableRuns {
    entries: Mutex<HashMap<WorkflowId, UnrecoverableRun>>,
}

impl UnrecoverableRuns {
    /// Records that `workflow_id` could not be made resident.
    ///
    /// Replaces any previous entry: the newest boot's verdict is the true one,
    /// because the condition is a property of this process's attempt.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::RegistryPoisoned`] if the lock was poisoned.
    pub fn record(
        &self,
        workflow_id: WorkflowId,
        entry: UnrecoverableRun,
    ) -> Result<(), EngineError> {
        self.entries()?.insert(workflow_id, entry);
        Ok(())
    }

    /// Answers whether this run is known-unrecoverable in this process, and why.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::RegistryPoisoned`] if the lock was poisoned.
    pub fn get(&self, workflow_id: &WorkflowId) -> Result<Option<UnrecoverableRun>, EngineError> {
        Ok(self.entries()?.get(workflow_id).cloned())
    }

    /// Drops any entry for `workflow_id`, returning whether one was present.
    ///
    /// Called wherever the engine observes the run resident. This is what keeps
    /// the set from reporting a healthy run as degraded — see the module docs:
    /// a stale degraded flag is worse than no flag at all.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::RegistryPoisoned`] if the lock was poisoned.
    pub fn clear(&self, workflow_id: &WorkflowId) -> Result<bool, EngineError> {
        Ok(self.entries()?.remove(workflow_id).is_some())
    }

    /// Every currently-known unrecoverable run, for an operator-facing sweep.
    ///
    /// Order is unspecified: the backing map is unordered and callers that need
    /// a stable presentation must sort by a field they choose. Returning it
    /// unsorted rather than picking an arbitrary order keeps the arbitrariness
    /// visible at the call site instead of hiding it here.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::RegistryPoisoned`] if the lock was poisoned.
    pub fn list(&self) -> Result<Vec<(WorkflowId, UnrecoverableRun)>, EngineError> {
        Ok(self
            .entries()?
            .iter()
            .map(|(id, entry)| (id.clone(), entry.clone()))
            .collect())
    }

    fn entries(
        &self,
    ) -> Result<MutexGuard<'_, HashMap<WorkflowId, UnrecoverableRun>>, EngineError> {
        self.entries
            .lock()
            .map_err(|_| EngineError::RegistryPoisoned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use uuid::Uuid;

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    /// Deterministic ids: these tests are about the SET's behaviour, so the ids
    /// only need to be distinct and reproducible, never random.
    fn wid(n: u128) -> WorkflowId {
        WorkflowId::new(Uuid::from_u128(n))
    }

    /// `expect_used` is denied workspace-wide, tests included, so a test that
    /// needs a value out of an `Option` says why it must be there instead of
    /// panicking with a message no assertion checked.
    fn require(found: Option<UnrecoverableRun>, why: &str) -> TestResult<UnrecoverableRun> {
        found.ok_or_else(|| Box::<dyn std::error::Error>::from(why.to_owned()))
    }

    fn entry(reason: &str) -> UnrecoverableRun {
        UnrecoverableRun {
            workflow_type: "rig$deadbeef".to_owned(),
            reason: reason.to_owned(),
            observed_at: Utc::now(),
        }
    }

    #[test]
    fn a_recorded_run_can_be_asked_about_by_id() -> TestResult {
        let runs = UnrecoverableRuns::default();
        let id = wid(0x0756_ecd5);
        runs.record(id.clone(), entry("pinned version 13958627 is not loaded"))?;

        let found = require(runs.get(&id)?, "the run just recorded must be found")?;
        assert_eq!(found.reason, "pinned version 13958627 is not loaded");
        assert_eq!(found.workflow_type, "rig$deadbeef");
        Ok(())
    }

    #[test]
    fn an_unrecorded_run_is_not_degraded() -> TestResult {
        // The inverted control. Without this, `get` returning `Some` for
        // everything would still pass the test above.
        let runs = UnrecoverableRuns::default();
        runs.record(wid(1), entry("cannot load"))?;

        assert!(
            runs.get(&wid(2))?.is_none(),
            "a run that was never recorded must not report as degraded — this is \
             the discriminating half of the query"
        );
        Ok(())
    }

    #[test]
    fn observing_the_run_resident_clears_the_degraded_fact() -> TestResult {
        // The staleness guard. A flag that outlives the degradation sends an
        // operator to a redeploy for a run that is running fine.
        let runs = UnrecoverableRuns::default();
        let id = wid(0x0756_ecd5);
        runs.record(id.clone(), entry("cannot load"))?;

        assert!(
            runs.clear(&id)?,
            "clear must report that it removed an entry"
        );
        assert!(
            runs.get(&id)?.is_none(),
            "a run observed resident must stop reporting as unrecoverable"
        );
        assert!(
            !runs.clear(&id)?,
            "clearing an absent entry must report false, not silently succeed — \
             the boolean is how a caller can tell a real recovery from a no-op"
        );
        Ok(())
    }

    #[test]
    fn the_newest_boots_verdict_replaces_the_previous_one() -> TestResult {
        let runs = UnrecoverableRuns::default();
        let id = wid(0x0756_ecd5);
        runs.record(id.clone(), entry("first boot: cannot load"))?;
        runs.record(id.clone(), entry("second boot: different cause"))?;

        let found = require(runs.get(&id)?, "an entry recorded twice must be present")?;
        assert_eq!(
            found.reason, "second boot: different cause",
            "the condition is a property of THIS process's attempt, so the latest \
             verdict wins rather than the first being sticky"
        );
        assert_eq!(runs.list()?.len(), 1, "a replace must not also append");
        Ok(())
    }

    #[test]
    fn list_reports_every_degraded_run_and_only_those() -> TestResult {
        let runs = UnrecoverableRuns::default();
        runs.record(wid(1), entry("cannot load a"))?;
        runs.record(wid(2), entry("cannot load b"))?;
        runs.clear(&wid(2))?;
        runs.record(wid(3), entry("cannot load c"))?;

        let mut listed: Vec<WorkflowId> = runs.list()?.into_iter().map(|(id, _)| id).collect();
        listed.sort_by_key(WorkflowId::as_uuid);
        assert_eq!(
            listed,
            vec![wid(1), wid(3)],
            "list must reflect clears, not just records"
        );
        Ok(())
    }
}
