//! The fire side of the timer NIF bridge: what a due timer does, what a
//! torn-down wheel refuses, and how each is worded.
//!
//! Split out of `nif_timer_bridge` because that file crossed the 500-line
//! production cap this codebase holds itself to. The seam is a real one rather
//! than a slice taken to hit a number: everything here runs *after* a timer is
//! due — the fire callback, its bounded deadline retry, the refusals that stop
//! an append, and the history predicates those refusals consult — while the
//! bridge proper owns the struct, the wheel, and the [`EngineHandle`] impl.

use std::sync::Weak;
use std::time::Duration;

use aion_core::{Event, TimerCancelCause, TimerId, WorkflowId};
use aion_store::StoreError;
use chrono::{DateTime, Utc};

use crate::engine_seam::EngineSeamError;
use crate::runtime::nif_state::EngineNifState;
use crate::runtime::nif_timer_bridge::{TimerNifBridge, timer_bridge};
use crate::time::TimerServiceError;

/// The refusal the two ARMING paths report once this engine's wheel is gone.
///
/// Raised by the pre-arm gate and by the post-insert retraction — both of which
/// are refusing to *arm*, so "was not armed" is a true sentence at each.
///
/// 🔴 THIS IS NOT THE REFUSAL THE APPEND BOUNDARY USES, and for a while it was.
/// One constructor served all three sites, justified on the reasoning that a
/// refusal whose wording differs by site teaches the operator that the causes
/// differ when they do not. The reasoning was sound and the premise was false:
/// at the append boundary the timer HAD been armed, its sleep HAD elapsed, and
/// it HAD fired — what is refused there is the durable append, not the arm. An
/// operator reading "timer `X` was not armed" against a timer they watched fire
/// would go looking for an arming bug that does not exist. See
/// [`wheel_torn_down_after_firing`], which is the same cause said truthfully.
pub(super) fn wheel_torn_down_before_arming(timer_id: &TimerId) -> EngineSeamError {
    EngineSeamError::TimerWheel {
        reason: format!(
            "timer `{timer_id}` was not armed: this engine's timer wheel has been torn down, so \
             the fire belongs to whichever engine owns the run now"
        ),
    }
}

/// The refusal the append boundary reports for a refused FIRE.
///
/// The timer was armed and it fired; what this refuses is writing the durable
/// `TimerFired`, because a successor engine may already own the run and a
/// second writer for one workflow is the #119 breach. The timer is still live
/// in durable history and the owning engine re-arms it from there, so the
/// refusal costs the run nothing — which is the fact the wording has to carry,
/// or an operator reads this as a lost timer.
///
/// 🔴 THIS IS THE FIRE CASE ONLY. The same boundary refuses a `TimerCancelled`,
/// and for that one every clause above is wrong: nothing fired, and "still live,
/// the owner re-arms it" is the HARM rather than the reassurance. See
/// [`wheel_torn_down_before_cancelling`]. One constructor served both for
/// exactly as long as it took a reviewer to ask.
pub(super) fn wheel_torn_down_after_firing(timer_id: &TimerId) -> EngineSeamError {
    EngineSeamError::TimerWheel {
        reason: format!(
            "timer `{timer_id}` fired, but its append was refused: this engine's timer wheel has \
             been torn down, so the fire belongs to whichever engine owns the run now. The timer \
             is still live in durable history and the owning engine re-arms it from there"
        ),
    }
}

/// The refusal the append boundary reports for a refused CANCEL.
///
/// 🔴 A REFUSED CANCEL IS NOT A REFUSED FIRE WEARING A DIFFERENT HAT. A refused
/// fire preserves the run's intent by itself: the timer stays live, the owning
/// engine re-arms it, and it fires there. A refused cancel is the opposite —
/// the run asked for the timer to STOP, the timer stays live precisely because
/// the cancellation did not commit, and the owning engine will re-arm the very
/// timer the run wanted gone. The wording must say that, because the sentence
/// that reassures an operator about a fire is the sentence that should alarm
/// them about a cancel.
///
/// The intent is not lost — the run reissues the cancellation against the
/// owning engine as it re-executes — but that happens THERE and later, which is
/// a materially different fact from "costs the run nothing", so it is stated as
/// the condition it is rather than folded into the same sentence.
pub(super) fn wheel_torn_down_before_cancelling(timer_id: &TimerId) -> EngineSeamError {
    EngineSeamError::TimerWheel {
        reason: format!(
            "cancellation of timer `{timer_id}` was not recorded: this engine's timer wheel has \
             been torn down, so the run belongs to whichever engine owns it now. The timer stays \
             live in durable history and that engine re-arms it; the cancellation takes effect \
             only once the run reissues it there"
        ),
    }
}

/// The refusal the append boundary reports for a refused TEARDOWN cancel.
///
/// 🔴 "THE RUN REISSUES IT THERE" IS FALSE ON THIS PATH, and stating it here was
/// the same defect as F-A one level down. [`wheel_torn_down_before_cancelling`]
/// is true of a [`TimerCancelCause::WorkflowIntent`] cancel, where workflow code
/// re-executes on the owning engine and issues the cancellation again. A
/// [`TimerCancelCause::CancelTeardown`] cancel comes from `Engine::cancel`
/// retiring a run's in-flight timers: that run is being CANCELLED, it will never
/// re-execute, and nothing will ever reissue anything. Telling an operator to
/// wait for a reissue that cannot happen sends them looking for a stuck run.
///
/// The remedy, and the ONE CONDITION IT DEPENDS ON — which an earlier draft
/// asserted as already true and is not, at the moment this error is raised.
///
/// `Engine::cancel` calls `cancel_inflight_timers` BEFORE `terminate::cancel`,
/// and its own doc says that ordering is mandatory (a cancel that leaves a live
/// timer behind orphans it). So when this refusal fires, `WorkflowCancelled` has
/// **not** been recorded yet: the run is still `Running`. Saying "the run is
/// terminal" here states the intended end of a sequence as though it were the
/// current state.
///
/// When the sequence completes — the overwhelmingly common case — the remedy is
/// real: the timer stays live, the owning engine's wheel fires it, and
/// `fire_timer_guarded` puts that fire through the recorder seam, which refuses
/// a post-terminal append as `RecordOutcome::RefusedTerminal`, recording nothing
/// and waking nothing. Inert rather than dangerous, and no operator action.
///
/// When it does not complete, the remedy is not available and the operator needs
/// to know the shape of it. `cancel_inflight_timers` swallows every failure into
/// a `tracing::warn!`, and `Engine::cancel` propagates a `terminate::cancel`
/// failure to its caller — so a run can be left `Running` with this timer still
/// armed, and the owning engine's fire is then NOT post-terminal. It records
/// `TimerFired` and wakes a run the operator was told was cancelled.
pub(super) fn wheel_torn_down_before_teardown_cancel(timer_id: &TimerId) -> EngineSeamError {
    EngineSeamError::TimerWheel {
        reason: format!(
            "teardown cancellation of timer `{timer_id}` was not recorded: this engine's timer \
             wheel has been torn down, so the run belongs to whichever engine owns it now. The \
             timer stays live in durable history. The cancel transition that issued this runs \
             immediately after it, and once that run's terminal lands the timer is inert — the \
             owning engine's fire is refused as post-terminal, recording nothing. No operator \
             action in that case. If the cancel itself then failed, the run is still live with \
             this timer armed and it will fire: check the run's status before assuming it is gone"
        ),
    }
}

/// Which append a torn-down wheel refused — and for a cancel, on whose behalf.
///
/// The whole point of the type is that the wording cannot be chosen without it.
/// A `bool` was enough to tell a fire from a cancel and NOT enough to tell the
/// two cancels apart, so the workflow-intent sentence was raised for a teardown
/// cancel it is false of. Carrying the cause itself means a new
/// [`TimerCancelCause`] variant makes `into_seam_error` fail to compile until
/// somebody decides what it should say.
pub(super) enum RefusedAppend {
    /// A `TimerFired` append was refused.
    Fire,
    /// A `TimerCancelled` append was refused, with the cause it carried.
    Cancel(TimerCancelCause),
}

/// What can stop the bridge's durable append, kept TYPED rather than boxed.
///
/// 🔴 A BOXED ERROR HERE FLATTENS TWO UNRELATED FAILURES INTO ONE. The blocking
/// body used to return `Box<dyn Error>`, so the single `map_err` closing
/// `record_workflow_event` had nothing to switch on and reported every failure
/// as [`EngineSeamError::Recorder`] — including a wheel teardown, which is not
/// a recorder failure at all. That cost twice over: an operator chasing a
/// `Recorder` error into the durability layer for what was an ordinary engine
/// stand-down, and [`fire_wheel_timer`] below unable to tell that stand-down
/// from a store that had genuinely broken.
pub(super) enum TimerAppendError {
    /// This engine's wheel was torn down before this timer event's append.
    ///
    /// Carries the OUTCOME as well as the id, because the outcomes need
    /// different words. Dropping a discriminant here is what let one sentence be
    /// raised for several cases — twice, at two different depths: first `bool`
    /// was absent entirely and a cancel was reported as a fire, then `bool` was
    /// present and both KINDS of cancel got the workflow-intent sentence. The
    /// type now carries everything the wording distinguishes on, so a new cause
    /// cannot be added without this match forcing a decision about its words.
    WheelTornDown {
        /// The timer whose append was refused.
        timer_id: TimerId,
        /// Which append was refused, and for a cancel, on whose behalf.
        refused: RefusedAppend,
    },
    /// The store or the recorder refused the append on its own terms.
    Append(Box<dyn std::error::Error + Send + Sync>),
}

impl TimerAppendError {
    /// Map the typed failure onto the seam error it actually is.
    pub(super) fn into_seam_error(self) -> EngineSeamError {
        match self {
            Self::WheelTornDown {
                timer_id,
                refused: RefusedAppend::Fire,
            } => wheel_torn_down_after_firing(&timer_id),
            Self::WheelTornDown {
                timer_id,
                refused: RefusedAppend::Cancel(TimerCancelCause::WorkflowIntent),
            } => wheel_torn_down_before_cancelling(&timer_id),
            Self::WheelTornDown {
                timer_id,
                refused: RefusedAppend::Cancel(TimerCancelCause::CancelTeardown),
            } => wheel_torn_down_before_teardown_cancel(&timer_id),
            Self::Append(error) => EngineSeamError::Recorder {
                reason: error.to_string(),
            },
        }
    }

    /// Box a store or recorder failure into the `Append` arm.
    pub(super) fn append(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Append(Box::new(error))
    }
}

/// Whether a failed fire is THIS ENGINE STANDING DOWN rather than a fault.
///
/// An engine that has torn its wheel down will refuse this fire and every
/// retry of it identically — the `shut_down` flag latches and is never cleared
/// — and the run loses nothing by the refusal, because the timer stays live in
/// durable history for whichever engine owns it now. So the fire path returns
/// on it immediately and says so at DEBUG. A store that has genuinely broken is
/// the opposite case and must keep its retry, which is why this predicate has a
/// negative control in `stand_down_is_not_a_fault`.
///
/// 🔴 THE DEADLINE LADDER WAS NEVER EXPOSED TO THIS, and the check that
/// established it is worth keeping written down. The suspicion — reasonable on
/// the face of it — was that a torn-down wheel would send a deadline through
/// all six bounded attempts and out the `tracing::error!` at the bottom, with
/// `deadline_remains_live` truthfully answering "still live" each time and so
/// sustaining the very loop it exists to bound. Driving it refuted the premise:
/// `fire_timer_guarded` demuxes a reserved `deadline:{run}` timer to
/// `fire_deadline` BEFORE the generic record-then-deliver path, so a deadline
/// never reaches the append boundary that raises this refusal. An ORDINARY
/// timer does, and that is the path the early return serves and the test
/// measures.
///
/// The classification is by variant, and the bound on that is worth stating:
/// on the fire path the bridge raises [`EngineSeamError::TimerWheel`] only from
/// the two `wheel_torn_down_*` constructors in this module, because
/// `fire_timer` never arms.
pub(super) fn is_wheel_teardown(error: &TimerServiceError) -> bool {
    matches!(
        error,
        TimerServiceError::Engine(EngineSeamError::TimerWheel { .. })
    )
}

/// Fire a due wheel timer, retrying a DEADLINE fire with bounded backoff while
/// its history timer remains live.
///
/// The live wheel is one-shot and production runs no periodic recovery tick, so
/// without this a transient timeout-teardown/fire failure would be dropped and
/// never re-driven in the same engine epoch. Only a reserved `deadline:{run}`
/// timer that is STILL live in durable history is retried; an ordinary timer's
/// fire failure — or a deadline already retired/superseded — is logged and
/// dropped exactly as before. The backoff interval grows and is capped (never a
/// hot loop), attempts are bounded, and the durable deadline row stays live so
/// restart recovery remains the final backstop.
///
/// A wheel teardown is not a failure this function can make progress against
/// and is returned on immediately — see [`is_wheel_teardown`], which also
/// records why the deadline ladder below was never the path at risk.
pub(super) async fn fire_wheel_timer(
    nif_state: &Weak<EngineNifState>,
    workflow_id: &WorkflowId,
    timer_id: &TimerId,
    fire_at: DateTime<Utc>,
) {
    const MAX_ATTEMPTS: u32 = 6;
    const INITIAL_BACKOFF: Duration = Duration::from_millis(200);
    const MAX_BACKOFF: Duration = Duration::from_secs(30);

    let mut backoff = INITIAL_BACKOFF;
    for attempt in 1..=MAX_ATTEMPTS {
        let Some(bridge) = nif_state
            .upgrade()
            .and_then(|state| timer_bridge(&state).ok())
        else {
            return;
        };
        let result = bridge
            .service()
            .fire_timer(workflow_id.clone(), timer_id.clone(), fire_at)
            .await;
        let Err(error) = result else {
            return;
        };
        // This engine has stood down. Retrying reads the same latched flag and
        // fails identically, and the run loses nothing: the timer is still live
        // in durable history for its owner to drive. DEBUG, not WARN — an
        // orderly shutdown is not a fault for anyone to investigate.
        if is_wheel_teardown(&error) {
            tracing::debug!(
                %workflow_id,
                %timer_id,
                "timer fire abandoned: this engine's wheel has been torn down and the timer stays live for its owner"
            );
            return;
        }
        if !crate::time::is_deadline_timer(timer_id) {
            tracing::warn!(error = %error, "timer wheel fire callback failed");
            return;
        }
        // A deadline fire failed. It stays eligible for a bounded retry unless we
        // can POSITIVELY confirm it is no longer live. A liveness-read error —
        // e.g. the same store outage that failed the fire — is UNCERTAIN, not a
        // reason to abandon the same-epoch drive: fall through to the backoff and
        // the next attempt, whose `fire_timer` performs its own fresh liveness
        // read and safely no-ops if another actor has since retired the deadline.
        match deadline_remains_live(&bridge, workflow_id, timer_id).await {
            Ok(false) => return,
            Ok(true) => tracing::warn!(
                error = %error,
                attempt,
                "workflow deadline fire failed while its timer is still live; retrying with backoff"
            ),
            Err(read_error) => tracing::warn!(
                error = %error,
                %read_error,
                attempt,
                "workflow deadline fire failed and its liveness could not be read (store outage?); treating as still-eligible and retrying with backoff"
            ),
        }
        if attempt == MAX_ATTEMPTS {
            break;
        }
        tokio::time::sleep(backoff).await;
        backoff = backoff.saturating_mul(2).min(MAX_BACKOFF);
    }
    tracing::error!(
        %workflow_id,
        %timer_id,
        "workflow deadline fire exhausted same-epoch retries; the durable timer stays live for restart recovery"
    );
}

/// Whether the reserved deadline timer `timer_id` is still live in durable
/// history (its run has not retired it), so a failed fire is worth retrying.
async fn deadline_remains_live(
    bridge: &TimerNifBridge,
    workflow_id: &WorkflowId,
    timer_id: &TimerId,
) -> Result<bool, StoreError> {
    let Some(run_id) = crate::time::deadline_run_id(timer_id) else {
        return Ok(false);
    };
    let history = bridge.store.read_history(workflow_id).await?;
    Ok(crate::time::outstanding_deadline_timer(&history, &run_id).is_some())
}

/// Whether the workflow's active run segment has already recorded a terminal.
///
/// The active run is the one opened by the latest `WorkflowStarted`; a timer
/// event that arrives after that run terminated is a late fire/cancel the bridge
/// refuses to append.
pub(super) fn active_run_has_terminal(history: &[Event]) -> bool {
    let Some(run_id) = history.iter().rev().find_map(|event| match event {
        Event::WorkflowStarted { run_id, .. } => Some(run_id.clone()),
        _ => None,
    }) else {
        return false;
    };
    crate::lifecycle::completion::terminal_outcome_from_history(history, &run_id).is_some()
}

/// The event kinds this bridge names in its "cannot record" refusal.
pub(super) fn event_kind(event: &Event) -> &'static str {
    match event {
        Event::TimerFired { .. } => "TimerFired",
        Event::TimerCancelled { .. } => "TimerCancelled",
        Event::WithTimeoutCompleted { .. } => "WithTimeoutCompleted",
        _ => "non-timer",
    }
}
