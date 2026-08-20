//! Run-awareness of the query mailbox seam's `resolve_workflow` (#105).
//!
//! After a continue-as-new two handles share one workflow id — on the nif path
//! the predecessor's handle is never removed, and on the API path
//! `lifecycle::continue_as_new` records the terminal, starts the successor, and
//! only then removes it. These tests pin that the seam names the CURRENT run
//! rather than whichever handle a `HashMap` scan happened to yield.
//!
//! 🔴 THE DEFECT THESE GUARD IS NONDETERMINISTIC. The replaced implementation
//! scanned `Registry::list`, i.e. `HashMap::values()` under `RandomState`, so it
//! returned the right handle about half the time. The tests below are
//! deterministic on the fix; a MUTATION CHECK against the old scan is not, and
//! must be repeated — one green mutant run is luck, not evidence of vacuity.
//! Recorded red count when the fix was reverted: see `gate-logs/105-*`.

use std::sync::{Arc, Weak};

use aion_core::{RunId, WorkflowId, WorkflowStatus};
use aion_package::ContentHash;
use aion_store::InMemoryStore;

use super::*;
use crate::durability::Recorder;
use crate::engine_seam::{EngineHandle, WorkflowProcessHandle, WorkflowResidency};
use crate::registry::{
    CompletionNotifier, HandleResidency, Registry, WorkflowHandle, WorkflowHandleParts,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A predecessor's pid and a successor's pid, chosen distinct so an assertion
/// on the resolved pid names WHICH run answered rather than merely that one did.
const PREDECESSOR_PID: u64 = 4_001;
const SUCCESSOR_PID: u64 = 4_002;

fn handle_for(
    store: &Arc<InMemoryStore>,
    workflow_id: &WorkflowId,
    run_id: &RunId,
    pid: u64,
    cached_status: WorkflowStatus,
) -> WorkflowHandle {
    WorkflowHandle::new(WorkflowHandleParts {
        workflow_id: workflow_id.clone(),
        run_id: run_id.clone(),
        pid,
        workflow_type: "checkout".to_owned(),
        namespace: String::from("default"),
        loaded_version: ContentHash::from_bytes([9; 32]),
        cached_status,
        residency: HandleResidency::Resident,
        recorder: Recorder::resume_at(workflow_id.clone(), Arc::clone(store) as _, 0),
        completion: CompletionNotifier::new(),
    })
}

fn seam(registry: &Arc<Registry>) -> QueryMailboxEngine {
    // `resolve_workflow` reads only the registry, so the other two seats are
    // deliberately empty rather than faked: a stub here could be consulted by a
    // future edit without the test noticing, and an empty `Weak` cannot be.
    QueryMailboxEngine::new(Arc::clone(registry), Weak::new(), Weak::new())
}

/// The successor answers, not whichever handle the map scan yielded.
#[test]
fn a_continued_workflow_resolves_to_its_current_run() -> TestResult {
    let registry = Arc::new(Registry::default());
    let store = Arc::new(InMemoryStore::default());
    let workflow_id = WorkflowId::new_v4();
    let predecessor_run = RunId::new_v4();
    let successor_run = RunId::new_v4();

    // Insertion order is the live continue-as-new order: the predecessor was
    // registered long before the successor was started.
    registry.insert(
        (workflow_id.clone(), predecessor_run.clone()),
        handle_for(
            &store,
            &workflow_id,
            &predecessor_run,
            PREDECESSOR_PID,
            WorkflowStatus::ContinuedAsNew,
        ),
    )?;
    registry.insert(
        (workflow_id.clone(), successor_run.clone()),
        handle_for(
            &store,
            &workflow_id,
            &successor_run,
            SUCCESSOR_PID,
            WorkflowStatus::Running,
        ),
    )?;

    // Asserting the SUCCESSOR'S PID, not merely `Resident`: the predecessor is
    // also resident here, so a residency-only assertion would pass on the
    // defect whenever the scan happened to pick the predecessor.
    assert_eq!(
        seam(&registry).resolve_workflow(&workflow_id)?,
        WorkflowResidency::Resident(WorkflowProcessHandle::new(SUCCESSOR_PID)),
        "the seam must name the current run, not whichever handle a HashMap scan yielded"
    );
    Ok(())
}

/// A workflow whose current run has been removed is `Unknown`, even while a
/// predecessor's handle is still registered.
///
/// This is the ONE deliberate behaviour change in #105 and it is pinned rather
/// than left to be discovered. The replaced scan found the stale predecessor,
/// read its terminal `cached_status`, and answered `Terminal` — which
/// `QueryService` maps to `QueryError::NotRunning`. Consulting the index
/// answers `Unknown`, which maps to `QueryError::Unknown`. Both REFUSE the
/// query; `Unknown` is the accurate one when no run is current.
///
/// Reachable through the public API exactly as written: the nif continue-as-new
/// path never removes the predecessor's handle, so once the successor is
/// removed, `forget_live_index_entry` drops the index entry it owned and leaves
/// a live handle behind it.
#[test]
fn a_stale_predecessor_handle_without_an_index_entry_is_unknown() -> TestResult {
    let registry = Arc::new(Registry::default());
    let store = Arc::new(InMemoryStore::default());
    let workflow_id = WorkflowId::new_v4();
    let predecessor_run = RunId::new_v4();
    let successor_run = RunId::new_v4();

    registry.insert(
        (workflow_id.clone(), predecessor_run.clone()),
        handle_for(
            &store,
            &workflow_id,
            &predecessor_run,
            PREDECESSOR_PID,
            WorkflowStatus::ContinuedAsNew,
        ),
    )?;
    registry.insert(
        (workflow_id.clone(), successor_run.clone()),
        handle_for(
            &store,
            &workflow_id,
            &successor_run,
            SUCCESSOR_PID,
            WorkflowStatus::Running,
        ),
    )?;
    registry.remove(&workflow_id, &successor_run)?;

    // The predecessor's handle is still registered — the premise of the test,
    // asserted rather than assumed, because a `remove` that also dropped it
    // would make this case vacuous while staying green.
    assert!(
        registry.get(&workflow_id, &predecessor_run)?.is_some(),
        "premise: the predecessor's handle outlives the successor's removal"
    );
    assert_eq!(
        seam(&registry).resolve_workflow(&workflow_id)?,
        WorkflowResidency::Unknown,
        "no current run means Unknown, not the stale predecessor's Terminal"
    );
    Ok(())
}

/// The live-run index is last-writer-wins by CALL ORDER, so registration order
/// is load-bearing for this seam.
///
/// ⚠️ THIS PINS A PRECONDITION, NOT A BLESSING. `Registry::insert` upserts the
/// index unconditionally, and its comment reads "the newest run for a workflow
/// id wins" — which is only the same thing while runs are inserted in age
/// order. There is no way to recover recency from the ids themselves: `RunId`
/// is a v4 UUID and carries no ordering. Call order is the only signal
/// available, so this is inherent rather than a defect with a fix.
///
/// Live continue-as-new cannot produce the reversed order — the predecessor is
/// registered long before the successor exists. This test exists so that if any
/// future path (recovery, resurrection, an out-of-order sweep) ever registers a
/// superseded run last, the dependency is already written down at the seam that
/// relies on it instead of being rediscovered from a wrong answer in
/// production.
#[test]
fn the_live_run_index_follows_registration_order() -> TestResult {
    let registry = Arc::new(Registry::default());
    let store = Arc::new(InMemoryStore::default());
    let workflow_id = WorkflowId::new_v4();
    let predecessor_run = RunId::new_v4();
    let successor_run = RunId::new_v4();

    // Reversed against the live order: the successor is registered FIRST.
    registry.insert(
        (workflow_id.clone(), successor_run.clone()),
        handle_for(
            &store,
            &workflow_id,
            &successor_run,
            SUCCESSOR_PID,
            WorkflowStatus::Running,
        ),
    )?;
    registry.insert(
        (workflow_id.clone(), predecessor_run.clone()),
        handle_for(
            &store,
            &workflow_id,
            &predecessor_run,
            PREDECESSOR_PID,
            WorkflowStatus::ContinuedAsNew,
        ),
    )?;

    assert_eq!(
        registry.live_run_pid(&workflow_id)?,
        Some((predecessor_run, PREDECESSOR_PID)),
        "the index names the LAST run registered, whatever its age"
    );
    assert_eq!(
        seam(&registry).resolve_workflow(&workflow_id)?,
        WorkflowResidency::Terminal,
        "so a superseded run registered last would be the run this seam answers for"
    );
    Ok(())
}
