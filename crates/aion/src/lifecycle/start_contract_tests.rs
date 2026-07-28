//! Exact-version contract refusal tests for the workflow start boundary.

use std::sync::Arc;

use aion_core::{Payload, WorkflowId};
use aion_package::{ContentHash, PackageContract};
use aion_store::visibility::VisibilityStore;
use aion_store::{EventStore, InMemoryStore, ReadableEventStore};
use serde_json::json;

use super::{StartWorkflowContext, StartWorkflowOptions, start_workflow_with_options};
use crate::EngineError;
use crate::loader::{ActivityServing, WorkflowCatalog};
use crate::registry::Registry;
use crate::runtime::{RuntimeConfig, RuntimeHandle};
use crate::supervision::SupervisionTree;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn payload() -> Result<Payload, aion_core::PayloadError> {
    Payload::from_json(&json!({ "label": "input" }))
}

fn context(
    store: Arc<dyn EventStore>,
    visibility_store: Arc<dyn VisibilityStore>,
    catalog: Arc<WorkflowCatalog>,
    runtime: Arc<RuntimeHandle>,
    supervision: Arc<SupervisionTree>,
    registry: Arc<Registry>,
) -> StartWorkflowContext {
    StartWorkflowContext {
        store,
        visibility_store,
        catalog,
        runtime,
        supervision,
        registry,
        signal_handoff: None,
        search_attribute_schema: Arc::new(aion_core::SearchAttributeSchema::new()),
        monitor_tokio_handle: tokio::runtime::Handle::current(),
    }
}

#[tokio::test]
async fn pre_v4_exact_version_is_refused_before_history_or_registration() -> TestResult {
    let store = Arc::new(InMemoryStore::default());
    let catalog = Arc::new(WorkflowCatalog::new());
    let version = ContentHash::from_bytes([4; 32]);
    catalog.note_loaded_workflow_with_contract_for_test(
        "checkout",
        "checkout__legacy",
        "run",
        version.clone(),
        None,
    );
    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    let registry = Arc::new(Registry::default());
    let workflow_id = WorkflowId::new_v4();

    let result = start_workflow_with_options(
        context(
            store.clone(),
            store.clone(),
            catalog,
            Arc::clone(&runtime),
            Arc::new(SupervisionTree::new()),
            Arc::clone(&registry),
        ),
        "checkout",
        payload()?,
        StartWorkflowOptions {
            workflow_id: Some(workflow_id.clone()),
            loaded_version: Some(version),
            ..StartWorkflowOptions::default()
        },
    )
    .await;

    assert!(matches!(
        result,
        Err(EngineError::ContractIdentity { workflow_type, .. })
            if workflow_type == "checkout"
    ));
    assert!(store.read_history(&workflow_id).await?.is_empty());
    assert!(store.list_active().await?.is_empty());
    assert!(registry.list()?.is_empty());
    runtime.shutdown()?;
    Ok(())
}

#[tokio::test]
async fn unscoped_activity_contract_is_refused_before_history_or_registration() -> TestResult {
    let store = Arc::new(InMemoryStore::default());
    let catalog = Arc::new(WorkflowCatalog::new());
    catalog.note_loaded_workflow_with_contract_for_test(
        "checkout",
        "checkout__unscoped",
        "run",
        ContentHash::from_bytes([5; 32]),
        Some(PackageContract {
            unscoped_activities: vec!["charge".to_owned()],
            ..PackageContract::default()
        }),
    );
    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    let registry = Arc::new(Registry::default());
    let workflow_id = WorkflowId::new_v4();

    let result = start_workflow_with_options(
        context(
            store.clone(),
            store.clone(),
            catalog,
            Arc::clone(&runtime),
            Arc::new(SupervisionTree::new()),
            Arc::clone(&registry),
        ),
        "checkout",
        payload()?,
        StartWorkflowOptions {
            workflow_id: Some(workflow_id.clone()),
            ..StartWorkflowOptions::default()
        },
    )
    .await;

    assert!(matches!(
        result,
        Err(EngineError::NoQueueDeclaration {
            workflow_type,
            activities,
            ..
        }) if workflow_type == "checkout" && activities == "charge"
    ));
    assert!(store.read_history(&workflow_id).await?.is_empty());
    assert!(store.list_active().await?.is_empty());
    assert!(registry.list()?.is_empty());
    runtime.shutdown()?;
    Ok(())
}

/// Declared in-process serving admits an unscoped legacy contract past
/// admission: no queue exists to be unserved, so the start proceeds to
/// recording (proven by history landing) instead of the structural refusal.
#[tokio::test]
async fn in_process_serving_admits_unscoped_activities_past_admission() -> TestResult {
    let store = Arc::new(InMemoryStore::default());
    let catalog = Arc::new(WorkflowCatalog::new_with_serving(
        ActivityServing::InProcess,
    ));
    catalog.note_loaded_workflow_with_contract_for_test(
        "checkout",
        "checkout__unscoped",
        "run",
        ContentHash::from_bytes([6; 32]),
        Some(PackageContract {
            unscoped_activities: vec!["charge".to_owned()],
            ..PackageContract::default()
        }),
    );
    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    let registry = Arc::new(Registry::default());
    let workflow_id = WorkflowId::new_v4();

    let result = start_workflow_with_options(
        context(
            store.clone(),
            store.clone(),
            catalog,
            Arc::clone(&runtime),
            Arc::new(SupervisionTree::new()),
            Arc::clone(&registry),
        ),
        "checkout",
        payload()?,
        StartWorkflowOptions {
            workflow_id: Some(workflow_id.clone()),
            ..StartWorkflowOptions::default()
        },
    )
    .await;

    // Admission passed: whatever the fixture module does downstream, the
    // outcome is never the structural refusal, and `WorkflowStarted` landed.
    assert!(!matches!(
        result,
        Err(EngineError::NoQueueDeclaration { .. } | EngineError::ContractIdentity { .. })
    ));
    assert!(!store.read_history(&workflow_id).await?.is_empty());
    runtime.shutdown()?;
    Ok(())
}

/// The `.v4` identity floor is unconditional: declared in-process serving
/// does not grandfather a pre-`.v4` package identity.
#[tokio::test]
async fn in_process_serving_still_refuses_pre_v4_identity() -> TestResult {
    let store = Arc::new(InMemoryStore::default());
    let catalog = Arc::new(WorkflowCatalog::new_with_serving(
        ActivityServing::InProcess,
    ));
    let version = ContentHash::from_bytes([7; 32]);
    catalog.note_loaded_workflow_with_contract_for_test(
        "checkout",
        "checkout__legacy_inproc",
        "run",
        version.clone(),
        None,
    );
    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    let registry = Arc::new(Registry::default());
    let workflow_id = WorkflowId::new_v4();

    let result = start_workflow_with_options(
        context(
            store.clone(),
            store.clone(),
            catalog,
            Arc::clone(&runtime),
            Arc::new(SupervisionTree::new()),
            Arc::clone(&registry),
        ),
        "checkout",
        payload()?,
        StartWorkflowOptions {
            workflow_id: Some(workflow_id.clone()),
            loaded_version: Some(version),
            ..StartWorkflowOptions::default()
        },
    )
    .await;

    assert!(matches!(
        result,
        Err(EngineError::ContractIdentity { workflow_type, .. })
            if workflow_type == "checkout"
    ));
    assert!(store.read_history(&workflow_id).await?.is_empty());
    runtime.shutdown()?;
    Ok(())
}
