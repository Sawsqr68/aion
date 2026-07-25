//! Activity task-envelope generation and completion fencing.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use aion_core::{ActivityId, RunId, WorkflowId};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::{CompletionRejectionReason, ServerError};

type ExecutionKey = (WorkflowId, ActivityId);

/// Opaque proof that a worker owns one dispatched execution generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionToken(String);

impl CompletionToken {
    /// Parse a worker-echoed token without assigning meaning to its contents.
    ///
    /// # Errors
    ///
    /// Returns a typed compatibility refusal when a pre-fencing worker omits it.
    pub fn from_wire(
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
        value: String,
    ) -> Result<Self, ServerError> {
        if value.is_empty() {
            return Err(rejection(
                workflow_id,
                activity_id,
                CompletionRejectionReason::MissingCompletionToken,
            ));
        }
        Ok(Self(value))
    }

    /// Return the opaque wire representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Build a non-wire token for crate-local unit fixtures.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn for_test() -> Self {
        Self("test-generation".to_owned())
    }
}

/// Process-incarnation registry of the only generations allowed to complete.
///
/// Recovery deliberately starts with an empty registry. Re-dispatch issues a
/// fresh token, so a result carrying any pre-recovery token is refused whether
/// it arrives before or after the new dispatch.
#[derive(Clone, Debug, Default)]
pub struct CompletionFences {
    current: Arc<Mutex<HashMap<ExecutionKey, CompletionToken>>>,
}

impl CompletionFences {
    /// Supersede any prior generation and return the newly authorized token.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] when fence state cannot be trusted.
    pub fn issue(
        &self,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
    ) -> Result<CompletionToken, ServerError> {
        let token = CompletionToken(Uuid::new_v4().to_string());
        self.state()?
            .insert((workflow_id.clone(), activity_id.clone()), token.clone());
        Ok(token)
    }

    /// Consume the current generation only when `submitted` exactly matches it.
    ///
    /// Consumption and comparison share one mutex critical section, making two
    /// concurrent submissions unable to both become truth.
    ///
    /// # Errors
    ///
    /// Returns a typed rejection for a missing/current-generation mismatch, or
    /// [`ServerError::LockPoisoned`] when fence state cannot be trusted.
    pub fn accept(
        &self,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
        submitted: &CompletionToken,
    ) -> Result<(), ServerError> {
        let key = (workflow_id.clone(), activity_id.clone());
        let mut state = self.state()?;
        let Some(current) = state.get(&key) else {
            return Err(rejection(
                workflow_id,
                activity_id,
                CompletionRejectionReason::NoCurrentGeneration,
            ));
        };
        if current != submitted {
            return Err(rejection(
                workflow_id,
                activity_id,
                CompletionRejectionReason::StaleGeneration,
            ));
        }
        state.remove(&key);
        Ok(())
    }

    /// Revoke `token` if it is still current, without disturbing a newer retry.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] when fence state cannot be trusted.
    pub fn revoke(
        &self,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
        token: &CompletionToken,
    ) -> Result<(), ServerError> {
        let key = (workflow_id.clone(), activity_id.clone());
        let mut state = self.state()?;
        if state.get(&key) == Some(token) {
            state.remove(&key);
        }
        Ok(())
    }

    /// Revoke whichever generation is current while parking for recovery.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] when fence state cannot be trusted.
    pub fn revoke_current(
        &self,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
    ) -> Result<(), ServerError> {
        self.state()?
            .remove(&(workflow_id.clone(), activity_id.clone()));
        Ok(())
    }

    fn state(&self) -> Result<MutexGuard<'_, HashMap<ExecutionKey, CompletionToken>>, ServerError> {
        self.current
            .lock()
            .map_err(|_| ServerError::lock_poisoned("activity completion fences"))
    }
}

/// Derive the stable external-effect key for one action site in one workflow run.
///
/// Attempts and execution generations are intentionally absent. The domain tag,
/// workflow id, run id, and activity ordinal are length-unambiguous fixed-width
/// inputs to SHA-256.
#[must_use]
pub fn idempotency_key(
    workflow_id: &WorkflowId,
    run_id: &RunId,
    activity_id: &ActivityId,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"aion.activity.idempotency.v1\0");
    hasher.update(workflow_id.as_uuid().as_bytes());
    hasher.update(run_id.as_uuid().as_bytes());
    hasher.update(activity_id.sequence_position().to_be_bytes());
    encode_hex(&hasher.finalize())
}

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn rejection(
    workflow_id: &WorkflowId,
    activity_id: &ActivityId,
    reason: CompletionRejectionReason,
) -> ServerError {
    ServerError::ActivityCompletionRejected {
        workflow_id: workflow_id.clone(),
        activity_id: activity_id.clone(),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::{CompletionFences, CompletionToken, idempotency_key};
    use crate::error::{CompletionRejectionReason, ServerError};
    use aion_core::{ActivityId, RunId, WorkflowId};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn idempotency_key_is_attempt_independent_and_site_run_scoped() {
        let workflow = WorkflowId::new_v4();
        let run_a = RunId::new_v4();
        let run_b = RunId::new_v4();
        let site_a = ActivityId::from_sequence_position(7);
        let site_b = ActivityId::from_sequence_position(8);

        let first_attempt = idempotency_key(&workflow, &run_a, &site_a);
        let fifth_attempt = idempotency_key(&workflow, &run_a, &site_a);
        assert_eq!(first_attempt, fifth_attempt);
        assert_ne!(first_attempt, idempotency_key(&workflow, &run_a, &site_b));
        assert_ne!(first_attempt, idempotency_key(&workflow, &run_b, &site_a));
    }

    #[test]
    fn issuing_a_retry_rejects_the_stale_generation() -> TestResult {
        let fences = CompletionFences::default();
        let workflow = WorkflowId::new_v4();
        let activity = ActivityId::from_sequence_position(3);
        let stale = fences.issue(&workflow, &activity)?;
        let current = fences.issue(&workflow, &activity)?;

        let rejected = fences.accept(&workflow, &activity, &stale);
        assert!(matches!(
            rejected,
            Err(ServerError::ActivityCompletionRejected {
                reason: CompletionRejectionReason::StaleGeneration,
                ..
            })
        ));
        fences.accept(&workflow, &activity, &current)?;
        Ok(())
    }

    #[test]
    fn accepted_generation_is_consumed_exactly_once() -> TestResult {
        let fences = CompletionFences::default();
        let workflow = WorkflowId::new_v4();
        let activity = ActivityId::from_sequence_position(4);
        let token = fences.issue(&workflow, &activity)?;

        fences.accept(&workflow, &activity, &token)?;
        let duplicate = fences.accept(&workflow, &activity, &token);
        assert!(matches!(
            duplicate,
            Err(ServerError::ActivityCompletionRejected {
                reason: CompletionRejectionReason::NoCurrentGeneration,
                ..
            })
        ));
        Ok(())
    }

    #[test]
    fn revoking_an_old_generation_does_not_remove_its_replacement() -> TestResult {
        let fences = CompletionFences::default();
        let workflow = WorkflowId::new_v4();
        let activity = ActivityId::from_sequence_position(6);
        let old = fences.issue(&workflow, &activity)?;
        let replacement = fences.issue(&workflow, &activity)?;

        fences.revoke(&workflow, &activity, &old)?;
        fences.accept(&workflow, &activity, &replacement)?;
        Ok(())
    }

    #[test]
    fn an_empty_wire_token_is_a_typed_compatibility_refusal() {
        let workflow = WorkflowId::new_v4();
        let activity = ActivityId::from_sequence_position(9);
        let rejected = CompletionToken::from_wire(&workflow, &activity, String::new());
        assert!(matches!(
            rejected,
            Err(ServerError::ActivityCompletionRejected {
                reason: CompletionRejectionReason::MissingCompletionToken,
                ..
            })
        ));
    }

    #[test]
    fn a_pre_recovery_generation_is_rejected_after_recovery() -> TestResult {
        let before_recovery = CompletionFences::default();
        let workflow = WorkflowId::new_v4();
        let activity = ActivityId::from_sequence_position(5);
        let stale = before_recovery.issue(&workflow, &activity)?;

        let after_recovery = CompletionFences::default();
        let current = after_recovery.issue(&workflow, &activity)?;
        let rejected = after_recovery.accept(&workflow, &activity, &stale);
        assert!(matches!(
            rejected,
            Err(ServerError::ActivityCompletionRejected {
                reason: CompletionRejectionReason::StaleGeneration,
                ..
            })
        ));
        after_recovery.accept(&workflow, &activity, &current)?;
        Ok(())
    }
}
