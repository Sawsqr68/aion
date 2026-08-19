//! End-to-end proof that workflow-visible `now` is the REPLAY POSITION, not
//! the history tail (aion#1).
//!
//! The shape is the issue's flagship symptom. A run slept, the engine died
//! while it slept, and the durable timer fired: the run segment ends
//! `WorkflowStarted@T0`, `TimerStarted@T1`, `TimerFired@T_wake`. Startup
//! recovery re-spawns the run and replays it. The fixture reads `now` once
//! BEFORE the sleep and once AFTER it and returns the span.
//!
//! Correct: the first read sits before any command is consumed, so it answers
//! the run's start (`T0`); the second sits after the recorded `TimerFired` has
//! been consumed, so it answers `T_wake`. The span is `T_wake - T0`.
//!
//! The defect: `now` answered from `history.last()` — the wake event — at
//! EVERY position of the replayed run, so both reads returned `T_wake` and the
//! span collapsed to zero. A workflow that measures its own elapsed time is
//! silently wrong after every recovery.
//!
//! This drives the PRODUCTION path end to end: real `aion_flow_ffi:now/0` and
//! `aion_flow_ffi:sleep/1` natives, real recovery replay, real store.

use std::sync::Arc;
use std::time::{Duration, Instant};

use aion::EngineBuilder;
use aion_core::{
    Event, EventEnvelope, Payload, RunId, TimerId, WorkflowId, WorkflowStatus, status_from_events,
};
use aion_package::{
    BeamModule, BeamSet, CURRENT_FORMAT_VERSION, ExtractionLimits, Manifest, ManifestVersion,
    Package, PackageBuilder,
};
use aion_store::{EventStore, InMemoryStore, WriteToken};
use chrono::{DateTime, TimeZone, Utc};
use serde_json::json;

const FIXTURE_MODULE: &str = "aion_fixture_workflow";
const FIXTURE_BEAM: &[u8] = include_bytes!("fixtures/aion_fixture_workflow.beam");
const FIXTURE_SOURCE: &[u8] = include_bytes!("fixtures/aion_fixture_workflow.erl");

/// The committed BEAM fixture packaged around its `now_span/1` entry.
///
/// Built locally rather than through `tests/common` (the precedent
/// `sdk_sleep_query_e2e` sets): this suite needs the package's content hash
/// BEFORE the engine exists, so it can stamp the same `package_version` onto
/// the seeded `WorkflowStarted` that startup recovery will replay.
fn fixture_package() -> Result<Package, Box<dyn std::error::Error>> {
    let beams = BeamSet::new(vec![BeamModule::new(FIXTURE_MODULE, FIXTURE_BEAM)])?;
    let manifest = Manifest {
        entry_module: FIXTURE_MODULE.to_owned(),
        entry_function: "now_span".to_owned(),
        input_schema: json!({ "type": "object" }),
        output_schema: json!({}),
        timeout: None,
        activities: Vec::new(),
        version: ManifestVersion::new("stamped-by-builder"),
        format_version: CURRENT_FORMAT_VERSION,
        additional_workflows: Vec::new(),
    };
    let archive =
        PackageBuilder::with_source(manifest, beams, [(FIXTURE_MODULE, FIXTURE_SOURCE.to_vec())])
            .write_to_bytes()?;
    Ok(Package::load_from_bytes(
        archive,
        ExtractionLimits::unbounded(),
    )?)
}

/// Recorded start of the run.
const STARTED_AT_MILLIS: i64 = 1_700_000_000_000;
/// Recorded arming of the durable sleep timer.
const TIMER_STARTED_AT_MILLIS: i64 = 1_700_000_000_500;
/// Recorded wake: the durable timer fired an hour after the run started.
const TIMER_FIRED_AT_MILLIS: i64 = 1_700_003_600_000;

fn timestamp(millis: i64) -> Result<DateTime<Utc>, Box<dyn std::error::Error>> {
    Utc.timestamp_millis_opt(millis)
        .single()
        .ok_or_else(|| format!("invalid fixed timestamp {millis}").into())
}

fn envelope(
    workflow_id: &WorkflowId,
    seq: u64,
    millis: i64,
) -> Result<EventEnvelope, Box<dyn std::error::Error>> {
    Ok(EventEnvelope {
        seq,
        recorded_at: timestamp(millis)?,
        workflow_id: workflow_id.clone(),
    })
}

/// The run segment a crash mid-sleep leaves behind: started, slept, woke.
///
/// The timer identity is `TimerId::anonymous(0)` — the first timer ordinal a
/// run allocates — so the fixture's single `sleep` correlates with it on
/// replay exactly as it did live.
fn slept_and_woke_history(
    workflow_id: &WorkflowId,
    run_id: &RunId,
    package_version: &aion_core::PackageVersion,
    input: Payload,
) -> Result<Vec<Event>, Box<dyn std::error::Error>> {
    let timer_id = TimerId::anonymous(0);
    Ok(vec![
        Event::WorkflowStarted {
            envelope: envelope(workflow_id, 1, STARTED_AT_MILLIS)?,
            workflow_type: FIXTURE_MODULE.to_owned(),
            input,
            run_id: run_id.clone(),
            parent_run_id: None,
            parent_workflow_id: None,
            package_version: package_version.clone(),
        },
        Event::TimerStarted {
            envelope: envelope(workflow_id, 2, TIMER_STARTED_AT_MILLIS)?,
            timer_id: timer_id.clone(),
            fire_at: timestamp(TIMER_FIRED_AT_MILLIS)?,
        },
        Event::TimerFired {
            envelope: envelope(workflow_id, 3, TIMER_FIRED_AT_MILLIS)?,
            timer_id,
        },
    ])
}

async fn await_terminal(
    store: &Arc<dyn EventStore>,
    workflow_id: &WorkflowId,
) -> Result<Vec<Event>, Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let history = store.read_history(workflow_id).await?;
        if status_from_events(&history) != WorkflowStatus::Running {
            return Ok(history);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "recovered run never reached a terminal status; history: {history:#?}"
            )
            .into());
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

#[tokio::test]
async fn replayed_sleep_reports_the_recorded_span_not_zero_elapsed()
-> Result<(), Box<dyn std::error::Error>> {
    let package = fixture_package()?;
    let package_version = aion_core::PackageVersion::new(package.content_hash().to_string());
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let workflow_id = WorkflowId::new_v4();
    let run_id = RunId::new_v4();
    let history = slept_and_woke_history(
        &workflow_id,
        &run_id,
        &package_version,
        Payload::from_json(&json!({ "fixture": "input" }))?,
    )?;
    store
        .append(WriteToken::recorder(), &workflow_id, &history, 0)
        .await?;

    // Building the engine runs startup recovery: the run is re-spawned and
    // replayed through the production NIF path.
    let engine = EngineBuilder::new()
        .in_process_activity_serving()
        .store_arc(Arc::clone(&store))
        .in_memory_visibility()
        .scheduler_threads(1)
        .load_workflows(package)
        .build()
        .await?;

    let history = await_terminal(&store, &workflow_id).await?;
    engine.shutdown()?;

    let result = history
        .iter()
        .find_map(|event| match event {
            Event::WorkflowCompleted { result, .. } => Some(result.clone()),
            _ => None,
        })
        .ok_or_else(|| format!("replayed run did not complete; history: {history:#?}"))?;
    let elapsed = serde_json::from_slice::<serde_json::Value>(result.bytes())?
        .as_i64()
        .ok_or("now_span result is not an integer")?;

    assert_eq!(
        elapsed,
        TIMER_FIRED_AT_MILLIS - STARTED_AT_MILLIS,
        "workflow-visible elapsed time across a replayed sleep must be the recorded \
         start-to-wake span; a zero span is the history-tail read (aion#1)"
    );
    Ok(())
}
