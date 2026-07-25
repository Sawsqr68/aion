//! Targeted worker drain and stop administration.

use aion_proto::WireError;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Serialize;

use super::auth::HttpCaller;
use super::error::HttpWireError;
use crate::ServerState;
use crate::namespace::CallerIdentity;
use crate::worker::WorkerId;

/// Accepted worker lifecycle operation.
#[derive(Debug, Serialize)]
pub(crate) struct WorkerAdminResponse {
    worker_id: u64,
    state: &'static str,
}

/// Fence one worker from assignment and ask its transport to drain gracefully.
pub(crate) async fn drain_worker(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Path(worker_id): Path<u64>,
) -> Result<(StatusCode, Json<WorkerAdminResponse>), HttpWireError> {
    deploy_gate(&caller)?;
    let worker_id = WorkerId::from_value(worker_id);
    require_worker(&state, worker_id)?;
    let signalled = state
        .worker_registry()
        .drain_worker(worker_id)
        .map_err(|error| server_error(&error))?;
    let state_name = if signalled {
        "draining"
    } else {
        "deregistered"
    };
    Ok((
        StatusCode::ACCEPTED,
        Json(WorkerAdminResponse {
            worker_id: worker_id.value(),
            state: state_name,
        }),
    ))
}

/// Drain one worker, then force worker-loss teardown at the configured deadline.
pub(crate) async fn stop_worker(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Path(worker_id): Path<u64>,
) -> Result<(StatusCode, Json<WorkerAdminResponse>), HttpWireError> {
    deploy_gate(&caller)?;
    let worker_id = WorkerId::from_value(worker_id);
    require_worker(&state, worker_id)?;
    let signalled = state
        .worker_registry()
        .drain_worker(worker_id)
        .map_err(|error| server_error(&error))?;
    if signalled {
        let deadline = state.runtime_config().drain_timeout;
        let registry = state.worker_registry().clone();
        let tracker = state.heartbeat_tracker().clone();
        let pending = state.pending_activities().clone();
        tokio::spawn(async move {
            tokio::time::sleep(deadline).await;
            match registry.is_registered(worker_id) {
                Ok(false) => {}
                Ok(true) => {
                    match tracker.fail_disconnected_worker(worker_id, &registry, &pending) {
                        Ok(report) => tracing::warn!(
                            worker_id = worker_id.value(),
                            failed_tasks = report.tasks.len(),
                            deadline_ms = deadline.as_millis(),
                            "targeted worker stop deadline expired; force-deregistered worker"
                        ),
                        Err(error) => tracing::error!(
                            worker_id = worker_id.value(),
                            %error,
                            "targeted worker stop failed to force-deregister worker"
                        ),
                    }
                }
                Err(error) => tracing::error!(
                    worker_id = worker_id.value(),
                    %error,
                    "targeted worker stop could not inspect worker at deadline"
                ),
            }
        });
    }
    Ok((
        StatusCode::ACCEPTED,
        Json(WorkerAdminResponse {
            worker_id: worker_id.value(),
            state: if signalled {
                "stopping"
            } else {
                "deregistered"
            },
        }),
    ))
}

fn require_worker(state: &ServerState, worker_id: WorkerId) -> Result<(), HttpWireError> {
    let registered = state
        .worker_registry()
        .is_registered(worker_id)
        .map_err(|error| server_error(&error))?;
    if registered {
        Ok(())
    } else {
        Err(HttpWireError(WireError::not_found(format!(
            "worker {} is not registered",
            worker_id.value()
        ))))
    }
}

fn deploy_gate(caller: &CallerIdentity) -> Result<(), HttpWireError> {
    if caller.deploy_granted() {
        Ok(())
    } else {
        Err(HttpWireError(WireError::deploy_denied(
            "worker administration requires the deployment-wide deploy grant",
        )))
    }
}

fn server_error(error: &crate::ServerError) -> HttpWireError {
    HttpWireError(error.to_wire_error())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    use super::super::router::workflow_router;
    use super::super::test_support::{runtime_config, server_state, shared_engine};
    use crate::{
        NamespaceResolver, StaticScheduleNamespaces, StaticWorkflowNamespaces,
        config::NamespaceMode,
    };

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[tokio::test]
    async fn drain_route_fences_one_worker_and_signals_it() -> TestResult {
        let state = admin_state().await?;
        let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
        let activity_types = [String::from("admin-drain")];
        let registration =
            state
                .worker_registry()
                .register("default", activity_types.iter(), sender)?;
        let worker_id = registration
            .worker_id()
            .ok_or_else(|| std::io::Error::other("worker id missing"))?;
        let router = workflow_router(state.clone());

        let response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/workers/{}/drain", worker_id.value()))
                    .body(Body::empty())?,
            )
            .await?;

        assert_eq!(response.status(), axum::http::StatusCode::ACCEPTED);
        assert!(
            state
                .worker_registry()
                .select_worker("default", "default", "admin-drain", None)?
                .is_none()
        );
        assert_eq!(
            receiver
                .recv()
                .await
                .ok_or_else(|| std::io::Error::other("drain signal missing"))?,
            crate::worker::registry::WorkerMessage::DrainRequest
        );
        Ok(())
    }

    #[tokio::test]
    async fn stop_route_is_typed_not_found_for_absent_worker() -> TestResult {
        let state = admin_state().await?;
        let response = workflow_router(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/workers/999/stop")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
        Ok(())
    }

    async fn admin_state() -> Result<crate::ServerState, Box<dyn std::error::Error>> {
        let (engine, store, visibility) = shared_engine().await?;
        std::hint::black_box((store, visibility));
        let resolver = NamespaceResolver::from_parts(
            NamespaceMode::SharedEngine,
            Some(engine),
            Arc::new(StaticWorkflowNamespaces::default()),
            Arc::new(StaticScheduleNamespaces::default()),
        );
        let mut runtime = runtime_config();
        runtime.auth.enabled = false;
        server_state(resolver, runtime).await
    }
}
