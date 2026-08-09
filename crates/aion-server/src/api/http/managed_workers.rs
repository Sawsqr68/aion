//! The managed-worker status surface: one route, one join, one truth.
//!
//! `GET /workers/managed` answers the operator's actual question — "what should
//! be running, what IS running, and what happened to the rest" — from the
//! supervisor's join of the durable deployment records with live instance
//! state. It is deliberately a READ: the lifecycle mutations
//! (start / stop / restart) are authorized operator commands and live on the
//! deploy service beside the deployment records they act on.
//!
//! The failure mapping is shared with that gRPC surface
//! ([`crate::api::handlers::managed_workers`]) so the two cannot drift.

use axum::{Json, extract::State};

use super::auth::HttpCaller;
use super::error::HttpWireError;
use crate::ServerState;
use crate::api::handlers::managed_workers::wire_error;
use crate::namespace::CallerIdentity;
use crate::worker::supervisor::ManagedWorkerReport;

/// Report every managed worker: desired state joined with actual state.
pub(crate) async fn list_managed_workers(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
) -> Result<Json<ManagedWorkerReport>, HttpWireError> {
    deploy_gate(&caller)?;
    state
        .worker_supervisor()
        .report()
        .await
        .map(Json)
        .map_err(|error| HttpWireError(wire_error(&error)))
}

fn deploy_gate(caller: &CallerIdentity) -> Result<(), HttpWireError> {
    if caller.deploy_granted() {
        Ok(())
    } else {
        Err(HttpWireError(aion_proto::WireError::deploy_denied(
            "managed-worker administration requires the deployment-wide deploy grant",
        )))
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    use super::super::router::workflow_router;
    use super::super::test_support::{runtime_config, server_state, shared_engine};
    use crate::{
        NamespaceResolver, StaticScheduleNamespaces, StaticWorkflowNamespaces,
        config::NamespaceMode,
    };

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// The route exists on a stock server and answers honestly when nothing is
    /// commissioned: an empty fleet AND the remedy, not a bare empty list that
    /// reads as "all is well".
    #[tokio::test]
    async fn the_status_route_reports_an_uncommissioned_server_with_its_remedy() -> TestResult {
        let state = admin_state().await?;
        let response = workflow_router(state)
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/workers/managed")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024).await?;
        let body: serde_json::Value = serde_json::from_slice(&bytes)?;
        assert_eq!(body["commissioned"], serde_json::Value::Bool(false));
        let remedy = body["remedy"].as_str().ok_or("a remedy must be present")?;
        assert!(remedy.contains("[worker_supervision]"), "{remedy}");
        assert_eq!(
            body["workers"].as_array().map(Vec::len),
            Some(0),
            "no deployment was created, so no worker may be reported"
        );
        Ok(())
    }

    async fn admin_state() -> Result<crate::ServerState, Box<dyn std::error::Error>> {
        let (engine, store, visibility) = shared_engine().await?;
        std::hint::black_box((store, visibility));
        let resolver = NamespaceResolver::from_parts(
            NamespaceMode::SharedEngine,
            Some(engine),
            std::sync::Arc::new(StaticWorkflowNamespaces::default()),
            std::sync::Arc::new(StaticScheduleNamespaces::default()),
        );
        let mut runtime = runtime_config();
        runtime.auth.enabled = false;
        server_state(resolver, runtime).await
    }
}
