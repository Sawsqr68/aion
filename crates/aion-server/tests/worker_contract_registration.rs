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
    let root = tempfile::tempdir()?;
    let prepared = compile_and_assemble_awl(SOURCE, root.path())?;
    let package = Package::load_from_bytes(prepared.archive, ExtractionLimits::unbounded())?;
    let action = package.contract()?.workers[0].actions[0].clone();
    let state =
        ServerState::build_with_store(aion_store::InMemoryStore::default(), runtime_config())
            .await?;
    state.engine()?.load_package(package).await?;
    Ok((state, action))
}

async fn register(
    state: ServerState,
    activity: generated::ActivityDescriptor,
) -> TestResult<Result<tonic::Response<tonic::Streaming<generated::ServerToWorker>>, tonic::Status>>
{
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
                    activity_types: vec!["fetch".to_owned()],
                    task_queue: "typed_queue".to_owned(),
                    node: String::new(),
                    activities: vec![activity],
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
