//! Engine-side handler that drives an elapsed workflow deadline to a
//! `WorkflowTimedOut` terminal.
//!
//! Registered on the timer bridge at engine construction, this is the seam the
//! `TimerService` demuxes a reserved `deadline:{run_id}` fire to. It records the
//! terminal under the per-handle recorder lock — with a terminal re-check so it
//! loses cleanly to a concurrent completion — then tears the run down matching
//! `terminate::cancel` discipline: kill the process, refresh visibility, notify
//! result awaiters, and deregister.
//!
//! It holds a `Weak<RuntimeHandle>` (never a strong one) so the engine's
//! `RuntimeHandle` → `EngineNifState` → timer bridge → handler chain does not
//! cycle back into the runtime — the same cycle-avoidance the timer bridge's
//! `Weak<EngineNifState>` observes.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use aion_core::{Event, RunId, TimerCancelCause, WorkflowId};
use aion_store::EventStore;
use aion_store::visibility::VisibilityStore;
use chrono::Utc;

use crate::durability::Recorder;
use crate::registry::{Registry, TerminalOutcome, WorkflowHandle};
use crate::runtime::RuntimeHandle;
use crate::time::timer_service::live_timers_in_active_segment;
use crate::time::{DeadlineHandler, DeadlineHandlerError, WORKFLOW_TIMEOUT_DESCRIPTOR};

use super::completion::terminal_outcome_from_history;
use super::visibility::upsert_workflow_visibility;

/// Whether the elapsed deadline records a fresh terminal, resumes an interrupted
/// teardown of its own prior terminal, or loses cleanly to a competing terminal.
enum DeadlineDisposition {
    /// This call appended `WorkflowTimedOut`; run the full teardown.
    Appended,
    /// Our own `WorkflowTimedOut` is already durable but teardown was
    /// interrupted; resume the idempotent teardown without a second terminal.
    ResumeTeardown,
    /// A competing terminal already won (or the deadline is no longer live);
    /// nothing to record and nothing to tear down.
    LoseCleanly,
    /// This engine's timer wheel has been torn down; the run belongs to
    /// whichever engine owns it now, so record nothing and tear nothing down.
    ///
    /// 🔴 DELIBERATELY NOT [`Self::LoseCleanly`], though both end in `Ok(())`.
    /// `LoseCleanly` means a competing TERMINAL won and this run is settled;
    /// `StoodDown` means this run is not settled at all and this engine has
    /// merely stopped being the one entitled to speak for it. Folding them would
    /// put an operator reading the debug line on the wrong trail, and this lane
    /// exists precisely because one message was raised for two causes.
    StoodDown,
}

/// Records `WorkflowTimedOut` and tears down a run whose deadline elapsed.
pub struct WorkflowDeadlineHandler {
    /// Weak to avoid the `RuntimeHandle`↔`EngineNifState`↔bridge cycle; upgraded
    /// only to kill the timed-out process.
    runtime: Weak<RuntimeHandle>,
    store: Arc<dyn EventStore>,
    visibility_store: Arc<dyn VisibilityStore>,
    registry: Arc<Registry>,
    /// The timer bridge's own `shut_down` latch, SHARED (not copied).
    ///
    /// 🔴 THE DEADLINE PATH IS THE ONE DURABLE WRITER THE WHEEL'S APPEND
    /// BOUNDARY CANNOT REACH. `TimerService::fire_timer_guarded` demuxes a
    /// reserved `deadline:{run}` fire to this handler BEFORE the generic
    /// record-then-deliver path, so the boundary refusal in
    /// `TimerNifBridge::record_workflow_event` — which is what stops an ordinary
    /// `TimerFired` from being appended by an engine that has stood down — is
    /// never on a deadline's route. Without this flag a deadline task already
    /// inside its poll when `shutdown_timer_wheel` ran would go on to append a
    /// durable `WorkflowTimedOut` and tear the run down, for a run a successor
    /// engine may already own: a second writer for one workflow, which is the
    /// #119 breach and load-bearing invariant 3.
    ///
    /// `abort` cannot prevent it — a `JoinHandle::abort` does not stop a task
    /// that has already entered a poll — so the refusal has to sit here, at the
    /// point of writing.
    stand_down: Arc<AtomicBool>,
}

impl WorkflowDeadlineHandler {
    /// Assembles a deadline handler from the engine's teardown dependencies.
    ///
    /// `runtime` is held weakly on purpose (see the module docs); the rest are
    /// the same durable store, visibility index, and active registry the
    /// `terminate::cancel` path uses.
    ///
    /// `stand_down` must be the timer bridge's OWN latch, shared by `Arc` — see
    /// the field. A fresh flag here would compile, pass every test that sets it
    /// directly, and gate nothing in production, because nothing would ever set
    /// it.
    ///
    /// 🔴 CRATE-PRIVATE BECAUSE IT HAS NO BUSINESS BEING PUBLIC — AND FOR NO
    /// LARGER REASON THAN THAT.
    ///
    /// An earlier version of this comment justified the narrowing with a threat:
    /// that a downstream consumer of `aion-rs` could build a handler around
    /// `Arc::new(AtomicBool::new(false))` and get a deadline writer that never
    /// stands down. **That threat was not real, and the boundary it implied does
    /// not exist.** Both halves are wrong, and recording why is worth more than
    /// the tidier sentence it replaces:
    ///
    /// - The attack was unreachable. `register_deadline_handler`
    ///   (`runtime/nif_timer_bridge.rs`) is ALREADY `pub(crate)`, so no external
    ///   caller could register such a handler in the first place; the timer
    ///   service would never route a fire to it. The registration seam's claim
    ///   was already true, not aspirational.
    /// - The larger hole the sentence implied was closed is wide open, and this
    ///   constructor is nowhere near it. `Recorder::new` is `pub`, `durability`
    ///   is a `pub mod` re-exporting it, and `record_workflow_timed_out`,
    ///   `record_workflow_continued_as_new`, and `record_workflow_failed` are all
    ///   `pub`. Any downstream consumer can write any terminal into any history
    ///   without going near a deadline handler. Narrowing this signature buys
    ///   nothing against that, and pretending otherwise would leave the next
    ///   reader believing in a wall that is not there.
    ///
    /// What the narrowing IS good for: this constructor takes five collaborators
    /// that must be the engine's own, one of which — `stand_down` — is only
    /// correct when it is the timer bridge's shared latch rather than a fresh
    /// flag. Nothing in the signature can enforce that, so the type keeps the
    /// only guarantee it can: the sole supported way to build one is the seam
    /// that supplies the right latch. `pub(crate)` states that in the language
    /// instead of in a comment. Do not widen it back.
    #[must_use]
    pub(crate) fn new(
        runtime: Weak<RuntimeHandle>,
        store: Arc<dyn EventStore>,
        visibility_store: Arc<dyn VisibilityStore>,
        registry: Arc<Registry>,
        stand_down: Arc<AtomicBool>,
    ) -> Self {
        Self {
            runtime,
            store,
            visibility_store,
            registry,
            stand_down,
        }
    }

    /// Body of the timeout terminal + teardown, returning typed engine errors.
    async fn drive_timed_out(
        &self,
        workflow_id: WorkflowId,
        run_id: RunId,
    ) -> Result<(), crate::EngineError> {
        let Some(handle) = self.registry.get(&workflow_id, &run_id)? else {
            // No registered handle. This is NOT automatically a no-op: a cold
            // engine (or a shard adopter) never registers a terminal run, so a
            // recovered deadline row whose durable history shows `WorkflowTimedOut`
            // with teardown left unfinished reaches here with no handle. Complete
            // that teardown registry-free — this is the ONLY actor that finishes
            // it. A non-timeout terminal, or a fully-torn-down run, is a genuine
            // no-op (its deadline is already retired or was never this run's).
            return self
                .finalize_timed_out_without_handle(&workflow_id, &run_id)
                .await;
        };

        let disposition = self
            .decide_disposition(&handle, &workflow_id, &run_id)
            .await?;
        match disposition {
            DeadlineDisposition::LoseCleanly => Ok(()),
            DeadlineDisposition::StoodDown => {
                // Not a fault and not a loss: an orderly stand-down. The durable
                // deadline row is untouched and still live, so whichever engine
                // owns the run re-arms it and times the run out there. DEBUG,
                // like every other stand-down in the crate.
                tracing::debug!(
                    %workflow_id,
                    %run_id,
                    "workflow deadline abandoned: this engine's timer wheel has been torn down and the deadline stays live for its owner"
                );
                Ok(())
            }
            DeadlineDisposition::Appended | DeadlineDisposition::ResumeTeardown => {
                self.tear_down(&handle, &workflow_id, &run_id).await
            }
        }
    }

    /// Whether this engine has stood down and may no longer speak for the run.
    ///
    /// The one place the flag and its ordering are named. `SeqCst` matters: it
    /// is the same store `shutdown_timer_wheel` performs, and the total order
    /// across the two is what makes "set before we looked" observable at all.
    fn stood_down(&self) -> bool {
        self.stand_down.load(Ordering::SeqCst)
    }

    /// Decides — under the recorder lock — whether to append a fresh
    /// `WorkflowTimedOut`, resume an interrupted teardown of an already-recorded
    /// one, or lose cleanly.
    ///
    /// The terminal re-check, the deadline-liveness re-check, and the terminal
    /// append are one critical section: a concurrent complete/fail/cancel
    /// records through the same recorder, so checking outside the lock could
    /// double-record a terminal or let a cancelled deadline still time the run
    /// out.
    ///
    /// 🔴 THE STAND-DOWN CHECK IS DELIBERATELY NOT IN THAT LIST. An earlier
    /// version of this doc put it there, which was false and load-bearing:
    /// `shutdown_timer_wheel` sets the flag with an atomic store and a map
    /// drain, and neither it nor either of its callers (`Engine::shutdown`,
    /// `Engine::drop`) ever takes a recorder lock. So this lock excludes other
    /// RECORDER WRITERS and excludes nothing whatever about the flag. Naming a
    /// mechanism that does not cover the case is the error this file has now
    /// made twice; see the note on the check itself.
    async fn decide_disposition(
        &self,
        handle: &WorkflowHandle,
        workflow_id: &WorkflowId,
        run_id: &RunId,
    ) -> Result<DeadlineDisposition, crate::EngineError> {
        let recorder = handle.recorder();
        let mut recorder = recorder.lock().await;
        // 🔴 CHECK-THEN-ACT, AND SAYING OTHERWISE WAS THE DEFECT. This read is
        // NOT made decisive by the lock it sits under — see the note on this
        // function. A stand-down landing after this load and before a write
        // below is not excluded by anything here, and the `.await` on
        // `read_history` is a real yield point in that window.
        //
        // What this read IS worth: it settles the common case (the engine had
        // already stood down when the fire began) before spending a store round
        // trip, and it is re-taken immediately before EVERY durable write that
        // follows, so the exposed window is an instruction or two rather than a
        // store round trip. That is narrowing, not closing, and the difference
        // is the whole reason this comment is worded the way it is.
        //
        // It cannot be closed here. Closing it means the stand-down setter and
        // this writer taking one common lock — but the setter runs in a `Drop`,
        // synchronously, over every workflow at once, so making it take a
        // per-workflow async recorder lock is not available at any price. A
        // second lock spanning one workflow is what invariant 3 forbids
        // outright.
        if self.stood_down() {
            return Ok(DeadlineDisposition::StoodDown);
        }
        let history = self.store.read_history(workflow_id).await?;
        match terminal_outcome_from_history(&history, run_id) {
            Some(TerminalOutcome::TimedOut(_)) => {
                // Our own terminal is durable but teardown did not finish (a
                // dropped runtime, a failed visibility upsert, an interrupted
                // fire). Resume the idempotent teardown — do NOT append again.
                tracing::debug!(
                    %workflow_id,
                    %run_id,
                    "workflow deadline re-fired after its WorkflowTimedOut was recorded; resuming teardown"
                );
                Ok(DeadlineDisposition::ResumeTeardown)
            }
            Some(_) => {
                // A competing terminal (complete/fail/cancel/continue-as-new) won.
                // The deadline loses — but if it is still outstanding, that
                // terminal writer's own deadline cancellation did not commit (a
                // two-write crash), so this fire REPAIRS it: retire the deadline
                // here, under the recorder lock, rather than losing without
                // cancelling and letting whole-history recovery keep re-arming it.
                // This is the guaranteed re-drive for an interrupted non-timeout
                // terminal transition — the live wheel or `recover_due`/`tick`
                // re-arms the still-live deadline, and this fire completes D5.
                tracing::debug!(
                    %workflow_id,
                    %run_id,
                    "workflow deadline elapsed but another terminal was already recorded; retiring the deadline and losing"
                );
                // Re-taken after the `read_history` await: the repair below is
                // a durable write, and a stand-down during that round trip
                // means it is no longer ours to make. The owner's own recovery
                // re-arms the still-live deadline and repairs it there.
                if self.stood_down() {
                    return Ok(DeadlineDisposition::StoodDown);
                }
                crate::time::retire_run_deadline(&mut recorder, &history, run_id).await?;
                Ok(DeadlineDisposition::LoseCleanly)
            }
            None => {
                // Re-check THIS deadline is still live: a cancel that recorded
                // `TimerCancelled { WorkflowIntent }` before its terminal must win,
                // so a retired deadline loses cleanly rather than timing the run
                // out after its cancellation.
                if crate::time::outstanding_deadline_timer(&history, run_id).is_none() {
                    tracing::debug!(
                        %workflow_id,
                        %run_id,
                        "workflow deadline elapsed but its timer was already retired; deadline loses"
                    );
                    return Ok(DeadlineDisposition::LoseCleanly);
                }
                // Re-taken immediately before the terminal. This is the write
                // the whole stand-down exists to stop, and the `read_history`
                // above was a yield point — so the load at the top of this
                // function is too old to be the one that decides it.
                if self.stood_down() {
                    return Ok(DeadlineDisposition::StoodDown);
                }
                recorder
                    .record_workflow_timed_out(Utc::now(), WORKFLOW_TIMEOUT_DESCRIPTOR.to_owned())
                    .await?;
                Ok(DeadlineDisposition::Appended)
            }
        }
    }

    /// Idempotent, resumable teardown after the `WorkflowTimedOut` terminal is
    /// durable.
    ///
    /// Ordering is the invariant that makes resume reachable: the run's OWN
    /// deadline timer stays live and its registry entry stays present until every
    /// fallible teardown step has succeeded. So it retires the ordinary
    /// (non-deadline) timers first, confirms process teardown and refreshes
    /// visibility, notifies awaiters, and only THEN retires the deadline itself
    /// and deregisters. A failure in any earlier step is PROPAGATED (not merely
    /// logged): the handler returns it as a fire failure, the deadline remains
    /// live, and recovery's `outstanding_future_timers` re-arms it so a later fire
    /// re-enters here and resumes — rather than destroying both retry anchors
    /// before the work that needs them.
    ///
    /// # Errors
    ///
    /// Returns the typed [`crate::EngineError`] from the first failing durable
    /// step so recovery retries the interrupted teardown.
    async fn tear_down(
        &self,
        handle: &WorkflowHandle,
        workflow_id: &WorkflowId,
        run_id: &RunId,
    ) -> Result<(), crate::EngineError> {
        // 1. Retire the run's ordinary (non-deadline) timers. The deadline is
        //    deliberately NOT retired here — it is the resume anchor.
        self.retire_ordinary_timers(handle, workflow_id, run_id)
            .await?;

        // 2. Stop the timed-out process. A cancel failure means it already
        //    exited (benign); a dropped runtime is propagated so a re-fire under a
        //    live runtime completes the kill.
        match self.runtime.upgrade() {
            Some(runtime) => {
                if let Err(error) = runtime.cancel_pid(handle.pid()) {
                    tracing::debug!(
                        %workflow_id,
                        %run_id,
                        %error,
                        "workflow process already exited during deadline teardown"
                    );
                }
            }
            None => {
                return Err(crate::EngineError::Runtime {
                    reason: format!(
                        "runtime dropped during deadline teardown of {workflow_id}/{run_id}; a later re-fire resumes teardown"
                    ),
                });
            }
        }

        // 3. Refresh visibility; a failure is propagated so it is retried.
        upsert_workflow_visibility(
            Arc::clone(&self.store),
            Arc::clone(&self.visibility_store),
            workflow_id,
            run_id,
        )
        .await?;

        // 4. Notify awaiters (a doorbell send; never a retry condition).
        handle.completion().notify(TerminalOutcome::TimedOut(
            WORKFLOW_TIMEOUT_DESCRIPTOR.to_owned(),
        ));

        // 5. Retire the deadline LAST, once teardown has otherwise succeeded, so
        //    no earlier failure could have removed the resume anchor. Idempotent.
        self.retire_deadline(handle, workflow_id, run_id).await?;

        // 6. Deregister LAST.
        self.registry.remove(workflow_id, run_id)?;
        Ok(())
    }

    /// Retires the timed-out run's still-live ORDINARY timers (every live timer
    /// except this run's own deadline) by recording `TimerCancelled { WorkflowIntent }`
    /// for each, through the handle recorder under its lock. The deadline is
    /// excluded so it stays live as the teardown resume anchor. Idempotent — a
    /// re-run sees the same timers already retired and records nothing.
    ///
    /// # Errors
    ///
    /// Returns the typed [`crate::EngineError`] when history cannot be read or a
    /// cancellation append fails, so the interrupted teardown is retried.
    async fn retire_ordinary_timers(
        &self,
        handle: &WorkflowHandle,
        workflow_id: &WorkflowId,
        run_id: &RunId,
    ) -> Result<(), crate::EngineError> {
        let recorder = handle.recorder();
        let mut recorder = recorder.lock().await;
        let history = self.store.read_history(workflow_id).await?;
        record_ordinary_timer_retirements(&mut recorder, &history, run_id).await?;
        Ok(())
    }

    /// Registry-free completion of an interrupted timeout teardown.
    ///
    /// A cold engine and a shard adopter never register a terminal run, so a
    /// recovered due deadline row reaches [`Self::drive_timed_out`] with no
    /// handle. When durable history shows this run's own `WorkflowTimedOut` with
    /// teardown left unfinished (an outstanding deadline or still-live ordinary
    /// timers), this finishes the SAME durable steps the handle path runs —
    /// ordinary timers first, visibility, then the deadline LAST — through an
    /// independent recorder. It deliberately omits the handle-only side effects:
    /// the process is already gone (the run is terminal), there are no local
    /// awaiters this epoch, and nothing is registered to deregister. A non-timeout
    /// or already-finished run is a clean no-op.
    ///
    /// # Errors
    ///
    /// Returns the typed [`crate::EngineError`] from the first failing durable
    /// step so the caller (recovery) retries.
    async fn finalize_timed_out_without_handle(
        &self,
        workflow_id: &WorkflowId,
        run_id: &RunId,
    ) -> Result<(), crate::EngineError> {
        let history = self.store.read_history(workflow_id).await?;
        if !matches!(
            terminal_outcome_from_history(&history, run_id),
            Some(TerminalOutcome::TimedOut(_))
        ) {
            tracing::debug!(
                %workflow_id,
                %run_id,
                "unregistered deadline elapsed for a run that is not TimedOut; nothing to finalize"
            );
            return Ok(());
        }
        // 🔴 CHECK-THEN-ACT, exactly like the gate on the handle path — and for
        // the same reason, not a different one. An earlier version of this
        // comment drew a distinction ("honestly weaker than the other gate")
        // that does not exist: neither gate is decisive, because the flag's
        // setter takes no lock either path could share with it.
        //
        // What CANNOT be done is making it decisive — that needs the setter and
        // this writer under one lock, and the setter is a synchronous `Drop`
        // sweeping every workflow at once. What CAN be done, and now is, is
        // re-reading before each of the three durable writes below: the earlier
        // wording said it "cannot be strengthened", which conflated the two and
        // was a limitation dressed up as a contract. One read guarding three
        // writes separated by `.await` points left the second and third exposed
        // for a whole visibility round trip.
        if self.stood_down() {
            tracing::debug!(
                %workflow_id,
                %run_id,
                "unregistered deadline finalization abandoned: this engine's timer wheel has been torn down"
            );
            return Ok(());
        }
        let head = history.iter().map(Event::seq).max().unwrap_or_default();
        let mut recorder = Recorder::resume_at(workflow_id.clone(), Arc::clone(&self.store), head);
        // Ordinary timers first (the deadline is retired LAST), then visibility.
        //
        // Re-read before each. Abandoning PART WAY through is safe here and is
        // the point of the ordering: the deadline is retired last, so a run
        // abandoned mid-teardown still has a live deadline, and the owner's
        // recovery re-arms it and finishes the same idempotent steps. Carrying
        // on instead would be this engine writing to a run it has just been
        // told is not its own.
        record_ordinary_timer_retirements(&mut recorder, &history, run_id).await?;
        if self.stood_down() {
            tracing::debug!(
                %workflow_id,
                %run_id,
                "unregistered deadline finalization abandoned after retiring ordinary timers: this engine's timer wheel has been torn down"
            );
            return Ok(());
        }
        upsert_workflow_visibility(
            Arc::clone(&self.store),
            Arc::clone(&self.visibility_store),
            workflow_id,
            run_id,
        )
        .await?;
        if self.stood_down() {
            tracing::debug!(
                %workflow_id,
                %run_id,
                "unregistered deadline finalization abandoned before retiring the deadline: this engine's timer wheel has been torn down"
            );
            return Ok(());
        }
        crate::time::retire_run_deadline(&mut recorder, &history, run_id).await?;
        Ok(())
    }

    /// Retires this run's own declared-timeout deadline as the final teardown
    /// step, via the shared `retire_run_deadline` primitive. Idempotent — a
    /// resumed teardown whose deadline is already retired records nothing.
    ///
    /// # Errors
    ///
    /// Returns the typed [`crate::EngineError`] when history cannot be read or the
    /// cancellation append fails.
    async fn retire_deadline(
        &self,
        handle: &WorkflowHandle,
        workflow_id: &WorkflowId,
        run_id: &RunId,
    ) -> Result<(), crate::EngineError> {
        let recorder = handle.recorder();
        let mut recorder = recorder.lock().await;
        let history = self.store.read_history(workflow_id).await?;
        crate::time::retire_run_deadline(&mut recorder, &history, run_id).await?;
        Ok(())
    }
}

/// Records `TimerCancelled { WorkflowIntent }` for every still-live ORDINARY
/// timer in the run's active segment — the deadline itself is excluded so it
/// stays live as the teardown resume anchor. Shared by the handle-based teardown
/// and the registry-free finalizer so both settle ordinary timers identically.
/// Idempotent: a re-run sees the same timers already retired and records nothing.
///
/// # Errors
///
/// Returns the recorder's [`crate::durability::DurabilityError`] when a
/// cancellation append fails.
async fn record_ordinary_timer_retirements(
    recorder: &mut Recorder,
    history: &[Event],
    run_id: &RunId,
) -> Result<(), crate::durability::DurabilityError> {
    let deadline = crate::time::outstanding_deadline_timer(history, run_id);
    for timer_id in live_timers_in_active_segment(history) {
        if deadline.as_ref() == Some(&timer_id) {
            continue;
        }
        recorder
            .record_timer_cancelled(Utc::now(), timer_id, TimerCancelCause::WorkflowIntent)
            .await?;
    }
    Ok(())
}

#[async_trait::async_trait]
impl DeadlineHandler for WorkflowDeadlineHandler {
    async fn on_deadline_elapsed(
        &self,
        workflow_id: WorkflowId,
        run_id: RunId,
    ) -> Result<(), DeadlineHandlerError> {
        self.drive_timed_out(workflow_id, run_id)
            .await
            .map_err(|error| DeadlineHandlerError(error.to_string()))
    }
}

#[cfg(test)]
#[path = "deadline_tests.rs"]
mod tests;
