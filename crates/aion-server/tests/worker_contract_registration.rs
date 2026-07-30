//! Real gRPC registration contract-admission regressions for matching and
//! field-level mismatched activity schemas.

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

const SOURCE: &str = "//! Registration contract fixture.\nworkflow contract_registration\n  input url: String\n  outcome done: type Result, route success\n\ntype Result { ok: Bool }\n\nworker typed_queue\n  action fetch(url: String) -> Result\n\nstep call\n  fetch(url: url) -> result\n  route done(ok: result.ok)\n";

/// A queue whose two actions are pinned to DIFFERENT nodes — the shape every
/// multi-role worker serves, since the server routes by
/// (namespace × `task_queue` × node) and a process serving both nodes must open
/// one connection per node advertising only that node's actions.
const PARTITIONED_SOURCE: &str = "//! Node-partitioned registration fixture.\nworkflow partitioned_registration\n  input url: String\n  outcome done: type Result, route success\n\ntype Result { ok: Bool }\n\nworker typed_queue\n  action fetch(url: String) -> Result\n    node netbox\n  action grade(url: String) -> Result\n    node shell\n\nstep call\n  fetch(url: url) -> fetched\n  grade(url: url) -> graded\n  route done(ok: fetched.ok and graded.ok)\n";

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn runtime_config() -> RuntimeConfig {
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
        observability: aion_server::config::ObservabilityConfig::default(),
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

async fn state_with_contract() -> TestResult<(ServerState, aion_package::ActionContract)> {
    let (state, actions) = state_from(SOURCE).await?;
    let action = actions
        .into_iter()
        .next()
        .ok_or("the fixture package declared no actions")?;
    Ok((state, action))
}

async fn state_from(source: &str) -> TestResult<(ServerState, Vec<aion_package::ActionContract>)> {
    let root = tempfile::tempdir()?;
    let prepared = compile_and_assemble_awl(source, root.path())?;
    let package = Package::load_from_bytes(prepared.archive, ExtractionLimits::unbounded())?;
    let actions = package.contract()?.workers[0].actions.clone();
    let state =
        ServerState::build_with_store(aion_store::InMemoryStore::default(), runtime_config())
            .await?;
    state.engine()?.load_package(package).await?;
    Ok((state, actions))
}

/// The action contract the fixture declared under `name`.
fn action_named(
    actions: &[aion_package::ActionContract],
    name: &str,
) -> TestResult<aion_package::ActionContract> {
    actions
        .iter()
        .find(|action| action.name == name)
        .cloned()
        .ok_or_else(|| format!("the fixture declared no action `{name}`").into())
}

/// The wire descriptor advertising an action exactly as its contract declares it.
fn descriptor_for(action: &aion_package::ActionContract) -> generated::ActivityDescriptor {
    generated::ActivityDescriptor {
        name: action.name.clone(),
        input_schema_json: action.input_schema.to_string(),
        output_schema_json: action.output_schema.to_string(),
    }
}

async fn register(
    state: ServerState,
    activity: generated::ActivityDescriptor,
) -> TestResult<Result<tonic::Response<tonic::Streaming<generated::ServerToWorker>>, tonic::Status>>
{
    register_on_node(state, String::new(), vec![activity]).await
}

async fn register_on_node(
    state: ServerState,
    node: String,
    activities: Vec<generated::ActivityDescriptor>,
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
                    identity: "test-build".to_owned(),
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

#[tokio::test]
async fn mismatched_worker_is_refused_with_field_level_diff() -> TestResult {
    let (state, action) = state_with_contract().await?;
    let result = register(
        state,
        generated::ActivityDescriptor {
            name: "fetch".to_owned(),
            input_schema_json: serde_json::json!({"type":"string"}).to_string(),
            output_schema_json: action.output_schema.to_string(),
        },
    )
    .await?;
    let status = result.err().ok_or("mismatched registration was accepted")?;

    assert_eq!(status.code(), tonic::Code::FailedPrecondition);
    assert!(status.message().contains("WORKER_CONTRACT_MISMATCH"));
    assert!(status.message().contains("input_schema"));
    assert!(status.message().contains("expected"));
    assert!(status.message().contains("worker advertised"));
    Ok(())
}

/// THE DEFECT, on the real gRPC registration path. Admission checked every
/// bodyless action of the queue against every connection with no regard for the
/// node dimension, so a worker serving `netbox` was refused for omitting the
/// `shell` node's action and vice versa: BOTH connections rejected on every
/// dial, the queue unservable, and the refused process looking merely asleep.
///
/// This test lives at the transport rather than only over `contract_diffs`
/// because the unit-level rule can be perfectly right while the server hands it
/// an empty node forever — the threading is the part that was missing.
#[tokio::test]
async fn a_connection_serving_one_node_is_admitted_without_the_other_nodes_action() -> TestResult {
    let (state, actions) = state_from(PARTITIONED_SOURCE).await?;
    let fetch = action_named(&actions, "fetch")?;

    let result = register_on_node(state, "netbox".to_owned(), vec![descriptor_for(&fetch)]).await?;

    assert!(
        result.is_ok(),
        "the netbox connection serves its whole job and must be admitted without \
         advertising the shell node's action: {:?}",
        result.err().map(|status| status.message().to_owned())
    );
    Ok(())
}

/// The gate keeps its teeth on the actions that CAN reach the connection: the
/// `shell` node's own action, omitted, is still refused — and the refusal names
/// the node that owed it so the operator knows which connection to look at.
#[tokio::test]
async fn a_connection_omitting_its_own_nodes_action_is_still_refused() -> TestResult {
    let (state, actions) = state_from(PARTITIONED_SOURCE).await?;
    let fetch = action_named(&actions, "fetch")?;

    // On the `shell` node, advertising only the netbox node's action.
    let result = register_on_node(state, "shell".to_owned(), vec![descriptor_for(&fetch)]).await?;
    let status = result
        .err()
        .ok_or("a connection omitting its own node's action was accepted")?;

    assert_eq!(status.code(), tonic::Code::FailedPrecondition);
    assert!(status.message().contains("WORKER_CONTRACT_MISMATCH"));
    assert!(
        status.message().contains("grade"),
        "the refusal must name the unserved action: {}",
        status.message()
    );
    assert!(
        status.message().contains("pinned to node `shell`"),
        "the refusal must name the node that owed the action: {}",
        status.message()
    );
    Ok(())
}

#[tokio::test]
async fn matching_worker_receives_register_ack() -> TestResult {
    let (state, action) = state_with_contract().await?;
    let result = register(
        state,
        generated::ActivityDescriptor {
            name: action.name,
            input_schema_json: action.input_schema.to_string(),
            output_schema_json: action.output_schema.to_string(),
        },
    )
    .await?;

    assert!(result.is_ok());
    Ok(())
}
