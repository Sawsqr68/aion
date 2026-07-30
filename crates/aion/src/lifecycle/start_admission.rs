//! Exact-version structural admission before workflow start mutation.

use std::sync::Arc;

use aion_core::{Event, Payload, WorkflowId};
use aion_package::ContentHash;
use aion_store::EventStore;

use crate::EngineError;
use crate::loader::{ActivityServing, LoadedWorkflow, PinnedWorkflow, WorkflowCatalog};

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

/// Admit a caller-supplied start input against the input type declared by the
/// exact package identity the start resolved to.
///
/// Runs BEFORE `workflow_identity`, before the recorder, and before any
/// process is spawned, so a refused start records nothing and leaves nothing to
/// clean up: the caller sees their own mistake at the moment they made it,
/// rather than the run dying from the inside some time later.
///
/// A contract whose entry declares no input shape — a legacy or defaulted
/// manifest, whose `input_schema` is `null` — admits everything, because that
/// is precisely what an absent declaration says.
///
/// A schema that IS declared but cannot be compiled is a defect in the
/// PACKAGE, not in the caller's value. It is logged and admitted: refusing
/// would blame the caller for something they cannot fix and would make every
/// start of that package impossible, which is a worse failure than the one
/// this admission exists to prevent.
///
/// # Errors
///
/// Returns [`EngineError::ContractIdentity`] for a pre-`.v4` identity (already
/// refused by [`resolve_contract`], propagated rather than assumed) and
/// [`EngineError::StartInputRefused`] when the input does not satisfy the
/// declared schema.
pub(super) fn admit_declared_input(
    loaded: &LoadedWorkflow,
    input: &Payload,
) -> Result<(), EngineError> {
    let workflow_type = loaded.workflow_type();
    let contract = loaded
        .contract()
        .map_err(|source| EngineError::ContractIdentity {
            workflow_type: workflow_type.to_owned(),
            source,
        })?;
    let schema = contract.entry_input_schema(workflow_type);
    if aion_package::declares_nothing(schema) {
        return Ok(());
    }
    let refuse = |reason: String| EngineError::StartInputRefused {
        workflow_type: workflow_type.to_owned(),
        version: loaded.version().clone(),
        reason,
    };
    let value = input
        .to_json()
        .map_err(|error| refuse(format!("the input is not decodable JSON: {error}")))?;
    match aion_package::admit_value(schema, &value) {
        Ok(()) => Ok(()),
        Err(aion_package::AdmissionError::UnusableSchema { reason }) => {
            tracing::warn!(
                workflow_type,
                version = %loaded.version(),
                %reason,
                "package declares an input schema that is not valid JSON Schema; the start input could not be admitted against it and was allowed through"
            );
            Ok(())
        }
        Err(error @ aion_package::AdmissionError::Mismatch { .. }) => {
            Err(refuse(error.to_string()))
        }
    }
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
