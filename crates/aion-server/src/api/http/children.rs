//! One-level child workflow visibility derived from authoritative histories.
//!
//! This is intentionally a read projection, not a parent/child schema relation:
//! the parent run's `ChildWorkflowStarted` events are the complete discovery seam.

use aion_core::{
    ActivityId, Event, RunId, WorkflowId, display_name, run_segment, status_from_events,
};
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};

use super::auth::HttpCaller;
use super::error::HttpWireError;
use super::history::scoped_engine;
use crate::{ServerError, ServerState};

#[derive(Debug, Deserialize)]
pub(crate) struct ChildrenRequest {
    namespace: String,
    workflow_id: WorkflowId,
    run_id: RunId,
}

#[derive(Debug, Serialize)]
pub(crate) struct ChildrenResponse {
    children: Vec<ChildEntry>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ChildEntry {
    workflow_id: WorkflowId,
    run_id: Option<RunId>,
    workflow_type: String,
    display_name: Option<String>,
    status: String,
    current_activity_id: Option<ActivityId>,
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
    let parent_run = run_segment(&parent_history, &request.run_id);
    let mut children = Vec::new();

    for event in parent_run {
        let Event::ChildWorkflowStarted {
            child_workflow_id,
            workflow_type,
            ..
        } = event
        else {
            continue;
        };
        if children
            .iter()
            .any(|child: &ChildEntry| child.workflow_id == *child_workflow_id)
        {
            continue;
        }
        let child_history = engine
            .store()
            .read_history(child_workflow_id)
            .await
            .map_err(store_error)?;
        children.push(project_child(
            child_workflow_id.clone(),
            workflow_type.clone(),
            &child_history,
        ));
    }

    Ok(Json(ChildrenResponse { children }))
}

fn project_child(workflow_id: WorkflowId, workflow_type: String, history: &[Event]) -> ChildEntry {
    let run_id = history.iter().rev().find_map(|event| match event {
        Event::WorkflowStarted { run_id, .. } => Some(run_id.clone()),
        _ => None,
    });
    let run = run_id
        .as_ref()
        .map_or(&[][..], |run_id| run_segment(history, run_id));
    let (current_activity_id, current_attempt) = current_attempt(run).map_or(
        (None, None),
        |(activity_id, attempt)| (Some(activity_id), Some(attempt)),
    );
    let status = if run_id.is_some() {
        format!("{:?}", status_from_events(run))
    } else {
        "Pending".to_owned()
    };

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
            } if current
                .as_ref()
                .is_some_and(|(id, active_attempt)| id == activity_id && active_attempt == attempt) =>
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
    use aion_core::{EventEnvelope, PackageVersion, Payload};
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
}
