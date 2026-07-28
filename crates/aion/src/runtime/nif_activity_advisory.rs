//! The ADVISORY action class at the dispatch seam (RUNTIME-OPERATIONS.md R5).
//!
//! `advisory` is a DECLARATION-only marker: an action is a side channel — a
//! heartbeat, a notification — everywhere or nowhere. The AWL compiler commits
//! it into the package's `.v4` worker contract, so the contract is the engine's
//! authority on the class; nothing about it rides the dispatch wire the SDK
//! builds, and nothing about it can be forged by one call site.
//!
//! Two halves meet here. The COMPILER routes an advisory call around the fault
//! arm, so an exhausted side channel cannot fault the step that called it. This
//! module supplies the other half — the visibility the ruling demands in the
//! same breath: at the seam, an advisory dispatch is flagged, and when its
//! attempt budget is spent the retry loop records an
//! [`aion_core::Event::ActivityAdvisoryExhausted`] warning on the run ALONGSIDE
//! the activity's honest terminal `ActivityFailed`. The failure is never
//! erased — only declawed.

use crate::loader::WorkflowCatalog;
use crate::registry::WorkflowHandle;

/// Whether `activity_name` is declared `advisory` by the contract of the exact
/// package version this workflow run is pinned to.
///
/// Resolution is by `(workflow_type, loaded_version)` — the run's recorded
/// pin — so a hot redeploy that changes an action's class never retroactively
/// reclasses an in-flight run's activities.
///
/// Returns `false` for every absence: no catalog installed, no such loaded
/// workflow, a package that predates contract-bound identity, or an activity
/// the contract does not declare. `false` is the pre-R5 posture (the failure
/// faults the step), so an unresolvable class degrades to the loud behaviour,
/// never the quiet one.
pub(super) fn declared_advisory(
    catalog: Option<&WorkflowCatalog>,
    handle: &WorkflowHandle,
    activity_name: &str,
) -> bool {
    let Some(catalog) = catalog else {
        return false;
    };
    let Ok(Some(loaded)) = catalog.get(handle.workflow_type(), handle.loaded_version()) else {
        return false;
    };
    let Ok(contract) = loaded.contract() else {
        return false;
    };
    contract
        .workers
        .iter()
        .flat_map(|worker| worker.actions.iter())
        .any(|action| action.name == activity_name && action.advisory)
}
