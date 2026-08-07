//! Durable continue-as-new NIF implementation.

use aion_core::{Payload, RunId};
use beamr::native::ProcessContext;
use beamr::term::Term;
use chrono::Utc;

use crate::durability::{DurabilityError, Recorder};
use crate::runtime::engine_tasks::EngineTaskRuntime;
use crate::runtime::nif_activity::{
    context_error_term, decode_string_arg, error_result_term, json_payload, ok_result_term,
    runtime_context,
};
use crate::runtime::nif_context::{NifContext, NifContextError};

/// Record `WorkflowContinuedAsNew` through the current workflow recorder and
/// terminate the current workflow process so the lifecycle monitor can start the
/// replacement run from the terminal history event.
pub(crate) fn continue_as_new_impl(
    args: &[Term],
    process_context: &mut ProcessContext,
) -> Result<Term, Term> {
    if args.len() > 255 {
        return Err(Term::NIL);
    }
    if args.len() != 1 {
        return Ok(error_result_term(
            process_context,
            &format!("continue_as_new: expected 1 arguments, got {}", args.len()),
        )
        .unwrap_or(Term::NIL));
    }

    let result = continue_as_new(args, process_context);
    match result {
        Ok(()) => Ok(ok_result_term(process_context, b"continued_as_new").unwrap_or(Term::NIL)),
        Err(term) => Ok(term),
    }
}

fn continue_as_new(args: &[Term], process_context: &mut ProcessContext) -> Result<(), Term> {
    let state = crate::runtime::nif_state::engine_nif_state(process_context)
        .map_err(|error| error_result_term(process_context, &error).unwrap_or(Term::NIL))?;
    let runtime =
        runtime_context(&state).map_err(|error| context_error_term(process_context, &error))?;
    let pid = process_context.pid().ok_or_else(|| {
        error_result_term(process_context, "continue_as_new: missing calling pid")
            .unwrap_or(Term::NIL)
    })?;
    // continue_as_new records a terminal event; a query handler must stay
    // read-only.
    crate::runtime::nif_query_pump::ensure_not_servicing_query(&state, pid, "continue_as_new")
        .map_err(|error| error_result_term(process_context, &error).unwrap_or(Term::NIL))?;
    let context = NifContext::new(
        pid,
        runtime.registry.as_ref(),
        runtime.tokio_handle.clone(),
        runtime.runtime.signal_delivery(),
    )
    .map_err(|error| context_error_term(process_context, &error))?;
    let input_text = decode_string_arg(args[0]).map_err(|error| {
        error_result_term(process_context, &format!("continue_as_new input: {error}"))
            .unwrap_or(Term::NIL)
    })?;
    let input = json_payload(process_context, &input_text, "continue_as_new", "input")?;
    let parent_run_id = context.run_id().clone();
    let input_for_record = input.clone();

    let engine_tasks = runtime.runtime.engine_tasks();
    let recorded = context.block_on_recorder(move |recorder| {
        Box::pin(async move {
            record_continuation(recorder, &engine_tasks, &parent_run_id, input_for_record).await
        })
    });

    // 🔴 THE EPOCH REFUSAL MUST STILL END THIS PROCESS — AND IT IS THE ONLY
    // REFUSAL THAT MAY.
    //
    // Before the gate below existed, the recorder call either succeeded or
    // aborted the whole NIF, and the success path ALWAYS reached `cancel_pid`.
    // The fifth writer died at that instruction. Adding a refusal that returns
    // early silently removed it from the epoch path, which would leave this
    // workflow process runnable on a scheduler `Engine::drop` deliberately
    // keeps alive — and every NIF it goes on to call writes durably: timers,
    // activity records, `spawn_child`'s whole chain, and `send_signal` into a
    // THIRD workflow's history. Against a store a successor engine may already
    // own. That is load-bearing invariant 3, and refusing one terminal while
    // licensing every other write would have made this gate a net loss.
    //
    // It is also what makes the refusal's own promise true rather than a race.
    // The message tells the operator the run stays live in history and a
    // recovering engine resumes it. That is only true if nothing keeps writing
    // to it here.
    //
    // Only the epoch refusal, though. `HistoryShape` (the run already recorded
    // a terminal) and any store error are ordinary failures that workflow code
    // may legitimately catch and handle — killing the process for those would
    // convert a recoverable authoring mistake into a dead run. The epoch is the
    // one condition that means "this engine must stop speaking for this run",
    // and it cannot clear while this process lives.
    if recorded
        .as_ref()
        .err()
        .is_none_or(refusal_must_end_the_process)
    {
        runtime.runtime.cancel_pid(context.pid()).map_err(|error| {
            error_result_term(
                process_context,
                &format!("continue_as_new termination failed: {error}"),
            )
            .unwrap_or(Term::NIL)
        })?;
    }

    match recorded {
        Ok(()) => Ok(()),
        Err(error) => Err(context_error_term(process_context, &error)),
    }
}

/// Whether a refused continue-as-new must also END the calling workflow
/// process, rather than hand the error back to workflow code.
///
/// Lifted out of the NIF for the same reason [`record_continuation`] is: this
/// predicate is the load-bearing half of the refusal, and a test that needed a
/// live beamr process to reach it would be too heavy to keep and too coarse to
/// attribute. The NIF is the adapter; this is the decision.
///
/// **True for exactly one condition.** `EngineTaskEpochClosed` means this engine
/// has begun closing and may no longer speak for this run — a state no amount of
/// workflow-code error handling can improve, and one that cannot clear while
/// this process lives. Leaving the process runnable would leave a durable writer
/// alive on a scheduler `Engine::drop` deliberately does not stop, against a
/// store a successor engine may already own.
///
/// **False for everything else, and that asymmetry is the point.**
/// `HistoryShape` (this run already recorded a terminal) and any store error are
/// ordinary failures a workflow author may legitimately catch, retry, or report.
/// Terminating the process for those would turn a recoverable authoring mistake
/// into a dead run — the mirror of the defect this whole gate exists to prevent,
/// pointed the other way.
const fn refusal_must_end_the_process(error: &NifContextError) -> bool {
    matches!(
        error,
        NifContextError::Durability(DurabilityError::EngineTaskEpochClosed { .. })
    )
}

/// The durable half of a continue-as-new transition: the terminal, then the
/// predecessor's deadline retirement, both under one recorder lock.
///
/// Lifted out of the NIF closure so the gate below is reachable by a test that
/// needs no beamr process — the NIF is the adapter, this is the behaviour.
///
/// # 🔴 THE EPOCH GATE, AND WHY IT HAD TO EXIST
///
/// `WorkflowContinuedAsNew` is a TERMINAL event, and it is the ONE terminal a
/// live workflow process could still write after its `Engine` was dropped
/// without `shutdown`. Every other terminal reachable from a workflow process
/// is already gated: the process-exit `WorkflowCompleted`/`WorkflowFailed` at
/// [`crate::lifecycle::completion`]'s append boundary, and `WorkflowTimedOut`
/// off the timer bridge's stand-down latch. This one was not, and the
/// asymmetry is the whole defect:
///
/// **the successor this terminal makes mandatory is ALREADY refused.**
/// `start_continuation_replacement` checks the same epoch before spawning the
/// replacement run, for the stated reason that spawning one hands a successor
/// engine a run this dying process is still executing. So without this gate the
/// two halves of one transition disagreed: the terminal landed, the successor
/// did not, and the run was left terminal-`ContinuedAsNew` with no continuation,
/// its declared deadline retired by the line below and its process killed by the
/// caller. A severed chain, repaired only by
/// `sweep_continued_as_new_replacements` at some later engine's startup.
///
/// # 🔴 WHAT GATING COSTS — STATED, BECAUSE IT IS NOT NOTHING
///
/// An earlier draft of this comment claimed gating "costs nothing that was not
/// already lost". That was false, and the falsehood was load-bearing enough to
/// be worth naming rather than quietly deleting.
///
/// **Refusing leaves the predecessor's deadline ARMED.** The `deadline:{run}`
/// row is retired by the line at the bottom of this function, which the refusal
/// never reaches — deliberately, because retiring it would itself be a durable
/// write by an engine that has begun closing, which is the thing being
/// prevented. So the row stays live at its original `fire_at` while wall-clock
/// passes during the outage. On a successor engine's boot,
/// `recover_active_workflows_on_startup` only SPAWNS recovered processes (replay
/// proceeds asynchronously), and `recover_timers_on_startup` then fires every
/// overdue timer synchronously. A deadline can therefore fire while this run is
/// still replaying, appending `WorkflowTimedOut` — and nothing repairs a
/// `TimedOut` run, whereas `ContinuedAsNew` has a sweep.
///
/// That trade is accepted deliberately, and here is the argument for it. What
/// the old path bought was not safety but an **unearned exemption**: it escaped
/// deadline expiry only because it retired the deadline as one half of a
/// transition whose other half never happened. The exemption was purchased with
/// a false terminal. Refusing puts this run under exactly the exposure every
/// other in-flight run already carries — a run parked in an activity when its
/// engine dies also times out if the outage outlasts its remaining budget,
/// because that is what a wall-clock deadline MEANS. And a `WorkflowTimedOut`
/// on a run past its author's declared deadline is the declared contract, not
/// data loss.
///
/// **The benefit is narrower than "recoverable versus not".** Both outcomes
/// depend on a later engine reading the same store; an embedded engine released
/// mid-process strands a `Running` run exactly as permanently as a
/// `ContinuedAsNew` one, so that condition cannot be charged to one side and
/// waived for the other. What actually improves is which machinery does the
/// repair: a `Running` run is picked up by the ordinary active-workflow recovery
/// path, rather than requiring the specialised continued-as-new sweep to notice
/// a chain that was severed. History is also left honest — no terminal claiming
/// a transition that did not occur.
///
/// # 🔴 ONE CHECK, NOT ONE PER WRITE — AND THAT DIFFERS FROM THE DEADLINE GATE
/// DELIBERATELY
///
/// [`crate::lifecycle::deadline`] re-reads its latch before each of its durable
/// writes, because abandoning that sequence part-way is safe: its deadline is
/// retired last, so a partial run of it leaves the deadline live and the work
/// simply repeats. The opposite is true here. Once the terminal has landed, the
/// deadline retirement below is no longer optional — leaving a predecessor's
/// deadline armed against a run that has already continued is the exact hazard
/// `retire_run_deadline` exists to prevent. So the check sits once, immediately
/// before the first durable write, where refusing still changes the outcome,
/// and nothing between the two writes may abandon.
///
/// The check is under the recorder lock but is still, strictly, check-then-act:
/// `begin_close` is an atomic store that takes no recorder lock, so it can land
/// between this read and the append. It narrows the window to one append rather
/// than closing it. Closing it would need the epoch inside the append path
/// itself.
///
/// # Errors
///
/// Returns [`DurabilityError::EngineTaskEpochClosed`] when this engine has begun
/// closing, [`DurabilityError::HistoryShape`] when the run already recorded a
/// terminal, or any error the recorder raises.
async fn record_continuation(
    recorder: &mut Recorder,
    engine_tasks: &EngineTaskRuntime,
    parent_run_id: &RunId,
    input: Payload,
) -> Result<(), DurabilityError> {
    // Terminal check and terminal record are atomic under the recorder lock: a
    // concurrent cancel/complete/fail transition records through the same
    // recorder, and continuing a run that already has a terminal event would
    // corrupt its history with a second terminal.
    let history = recorder.read_history().await?;
    if crate::lifecycle::completion::terminal_outcome_from_history(&history, parent_run_id)
        .is_some()
    {
        return Err(DurabilityError::HistoryShape {
            reason: format!(
                "continue_as_new rejected: run {parent_run_id} already recorded a terminal event"
            ),
        });
    }
    if !engine_tasks.is_epoch_open() {
        return Err(DurabilityError::EngineTaskEpochClosed {
            reason: format!(
                "continue_as_new refused for run {parent_run_id}: this engine has begun closing, \
                 so the replacement run it would oblige is already refused at the completion \
                 seam. Recording the terminal anyway would end this run with no continuation. \
                 Nothing is written and the run stays live in history, so a recovering engine \
                 picks it up on the ordinary active-workflow path. Its declared deadline stays \
                 ARMED, though — it is retired only as part of a completed transition — so an \
                 outage longer than the run's remaining timeout budget times the run out instead \
                 of continuing it. No operator action while the restart is prompt; a long one \
                 costs this run its deadline"
            ),
        });
    }
    recorder
        .record_workflow_continued_as_new(Utc::now(), input, None, parent_run_id.clone())
        .await?;
    // D5: retire the predecessor's declared-timeout deadline as part of the
    // continue-as-new transition, under the same recorder lock, via the shared
    // `retire_run_deadline` primitive. The deadline id is read from history (no
    // minting) and matched to exactly this predecessor run, so an uncancelled
    // predecessor deadline is never re-armed against the continued run after
    // failover.
    crate::time::retire_run_deadline(recorder, &history, parent_run_id).await
}

#[cfg(test)]
#[path = "nif_continue_as_new_tests.rs"]
mod tests;
