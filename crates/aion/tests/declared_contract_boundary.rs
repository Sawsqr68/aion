//! The declared contract is enforced AT THE BOUNDARY — before anything is
//! recorded and before an arrival can be consumed.
//!
//! THE RULE THESE TESTS EXIST FOR: a refusal can never consume the arrival.
//! The defect these replace was a signal payload recorded opaquely, marked
//! consumed, and only then decoded inside the workflow process — so a
//! malformed body from an operator killed a run that may have been parked for
//! months, with nothing left to retry. Every refusal below is asserted against
//! the run's history BYTES, not merely its length: a refused signal leaves the
//! history byte-identical and the run parked exactly where it was.

use std::collections::HashMap;
use std::sync::Arc;

use aion::{
    Engine, EngineBuilder, EngineError, HandleResidency, RuntimeHandle, SignalRouter,
    signal::ConcreteSignalRouter,
};
use aion_core::{Event, Payload, RunId, WorkflowId, WorkflowStatus};
use aion_package::{
    BeamModule, BeamSet, CURRENT_FORMAT_VERSION, ExtractionLimits, Manifest, ManifestVersion,
    Package, PackageBuilder, PackageContract, SignalContract,
};
use aion_store::{EventStore, InMemoryStore};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const FIXTURE_MODULE: &str = "aion_fixture_workflow";
const FIXTURE_BEAM: &[u8] = include_bytes!("fixtures/aion_fixture_workflow.beam");
/// The one control signal these fixtures declare — named after the real
/// `assistant.awl` signal this defect was found on.
const DECLARED_SIGNAL: &str = "assistant_continue";

fn payload(value: &Value) -> Result<Payload, aion_core::PayloadError> {
    Payload::from_json(value)
}

/// The declared `Continuation` payload type: both fields required, exactly as
/// `examples/assistant/awl/assistant.awl` declares them.
fn continuation_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "message": { "type": "string" },
            "end": { "type": "boolean" }
        },
        "required": ["end", "message"]
    })
}

fn declaring_package(contract: PackageContract) -> Result<Package, Box<dyn std::error::Error>> {
    let beams = BeamSet::new(vec![BeamModule::new(FIXTURE_MODULE, FIXTURE_BEAM)])?;
    let manifest = Manifest {
        entry_module: FIXTURE_MODULE.to_owned(),
        entry_function: "wait".to_owned(),
        input_schema: contract.input_schema.clone(),
        output_schema: json!({}),
        timeout: None,
        activities: Vec::new(),
        version: ManifestVersion::new("stamped-by-builder"),
        format_version: CURRENT_FORMAT_VERSION,
        additional_workflows: Vec::new(),
    };
    let archive = PackageBuilder::new(manifest, beams)
        .with_contract(contract)
        .write_to_bytes()?;
    Ok(Package::load_from_bytes(
        archive,
        ExtractionLimits::unbounded(),
    )?)
}

/// A contract that declares one signal and accepts any object as input.
fn signal_declaring_contract() -> PackageContract {
    PackageContract {
        input_schema: json!({ "type": "object" }),
        output_schema: json!({}),
        signals: vec![SignalContract {
            name: DECLARED_SIGNAL.to_owned(),
            input_schema: continuation_schema(),
        }],
        ..PackageContract::default()
    }
}

async fn engine_for(contract: PackageContract) -> Result<Engine, Box<dyn std::error::Error>> {
    engine_over(
        Arc::new(InMemoryStore::default()) as Arc<dyn EventStore>,
        contract,
    )
    .await
}

async fn engine_over(
    store: Arc<dyn EventStore>,
    contract: PackageContract,
) -> Result<Engine, Box<dyn std::error::Error>> {
    Ok(EngineBuilder::new()
        .in_process_activity_serving()
        .store_arc(store)
        .in_memory_visibility()
        .scheduler_threads(1)
        .signal_router_factory(|runtime: Arc<RuntimeHandle>, handoff| {
            Arc::new(ConcreteSignalRouter::new(runtime, handoff)) as Arc<dyn SignalRouter>
        })
        .load_workflows(declaring_package(contract)?)
        .build()
        .await?)
}

/// The run's complete recorded history, serialized. Comparing these bytes is
/// what "the history is unchanged" has to mean: an event count can stay equal
/// while a payload silently differs.
async fn history_bytes(
    store: &Arc<dyn EventStore>,
    workflow_id: &WorkflowId,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let history = store.read_history(workflow_id).await?;
    Ok(serde_json::to_vec(&history)?)
}

/// Whether the run is still resident and its process still live — "parked on
/// the same wait" as the engine can observe it.
fn still_parked(
    engine: &Engine,
    workflow_id: &WorkflowId,
    run_id: &RunId,
) -> Result<bool, EngineError> {
    let Some(handle) = engine.registry().get(workflow_id, run_id)? else {
        return Ok(false);
    };
    Ok(handle.residency() == HandleResidency::Resident && engine.runtime().is_live(handle.pid()))
}

async fn run_status(
    store: &Arc<dyn EventStore>,
    workflow_id: &WorkflowId,
    run_id: &RunId,
) -> Result<WorkflowStatus, Box<dyn std::error::Error>> {
    let history = store.read_history(workflow_id).await?;
    Ok(aion_core::status_from_events(aion_core::run_segment(
        &history, run_id,
    )))
}

/// THE ORDERING TEST. A malformed payload for a DECLARED signal is refused at
/// the boundary; the run's history is byte-identical afterwards, the run is
/// still parked, and a conforming payload then proceeds normally — proving the
/// refusal cost the run nothing at all.
#[tokio::test]
async fn a_refused_signal_payload_leaves_the_history_byte_identical_and_the_run_parked()
-> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = engine_over(Arc::clone(&store), signal_declaring_contract()).await?;
    let handle = engine
        .start_workflow(
            FIXTURE_MODULE,
            payload(&json!({ "objective": "prove the boundary" }))?,
            HashMap::new(),
            String::from("default"),
        )
        .await?;
    let before = history_bytes(&store, handle.workflow_id()).await?;

    // `end` is declared Bool; the operator sent the STRING "true".
    let error = engine
        .signal(
            handle.workflow_id(),
            handle.run_id(),
            DECLARED_SIGNAL,
            payload(&json!({ "message": "carry on", "end": "true" }))?,
        )
        .await
        .err()
        .ok_or("a malformed payload for a declared signal must be refused")?;

    let EngineError::SignalRefused {
        signal_name,
        reason,
        ..
    } = &error
    else {
        return Err(format!("expected a declared-contract refusal, got: {error}").into());
    };
    assert_eq!(signal_name, DECLARED_SIGNAL);
    assert!(
        reason.contains("/end"),
        "the refusal must name the field that did not match: {reason}"
    );

    // Nothing recorded, nothing consumed.
    assert_eq!(
        history_bytes(&store, handle.workflow_id()).await?,
        before,
        "a refused signal must leave the run's history byte-identical"
    );
    assert!(
        still_parked(&engine, handle.workflow_id(), handle.run_id())?,
        "a refused signal must leave the run parked on the same wait"
    );
    assert_eq!(
        run_status(&store, handle.workflow_id(), handle.run_id()).await?,
        WorkflowStatus::Running
    );

    // And the same run still accepts a conforming payload afterwards: the
    // refusal is the caller's problem, never the run's.
    let accepted = payload(&json!({ "message": "carry on", "end": false }))?;
    engine
        .signal(
            handle.workflow_id(),
            handle.run_id(),
            DECLARED_SIGNAL,
            accepted.clone(),
        )
        .await?;
    let history = store.read_history(handle.workflow_id()).await?;
    let recorded = history
        .iter()
        .find_map(|event| match event {
            Event::SignalReceived { name, payload, .. } => Some((name, payload)),
            _ => None,
        })
        .ok_or("the conforming signal was not recorded")?;
    assert_eq!(recorded.0, DECLARED_SIGNAL);
    assert_eq!(recorded.1, &accepted);

    engine.shutdown()?;
    Ok(())
}

/// A name the package does not declare is a refusal, not a silently recorded
/// no-op that no wait will ever resolve.
#[tokio::test]
async fn an_undeclared_signal_name_is_refused_without_recording() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = engine_over(Arc::clone(&store), signal_declaring_contract()).await?;
    let handle = engine
        .start_workflow(
            FIXTURE_MODULE,
            payload(&json!({ "objective": "prove the boundary" }))?,
            HashMap::new(),
            String::from("default"),
        )
        .await?;
    let before = history_bytes(&store, handle.workflow_id()).await?;

    let error = engine
        .signal(
            handle.workflow_id(),
            handle.run_id(),
            "assistant_stop",
            payload(&json!({ "message": "", "end": true }))?,
        )
        .await
        .err()
        .ok_or("an undeclared signal name must be refused")?;

    let EngineError::SignalRefused { reason, .. } = &error else {
        return Err(format!("expected a declared-contract refusal, got: {error}").into());
    };
    assert!(
        reason.contains(DECLARED_SIGNAL),
        "the refusal must say which names the package does declare: {reason}"
    );
    assert_eq!(history_bytes(&store, handle.workflow_id()).await?, before);
    assert!(still_parked(
        &engine,
        handle.workflow_id(),
        handle.run_id()
    )?);

    engine.shutdown()?;
    Ok(())
}

/// A payload whose bytes are not JSON at all is refused with the same
/// ordering. This is the exact shape of the original defect: undecodable in,
/// run dead, arrival consumed.
#[tokio::test]
async fn an_undecodable_payload_is_refused_without_recording() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = engine_over(Arc::clone(&store), signal_declaring_contract()).await?;
    let handle = engine
        .start_workflow(
            FIXTURE_MODULE,
            payload(&json!({ "objective": "prove the boundary" }))?,
            HashMap::new(),
            String::from("default"),
        )
        .await?;
    let before = history_bytes(&store, handle.workflow_id()).await?;

    let error = engine
        .signal(
            handle.workflow_id(),
            handle.run_id(),
            DECLARED_SIGNAL,
            Payload::new(aion_core::ContentType::Json, b"{not json".to_vec()),
        )
        .await
        .err()
        .ok_or("an undecodable payload must be refused")?;

    assert!(
        matches!(error, EngineError::SignalRefused { .. }),
        "expected a declared-contract refusal, got: {error}"
    );
    assert_eq!(history_bytes(&store, handle.workflow_id()).await?, before);
    assert!(still_parked(
        &engine,
        handle.workflow_id(),
        handle.run_id()
    )?);

    engine.shutdown()?;
    Ok(())
}

/// A package that declares NO signal surface promises nothing about signals,
/// so nothing is enforced. This is what keeps the manifest-derived (Gleam SDK)
/// contracts working: an empty declaration must never be read as "accepts
/// nothing".
#[tokio::test]
async fn a_package_declaring_no_signals_admits_any_signal() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = engine_over(
        Arc::clone(&store),
        PackageContract {
            input_schema: json!({ "type": "object" }),
            output_schema: json!({}),
            ..PackageContract::default()
        },
    )
    .await?;
    let handle = engine
        .start_workflow(
            FIXTURE_MODULE,
            payload(&json!({ "anything": true }))?,
            HashMap::new(),
            String::from("default"),
        )
        .await?;

    let sent = payload(&json!({ "whatever": [1, 2, 3] }))?;
    engine
        .signal(
            handle.workflow_id(),
            handle.run_id(),
            "undeclared_but_admitted",
            sent.clone(),
        )
        .await?;

    let history = store.read_history(handle.workflow_id()).await?;
    let recorded = history
        .iter()
        .find_map(|event| match event {
            Event::SignalReceived { name, payload, .. } => Some((name, payload)),
            _ => None,
        })
        .ok_or("an unenforced signal must still be recorded")?;
    assert_eq!(recorded.0, "undeclared_but_admitted");
    assert_eq!(recorded.1, &sent);

    engine.shutdown()?;
    Ok(())
}

/// A start whose input does not satisfy the declared input schema is refused
/// before any history exists at all — no `WorkflowStarted`, no process, no
/// half-started run to clean up.
#[tokio::test]
async fn a_start_input_violating_the_declared_schema_is_refused_before_any_history() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = engine_over(
        Arc::clone(&store),
        PackageContract {
            input_schema: json!({
                "type": "object",
                "properties": { "objective": { "type": "string" } },
                "required": ["objective"]
            }),
            output_schema: json!({}),
            ..PackageContract::default()
        },
    )
    .await?;
    let workflow_id = WorkflowId::new_v4();

    let error = engine
        .start_workflow_with_id(
            FIXTURE_MODULE,
            payload(&json!({ "objective": 42 }))?,
            HashMap::new(),
            String::from("default"),
            Some(workflow_id.clone()),
            None,
        )
        .await
        .err()
        .ok_or("a start input violating the declared schema must be refused")?;

    let EngineError::StartInputRefused { reason, .. } = &error else {
        return Err(format!("expected a declared-input refusal, got: {error}").into());
    };
    assert!(
        reason.contains("/objective"),
        "the refusal must name the field that did not match: {reason}"
    );
    assert!(
        store.read_history(&workflow_id).await?.is_empty(),
        "a refused start must record nothing at all"
    );

    // The same workflow type still starts with a conforming input.
    let handle = engine
        .start_workflow(
            FIXTURE_MODULE,
            payload(&json!({ "objective": "prove the boundary" }))?,
            HashMap::new(),
            String::from("default"),
        )
        .await?;
    assert!(!store.read_history(handle.workflow_id()).await?.is_empty());

    engine.shutdown()?;
    Ok(())
}

/// A contract that declares NO input shape — `input_schema: null`, which is
/// what [`PackageContract::default`] and every defaulted manifest carry —
/// admits anything. An absent declaration is not an uncompilable schema and
/// must never be read as one: doing so refuses every start of every legacy
/// package for a mistake the caller did not make.
#[tokio::test]
async fn a_package_declaring_no_input_shape_admits_any_input() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = engine_over(Arc::clone(&store), PackageContract::default()).await?;

    let handle = engine
        .start_workflow(
            FIXTURE_MODULE,
            payload(&json!({ "anything": [1, "two", null] }))?,
            HashMap::new(),
            String::from("default"),
        )
        .await?;

    assert!(!store.read_history(handle.workflow_id()).await?.is_empty());
    engine.shutdown()?;
    Ok(())
}

/// The declaration is measured against the EXACT identity the run is pinned
/// to, so an engine that no longer retains that identity has nothing to
/// enforce and must not strand the run.
#[tokio::test]
async fn a_signal_is_admitted_when_the_pinned_identity_is_no_longer_retained() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = engine_for(signal_declaring_contract()).await?;
    // A run whose start was recorded against a version this engine never
    // loaded: the store carries the history, the catalog carries no
    // declaration for it.
    let workflow_id = WorkflowId::new_v4();
    let run_id = RunId::new_v4();
    let mut recorder = aion::durability::Recorder::new(workflow_id.clone(), Arc::clone(&store));
    recorder
        .record_workflow_started(
            chrono::Utc::now(),
            aion::durability::WorkflowStartRecord {
                workflow_type: FIXTURE_MODULE.to_owned(),
                input: payload(&json!({ "objective": "unretained" }))?,
                run_id: run_id.clone(),
                parent_run_id: None,
                parent_workflow_id: None,
                package_version: aion_core::PackageVersion::new("b".repeat(64)),
            },
        )
        .await?;
    engine
        .registry()
        .replace_residency(&workflow_id, &run_id, HandleResidency::Suspended)
        .ok();

    // The run is durably started, non-terminal, and not resident: the signal
    // path resolves its identity from history, finds no retained declaration,
    // and must admit rather than refuse.
    let outcome = engine
        .signal(
            &workflow_id,
            &run_id,
            DECLARED_SIGNAL,
            payload(&json!({ "message": "", "end": true }))?,
        )
        .await;
    assert!(
        !matches!(outcome, Err(EngineError::SignalRefused { .. })),
        "an unretained identity declares nothing and must never be refused: {outcome:?}"
    );

    engine.shutdown()?;
    Ok(())
}
