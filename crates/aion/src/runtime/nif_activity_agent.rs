//! The AGENT action class at the dispatch seam.
//!
//! `agent` is a DECLARATION-only marker, exactly as `advisory` is: the AWL
//! compiler commits it into the package's `.v4` worker contract, so the
//! contract is the engine's authority on the class. Nothing about it rides the
//! dispatch wire the SDK builds, and no single call site can forge it.
//!
//! # Why the engine reads it at all
//!
//! Recovery has to decide what a DANGLING attempt means — an attempt a dead
//! engine generation left started with nothing settling it. The two rulings in
//! play disagree about that, and they disagree for good reasons:
//!
//! * **#266 Defect B**: an ordinary activity the dead server left in flight
//!   gets an honest NON-terminal `superseded:server-death` record, so an
//!   orphaned run stops reading as silently still working, and the re-dispatch
//!   continues the trail at the NEXT attempt.
//! * **#36**: a remote-agent attempt's holder can outlive the engine by
//!   construction — the agent is its own OS process, reparented, still working
//!   — so recording it as superseded by server death is a LIE about live work,
//!   and the identity must be RETAINED so the worker's spawn gate and
//!   single-flight can recognise the execution they already hold.
//!
//! The declared class is the only thing at this seam that can tell the two
//! apart. The engine cannot observe a harness process, and it must not guess:
//! guessing "an agent might have survived" for an ordinary parked activity
//! invents a holder that the drain deliberately stood down, and guessing
//! "nothing survived" for an agent is the #36 falsehood.
//!
//! Reading the flag here is sound because the contract documents it as
//! reliable — the checker enforces the agent seam's shape, so a reader may rely
//! on it — and because it is identity-bound: an action that becomes an agent
//! seam is a different package.

use crate::loader::WorkflowCatalog;
use crate::registry::WorkflowHandle;

/// Whether `activity_name` is declared `agent` by the contract of the exact
/// package version this workflow run is pinned to.
///
/// Resolution is by `(workflow_type, loaded_version)` — the run's recorded pin
/// — so a hot redeploy that changes an action's class never retroactively
/// reclasses an in-flight run's activities.
///
/// Returns `false` for every absence: no catalog installed, no such loaded
/// workflow, a package that predates contract-bound identity, an activity the
/// contract does not declare, or a poisoned catalog lock. `false` is the
/// pre-#36 posture — the #266 supersession record — so an unresolvable class
/// degrades to the record that says something happened, never to the silence
/// that says nothing did.
pub(super) fn declared_agent(
    catalog: Option<&WorkflowCatalog>,
    handle: &WorkflowHandle,
    activity_name: &str,
) -> bool {
    let Some(catalog) = catalog else {
        return false;
    };
    catalog
        .declares_agent_action(
            handle.workflow_type(),
            handle.loaded_version(),
            activity_name,
        )
        .unwrap_or(false)
}
