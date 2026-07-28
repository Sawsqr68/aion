//! `ActivityTask` decode and `TaskResult`/`TaskFailure` encode.

use std::collections::BTreeMap;

use aion_core::{ActivityId, Payload, RunId, WorkflowId};
use aion_proto::ProtoActivityTask;

use crate::error::WorkerError;

/// SDK-level activity task envelope decoded from the AW-owned worker proto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityTask {
    /// Owning workflow id, required later when reporting this task's outcome.
    pub workflow_id: WorkflowId,
    /// Activity id correlating reports and heartbeats with this task.
    pub activity_id: ActivityId,
    /// Concrete workflow run that staged this task. Live decoding rejects a
    /// missing value; the `Option` remains only for manually built test tasks.
    pub run_id: Option<RunId>,
    /// Registered activity type name requested by the engine.
    pub activity_type: String,
    /// One-based delivery attempt stamped by the dispatching engine seam and
    /// read from the wire. Zero is malformed and rejected at decode.
    pub attempt: u32,
    /// Opaque execution-generation token echoed verbatim on every outcome.
    pub completion_token: String,
    /// Stable external-effect key for this run and action site. Identical across
    /// retries and available to handlers through [`crate::ActivityContext`].
    pub idempotency_key: String,
    /// Opaque activity input payload, preserving its content-type tag.
    pub input: Payload,
    /// Human-meaningful display labels the workflow attached to the activity
    /// (for example `brief=IP-001`). Display metadata only — surfaced in the
    /// worker's logs. `BTreeMap` keeps the rendered order stable; empty when
    /// the workflow attached none.
    pub labels: BTreeMap<String, String>,
}

impl TryFrom<ProtoActivityTask> for ActivityTask {
    type Error = WorkerError;

    fn try_from(value: ProtoActivityTask) -> Result<Self, Self::Error> {
        let workflow_id = value
            .workflow_id
            .ok_or(MalformedActivityTask::MissingWorkflowId)
            .and_then(|workflow_id| {
                WorkflowId::try_from(workflow_id)
                    .map_err(|source| MalformedActivityTask::InvalidWorkflowId { source })
            })
            .map_err(WorkerError::decode)?;
        let activity_id = value
            .activity_id
            .ok_or(MalformedActivityTask::MissingActivityId)
            .map(ActivityId::from)
            .map_err(WorkerError::decode)?;
        let run_id = value
            .run_id
            .ok_or(MalformedActivityTask::MissingRunId)
            .and_then(|run_id| {
                RunId::try_from(run_id)
                    .map_err(|source| MalformedActivityTask::InvalidRunId { source })
            })
            .map_err(WorkerError::decode)?;
        if value.activity_type.is_empty() {
            return Err(WorkerError::decode(
                MalformedActivityTask::MissingActivityType,
            ));
        }
        let input = value
            .input
            .ok_or(MalformedActivityTask::MissingInput)
            .and_then(|input| {
                Payload::try_from(input)
                    .map_err(|source| MalformedActivityTask::InvalidInput { source })
            })
            .map_err(WorkerError::decode)?;

        if value.attempt == 0 {
            // proto3 zero default = the producer failed to stamp the attempt.
            return Err(WorkerError::decode(MalformedActivityTask::MissingAttempt));
        }
        if value.completion_token.is_empty() {
            return Err(WorkerError::decode(
                MalformedActivityTask::MissingCompletionToken,
            ));
        }
        if value.idempotency_key.is_empty() {
            return Err(WorkerError::decode(
                MalformedActivityTask::MissingIdempotencyKey,
            ));
        }

        Ok(Self {
            workflow_id,
            activity_id,
            run_id: Some(run_id),
            activity_type: value.activity_type,
            attempt: value.attempt,
            completion_token: value.completion_token,
            idempotency_key: value.idempotency_key,
            input,
            labels: value.labels.into_iter().collect(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
enum MalformedActivityTask {
    #[error("activity task workflow_id is missing")]
    MissingWorkflowId,
    #[error("activity task workflow_id is invalid: {source}")]
    InvalidWorkflowId { source: aion_proto::WireError },
    #[error("activity task activity_id is missing")]
    MissingActivityId,
    #[error("activity task activity_type is missing")]
    MissingActivityType,
    #[error("activity task input payload is missing")]
    MissingInput,
    #[error("activity task attempt is missing or zero (producer failed to stamp it)")]
    MissingAttempt,
    #[error("activity task completion_token is missing (server registration era is incompatible)")]
    MissingCompletionToken,
    #[error("activity task idempotency_key is missing (server registration era is incompatible)")]
    MissingIdempotencyKey,
    #[error("activity task input payload is invalid: {source}")]
    InvalidInput { source: aion_proto::WireError },
    #[error("activity task run_id is missing (server registration era is incompatible)")]
    MissingRunId,
    #[error("activity task run_id is invalid: {source}")]
    InvalidRunId { source: aion_proto::WireError },
}

#[cfg(test)]
mod tests {
    use aion_core::{ActivityId, ContentType, Payload, WorkflowId};
    use aion_proto::{
        ProtoActivityId, ProtoActivityTask, ProtoPayload, ProtoRunId, ProtoWorkflowId,
    };
    use serde_json::json;

    use super::ActivityTask;
    use crate::WorkerError;

    #[test]
    fn decodes_proto_activity_task_preserving_payload_content_type()
    -> Result<(), Box<dyn std::error::Error>> {
        let workflow_id = WorkflowId::new_v4();
        let activity_id = ActivityId::from_sequence_position(42);
        let run_id = aion_core::RunId::new_v4();
        let input_value = json!({"amount": 1250, "currency": "USD"});
        let input = Payload::from_json(&input_value)?;
        let proto = ProtoActivityTask {
            workflow_id: Some(ProtoWorkflowId::from(workflow_id.clone())),
            activity_id: Some(ProtoActivityId::from(activity_id.clone())),
            run_id: Some(ProtoRunId::from(run_id.clone())),
            activity_type: String::from("charge-card"),
            input: Some(ProtoPayload::from(input.clone())),
            attempt: 3,
            completion_token: String::from("generation-3"),
            idempotency_key: String::from("effect-key"),
            labels: [(String::from("brief"), String::from("IP-001"))]
                .into_iter()
                .collect(),
        };

        let task = ActivityTask::try_from(proto)?;

        assert_eq!(task.workflow_id, workflow_id);
        assert_eq!(task.activity_id, activity_id);
        assert_eq!(task.run_id, Some(run_id));
        assert_eq!(task.activity_type, "charge-card");
        assert_eq!(task.attempt, 3, "attempt must be read from the wire");
        assert_eq!(task.input.content_type(), &ContentType::Json);
        assert_eq!(task.input.bytes(), input.bytes());
        assert_eq!(task.input.to_json()?, input_value);
        assert_eq!(
            task.labels.get("brief").map(String::as_str),
            Some("IP-001"),
            "display labels must decode from the wire"
        );
        Ok(())
    }

    #[test]
    fn missing_required_field_maps_to_decode_error() {
        let result = ActivityTask::try_from(ProtoActivityTask {
            workflow_id: None,
            activity_id: Some(ProtoActivityId::from(ActivityId::from_sequence_position(1))),
            run_id: None,
            activity_type: String::from("charge-card"),
            input: Some(ProtoPayload::from(Payload::new(
                ContentType::Json,
                b"{}".to_vec(),
            ))),
            attempt: 1,
            completion_token: String::from("generation-1"),
            idempotency_key: String::from("effect-key"),
            labels: std::collections::HashMap::new(),
        });

        assert!(matches!(result, Err(WorkerError::Decode { .. })));
    }

    #[test]
    fn zero_attempt_is_a_malformed_task() -> Result<(), Box<dyn std::error::Error>> {
        let result = ActivityTask::try_from(ProtoActivityTask {
            workflow_id: Some(ProtoWorkflowId::from(WorkflowId::new_v4())),
            activity_id: Some(ProtoActivityId::from(ActivityId::from_sequence_position(1))),
            run_id: Some(ProtoRunId::from(aion_core::RunId::new_v4())),
            activity_type: String::from("charge-card"),
            input: Some(ProtoPayload::from(Payload::new(
                ContentType::Json,
                b"{}".to_vec(),
            ))),
            attempt: 0,
            completion_token: String::from("generation-1"),
            idempotency_key: String::from("effect-key"),
            labels: std::collections::HashMap::new(),
        });

        let error = result
            .err()
            .ok_or("attempt 0 must be rejected as malformed")?;
        assert!(matches!(error, WorkerError::Decode { .. }));
        assert!(
            error.to_string().contains("attempt"),
            "error must name the attempt field: {error}"
        );
        Ok(())
    }

    #[test]
    fn missing_fencing_fields_are_incompatible_tasks() -> Result<(), Box<dyn std::error::Error>> {
        let valid = ProtoActivityTask {
            workflow_id: Some(ProtoWorkflowId::from(WorkflowId::new_v4())),
            activity_id: Some(ProtoActivityId::from(ActivityId::from_sequence_position(1))),
            run_id: Some(ProtoRunId::from(aion_core::RunId::new_v4())),
            activity_type: String::from("charge-card"),
            input: Some(ProtoPayload::from(Payload::new(
                ContentType::Json,
                b"{}".to_vec(),
            ))),
            attempt: 1,
            completion_token: String::from("generation-1"),
            idempotency_key: String::from("effect-key"),
            labels: std::collections::HashMap::new(),
        };

        let mut missing_token = valid.clone();
        missing_token.completion_token.clear();
        let token_error = ActivityTask::try_from(missing_token)
            .err()
            .ok_or("empty completion token must be rejected")?;
        assert!(
            token_error.to_string().contains("completion_token"),
            "refusal must name the missing generation proof: {token_error}"
        );

        let mut missing_key = valid.clone();
        missing_key.idempotency_key.clear();
        let key_error = ActivityTask::try_from(missing_key)
            .err()
            .ok_or("empty idempotency key must be rejected")?;
        assert!(
            key_error.to_string().contains("idempotency_key"),
            "refusal must name the missing external-effect key: {key_error}"
        );

        let mut missing_run = valid;
        missing_run.run_id = None;
        let run_error = ActivityTask::try_from(missing_run)
            .err()
            .ok_or("missing run id must be rejected")?;
        assert!(
            run_error.to_string().contains("run_id"),
            "refusal must name the missing concrete run: {run_error}"
        );
        Ok(())
    }
}
