//! Deploy-granted HTTP management for durable worker deployments.

use std::collections::BTreeSet;

use aion_core::{ClusterEvent, PutOutcome};
use aion_proto::WireError;
use aion_store::{
    DeployedBinaryIdentity, DesiredState, NewWorkerDeployment, WorkerArtifactRef, WorkerDeployment,
    WorkerDeploymentDeleteOutcome, WorkerDeploymentListing,
};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::auth::HttpCaller;
use super::error::HttpWireError;
use crate::{ServerError, ServerState, build_identity::BuildIdentity};

/// Operator-authored fields for a builtin worker deployment.
#[derive(Debug, Deserialize)]
pub(crate) struct PutWorkerDeploymentRequest {
    artifact: WorkerArtifactRef,
    namespaces: BTreeSet<String>,
    task_queue: String,
    node: Option<String>,
    desired: DesiredState,
}

/// Desired-state mutation body.
#[derive(Debug, Deserialize)]
pub(crate) struct SetDesiredStateRequest {
    desired: DesiredState,
}

/// Create-or-replace response carrying both the outcome and durable record.
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct PutWorkerDeploymentResponse {
    outcome: PutOutcome,
    deployment: WorkerDeployment,
}

/// Typed deploy-time executable identity capture failures.
#[derive(Debug, thiserror::Error)]
pub(crate) enum BinaryIdentityCaptureError {
    /// The operating system could not identify this executable.
    #[error("could not resolve the running server executable: {source}")]
    CurrentExecutable {
        #[source]
        source: std::io::Error,
    },
    /// The executable bytes could not be read.
    #[error("could not read running server executable `{path}`: {source}")]
    ReadExecutable {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub(crate) async fn put_worker_deployment(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Path(name): Path<String>,
    Json(request): Json<PutWorkerDeploymentRequest>,
) -> Result<Json<PutWorkerDeploymentResponse>, HttpWireError> {
    require_deploy_grant(&caller)?;
    let binary = capture_binary_identity().map_err(|error| capture_refusal(&error))?;
    let requested = WorkerDeployment::new(
        NewWorkerDeployment {
            name,
            artifact: request.artifact,
            binary,
            namespaces: request.namespaces,
            task_queue: request.task_queue,
            node: request.node,
            desired: request.desired,
        },
        chrono::Utc::now(),
    )
    .map_err(|error| HttpWireError(WireError::invalid_input(error.to_string())))?;
    let result = state
        .worker_deployment_store()
        .put_worker_deployment(requested)
        .await
        .map_err(store_error)?;
    let outcome = result.outcome;
    let record = result.deployment;
    let event_name = record.name.clone();
    let desired_state = record.desired;
    let binary_version = record.binary.version.clone();
    let binary_content_hash = record.binary.content_hash.clone();
    drop(
        state
            .cluster_publisher()
            .emit(|meta| ClusterEvent::WorkerDeploymentPut {
                meta,
                name: event_name,
                outcome,
                desired_state,
                binary_version,
                binary_content_hash,
            }),
    );
    Ok(Json(PutWorkerDeploymentResponse {
        outcome,
        deployment: record,
    }))
}

pub(crate) async fn list_worker_deployments(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
) -> Result<Json<WorkerDeploymentListing>, HttpWireError> {
    require_deploy_grant(&caller)?;
    state
        .worker_deployment_store()
        .list_worker_deployments()
        .await
        .map(Json)
        .map_err(store_error)
}

pub(crate) async fn get_worker_deployment(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Path(name): Path<String>,
) -> Result<Json<WorkerDeployment>, HttpWireError> {
    require_deploy_grant(&caller)?;
    state
        .worker_deployment_store()
        .get_worker_deployment(&name)
        .await
        .map_err(store_error)?
        .map(Json)
        .ok_or_else(|| not_found(&name))
}

pub(crate) async fn set_worker_deployment_desired_state(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Path(name): Path<String>,
    Json(request): Json<SetDesiredStateRequest>,
) -> Result<Json<WorkerDeployment>, HttpWireError> {
    require_deploy_grant(&caller)?;
    let record = state
        .worker_deployment_store()
        .set_desired_state(&name, request.desired)
        .await
        .map_err(store_error)?
        .ok_or_else(|| not_found(&name))?;
    let event_name = record.name.clone();
    let desired_state = record.desired;
    drop(state.cluster_publisher().emit(|meta| {
        ClusterEvent::WorkerDeploymentDesiredStateChanged {
            meta,
            name: event_name,
            desired_state,
        }
    }));
    Ok(Json(record))
}

pub(crate) async fn delete_worker_deployment(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Path(name): Path<String>,
) -> Result<Json<WorkerDeploymentDeleteOutcome>, HttpWireError> {
    require_deploy_grant(&caller)?;
    let outcome = state
        .worker_deployment_store()
        .delete_worker_deployment(&name)
        .await
        .map_err(store_error)?;
    if !outcome.existed {
        return Err(not_found(&name));
    }
    let event_name = name;
    drop(
        state
            .cluster_publisher()
            .emit(|meta| ClusterEvent::WorkerDeploymentDeleted {
                meta,
                name: event_name,
            }),
    );
    Ok(Json(outcome))
}

fn capture_binary_identity() -> Result<DeployedBinaryIdentity, BinaryIdentityCaptureError> {
    let path = std::env::current_exe()
        .map_err(|source| BinaryIdentityCaptureError::CurrentExecutable { source })?;
    let bytes =
        std::fs::read(&path).map_err(|source| BinaryIdentityCaptureError::ReadExecutable {
            path: path.clone(),
            source,
        })?;
    let identity = BuildIdentity::current();
    let content_hash = lowercase_hex(&Sha256::digest(bytes));
    Ok(DeployedBinaryIdentity {
        version: identity.version.to_owned(),
        commit: identity.commit.to_owned(),
        dirty: identity.dirty.to_owned(),
        content_hash,
    })
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn require_deploy_grant(caller: &crate::CallerIdentity) -> Result<(), HttpWireError> {
    if caller.deploy_granted() {
        Ok(())
    } else {
        Err(HttpWireError(
            ServerError::namespace_denied(
                "worker-deployment management requires the deployment-wide deploy grant",
            )
            .to_wire_error(),
        ))
    }
}

fn capture_refusal(error: &BinaryIdentityCaptureError) -> HttpWireError {
    HttpWireError(WireError::backend(format!(
        "worker deployment refused because binary identity capture failed: {error}"
    )))
}

fn store_error(error: aion_store::StoreError) -> HttpWireError {
    HttpWireError(ServerError::from(error).to_wire_error())
}

fn not_found(name: &str) -> HttpWireError {
    HttpWireError(WireError::not_found(format!(
        "worker deployment `{name}` was not found"
    )))
}

#[cfg(test)]
#[path = "worker_deployments_tests.rs"]
mod tests;
