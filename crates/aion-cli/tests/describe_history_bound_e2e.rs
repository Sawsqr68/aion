//! Regression pin for the Rust client's bounded `describe` contract.
//!
//! The fixture serves the production workflow gRPC service over loopback and
//! dials it through the public `aion_client::Client`. Histories are recorded
//! directly into the in-memory store, which is the sanctioned test-only writer.

use std::{net::SocketAddr, sync::Arc, time::Duration};

use aion::EngineBuilder;
use aion_client::{Client, ClientBuilder};
use aion_core::{
    ContentType, Event, EventEnvelope, PackageVersion, Payload, RunId, WorkflowId, WorkflowStatus,
};
use aion_server::api::grpc::workflow_service;
use aion_server::config::{
    AuthConfig, AuthoringConfig, DeployConfig, ListenConfig, MetricsConfig, NamespaceConfig,
    NamespaceMode, OpsConsoleAssetSource, OpsConsoleConfig, RuntimeConfig, WebSocketConfig,
    WorkerConfig,
};
use aion_server::{
    NamespaceResolver, ServerState, StaticScheduleNamespaces, StaticWorkflowNamespaces,
};
use aion_store::{EventStore, InMemoryStore, WriteToken, visibility::VisibilityStore};
use chrono::Utc;
use tokio_stream::wrappers::TcpListenerStream;
use uuid::Uuid;

const NAMESPACE: &str = "tenant-a";
const SIGNAL_PAYLOAD_BYTES: usize = 100_000;
const LARGE_HISTORY_EVENTS: usize = 50;
const MUCH_LARGER_HISTORY_EVENTS: usize = LARGE_HISTORY_EVENTS * 10;

type TestError = Box<dyn std::error::Error>;

fn workflow_id() -> WorkflowId {
    WorkflowId::new(Uuid::from_u128(0x86))
}

fn run_id() -> RunId {
    RunId::new(Uuid::from_u128(0x8601))
}

fn envelope(seq: u64) -> EventEnvelope {
    EventEnvelope {
        seq,
        recorded_at: Utc::now(),
        workflow_id: workflow_id(),
    }
}

fn started() -> Event {
    Event::WorkflowStarted {
        envelope: envelope(1),
        workflow_type: "describe_control".to_owned(),
        input: Payload::new(ContentType::Json, b"{}".to_vec()),
        run_id: run_id(),
        parent_run_id: None,
        parent_workflow_id: None,
        package_version: PackageVersion::new("a".repeat(64)),
    }
}

fn signal(index: usize) -> Event {
    Event::SignalReceived {
        envelope: envelope((index + 2) as u64),
        name: format!("signal-{index}"),
        payload: Payload::new(ContentType::Json, vec![b'x'; SIGNAL_PAYLOAD_BYTES]),
    }
}

fn runtime_config() -> RuntimeConfig {
    RuntimeConfig {
        listen: ListenConfig {
            grpc: SocketAddr::from(([127, 0, 0, 1], 0)),
            http: SocketAddr::from(([127, 0, 0, 1], 0)),
        },
        tls: None,
        cors_allowed_origins: Vec::new(),
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
            ..WorkerConfig::default()
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
        jit_threshold: None,
        query_timeout: Some(Duration::from_secs(10)),
        default_namespace: "default".to_owned(),
        auto_create: aion_server::config::AutoCreate::default(),
        max_in_flight_activities: aion_server::config::DEFAULT_MAX_IN_FLIGHT_ACTIVITIES,
        drain_timeout: Duration::from_secs(30),
        metrics: MetricsConfig { enabled: false },
        owned_shards: Vec::new(),
    }
}

async fn serve_history(
    event_count: usize,
) -> Result<(Client, tokio::task::JoinHandle<()>), TestError> {
    let backing = Arc::new(InMemoryStore::default());
    let store: Arc<dyn EventStore> = backing.clone();
    let visibility_store: Arc<dyn VisibilityStore> = backing;
    let mut events = Vec::with_capacity(event_count + 1);
    events.push(started());
    events.extend((0..event_count).map(signal));
    store
        .append(WriteToken::recorder(), &workflow_id(), &events, 0)
        .await?;

    let engine = Arc::new(
        EngineBuilder::new()
            .store_arc(Arc::clone(&store))
            .visibility_store_arc(visibility_store)
            .scheduler_threads(1)
            .build()
            .await?,
    );
    let ownership = StaticWorkflowNamespaces::default();
    ownership.record(workflow_id(), NAMESPACE)?;
    let resolver = NamespaceResolver::from_parts(
        NamespaceMode::SharedEngine,
        Some(engine),
        Arc::new(ownership),
        Arc::new(StaticScheduleNamespaces::default()),
    );
    let state = ServerState::from_parts(resolver, runtime_config());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move {
        let result = tonic::transport::Server::builder()
            .add_service(workflow_service(state))
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await;
        if let Err(error) = result {
            eprintln!("describe fixture server exited: {error}");
        }
    });
    let client = ClientBuilder::new(format!("http://{address}"))
        .with_namespace(NAMESPACE)
        .build()
        .await?;
    Ok((client, server))
}

fn assert_description_identity(
    description: &aion_client::WorkflowDescription,
    expected_head_seq: u64,
) {
    assert_eq!(description.summary.workflow_id, workflow_id());
    assert_eq!(description.summary.workflow_type, "describe_control");
    assert_eq!(description.summary.status, WorkflowStatus::Running);
    assert_eq!(description.run_id, run_id());
    assert_eq!(description.history_head_seq, expected_head_seq);
    assert_eq!(description.terminal_event, None);
}

#[tokio::test]
async fn short_history_describe_is_unchanged() -> Result<(), TestError> {
    let (client, server) = serve_history(0).await?;
    let description = client.describe(&workflow_id(), Some(&run_id())).await?;
    assert_description_identity(&description, 1);
    let page = client.read_history(&workflow_id(), Some(1), None).await?;
    assert_eq!(page.head_seq, 1);
    assert_eq!(page.next_from_seq, None);
    assert_eq!(page.events.len(), 1);
    let Some(Event::WorkflowStarted {
        workflow_type,
        run_id: described_run_id,
        input,
        ..
    }) = page.events.first()
    else {
        return Err("short describe did not return its WorkflowStarted identity".into());
    };
    assert_eq!(workflow_type, "describe_control");
    assert_eq!(described_run_id, &run_id());
    assert_eq!(input, &Payload::new(ContentType::Json, b"{}".to_vec()));
    server.abort();
    Ok(())
}

#[tokio::test]
async fn describe_answers_for_histories_of_arbitrary_size() -> Result<(), TestError> {
    for event_count in [LARGE_HISTORY_EVENTS, MUCH_LARGER_HISTORY_EVENTS] {
        let (client, server) = serve_history(event_count).await?;
        let description = client.describe(&workflow_id(), Some(&run_id())).await?;
        assert_description_identity(&description, (event_count + 1) as u64);
        server.abort();
    }
    Ok(())
}
