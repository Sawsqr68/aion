//! Epoch-gate tests for a continue-as-new transition.
//!
//! Most of these drive [`super::record_continuation`] directly rather than the
//! NIF that wraps it. That is the point of the split: the gate under test
//! decides whether a durable TERMINAL is appended, and reaching it through a
//! live beamr process would be coarse to attribute.
//!
//! The NIF's own gate — the `if` that CONSUMES the predicate, and the
//! `cancel_pid` behind it — is measured separately, at the bottom of this file,
//! against a live beamr process. It has to be: a reviewer reverted that `if` to
//! its pre-remediation form and the entire suite stayed green.
//!
//! 🔴 A FIRST ATTEMPT AT THAT TEST MEASURED NOTHING AND WAS WITHDRAWN RATHER
//! THAN SHIPPED. Its processes died on their own, so "the process is gone"
//! passed with the gate removed. The cause was the harness, not beamr and not
//! the NIF: `RuntimeHandle::new` registers beamr's BIFs but NOT the
//! `aion_flow_ffi` table, so every call the fixture made was an undef. See
//! docs/design/aion-authoring/SEAT-RESUME-2026-08-07-CB.md §1 for the seven
//! process shapes that separated "the call failed" from "the call could not be
//! resolved", and for the two further wrong readings that diagnosis produced
//! before it produced the right one.

use std::sync::Arc;

use aion_core::{Event, PackageVersion, Payload, RunId, WorkflowId};
use aion_store::{EventStore, InMemoryStore, ReadableEventStore};
use chrono::Utc;

use super::{ContinuationOutcome, outcome_must_end_the_process, record_continuation};
use crate::durability::{DurabilityError, Recorder, WorkflowStartRecord};
use crate::runtime::engine_tasks::EngineTaskRuntime;
use crate::runtime::nif_context::NifContextError;
use crate::store_faults::FlakyStore;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Seed a live run with a declared-timeout deadline outstanding, returning a
/// recorder positioned at its head.
///
/// The deadline matters: `record_continuation` retires it as the second half of
/// the transition, so its retirement is what distinguishes "the transition ran"
/// from "the terminal happened to land".
///
/// Takes the store as `Arc<dyn EventStore>` so the fault-injecting fixture and
/// the plain in-memory one are seeded by exactly the same code — a second
/// seeding path is how the faulted test and its control start differing for
/// reasons neither test names.
async fn seed_running_run(
    store: Arc<dyn EventStore>,
) -> Result<(Recorder, RunId), Box<dyn std::error::Error>> {
    let workflow_id = WorkflowId::new_v4();
    let run_id = RunId::new_v4();
    let deadline_id = crate::time::deadline_timer_id(&run_id)?;
    let mut recorder = Recorder::new(workflow_id, store);
    recorder
        .record_workflow_started(
            Utc::now(),
            WorkflowStartRecord {
                workflow_type: "continuer".to_owned(),
                input: Payload::from_json(&serde_json::json!({}))?,
                run_id: run_id.clone(),
                parent_run_id: None,
                package_version: PackageVersion::new("a".repeat(64)),
            },
        )
        .await?;
    recorder
        .record_timer_started(Utc::now(), deadline_id, Utc::now())
        .await?;
    Ok((recorder, run_id))
}

/// Whether history records a `WorkflowContinuedAsNew`.
fn continued(history: &[Event]) -> bool {
    history
        .iter()
        .any(|event| matches!(event, Event::WorkflowContinuedAsNew { .. }))
}

/// Whether history records a `WorkflowStarted`.
fn started(history: &[Event]) -> bool {
    history
        .iter()
        .any(|event| matches!(event, Event::WorkflowStarted { .. }))
}

/// Whether history retires any timer.
fn retired_a_timer(history: &[Event]) -> bool {
    history
        .iter()
        .any(|event| matches!(event, Event::TimerCancelled { .. }))
}

/// 🔴 A CLOSED EPOCH MUST NOT LET A RUN CONTINUE ITSELF INTO NOTHING.
///
/// `WorkflowContinuedAsNew` is the one TERMINAL a live workflow process could
/// still write after its engine was dropped without `shutdown` — every other
/// terminal reachable from workflow code is gated. The harm is not the terminal
/// on its own: it is that the successor run this terminal obliges is ALREADY
/// refused, at `start_continuation_replacement`'s own epoch check. So an
/// ungated terminal ends the predecessor, retires its deadline, kills its
/// process, and produces no replacement — a severed continuation chain that only
/// a later engine's startup sweep can repair, and that nothing repairs at all if
/// this was an embedded engine released mid-process.
///
/// The control is not optional here. Without it, a treatment that appended
/// nothing would look identical to a `record_continuation` that never reached
/// the append for some unrelated reason — a seeding mistake, a store that
/// rejects everything, a run already terminal.
#[tokio::test(flavor = "multi_thread")]
async fn a_closed_epoch_refuses_the_continue_as_new_terminal() -> TestResult {
    let store = Arc::new(InMemoryStore::default());
    let tasks = EngineTaskRuntime::new()?;

    // POSITIVE CONTROL: epoch open, the whole transition lands.
    let (mut control_recorder, control_run) =
        seed_running_run(Arc::clone(&store) as Arc<dyn EventStore>).await?;
    let control_id = control_recorder.workflow_id().clone();
    let control_outcome = record_continuation(
        &mut control_recorder,
        &tasks,
        &control_run,
        Payload::from_json(&serde_json::json!({"round": 2}))?,
    )
    .await?;
    assert!(
        matches!(control_outcome, ContinuationOutcome::Complete),
        "control: an unfaulted transition must report BOTH halves done — a `TerminalOnly` here \
         would mean the deadline retirement silently failed and every assertion below about \
         what a completed transition looks like is measuring the wrong thing"
    );
    let control_history = store.read_history(&control_id).await?;
    assert!(
        continued(&control_history),
        "control: an open epoch must record WorkflowContinuedAsNew, or the refusal below \
         proves nothing: {control_history:#?}"
    );
    assert!(
        retired_a_timer(&control_history),
        "control: the transition also retires the predecessor's deadline — without this the \
         treatment could be measuring a half-built transition: {control_history:#?}"
    );

    // TREATMENT: same store, same tasks, epoch closed.
    let (mut recorder, run_id) =
        seed_running_run(Arc::clone(&store) as Arc<dyn EventStore>).await?;
    let workflow_id = recorder.workflow_id().clone();
    let before = store.read_history(&workflow_id).await?.len();
    tasks.begin_close();

    let refusal = record_continuation(
        &mut recorder,
        &tasks,
        &run_id,
        Payload::from_json(&serde_json::json!({"round": 2}))?,
    )
    .await;
    let Err(error) = refusal else {
        return Err("a closed epoch must refuse the continue-as-new terminal".into());
    };

    assert!(
        matches!(error, DurabilityError::EngineTaskEpochClosed { .. }),
        "the refusal must name the epoch, not be laundered through a generic durability \
         fault — an operator reading `history shape error` would go looking for corrupt \
         history: {error:?}"
    );
    let history = store.read_history(&workflow_id).await?;
    assert!(
        !continued(&history),
        "a closed epoch must NOT record WorkflowContinuedAsNew: the replacement run it obliges \
         is already refused, so the terminal would end this run with no continuation: \
         {history:#?}"
    );
    assert_eq!(
        history.len(),
        before,
        "the refusal must append NOTHING — the gate sits before the first durable write, not \
         between the terminal and the deadline retirement"
    );
    // Absence paired with survival: an empty or unreadable history would satisfy
    // both assertions above for entirely the wrong reason.
    assert!(
        started(&history),
        "the run must still be live in history after the refusal — that is the whole point of \
         refusing, and an empty history would pass the two assertions above vacuously: \
         {history:#?}"
    );
    Ok(())
}

/// The gate must not be so wide that it refuses a run that was never going to
/// continue anyway.
///
/// `record_continuation` rejects a run that already recorded a terminal, and
/// that rejection predates the epoch gate. If the epoch check were placed first,
/// this case would change its reported cause — an operator would be told the
/// engine was closing when the truth is the run was already finished. Ordering
/// the two checks is a decision, so it is pinned.
#[tokio::test(flavor = "multi_thread")]
async fn an_already_terminal_run_reports_its_own_cause_not_the_epoch() -> TestResult {
    let store = Arc::new(InMemoryStore::default());
    let tasks = EngineTaskRuntime::new()?;
    let (mut recorder, run_id) =
        seed_running_run(Arc::clone(&store) as Arc<dyn EventStore>).await?;
    recorder
        .record_workflow_completed(Utc::now(), Payload::from_json(&serde_json::json!("done"))?)
        .await?;
    tasks.begin_close();

    let refusal = record_continuation(
        &mut recorder,
        &tasks,
        &run_id,
        Payload::from_json(&serde_json::json!({}))?,
    )
    .await;
    let Err(error) = refusal else {
        return Err("a run that already recorded a terminal cannot continue".into());
    };

    assert!(
        matches!(error, DurabilityError::HistoryShape { .. }),
        "an already-terminal run must report THAT, even with the epoch closed — reporting the \
         epoch would send an operator to look at engine shutdown for a run that simply \
         finished: {error:?}"
    );
    Ok(())
}

/// 🔴 A LANDED TERMINAL ENDS THE PROCESS, AND SO DOES THE EPOCH REFUSAL — AND
/// NOTHING ELSE MAY.
///
/// Refusing the terminal is only half of the gate, and on its own it is a net
/// loss. Before the gate existed the recorder call either succeeded or aborted
/// the NIF, and the success path always reached `cancel_pid`; that instruction
/// is where the fifth writer died. A refusal that returns early skips it, which
/// would leave this workflow process runnable on a scheduler `Engine::drop`
/// deliberately keeps alive — free to record timers, activities, children, and
/// signals into a THIRD workflow's history, against a store a successor engine
/// may already own. Refusing one terminal while licensing every other write is
/// worse than not refusing at all.
///
/// **The `TerminalOnly` case is the one that was wrong.** An earlier version of
/// this predicate took only the error, which answered "was this a store error?"
/// — and a store error raised AFTER the terminal landed got the same answer as
/// one raised before it, so a run whose `WorkflowContinuedAsNew` was already
/// durable kept a live process. That is two writers on one history the moment
/// the sweep starts the successor, and no successor at all until it does.
///
/// **The controls are the whole test.** A predicate that answered `true`
/// unconditionally would satisfy the first three assertions and close the
/// exposure — and would also kill a run for an ordinary authoring mistake,
/// converting a recoverable error into a dead workflow. So the negative cases
/// are not decoration: they are the half that distinguishes a gate from a
/// hammer.
///
/// Killing mutations, every direction — RUN, not reasoned (2026-08-07, each
/// applied to the literal predicate text, module suite red, file restored and
/// proven byte-identical):
/// - `Err(_) => true` (every error ends the process): this test red at the two
///   negative assertions, AND the live test red at CONTROL 2 — the gate kills
///   the spared process.
/// - `HistoryShape` in place of `EngineTaskEpochClosed`: this test red at the
///   epoch assertion, AND the live test red — the treatment's process survives.
/// - `Ok` narrowed to `Complete`: this test red at the `TerminalOnly`
///   assertion, AND the terminal-only integration test below red at its final
///   decision assertion.
///
/// Every mutation is also caught by a SECOND test grounded in real machinery,
/// so this unit test asserting what the predicate says about hand-built values
/// is corroborated, not load-bearing alone.
#[test]
fn the_process_ends_whenever_the_terminal_landed_and_for_one_refusal_besides() {
    assert!(
        outcome_must_end_the_process(&Ok(ContinuationOutcome::Complete)),
        "a completed transition must end the process: the run is terminal and its successor is \
         started only by the process-exit monitor, so a process that does not exit is a chain \
         that never continues"
    );

    assert!(
        outcome_must_end_the_process(&Ok(ContinuationOutcome::TerminalOnly(
            DurabilityError::HistoryShape {
                reason: "the deadline retirement failed after the terminal landed".to_owned(),
            }
        ))),
        "a HALF-completed transition must end the process too — the terminal is durable, so the \
         run is over whatever happened next, and this is the exact case the old error-keyed \
         predicate got backwards"
    );

    let epoch_closed = NifContextError::Durability(DurabilityError::EngineTaskEpochClosed {
        reason: "this engine has begun closing".to_owned(),
    });
    assert!(
        outcome_must_end_the_process(&Err(epoch_closed)),
        "a closed epoch must end the process: it is the one condition workflow code cannot \
         improve on and cannot outlive, and leaving the process alive leaves a durable writer \
         running against a store a successor may already own"
    );

    // CONTROL 1: the run already finished, and nothing was appended HERE. Not
    // spared because it is recoverable — its terminal is already durable — but
    // because the seam that recorded that terminal owns the teardown; see the
    // predicate's doc for the two KINDS of owner (pid-enders and
    // deregister-only), and the "Five ordinary terminal paths" paragraph in
    // `lifecycle/completion.rs` for the one enumeration of them.
    let already_terminal = NifContextError::Durability(DurabilityError::HistoryShape {
        reason: "run already recorded a terminal event".to_owned(),
    });
    assert!(
        !outcome_must_end_the_process(&Err(already_terminal)),
        "an already-terminal run is spared here NOT because it is live — its terminal is \
         already durable — but because whichever seam recorded that terminal owns the \
         teardown: a kill from this one would race the owners that end the pid and usurp \
         the ones that only deregister"
    );

    // CONTROL 2: a non-`Durability` variant of the same error type.
    //
    // 🔴 THIS IS A TYPE-LEVEL CONTROL, NOT A REACHABLE CASE, and saying
    // otherwise was wrong. An earlier version of this comment claimed the case
    // was "reached through a DIFFERENT variant", implying production can produce
    // it here. It cannot: the NIF obtains this outcome from
    // `NifContext::block_on_recorder`, whose closure returns
    // `Result<_, DurabilityError>` and is widened by the single `#[from]` impl,
    // so `Durability` is the ONLY variant structurally reachable at that call
    // site. `RecorderPoisoned`, `UnknownProcess`, and `TermEncoding` arrive from
    // other seams entirely.
    //
    // It stays because it kills a mutation nothing else here does, and the
    // mutation was RUN rather than guessed. The tempting wrong rule is
    // "everything except an already-terminal run ends the process" —
    // `!matches!(error, Durability(HistoryShape { .. }))` — which answers
    // correctly for the epoch and for CONTROL 1 and is caught only on this line.
    // (The family mutation, `matches!(error, Durability(_))`, is NOT this
    // control's: CONTROL 1 fails first. An earlier draft of this comment named
    // that one, which was wrong.) The signature admits the value either way, so
    // its answer is part of the contract whether or not today's caller can
    // produce it.
    let poisoned = NifContextError::RecorderPoisoned;
    assert!(
        !outcome_must_end_the_process(&Err(poisoned)),
        "a poisoned recorder is not this engine standing down; the process must be left to \
         handle it, not destroyed"
    );
}

/// 🔴 THE TERMINAL LANDS, THE DEADLINE RETIREMENT FAILS — AND THE CALLER IS TOLD
/// THE RUN IS OVER.
///
/// The transition is two independent durable appends. This drives a real failure
/// of the SECOND one and pins that `record_continuation` reports it as
/// [`ContinuationOutcome::TerminalOnly`] rather than as an error, because the
/// caller's next act is deciding whether to end a workflow process and the only
/// fact that decides it is whether the terminal is durable.
///
/// Without this the fix is a type and a predicate with nothing behind them: the
/// unit test above asserts what the predicate says about a hand-built
/// `TerminalOnly`, and would pass just as happily if `record_continuation` could
/// never produce one.
#[tokio::test(flavor = "multi_thread")]
async fn a_deadline_retirement_that_fails_after_the_terminal_reports_terminal_only() -> TestResult {
    let store = Arc::new(FlakyStore::new());
    let tasks = EngineTaskRuntime::new()?;

    // CONTROL: the same fixture with no fault set completes both halves. The
    // treatment below differs from this by ONE call, so a `TerminalOnly` there
    // is attributable to the injected fault and not to the fixture.
    let (mut control_recorder, control_run) =
        seed_running_run(Arc::clone(&store) as Arc<dyn EventStore>).await?;
    let control_id = control_recorder.workflow_id().clone();
    let control = record_continuation(
        &mut control_recorder,
        &tasks,
        &control_run,
        Payload::from_json(&serde_json::json!({"round": 2}))?,
    )
    .await?;
    assert!(
        matches!(control, ContinuationOutcome::Complete),
        "control: an unfaulted FlakyStore must complete the whole transition, or the treatment \
         is measuring the fixture rather than the fault"
    );
    let control_history = store.recorded_history(&control_id).await?;
    assert!(
        retired_a_timer(&control_history),
        "control: the unfaulted transition must retire the deadline — the treatment's claim that \
         it did NOT is meaningless unless this one did: {control_history:#?}"
    );

    // TREATMENT: let the terminal through, refuse the retirement that follows it.
    let (mut recorder, run_id) =
        seed_running_run(Arc::clone(&store) as Arc<dyn EventStore>).await?;
    let workflow_id = recorder.workflow_id().clone();
    store.fail_appends_after(1, 1);

    let outcome = record_continuation(
        &mut recorder,
        &tasks,
        &run_id,
        Payload::from_json(&serde_json::json!({"round": 2}))?,
    )
    .await?;

    assert!(
        matches!(outcome, ContinuationOutcome::TerminalOnly(_)),
        "a post-terminal append failure must be reported as TerminalOnly, not as Complete and \
         not as an error: the run has continued and the caller must be able to see that"
    );

    // Read PAST the injector: an oracle drawing from the same failure budget as
    // the code under test can be refused, which is an instrument failure
    // reported as a fix failure.
    let history = store.recorded_history(&workflow_id).await?;
    assert!(
        continued(&history),
        "the terminal must be DURABLE — that is the whole premise of the TerminalOnly report, \
         and if the fault had landed on the terminal instead this test would be pinning the \
         pre-terminal case under a post-terminal name: {history:#?}"
    );
    assert!(
        !retired_a_timer(&history),
        "the injected fault must have taken the deadline retirement specifically — a run whose \
         deadline WAS retired has nothing left over to report: {history:#?}"
    );

    // The decision the whole type exists to serve.
    assert!(
        outcome_must_end_the_process(&Ok(outcome)),
        "a half-completed transition must still end the workflow process: the run is terminal, \
         and a live process on a terminal run writes into a closed history alongside the \
         successor the sweep will start"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The NIF's OWN gate, measured against a live beamr workflow process.
//
// 🔴 EVERYTHING ABOVE TESTS THE DECISION. THIS TESTS THE ACT. The predicate
// `outcome_must_end_the_process` is a function and is exercised directly; the
// `if` in the NIF that CONSUMES it — and the `cancel_pid` behind it — is only
// reachable through a real process making a real NIF call. A reviewer proved
// the gap the hard way: reverting that `if` to its pre-remediation form left
// the whole suite green.
//
// 🔴 AN EARLIER ATTEMPT AT THIS TEST MEASURED NOTHING, AND THE REASON IS WORTH
// KEEPING. Its fixture's process died on its own — so the assertion "the
// process is gone" passed with the gate removed. The diagnosis ran SEVEN
// process shapes through one harness; the three that carry the argument are a
// `park_only` process, which lived, one calling a NIF, which died, and one
// calling a module that does not exist, which died the same way. (The other
// four separated "an exception was raised" from "the call could not be
// resolved" — a caught badarg and a caught local `erlang:error` both LIVED, so
// try/catch was never the explanation. All seven are in
// docs/design/aion-authoring/SEAT-RESUME-2026-08-07-CB.md §1.)
//
// The common factor was not "the NIF failed" but "the call could not be
// RESOLVED": `RuntimeHandle::new` registers beamr's BIFs, not the
// `aion_flow_ffi` NIF table, so every call the fixture made was an undef.
// `install_nifs` is what this harness was missing, and with it a workflow
// process survives a refused `continue_as_new` — which is precisely what makes
// the gate load-bearing rather than decorative.
// ---------------------------------------------------------------------------

/// A live runtime with the engine NIF table installed and the fixture loaded.
///
/// The `install_nifs` call is the load-bearing line. Without it `aion_flow_ffi`
/// resolves nowhere and every fixture process dies of undef, which reads as a
/// successful termination to a test asserting the process is gone.
fn live_runtime() -> Result<Arc<crate::runtime::RuntimeHandle>, Box<dyn std::error::Error>> {
    let runtime = Arc::new(crate::runtime::RuntimeHandle::new(
        crate::runtime::config::RuntimeConfig::new(Some(2)),
    )?);
    let mut registration = crate::runtime::nif::NifRegistration::new();
    registration.add_engine_nifs();
    runtime.install_nifs(registration)?;
    runtime.register_module(
        "aion_continue_fixture",
        include_bytes!("../../tests/fixtures/aion_continue_fixture.beam"),
    )?;
    Ok(runtime)
}

/// Install the DETERMINISM context source, which is a second installation and
/// not the same one as [`crate::runtime::install_nif_runtime_context`].
///
/// 🔴 THE FIXTURE'S SYNCHRONISATION DEPENDS ON THIS AND NOTHING SAYS SO AT THE
/// CALL SITE. `aion_flow_ffi:workflow_id/0` resolves through
/// `EngineNifState::context_source`, not through the runtime context the durable
/// NIFs use; `builder_assembly.rs` installs both, so a real engine has never
/// noticed they are separate. A harness that installs only the runtime context
/// leaves the pure NIF answering `{error, ...}` forever, which turns the
/// fixture's poll into a wait that can only ever time out.
fn install_determinism_context(
    runtime: &Arc<crate::runtime::RuntimeHandle>,
    registry: &Arc<crate::registry::Registry>,
    store: &Arc<dyn EventStore>,
    tokio_runtime: &tokio::runtime::Runtime,
) {
    crate::runtime::nif_determinism::install_nif_context_source(
        runtime.nif_state(),
        Arc::new(crate::runtime::nif_determinism::NifContextSource::new(
            Arc::clone(registry),
            tokio_runtime.handle().clone(),
            Arc::clone(store),
            runtime.signal_delivery(),
        )),
    );
}

/// Whether [`register_live_run`] leaves a declared-timeout deadline
/// outstanding in the run's history.
///
/// The distinction decides which `ContinuationOutcome` the NIF can produce:
/// with no deadline in history, `retire_run_deadline` has nothing to append,
/// so `Ok(TerminalOnly)` is UNREACHABLE and the run's transition is
/// terminal-append-or-nothing. Review 22 proved that mattered the hard way — a
/// gate mutated to spare exactly the `TerminalOnly` arm survived every test in
/// the crate, because every live run to that point was registered deadline-free.
enum SeededDeadline {
    /// No deadline: the transition is a single terminal append.
    None,
    /// A deadline is outstanding: the transition is terminal + retirement,
    /// and a fault landing on the second half produces `Ok(TerminalOnly)`.
    Outstanding,
}

/// Put a live run behind `pid` in the registry, optionally with a
/// declared-timeout deadline outstanding.
///
/// The fixture polls a pure NIF until this lands, so this call is also the
/// starting gun: everything the test wants the durable call to see must already
/// be arranged before it returns — including the deadline seed, which is why it
/// is appended here, BEFORE the `registry.insert` that releases the fixture.
fn register_live_run(
    tokio_runtime: &tokio::runtime::Runtime,
    registry: &crate::registry::Registry,
    store: Arc<dyn EventStore>,
    pid: crate::Pid,
    deadline: &SeededDeadline,
) -> Result<WorkflowId, Box<dyn std::error::Error>> {
    let workflow_id = WorkflowId::new_v4();
    let run_id = RunId::new_v4();
    let mut recorder = Recorder::new(workflow_id.clone(), store);
    tokio_runtime.block_on(recorder.record_workflow_started(
        Utc::now(),
        WorkflowStartRecord {
            workflow_type: "continuer".to_owned(),
            input: Payload::from_json(&serde_json::json!({}))?,
            run_id: run_id.clone(),
            parent_run_id: None,
            package_version: PackageVersion::new("a".repeat(64)),
        },
    ))?;
    match deadline {
        SeededDeadline::None => {}
        SeededDeadline::Outstanding => {
            // The same seed `seed_running_run` plants for the direct-call
            // tests: the id is minted by `deadline_timer_id`, never pasted.
            // No timer service runs in this harness, so the fire time is
            // never acted on — what matters is the row being outstanding in
            // history when `record_continuation` reads it.
            let deadline_id = crate::time::deadline_timer_id(&run_id)?;
            tokio_runtime.block_on(recorder.record_timer_started(
                Utc::now(),
                deadline_id,
                Utc::now(),
            ))?;
        }
    }
    registry.insert(
        (workflow_id.clone(), run_id.clone()),
        crate::registry::WorkflowHandle::new(crate::registry::WorkflowHandleParts {
            workflow_id: workflow_id.clone(),
            run_id,
            pid,
            workflow_type: "continuer".to_owned(),
            namespace: "default".to_owned(),
            loaded_version: aion_package::ContentHash::from_bytes([9; 32]),
            cached_status: aion_core::WorkflowStatus::Running,
            residency: crate::registry::HandleResidency::Resident,
            recorder,
            completion: crate::registry::CompletionNotifier::new(),
        }),
    )?;
    Ok(workflow_id)
}

/// How long [`settles`] waits for a condition to appear.
///
/// 🔴 ONE CONSTANT, USED IN BOTH DIRECTIONS, AND THAT IS THE POINT. The
/// treatment waits up to this long for a process to DIE; CONTROL 2 waits the
/// same span to establish that a process does NOT. An earlier draft checked the
/// control's survival exactly once, immediately after its witness fired — and
/// the witness (an append budget drained inside the recorder call) is spent
/// STRICTLY BEFORE the `cancel_pid` that would kill it. So a gate mutated to
/// fire unconditionally could still be observed alive at that instant, and
/// whether the mutation was caught came down to scheduling. Giving both
/// directions the same budget makes the kill structural rather than probable.
///
/// It is 10 s because it bounds a beamr process's whole scheduling latency, not
/// an expected wait — the treatment settles in tens of milliseconds in practice.
/// The negative direction spends the full budget on every green run, and that
/// cost is accepted: a shorter one buys back seconds by weakening the only
/// assertion in this file that can distinguish a discriminating gate from a
/// hammer.
const SETTLE_BUDGET: std::time::Duration = std::time::Duration::from_secs(10);

/// Poll `probe` until it holds or [`SETTLE_BUDGET`] runs out, reporting whether
/// it held.
///
/// Polling rather than sleeping a fixed span: the fixture's own wait is bounded
/// at 3 s, and a test that slept exactly as long as it expected to wait would
/// turn every scheduling hiccup into a failure while proving nothing extra.
fn settles(probe: impl Fn() -> bool) -> bool {
    let deadline = std::time::Instant::now() + SETTLE_BUDGET;
    while std::time::Instant::now() < deadline {
        if probe() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    probe()
}

/// The shared world of the live-gate test: one tokio runtime, one beamr
/// runtime with the NIF table installed, one registry, one fault-injecting
/// store.
///
/// A struct rather than locals because the conditions are functions — the
/// function-length budget forced a split, and it cut along the condition
/// boundaries. Each condition borrows the SAME harness, so "one runtime, one
/// registry, one store" is structural: a condition cannot quietly build its
/// own world, which is exactly the drift the test's doc comment forbids.
struct GateHarness {
    tokio_runtime: tokio::runtime::Runtime,
    runtime: Arc<crate::runtime::RuntimeHandle>,
    registry: Arc<crate::registry::Registry>,
    store: Arc<FlakyStore>,
    /// Whether [`GateHarness::shutdown`] has run, so the [`Drop`] backstop
    /// neither double-shuts a runtime nor retries a teardown that already
    /// failed and reported.
    torn_down: std::cell::Cell<bool>,
}

/// Backstop for the red path: a failing assertion in any condition unwinds
/// past the orchestrator's explicit [`GateHarness::shutdown`], and without
/// this the leaked runtime would outlive the red test for the rest of the
/// binary. A `Drop` can neither propagate the error nor panic mid-unwind, so
/// a teardown failure here is REPORTED on stderr — beside the failing test's
/// own output — rather than swallowed.
impl Drop for GateHarness {
    fn drop(&mut self) {
        if self.torn_down.get() {
            return;
        }
        if let Err(error) = self.runtime.shutdown() {
            eprintln!("GateHarness: teardown after a failing condition also failed: {error}");
        }
    }
}

impl GateHarness {
    /// Build the world and install BOTH NIF contexts — the runtime context the
    /// durable NIFs resolve through and the determinism context the fixture's
    /// registration poll depends on (see [`install_determinism_context`]).
    fn build() -> Result<Self, Box<dyn std::error::Error>> {
        let tokio_runtime = tokio::runtime::Runtime::new()?;
        let runtime = live_runtime()?;
        let registry = Arc::new(crate::registry::Registry::default());
        let store = Arc::new(FlakyStore::new());
        crate::runtime::install_nif_runtime_context(
            runtime.nif_state(),
            Arc::clone(&registry),
            Arc::clone(&runtime),
            tokio_runtime.handle().clone(),
        );
        install_determinism_context(
            &runtime,
            &registry,
            &(Arc::clone(&store) as Arc<dyn EventStore>),
            &tokio_runtime,
        );
        Ok(Self {
            tokio_runtime,
            runtime,
            registry,
            store,
            torn_down: std::cell::Cell::new(false),
        })
    }

    /// Explicit teardown for the green path — propagates the error a `Drop`
    /// cannot. The flag is set BEFORE the attempt so a shutdown that fails
    /// here (and is reported through `?`) is not retried by the backstop.
    fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.torn_down.set(true);
        Ok(self.runtime.shutdown()?)
    }

    /// Spawn a fixture process that waits for registration and then calls
    /// `continue_as_new` through the NIF under test.
    ///
    /// Spawned inside each condition rather than up front: the fixture's
    /// registration wait is bounded at 3 s, and a process spawned before an
    /// earlier condition's settle budget had run would time its wait out and
    /// park without ever calling.
    fn spawn_continuer(&self) -> Result<crate::Pid, Box<dyn std::error::Error>> {
        Ok(self.runtime.spawn_workflow(
            "aion_continue_fixture",
            "await_then_continue",
            crate::runtime::handle::RuntimeInput::default(),
        )?)
    }

    /// Register a live run behind `pid` — the starting gun that releases the
    /// fixture's wait. Everything a condition wants the durable call to see
    /// must be arranged before this returns; see [`register_live_run`].
    fn register(
        &self,
        pid: crate::Pid,
        deadline: &SeededDeadline,
    ) -> Result<WorkflowId, Box<dyn std::error::Error>> {
        register_live_run(
            &self.tokio_runtime,
            &self.registry,
            Arc::clone(&self.store) as Arc<dyn EventStore>,
            pid,
            deadline,
        )
    }

    /// The run's recorded history.
    fn history(&self, workflow: &WorkflowId) -> Result<Vec<Event>, Box<dyn std::error::Error>> {
        Ok(self
            .tokio_runtime
            .block_on(self.store.recorded_history(workflow))?)
    }
}

/// CONTROL 2: a refusal the predicate SPARES leaves the process alive.
///
/// Runs FIRST because it needs the epoch open. The refusal is a store fault
/// raised BEFORE the terminal — the one `Err` class the gate must NOT act on.
/// This is the only assertion in the file that separates "the gate fired" from
/// "making this call kills you"; CONTROL 1 cannot see that difference.
fn control_two_spared_refusal(h: &GateHarness) -> TestResult {
    let spared = h.spawn_continuer()?;
    // 🔴 ARMED BEFORE THE STARTING GUN, NOT AFTER IT. Registration is what
    // releases the fixture, and the fixture polls every 5 ms; arming after
    // it returns is a race the instrument can lose, and losing it means the call
    // runs unfaulted and the test fails blaming the fixture. The budget skips
    // exactly the one append this deadline-free registration makes —
    // `record_workflow_started` — and refuses the next, which is
    // `record_continuation`'s terminal.
    h.store.fail_appends_after(1, 1);
    let spared_workflow = h.register(spared, &SeededDeadline::None)?;

    // The fault being SPENT is what proves the durable call happened at all. A
    // refusal writes nothing, so an unmade call and a refused one leave
    // identical histories; without this the control's survival would be
    // consistent with the fixture never reaching the NIF.
    //
    // 🔴 AN APPEND FAULT, NOT A READ FAULT. The first draft injected a read
    // failure and this control died anyway: the fixture's own registration poll
    // spends the read budget, so the witness fired for the WRONG caller and the
    // durable call then ran against a healthy store, recorded its terminal, and
    // was correctly killed for it. Only the durable half appends, and inside
    // this window nothing else appends at all, so this budget can only be spent
    // by the call under test.
    assert!(
        settles(|| h.store.unspent_append_failures() == 0),
        "CONTROL 2 never reached the durable half — the injected APPEND failure was never spent, \
         so this process's survival says nothing about the gate"
    );
    // Same budget as the treatment's death, deliberately: see `SETTLE_BUDGET`.
    // The witness above fires strictly before the `cancel_pid` a mutated gate
    // would reach, so a single check here would catch that mutation only by luck.
    //
    // 🔴 THE FAILURE MESSAGE CARRIES THE DIAGNOSIS, because "the process died"
    // on its own distinguishes nothing: a gate mutated to fire unconditionally,
    // a terminal that actually landed (Ok(Complete) → the gate fired CORRECTLY),
    // and a process dying on its own (the failure that made this test's first
    // incarnation vacuous) all look identical from the liveness probe. The two
    // facts that separate them — did the terminal land, and was the fault
    // spent — are read here so a red run arrives with its own attribution
    // instead of demanding a re-instrumented re-run that may not reproduce.
    //
    // A tracing capture is deliberately NOT installed. The crate's one capture
    // facility (`log_capture`) is thread-scoped by design and its own header
    // states the limit that governs here: an emission made on a different
    // thread is invisible to it, and the NIF's `error!` fires on a beamr
    // scheduler thread. A global capturing subscriber is the alternative that
    // module exists to forbid — parallel tests on one process cross-contaminate
    // a capturing global in both directions. The history + budget read above
    // answers the same question the log line would.
    let spared_died = settles(|| !h.runtime.is_live(spared));
    let spared_history = h.history(&spared_workflow)?;
    assert!(
        !spared_died,
        "CONTROL 2: a store failure raised BEFORE the terminal must leave the workflow \
         process ALIVE, and must go on leaving it alive for as long as the treatment is \
         given to die — terminating here would turn a transient backend blip into a dead \
         run, the mirror of the defect the gate exists to prevent. Diagnosis: terminal in \
         history = {terminal} (true means the fault was NOT taken by the terminal append \
         and the gate fired correctly for a landed terminal — suspect the fault-arming \
         window, or a stale test binary: a restore that preserves an old mtime leaves the \
         previous mutation's binary looking fresh); unspent append failures = {unspent} \
         (non-zero means the durable call never happened and the death is the harness's, \
         not the gate's); history at death: {spared_history:#?}",
        terminal = continued(&spared_history),
        unspent = h.store.unspent_append_failures(),
    );
    assert!(
        !continued(&spared_history),
        "CONTROL 2's refusal must also be pre-terminal, or it is testing the Ok arm under an Err \
         name: {spared_history:#?}"
    );
    Ok(())
}

/// TREATMENT A: a HALF-COMPLETED transition must end the process.
///
/// The input class here is `Ok(TerminalOnly)` — the terminal LANDS and the
/// deadline retirement behind it is refused — and it is the one class a
/// fault-free engine can never produce, which is why it needs the injector
/// and why it was missed: Review 22 mutated the gate to spare exactly this
/// arm and every test in the crate stayed green. `ContinuationOutcome`
/// exists ONLY to carry this decision; a gate that never consumes the
/// variant under test is decoration. Runs while the epoch is still OPEN,
/// because its terminal must land.
fn treatment_a_terminal_only(h: &GateHarness) -> TestResult {
    let half_completed = h.spawn_continuer()?;
    // Armed before the starting gun, as in CONTROL 2. This registration makes
    // TWO appends — `record_workflow_started`, then the deadline seed — and the
    // NIF's transition makes two more: the terminal, which must SUCCEED, and
    // the retirement of the seeded deadline, which must fail. So the budget
    // skips three and refuses the fourth. Nothing else appends in this window:
    // CONTROL 2's process has already made its one call and parks, `park_only`
    // never calls, and no timer service runs in this harness.
    h.store.fail_appends_after(3, 1);
    let half_workflow = h.register(half_completed, &SeededDeadline::Outstanding)?;
    assert!(
        settles(|| h.store.unspent_append_failures() == 0),
        "TREATMENT A never reached the deadline retirement — the injected APPEND failure was \
         never spent, so nothing here measured the TerminalOnly arm. Either the fixture never \
         called, or the terminal append itself was refused (which is CONTROL 2's condition, \
         not this one)"
    );
    // Same diagnosis discipline as CONTROL 2, because "the process died" and
    // "the process lived" each cover three unlike causes. The two history
    // facts asserted after the death separate them: a death WITHOUT a durable
    // terminal is the epoch treatment's shape leaking in; a death with the
    // deadline RETIRED is the Complete arm wearing this one's name.
    let half_died = settles(|| !h.runtime.is_live(half_completed));
    let half_history = h.history(&half_workflow)?;
    assert!(
        half_died,
        "TREATMENT A: a transition whose terminal LANDED must end the calling process even \
         though the deadline retirement failed — this is the gate consuming Ok(TerminalOnly), \
         the arm the type exists for. A process left alive here keeps writing timers, \
         activities, children and signals into a history that already holds its terminal, \
         concurrently with the successor the sweep will start. Diagnosis: terminal in history \
         = {terminal} (false means the terminal append was refused and this measured the \
         wrong arm); timer retired = {retired} (true means the transition COMPLETED and this \
         measured the Complete arm); unspent append failures = {unspent}; history: \
         {half_history:#?}",
        terminal = continued(&half_history),
        retired = retired_a_timer(&half_history),
        unspent = h.store.unspent_append_failures(),
    );
    assert!(
        continued(&half_history),
        "TREATMENT A's terminal must have LANDED — a death without a durable terminal is the \
         epoch refusal's condition wearing this one's name: {half_history:#?}"
    );
    assert!(
        !retired_a_timer(&half_history),
        "TREATMENT A's deadline retirement must have FAILED — a retired deadline means the \
         transition completed and this measured Ok(Complete), not Ok(TerminalOnly): \
         {half_history:#?}"
    );
    Ok(())
}

/// TREATMENT B: a COMPLETED transition must end the process.
///
/// No fault at all: the terminal lands, the retirement lands, the outcome
/// is `Ok(Complete)` — and the gate must still end the process, because a
/// spared one is a live writer on a closed history AND a stranded chain
/// (the successor starts only from the process-exit monitor). There is no
/// fault-spend witness here; the witness is the history itself, which a
/// completed transition writes and a refused one does not.
fn treatment_b_complete(h: &GateHarness) -> TestResult {
    let completed = h.spawn_continuer()?;
    let completed_workflow = h.register(completed, &SeededDeadline::Outstanding)?;
    let completed_died = settles(|| !h.runtime.is_live(completed));
    let completed_history = h.history(&completed_workflow)?;
    assert!(
        completed_died,
        "TREATMENT B: a transition that COMPLETED — terminal durable, deadline retired — must \
         end the calling process; this is the gate consuming Ok(Complete), the ordinary path \
         every real continue-as-new takes. Diagnosis: terminal in history = {terminal}; timer \
         retired = {retired} (false/false means the fixture never made the call and this \
         measured the harness); history: {completed_history:#?}",
        terminal = continued(&completed_history),
        retired = retired_a_timer(&completed_history),
    );
    assert!(
        continued(&completed_history) && retired_a_timer(&completed_history),
        "TREATMENT B's transition must have run to completion — terminal AND retirement \
         durable — or the death above was measured under the wrong input class: \
         {completed_history:#?}"
    );
    Ok(())
}

/// TREATMENT C: a refused terminal must end the calling process.
///
/// 🔴 FLIPS THE SHARED EPOCH LATCH, so it must run LAST of the ordered
/// conditions. The epoch closes BEFORE the run is registered, and the fixture
/// waits for registration, so the ordering is guaranteed rather than raced:
/// the one durable call the fixture makes cannot land while the epoch is
/// still open.
fn treatment_c_epoch_refusal(h: &GateHarness) -> TestResult {
    let refused = h.spawn_continuer()?;
    h.runtime.engine_tasks().begin_close();
    let refused_workflow = h.register(refused, &SeededDeadline::None)?;

    assert!(
        settles(|| !h.runtime.is_live(refused)),
        "the epoch refusal must END the workflow process: it holds the run's only Recorder, and \
         every durable NIF it goes on to call writes into a history this closing engine may no \
         longer own"
    );
    let refused_history = h.history(&refused_workflow)?;
    assert!(
        !continued(&refused_history),
        "the refusal must leave history UNMOVED — the whole point of gating before the first \
         append: {refused_history:#?}"
    );
    Ok(())
}

/// 🔴 ONE RUNTIME, ONE REGISTRY, ONE STORE — THE TREATMENT AND ITS CONTROLS
/// DIFFER BY THE REFUSAL AND NOTHING ELSE.
///
/// An earlier draft ran CONTROL 2 on a second `RuntimeHandle` with its own
/// `Registry` and its own store, and still described itself as differing "in
/// exactly one respect". It differed in four, and all of the attribution rested
/// on it. Everything shareable is now shared, and the two conditions differ only
/// in why `record_continuation` returns `Err`: an armed append fault for the
/// control, a closed epoch for the treatment.
///
/// **The epoch is the one thing that cannot be held equal**, because it is a
/// single latch on the shared runtime and the conditions need it in opposite
/// states. So they are ORDERED rather than simultaneous: CONTROL 2 and
/// TREATMENTS A and B run while it is open (the control's refusal is a store
/// fault; both treatments' terminals must LAND), TREATMENT C after
/// `begin_close()`. CONTROL 1's parked process spans all of it and is asserted
/// at the end, so a harness that changed character partway cannot go unnoticed.
///
/// **The NIF sees all four of the predicate's input classes here** — spared
/// `Err` (CONTROL 2), `Ok(TerminalOnly)` (TREATMENT A), `Ok(Complete)`
/// (TREATMENT B), ending `Err` (TREATMENT C). TREATMENT B exists because the
/// comfortable belief that `tests/continue_as_new.rs` owned the `Ok(Complete)`
/// arm was MEASURED AND REFUTED: a gate mutated to spare exactly that arm left
/// those three tests green in half a second — they never drive this NIF — and
/// left every other test green too. A cleanly-continued process the gate
/// spares is a stranded chain: the successor only ever starts from the
/// process-exit monitor, so no exit means no successor, plus a live writer on
/// a history that already holds its terminal.
///
/// **What each control can and cannot see** — worth stating, because CONTROL 1
/// alone is not enough and was once presented as though it were:
/// - CONTROL 1 (`park_only`) shows only that the harness can keep SOME process
///   alive. It is blind to the exact failure that made the first incarnation of
///   this test vacuous: under a harness missing `install_nifs`, `park_only` LIVED
///   and only NIF-calling processes died, so CONTROL 1 passes unchanged in a
///   world where every call is fatal.
/// - CONTROL 2 is the one that closes that gap, because it CALLS the NIF, is
///   refused, and survives. It is the only assertion here that separates "the
///   gate fired" from "making this call kills you".
#[test]
fn the_nif_gate_ends_a_live_process_for_each_outcome_that_must_end_it_and_spares_the_rest()
-> TestResult {
    let harness = GateHarness::build()?;

    // ---- CONTROL 1: the harness can keep a workflow process alive. ----------
    // Spawned first and asserted last, so it spans every condition.
    let parked = harness.runtime.spawn_workflow(
        "aion_continue_fixture",
        "park_only",
        crate::runtime::handle::RuntimeInput::default(),
    )?;

    // Ordered rather than simultaneous — the epoch is a single latch on the
    // shared runtime, open for the first three conditions, closed by the
    // fourth. Each condition is its own function; their doc comments carry
    // what each one proves and cannot prove.
    control_two_spared_refusal(&harness)?;
    treatment_a_terminal_only(&harness)?;
    treatment_b_complete(&harness)?;
    treatment_c_epoch_refusal(&harness)?;

    assert!(
        harness.runtime.is_live(parked),
        "CONTROL 1: a workflow process that never calls the NIF must survive the whole test. If \
         this dies, none of the results above say anything about cancel_pid — though note it \
         survives a harness in which every CALLING process dies, which is why CONTROL 2 exists"
    );
    harness.shutdown()?;
    Ok(())
}
