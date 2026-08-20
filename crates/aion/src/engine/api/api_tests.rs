use std::sync::Arc;
use std::time::Duration;

use aion_core::{
    Event, EventEnvelope, PackageVersion, Payload, RunId, SearchAttributeSchema, TimerCancelCause,
    TimerId, WorkflowFilter, WorkflowId, WorkflowStatus,
};
use aion_package::ContentHash;
use aion_store::visibility::VisibilityStore;
use aion_store::{EventStore, InMemoryStore, ReadableEventStore};
use serde_json::json;

use std::collections::HashMap;

use super::{DelegatedSeams, Engine, EngineComponents};
use crate::durability::Recorder;
use crate::lifecycle::terminate::{self, TerminateWorkflowContext};
use crate::registry::{CompletionNotifier, HandleResidency, WorkflowHandleParts};
use crate::time::TimerRecovery;
use crate::time::timer_service::live_timers_in_active_segment;
use crate::{
    EngineError, Registry, RuntimeConfig, RuntimeHandle, SupervisionTree, WorkflowCatalog,
    WorkflowHandle,
};

fn payload(label: &str) -> Result<Payload, aion_core::PayloadError> {
    Payload::from_json(&json!({ "label": label }))
}

fn workflow_error(message: &str) -> aion_core::WorkflowError {
    aion_core::WorkflowError {
        message: message.to_owned(),
        details: None,
    }
}

fn workflow_catalog(workflow_type: &str, deployed_module: &str) -> Arc<WorkflowCatalog> {
    let catalog = Arc::new(WorkflowCatalog::new());
    catalog.note_loaded_workflow_for_test(
        workflow_type,
        deployed_module,
        "run",
        ContentHash::from_bytes([5; 32]),
    );
    catalog
}

fn engine_with_loaded_workflow(
    store: Arc<dyn EventStore>,
    workflow_type: &str,
    deployed_module: &str,
) -> Result<Engine, EngineError> {
    let runtime = RuntimeHandle::new(RuntimeConfig::new(Some(1)))?;
    runtime.register_waiting_test_module(deployed_module, "run");
    let visibility_store: Arc<dyn VisibilityStore> = Arc::new(InMemoryStore::default());
    Ok(Engine::new(EngineComponents {
        store,
        visibility_store,
        runtime: Arc::new(runtime),
        catalog: workflow_catalog(workflow_type, deployed_module),
        registry: Arc::new(Registry::default()),
        supervision: Arc::new(SupervisionTree::new()),
        delegated: DelegatedSeams::default(),
        signal_handoff: Arc::new(crate::signal::SignalResumeHandoff::new()),
        search_attribute_schema: Arc::new(SearchAttributeSchema::new()),
        visibility_reconciliation_task: None,
        deferred_startup_recovery: None,
    }))
}

fn termination_context(engine: &Engine) -> TerminateWorkflowContext<'_> {
    TerminateWorkflowContext {
        runtime: engine.runtime(),
        store: engine.store(),
        visibility_store: engine.visibility_store(),
        registry: engine.registry(),
        catalog: engine.workflow_catalog(),
    }
}

async fn insert_active_handle(
    engine: &Engine,
    store: Arc<dyn EventStore>,
    workflow_type: &str,
) -> Result<WorkflowHandle, Box<dyn std::error::Error>> {
    let workflow_id = aion_core::WorkflowId::new_v4();
    let run_id = aion_core::RunId::new_v4();
    let mut recorder = Recorder::new(workflow_id.clone(), store);
    recorder
        .record_workflow_started(
            chrono::Utc::now(),
            crate::durability::WorkflowStartRecord {
                workflow_type: workflow_type.to_owned(),
                input: payload("input")?,
                run_id: run_id.clone(),
                parent_run_id: None,
                parent_workflow_id: None,
                package_version: aion_core::PackageVersion::new("a".repeat(64)),
            },
        )
        .await?;
    let pid = engine.runtime().spawn_test_process_with_trap_exit(true)?;
    let handle = WorkflowHandle::new(WorkflowHandleParts {
        workflow_id: workflow_id.clone(),
        run_id: run_id.clone(),
        pid,
        workflow_type: workflow_type.to_owned(),
        namespace: String::from("default"),
        loaded_version: ContentHash::from_bytes([9; 32]),
        cached_status: WorkflowStatus::Running,
        residency: HandleResidency::Resident,
        recorder,
        completion: CompletionNotifier::new(),
    });
    engine
        .registry()
        .insert((workflow_id, run_id), handle.clone())?;
    Ok(handle)
}

mod lifecycle;
mod shutdown;
