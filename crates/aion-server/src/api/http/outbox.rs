//! Dead-letter inspection endpoint — the discovery half of outbox redrive.
//!
//! `POST /outbox/dead-letters` lists the dead-lettered rows of one workflow: the durable evidence
//! behind a workflow that is still `Running` while an activity never returned. Each row reports
//! `failure_delivered`, which is the fact an operator needs before acting:
//!
//! - `false` — the workflow was never told the activity failed. It is still waiting, judgment never
//!   passed, and `ClusterCommand::RedriveOutboxRow` will re-queue the work.
//! - `true` — the failure reached the workflow, which has already reacted, and that reaction is
//!   recorded history. Redrive REFUSES such a row: re-running it would re-execute a possibly
//!   non-idempotent activity behind a recorded judgment.
//!
//! Auth is the deployment-wide deploy grant — the same authority that may issue the redrive itself
//! (`/cluster/command`), so what an operator can see here is exactly what they can act on there.

use aion_core::WorkflowId;
use aion_proto::WireError;
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::auth::HttpCaller;
use super::error::HttpWireError;
use crate::ServerState;

/// Request body: the workflow whose dead letters are being inspected.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct DeadLettersRequest {
    /// The workflow to enumerate dead-lettered outbox rows for.
    pub workflow_id: String,
}

/// One dead-lettered outbox row, as an operator sees it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct DeadLetterRow {
    /// The row's durable idempotency key (`"{workflow_id}:{ordinal}"`).
    pub dispatch_key: String,
    /// The fan-out ordinal within the workflow.
    pub ordinal: u64,
    /// The activity type that never got through.
    pub activity_type: String,
    /// Routing identity the row was staged with.
    pub namespace: String,
    /// Pool/flavour selector the row was staged with.
    pub task_queue: String,
    /// Optional node affinity, or `null` for an unpinned row.
    pub node: Option<String>,
    /// Dispatch attempts spent before the row dead-lettered.
    pub attempt: u32,
    /// Whether the workflow was TOLD this activity failed. `true` means judgment has passed and
    /// redrive will refuse the row.
    pub failure_delivered: bool,
    /// Whether an ordinary (non-forced) redrive would accept this row.
    pub redrivable: bool,
}

/// Response body: the workflow's dead letters, ordered by ordinal.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct DeadLettersResponse {
    /// The dead-lettered rows.
    pub dead_letters: Vec<DeadLetterRow>,
}

/// `POST /outbox/dead-letters`.
pub(crate) async fn list_dead_letters(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Json(request): Json<DeadLettersRequest>,
) -> Result<Json<DeadLettersResponse>, HttpWireError> {
    // GATE FIRST, exactly like the cluster-command seam: the dead-letter table is a
    // deployment-wide operational view, not a per-namespace read.
    super::cluster_command::deploy_gate(&caller)?;

    let Some(outbox_store) = state.outbox_store() else {
        return Err(HttpWireError(WireError::invalid_state(
            "the durable outbox is not commissioned on this server, so it has no dead letters",
        )));
    };
    let workflow_id = Uuid::parse_str(&request.workflow_id)
        .map(WorkflowId::new)
        .map_err(|_error| HttpWireError(WireError::invalid_input("workflow_id must be a UUID")))?;

    let rows = crate::worker::list_dead_letters(outbox_store.as_ref(), &workflow_id)
        .await
        .map_err(|error| HttpWireError(crate::ServerError::from(error).to_wire_error()))?;

    Ok(Json(DeadLettersResponse {
        dead_letters: rows
            .into_iter()
            .map(|row| DeadLetterRow {
                dispatch_key: row.dispatch_key,
                ordinal: row.ordinal,
                activity_type: row.activity_type,
                namespace: row.namespace,
                task_queue: row.task_queue,
                node: row.node,
                attempt: row.attempt,
                failure_delivered: row.failure_delivered,
                redrivable: !row.failure_delivered,
            })
            .collect(),
    }))
}
