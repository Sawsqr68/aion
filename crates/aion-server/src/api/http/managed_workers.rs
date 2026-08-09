//! The managed-worker surface: one status join, three lifecycle commands.
//!
//! `GET /workers/managed` answers the operator's actual question — "what should
//! be running, what IS running, and what happened to the rest" — from the
//! supervisor's join of the durable deployment records with live instance
//! state. It is always mounted: reporting the fleet is a read, not a
//! deploy-surface mutation.
//!
//! Beside it sit the lifecycle mutations `POST
//! /workers/managed/{name}/start`, `/stop`, and `/restart`. Their ACTUAL
//! exposure, stated plainly: the router mounts them only when
//! `[deploy].enabled` is set — the same switch that mounts their
//! `DeployService` gRPC counterparts and the `/worker-deployments/*` family —
//! so with deploy off they are a plain 404 and no transport can mutate the
//! fleet. Within a mounted route, [`deploy_gate`] refuses callers without the
//! deploy grant; with authentication disabled the caller IS the single-tenant
//! operator, who holds that grant server-side, so the mount switch — not the
//! gate — is what keeps a non-deploy-target server's fleet immutable.
//!
//! Each mutation performs the same
//! [`crate::worker::supervisor::WorkerSupervisor`] call as its gRPC
//! counterpart, and the failure mapping is shared with that surface
//! ([`crate::api::handlers::managed_workers`]) so the two cannot drift.

use axum::{
    Json,
    extract::{Path, State},
};
use serde::Serialize;

use super::auth::HttpCaller;
use super::error::HttpWireError;
use crate::ServerState;
use crate::api::handlers::managed_workers::wire_error;
use crate::namespace::CallerIdentity;
use crate::worker::supervisor::{ManagedWorkerReport, ManagedWorkerStatus};

/// One managed worker after a lifecycle command, mirroring the gRPC
/// `ManagedWorkerResponse` shape: the acted-on worker under a `worker` key.
///
/// The inner status serializes exactly like the entries of
/// `GET /workers/managed`, so a console reads one worker shape everywhere.
#[derive(Debug, Serialize)]
pub(crate) struct ManagedWorkerActionResponse {
    /// The worker's durable intent joined with its live state after the action.
    pub(crate) worker: ManagedWorkerStatus,
}

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

/// `POST /workers/managed/{name}/start` — start (or adopt) supervision of one
/// deployment, recording the intent durably first.
///
/// Mirrors `DeployService.StartManagedWorker`: an uncommissioned server
/// refuses with the `[worker_supervision]` remedy, an unknown name is
/// not-found, and success reports the worker as it now stands.
pub(crate) async fn start_managed_worker(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Path(name): Path<String>,
) -> Result<Json<ManagedWorkerActionResponse>, HttpWireError> {
    deploy_gate(&caller)?;
    state
        .worker_supervisor()
        .start(&name)
        .await
        .map(|worker| Json(ManagedWorkerActionResponse { worker }))
        .map_err(|error| HttpWireError(wire_error(&error)))
}

/// `POST /workers/managed/{name}/stop` — stop one deployment and record the
/// intent durably.
///
/// Mirrors `DeployService.StopManagedWorker`: it needs no commission (stopping
/// is always available, exactly as on gRPC), an unknown name is not-found, and
/// success is reported only once the process group has been proven empty.
pub(crate) async fn stop_managed_worker(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Path(name): Path<String>,
) -> Result<Json<ManagedWorkerActionResponse>, HttpWireError> {
    deploy_gate(&caller)?;
    state
        .worker_supervisor()
        .stop(&name)
        .await
        .map(|worker| Json(ManagedWorkerActionResponse { worker }))
        .map_err(|error| HttpWireError(wire_error(&error)))
}

/// `POST /workers/managed/{name}/restart` — stop and start one deployment,
/// leaving desired state at `Running`.
///
/// Mirrors `DeployService.RestartManagedWorker`: the same commission
/// requirement and failure set as start plus stop.
pub(crate) async fn restart_managed_worker(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Path(name): Path<String>,
) -> Result<Json<ManagedWorkerActionResponse>, HttpWireError> {
    deploy_gate(&caller)?;
    state
        .worker_supervisor()
        .restart(&name)
        .await
        .map(|worker| Json(ManagedWorkerActionResponse { worker }))
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
    use std::collections::BTreeSet;
    use std::num::NonZeroU32;
    use std::path::PathBuf;
    use std::time::Duration;

    use aion_store::{
        DeployedBinaryIdentity, DesiredState, NewWorkerDeployment, WorkerArtifactRef,
        WorkerDeployment,
    };
    use axum::{
        body::Body,
        extract::{Path, State},
        http::Request,
    };
    use tower::ServiceExt;

    use super::super::auth::HttpCaller;
    use super::super::router::workflow_router;
    use super::super::test_support::{runtime_config, server_state, shared_engine};
    use super::{restart_managed_worker, start_managed_worker, stop_managed_worker};
    use crate::worker::supervisor::{ManagedExecutable, SupervisionPolicy};
    use crate::{
        CallerIdentity, NamespaceResolver, StaticScheduleNamespaces, StaticWorkflowNamespaces,
        config::NamespaceMode,
    };

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// The status route exists on a stock server — deploy surface OFF, since
    /// the read is not deploy-gated by mounting — and answers honestly when
    /// nothing is commissioned: an empty fleet AND the remedy, not a bare
    /// empty list that reads as "all is well".
    #[tokio::test]
    async fn the_status_route_reports_an_uncommissioned_server_with_its_remedy() -> TestResult {
        let state = admin_state(false).await?;
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

    /// 🔴 THE MOUNT SWITCH. With `[deploy] enabled = false` every lifecycle
    /// verb is a plain 404 — no durable write, no spawn — exactly as the gRPC
    /// `DeployService` and the `/worker-deployments/*` sibling go dark, while
    /// the status read keeps answering. The commissioned supervisor and the
    /// durable record make this discriminating: everything else about the
    /// server is ready to act, and only the switch refuses.
    #[tokio::test]
    async fn the_lifecycle_routes_are_dark_when_deploy_is_disabled() -> TestResult {
        let state = admin_state(false).await?;
        assert!(
            state
                .worker_supervisor()
                .commission(policy()?, ManagedExecutable::Path(PathBuf::from("/bin/sh")))
        );
        state
            .worker_deployment_store()
            .put_worker_deployment(deployment("dark", DesiredState::Stopped)?)
            .await
            .map(drop)?;
        let router = workflow_router(state.clone());

        for verb in ["start", "stop", "restart"] {
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/workers/managed/dark/{verb}"))
                        .body(Body::empty())?,
                )
                .await?;
            assert_eq!(
                response.status(),
                axum::http::StatusCode::NOT_FOUND,
                "{verb} answered with the deploy surface disabled"
            );
        }

        let record = state
            .worker_deployment_store()
            .get_worker_deployment("dark")
            .await?
            .ok_or("the durable record must still exist")?;
        assert_eq!(
            record.desired,
            DesiredState::Stopped,
            "a dark route still changed durable intent"
        );
        let report = state.worker_supervisor().report().await?;
        let dark = report
            .workers
            .iter()
            .find(|worker| worker.name == "dark")
            .ok_or("the status read must keep answering with deploy off")?;
        assert!(dark.pid.is_none(), "a dark route still spawned a process");
        Ok(())
    }

    /// The full lifecycle over the HTTP mutations against a real supervised
    /// process: start reports a running worker, restart keeps it running, and
    /// stop is reported only with the durable intent flipped to `stopped`.
    /// Every response carries the SAME worker shape the status join lists.
    ///
    /// Cleanup is UNCONDITIONAL: the three requests are collected without
    /// early return, then the supervisor is asked to stop the deployment
    /// before any assertion can fail — so a red run cannot leak the sleeping
    /// child for its remaining minutes.
    #[tokio::test]
    async fn the_lifecycle_routes_start_restart_and_stop_a_managed_worker() -> TestResult {
        let state = admin_state(true).await?;
        assert!(
            state
                .worker_supervisor()
                .commission(policy()?, ManagedExecutable::Path(PathBuf::from("/bin/sh")))
        );
        state
            .worker_deployment_store()
            .put_worker_deployment(deployment("lifecycle", DesiredState::Stopped)?)
            .await
            .map(drop)?;
        let router = workflow_router(state.clone());

        // Collect every outcome first — no `?`, no assert — so the guard stop
        // below runs whatever happened.
        let started = post_json(&router, "/workers/managed/lifecycle/start").await;
        let restarted = post_json(&router, "/workers/managed/lifecycle/restart").await;
        let stopped = post_json(&router, "/workers/managed/lifecycle/stop").await;
        // The guard: idempotent over an already-stopped worker, and the last
        // thing that can leave a child behind. Checked (not dropped) after the
        // request outcomes are unwrapped, so a genuine stop failure still
        // fails the test.
        let guard = state.worker_supervisor().stop("lifecycle").await;
        let (started, restarted, stopped) = (started?, restarted?, stopped?);
        drop(guard?);

        assert_eq!(started.0, axum::http::StatusCode::OK, "{}", started.1);
        assert_eq!(started.1["worker"]["name"], "lifecycle");
        assert_eq!(started.1["worker"]["desired"], "running");
        assert_eq!(started.1["worker"]["task_queue"], "shell");

        assert_eq!(restarted.0, axum::http::StatusCode::OK, "{}", restarted.1);
        assert_eq!(restarted.1["worker"]["desired"], "running");

        assert_eq!(stopped.0, axum::http::StatusCode::OK, "{}", stopped.1);
        assert_eq!(stopped.1["worker"]["desired"], "stopped");
        assert_eq!(stopped.1["worker"]["state"], "stopped");

        let record = state
            .worker_deployment_store()
            .get_worker_deployment("lifecycle")
            .await?
            .ok_or("the durable record must survive the lifecycle")?;
        assert_eq!(
            record.desired,
            DesiredState::Stopped,
            "the stop must have recorded the operator intent durably"
        );
        Ok(())
    }

    /// A name with no durable deployment record is not-found on every verb —
    /// the same `UnknownDeployment` refusal the gRPC counterparts map.
    #[tokio::test]
    async fn an_unknown_deployment_name_is_not_found_on_every_verb() -> TestResult {
        let state = admin_state(true).await?;
        assert!(
            state
                .worker_supervisor()
                .commission(policy()?, ManagedExecutable::Path(PathBuf::from("/bin/sh")))
        );
        let router = workflow_router(state);
        for verb in ["start", "stop", "restart"] {
            let (status, body) =
                post_json(&router, &format!("/workers/managed/absent/{verb}")).await?;
            assert_eq!(
                status,
                axum::http::StatusCode::NOT_FOUND,
                "{verb} answered {status} for an unknown deployment"
            );
            assert_eq!(body["code"], "not_found", "{verb}: {body}");
        }
        Ok(())
    }

    /// An uncommissioned server refuses start and restart with the
    /// `[worker_supervision]` remedy — while stop still succeeds, exactly as
    /// on gRPC: stopping needs no restart discipline, and refusing it would
    /// strand an operator who wants a fleet DOWN on an unconfigured server.
    #[tokio::test]
    async fn an_uncommissioned_server_refuses_start_and_restart_with_the_remedy() -> TestResult {
        let state = admin_state(true).await?;
        state
            .worker_deployment_store()
            .put_worker_deployment(deployment("unsupervised", DesiredState::Running)?)
            .await
            .map(drop)?;
        let router = workflow_router(state.clone());

        for verb in ["start", "restart"] {
            let (status, body) =
                post_json(&router, &format!("/workers/managed/unsupervised/{verb}")).await?;
            assert_eq!(
                status,
                axum::http::StatusCode::CONFLICT,
                "{verb} must refuse on an uncommissioned server: {body}"
            );
            assert_eq!(body["code"], "invalid_state", "{verb}: {body}");
            let message = body["message"].as_str().ok_or("refusal had no message")?;
            assert!(
                message.contains("[worker_supervision]"),
                "{verb}: {message}"
            );
        }

        let (status, body) = post_json(&router, "/workers/managed/unsupervised/stop").await?;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");
        assert_eq!(body["worker"]["desired"], "stopped");
        let record = state
            .worker_deployment_store()
            .get_worker_deployment("unsupervised")
            .await?
            .ok_or("the durable record must survive the stop")?;
        assert_eq!(record.desired, DesiredState::Stopped);
        Ok(())
    }

    /// Every verb is denied BEFORE the supervisor is consulted for a caller
    /// without the deploy grant: the durable intent is untouched, nothing is
    /// spawned, and the refusal is the deploy denial — the same gate the gRPC
    /// counterparts and the status GET apply.
    #[tokio::test]
    async fn a_caller_without_the_deploy_grant_is_denied_before_any_effect() -> TestResult {
        let state = admin_state(true).await?;
        assert!(
            state
                .worker_supervisor()
                .commission(policy()?, ManagedExecutable::Path(PathBuf::from("/bin/sh")))
        );
        state
            .worker_deployment_store()
            .put_worker_deployment(deployment("guarded", DesiredState::Stopped)?)
            .await
            .map(drop)?;
        let denied = CallerIdentity::operator("reader").with_deploy(false);

        let start = start_managed_worker(
            State(state.clone()),
            HttpCaller(denied.clone()),
            Path("guarded".to_owned()),
        )
        .await;
        let stop = stop_managed_worker(
            State(state.clone()),
            HttpCaller(denied.clone()),
            Path("guarded".to_owned()),
        )
        .await;
        let restart = restart_managed_worker(
            State(state.clone()),
            HttpCaller(denied),
            Path("guarded".to_owned()),
        )
        .await;
        for (verb, result) in [("start", start), ("stop", stop), ("restart", restart)] {
            let refusal = result
                .err()
                .ok_or_else(|| format!("{verb} served an ungranted caller"))?;
            assert_eq!(
                refusal.0.code,
                aion_proto::WireErrorCode::DeployDenied,
                "{verb}: {}",
                refusal.0.message
            );
        }

        let record = state
            .worker_deployment_store()
            .get_worker_deployment("guarded")
            .await?
            .ok_or("the durable record must still exist")?;
        assert_eq!(
            record.desired,
            DesiredState::Stopped,
            "a denied verb still changed durable intent"
        );
        let report = state.worker_supervisor().report().await?;
        let guarded = report
            .workers
            .iter()
            .find(|worker| worker.name == "guarded")
            .ok_or("the deployment must be reported")?;
        assert!(
            guarded.pid.is_none(),
            "a denied start still spawned a process"
        );
        Ok(())
    }

    /// 🔴 THE GATE AT THE TRANSPORT, both arms. With authentication ON, a
    /// caller withholding the deploy grant gets `403` with the
    /// `deploy_denied` wire code on every verb — and the granted arm is the
    /// control: the same credential WITH the grant passes the gate and lands
    /// on the verb's own downstream refusal, so a surface that refused
    /// everyone could not pass. That downstream refusal differs by verb,
    /// which the pin states exactly: start and restart check commission FIRST
    /// (`409` with the remedy on this uncommissioned state), while stop needs
    /// no commission and reaches the unknown-name `404`. The grant travels by
    /// whichever credential path is compiled: the JWT `deploy` claim under
    /// `feature = "auth"`, the development `x-aion-deploy` header otherwise.
    #[tokio::test]
    async fn a_deploy_denied_lifecycle_call_is_a_403_at_the_transport() -> TestResult {
        let state = auth_on_state().await?;
        let router = workflow_router(state);

        for (verb, past_the_gate) in [
            ("start", axum::http::StatusCode::CONFLICT),
            ("stop", axum::http::StatusCode::NOT_FOUND),
            ("restart", axum::http::StatusCode::CONFLICT),
        ] {
            let uri = format!("/workers/managed/absent/{verb}");
            let denied = router
                .clone()
                .oneshot(lifecycle_request(&uri, false)?)
                .await?;
            assert_eq!(
                denied.status(),
                axum::http::StatusCode::FORBIDDEN,
                "{verb} served a caller with no deploy grant"
            );
            let bytes = axum::body::to_bytes(denied.into_body(), 64 * 1024).await?;
            let body: serde_json::Value = serde_json::from_slice(&bytes)?;
            assert_eq!(body["code"], "deploy_denied", "{verb}: {body}");

            let granted = router
                .clone()
                .oneshot(lifecycle_request(&uri, true)?)
                .await?;
            assert_eq!(
                granted.status(),
                past_the_gate,
                "{verb} must pass the gate for a granted caller and land on \
                 its own downstream refusal"
            );
        }
        Ok(())
    }

    /// A POST lifecycle request carrying (or withholding) the deploy grant
    /// through whichever credential path is compiled.
    fn lifecycle_request(
        uri: &str,
        granted: bool,
    ) -> Result<Request<Body>, Box<dyn std::error::Error>> {
        #[cfg(feature = "auth")]
        let token = if granted {
            crate::auth::test_support::mint_token_with_deploy("alice", "tenant-a", true)?
        } else {
            crate::auth::test_support::mint_token("alice", "tenant-a")?
        };
        #[cfg(not(feature = "auth"))]
        let token = super::super::test_support::TOKEN.to_owned();

        let mut builder = Request::builder()
            .method("POST")
            .uri(uri)
            .header("authorization", format!("Bearer {token}"))
            .header("x-aion-subject", "alice")
            .header("x-aion-namespaces", "tenant-a");
        #[cfg(not(feature = "auth"))]
        if granted {
            builder = builder.header("x-aion-deploy", "true");
        }
        Ok(builder.body(Body::empty())?)
    }

    /// POST the empty-bodied lifecycle request the console sends and decode
    /// the JSON answer, success or refusal alike.
    async fn post_json(
        router: &axum::Router,
        uri: &str,
    ) -> Result<(axum::http::StatusCode, serde_json::Value), Box<dyn std::error::Error>> {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .body(Body::empty())?,
            )
            .await?;
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024).await?;
        Ok((status, serde_json::from_slice(&bytes)?))
    }

    fn policy() -> Result<SupervisionPolicy, &'static str> {
        Ok(SupervisionPolicy {
            restart_backoff_initial: Duration::from_millis(20),
            restart_backoff_max: Duration::from_millis(20),
            restart_backoff_multiplier: NonZeroU32::new(1).ok_or("multiplier")?,
            restart_window: Duration::from_secs(600),
            max_restarts_per_window: NonZeroU32::new(5).ok_or("budget")?,
            stop_grace: Duration::from_secs(2),
        })
    }

    /// A durable builtin deployment whose spawn is a long `/bin/sh` sleep —
    /// the same seam the drain e2e supervises, so the test binary is never
    /// re-executed as a "worker".
    fn deployment(
        name: &str,
        desired: DesiredState,
    ) -> Result<WorkerDeployment, Box<dyn std::error::Error>> {
        Ok(WorkerDeployment::new(
            NewWorkerDeployment {
                name: name.to_owned(),
                artifact: WorkerArtifactRef::Builtin {
                    verb: vec!["-c".to_owned(), "sleep 300".to_owned()],
                },
                binary: DeployedBinaryIdentity {
                    version: "test".to_owned(),
                    commit: "test".to_owned(),
                    dirty: "false".to_owned(),
                    content_hash: "deploy-time-hash".to_owned(),
                },
                namespaces: BTreeSet::from(["default".to_owned()]),
                task_queue: "shell".to_owned(),
                node: None,
                desired,
            },
            chrono::Utc::now(),
        )?)
    }

    /// Auth-off operator state, with the deploy surface switched per test:
    /// the lifecycle routes exist only when it is on.
    async fn admin_state(
        deploy_enabled: bool,
    ) -> Result<crate::ServerState, Box<dyn std::error::Error>> {
        let mut runtime = runtime_config();
        runtime.auth.enabled = false;
        runtime.deploy.enabled = deploy_enabled;
        engine_state(runtime).await
    }

    /// Deploy-enabled state with authentication ON, for the transport-level
    /// grant pin.
    async fn auth_on_state() -> Result<crate::ServerState, Box<dyn std::error::Error>> {
        let mut runtime = runtime_config();
        runtime.deploy.enabled = true;
        engine_state(runtime).await
    }

    async fn engine_state(
        runtime: crate::config::RuntimeConfig,
    ) -> Result<crate::ServerState, Box<dyn std::error::Error>> {
        let (engine, store, visibility) = shared_engine().await?;
        std::hint::black_box((store, visibility));
        let resolver = NamespaceResolver::from_parts(
            NamespaceMode::SharedEngine,
            Some(engine),
            std::sync::Arc::new(StaticWorkflowNamespaces::default()),
            std::sync::Arc::new(StaticScheduleNamespaces::default()),
        );
        server_state(resolver, runtime).await
    }
}
