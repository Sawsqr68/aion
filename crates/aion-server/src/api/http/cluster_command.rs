//! `/cluster/command` HTTP facade for the ADR-020 cluster command seam (WS3).
//!
//! Two commands do real work: the read-only [`ClusterCommand::RequestClusterSnapshot`] (the ops
//! console's calm-state baseline; also obtainable as the WS priming reply) and
//! [`ClusterCommand::RedriveOutboxRow`], which returns one dead-lettered outbox row to the pending
//! claim path after refusing every row it must not resurrect (a terminal workflow's row, a row that
//! is not a dead letter, or a dead letter whose failure the workflow already judged). The remaining
//! mutating variants compile so the contract exists, but their handlers run the full deploy-auth
//! gate FIRST and then return an `unimplemented` wire error — so the seam's authorization contract
//! is exercised and tested now, and an `unimplemented` stub is never an auth-bypass-shaped hole.
//!
//! Auth: `HttpCaller` (header-based; a browser CAN set headers on a POST, so no
//! query-param promotion is needed). The gate is the deployment-wide deploy
//! grant — cluster commands are deployment-scoped, never namespace-scoped.

use aion_core::{ClusterCommand, WorkflowId};
use aion_proto::WireError;
use aion_store::RedriveMode;
use axum::{
    Json,
    extract::State,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::auth::HttpCaller;
use super::error::HttpWireError;
use crate::ServerState;
use crate::namespace::CallerIdentity;
use crate::worker::{RedriveRefused, redrive_dead_lettered_row};

/// The ack an operator gets back from a successful [`ClusterCommand::RedriveOutboxRow`].
///
/// Reports the post-state the redrive produced, so the operator sees the work really is queued
/// again rather than having to trust an empty `200`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RedriveOutboxRowResponse {
    /// The row's durable idempotency key (`"{workflow_id}:{ordinal}"`).
    pub dispatch_key: String,
    /// The workflow that owns the redriven activity.
    pub workflow_id: String,
    /// The fan-out ordinal that was redriven.
    pub ordinal: u64,
    /// The activity type that will be dispatched again.
    pub activity_type: String,
    /// The row's lifecycle state after the redrive (always `pending`).
    pub status: String,
    /// The reset attempt count the next dispatch starts from.
    pub attempt: u32,
}

/// Handle a cluster command. Deploy-gated first, then dispatched.
pub(crate) async fn cluster_command(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Json(command): Json<ClusterCommand>,
) -> Result<Response, HttpWireError> {
    // GATE FIRST for EVERY variant — including the aspirational ones — so the
    // auth contract is exercised before any branch returns, and an unimplemented
    // handler can never be an auth bypass.
    deploy_gate(&caller)?;

    match command {
        ClusterCommand::RequestClusterSnapshot {} => {
            let snapshot = crate::stream::cluster_stream::build_snapshot(&state, &caller)
                .await
                .map_err(|error| HttpWireError(error.to_wire_error()))?;
            Ok(Json(snapshot).into_response())
        }
        // Redrive a dead-lettered outbox row (rider 2). Deploy-gated above like every other
        // cluster command; the redrive itself refuses anything it must not resurrect.
        ClusterCommand::RedriveOutboxRow {
            namespace,
            workflow_id,
            ordinal,
        } => {
            let response = redrive_outbox_row(&state, &namespace, &workflow_id, ordinal).await?;
            Ok(Json(response).into_response())
        }
        // Aspirational ADR-020 mutating commands: the gate already passed above,
        // so reaching here proves the deploy grant was checked; the handler then
        // declines with a typed unimplemented error and zero side effects.
        ClusterCommand::CancelWorkflow { .. }
        | ClusterCommand::ReopenWorkflow { .. }
        | ClusterCommand::DrainNode { .. }
        | ClusterCommand::PlannedHandoff { .. }
        | ClusterCommand::ChaosKillNode { .. } => Err(HttpWireError(WireError::backend_with_type(
            "Unimplemented",
            "this cluster command is part of the ADR-020 seam but is not implemented in Phase 1",
        ))),
    }
}

/// Return one dead-lettered outbox row to the pending claim path.
///
/// Runs in [`RedriveMode::Eligible`] — the only safe default. A row whose failure was already
/// delivered to (and judged by) its workflow is REFUSED here, because redriving it would re-execute
/// a possibly non-idempotent activity behind recorded history. The forced override exists in
/// [`redrive_dead_lettered_row`] but is deliberately not reachable from this seam: forcing a
/// re-execution behind a recorded judgment is not a one-field-on-a-wire-command decision.
///
/// `namespace` is carried by the ADR-020 command for routing/audit context; authorization for
/// cluster commands is the deployment-wide deploy grant applied by the caller, exactly as it is for
/// every other variant of this seam.
async fn redrive_outbox_row(
    state: &ServerState,
    namespace: &str,
    workflow_id: &str,
    ordinal: u64,
) -> Result<RedriveOutboxRowResponse, HttpWireError> {
    let Some(outbox_store) = state.outbox_store() else {
        return Err(HttpWireError(WireError::invalid_state(
            "the durable outbox is not commissioned on this server, so there are no dead-lettered \
             rows to redrive",
        )));
    };
    let workflow_id = Uuid::parse_str(workflow_id)
        .map(WorkflowId::new)
        .map_err(|_error| HttpWireError(WireError::invalid_input("workflow_id must be a UUID")))?;
    let engine = state
        .namespace_guard()
        .resolver()
        .engine()
        .map_err(|error| HttpWireError(error.to_wire_error()))?;

    let row = redrive_dead_lettered_row(
        engine.store().as_ref(),
        outbox_store.as_ref(),
        &workflow_id,
        ordinal,
        RedriveMode::Eligible,
    )
    .await
    .map_err(|refusal| redrive_wire_error(namespace, &refusal))?;

    Ok(RedriveOutboxRowResponse {
        dispatch_key: row.dispatch_key,
        workflow_id: row.workflow_id.to_string(),
        ordinal: row.ordinal,
        activity_type: row.activity_type,
        status: row.status.as_str().to_owned(),
        attempt: row.attempt,
    })
}

/// Map a typed redrive refusal onto the wire, keeping the reason intact.
///
/// Each refusal keeps its own shape so an operator can tell "there is no such row" from "this row
/// must not be resurrected" — never a flattened generic failure.
fn redrive_wire_error(namespace: &str, refusal: &RedriveRefused) -> HttpWireError {
    let detail = format!("outbox redrive refused in namespace '{namespace}': {refusal}");
    HttpWireError(match refusal {
        RedriveRefused::Row(aion_store::RedriveRefusal::NoSuchRow { .. }) => {
            WireError::not_found(detail)
        }
        RedriveRefused::Row(
            aion_store::RedriveRefusal::NotDeadLettered { .. }
            | aion_store::RedriveRefusal::AlreadyJudged { .. },
        )
        | RedriveRefused::WorkflowTerminal { .. }
        | RedriveRefused::HistoryRecordsOutcome { .. } => WireError::invalid_state(detail),
        RedriveRefused::Store(error) => crate::ServerError::from(error.clone()).to_wire_error(),
    })
}

/// Require the deployment-wide deploy grant. Denial is a `deploy_denied` wire
/// error (403), the same shape the deploy API uses.
pub(super) fn deploy_gate(caller: &CallerIdentity) -> Result<(), HttpWireError> {
    if caller.deploy_granted() {
        Ok(())
    } else {
        Err(HttpWireError(WireError::deploy_denied(
            "cluster commands require the deployment-wide deploy grant",
        )))
    }
}
