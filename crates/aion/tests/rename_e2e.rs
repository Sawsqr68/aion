//! End-to-end display-name/rename integration tests over `InMemoryStore` (#211).
//!
//! A display name is a LABEL over UUID identity, never an address. Renaming is
//! a RECORDED [`Event::SearchAttributesUpdated`] carrying the
//! `aion.display_name` attribute, so history keeps every name that has been
//! worn and the current name is the last-write-wins fold
//! ([`aion_core::display_name`]) — a fold over the whole WORKFLOW history, so
//! a continue-as-new successor inherits the name, even though a rename is
//! addressed to one run.
//!
//! These tests drive the real `Engine::rename_workflow` op over a real engine
//! and cover both append paths the single-writer invariant admits: a RESIDENT
//! run, which must append through its live handle's own recorder, and a
//! non-resident (terminal) run, which appends through a one-shot recorder at the
//! durable head. The resident case also proves the live recorder is not
//! desynced — a second writer at the same head would make the run's NEXT append
//! fail on the store's expected-sequence check.
//!
//! One posture cannot be reached by sequencing engine calls at all: the window
//! INSIDE a continue-as-new, between the predecessor's terminal append and the
//! successor's first one. [`WindowStore`] holds a chosen store operation at the
//! door so that window can be occupied deterministically, without a sleep and
//! without a race the test could win by luck.

mod common;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use aion::{Engine, EngineBuilder, EngineError};
use aion_core::{
    DISPLAY_NAME_ATTRIBUTE, Event, EventEnvelope, RunId, SearchAttributeSchema,
    SearchAttributeType, SearchAttributeValue, WorkflowId, WorkflowStatus, run_segment,
    status_from_events,
};
use aion_store::visibility::{ListWorkflowsFilter, VisibilityStore};
use aion_store::{
    EventStore, InMemoryStore, OutboxRow, PackageRecord, PackageRouteRecord, PackageStore,
    ReadableEventStore, RunSummary, StoreError, TimerEntry, TimerId, WorkflowFilter,
    WorkflowSummary, WritableEventStore, WriteToken,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::sync::Notify;

use common::{FIXTURE_MODULE, fixture_package, input_payload};

const NAME: &str = "Nightly settlement";
const RENAMED: &str = "Nightly settlement (rerun)";

/// The schema a server-embedded engine runs with, as far as this feature is
/// concerned: the recorder REFUSES an unregistered attribute, so an engine that
/// has not registered `aion.display_name` cannot record one.
fn schema() -> Result<SearchAttributeSchema, Box<dyn std::error::Error>> {
    let mut schema = SearchAttributeSchema::new();
    schema.register(DISPLAY_NAME_ATTRIBUTE, SearchAttributeType::String)?;
    Ok(schema)
}

/// An engine over `store`, projecting visibility into that SAME store so the
/// test reads the row the production recorder actually refreshed — not a
/// side-allocated projection nothing writes to.
async fn engine_over(
    store: &Arc<InMemoryStore>,
    entry_function: &str,
) -> Result<(Engine, String), Box<dyn std::error::Error>> {
    let package = fixture_package(entry_function)?;
    let hash = package.content_hash().to_string();
    let engine = EngineBuilder::new()
        .in_process_activity_serving()
        .store_arc(Arc::clone(store) as Arc<dyn EventStore>)
        .visibility_store_arc(Arc::clone(store) as Arc<dyn VisibilityStore>)
        .search_attribute_schema(schema()?)
        .scheduler_threads(1)
        .load_workflows(package)
        .build()
        .await?;
    Ok((engine, hash))
}

fn envelope(workflow_id: &WorkflowId, seq: u64) -> EventEnvelope {
    EventEnvelope {
        seq,
        recorded_at: Utc::now(),
        workflow_id: workflow_id.clone(),
    }
}

fn started(
    workflow_id: &WorkflowId,
    run_id: &RunId,
    hash: &str,
) -> Result<Event, Box<dyn std::error::Error>> {
    Ok(Event::WorkflowStarted {
        envelope: envelope(workflow_id, 1),
        workflow_type: FIXTURE_MODULE.to_owned(),
        input: input_payload()?,
        run_id: run_id.clone(),
        parent_run_id: None,
        package_version: aion_core::PackageVersion::new(hash.to_owned()),
    })
}

/// A terminal-`Completed` run: the durable posture in which no live recorder
/// can exist, so the one-shot append path is legitimately open.
async fn seed_completed(
    store: &Arc<InMemoryStore>,
    id: &WorkflowId,
    run: &RunId,
    hash: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    seed(
        store,
        id,
        vec![
            started(id, run, hash)?,
            Event::WorkflowCompleted {
                envelope: envelope(id, 2),
                result: input_payload()?,
            },
        ],
    )
    .await
}

async fn seed(
    store: &Arc<InMemoryStore>,
    workflow_id: &WorkflowId,
    events: Vec<Event>,
) -> Result<(), Box<dyn std::error::Error>> {
    seed_appended(store, workflow_id, events, 0).await
}

/// Seed onto an existing history at `expected_seq` — the same append the
/// recorder makes, so a fixture can extend a chain rather than only start one.
async fn seed_appended(
    store: &Arc<InMemoryStore>,
    workflow_id: &WorkflowId,
    events: Vec<Event>,
    expected_seq: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    store
        .append(WriteToken::recorder(), workflow_id, &events, expected_seq)
        .await?;
    Ok(())
}

/// Every recorded display name, oldest first — the history a rename appends to
/// rather than overwrites.
fn recorded_names(history: &[Event]) -> Vec<String> {
    history
        .iter()
        .filter_map(|event| match event {
            Event::SearchAttributesUpdated { attributes, .. } => {
                match attributes.get(DISPLAY_NAME_ATTRIBUTE) {
                    Some(SearchAttributeValue::String(name)) => Some(name.clone()),
                    _ => None,
                }
            }
            _ => None,
        })
        .collect()
}

/// The display name the visibility projection carries for `run`, which is what
/// the console's list rows read.
async fn projected_display_name(
    store: &Arc<InMemoryStore>,
    workflow_id: &WorkflowId,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let summaries = store.list_workflows(ListWorkflowsFilter::default()).await?;
    let Some(summary) = summaries
        .iter()
        .find(|summary| &summary.workflow_id == workflow_id)
    else {
        return Ok(None);
    };
    Ok(aion_core::display_name_from_attributes(
        &summary.search_attributes,
    ))
}

/// How long a fixture waits for the engine to reach a state before declaring it
/// stuck. Not a tuning knob and not a guess: it is this file's own established
/// liveness-wait bound, which `wait_until_not_live` has always used, named once
/// here so the store-hold waits below cannot drift away from it.
const FIXTURE_WAIT: Duration = Duration::from_secs(5);

fn wait_until_not_live(runtime: &aion::RuntimeHandle, pid: aion::Pid) -> bool {
    let deadline = Instant::now() + FIXTURE_WAIT;
    while Instant::now() < deadline {
        if !runtime.is_live(pid) {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    !runtime.is_live(pid)
}

/// A RESIDENT run renames through its live handle's own recorder: the event
/// lands in history, the projector reads the new name, the visibility row
/// carries it — and the run's NEXT append still succeeds, which is what proves
/// no second writer was created (a desynced recorder would fail its next
/// expected-sequence check).
#[tokio::test]
async fn renaming_a_resident_run_records_the_event_and_projects_the_new_name()
-> Result<(), Box<dyn std::error::Error>> {
    let store = Arc::new(InMemoryStore::default());
    let (engine, _hash) = engine_over(&store, "wait").await?;
    let handle = engine
        .start_workflow(
            FIXTURE_MODULE,
            input_payload()?,
            HashMap::new(),
            String::from("default"),
        )
        .await?;
    let (id, run) = (handle.workflow_id().clone(), handle.run_id().clone());

    // Unnamed until named: the run renders as its bare UUID.
    let history = store.read_history(&id).await?;
    assert_eq!(aion_core::display_name(&history), None);

    let recorded = engine.rename_workflow(&id, &run, NAME).await?;
    assert_eq!(recorded, NAME, "the op returns the name as recorded");

    let history = store.read_history(&id).await?;
    assert_eq!(recorded_names(&history), vec![NAME.to_owned()]);
    assert_eq!(aion_core::display_name(&history).as_deref(), Some(NAME));
    assert_eq!(
        aion_core::WorkflowSummary::from_history(&history)
            .and_then(|summary| summary.display_name)
            .as_deref(),
        Some(NAME),
        "the summary projection carries the recorded name"
    );
    assert_eq!(
        projected_display_name(&store, &id).await?.as_deref(),
        Some(NAME),
        "the visibility row the console lists carries the recorded name"
    );
    assert_eq!(
        status_from_events(&history),
        WorkflowStatus::Running,
        "a rename is a label, not a lifecycle transition"
    );

    // SINGLE WRITER: the live recorder is still in sync, so the run's next
    // append (the cancel terminal) succeeds rather than hitting a sequence
    // conflict.
    engine.cancel(&id, &run, "operator stop").await?;
    assert!(wait_until_not_live(engine.runtime(), handle.pid()));
    let history = store.read_history(&id).await?;
    assert!(
        matches!(history.last(), Some(Event::WorkflowCancelled { .. })),
        "the run's next append after a rename must still land: {:?}",
        history.last()
    );
    assert_eq!(
        aion_core::display_name(&history).as_deref(),
        Some(NAME),
        "the name outlives the run"
    );

    engine.shutdown()?;
    Ok(())
}

/// A second rename SUPERSEDES the first (last write wins) while history keeps
/// BOTH names — renaming appends, it never rewrites the past.
#[tokio::test]
async fn renaming_twice_supersedes_while_history_keeps_both_names()
-> Result<(), Box<dyn std::error::Error>> {
    let store = Arc::new(InMemoryStore::default());
    let (engine, _hash) = engine_over(&store, "wait").await?;
    let handle = engine
        .start_workflow(
            FIXTURE_MODULE,
            input_payload()?,
            HashMap::new(),
            String::from("default"),
        )
        .await?;
    let (id, run) = (handle.workflow_id().clone(), handle.run_id().clone());

    engine.rename_workflow(&id, &run, NAME).await?;
    engine.rename_workflow(&id, &run, RENAMED).await?;

    let history = store.read_history(&id).await?;
    assert_eq!(
        recorded_names(&history),
        vec![NAME.to_owned(), RENAMED.to_owned()],
        "history keeps every name the run has worn"
    );
    assert_eq!(aion_core::display_name(&history).as_deref(), Some(RENAMED));
    assert_eq!(
        projected_display_name(&store, &id).await?.as_deref(),
        Some(RENAMED)
    );

    engine.shutdown()?;
    Ok(())
}

/// A TERMINAL run has no registered handle, so the rename appends through a
/// one-shot recorder at the durable head. A completed run's label is as
/// legitimate as a running one's, and recording it must not disturb the
/// terminal status projection.
#[tokio::test]
async fn renaming_a_terminal_run_appends_at_the_head_without_changing_status()
-> Result<(), Box<dyn std::error::Error>> {
    let store = Arc::new(InMemoryStore::default());
    let (engine, hash) = engine_over(&store, "complete").await?;
    let (id, run) = (WorkflowId::new_v4(), RunId::new_v4());
    seed(
        &store,
        &id,
        vec![
            started(&id, &run, &hash)?,
            Event::WorkflowCompleted {
                envelope: envelope(&id, 2),
                result: input_payload()?,
            },
        ],
    )
    .await?;

    let recorded = engine.rename_workflow(&id, &run, NAME).await?;
    assert_eq!(recorded, NAME);

    let history = store.read_history(&id).await?;
    assert_eq!(history.len(), 3, "exactly one event was appended");
    match history.last() {
        Some(Event::SearchAttributesUpdated { envelope, .. }) => {
            assert_eq!(envelope.seq, 3, "the one-shot recorder appends at the head");
        }
        other => return Err(format!("expected SearchAttributesUpdated, found {other:?}").into()),
    }
    assert_eq!(aion_core::display_name(&history).as_deref(), Some(NAME));
    assert_eq!(
        status_from_events(&history),
        WorkflowStatus::Completed,
        "a rename never disturbs the status projection"
    );

    engine.shutdown()?;
    Ok(())
}

/// SINGLE WRITER, the non-resident half: a run that is durably `Running` but
/// has NO registered handle is mid-birth or mid-recovery, and its live recorder
/// owns the head. Renaming it must be REFUSED rather than served through a
/// second, one-shot recorder — a second writer at the same head would desync
/// the live one and hard-fail its next append.
///
/// A `Paused` run in the same registry state is the opposite case: it is
/// unregistered BY DESIGN (deliberately excluded from respawn), so no live
/// recorder exists and the rename is served.
#[tokio::test]
async fn renaming_a_running_but_non_resident_run_is_refused_not_double_written()
-> Result<(), Box<dyn std::error::Error>> {
    let store = Arc::new(InMemoryStore::default());
    let (engine, hash) = engine_over(&store, "complete").await?;

    // Durably Running, never registered — the recovery/birth window.
    let (running_id, running_run) = (WorkflowId::new_v4(), RunId::new_v4());
    seed(
        &store,
        &running_id,
        vec![started(&running_id, &running_run, &hash)?],
    )
    .await?;
    let before = store.read_history(&running_id).await?.len();

    match engine
        .rename_workflow(&running_id, &running_run, NAME)
        .await
    {
        Err(EngineError::InvalidState { reason }) => {
            assert!(
                reason.contains("not resident"),
                "the refusal must name the reason: {reason}"
            );
        }
        other => return Err(format!("expected InvalidState, got {other:?}").into()),
    }
    assert_eq!(
        store.read_history(&running_id).await?.len(),
        before,
        "a refused rename appends nothing"
    );

    // Durably Paused, also unregistered — but unregistered BY DESIGN, so the
    // one-shot path is open and the rename is SERVED.
    let (paused_id, paused_run) = (WorkflowId::new_v4(), RunId::new_v4());
    seed(
        &store,
        &paused_id,
        vec![
            started(&paused_id, &paused_run, &hash)?,
            Event::WorkflowPaused {
                envelope: envelope(&paused_id, 2),
                run_id: paused_run.clone(),
                reason: Some(String::from("operator hold")),
                operator: None,
            },
        ],
    )
    .await?;

    let recorded = engine
        .rename_workflow(&paused_id, &paused_run, NAME)
        .await?;
    assert_eq!(recorded, NAME);
    let history = store.read_history(&paused_id).await?;
    assert_eq!(aion_core::display_name(&history).as_deref(), Some(NAME));
    assert_eq!(
        status_from_events(&history),
        WorkflowStatus::Paused,
        "renaming a paused run leaves it paused"
    );

    engine.shutdown()?;
    Ok(())
}

/// SINGLE WRITER, the superseded-run half (the defect a review caught).
///
/// `SearchAttributesUpdated` carries NO run id and `run_segment` is positional,
/// so an event appended at the workflow head always lands in the LAST run's
/// segment. A `ContinuedAsNew` predecessor is terminal and unregistered, so a
/// status-only gate would let it through the one-shot path — and then the
/// append would go to the SUCCESSOR's head. That is three bugs at once: a
/// second writer behind the successor's live recorder, the operator's name
/// silently recorded onto the wrong run, and a visibility row written for the
/// predecessor carrying the successor's state.
///
/// Renaming a superseded run must therefore be REFUSED. Renaming the CURRENT
/// run of the same workflow is unaffected.
#[tokio::test]
async fn renaming_a_superseded_run_is_refused_not_recorded_onto_its_successor()
-> Result<(), Box<dyn std::error::Error>> {
    let store = Arc::new(InMemoryStore::default());
    let (engine, hash) = engine_over(&store, "complete").await?;
    let id = WorkflowId::new_v4();
    let (first, second) = (RunId::new_v4(), RunId::new_v4());

    // A continue-as-new chain: run `first` is terminal-ContinuedAsNew, run
    // `second` carries on under the SAME workflow id and owns the head.
    seed(
        &store,
        &id,
        vec![
            started(&id, &first, &hash)?,
            Event::WorkflowContinuedAsNew {
                envelope: envelope(&id, 2),
                input: input_payload()?,
                workflow_type: None,
                parent_run_id: first.clone(),
            },
            Event::WorkflowStarted {
                envelope: envelope(&id, 3),
                workflow_type: FIXTURE_MODULE.to_owned(),
                input: input_payload()?,
                run_id: second.clone(),
                parent_run_id: None,
                package_version: aion_core::PackageVersion::new(hash.clone()),
            },
            // The successor is terminal too, so the control arm below exercises
            // the superseded gate rather than the residency one.
            Event::WorkflowCompleted {
                envelope: envelope(&id, 4),
                result: input_payload()?,
            },
        ],
    )
    .await?;
    let before = store.read_history(&id).await?.len();

    // The predecessor IS terminal, so a status-only gate would admit it.
    assert!(
        status_from_events(run_segment(&store.read_history(&id).await?, &first)).is_terminal(),
        "the fixture must present a TERMINAL predecessor, or this test cannot          see the status-only gate let it through"
    );

    match engine.rename_workflow(&id, &first, NAME).await {
        Err(EngineError::InvalidState { reason }) => {
            assert!(
                reason.contains("superseded"),
                "the refusal must name the reason: {reason}"
            );
        }
        other => return Err(format!("expected InvalidState, got {other:?}").into()),
    }

    let history = store.read_history(&id).await?;
    assert_eq!(history.len(), before, "a refused rename appends nothing");
    assert_eq!(
        aion_core::display_name(&history),
        None,
        "nothing was recorded onto the successor"
    );

    // CONTROL: the CURRENT run of the SAME workflow, in the same registry and
    // status posture, renames normally — so the refusal above is attributable
    // to supersession and to nothing else the fixture shares.
    let recorded = engine.rename_workflow(&id, &second, NAME).await?;
    assert_eq!(recorded, NAME);
    let history = store.read_history(&id).await?;
    assert_eq!(aion_core::display_name(&history).as_deref(), Some(NAME));

    engine.shutdown()?;
    Ok(())
}

/// An engine over the window fixture: history through `store` (which can park
/// an operation at the door), visibility into the SAME inner store, so a test
/// can read the projection while an operation is held.
async fn engine_over_window(
    store: &Arc<WindowStore>,
    inner: &Arc<InMemoryStore>,
) -> Result<Arc<Engine>, Box<dyn std::error::Error>> {
    Ok(Arc::new(
        EngineBuilder::new()
            .in_process_activity_serving()
            .store_arc(Arc::clone(store) as Arc<dyn EventStore>)
            .visibility_store_arc(Arc::clone(inner) as Arc<dyn VisibilityStore>)
            .search_attribute_schema(schema()?)
            .scheduler_threads(1)
            .load_workflows(fixture_package("wait")?)
            .build()
            .await?,
    ))
}

/// SINGLE WRITER, the two-one-shots case: a rename and a paused-run SIGNAL both
/// build a `Recorder::resume_at` at the durable head of the SAME paused run
/// (`lifecycle/rename.rs` and `engine/delegated.rs` construct it identically),
/// so they can collide there.
///
/// The window is held open rather than raced for: the rename is parked on its
/// pre-lock validation read, so the head it carries forward is provably stale by
/// the time it appends — the signal has already taken that sequence. Racing two
/// tasks and hoping would prove nothing about which one lost.
///
/// What this pins is the INVARIANT, not which verb wins: the store's
/// expected-sequence fence admits exactly ONE append at a given head, the loser
/// records NOTHING, and the history is left contiguous with no event written
/// twice and none silently dropped. That is a correctly-refused race rather than
/// a double-writer hazard — the fence is the serialization point, and neither
/// writer ever proceeds on stale state.
///
/// Killing mutation: have the one-shot rename path re-read the head immediately
/// before appending instead of carrying its validated one. Both appends then
/// land and the assertion that exactly one did fails.
#[tokio::test]
async fn a_rename_and_a_paused_run_signal_at_the_same_head_are_fenced_not_double_written()
-> Result<(), Box<dyn std::error::Error>> {
    let inner = Arc::new(InMemoryStore::default());
    let store = Arc::new(WindowStore::new(Arc::clone(&inner)));
    let engine = engine_over_window(&store, &inner).await?;
    let hash = fixture_package("wait")?.content_hash().to_string();
    let (id, run) = (WorkflowId::new_v4(), RunId::new_v4());

    // Durably Paused and unregistered BY DESIGN: the exact posture in which both
    // verbs are allowed to build a one-shot recorder.
    seed(
        &inner,
        &id,
        vec![
            started(&id, &run, &hash)?,
            Event::WorkflowPaused {
                envelope: envelope(&id, 2),
                run_id: run.clone(),
                reason: Some(String::from("operator hold")),
                operator: None,
            },
        ],
    )
    .await?;
    let head_before = inner.read_history(&id).await?.len();

    // 1. Park the rename on its validation read, fixing the head it will resume
    //    at while the history is still two events long.
    store.history_read.arm(&id)?;
    let rename = tokio::spawn({
        let (engine, id, run) = (Arc::clone(&engine), id.clone(), run.clone());
        DRIVER.scope(Driver::Rename, async move {
            engine.rename_workflow(&id, &run, NAME).await
        })
    });
    store
        .history_read
        .wait_entered("pre-lock history read")
        .await?;

    // 2. The signal lands FIRST, through its own one-shot recorder, taking the
    //    sequence the parked rename has already claimed as its expected head.
    let signalled = engine.signal(&id, &run, "stop", input_payload()?).await;

    // 3. Release the rename into a head that has moved under it.
    store.history_read.release();
    let renamed = rename.await?;

    let history = inner.read_history(&id).await?;

    // NOT VACUOUS: the signal must actually have reached its one-shot recorder
    // and appended. Were it refused earlier (admission, a terminal run, an
    // unknown pair) the collision this test exists to force would never have
    // happened, and every assertion below would pass over a race that never
    // occurred.
    signalled
        .as_ref()
        .map_err(|error| format!("the paused-run signal never reached its recorder: {error}"))?;
    assert_eq!(
        history.len(),
        head_before + 1,
        "the signal must have appended exactly the one event that moves the head under the \
         parked rename: {history:?}"
    );

    // THE INVARIANT: the fence admitted exactly one append at that head.
    let appended = history.len() - head_before;
    let landed = usize::from(signalled.is_ok()) + usize::from(renamed.is_ok());
    assert_eq!(
        appended, landed,
        "every operation that reported success must have appended exactly one event, and every \
         operation that failed must have appended none — signal={signalled:?} rename={renamed:?} \
         history={history:?}"
    );
    assert_eq!(
        landed, 1,
        "exactly one of the two one-shot recorders may win the head; the other must be fenced \
         — signal={signalled:?} rename={renamed:?}"
    );

    // Nothing was written twice and nothing was skipped: the sequence is
    // contiguous, which is what a desynced second writer would break.
    let seqs: Vec<u64> = history.iter().map(Event::seq).collect();
    let contiguous: Vec<u64> = (1..=u64::try_from(history.len())?).collect();
    assert_eq!(
        seqs, contiguous,
        "the fenced append must leave a contiguous history, never a gap or a duplicated sequence"
    );

    // The loser recorded NOTHING — not a partial event, not a name.
    if renamed.is_err() {
        assert_eq!(
            aion_core::display_name(&history),
            None,
            "a fenced rename must record no name at all"
        );
    }

    // Status is untouched either way: neither verb is a lifecycle transition.
    assert_eq!(
        status_from_events(&history),
        WorkflowStatus::Paused,
        "neither a rename nor a paused-run signal moves the run out of Paused"
    );

    engine.shutdown()?;
    Ok(())
}

/// The chain a refused rename must leave behind: predecessor, its terminal
/// continue-as-new, successor — and no name recorded onto either run.
fn assert_intact_unnamed_chain(history: &[Event]) -> Result<(), Box<dyn std::error::Error>> {
    match history {
        [
            Event::WorkflowStarted { .. },
            Event::WorkflowContinuedAsNew { .. },
            Event::WorkflowStarted { .. },
        ] => {}
        other => {
            return Err(
                format!("expected an intact continue-as-new chain, found {other:?}").into(),
            );
        }
    }
    assert_eq!(
        aion_core::display_name(history),
        None,
        "the refused rename recorded nothing, onto either run"
    );
    assert_eq!(
        status_from_events(history),
        WorkflowStatus::Running,
        "the workflow carries on as the successor run — nothing was stranded"
    );
    Ok(())
}

/// SINGLE WRITER, the continue-as-new WINDOW (the defect the #211 review
/// caught).
///
/// A continue-as-new is TWO appends by TWO recorders: the predecessor records
/// `WorkflowContinuedAsNew` through its own recorder and releases that lock,
/// and only then does the successor's freshly minted recorder read the head and
/// append `WorkflowStarted` — outside the predecessor's lock, and BEFORE the
/// predecessor is deregistered. Inside that window the predecessor is still
/// registered, is not yet superseded, and the successor's expected sequence is
/// already fixed. A rename admitted there appends at that very sequence,
/// hard-fails the successor's `WorkflowStarted`, and strands the workflow
/// terminal-`ContinuedAsNew` with NO successor run — while reporting success.
///
/// This test drives a REAL `continue_as_new` and holds the window open at the
/// store, with the rename's own validation read taken BEFORE the
/// `WorkflowContinuedAsNew` append. That is what makes the refusal attributable
/// to the IN-LOCK re-read: the rename's pre-lock snapshot cannot see the
/// continue-as-new at all.
///
/// Killing mutation: delete the in-lock re-read and its refusal in
/// `lifecycle/rename.rs`. The rename then records at the successor's expected
/// sequence and `continue_as_new` fails with a sequence conflict.
#[tokio::test]
async fn renaming_a_run_that_continues_as_new_mid_rename_is_refused_and_strands_nothing()
-> Result<(), Box<dyn std::error::Error>> {
    let inner = Arc::new(InMemoryStore::default());
    let store = Arc::new(WindowStore::new(Arc::clone(&inner)));
    let engine = engine_over_window(&store, &inner).await?;
    let handle = engine
        .start_workflow(
            FIXTURE_MODULE,
            input_payload()?,
            HashMap::new(),
            String::from("default"),
        )
        .await?;
    let (id, run) = (handle.workflow_id().clone(), handle.run_id().clone());

    // 1. Park the rename on its PRE-LOCK validation read, so the snapshot it
    //    carries forward predates the continue-as-new entirely. The hold is
    //    armed for this workflow AND pinned to the rename's own task, so what
    //    it catches is the rename's read rather than whichever caller reads
    //    this history first.
    store.history_read.arm(&id)?;
    let rename = tokio::spawn({
        let (engine, id, run) = (Arc::clone(&engine), id.clone(), run.clone());
        DRIVER.scope(Driver::Rename, async move {
            engine.rename_workflow(&id, &run, NAME).await
        })
    });
    store
        .history_read
        .wait_entered("pre-lock history read")
        .await?;

    // 2. Park the SUCCESSOR's `WorkflowStarted` append: the continue-as-new
    //    records its terminal event, releases the predecessor's recorder lock,
    //    and stops at the door with its expected sequence already read.
    store.run_start_append.arm(&id)?;
    let continued = tokio::spawn({
        let (engine, id, run, carried) = (
            Arc::clone(&engine),
            id.clone(),
            run.clone(),
            input_payload()?,
        );
        async move { engine.continue_as_new(&id, &run, carried, None).await }
    });
    store
        .run_start_append
        .wait_entered("successor WorkflowStarted append")
        .await?;

    // THE WINDOW, asserted rather than assumed: the predecessor's terminal
    // event is durable, no successor run exists yet, the predecessor is STILL
    // REGISTERED, and the rename has not moved.
    let windowed = inner.read_history(&id).await?;
    assert!(
        matches!(windowed.last(), Some(Event::WorkflowContinuedAsNew { .. })),
        "the window must have the predecessor's terminal event at the head: {windowed:?}"
    );
    assert!(
        engine.registry().get(&id, &run)?.is_some(),
        "the predecessor is deregistered only after its successor starts, so it must still own \
         its recorder here — otherwise this drives the one-shot path instead"
    );
    assert!(
        !rename.is_finished(),
        "the rename must still be parked on its pre-continue-as-new snapshot, or the refusal \
         below would be attributable to the pre-lock check instead of the in-lock one"
    );

    // 3. Release the rename into the window. Its snapshot is clean, so only the
    //    re-read under the recorder lock can refuse it. Then release the
    //    successor. BOTH outcomes are collected before either is asserted, so a
    //    regression is reported as the whole failure it is rather than as
    //    whichever half the first assertion happens to reach.
    store.history_read.release();
    let renamed = rename.await?;
    store.run_start_append.release();
    let continued = continued.await?;

    // THE CONSEQUENCE FIRST: the continue-as-new still completes. A rename
    // admitted into the window takes the sequence the successor's recorder had
    // already claimed, and this is where that shows up — as a workflow left
    // terminal-`ContinuedAsNew` with no successor run and nothing to recover.
    let successor = match continued {
        Ok(successor) => successor,
        Err(error) => {
            return Err(format!(
                "the continue-as-new was STRANDED — its successor could not start after the \
                 rename appended into the window: {error}"
            )
            .into());
        }
    };
    match renamed {
        Err(EngineError::InvalidState { reason }) => {
            assert!(
                reason.contains("in flight"),
                "the refusal must be the IN-LOCK one, naming the race: {reason}"
            );
        }
        other => {
            return Err(format!("expected the in-lock InvalidState refusal, got {other:?}").into());
        }
    }
    assert_eq!(successor.workflow_id(), &id);
    assert_ne!(successor.run_id(), &run);

    assert_intact_unnamed_chain(&inner.read_history(&id).await?)?;
    assert_eq!(
        (store.history_read.parked(), store.run_start_append.parked()),
        (1, 1),
        "each hold must have parked exactly the one operation it was armed for"
    );

    engine.shutdown()?;
    Ok(())
}

/// The same refusal on the ONE-SHOT path, where there is no recorder lock to
/// re-read under and the pre-lock check is the only guard there is.
///
/// A crash between a continue-as-new's two appends leaves exactly this durable
/// state: `WorkflowContinuedAsNew` recorded, no successor `WorkflowStarted`, and
/// nothing registered. The predecessor is then TERMINAL and IS the last segment,
/// so neither the status gate nor the positional supersession gate refuses it —
/// both controls are asserted below, so the refusal is attributable to the
/// continued-as-new check and to nothing else the fixture shares.
///
/// Killing mutation: delete the pre-lock `has_continued_as_new` refusal in
/// `lifecycle/rename.rs`. The rename is then served and the name is recorded.
#[tokio::test]
async fn renaming_a_run_that_has_continued_as_new_is_refused_on_the_one_shot_path()
-> Result<(), Box<dyn std::error::Error>> {
    let store = Arc::new(InMemoryStore::default());
    let (engine, hash) = engine_over(&store, "complete").await?;
    let id = WorkflowId::new_v4();
    let (stranded, control) = (RunId::new_v4(), RunId::new_v4());
    seed(
        &store,
        &id,
        vec![
            started(&id, &stranded, &hash)?,
            Event::WorkflowContinuedAsNew {
                envelope: envelope(&id, 2),
                input: input_payload()?,
                workflow_type: None,
                parent_run_id: stranded.clone(),
            },
        ],
    )
    .await?;
    let before = store.read_history(&id).await?.len();

    // CONTROLS: the two gates that already existed both ADMIT this run.
    let history = store.read_history(&id).await?;
    let segment = run_segment(&history, &stranded);
    assert!(
        status_from_events(segment).is_terminal(),
        "the status gate must admit it, or the refusal is not attributable"
    );
    assert_eq!(
        segment.last().map(Event::seq),
        history.last().map(Event::seq),
        "the supersession gate must admit it too: no successor run exists"
    );
    assert!(
        engine.registry().get(&id, &stranded)?.is_none(),
        "nothing is registered, so this is the one-shot path"
    );

    match engine.rename_workflow(&id, &stranded, NAME).await {
        Err(EngineError::InvalidState { reason }) => {
            assert!(
                reason.contains("has continued as new"),
                "the refusal must name the reason: {reason}"
            );
        }
        other => return Err(format!("expected InvalidState, got {other:?}").into()),
    }
    let history = store.read_history(&id).await?;
    assert_eq!(history.len(), before, "a refused rename appends nothing");
    assert_eq!(aion_core::display_name(&history), None);

    // CONTROL: a run of the SAME workflow that has NOT continued as new, in the
    // same registry and terminal posture, renames normally.
    seed_appended(
        &store,
        &id,
        vec![
            Event::WorkflowStarted {
                envelope: envelope(&id, 3),
                workflow_type: FIXTURE_MODULE.to_owned(),
                input: input_payload()?,
                run_id: control.clone(),
                parent_run_id: Some(stranded.clone()),
                package_version: aion_core::PackageVersion::new(hash.clone()),
            },
            Event::WorkflowCompleted {
                envelope: envelope(&id, 4),
                result: input_payload()?,
            },
        ],
        2,
    )
    .await?;
    assert_eq!(engine.rename_workflow(&id, &control, NAME).await?, NAME);
    assert_eq!(
        aion_core::display_name(&store.read_history(&id).await?).as_deref(),
        Some(NAME)
    );

    engine.shutdown()?;
    Ok(())
}

/// A blank rename is refused and appends NOTHING — a rename records a non-empty
/// label or it records nothing at all.
#[tokio::test]
async fn a_blank_rename_is_refused_and_appends_nothing() -> Result<(), Box<dyn std::error::Error>> {
    let store = Arc::new(InMemoryStore::default());
    let (engine, hash) = engine_over(&store, "complete").await?;
    let (id, run) = (WorkflowId::new_v4(), RunId::new_v4());
    seed(&store, &id, vec![started(&id, &run, &hash)?]).await?;
    let before = store.read_history(&id).await?.len();

    for blank in ["", "   ", "\t\n "] {
        match engine.rename_workflow(&id, &run, blank).await {
            Err(EngineError::InvalidState { reason }) => {
                assert!(
                    reason.contains("non-empty display name"),
                    "the rejection must say why: {reason}"
                );
            }
            other => {
                return Err(format!("expected InvalidState for {blank:?}, got {other:?}").into());
            }
        }
    }
    assert_eq!(store.read_history(&id).await?.len(), before);

    engine.shutdown()?;
    Ok(())
}

/// Surrounding whitespace is trimmed, and the TRIMMED value is what is
/// recorded — the op returns exactly the string history now carries.
#[tokio::test]
async fn a_rename_records_the_trimmed_name() -> Result<(), Box<dyn std::error::Error>> {
    let store = Arc::new(InMemoryStore::default());
    let (engine, hash) = engine_over(&store, "complete").await?;
    let (id, run) = (WorkflowId::new_v4(), RunId::new_v4());
    seed_completed(&store, &id, &run, &hash).await?;

    let recorded = engine
        .rename_workflow(&id, &run, "  Nightly settlement  ")
        .await?;

    assert_eq!(recorded, NAME);
    let history = store.read_history(&id).await?;
    assert_eq!(recorded_names(&history), vec![NAME.to_owned()]);

    engine.shutdown()?;
    Ok(())
}

/// Renaming a run with no recorded history is `WorkflowNotFound`, and nothing
/// is appended — the target is validated before any write.
#[tokio::test]
async fn renaming_an_unknown_run_is_not_found() -> Result<(), Box<dyn std::error::Error>> {
    let store = Arc::new(InMemoryStore::default());
    let (engine, hash) = engine_over(&store, "complete").await?;
    let (id, run) = (WorkflowId::new_v4(), RunId::new_v4());

    match engine.rename_workflow(&id, &run, NAME).await {
        Err(EngineError::WorkflowNotFound { .. }) => {}
        other => return Err(format!("expected WorkflowNotFound, got {other:?}").into()),
    }
    assert!(store.read_history(&id).await?.is_empty());

    // A KNOWN workflow addressed by an unknown RUN is equally not found: the
    // run segment, not just the workflow, must exist.
    seed(&store, &id, vec![started(&id, &run, &hash)?]).await?;
    match engine.rename_workflow(&id, &RunId::new_v4(), NAME).await {
        Err(EngineError::WorkflowNotFound { .. }) => {}
        other => {
            return Err(
                format!("expected WorkflowNotFound for a foreign run, got {other:?}").into(),
            );
        }
    }
    assert_eq!(store.read_history(&id).await?.len(), 1);

    engine.shutdown()?;
    Ok(())
}

/// An engine that has NOT registered `aion.display_name` cannot record one: the
/// rename fails as a durability error rather than silently dropping the label.
/// This is the coupling that makes the server's schema registration load-bearing.
#[tokio::test]
async fn renaming_without_the_registered_attribute_is_a_durability_failure()
-> Result<(), Box<dyn std::error::Error>> {
    // The SHARED default-engine helper, whose schema registers nothing — so
    // this arm tests the genuine unregistered-attribute posture rather than an
    // emptiness this file constructed for itself.
    let (engine, store) = common::engine_with_fixture("complete").await?;
    let hash = fixture_package("complete")?.content_hash().to_string();
    let (id, run) = (WorkflowId::new_v4(), RunId::new_v4());
    // Terminal, so the append path is open and the SCHEMA is what refuses —
    // not the residency gate.
    store
        .append(
            WriteToken::recorder(),
            &id,
            &[
                started(&id, &run, &hash)?,
                Event::WorkflowCompleted {
                    envelope: envelope(&id, 2),
                    result: input_payload()?,
                },
            ],
            0,
        )
        .await?;

    match engine.rename_workflow(&id, &run, NAME).await {
        Err(EngineError::Durability(_)) => {}
        other => return Err(format!("expected Durability, got {other:?}").into()),
    }
    assert_eq!(
        store.read_history(&id).await?.len(),
        2,
        "a refused rename appends nothing"
    );

    engine.shutdown()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// The window fixture.
// ---------------------------------------------------------------------------

/// Which operation the task issuing a store call is driving.
///
/// A hold armed only with a workflow id parks the FIRST matching store call for
/// that workflow, WHOEVER issues it — so what it caught would be a matter of
/// timing: any other component that read the same history first would take the
/// hold, and the operation under test would sail past unparked while the test
/// still reported the assertion it wanted. The driver is set on the task that
/// issues the operation under test, and a hold checks it, so the hold parks THAT
/// caller and nothing else.
///
/// It pins the caller because a task-local is carried by the task, not by the
/// clock: `Engine::rename_workflow` reads history inline on the calling task
/// (`engine/api_workflow_ops.rs`), so the read this catches is provably the
/// rename's own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Driver {
    /// An `Engine::rename_workflow` call.
    Rename,
}

tokio::task_local! {
    /// The operation the current task is driving, when it is one a hold cares
    /// about. Unset on every other task, which is what makes the pin exclusive.
    static DRIVER: Driver;
}

impl Driver {
    /// Whether the CURRENT task is driving this operation.
    fn is_current(self) -> bool {
        DRIVER.try_with(|driver| *driver == self).unwrap_or(false)
    }
}

/// A one-shot hold on a single store operation for one workflow.
///
/// Armed with the workflow it should catch, it parks the FIRST matching
/// operation at the door, announces that it has done so, and holds it there
/// until released. It is one-shot because the thing it exists to catch happens
/// once: a second operation of the same kind must pass through, or the fixture
/// would deadlock the engine it is measuring.
///
/// The workflow id is only half the match. Which CALLER may be parked is the
/// other half, and it is applied by the store operation itself (see
/// [`Driver`]) — either through the task-local driver, or, where the operation
/// is issued by a task the engine spawns for itself, by the shape of what is
/// being written.
struct Hold {
    target: std::sync::Mutex<Option<WorkflowId>>,
    entered: Notify,
    release: Notify,
    parked: AtomicUsize,
}

impl Hold {
    fn new() -> Self {
        Self {
            target: std::sync::Mutex::new(None),
            entered: Notify::new(),
            release: Notify::new(),
            parked: AtomicUsize::new(0),
        }
    }

    /// Arm the hold for `workflow_id`.
    fn arm(&self, workflow_id: &WorkflowId) -> Result<(), StoreError> {
        *self.locked()? = Some(workflow_id.clone());
        Ok(())
    }

    /// Claim the hold for `workflow_id`, disarming it in the same critical
    /// section so exactly one operation can ever be parked per arming.
    fn claim(&self, workflow_id: &WorkflowId) -> Result<bool, StoreError> {
        let mut target = self.locked()?;
        if target.as_ref() != Some(workflow_id) {
            return Ok(false);
        }
        *target = None;
        Ok(true)
    }

    /// Park the caller until [`Hold::release`], announcing its arrival first.
    async fn park(&self) {
        self.parked.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        self.release.notified().await;
    }

    /// Wait until a matching operation is parked at the door.
    ///
    /// BOUNDED, and reported as a named failure when the bound elapses. A hold
    /// matches on the workflow id AND on which caller may be parked (see
    /// [`Driver`]), so a refactor that moves the held operation off the caller's
    /// task — a `spawn_blocking`, a `tokio::spawn` — silently stops the pin from
    /// matching. Unbounded, that turns into a test binary that HANGS: under a
    /// suite runner it reads as a timeout with no attribution, not as an
    /// assertion naming the pin that broke. `what` names the hold so the
    /// elapsed case says which one never engaged.
    async fn wait_entered(&self, what: &str) -> Result<(), Box<dyn std::error::Error>> {
        tokio::time::timeout(FIXTURE_WAIT, self.entered.notified())
            .await
            .map_err(|_| {
                format!(
                    "the {what} hold never engaged within {FIXTURE_WAIT:?}: nothing matching it \
                     reached the store. The hold matches on workflow id AND on the driver of the \
                     calling task, so this is what a pin that stopped matching looks like — check \
                     whether the held operation still runs on the caller's own task."
                )
            })?;
        Ok(())
    }

    /// Let the parked operation through.
    fn release(&self) {
        self.release.notify_one();
    }

    /// How many operations this hold has parked — the fixture's own control
    /// against having caught something other than what it was armed for.
    fn parked(&self) -> usize {
        self.parked.load(Ordering::SeqCst)
    }

    fn locked(&self) -> Result<std::sync::MutexGuard<'_, Option<WorkflowId>>, StoreError> {
        self.target
            .lock()
            .map_err(|error| StoreError::Backend(format!("hold target lock poisoned: {error}")))
    }
}

/// An [`EventStore`] that delegates everything to an inner [`InMemoryStore`],
/// except that it can park ONE history read and ONE run-starting append at the
/// door on request.
///
/// Those two are the ends of the continue-as-new window: a rename's validation
/// read on one side, the successor's `WorkflowStarted` on the other. An honest
/// store cannot be asked to pause between them, and nothing above the store can
/// occupy that window without racing for it — which is why the fixture lives
/// here rather than in a sleep.
///
/// The inner store is shared (`Arc`), so a test can read history and visibility
/// through it directly, unheld, while an operation is parked.
struct WindowStore {
    inner: Arc<InMemoryStore>,
    /// Parks the RENAME's own `read_history` AFTER it has read, so the caller
    /// carries a snapshot taken before whatever the test does next. Pinned to
    /// the rename by [`Driver`]: a history read issued by anything else passes
    /// straight through.
    history_read: Hold,
    /// Parks an append carrying a `WorkflowStarted` BEFORE it is written, so
    /// the run's expected sequence is already fixed while it waits. Pinned by
    /// the event shape rather than by a driver, because the successor's start
    /// is appended by a task the engine spawns for itself, which no task-local
    /// of the caller's reaches.
    run_start_append: Hold,
}

impl WindowStore {
    fn new(inner: Arc<InMemoryStore>) -> Self {
        Self {
            inner,
            history_read: Hold::new(),
            run_start_append: Hold::new(),
        }
    }

    async fn hold_run_start(
        &self,
        workflow_id: &WorkflowId,
        events: &[Event],
    ) -> Result<(), StoreError> {
        if !events
            .iter()
            .any(|event| matches!(event, Event::WorkflowStarted { .. }))
        {
            return Ok(());
        }
        if self.run_start_append.claim(workflow_id)? {
            self.run_start_append.park().await;
        }
        Ok(())
    }
}

#[async_trait]
impl ReadableEventStore for WindowStore {
    async fn read_history(&self, workflow_id: &WorkflowId) -> Result<Vec<Event>, StoreError> {
        let events = self.inner.read_history(workflow_id).await?;
        // ONLY the rename's own validation read may be parked here. Without the
        // driver check this would park whichever caller happened to read this
        // workflow's history first — a recovery sweep, a projection refresh, an
        // engine-internal status read — and the window the test then asserts
        // over would be occupied by something it never named.
        if Driver::Rename.is_current() && self.history_read.claim(workflow_id)? {
            self.history_read.park().await;
        }
        Ok(events)
    }

    async fn read_history_from(
        &self,
        workflow_id: &WorkflowId,
        from_seq: u64,
    ) -> Result<Vec<Event>, StoreError> {
        self.inner.read_history_from(workflow_id, from_seq).await
    }

    async fn read_run_chain(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<RunSummary>, StoreError> {
        self.inner.read_run_chain(workflow_id).await
    }

    async fn list_workflow_ids(&self) -> Result<Vec<WorkflowId>, StoreError> {
        self.inner.list_workflow_ids().await
    }

    async fn list_active(&self) -> Result<Vec<WorkflowId>, StoreError> {
        self.inner.list_active().await
    }

    async fn list_paused(&self) -> Result<Vec<WorkflowId>, StoreError> {
        self.inner.list_paused().await
    }

    async fn query(&self, filter: &WorkflowFilter) -> Result<Vec<WorkflowSummary>, StoreError> {
        self.inner.query(filter).await
    }

    async fn schedule_timer(
        &self,
        workflow_id: &WorkflowId,
        timer_id: &TimerId,
        fire_at: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.inner
            .schedule_timer(workflow_id, timer_id, fire_at)
            .await
    }

    async fn expired_timers(&self, as_of: DateTime<Utc>) -> Result<Vec<TimerEntry>, StoreError> {
        self.inner.expired_timers(as_of).await
    }

    // The shard-ownership seams are FORWARDED rather than defaulted: a wrapper
    // that silently answered the trait defaults would put the double in a state
    // no real backend occupies — disclaiming shards whose history it is serving.
    fn set_owned_shards(&self, shards: Option<&[usize]>) {
        self.inner.set_owned_shards(shards);
    }

    fn acquire_owned_shards(&self, shards: &[usize]) -> Result<(), StoreError> {
        self.inner.acquire_owned_shards(shards)
    }

    fn acquire_owned_shard(&self, shard: usize) -> Result<(), StoreError> {
        self.inner.acquire_owned_shard(shard)
    }

    fn extend_owned_shards(&self, shards: &[usize]) {
        self.inner.extend_owned_shards(shards);
    }

    fn is_current_owner(&self, shard: usize) -> bool {
        self.inner.is_current_owner(shard)
    }

    fn publish_shard_owner(&self, shard: usize) -> Result<(), StoreError> {
        self.inner.publish_shard_owner(shard)
    }
}

#[async_trait]
impl WritableEventStore for WindowStore {
    async fn append(
        &self,
        token: WriteToken,
        workflow_id: &WorkflowId,
        events: &[Event],
        expected_seq: u64,
    ) -> Result<(), StoreError> {
        self.hold_run_start(workflow_id, events).await?;
        self.inner
            .append(token, workflow_id, events, expected_seq)
            .await
    }

    async fn append_with_outbox(
        &self,
        token: WriteToken,
        workflow_id: &WorkflowId,
        events: &[Event],
        expected_seq: u64,
        outbox_rows: &[OutboxRow],
    ) -> Result<(), StoreError> {
        self.hold_run_start(workflow_id, events).await?;
        self.inner
            .append_with_outbox(token, workflow_id, events, expected_seq, outbox_rows)
            .await
    }

    async fn rearm_outbox_pending(&self, rows: &[OutboxRow]) -> Result<(), StoreError> {
        self.inner.rearm_outbox_pending(rows).await
    }

    async fn settle_outbox_row_cancelled(&self, dispatch_key: &str) -> Result<(), StoreError> {
        self.inner.settle_outbox_row_cancelled(dispatch_key).await
    }

    async fn settle_workflow_outbox_rows_cancelled(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<String>, StoreError> {
        self.inner
            .settle_workflow_outbox_rows_cancelled(workflow_id)
            .await
    }
}

#[async_trait]
impl PackageStore for WindowStore {
    async fn put_package(&self, record: PackageRecord) -> Result<(), StoreError> {
        self.inner.put_package(record).await
    }

    async fn put_package_with_routes(
        &self,
        record: PackageRecord,
        route_workflow_types: &[String],
    ) -> Result<(), StoreError> {
        self.inner
            .put_package_with_routes(record, route_workflow_types)
            .await
    }

    async fn list_packages(&self) -> Result<Vec<PackageRecord>, StoreError> {
        self.inner.list_packages().await
    }

    async fn delete_package(
        &self,
        workflow_type: &str,
        content_hash: &str,
    ) -> Result<(), StoreError> {
        self.inner.delete_package(workflow_type, content_hash).await
    }

    async fn put_package_route(
        &self,
        workflow_type: &str,
        content_hash: &str,
    ) -> Result<(), StoreError> {
        self.inner
            .put_package_route(workflow_type, content_hash)
            .await
    }

    async fn list_package_routes(&self) -> Result<Vec<PackageRouteRecord>, StoreError> {
        self.inner.list_package_routes().await
    }
}

// `EventStore` is a blanket impl over the three traits above — nothing to write.
