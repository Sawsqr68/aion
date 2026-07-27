//! Exact-version structural admission before workflow start mutation.

use std::sync::Arc;

use aion_core::{Event, WorkflowId};
use aion_package::ContentHash;
use aion_store::EventStore;

use crate::EngineError;
use crate::loader::{ActivityServing, PinnedWorkflow, WorkflowCatalog};

pub(super) fn resolve_contract(
    catalog: &WorkflowCatalog,
    workflow_type: &str,
    loaded_version: Option<&ContentHash>,
) -> Result<PinnedWorkflow, EngineError> {
    let pinned = match loaded_version {
        Some(version) => catalog.resolve_exact(workflow_type, version)?,
        None => catalog.resolve_routed(workflow_type)?,
    }
    .ok_or_else(|| EngineError::WorkflowNotFound {
        workflow_type: workflow_type.to_owned(),
    })?;
    let loaded = pinned.workflow();
    // The `.v4` identity floor holds in every serving posture: a pre-`.v4`
    // package is never grandfathered.
    let contract = loaded
        .contract()
        .map_err(|source| EngineError::ContractIdentity {
            workflow_type: workflow_type.to_owned(),
            source,
        })?;
    // The queue-service check protects only queue-routed fulfilment (an
    // unserved queue waits silently forever). Under declared in-process
    // serving no queue exists: dispatch runs on the configured dispatcher
    // immediately, and a missing dispatcher fails loudly at dispatch.
    if catalog.activity_serving() == ActivityServing::QueueRouted
        && !contract.unscoped_activities.is_empty()
    {
        let mut activities = contract.unscoped_activities.clone();
        activities.sort();
        return Err(EngineError::NoQueueDeclaration {
            workflow_type: workflow_type.to_owned(),
            version: loaded.version().clone(),
            activities: activities.join(","),
        });
    }
    Ok(pinned)
}

pub(super) async fn workflow_identity(
    store: &Arc<dyn EventStore>,
    requested: Option<WorkflowId>,
) -> Result<(WorkflowId, u64), EngineError> {
    let Some(workflow_id) = requested else {
        return Ok((WorkflowId::new_v4(), 0));
    };
    let initial_head = store
        .read_history(&workflow_id)
        .await?
        .iter()
        .map(Event::seq)
        .max()
        .unwrap_or_default();
    Ok((workflow_id, initial_head))
}
