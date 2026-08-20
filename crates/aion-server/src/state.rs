//! Shared server state constructed once at startup.

use std::{path::PathBuf, sync::Arc};

use aion::{
    ActivityDispatcher, EngineBuilder, RuntimeHandle, SignalRouter, signal::ConcreteSignalRouter,
};
use aion_store::{EventStore, NamespaceStore, OutboxStore, WorkerDeploymentStore};

use crate::dev_ui::{ActivityMockRegistry, DevMockingDispatcher};

#[cfg(feature = "auth")]
use crate::auth::JwksCache;
use crate::{
    config::{RuntimeConfig, ServerConfig, StoreBackend, StoreConfig},
    error::ServerError,
    namespace::{NamespaceGuard, NamespaceMinter, resolver::NamespaceResolver},
    observability::{
        Metrics, health::HealthState, instrumented_store::InstrumentedEventStore,
        metrics::MetricsError,
    },
    shutdown::DrainState,
    worker::{
        ConnectedWorkerRegistry, HeartbeatTracker, PendingActivities, WorkerActivityDispatcher,
        supervisor::WorkerSupervisor,
    },
};

/// Build an uncommissioned supervisor over one durable deployment store and
/// the state's cluster channel.
///
/// Every state constructor routes through this, so no construction path can
/// accidentally hand the supervisor a DIFFERENT store from the one the
/// management API writes through — desired state written in one place and read
/// in another is exactly the drift this single call site removes. The same
/// argument holds for the publisher: it is the state's own cluster channel,
/// so a desired-state write made by the supervisor reaches the same live feed
/// as one made by the worker-deployment endpoints.
fn new_supervisor(
    store: &Arc<dyn WorkerDeploymentStore>,
    publisher: &crate::cluster_publisher::ClusterEventPublisher,
) -> Arc<WorkerSupervisor> {
    Arc::new(WorkerSupervisor::new(Arc::clone(store), publisher.clone()))
}

/// Cloneable shared state passed to all server transports.
#[derive(Clone)]
pub struct ServerState {
    inner: Arc<ServerStateInner>,
}

struct ServerStateInner {
    namespace_guard: NamespaceGuard,
    runtime: RuntimeConfig,
    worker_registry: ConnectedWorkerRegistry,
    pending_activities: PendingActivities,
    heartbeat_tracker: HeartbeatTracker,
    /// Correlation registry joining the liveness probe's pushed gRPC pings to
    /// the answers that come back up each worker's inbound stream (#197).
    ///
    /// Held on the state because the two halves live in tasks that cannot see
    /// each other: the probe owns a timer loop, and each answer arrives inside
    /// the tonic stream handler for one worker.
    grpc_liveness_waiters: crate::worker::GrpcLivenessWaiters,
    drain_state: DrainState,
    metrics: Option<Metrics>,
    health: Option<HealthState>,
    /// Shared per-run activity-mock registry. Present only when the dev surface
    /// is commissioned; the engine's dispatcher consults this exact instance.
    activity_mock_registry: Option<ActivityMockRegistry>,
    /// The durable store cast as an [`OutboxStore`]. `None` for the in-memory
    /// backend, which intentionally has no outbox.
    outbox_store: Option<Arc<dyn OutboxStore>>,
    /// The durable namespace registry, captured from the SAME concrete leaf
    /// backend as the engine's `EventStore` BEFORE that leaf is wrapped in the
    /// decorator chain (`PublishingEventStore` → `InstrumentedEventStore`),
    /// which do not implement [`NamespaceStore`]. The haematite backend supplies
    /// the quorum-replicated implementation; the in-memory backend supplies a
    /// local-only one. Always present so the control-plane mint
    /// (Phase 1 S5) and `GET /namespaces` (S7) can reach a real store on every
    /// boot. Mirrors the `cluster_store` retention pattern.
    namespace_store: Arc<dyn NamespaceStore>,
    /// Durable worker-deployment records captured from the same leaf backend.
    worker_deployment_store: Arc<dyn WorkerDeploymentStore>,
    /// Managed-worker supervision (W-1..W-4). Always present and always
    /// UNCOMMISSIONED at construction: it supervises nothing until
    /// [`crate::run`] installs the operator's `[worker_supervision]` policy, so
    /// state construction can never start a process by itself.
    worker_supervisor: Arc<WorkerSupervisor>,
    /// Advisory outbox wake (LSUB-2): the in-process `Notify` shared by the
    /// engine's stage seam (the `InstrumentedEventStore`'s `append_with_outbox`)
    /// and the [`OutboxDispatcher`](crate::worker::OutboxDispatcher) run loop, so
    /// a committed fan-out row wakes the dispatcher in ~RTT instead of waiting up
    /// to one poll interval. Always present (cheap, no `Option`): the handle is
    /// harmless when the outbox is not commissioned, since nothing pulses it.
    outbox_wake: Arc<tokio::sync::Notify>,
    /// WS3 cluster topology/ownership publisher. Always present: the ops console's
    /// cluster channel is served on every boot (calm state with no peers on a
    /// single-node server). Sized from `websocket.cluster_broadcast_capacity`.
    cluster_publisher: crate::cluster_publisher::ClusterEventPublisher,
    /// NOI-5b agent-observability transcript sequencer + live fan-out. Always
    /// present: the transcript channel is served on every boot. The backing
    /// [`ObservabilityStore`](aion_store::ObservabilityStore) is the durable
    /// `O`-keyspace impl on a haematite boot and an in-memory impl on the memory
    /// backend (which has no `O` keyspace), so the transcript path is uniform
    /// across backends while only haematite persists across restart.
    /// Sized from `websocket.cluster_broadcast_capacity` (the same deployment-wide
    /// real-time channel capacity the cluster tail uses).
    transcript_publisher: crate::activity_publisher::ActivityEventPublisher,
    /// NOI-6 server-side intervention routing: the `attempt -> owning-worker`
    /// back-index the intervention router resolves a command's target through.
    /// Always present (cheap, no `Option`): the agent-dispatch path binds an owner
    /// when it dispatches an agent attempt and releases it on completion, so the
    /// router resolves the CURRENT owner. Empty until an agent attempt is
    /// dispatched — a command to an unbound attempt is the attempt-scoped no-op.
    attempt_owners: crate::worker::AttemptOwnerIndex,
    /// R1 live unserved-queue state. Always present (cheap, no `Option`): the
    /// bridge dispatcher publishes every parked dispatch into THIS instance, so
    /// `unserved_queues()` answers "which addresses are unserved, why, and which
    /// runs are waiting on them" without reading logs. Empty whenever nothing is
    /// parked.
    queue_service_state: crate::worker::QueueServiceState,
    /// R1 queue-declaration source, filled in with the engine-backed reader once
    /// the engine exists. Held so surfaces (and the bridge) share ONE reader
    /// rather than each building their own view of the deployed contracts.
    queue_declarations: crate::worker::QueueDeclarationSource,
    /// The declared bodies THIS server is executing right now, shared with the
    /// [`crate::worker::DeclaredCommandDispatcher`] that registers each attempt
    /// for the life of its command. Always present (cheap, no `Option`): the
    /// cancel path signals through it on every boot. Empty whenever no
    /// server-run command is in flight — which, on a `from_parts*` state that
    /// builds no declared-body dispatcher, is always.
    declared_attempts: crate::worker::DeclaredCommandAttempts,
    /// The last completed update check (#189 slice one), shared with the
    /// [`crate::update_check::UpdateCheckObserver`] decorator that writes it
    /// on the full boot path. Always present and always EMPTY at
    /// construction: a server that has never checked says so, and nothing
    /// here ever fetches anything — every check is an explicit operator act.
    update_status: crate::update_check::UpdateStatusState,
    /// The server-resolved workspace root for declared action bodies (#139):
    /// the aion home's `clones/` directory, resolved ONCE at state
    /// construction. The declared-body dispatcher expands `{workspace_root}`
    /// with THIS value and the startup banner reports it, so composition
    /// points read the value instead of re-deriving it. A resolution failure
    /// is held here — boot proceeds, and the failure surfaces as a terminal
    /// refusal when a placeholder-bearing body dispatches.
    workspace_root: crate::worker::WorkspaceRoot,
    /// This node's distribution name for the WS3 cluster snapshot self-identity.
    /// `Some` on a distributed haematite boot (the configured `store.cluster.node_id`),
    /// `None` on a single-node boot — the snapshot then reports the standalone
    /// self-label so the ops console still has a node to render.
    cluster_self_node: Option<String>,
    /// Owns the distributed haematite inbound-write responder thread, kept alive
    /// for the server's lifetime so a cluster node keeps answering peers'
    /// replication/election traffic. `None` for non-distributed boots. Dropping
    /// the state stops the responder.
    cluster_responder: Option<aion_store_haematite::ClusterResponder>,
    /// The concrete distributed haematite store the SS-5b supervisor polls for
    /// peer liveness. `None` for every non-distributed boot.
    cluster_store: Option<Arc<aion_store_haematite::HaematiteStore>>,
    /// The peers the SS-5b supervisor watches (each with the shards this node
    /// adopts on its death). Empty for non-distributed boots.
    watched_peers: Vec<crate::cluster::WatchedPeer>,
    /// The request-routing shard directory (R-2), built over the cluster store +
    /// static peer config. `None` for every non-distributed boot, so the routing
    /// edge falls back to the bare R-1 ownership check (and the default path is a
    /// no-op).
    shard_directory: Option<Arc<crate::routing::StaticShardDirectory>>,
    /// The request forwarder (R-3): relays a non-local signal/query/cancel to the
    /// shard owner's gRPC address. `None` for non-distributed boots. The trait
    /// object makes the liminal forwarder a one-line swap when 13-L0/L1 land (R-6).
    request_forwarder: Option<Arc<dyn crate::routing::RequestForwarder>>,
    #[cfg(feature = "auth")]
    jwks_cache: Option<JwksCache>,
}

impl ServerState {
    /// Fallback cluster broadcast capacity for the `from_parts*` embedder/test
    /// constructors, which bypass config validation. The config-driven
    /// [`Self::build`] path always sizes the publisher from the validated
    /// `websocket.cluster_broadcast_capacity` instead.
    ///
    /// `NonZeroUsize::new(64)` is statically non-`None`, so the
    /// [`Option::unwrap`]-free `match` keeps the value `const` without tripping
    /// the workspace `unwrap_used`/`expect_used` deny lints.
    const FALLBACK_CLUSTER_BROADCAST_CAPACITY: std::num::NonZeroUsize =
        match std::num::NonZeroUsize::new(64) {
            Some(value) => value,
            None => std::num::NonZeroUsize::MIN,
        };

    /// Build shared state from operator configuration.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] if the store cannot connect or the engine cannot
    /// be constructed.
    pub async fn build(config: ServerConfig) -> Result<Self, ServerError> {
        let (store_config, runtime) = config.into_parts();
        let connected = connect_store(store_config).await?;
        Self::build_with_connected_store(connected, runtime).await
    }

    /// Build shared state from an already-constructed store.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::EngineCall`] if the engine cannot be constructed.
    pub async fn build_with_store<S>(store: S, runtime: RuntimeConfig) -> Result<Self, ServerError>
    where
        S: EventStore + NamespaceStore + WorkerDeploymentStore,
    {
        // Capture the concrete leaf as BOTH the event store and the namespace
        // registry before it is wrapped in the (NamespaceStore-unaware) decorator
        // chain — the same leaf, two trait objects.
        let leaf = Arc::new(store);
        let namespace_store: Arc<dyn NamespaceStore> = leaf.clone();
        let worker_deployment_store: Arc<dyn WorkerDeploymentStore> = leaf.clone();
        Self::build_with_connected_store(
            ConnectedStore::local(leaf, None, namespace_store, worker_deployment_store),
            runtime,
        )
        .await
    }

    async fn build_with_connected_store(
        connected: ConnectedStore,
        runtime: RuntimeConfig,
    ) -> Result<Self, ServerError> {
        let cluster_self_node = connected.cluster_self_node();
        let outbox_store = connected.outbox_store;
        let bootstrap_coordinator = connected.bootstrap_coordinator;
        let cluster_responder = connected.cluster_responder;
        let cluster_store = connected.cluster_store;
        let watched_peers = connected.watched_peers;
        // Build the R-2 directory + R-3 forwarder over the (live, failover-aware)
        // cluster store and static peer config. Both present only for a
        // distributed boot; `None` otherwise leaves the routing edge a no-op.
        let RoutingState {
            shard_directory,
            request_forwarder,
            mint_routing,
        } = build_routing_state(
            cluster_store.as_ref(),
            connected.directory_peers,
            connected.self_node_id,
        );
        let (event_broadcast_capacity, query_timeout) = required_engine_seams(&runtime)?;
        let (cluster_publisher, transcript_publisher) =
            build_real_time_publishers(&runtime, connected.observability_store)?;
        let (metrics, outbox_wake, instrumented_store) =
            build_instrumented_store(&runtime, connected.event_store)?;
        let exported_metrics = runtime.metrics.enabled.then_some(metrics.clone());
        let seams = build_worker_seams(
            &runtime,
            &cluster_publisher,
            &connected.namespace_store,
            &connected.worker_deployment_store,
            mint_routing,
        );
        let (
            activity_dispatcher,
            activity_mock_registry,
            attempt_owners,
            workspace_root,
            update_status,
        ) = build_decorated_dispatcher(&runtime, &seams, transcript_publisher.clone());

        let engine = boot_engine(EngineAssembly {
            seams: &seams,
            instrumented_store: &instrumented_store,
            event_broadcast_capacity,
            query_timeout,
            activity_dispatcher,
            active_registry: Arc::new(aion::Registry::default()),
            bootstrap_coordinator,
            runtime: &runtime,
        })
        .await?;
        let resolver = NamespaceResolver::from_config(runtime.namespace.clone(), engine);
        let worker_supervisor =
            new_supervisor(&connected.worker_deployment_store, &cluster_publisher);
        #[cfg(feature = "auth")]
        let jwks_cache = build_jwks_cache(&runtime).await?;
        Ok(Self {
            inner: Arc::new(ServerStateInner {
                namespace_guard: NamespaceGuard::new(resolver),
                runtime,
                metrics: exported_metrics,
                worker_registry: seams.worker_registry,
                pending_activities: seams.pending_activities,
                heartbeat_tracker: seams.heartbeat_tracker,
                grpc_liveness_waiters: crate::worker::GrpcLivenessWaiters::new(),
                drain_state: seams.drain_state,
                health: Some(HealthState::new(instrumented_store, true)),
                activity_mock_registry,
                outbox_store,
                namespace_store: connected.namespace_store,
                worker_supervisor,
                worker_deployment_store: connected.worker_deployment_store,
                outbox_wake,
                cluster_publisher,
                transcript_publisher,
                attempt_owners,
                queue_service_state: seams.queue_service_state,
                queue_declarations: seams.queue_declarations,
                declared_attempts: seams.declared_attempts,
                update_status,
                workspace_root,
                cluster_self_node,
                cluster_responder,
                cluster_store,
                watched_peers,
                shard_directory,
                request_forwarder,
                #[cfg(feature = "auth")]
                jwks_cache,
            }),
        })
    }

    /// Build shared state from explicit parts with a default worker registry.
    #[must_use]
    pub fn from_parts(namespace_resolver: NamespaceResolver, runtime: RuntimeConfig) -> Self {
        // No durable store was supplied (this constructor builds state from a
        // resolver only), so the registry is a local-only in-memory store —
        // present so `namespace_store()` is always reachable, never mutating any
        // durable backend.
        Self::from_parts_with_namespace_store(
            namespace_resolver,
            runtime,
            Arc::new(aion_store::InMemoryStore::default()),
        )
    }

    /// Build shared state from explicit parts with one caller-supplied durable
    /// namespace and worker-deployment leaf.
    ///
    /// Identical to [`Self::from_parts`] except both control-plane dimensions are
    /// derived from `store`: namespace registry reads/writes and durable worker
    /// deployments use the same supplied leaf rather than fresh in-memory state.
    #[must_use]
    pub fn from_parts_with_namespace_store<S>(
        namespace_resolver: NamespaceResolver,
        runtime: RuntimeConfig,
        store: Arc<S>,
    ) -> Self
    where
        S: NamespaceStore + WorkerDeploymentStore,
    {
        let namespace_store: Arc<dyn NamespaceStore> = store.clone();
        let worker_deployment_store: Arc<dyn WorkerDeploymentStore> = store;
        Self::from_parts_with_control_stores(
            namespace_resolver,
            runtime,
            namespace_store,
            worker_deployment_store,
        )
    }

    /// Build shared state with caller-supplied namespace and worker-deployment stores.
    ///
    /// This is the explicit embedder/test seam for retaining both control-plane
    /// contracts from one durable leaf without constructing the full engine boot.
    #[must_use]
    pub fn from_parts_with_control_stores(
        namespace_resolver: NamespaceResolver,
        runtime: RuntimeConfig,
        namespace_store: Arc<dyn NamespaceStore>,
        worker_deployment_store: Arc<dyn WorkerDeploymentStore>,
    ) -> Self {
        let heartbeat_tracker = HeartbeatTracker::new(runtime.worker.heartbeat_window);
        // Bound here, beside the tracker, for the same reason: `runtime` moves
        // into the state below, and the transport-loss budget must be read from
        // the operator's heartbeat window BEFORE it does. The window is a
        // constructor parameter, so an embedder-booted server cannot reach the
        // state carrying a budget the operator never declared.
        let pending_activities = PendingActivities::new(runtime.worker.heartbeat_window);
        // Computed before `runtime` moves into the state: the retention bounds
        // flow from `[observability]` config on the embedder path too, so a
        // from-parts server enforces the same truncation/cap as a full boot.
        let bounds = transcript_bounds(&runtime);
        // These constructors bypass config validation and cannot fail, so an
        // unruled flush policy falls back to the identity (unbatched) one rather
        // than inventing a tuning value; see `TranscriptBatchPolicy::UNBATCHED`.
        let batch = required_transcript_batch_policy(&runtime)
            .unwrap_or(crate::activity_publisher::TranscriptBatchPolicy::UNBATCHED);
        let cluster_publisher = crate::cluster_publisher::ClusterEventPublisher::new(
            Self::FALLBACK_CLUSTER_BROADCAST_CAPACITY,
        );
        Self {
            inner: Arc::new(ServerStateInner {
                namespace_guard: NamespaceGuard::new(namespace_resolver),
                runtime,
                worker_registry: ConnectedWorkerRegistry::default()
                    .with_worker_deployment_store(worker_deployment_store.clone())
                    .with_cluster_publisher(cluster_publisher.clone()),
                pending_activities,
                heartbeat_tracker,
                grpc_liveness_waiters: crate::worker::GrpcLivenessWaiters::new(),
                drain_state: DrainState::default(),
                metrics: None,
                health: None,
                activity_mock_registry: None,
                outbox_store: None,
                namespace_store,
                worker_supervisor: new_supervisor(&worker_deployment_store, &cluster_publisher),
                worker_deployment_store,
                outbox_wake: Arc::new(tokio::sync::Notify::new()),
                cluster_publisher,
                // NOI-5b: a from-parts / embedder state has no durable store, so
                // the transcript sequencer runs over an in-memory `O`-keyspace
                // impl — the transcript channel is served on every boot.
                transcript_publisher: build_transcript_publisher(
                    None,
                    Self::FALLBACK_CLUSTER_BROADCAST_CAPACITY,
                    bounds,
                    batch,
                ),
                attempt_owners: crate::worker::AttemptOwnerIndex::new(),
                queue_service_state: crate::worker::QueueServiceState::default(),
                queue_declarations: crate::worker::QueueDeclarationSource::default(),
                // No declared-body dispatcher is built on this path, so nothing
                // ever registers here; present so the cancel path reads one
                // registry on every construction rather than an `Option`.
                declared_attempts: crate::worker::DeclaredCommandAttempts::new(),
                // Empty and STAYING empty: this constructor builds no
                // dispatcher, so no observer ever writes it — the honest
                // answer for an embedder/test state is "never checked".
                update_status: crate::update_check::UpdateStatusState::default(),
                // Inert here: this constructor builds no declared-body
                // dispatcher, so nothing expands `{workspace_root}` with this
                // value — it exists so `workspace_root()` is always readable.
                workspace_root: crate::worker::WorkspaceRoot::resolve(),
                cluster_self_node: None,
                cluster_responder: None,
                cluster_store: None,
                watched_peers: Vec::new(),
                shard_directory: None,
                request_forwarder: None,
                #[cfg(feature = "auth")]
                jwks_cache: None,
            }),
        }
    }

    /// Build shared state from explicit parts with one caller-supplied durable
    /// namespace/worker-deployment leaf and a caller-supplied JWKS cache.
    ///
    /// The combined seam of [`Self::from_parts_with_namespace_store`] (seed the
    /// durable registry the control-plane read/create paths observe) and
    /// [`Self::from_parts_with_jwks`] (validate bearer tokens against an injected
    /// issuer): an enumerated caller can exercise the real JWT authorization path
    /// against a seeded registry without a full [`Self::build`] boot.
    #[cfg(feature = "auth")]
    #[must_use]
    pub fn from_parts_with_namespace_store_and_jwks<S>(
        namespace_resolver: NamespaceResolver,
        runtime: RuntimeConfig,
        store: Arc<S>,
        jwks_cache: JwksCache,
    ) -> Self
    where
        S: NamespaceStore + WorkerDeploymentStore,
    {
        let namespace_store: Arc<dyn NamespaceStore> = store.clone();
        let worker_deployment_store: Arc<dyn WorkerDeploymentStore> = store;
        let heartbeat_tracker = HeartbeatTracker::new(runtime.worker.heartbeat_window);
        // Bound here, beside the tracker, for the same reason: `runtime` moves
        // into the state below, and the transport-loss budget must be read from
        // the operator's heartbeat window BEFORE it does. The window is a
        // constructor parameter, so an embedder-booted server cannot reach the
        // state carrying a budget the operator never declared.
        let pending_activities = PendingActivities::new(runtime.worker.heartbeat_window);
        // Computed before `runtime` moves into the state: the retention bounds
        // flow from `[observability]` config on the embedder path too, so a
        // from-parts server enforces the same truncation/cap as a full boot.
        let bounds = transcript_bounds(&runtime);
        // These constructors bypass config validation and cannot fail, so an
        // unruled flush policy falls back to the identity (unbatched) one rather
        // than inventing a tuning value; see `TranscriptBatchPolicy::UNBATCHED`.
        let batch = required_transcript_batch_policy(&runtime)
            .unwrap_or(crate::activity_publisher::TranscriptBatchPolicy::UNBATCHED);
        let cluster_publisher = crate::cluster_publisher::ClusterEventPublisher::new(
            Self::FALLBACK_CLUSTER_BROADCAST_CAPACITY,
        );
        Self {
            inner: Arc::new(ServerStateInner {
                namespace_guard: NamespaceGuard::new(namespace_resolver),
                runtime,
                worker_registry: ConnectedWorkerRegistry::default()
                    .with_worker_deployment_store(worker_deployment_store.clone())
                    .with_cluster_publisher(cluster_publisher.clone()),
                pending_activities,
                heartbeat_tracker,
                grpc_liveness_waiters: crate::worker::GrpcLivenessWaiters::new(),
                drain_state: DrainState::default(),
                metrics: None,
                health: None,
                activity_mock_registry: None,
                outbox_store: None,
                namespace_store,
                worker_supervisor: new_supervisor(&worker_deployment_store, &cluster_publisher),
                worker_deployment_store,
                outbox_wake: Arc::new(tokio::sync::Notify::new()),
                cluster_publisher,
                // NOI-5b: a from-parts / embedder state has no durable store, so
                // the transcript sequencer runs over an in-memory `O`-keyspace
                // impl — the transcript channel is served on every boot.
                transcript_publisher: build_transcript_publisher(
                    None,
                    Self::FALLBACK_CLUSTER_BROADCAST_CAPACITY,
                    bounds,
                    batch,
                ),
                attempt_owners: crate::worker::AttemptOwnerIndex::new(),
                queue_service_state: crate::worker::QueueServiceState::default(),
                queue_declarations: crate::worker::QueueDeclarationSource::default(),
                // No declared-body dispatcher is built on this path, so nothing
                // ever registers here; present so the cancel path reads one
                // registry on every construction rather than an `Option`.
                declared_attempts: crate::worker::DeclaredCommandAttempts::new(),
                // Empty and STAYING empty: this constructor builds no
                // dispatcher, so no observer ever writes it — the honest
                // answer for an embedder/test state is "never checked".
                update_status: crate::update_check::UpdateStatusState::default(),
                // Inert here: this constructor builds no declared-body
                // dispatcher, so nothing expands `{workspace_root}` with this
                // value — it exists so `workspace_root()` is always readable.
                workspace_root: crate::worker::WorkspaceRoot::resolve(),
                cluster_self_node: None,
                cluster_responder: None,
                cluster_store: None,
                watched_peers: Vec::new(),
                shard_directory: None,
                request_forwarder: None,
                jwks_cache: Some(jwks_cache),
            }),
        }
    }

    /// Build shared state from explicit parts with a caller-supplied JWKS cache.
    ///
    /// Embedders that construct their own [`JwksCache`] (for example against a
    /// private issuer) can install it here; transports then validate bearer
    /// tokens against it exactly as with a [`Self::build`]-constructed state.
    #[cfg(feature = "auth")]
    #[must_use]
    pub fn from_parts_with_jwks(
        namespace_resolver: NamespaceResolver,
        runtime: RuntimeConfig,
        jwks_cache: JwksCache,
    ) -> Self {
        // No durable store was supplied, so the deployment records and the
        // supervisor built over them share ONE in-memory leaf: two leaves
        // would let the supervisor read a record the API never wrote.
        let fallback_deployment_store: Arc<dyn WorkerDeploymentStore> =
            Arc::new(aion_store::InMemoryStore::default());
        let heartbeat_tracker = HeartbeatTracker::new(runtime.worker.heartbeat_window);
        // Bound here, beside the tracker, for the same reason: `runtime` moves
        // into the state below, and the transport-loss budget must be read from
        // the operator's heartbeat window BEFORE it does. The window is a
        // constructor parameter, so an embedder-booted server cannot reach the
        // state carrying a budget the operator never declared.
        let pending_activities = PendingActivities::new(runtime.worker.heartbeat_window);
        // Computed before `runtime` moves into the state: the retention bounds
        // flow from `[observability]` config on the embedder path too, so a
        // from-parts server enforces the same truncation/cap as a full boot.
        let bounds = transcript_bounds(&runtime);
        // These constructors bypass config validation and cannot fail, so an
        // unruled flush policy falls back to the identity (unbatched) one rather
        // than inventing a tuning value; see `TranscriptBatchPolicy::UNBATCHED`.
        let batch = required_transcript_batch_policy(&runtime)
            .unwrap_or(crate::activity_publisher::TranscriptBatchPolicy::UNBATCHED);
        let cluster_publisher = crate::cluster_publisher::ClusterEventPublisher::new(
            Self::FALLBACK_CLUSTER_BROADCAST_CAPACITY,
        );
        Self {
            inner: Arc::new(ServerStateInner {
                namespace_guard: NamespaceGuard::new(namespace_resolver),
                runtime,
                worker_registry: ConnectedWorkerRegistry::default(),
                pending_activities,
                heartbeat_tracker,
                grpc_liveness_waiters: crate::worker::GrpcLivenessWaiters::new(),
                drain_state: DrainState::default(),
                metrics: None,
                health: None,
                activity_mock_registry: None,
                outbox_store: None,
                // No durable store was supplied (these constructors build state
                // from a resolver only), so the registry is a local-only
                // in-memory store — present so `namespace_store()` is always
                // reachable, never mutating any durable backend.
                namespace_store: Arc::new(aion_store::InMemoryStore::default()),
                worker_supervisor: new_supervisor(&fallback_deployment_store, &cluster_publisher),
                worker_deployment_store: fallback_deployment_store,
                outbox_wake: Arc::new(tokio::sync::Notify::new()),
                cluster_publisher,
                // NOI-5b: a from-parts / embedder state has no durable store, so
                // the transcript sequencer runs over an in-memory `O`-keyspace
                // impl — the transcript channel is served on every boot.
                transcript_publisher: build_transcript_publisher(
                    None,
                    Self::FALLBACK_CLUSTER_BROADCAST_CAPACITY,
                    bounds,
                    batch,
                ),
                attempt_owners: crate::worker::AttemptOwnerIndex::new(),
                queue_service_state: crate::worker::QueueServiceState::default(),
                queue_declarations: crate::worker::QueueDeclarationSource::default(),
                // No declared-body dispatcher is built on this path, so nothing
                // ever registers here; present so the cancel path reads one
                // registry on every construction rather than an `Option`.
                declared_attempts: crate::worker::DeclaredCommandAttempts::new(),
                // Empty and STAYING empty: this constructor builds no
                // dispatcher, so no observer ever writes it — the honest
                // answer for an embedder/test state is "never checked".
                update_status: crate::update_check::UpdateStatusState::default(),
                // Inert here: this constructor builds no declared-body
                // dispatcher, so nothing expands `{workspace_root}` with this
                // value — it exists so `workspace_root()` is always readable.
                workspace_root: crate::worker::WorkspaceRoot::resolve(),
                cluster_self_node: None,
                cluster_responder: None,
                cluster_store: None,
                watched_peers: Vec::new(),
                shard_directory: None,
                request_forwarder: None,
                jwks_cache: Some(jwks_cache),
            }),
        }
    }

    /// Build shared state from explicit parts with a caller-supplied registry.
    #[must_use]
    pub fn from_parts_with_registry(
        namespace_resolver: NamespaceResolver,
        runtime: RuntimeConfig,
        worker_registry: ConnectedWorkerRegistry,
    ) -> Self {
        // No durable store was supplied, so the deployment records and the
        // supervisor built over them share ONE in-memory leaf: two leaves
        // would let the supervisor read a record the API never wrote.
        let fallback_deployment_store: Arc<dyn WorkerDeploymentStore> =
            Arc::new(aion_store::InMemoryStore::default());
        let heartbeat_tracker = HeartbeatTracker::new(runtime.worker.heartbeat_window);
        // Bound here, beside the tracker, for the same reason: `runtime` moves
        // into the state below, and the transport-loss budget must be read from
        // the operator's heartbeat window BEFORE it does. The window is a
        // constructor parameter, so an embedder-booted server cannot reach the
        // state carrying a budget the operator never declared.
        let pending_activities = PendingActivities::new(runtime.worker.heartbeat_window);
        // Computed before `runtime` moves into the state: the retention bounds
        // flow from `[observability]` config on the embedder path too, so a
        // from-parts server enforces the same truncation/cap as a full boot.
        let bounds = transcript_bounds(&runtime);
        // These constructors bypass config validation and cannot fail, so an
        // unruled flush policy falls back to the identity (unbatched) one rather
        // than inventing a tuning value; see `TranscriptBatchPolicy::UNBATCHED`.
        let batch = required_transcript_batch_policy(&runtime)
            .unwrap_or(crate::activity_publisher::TranscriptBatchPolicy::UNBATCHED);
        let cluster_publisher = crate::cluster_publisher::ClusterEventPublisher::new(
            Self::FALLBACK_CLUSTER_BROADCAST_CAPACITY,
        );
        Self {
            inner: Arc::new(ServerStateInner {
                namespace_guard: NamespaceGuard::new(namespace_resolver),
                runtime,
                worker_registry,
                pending_activities,
                heartbeat_tracker,
                grpc_liveness_waiters: crate::worker::GrpcLivenessWaiters::new(),
                drain_state: DrainState::default(),
                metrics: None,
                health: None,
                activity_mock_registry: None,
                outbox_store: None,
                // No durable store was supplied (these constructors build state
                // from a resolver only), so the registry is a local-only
                // in-memory store — present so `namespace_store()` is always
                // reachable, never mutating any durable backend.
                namespace_store: Arc::new(aion_store::InMemoryStore::default()),
                worker_supervisor: new_supervisor(&fallback_deployment_store, &cluster_publisher),
                worker_deployment_store: fallback_deployment_store,
                outbox_wake: Arc::new(tokio::sync::Notify::new()),
                cluster_publisher,
                // NOI-5b: a from-parts / embedder state has no durable store, so
                // the transcript sequencer runs over an in-memory `O`-keyspace
                // impl — the transcript channel is served on every boot.
                transcript_publisher: build_transcript_publisher(
                    None,
                    Self::FALLBACK_CLUSTER_BROADCAST_CAPACITY,
                    bounds,
                    batch,
                ),
                attempt_owners: crate::worker::AttemptOwnerIndex::new(),
                queue_service_state: crate::worker::QueueServiceState::default(),
                queue_declarations: crate::worker::QueueDeclarationSource::default(),
                // No declared-body dispatcher is built on this path, so nothing
                // ever registers here; present so the cancel path reads one
                // registry on every construction rather than an `Option`.
                declared_attempts: crate::worker::DeclaredCommandAttempts::new(),
                // Empty and STAYING empty: this constructor builds no
                // dispatcher, so no observer ever writes it — the honest
                // answer for an embedder/test state is "never checked".
                update_status: crate::update_check::UpdateStatusState::default(),
                // Inert here: this constructor builds no declared-body
                // dispatcher, so nothing expands `{workspace_root}` with this
                // value — it exists so `workspace_root()` is always readable.
                workspace_root: crate::worker::WorkspaceRoot::resolve(),
                cluster_self_node: None,
                cluster_responder: None,
                cluster_store: None,
                watched_peers: Vec::new(),
                shard_directory: None,
                request_forwarder: None,
                #[cfg(feature = "auth")]
                jwks_cache: None,
            }),
        }
    }

    /// Borrow the namespace guard shared by all transports.
    #[must_use]
    pub fn namespace_guard(&self) -> &NamespaceGuard {
        &self.inner.namespace_guard
    }

    /// Build the deploy authorization guard over the shared resolver.
    #[must_use]
    pub fn deploy_guard(&self) -> crate::deploy::DeployGuard {
        crate::deploy::DeployGuard::new(self.inner.namespace_guard.resolver().clone())
    }

    /// Borrow non-secret runtime settings needed by transports.
    #[must_use]
    pub fn runtime_config(&self) -> &RuntimeConfig {
        &self.inner.runtime
    }

    /// Borrow the last-completed-update-check slot (#189 slice one).
    ///
    /// Written only by the [`crate::update_check::UpdateCheckObserver`] on the
    /// full boot path; read by `GET /update-status`. Empty until a check has
    /// genuinely completed — a fresh server has honestly never checked.
    /// Crate-visible only: the slot's type is implementation detail behind
    /// the route, not `aion-server` public API.
    #[must_use]
    pub(crate) fn update_status(&self) -> &crate::update_check::UpdateStatusState {
        &self.inner.update_status
    }

    /// Borrow the server-resolved workspace root declared bodies expand
    /// `{workspace_root}` with (#139).
    ///
    /// The startup banner reads this so composition points learn the value
    /// from the server that will use it, rather than re-deriving it. That
    /// banner claim holds for [`Self::build`]-constructed states, where this
    /// same value is threaded into the declared-body dispatcher; the
    /// `from_parts*` constructors build no such dispatcher, so their copy is
    /// inert — readable, but expanded by nothing.
    #[must_use]
    pub fn workspace_root(&self) -> &crate::worker::WorkspaceRoot {
        &self.inner.workspace_root
    }

    /// Borrow the connected-worker registry shared by worker transports and dispatch.
    #[must_use]
    pub fn worker_registry(&self) -> &ConnectedWorkerRegistry {
        &self.inner.worker_registry
    }

    /// Stop every in-flight activity of `workflow_id`, by whichever path is
    /// executing it (#233).
    ///
    /// Joins the three pieces of live state this node holds — the heartbeat
    /// tracker (which worker holds which activity), the connected-worker
    /// registry (how to reach that worker), and the declared-attempt registry
    /// (which commands this server is running itself) — so the cancel handler
    /// needs only a `ServerState` and never learns the routing itself.
    ///
    /// Returns one record per tracked in-flight activity, INCLUDING the ones
    /// that could not be asked, plus the server-executed declared bodies that
    /// were signalled. A worker cancel is a request, not a guarantee; a
    /// declared body's signal reaches its process group.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] when the tracker's, the registry's,
    /// or the declared-attempt registry's state cannot be read.
    pub fn cancel_in_flight_activities(
        &self,
        workflow_id: &aion_core::WorkflowId,
    ) -> Result<crate::worker::InFlightCancellation, ServerError> {
        crate::worker::cancel_in_flight_activities(
            self.heartbeat_tracker(),
            self.worker_registry(),
            self.declared_attempts(),
            workflow_id,
        )
    }

    /// Borrow the registry of declared bodies this server is executing.
    ///
    /// The declared-body dispatcher registers each attempt here for the life of
    /// its command; [`Self::cancel_in_flight_activities`] signals through it.
    #[must_use]
    pub fn declared_attempts(&self) -> &crate::worker::DeclaredCommandAttempts {
        &self.inner.declared_attempts
    }

    /// Borrow the WS3 cluster-event publisher shared by the cluster state-change
    /// sites (supervisor, worker registry) and the cluster subscription endpoint.
    /// Always present, on every boot.
    #[must_use]
    pub fn cluster_publisher(&self) -> &crate::cluster_publisher::ClusterEventPublisher {
        &self.inner.cluster_publisher
    }

    /// Borrow the NOI-5b transcript sequencer shared by the worker->server
    /// ingestion seam (which publishes a running activity's `ActivityEvent`s) and
    /// the transcript subscription endpoint (which tails + resumes them). Always
    /// present, on every boot.
    #[must_use]
    pub fn transcript_publisher(&self) -> &crate::activity_publisher::ActivityEventPublisher {
        &self.inner.transcript_publisher
    }

    /// Borrow the NOI-6 `attempt -> owning-worker` back-index. The agent-dispatch
    /// path binds an owner when it dispatches an agent attempt and releases it on
    /// completion, so the intervention router always resolves the CURRENT owner.
    #[must_use]
    pub fn attempt_owners(&self) -> &crate::worker::AttemptOwnerIndex {
        &self.inner.attempt_owners
    }

    /// Borrow the R1 live unserved-queue state — the same instance the bridge
    /// dispatcher publishes every parked dispatch into.
    #[must_use]
    pub fn queue_service_state(&self) -> &crate::worker::QueueServiceState {
        &self.inner.queue_service_state
    }

    /// Borrow the R1 queue-declaration source — the engine-backed reader the
    /// bridge classifies against. Answers `Unknown` on a state built without an
    /// engine, which never refuses anything.
    #[must_use]
    pub fn queue_declarations(&self) -> &crate::worker::QueueDeclarationSource {
        &self.inner.queue_declarations
    }

    /// Every queue address currently unserved, with its taxonomy reason, the
    /// policy it is held under, the live poller census behind the verdict, and
    /// the runs parked on it.
    ///
    /// This is the server-side answer to "is anything stuck, and on what" — the
    /// question the pre-R1 seam could only be asked by reading logs.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the state lock is poisoned.
    pub fn unserved_queues(&self) -> Result<Vec<crate::worker::UnservedQueue>, ServerError> {
        self.inner.queue_service_state.unserved()
    }

    /// Every run this engine process could not make resident, with the reason it
    /// could not and when the failure was observed (#117).
    ///
    /// This is the fleet half of the degraded-residency question. `POST
    /// /workflows/describe` answers it for a run an operator can already name;
    /// this answers it for the operator who cannot, which is the case that made
    /// the original defect unrecoverable in practice — the id was only ever
    /// printed in a boot log line that had scrolled away.
    ///
    /// The set is per-process and self-clearing: an entry disappears the moment
    /// the engine observes that run resident, so an EMPTY list is the healthy
    /// answer and never a stale one.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] when the state carries no engine handle, or when
    /// the registry lock is poisoned.
    pub fn unrecoverable_runs(
        &self,
    ) -> Result<Vec<(aion_core::WorkflowId, aion::registry::UnrecoverableRun)>, ServerError> {
        self.engine()?
            .registry()
            .unrecoverable()
            .list()
            .map_err(ServerError::from)
    }

    /// Build the NOI-6 intervention router over the connected-worker registry, the
    /// attempt-owner back-index, and the active intervention transport.
    ///
    /// The transport is the liminal server-push
    /// ([`LiminalInterventionTransport`](crate::worker::LiminalInterventionTransport))
    /// when the `liminal-transport` feature is compiled in — the production path
    /// that pushes a routed command out on the owning worker's connection — and a
    /// null transport otherwise, which reports the target unreachable so every
    /// command NACKs the attempt-scoped no-op rather than silently vanishing. The
    /// router is cheap to build (it clones cloneable handles), so it is constructed
    /// per request at the endpoint rather than stored.
    #[must_use]
    pub fn intervention_router(&self) -> crate::worker::InterventionRouter {
        let transport: std::sync::Arc<dyn crate::worker::InterventionTransport> = {
            #[cfg(feature = "liminal-transport")]
            {
                std::sync::Arc::new(crate::worker::LiminalInterventionTransport)
            }
            #[cfg(not(feature = "liminal-transport"))]
            {
                std::sync::Arc::new(NullInterventionTransport)
            }
        };
        crate::worker::InterventionRouter::new(
            self.inner.worker_registry.clone(),
            self.inner.attempt_owners.clone(),
            transport,
        )
        // Lane #229: an APPLIED InjectMessage is teed into the durable
        // transcript, so the retained record holds the operator's words.
        .with_transcript_publisher(self.inner.transcript_publisher.clone())
    }

    /// This node's configured cluster distribution name for the WS3 snapshot
    /// self-identity, or `None` on a single-node boot (the snapshot then reports
    /// the standalone self-label).
    #[must_use]
    pub fn cluster_self_node(&self) -> Option<&str> {
        self.inner.cluster_self_node.as_deref()
    }

    /// Clone the live engine handle the completion path records terminals through.
    ///
    /// This is the SAME `Arc<Engine>` the gRPC completion callback is built over
    /// (state.rs installs `ServerOutboxDeliveryCallback::new(engine)` on the
    /// pending tracker when `outbox.enabled`), so the liminal completion path
    /// re-enters worker results through the identical `record_fan_out_completion`
    /// seam rather than inventing a second one.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] when the namespace resolver has no engine handle
    /// (a state built from parts without an engine).
    pub fn engine(&self) -> Result<Arc<aion::Engine>, ServerError> {
        self.inner
            .namespace_guard
            .resolver()
            .engine()
            .map(Arc::clone)
    }

    /// Borrow the pending-activities tracker shared by the NIF bridge and worker stream handler.
    #[must_use]
    pub fn pending_activities(&self) -> &PendingActivities {
        &self.inner.pending_activities
    }

    /// Borrow the heartbeat/liveness tracker shared by dispatch and worker streams.
    #[must_use]
    pub fn heartbeat_tracker(&self) -> &HeartbeatTracker {
        &self.inner.heartbeat_tracker
    }

    /// Borrow the gRPC liveness answer-correlation registry (#197).
    ///
    /// The worker stream handler delivers each `LivenessAnswer` into it; the
    /// liveness probe arms and awaits through the SAME handle. There is exactly
    /// one per server, for the same reason there is exactly one probe.
    #[must_use]
    pub fn grpc_liveness_waiters(&self) -> &crate::worker::GrpcLivenessWaiters {
        &self.inner.grpc_liveness_waiters
    }

    /// Borrow the drain gate shared by transports and worker dispatch.
    #[must_use]
    pub fn drain_state(&self) -> &DrainState {
        &self.inner.drain_state
    }

    /// Borrow the prometheus metrics handle when this state was built with a store.
    #[must_use]
    pub fn metrics(&self) -> Option<&Metrics> {
        self.inner.metrics.as_ref()
    }

    /// Borrow health probe state when this state was built with a store.
    #[must_use]
    pub fn health(&self) -> Option<&HealthState> {
        self.inner.health.as_ref()
    }

    /// Borrow the shared per-run activity-mock registry when the dev surface is
    /// commissioned. Returns [`None`] on a server with the dev surface dark, so
    /// the dev handlers refuse cleanly rather than mocking on a production
    /// server.
    #[must_use]
    pub fn activity_mock_registry(&self) -> Option<&ActivityMockRegistry> {
        self.inner.activity_mock_registry.as_ref()
    }

    /// Borrow the outbox store the dispatcher claims rows from, when the durable
    /// (haematite) backend is in use. This is the SAME leaf `Arc<HaematiteStore>` the
    /// engine writes through, so the dispatcher shares its single
    /// `haematite::Connection` rather than opening a second contending one. Returns
    /// [`None`] for the in-memory backend, which has no outbox table.
    #[must_use]
    pub fn outbox_store(&self) -> Option<Arc<dyn OutboxStore>> {
        self.inner.outbox_store.clone()
    }

    /// Borrow the durable namespace registry shared by the control plane.
    ///
    /// This is the SAME concrete leaf backend the engine writes events through
    /// (haematite quorum-replicated, or in-memory local-only),
    /// captured as a [`NamespaceStore`] before the decorator chain wrapped it.
    /// Always present on every boot, so the mint-on-register path (Phase 1 S5)
    /// and `GET /namespaces` (S7) can reach a real registry regardless of
    /// backend.
    #[must_use]
    pub fn namespace_store(&self) -> &Arc<dyn NamespaceStore> {
        &self.inner.namespace_store
    }

    /// Borrow the durable worker-deployment store shared by the control plane.
    #[must_use]
    pub fn worker_deployment_store(&self) -> &Arc<dyn WorkerDeploymentStore> {
        &self.inner.worker_deployment_store
    }

    /// Borrow the managed-worker supervisor.
    ///
    /// Built over the SAME durable deployment store as
    /// [`Self::worker_deployment_store`], so desired state written through the
    /// API is the desired state the supervisor converges on. Uncommissioned
    /// until [`crate::run`] installs an operator policy.
    #[must_use]
    pub fn worker_supervisor(&self) -> &Arc<WorkerSupervisor> {
        &self.inner.worker_supervisor
    }

    /// Build the shared minted-on-use hook over the durable namespace store and
    /// the configured [`AutoCreate`](crate::config::AutoCreate) policy.
    ///
    /// This is the SAME policy logic the worker-registration seam applies (S5);
    /// the workflow-start safety net (S6) calls it after authorization so a
    /// client that starts a workflow before any worker registers still gets a
    /// durable namespace record. Cheap to build (clones an `Arc` + a `Copy`
    /// policy), so transports construct it per request rather than holding it.
    #[must_use]
    pub fn namespace_minter(&self) -> NamespaceMinter {
        let minter = NamespaceMinter::new(
            Arc::clone(&self.inner.namespace_store),
            self.inner.runtime.auto_create,
        )
        // Thread the deployment-global cluster channel so the start-time safety
        // net (S6) and the explicit `POST /namespaces` path (S7) emit the same
        // live "namespace created" delta the worker-mint seam (S5) does — all
        // three mint choke-points surface on the one ops-console push channel.
        .with_cluster_publisher(self.inner.cluster_publisher.clone());
        // And the namespace-mint routing context on a clustered boot, so a
        // namespace whose registry shard this node does not own is minted by the
        // node that does rather than being fenced forever. `None` off-cluster,
        // where the minter is byte-identical to before routing existed.
        match self.namespace_routing() {
            Some(routing) => minter.with_routing(routing),
            None => minter,
        }
    }

    /// The namespace-mint routing context for this boot, or `None` when there is
    /// nothing to route to.
    ///
    /// Present only when ALL THREE handles exist: the distributed store (which
    /// hashes a namespace to its registry shard), the R-2 shard directory (which
    /// resolves that shard's current owner), and the R-3 request forwarder (which
    /// dials it). Those three are populated together by `build_routing_state` on
    /// a `[store.cluster]` boot and are all `None` otherwise, so a partial
    /// context can never arise — but each is checked rather than assumed.
    #[must_use]
    pub fn namespace_routing(&self) -> Option<crate::namespace::NamespaceRouting> {
        build_namespace_routing(
            self.cluster_store(),
            self.shard_directory(),
            self.request_forwarder(),
        )
    }

    /// Clone the advisory outbox wake (LSUB-2) shared with the engine's stage
    /// seam. The outbox dispatcher installs this handle so a committed fan-out row
    /// wakes its run loop in ~RTT rather than waiting for the next poll tick. The
    /// handle is always present; it is simply never pulsed when the outbox is not
    /// commissioned, so wiring it is free and behaviour is unchanged.
    #[must_use]
    pub fn outbox_wake(&self) -> Arc<tokio::sync::Notify> {
        Arc::clone(&self.inner.outbox_wake)
    }

    /// Whether this server is a node in a distributed haematite cluster.
    ///
    /// `true` when boot constructed the distributed backend (a `[store.cluster]`
    /// section was present) and is holding its inbound-write responder alive;
    /// `false` for every single-node / non-haematite boot.
    #[must_use]
    pub fn is_clustered(&self) -> bool {
        self.inner.cluster_responder.is_some()
    }

    /// The concrete distributed haematite store the request-routing edge consults
    /// for shard ownership (`shard_for_workflow` / `owns_workflow_shard`) and
    /// unsteered-start remint. `None` for every single-node / non-clustered boot,
    /// so the routing pre-step is a no-op and the default path is unchanged.
    #[must_use]
    pub fn cluster_store(&self) -> Option<&Arc<aion_store_haematite::HaematiteStore>> {
        self.inner.cluster_store.as_ref()
    }

    /// The request-routing shard directory (R-2) the edge consults to resolve a
    /// non-owned shard's owner. `None` for single-node / non-clustered boots, so
    /// the edge falls back to the bare R-1 ownership check.
    #[must_use]
    pub fn shard_directory(&self) -> Option<&Arc<crate::routing::StaticShardDirectory>> {
        self.inner.shard_directory.as_ref()
    }

    /// The R-3 request forwarder used to relay a non-local signal/query/cancel to
    /// the shard owner. `None` for single-node / non-clustered boots.
    #[must_use]
    pub fn request_forwarder(&self) -> Option<&Arc<dyn crate::routing::RequestForwarder>> {
        self.inner.request_forwarder.as_ref()
    }

    /// Spawn the worker heartbeat expiry sweeper (#176): the production driver
    /// of [`HeartbeatTracker::fail_expired_workers`], failing every worker with
    /// an in-flight task beyond the operator's `worker.heartbeat_window` and
    /// deregistering it with the provable
    /// [`WorkerDeathReason::Timeout`](aion_core::WorkerDeathReason::Timeout).
    ///
    /// Always spawned on the server boot path — dead-worker detection is a
    /// liveness correctness property, not an opt-in feature. The cadence is
    /// derived from the heartbeat window
    /// ([`sweep_interval`](crate::worker::sweep_interval): a quarter of the
    /// window clamped to `[1s, window]`, so the default 30s window sweeps every
    /// 7.5s); there is deliberately no separate config knob. The task exits
    /// when `shutdown` flips to `true`, exactly like the transports; the
    /// returned handle may be dropped to detach it (dropping a tokio
    /// `JoinHandle` never cancels the task) and is returned so tests can await
    /// clean shutdown.
    #[must_use]
    pub fn spawn_heartbeat_sweeper(
        &self,
        shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> tokio::task::JoinHandle<()> {
        let sweeper = crate::worker::HeartbeatSweeper::new(
            self.inner.heartbeat_tracker.clone(),
            self.inner.worker_registry.clone(),
            self.inner.pending_activities.clone(),
            self.inner.drain_state.clone(),
            self.inner.runtime.worker.heartbeat_window,
        )
        // The SAME queue-service state the bridge parks dispatches into, so a
        // deregistration names how many dispatches are already stranded on the
        // queue the dead worker was serving.
        .with_queue_state(self.inner.queue_service_state.clone());
        tokio::spawn(sweeper.run(shutdown))
    }

    /// Spawn the liminal connection dead-man switch (the liveness probe) over
    /// `notifier`.
    ///
    /// Always spawned on a boot that hosts the liminal worker listener:
    /// connection liveness is a correctness property of the transport, not an
    /// opt-in feature. Both timings derive from the operator's
    /// `worker.heartbeat_window` — see
    /// [`LivenessProbe`](crate::worker::LivenessProbe) — so there is no separate
    /// knob. The task exits when `shutdown` flips to `true`, exactly like the
    /// heartbeat sweeper and the transports.
    #[cfg(feature = "liminal-transport")]
    #[must_use]
    pub fn spawn_liminal_liveness_probe(
        &self,
        notifier: std::sync::Arc<crate::worker::LiminalConnectionNotifier>,
        shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> tokio::task::JoinHandle<()> {
        let probe = crate::worker::LivenessProbe::across_transports(
            Some(notifier),
            // #197: the SAME probe covers gRPC-delivered workers. One probe,
            // one probation, one eligibility set — two probes would each
            // publish a whole verdict over the other's, because publication is
            // a replacement rather than a merge.
            Some(self.inner.grpc_liveness_waiters.clone()),
            self.inner.heartbeat_tracker.clone(),
            self.inner.worker_registry.clone(),
            self.inner.runtime.worker.heartbeat_window,
        );
        tokio::spawn(probe.run(shutdown))
    }

    /// Spawn the SS-5b cluster supervisor: a background task that watches every
    /// declared peer's replication liveness and, on a confirmed peer death,
    /// calls `adopt_shards` for that peer's shards on THIS node's live engine —
    /// automatic failover with no manual trigger.
    ///
    /// Does nothing (returns `Ok(())` without spawning) unless this is a
    /// distributed boot whose cluster config declared at least one peer with
    /// `owned_shards`. A single-node / non-clustered server therefore never runs
    /// a supervisor, so default behaviour is unchanged.
    ///
    /// The spawned task drains on `shutdown` exactly like the transports.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] when the engine handle cannot be resolved.
    pub fn spawn_cluster_supervisor(
        &self,
        config: crate::cluster::SupervisorConfig,
        shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Result<bool, ServerError> {
        let Some(cluster_store) = self.inner.cluster_store.clone() else {
            return Ok(false);
        };
        if self.inner.watched_peers.is_empty() {
            return Ok(false);
        }
        let engine = Arc::clone(self.inner.namespace_guard.resolver().engine()?);
        // WS3: feed cluster topology deltas from the supervisor's existing
        // decision points into the ops console channel. `self_node` is the
        // configured distribution name (already captured for the snapshot).
        let publisher = Arc::new(self.inner.cluster_publisher.clone());
        let self_node = self.inner.cluster_self_node.clone().unwrap_or_default();
        // #253: adoption re-runs the terminal-workflow outbox settlement sweep
        // over the widened owned-shard scope, so a dead peer's stranded row for
        // a terminal workflow is settled — never re-armed — by its adopter.
        // With no outbox commissioned there is nothing to settle and the
        // adopter delegates straight to the engine.
        let adopter = Arc::new(crate::cluster::OutboxSettlingAdopter::new(
            engine,
            self.inner.outbox_store.clone(),
        ));
        let supervisor = crate::cluster::ClusterSupervisor::new(
            cluster_store,
            adopter,
            self.inner.watched_peers.clone(),
            config,
        )
        .with_publisher(publisher, self_node);
        if !supervisor.watches_any() {
            return Ok(false);
        }
        tokio::spawn(supervisor.run(shutdown));
        Ok(true)
    }

    /// Borrow the shared JWKS cache when authentication is enabled.
    #[cfg(feature = "auth")]
    #[must_use]
    pub fn jwks_cache(&self) -> Option<&JwksCache> {
        self.inner.jwks_cache.as_ref()
    }

    /// Shut down the embedded engine so in-flight durable appends can finish.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] if the namespace resolver has no engine handle or the engine rejects
    /// shutdown.
    pub fn shutdown(&self) -> Result<(), ServerError> {
        self.inner.namespace_guard.resolver().shutdown_engine()
    }
}

#[cfg(feature = "auth")]
async fn build_jwks_cache(runtime: &RuntimeConfig) -> Result<Option<JwksCache>, ServerError> {
    if !runtime.auth.enabled {
        return Ok(None);
    }
    let Some(url) = runtime.auth.jwks_url.clone() else {
        return Err(ServerError::Config {
            message: "auth.jwks_url must not be empty when auth.enabled is true".to_owned(),
        });
    };
    let interval = std::time::Duration::from_secs(runtime.auth.jwks_refresh_seconds);
    let cache = JwksCache::new(url, interval)
        .await
        .map_err(|error| ServerError::Config {
            message: format!("auth jwks initial fetch failed: {error}"),
        })?;
    Ok(Some(cache))
}

fn metrics_config_error(error: &MetricsError) -> ServerError {
    ServerError::Config {
        message: error.to_string(),
    }
}

/// Borrowed inputs assembled into the embedded engine by [`boot_engine`].
struct EngineAssembly<'a> {
    /// The worker seams installed onto the engine before startup recovery runs.
    seams: &'a WorkerSeams,
    /// The metrics-instrumented store the engine writes through.
    instrumented_store: &'a Arc<InstrumentedEventStore>,
    /// Explicitly-sized broadcast channel capacity for `/events/stream`.
    event_broadcast_capacity: std::num::NonZeroUsize,
    /// Explicit workflow-query reply deadline for `/workflows/query`.
    query_timeout: std::time::Duration,
    /// The activity dispatcher (optionally dev-mock-decorated) the engine uses.
    activity_dispatcher: Arc<dyn ActivityDispatcher>,
    /// The shared active-workflow registry server dispatchers correlate against.
    active_registry: Arc<aion::Registry>,
    /// Whether THIS node seeds the schedule coordinator (SS-2 ownership gate).
    bootstrap_coordinator: bool,
    /// Non-secret runtime settings driving scheduler/outbox/package/shard knobs.
    runtime: &'a RuntimeConfig,
}

/// The search-attribute schema every server-embedded engine runs with.
///
/// The engine REFUSES an append carrying an unregistered attribute, and this is
/// the only place the server registers any. So every attribute name a server
/// writer records must appear here or the write it rides on fails outright — a
/// missing `display_name` registration does not lose the label, it fails every
/// named start (#211). Kept as its own function so that coupling is testable
/// against the actual writers rather than only observable at boot.
fn server_search_attribute_schema() -> Result<aion_core::SearchAttributeSchema, ServerError> {
    let mut schema = aion_core::SearchAttributeSchema::new();
    for (name, label) in [
        (crate::namespace::NAMESPACE_ATTRIBUTE, "namespace"),
        (crate::namespace::TASK_QUEUE_ATTRIBUTE, "task_queue"),
        (crate::namespace::DISPLAY_NAME_ATTRIBUTE, "display_name"),
    ] {
        schema
            .register(name, aion_core::SearchAttributeType::String)
            .map_err(|error| ServerError::Config {
                message: format!("failed to register {label} search attribute: {error}"),
            })?;
    }
    Ok(schema)
}

/// Assemble the embedded engine from the server's runtime configuration and
/// bring it to serving state: build, install every engine-backed seam, then run
/// startup recovery — in that order, enforced here by construction rather than
/// by call-site discipline.
///
/// Factored out of [`ServerState::build_with_connected_store`]; it carries the
/// SS-2 wiring — the coordinator bootstrap gate fed from real ownership and the
/// `owned_shards` hook that drives both scoping and the per-shard election
/// before recovery.
async fn boot_engine(assembly: EngineAssembly<'_>) -> Result<Arc<aion::Engine>, ServerError> {
    let search_attribute_schema = server_search_attribute_schema()?;
    let runtime = assembly.runtime;
    let builder = EngineBuilder::new()
        .store_arc(assembly.instrumented_store.clone())
        .event_streaming(assembly.event_broadcast_capacity)
        .in_memory_visibility()
        .search_attribute_schema(search_attribute_schema)
        .scheduler_threads(runtime.scheduler_threads)
        .outbox_enabled(runtime.outbox.enabled)
        .activity_dispatcher(assembly.activity_dispatcher)
        .active_registry(assembly.active_registry)
        .production_recovery_seam()
        // #266: recovery replay re-dispatches every in-flight activity through
        // the dispatcher decorated above, and that dispatcher consults seams
        // (the declared-body source foremost) that are only installed once the
        // engine exists. Defer recovery out of `build()`;
        // `build_with_connected_store` runs it via `run_startup_recovery`
        // immediately after `install_engine_backed_seams`.
        .defer_startup_recovery()
        .signal_router_factory(|runtime: Arc<RuntimeHandle>, handoff| {
            Arc::new(ConcreteSignalRouter::new(runtime, handoff)) as Arc<dyn SignalRouter>
        })
        .query_timeout(assembly.query_timeout)
        // SS-2: only the node owning the schedule-coordinator's shard seeds and
        // serves it. `true` for every non-distributed boot (owns all shards); a
        // distributed non-owner passes `false` so it does not fence the
        // coordinator stream (AA-4-4). Default `true`, so a single-node boot is
        // byte-identical to today.
        .bootstrap_schedule_coordinator(assembly.bootstrap_coordinator)
        .load_workflow_sources(runtime.workflow_packages.iter().map(PathBuf::as_path));
    // Owned-shard assignment: when the operator pins this node to a shard subset,
    // scope the engine to it AND (SS-2) elect those shards before recovery — the
    // builder's `owned_shards` hook drives both. Empty (the default) leaves the
    // builder untouched, so single-node boot owns ALL shards, elects nothing, and
    // is byte-identical to today.
    let builder = if runtime.owned_shards.is_empty() {
        builder
    } else {
        builder.owned_shards(runtime.owned_shards.iter().copied())
    };
    // JIT compilation threshold: applied ONLY when the operator named one, so
    // absent stays absent all the way down to beamr rather than becoming a
    // value the server picked. Same shape as `owned_shards` above — an unset
    // knob leaves the builder untouched and the boot is byte-identical to a
    // tree without this field.
    let builder = match runtime.jit_threshold {
        Some(threshold) => builder.scheduler_jit_threshold(threshold),
        None => builder,
    };
    let engine = Arc::new(builder.build().await.map_err(ServerError::from)?);
    install_engine_backed_seams(assembly.seams, &engine, runtime.outbox.enabled);
    // #266: startup recovery runs HERE — after every seam the decorated
    // activity dispatcher consults is installed, never before. The build
    // above deferred it; running it earlier re-dispatched adopted
    // in-flight declared-body activities into an empty body source, and
    // they parked forever on their own queue (the fleet e2e pins this).
    engine
        .run_startup_recovery()
        .await
        .map_err(ServerError::from)?;
    Ok(engine)
}

/// Validate the two engine seams the server unconditionally mounts: the event
/// broadcast channel capacity (`/events/stream`) and the query reply deadline
/// (`/workflows/query`). Both are explicit-no-default — a mounted-but-
/// unconfigured surface is never acceptable.
fn required_engine_seams(
    runtime: &RuntimeConfig,
) -> Result<(std::num::NonZeroUsize, std::time::Duration), ServerError> {
    let event_broadcast_capacity = runtime
        .websocket
        .event_broadcast_capacity
        .and_then(std::num::NonZeroUsize::new)
        .ok_or_else(|| ServerError::Config {
            message: crate::config::EVENT_BROADCAST_CAPACITY_REQUIRED.to_owned(),
        })?;
    let query_timeout = runtime
        .query_timeout
        .filter(|timeout| !timeout.is_zero())
        .ok_or_else(|| ServerError::Config {
            message: crate::config::QUERY_TIMEOUT_REQUIRED.to_owned(),
        })?;
    Ok((event_broadcast_capacity, query_timeout))
}

/// Install the outbox delivery callback when the durable outbox is commissioned.
///
/// Routes unmatched worker completions arriving at the sink into the live
/// workflow's mailbox. Flag-off, no callback is installed and the sink's
/// unmatched branch stays a silent drop. The dispatcher is not rebuilt — it
/// shares this exact pending tracker.
fn install_outbox_delivery(
    pending_activities: &PendingActivities,
    engine: &Arc<aion::Engine>,
    outbox_enabled: bool,
) {
    if outbox_enabled {
        let callback = Arc::new(crate::worker::ServerOutboxDeliveryCallback::new(
            Arc::clone(engine),
        ));
        pending_activities.set_outbox_delivery(callback);
    }
}

/// The per-boot worker-side seams every dispatch path shares.
struct WorkerSeams {
    worker_registry: ConnectedWorkerRegistry,
    pending_activities: PendingActivities,
    heartbeat_tracker: HeartbeatTracker,
    drain_state: DrainState,
    queue_declarations: crate::worker::QueueDeclarationSource,
    queue_service_state: crate::worker::QueueServiceState,
    declared_bodies: crate::worker::DeclaredBodySource,
    /// The declared bodies this server is EXECUTING, as opposed to the ones it
    /// can look up. The declared-body dispatcher registers each attempt here for
    /// the life of its command and the cancel path signals through it, so a
    /// cancelled run's server-run command is stopped rather than left holding
    /// the machine.
    declared_attempts: crate::worker::DeclaredCommandAttempts,
    /// The boot's one cluster-event publisher, kept here so the dispatchers
    /// built over these seams announce an unbounded park on the operator's
    /// real-time channel (#266 T4).
    cluster_publisher: crate::cluster_publisher::ClusterEventPublisher,
}

/// Build the worker-side seams: the connected-worker registry (WS3 topology
/// deltas + the Control-Plane Phase 1 mint hook), the shared completion tracker,
/// worker liveness, the drain gate, and the two R1 queue-service handles the
/// bridge publishes into and the state reads from.
fn build_worker_seams(
    runtime: &RuntimeConfig,
    cluster_publisher: &crate::cluster_publisher::ClusterEventPublisher,
    namespace_store: &Arc<dyn NamespaceStore>,
    worker_deployment_store: &Arc<dyn WorkerDeploymentStore>,
    mint_routing: Option<crate::namespace::NamespaceRouting>,
) -> WorkerSeams {
    let worker_registry = ConnectedWorkerRegistry::default()
        .with_cluster_publisher(cluster_publisher.clone())
        .with_namespace_minting(namespace_store.clone(), runtime.auto_create)
        .with_worker_deployment_store(worker_deployment_store.clone());
    // The worker-registration mint is the OTHER `mint_or_gate` choke-point, so
    // it gets the same routing the start seams get: a worker registering for a
    // namespace whose registry shard this node does not own must not be refused
    // `NotOwner` forever either. `None` off-cluster leaves the registry
    // byte-identical.
    let worker_registry = match mint_routing {
        Some(routing) => worker_registry.with_namespace_routing(routing),
        None => worker_registry,
    };
    WorkerSeams {
        worker_registry,
        // The transport-loss budget derives from the operator's heartbeat
        // window (the one value declaring what silence means), so a worker that
        // keeps dying under an activity is re-dispatched attempt-neutrally for
        // a bounded span and then terminates naming the TRANSPORT.
        pending_activities: PendingActivities::new(runtime.worker.heartbeat_window),
        heartbeat_tracker: HeartbeatTracker::new(runtime.worker.heartbeat_window),
        drain_state: DrainState::default(),
        queue_declarations: crate::worker::QueueDeclarationSource::default(),
        queue_service_state: crate::worker::QueueServiceState::default(),
        declared_bodies: crate::worker::DeclaredBodySource::default(),
        declared_attempts: crate::worker::DeclaredCommandAttempts::new(),
        cluster_publisher: cluster_publisher.clone(),
    }
}

/// Point the bridge's R1 classifier at the engine's live workflow catalog.
///
/// Installed only after the engine exists (the dispatcher is built before it),
/// exactly like the outbox delivery callback: the dispatcher is not rebuilt, it
/// shares this handle. Until this runs — and on any state built without an
/// engine — the classifier answers `Unknown`, which never refuses anything.
fn install_queue_declarations(
    queue_declarations: &crate::worker::QueueDeclarationSource,
    engine: &Arc<aion::Engine>,
) {
    queue_declarations.install(Arc::new(crate::worker::EngineQueueDeclarations::new(
        Arc::clone(engine),
    )));
}

/// Point the declared-body executor at the engine's live workflow catalog.
///
/// Installed only after the engine exists, exactly like the queue-declaration
/// reader above: the dispatcher already holds a clone of this handle. Until
/// this runs, every lookup answers `None` and every dispatch takes the worker
/// path — and that window is why the boot defers startup recovery (#266):
/// deploys are durable, so a restarted server DOES have declared bodies its
/// recovered runs depend on before this install, and recovery replay
/// re-dispatching an adopted in-flight activity inside `EngineBuilder::build()`
/// fell through the uninstalled source to a queue with no pollers and parked
/// forever. [`boot_engine`] therefore runs `Engine::run_startup_recovery()`
/// only AFTER `install_engine_backed_seams`, enforced by construction.
/// The one dispatch source that legitimately precedes this install is a
/// fresh start arriving over the API — and the API is not serving yet.
fn install_declared_bodies(
    declared_bodies: &crate::worker::DeclaredBodySource,
    engine: &Arc<aion::Engine>,
) {
    declared_bodies.install(Arc::new(crate::worker::EngineDeclaredBodies::new(
        Arc::clone(engine),
    )));
}

/// Hand every engine-backed seam its reader once the engine exists.
///
/// One boot-path step for the three handles built before the engine and
/// filled in after it: outbox completion delivery, the R1 queue-declaration
/// classifier, and the declared-body executor.
fn install_engine_backed_seams(seams: &WorkerSeams, engine: &Arc<aion::Engine>, outbox: bool) {
    install_outbox_delivery(&seams.pending_activities, engine, outbox);
    install_queue_declarations(&seams.queue_declarations, engine);
    install_declared_bodies(&seams.declared_bodies, engine);
}

/// Decorate the worker activity dispatcher with the per-run activity-mock layer
/// when the dev surface is commissioned, returning the dispatcher and the shared
/// mock registry (if any).
///
/// Dark by default: with the dev surface off the engine gets the bare production
/// dispatcher and there is no mocking path at all (CN4).
/// Compose the engine-seam bridge dispatcher over the state's shared parts.
///
/// Also mints the NOI-6 attempt→owner index and returns it alongside: the
/// bridge binds each liminal-delivered attempt into it for the dispatch's
/// lifetime, and the state stores the SAME instance for the intervention
/// router to read, so the ops console can enumerate and target live attempts.
fn build_bridge_dispatcher(
    runtime: &RuntimeConfig,
    seams: &WorkerSeams,
) -> (WorkerActivityDispatcher, crate::worker::AttemptOwnerIndex) {
    let attempt_owners = crate::worker::AttemptOwnerIndex::new();
    let dispatcher = WorkerActivityDispatcher::new(
        seams.worker_registry.clone(),
        runtime.default_namespace.clone(),
        seams.heartbeat_tracker.clone(),
    )
    .with_pending(seams.pending_activities.clone())
    .with_drain_state(seams.drain_state.clone())
    .with_tokio_handle(tokio::runtime::Handle::current())
    .with_attempt_owners(attempt_owners.clone())
    .with_queue_service(runtime.worker.queue_service.clone())
    .with_queue_declarations(seams.queue_declarations.clone())
    .with_queue_state(seams.queue_service_state.clone())
    .with_cluster_publisher(seams.cluster_publisher.clone());
    (dispatcher, attempt_owners)
}

/// Build the process metrics, the LSUB-2 advisory outbox wake, and the
/// instrumented event store wired to pulse it — the storage-side trio the
/// full boot path assembles before the engine exists.
///
/// The wake is one process-wide `Notify` shared by the engine's stage seam
/// and the outbox dispatcher. A single handle is correct here because there
/// is exactly one in-process dispatcher that sweeps all owned shards per tick
/// — a wake just means "something was staged; sweep".
///
/// # Errors
///
/// Returns [`ServerError`] when the metrics registry cannot be constructed.
fn build_instrumented_store(
    runtime: &RuntimeConfig,
    event_store: Arc<dyn EventStore>,
) -> Result<
    (
        Metrics,
        Arc<tokio::sync::Notify>,
        Arc<InstrumentedEventStore>,
    ),
    ServerError,
> {
    let metrics = Metrics::new().map_err(|error| metrics_config_error(&error))?;
    let outbox_wake = Arc::new(tokio::sync::Notify::new());
    let instrumented_store = Arc::new(
        InstrumentedEventStore::new(
            event_store,
            metrics.clone(),
            runtime.default_namespace.clone(),
        )
        .with_outbox_wake(Arc::clone(&outbox_wake)),
    );
    Ok((metrics, outbox_wake, instrumented_store))
}

/// Build the production bridge dispatcher and its decoration stack in one
/// step: declared-body execution always, the update-check observer above it,
/// the dev mock when commissioned. Also mints the update-status slot the
/// observer writes, returned so the state serves the same slot.
fn build_decorated_dispatcher(
    runtime: &RuntimeConfig,
    seams: &WorkerSeams,
    transcript: crate::activity_publisher::ActivityEventPublisher,
) -> (
    Arc<dyn ActivityDispatcher>,
    Option<ActivityMockRegistry>,
    crate::worker::AttemptOwnerIndex,
    crate::worker::WorkspaceRoot,
    crate::update_check::UpdateStatusState,
) {
    // #139: THE one resolution of the declared-body workspace root. The
    // dispatcher expands `{workspace_root}` with it and the returned value is
    // what the state exposes for the startup banner; nothing else derives it.
    let workspace_root = crate::worker::WorkspaceRoot::resolve();
    // #189 slice one: the update-status slot is created beside the observer
    // that writes it and returned so the state serves the SAME slot.
    let update_status = crate::update_check::UpdateStatusState::default();
    let (dispatcher, attempt_owners) = build_bridge_dispatcher(runtime, seams);
    let (activity_dispatcher, activity_mock_registry) = decorate_activity_dispatcher(
        dispatcher,
        seams.declared_bodies.clone(),
        seams.declared_attempts.clone(),
        workspace_root.clone(),
        transcript,
        update_status.clone(),
        runtime.dev.enabled,
    );
    (
        activity_dispatcher,
        activity_mock_registry,
        attempt_owners,
        workspace_root,
        update_status,
    )
}

fn decorate_activity_dispatcher(
    dispatcher: WorkerActivityDispatcher,
    declared_bodies: crate::worker::DeclaredBodySource,
    declared_attempts: crate::worker::DeclaredCommandAttempts,
    workspace_root: crate::worker::WorkspaceRoot,
    transcript: crate::activity_publisher::ActivityEventPublisher,
    update_status: crate::update_check::UpdateStatusState,
    dev_enabled: bool,
) -> (Arc<dyn ActivityDispatcher>, Option<ActivityMockRegistry>) {
    // The declared-body layer wraps the production dispatcher UNCONDITIONALLY:
    // an action whose deployed contract declares a body executes at the
    // server, everything else falls through to the worker path untouched. The
    // update-check observer wraps THAT layer so it sees each completed
    // declared execution's result (it records completed checks and touches
    // nothing else). The dev mock (when commissioned) stays outermost so a
    // mocked activity short-circuits before either real execution path.
    let declared = crate::worker::DeclaredCommandDispatcher::new(
        Arc::new(dispatcher),
        declared_bodies.clone(),
        declared_attempts,
        tokio::runtime::Handle::current(),
        workspace_root,
        transcript,
    );
    let observed = crate::update_check::UpdateCheckObserver::new(
        Arc::new(declared),
        declared_bodies,
        update_status,
    );
    if dev_enabled {
        let registry = ActivityMockRegistry::new();
        let decorated = DevMockingDispatcher::new(Arc::new(observed), registry.clone());
        (Arc::new(decorated), Some(registry))
    } else {
        (Arc::new(observed), None)
    }
}

/// Validate the WS3 cluster broadcast capacity the server unconditionally mounts
/// (the `cluster` subscription on `/events/stream`). Explicit-no-default with the
/// same non-zero startup guard as the workflow event channel: the lag contract
/// has no buffer to lag against unless sized.
fn required_cluster_broadcast_capacity(
    runtime: &RuntimeConfig,
) -> Result<std::num::NonZeroUsize, ServerError> {
    runtime
        .websocket
        .cluster_broadcast_capacity
        .and_then(std::num::NonZeroUsize::new)
        .ok_or_else(|| ServerError::Config {
            message: crate::config::CLUSTER_BROADCAST_CAPACITY_REQUIRED.to_owned(),
        })
}

/// Build the deployment-wide real-time publishers the server mounts on every
/// boot — the WS3 cluster topology channel and the NOI-5b agent-observability
/// transcript channel — from the validated `websocket.cluster_broadcast_capacity`.
///
/// The transcript sequencer runs over `observability_store` (the durable
/// `O`-keyspace impl on a haematite boot) or an in-memory impl when the backend
/// has none — see [`build_transcript_publisher`].
///
/// # Errors
///
/// Returns [`ServerError`] when `websocket.cluster_broadcast_capacity` is unset
/// or zero (the same explicit-no-default guard the cluster channel already had).
fn build_real_time_publishers(
    runtime: &RuntimeConfig,
    observability_store: Option<Arc<dyn aion_store::ObservabilityStore>>,
) -> Result<
    (
        crate::cluster_publisher::ClusterEventPublisher,
        crate::activity_publisher::ActivityEventPublisher,
    ),
    ServerError,
> {
    let capacity = required_cluster_broadcast_capacity(runtime)?;
    let batch = required_transcript_batch_policy(runtime)?;
    Ok((
        crate::cluster_publisher::ClusterEventPublisher::new(capacity),
        build_transcript_publisher(
            observability_store,
            capacity,
            transcript_bounds(runtime),
            batch,
        ),
    ))
}

/// The operator's transcript flush policy, or a startup refusal naming the key
/// they have not ruled on.
///
/// `observability.max_batch_events` and `observability.max_batch_hold_ms` have
/// NO shipped default (see [`crate::config::ObservabilityConfig`]): the trade
/// between store cost and how much not-yet-durable transcript a crash can lose
/// belongs to the deployment, and a guessed value would make it silently. This
/// is the same explicit-no-default guard [`required_cluster_broadcast_capacity`]
/// applies one line above, for the same reason.
///
/// # Errors
///
/// Returns [`ServerError`] when either key is unset, when `max_batch_events` is
/// zero (a batch of no events would commit nothing, forever), or when the hold
/// in milliseconds does not fit a [`Duration`](std::time::Duration).
/// The operator's node-cache byte budget, or a refusal that names the key.
///
/// There is no default to fall back to — not here, and not in haematite, which
/// refuses a `DatabaseConfig` that does not carry one. A node is not a
/// fixed-size thing, so a cache bounded only by its entry count is unbounded in
/// BYTES, and how much resident memory this process may spend on cached nodes is
/// a deployment decision. `"unlimited"` is available as an explicit choice, so
/// the pre-budget behaviour is still reachable — by NAME, visible in a diff.
///
/// # Errors
///
/// Returns [`ServerError::Config`] carrying
/// [`STORE_NODE_CACHE_BUDGET_REQUIRED`](crate::config::STORE_NODE_CACHE_BUDGET_REQUIRED)
/// when `store.node_cache_budget` is unset.
fn required_node_cache_budget(
    config: &StoreConfig,
) -> Result<haematite::NodeCacheBudget, ServerError> {
    config.node_cache_budget.ok_or_else(|| ServerError::Config {
        message: crate::config::STORE_NODE_CACHE_BUDGET_REQUIRED.to_owned(),
    })
}

fn required_lock_acquisition_ruling(
    config: &StoreConfig,
) -> Result<aion_store_haematite::LockAcquisitionRuling, ServerError> {
    let absent = || ServerError::Config {
        message: crate::config::STORE_LOCK_ACQUISITION_REQUIRED.to_owned(),
    };
    let patience_ms = config.lock_acquisition_patience_ms.ok_or_else(&absent)?;
    let retry_cadence_ms = config
        .lock_acquisition_retry_cadence_ms
        .ok_or_else(absent)?;
    // Absence is this function's judgement; the SHAPE of the spans is the
    // ruling type's own invariant. Re-deciding "both positive, cadence no
    // greater than patience" here would be a second copy of one rule, free to
    // drift from the one the type enforces on every other caller.
    aion_store_haematite::LockAcquisitionRuling::from_millis(patience_ms, retry_cadence_ms).map_err(
        |error| ServerError::Config {
            message: format!(
                "{}: {error}",
                crate::config::STORE_LOCK_ACQUISITION_REQUIRED
            ),
        },
    )
}

fn required_transcript_batch_policy(
    runtime: &RuntimeConfig,
) -> Result<crate::activity_publisher::TranscriptBatchPolicy, ServerError> {
    let max_batch_events = runtime
        .observability
        .max_batch_events
        .and_then(std::num::NonZeroUsize::new)
        .ok_or_else(|| ServerError::Config {
            message: crate::config::OBSERVABILITY_MAX_BATCH_EVENTS_REQUIRED.to_owned(),
        })?;
    // Zero is a MEANINGFUL setting here (never hold a partial batch open), so
    // absence — not zero — is what fails.
    let max_batch_hold_ms =
        runtime
            .observability
            .max_batch_hold_ms
            .ok_or_else(|| ServerError::Config {
                message: crate::config::OBSERVABILITY_MAX_BATCH_HOLD_MS_REQUIRED.to_owned(),
            })?;
    Ok(crate::activity_publisher::TranscriptBatchPolicy {
        max_batch_events,
        max_hold: std::time::Duration::from_millis(max_batch_hold_ms),
    })
}

/// The operator-configured transcript retention bounds from `[observability]`.
fn transcript_bounds(runtime: &RuntimeConfig) -> crate::activity_bounds::TranscriptBounds {
    crate::activity_bounds::TranscriptBounds {
        max_event_bytes: runtime.observability.max_event_bytes,
        max_stream_events: runtime.observability.max_stream_events,
    }
}

/// Build the NOI-5b transcript sequencer over `observability_store` (the durable
/// `O`-keyspace impl when the backend has one, an in-memory impl otherwise) with
/// a live-tail buffer of `capacity` and the `[observability]` retention bounds.
///
/// The publisher is ALWAYS constructed (the transcript channel is served on every
/// boot); only the durability of the backing store varies by backend. The memory
/// backend, which has no `O` keyspace, gets the in-memory
/// [`InMemoryObservabilityStore`](aion_store::InMemoryObservabilityStore), so the
/// live-tail + resume path behaves identically and only cross-restart durability
/// differs — exactly the "keep the no-observability path uniform" contract.
fn build_transcript_publisher(
    observability_store: Option<Arc<dyn aion_store::ObservabilityStore>>,
    capacity: std::num::NonZeroUsize,
    bounds: crate::activity_bounds::TranscriptBounds,
    batch: crate::activity_publisher::TranscriptBatchPolicy,
) -> crate::activity_publisher::ActivityEventPublisher {
    let store = observability_store
        .unwrap_or_else(|| Arc::new(aion_store::InMemoryObservabilityStore::default()));
    crate::activity_publisher::ActivityEventPublisher::new(store, capacity, batch)
        .with_bounds(bounds)
}

/// The request-routing pieces built from the cluster store + peer config.
struct RoutingState {
    shard_directory: Option<Arc<crate::routing::StaticShardDirectory>>,
    request_forwarder: Option<Arc<dyn crate::routing::RequestForwarder>>,
    /// The namespace-mint routing context assembled from the two handles above
    /// plus the cluster store, so the boot path can thread it into the worker
    /// registry's minter (the second of the two minter construction sites)
    /// without re-deriving it.
    mint_routing: Option<crate::namespace::NamespaceRouting>,
}

/// Build the R-2 shard directory and R-3 request forwarder over the cluster
/// store and static peer config, or all-`None` when this is not a distributed
/// boot (no cluster store) so the routing edge is a no-op (default path).
fn build_routing_state(
    cluster_store: Option<&Arc<aion_store_haematite::HaematiteStore>>,
    directory_peers: Vec<crate::routing::DirectoryPeer>,
    self_node_id: Option<String>,
) -> RoutingState {
    let Some(store) = cluster_store else {
        return RoutingState {
            shard_directory: None,
            request_forwarder: None,
            mint_routing: None,
        };
    };
    let shard_directory = Arc::new(crate::routing::StaticShardDirectory::new(
        Arc::clone(store),
        directory_peers,
        self_node_id,
    ));
    let request_forwarder: Arc<dyn crate::routing::RequestForwarder> =
        Arc::new(crate::routing::GrpcRequestForwarder::new());
    let mint_routing = build_namespace_routing(
        Some(store),
        Some(&shard_directory),
        Some(&request_forwarder),
    );
    RoutingState {
        shard_directory: Some(shard_directory),
        request_forwarder: Some(request_forwarder),
        mint_routing,
    }
}

/// Assemble the namespace-mint routing context from the three handles a
/// distributed boot produces, or `None` when any is absent.
///
/// The single place the context is built, shared by the boot path (which wires
/// it into the worker registry's minter before the state exists) and
/// [`ServerState::namespace_routing`] (which serves the per-request minters).
/// Each handle is checked rather than assumed present: `build_routing_state`
/// populates them together, but a partial context would silently route mints to
/// nowhere.
fn build_namespace_routing(
    cluster_store: Option<&Arc<aion_store_haematite::HaematiteStore>>,
    shard_directory: Option<&Arc<crate::routing::StaticShardDirectory>>,
    request_forwarder: Option<&Arc<dyn crate::routing::RequestForwarder>>,
) -> Option<crate::namespace::NamespaceRouting> {
    use crate::namespace::{
        GrpcMintForwarder, MintForwarder, MintShardOwners, NamespaceRouting, NamespaceShardResolver,
    };
    let store = Arc::clone(cluster_store?);
    let directory = Arc::clone(shard_directory?);
    let shards: Arc<dyn NamespaceShardResolver> = store;
    let owners: Arc<dyn MintShardOwners> = directory;
    let forwarder: Arc<dyn MintForwarder> =
        Arc::new(GrpcMintForwarder::new(Arc::clone(request_forwarder?)));
    Some(NamespaceRouting::new(shards, owners, forwarder))
}

/// A connected durable store plus the lifecycle pieces the boot path needs.
///
/// `outbox_store` is the SAME leaf store cast as an [`OutboxStore`] on the
/// haematite backend, which is the one that has a durable outbox; the in-memory
/// backend yields `None`. `bootstrap_coordinator` gates the schedule-coordinator seed on real
/// ownership (SS-2 / AA-4-4): `true` for every non-distributed boot (single-node
/// owns the coordinator's shard), and for a distributed node only when it owns
/// that shard. `cluster_responder` owns the distributed inbound-write responder
/// thread, kept alive for the server's lifetime; `None` for non-distributed boots.
struct ConnectedStore {
    event_store: Arc<dyn EventStore>,
    outbox_store: Option<Arc<dyn OutboxStore>>,
    /// The SAME concrete leaf store as `event_store`, captured as a
    /// [`NamespaceStore`] before the decorator chain wraps it (the decorators
    /// are `NamespaceStore`-unaware). The control plane mints and lists through
    /// this handle. Every backend populates it: distributed haematite supplies
    /// the quorum-replicated implementation, single-node haematite and the
    /// in-memory store the local-only one.
    namespace_store: Arc<dyn NamespaceStore>,
    /// Same concrete leaf captured as the deployment-store contract.
    worker_deployment_store: Arc<dyn WorkerDeploymentStore>,
    /// NOI-5b: the SAME concrete leaf store captured as an
    /// [`ObservabilityStore`](aion_store::ObservabilityStore) when the backend
    /// implements the durable `O` keyspace (haematite). `None` for backends with
    /// no `O` keyspace (haematite / in-memory), where the transcript sequencer runs
    /// over an in-memory impl instead. Captured before the leaf is wrapped in the
    /// (`ObservabilityStore`-unaware) decorator chain, exactly like
    /// `namespace_store`.
    observability_store: Option<Arc<dyn aion_store::ObservabilityStore>>,
    bootstrap_coordinator: bool,
    cluster_responder: Option<aion_store_haematite::ClusterResponder>,
    /// The concrete distributed haematite store (the SAME leaf as `event_store`),
    /// retained for the SS-5b cluster supervisor's peer-liveness polling. `None`
    /// for every non-distributed boot.
    cluster_store: Option<Arc<aion_store_haematite::HaematiteStore>>,
    /// The peers the SS-5b supervisor watches, each with the shards this node
    /// adopts on its death. Empty for non-distributed boots.
    watched_peers: Vec<crate::cluster::WatchedPeer>,
    /// The static shard-directory peer entries (name + declared shards + gRPC
    /// forward address) used to build the request-routing directory (R-2). Empty
    /// for non-distributed boots.
    directory_peers: Vec<crate::routing::DirectoryPeer>,
    /// This node's own distribution name (cluster `node_id`), so the SS-3
    /// directory can resolve a shard-owner record naming THIS node to `Local`.
    /// `None` for non-distributed boots.
    self_node_id: Option<String>,
}

impl ConnectedStore {
    /// A non-distributed connected store: owns the coordinator's shard (so it
    /// bootstraps the coordinator) and has no cluster responder.
    ///
    /// `namespace_store` is the SAME concrete leaf as `event_store`, captured as
    /// a [`NamespaceStore`] by the caller (where the concrete type is still
    /// known) before the decorator chain wraps the event store.
    fn local(
        event_store: Arc<dyn EventStore>,
        outbox_store: Option<Arc<dyn OutboxStore>>,
        namespace_store: Arc<dyn NamespaceStore>,
        worker_deployment_store: Arc<dyn WorkerDeploymentStore>,
    ) -> Self {
        Self {
            event_store,
            outbox_store,
            namespace_store,
            worker_deployment_store,
            // `local` is the memory backend and the embedder's own-store path.
            // A haematite boot never arrives here — `connect_store` routes it to
            // `connect_haematite_store`, which sets `observability_store`. So a
            // `local` store has no durable `O` keyspace and the transcript
            // sequencer falls back to an in-memory impl (NOI-5b).
            observability_store: None,
            bootstrap_coordinator: true,
            cluster_responder: None,
            cluster_store: None,
            watched_peers: Vec::new(),
            directory_peers: Vec::new(),
            self_node_id: None,
        }
    }

    /// Capture this node's self-identity for the cluster snapshot before the
    /// distributed boot moves it into routing state.
    fn cluster_self_node(&self) -> Option<String> {
        self.self_node_id.clone()
    }
}

/// Connect the selected store. Haematite supplies both durable events and the
/// durable outbox; the in-memory development backend has no outbox.
async fn connect_store(config: StoreConfig) -> Result<ConnectedStore, ServerError> {
    match config.backend {
        StoreBackend::Memory => {
            // One leaf store, captured as both the engine's event store and the
            // namespace registry (in-memory backends have no outbox table).
            let leaf = Arc::new(aion_store::InMemoryStore::default());
            let namespace_store: Arc<dyn NamespaceStore> = leaf.clone();
            let worker_deployment_store: Arc<dyn WorkerDeploymentStore> = leaf.clone();
            Ok(ConnectedStore::local(
                leaf,
                None,
                namespace_store,
                worker_deployment_store,
            ))
        }
        StoreBackend::Haematite => connect_haematite_store(config).await,
    }
}

/// Connect the haematite backend, opening the on-disk database if `store.data_dir`
/// already holds one and otherwise creating it with `store.shard_count` shards.
///
/// Without a `[store.cluster]` section this is the SINGLE-NODE path
/// ([`HaematiteStore::open`] / [`create_with_shard_count`]), byte-identical to
/// before: no endpoint, no election, owns everything, bootstraps the coordinator.
/// With a cluster section this is the DISTRIBUTED path
/// ([`HaematiteStore::open_or_create_distributed`]): it binds the replication
/// endpoint, builds the quorum membership, dials peers, starts the responder, and
/// computes whether THIS node owns the schedule-coordinator's shard so the engine
/// boot path seeds the coordinator on exactly one owner cluster-wide (SS-2).
///
/// The SAME leaf `Arc<HaematiteStore>` is shared as both the engine's
/// [`EventStore`] and the dispatcher's [`OutboxStore`] (one inner haematite
/// database), mirroring the haematite backend.
///
/// [`HaematiteStore::open`]: aion_store_haematite::HaematiteStore::open
/// [`create_with_shard_count`]: aion_store_haematite::HaematiteStore::create_with_shard_count
/// [`HaematiteStore::open_or_create_distributed`]: aion_store_haematite::HaematiteStore::open_or_create_distributed
async fn connect_haematite_store(config: StoreConfig) -> Result<ConnectedStore, ServerError> {
    // Required HERE, at the only seam that consumes it: haematite is the only
    // backend with a node cache, so a `memory` deployment is never asked to
    // rule on a cache it does not have. Same explicit-no-default guard
    // `observability.max_batch_events` uses, at the same kind of seam. Read
    // before `config` is partially moved below.
    let node_cache_budget = required_node_cache_budget(&config)?;
    let lock_acquisition = required_lock_acquisition_ruling(&config)?;
    let Some(data_dir) = config.data_dir else {
        return Err(ServerError::Config {
            message: "store.data_dir must not be empty when store.backend is haematite".to_owned(),
        });
    };
    let shard_count = config.shard_count;
    let owned_shards = config.owned_shards.clone();
    let cluster = config.cluster.clone();
    // The peers the SS-5b supervisor watches, captured before `cluster` is moved
    // into the blocking build. A peer with declared `owned_shards` becomes a
    // watch target; peers without are kept out of the watch set (the supervisor
    // would have nothing to adopt for them).
    let watched_peers: Vec<crate::cluster::WatchedPeer> = cluster
        .as_ref()
        .map(|cluster| {
            cluster
                .peers
                .iter()
                .map(|peer| crate::cluster::WatchedPeer {
                    name: peer.name.clone(),
                    owned_shards: peer.owned_shards.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    // The static shard-directory entries (R-2): each peer's declared shards plus
    // its gRPC forward address. Built from the same config the supervisor uses.
    let directory_peers: Vec<crate::routing::DirectoryPeer> = cluster
        .as_ref()
        .map(|cluster| {
            cluster
                .peers
                .iter()
                .map(|peer| crate::routing::DirectoryPeer {
                    name: peer.name.clone(),
                    owned_shards: peer.owned_shards.clone(),
                    grpc_addr: peer.grpc_address,
                })
                .collect()
        })
        .unwrap_or_default();
    // This node's own distribution name, so the SS-3 directory resolves a
    // shard-owner record naming THIS node to `Local`.
    let self_node_id: Option<String> = cluster.as_ref().map(|cluster| cluster.node_id.clone());
    // Construction (and, for the distributed path, the off-runtime endpoint bind)
    // must not stall the async runtime, so run it on the blocking pool. The
    // distributed constructor itself steps onto a bare thread for the bind.
    let (store, responder) = tokio::task::spawn_blocking(move || {
        build_haematite_store(
            &data_dir,
            shard_count,
            cluster,
            node_cache_budget,
            lock_acquisition,
        )
    })
    .await
    .map_err(|error| ServerError::Config {
        message: format!("haematite store initialization task failed: {error}"),
    })??;

    // Gate the coordinator bootstrap on real ownership: a distributed node that
    // does NOT own the coordinator's shard must not seed/fence it (AA-4-4). A
    // single-node boot owns all shards, so it always bootstraps.
    let bootstrap_coordinator = if owned_shards.is_empty() {
        true
    } else {
        store.set_owned_shards(owned_shards.iter().copied());
        store.owns_workflow_shard(&aion::schedule_coordinator_workflow_id())
    };

    let leaf = Arc::new(store);
    let event_store: Arc<dyn EventStore> = leaf.clone();
    let outbox_store: Arc<dyn OutboxStore> = leaf.clone();
    // The namespace registry is the SAME concrete `HaematiteStore` leaf (the
    // quorum-replicated implementation), captured before the leaf is moved into
    // the cluster-store retention below.
    let namespace_store: Arc<dyn NamespaceStore> = leaf.clone();
    let worker_deployment_store: Arc<dyn WorkerDeploymentStore> = leaf.clone();
    // NOI-5b: the SAME concrete leaf captured as the durable `O`-keyspace
    // observability store, so the transcript sequencer persists to haematite and
    // survives restart/failover. Captured here (before the decorator chain wraps
    // the event store) exactly like the namespace registry.
    let observability_store: Arc<dyn aion_store::ObservabilityStore> = leaf.clone();
    // Retain the concrete store ONLY for a distributed boot (responder present),
    // where the SS-5b supervisor will poll it for peer liveness. A single-node
    // boot has no peers, so it carries no cluster store and never supervises.
    let cluster_store = responder.as_ref().map(|_| leaf);
    let (watched_peers, directory_peers, self_node_id) = if cluster_store.is_some() {
        (watched_peers, directory_peers, self_node_id)
    } else {
        (Vec::new(), Vec::new(), None)
    };
    Ok(ConnectedStore {
        event_store,
        outbox_store: Some(outbox_store),
        namespace_store,
        worker_deployment_store,
        observability_store: Some(observability_store),
        bootstrap_coordinator,
        cluster_responder: responder,
        cluster_store,
        watched_peers,
        directory_peers,
        self_node_id,
    })
}

/// Build the haematite store: the distributed path when a cluster section is
/// present, otherwise the single-node path. Returns the store and (for the
/// distributed path) its inbound-write responder. Restart-safe: an existing
/// on-disk database is reused (its shard count wins) rather than re-created.
///
/// Linux/Android give Haematite a descriptor-authoritative `/proc/self/fd` path.
/// On path-ambient Unix targets such as macOS, startup instead resolves the held
/// descriptor's current path and refuses any ancestor owned by an unprivileged
/// principal other than the server euid or writable by group/world. That policy
/// prevents a second principal from renaming a parent after startup and replacing
/// the old name with a symlink that redirects Haematite's normal reads/commits.
/// Every shard is still eagerly materialized and the capability retained, but on
/// those targets neither action confines later pathname I/O. A descriptor-relative
/// Haematite constructor and backend I/O remain the long-term fix.
fn build_haematite_store(
    data_dir: &str,
    shard_count: usize,
    cluster: Option<crate::config::ClusterConfig>,
    node_cache_budget: haematite::NodeCacheBudget,
    lock_acquisition: aion_store_haematite::LockAcquisitionRuling,
) -> Result<
    (
        aion_store_haematite::HaematiteStore,
        Option<aion_store_haematite::ClusterResponder>,
    ),
    ServerError,
> {
    build_haematite_store_with_hook(
        data_dir,
        shard_count,
        cluster,
        node_cache_budget,
        lock_acquisition,
        || Ok(()),
    )
}

fn build_haematite_store_with_hook(
    data_dir: &str,
    shard_count: usize,
    cluster: Option<crate::config::ClusterConfig>,
    node_cache_budget: haematite::NodeCacheBudget,
    lock_acquisition: aion_store_haematite::LockAcquisitionRuling,
    before_backend_touch: impl FnOnce() -> Result<(), std::io::Error>,
) -> Result<
    (
        aion_store_haematite::HaematiteStore,
        Option<aion_store_haematite::ClusterResponder>,
    ),
    ServerError,
> {
    use aion_store_haematite::{ClusterBootstrap, HaematiteStore};

    // Acquire the data root through the same no-follow component walk used by
    // authoring. New components are created 0700 on Unix, and an existing root
    // that the server's own user owns is tightened to 0700 rather than refused —
    // provisioning our own directory is Aion's job, not the operator's. Only a
    // root Aion cannot make safe (foreign owner, a filesystem without Unix
    // modes) is a loud startup failure here; an unsafe ANCESTOR is caught
    // separately below and is never repaired.
    let private_root = crate::filesystem::ConfinedDir::open_or_create(std::path::Path::new(
        data_dir,
    ))
    .map_err(|error| ServerError::Config {
        message: format!("unsafe store.data_dir `{data_dir}`: {error}"),
    })?;

    // Haematite 0.5 creates shard directories lazily. Pre-create every configured
    // directory descriptor-relatively, then force the backend's actual shard
    // spawn/recovery path below while this checked-and-hardened window is held.
    for shard in 0..shard_count {
        private_root
            .create_dir_all(std::path::Path::new(&format!("shard-{shard}")))
            .map_err(|error| ServerError::Config {
                message: format!(
                    "failed to materialize shard-{shard} under store.data_dir `{data_dir}`: {error}"
                ),
            })?;
    }
    private_root
        .harden_tree()
        .map_err(|error| private_store_mode_error(data_dir, &error))?;

    // Deterministic regression seam: the capability and shard directories exist,
    // but Haematite has not touched any path yet.
    before_backend_touch().map_err(|error| ServerError::Config {
        message: format!("store.data_dir pre-open hook failed: {error}"),
    })?;

    #[cfg(unix)]
    let backend_path = private_root
        .backend_path()
        .map_err(|error| ServerError::Config {
            message: format!("failed to resolve held store.data_dir `{data_dir}`: {error}"),
        })?;
    #[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
    crate::filesystem::validate_ambient_backend_ancestors(&backend_path).map_err(|error| {
        let (component, reason) = error.into_parts();
        ServerError::UnsafeDataRootAncestor {
            data_root: backend_path.clone(),
            component,
            reason,
        }
    })?;
    #[cfg(not(unix))]
    let backend_path = std::path::PathBuf::from(data_dir);

    let Some(cluster) = cluster else {
        let store = if backend_path.join("config.json").exists() {
            // The configured budget rides along as the migration ruling for a
            // store that predates the field (aion#75: an upgrade must never
            // present as data loss); a store that already rules keeps its
            // recorded ruling and this value is ignored.
            HaematiteStore::open(&backend_path, node_cache_budget, lock_acquisition)
                .map_err(ServerError::from)?
        } else {
            HaematiteStore::create_with_shard_count(&backend_path, shard_count, node_cache_budget)
                .map_err(ServerError::from)?
        };
        store.materialize_all_shards().map_err(ServerError::from)?;
        private_root
            .harden_tree()
            .map_err(|error| private_store_mode_error(data_dir, &error))?;
        let store = store.retain_data_root_capability(private_root);
        return Ok((store, None));
    };

    let boot = ClusterBootstrap {
        node_id: cluster.node_id,
        bind_address: cluster.bind_address,
        members: cluster.members,
        peers: cluster
            .peers
            .into_iter()
            .map(|peer| (peer.name, peer.address))
            .collect(),
        timeout: HAEMATITE_CLUSTER_OP_TIMEOUT,
    };
    let (store, responder) = HaematiteStore::open_or_create_distributed(
        &backend_path,
        shard_count,
        boot,
        node_cache_budget,
    )
    .map_err(ServerError::from)?;
    store.materialize_all_shards().map_err(ServerError::from)?;
    private_root
        .harden_tree()
        .map_err(|error| private_store_mode_error(data_dir, &error))?;
    let store = store.retain_data_root_capability(private_root);
    Ok((store, Some(responder)))
}

fn private_store_mode_error(data_dir: &str, error: &std::io::Error) -> ServerError {
    ServerError::Config {
        message: format!(
            "failed to apply private modes under store.data_dir `{data_dir}`: {error}"
        ),
    }
}

/// Per-operation quorum/election timeout for the distributed haematite backend.
const HAEMATITE_CLUSTER_OP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// The NOI-6 intervention transport used when no push transport is compiled in.
///
/// Without the `liminal-transport` feature there is no way to reach a worker's
/// out-of-band connection, so every routed command reports the owning worker
/// unreachable — which the router maps onto the attempt-scoped stale-target no-op.
/// This keeps the intervention endpoint honest on a transport-less build (an
/// operator gets a NACK, never a false "applied") without gating the endpoint on a
/// feature.
#[cfg(not(feature = "liminal-transport"))]
#[derive(Clone, Debug)]
struct NullInterventionTransport;

#[cfg(not(feature = "liminal-transport"))]
#[async_trait::async_trait]
impl crate::worker::InterventionTransport for NullInterventionTransport {
    async fn push(
        &self,
        _worker: &crate::worker::WorkerHandle,
        _command: aion_core::InterventionCommand,
    ) -> Result<aion_core::InterventionOutcome, ServerError> {
        Err(ServerError::worker_connection_lost(
            "intervention",
            "no intervention push transport is compiled in".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, time::Duration};

    use aion_store::InMemoryStore;

    use super::ServerState;
    use crate::config::{
        AuthConfig, AuthoringConfig, DeployConfig, DevConfig, ListenConfig, MetricsConfig,
        NamespaceConfig, NamespaceMode, OpsConsoleAssetSource, OpsConsoleConfig, OutboxConfig,
        RuntimeConfig, WebSocketConfig, WorkerConfig,
    };

    fn runtime_config() -> RuntimeConfig {
        RuntimeConfig {
            listen: ListenConfig {
                grpc: SocketAddr::from(([127, 0, 0, 1], 50051)),
                http: SocketAddr::from(([127, 0, 0, 1], 8080)),
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
            dev: DevConfig::default(),
            outbox: OutboxConfig::default(),
            observability: crate::config::ObservabilityConfig::with_flush_policy(64, 0),
            mcp: crate::config::ResolvedMcpConfig::default(),
            scheduler_threads: 1,
            jit_threshold: None,
            query_timeout: Some(Duration::from_secs(10)),
            default_namespace: "default".to_owned(),
            auto_create: crate::config::AutoCreate::Open,
            max_in_flight_activities: crate::config::DEFAULT_MAX_IN_FLIGHT_ACTIVITIES,
            drain_timeout: Duration::from_secs(30),
            metrics: MetricsConfig { enabled: true },
            owned_shards: Vec::new(),
            cors_allowed_origins: Vec::new(),
        }
    }

    /// The flush policy is REQUIRED and has no default: a runtime that has not
    /// ruled on it refuses to build the transcript publisher, and the refusal
    /// names the key the operator must set. This is the loud-at-startup half of
    /// "no invented tuning values".
    #[test]
    fn an_unruled_flush_policy_refuses_to_build_the_publisher() {
        for (mutate, expected_key) in [
            (
                Box::new(|runtime: &mut RuntimeConfig| {
                    runtime.observability.max_batch_events = None;
                }) as Box<dyn Fn(&mut RuntimeConfig)>,
                "observability.max_batch_events",
            ),
            (
                Box::new(|runtime: &mut RuntimeConfig| {
                    runtime.observability.max_batch_events = Some(0);
                }),
                "observability.max_batch_events",
            ),
            (
                Box::new(|runtime: &mut RuntimeConfig| {
                    runtime.observability.max_batch_hold_ms = None;
                }),
                "observability.max_batch_hold_ms",
            ),
        ] {
            let mut runtime = runtime_config();
            mutate(&mut runtime);
            let message = super::required_transcript_batch_policy(&runtime)
                .err()
                .map_or_else(String::new, |error| error.to_string());
            assert!(
                message.contains(expected_key),
                "the refusal must name {expected_key}: {message}"
            );
            assert!(
                message.contains("no default"),
                "and must say the key has no default: {message}"
            );
        }
    }

    /// A stated policy is carried through verbatim — including a hold of ZERO,
    /// which is a real ruling ("never wait"), not a missing one.
    #[test]
    fn a_stated_flush_policy_is_carried_through_verbatim() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut runtime = runtime_config();
        runtime.observability = crate::config::ObservabilityConfig::with_flush_policy(32, 0);
        let policy = super::required_transcript_batch_policy(&runtime)?;
        assert_eq!(policy.max_batch_events.get(), 32);
        assert_eq!(policy.max_hold, Duration::ZERO);

        runtime.observability = crate::config::ObservabilityConfig::with_flush_policy(8, 250);
        let policy = super::required_transcript_batch_policy(&runtime)?;
        assert_eq!(policy.max_batch_events.get(), 8);
        assert_eq!(policy.max_hold, Duration::from_millis(250));
        Ok(())
    }

    /// The engine's schema must accept EVERY attribute the server's start
    /// writer actually records — the invariant, not an enumeration of names.
    ///
    /// The two sides are genuinely coupled at runtime: the recorder validates
    /// each attribute against this schema before appending, so an attribute the
    /// writer produces and the schema does not register fails the START, not
    /// just the label (#211). The map is taken from the production
    /// `start_search_attributes` with every optional field populated, so any
    /// future attribute the writer learns to record is covered here without
    /// this test being edited.
    #[test]
    fn engine_schema_accepts_every_attribute_the_start_writer_records()
    -> Result<(), Box<dyn std::error::Error>> {
        let schema = super::server_search_attribute_schema()?;
        let recorded = crate::api::handlers::workflows::start_search_attributes(
            "tenant-a",
            Some("gpu"),
            Some("Nightly settlement"),
        );

        assert!(
            recorded.contains_key(crate::namespace::DISPLAY_NAME_ATTRIBUTE),
            "the fixture must exercise the display-name attribute, or this test \
             cannot see its registration go missing"
        );
        for (name, value) in &recorded {
            schema.validate(name, value).map_err(|error| {
                format!(
                    "the start writer records {name}, but the engine's schema refuses it: {error}"
                )
            })?;
        }
        Ok(())
    }

    #[tokio::test]
    async fn builds_state_with_in_memory_store() -> Result<(), Box<dyn std::error::Error>> {
        let state =
            ServerState::build_with_store(InMemoryStore::default(), runtime_config()).await?;

        std::hint::black_box(state.namespace_guard());
        std::hint::black_box(state.worker_registry());

        Ok(())
    }

    /// R1 surfacing: a real boot exposes the unserved-queue state, the bridge
    /// publishes parked dispatches into THAT instance, and the address leaves
    /// the state the moment the dispatch resolves.
    ///
    /// The dispatch is driven through a dispatcher built over the state's OWN
    /// registry, queue state, and engine-backed declaration source — the same
    /// three handles `build_bridge_dispatcher` hands the production bridge.
    #[tokio::test]
    async fn unserved_queues_surfaces_a_parked_dispatch_and_clears_it()
    -> Result<(), Box<dyn std::error::Error>> {
        use aion::{ActivityDispatch, ActivityDispatcher as _};
        use aion_core::{ActivityId, RunId, WorkflowId};
        use std::sync::Arc;

        let state =
            ServerState::build_with_store(InMemoryStore::default(), runtime_config()).await?;
        assert!(
            state.unserved_queues()?.is_empty(),
            "a calm boot has no unserved queues"
        );
        // The engine-backed reader IS installed on a real boot; with no
        // queue-declaring package deployed it can contradict nothing, so it must
        // answer Unknown rather than manufacture a structural refusal.
        assert!(state.queue_declarations().is_installed());
        assert_eq!(
            state
                .queue_declarations()
                .declaration_for("nobody-serves-this"),
            crate::worker::QueueDeclaration::Unknown
        );

        let dispatcher = Arc::new(
            crate::worker::WorkerActivityDispatcher::new(
                state.worker_registry().clone(),
                "default",
                crate::worker::HeartbeatTracker::new(Duration::from_secs(5)),
            )
            .with_queue_state(state.queue_service_state().clone())
            .with_queue_declarations(state.queue_declarations().clone()),
        );
        let workflow_id = WorkflowId::new_v4();
        let request = ActivityDispatch {
            namespace: "default".to_owned(),
            task_queue: "nobody-serves-this".to_owned(),
            node: None,
            workflow_id: workflow_id.clone(),
            run_id: RunId::new_v4(),
            activity_id: ActivityId::from_sequence_position(0),
            name: "greet".to_owned(),
            input: "{}".to_owned(),
            config: "{}".to_owned(),
            attempt: 1,
            advisory: false,
            labels: std::collections::BTreeMap::new(),
        };
        let parked = std::thread::spawn(move || dispatcher.dispatch(request));

        let mut unserved = Vec::new();
        for _ in 0..30 {
            unserved = state.unserved_queues()?;
            if !unserved.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert_eq!(unserved.len(), 1, "the parked dispatch is not surfaced");
        assert_eq!(
            unserved[0].reason,
            crate::worker::QueueServiceReason::NoLivePollers,
            "an empty catalog must not be read as a structural refusal"
        );
        assert_eq!(unserved[0].key.task_queue, "nobody-serves-this");
        assert_eq!(unserved[0].waiting.len(), 1);
        assert_eq!(unserved[0].waiting[0].workflow_id, workflow_id);

        // Release the dispatch: a worker arrives whose receiver is already gone.
        let (worker_tx, worker_rx) = tokio::sync::mpsc::channel(1);
        drop(worker_rx);
        let registration = state.worker_registry().register_namespaces(
            [String::from("default")],
            "nobody-serves-this",
            None,
            [String::from("greet")].iter(),
            worker_tx,
        )?;
        let outcome = parked.join().map_err(|_| "parked dispatch panicked")?;
        assert!(outcome.is_err(), "the released dispatch must resolve");
        assert!(
            state.unserved_queues()?.is_empty(),
            "a resolved dispatch must leave the unserved state"
        );
        registration.deregister()?;
        Ok(())
    }

    #[tokio::test]
    async fn namespace_store_is_reachable_and_functional_after_default_boot()
    -> Result<(), Box<dyn std::error::Error>> {
        use aion_store::{MintOutcome, NamespaceOrigin};

        // A default single-node (in-memory) boot must expose a real, functional
        // namespace registry through `state.namespace_store()` — the control
        // plane's mint (S5) and `GET /namespaces` (S7) reach the store this way.
        let state =
            ServerState::build_with_store(InMemoryStore::default(), runtime_config()).await?;

        let store = state.namespace_store();

        // Mint a fresh namespace: the first reference creates it.
        let outcome = store
            .register_namespace("orders", NamespaceOrigin::WorkerMint)
            .await?;
        assert_eq!(
            outcome,
            MintOutcome::Created,
            "the first reference to a namespace mints it"
        );

        // Re-referencing is idempotent: the record already exists.
        let again = store
            .register_namespace("orders", NamespaceOrigin::WorkerMint)
            .await?;
        assert_eq!(
            again,
            MintOutcome::AlreadyExisted,
            "a second reference touches the existing record rather than re-creating it"
        );

        // Single lookup returns the durable record.
        let fetched = store.get_namespace("orders").await?;
        let record = fetched.ok_or("registered namespace must be retrievable via get_namespace")?;
        assert_eq!(record.name, "orders");
        assert_eq!(record.origin, NamespaceOrigin::WorkerMint);

        // The live set lists the namespace.
        let listed = store.list_namespaces().await?;
        assert!(
            listed.iter().any(|record| record.name == "orders"),
            "list_namespaces returns the minted namespace"
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn connect_store_haematite_round_trips_through_event_store()
    -> Result<(), Box<dyn std::error::Error>> {
        use aion_core::{ContentType, EventEnvelope, PackageVersion, Payload, RunId, WorkflowId};
        use aion_store::WriteToken;
        use chrono::Utc;

        use crate::config::{StoreBackend, StoreConfig};

        let data_dir = crate::test_support::private_tempdir()?;
        // Single shard, a fresh temp data_dir: the production connect path opens
        // an existing haematite database or creates one, then shares the leaf as
        // both the engine EventStore and the dispatcher OutboxStore.
        let connected = super::connect_store(StoreConfig {
            backend: StoreBackend::Haematite,
            owned_shards: Vec::new(),
            data_dir: Some(data_dir.path().to_string_lossy().into_owned()),
            shard_count: 1,
            cluster: None,
            node_cache_budget: Some(test_node_cache_budget()?),
            lock_acquisition_patience_ms: Some(250),
            lock_acquisition_retry_cadence_ms: Some(5),
            ..StoreConfig::default()
        })
        .await?;
        let event_store = connected.event_store;
        assert!(
            connected.outbox_store.is_some(),
            "the haematite backend shares its leaf store as the dispatcher's outbox store"
        );
        assert!(
            connected.bootstrap_coordinator,
            "a single-node haematite boot owns all shards and bootstraps the coordinator"
        );
        assert!(
            connected.cluster_responder.is_none(),
            "a single-node (no [cluster]) haematite boot has no distributed responder"
        );

        let workflow_id = WorkflowId::new_v4();
        let event = aion_core::Event::WorkflowStarted {
            envelope: EventEnvelope {
                seq: 1,
                recorded_at: Utc::now(),
                workflow_id: workflow_id.clone(),
            },
            workflow_type: String::from("checkout"),
            input: Payload::new(ContentType::Json, b"{}".to_vec()),
            run_id: RunId::new_v4(),
            parent_run_id: None,
            parent_workflow_id: None,
            package_version: PackageVersion::new("a".repeat(64)),
        };
        event_store
            .append(
                WriteToken::recorder(),
                &workflow_id,
                std::slice::from_ref(&event),
                0,
            )
            .await?;
        let history = event_store.read_history(&workflow_id).await?;
        assert_eq!(
            history.len(),
            1,
            "an event appended through the server's dyn EventStore reads back"
        );
        Ok(())
    }

    /// A generous node-cache byte budget (1 GiB) for the haematite fixtures.
    ///
    /// Roomy enough that no fixture here can reach it, so these stay boot-path
    /// and data-root tests rather than accidental cache-eviction tests. It is a
    /// TEST value, not a default: production reads the operator's ruling.
    fn test_node_cache_budget() -> Result<haematite::NodeCacheBudget, Box<dyn std::error::Error>> {
        Ok(haematite::NodeCacheBudget::bytes(1 << 30)?)
    }

    fn test_lock_acquisition()
    -> Result<aion_store_haematite::LockAcquisitionRuling, Box<dyn std::error::Error>> {
        // Explicit test ruling for the measured 93–150 ms inherited-descriptor
        // window; this bounds real lock attempts and is not a hidden sleep. It
        // is a TEST value, not a default: production reads the operator's
        // ruling, and the type refuses spans that bound no waited retry.
        Ok(aion_store_haematite::LockAcquisitionRuling::from_millis(
            250, 5,
        )?)
    }

    #[cfg(unix)]
    #[test]
    fn haematite_root_swap_before_first_backend_touch_cannot_redirect_writes()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let sandbox = crate::test_support::private_tempdir()?;
        let configured_root = sandbox.path().join("data");
        let held_root = sandbox.path().join("held-data");
        let outside = sandbox.path().join("outside");
        std::fs::create_dir(&outside)?;
        let configured = configured_root
            .to_str()
            .ok_or("temporary data path was not UTF-8")?;

        let (store, responder) = super::build_haematite_store_with_hook(
            configured,
            4,
            None,
            test_node_cache_budget()?,
            test_lock_acquisition()?,
            || {
                // The server has acquired and hardened `configured_root`, but
                // Haematite has not opened or created anything. Replace the
                // ambient name with an attacker-controlled symlink at exactly
                // the old check/use boundary.
                std::fs::rename(&configured_root, &held_root)?;
                symlink(&outside, &configured_root)?;
                Ok(())
            },
        )?;
        assert!(responder.is_none());

        let outside_entries = std::fs::read_dir(&outside)?.collect::<Result<Vec<_>, _>>()?;
        assert!(
            outside_entries.is_empty(),
            "Haematite followed the replaced ambient root and wrote outside"
        );
        assert!(held_root.join("config.json").is_file());
        for shard in 0..4 {
            let shard_path = held_root.join(format!("shard-{shard}"));
            assert!(shard_path.is_dir(), "shard {shard} was not materialized");
            assert!(
                std::fs::read_dir(&shard_path)?
                    .next()
                    .transpose()?
                    .is_some(),
                "shard {shard} did not run Haematite's materialization path"
            );
        }

        drop(store);
        Ok(())
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[tokio::test]
    async fn proc_fd_backend_path_survives_a_post_startup_root_swap()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        use aion_core::{ContentType, EventEnvelope, PackageVersion, Payload, RunId, WorkflowId};
        use aion_store::{WritableEventStore as _, WriteToken};
        use chrono::Utc;

        let sandbox = crate::test_support::private_tempdir()?;
        let configured_root = sandbox.path().join("data");
        let held_root = sandbox.path().join("held-data");
        let capture = sandbox.path().join("capture");
        std::fs::create_dir(&capture)?;
        let configured = configured_root
            .to_str()
            .ok_or("temporary data path was not UTF-8")?;

        let (store, responder) = super::build_haematite_store(
            configured,
            4,
            None,
            test_node_cache_budget()?,
            test_lock_acquisition()?,
        )?;
        assert!(responder.is_none());
        std::fs::rename(&configured_root, &held_root)?;
        symlink(&capture, &configured_root)?;

        let workflow_id = WorkflowId::new_v4();
        let event = aion_core::Event::WorkflowStarted {
            envelope: EventEnvelope {
                seq: 1,
                recorded_at: Utc::now(),
                workflow_id: workflow_id.clone(),
            },
            workflow_type: String::from("post-startup-root-swap"),
            input: Payload::new(ContentType::Json, b"{}".to_vec()),
            run_id: RunId::new_v4(),
            parent_run_id: None,
            parent_workflow_id: None,
            package_version: PackageVersion::new("a".repeat(64)),
        };
        store
            .append(
                WriteToken::recorder(),
                &workflow_id,
                std::slice::from_ref(&event),
                0,
            )
            .await?;

        let captured = std::fs::read_dir(&capture)?.collect::<Result<Vec<_>, _>>()?;
        assert!(
            captured.is_empty(),
            "post-startup append followed the replacement symlink into capture"
        );
        assert!(held_root.join("config.json").is_file());
        drop(store);
        Ok(())
    }

    #[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
    #[test]
    fn path_ambient_haematite_refuses_group_or_world_writable_ancestors()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt as _;

        let sandbox = crate::test_support::private_tempdir()?;
        std::fs::set_permissions(sandbox.path(), std::fs::Permissions::from_mode(0o700))?;

        for mode in [0o770, 0o1777] {
            let shared = sandbox.path().join(format!("shared-{mode:o}"));
            let data_root = shared.join("data");
            std::fs::create_dir(&shared)?;
            std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(mode))?;
            std::fs::create_dir(&data_root)?;
            std::fs::set_permissions(&data_root, std::fs::Permissions::from_mode(0o700))?;
            let configured = data_root
                .to_str()
                .ok_or("temporary data path was not UTF-8")?;

            let Err(error) = super::build_haematite_store(
                configured,
                4,
                None,
                test_node_cache_budget()?,
                test_lock_acquisition()?,
            ) else {
                return Err(format!("mode {mode:04o} ancestor was accepted").into());
            };
            let message = error.to_string();
            let crate::ServerError::UnsafeDataRootAncestor {
                data_root: resolved_root,
                component,
                reason,
            } = error
            else {
                return Err(format!("expected typed unsafe-ancestor error, got {message}").into());
            };
            assert_eq!(resolved_root, std::fs::canonicalize(&data_root)?);
            assert_eq!(component, std::fs::canonicalize(&shared)?);
            assert!(
                reason.contains(&format!("mode {mode:04o}")),
                "unexpected reason: {reason}"
            );
            if mode & 0o1000 != 0 {
                assert!(reason.contains("sticky bit is not accepted"));
            }
            assert!(message.contains("private Aion home"));
            assert!(
                !data_root.join("config.json").exists(),
                "Haematite touched its ambient path before the refusal"
            );
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn path_ambient_haematite_refuses_mutating_allow_acl_ancestor()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt as _;

        let sandbox = crate::test_support::private_tempdir()?;
        std::fs::set_permissions(sandbox.path(), std::fs::Permissions::from_mode(0o700))?;
        let shared = sandbox.path().join("acl-shared");
        let data_root = shared.join("data");
        std::fs::create_dir(&shared)?;
        std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o700))?;
        let acl = "everyone allow list,search,add_file,add_subdirectory,delete_child";
        let status = std::process::Command::new("chmod")
            .arg("+a")
            .arg(acl)
            .arg(&shared)
            .status()?;
        assert!(status.success(), "failed to install Darwin regression ACL");
        let configured = data_root
            .to_str()
            .ok_or("temporary data path was not UTF-8")?;

        let result = super::build_haematite_store(
            configured,
            4,
            None,
            test_node_cache_budget()?,
            test_lock_acquisition()?,
        );
        let cleanup = std::process::Command::new("chmod")
            .arg("-RN")
            .arg(&shared)
            .status()?;
        assert!(cleanup.success(), "failed to clean Darwin regression ACL");

        let Err(error) = result else {
            return Err("mutating non-euid allow ACL ancestor was accepted".into());
        };
        let message = error.to_string();
        let crate::ServerError::UnsafeDataRootAncestor {
            component, reason, ..
        } = error
        else {
            return Err(format!("expected typed unsafe-ancestor error, got {message}").into());
        };
        assert_eq!(component, std::fs::canonicalize(&shared)?);
        assert!(
            reason.contains("allow"),
            "reason did not name the ACE: {reason}"
        );
        assert!(
            reason.contains("everyone"),
            "reason did not name the ACE principal: {reason}"
        );
        assert!(
            !data_root.join("config.json").exists(),
            "Haematite touched its ambient path before the ACL refusal"
        );
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn path_ambient_haematite_accepts_the_euid_uuid_allow_ace()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt as _;

        use exacl::{AclEntry, AclOption, Perm};

        let sandbox = crate::test_support::private_tempdir()?;
        std::fs::set_permissions(sandbox.path(), std::fs::Permissions::from_mode(0o700))?;
        let private_parent = sandbox.path().join("euid-uuid-allow");
        let data_root = private_parent.join("data");
        std::fs::create_dir(&private_parent)?;
        std::fs::set_permissions(&private_parent, std::fs::Permissions::from_mode(0o700))?;

        let server_uid = rustix::process::geteuid().as_raw();
        let ace_qualifier = crate::filesystem::darwin_user_uuid_for_test(server_uid)?;
        let entry = AclEntry::allow_user(
            &ace_qualifier.to_string(),
            Perm::EXECUTE | Perm::WRITE | Perm::APPEND | Perm::DELETE_CHILD,
            None,
        );
        exacl::setfacl(
            &[private_parent.as_path()],
            &[entry],
            AclOption::SYMLINK_ACL,
        )?;
        let configured = data_root
            .to_str()
            .ok_or("temporary data path was not UTF-8")?;

        let result = super::build_haematite_store(
            configured,
            4,
            None,
            test_node_cache_budget()?,
            test_lock_acquisition()?,
        );
        let cleanup = std::process::Command::new("chmod")
            .arg("-RN")
            .arg(&private_parent)
            .status()?;
        assert!(cleanup.success(), "failed to clean euid UUID allow ACL");

        let (store, responder) = result?;
        assert!(responder.is_none());
        assert!(data_root.join("config.json").is_file());
        drop(store);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn path_ambient_haematite_refuses_a_non_euid_user_uuid_allow_ace()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt as _;

        use exacl::{AclEntry, AclOption, Perm};

        let sandbox = crate::test_support::private_tempdir()?;
        std::fs::set_permissions(sandbox.path(), std::fs::Permissions::from_mode(0o700))?;
        let shared = sandbox.path().join("non-euid-uuid-allow");
        let data_root = shared.join("data");
        std::fs::create_dir(&shared)?;
        std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o700))?;

        let server_uid = rustix::process::geteuid().as_raw();
        let foreign_uid = u32::from(server_uid == 0);
        let foreign_qualifier = crate::filesystem::darwin_user_uuid_for_test(foreign_uid)?;
        let entry = AclEntry::allow_user(
            &foreign_qualifier.to_string(),
            Perm::EXECUTE | Perm::WRITE | Perm::APPEND | Perm::DELETE_CHILD,
            None,
        );
        exacl::setfacl(&[shared.as_path()], &[entry], AclOption::SYMLINK_ACL)?;
        let configured = data_root
            .to_str()
            .ok_or("temporary data path was not UTF-8")?;

        let result = super::build_haematite_store(
            configured,
            4,
            None,
            test_node_cache_budget()?,
            test_lock_acquisition()?,
        );
        let cleanup = std::process::Command::new("chmod")
            .arg("-RN")
            .arg(&shared)
            .status()?;
        assert!(cleanup.success(), "failed to clean non-euid UUID allow ACL");

        let Err(error) = result else {
            return Err("mutating non-euid user UUID allow ACE was accepted".into());
        };
        let message = error.to_string();
        let crate::ServerError::UnsafeDataRootAncestor {
            component, reason, ..
        } = error
        else {
            return Err(format!("expected typed unsafe-ancestor error, got {message}").into());
        };
        assert_eq!(component, std::fs::canonicalize(&shared)?);
        assert!(
            reason.contains("allow") && reason.contains(&format!("server euid {server_uid}")),
            "reason did not name the rejected ACE: {reason}"
        );
        assert!(
            !data_root.join("config.json").exists(),
            "Haematite touched its ambient path before the UUID ACL refusal"
        );
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn path_ambient_haematite_accepts_a_deny_only_acl_ancestor()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt as _;

        let sandbox = crate::test_support::private_tempdir()?;
        std::fs::set_permissions(sandbox.path(), std::fs::Permissions::from_mode(0o700))?;
        let private_parent = sandbox.path().join("deny-only");
        let data_root = private_parent.join("data");
        std::fs::create_dir(&private_parent)?;
        std::fs::set_permissions(&private_parent, std::fs::Permissions::from_mode(0o700))?;
        let status = std::process::Command::new("chmod")
            .arg("+a")
            .arg("everyone deny delete")
            .arg(&private_parent)
            .status()?;
        assert!(status.success(), "failed to install Darwin deny-only ACL");
        let configured = data_root
            .to_str()
            .ok_or("temporary data path was not UTF-8")?;

        let result = super::build_haematite_store(
            configured,
            4,
            None,
            test_node_cache_budget()?,
            test_lock_acquisition()?,
        );
        let cleanup = std::process::Command::new("chmod")
            .arg("-RN")
            .arg(&private_parent)
            .status()?;
        assert!(cleanup.success(), "failed to clean Darwin deny-only ACL");

        let (store, responder) = result?;
        assert!(responder.is_none());
        assert!(data_root.join("config.json").is_file());
        drop(store);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn path_ambient_haematite_accepts_the_stock_home_acl_chain()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt as _;
        use users::os::unix::UserExt as _;

        let effective_uid = rustix::process::geteuid().as_raw();
        let effective_user = users::get_user_by_uid(effective_uid)
            .ok_or_else(|| format!("server euid {effective_uid} has no account record"))?;
        let sandbox = tempfile::Builder::new()
            .prefix(".aion-acl-home-proof-")
            .tempdir_in(effective_user.home_dir())?;
        std::fs::set_permissions(sandbox.path(), std::fs::Permissions::from_mode(0o700))?;
        let data_root = sandbox.path().join("data");
        let configured = data_root
            .to_str()
            .ok_or("temporary data path was not UTF-8")?;

        let (store, responder) = super::build_haematite_store(
            configured,
            4,
            None,
            test_node_cache_budget()?,
            test_lock_acquisition()?,
        )?;
        assert!(responder.is_none());
        assert!(data_root.join("config.json").is_file());
        drop(store);
        Ok(())
    }

    #[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
    #[test]
    fn path_ambient_haematite_accepts_an_owner_controlled_chain()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt as _;

        let sandbox = crate::test_support::private_tempdir()?;
        std::fs::set_permissions(sandbox.path(), std::fs::Permissions::from_mode(0o700))?;
        let private_parent = sandbox.path().join("private");
        let data_root = private_parent.join("data");
        std::fs::create_dir(&private_parent)?;
        std::fs::set_permissions(&private_parent, std::fs::Permissions::from_mode(0o700))?;
        let configured = data_root
            .to_str()
            .ok_or("temporary data path was not UTF-8")?;

        let (store, responder) = super::build_haematite_store(
            configured,
            4,
            None,
            test_node_cache_budget()?,
            test_lock_acquisition()?,
        )?;
        assert!(responder.is_none());
        assert!(data_root.join("config.json").is_file());
        for shard in 0..4 {
            assert!(data_root.join(format!("shard-{shard}")).is_dir());
        }
        drop(store);
        Ok(())
    }

    #[tokio::test]
    async fn connect_store_memory_backend_exposes_no_outbox_store()
    -> Result<(), Box<dyn std::error::Error>> {
        use crate::config::{StoreBackend, StoreConfig};

        // Memory backend: no durable outbox table, so no outbox store handle —
        // and `outbox.enabled` over memory is rejected at dispatcher commission.
        let connected = super::connect_store(StoreConfig {
            backend: StoreBackend::Memory,
            owned_shards: Vec::new(),
            data_dir: None,
            shard_count: 1,
            cluster: None,
            node_cache_budget: None,
            ..StoreConfig::default()
        })
        .await?;
        assert!(
            connected.outbox_store.is_none(),
            "the in-memory backend exposes no outbox store"
        );
        Ok(())
    }

    /// The haematite boot path REFUSES a store config that does not rule on the
    /// node cache's byte budget, naming the missing key — the same
    /// explicit-no-default guard `observability.max_batch_events` uses, applied
    /// where the value is used (haematite is the only backend that has a node
    /// cache, so this is the seam that consumes the ruling).
    #[tokio::test]
    async fn haematite_boot_refuses_without_a_node_cache_budget()
    -> Result<(), Box<dyn std::error::Error>> {
        use crate::ServerError;
        use crate::config::{StoreBackend, StoreConfig};

        let sandbox = crate::test_support::private_tempdir()?;
        let data_dir = sandbox.path().join("data");
        let error = super::connect_haematite_store(StoreConfig {
            backend: StoreBackend::Haematite,
            data_dir: Some(
                data_dir
                    .to_str()
                    .ok_or("temporary data path was not UTF-8")?
                    .to_owned(),
            ),
            shard_count: 4,
            ..StoreConfig::default()
        })
        .await
        .err()
        .ok_or("the haematite boot path must refuse a store config with no node_cache_budget")?;
        let ServerError::Config { message } = error else {
            return Err(format!("expected a config refusal, got {error:?}").into());
        };
        assert!(
            message.contains("store.node_cache_budget"),
            "the refusal must name the missing key, got: {message}"
        );
        assert!(
            message.contains("AION_STORE_NODE_CACHE_BUDGET"),
            "the refusal must name the environment override, got: {message}"
        );
        Ok(())
    }

    /// The operator's configured budget reaches the constructed
    /// [`haematite::DatabaseConfig`] — observed where haematite records it, in
    /// the created database's own `config.json`, so the assertion cannot pass by
    /// a value that stopped short of `Database::create`.
    #[tokio::test]
    async fn configured_node_cache_budget_reaches_the_created_database()
    -> Result<(), Box<dyn std::error::Error>> {
        use crate::config::{StoreBackend, StoreConfig};

        const ONE_GIB: usize = 1 << 30;

        let sandbox = crate::test_support::private_tempdir()?;
        let data_dir = sandbox.path().join("data");
        let connected = super::connect_haematite_store(StoreConfig {
            backend: StoreBackend::Haematite,
            data_dir: Some(
                data_dir
                    .to_str()
                    .ok_or("temporary data path was not UTF-8")?
                    .to_owned(),
            ),
            shard_count: 4,
            node_cache_budget: Some(haematite::NodeCacheBudget::bytes(ONE_GIB)?),
            lock_acquisition_patience_ms: Some(250),
            lock_acquisition_retry_cadence_ms: Some(5),
            ..StoreConfig::default()
        })
        .await?;
        drop(connected);

        let recorded: serde_json::Value =
            serde_json::from_slice(&std::fs::read(data_dir.join("config.json"))?)?;
        assert_eq!(
            recorded.get("node_cache_budget"),
            Some(&serde_json::json!({ "bytes": ONE_GIB })),
            "the operator's budget must be the one haematite created the database with"
        );
        Ok(())
    }

    #[tokio::test]
    async fn state_build_fails_without_event_broadcast_capacity()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut runtime = runtime_config();
        runtime.websocket.event_broadcast_capacity = None;

        let error = ServerState::build_with_store(InMemoryStore::default(), runtime)
            .await
            .err()
            .ok_or("state build must fail when event streaming is unsized")?;

        assert!(error.is_config(), "expected a config error, got {error}");
        assert!(
            error
                .to_string()
                .contains("websocket.event_broadcast_capacity"),
            "error must name the missing key: {error}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn state_build_fails_without_query_timeout() -> Result<(), Box<dyn std::error::Error>> {
        let mut runtime = runtime_config();
        runtime.query_timeout = None;

        let error = ServerState::build_with_store(InMemoryStore::default(), runtime)
            .await
            .err()
            .ok_or("state build must fail when the query reply deadline is unset")?;

        assert!(error.is_config(), "expected a config error, got {error}");
        assert!(
            error.to_string().contains("runtime.query_timeout_ms"),
            "error must name the missing key: {error}"
        );
        assert!(
            error.to_string().contains("AION_RUNTIME_QUERY_TIMEOUT_MS"),
            "error must name the environment override: {error}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn state_build_fails_with_zero_query_timeout() -> Result<(), Box<dyn std::error::Error>> {
        let mut runtime = runtime_config();
        runtime.query_timeout = Some(Duration::ZERO);

        let error = ServerState::build_with_store(InMemoryStore::default(), runtime)
            .await
            .err()
            .ok_or("state build must fail when the query reply deadline is zero")?;

        assert!(error.is_config(), "expected a config error, got {error}");
        assert!(
            error.to_string().contains("runtime.query_timeout_ms"),
            "error must name the zero-valued key: {error}"
        );
        Ok(())
    }

    /// THE #189 WIRING PIN (r1 Blocker B1): a completed update check driven
    /// through the dispatcher stack `build_decorated_dispatcher` actually
    /// builds lands in the update-status slot that same call RETURNS — the
    /// slot the boot path stores and `GET /update-status` serves.
    ///
    /// Everything between the dispatch and the slot is the production object:
    /// the real `DeclaredCommandDispatcher` executes a real server-run command
    /// through the real `ShellAction` (transcript pump and all), the real
    /// `UpdateCheckObserver` sits in its real position, and the assertion
    /// reads the slot off the function's own return value. The one test
    /// double is the `DeclaredBodies` source — the seam production code
    /// installs after the engine exists — and it is SEQUENCED because the
    /// gates run offline: the observer's verification (first resolution)
    /// sees the genuine `FETCH_COMMAND`, and the executor (second
    /// resolution) is handed a local `cat` of the captured real index body,
    /// standing in for the network transfer the genuine curl would perform.
    ///
    /// The r1 review proved the absence of this pin by mutation: returning a
    /// FRESH slot instead of the observer's left all 1268 tests green while
    /// `/update-status` would answer null forever. Under this test that
    /// exact mutation goes red: the returned slot stays empty and the
    /// assertion below names it.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_completed_check_through_the_built_dispatcher_lands_in_the_returned_slot()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::collections::{BTreeMap, VecDeque};
        use std::sync::{Arc, Mutex};

        use aion::ActivityDispatch;
        use aion_core::{ActivityId, RunId, WorkflowId};
        use aion_package::ActionBodyContract;

        use crate::update_check::document::{FETCH_ACTION, FETCH_COMMAND, UPDATE_CHECK_QUEUE};
        use crate::worker::{DeclaredBodies, DeclaredBodyLookup, DispatchingRun};

        /// Hands out one scripted resolution per call, in order. Documented
        /// above: first the observer's verification, then the executor's.
        struct SequencedBodies {
            replies: Mutex<VecDeque<DeclaredBodyLookup>>,
        }

        impl DeclaredBodies for SequencedBodies {
            fn body_for(
                &self,
                _task_queue: &str,
                _action: &str,
                _run: DispatchingRun<'_>,
            ) -> DeclaredBodyLookup {
                let mut replies = match self.replies.lock() {
                    Ok(replies) => replies,
                    Err(poisoned) => poisoned.into_inner(),
                };
                replies.pop_front().unwrap_or(DeclaredBodyLookup::None)
            }
        }

        let runtime = runtime_config();
        let cluster_publisher = crate::cluster_publisher::ClusterEventPublisher::new(
            ServerState::FALLBACK_CLUSTER_BROADCAST_CAPACITY,
        );
        let namespace_store: Arc<dyn aion_store::NamespaceStore> =
            Arc::new(InMemoryStore::default());
        let worker_deployment_store: Arc<dyn aion_store::WorkerDeploymentStore> =
            Arc::new(InMemoryStore::default());
        let seams = super::build_worker_seams(
            &runtime,
            &cluster_publisher,
            &namespace_store,
            &worker_deployment_store,
            None,
        );

        // The captured REAL index body, served to the executor by a local
        // command instead of the network (gates run offline).
        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/update_check/fixtures/aion-cli-index.jsonl"
        );
        seams.declared_bodies.install(Arc::new(SequencedBodies {
            replies: Mutex::new(VecDeque::from([
                DeclaredBodyLookup::Declared(ActionBodyContract::Run {
                    command: FETCH_COMMAND.to_owned(),
                }),
                DeclaredBodyLookup::Declared(ActionBodyContract::Run {
                    command: format!("cat {fixture}"),
                }),
            ])),
        }));

        let transcript = crate::activity_publisher::ActivityEventPublisher::new(
            Arc::new(aion_store::InMemoryObservabilityStore::default()),
            ServerState::FALLBACK_CLUSTER_BROADCAST_CAPACITY,
            crate::activity_publisher::TranscriptBatchPolicy::UNBATCHED,
        );
        let (dispatcher, _mock_registry, _attempt_owners, _workspace_root, update_status) =
            super::build_decorated_dispatcher(&runtime, &seams, transcript);

        assert_eq!(
            update_status.last(),
            None,
            "the returned slot must start honestly empty"
        );

        let dispatch = ActivityDispatch {
            namespace: "default".to_owned(),
            task_queue: UPDATE_CHECK_QUEUE.to_owned(),
            node: None,
            workflow_id: WorkflowId::new_v4(),
            run_id: RunId::new_v4(),
            activity_id: ActivityId::from_sequence_position(1),
            name: FETCH_ACTION.to_owned(),
            input: "{}".to_owned(),
            config: "{}".to_owned(),
            attempt: 1,
            labels: BTreeMap::new(),
            advisory: false,
        };
        let handle = tokio::task::spawn_blocking(move || dispatcher.dispatch(dispatch));
        let encoded = handle
            .await?
            .map_err(|error| format!("the check dispatch failed: {error}"))?;
        let outcome: serde_json::Value = serde_json::from_str(&encoded)?;
        assert_eq!(outcome["exit_code"], 0, "the local stand-in command ran");

        let recorded = update_status.last().ok_or(
            "the completed check must land in the RETURNED slot — the one the boot path \
             stores and /update-status serves; an empty slot here is the disconnected-\
             producer mis-wire the r1 review proved unmeasured",
        )?;
        assert_eq!(recorded.latest_known, "0.13.7");
        Ok(())
    }
}
