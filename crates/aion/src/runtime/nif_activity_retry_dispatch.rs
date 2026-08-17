//! Remote activity completion delivery and durable retry execution.

use std::sync::Arc;

use crate::activity::bridge::{ActivityDispatch, ActivityDispatcher};
use crate::durability::Recorder;

/// Spawn the completion task for one dispatched activity.
///
/// The task drives the dispatch to its FINAL outcome before waking the
/// workflow: a retryable-class failure (`retryable:` reason prefix — the
/// string form of the wire's structured `ActivityErrorKind`, see
/// [`super::nif_activity_retry`]) with budget left under the SDK-declared
/// retry policy is recorded durably as a non-terminal `ActivityFailed`
/// (kind `Retryable`), backed off, and re-dispatched with the SAME ordinal
/// and routing at the incremented attempt. Non-retryable failures, absent
/// policies (`"retry": null` — the SDK's run-exactly-once contract), and an
/// exhausted budget deliver to the workflow exactly as before, with the last
/// reason verbatim.
///
/// Every durable retry record is guarded against the settle races the
/// workflow thread can win mid-loop (a `with_timeout` expiry recording the
/// ordinal's terminal, a workflow terminal): the guard re-reads history under
/// the recorder lock and aborts the loop once the decision was made elsewhere.
/// The backoff sleep itself is task-local, not a durable timer: an engine
/// crash mid-backoff recovers through replay, whose dangling retryable
/// failure re-dispatches the activity live at the next attempt.
pub(super) fn spawn_completion_task(
    tokio_handle: &tokio::runtime::Handle,
    runtime: Arc<crate::RuntimeHandle>,
    dispatcher: Arc<dyn ActivityDispatcher>,
    seam: RetryRecorderSeam,
    workflow_pid: u64,
    correlation_id: String,
    request: ActivityDispatch,
) {
    let future = async move {
        let outcome = dispatch_with_retries(&dispatcher, &seam, &request).await;
        let attempt = outcome.attempt;
        match outcome.terminal {
            RetryLoopTerminal::Completed(payload) => {
                if let Err(error) = runtime.deliver_activity_completion_message_with_attempt(
                    workflow_pid,
                    &correlation_id,
                    payload,
                    Some(attempt),
                ) {
                    tracing::warn!(%error, workflow_pid, correlation_id, "activity completion delivery failed");
                }
            }
            RetryLoopTerminal::Failed(reason) => {
                if let Err(error) = runtime.deliver_activity_failure_message_with_attempt(
                    workflow_pid,
                    &correlation_id,
                    reason,
                    Some(attempt),
                ) {
                    tracing::warn!(%error, workflow_pid, correlation_id, "activity failure delivery failed");
                }
            }
            RetryLoopTerminal::SettledElsewhere => {
                // The awaited ordinal (or the whole workflow) reached a
                // recorded terminal while the loop ran — deliver nothing; the
                // workflow already took that branch.
                tracing::debug!(
                    workflow_id = %request.workflow_id,
                    activity_id = %request.activity_id,
                    attempt,
                    "activity retry loop stopped: the activity settled through another path"
                );
            }
            RetryLoopTerminal::Parked => {
                // The server parked this dispatch for restart recovery
                // (graceful drain, #207): record nothing, deliver nothing. The
                // durable log ends at the dangling scheduled/started trail —
                // byte-equivalent to a kill -9 — so post-restart replay
                // re-dispatches the activity live (cursor Exhausted →
                // ResumeLive), exactly the SettledElsewhere stand-down shape.
                tracing::debug!(
                    workflow_id = %request.workflow_id,
                    activity_id = %request.activity_id,
                    attempt,
                    "activity dispatch parked for restart recovery; retry loop stood down"
                );
            }
        }
    };
    tokio_handle.spawn(future);
}

/// The durable seam one completion task records retry attempts through: the
/// workflow's single-writer recorder plus the run the dispatch belongs to
/// (settlement is a per-run question — see [`record_retry_event`]).
pub(super) struct RetryRecorderSeam {
    /// The workflow's single-writer recorder, shared with the NIF contexts.
    pub(super) recorder: Arc<tokio::sync::Mutex<Recorder>>,
    /// The run this dispatch was issued by.
    pub(super) run_id: aion_core::RunId,
    /// The engine-task epoch this dispatch belongs to.
    ///
    /// 🔴 A FIELD, NOT AN ARGUMENT, DELIBERATELY. The completion task is the
    /// FOURTH durable writer this process can hold open, and until this field
    /// existed it was the one with no epoch check at all:
    /// [`spawn_completion_task`] discards its `JoinHandle`, so the task is
    /// detached on the host runtime and reached by neither `Drop for Engine`
    /// nor `Engine::shutdown` — and between attempts it sleeps an
    /// SDK-declared backoff with no ceiling. A released engine could therefore
    /// wake a minute later and append `ActivityStarted` into a history a
    /// successor has already adopted: `SequenceConflict` on the survivor at
    /// best, and a replay resolving a command against an event the live run
    /// never issued at worst. Invariant 3, and through it invariant 2.
    ///
    /// Carrying it on the SEAM rather than threading it through the three
    /// call sites means a future record path cannot be written without one.
    pub(super) engine_tasks: Arc<super::engine_tasks::EngineTaskRuntime>,
}

/// The retry loop's final disposition, carrying the attempt that produced it.
#[derive(Debug)]
pub(super) struct RetryLoopOutcome {
    pub(super) attempt: u32,
    pub(super) terminal: RetryLoopTerminal,
}

#[derive(Debug)]
pub(super) enum RetryLoopTerminal {
    /// The encoded output of the successful attempt.
    Completed(String),
    /// The last failure reason, verbatim (prefix included).
    Failed(String),
    /// A terminal for this ordinal/workflow was recorded by another path
    /// mid-loop; nothing may be delivered or recorded for it anymore.
    SettledElsewhere,
    /// The server parked the dispatch for restart recovery during a graceful
    /// drain (#207): nothing may be delivered or recorded — the workflow stays
    /// suspended and post-restart replay re-dispatches the dangling ordinal.
    Parked,
}

/// The shared stand-down for a record refused because this engine's task epoch
/// closed.
///
/// Named ONCE because both record sites in the retry loop take it, and a rule
/// spelled out in two places is held equal only by diligence.
///
/// Deliberately NOT reported as `Settled`: nothing was recorded by anyone.
/// This process simply stopped being the run's writer, and the owning engine
/// re-derives the activity from the durable trail — which ends here exactly as
/// a `kill -9` at this instant would leave it, the shape the parking path is
/// already built for.
fn stood_down_on_closed_epoch(request: &ActivityDispatch, attempt: u32) -> RetryLoopOutcome {
    tracing::info!(
        workflow_id = %request.workflow_id,
        activity_id = %request.activity_id,
        attempt,
        "engine task epoch closed mid-retry; standing down rather than writing for a run this \
         process no longer owns"
    );
    RetryLoopOutcome {
        attempt,
        terminal: RetryLoopTerminal::SettledElsewhere,
    }
}

/// Classify a failed dispatch BEFORE any durable retry record: the parked
/// sentinel stands the loop down (park beats retry, #207 — nothing recorded,
/// nothing delivered, no budget consumed; restart recovery re-dispatches the
/// dangling ordinal); a non-retryable class, an absent policy (`"retry":
/// null`), or an exhausted budget fails with the reason verbatim. `None`
/// means the loop retries under the policy.
///
/// `expired` says the loop itself ended the attempt on the authored
/// per-attempt bound (#223) rather than the worker reporting a reason. It is
/// passed rather than re-derived from `reason` deliberately: the expiry
/// reason carries the `timeout:` prefix, which is a NON-retryable class on the
/// wire because it types the SDK's `error.ActivityTimedOut`, and re-parsing
/// our own synthesized string to recover a fact we already hold is exactly the
/// drift this codebase keeps closing. The eligibility itself is read off the
/// language: `declarations/valid/timeout_inside_retry` states the bound is
/// "a per-attempt timeout [that] sits INSIDE a bounded retry schedule", so the
/// schedule takes an expiry while budget remains. Per-attempt has no meaning
/// otherwise.
fn failure_stand_down(
    policy: Option<&super::nif_activity_retry::RetryPolicy>,
    reason: &str,
    attempt: u32,
    expired: bool,
) -> Option<RetryLoopTerminal> {
    use super::nif_activity_retry::{is_parked_reason, is_retryable_reason};

    if is_parked_reason(reason) {
        return Some(RetryLoopTerminal::Parked);
    }
    let retry_eligible = expired || is_retryable_reason(reason);
    match policy {
        Some(policy) if retry_eligible && attempt < policy.max_attempts => None,
        Some(policy) if retry_eligible => Some(RetryLoopTerminal::Failed(format!(
            "exhausted: spent {attempt} of {} attempts; last refusal: {reason}",
            policy.max_attempts
        ))),
        _ => Some(RetryLoopTerminal::Failed(reason.to_owned())),
    }
}

/// The loop's success terminal for one attempt.
const fn dispatch_completed(attempt: u32, payload: String) -> RetryLoopOutcome {
    RetryLoopOutcome {
        attempt,
        terminal: RetryLoopTerminal::Completed(payload),
    }
}

/// Deliver ONE attempt, bounded by the authored per-attempt timeout when the
/// document declared one (#223).
///
/// `Err((reason, expired))` carries the failure reason and whether this loop
/// ended the attempt on the bound — see [`failure_stand_down`] for why that
/// fact travels rather than being re-read out of the reason string.
///
/// On expiry the dispatch future is DROPPED, which stops this run waiting and
/// nothing more. The default `dispatch_async` runs the dispatcher on
/// `spawn_blocking`, and a blocking task cannot be cancelled, so the
/// worker-side call runs on to its own end and its result is discarded. That
/// is the bound this word promises — the same one `with_timeout` gives a
/// `wait` — and reaching into the worker's child process is a cancellation
/// question, not this one.
async fn deliver_one_attempt(
    dispatcher: &Arc<dyn ActivityDispatcher>,
    delivery: ActivityDispatch,
    bound: Option<std::time::Duration>,
) -> Result<String, (String, bool)> {
    use super::nif_activity_retry::activity_timeout_reason;

    let dispatch = Arc::clone(dispatcher).dispatch_async(delivery);
    match bound {
        None => dispatch.await.map_err(|reason| (reason, false)),
        Some(bound) => match tokio::time::timeout(bound, dispatch).await {
            Ok(result) => result.map_err(|reason| (reason, false)),
            Err(_elapsed) => Err((activity_timeout_reason(bound), true)),
        },
    }
}

/// Drive one activity dispatch to its final outcome under the SDK-declared
/// retry policy and per-attempt timeout carried in the dispatch config
/// (#197, #223).
pub(super) async fn dispatch_with_retries(
    dispatcher: &Arc<dyn ActivityDispatcher>,
    seam: &RetryRecorderSeam,
    request: &ActivityDispatch,
) -> RetryLoopOutcome {
    use super::nif_activity_retry::{activity_timeout_from_config, retry_policy_from_config};

    let policy = retry_policy_from_config(&request.config);
    // #223: the AUTHORED per-attempt bound, or `None` for unbounded. The
    // engine imposes no deadline of its own — an activity whose document
    // declares no `timeout` waits exactly as long as it always has.
    let bound = activity_timeout_from_config(&request.config);
    let mut attempt = request.attempt;
    loop {
        let mut delivery = request.clone();
        delivery.attempt = attempt;
        let (reason, expired) = match deliver_one_attempt(dispatcher, delivery, bound).await {
            Ok(payload) => return dispatch_completed(attempt, payload),
            Err(failure) => failure,
        };
        // TRANSPORT-domain worker loss: the activity never executed to a result,
        // so it is re-dispatched at the SAME attempt with NOTHING recorded —
        // attempt-neutral. The prior behaviour delivered this as a terminal
        // failure whenever the activity carried no authored retry policy (the
        // SDK's run-exactly-once default), so every infrastructure death read to
        // the operator as a red action.
        //
        // The loop cannot spin: the re-dispatch goes through the same
        // unserved-queue park a first dispatch does, so it waits for a worker
        // rather than failing fast, and the server stops sending this class once
        // its transport-loss budget is spent (a `transport-exhausted:` reason
        // that no arm here recognises, so it falls through to the terminal
        // below). A terminal recorded elsewhere mid-loop still wins — checked
        // before every re-dispatch, because this arm records nothing and would
        // otherwise never notice.
        if super::nif_activity_retry::is_worker_lost_reason(&reason) {
            match worker_loss_stand_down(seam, request, &reason, attempt).await {
                Some(terminal) => return RetryLoopOutcome { attempt, terminal },
                None => continue,
            }
        }
        if let Some(terminal) = failure_stand_down(policy.as_ref(), &reason, attempt, expired) {
            if matches!(terminal, RetryLoopTerminal::Failed(_)) {
                record_advisory_exhaustion(seam, request, &reason, attempt).await;
            }
            return RetryLoopOutcome { attempt, terminal };
        }
        // The stand-down above returns `Failed` whenever no policy is present,
        // so a `None` here is structurally unreachable — kept as the honest
        // failure terminal rather than an unwrap.
        let Some(policy) = policy.as_ref() else {
            return RetryLoopOutcome {
                attempt,
                terminal: RetryLoopTerminal::Failed(reason),
            };
        };
        // Record the failed attempt durably as a NON-terminal (Retryable)
        // `ActivityFailed` — the observable retry record the history cursor
        // walks past to the eventual terminal. Recording failures abort the
        // loop into an honest terminal failure: an unrecorded retry is a
        // silent retry.
        match record_retry_event(
            seam,
            request,
            RetryRecord::AttemptFailed {
                attempt,
                reason: reason.clone(),
            },
        )
        .await
        {
            RetryRecordOutcome::Recorded => {}
            RetryRecordOutcome::Settled => {
                return RetryLoopOutcome {
                    attempt,
                    terminal: RetryLoopTerminal::SettledElsewhere,
                };
            }
            RetryRecordOutcome::EpochClosed => {
                return stood_down_on_closed_epoch(request, attempt);
            }
            RetryRecordOutcome::RecordFailed(record_error) => {
                tracing::warn!(
                    workflow_id = %request.workflow_id,
                    activity_id = %request.activity_id,
                    attempt,
                    error = %record_error,
                    "failed to record a retryable activity failure; failing the activity instead \
                     of retrying unrecorded"
                );
                return RetryLoopOutcome {
                    attempt,
                    terminal: RetryLoopTerminal::Failed(reason),
                };
            }
        }
        announce_and_back_off(policy, request, &reason, attempt).await;
        attempt += 1;
        // Record the retry delivery's `ActivityStarted` before it goes on the
        // wire, so history and the worker wire agree on the attempt (NOI-0) —
        // re-guarded, because the backoff sleep is a settle-race window.
        match record_retry_event(seam, request, RetryRecord::AttemptStarted { attempt }).await {
            RetryRecordOutcome::Recorded => {}
            RetryRecordOutcome::Settled => {
                return RetryLoopOutcome {
                    attempt,
                    terminal: RetryLoopTerminal::SettledElsewhere,
                };
            }
            RetryRecordOutcome::EpochClosed => {
                return stood_down_on_closed_epoch(request, attempt);
            }
            RetryRecordOutcome::RecordFailed(record_error) => {
                tracing::warn!(
                    workflow_id = %request.workflow_id,
                    activity_id = %request.activity_id,
                    attempt,
                    error = %record_error,
                    "failed to record a retry attempt start; failing the activity instead of \
                     dispatching unrecorded"
                );
                return RetryLoopOutcome {
                    attempt,
                    terminal: RetryLoopTerminal::Failed(reason),
                };
            }
        }
    }
}

/// Announce a retryable action failure and sleep out its backoff before the next
/// attempt goes on the wire.
///
/// Split out of the loop body purely for size; the sleep is deliberately
/// task-local rather than a durable timer (an engine crash mid-backoff recovers
/// through replay, whose dangling retryable failure re-dispatches live).
async fn announce_and_back_off(
    policy: &super::nif_activity_retry::RetryPolicy,
    request: &ActivityDispatch,
    reason: &str,
    attempt: u32,
) {
    let delay = policy.backoff.delay_after(attempt);
    tracing::warn!(
        workflow_id = %request.workflow_id,
        activity_id = %request.activity_id,
        activity_type = %request.name,
        attempt,
        max_attempts = policy.max_attempts,
        retry_in_ms = u64::try_from(delay.as_millis()).unwrap_or(u64::MAX),
        reason = %reason,
        "activity attempt failed with a retryable error; re-dispatching"
    );
    tokio::time::sleep(delay).await;
}

/// Decide what an attempt-neutral worker-loss re-dispatch does: stand the loop
/// down when the ordinal was settled elsewhere, or `None` to re-dispatch the
/// SAME attempt, recording nothing.
///
/// The settlement read is load-bearing precisely BECAUSE this path records
/// nothing: every other arm of the loop rides `record_retry_event`'s settlement
/// check, and without this one a re-dispatch could outlive a `with_timeout`
/// expiry (or a workflow terminal) that already decided the ordinal.
async fn worker_loss_stand_down(
    seam: &RetryRecorderSeam,
    request: &ActivityDispatch,
    reason: &str,
    attempt: u32,
) -> Option<RetryLoopTerminal> {
    if activity_settled_elsewhere(seam, request).await {
        return Some(RetryLoopTerminal::SettledElsewhere);
    }
    tracing::warn!(
        workflow_id = %request.workflow_id,
        activity_id = %request.activity_id,
        activity_type = %request.name,
        attempt,
        reason = %reason,
        "activity's worker was lost before it reported a result; re-dispatching the same \
         attempt (transport loss consumes no authored retry budget)"
    );
    None
}

/// Whether this ordinal (within its run) already carries a recorded terminal.
///
/// The attempt-neutral worker-loss re-dispatch records NOTHING, so it has no
/// `record_retry_event` settle check to ride on; without this read it could
/// re-dispatch an ordinal a `with_timeout` expiry (or a workflow terminal)
/// already settled. A read failure answers `false` — the loop then re-dispatches
/// and the recorder's own guards still refuse a post-terminal append, which is
/// strictly safer than standing an activity down on an unreadable history.
async fn activity_settled_elsewhere(seam: &RetryRecorderSeam, request: &ActivityDispatch) -> bool {
    let recorder = seam.recorder.lock().await;
    let Ok(history) = recorder.read_history().await else {
        return false;
    };
    let Ok(history) = crate::durability::current_run_segment(history, &seam.run_id) else {
        return false;
    };
    super::nif_activity_retry::activity_settled(&history, &request.activity_id)
}

/// Record the R5 warning when an ADVISORY activity's attempt budget is spent
/// (RUNTIME-OPERATIONS.md R5).
///
/// "Retry exhaustion" is read as the attempt budget being SPENT, whatever its
/// size: an advisory action with no declared retry has a budget of one, and its
/// single failure exhausts it exactly as a declared `retry 5` exhausts five.
/// The warning therefore fires on every terminal failure of an advisory
/// dispatch, and never on a park or a settled-elsewhere stand-down (neither is
/// a failure, and both must record nothing).
///
/// Ordering: the warning lands JUST BEFORE the failure is delivered, so it
/// precedes the terminal `ActivityFailed` the workflow thread records on
/// receipt. Both are in history; the warning never replaces the failure.
///
/// A recording failure here is logged and swallowed — deliberately. The
/// activity's own honest terminal is what the workflow acts on; losing the
/// warning must not also change the outcome the run gets.
async fn record_advisory_exhaustion(
    seam: &RetryRecorderSeam,
    request: &ActivityDispatch,
    reason: &str,
    attempt: u32,
) {
    if !request.advisory {
        return;
    }
    match record_retry_event(
        seam,
        request,
        RetryRecord::AdvisoryExhausted {
            attempt,
            reason: reason.to_owned(),
        },
    )
    .await
    {
        RetryRecordOutcome::Recorded | RetryRecordOutcome::Settled => {}
        // Advisory only, so there is nothing to fail — but it is still not the
        // same event as `Settled`, and a reader of these logs must be able to
        // tell "somebody else finished it" from "we stopped being the writer".
        RetryRecordOutcome::EpochClosed => {
            tracing::info!(
                workflow_id = %request.workflow_id,
                activity_id = %request.activity_id,
                attempt,
                "engine task epoch closed before the advisory exhaustion note could be recorded"
            );
        }
        RetryRecordOutcome::RecordFailed(record_error) => {
            tracing::warn!(
                workflow_id = %request.workflow_id,
                activity_id = %request.activity_id,
                attempt,
                error = %record_error,
                "failed to record the advisory-exhaustion warning; the activity's terminal \
                 failure still stands"
            );
        }
    }
}

/// One durable retry record the loop appends between attempts.
enum RetryRecord {
    /// The just-failed attempt's non-terminal `ActivityFailed`.
    AttemptFailed { attempt: u32, reason: String },
    /// The next delivery's `ActivityStarted`.
    AttemptStarted { attempt: u32 },
    /// The R5 warning that an ADVISORY activity spent its attempt budget.
    AdvisoryExhausted { attempt: u32, reason: String },
}

enum RetryRecordOutcome {
    Recorded,
    /// The ordinal (or workflow) already has a recorded terminal; the loop
    /// must stop without recording or delivering anything further.
    Settled,
    /// This engine's task epoch closed before the append could land, so this
    /// process is no longer the run's single writer. Distinct from [`Settled`]
    /// on purpose: nothing was recorded by anyone, the run is simply not ours
    /// to write for any more, and the two must not be reported as one thing.
    EpochClosed,
    RecordFailed(crate::durability::DurabilityError),
}

/// Append one retry record under the recorder lock, re-checking settlement
/// first so the append can never land after a terminal recorded by the
/// workflow thread (`with_timeout` expiry, workflow terminal).
async fn record_retry_event(
    seam: &RetryRecorderSeam,
    request: &ActivityDispatch,
    record: RetryRecord,
) -> RetryRecordOutcome {
    let mut recorder = seam.recorder.lock().await;
    // 🔴 THE EPOCH GATE, UNDER THE RECORDER LOCK. This is the last point at
    // which this process can still decline to be a second writer: after it,
    // the append is issued. Checking here rather than at spawn or between
    // attempts is the same argument `EngineTaskRuntime::is_epoch_open` makes
    // about itself — an earlier check can be walked past, because the attempt
    // that passed it goes on to hold the engine open across a history read, a
    // lock acquisition and a store round-trip before it writes anything.
    //
    // Refusing is the safe direction: nothing is lost that the owning engine
    // cannot re-derive. The activity's durable trail simply ends where this
    // process stopped being its writer, which is byte-equivalent to a
    // `kill -9` at the same instant — the shape the parking path below is
    // already built for.
    if !seam.engine_tasks.is_epoch_open() {
        return RetryRecordOutcome::EpochClosed;
    }
    let history = match recorder.read_history().await {
        Ok(history) => history,
        Err(error) => return RetryRecordOutcome::RecordFailed(error),
    };
    // Settlement is a per-run question: scope to the current run's segment so
    // a prior run's terminal (continue-as-new) never aborts this run's loop.
    let history = match crate::durability::current_run_segment(history, &seam.run_id) {
        Ok(history) => history,
        Err(error) => return RetryRecordOutcome::RecordFailed(error),
    };
    if super::nif_activity_retry::activity_settled(&history, &request.activity_id) {
        return RetryRecordOutcome::Settled;
    }
    let append_result = match record {
        RetryRecord::AttemptFailed { attempt, reason } => {
            recorder
                .record_activity_failed(
                    chrono::Utc::now(),
                    request.activity_id.clone(),
                    aion_core::ActivityError {
                        kind: aion_core::ActivityErrorKind::Retryable,
                        message: reason,
                        details: None,
                    },
                    attempt,
                )
                .await
        }
        RetryRecord::AttemptStarted { attempt } => {
            recorder
                .record_activity_started(chrono::Utc::now(), request.activity_id.clone(), attempt)
                .await
        }
        RetryRecord::AdvisoryExhausted { attempt, reason } => {
            recorder
                .record_activity_advisory_exhausted(
                    chrono::Utc::now(),
                    request.activity_id.clone(),
                    request.name.clone(),
                    reason,
                    attempt,
                )
                .await
        }
    };
    match append_result {
        Ok(()) => RetryRecordOutcome::Recorded,
        Err(error) => RetryRecordOutcome::RecordFailed(error),
    }
}
