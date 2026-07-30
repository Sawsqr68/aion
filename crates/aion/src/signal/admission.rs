//! Declared-signal admission at the engine's signal boundary.
//!
//! THE ORDERING IS THE POINT. A signal payload used to be recorded opaquely
//! and decoded later, inside the workflow process, with the arrival marked
//! consumed BEFORE the decode was attempted — so a malformed body from an
//! operator killed a durable run that may have been parked for months, and the
//! body was gone, so there was nothing to retry.
//!
//! Everything in this module runs BEFORE any append and before any delivery.
//! A refusal therefore cannot consume the arrival: the run's history is
//! byte-identical afterwards and it stays parked on exactly the wait it was
//! parked on, while the caller is told which field of their payload was wrong.

use aion_core::{Event, Payload, RunId, WorkflowId};
use aion_package::ContentHash;

use crate::error::EngineError;
use crate::loader::{WorkflowCatalog, parse_package_version};
use crate::registry::WorkflowHandle;

/// Admit a signal against the declaration of the identity a LIVE handle is
/// pinned to.
///
/// # Errors
///
/// Returns [`EngineError::SignalRefused`] when the signal is not declared or
/// its payload does not satisfy the declared type, and
/// [`EngineError::CatalogPoisoned`] when the catalog snapshot lock is poisoned.
pub(crate) fn admit_against_handle(
    catalog: &WorkflowCatalog,
    handle: &WorkflowHandle,
    signal_name: &str,
    payload: &Payload,
) -> Result<(), EngineError> {
    admit(
        catalog,
        handle.workflow_id(),
        handle.run_id(),
        handle.workflow_type(),
        handle.loaded_version(),
        signal_name,
        payload,
    )
}

/// Admit a signal against the declaration of the identity the run's recorded
/// `WorkflowStarted` pins it to.
///
/// This is the non-resident path — a run that is paused, or still inside the
/// registration birth window — where no live handle exists to read the pinned
/// identity from. The recorded start event is the durable equivalent: replay
/// and recovery resolve this run's code from exactly that version, so it is
/// the same ruler the resident path uses.
///
/// A run with no recorded `WorkflowStarted` in `history` has no identity to
/// admit against and is left to the caller's own not-found handling; nothing
/// is recorded either way. A recorded version that is not a canonical content
/// hash resolves to no identity either — it is logged and admitted rather than
/// propagated, because failing here would strand every future signal to that
/// run over a historical value the caller cannot influence.
///
/// # Errors
///
/// Returns [`EngineError::SignalRefused`] when the signal is not declared or
/// its payload does not satisfy the declared type, and
/// [`EngineError::CatalogPoisoned`] when the catalog snapshot lock is
/// poisoned.
pub(crate) fn admit_against_history(
    catalog: &WorkflowCatalog,
    workflow_id: &WorkflowId,
    run_id: &RunId,
    history: &[Event],
    signal_name: &str,
    payload: &Payload,
) -> Result<(), EngineError> {
    let Some((workflow_type, package_version)) = started_identity(history, run_id) else {
        return Ok(());
    };
    let version = match parse_package_version(workflow_type, package_version) {
        Ok(version) => version,
        Err(error) => {
            tracing::warn!(
                %workflow_id,
                %run_id,
                workflow_type,
                %package_version,
                signal_name,
                %error,
                "run's recorded package version is not a canonical content hash; no declaration could be resolved to admit this signal against"
            );
            return Ok(());
        }
    };
    admit(
        catalog,
        workflow_id,
        run_id,
        workflow_type,
        &version,
        signal_name,
        payload,
    )
}

/// The `(workflow type, package version)` the run's `WorkflowStarted` pinned.
fn started_identity<'a>(
    history: &'a [Event],
    run_id: &RunId,
) -> Option<(&'a str, &'a aion_core::PackageVersion)> {
    history.iter().find_map(|event| match event {
        Event::WorkflowStarted {
            workflow_type,
            run_id: started_run,
            package_version,
            ..
        } if started_run == run_id => Some((workflow_type.as_str(), package_version)),
        _ => None,
    })
}

fn admit(
    catalog: &WorkflowCatalog,
    workflow_id: &WorkflowId,
    run_id: &RunId,
    workflow_type: &str,
    version: &ContentHash,
    signal_name: &str,
    payload: &Payload,
) -> Result<(), EngineError> {
    let Some(reason) =
        catalog.declared_signal_refusal(workflow_type, version, signal_name, payload)?
    else {
        return Ok(());
    };
    Err(EngineError::SignalRefused {
        workflow_id: workflow_id.clone(),
        run_id: run_id.clone(),
        signal_name: signal_name.to_owned(),
        version: version.clone(),
        reason: reason.to_string(),
    })
}
