//! Epoch-gate tests for the durable half of a continue-as-new transition.
//!
//! These drive [`super::record_continuation`] directly rather than the NIF that
//! wraps it. That is the point of the split: the gate under test decides whether
//! a durable TERMINAL is appended, and a test that needed a live beamr process
//! to reach it would be too heavy to keep and too coarse to attribute.

use std::sync::Arc;

use aion_core::{Event, PackageVersion, Payload, RunId, WorkflowId};
use aion_store::{EventStore, InMemoryStore, ReadableEventStore};
use chrono::Utc;

use super::{record_continuation, refusal_must_end_the_process};
use crate::durability::{DurabilityError, Recorder, WorkflowStartRecord};
use crate::runtime::engine_tasks::EngineTaskRuntime;
use crate::runtime::nif_context::NifContextError;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Seed a live run with a declared-timeout deadline outstanding, returning a
/// recorder positioned at its head.
///
/// The deadline matters: `record_continuation` retires it as the second half of
/// the transition, so its retirement is what distinguishes "the transition ran"
/// from "the terminal happened to land".
async fn seed_running_run(
    store: &Arc<InMemoryStore>,
) -> Result<(Recorder, RunId), Box<dyn std::error::Error>> {
    let workflow_id = WorkflowId::new_v4();
    let run_id = RunId::new_v4();
    let deadline_id = crate::time::deadline_timer_id(&run_id)?;
    let mut recorder = Recorder::new(workflow_id, Arc::clone(store) as Arc<dyn EventStore>);
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
    let (mut control_recorder, control_run) = seed_running_run(&store).await?;
    let control_id = control_recorder.workflow_id().clone();
    record_continuation(
        &mut control_recorder,
        &tasks,
        &control_run,
        Payload::from_json(&serde_json::json!({"round": 2}))?,
    )
    .await?;
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
    let (mut recorder, run_id) = seed_running_run(&store).await?;
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
    let (mut recorder, run_id) = seed_running_run(&store).await?;
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

/// 🔴 THE EPOCH REFUSAL MUST ALSO KILL THE PROCESS — AND NO OTHER REFUSAL MAY.
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
/// **The controls are the whole test.** A predicate that answered `true`
/// unconditionally would satisfy the first assertion and close the exposure —
/// and would also kill a run for an ordinary authoring mistake, converting a
/// recoverable error into a dead workflow. So the negative cases are not
/// decoration: they are the half that distinguishes a gate from a hammer.
///
/// Killing mutations, both directions: make the predicate `true` for every
/// error and the `HistoryShape`/`RecorderPoisoned` assertions fail; make it
/// match `HistoryShape` instead of `EngineTaskEpochClosed` and the first fails.
#[test]
fn only_the_epoch_refusal_ends_the_workflow_process() {
    let epoch_closed = NifContextError::Durability(DurabilityError::EngineTaskEpochClosed {
        reason: "this engine has begun closing".to_owned(),
    });
    assert!(
        refusal_must_end_the_process(&epoch_closed),
        "a closed epoch must end the process: it is the one condition workflow code cannot \
         improve on and cannot outlive, and leaving the process alive leaves a durable writer \
         running against a store a successor may already own"
    );

    // CONTROL 1: the run simply already finished. The author may want to know.
    let already_terminal = NifContextError::Durability(DurabilityError::HistoryShape {
        reason: "run already recorded a terminal event".to_owned(),
    });
    assert!(
        !refusal_must_end_the_process(&already_terminal),
        "an already-terminal run is an ordinary error a workflow author may catch and report — \
         killing the process for it would turn an authoring mistake into a dead run"
    );

    // CONTROL 2: an infrastructure failure that is not the epoch. Same rule,
    // and reached through a DIFFERENT `NifContextError` variant, so the
    // predicate cannot pass by keying on `Durability` alone.
    let poisoned = NifContextError::RecorderPoisoned;
    assert!(
        !refusal_must_end_the_process(&poisoned),
        "a poisoned recorder is not this engine standing down; the process must be left to \
         handle it, not destroyed"
    );
}
