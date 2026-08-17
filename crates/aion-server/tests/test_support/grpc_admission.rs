//! Shared harness for the gRPC worker-registration admission tests.
//!
//! Two test binaries drive a REAL `stream_worker` registration against a REAL
//! deployed AWL contract: `worker_contract_registration` asserts on the
//! `Status` the worker receives, and `grpc_admission_refusal_logs` asserts on
//! what the SERVER said about it. They must build the same server state from
//! the same fixture, or the two halves of one refusal would be measured against
//! two different worlds.
//!
//! The naming half needs its own binary because the tonic server runs inside a
//! `tokio::spawn`: only a GLOBALLY installed subscriber sees its events, and a
//! global subscriber is process-wide, so it cannot share a binary with tests
//! that must not have one.
//!
//! # Scope note, stated rather than quietly widened
//!
//! `runtime_config` is duplicated across **19** test files in this crate. That
//! is a real drift surface and it is NOT this lane's truth to fix; extracting
//! all 19 would be scope creep on a defect about admission logging. This module
//! deduplicates only the two files on this lane's path, so the lane adds no new
//! copy. The other 17 are left, named here, untouched.

use std::net::SocketAddr;
use std::time::Duration;

use aion_awl_package::compile_and_assemble_awl;
use aion_package::{ExtractionLimits, Package};
use aion_proto::generated::worker_protocol_client::WorkerProtocolClient;
use aion_proto::generated::{self, worker_to_server};
use aion_server::ServerState;
use aion_server::api::worker_grpc::worker_service;
use aion_server::config::{
    AuthConfig, AuthoringConfig, DeployConfig, ListenConfig, MetricsConfig, NamespaceConfig,
    NamespaceMode, OpsConsoleAssetSource, OpsConsoleConfig, RuntimeConfig, WebSocketConfig,
    WorkerConfig,
};
use tokio::net::TcpListener;
use tokio_stream::wrappers::{ReceiverStream, TcpListenerStream};

/// Test result alias shared by both admission binaries.
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// The fixture: queue `typed_queue` declaring one action, `fetch(url: String)`.
pub const SOURCE: &str = "//! Registration contract fixture.\nworkflow contract_registration\n  input url: String\n  outcome done: type Result, route success\n\ntype Result { ok: Bool }\n\nworker typed_queue\n  action fetch(url: String) -> Result\n\nstep call\n  fetch(url: url) -> result\n  route done(ok: result.ok)\n";

/// The runtime configuration both admission binaries serve under.
pub fn runtime_config() -> RuntimeConfig {
    RuntimeConfig {
        listen: ListenConfig {
            grpc: SocketAddr::from(([127, 0, 0, 1], 0)),
            http: SocketAddr::from(([127, 0, 0, 1], 0)),
        },
        tls: None,
        auth: AuthConfig {
            enabled: false,
            jwks_url: None,
            jwks_refresh_seconds: 300,
        },
        ops_console: OpsConsoleConfig {
            source: OpsConsoleAssetSource::Embedded,
        },
        namespace: NamespaceConfig {
            mode: NamespaceMode::SharedEngine,
        },
        worker: WorkerConfig {
            heartbeat_window: Duration::from_secs(30),
            ..Default::default()
        },
        websocket: WebSocketConfig {
            outbound_buffer_bound: 32,
            event_broadcast_capacity: Some(64),
            cluster_broadcast_capacity: Some(64),
        },
        workflow_packages: Vec::new(),
        deploy: DeployConfig::default(),
        authoring: AuthoringConfig::default(),
        dev: aion_server::config::DevConfig::default(),
        outbox: aion_server::config::OutboxConfig::default(),
        observability: aion_server::config::ObservabilityConfig::with_flush_policy(64, 0),
        mcp: aion_server::config::ResolvedMcpConfig::default(),
        scheduler_threads: 1,
        query_timeout: Some(Duration::from_secs(10)),
        default_namespace: "default".to_owned(),
        auto_create: aion_server::config::AutoCreate::Open,
        max_in_flight_activities: aion_server::config::DEFAULT_MAX_IN_FLIGHT_ACTIVITIES,
        drain_timeout: Duration::from_secs(30),
        metrics: MetricsConfig { enabled: false },
        owned_shards: Vec::new(),
        cors_allowed_origins: Vec::new(),
    }
}

/// A server state with [`SOURCE`] deployed, and that fixture's one action.
pub async fn state_with_contract() -> TestResult<(ServerState, aion_package::ActionContract)> {
    let (state, actions) = state_from(SOURCE).await?;
    let action = actions
        .into_iter()
        .next()
        .ok_or("the fixture package declared no actions")?;
    Ok((state, action))
}

/// A server state with `source` compiled, packaged and loaded into the engine.
pub async fn state_from(
    source: &str,
) -> TestResult<(ServerState, Vec<aion_package::ActionContract>)> {
    let root = tempfile::tempdir()?;
    let prepared = compile_and_assemble_awl(source, root.path(), "contract_registration.awl")?;
    let package = Package::load_from_bytes(prepared.archive, ExtractionLimits::unbounded())?;
    let actions = package.contract()?.workers[0].actions.clone();
    let state =
        ServerState::build_with_store(aion_store::InMemoryStore::default(), runtime_config())
            .await?;
    state.engine()?.load_package(package).await?;
    Ok((state, actions))
}

/// The wire descriptor advertising an action exactly as its contract declares it.
pub fn descriptor_for(action: &aion_package::ActionContract) -> generated::ActivityDescriptor {
    generated::ActivityDescriptor {
        name: action.name.clone(),
        input_schema_json: action.input_schema.to_string(),
        output_schema_json: action.output_schema.to_string(),
    }
}

/// Dial `stream_worker` with one advertised activity and no node.
pub async fn register(
    state: ServerState,
    activity: generated::ActivityDescriptor,
) -> TestResult<Result<tonic::Response<tonic::Streaming<generated::ServerToWorker>>, tonic::Status>>
{
    register_on_node(state, String::new(), vec![activity]).await
}

/// Dial `stream_worker` advertising `activities` from `node`.
///
/// The identity is fixed at `test-build` so a caller asserting on repeated
/// dials is varying only what it means to vary.
pub async fn register_on_node(
    state: ServerState,
    node: String,
    activities: Vec<generated::ActivityDescriptor>,
) -> TestResult<Result<tonic::Response<tonic::Streaming<generated::ServerToWorker>>, tonic::Status>>
{
    register_as(state, node, activities, "test-build").await
}

/// Dial `stream_worker` under an explicit worker-build `identity`.
///
/// The refusal-transition rule is keyed on (queue, node) — never on anything the
/// refused party supplies — and the build identity rides in the remembered
/// VALUE. So a test proving a CHANGED refusal speaks again holds identity fixed
/// while the reason moves, and a test proving a redeployed-but-still-broken
/// worker is heard again moves the identity alone. Both need this to be a
/// parameter, not a constant.
pub async fn register_as(
    state: ServerState,
    node: String,
    activities: Vec<generated::ActivityDescriptor>,
    identity: &str,
) -> TestResult<Result<tonic::Response<tonic::Streaming<generated::ServerToWorker>>, tonic::Status>>
{
    let activity_types = activities
        .iter()
        .map(|activity| activity.name.clone())
        .collect();
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(
        tonic::transport::Server::builder()
            .add_service(worker_service(state))
            .serve_with_incoming(TcpListenerStream::new(listener)),
    );
    let mut client = WorkerProtocolClient::connect(format!("http://{address}")).await?;
    let (worker_tx, worker_rx) = tokio::sync::mpsc::channel(2);
    worker_tx
        .send(generated::WorkerToServer {
            message: Some(worker_to_server::Message::Register(
                generated::RegisterWorker {
                    namespaces: vec!["default".to_owned()],
                    activity_types,
                    task_queue: "typed_queue".to_owned(),
                    node,
                    activities,
                    identity: identity.to_owned(),
                    instance: None,
                },
            )),
        })
        .await?;
    let mut request = tonic::Request::new(ReceiverStream::new(worker_rx));
    request
        .metadata_mut()
        .insert("x-aion-namespaces", "default".parse()?);
    let result = client.stream_worker(request).await;
    server.abort();
    Ok(result)
}

/// An advertisement of `fetch` whose input schema is a bare string — the exact
/// class of drift the live incident hit, and a guaranteed contract refusal.
pub fn mismatched_fetch(action: &aion_package::ActionContract) -> generated::ActivityDescriptor {
    generated::ActivityDescriptor {
        name: "fetch".to_owned(),
        input_schema_json: serde_json::json!({"type": "string"}).to_string(),
        output_schema_json: action.output_schema.to_string(),
    }
}
