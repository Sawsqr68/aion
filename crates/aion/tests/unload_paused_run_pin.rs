//! A durably-`Paused` run pins its package version against unload.
//!
//! `EventStore::list_active` projects `Running` ONLY — `Paused` has its own
//! `list_paused` projection — and startup recovery deliberately does not
//! respawn a paused run, so it holds no registry handle either. Unload
//! verification scanned the active set alone, which meant a paused run was
//! invisible to every one of the four pin checks: no in-flight start, no live
//! handle, not in `list_active`, and not a recorded-unstarted child. Unloading
//! then deleted the module the operator's `resume` would put that run straight
//! back into, stranding it permanently with no way to recover it.
//!
//! `Paused` is non-terminal by the status projection (invariant 4) and is
//! superseded by `WorkflowResumed`, so it is a live run in every sense that
//! matters to unload safety.

use std::sync::Arc;

use aion::{EngineBuilder, EngineError, PinHolder};
use aion_core::{Payload, RunId, WorkflowId};
use aion_package::{ExtractionLimits, Package};
use aion_store::{EventStore, InMemoryStore};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const V1: &str = r"//! Paused-pin fixture, first deploy.
workflow paused_pin
  input amount: Int
  outcome completed: type Result, route success

type Result { approved: Bool }

worker payments
  action charge(amount: Int) -> Result

step run
  charge(amount: amount) -> result
  route completed(approved: result.approved)
";

const V2: &str = r"//! Paused-pin fixture, second deploy.
workflow paused_pin
  input amount: Int
  input currency: String
  outcome completed: type Result, route success

type Result { approved: Bool }

worker payments
  action charge(amount: Int, currency: String) -> Result

step run
  charge(amount: amount, currency: currency) -> result
  route completed(approved: result.approved)
";

fn package(source: &str) -> Result<Package, Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let prepared = aion_awl_package::compile_and_assemble_awl(source, root.path())?;
    Ok(Package::load_from_bytes(
        prepared.archive,
        ExtractionLimits::unbounded(),
    )?)
}

#[tokio::test]
async fn a_durably_paused_run_refuses_the_unload_of_its_version() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = EngineBuilder::new()
        .store_arc(Arc::clone(&store))
        .in_memory_visibility()
        .build()
        .await?;

    let v1 = package(V1)?;
    let v1_hash = v1.content_hash().clone();
    engine.load_package(v1).await?;
    // A second deploy takes the route, so v1 is unload-eligible on routing.
    engine.load_package(package(V2)?).await?;

    // A run pinned to v1 that is durably PAUSED and holds no registry handle:
    // exactly the state a `kill -9` leaves behind, and exactly what startup
    // recovery declines to respawn.
    let paused_id = WorkflowId::new_v4();
    let paused_run = RunId::new_v4();
    let mut recorder = aion::durability::Recorder::new(paused_id.clone(), Arc::clone(&store));
    recorder
        .record_workflow_started(
            chrono::Utc::now(),
            aion::durability::WorkflowStartRecord {
                workflow_type: "paused_pin".to_owned(),
                input: Payload::from_json(&serde_json::json!({ "amount": 1 }))?,
                run_id: paused_run.clone(),
                parent_run_id: None,
                package_version: aion_core::PackageVersion::new(v1_hash.to_string()),
            },
        )
        .await?;
    recorder
        .record_workflow_paused(
            chrono::Utc::now(),
            paused_run.clone(),
            Some("operator hold".to_owned()),
            Some("test".to_owned()),
        )
        .await?;

    // The premise of the defect: this run is invisible to the active listing.
    assert!(
        !store.list_active().await?.contains(&paused_id),
        "the fixture must reproduce the blind spot: a paused run is not `list_active`"
    );
    assert!(
        store.list_paused().await?.contains(&paused_id),
        "the paused run must be in the paused projection"
    );

    let result = engine.unload_workflow_version("paused_pin", &v1_hash).await;
    assert!(
        matches!(&result, Err(EngineError::VersionPinned {
            pinned_by: PinHolder::RecoverableRun { workflow_id },
            ..
        }) if workflow_id == &paused_id),
        "unload must refuse and name the paused run pinning the version: {result:?}"
    );
    // The refusal restored the catalog untouched.
    assert_eq!(engine.list_workflow_versions()?.len(), 2);

    // The check is precise, not a blanket refusal: once the run is terminal it
    // pins nothing and the version unloads.
    recorder
        .record_workflow_completed(
            chrono::Utc::now(),
            Payload::from_json(&serde_json::json!({ "approved": true }))?,
        )
        .await?;
    engine
        .unload_workflow_version("paused_pin", &v1_hash)
        .await?;
    assert_eq!(engine.list_workflow_versions()?.len(), 1);

    engine.shutdown()?;
    Ok(())
}
