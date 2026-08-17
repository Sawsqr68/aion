//! Retrying one run's completion until its terminal lands.
//!
//! The unit of work — one attempt at recording a terminal and the bookkeeping
//! that follows it — lives in [`super::completion`]. This module owns everything
//! about REPEATING that attempt: the normalized intent an attempt is re-run
//! from, what an attempt's failure means, whether a failure is worth repeating,
//! the armed background task, and the loop's own speech policy.
//!
//! The two modules reference each other, which is the honest shape of a retry
//! loop around a unit of work rather than a sign the seam is wrong: the loop
//! must call the attempt, and the attempt's entry point must be able to arm the
//! loop.

use std::sync::Arc;

use aion_core::{Payload, WorkflowError};
use aion_store::EventStore;
use aion_store::visibility::VisibilityStore;
use chrono::{DateTime, Utc};
use tokio::runtime::Handle;

use crate::EngineError;
use crate::loader::WorkflowCatalog;
use crate::registry::{Registry, WorkflowHandle};
use crate::runtime::engine_tasks::{ArmOutcome, CompletionRetryKey};
use crate::runtime::{RuntimeHandle, WorkflowProcessOutcome};
use crate::supervision::SupervisionTree;

use super::completion::{ProcessExitContext, handle_process_exit_attempt};

/// What terminal this exit means, normalized so a retry can re-attempt it.
///
/// The monitor is handed `Result<WorkflowProcessOutcome, EngineError>`, and the
/// `Err` arm has always been folded into a recorded `WorkflowFailed` — so the
/// fold happens ONCE, up front, rather than inside an attempt that may run more
/// than once. That also keeps the retry independent of whether `EngineError` is
/// cloneable, which it is not.
#[derive(Clone, Debug)]
pub(super) struct TerminalIntent {
    /// What terminal this exit means.
    pub(super) outcome: TerminalIntentOutcome,
    /// When the workflow process was observed to exit.
    ///
    /// 🔴 Captured **once**, here, and reused by every attempt — including
    /// retries that may land minutes or hours later.
    ///
    /// Stamping `Utc::now()` at the append instead would make a retried
    /// terminal's `recorded_at` — and therefore `close_time` in the visibility
    /// projection — the moment the *store came back*, not the moment the run
    /// finished. Under a forty-minute outage the durable close time of every
    /// affected run would silently drift by forty minutes. That is not a
    /// determinism violation, because the value is recorded and replay reads it
    /// back; it is worse in a different way. It is a wrong fact, recorded
    /// confidently, about when work finished — and for the financial, legal and
    /// clinical settings this engine is built for, a close time that moves with
    /// an unrelated infrastructure fault is a data-fidelity regression that the
    /// retry itself would have introduced.
    ///
    /// The exit instant is known here for free. Carrying it costs one field.
    ///
    /// # What reusing it costs, and who was checked
    ///
    /// Reuse means a terminal's `recorded_at` can be arbitrarily earlier than
    /// the wall-clock instant it was appended, so **`recorded_at` is not a proxy
    /// for append order** — two runs' terminals can be stamped in the reverse of
    /// the order they were written. `sequence` is the ordering, and it is
    /// unaffected. The consumers were surveyed rather than assumed:
    ///
    /// - **`close_time`** is `terminal_recorded_at`, which is this value by
    ///   design: the run really did finish then. That is the point of carrying
    ///   it, not a side effect.
    /// - **The determinism clock.** Two earlier revisions of this bullet are
    ///   retracted, and the history matters more than the answer.
    ///
    ///   The FIRST argued the clock was safe because `workflow.now` "advances
    ///   only in `Replay::step`, on commands resolved against recorded
    ///   history". False: `Replay::step` was the *designed* clock, and
    ///   production `workflow.now` did not go through it. The survey had
    ///   checked the implementation production does not use.
    ///
    ///   The SECOND re-derived the same conclusion against what production did
    ///   use — `nif_determinism::now_from_context` reading
    ///   `NifContext::last_recorded_at`, i.e. `history.last()`, the run
    ///   segment's TAIL — and argued a reused `exit_time` was unreachable
    ///   because nothing executes while its own terminal is the tail, and the
    ///   one path back (`reopen`) appends later-stamped events first. That
    ///   reasoning was sound about the tail read, but the tail read was itself
    ///   the defect (aion#1: on a resumed run the tail is the run's own
    ///   future), and it has been removed. The bullet was written knowing it
    ///   would not survive the fix, and it has not.
    ///
    ///   Re-derived against the mechanism production uses NOW, the conclusion
    ///   holds for a stronger and simpler reason. `workflow.now` is the replay
    ///   POSITION ([`crate::runtime::nif_context::NifContext::workflow_now`]):
    ///   it is seeded from the run's own `WorkflowStarted` and advances ONLY to
    ///   the `recorded_at` of an outcome the workflow has CONSUMED — a
    ///   resolution the resolver returned, or an event a seam recorded and
    ///   returned in the same step. A workflow terminal is neither: the
    ///   resolver rejects every workflow-terminal event as a history-shape
    ///   fault rather than resolving it as a recorded command outcome, and no
    ///   `observe_recorded_at` seam records one. So a reused `exit_time` is not
    ///   a value the position cell can ever hold, on any path, live or
    ///   replayed — the reopen ordering argument is no longer load-bearing.
    ///
    ///   Two further checks, since a reused `exit_time` is non-monotonic by
    ///   construction: `HistoryCursor` orders on `seq`, not `recorded_at`, so a
    ///   backwards timestamp cannot make a history unreplayable; and the
    ///   position cell advances by `fetch_max` over a floor of the run's own
    ///   `WorkflowStarted`, so even a backwards `recorded_at` reaching it could
    ///   not move workflow-visible time backwards.
    ///
    /// No consumer was found that treats `recorded_at` as append order. That is
    /// a survey of today's callers, not a guarantee about future ones — anything
    /// added later that sorts history by time rather than by `sequence` inherits
    /// this and would be wrong.
    pub(super) exit_time: DateTime<Utc>,
}

/// The terminal an observed exit normalizes to.
#[derive(Clone, Debug)]
pub(super) enum TerminalIntentOutcome {
    /// The process exited normally with this result.
    Completed(Payload),
    /// The process failed, or the monitor itself could not read the exit.
    Failed(WorkflowError),
}

impl TerminalIntent {
    pub(super) fn from_outcome(outcome: Result<WorkflowProcessOutcome, EngineError>) -> Self {
        let observed = match outcome {
            Ok(WorkflowProcessOutcome::Completed(result)) => {
                TerminalIntentOutcome::Completed(result)
            }
            Ok(WorkflowProcessOutcome::Failed(error)) => TerminalIntentOutcome::Failed(error),
            Err(error) => TerminalIntentOutcome::Failed(WorkflowError {
                message: format!("workflow process monitor failed: {error}"),
                details: None,
            }),
        };
        Self {
            outcome: observed,
            exit_time: Utc::now(),
        }
    }
}

/// Why one completion attempt did not finish.
///
/// The split is the same one `nif_child_watch` makes, and for the same reason:
/// a store that is briefly unavailable is a different fact from a logic error,
/// and retrying the second forever is a busy loop that never converges.
#[derive(Debug)]
pub(super) enum CompletionFailure {
    /// A durable read or write failed. The terminal check makes the whole
    /// attempt idempotent, so re-running it is safe: an attempt that finds its
    /// own terminal already recorded takes the resume path instead of appending
    /// a second one.
    Retryable(EngineError, TerminalProgress),
    /// Nothing a retry can repair — reported to the caller as before.
    Invariant(EngineError, TerminalProgress),
}

/// Whether the run's terminal event was durably appended before a failure.
///
/// Carried because an attempt is a pipeline, not a single write: the append is
/// followed by a deadline retirement, a visibility upsert and a registry
/// reconcile, and every one of those can fail on its own. Without this the
/// operator-facing messages had no way to distinguish "the terminal never
/// landed" from "the terminal landed and the bookkeeping after it did not", so
/// they asserted the first in both cases — telling an operator a `Completed`
/// run "stays Running", which sends them hunting a wedge that does not exist.
///
/// A status is a projection of history (invariant 4), so what this variant
/// records is that a terminal event for the run WAS OBSERVED DURABLE — not that
/// the run is terminal now and forever. 🔴 The stronger claim was written here
/// first and is false: `ContinuedAsNew` and a reopen both put further events
/// after a terminal one, and the projection then answers with the LATER state.
/// The distinction this variant exists to draw survives that correction intact —
/// "the terminal never landed" against "the terminal landed and the bookkeeping
/// after it did not" is a fact about this attempt, and an attempt cannot be
/// retroactively un-appended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TerminalProgress {
    /// The failure happened before any terminal event reached the store, so the
    /// run is still `Running` and its terminal is still owed.
    NotRecorded,
    /// A terminal event for this run is durably in history — either appended by
    /// this attempt, or found already present by its terminal check. What failed
    /// is the bookkeeping that follows it.
    Recorded,
}

impl TerminalProgress {
    /// How to describe this state to an operator, as a clause that completes
    /// "workflow completion ...".
    fn operator_summary(self) -> &'static str {
        match self {
            Self::NotRecorded => "could not be recorded",
            Self::Recorded => "was recorded, but the bookkeeping that follows it did not complete",
        }
    }
}

/// The retry's own view of the engine, which holds the runtime **weakly**.
///
/// A completion retry runs on an executor that `RuntimeHandle` owns. If the
/// task also held that handle strongly the ownership graph would be cyclic —
/// executor → task → handle → executor — so this task would keep the handle,
/// and therefore its own executor, alive for as long as it ran. A task must not
/// be able to pin the thing that is supposed to be able to stop it.
///
/// Holding it weakly also gives the loop a principled end: when the engine is
/// gone there is no longer a single writer to record through, so continuing
/// would be the second-writer hazard (invariant 3) rather than a repair.
///
/// 🔴 It does NOT make `RuntimeHandle::drop` reachable on the engine-released-
/// without-shutdown path, and an earlier revision of this comment claimed it
/// did. That path already has a cycle this task plays no part in:
/// `ChildNifBridge` and `SignalNifBridge` each hold `Arc<RuntimeHandle>`
/// (`runtime/nif_child_engine.rs`) and are installed unconditionally into the
/// NIF state the handle itself owns, and only `EngineNifState::
/// clear_engine_seams` breaks it — which runs from `Engine::shutdown` alone.
/// So on that exact path the refcount never reaches zero whatever this task
/// does, and `upgrade` never returns `None`. What actually stops a retry when
/// an engine is released is `Drop for Engine`, which gates and aborts the
/// engine-task epoch directly rather than relying on any refcount.
pub(super) struct CompletionRetryContext {
    store: Arc<dyn EventStore>,
    visibility_store: Arc<dyn VisibilityStore>,
    registry: Arc<Registry>,
    catalog: Arc<WorkflowCatalog>,
    runtime: std::sync::Weak<RuntimeHandle>,
    supervision: Arc<SupervisionTree>,
    tokio_handle: Handle,
    search_attribute_schema: Arc<aion_core::SearchAttributeSchema>,
}

impl CompletionRetryContext {
    /// Visible to the rest of `lifecycle` so a proof whose subject is what the
    /// retry LOOP says can build the loop's own argument and call it directly.
    /// A thread-scoped log capture cannot see an emission made on the
    /// engine-task executor, so such a proof cannot go through the production
    /// spawn — see `crate::log_capture`.
    pub(super) fn downgrade(context: ProcessExitContext) -> Self {
        let ProcessExitContext {
            store,
            visibility_store,
            registry,
            catalog,
            runtime,
            supervision,
            tokio_handle,
            search_attribute_schema,
        } = context;
        Self {
            store,
            visibility_store,
            registry,
            catalog,
            runtime: Arc::downgrade(&runtime),
            supervision,
            tokio_handle,
            search_attribute_schema,
        }
    }

    /// Rebuild the attempt's context, or `None` once the engine is released.
    fn upgrade(&self) -> Option<ProcessExitContext> {
        Some(ProcessExitContext {
            store: Arc::clone(&self.store),
            visibility_store: Arc::clone(&self.visibility_store),
            registry: Arc::clone(&self.registry),
            catalog: Arc::clone(&self.catalog),
            runtime: self.runtime.upgrade()?,
            supervision: Arc::clone(&self.supervision),
            tokio_handle: self.tokio_handle.clone(),
            search_attribute_schema: Arc::clone(&self.search_attribute_schema),
        })
    }
}

/// Releases a run's retry registration however the task ends.
///
/// The removal used to be the task's last statement, so a panic anywhere in the
/// loop left the entry behind with no log at all, while
/// `armed_completion_retry_count` went on reporting the run as owned — a
/// stranded slot that nothing would ever reclaim, because the replace-on-
/// `is_finished` path only fires if something arms that same key again, and
/// nothing will. A guard runs on return, on unwind, and on cancellation.
///
/// The executor is held weakly for the same reason the runtime is: a guard
/// living inside a task must not keep its own executor alive.
///
/// 🔴 A SLOT IS CLAIMED BY A RUNNING TASK, NEVER BY A FUTURE THAT MIGHT NOT RUN.
///
/// [`Self::claim`] must be called from inside the spawned body, and
/// [`arm_completion_retry`] is the only caller. Building the guard where the
/// future is CONSTRUCTED instead was a live defect: `EngineTaskRuntime::arm`
/// takes the future by value and returns without spawning on both refusal
/// paths (`AlreadyArmed`, `EpochClosed`), so the un-polled future was dropped —
/// and with it a guard that then released the registration belonging to the
/// task still running. A second arm for a run therefore DELETED the first
/// retry's entry, and a third spawned a second writer of that run's terminal.
/// Statements inside an `async` block do not execute until the future is
/// polled, so claiming here means a refused arm claims nothing.
pub(super) struct CompletionRetrySlot {
    tasks: std::sync::Weak<crate::runtime::engine_tasks::EngineTaskRuntime>,
    lease: CompletionRetryKey,
    /// This task's own id, compared against the registered handle's on release
    /// so a finishing task can never evict its successor's registration.
    task: tokio::task::Id,
}

impl CompletionRetrySlot {
    /// Claim the registration for the task that is running right now.
    fn claim(
        tasks: std::sync::Weak<crate::runtime::engine_tasks::EngineTaskRuntime>,
        lease: CompletionRetryKey,
    ) -> Option<Self> {
        // `try_id` rather than `id`: the latter panics off-task, and this crate
        // does not panic in library code. `None` is unreachable from the only
        // call site — an `async` body only runs inside a spawned task — so it is
        // reported rather than swallowed. Declining to claim is the safe
        // direction: the entry is then reclaimed by the replace-on-`is_finished`
        // path or by the epoch close, whereas claiming without an identity
        // would restore the eviction race this type exists to prevent.
        let Some(task) = tokio::task::try_id() else {
            tracing::error!(
                workflow_id = %lease.workflow_id,
                run_id = %lease.run_id,
                monitor_pid = lease.monitor_pid,
                "completion retry could not read its own task id; its registry entry will be \
                 reclaimed by the next arm for this lease or by the epoch close rather than on \
                 completion"
            );
            return None;
        };
        Some(Self { tasks, lease, task })
    }
}

impl Drop for CompletionRetrySlot {
    fn drop(&mut self) {
        if let Some(tasks) = self.tasks.upgrade() {
            tasks.remove_completion_retry(&self.lease, self.task);
        }
    }
}

/// Arm the durable retry for a monitor lease whose terminal did not land.
///
/// Keyed by [`CompletionRetryKey`] — workflow, run **and monitor pid** — all
/// read off the handle, so the registry is bounded by the leases this node is
/// actually monitoring and arming twice for one lease is refused rather than
/// racing. 🔴 The pid is load-bearing: keyed by run alone, a reopened run's
/// successor lease was refused by its own superseded predecessor's retry, which
/// then stood down without writing. See [`CompletionRetryKey`].
pub(super) fn arm_completion_retry(
    context: ProcessExitContext,
    handle: WorkflowHandle,
    intent: TerminalIntent,
    cause: &EngineError,
    progress: TerminalProgress,
) {
    let run = CompletionRetryKey {
        workflow_id: handle.workflow_id().clone(),
        run_id: handle.run_id().clone(),
        monitor_pid: handle.pid(),
    };
    let tasks = context.runtime.engine_tasks();
    let backoff = context.runtime.completion_retry();
    let armed = {
        let weak_tasks = Arc::downgrade(&tasks);
        let slot_run = run.clone();
        let retry_context = CompletionRetryContext::downgrade(context);
        tasks.arm_completion_retry(run.clone(), async move {
            // Claimed HERE, on first poll — see `CompletionRetrySlot`. Bound to
            // the body, so it is released on every exit path including panic
            // and abort, and released by nothing if this future is dropped
            // un-polled because the arm was refused.
            let _slot = CompletionRetrySlot::claim(weak_tasks, slot_run);
            retry_process_exit(&retry_context, &handle, &intent, backoff, progress).await;
        })
    };
    // 🔴 Every line below reports what was OBSERVED, never what was assumed. An
    // attempt can fail after its terminal event is durably in history — the
    // deadline retirement, the visibility upsert and the registry reconcile all
    // follow the append and all have their own failure modes — and a message
    // that says "could not be recorded" in that case sends an operator hunting a
    // wedge that does not exist. Worse, on the unretryable path it tells them a
    // `Completed` run "stays Running", which is a false durable fact about the
    // one thing this feature exists to get right.
    match armed {
        ArmOutcome::Armed => {
            tracing::warn!(
                workflow_id = %run.workflow_id,
                run_id = %run.run_id,
                monitor_pid = run.monitor_pid,
                error = %cause,
                terminal_recorded = matches!(progress, TerminalProgress::Recorded),
                "workflow completion {}; retrying durably in the background",
                progress.operator_summary()
            );
            return;
        }
        // 🔴 A retry for this exact run is ALREADY in flight. The terminal is
        // owned, and the sentences below — which say no retry was armed and the
        // run stays Running until a monitor is re-installed — would be false
        // about it. This arm exists because those two facts were once collapsed
        // into one `bool`: the log could not tell "nobody owns this" from
        // "somebody already does", so it said the alarming one for both.
        //
        // Not silent, because a *second* exit for a run whose terminal is
        // already being retried is itself worth seeing — it is how a double
        // process-exit would show up — but at `debug`, and saying what is
        // actually true.
        ArmOutcome::AlreadyArmed => {
            tracing::debug!(
                workflow_id = %run.workflow_id,
                run_id = %run.run_id,
                monitor_pid = run.monitor_pid,
                error = %cause,
                terminal_recorded = matches!(progress, TerminalProgress::Recorded),
                "workflow completion {}; a durable retry for this run is already in \
                 flight and owns the terminal, so this exit arms nothing",
                progress.operator_summary()
            );
            return;
        }
        ArmOutcome::EpochClosed => {}
    }
    // The epoch is closing: nothing was spawned and nothing in this process will
    // spawn it. The startup sweep re-installs a monitor on the successor, and
    // until it does the facts below are what an operator is looking at. This is
    // the case that must never be silent.
    match progress {
        TerminalProgress::NotRecorded => tracing::warn!(
            workflow_id = %run.workflow_id,
            run_id = %run.run_id,
            monitor_pid = run.monitor_pid,
            error = %cause,
            terminal_recorded = false,
            "workflow completion could not be recorded and no retry was armed; the run \
             stays Running until a monitor is re-installed"
        ),
        TerminalProgress::Recorded => tracing::warn!(
            workflow_id = %run.workflow_id,
            run_id = %run.run_id,
            monitor_pid = run.monitor_pid,
            error = %cause,
            terminal_recorded = true,
            "workflow completion was recorded and no retry was armed; the run is terminal \
             and its status projection is correct, but the bookkeeping that follows the \
             terminal — deadline retirement, visibility upsert, registry reconcile — did \
             not complete, so a derived index may lag until recovery re-runs it"
        ),
    }
}

/// When an unbounded retry loop should re-state its fault to the operator.
///
/// Two rules, and the loop needs both, because either alone fails:
///
/// - **On change.** Re-stating an unchanged fact once per sleep emits hundreds
///   of identical lines a second for the whole outage — the #94 shape. So a
///   fault is stated when it changes. What it is keyed on has to be a TYPED
///   discriminant, never the rendered error; see [`failure_discriminant`] for
///   why a `Display` string cannot answer "did this change in substance".
/// - **On escalation.** With a transition-only rule and no attempt budget, a run
///   whose completion never lands emits exactly one line at t=0 and nothing
///   afterwards: six hours stuck reads identically to three milliseconds
///   succeeded, which is a silent failure with a warm CPU. So the fault is
///   re-stated whenever the attempt count reaches the next power of two.
///
/// That cadence is DERIVED from the attempt counter, not chosen: it invents no
/// interval, no cap and no budget, and it is self-damping — frequent while an
/// outage is young, rare once it is old, and never silent.
///
/// Extracted from the loop body rather than left inline so the rule can be
/// stated once and measured directly. Inline, its only observable was tracing
/// output, and a rule whose sole witness is a log line is a rule nothing holds.
pub(super) struct FaultReporter {
    /// Every typed discriminant stated so far on this run.
    ///
    /// A set rather than a single "last stated" value, because the rule is
    /// "state each fault the first time it is seen", not "state it whenever it
    /// differs from last time" — the latter is satisfied trivially by a store
    /// alternating between two faults, which is how a transition rule becomes a
    /// flood. Bounded by the number of discriminants in
    /// [`failure_discriminant`], which is a fixed, small catalogue.
    seen: Vec<&'static str>,
    /// The attempt number at which an already-seen fault is stated again.
    report_at_attempt: u64,
}

impl FaultReporter {
    pub(super) fn new() -> Self {
        Self {
            seen: Vec::new(),
            // The first attempt always reports: at that point nothing is known
            // to the operator at all.
            report_at_attempt: 1,
        }
    }

    /// Record this attempt's fault and answer whether it must be stated.
    pub(super) fn should_report(&mut self, fault: &'static str, attempts: u64) -> bool {
        let unseen = !self.seen.contains(&fault);
        let escalated = attempts >= self.report_at_attempt;
        // 🔴 A CHANGE ALONE IS NOT A REASON TO SPEAK, AND THE FIRST CUT SAID IT
        // WAS. There are exactly two transient discriminants, so a store that
        // alternates between them makes `changed` true on EVERY attempt — and an
        // unbounded loop with no rate limit on the change path emits one warn
        // per attempt for the whole outage. That is the #94 flood wearing the
        // transition rule's name: the rule that was supposed to close it becomes
        // the thing that reopens it, because "different from last time" is
        // trivially satisfiable by flapping.
        //
        // A fault is therefore stated when it is genuinely NEW — never seen on
        // this run, which can happen at most once per discriminant — or when the
        // attempt ladder says an old one is due again. Alternation now reports
        // each fault once and then falls back to the ladder, so the operator
        // learns both faults exist without the flood.
        if !unseen && !escalated {
            return false;
        }
        if unseen {
            // Guarded, not unconditional: an escalation re-states a fault that is
            // already in the set, and pushing it again would grow the vector once
            // per escalation for the life of the outage. The bound stated on
            // `seen` — one entry per discriminant — is only true if the insert is
            // the one that establishes it.
            self.seen.push(fault);
        }
        // Only escalation moves the ladder. A fault that was merely NEW must not
        // push the next scheduled re-statement further out, or a store cycling
        // through fault kinds would go quiet by introducing them one at a time.
        if escalated {
            self.report_at_attempt = attempts.saturating_mul(2);
        }
        true
    }
}

/// Retry one run's completion until it lands, the run is observed terminal, the
/// run is observed reopened, the engine is released, the epoch closes underneath
/// the task, **or an unretryable failure is hit** — that last outcome abandons
/// the run with its terminal possibly still unrecorded, and is the one an
/// operator most needs to know can happen.
///
/// Unbounded by attempt count on purpose: there is no attempt budget at which
/// abandoning a finished run's terminal becomes the right answer, and the epoch
/// close is what ends this task. The interval comes from the builder-supplied
/// [`crate::runtime::CompletionRetryConfig`] — its own policy, not the signal
/// ladder it used to borrow, and nothing here invents a duration.
pub(super) async fn retry_process_exit(
    context: &CompletionRetryContext,
    handle: &WorkflowHandle,
    intent: &TerminalIntent,
    policy: crate::runtime::CompletionRetryConfig,
    arming_progress: TerminalProgress,
) {
    let mut backoff = policy.initial_backoff();
    let mut attempts: u64 = 0;
    let mut reporter = FaultReporter::new();
    // Engine-side wall clock, not a workflow-visible one: this value is
    // reported to an operator and never recorded, so it does not cross the
    // determinism boundary.
    let started = std::time::Instant::now();
    // Carried ACROSS attempts, seeded from what the attempt that armed this
    // retry established. Durability is a fact about history: a terminal seen
    // durable stays durable, so an attempt that later reads a history without
    // it is looking at a reopen, not at a lost write. Resetting this per
    // attempt would make those two indistinguishable.
    let mut progress = arming_progress;
    loop {
        crate::runtime::engine_tasks::sleep_backoff(&mut backoff, policy.max_backoff()).await;
        attempts += 1;
        // Rebuilt per attempt from a weak runtime reference. `None` means the
        // engine that owned this run has been released, so there is no longer
        // a single writer to record through and continuing would be the
        // second-writer hazard rather than a repair.
        let Some(attempt_context) = context.upgrade() else {
            tracing::warn!(
                workflow_id = %handle.workflow_id(),
                run_id = %handle.run_id(),
                monitor_pid = handle.pid(),
                attempts,
                "abandoning workflow completion retry: the engine was released before \
                 the terminal landed; the run stays Running until a monitor is re-installed"
            );
            return;
        };
        match complete_process_exit(&attempt_context, handle, intent, progress).await {
            // 🔴 AN `Ok` IS NOT A RECORDING. `handle_process_exit_attempt`
            // also returns `Ok(())` from the stand-down path, where this
            // monitor deliberately wrote NOTHING because a newer lease holds
            // the writer slot. Reporting "recorded" there is a false durable
            // fact about the one thing this feature exists to get right, and it
            // contradicts the `info!` `monitor_stands_down` emitted one line
            // earlier. So the arm reports the progress it OBSERVED — which is
            // exactly the rule stated above `armed`, applied to the one site
            // that was still assuming.
            // 🔴 AND `Recorded` IS NOT "EVERYTHING RAN". `monitor_stands_down`
            // returns true the moment `progress` is already `Recorded`, and the
            // attempt then returns `Ok(Recorded)` from a body that wrote
            // NOTHING this time round — so the terminal is durable while the
            // visibility upsert and the registry reconcile that follow it may
            // never have run. The sentence was true and still incomplete, and
            // an incomplete sentence about durable state is how an operator
            // decides no follow-up is owed. It now says which fact is
            // established and which is not, matching the `NotRecorded` arm
            // below, whose whole virtue is naming who owes the remaining work.
            Ok(TerminalProgress::Recorded) => {
                tracing::info!(
                    workflow_id = %handle.workflow_id(),
                    run_id = %handle.run_id(),
                    monitor_pid = handle.pid(),
                    attempts,
                    "workflow completion is durable after a transient durable failure: the \
                     terminal event is in history, so no further append is owed. The \
                     bookkeeping that follows a terminal — the visibility upsert and the \
                     registry reconcile — is NOT confirmed by this line; where the retry \
                     stood down on an already-durable terminal it did not run at all, and a \
                     visibility row may lag until the next reconciliation sweep"
                );
                return;
            }
            Ok(TerminalProgress::NotRecorded) => {
                tracing::info!(
                    workflow_id = %handle.workflow_id(),
                    run_id = %handle.run_id(),
                    monitor_pid = handle.pid(),
                    attempts,
                    "workflow completion retry stood down without recording: this run's \
                     terminal is no longer this monitor's to write, so the retry is complete \
                     for this lease and the terminal is owed by whoever holds the run now"
                );
                return;
            }
            Err(CompletionFailure::Retryable(error, attempt_progress)) => {
                progress = attempt_progress;
                let current = failure_discriminant(&error);
                // 🔴 THE PRICE OF AN UNBOUNDED RETRY IS LOUDNESS. Ruled
                // 2026-08-06 (see `CompletionRetryConfig::default`): this loop
                // has no attempt budget on purpose, because a completion has no
                // re-driver and giving up would turn a transient store outage
                // into permanent silent data loss. A loop ruled unbounded must
                // never be invisible, so once the ladder has climbed to its
                // ceiling every attempt states itself — attempt count, elapsed
                // time, and the last error classification.
                //
                // 🔴 The rate is one line per ceiling-interval attempt, and
                // that is a CONSEQUENCE of the ceiling, not a throttle. Do not
                // add a rate limiter here; the ladder already is the rate, and a
                // second mechanism maintaining the same truth is the drift this
                // repository keeps paying for. It is also an invented cap, which
                // CLAUDE.md forbids.
                //
                // 🔴 SO THE CEILING IS ALSO THE LOG RATE, AND WHOEVER MOVES IT
                // MOVES BOTH. That coupling is stated here rather than guarded,
                // because guarding it would be the rate limiter this comment
                // just forbade. It is not hypothetical: the shipped default is
                // a 30 s ceiling, so a stuck retry costs two lines a minute —
                // but `CompletionRetryConfig::try_new` accepts any self-
                // consistent ladder, and `try_new(1ms, 2ms)` is one. That
                // configuration is legitimate, it is what an operator asked
                // for, and it emits on the order of five hundred warn lines a
                // second for as long as the store stays unwell. The constructor
                // refuses ladders that cannot climb; it does not, and must not,
                // second-guess how fast an operator wants one that can.
                let at_ceiling = backoff >= policy.max_backoff();
                // Called unconditionally, never short-circuited by `at_ceiling`:
                // the reporter must SEE every fault to keep its own first-sight
                // bookkeeping honest, whether or not this attempt speaks.
                let first_sight = reporter.should_report(current, attempts);
                if at_ceiling || first_sight {
                    tracing::warn!(
                        workflow_id = %handle.workflow_id(),
                        run_id = %handle.run_id(),
                        monitor_pid = handle.pid(),
                        attempts,
                        elapsed_seconds = started.elapsed().as_secs(),
                        at_ceiling,
                        error = %error,
                        fault = current,
                        terminal_recorded = matches!(progress, TerminalProgress::Recorded),
                        "workflow completion retry failed transiently; retrying with backoff"
                    );
                }
            }
            Err(CompletionFailure::Invariant(error, progress)) => {
                match progress {
                    TerminalProgress::NotRecorded => tracing::error!(
                        workflow_id = %handle.workflow_id(),
                        run_id = %handle.run_id(),
                        monitor_pid = handle.pid(),
                        attempts,
                        error = %error,
                        terminal_recorded = false,
                        "workflow completion hit an unretryable failure; the run stays \
                         Running until a monitor is re-installed"
                    ),
                    TerminalProgress::Recorded => tracing::error!(
                        workflow_id = %handle.workflow_id(),
                        run_id = %handle.run_id(),
                        monitor_pid = handle.pid(),
                        attempts,
                        error = %error,
                        terminal_recorded = true,
                        "workflow completion hit an unretryable failure AFTER its terminal \
                         event was durably recorded; the run is terminal and its status \
                         projection is correct, but the bookkeeping that follows the \
                         terminal did not complete"
                    ),
                }
                return;
            }
        }
    }
}

pub(super) async fn complete_process_exit(
    context: &ProcessExitContext,
    handle: &WorkflowHandle,
    intent: &TerminalIntent,
    prior: TerminalProgress,
) -> Result<TerminalProgress, CompletionFailure> {
    // Set by the attempt at the two instants where a terminal event becomes
    // durable — its own successful append, and finding one already present —
    // so a failure raised by the bookkeeping AFTER either of those is reported
    // as what it is rather than as a lost terminal.
    //
    // Seeded with what EARLIER attempts established rather than reset to
    // `NotRecorded`, because durability is a fact about history, not about one
    // attempt: once a terminal for this run has been seen durable it stays
    // durable, and an attempt that re-reads a history no longer showing it is
    // looking at a reopen. Resetting per attempt would throw that away and make
    // the two situations — never recorded, and recorded then superseded —
    // indistinguishable at exactly the point where they demand opposite
    // actions.
    let mut progress = prior;
    handle_process_exit_attempt(context, handle, intent, &mut progress)
        .await
        .map(|()| progress)
        .map_err(|error| classify_completion_failure(error, progress))
}

/// Which failures a retry can repair.
///
/// **Two layers, and they are not the same kind of rule.** The `Store` and
/// `Durability` layers are enumerated per variant; the outer `EngineError`
/// layer is a fail-closed *family default* (`_ => false`) covering everything
/// else. Saying "enumerated per variant" of the whole classifier would be
/// false — roughly fifty engine variants are covered by that one arm, and they
/// are covered by it deliberately: the safe direction is to abandon loudly, and
/// an unbounded loop must never spin on a fault no attempt can repair.
///
/// The per-variant half is load-bearing and mechanically enforced: neither
/// `StoreError` nor `DurabilityError` is `#[non_exhaustive]`, so adding a
/// variant to either fails this match to compile rather than silently taking a
/// default. That is a genuine fail-closed property, not a convention.
///
/// Retrying the whole `Store` and `Durability` families would sweep in faults
/// that no later attempt can repair, and — because this loop has no attempt
/// budget — would spin on them forever at the backoff ceiling:
///
/// - [`StoreError::SequenceConflict`] is, in this engine, the documented
///   signature of a double-writer bug (CLAUDE.md invariant 3). It must stay
///   loud and diagnosable, not become a silent warm loop.
/// - [`StoreError::NotFound`] and [`StoreError::Serialization`] describe the
///   request, not the store's availability; the next attempt makes the same
///   request.
/// - [`DurabilityError::NonDeterminism`], `HistoryShape` and `SearchAttribute`
///   are replay/logic faults. A retry re-derives the same history and hits
///   them identically.
///
/// - [`StoreError::NotOwner`] is **not** transient *here*, and that is a
///   narrower reading than the variant's own documentation. That documentation
///   says the caller "should re-resolve the shard's owner and retry or
///   forward" — re-resolution is the load-bearing half, and this loop does not
///   have it. It re-calls one already-resolved `Arc<dyn EventStore>`, so it
///   would wait at the backoff ceiling for a condition that cannot recur: the
///   shard moved, and this node does not own it again until another failover.
///   Retrying without re-resolving is not the store's advice, it is half of it.
///   Abandoning loudly hands the run to the new owner's own recovery sweep,
///   which is the mechanism that actually repairs this.
///
/// What is genuinely transient is unavailability — an opaque backend failure.
/// Retrying that is safe because the attempt re-checks the run's terminal under
/// the recorder lock before appending, so a re-run cannot double-write.
pub(super) fn classify_completion_failure(
    error: EngineError,
    progress: TerminalProgress,
) -> CompletionFailure {
    if completion_failure_is_transient(&error) {
        CompletionFailure::Retryable(error, progress)
    } else {
        CompletionFailure::Invariant(error, progress)
    }
}

/// A stable, structural name for a failure — the retry loop's dedup key.
///
/// Deliberately **not** the rendered error. [`aion_store::StoreError::Backend`]
/// is constructed by embedding the backend's own `Display`
/// (`format!("haematite api error: {error}")`, `error.to_string()` in the libSQL
/// adapter), and those inner renderings routinely carry a peer address, an
/// elapsed time, a current-leader hint or an OS errno. Keying dedup on that
/// string means the key changes on every single attempt against a real backend
/// under a real outage, so the loop emits an identical-in-substance warning at
/// the full retry rate for the life of the process — which is exactly the #94
/// flood, now aimed at a store that is already unwell.
///
/// The converse failure is just as real: two genuinely different faults that
/// happen to render the same string would collapse into one reported line, and
/// the operator would never learn the fault had changed.
///
/// A typed variant is the thing that is actually the same or actually
/// different, so that is what is compared. The full rendered error still goes
/// on every line that IS emitted — this governs only whether to speak, never
/// what to say.
pub(super) fn failure_discriminant(error: &EngineError) -> &'static str {
    match error {
        EngineError::Store(store) => store_discriminant(store),
        EngineError::Durability(durability) => match durability {
            crate::durability::DurabilityError::Store(store) => store_discriminant(store),
            crate::durability::DurabilityError::NonDeterminism(_) => "durability/non-determinism",
            crate::durability::DurabilityError::HistoryShape { .. } => "durability/history-shape",
            crate::durability::DurabilityError::SearchAttribute(_) => "durability/search-attribute",
            crate::durability::DurabilityError::EngineTaskEpochClosed { .. } => {
                "durability/engine-task-epoch-closed"
            }
        },
        // Retryable, so it CAN be seen twice and needs its own name — without
        // it a terminal-writer wait would share one bucket with the terminal
        // faults and an operator could not tell a self-clearing reservation
        // from a permanent stop.
        EngineError::TerminalWriterHeld { .. } => "engine/terminal-writer-held",
        // Every other engine failure is unretryable (see
        // `completion_failure_is_transient`), so it is reported once at
        // `error!` and the loop ends. It never reaches the dedup comparison a
        // second time, which is why one shared name is enough here and why
        // giving each of ~50 variants its own would be a list to maintain that
        // nothing reads.
        _ => "engine/unretryable",
    }
}

/// The typed name of one store failure, for [`failure_discriminant`].
fn store_discriminant(error: &aion_store::StoreError) -> &'static str {
    match error {
        aion_store::StoreError::Backend(_) => "store/backend-unavailable",
        aion_store::StoreError::NotOwner { .. } => "store/not-owner",
        aion_store::StoreError::SequenceConflict { .. } => "store/sequence-conflict",
        aion_store::StoreError::NotFound { .. } => "store/not-found",
        aion_store::StoreError::Serialization(_) => "store/serialization",
    }
}

/// Whether one failure describes the store being briefly unavailable.
///
/// Split out so the recursion through [`DurabilityError::Store`] reads once and
/// the two callers cannot drift apart.
fn completion_failure_is_transient(error: &EngineError) -> bool {
    match error {
        EngineError::Store(store) => store_error_is_transient(store),
        EngineError::Durability(durability) => match durability {
            crate::durability::DurabilityError::Store(store) => store_error_is_transient(store),
            // None of these clears by waiting, so retrying any of them burns
            // attempts against a condition the attempts cannot touch.
            //
            // `EngineTaskEpochClosed` is the one worth its own sentence,
            // because it is permanent BY CONSTRUCTION rather than by
            // judgement: `EngineTaskRuntime::begin_close` sets a one-way latch
            // and `is_epoch_open` is its negation, so no number of attempts
            // reopens it. Classifying it transient would spin the retry ladder
            // against a condition that cannot clear until a different engine
            // exists — the mirror of the `TerminalWriterHeld` mistake recorded
            // below, in the opposite direction.
            crate::durability::DurabilityError::NonDeterminism(_)
            | crate::durability::DurabilityError::HistoryShape { .. }
            | crate::durability::DurabilityError::SearchAttribute(_)
            | crate::durability::DurabilityError::EngineTaskEpochClosed { .. } => false,
        },
        // Documented transient BY THE VARIANT ITSELF: "a reservation lives only
        // across one terminal transition", so the very next attempt finds it
        // released. It is reachable from here — `start_continuation_replacement`
        // → `start_workflow_with_options` → `registry.insert` — and the
        // reservation is keyed by WORKFLOW id, not run id, so a continue-as-new
        // successor can meet a reservation held by its own predecessor.
        //
        // Sweeping it into the family default classified a self-clearing
        // condition as unrepairable and stopped a continue-as-new chain
        // permanently. That is the cost of a family default nobody re-reads: it
        // is safe in direction but not automatically right per variant.
        EngineError::TerminalWriterHeld { .. } => true,
        _ => false,
    }
}

/// Whether one store failure is unavailability rather than a bad request.
fn store_error_is_transient(error: &aion_store::StoreError) -> bool {
    match error {
        aion_store::StoreError::Backend(_) => true,
        aion_store::StoreError::NotOwner { .. }
        | aion_store::StoreError::SequenceConflict { .. }
        | aion_store::StoreError::NotFound { .. }
        | aion_store::StoreError::Serialization(_) => false,
    }
}
