use std::sync::Arc;

use aion_core::{ClusterEvent, DeploymentAssociation, DesiredState, PutOutcome};
use aion_proto::{ProtoRegisterWorker, ProtoWorkerInstanceIdentity};
use aion_store::{InMemoryStore, NamespaceStore, WorkerDeploymentListing, WorkerDeploymentStore};
use axum::{
    Json, body,
    extract::{Path, State},
    http::Request,
};
use futures::StreamExt;
use sha2::{Digest, Sha256};
use tower::ServiceExt;

use super::{
    PutWorkerDeploymentRequest, PutWorkerDeploymentResponse, capture_binary_identity,
    put_worker_deployment,
};
use crate::api::http::auth::HttpCaller;
use crate::api::http::router::workflow_router;
use crate::api::http::test_support::runtime_config;
use crate::build_identity::BuildIdentity;
use crate::config::NamespaceMode;
use crate::{
    CallerIdentity, NamespaceResolver, ServerState, StaticScheduleNamespaces,
    StaticWorkflowNamespaces,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;
type EventStream = futures::stream::BoxStream<
    'static,
    Result<ClusterEvent, crate::cluster_publisher::ClusterStreamLagged>,
>;

#[test]
fn capture_uses_build_identity_and_running_executable_sha256()
-> Result<(), Box<dyn std::error::Error>> {
    let captured = capture_binary_identity()?;
    let build = BuildIdentity::current();
    let path = std::env::current_exe()?;
    let expected_hash = super::lowercase_hex(&Sha256::digest(std::fs::read(path)?));
    assert_eq!(captured.version, build.version);
    assert_eq!(captured.commit, build.commit);
    assert_eq!(captured.dirty, build.dirty);
    assert_eq!(captured.content_hash, expected_hash);
    assert_ne!(captured.content_hash, "unknown");
    Ok(())
}

#[test]
fn lowercase_hex_matches_sha256_empty_known_answer() {
    let digest = Sha256::digest([]);
    assert_eq!(
        super::lowercase_hex(&digest),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[tokio::test]
async fn deployment_put_is_denied_before_any_durable_write() -> TestResult {
    let store = Arc::new(InMemoryStore::default());
    let state = state_with_stores(store.clone(), store.clone());
    let denied = CallerIdentity::operator("reader").with_deploy(false);
    let request: PutWorkerDeploymentRequest = serde_json::from_value(deployment_body())?;

    let result = put_worker_deployment(
        State(state),
        HttpCaller(denied),
        Path("forbidden".to_owned()),
        Json(request),
    )
    .await;

    assert!(result.is_err());
    assert!(store.get_worker_deployment("forbidden").await?.is_none());
    Ok(())
}

#[tokio::test]
async fn deployment_routes_follow_the_deploy_surface_switch() -> TestResult {
    let disabled_store = Arc::new(InMemoryStore::default());
    let disabled_state =
        state_with_stores_and_deploy(disabled_store.clone(), disabled_store.clone(), false);
    let disabled = workflow_router(disabled_state)
        .oneshot(json_request(
            "PUT",
            "/worker-deployments/disabled",
            &deployment_body(),
        )?)
        .await?;
    assert_eq!(disabled.status(), axum::http::StatusCode::NOT_FOUND);
    assert!(
        disabled_store
            .get_worker_deployment("disabled")
            .await?
            .is_none()
    );

    let enabled_store = Arc::new(InMemoryStore::default());
    let enabled_state = state_with_stores_and_deploy(enabled_store.clone(), enabled_store, true);
    let enabled = workflow_router(enabled_state)
        .oneshot(json_request(
            "GET",
            "/worker-deployments",
            &serde_json::Value::Null,
        )?)
        .await?;
    assert!(enabled.status().is_success());
    let listing: WorkerDeploymentListing = read_json(enabled).await?;
    assert!(listing.deployments.is_empty());
    assert!(listing.undecodable.is_empty());
    Ok(())
}

#[tokio::test]
async fn poisoned_row_does_not_break_cluster_snapshot() -> TestResult {
    let store = Arc::new(InMemoryStore::default());
    let state = state_with_stores(store.clone(), store.clone());
    store.write_raw_worker_deployment("poisoned", b"not-json".to_vec())?;
    let snapshot =
        crate::stream::cluster_stream::build_snapshot(&state, &CallerIdentity::operator("test"))
            .await?;
    assert!(snapshot.deployments.is_empty());
    Ok(())
}

#[tokio::test]
async fn http_put_survives_a_second_server_state_over_the_same_store() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("worker-deployments.db");
    let first_store =
        Arc::new(aion_store_haematite::HaematiteStore::open_or_create(path.clone(), haematite::NodeCacheBudget::Unlimited).await?);
    let first_state = state_with_leaf(first_store.clone());
    let first_router = workflow_router(first_state.clone());

    let response = first_router
        .clone()
        .oneshot(json_request(
            "PUT",
            "/worker-deployments/shells",
            &deployment_body(),
        )?)
        .await?;
    assert!(response.status().is_success());
    let put: PutWorkerDeploymentResponse = read_json(response).await?;
    assert_eq!(put.outcome, PutOutcome::Created);
    assert!(put.deployment.last_spawn_binary.is_none());
    assert_eq!(put.deployment.binary.content_hash.len(), 64);

    drop(first_router);
    drop(first_state);
    drop(first_store);

    let second_store = Arc::new(aion_store_haematite::HaematiteStore::open_or_create(path, haematite::NodeCacheBudget::Unlimited).await?);
    let second_state = state_with_leaf(second_store);
    let response = workflow_router(second_state)
        .oneshot(json_request(
            "GET",
            "/worker-deployments/shells",
            &serde_json::Value::Null,
        )?)
        .await?;
    assert!(response.status().is_success());
    let reopened: aion_store::WorkerDeployment = read_json(response).await?;
    assert_eq!(reopened, put.deployment);
    Ok(())
}

#[tokio::test]
async fn put_and_desired_events_arrive_and_snapshot_lists_existing_record() -> TestResult {
    let store = Arc::new(InMemoryStore::default());
    let state = state_with_stores(store.clone(), store);
    let router = workflow_router(state.clone());
    let mut events = state.cluster_publisher().subscribe(0);

    let put_response = router
        .clone()
        .oneshot(json_request(
            "PUT",
            "/worker-deployments/shells",
            &deployment_body(),
        )?)
        .await?;
    assert!(put_response.status().is_success());
    let put_event = next_event(&mut events).await?;
    assert!(matches!(
        put_event,
        ClusterEvent::WorkerDeploymentPut {
            ref name,
            outcome,
            desired_state,
            ..
        } if name == "shells" && outcome == PutOutcome::Created
            && desired_state == DesiredState::Running
    ));

    let desired_response = router
        .clone()
        .oneshot(json_request(
            "POST",
            "/worker-deployments/shells/desired-state",
            &serde_json::json!({ "desired": "stopped" }),
        )?)
        .await?;
    assert!(desired_response.status().is_success());
    let desired_event = next_event(&mut events).await?;
    assert!(matches!(
        desired_event,
        ClusterEvent::WorkerDeploymentDesiredStateChanged {
            ref name,
            desired_state,
            ..
        } if name == "shells" && desired_state == DesiredState::Stopped
    ));

    assert_registration_associations_and_snapshot(&state, &mut events).await?;
    for _ in 0..3 {
        assert!(matches!(
            next_event(&mut events).await?,
            ClusterEvent::WorkerDisconnected { .. }
        ));
    }

    let deleted = router
        .oneshot(json_request(
            "DELETE",
            "/worker-deployments/shells",
            &serde_json::Value::Null,
        )?)
        .await?;
    assert!(deleted.status().is_success());
    let delete_event = next_event(&mut events).await?;
    assert!(matches!(
        delete_event,
        ClusterEvent::WorkerDeploymentDeleted { ref name, .. } if name == "shells"
    ));
    let snapshot =
        crate::stream::cluster_stream::build_snapshot(&state, &CallerIdentity::operator("test"))
            .await?;
    assert!(snapshot.deployments.is_empty());
    Ok(())
}

async fn assert_registration_associations_and_snapshot(
    state: &ServerState,
    events: &mut EventStream,
) -> TestResult {
    let (worker_tx, worker_rx) = tokio::sync::mpsc::channel(1);
    let associated_registration = state
        .worker_registry()
        .accept_registration(
            state.namespace_guard(),
            &CallerIdentity::operator("worker"),
            &worker_registration(Some(ProtoWorkerInstanceIdentity {
                deployment: "shells".to_owned(),
                instance_id: "instance-1".to_owned(),
            })),
            worker_tx,
        )
        .await?;
    let connected = next_event(events).await?;
    assert!(matches!(
        connected,
        ClusterEvent::WorkerConnected {
            deployment: Some(ref deployment),
            deployment_association: Some(DeploymentAssociation::Known),
            ..
        } if deployment == "shells"
    ));

    let (unknown_tx, unknown_rx) = tokio::sync::mpsc::channel(1);
    let unknown_registration = state
        .worker_registry()
        .accept_registration(
            state.namespace_guard(),
            &CallerIdentity::operator("worker"),
            &worker_registration(Some(ProtoWorkerInstanceIdentity {
                deployment: "missing".to_owned(),
                instance_id: "instance-2".to_owned(),
            })),
            unknown_tx,
        )
        .await?;
    let unknown_connected = next_event(events).await?;
    assert!(matches!(
        unknown_connected,
        ClusterEvent::WorkerConnected {
            deployment: Some(ref deployment),
            deployment_association: Some(DeploymentAssociation::Absent),
            ..
        } if deployment == "missing"
    ));

    let (plain_tx, plain_rx) = tokio::sync::mpsc::channel(1);
    let plain_registration = state
        .worker_registry()
        .accept_registration(
            state.namespace_guard(),
            &CallerIdentity::operator("worker"),
            &worker_registration(None),
            plain_tx,
        )
        .await?;
    let plain_connected = next_event(events).await?;
    assert!(matches!(
        plain_connected,
        ClusterEvent::WorkerConnected {
            deployment: None,
            deployment_association: None,
            ..
        }
    ));

    let snapshot =
        crate::stream::cluster_stream::build_snapshot(state, &CallerIdentity::operator("test"))
            .await?;
    assert_eq!(snapshot.deployments.len(), 1);
    assert_eq!(snapshot.deployments[0].name, "shells");
    assert_eq!(snapshot.deployments[0].desired_state, DesiredState::Stopped);
    assert!(snapshot.workers.iter().any(|worker| {
        worker.deployment.as_deref() == Some("shells")
            && worker.deployment_association == Some(DeploymentAssociation::Known)
    }));
    assert!(snapshot.workers.iter().any(|worker| {
        worker.deployment.as_deref() == Some("missing")
            && worker.deployment_association == Some(DeploymentAssociation::Absent)
    }));
    assert!(
        snapshot.workers.iter().any(|worker| {
            worker.deployment.is_none() && worker.deployment_association.is_none()
        })
    );
    drop((
        associated_registration,
        unknown_registration,
        plain_registration,
        worker_rx,
        unknown_rx,
        plain_rx,
    ));
    Ok(())
}

#[tokio::test]
async fn registry_without_deployment_store_reports_unchecked_and_preserves_null_pairing()
-> TestResult {
    let registry = crate::worker::ConnectedWorkerRegistry::default();
    let resolver = NamespaceResolver::authorization_only(
        NamespaceMode::SharedEngine,
        StaticWorkflowNamespaces::default(),
        StaticScheduleNamespaces::default(),
    );
    let mut runtime = runtime_config();
    runtime.auth.enabled = false;
    runtime.deploy.enabled = true;
    let state = ServerState::from_parts_with_registry(resolver, runtime, registry);
    let (unchecked_tx, unchecked_rx) = tokio::sync::mpsc::channel(1);
    let unchecked = state
        .worker_registry()
        .accept_registration(
            state.namespace_guard(),
            &CallerIdentity::operator("worker"),
            &worker_registration(Some(ProtoWorkerInstanceIdentity {
                deployment: "unverified".to_owned(),
                instance_id: "instance-u".to_owned(),
            })),
            unchecked_tx,
        )
        .await?;
    let (plain_tx, plain_rx) = tokio::sync::mpsc::channel(1);
    let plain = state
        .worker_registry()
        .accept_registration(
            state.namespace_guard(),
            &CallerIdentity::operator("worker"),
            &worker_registration(None),
            plain_tx,
        )
        .await?;
    let snapshot =
        crate::stream::cluster_stream::build_snapshot(&state, &CallerIdentity::operator("test"))
            .await?;
    assert!(snapshot.workers.iter().any(|worker| {
        worker.deployment.as_deref() == Some("unverified")
            && worker.deployment_association == Some(DeploymentAssociation::Unchecked)
    }));
    assert!(
        snapshot.workers.iter().any(|worker| {
            worker.deployment.is_none() && worker.deployment_association.is_none()
        })
    );
    drop((unchecked, plain, unchecked_rx, plain_rx));
    Ok(())
}

fn state_with_stores<S, W>(namespace_store: Arc<S>, worker_store: Arc<W>) -> ServerState
where
    S: NamespaceStore,
    W: WorkerDeploymentStore,
{
    state_with_stores_and_deploy(namespace_store, worker_store, true)
}

fn state_with_stores_and_deploy<S, W>(
    namespace_store: Arc<S>,
    worker_store: Arc<W>,
    deploy_enabled: bool,
) -> ServerState
where
    S: NamespaceStore,
    W: WorkerDeploymentStore,
{
    let resolver = NamespaceResolver::authorization_only(
        NamespaceMode::SharedEngine,
        StaticWorkflowNamespaces::default(),
        StaticScheduleNamespaces::default(),
    );
    let mut runtime = runtime_config();
    runtime.auth.enabled = false;
    runtime.deploy.enabled = deploy_enabled;
    let namespace_store: Arc<dyn NamespaceStore> = namespace_store;
    let worker_store: Arc<dyn WorkerDeploymentStore> = worker_store;
    ServerState::from_parts_with_control_stores(resolver, runtime, namespace_store, worker_store)
}

fn state_with_leaf<S>(store: Arc<S>) -> ServerState
where
    S: NamespaceStore + WorkerDeploymentStore,
{
    let resolver = NamespaceResolver::authorization_only(
        NamespaceMode::SharedEngine,
        StaticWorkflowNamespaces::default(),
        StaticScheduleNamespaces::default(),
    );
    let mut runtime = runtime_config();
    runtime.auth.enabled = false;
    runtime.deploy.enabled = true;
    ServerState::from_parts_with_namespace_store(resolver, runtime, store)
}

fn deployment_body() -> serde_json::Value {
    serde_json::json!({
        "artifact": { "type": "builtin", "verb": ["worker", "shell"] },
        "namespaces": ["orders"],
        "task_queue": "shell",
        "node": "node-a",
        "desired": "running"
    })
}

fn worker_registration(instance: Option<ProtoWorkerInstanceIdentity>) -> ProtoRegisterWorker {
    ProtoRegisterWorker {
        namespaces: vec!["orders".to_owned()],
        activity_types: vec!["shell".to_owned()],
        task_queue: "shell".to_owned(),
        node: "node-a".to_owned(),
        activities: Vec::new(),
        identity: "build-a".to_owned(),
        instance,
    }
}

fn json_request(
    method: &str,
    path: &str,
    value: &serde_json::Value,
) -> Result<Request<body::Body>, Box<dyn std::error::Error>> {
    let body = if method == "GET" {
        body::Body::empty()
    } else {
        body::Body::from(serde_json::to_vec(&value)?)
    };
    Ok(Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(body)?)
}

async fn read_json<T>(response: axum::response::Response) -> Result<T, Box<dyn std::error::Error>>
where
    T: serde::de::DeserializeOwned,
{
    let bytes = body::to_bytes(response.into_body(), usize::MAX).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

async fn next_event(events: &mut EventStream) -> Result<ClusterEvent, Box<dyn std::error::Error>> {
    match events.next().await {
        Some(Ok(event)) => Ok(event),
        Some(Err(error)) => Err(format!("cluster stream lagged: {error:?}").into()),
        None => Err("cluster stream closed before the expected event".into()),
    }
}
