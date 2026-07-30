//! Declared-signal admission against an exact retained package identity.
//!
//! The catalog is the authority on what a `(workflow type, version)` promised,
//! so it is where a caller-supplied signal is measured against the declaration
//! — before the engine records anything.

use aion_core::Payload;
use aion_package::ContentHash;

use super::WorkflowCatalog;
use crate::error::EngineError;

/// Why a signal did not satisfy the contract declared by the exact package
/// identity the target run is pinned to.
///
/// Every variant is a caller-facing refusal: the caller sent something the
/// package does not accept. None of them is an engine fault, and none of them
/// is reachable after anything has been recorded.
#[derive(Clone, Debug, thiserror::Error)]
pub enum SignalRefusalReason {
    /// The package declares a signal surface and this name is not on it.
    #[error("the package declares no signal `{signal_name}`; it declares {declared}")]
    Undeclared {
        /// Signal name the caller sent.
        signal_name: String,
        /// Comma-separated declared signal names.
        declared: String,
    },

    /// The payload bytes are not the JSON the declared type is written over.
    #[error("the payload is not decodable JSON: {reason}")]
    UndecodablePayload {
        /// Decode failure text.
        reason: String,
    },

    /// The payload decoded but does not satisfy the declared payload type.
    #[error("{violations}")]
    PayloadMismatch {
        /// Every field that did not match the declaration.
        violations: String,
    },
}

impl WorkflowCatalog {
    /// Admit a caller-supplied signal against the signal surface declared by
    /// the EXACT `(workflow_type, version)` identity a run is pinned to.
    ///
    /// The exact identity is load-bearing. A run parked for months holds the
    /// version it started on; a newer deploy of the same logical package may
    /// declare a different payload type, and admitting against the routed
    /// version would measure the operator's payload with the wrong ruler.
    ///
    /// Admission is CONTRACT-DRIVEN. Three cases admit without a check, and
    /// each is a deliberate reading of the declaration rather than a fallback:
    ///
    /// - The identity is no longer retained (its version was unloaded). This
    ///   engine holds no declaration for it, and refusing on absence would
    ///   strand a live run whose package was swapped out.
    /// - The identity predates the `.v4` contract commitment, so it commits to
    ///   no contract at all.
    /// - The contract declares NO signals. The package makes no promise about
    ///   its signal surface, so there is nothing to enforce; a package that
    ///   declares nothing must not have an empty declaration read as "accepts
    ///   nothing".
    ///
    /// Once a package DOES declare signals, the declaration is exhaustive: an
    /// undeclared name and a non-conforming payload are both refused. A
    /// declared signal whose own schema says nothing (`null`, `{}`, `true`)
    /// still constrains nothing, and a declared schema that cannot be compiled
    /// is a defect in the PACKAGE rather than in the caller's payload: it is
    /// logged and admitted, because refusing would blame the caller for
    /// something only a re-deploy can fix and would strand every signal that
    /// package's runs wait on.
    ///
    /// `Ok(None)` is admission; `Ok(Some(reason))` is the caller-facing
    /// refusal to report.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::CatalogPoisoned`] when the snapshot lock is
    /// poisoned.
    pub fn declared_signal_refusal(
        &self,
        workflow_type: &str,
        version: &ContentHash,
        signal_name: &str,
        payload: &Payload,
    ) -> Result<Option<SignalRefusalReason>, EngineError> {
        let snapshot = self.current()?;
        let Some(entry) = snapshot
            .by_version
            .get(&(workflow_type.to_owned(), version.clone()))
        else {
            tracing::debug!(
                workflow_type,
                %version,
                signal_name,
                "signal target's exact package identity is not retained; no declaration to admit against"
            );
            return Ok(None);
        };
        let Ok(contract) = entry.workflow.contract() else {
            tracing::debug!(
                workflow_type,
                %version,
                signal_name,
                "signal target's package identity predates the .v4 contract commitment; no declaration to admit against"
            );
            return Ok(None);
        };
        if contract.signals.is_empty() {
            return Ok(None);
        }
        let Some(declared) = contract.declared_signal(signal_name) else {
            return Ok(Some(SignalRefusalReason::Undeclared {
                signal_name: signal_name.to_owned(),
                declared: contract.declared_signal_names().join(", "),
            }));
        };
        if aion_package::declares_nothing(&declared.input_schema) {
            return Ok(None);
        }
        let value = match payload.to_json() {
            Ok(value) => value,
            Err(error) => {
                return Ok(Some(SignalRefusalReason::UndecodablePayload {
                    reason: error.to_string(),
                }));
            }
        };
        match aion_package::admit_value(&declared.input_schema, &value) {
            Ok(()) => Ok(None),
            Err(aion_package::AdmissionError::UnusableSchema { reason }) => {
                tracing::warn!(
                    workflow_type,
                    %version,
                    signal_name,
                    %reason,
                    "package declares a signal schema that is not valid JSON Schema; the payload could not be admitted against it and was allowed through"
                );
                Ok(None)
            }
            Err(aion_package::AdmissionError::Mismatch { violations }) => {
                Ok(Some(SignalRefusalReason::PayloadMismatch { violations }))
            }
        }
    }
}
