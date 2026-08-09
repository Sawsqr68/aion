//! The gRPC half of namespace-mint routing, over a REAL tonic hop.
//!
//! Stands up an "owner" `WorkflowGrpcService` on a real tonic server and drives
//! the production [`GrpcMintForwarder`] against it — the same shape
//! `routing_forward_e2e.rs` uses for signals and steered starts, and for the
//! same reason: it proves the wire without needing haematite distribution stood
//! up. (The routing DECISION and the durable quorum write it produces are
//! proved over a real 3-node cluster in `namespace_mint_routing_e2e.rs`.)
//!
//! What is proved here:
//!
//! - a forwarded mint reaches the owner's handler with the caller's metadata
//!   copied verbatim and the forward-hop counter stamped (an interceptor on the
//!   owner records exactly what arrived);
//! - the owner durably mints the namespace with the forwarded provenance;
//! - **policy parity (the tear's rider):** an `auto_create = closed` deployment
//!   refuses a FORWARDED mint with the identical typed refusal it gives a LOCAL
//!   one, and creates nothing. The owner-side handler runs the ordinary
//!   `mint_or_gate` policy path — never a raw store write — so this RPC can
//!   never be a policy bypass;
//! - a mint naming no namespaces, or an unspecified provenance, is refused as
//!   invalid input rather than guessed at.
//!
//! Behind `--features haematite-backend` (the forwarder lives there).

#![cfg(feature = "haematite-backend")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use aion::EngineBuilder;
use aion_proto::generated::{self, workflow_service_server::WorkflowServiceServer};
use aion_server::api::grpc::workflow_service;
use aion_server::config::{AutoCreate, NamespaceConfig, NamespaceMode};
use aion_server::namespace::{
    ForwardMintError, GrpcMintForwarder, MintCredentials, MintForwarder, encode_mint_origin,
};
use aion_server::routing::{FORWARD_HOPS_METADATA, GrpcRequestForwarder, RequestForwarder};
use aion_server::{NamespaceResolver, ServerState};
use aion_store::{EventStore, InMemoryStore, NamespaceOrigin, NamespaceStore};
use tokio::net::TcpListener;
use tonic::transport::Server;

type TestError = Box<dyn std::error::Error>;

const NAMESPACE: &str = "tenant-a";
const SUBJECT: &str = "alice";

/// What the owner's server actually received, captured by an interceptor in
/// front of the real service.
#[derive(Clone, Default)]
struct Received {
    subject: Option<String>,
    hops: Option<String>,
}

/// An owner: its `ServerState` (whose namespace registry the test inspects) and
/// the metadata its server saw.
struct Owner {
    state: ServerState,
    namespaces: Arc<InMemoryStore>,
    received: Arc<Mutex<Received>>,
}

/// Build an owner `ServerState` over an in-memory engine and an in-memory
/// namespace registry the test holds a handle to, wired through the production
/// resolver exactly as a server boot does.
async fn owner(policy: AutoCreate) -> Result<Owner, TestError> {
    let backing = Arc::new(InMemoryStore::default());
    let store: Arc<dyn EventStore> = Arc::clone(&backing) as Arc<dyn EventStore>;
    let engine = Arc::new(
        EngineBuilder::new()
            .store_arc(store)
            .in_memory_visibility()
            .scheduler_threads(1)
            .query_timeout(Duration::from_secs(5))
            .build()
            .await?,
    );
    let resolver = NamespaceResolver::from_config(
        NamespaceConfig {
            mode: NamespaceMode::SharedEngine,
        },
        engine,
    );
    Ok(Owner {
        state: ServerState::from_parts_with_namespace_store(
            resolver,
            test_runtime(policy),
            backing.clone(),
        ),
        namespaces: backing,
        received: Arc::new(Mutex::new(Received::default())),
    })
}

fn test_runtime(auto_create: AutoCreate) -> aion_server::config::RuntimeConfig {
    use aion_server::config::{
        AuthConfig, AuthoringConfig, DeployConfig, DevConfig, ListenConfig, MetricsConfig,
        OpsConsoleAssetSource, OpsConsoleConfig, OutboxConfig, RuntimeConfig, WebSocketConfig,
        WorkerConfig,
    };
    RuntimeConfig {
        listen: ListenConfig {
            grpc: std::net::SocketAddr::from(([127, 0, 0, 1], 0)),
            http: std::net::SocketAddr::from(([127, 0, 0, 1], 0)),
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
        dev: DevConfig::default(),
        outbox: OutboxConfig::default(),
        observability: aion_server::config::ObservabilityConfig::default(),
        mcp: aion_server::config::ResolvedMcpConfig::default(),
        scheduler_threads: 1,
        query_timeout: Some(Duration::from_secs(5)),
        default_namespace: "default".to_owned(),
        auto_create,
        max_in_flight_activities: aion_server::config::DEFAULT_MAX_IN_FLIGHT_ACTIVITIES,
        drain_timeout: Duration::from_secs(30),
        metrics: MetricsConfig { enabled: true },
        owned_shards: Vec::new(),
        cors_allowed_origins: Vec::new(),
    }
}

/// Spawn the owner's gRPC server on an ephemeral port behind a metadata-recording
/// interceptor; returns its address and a shutdown trigger.
async fn spawn_owner(
    service: WorkflowServiceServer<impl generated::workflow_service_server::WorkflowService>,
    received: Arc<Mutex<Received>>,
) -> Result<(std::net::SocketAddr, tokio::sync::oneshot::Sender<()>), TestError> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
    let intercepted = tonic::service::interceptor::InterceptedService::new(
        service,
        move |request: tonic::Request<()>| {
            let metadata = request.metadata();
            let read = |key: &str| {
                metadata
                    .get(key)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned)
            };
            if let Ok(mut seen) = received.lock() {
                *seen = Received {
                    subject: read("x-aion-subject"),
                    hops: read(FORWARD_HOPS_METADATA),
                };
            }
            Ok(request)
        },
    );
    tokio::spawn(async move {
        let _ = Server::builder()
            .add_service(intercepted)
            .serve_with_incoming_shutdown(incoming, async {
                let _ = rx.await;
            })
            .await;
    });
    // Give the server a moment to start accepting.
    tokio::time::sleep(Duration::from_millis(50)).await;
    Ok((addr, tx))
}

/// The caller credentials a real forwarded start carries: the inbound request's
/// metadata, copied verbatim.
fn caller_credentials() -> Result<MintCredentials, TestError> {
    let mut metadata = tonic::metadata::MetadataMap::new();
    metadata.insert("x-aion-subject", SUBJECT.parse()?);
    metadata.insert("x-aion-namespaces", NAMESPACE.parse()?);
    Ok(MintCredentials::from_grpc_metadata(&metadata))
}

fn mint_forwarder() -> GrpcMintForwarder {
    let transport: Arc<dyn RequestForwarder> = Arc::new(GrpcRequestForwarder::new());
    GrpcMintForwarder::new(transport)
}

/// A forwarded mint lands on the owner: it arrives with the caller's metadata
/// intact and the forward-hop counter stamped, the owner mints it durably with
/// the forwarded provenance, and the caller gets a plain success.
#[tokio::test]
async fn forwarded_mint_lands_on_the_owner_with_metadata_and_hop_stamp() -> Result<(), TestError> {
    let owner = owner(AutoCreate::Open).await?;
    let registry = Arc::clone(&owner.namespaces);
    let received = Arc::clone(&owner.received);
    let (addr, shutdown) =
        spawn_owner(workflow_service(owner.state), Arc::clone(&received)).await?;

    let result = mint_forwarder()
        .forward_mint(
            addr,
            &caller_credentials()?,
            &[NAMESPACE.to_owned()],
            NamespaceOrigin::StartMint,
        )
        .await;
    let _ = shutdown.send(());
    result.map_err(|error| format!("the forwarded mint was refused: {error:?}"))?;

    let record = registry
        .get_namespace(NAMESPACE)
        .await?
        .ok_or("the owner must hold the record it was asked to mint")?;
    assert_eq!(record.name, NAMESPACE);
    assert_eq!(
        record.origin,
        NamespaceOrigin::StartMint,
        "the initiator's provenance travels with the mint"
    );

    let seen = received
        .lock()
        .map_err(|_| "the interceptor's record is poisoned")?
        .clone();
    assert_eq!(
        seen.subject.as_deref(),
        Some(SUBJECT),
        "the caller's metadata must reach the owner verbatim, so it authorizes \
         the ORIGINAL caller and not an anonymous peer"
    );
    assert_eq!(
        seen.hops.as_deref(),
        Some("1"),
        "the forward-hop counter must be stamped, so the cluster's existing loop \
         prevention covers a forwarded mint too"
    );
    Ok(())
}

/// POLICY PARITY (the tear's rider). An `auto_create = closed` deployment
/// refuses a FORWARDED mint with the identical typed refusal it gives a LOCAL
/// one, and creates nothing either way. The owner-side handler runs the ordinary
/// `mint_or_gate` policy path, so the RPC cannot be a way around the policy.
#[tokio::test]
async fn a_closed_policy_refuses_a_forwarded_mint_exactly_as_a_local_one() -> Result<(), TestError>
{
    let owner = owner(AutoCreate::Closed).await?;
    let registry = Arc::clone(&owner.namespaces);

    // The LOCAL refusal, from the very minter the owner-side handler will use.
    let local = owner
        .state
        .namespace_minter()
        .mint_or_gate(&[NAMESPACE.to_owned()], NamespaceOrigin::StartMint)
        .await
        .err()
        .ok_or("a closed policy must refuse an unknown namespace locally")?
        .to_wire_error();

    let received = Arc::clone(&owner.received);
    let (addr, shutdown) = spawn_owner(workflow_service(owner.state), received).await?;
    let forwarded = mint_forwarder()
        .forward_mint(
            addr,
            &caller_credentials()?,
            &[NAMESPACE.to_owned()],
            NamespaceOrigin::StartMint,
        )
        .await
        .err()
        .ok_or("a closed policy must refuse a FORWARDED mint too")?;
    let _ = shutdown.send(());

    let ForwardMintError::Refused(wire) = forwarded else {
        return Err(format!("expected the owner's own typed refusal, got {forwarded:?}").into());
    };
    assert_eq!(
        wire.code, local.code,
        "the forwarded refusal must carry the SAME typed code as the local one"
    );
    assert_eq!(
        wire.message, local.message,
        "and the same message: a forwarded mint is exactly as privileged as a local one"
    );
    assert!(
        registry.get_namespace(NAMESPACE).await?.is_none(),
        "a refused mint must create nothing"
    );
    Ok(())
}

/// A mint naming no namespaces is refused as invalid input, not silently
/// accepted as a no-op ack.
#[tokio::test]
async fn an_empty_namespace_set_is_refused() -> Result<(), TestError> {
    let owner = owner(AutoCreate::Open).await?;
    let received = Arc::clone(&owner.received);
    let (addr, shutdown) = spawn_owner(workflow_service(owner.state), received).await?;

    let forwarder: Arc<dyn RequestForwarder> = Arc::new(GrpcRequestForwarder::new());
    let status = forwarder
        .forward(
            addr,
            caller_credentials()?.to_grpc_metadata(),
            aion_server::routing::ForwardRequest::MintNamespace(generated::MintNamespaceRequest {
                namespaces: Vec::new(),
                origin: encode_mint_origin(NamespaceOrigin::StartMint),
            }),
        )
        .await
        .err()
        .ok_or("an empty mint set must be refused")?;
    let _ = shutdown.send(());
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
    Ok(())
}

/// An unspecified provenance is refused rather than guessed at: the owner will
/// not invent a namespace's origin.
#[tokio::test]
async fn an_unspecified_origin_is_refused() -> Result<(), TestError> {
    let owner = owner(AutoCreate::Open).await?;
    let registry = Arc::clone(&owner.namespaces);
    let received = Arc::clone(&owner.received);
    let (addr, shutdown) = spawn_owner(workflow_service(owner.state), received).await?;

    let forwarder: Arc<dyn RequestForwarder> = Arc::new(GrpcRequestForwarder::new());
    let status = forwarder
        .forward(
            addr,
            caller_credentials()?.to_grpc_metadata(),
            aion_server::routing::ForwardRequest::MintNamespace(generated::MintNamespaceRequest {
                namespaces: vec![NAMESPACE.to_owned()],
                origin: 0,
            }),
        )
        .await
        .err()
        .ok_or("an unspecified mint origin must be refused")?;
    let _ = shutdown.send(());
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
    assert!(
        registry.get_namespace(NAMESPACE).await?.is_none(),
        "a refused mint must create nothing"
    );
    Ok(())
}

/// The measured cost of routing, stated rather than assumed.
///
/// `GrpcRequestForwarder` is a unit struct with no connection pool, so EVERY
/// forwarded mint pays a fresh TCP + HTTP/2 handshake to the owner. This walks
/// the same number of mints down both paths on the same box in the same process
/// and prints both, so the evidence file quotes a measurement and not a guess.
///
/// It asserts only that both paths mint — never a timing threshold, which would
/// be a flake on a contended box. The numbers are read from the printed line.
#[tokio::test]
async fn measure_the_cost_of_a_forwarded_mint_against_a_local_one() -> Result<(), TestError> {
    /// Enough mints to average out scheduler noise, few enough to stay fast.
    const MINTS: u32 = 20;

    let owner = owner(AutoCreate::Open).await?;
    let registry = Arc::clone(&owner.namespaces);
    let local_minter = owner.state.namespace_minter();

    let began = std::time::Instant::now();
    for index in 0..MINTS {
        local_minter
            .mint_or_gate(&[format!("local-{index}")], NamespaceOrigin::StartMint)
            .await?;
    }
    let local_elapsed = began.elapsed();

    let received = Arc::clone(&owner.received);
    let (addr, shutdown) = spawn_owner(workflow_service(owner.state), received).await?;
    let forwarder = mint_forwarder();
    let began = std::time::Instant::now();
    for index in 0..MINTS {
        forwarder
            .forward_mint(
                addr,
                &caller_credentials()?,
                &[format!("forwarded-{index}")],
                NamespaceOrigin::StartMint,
            )
            .await
            .map_err(|error| format!("forwarded mint {index} failed: {error:?}"))?;
    }
    let forwarded_elapsed = began.elapsed();
    let _ = shutdown.send(());

    println!(
        "MINT COST over {MINTS} mints: local total {local_elapsed:?} (mean {:?}); \
         forwarded total {forwarded_elapsed:?} (mean {:?}), fresh dial per forward",
        local_elapsed / MINTS,
        forwarded_elapsed / MINTS,
    );

    // Both paths really minted: the measurement is of work, not of no-ops.
    assert!(registry.get_namespace("local-0").await?.is_some());
    assert!(registry.get_namespace("forwarded-0").await?.is_some());
    Ok(())
}
