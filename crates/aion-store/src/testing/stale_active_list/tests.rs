//! The instrument has to be proven before anything is measured with it.
//!
//! These tests establish the three properties every use of
//! [`StaleActiveListStore`] relies on: that forcing an id genuinely defeats the
//! inner filter, that forcing NOTHING leaves an honest store (so a test that
//! forgets to force gets a real answer rather than a lie), and that the
//! delegation is real rather than a second empty store standing beside the one
//! the test wrote to.

use aion_core::{Event, EventEnvelope, Payload, RunId, WorkflowId, WorkflowStatus};
use serde_json::json;
use uuid::Uuid;

use super::StaleActiveListStore;
use crate::{ReadableEventStore, WritableEventStore, WriteToken};

type TestResult = Result<(), Box<dyn std::error::Error>>;
type TestResult2<T> = Result<T, Box<dyn std::error::Error>>;

fn workflow_id(value: u128) -> WorkflowId {
    WorkflowId::new(Uuid::from_u128(value))
}

/// Fallible so the two conversions inside are propagated rather than defaulted —
/// a silently-defaulted timestamp would give every seeded history one instant.
fn envelope(seq: u64, workflow_id: &WorkflowId) -> TestResult2<EventEnvelope> {
    Ok(EventEnvelope {
        seq,
        recorded_at: chrono::DateTime::from_timestamp(1_700_000_000 + i64::try_from(seq)?, 0)
            .ok_or("timestamp out of range")?,
        workflow_id: workflow_id.clone(),
    })
}

fn started(seq: u64, workflow_id: &WorkflowId) -> TestResult2<Event> {
    Ok(Event::WorkflowStarted {
        envelope: envelope(seq, workflow_id)?,
        workflow_type: String::from("probe"),
        input: Payload::from_json(&json!({}))?,
        run_id: RunId::new(Uuid::from_u128(1)),
        parent_run_id: None,
        parent_workflow_id: None,
        package_version: aion_core::PackageVersion::new("a".repeat(64)),
    })
}

fn completed(seq: u64, workflow_id: &WorkflowId) -> TestResult2<Event> {
    Ok(Event::WorkflowCompleted {
        envelope: envelope(seq, workflow_id)?,
        result: Payload::from_json(&json!({}))?,
    })
}

fn paused(seq: u64, workflow_id: &WorkflowId) -> TestResult2<Event> {
    Ok(Event::WorkflowPaused {
        envelope: envelope(seq, workflow_id)?,
        run_id: RunId::new(Uuid::from_u128(1)),
        reason: None,
        operator: None,
    })
}

/// Seed a run that is Running, and one that has completed, through the double
/// itself — so the delegation to the inner store is exercised by the seeding and
/// not merely asserted.
async fn seed(store: &StaleActiveListStore) -> TestResult2<(WorkflowId, WorkflowId)> {
    let running = workflow_id(1);
    let finished = workflow_id(2);
    store
        .append(
            WriteToken::recorder(),
            &running,
            &[started(1, &running)?],
            0,
        )
        .await?;
    store
        .append(
            WriteToken::recorder(),
            &finished,
            &[started(1, &finished)?, completed(2, &finished)?],
            0,
        )
        .await?;
    Ok((running, finished))
}

/// The property the whole instrument exists for: a run the inner store's filter
/// excludes is reported active anyway, while every other answer stays honest.
#[tokio::test]
async fn a_forced_terminal_id_is_reported_active_while_its_history_still_says_terminal()
-> TestResult {
    let store = StaleActiveListStore::new();
    let (running, finished) = seed(&store).await?;

    // Baseline FIRST, computed rather than assumed: without forcing, the double
    // must already agree with an honest store. If this half were wrong the
    // assertion below would be satisfied by a store that lists everything.
    let honest = store.list_active().await?;
    assert!(
        honest.contains(&running),
        "the Running run must be listed before forcing, or the double is not delegating"
    );
    assert!(
        !honest.contains(&finished),
        "the completed run must be EXCLUDED before forcing — this is the inner \
         filter the instrument exists to defeat, and if it were already absent \
         the forcing below would prove nothing"
    );

    store.force_active(finished.clone());

    let forced = store.list_active().await?;
    assert!(
        forced.contains(&finished),
        "forcing must defeat the inner `== Running` filter"
    );
    assert!(
        forced.contains(&running),
        "forcing one id must not displace the honest answer"
    );

    // The lie is confined to `list_active`. The reader's own defence — the
    // per-run re-read — must still report the truth, or the guard under test
    // would never see the contradiction it exists to catch.
    let status = aion_core::status_from_events(&store.read_history(&finished).await?);
    assert_eq!(
        status,
        WorkflowStatus::Completed,
        "the forced run's history must still project terminal; a double that also \
         rewrote history would hide the very inconsistency being injected"
    );

    Ok(())
}

/// The #204 shape: `Paused` is non-terminal, so a reader that only checks
/// `is_terminal` lets it through. The double must be able to force it.
#[tokio::test]
async fn a_forced_paused_id_is_reported_active_and_still_projects_paused() -> TestResult {
    let store = StaleActiveListStore::new();
    let (_running, _finished) = seed(&store).await?;

    let held = workflow_id(3);
    store
        .append(
            WriteToken::recorder(),
            &held,
            &[started(1, &held)?, paused(2, &held)?],
            0,
        )
        .await?;

    assert!(
        !store.list_active().await?.contains(&held),
        "a paused run must be excluded before forcing"
    );
    assert!(
        store.list_paused().await?.contains(&held),
        "list_paused must still be honest — the double lies in exactly one place"
    );

    store.force_active(held.clone());

    assert!(
        store.list_active().await?.contains(&held),
        "forcing must defeat the filter for Paused as well as terminal"
    );
    let status = aion_core::status_from_events(&store.read_history(&held).await?);
    assert_eq!(
        status,
        WorkflowStatus::Paused,
        "the forced run must still project Paused, non-terminal — which is exactly \
         why an `is_terminal` check alone does not stop it"
    );

    Ok(())
}

/// The control on the instrument itself. A double that lies unconditionally
/// would pass both tests above; this is what distinguishes it from one that
/// lies only when asked.
#[tokio::test]
async fn forcing_nothing_leaves_the_answer_identical_to_the_inner_store() -> TestResult {
    let store = StaleActiveListStore::new();
    let (running, finished) = seed(&store).await?;

    let mine = store.list_active().await?;
    assert_eq!(
        mine,
        vec![running],
        "with nothing forced the answer must be exactly the honest one — a test \
         that forgets to call force_active must get truth, not a silent lie"
    );
    assert!(!mine.contains(&finished));

    Ok(())
}

/// Forcing an id the store would have listed anyway must not duplicate it — a
/// reader that iterates `list_active` would otherwise recover the same run twice
/// and the duplication, not the guard, would explain the outcome.
#[tokio::test]
async fn forcing_an_already_active_id_does_not_duplicate_it() -> TestResult {
    let store = StaleActiveListStore::new();
    let (running, _finished) = seed(&store).await?;

    store.force_active(running.clone());

    let active = store.list_active().await?;
    assert_eq!(
        active.iter().filter(|id| **id == running).count(),
        1,
        "the honest answer and the forced set must be unioned, not concatenated"
    );

    Ok(())
}

/// Two forced ids must both survive, in the order forced — a reader recovering
/// them in a stated order needs the input order to be stated too.
#[tokio::test]
async fn multiple_forced_ids_are_appended_in_the_order_they_were_forced() -> TestResult {
    let store = StaleActiveListStore::new();
    let (running, finished) = seed(&store).await?;

    let held = workflow_id(3);
    store
        .append(
            WriteToken::recorder(),
            &held,
            &[started(1, &held)?, paused(2, &held)?],
            0,
        )
        .await?;

    store.force_active(held.clone());
    store.force_active(finished.clone());

    assert_eq!(
        store.list_active().await?,
        vec![running, held, finished],
        "honest answer first, then each forced id in the order it was forced"
    );

    Ok(())
}
