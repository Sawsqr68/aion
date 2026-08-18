//! One-level child workflow visibility derived from authoritative histories.
//!
//! This is intentionally a read projection, not a parent/child schema relation:
//! the parent run's `ChildWorkflowStarted` events are the complete discovery seam.
//!
//! # Authorization decision
//!
//! The parent history discloses a child id and workflow type, but child-owned
//! display names and progress are projected only after applying the same
//! per-workflow describe gate as sibling history routes. A child that resolves
//! to the guard's anti-leak `not_found`/`namespace_denied` result is OMITTED
//! from the listing. This deliberately makes unauthorized and nonexistent
//! children indistinguishable and never emits a partly disclosed row. Because
//! children inherit the parent namespace, `not_found` can also mean attribution
//! has not caught up with the durable parent event; that indistinguishable lag
//! case is logged internally before omission.
//!
//! # Read-shape decision
//!
//! This projection deliberately performs whole-history reads. The store has no
//! filtered lifecycle/current-attempt projection, and adding one would cross
//! #107's no-parent/child-schema wall or create a second source of truth. Child
//! histories are read only for ids derived from the selected parent run and
//! only after their per-workflow gate. K9-scale cost therefore grows with real
//! direct fanout and recorded history size; correctness and one authoritative
//! event projection are chosen over an arbitrary cap that would lie about
//! status or activity. Child gates and reads run concurrently while the final
//! response preserves parent-history discovery order. The console re-reads the
//! projection as fanout changes and while any child is pending, so cumulative
//! page traffic can be quadratic in fanout; a future bounded read requires a
//! store-level filtered/batched history contract or incremental projection,
//! not a limit invented at this HTTP route.

use std::collections::HashSet;

use aion_core::{
    ActivityId, Event, RunId, WorkflowId, WorkflowStatus, display_name, run_segment,
    status_from_events,
};
use aion_proto::{WireError, WireErrorCode};
use axum::{Json, extract::State};
use futures::future::join_all;
use serde::{Deserialize, Serialize};

use super::auth::HttpCaller;
use super::error::HttpWireError;
use super::history::scoped_engine;
use crate::{CallerIdentity, ServerError, ServerState};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChildrenRequest {
    /// Namespace that scopes both parent and child authorization.
    namespace: String,
    /// Parent workflow whose recorded run discovers direct children.
    workflow_id: WorkflowId,
    /// Parent generation; omitted selects its latest recorded run.
    #[serde(default)]
    run_id: Option<RunId>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ChildrenResponse {
    /// Authorized direct children in parent-history discovery order.
    children: Vec<ChildEntry>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ChildEntry {
    /// Stable child workflow identity.
    workflow_id: WorkflowId,
    /// Latest recorded child generation, or `null` before its run starts.
    run_id: Option<RunId>,
    /// Workflow type recorded by the parent's `ChildWorkflowStarted` event.
    workflow_type: String,
    /// Child-owned operator display name, when one is recorded.
    display_name: Option<String>,
    /// Typed child run status, or `null` when the child run has not started.
    status: Option<WorkflowStatus>,
    /// Currently executing activity, when one is recorded.
    current_activity_id: Option<ActivityId>,
    /// Attempt number paired with `current_activity_id`.
    current_attempt: Option<u32>,
}

/// `POST /workflows/children`: derive the addressed parent run's direct children.
pub(crate) async fn list_children(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Json(request): Json<ChildrenRequest>,
) -> Result<Json<ChildrenResponse>, HttpWireError> {
    let workflow_id_wire = request.workflow_id.to_string();
    let engine = scoped_engine(
        &state,
        &caller,
        &request.namespace,
        &workflow_id_wire,
        &request.workflow_id,
    )
    .await?;
    let parent_history = engine
        .store()
        .read_history(&request.workflow_id)
        .await
        .map_err(store_error)?;
    let selected_run = request.run_id.or_else(|| {
        parent_history.iter().rev().find_map(|event| match event {
            Event::WorkflowStarted { run_id, .. } => Some(run_id.clone()),
            _ => None,
        })
    });
    if let Some(run_id) = selected_run.as_ref()
        && !parent_history.iter().any(
            |event| matches!(event, Event::WorkflowStarted { run_id: recorded, .. } if recorded == run_id),
        )
    {
        return Err(HttpWireError(WireError::not_found(format!(
            "workflow {} run {} was not found",
            request.workflow_id, run_id
        ))));
    }
    let parent_run = selected_run
        .as_ref()
        .map_or(&[][..], |run| run_segment(&parent_history, run));
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();
    for event in parent_run {
        let Event::ChildWorkflowStarted {
            child_workflow_id,
            workflow_type,
            ..
        } = event
        else {
            continue;
        };
        if seen.insert(child_workflow_id.clone()) {
            candidates.push((child_workflow_id.clone(), workflow_type.clone()));
        }
    }

    let children = project_candidates(
        &state,
        &caller,
        &request.namespace,
        &request.workflow_id,
        candidates,
    )
    .await?;
    Ok(Json(ChildrenResponse { children }))
}

async fn project_candidates(
    state: &ServerState,
    caller: &CallerIdentity,
    namespace: &str,
    parent_workflow_id: &WorkflowId,
    candidates: Vec<(WorkflowId, String)>,
) -> Result<Vec<ChildEntry>, HttpWireError> {
    let projections = join_all(
        candidates
            .into_iter()
            .map(|(child_workflow_id, workflow_type)| {
                let state = state.clone();
                let caller = caller.clone();
                let namespace = namespace.to_owned();
                let parent_workflow_id = parent_workflow_id.clone();
                async move {
                    let child_wire = child_workflow_id.to_string();
                    let child_engine = match scoped_engine(
                        &state,
                        &caller,
                        &namespace,
                        &child_wire,
                        &child_workflow_id,
                    )
                    .await
                    {
                        Ok(child_engine) => child_engine,
                        Err(error) if error.0.code == WireErrorCode::NotFound => {
                            tracing::warn!(
                                parent_workflow_id = %parent_workflow_id,
                                child_workflow_id = %child_workflow_id,
                                "omitting child whose namespace attribution is not yet resolvable"
                            );
                            return Ok(None);
                        }
                        Err(error) if is_omitted_child_error(&error) => return Ok(None),
                        Err(error) => return Err(error),
                    };
                    let child_history = child_engine
                        .store()
                        .read_history(&child_workflow_id)
                        .await
                        .map_err(store_error)?;
                    Ok::<_, HttpWireError>(Some(project_child(
                        child_workflow_id,
                        workflow_type,
                        &child_history,
                    )))
                }
            }),
    )
    .await;

    projections
        .into_iter()
        .try_fold(Vec::new(), |mut children, projection| {
            if let Some(child) = projection? {
                children.push(child);
            }
            Ok(children)
        })
}

fn is_omitted_child_error(error: &HttpWireError) -> bool {
    error.0.code == WireErrorCode::NamespaceDenied
}

fn project_child(workflow_id: WorkflowId, workflow_type: String, history: &[Event]) -> ChildEntry {
    let run_id = history.iter().rev().find_map(|event| match event {
        Event::WorkflowStarted { run_id, .. } => Some(run_id.clone()),
        _ => None,
    });
    let run = run_id
        .as_ref()
        .map_or(&[][..], |run_id| run_segment(history, run_id));
    let (current_activity_id, current_attempt) = current_attempt(run)
        .map_or((None, None), |(activity_id, attempt)| {
            (Some(activity_id), Some(attempt))
        });
    let status = run_id.as_ref().map(|_| status_from_events(run));

    ChildEntry {
        workflow_id,
        run_id,
        workflow_type,
        display_name: display_name(history),
        status,
        current_activity_id,
        current_attempt,
    }
}

fn current_attempt(events: &[Event]) -> Option<(ActivityId, u32)> {
    let mut current = None;
    for event in events {
        match event {
            Event::ActivityStarted {
                activity_id,
                attempt,
                ..
            } => current = Some((activity_id.clone(), *attempt)),
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
            } if current.as_ref().is_some_and(|(id, active_attempt)| {
                id == activity_id && active_attempt == attempt
            }) =>
            {
                current = None;
            }
            _ => {}
        }
    }
    current
}

fn store_error(error: aion_store::StoreError) -> HttpWireError {
    HttpWireError(ServerError::from(error).to_wire_error())
}

#[cfg(test)]
mod tests {
    use aion_core::{ActivityError, ActivityErrorKind, EventEnvelope, PackageVersion, Payload};
    use chrono::Utc;
    use serde_json::json;

    use super::*;

    #[test]
    fn completed_attempt_is_not_projected_as_current() -> Result<(), aion_core::PayloadError> {
        let workflow_id = WorkflowId::new(uuid::Uuid::from_u128(1));
        let run_id = RunId::new(uuid::Uuid::from_u128(2));
        let activity_id = ActivityId::from_sequence_position(3);
        let envelope = |seq| EventEnvelope {
            seq,
            recorded_at: Utc::now(),
            workflow_id: workflow_id.clone(),
        };
        let history = vec![
            Event::WorkflowStarted {
                envelope: envelope(1),
                workflow_type: "leg".to_owned(),
                input: Payload::from_json(&json!(null))?,
                run_id,
                parent_run_id: None,
                parent_workflow_id: None,
                package_version: PackageVersion::new("a".repeat(64)),
            },
            Event::ActivityStarted {
                envelope: envelope(2),
                activity_id: activity_id.clone(),
                attempt: 1,
            },
            Event::ActivityCompleted {
                envelope: envelope(3),
                activity_id,
                result: Payload::from_json(&json!(null))?,
                attempt: 1,
            },
        ];
        assert_eq!(current_attempt(&history), None);
        Ok(())
    }

    #[test]
    fn only_namespace_denial_is_silently_omitted_after_not_found_is_logged() {
        let denied = HttpWireError(WireError::new(WireErrorCode::NamespaceDenied, "denied"));
        let backend = HttpWireError(WireError::backend("store unavailable"));
        assert!(is_omitted_child_error(&denied));
        assert!(!is_omitted_child_error(&backend));
    }

    #[test]
    fn failed_and_cancelled_attempts_are_not_projected_as_current() {
        let activity_id = ActivityId::from_sequence_position(3);
        let started = Event::ActivityStarted {
            envelope: test_envelope(1),
            activity_id: activity_id.clone(),
            attempt: 2,
        };
        let failed = Event::ActivityFailed {
            envelope: test_envelope(2),
            activity_id: activity_id.clone(),
            error: ActivityError {
                kind: ActivityErrorKind::Terminal,
                message: "boom".to_owned(),
                details: None,
            },
            attempt: 2,
        };
        assert_eq!(current_attempt(&[started.clone(), failed]), None);

        let cancelled = Event::ActivityCancelled {
            envelope: test_envelope(3),
            activity_id,
            attempt: 2,
        };
        assert_eq!(current_attempt(&[started, cancelled]), None);
    }

    #[test]
    fn mismatched_terminal_attempt_does_not_clear_the_current_attempt()
    -> Result<(), aion_core::PayloadError> {
        let activity_id = ActivityId::from_sequence_position(3);
        let history = vec![
            Event::ActivityStarted {
                envelope: test_envelope(1),
                activity_id: activity_id.clone(),
                attempt: 2,
            },
            Event::ActivityCompleted {
                envelope: test_envelope(2),
                activity_id: activity_id.clone(),
                result: Payload::from_json(&json!(null))?,
                attempt: 1,
            },
        ];
        assert_eq!(current_attempt(&history), Some((activity_id, 2)));
        Ok(())
    }

    fn test_envelope(seq: u64) -> EventEnvelope {
        EventEnvelope {
            seq,
            recorded_at: Utc::now(),
            workflow_id: WorkflowId::new(uuid::Uuid::from_u128(1)),
        }
    }
}
