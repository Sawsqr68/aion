//! Cancelling a run that was never alive (#117(c)).
//!
//! # The hole this closes
//!
//! A run whose pinned package version no longer loads is skipped by startup
//! recovery. It never becomes resident, so it never obtains a `Recorder`, so
//! [`super::terminate::cancel`] — which opens by taking the run's recorder —
//! refuses it. The run stays `Running` forever and the operator's only lever is
//! the one thing denied. Measured on run `0756ecd5`: `describe` returns a full
//! history, `resume` reads its status, and `cancel` answers "not found".
//!
//! # Why this is a separate path and not a relaxation of the ordinary one
//!
//! The ordinary path is correct and stays untouched. What it enforces —
//! exactly one writer per workflow (invariant #3) — is not negotiable, and the
//! reason a never-alive run cannot use it is that it has no writer to take, not
//! that the rule is too strict. So this path does not weaken the rule; it
//! satisfies it by a different primitive: a
//! [`TerminalWriterReservation`](crate::registry::TerminalWriterReservation),
//! which the registry grants only when it can prove, under its own lock, that
//! the workflow has no other writer of any kind.
//!
//! # The door is narrow, and every hinge is measured
//!
//! Four conditions, in this order, all under the reservation once it is held:
//!
//! 1. **This engine recorded a reason the run could not be made resident.** The
//!    cancellation cites that verdict. No verdict, no cancellation — a run that
//!    is merely not resident right now is not this case.
//! 2. **A writer slot is available.** Proven by the registry, atomically, not by
//!    a check here.
//! 3. **The run's pinned package does not resolve right now.** Re-measured
//!    against the live catalog through the SAME resolution startup recovery
//!    uses, never cited from the boot-time verdict. **If it resolves, this stops
//!    and says so**: a redeploy has landed, the run is recoverable, and
//!    cancelling it here would take the extraordinary route past a working
//!    ordinary one.
//! 4. **The run has recorded no terminal event**, and the history's run id is
//!    the run the caller named.
//!
//! On condition 3 there is a window: the catalog is read while the reservation
//! is held, but a deploy can land between that read and the append. That window
//! cannot produce a second writer — the reservation excludes handles for as long
//! as it lives — so its worst case is that a run which became recoverable a
//! microsecond ago is cancelled anyway, which is what the operator asked for.
//! The dangerous failure is closed structurally; this one is not dangerous.
//!
//! # Stated limitation
//!
//! The probe is the package RESOLUTION step, not a spawn. A run that is
//! unrecoverable because spawning fails (rather than because its version is
//! absent) will resolve, and this path will refuse it and say the run is
//! recoverable. That is the conservative direction — refusing the extraordinary
//! route when the ordinary one might work — and it is deliberate: making the
//! probe a real spawn would make a cancellation path leave a resident process
//! behind on success. The measured case for `0756ecd5` is resolution failure.

use aion_core::{Event, RunId, WorkflowId};
use chrono::Utc;

use crate::EngineError;

use super::terminate::TerminateWorkflowContext;
use super::visibility::upsert_workflow_visibility;

/// Cancels a run that holds no handle and can never obtain one.
///
/// Called only by [`super::terminate::cancel`], after it has established that
/// the `(workflow, run)` pair has no registered handle.
///
/// # Errors
///
/// - [`EngineError::NoResidencyVerdict`] when this engine recorded no reason the
///   run could not be made resident. The verb names the true state: the run
///   exists and is readable — what is missing is the verdict this path must
///   cite.
/// - [`EngineError::RunIsRecoverable`] when the pinned package resolves now.
/// - [`EngineError::TerminalWriterUnavailable`] when the workflow already has a
///   writer.
/// - The typed recorder or store error from the terminal transition.
pub(super) async fn cancel_never_alive(
    context: TerminateWorkflowContext<'_>,
    id: &WorkflowId,
    run: &RunId,
    reason: String,
) -> Result<(), EngineError> {
    // (0) Does the workflow exist at all? A caller naming an id nothing was ever
    // started under gets the honest answer, unchanged: not found. Requirement (4)
    // — the verb names the true state — is a demand for accuracy in BOTH
    // directions, and calling a nonexistent workflow "unrecoverable" would be the
    // same dishonesty pointed the other way.
    let triage = context.store.read_history(id).await?;
    if triage.is_empty() {
        return Err(EngineError::WorkflowNotFound {
            workflow_type: format!("{id}/{run}"),
        });
    }

    // (0b) Already terminal? Then THAT is the most informative true statement
    // about this run, and it outranks everything below — a run that finished a
    // microsecond ago is not "missing a residency verdict", it is finished.
    //
    // This is the ordinary path's own refusal, reached here for the runs the
    // ordinary path can no longer see. It is load-bearing for a real race: a
    // resident run whose deadline fires while a cancel is in flight deregisters
    // between the cancel's registry lookup and this branch, and the cancel
    // arrives to find a terminated run with no handle. The refusal it gets must
    // name the terminal, not the absence of a handle.
    //
    // Re-checked again below under the reservation. This one is triage, that one
    // is authoritative; both are cheap and only the second can be trusted.
    super::terminate::reject_if_recorded_terminal(&triage, id, run)?;

    // (1) The verdict this cancellation cites. Its `workflow_type` is the one
    // read from the run's own `WorkflowStarted` at boot, so the probe below asks
    // about the same type recovery asked about.
    let Some(verdict) = context.registry.unrecoverable().get(id)? else {
        return Err(EngineError::NoResidencyVerdict {
            workflow_id: id.to_string(),
            run_id: run.to_string(),
        });
    };

    // (2) The writer slot. Taken BEFORE anything is measured, so every
    // measurement below describes a state no other writer can be changing.
    let reservation = context
        .registry
        .reserve_terminal_writer(id, run, context.store.clone())?;

    // Re-read under the reservation. The triage read above happened before the
    // writer slot was held, so a handle could still have been departing then;
    // this one cannot be stale, and it is the read the append derives its head
    // from.
    let history = context.store.read_history(id).await?;

    // (3) Re-measured, not cited.
    let (version, loaded) = crate::durability::pinned_package_if_loaded(
        id,
        &verdict.workflow_type,
        &history,
        context.catalog,
    )?;
    if loaded.is_some() {
        return Err(EngineError::RunIsRecoverable {
            workflow_id: id.to_string(),
            run_id: run.to_string(),
            version: version.to_string(),
        });
    }

    // (4) The run is the one named, and has recorded no terminal.
    reject_if_wrong_run(&history, id, run)?;
    super::terminate::reject_if_recorded_terminal(&history, id, run)?;

    tracing::warn!(
        workflow_id = %id,
        run_id = %run,
        workflow_type = %verdict.workflow_type,
        pinned_version = %version,
        recovery_failure = %verdict.reason,
        observed_at = %verdict.observed_at,
        "cancelling a run that was never made resident: its pinned package does not load, so it \
         holds no writer and never will under this build; recording the cancellation through a \
         terminal-writer reservation"
    );

    // The reservation is consumed here: one terminal transition, then it ends.
    reservation
        .record_cancelled(&history, Utc::now(), reason)
        .await?;

    upsert_workflow_visibility(context.store, context.visibility_store, id, run).await?;
    // Nothing to deregister and nobody to notify: this run never had a handle,
    // so it never had a completion notifier and no waiter is parked on one.
    Ok(())
}

/// Refuses a cancellation that names a run the history does not carry.
///
/// A never-alive run has exactly one `WorkflowStarted`, so this is a direct
/// comparison rather than a projection. Without it, an operator naming a stale
/// run id would append a `WorkflowCancelled` that claims to terminate a run the
/// history never contained.
fn reject_if_wrong_run(history: &[Event], id: &WorkflowId, run: &RunId) -> Result<(), EngineError> {
    let started = history.iter().rev().find_map(|event| match event {
        Event::WorkflowStarted { run_id, .. } => Some(run_id.clone()),
        _ => None,
    });
    match started {
        Some(started_run) if &started_run == run => Ok(()),
        Some(started_run) => Err(EngineError::InvalidState {
            reason: format!(
                "workflow `{id}` has no run `{run}`: its history's latest run is `{started_run}`"
            ),
        }),
        None => Err(EngineError::InvalidState {
            reason: format!(
                "workflow `{id}` has no WorkflowStarted event in durable history, so run `{run}` cannot be cancelled"
            ),
        }),
    }
}

#[cfg(test)]
#[path = "cancel_never_alive_tests.rs"]
mod tests;
