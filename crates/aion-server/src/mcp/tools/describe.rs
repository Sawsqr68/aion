//! `describe_run`: the join an agent makes first.

use std::collections::BTreeMap;

use aion_core::Event;
use aion_mcp::tools::service::{ToolCall, ToolFailure, ToolOutcome};
use serde_json::{Value, json};

use crate::worker::ActivityReachability;
use crate::{CallerIdentity, ServerState};

use super::errors::tool_failure;
use super::run_view::RunView;

/// One activity this run dispatched and that has recorded no terminal event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CurrentStep {
    /// The activity ordinal within the run.
    pub(crate) activity_id: u64,
    /// The activity type, from the run's own `ActivityScheduled`.
    pub(crate) activity_type: String,
    /// The attempt that is in flight.
    pub(crate) attempt: u32,
    /// The task queue the dispatch was recorded against.
    pub(crate) task_queue: Option<String>,
    /// The node affinity recorded on the dispatch, if any.
    pub(crate) node: Option<String>,
    /// When the ENGINE recorded the dispatch — not when a worker took it, which
    /// may be a thing that never happened. `unserved` is what distinguishes the
    /// two.
    pub(crate) dispatched_at: String,
}

/// Fold a run's own segment into its current step.
///
/// "Current" is the LAST activity the run dispatched that no later event
/// terminates. Terminals are matched on `(activity_id, attempt)`, not on
/// `activity_id` alone: a retry is a new attempt, and an earlier attempt's
/// failure must not be read as terminating the attempt now in flight.
pub(crate) fn current_step(segment: &[Event]) -> Option<CurrentStep> {
    let mut scheduled: BTreeMap<u64, (String, Option<String>, Option<String>)> = BTreeMap::new();
    let mut live: BTreeMap<(u64, u32), (usize, String)> = BTreeMap::new();
    for (position, event) in segment.iter().enumerate() {
        match event {
            Event::ActivityScheduled {
                activity_id,
                activity_type,
                task_queue,
                node,
                ..
            } => {
                scheduled.insert(
                    activity_id.sequence_position(),
                    (
                        activity_type.clone(),
                        Some(task_queue.clone()),
                        node.clone(),
                    ),
                );
            }
            Event::ActivityStarted {
                envelope,
                activity_id,
                attempt,
            } => {
                live.insert(
                    (activity_id.sequence_position(), *attempt),
                    (position, envelope.recorded_at.to_rfc3339()),
                );
            }
            Event::ActivityCompleted {
                activity_id,
                attempt,
                ..
            }
            | Event::ActivityFailed {
                activity_id,
                attempt,
                ..
            }
            | Event::ActivityCancelled {
                activity_id,
                attempt,
                ..
            } => {
                drop(live.remove(&(activity_id.sequence_position(), *attempt)));
            }
            _ => {}
        }
    }
    let ((activity_id, attempt), (_, dispatched_at)) = live
        .into_iter()
        .max_by_key(|(_, (position, _))| *position)?;
    let (activity_type, task_queue, node) = scheduled
        .get(&activity_id)
        .cloned()
        .unwrap_or_else(|| (String::from("<unrecorded>"), None, None));
    Some(CurrentStep {
        activity_id,
        activity_type,
        attempt,
        task_queue,
        node,
        dispatched_at,
    })
}

/// Every retained transcript stream of this run, each as a complete
/// `read_transcript` handle.
///
/// The enumeration is run-scoped by the storage key itself: the run sits inside
/// the `O`-region scan prefix, so a sibling generation's streams are excluded
/// by the key range, not by a droppable predicate here.
async fn transcript_handles(
    state: &ServerState,
    view: &RunView,
) -> Result<Vec<Value>, ToolFailure> {
    let summaries = state
        .transcript_publisher()
        .list_streams(&view.workflow_id, &view.run_id)
        .await
        .map_err(|error| tool_failure(&crate::ServerError::from(error).to_wire_error()))?;
    let mut handles = Vec::new();
    for summary in summaries {
        handles.push(json!({
            "workflow_id": view.workflow_id.to_string(),
            "run_id": view.run_id.to_string(),
            "activity_id": summary.key.activity_id.sequence_position(),
            "attempt": summary.key.attempt,
            "head_seq": summary.head,
        }));
    }
    Ok(handles)
}

/// Run `describe_run`.
///
/// # Errors
///
/// [`ToolFailure`] when the arguments are malformed, the caller does not hold
/// the namespace, or the workflow/run does not exist.
pub(crate) async fn describe_run(
    state: &ServerState,
    caller: &CallerIdentity,
    call: &ToolCall,
) -> Result<ToolOutcome, ToolFailure> {
    let view = RunView::read(state, caller, call).await?;
    let step = current_step(view.run_segment());
    let unserved = ActivityReachability {
        registry: state.worker_registry(),
        declarations: state.queue_declarations(),
        state: state.queue_service_state(),
    }
    .unserved(&view.namespace, &view.workflow_id, &view.history)
    .map_err(|error| tool_failure(&error.to_wire_error()))?;
    let transcripts = transcript_handles(state, &view).await?;
    let unserved = serde_json::to_value(&unserved).map_err(|error| {
        ToolFailure::new(
            format!("the unserved-activity verdict could not be encoded: {error}"),
            json!({ "code": "backend" }),
        )
    })?;

    let summary_line = format!(
        "{} run {} is {:?}{}",
        view.summary.workflow_type,
        view.run_id,
        view.summary.status,
        step.as_ref().map_or_else(String::new, |step| format!(
            "; current step is activity {} ({}) attempt {}",
            step.activity_id, step.activity_type, step.attempt
        ))
    );

    Ok(ToolOutcome {
        structured: json!({
            "namespace": view.namespace,
            "workflow_id": view.workflow_id.to_string(),
            "run_id": view.run_id.to_string(),
            "workflow_type": view.summary.workflow_type,
            // #211: the operator-facing label, so an agent that SET a display
            // name can read it back here rather than only through `list_runs`.
            // Per WORKFLOW, not per run — the recorded attribute carries no run
            // id and the projection folds the whole history — which is why it
            // sits beside the workflow's own fields and not inside
            // `current_step`.
            "display_name": view.summary.display_name,
            "status": view.summary.status,
            "started_at": view.summary.started_at.to_rfc3339(),
            "ended_at": view.summary.ended_at.map(|at| at.to_rfc3339()),
            "failed_step": view.summary.failed_step,
            "failure_reason": view.summary.failure_reason,
            "history_head_seq": view.history_head_seq(),
            "current_step": step.map(|step| json!({
                "activity_id": step.activity_id,
                "activity_type": step.activity_type,
                "attempt": step.attempt,
                "task_queue": step.task_queue,
                "node": step.node,
                "dispatched_at": step.dispatched_at,
            })),
            "unserved": unserved,
            "transcripts": transcripts,
        }),
        summary: summary_line,
    })
}

#[cfg(test)]
mod tests {
    use aion_core::{
        ActivityError, ActivityErrorKind, ActivityId, ContentType, Event, EventEnvelope, Payload,
        WorkflowId,
    };
    use chrono::{DateTime, Utc};
    use uuid::Uuid;

    use super::{CurrentStep, current_step};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn step_of(segment: &[Event]) -> Result<CurrentStep, Box<dyn std::error::Error>> {
        current_step(segment).ok_or_else(|| "expected a current step".into())
    }

    fn envelope(seq: u64) -> EventEnvelope {
        EventEnvelope {
            seq,
            recorded_at: DateTime::from_timestamp(
                1_700_000_000 + i64::try_from(seq).unwrap_or(0),
                0,
            )
            .unwrap_or_else(Utc::now),
            workflow_id: WorkflowId::new(Uuid::from_u128(1)),
        }
    }

    fn scheduled(seq: u64, ordinal: u64, activity_type: &str) -> Event {
        Event::ActivityScheduled {
            envelope: envelope(seq),
            activity_id: ActivityId::from_sequence_position(ordinal),
            activity_type: activity_type.to_owned(),
            input: Payload::new(ContentType::Json, b"{}".to_vec()),
            task_queue: "default".to_owned(),
            node: None,
        }
    }

    fn started(seq: u64, ordinal: u64, attempt: u32) -> Event {
        Event::ActivityStarted {
            envelope: envelope(seq),
            activity_id: ActivityId::from_sequence_position(ordinal),
            attempt,
        }
    }

    fn failed(seq: u64, ordinal: u64, attempt: u32) -> Event {
        Event::ActivityFailed {
            envelope: envelope(seq),
            activity_id: ActivityId::from_sequence_position(ordinal),
            error: ActivityError {
                kind: ActivityErrorKind::Retryable,
                message: "transient".to_owned(),
                details: None,
            },
            attempt,
        }
    }

    fn completed(seq: u64, ordinal: u64, attempt: u32) -> Event {
        Event::ActivityCompleted {
            envelope: envelope(seq),
            activity_id: ActivityId::from_sequence_position(ordinal),
            result: Payload::new(ContentType::Json, b"{}".to_vec()),
            attempt,
        }
    }

    #[test]
    fn a_run_with_nothing_in_flight_has_no_current_step() {
        let segment = vec![
            scheduled(1, 0, "fetch"),
            started(2, 0, 1),
            completed(3, 0, 1),
        ];
        assert_eq!(current_step(&segment), None);
    }

    #[test]
    fn the_current_step_is_the_latest_unterminated_dispatch() -> TestResult {
        let segment = vec![
            scheduled(1, 0, "fetch"),
            started(2, 0, 1),
            completed(3, 0, 1),
            scheduled(4, 1, "review"),
            started(5, 1, 1),
        ];
        let step = step_of(&segment)?;
        assert_eq!(step.activity_id, 1);
        assert_eq!(step.activity_type, "review");
        assert_eq!(step.attempt, 1);
        assert_eq!(step.task_queue.as_deref(), Some("default"));
        Ok(())
    }

    /// A retry's earlier failure terminates ATTEMPT ONE. Matching terminals on
    /// the ordinal alone would erase the live attempt two and report nothing in
    /// flight — the exact silence an operator would read as "it is done".
    #[test]
    fn an_earlier_attempts_failure_does_not_terminate_the_live_attempt() -> TestResult {
        let segment = vec![
            scheduled(1, 0, "review"),
            started(2, 0, 1),
            failed(3, 0, 1),
            started(4, 0, 2),
        ];
        let step = step_of(&segment)?;
        assert_eq!(step.attempt, 2);
        assert_eq!(step.activity_id, 0);
        Ok(())
    }

    #[test]
    fn a_dispatch_with_no_schedule_record_is_reported_rather_than_dropped() -> TestResult {
        let step = step_of(&[started(2, 5, 1)])?;
        assert_eq!(step.activity_id, 5);
        assert_eq!(step.activity_type, "<unrecorded>");
        assert_eq!(step.task_queue, None);
        Ok(())
    }
}
