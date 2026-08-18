//! Process-exit completion handling.

use std::sync::Arc;

use aion_core::{Event, Payload, RunId, WorkflowId, current_lease_terminal, run_segment};
use aion_store::EventStore;
use aion_store::visibility::VisibilityStore;
use tokio::runtime::Handle;

use crate::EngineError;
use crate::loader::WorkflowCatalog;
use crate::registry::{Registry, Residency, TerminalOutcome, WorkflowHandle};
use crate::runtime::{RuntimeHandle, WorkflowProcessOutcome};
use crate::supervision::SupervisionTree;

use super::completion_retry::{
    CompletionFailure, TerminalIntent, TerminalIntentOutcome, TerminalProgress,
    arm_completion_retry, complete_process_exit,
};
use super::start::{self, StartWorkflowContext, StartWorkflowOptions};
use super::visibility::upsert_workflow_visibility;

/// Owned state needed by the runtime monitor callback.
#[derive(Clone)]
pub struct ProcessExitContext {
    /// Durable event store used to rebuild projections after terminal append.
    pub store: Arc<dyn EventStore>,
    /// Visibility index updated after terminal lifecycle events.
    pub visibility_store: Arc<dyn VisibilityStore>,
    /// Active execution registry to reconcile status and residency.
    pub registry: Arc<Registry>,
    /// Shared workflow catalog used to start continue-as-new replacements.
    pub catalog: Arc<WorkflowCatalog>,
    /// Runtime boundary used to spawn continue-as-new replacements.
    pub runtime: Arc<RuntimeHandle>,
    /// Structural supervision tree for replacement workflow placement.
    pub supervision: Arc<SupervisionTree>,
    /// Tokio runtime handle used to run async recorder/store work from the monitor thread.
    pub tokio_handle: Handle,
    /// Schema validating initial search attributes on continue-as-new replacements.
    pub search_attribute_schema: Arc<aion_core::SearchAttributeSchema>,
}

/// Handle one observed workflow process exit.
///
/// The monitor calls this from outside the workflow dirty NIF thread. All durable
/// terminal events are recorded through the handle-owned Recorder, then registry
/// projections are reconciled from authoritative history and subscribers are
/// notified.
///
/// # A transient store failure is retried, and NOT on this thread
///
/// The terminal event is the only thing that makes a finished run stop
/// projecting `Running` (status is a projection, never a stored field). Before
/// this existed, a single transient store error propagated to the monitor
/// callback, which logged it and returned — and because `monitor_process` is
/// `FnOnce` and the exit record is retired unconditionally, nothing re-armed.
/// The run stayed `Running` until the node restarted and the startup sweep
/// installed a fresh monitor.
///
/// The retry deliberately does **not** run here. Every process-exit callback on
/// the node is drained by ONE dedicated thread
/// (`runtime::process_exit_callback`), and this function is `block_on`-ed on it.
/// A backoff loop in this call would hold that thread, so one unlucky run's
/// flaky store would stall every other workflow's completion — turning a
/// single-run wedge into a node-wide one. The retry is therefore armed on the
/// engine task runtime and this call returns immediately.
///
/// # Errors
///
/// Returns typed recorder, visibility, registry or invariant errors — including
/// the store and durability faults a retry cannot repair, such as a
/// `SequenceConflict` or a non-determinism violation.
///
/// A *transient* durable failure returns `Ok(())` in every case, including the
/// one where arming was refused because the epoch is closing and therefore
/// nothing owns the terminal. That is deliberate and it is the reason
/// `arm_completion_retry` logs at `warn` rather than leaving the caller to
/// report: the only caller is the process-exit monitor closure, which can do
/// nothing with an `Err` but log it, and an epoch closing under an exiting
/// process is an expected shutdown shape rather than an engine fault. The run
/// stays `Running` and the next epoch's startup sweep re-installs a monitor —
/// which is exactly what that warn line says.
pub fn handle_process_exit(
    context: ProcessExitContext,
    handle: WorkflowHandle,
    outcome: Result<WorkflowProcessOutcome, EngineError>,
) -> Result<(), EngineError> {
    context
        .tokio_handle
        .clone()
        .block_on(handle_process_exit_async(context, handle, outcome))
}

async fn handle_process_exit_async(
    context: ProcessExitContext,
    handle: WorkflowHandle,
    outcome: Result<WorkflowProcessOutcome, EngineError>,
) -> Result<(), EngineError> {
    let intent = TerminalIntent::from_outcome(outcome);
    match complete_process_exit(&context, &handle, &intent, TerminalProgress::NotRecorded).await {
        Ok(_progress) => Ok(()),
        Err(CompletionFailure::Invariant(error, _)) => Err(error),
        Err(CompletionFailure::Retryable(error, progress)) => {
            arm_completion_retry(context, handle, intent, &error, progress);
            Ok(())
        }
    }
}

/// Decide whether this monitor's terminal is still this monitor's to write.
///
/// Extracted from [`handle_process_exit_attempt`] because the decision is a
/// question in its own right — "has this run's writer slot moved on?" — and
/// because the caller had grown past the size ceiling with it inline. The
/// reasoning below is the whole justification for the guard's SHAPE, which is
/// two disjuncts and deliberately not three.
///
/// 🔴 A TERMINAL THAT WAS DURABLE AND IS NOW GONE MEANS THE RUN WAS
/// REOPENED, NOT THAT A WRITE WAS LOST.
///
/// Reachable, and benign: once an earlier attempt set `Recorded` the
/// run is terminal, so `reopen` becomes admissible — and the retry is
/// still spinning precisely when the EVENT store is healthy while the
/// separate VISIBILITY store is the thing failing the upsert. An
/// operator reopening in that window appends `WorkflowReopened`,
/// which supersedes the terminal, so this read now finds none.
///
/// Appending again would be a second writer racing a live run. The
/// store's expected-sequence discipline refuses it — the recorder
/// never re-reads the store head — so nothing is corrupted, but the
/// operator is handed a `SequenceConflict`, which this codebase
/// defines as THE double-writer indicator, plus a message saying an
/// unretryable failure hit after the terminal was recorded. That
/// reads as an incident. It is a reopen.
///
/// 🔴 THE TEST IS LEASE IDENTITY, NOT HISTORY, AND NOT THIS
/// MONITOR'S MEMORY.
///
/// An earlier revision gated this on `*progress == Recorded` alone —
/// "did *I* append the terminal". That covers one branch of two.
/// `progress` is this monitor's private recollection; the run can be
/// driven terminal by a path this monitor never took (an explicit
/// cancel, a `WorkflowDeadlineHandler` timeout teardown, a
/// predecessor engine's writer) and then reopened, all while this
/// monitor still holds `NotRecorded`. On that branch the old guard
/// let the append through — into a live reopened run — which is the
/// exact outcome F11 exists to prevent, on the branch F11 did not
/// cover.
///
/// 🔴 THE OBVIOUS SECOND DISJUNCT — "is there a `WorkflowReopened` in
/// this run's segment" — IS A REGRESSION, AND WAS WRITTEN HERE ONCE.
///
/// `reopen` reuses the SAME run id (`reopen.rs`: the reopen event
/// carries `run.clone()` and `respawn_and_register` re-registers
/// under it), so that event sits in the segment permanently. This
/// function is then entered IDENTICALLY by a superseded lease's stale
/// exit callback and by the reopened lease's own ordinary exit — the
/// history predicate is true in both, so it cannot separate them. It
/// can only choose which one to break, and breaking the second one
/// strands the run in `Running` forever with the completion doorbell
/// unrung and `Ok` returned, so not one line is logged.
///
/// The discriminator that does exist is the lease itself. Reopen
/// spawns a fresh process and registers a NEW `WorkflowHandle` around
/// that new pid under the same `(workflow, run)` key; a stale
/// monitor's closure still holds the OLD handle. So: ask the registry
/// who the current lease is, and stand down only when a DIFFERENT one
/// is positively observed. An absent entry (the run has since been
/// unloaded) or an unreadable registry falls through to the behaviour
/// that predates this guard — this refuses to invent a stand-down out
/// of the absence of evidence.
///
/// 🔴 The `progress` disjunct is kept because it covers the one
/// window in which lease identity is BLIND, and that window is a real
/// interval in `reopen`, not a hypothetical.
///
/// `reopen` removes the terminal-cached handle from the registry
/// FIRST (`reopen.rs`, the `cached_status().is_terminal()` arm),
/// THEN appends `WorkflowReopened`, and only then does
/// `respawn_and_register` put the successor handle back. Between the
/// append and the registration the registry holds NOTHING for this
/// key: `get` yields `None`, so `superseded_by_a_newer_lease` is
/// false — correctly, because no newer lease is observable yet — and
/// history yields no terminal, because the reopen superseded it. A
/// retry attempt landing in that interval, having already recorded
/// its own terminal, is left with `progress` as the only thing that
/// still knows it is finished. The cost of losing that knowledge is
/// the same one stated at the top of this block and worth restating
/// exactly: the reopen wrote through a recorder of its OWN, so this
/// monitor's recorder is stale — by AT LEAST one, and by more when
/// the reopen re-armed timers, since each re-arm appends its own
/// `TimerStarted` after `WorkflowReopened` (an earlier revision said
/// "by one", which is the floor and not the count) — and the append
/// is REFUSED on sequence, not silently doubled. The refusal does not
/// depend on the size of the gap, only on there being one. What the
/// disjunct buys is not
/// data integrity — the store already has that — it is the operator
/// not being handed a `SequenceConflict` incident for an ordinary
/// reopen. `a_recorded_monitor_stands_down_inside_the_reopen_window`
/// is the test that holds it.
///
/// 🔴 NAMED RESIDUAL. The mirror case is NOT covered and is
/// reachable: a monitor holding `NotRecorded` whose run was driven
/// terminal by another writer and then reopened, landing inside that
/// same registry-empty interval. Neither disjunct fires, so it
/// attempts the append and eats the same false incident.
///
/// The obvious close is a third disjunct keyed on "registry empty
/// AND a reopen sits in this segment". It is deliberately NOT
/// written, and the reason is stronger than it first looks: **an
/// empty registry entry is not a reopen signature.** Five ordinary
/// terminal paths remove the entry the moment they finish —
/// `terminate::complete` (`terminate.rs`), `terminate::fail`,
/// `terminate::cancel`, `continue_as_new`, and the deadline
/// teardown's "deregister LAST" step. (This monitor's own
/// `reconcile_terminal_registry` is the exception: it suspends
/// rather than removes. Generalising from that one path to "nothing
/// empties the entry" was the mistake an earlier revision of this
/// comment made, and it is retracted.)
///
/// So the third disjunct would be the history predicate refuted
/// above wearing a registry check as a disguise: on any path where a
/// reopen sits in the segment and the entry has been removed by a
/// LATER legitimate terminal rather than by the reopen, it fires on
/// that terminal and strands the run in `Running` forever with the
/// doorbell unrung and nothing logged at any level. A logged,
/// fail-safe false alarm is the better residual than a possible
/// silent permanent zombie.
///
/// An earlier revision justified this disjunct by saying it "survives
/// a compaction that the history scan would not". That reason was
/// FALSE — history compaction does not exist in this codebase; it is
/// an open design question and a reserved, unimplemented event — and
/// it is retracted.
/// 🔴 A poisoned registry is REPORTED, not swallowed. This read used
/// `.ok().flatten()`, which folded `RegistryPoisoned` — a real fault,
/// and the one condition under which this whole check is blind — into
/// the same `false` a healthy registry produces for an unsuperseded
/// run. The two are not the same fact and must not print the same.
///
/// `false` is still the value chosen on the error, and that is the
/// fail-safe direction rather than a convenience: it sends this
/// monitor on to ATTEMPT the append, where the store's own sequence
/// check is the authority. Refusing to attempt would strand a run
/// whose terminal nobody else is going to write.
fn monitor_stands_down(
    context: &ProcessExitContext,
    handle: &WorkflowHandle,
    progress: TerminalProgress,
) -> bool {
    let superseded_by_a_newer_lease =
        match context.registry.get(handle.workflow_id(), handle.run_id()) {
            Ok(current) => current.is_some_and(|current| current.pid() != handle.pid()),
            Err(error) => {
                tracing::error!(
                    workflow_id = %handle.workflow_id(),
                    run_id = %handle.run_id(),
                    monitor_pid = handle.pid(),
                    error = %error,
                    "the workflow registry could not be read while deciding whether a newer \
                     lease had superseded this monitor; proceeding to attempt the terminal \
                     append, where the store's sequence check is the authority"
                );
                false
            }
        };
    if matches!(progress, TerminalProgress::Recorded) || superseded_by_a_newer_lease {
        tracing::info!(
            workflow_id = %handle.workflow_id(),
            run_id = %handle.run_id(),
            monitor_pid = handle.pid(),
            superseded_by_a_newer_lease,
            "abandoning workflow completion retry: this run's terminal is no longer \
             this monitor's to write — either it already recorded one, or the run was \
             reopened and a newer lease now holds the writer slot"
        );
        return true;
    }
    false
}

/// Refuse to write if this engine's background-task epoch has closed.
///
/// One rule in one function so the two sites that need it cannot drift: a rule
/// known in two places with nothing forcing agreement has already drifted. Both
/// callers are durable-write boundaries — the recorder critical section, and the
/// continuation start that follows a `ContinuedAsNew` terminal.
///
/// 🔴 THIS IS THE ONLY PLACE `EngineTaskEpochClosed` IS CONSTRUCTED, AND
/// ANOTHER CRATE DEPENDS ON THAT BEING TRUE. `aion-client`'s `map_engine_error`
/// deliberately gives the variant no arm, on the stated ground that it is
/// reachable only through the completion monitor and never through any
/// operation that transport exposes — so it would be classifying an outcome
/// nobody can observe. That argument is sound only while this remains the sole
/// construction site. Nothing in the build enforces it: a second site reachable
/// from a client-facing operation would silently take that transport's `_ =>`
/// catch-all and be reported as a generic server error. If you add one, go and
/// settle `aion-client`'s omission first.
fn refuse_if_epoch_closed(
    context: &ProcessExitContext,
    handle: &WorkflowHandle,
) -> Result<(), EngineError> {
    if context.runtime.engine_tasks().is_epoch_open() {
        return Ok(());
    }
    Err(EngineError::EngineTaskEpochClosed {
        workflow_id: handle.workflow_id().to_string(),
        run_id: handle.run_id().to_string(),
    })
}

/// One completion attempt.
///
/// `progress` is an out-parameter rather than a return value because the fact
/// it carries must survive the `?` that abandons this function: the caller needs
/// to know whether the terminal landed *precisely when the attempt failed*, and
/// a `Result` can only carry one of those two things at a time. It is set at the
/// two instants — and only those two — at which a terminal event for this run is
/// durably in history.
pub(super) async fn handle_process_exit_attempt(
    context: &ProcessExitContext,
    handle: &WorkflowHandle,
    intent: &TerminalIntent,
    progress: &mut TerminalProgress,
) -> Result<(), EngineError> {
    // The terminal check and the terminal record must be atomic under the
    // recorder lock: a concurrent cancel/complete/fail transition records
    // through the same recorder, and a check outside the lock would let both
    // writers append a terminal event for the same run.
    let recorded = {
        let recorder = handle.recorder();
        let mut recorder = recorder.lock().await;
        // 🔴 THE APPEND BOUNDARY, for EVERY durable write this attempt makes.
        //
        // A completion retry re-derives its engine reference once per attempt
        // and then holds it strongly for the whole attempt — a history read,
        // this lock, and a store round-trip — so a check at the top of the
        // attempt can be walked past by everything that follows it. Worse, that
        // strong reference is itself what keeps `EngineTaskRuntime::drop` from
        // running, so the backstop is pinned shut for exactly the span of the
        // append it would need to stop. Holding the recorder lock with the
        // writes about to happen is the only place where refusing changes the
        // outcome.
        //
        // 🔴 It sits BEFORE the branch, not inside one arm of it. The first cut
        // put it in the no-terminal-yet arm only, which left the resume arm's
        // `retire_run_deadline` — a real `TimerCancelled` append — ungated, and
        // its own comment claimed the boundary was "checked here and nowhere
        // earlier" while an earlier append existed. The mutation that covers
        // this gate drives the fresh-append arm, so the hole was invisible to
        // the board: A MUTATION ONLY MEASURES THE BRANCH THE TEST EXECUTES.
        //
        // 🔴 THIS REFUSES SOME EXITS DURING GRACEFUL SHUTDOWN, AND THAT IS THE
        // TRADE, NOT AN OVERSIGHT.
        //
        // An earlier revision of this comment claimed the ordinary path was
        // safe by construction, because "`RuntimeHandle::shutdown` closes the
        // epoch only after every process-exit callback has drained". That is
        // true of `RuntimeHandle::shutdown` — and FALSE of `Engine::shutdown`,
        // which is the call a server actually makes. `Engine::shutdown` closes
        // the epoch FIRST (deliberately: see the comment at its `begin_close`),
        // then waits on `ShutdownGate::close_and_wait`, and only reaches
        // `process_exits.begin_shutdown()` afterwards. Exit callbacks are still
        // admitted for that whole span.
        //
        // So: a run that exits between `begin_close()` and `begin_shutdown()`
        // is refused here, records no terminal, and stays `Running` in the
        // store with one `error!` line naming it. The span is UNBOUNDED —
        // `close_and_wait` is a condvar wait with no timeout — and it is
        // longest under exactly the degraded-store condition this whole
        // completion-retry machinery exists for.
        //
        // It is still the right trade, because the alternative is worse: the
        // epoch existing at all is what stops a retry appending a terminal
        // while a successor recovers the same history. A refused exit costs a
        // recovery sweep, which the successor performs anyway; an unrefused one
        // risks two writers on one run. Fail-safe, and the operator has the
        // `error!` line — but it is a real window and it is named here rather
        // than denied.
        refuse_if_epoch_closed(context, handle)?;
        let history = context.store.read_history(handle.workflow_id()).await?;
        if let Some(existing) = terminal_outcome_from_history(&history, handle.run_id()) {
            // DURABLE POINT 1: a terminal event for this run is already in
            // history. Recorded before the deadline retirement below, because
            // that retirement can fail and the terminal is durable either way.
            *progress = TerminalProgress::Recorded;
            // Resume an interrupted terminal transition: the terminal append and
            // its deadline cancellation are two durable writes, so a crash
            // between them (in a complete/fail/cancel/CAN writer) leaves the
            // deadline outstanding. Re-encountering this run's own terminal,
            // complete the cancellation under the recorder lock so whole-history
            // recovery never re-arms the predecessor deadline. Idempotent: an
            // already-cancelled (or never-armed) deadline records nothing.
            //
            // A `WorkflowTimedOut` terminal is the ONE exception: its deadline is
            // owned exclusively by `WorkflowDeadlineHandler` teardown, which keeps
            // it live as its own resume anchor across fallible visibility work and
            // retires it LAST. This monitor is woken by that teardown's own
            // `cancel_pid`, so retiring the deadline here would destroy the
            // teardown's anchor mid-flight. Leave it to the handler; a re-fire via
            // `recover_due` drives `ResumeTeardown`.
            if !matches!(existing, TerminalOutcome::TimedOut(_)) {
                crate::time::retire_run_deadline(&mut recorder, &history, handle.run_id()).await?;
            }
            Err(existing)
        } else {
            if monitor_stands_down(context, handle, *progress) {
                return Ok(());
            }
            let outcome = match &intent.outcome {
                TerminalIntentOutcome::Completed(result) => {
                    recorder
                        .record_workflow_completed(intent.exit_time, result.clone())
                        .await?;
                    TerminalOutcome::Completed(result.clone())
                }
                TerminalIntentOutcome::Failed(error) => {
                    recorder
                        .record_workflow_failed(intent.exit_time, error.clone())
                        .await?;
                    TerminalOutcome::Failed(error.clone())
                }
            };
            // DURABLE POINT 2: this attempt's own append returned `Ok`, so the
            // terminal event is in the store. Everything below — the deadline
            // retirement in this block, and the doorbell, visibility upsert and
            // registry reconcile after it — is bookkeeping that follows a run
            // which is already terminal.
            *progress = TerminalProgress::Recorded;
            // LAW 1 + D5: this monitor recorded the terminal, so it permanently
            // retires the run's declared-timeout deadline under the SAME recorder
            // lock, via the shared `retire_run_deadline` primitive. The deadline
            // id is read from history (no speculative minting) and matched to
            // exactly this run, so a timeout-less run — which has no such
            // `TimerStarted` — retires nothing and touches no deadline object at
            // all. `WorkflowIntent` keeps reopen from resurrecting it and closes
            // the whole-history `outstanding_future_timers` re-arm hazard.
            crate::time::retire_run_deadline(&mut recorder, &history, handle.run_id()).await?;
            Ok(outcome)
        }
    };

    // Notify as soon as the durable terminal is decided: subscribers
    // (result waiters, child-terminal watchers) resolve from the recorded
    // store truth, so the doorbell must never be muted by a failure in the
    // post-record bookkeeping below — a watcher parked on a doorbell that
    // never rings strands the awaiting parent for the whole epoch.
    let terminal = match recorded {
        Err(existing) => {
            handle.completion().notify(existing.clone());
            // Any deadline retirement a NON-timeout pre-recorded terminal needed
            // was already completed above, under the recorder lock, by the resume
            // call — repairing a transition interrupted between its terminal append
            // and its deadline cancellation. A `WorkflowTimedOut` terminal is left
            // untouched on purpose: its deadline belongs to the deadline handler's
            // teardown, not to this monitor.
            //
            // The visibility upsert runs on THIS path too, and that is not
            // redundancy. It used to be safe to skip: reaching here meant another
            // writer had recorded the terminal, and that writer did its own
            // upsert. Under retry the "other writer" can be this operation's own
            // earlier attempt, which may have appended the terminal and then died
            // before upserting — so skipping would trade a wedged status for a
            // permanently stale index. It is an upsert, so the genuinely
            // redundant case costs one write and changes nothing.
            //
            // 🔴 With ONE exception, and it is the same exception as the
            // deadline above — the first cut applied that reasoning to the
            // deadline retirement and not to the visibility write, which left a
            // lost update behind.
            //
            // A `WorkflowTimedOut` terminal can never be this operation's own
            // earlier attempt: this monitor appends `WorkflowCompleted` or
            // `WorkflowFailed` and nothing else. So for a timed-out run the
            // "other writer" is always `WorkflowDeadlineHandler` teardown, which
            // is the pre-retry safe-to-skip condition, unchanged. Writing anyway
            // makes this a second writer of one run's index CONCURRENTLY with
            // that teardown — this monitor is woken by the teardown's own
            // `cancel_pid`, so the two overlap by construction. Both derive the
            // row from history and both re-read it, but a fresh read does not
            // order the writes: read-H1 / teardown-writes-H2 / write-H1 leaves
            // `close_time` and `failed_step` one step stale, permanently.
            //
            // Nothing is lost by yielding. If the teardown dies before its own
            // upsert, its deadline is still live — it retires it LAST precisely
            // as its resume anchor — so `recover_due` re-fires and
            // `ResumeTeardown` redoes the visibility work. That is a real
            // recovery path, not an assumption that the teardown succeeds.
            if !matches!(existing, TerminalOutcome::TimedOut(_)) {
                upsert_workflow_visibility(
                    Arc::clone(&context.store),
                    Arc::clone(&context.visibility_store),
                    handle.workflow_id(),
                    handle.run_id(),
                )
                .await?;
            }
            reconcile_terminal_registry(context, handle.workflow_id(), handle.run_id()).await?;
            if let TerminalOutcome::ContinuedAsNew {
                input,
                workflow_type,
                parent_run_id,
            } = existing
            {
                start_continuation_replacement(
                    context,
                    handle,
                    input,
                    workflow_type,
                    parent_run_id,
                )
                .await?;
            }
            return Ok(());
        }
        Ok(terminal) => terminal,
    };
    handle.completion().notify(terminal);

    // 🔴 Below this line the recorder lock has dropped and the epoch gate above
    // no longer covers anything. That is correct and deliberate — neither call
    // appends to history, so neither can be the second writer the gate exists to
    // stop — but it does mean both can run to completion against a store while
    // this engine's epoch closes underneath them. They are idempotent
    // derived-index writes (both re-read history and upsert), so a successor's
    // recovery redoes them without conflict; what they are NOT is protected, and
    // a reader of the gate should not assume otherwise. The same is true of the
    // pair inside the already-terminal branch above.
    upsert_workflow_visibility(
        Arc::clone(&context.store),
        Arc::clone(&context.visibility_store),
        handle.workflow_id(),
        handle.run_id(),
    )
    .await?;
    reconcile_terminal_registry(context, handle.workflow_id(), handle.run_id()).await?;
    Ok(())
}

async fn reconcile_terminal_registry(
    context: &ProcessExitContext,
    id: &WorkflowId,
    run: &RunId,
) -> Result<(), EngineError> {
    let history = context.store.read_history(id).await?;
    context.registry.reconcile(id, run, &history)?;
    context
        .registry
        .replace_residency(id, run, Residency::Suspended)?;
    Ok(())
}

async fn start_continuation_replacement(
    context: &ProcessExitContext,
    handle: &WorkflowHandle,
    input: Payload,
    workflow_type: Option<String>,
    parent_run_id: RunId,
) -> Result<(), EngineError> {
    // 🔴 The second append boundary, and the heaviest one: this does not merely
    // append `WorkflowStarted`, it SPAWNS A BEAM PROCESS and installs a fresh
    // monitor. Doing that after the epoch closed hands a successor engine a run
    // this dying process is still executing. It sits outside the recorder
    // critical section — a different run's history is being written — so the
    // gate inside `handle_process_exit_attempt` cannot reach it.
    refuse_if_epoch_closed(context, handle)?;
    let replacement_type = workflow_type.as_deref().unwrap_or(handle.workflow_type());
    let already_started = context
        .store
        .read_history(handle.workflow_id())
        .await?
        .iter()
        .any(|event| {
            matches!(
                event,
                Event::WorkflowStarted {
                    parent_run_id: Some(existing_parent),
                    ..
                } if existing_parent == &parent_run_id
            )
        });
    if already_started {
        return Ok(());
    }

    start::start_workflow_with_options(
        StartWorkflowContext {
            store: Arc::clone(&context.store),
            visibility_store: Arc::clone(&context.visibility_store),
            catalog: Arc::clone(&context.catalog),
            runtime: Arc::clone(&context.runtime),
            supervision: Arc::clone(&context.supervision),
            registry: Arc::clone(&context.registry),
            signal_handoff: None,
            search_attribute_schema: Arc::clone(&context.search_attribute_schema),
            monitor_tokio_handle: context.tokio_handle.clone(),
        },
        replacement_type,
        input,
        StartWorkflowOptions {
            workflow_id: Some(handle.workflow_id().clone()),
            // Continue-as-new reuses the existing id; steering does not apply.
            routing_key: None,
            parent_run_id: Some(parent_run_id),
            parent_workflow_id: None,
            // D1: the continue-as-new successor resolves the latest loaded
            // version at record time, identically to the startup sweep.
            loaded_version: None,
            // Recorded attributes carry into the replacement run's projection.
            search_attributes: std::collections::HashMap::new(),
            namespace: Some(handle.namespace().to_owned()),
            // Engine-internal re-entry: this input was already produced by the
            // predecessor run's own workflow code. Re-admitting it here could
            // only kill a live continuation, never protect one.
            input_admission: start::InputAdmission::Trusted,
        },
    )
    .await?;
    Ok(())
}

/// This run's current-lease terminal outcome, if it has one.
///
/// Run-scoped and reset-aware: a `WorkflowReopened` supersedes the terminal
/// before it, so a reopened run reports `None` until it terminates again.
/// Distinct from the whole-history `terminal_outcome_from_history` in
/// `engine/api.rs`, which answers a different question.
pub(crate) fn terminal_outcome_from_history(
    events: &[Event],
    run_id: &RunId,
) -> Option<TerminalOutcome> {
    // Reset-aware: scope to the run, then take the current lease's terminal
    // event. A WorkflowReopened after a terminal supersedes it, so a reopened run
    // reports no terminal outcome until it terminates again.
    match current_lease_terminal(run_segment(events, run_id))? {
        Event::WorkflowCompleted { result, .. } => Some(TerminalOutcome::Completed(result.clone())),
        Event::WorkflowFailed { error, .. } => Some(TerminalOutcome::Failed(error.clone())),
        Event::WorkflowCancelled { reason, .. } => Some(TerminalOutcome::Cancelled(reason.clone())),
        Event::WorkflowTimedOut { timeout, .. } => Some(TerminalOutcome::TimedOut(timeout.clone())),
        Event::WorkflowContinuedAsNew {
            input,
            workflow_type,
            parent_run_id,
            ..
        } if parent_run_id == run_id => Some(TerminalOutcome::ContinuedAsNew {
            input: input.clone(),
            workflow_type: workflow_type.clone(),
            parent_run_id: parent_run_id.clone(),
        }),
        // current_lease_terminal yields only terminal lifecycle events; a
        // ContinuedAsNew whose parent is a different run is not this run's
        // outcome.
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use aion_core::{Event, Payload, WorkflowStatus};
    use aion_package::ContentHash;
    use aion_store::visibility::VisibilityStore;
    use aion_store::{EventStore, InMemoryStore};
    use serde_json::json;

    use super::super::completion_retry::{
        CompletionRetryContext, FaultReporter, failure_discriminant, retry_process_exit,
    };
    use super::{
        CompletionFailure, ProcessExitContext, TerminalIntent, TerminalIntentOutcome,
        TerminalProgress, complete_process_exit, handle_process_exit_async,
        terminal_outcome_from_history,
    };
    use crate::durability::Recorder;
    use crate::loader::WorkflowCatalog;
    use crate::registry::{
        CompletionNotifier, HandleResidency, Registry, TerminalOutcome, WorkflowHandle,
        WorkflowHandleParts,
    };
    use crate::runtime::{RuntimeConfig, RuntimeHandle, WorkflowProcessOutcome};
    use crate::supervision::SupervisionTree;

    struct ActiveWorkflow {
        context: ProcessExitContext,
        handle: WorkflowHandle,
    }

    fn payload(label: &str) -> Result<Payload, aion_core::PayloadError> {
        Payload::from_json(&json!({ "label": label }))
    }

    fn workflow_error(message: &str) -> aion_core::WorkflowError {
        aion_core::WorkflowError {
            message: message.to_owned(),
            details: None,
        }
    }

    /// A retry cadence fast enough that the rest of this module does not spend
    /// its wall clock waiting, supplied the way production supplies it — through
    /// the runtime config.
    ///
    /// 🔴 Read this narrowly. Passing the ladder through `RuntimeConfig` is not
    /// by itself evidence that anything READS it: 1 ms / 5 ms and the inherited
    /// default of 1 ms / 8 ms are indistinguishable at the resolution these
    /// tests observe, so every one of them would stay green against a hardcoded
    /// constant. The claim that the configured ladder actually governs the retry
    /// is held by exactly one test — `the_configured_backoff_ladder_governs_when_
    /// a_retry_lands` — which is the only place the two values are separated far
    /// enough apart to tell them apart.
    fn fast_backoff()
    -> Result<crate::runtime::CompletionRetryConfig, crate::runtime::InvalidCompletionRetryLadder>
    {
        crate::runtime::CompletionRetryConfig::try_new(
            std::time::Duration::from_millis(1),
            std::time::Duration::from_millis(5),
        )
    }

    /// An active workflow over a store that can be told to fail transiently.
    ///
    /// Returns the fault handle alongside, because a test that cannot turn the
    /// fault on is a test of the healthy path wearing the name of a retry proof.
    async fn flaky_workflow()
    -> Result<(ActiveWorkflow, Arc<crate::store_faults::FlakyStore>), Box<dyn std::error::Error>>
    {
        flaky_workflow_with_retry(fast_backoff()?).await
    }

    /// The same fixture with the retry ladder chosen by the caller, for the one
    /// test whose subject IS the ladder.
    async fn flaky_workflow_with_retry(
        completion_retry: crate::runtime::CompletionRetryConfig,
    ) -> Result<(ActiveWorkflow, Arc<crate::store_faults::FlakyStore>), Box<dyn std::error::Error>>
    {
        let flaky = Arc::new(crate::store_faults::FlakyStore::new());
        let visibility = Arc::new(InMemoryStore::default());
        let active = active_workflow_over(
            Arc::clone(&flaky) as Arc<dyn EventStore>,
            visibility as Arc<dyn VisibilityStore>,
            completion_retry,
        )
        .await?;
        Ok((active, flaky))
    }

    async fn active_workflow() -> Result<ActiveWorkflow, Box<dyn std::error::Error>> {
        let backing = Arc::new(InMemoryStore::default());
        active_workflow_over(
            Arc::clone(&backing) as Arc<dyn EventStore>,
            backing as Arc<dyn VisibilityStore>,
            fast_backoff()?,
        )
        .await
    }

    async fn active_workflow_over(
        store: Arc<dyn EventStore>,
        visibility_store: Arc<dyn VisibilityStore>,
        completion_retry: crate::runtime::CompletionRetryConfig,
    ) -> Result<ActiveWorkflow, Box<dyn std::error::Error>> {
        let registry = Arc::new(Registry::default());
        let workflow_id = aion_core::WorkflowId::new_v4();
        let run_id = aion_core::RunId::new_v4();
        let mut recorder = Recorder::new(workflow_id.clone(), Arc::clone(&store));
        recorder
            .record_workflow_started(
                chrono::Utc::now(),
                crate::durability::WorkflowStartRecord {
                    workflow_type: "checkout".to_owned(),
                    input: payload("input")?,
                    run_id: run_id.clone(),
                    parent_run_id: None,
                    parent_workflow_id: None,
                    package_version: aion_core::PackageVersion::new("a".repeat(64)),
                },
            )
            .await?;
        let handle = WorkflowHandle::new(WorkflowHandleParts {
            workflow_id: workflow_id.clone(),
            run_id: run_id.clone(),
            pid: 1,
            workflow_type: "checkout".to_owned(),
            namespace: String::from("default"),
            loaded_version: ContentHash::from_bytes([9; 32]),
            cached_status: WorkflowStatus::Running,
            residency: HandleResidency::Resident,
            recorder,
            completion: CompletionNotifier::new(),
        });
        registry.insert((workflow_id, run_id), handle.clone())?;
        Ok(ActiveWorkflow {
            context: ProcessExitContext {
                store,
                visibility_store,
                registry,
                catalog: Arc::new(WorkflowCatalog::new()),
                runtime: Arc::new(RuntimeHandle::new(
                    RuntimeConfig::new(Some(1)).with_completion_retry(completion_retry),
                )?),
                supervision: Arc::new(SupervisionTree::new()),
                tokio_handle: tokio::runtime::Handle::current(),
                search_attribute_schema: Arc::new(aion_core::SearchAttributeSchema::new()),
            },
            handle,
        })
    }

    #[tokio::test]
    async fn normal_exit_records_completed_reconciles_and_notifies()
    -> Result<(), Box<dyn std::error::Error>> {
        let active = active_workflow().await?;
        let result = payload("result")?;
        let mut early = active.handle.completion().subscribe();

        handle_process_exit_async(
            active.context.clone(),
            active.handle.clone(),
            Ok(WorkflowProcessOutcome::Completed(result.clone())),
        )
        .await?;
        early.changed().await?;

        assert_eq!(
            early.borrow().clone(),
            Some(TerminalOutcome::Completed(result.clone()))
        );
        assert_eq!(
            active.handle.completion().subscribe().borrow().clone(),
            Some(TerminalOutcome::Completed(result.clone()))
        );
        let registered = active
            .context
            .registry
            .get(active.handle.workflow_id(), active.handle.run_id())?
            .ok_or("missing registered handle")?;
        assert_eq!(registered.cached_status(), WorkflowStatus::Completed);
        assert_eq!(registered.residency(), HandleResidency::Suspended);
        let history = active
            .context
            .store
            .read_history(active.handle.workflow_id())
            .await?;
        match history.as_slice() {
            [
                Event::WorkflowStarted { .. },
                Event::WorkflowCompleted {
                    result: recorded, ..
                },
            ] => {
                assert_eq!(recorded, &result);
            }
            other => return Err(format!("expected started then completed, found {other:?}").into()),
        }
        Ok(())
    }

    #[test]
    fn terminal_outcome_is_scoped_to_requested_run_segment()
    -> Result<(), Box<dyn std::error::Error>> {
        let old_run_id = aion_core::RunId::new(uuid::Uuid::from_u128(1));
        let new_run_id = aion_core::RunId::new(uuid::Uuid::from_u128(2));
        let input = payload("next")?;
        let result = payload("done")?;
        let workflow_id = aion_core::WorkflowId::new_v4();
        let envelope = |seq| aion_core::EventEnvelope {
            seq,
            recorded_at: chrono::Utc::now(),
            workflow_id: workflow_id.clone(),
        };
        let events = vec![
            Event::WorkflowStarted {
                envelope: envelope(1),
                workflow_type: "checkout".to_owned(),
                input: payload("first")?,
                run_id: old_run_id.clone(),
                parent_run_id: None,
                parent_workflow_id: None,
                package_version: aion_core::PackageVersion::new("a".repeat(64)),
            },
            Event::WorkflowContinuedAsNew {
                envelope: envelope(2),
                input: input.clone(),
                workflow_type: None,
                parent_run_id: old_run_id.clone(),
            },
            Event::WorkflowStarted {
                envelope: envelope(3),
                workflow_type: "checkout".to_owned(),
                input,
                run_id: new_run_id.clone(),
                parent_run_id: Some(old_run_id.clone()),
                parent_workflow_id: None,
                package_version: aion_core::PackageVersion::new("a".repeat(64)),
            },
            Event::WorkflowCompleted {
                envelope: envelope(4),
                result: result.clone(),
            },
        ];

        assert_eq!(
            terminal_outcome_from_history(&events, &old_run_id),
            Some(TerminalOutcome::ContinuedAsNew {
                input: payload("next")?,
                workflow_type: None,
                parent_run_id: old_run_id,
            })
        );
        assert_eq!(
            terminal_outcome_from_history(&events, &new_run_id),
            Some(TerminalOutcome::Completed(result))
        );
        Ok(())
    }

    #[tokio::test]
    async fn process_exit_resumes_interrupted_deadline_cancellation()
    -> Result<(), Box<dyn std::error::Error>> {
        // An interrupted terminal transition: the terminal committed but its
        // deadline cancellation did not, leaving the deadline outstanding. The
        // process-exit monitor, re-encountering the run's own terminal, completes
        // the interrupted cancellation under the recorder lock — closing the
        // whole-history re-arm hazard.
        let active = active_workflow().await?;
        let run_id = active.handle.run_id().clone();
        let deadline_id = crate::time::deadline_timer_id(&run_id)?;
        let result = payload("result")?;
        {
            let recorder = active.handle.recorder();
            let mut recorder = recorder.lock().await;
            recorder
                .record_timer_started(chrono::Utc::now(), deadline_id.clone(), chrono::Utc::now())
                .await?;
            recorder
                .record_workflow_completed(chrono::Utc::now(), result.clone())
                .await?;
        }
        let before = active
            .context
            .store
            .read_history(active.handle.workflow_id())
            .await?;
        assert!(
            crate::time::outstanding_deadline_timer(&before, &run_id).is_some(),
            "the deadline is outstanding before the resume"
        );

        handle_process_exit_async(
            active.context.clone(),
            active.handle.clone(),
            Ok(WorkflowProcessOutcome::Completed(result.clone())),
        )
        .await?;

        let history = active
            .context
            .store
            .read_history(active.handle.workflow_id())
            .await?;
        assert_eq!(
            crate::time::outstanding_deadline_timer(&history, &run_id),
            None,
            "re-encountering the own terminal completes the deadline cancellation: {history:#?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn process_exit_does_not_retire_a_timed_out_deadline()
    -> Result<(), Box<dyn std::error::Error>> {
        // Item 3×5 interaction: timeout teardown keeps its deadline live as its
        // resume anchor across fallible visibility work, and the process-exit
        // monitor it wakes via `cancel_pid` must NOT retire that TimedOut
        // deadline. A monitor that retired it would strand an interrupted
        // teardown.
        let active = active_workflow().await?;
        let run_id = active.handle.run_id().clone();
        let deadline_id = crate::time::deadline_timer_id(&run_id)?;
        {
            let recorder = active.handle.recorder();
            let mut recorder = recorder.lock().await;
            recorder
                .record_timer_started(chrono::Utc::now(), deadline_id.clone(), chrono::Utc::now())
                .await?;
            recorder
                .record_workflow_timed_out(chrono::Utc::now(), String::from("workflow"))
                .await?;
        }

        handle_process_exit_async(
            active.context.clone(),
            active.handle.clone(),
            Ok(WorkflowProcessOutcome::Completed(payload("late")?)),
        )
        .await?;

        let history = active
            .context
            .store
            .read_history(active.handle.workflow_id())
            .await?;
        assert!(
            crate::time::outstanding_deadline_timer(&history, &run_id).is_some(),
            "the monitor must leave a TimedOut deadline live for its owning teardown: {history:#?}"
        );
        Ok(())
    }

    /// The same exception, applied to the other write the teardown owns.
    ///
    /// The first cut of the retry gave the resume path a visibility upsert and
    /// carved out `TimedOut` for the DEADLINE only. That left this monitor
    /// writing one run's index concurrently with the very teardown that woke it,
    /// and re-reading history does not order two writers: read-H1 /
    /// teardown-writes-H2 / write-H1 leaves the row one step stale for good.
    ///
    /// This test's own control is `a_resumed_completion_appends_nothing_and_
    /// still_refreshes_visibility`, which drives the identical resume path with
    /// a `Completed` terminal and asserts the index IS written. Same fixture,
    /// same call, one difference — so the absence below is caused by the timeout
    /// exception and not by a monitor that never writes visibility at all.
    #[tokio::test]
    async fn process_exit_leaves_a_timed_out_runs_visibility_to_its_teardown()
    -> Result<(), Box<dyn std::error::Error>> {
        let active = active_workflow().await?;
        let run_id = active.handle.run_id().clone();
        let deadline_id = crate::time::deadline_timer_id(&run_id)?;
        {
            let recorder = active.handle.recorder();
            let mut recorder = recorder.lock().await;
            recorder
                .record_timer_started(chrono::Utc::now(), deadline_id.clone(), chrono::Utc::now())
                .await?;
            recorder
                .record_workflow_timed_out(chrono::Utc::now(), String::from("workflow"))
                .await?;
        }

        handle_process_exit_async(
            active.context.clone(),
            active.handle.clone(),
            Ok(WorkflowProcessOutcome::Completed(payload("late")?)),
        )
        .await?;

        let summaries = active
            .context
            .visibility_store
            .list_workflows(aion_store::visibility::ListWorkflowsFilter::default())
            .await?;
        assert!(
            summaries.iter().all(
                |summary| summary.workflow_id != *active.handle.workflow_id()
                    || summary.status != WorkflowStatus::TimedOut
            ),
            "the monitor must not project a timed-out run's terminal into the index — that write \
             belongs to the teardown it races: {summaries:#?}"
        );
        Ok(())
    }

    /// 🔴 A REOPENED RUN IS NOT APPENDED INTO, EVEN WHEN THIS MONITOR NEVER
    /// RECORDED THE TERMINAL THAT WAS REOPENED.
    ///
    /// The F11 guard originally asked "did *I* record the terminal"
    /// (`progress == Recorded`) — this monitor's own memory. That covers one
    /// branch of two. The run can be driven terminal by a path this monitor
    /// never took (an explicit cancel, a timeout teardown, a predecessor
    /// engine's writer) and then reopened, and on THAT branch the monitor still
    /// holds `NotRecorded`: the old guard let the append through, into a live
    /// reopened run.
    ///
    /// The fixture is exactly that branch. The terminal and the reopen are both
    /// written directly through the handle's recorder BEFORE the monitor is
    /// invoked, so as far as `handle_process_exit_async` is concerned no attempt
    /// of its own ever recorded anything — and the reopen's respawn is modelled
    /// the way `reopen::respawn_and_register` performs it, by registering a
    /// SECOND handle around a fresh pid under the same `(workflow, run)` key.
    /// The stale monitor still holds the first one. That difference is the
    /// entire discriminator, so the fixture asserts it before proceeding.
    ///
    /// 🔴 WHAT THE GUARD IS WORTH, STATED ACCURATELY. Without it the append is
    /// not silently duplicated — it is REFUSED. The monitor's recorder is
    /// sequence-stale (both events below are written by other writers, through
    /// their own recorders, exactly as production does) so the store rejects
    /// the append with `SequenceConflict`. That is fail-safe for the data and
    /// wrong for the operator: this codebase defines `SequenceConflict` as THE
    /// double-writer indicator, `classify_completion_failure` treats it as
    /// unretryable, and it surfaces as an `error!` line. The guard turns a
    /// false incident report about a benign reopen into a clean, logged stand
    /// down. So this test asserts BOTH halves: nothing appended, AND no error
    /// handed back.
    #[tokio::test]
    async fn a_reopened_run_is_not_appended_into_by_a_monitor_that_never_recorded()
    -> Result<(), Box<dyn std::error::Error>> {
        let active = active_workflow().await?;
        let run_id = active.handle.run_id().clone();
        // Terminal written by SOMEONE ELSE, as far as the monitor knows — so it
        // goes through that other writer's recorder, not the monitor's.
        terminal_by_another_writer(&active, payload("first")?).await?;
        // ...and superseded by an operator reopen, which `reopen::reopen` also
        // writes through a recorder of its own.
        reopen_by_its_own_recorder(&active, &run_id).await?;
        // The reopen respawned the run: a new process, a new handle, the same
        // key. `active.handle` is now the SUPERSEDED lease.
        let successor = respawned_lease(&active).await?;
        active.context.registry.insert(
            (active.handle.workflow_id().clone(), run_id.clone()),
            successor,
        )?;

        let completed_before = completed_count(&active, &run_id).await?;
        assert_eq!(
            completed_before, 1,
            "fixture control: the run must carry exactly one terminal before the monitor runs"
        );
        // Control on the OTHER half of the fixture: without an observably newer
        // lease in the registry the whole test would be measuring nothing,
        // because the guard would be reached for the wrong reason.
        assert_ne!(
            active
                .context
                .registry
                .get(active.handle.workflow_id(), &run_id)?
                .map(|current| current.pid()),
            Some(active.handle.pid()),
            "fixture control: the registry must hold a DIFFERENT lease than the monitor's, or \
             this test is not exercising a superseded monitor at all"
        );

        let outcome = handle_process_exit_async(
            active.context.clone(),
            active.handle.clone(),
            Ok(WorkflowProcessOutcome::Completed(payload("late")?)),
        )
        .await;

        assert!(
            outcome.is_ok(),
            "a superseded monitor tried to append into a reopened run, the store refused it on \
             sequence, and the operator was handed this codebase's double-writer indicator for \
             what is an ordinary reopen: {outcome:?}"
        );
        assert_eq!(
            completed_count(&active, &run_id).await?,
            1,
            "the monitor appended a second terminal into a run that had been reopened — it is no \
             longer this run's writer"
        );
        Ok(())
    }

    /// 🔴 A REOPENED RUN THAT FINISHES AGAIN STILL RECORDS ITS TERMINAL.
    ///
    /// The other half of the F11 guard, and the one that a history-only
    /// predicate cannot hold at the same time as the test above.
    ///
    /// `handle_process_exit_async` is entered identically by a superseded
    /// lease's stale exit callback and by the CURRENT lease's ordinary exit.
    /// "Was there ever a reopen in this run's segment" is true in both, so it
    /// cannot separate them — it can only choose which one to break. This is
    /// the lease that must be let through: the operator reopened the run, the
    /// new lease did its work, and it exited normally.
    ///
    /// If the guard fires here the run never reaches a terminal at all: it
    /// projects `Running` forever, `handle.completion().notify` is skipped so
    /// every `await_result` caller and every parked child-terminal watcher
    /// hangs for the epoch, and the function returns `Ok`, so nothing is logged
    /// at any level. A silent permanent zombie in a durable workflow engine.
    ///
    /// The fixture's discriminator is the registry: this handle is the one the
    /// registry holds for `(workflow, run)`, so it IS the current lease. The
    /// test above supersedes that entry with a second lease's handle; here it is
    /// left alone.
    #[tokio::test]
    async fn a_reopened_run_that_finishes_again_records_its_terminal()
    -> Result<(), Box<dyn std::error::Error>> {
        let active = active_workflow().await?;
        let run_id = active.handle.run_id().clone();
        {
            let recorder = active.handle.recorder();
            let mut recorder = recorder.lock().await;
            recorder
                .record_workflow_failed(chrono::Utc::now(), workflow_error("first attempt"))
                .await?;
            recorder
                .record_workflow_reopened(chrono::Utc::now(), run_id.clone(), Vec::new())
                .await?;
        }

        let history = active
            .context
            .store
            .read_history(active.handle.workflow_id())
            .await?;
        assert!(
            terminal_outcome_from_history(&history, &run_id).is_none(),
            "fixture control: the reopen must have superseded the first terminal, or this test \
             would pass without the monitor recording anything"
        );
        assert_eq!(
            active
                .context
                .registry
                .get(active.handle.workflow_id(), &run_id)?
                .map(|current| current.pid()),
            Some(active.handle.pid()),
            "fixture control: this handle must be the lease the registry currently holds — the \
             whole point of the case is that it has NOT been superseded"
        );

        handle_process_exit_async(
            active.context.clone(),
            active.handle.clone(),
            Ok(WorkflowProcessOutcome::Completed(payload("second")?)),
        )
        .await?;

        let history = active
            .context
            .store
            .read_history(active.handle.workflow_id())
            .await?;
        assert!(
            matches!(
                terminal_outcome_from_history(&history, &run_id),
                Some(TerminalOutcome::Completed(_))
            ),
            "the reopened run's own lease exited normally and its terminal was not recorded — the \
             run projects Running forever and every waiter on it is stranded: {:#?}",
            terminal_outcome_from_history(&history, &run_id)
        );
        Ok(())
    }

    /// 🔴 A MONITOR THAT ALREADY RECORDED ITS OWN TERMINAL STANDS DOWN INSIDE
    /// THE REOPEN WINDOW, WHERE THE REGISTRY CANNOT TELL IT ANYTHING.
    ///
    /// This is the branch lease identity is blind to, and the window is a real
    /// interval in `reopen`: the terminal-cached handle is removed from the
    /// registry, `WorkflowReopened` is appended, and only afterwards does
    /// `respawn_and_register` install the successor. A retry attempt landing
    /// between the append and the registration sees no terminal in history (the
    /// reopen superseded it) and no lease in the registry (it was removed), so
    /// the only thing that still knows this monitor is finished is its own
    /// `progress`.
    ///
    /// The fixture reproduces that interval exactly — terminal, reopen, registry
    /// entry removed — and asserts both halves of it before running, because a
    /// fixture that quietly left the handle registered would be measuring the
    /// lease disjunct instead and would pass for the wrong reason.
    ///
    /// `handle_process_exit_attempt` is called directly rather than through the
    /// retry loop because `progress` is exactly the loop's carried state and
    /// this test's subject is what an attempt does when handed it.
    #[tokio::test]
    async fn a_recorded_monitor_stands_down_inside_the_reopen_window()
    -> Result<(), Box<dyn std::error::Error>> {
        let active = active_workflow().await?;
        let run_id = active.handle.run_id().clone();
        {
            let recorder = active.handle.recorder();
            let mut recorder = recorder.lock().await;
            recorder
                .record_workflow_completed(chrono::Utc::now(), payload("recorded by me")?)
                .await?;
        }
        // The reopen is written by `reopen::reopen`'s OWN recorder, built at the
        // history head — which is what leaves this monitor's recorder stale by
        // one and its next append refusable. Sharing a recorder here would model
        // a machine that does not exist.
        reopen_by_its_own_recorder(&active, &run_id).await?;
        // The reopen has cleared the terminal-cached handle and has not yet
        // registered the respawned one.
        active
            .context
            .registry
            .remove(active.handle.workflow_id(), &run_id)?;

        assert_eq!(
            completed_count(&active, &run_id).await?,
            1,
            "fixture control: exactly one terminal before the attempt"
        );
        assert!(
            active
                .context
                .registry
                .get(active.handle.workflow_id(), &run_id)?
                .is_none(),
            "fixture control: the registry must be EMPTY for this key — with a lease present the \
             lease disjunct would carry the test and `progress` would prove nothing"
        );

        let intent = TerminalIntent {
            outcome: TerminalIntentOutcome::Completed(payload("second")?),
            exit_time: chrono::Utc::now(),
        };
        let mut progress = TerminalProgress::Recorded;
        let outcome = super::handle_process_exit_attempt(
            &active.context,
            &active.handle,
            &intent,
            &mut progress,
        )
        .await;

        // 🔴 THIS IS THE DECISIVE ASSERTION, AND THE COUNT BELOW IS THE
        // SECONDARY ONE. The fixture leaves this monitor's recorder stale by
        // design, so the store would REFUSE a second append on sequence: the
        // count can never reach 2 and an assertion on it alone would be
        // invariant to the very error this guard prevents. What the guard is
        // worth is that the refusal never happens — an ordinary reopen does not
        // hand the operator this codebase's double-writer indicator.
        assert!(
            outcome.is_ok(),
            "a monitor that had already recorded its terminal attempted an append inside the \
             reopen window, the store refused it on sequence, and the operator was handed a \
             `SequenceConflict` — the double-writer indicator — for what is an ordinary reopen: \
             {outcome:?}"
        );
        assert_eq!(
            completed_count(&active, &run_id).await?,
            1,
            "a monitor that had already recorded its terminal appended a second one into a run \
             mid-reopen — the registry could not see the successor yet, so nothing but its own \
             progress stood between it and a double write"
        );
        Ok(())
    }

    /// A recorder built the way every non-monitor writer in this engine builds
    /// one: fresh, at the history head as it stands right now.
    ///
    /// 🔴 THIS IS THE WHOLE POINT OF THE TWO HELPERS BELOW. A [`Recorder`] owns
    /// its own tracked sequence head and never re-reads the store after
    /// construction (`durability::recorder`), so writing a fixture's setup
    /// events through the monitor's own recorder leaves that recorder's head
    /// CURRENT — and the monitor's later append then succeeds for a reason
    /// production never offers it. In production the terminal and the reopen
    /// come from other writers with recorders of their own, which leaves the
    /// monitor's recorder stale and its append refusable on sequence. A fixture
    /// that shares one writer where production uses two is not a fixture, it is
    /// a different machine.
    async fn separate_recorder(
        active: &ActiveWorkflow,
    ) -> Result<Recorder, Box<dyn std::error::Error>> {
        let history = active
            .context
            .store
            .read_history(active.handle.workflow_id())
            .await?;
        let head = history.last().map(Event::seq).unwrap_or_default();
        Ok(Recorder::resume_at(
            active.handle.workflow_id().clone(),
            Arc::clone(&active.context.store),
            head,
        ))
    }

    /// Drive the run terminal from a writer that is not this monitor — an
    /// explicit cancel, a timeout teardown, a predecessor engine's recorder.
    ///
    /// The monitor's `progress` stays `NotRecorded`, because as far as it is
    /// concerned nothing of its own ever wrote.
    async fn terminal_by_another_writer(
        active: &ActiveWorkflow,
        result: Payload,
    ) -> Result<(), Box<dyn std::error::Error>> {
        separate_recorder(active)
            .await?
            .record_workflow_completed(chrono::Utc::now(), result)
            .await?;
        Ok(())
    }

    /// Append `WorkflowReopened` the way `reopen::reopen` does it: through a
    /// recorder built at the history head, carrying the run's visibility seam.
    async fn reopen_by_its_own_recorder(
        active: &ActiveWorkflow,
        run_id: &aion_core::RunId,
    ) -> Result<(), Box<dyn std::error::Error>> {
        separate_recorder(active)
            .await?
            .with_visibility(run_id.clone(), Arc::clone(&active.context.visibility_store))
            .record_workflow_reopened(chrono::Utc::now(), run_id.clone(), Vec::new())
            .await?;
        Ok(())
    }

    /// The handle a reopen's respawn registers: same `(workflow, run)` key, a
    /// different process.
    ///
    /// `reopen::respawn_and_register` spawns a fresh workflow process and builds
    /// a new [`WorkflowHandle`] around the pid the runtime hands back, then
    /// registers it under the run it is reviving. Everything except the pid and
    /// the recorder instance carries over, which is why the pid is the thing a
    /// stale monitor can be told apart by.
    ///
    /// 🔴 THE RECORDER IS BUILT AT THE REAL HEAD, NOT AT ZERO. `Recorder::new`
    /// starts its tracked sequence at 0 and a recorder never re-reads the store
    /// head after construction, so a fixture handing one out over a non-empty
    /// history is armed to produce a `SequenceConflict` on its FIRST append —
    /// the double-writer indicator — from a fixture defect rather than from the
    /// defect under test. No caller appends through this handle today; the trap
    /// is that the next one would, and would read the resulting conflict as a
    /// finding. Production does not have this shape: `respawn_and_register`
    /// resumes at the head it just wrote.
    async fn respawned_lease(
        active: &ActiveWorkflow,
    ) -> Result<WorkflowHandle, Box<dyn std::error::Error>> {
        Ok(WorkflowHandle::new(WorkflowHandleParts {
            workflow_id: active.handle.workflow_id().clone(),
            run_id: active.handle.run_id().clone(),
            pid: active.handle.pid() + 1,
            workflow_type: active.handle.workflow_type().to_owned(),
            namespace: active.handle.namespace().to_owned(),
            loaded_version: active.handle.loaded_version().clone(),
            cached_status: WorkflowStatus::Running,
            residency: HandleResidency::Resident,
            recorder: separate_recorder(active).await?,
            completion: CompletionNotifier::new(),
        }))
    }

    /// Count this run's `WorkflowCompleted` events in durable history.
    async fn completed_count(
        active: &ActiveWorkflow,
        run_id: &aion_core::RunId,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let history = active
            .context
            .store
            .read_history(active.handle.workflow_id())
            .await?;
        Ok(aion_core::run_segment(&history, run_id)
            .iter()
            .filter(|event| matches!(event, Event::WorkflowCompleted { .. }))
            .count())
    }

    #[tokio::test]
    async fn abnormal_exit_records_failed_reconciles_and_notifies()
    -> Result<(), Box<dyn std::error::Error>> {
        let active = active_workflow().await?;
        let error = workflow_error("process crashed: error");
        let mut early = active.handle.completion().subscribe();

        handle_process_exit_async(
            active.context.clone(),
            active.handle.clone(),
            Ok(WorkflowProcessOutcome::Failed(error.clone())),
        )
        .await?;
        early.changed().await?;

        assert_eq!(
            early.borrow().clone(),
            Some(TerminalOutcome::Failed(error.clone()))
        );
        assert_eq!(
            active.handle.completion().subscribe().borrow().clone(),
            Some(TerminalOutcome::Failed(error.clone()))
        );
        let registered = active
            .context
            .registry
            .get(active.handle.workflow_id(), active.handle.run_id())?
            .ok_or("missing registered handle")?;
        assert_eq!(registered.cached_status(), WorkflowStatus::Failed);
        assert_eq!(registered.residency(), HandleResidency::Suspended);
        let history = active
            .context
            .store
            .read_history(active.handle.workflow_id())
            .await?;
        match history.as_slice() {
            [
                Event::WorkflowStarted { .. },
                Event::WorkflowFailed {
                    error: recorded, ..
                },
            ] => {
                assert_eq!(recorded, &error);
            }
            other => return Err(format!("expected started then failed, found {other:?}").into()),
        }
        Ok(())
    }

    /// How long a retry proof waits before calling the terminal lost.
    ///
    /// Generous against the 1–5 ms cadence above so the assertion is about the
    /// retry landing, never about scheduler luck on a loaded box.
    const RETRY_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

    /// Wait for the terminal, observing GROUND TRUTH rather than the faulted
    /// interface.
    ///
    /// The first cut of this helper polled through the `FlakyStore` itself, so
    /// the test's own observation drew from the same injected failure budget as
    /// the code under test — and the test failed reporting the injected error,
    /// which looked exactly like the fix not working. An oracle that can be
    /// refused by the treatment is measuring the instrument.
    async fn await_terminal(
        store: &Arc<crate::store_faults::FlakyStore>,
        workflow_id: &aion_core::WorkflowId,
    ) -> Result<Vec<Event>, Box<dyn std::error::Error>> {
        let deadline = std::time::Instant::now() + RETRY_DEADLINE;
        loop {
            let history = store.recorded_history(workflow_id).await?;
            if history
                .iter()
                .any(|event| matches!(event, Event::WorkflowCompleted { .. }))
            {
                return Ok(history);
            }
            if std::time::Instant::now() >= deadline {
                return Err(format!(
                    "the terminal never landed within {RETRY_DEADLINE:?}: {history:#?}"
                )
                .into());
            }
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
    }

    /// A1, on the exact line the defect was read at.
    ///
    /// `completion.rs` reads history with `?` before it decides anything. A
    /// transient failure there used to propagate to the monitor callback, which
    /// logged and returned — and since the exit record is retired
    /// unconditionally, nothing re-armed. The run then projected `Running`
    /// forever despite having finished.
    ///
    /// Failing READS specifically matters: the append-only fixture this test
    /// grew from could not reach this line at all.
    #[tokio::test]
    async fn a_transient_history_read_failure_still_lands_the_terminal()
    -> Result<(), Box<dyn std::error::Error>> {
        let (active, flaky) = flaky_workflow().await?;
        let result = payload("result")?;
        flaky.fail_next_reads(3);

        // The call itself must SUCCEED: the failure is handled from here, not
        // swallowed. Returning `Err` would put us back at the swallowing log.
        handle_process_exit_async(
            active.context.clone(),
            active.handle.clone(),
            Ok(WorkflowProcessOutcome::Completed(result.clone())),
        )
        .await?;

        let history = await_terminal(&flaky, active.handle.workflow_id()).await?;
        let terminals = history
            .iter()
            .filter(|event| matches!(event, Event::WorkflowCompleted { .. }))
            .count();
        assert_eq!(
            terminals, 1,
            "exactly one terminal, never a double: {history:#?}"
        );
        Ok(())
    }

    /// A1, write side: the same guarantee when the APPEND is what fails.
    #[tokio::test]
    async fn a_transient_append_failure_still_lands_the_terminal()
    -> Result<(), Box<dyn std::error::Error>> {
        let (active, flaky) = flaky_workflow().await?;
        let result = payload("result")?;
        flaky.fail_next_appends(3);

        handle_process_exit_async(
            active.context.clone(),
            active.handle.clone(),
            Ok(WorkflowProcessOutcome::Completed(result.clone())),
        )
        .await?;

        let history = await_terminal(&flaky, active.handle.workflow_id()).await?;
        match history
            .iter()
            .filter(|event| matches!(event, Event::WorkflowCompleted { .. }))
            .collect::<Vec<_>>()
            .as_slice()
        {
            [
                Event::WorkflowCompleted {
                    result: recorded, ..
                },
            ] => {
                assert_eq!(recorded, &result, "the retry records the ORIGINAL outcome");
            }
            other => return Err(format!("expected exactly one completion, found {other:?}").into()),
        }
        Ok(())
    }

    /// A2: the call returns while the retry is still outstanding.
    ///
    /// Why that is the property worth pinning: every process-exit callback on a
    /// node is drained by ONE thread, and `handle_process_exit` is `block_on`-ed
    /// on it. A backoff loop running inline would hold that thread for as long
    /// as the store kept failing, promoting a one-run wedge to a node-wide one.
    /// Returning with the work still armed elsewhere is what makes the wedge
    /// impossible.
    ///
    /// **What this test observes, and what it deliberately no longer claims.**
    /// It calls the async function directly, so the real single-threaded
    /// dispatcher is never on the path; the wedge above is the *motivation*, not
    /// the measurement. What is measured is decisive on its own:
    /// `armed_completion_retry_count() == 1` at the moment the call has already
    /// returned — the work outlived the call. Awaiting the retry inline instead
    /// of arming it kills this test (control A2), and the run time moves
    /// 0.07s → 3.36s.
    ///
    /// 🔴 This test used to complete a SECOND workflow afterwards and assert it
    /// reached `Completed`, under the name "does not hold up another workflow's
    /// completion". That second engine shared nothing with the first — its own
    /// `RuntimeHandle`, thread, `Registry` and store — so it could not have been
    /// held up by the first no matter how the retry behaved, and the assertion
    /// was green by construction. It is deleted rather than re-worded: a healthy
    /// exit reaching `Completed` is already pinned by
    /// `normal_exit_records_completed_reconciles_and_notifies`, so what it added
    /// was the appearance of coverage, and nothing else.
    #[tokio::test]
    async fn a_retrying_completion_returns_with_the_retry_still_armed()
    -> Result<(), Box<dyn std::error::Error>> {
        let (stalled, flaky) = flaky_workflow().await?;
        // Enough refusals that the retry is certainly still in flight below.
        flaky.fail_next_reads(200);
        handle_process_exit_async(
            stalled.context.clone(),
            stalled.handle.clone(),
            Ok(WorkflowProcessOutcome::Completed(payload("stalled")?)),
        )
        .await?;
        assert_eq!(
            stalled
                .context
                .runtime
                .engine_tasks()
                .armed_completion_retry_count(),
            1,
            "the call returned while the run was still retrying: the loop is armed elsewhere, \
             not run inline on the caller"
        );
        Ok(())
    }

    /// A4: a retry that finds its own terminal appends nothing — and still
    /// refreshes visibility.
    ///
    /// This is the gap the retry EXPOSED. The pre-existing-terminal branch used
    /// to skip the visibility upsert, which was correct while the only way to
    /// reach it was another writer having done that work. Under retry the
    /// "other writer" can be this operation's own earlier attempt, which may
    /// have appended the terminal and died before upserting — so skipping would
    /// trade a wedged status for a permanently stale index.
    #[tokio::test]
    async fn a_resumed_completion_appends_nothing_and_still_refreshes_visibility()
    -> Result<(), Box<dyn std::error::Error>> {
        let active = active_workflow().await?;
        let result = payload("result")?;
        // Stand in for an attempt that appended the terminal and then died
        // before it could upsert visibility.
        {
            let recorder = active.handle.recorder();
            let mut recorder = recorder.lock().await;
            recorder
                .record_workflow_completed(chrono::Utc::now(), result.clone())
                .await?;
        }
        let before = active
            .context
            .store
            .read_history(active.handle.workflow_id())
            .await?;
        assert!(
            active
                .context
                .visibility_store
                .list_workflows(aion_store::visibility::ListWorkflowsFilter::default())
                .await?
                .iter()
                .all(|summary| summary.status != WorkflowStatus::Completed),
            "the index must NOT already show the terminal, or this proves nothing"
        );

        handle_process_exit_async(
            active.context.clone(),
            active.handle.clone(),
            Ok(WorkflowProcessOutcome::Completed(result)),
        )
        .await?;

        let after = active
            .context
            .store
            .read_history(active.handle.workflow_id())
            .await?;
        assert_eq!(
            before.len(),
            after.len(),
            "a resumed completion must append nothing: {after:#?}"
        );
        let summaries = active
            .context
            .visibility_store
            .list_workflows(aion_store::visibility::ListWorkflowsFilter::default())
            .await?;
        assert!(
            summaries.iter().any(
                |summary| summary.workflow_id == *active.handle.workflow_id()
                    && summary.status == WorkflowStatus::Completed
            ),
            "the resume path refreshes the index it used to skip: {summaries:#?}"
        );
        Ok(())
    }

    /// One monitor lease can never hold two completion retries.
    ///
    /// Arming twice for one lease is refused rather than racing two writers of
    /// the same terminal. Every arm below is for the SAME handle — one workflow,
    /// one run, one pid — so they all land on one [`CompletionRetryKey`] and the
    /// refusal is the property under test.
    ///
    /// 🔴 THIS DOC USED TO SAY "keyed by the run, so one RUN cannot hold two",
    /// and justified it with "a reopen REUSES the run id, so it arrives at the
    /// same entry as the exit that is still retrying against it". **That is
    /// retracted, and it is not a wording slip — it is a verbatim statement of
    /// the silent permanent zombie this lane was blocked on.** Arriving at the
    /// same entry is precisely what made a reopened run's successor lease be
    /// refused by its superseded predecessor, which then stood down without
    /// writing; the run projected `Running` for the epoch with nothing above
    /// `debug!` to show for it.
    ///
    /// The key carries the monitor pid now, so a reopen arrives at a DIFFERENT
    /// entry — and that is asserted, from the other side, by
    /// `a_reopened_runs_successor_lease_can_arm_its_own_completion_retry` in
    /// `runtime/engine_tasks.rs`, which requires the count to reach 2 for one
    /// run. Read the two together: this test says a lease cannot double-arm,
    /// that one says two leases of one run must be able to arm separately.
    /// Anyone re-reading this doc as licence to key by the run alone would
    /// reintroduce the zombie, and that sibling test is what would stop them.
    ///
    /// 🔴 THE COUNT IS CHECKED AFTER EVERY ARM, NOT ONLY THE LAST ONE, AND THAT
    /// IS THE WHOLE POINT OF THIS TEST. Its first form asserted `== 1` once, at
    /// the end of twenty-five arms. A defect then live made every arm past the
    /// first DELETE the registry entry belonging to the retry that was still
    /// running: `arm_completion_retry` takes the task by value and drops it
    /// un-polled when it refuses, and the drop guard — built at future
    /// construction, removing by key with no identity check — evicted the live
    /// entry. So odd arms inserted, even arms deleted, the count alternated
    /// 1,0,1,0…, and the twenty-fifth landed on 1. Evicting the entry only
    /// drops the `JoinHandle`, which DETACHES rather than cancels, so the
    /// assertion passed with thirteen live retries all holding the same run's
    /// terminal. A verdict invariant to the error it was written to catch is
    /// not a verdict.
    ///
    /// An even bound would have caught it and an odd one would not, which is
    /// exactly the kind of accident a test must not depend on. Checking every
    /// arm removes the parity from the question: no schedule of inserts and
    /// deletes can hide behind where the loop happens to stop.
    ///
    /// 🔴 BOTH HALVES OF THAT ARE MEASURED, NOT ARGUED. The pre-fix slot was
    /// restored verbatim (unconditional construction-time guard plus key-only
    /// removal) and both forms of this test were run against it: the old
    /// single-trailing-assertion form passed, `1 passed; 0 failed`; this form
    /// failed. Same defect, same fixture, opposite verdicts — which is what
    /// makes the rewrite load-bearing rather than cosmetic. Reverting either
    /// half of the fix ALONE leaves this test green, because `claim` itself is
    /// part of the fix and `tokio::task::try_id` returns `None` on the caller's
    /// thread (`handle_process_exit` is `block_on`-ed on a dedicated thread, not
    /// a spawned task), so a construction-time `claim` declines and removes
    /// nothing. The two halves are only jointly sufficient to reproduce it, and
    /// the identity check has its own separate pin in `engine_tasks.rs`
    /// (`a_release_from_a_foreign_task_cannot_evict_a_live_completion_retry`).
    #[tokio::test]
    async fn one_monitor_lease_can_never_hold_more_than_one_completion_retry()
    -> Result<(), Box<dyn std::error::Error>> {
        let (active, flaky) = flaky_workflow().await?;
        let tasks = active.context.runtime.engine_tasks();
        assert_eq!(
            tasks.armed_completion_retry_count(),
            0,
            "the control: nothing is armed for this run before the first exit, so a later count \
             of one is this test's doing and not the fixture's"
        );
        flaky.fail_next_reads(500);
        for arm in 1..=25_u32 {
            handle_process_exit_async(
                active.context.clone(),
                active.handle.clone(),
                Ok(WorkflowProcessOutcome::Completed(payload("result")?)),
            )
            .await?;
            assert_eq!(
                tasks.armed_completion_retry_count(),
                1,
                "after arm {arm} of 25 the run holds exactly one retry; a count of 0 here means a \
                 refused arm evicted the live retry's registration, and anything above 1 means two \
                 writers of one run's terminal"
            );
        }
        Ok(())
    }

    /// The runtime's own shutdown closes the completion-retry epoch.
    ///
    /// A completion retry appends TERMINAL events. If `RuntimeHandle::shutdown`
    /// returned with one still running, it could append against a store a
    /// successor engine is already recovering — two writers for one history,
    /// which invariant 3 exists to forbid.
    ///
    /// This pin is here because the property had none. The epoch close was
    /// reachable in production only through the *optional* child NIF bridge, so
    /// an engine that never installed one shut down with retries still live —
    /// and the whole suite stayed green with the close deleted. That is the
    /// failure the brief was written to prevent, and prose in the brief did not
    /// prevent it; only this test does.
    ///
    /// 🔴 Scope, stated because the earlier wording overclaimed it. What this
    /// observes is that shutting the runtime down leaves the epoch EMPTY — which
    /// `gate_and_abort`'s `retain` produces before the join runs, so deleting
    /// the join would leave this test green. It is not evidence of awaiting.
    /// The await is pinned separately and properly, by
    /// `shutdown_gates_new_arms_and_awaits_aborted_tasks` in `engine_tasks.rs`,
    /// which holds a `DropFlag` that can only be observed set if the aborted
    /// task was actually joined. Two properties, two tests; this one owns
    /// reachability of the close, that one owns the await.
    #[tokio::test]
    async fn runtime_shutdown_closes_the_completion_retry_epoch()
    -> Result<(), Box<dyn std::error::Error>> {
        let (active, flaky) = flaky_workflow().await?;
        flaky.fail_next_reads(500);
        handle_process_exit_async(
            active.context.clone(),
            active.handle.clone(),
            Ok(WorkflowProcessOutcome::Completed(payload("result")?)),
        )
        .await?;
        // Held across the shutdown on purpose: reading the count through
        // `context.runtime` afterwards would work too, but taking the handle
        // first proves the executor itself was emptied rather than merely
        // becoming unreachable.
        let tasks = active.context.runtime.engine_tasks();
        assert_eq!(
            tasks.armed_completion_retry_count(),
            1,
            "the fixture must actually have a retry armed, or the assert below is vacuous"
        );

        active.context.runtime.shutdown()?;

        assert_eq!(
            tasks.armed_completion_retry_count(),
            0,
            "shutting the runtime down must leave the completion-retry epoch empty, so no \
             armed retry can append a terminal after this point"
        );
        Ok(())
    }

    /// The ladder the operator configured is the ladder the retry actually
    /// sleeps on.
    ///
    /// Every other test in this module supplies 1 ms / 5 ms through
    /// `RuntimeConfig`, which LOOKS like wiring evidence and is not: the
    /// inherited default is 1 ms / 8 ms, so a retry that ignored the config
    /// entirely and slept on the default would land inside the same few
    /// milliseconds and leave all of them green. The knob shipped with no test
    /// that could tell the two apart.
    ///
    /// This one separates them, and the separation is against the default FIRST
    /// SLEEP — which is the only inherited value a single-refusal fixture can
    /// ever reach.
    ///
    /// 🔴 THE GUARD BELOW USED TO DEMAND THE CONFIGURED SLEEP EXCEED THE ENTIRE
    /// DEFAULT LADDER, AND IT WENT RED WHEN THE CEILING WAS RULED UP TO 30 s
    /// (2026-08-06; see `CompletionRetryConfig::default`). It went red honestly
    /// — it did what its own message told the next reader to expect — but the
    /// condition it stated was two claims collapsed into one inequality that was
    /// cheap to satisfy while the ceiling was 8 ms. Separated, they are:
    ///
    /// - **The floor must be distinguishable.** Exactly one read is refused
    ///   below, so the retry takes exactly ONE sleep, and `sleep_backoff` sleeps
    ///   the CURRENT value before it doubles. The only inherited constant that
    ///   sleep can be is `initial_backoff`, so that is what the configured value
    ///   must tower over. The ceiling never enters a single-sleep run.
    /// - **The terminal must still land.** That is the second assertion, and it
    ///   is what rules out the other direction: a retry sleeping on some larger
    ///   inherited value — the 30 s ceiling, say — would satisfy "not yet
    ///   landed" and then never land inside `await_terminal`'s wait.
    ///
    /// Together those two say the retry slept for what it was told: neither
    /// less, nor more. Restating the guard as "exceeds the whole ladder" would
    /// now force a >30 s test to prove a property one sleep already decides.
    ///
    /// The negative half is the decisive half, and it is safe in the direction
    /// that matters: a sleep is a FLOOR, so load or a slow box can only ever
    /// delay the landing further. The only way "not yet landed" fails is the
    /// retry sleeping for less than it was told to, which is exactly the defect.
    #[tokio::test]
    async fn the_configured_backoff_ladder_governs_when_a_retry_lands()
    -> Result<(), Box<dyn std::error::Error>> {
        // Flat ladder: first sleep and ceiling are the same, so the single
        // retry below waits exactly this long and the assertion does not depend
        // on how the ladder climbs.
        let configured = std::time::Duration::from_millis(500);
        let ladder = crate::runtime::CompletionRetryConfig::try_new(configured, configured)?;
        // Derived from the interval under test, not chosen: a fifth of it is far
        // enough below the configured sleep to be robust.
        let observation_window = configured / 5;
        assert!(
            observation_window
                > crate::runtime::CompletionRetryConfig::default().initial_backoff() * 10,
            "this test is only decisive while the window it watches is far wider than the sleep an \
             INHERITED first backoff would take — otherwise 'not yet landed' is satisfied by a \
             retry that ignored the config entirely; if the default first backoff moves, move \
             this value"
        );

        let (active, flaky) = flaky_workflow_with_retry(ladder).await?;
        // Exactly one refusal: attempt 1 fails its history read, the retry
        // sleeps once, attempt 2 succeeds.
        flaky.fail_next_reads(1);

        handle_process_exit_async(
            active.context.clone(),
            active.handle.clone(),
            Ok(WorkflowProcessOutcome::Completed(payload("result")?)),
        )
        .await?;
        assert_eq!(
            active
                .context
                .runtime
                .engine_tasks()
                .armed_completion_retry_count(),
            1,
            "the first attempt must have failed and armed a retry, or both assertions below are \
             vacuous"
        );

        tokio::time::sleep(observation_window).await;
        let early = flaky.recorded_history(active.handle.workflow_id()).await?;
        assert!(
            !early
                .iter()
                .any(|event| matches!(event, Event::WorkflowCompleted { .. })),
            "a retry told to wait {configured:?} had already appended its terminal after \
             {observation_window:?} — it is sleeping on something other than the configured \
             ladder: {early:#?}"
        );

        let history = await_terminal(&flaky, active.handle.workflow_id()).await?;
        assert_eq!(
            history
                .iter()
                .filter(|event| matches!(event, Event::WorkflowCompleted { .. }))
                .count(),
            1,
            "and once the configured interval elapses it lands, exactly once: {history:#?}"
        );
        Ok(())
    }

    /// 🔴 THE PRICE OF AN UNBOUNDED RETRY IS LOUDNESS, AND THIS IS THE ONLY
    /// PLACE THAT PRICE IS COLLECTED.
    ///
    /// Ruled 2026-08-06 (`docs/design/aion-authoring/
    /// RULING-COMPLETION-RETRY-BOUND-2026-08-06.md`, obligation O5): the loop
    /// has no attempt budget on purpose — a completion has no re-driver, so
    /// giving up would turn a transient store outage into permanent silent data
    /// loss — and the ceiling was raised to 30 s. Both decisions make the loop
    /// QUIETER, and a loop ruled unbounded must never also be invisible.
    ///
    /// The subject is the `at_ceiling` half of the emission condition, because
    /// the `first_sight` half alone is not loudness. [`FaultReporter`] states a
    /// fault when it is new and then on a doubling attempt ladder — 1, 2, 4, 8 —
    /// so against a single persistent fault it is SILENT on attempts 3, 5, 6, 7,
    /// 9…, and at a 30 s ceiling that silence is minutes long and grows without
    /// bound. The assertion below is therefore contiguity: at the ceiling the
    /// stated attempts are 1..=N with nothing skipped, which the ladder alone
    /// cannot produce past attempt 2.
    ///
    /// 🔴 THIS CALLS THE LOOP DIRECTLY RATHER THAN THROUGH
    /// `handle_process_exit_async`, AND THAT IS FORCED, NOT PREFERRED. The
    /// capture is thread-scoped on purpose (see `crate::log_capture`), and
    /// production arms the retry on the engine-task executor's own thread, where
    /// a thread-scoped subscriber cannot see it. A CAPTURING global subscriber
    /// would let every other unit test running in parallel write into this log —
    /// and let this test's assertions be satisfied by an emission it did not
    /// cause. What is skipped is the spawn; the loop is the same loop.
    ///
    /// (`log_capture` does install one global subscriber, and it is not that: it
    /// captures nothing and decides nothing. Its whole job is to keep the
    /// callsite-interest registry non-empty, because this test was silently
    /// capturing NOTHING from `completion_retry` in 5 of 5 runs of
    /// `cargo test -p aion-rs --lib -- lifecycle::completion` on 2026-08-06 —
    /// green alone, green at `--test-threads=1`, red beside its own siblings.
    /// The reason lives in one place, `crate::log_capture`'s header, and is
    /// pointed at rather than restated here.)
    ///
    /// 🔴 THE ONE ARGUMENT THAT COULD SILENTLY DIFFER IS READ FROM THE SAME
    /// EXPRESSION PRODUCTION READS. `arm_completion_retry` takes the ladder from
    /// `context.runtime.completion_retry()`, so this does too, rather than
    /// handing the loop the local it built the fixture with — a test that names
    /// the ladder twice can pass while the engine runs on a different one, which
    /// is the failure mode the whole ruling exists to prevent. An earlier
    /// revision of this paragraph claimed ALL the arguments matched
    /// `arm_completion_retry`; that was held by nothing but care. The rest are
    /// this exit's own — the handle, the terminal intent, the progress — and a
    /// test that supplies its own exit is what a unit test IS.
    #[tokio::test]
    async fn at_the_ceiling_every_failing_attempt_states_itself()
    -> Result<(), Box<dyn std::error::Error>> {
        // Flat ladder: the first sleep IS the ceiling, so `at_ceiling` holds
        // from the first attempt and this observes the emission rule rather
        // than how the ladder climbs.
        let interval = std::time::Duration::from_millis(1);
        let ladder = crate::runtime::CompletionRetryConfig::try_new(interval, interval)?;
        let (active, flaky) = flaky_workflow_with_retry(ladder).await?;
        // The budget IS the expected attempt count, because `fail_next_reads`
        // refuses from the FIRST read, so every attempt here dies on
        // `handle_process_exit_attempt`'s own history read and consumes exactly
        // one. That is asserted below rather than assumed: if an attempt ever
        // consumed two, the count would come up short and this test goes red
        // saying so. 🔴 The scope is the fail-fast path ONLY. An attempt that
        // gets past the append spends up to SIX reads on the continue-as-new
        // branch (enumerated in `CompletionRetryConfig::default`), and this
        // fixture can never reach that path — `fail_reads_after` is the shape
        // that can.
        let failing_reads: u32 = 64;
        flaky.fail_next_reads(failing_reads);

        let intent =
            TerminalIntent::from_outcome(Ok(WorkflowProcessOutcome::Completed(payload("result")?)));
        let retry_context = CompletionRetryContext::downgrade(active.context.clone());

        let (captured, subscriber) = crate::log_capture::LogCapture::new()?;
        {
            // `set_default` rather than `with_default`: the guard is held across
            // `await` points. Sound only because `#[tokio::test]` is
            // single-threaded — a multi-threaded flavour could move the loop to
            // a thread this subscriber was never installed on, and the capture
            // would silently empty.
            let _installed = tracing::subscriber::set_default(subscriber);
            retry_process_exit(
                &retry_context,
                &active.handle,
                &intent,
                // NOT the local `ladder`: the same read `arm_completion_retry`
                // performs, so the loop under test cannot be run on a ladder the
                // engine is not configured with.
                active.context.runtime.completion_retry(),
                TerminalProgress::NotRecorded,
            )
            .await;
        }

        let warnings = captured.at_level("WARN")?;
        // Selected by the message rather than by any field this fix introduced:
        // a mutation that drops the added fields must be caught by the field
        // assertions below, not silently excluded from the sample first.
        let stated: Vec<&crate::log_capture::CapturedEvent> = warnings
            .iter()
            .filter(|event| event.mentions("retrying with backoff"))
            .collect();
        let attempts: Vec<u64> = stated
            .iter()
            .filter_map(|event| event.field("attempts")?.parse().ok())
            .collect();
        // Read for the vacuity control's message only — the discriminator
        // between "the loop stopped early" and "this thread captured nothing".
        let total_captured = captured.events()?.len();
        let warning_count = warnings.len();

        // 🔴 THE VACUITY CONTROL, AND ITS MESSAGE MUST NOT NAME A CAUSE IT
        // CANNOT DISTINGUISH. An earlier revision said "fewer than three
        // attempts failed, so no ladder-silent attempt was ever reached" — which
        // blames the emission rule. An empty `stated` has two causes that this
        // assertion cannot tell apart: the loop really did stop early, or the
        // loop ran and the CAPTURE lost it (the `tracing` callsite-interest
        // defect `crate::log_capture` was rewritten to close). Reassigning blame
        // to the first sent an earlier investigation to the wrong file for a
        // day. The message now names both and points at the discriminator —
        // `stated` empty while other WARNs were captured means the loop; every
        // level empty means the capture.
        assert!(
            attempts.len() >= 3,
            "fewer than three failing attempts SPOKE, so every assertion below would be vacuous. \
             Two causes, and this assertion cannot separate them: the retry loop stopped early, \
             or this thread's capture never saw the emissions. Discriminate on the totals — \
             {total_captured} events captured at any level, {warning_count} at WARN. Both zero \
             points at the capture (see `crate::log_capture`), not at the loop. Stated: {stated:?}"
        );
        assert_eq!(
            attempts,
            (1..=attempts.len() as u64).collect::<Vec<u64>>(),
            "at the ceiling the stated attempts must be contiguous; a gap is the attempt ladder \
             speaking alone, which is the silence this obligation exists to close: {stated:?}"
        );
        // 🔴 CONTIGUITY ALONE CANNOT SEE A TRUNCATION. `[1,2,3]` is contiguous
        // whether the loop failed three times or sixty-four and stopped speaking
        // after the third, and "every failing attempt states itself" is exactly
        // the claim a truncation breaks. The count closes it, and it is DERIVED
        // from the outage budget rather than chosen: on THIS fixture's fail-fast
        // path each failing attempt spends one history read, so a budget of N
        // failing reads is N failing attempts.
        assert_eq!(
            attempts.len(),
            failing_reads as usize,
            "the outage was {failing_reads} failing reads and every attempt on this fixture's \
             fail-fast path spends exactly one, so {failing_reads} attempts must have spoken; a \
             shorter run means either the ladder fell silent partway or an attempt spent more \
             than one read, and both are findings: {stated:?}"
        );
        for event in &stated {
            assert_eq!(
                event.field("at_ceiling"),
                Some("true"),
                "an at-ceiling statement must say so: {event}"
            );
            for field in ["elapsed_seconds", "fault"] {
                assert!(
                    event.field(field).is_some(),
                    "an at-ceiling statement must carry {field}, or it tells the operator the \
                     retry is still running without telling them how long or against what: \
                     {event}"
                );
            }
        }
        // The outage was survived, not merely narrated: the terminal is durable.
        let history = flaky.recorded_history(active.handle.workflow_id()).await?;
        assert!(
            history
                .iter()
                .any(|event| matches!(event, Event::WorkflowCompleted { .. })),
            "the loop must have landed the terminal once the budget drained: {history:#?}"
        );
        Ok(())
    }

    /// A sequence conflict is reported, never retried.
    ///
    /// `SequenceConflict` is the store contract's double-writer indicator
    /// (CLAUDE.md invariant 3). The retry loop has no attempt budget, so
    /// classifying it as transient would spin a second writer against a history
    /// another writer already owns — forever, at the backoff ceiling, while the
    /// operator is told the completion was handled. Two things must hold: the
    /// error reaches the caller intact, and nothing is left owning the terminal.
    ///
    /// This is the pin for the whole per-variant classification. Matching the
    /// `Store` family as a unit passes every other test in this module and fails
    /// only here.
    #[tokio::test]
    async fn a_sequence_conflict_is_reported_not_retried() -> Result<(), Box<dyn std::error::Error>>
    {
        let (active, flaky) = flaky_workflow().await?;
        flaky.fail_next_reads_with_conflict(1);

        let error = handle_process_exit_async(
            active.context.clone(),
            active.handle.clone(),
            Ok(WorkflowProcessOutcome::Completed(payload("result")?)),
        )
        .await
        .err()
        .ok_or("a conflict must not be reported as handled")?;

        let store_error = match &error {
            crate::EngineError::Store(store)
            | crate::EngineError::Durability(crate::durability::DurabilityError::Store(store)) => {
                store
            }
            other => return Err(format!("expected a store fault, found {other:?}").into()),
        };
        assert!(
            matches!(store_error, aion_store::StoreError::SequenceConflict { .. }),
            "the double-writer indicator must reach the caller intact: {store_error:?}"
        );
        assert_eq!(
            active
                .context
                .runtime
                .engine_tasks()
                .armed_completion_retry_count(),
            0,
            "nothing may be left retrying a conflict"
        );
        Ok(())
    }

    /// A lost-ownership refusal is reported, never retried.
    ///
    /// `NotOwner` reads like a transient condition and is not one for THIS loop.
    /// The store contract's own doc says the caller should re-resolve the
    /// shard's owner and retry or forward — re-resolution is the load-bearing
    /// half, and this loop has no such step. Sleeping and re-reading is not that
    /// remedy: it is one node hammering a shard another node now owns, at the
    /// backoff ceiling, forever, because the retry has no attempt budget.
    ///
    /// Same shape as the sequence-conflict pin and for the same reason: matching
    /// the `Store` family as one unit, or defaulting unrecognised store errors to
    /// transient, passes every other test in this module and fails here.
    #[tokio::test]
    async fn a_lost_ownership_refusal_is_reported_not_retried()
    -> Result<(), Box<dyn std::error::Error>> {
        let (active, flaky) = flaky_workflow().await?;
        flaky.fail_next_reads_with_lost_ownership(1);

        let error = handle_process_exit_async(
            active.context.clone(),
            active.handle.clone(),
            Ok(WorkflowProcessOutcome::Completed(payload("result")?)),
        )
        .await
        .err()
        .ok_or("a lost shard must not be reported as handled")?;

        let store_error = match &error {
            crate::EngineError::Store(store)
            | crate::EngineError::Durability(crate::durability::DurabilityError::Store(store)) => {
                store
            }
            other => return Err(format!("expected a store fault, found {other:?}").into()),
        };
        assert!(
            matches!(store_error, aion_store::StoreError::NotOwner { .. }),
            "the ownership refusal must reach the caller intact: {store_error:?}"
        );
        assert_eq!(
            active
                .context
                .runtime
                .engine_tasks()
                .armed_completion_retry_count(),
            0,
            "nothing may be left spinning against a shard this node has lost"
        );
        Ok(())
    }

    /// Once the epoch is closed, an attempt refuses to append — at the boundary,
    /// not before it.
    ///
    /// This is the single-writer hazard the whole retry design turns on. A retry
    /// upgrades its weak runtime reference and then holds it strongly for the
    /// entire attempt, so the strong handle that keeps `EngineTaskRuntime::drop`
    /// from running is held for exactly the span of the append it would need to
    /// stop. A check anywhere earlier in the attempt can be walked past by
    /// everything that follows it.
    ///
    /// The second assertion is the one that distinguishes a refusal from a
    /// crash: nothing may be written. A test that only checked the error type
    /// would pass against an implementation that appended and then reported.
    #[tokio::test]
    async fn an_attempt_refuses_to_append_once_the_epoch_is_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let (active, flaky) = flaky_workflow().await?;
        let before = flaky.recorded_history(active.handle.workflow_id()).await?;

        // Exactly what `Drop for Engine` does: gate the epoch without awaiting.
        active.context.runtime.engine_tasks().begin_close();

        let error = handle_process_exit_async(
            active.context.clone(),
            active.handle.clone(),
            Ok(WorkflowProcessOutcome::Completed(payload("result")?)),
        )
        .await
        .err()
        .ok_or("an append after the epoch closed must not be reported as handled")?;
        assert!(
            matches!(error, crate::EngineError::EngineTaskEpochClosed { .. }),
            "the refusal must name itself, so an operator is not left reading a store fault \
             for an engine-lifetime decision: {error:?}"
        );

        let after = flaky.recorded_history(active.handle.workflow_id()).await?;
        assert_eq!(
            before.len(),
            after.len(),
            "a refused attempt must append NOTHING: {after:#?}"
        );
        assert_eq!(
            active
                .context
                .runtime
                .engine_tasks()
                .armed_completion_retry_count(),
            0,
            "and it must not arm a retry that would re-attempt the same append"
        );
        Ok(())
    }

    /// 🔴 THE THIRD GATE SITE, AND THE ONE WITH THE HEAVIEST ACTION BEHIND IT.
    ///
    /// `refuse_if_epoch_closed` has three callers. Two are covered by the tests
    /// above and below, and both of those refuse an APPEND.
    /// `start_continuation_replacement` is the third, and its own comment states
    /// the stake: it "does not merely append `WorkflowStarted`, it SPAWNS A BEAM
    /// PROCESS and installs a fresh monitor", handing a successor engine a run
    /// this dying process is still executing. That is CLAUDE.md invariant 3 —
    /// exactly one writer per workflow — and it was the only one of the three
    /// with nothing exercising it.
    ///
    /// 🔴 THIS CALLS THE FUNCTION DIRECTLY, AND THAT IS FORCED. Driving it
    /// through `handle_process_exit_async` cannot reach this gate with the epoch
    /// closed: the gate at the top of `handle_process_exit_attempt` refuses
    /// first and returns, so the continuation is never attempted. The window
    /// this site exists for is the epoch closing BETWEEN those two gates, which
    /// is a real interleaving and not one a single-threaded test can stage. What
    /// is skipped is the arrival; the call is the same call, with the arguments
    /// the `ContinuedAsNew` arm passes.
    ///
    /// The oracle is the workflow's own history. Continue-as-new keeps the
    /// workflow id and mints a new run id, so a successor that started would
    /// leave a SECOND `WorkflowStarted` under this id — read past the injector,
    /// so the count cannot be a fault of the instrument.
    #[tokio::test]
    async fn the_continuation_start_refuses_once_the_epoch_is_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let active = active_workflow().await?;
        let parent_run_id = active.handle.run_id().clone();
        let input = payload("successor-input")?;
        {
            let recorder = active.handle.recorder();
            let mut recorder = recorder.lock().await;
            recorder
                .record_workflow_continued_as_new(
                    chrono::Utc::now(),
                    input.clone(),
                    None,
                    parent_run_id.clone(),
                )
                .await?;
        }
        let before = active
            .context
            .store
            .read_history(active.handle.workflow_id())
            .await?;
        assert_eq!(
            before
                .iter()
                .filter(|event| matches!(event, Event::WorkflowStarted { .. }))
                .count(),
            1,
            "fixture: exactly the parent run has started, or the count below proves nothing: \
             {before:#?}"
        );

        // Exactly what `Drop for Engine` does: gate the epoch without awaiting.
        active.context.runtime.engine_tasks().begin_close();

        let error = super::start_continuation_replacement(
            &active.context,
            &active.handle,
            input,
            None,
            parent_run_id,
        )
        .await
        .err()
        .ok_or("a continuation start after the epoch closed must not be reported as handled")?;
        assert!(
            matches!(error, crate::EngineError::EngineTaskEpochClosed { .. }),
            "the refusal must name itself rather than surface as a store or spawn fault: {error:?}"
        );

        let after = active
            .context
            .store
            .read_history(active.handle.workflow_id())
            .await?;
        assert_eq!(
            after
                .iter()
                .filter(|event| matches!(event, Event::WorkflowStarted { .. }))
                .count(),
            1,
            "a refused continuation must not have started a successor run — a second \
             `WorkflowStarted` here is a run this dying engine handed to a successor while \
             still executing it, which is the double-writer hazard the gate exists to \
             prevent: {after:#?}"
        );
        Ok(())
    }

    /// 🔴 The RESUME arm refuses too — the arm the epoch gate did not cover.
    ///
    /// `an_attempt_refuses_to_append_once_the_epoch_is_closed` drives the
    /// no-terminal-yet arm, and it passed for a year while this arm had **no
    /// gate at all**. That is the whole shape of the defect: a mutation that
    /// removed the gate turned that test red, so the gate looked measured, while
    /// the branch beside it appended a real `TimerCancelled` into a history this
    /// engine no longer owns. A mutation only measures the branch the test
    /// executes.
    ///
    /// The setup is deliberately the one from
    /// `process_exit_resumes_interrupted_deadline_cancellation` — a terminal
    /// already in history plus an outstanding deadline — because that is the
    /// only shape in which the resume arm HAS an append to refuse. Without the
    /// outstanding deadline the arm writes nothing anyway and the test would
    /// pass with the gate deleted.
    #[tokio::test]
    async fn the_resume_arm_refuses_to_append_once_the_epoch_is_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let active = active_workflow().await?;
        let run_id = active.handle.run_id().clone();
        let deadline_id = crate::time::deadline_timer_id(&run_id)?;
        let result = payload("result")?;
        // Terminal already durable, deadline retirement still owed: this is what
        // sends the attempt down the `Err(existing)` resume arm.
        {
            let recorder = active.handle.recorder();
            let mut recorder = recorder.lock().await;
            recorder
                .record_timer_started(chrono::Utc::now(), deadline_id.clone(), chrono::Utc::now())
                .await?;
            recorder
                .record_workflow_completed(chrono::Utc::now(), result.clone())
                .await?;
        }
        let before = active
            .context
            .store
            .read_history(active.handle.workflow_id())
            .await?;
        assert!(
            crate::time::outstanding_deadline_timer(&before, &run_id).is_some(),
            "control: the resume arm must have a real append owed to it, or a passing \
             refusal proves nothing"
        );

        active.context.runtime.engine_tasks().begin_close();

        let error = handle_process_exit_async(
            active.context.clone(),
            active.handle.clone(),
            Ok(WorkflowProcessOutcome::Completed(result)),
        )
        .await
        .err()
        .ok_or("the resume arm must refuse once the epoch has closed, not report success")?;
        assert!(
            matches!(error, crate::EngineError::EngineTaskEpochClosed { .. }),
            "and the refusal must name itself on this arm too: {error:?}"
        );

        let after = active
            .context
            .store
            .read_history(active.handle.workflow_id())
            .await?;
        assert_eq!(
            before.len(),
            after.len(),
            "a refused resume must append NOTHING — not even the deadline retirement, which \
             is a real durable write into a history a successor engine may already own: \
             {after:#?}"
        );
        assert!(
            crate::time::outstanding_deadline_timer(&after, &run_id).is_some(),
            "the deadline therefore stays outstanding; the successor's startup sweep retires \
             it, which is the mechanism that actually owns this run now"
        );
        Ok(())
    }

    /// The dedup key is the typed variant, never the rendered error.
    ///
    /// `StoreError::Backend` embeds a backend-supplied string — peer address,
    /// elapsed time, leader hint, errno — so on a real distributed backend the
    /// `Display` output changes on nearly every iteration. Keying the "have I
    /// already said this" test on that string means the answer is always "no",
    /// and #94's log flood returns wearing a transition rule's name.
    ///
    /// The first assertion is the decisive one, and the second is its control:
    /// same variant with different payloads collapses, different variants do
    /// not. Without the second, a discriminant function that returned one
    /// constant for everything would pass.
    #[test]
    fn the_fault_key_is_the_typed_variant_not_the_rendered_error() {
        let first = crate::EngineError::Store(aion_store::StoreError::Backend(
            "peer 10.0.0.7:9042 unreachable after 1.20s".to_owned(),
        ));
        let second = crate::EngineError::Store(aion_store::StoreError::Backend(
            "peer 10.0.0.9:9042 unreachable after 4.80s; leader hint node-3".to_owned(),
        ));
        assert_ne!(
            first.to_string(),
            second.to_string(),
            "the rendered strings must differ, or this test cannot tell the two keys apart"
        );
        assert_eq!(
            failure_discriminant(&first),
            failure_discriminant(&second),
            "one backend fault reported twice with different detail is ONE fault"
        );

        let other = crate::EngineError::Store(aion_store::StoreError::NotOwner { shard: 3 });
        assert_ne!(
            failure_discriminant(&first),
            failure_discriminant(&other),
            "and genuinely different faults must still be distinguishable, or the key is a \
             constant and every change is suppressed"
        );
    }

    /// An unchanging fault is re-stated on attempt-count doubling, and only
    /// there.
    ///
    /// Both halves matter and they pull in opposite directions: state it every
    /// attempt and an outage floods the log at the backoff rate; state it only
    /// on change and a permanently stuck run says one thing at t=0 and then
    /// nothing at all, so six hours wedged looks exactly like three milliseconds
    /// succeeded.
    #[test]
    fn an_unchanging_fault_is_restated_on_attempt_count_doubling() {
        let mut reporter = FaultReporter::new();
        let stated_at: Vec<u64> = (1..=32)
            .filter(|attempt| reporter.should_report("store/backend-unavailable", *attempt))
            .collect();
        assert_eq!(
            stated_at,
            vec![1, 2, 4, 8, 16, 32],
            "an unchanged fault is re-stated on the powers of two and stays quiet between them"
        );
    }

    /// A changed fault is stated immediately, without pushing the ladder out.
    ///
    /// The second half is the subtle one. If a change also reset the escalation
    /// point, a store alternating between two faults would re-arm the ladder on
    /// every attempt and could go quiet indefinitely — a silence produced by
    /// flapping, which is the condition most worth hearing about.
    #[test]
    fn a_changed_fault_is_stated_at_once_and_does_not_defer_the_escalation() {
        let mut reporter = FaultReporter::new();
        assert!(reporter.should_report("store/backend-unavailable", 1));
        assert!(
            !reporter.should_report("store/backend-unavailable", 2 - 1),
            "the same fault at the same attempt says nothing new"
        );
        assert!(
            reporter.should_report("engine/terminal-writer-held", 1),
            "a different fault is news the moment it appears"
        );
        assert!(
            reporter.should_report("engine/terminal-writer-held", 2),
            "and the escalation point set by attempt 1 still fires at attempt 2"
        );
    }

    /// A failure AFTER the terminal landed says so.
    ///
    /// Every operator message on this path used to assert the terminal was not
    /// recorded and that the run stays `Running`. That is unchecked: the append
    /// is durable well before the attempt ends, and the deadline retirement,
    /// visibility upsert and registry reconcile that follow it can all fail. An
    /// operator told "the run stays Running" about a run that is `Completed`
    /// goes looking for the wrong thing.
    ///
    /// The fault is injected at the visibility read — the SECOND history read of
    /// the attempt — so it lands strictly after the terminal is durable. The
    /// first assertion is the control: without it a green result could mean the
    /// injected fault never fired at all.
    #[tokio::test]
    async fn a_failure_after_the_terminal_landed_reports_it_as_recorded()
    -> Result<(), Box<dyn std::error::Error>> {
        let (active, flaky) = flaky_workflow().await?;
        let result = payload("result")?;
        // Attempt read #1 passes; the visibility upsert's read is refused.
        flaky.fail_reads_after(1, 1);

        let intent =
            TerminalIntent::from_outcome(Ok(WorkflowProcessOutcome::Completed(result.clone())));
        let failure = complete_process_exit(
            &active.context,
            &active.handle,
            &intent,
            TerminalProgress::NotRecorded,
        )
        .await
        .err()
        .ok_or("the injected fault must have failed the attempt")?;

        let recorded = flaky.recorded_history(active.handle.workflow_id()).await?;
        assert!(
            recorded
                .iter()
                .any(|event| matches!(event, Event::WorkflowCompleted { .. })),
            "control: the terminal must actually be durable, or 'recorded' would be the wrong \
             answer and this test would be passing for the wrong reason: {recorded:#?}"
        );

        match failure {
            CompletionFailure::Retryable(_, TerminalProgress::Recorded)
            | CompletionFailure::Invariant(_, TerminalProgress::Recorded) => Ok(()),
            other => Err(format!(
                "a failure raised after a durable terminal must carry Recorded, found {other:?}"
            )
            .into()),
        }
    }
}
