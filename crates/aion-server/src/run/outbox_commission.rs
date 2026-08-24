//! Outbox commissioning for the run loop: the boot-state rebuild that must
//! precede the dispatcher's first claim, the dispatcher/reconciler spawns,
//! the cluster supervisor, the transport-selected row-dispatch build (with
//! its liminal variant), and the config resolutions each consumes.
//!
//! Split from `run.rs` so the run module stays a composition root; nothing
//! here is reachable from outside the run module.

use std::net::SocketAddr;
use std::sync::Arc;

use tracing::{error, info, warn};

use crate::{
    ServerConfig, ServerError, ServerState,
    config::{OutboxConfig, OutboxTransport},
    worker::{
        ActivityDispatcher, DeliveryGate, OutboxDeliveryCallback, OutboxDispatcher,
        OutboxDispatcherConfig, OutboxReconciler, OutboxReconcilerConfig, OutboxRowDispatch,
        ServerOutboxDeliveryCallback, WorkerOutboxDispatch,
    },
};

/// Short TTL for the dispatcher's per-namespace placement cache (Control-Plane
/// Phase 2, P2-P3). Kept small so an operator's `PUT /namespaces/{name}/placement`
/// takes effect on the hot claim loop within a couple of seconds, while still
/// collapsing a per-sweep quorum `get_namespace` into a cheap in-process lookup.
/// A stale entry under `Prefer` only mis-prefers a worker for at most one window
/// and self-corrects — it never affects correctness or replay.
const PLACEMENT_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(2);

/// Short TTL for the dispatcher's per-namespace quota cache (Control-Plane Phase 2,
/// P2-Q2). Kept small so an operator raising/lowering a tenant's
/// `max_in_flight_activities` takes effect on the hot claim loop within a couple of
/// seconds, while still collapsing a per-sweep quorum `get_namespace` into a cheap
/// in-process lookup. A stale entry only over- or under-admits slightly for one
/// window and self-corrects — backpressure never drops a row, so it cannot affect
/// correctness or replay.
const QUOTA_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(2);

/// Cadence of the ops-console quota-state broadcaster (Control-Plane Phase 2,
/// P2-Q3). Each tick samples every registry namespace's durable Claimed-row count
/// and cluster-wide ceiling, then pushes one `NamespaceQuotaState` per namespace
/// onto the cluster channel, so the console badge tracks live load. Kept at 1s:
/// brisk enough that the badge visibly ticks as work flows, throttled enough that
/// it is never a per-row firehose (in-flight changes on every claim/settle). It is
/// a server-side push on a timer, NOT a client poll — the dashboard rule bans the
/// latter, not a throttled server snapshot of REAL durable state.
const QUOTA_BROADCAST_CADENCE: std::time::Duration = std::time::Duration::from_secs(1);

/// Resolved keyed-backpressure inputs for the outbox dispatcher (Control-Plane
/// Phase 2, P2-Q2): the generous platform-default ceiling and this node's
/// owned-shard fraction of the cluster shard space.
#[derive(Clone, Copy, Debug)]
pub(super) struct BackpressureSettings {
    /// The `[namespaces] max_in_flight_activities` platform default, applied to any
    /// namespace carrying no explicit per-tenant override.
    pub(super) platform_default: u32,
    /// This node's owned-shard fraction of the cluster's virtual shard space,
    /// derived from `[store] owned_shards` and `[store] shard_count`.
    pub(super) fraction: crate::worker::OwnedShardFraction,
}

impl BackpressureSettings {
    /// Derive the backpressure inputs from the merged server config.
    ///
    /// An empty `[store] owned_shards` means own-all (the single-node default), so
    /// the fraction is 1 and per-node ceilings equal the cluster-wide quota. A
    /// declared owned set enforces the proportional per-node slice
    /// `|owned| / shard_count` (CP-Phase-2 §3.6).
    pub(super) fn from_config(config: &ServerConfig) -> Self {
        let total = u32::try_from(config.store.shard_count).unwrap_or(u32::MAX);
        let fraction = if config.store.owned_shards.is_empty() {
            crate::worker::OwnedShardFraction::own_all()
        } else {
            let owned = u32::try_from(config.store.owned_shards.len()).unwrap_or(u32::MAX);
            crate::worker::OwnedShardFraction::new(owned, total)
        };
        Self {
            platform_default: config.namespaces.max_in_flight_activities,
            fraction,
        }
    }
}

/// Owns the liminal worker listener for the server's lifetime when the outbox is
/// commissioned over the liminal transport.
///
/// The aion-server HOSTS the liminal listener that remote workers connect IN to;
/// its inner [`ServerListener`](liminal_server::server::listener::ServerListener)
/// owns the accept worker. Held as a local in [`run_server`] across the whole
/// serve `select!`, so it is dropped exactly at server shutdown — and the
/// listener's own `Drop` stops the accept worker cleanly (no leaked thread, no
/// orphaned listener). Every non-liminal boot (the default) carries the `None`
/// guard, which holds nothing and drops to a no-op, so behaviour is unchanged.
#[derive(Debug, Default)]
pub(super) struct OutboxWorkerListener {
    /// Held purely for its `Drop` side-effect (stopping the accept worker on
    /// server shutdown); never read after construction, hence the leading
    /// underscore.
    #[cfg(feature = "liminal-transport")]
    _inner: Option<liminal_server::server::listener::ServerListener>,
}

/// Rebuild the outbox-related boot state BEFORE the dispatcher's first claim,
/// when (and only when) the outbox is commissioned:
///
/// - #204: repopulate the durable pause dispatch-hold from `list_paused`, so a
///   run paused before a restart keeps its outbox rows held (never claimed)
///   after recovery. A run projecting `Paused` is excluded from `list_active`
///   respawn for free; this repopulates the hold that would otherwise be empty
///   in memory after a crash.
/// - #253: settle terminal workflows' stranded outbox rows. A workflow that
///   reached a durable terminal without its rows being settled (a settle-hook
///   failure, or a crash between the terminal append and the settle) must not
///   have those rows re-armed and redelivered after restart — that is the
///   zombie-round incident. A sweep error is loud but non-fatal: the
///   settle-at-terminal hook and the reconciler's liveness gate remain as
///   repair paths, and the residual window is one bounded dispatch whose
///   completion drops unmatched, never a re-arm loop.
pub(super) async fn rebuild_outbox_boot_state(state: &ServerState, outbox_config: &OutboxConfig) {
    if !outbox_config.enabled {
        return;
    }
    let Ok(engine) = state.engine() else {
        return;
    };
    if let Err(error) = engine.rebuild_paused_runs().await {
        warn!(%error, "failed to rebuild paused-runs dispatch hold at startup");
    }
    let Some(outbox_store) = state.outbox_store() else {
        return;
    };
    match crate::worker::settle_terminal_outbox_rows(engine.store().as_ref(), outbox_store.as_ref())
        .await
    {
        Ok(settled) if settled.is_empty() => {}
        Ok(settled) => {
            info!(
                settled = settled.len(),
                "boot sweep settled stranded outbox rows for terminal workflows"
            );
        }
        Err(error) => {
            error!(
                %error,
                "boot sweep failed to settle terminal workflows' outbox rows; \
                 the reconciler liveness gate remains the backstop"
            );
        }
    }
}

/// Spawn the durable-outbox fan-out dispatcher when, and only when, the
/// operator commissioned it (`outbox.enabled = true`).
///
/// This is the single gate that keeps Phase 2 dormant: with the flag off (the
/// default) the function returns immediately without spawning a task, so
/// default server behaviour — and the live workflow dispatch path — is entirely
/// unchanged. When commissioned, the dispatcher claims rows through the engine's
/// own shared haematite leaf, so its writes serialize through the same durable
/// store. The dispatcher shares the server's shutdown watch, so it drains on the
/// same signal as the transports.
///
/// NOTE (Phase boundary): the spawned dispatcher dispatches claimed rows and
/// records each row's terminal outbox state (done / retry / failed). Routing the
/// worker completion back into workflow history through the Recorder is Phase 3
/// and is not wired here.
pub(super) fn maybe_spawn_outbox_dispatcher(
    state: &ServerState,
    outbox_config: &OutboxConfig,
    clustered: bool,
    backpressure_settings: BackpressureSettings,
    shutdown_rx: &tokio::sync::watch::Receiver<bool>,
    liminal_address_hint: &str,
) -> Result<OutboxWorkerListener, ServerError> {
    if !outbox_config.enabled {
        return Ok(OutboxWorkerListener::default());
    }
    let dispatcher_config = resolve_outbox_config(outbox_config)?;
    // Share the engine's already-opened haematite store. The
    // dispatcher's `claim_outbox_rows` writes serialize against the engine's
    // `append_with_outbox`; the
    // in-memory backend has no outbox table, so `outbox_store()` is `None` and
    // commissioning the dispatcher against it is a configuration error (LSUB-4-2).
    let outbox_store = state.outbox_store().ok_or_else(|| ServerError::Config {
        message: "outbox.enabled=true requires store.backend=haematite: \
                  the durable outbox dispatcher claims rows from the store's outbox table, which \
                  the in-memory store does not provide"
            .to_owned(),
    })?;
    let dispatcher_builder = OutboxDispatcher::new(Arc::clone(&outbox_store), dispatcher_config);
    let delivery_gate = dispatcher_builder.delivery_gate();
    let engine = state.engine()?;
    let delivery_callback: Arc<dyn OutboxDeliveryCallback> =
        Arc::new(ServerOutboxDeliveryCallback::new(engine));
    let (row_dispatch, worker_listener) = select_outbox_row_dispatch(
        state,
        outbox_config,
        shutdown_rx,
        delivery_gate.clone(),
        Arc::clone(&delivery_callback),
        liminal_address_hint,
    )?;
    // LSUB-2: share the engine's advisory wake so the stage seam pulses this
    // dispatcher the instant a fan-out row commits, dispatching in ~RTT instead of
    // up to one poll interval. The wake is always-on and free; the interval poll is
    // untouched, so it remains the correctness backstop for any lost wake.
    // Control-Plane Phase 2 (P2-Q2): attach per-tenant keyed backpressure so each
    // sweep claims per-namespace, round-robin, capped at each tenant's CLAIMED-only
    // headroom (`per_node_ceiling − claimed`). The quota cache front-runs a per-sweep
    // quorum `get_namespace`. With the generous platform default and no tenant
    // override the ceiling never engages, so a default deployment's claim behaviour is
    // byte-identical to the pre-Phase-2 single unscoped claim.
    let quota_cache = crate::worker::QuotaCache::new(
        Arc::clone(state.namespace_store()),
        backpressure_settings.platform_default,
        QUOTA_CACHE_TTL,
    );
    let backpressure =
        crate::worker::Backpressure::new(quota_cache.clone(), backpressure_settings.fraction);
    let mut dispatcher = dispatcher_builder
        .with_dispatch(row_dispatch)
        .with_delivery_callback(delivery_callback)
        .with_wake(state.outbox_wake())
        .with_backpressure(backpressure);
    // #204: attach the engine's durable pause dispatch-hold so a held (paused)
    // run's rows are never claimed. The hold set is rebuilt from `list_paused`
    // BEFORE this spawn (see `run_server`), so the dispatcher's first claim
    // already excludes pre-pause rows after a restart.
    if let Ok(engine) = state.engine() {
        dispatcher = dispatcher.with_paused_runs(engine.paused_runs());
    }
    tokio::spawn(dispatcher.run(shutdown_rx.clone()));
    // Control-Plane Phase 2 (P2-Q3): commission the ops-console quota-state
    // broadcaster on the SAME durable stores + quota cache the dispatcher enforces
    // against, so the console badge is a faithful window onto the live per-tenant
    // in-flight/ceiling the backpressure caps. It shares the shutdown watch, so it
    // drains with the dispatcher. Only spawned alongside the (default-off)
    // dispatcher: quota state is meaningless without the outbox fan-out path, and
    // `in_flight` is the durable Claimed outbox count that path produces.
    let quota_broadcaster = crate::worker::QuotaBroadcaster::new(
        Arc::clone(state.namespace_store()),
        Arc::clone(&outbox_store),
        quota_cache,
        state.cluster_publisher().clone(),
        QUOTA_BROADCAST_CADENCE,
    );
    tokio::spawn(quota_broadcaster.run(shutdown_rx.clone()));
    // LSUB-4-1: the single dispatcher task is spawned in both modes. In a
    // single-node boot it owns all shards by construction; in an active-active
    // clustered boot it claims ONLY the shards this node owns, enforced by
    // `claim_outbox_rows`' owned-shard scope (already seeded before this point).
    info!(
        clustered,
        "outbox dispatcher commissioned (active-active per-shard ownership enforced by claim scope \
         when clustered; single-node owns all shards)"
    );
    // LSUB-4-4: the stale-claim reconciler is the in-flight recovery backstop. It
    // is only configured when BOTH reconcile knobs are set, so on a clustered boot
    // that left them unset, owner-kill in-flight recovery latency is bounded only
    // by re-residency replay (a survivor adopting the shard re-residents from
    // history and re-arms via `rearm_outbox_pending`), NOT by `stale_after`. Warn
    // so the operator knows the backstop is absent.
    if let Some(reconciler_config) = resolve_outbox_reconciler_config(outbox_config)? {
        // #253: the reconciler's liveness gate projects each stale candidate's
        // workflow status from the engine's event store before any re-arm, so
        // a terminal workflow's stranded row settles instead of redelivering.
        let event_store = state.engine()?.store();
        let reconciler = OutboxReconciler::new(outbox_store, event_store, reconciler_config)
            .with_delivery_gate(delivery_gate);
        tokio::spawn(reconciler.run(shutdown_rx.clone()));
        info!("outbox reconciler commissioned (terminal-workflow liveness gate active)");
    } else if clustered {
        warn!(
            "outbox reconciler is UNCONFIGURED on a clustered boot (outbox.reconcile_interval_ms \
             and outbox.reconcile_stale_after_ms are both unset): in-flight recovery after an \
             owner is killed is then bounded only by re-residency replay on the adopting node, \
             not by a stale-claim backstop; set both knobs to bound stale-claim recovery latency"
        );
    }
    Ok(worker_listener)
}

/// Spawn the SS-5b cluster supervisor when, and only when, this is a distributed
/// haematite boot whose `[store.cluster]` declared peers with owned shards.
///
/// Reads the failover cadence + debounce from the cluster config (or the
/// documented defaults), then asks the state to spawn the supervisor over its
/// retained concrete store and live engine. With no `[store.cluster]` section —
/// or with no peer declaring `owned_shards` — nothing is spawned and behaviour
/// is unchanged.
pub(super) fn maybe_spawn_cluster_supervisor(
    state: &ServerState,
    cluster_config: Option<&crate::config::ClusterConfig>,
    shutdown_rx: &tokio::sync::watch::Receiver<bool>,
) -> Result<(), ServerError> {
    let Some(cluster) = cluster_config else {
        return Ok(());
    };
    let poll_interval = std::time::Duration::from_millis(
        cluster
            .failover_poll_interval_ms
            .unwrap_or(crate::config::DEFAULT_FAILOVER_POLL_INTERVAL_MS),
    );
    let confirmations = cluster
        .failover_confirmations
        .unwrap_or(crate::config::DEFAULT_FAILOVER_CONFIRMATIONS);
    let supervisor_config = crate::cluster::SupervisorConfig {
        poll_interval,
        confirmations,
    };
    let spawned = state.spawn_cluster_supervisor(supervisor_config, shutdown_rx.clone())?;
    if spawned {
        info!(
            poll_interval_ms = %poll_interval.as_millis(),
            confirmations,
            "SS-5b cluster supervisor commissioned (automatic peer-down failover)"
        );
    }
    Ok(())
}

/// Select the outbox row-dispatch sink by the configured `outbox.transport`,
/// returning the sink plus the worker listener whose lifetime the caller must
/// hold.
///
/// `grpc` (the default) builds the unchanged [`WorkerOutboxDispatch`] over the
/// connected-worker registry and carries the empty [`OutboxWorkerListener`], so a
/// default server is byte-identical. `liminal` builds the cross-node
/// [`RegistryLiminalDispatch`](crate::worker::RegistryLiminalDispatch) AND stands
/// up the liminal worker listener the aion-server hosts (returned in the guard);
/// it is only reachable when the `liminal-transport` feature is compiled in, and
/// selecting it without that feature is a configuration error rather than a
/// silent fall-through to gRPC.
fn select_outbox_row_dispatch(
    state: &ServerState,
    outbox_config: &OutboxConfig,
    shutdown_rx: &tokio::sync::watch::Receiver<bool>,
    delivery_gate: DeliveryGate,
    delivery_callback: Arc<dyn OutboxDeliveryCallback>,
    liminal_address_hint: &str,
) -> Result<(Arc<dyn OutboxRowDispatch>, OutboxWorkerListener), ServerError> {
    match outbox_config.transport {
        OutboxTransport::Grpc => {
            let push_dispatcher = ActivityDispatcher::new(state.worker_registry().clone())
                .with_drain_state(state.drain_state().clone())
                .with_completion_fences(state.pending_activities().completion_fences())
                // Share the SAME queue-service seams the direct dispatch path
                // uses, so a row parked on this leg reaches `GET
                // /queues/unserved` and `describe`'s `unserved` list rather
                // than being invisible to both.
                .with_queue_service(
                    state.queue_declarations().clone(),
                    state.queue_service_state().clone(),
                    state.runtime_config().worker.queue_service.clone(),
                )
                // ...including the cluster publisher, so an unbounded park on
                // this leg is announced on the operator's real-time channel
                // exactly like the direct path (#266 T4).
                .with_cluster_publisher(state.cluster_publisher().clone());
            // Control-Plane Phase 2 (P2-P3): attach the short-TTL placement cache
            // so an unpinned row in a `Prefer{L}` namespace prefers an L-labelled
            // worker (spilling to any live worker). The cache front-runs a per-row
            // quorum `get_namespace` on the hot claim loop; a default-`Unplaced`
            // deployment is byte-identical (every row falls through to any-worker).
            let placement_cache = crate::worker::PlacementCache::new(
                Arc::clone(state.namespace_store()),
                PLACEMENT_CACHE_TTL,
            );
            let dispatch: Arc<dyn OutboxRowDispatch> = Arc::new(
                WorkerOutboxDispatch::new(push_dispatcher).with_placement_cache(placement_cache),
            );
            Ok((dispatch, OutboxWorkerListener::default()))
        }
        OutboxTransport::Liminal => build_liminal_row_dispatch(
            state,
            outbox_config,
            shutdown_rx,
            delivery_gate,
            delivery_callback,
            liminal_address_hint,
        ),
    }
}

/// Build the production liminal row-dispatch sink and host the worker listener, or
/// fail with the missing-feature error.
///
/// This lifts the tested cross-node wiring (the `lsub1`/`lsub5` e2e blueprint)
/// into the production boot. The aion-server HOSTS the liminal listener that
/// remote workers connect IN to, so its
/// [`ConnectionSupervisor`](liminal_server::server::connection::ConnectionSupervisor)
/// owns each worker's connection and can push a dispatch out on it. The
/// constructor cycle resolves the notifier <-> supervisor dependency:
///
/// 1. Reuse the registry already in [`ServerState`] — gRPC and liminal workers
///    share ONE registry and the same `select_worker`, so routing is identical.
/// 2. Build the [`LiminalConnectionNotifier`] over that registry (no supervisor
///    yet).
/// 3. Build the [`LiminalConnectionServices`] from the liminal listen config.
/// 4. Build the [`ConnectionSupervisor`] WITH the services + notifier.
/// 5. Bind the supervisor back into the notifier (must succeed).
/// 6. Bind the [`ServerListener`] on the configured listen address — workers
///    connect IN here.
/// 7. Reuse the SAME completion callback the gRPC completion path installs
///    ([`ServerOutboxDeliveryCallback`] over the live engine), so a liminal
///    completion re-enters aion through the identical terminal-recording seam.
/// 8. Build the [`RegistryLiminalDispatch`] over the registry + callback (it
///    constructs the [`LiminalCompletionSource`] internally).
///
/// The returned listener is held by the caller for the server's lifetime; its
/// `Drop` stops the accept worker on shutdown.
///
/// [`LiminalConnectionServices`]: liminal_server::server::connection::LiminalConnectionServices
/// [`ServerListener`]: liminal_server::server::listener::ServerListener
/// [`ServerOutboxDeliveryCallback`]: crate::worker::ServerOutboxDeliveryCallback
/// [`LiminalCompletionSource`]: crate::worker::LiminalCompletionSource
/// [`LiminalConnectionNotifier`]: crate::worker::LiminalConnectionNotifier
#[cfg(feature = "liminal-transport")]
fn build_liminal_row_dispatch(
    state: &ServerState,
    outbox_config: &OutboxConfig,
    shutdown_rx: &tokio::sync::watch::Receiver<bool>,
    delivery_gate: DeliveryGate,
    callback: Arc<dyn OutboxDeliveryCallback>,
    liminal_address_hint: &str,
) -> Result<(Arc<dyn OutboxRowDispatch>, OutboxWorkerListener), ServerError> {
    use liminal_server::config::ServerConfig as LiminalServerConfig;
    use liminal_server::config::{LimitsConfig, ServicesConfig};
    use liminal_server::server::connection::{ConnectionSupervisor, LiminalConnectionServices};
    use liminal_server::server::listener::ServerListener;

    use crate::worker::{LiminalConnectionNotifier, RegistryLiminalDispatch};

    let listen_address = outbox_config
        .liminal_listen_address
        .as_ref()
        .ok_or_else(|| ServerError::Config {
            message: format!(
                "outbox.transport=liminal requires outbox.liminal_listen_address (host:port \
                 the aion-server listens on for inbound liminal worker connections); \
                 {liminal_address_hint}"
            ),
        })?;
    let listen_address: SocketAddr =
        listen_address
            .parse()
            .map_err(|error| ServerError::Config {
                message: format!(
                    "outbox.liminal_listen_address must be a host:port socket address: {error}"
                ),
            })?;

    // The liminal listener is the worker-connection front door only: it binds the
    // wire listen address and serves the connection supervisor. `from_config` and
    // `ServerListener::bind` read neither `health_listen_address` nor `channels`
    // (the health probe is bound only by the standalone liminal server's full
    // boot, not this embedded path), so no separate health port is bound here;
    // it is set structurally to the listen address and never used.
    let liminal_config = LiminalServerConfig {
        listen_address,
        health_listen_address: listen_address,
        drain_timeout_ms: 30_000,
        channels: Vec::new(),
        routing_rules: Vec::new(),
        persistence_path: None,
        cluster: None,
        // liminal 0.2.3 (H4) added an optional shared-token Connect gate. `None`
        // keeps this embedded worker front door open at the liminal layer —
        // identical to the pre-0.2.3 wire behavior; worker identity/authorization
        // stays aion's job (x-aion-* registration metadata). Threading an
        // operator-configured token through aion's outbox config is a separate
        // feature decision, not part of the dependency alignment.
        auth: None,
        // liminal 0.2.4 (D2/§5): service profile + operational bounds. Defaults =
        // full profile + the certifying-pair-signed caps — byte-equivalent to the
        // 0.2.3 behaviour this embedded front door always had. A worker-front-door
        // profile election here is a future feature decision, not this migration.
        services: ServicesConfig::default(),
        limits: LimitsConfig::default(),
        // liminal 0.3.0 (LP-WS-TRANSPORT R1 / LP Part B): optional WebSocket
        // acceptor and participant lifecycle activation. `None` for both starts
        // no WebSocket listener and leaves the participant capability disabled —
        // documented as byte-identical to the pre-0.3.0 build. Electing either
        // for this embedded worker front door is a feature decision, not part of
        // the dependency alignment.
        websocket: None,
        participant: None,
    };

    // (1) Reuse the registry already in ServerState: gRPC + liminal workers share
    // ONE registry and the same `select_worker`.
    let registry = state.worker_registry().clone();
    // (2) Notifier over that registry (supervisor bound after it is built), with the
    // NOI-5b transcript tap: a worker's observability publishes on the reserved
    // channel drain into the SAME transcript sequencer the transcript socket serves,
    // so a live agent's transcript is persisted + fanned out. (Captures the current
    // runtime handle to bridge the sync connection callback onto the async append.)
    let notifier = Arc::new(
        LiminalConnectionNotifier::new(registry.clone())
            .with_contract_catalog(state.engine()?)
            .with_transcript_publisher(state.transcript_publisher().clone())
            // The SAME per-task liveness tracker the engine-seam bridge tracks
            // into: a liminal worker's automatic liveness beats refresh it, so
            // the #176 expiry sweeper never falsely expires a healthy liminal
            // worker running an activity longer than the heartbeat window.
            .with_heartbeat_tracker(state.heartbeat_tracker().clone()),
    );
    // (3) Connection services from the liminal listen config.
    let services = Arc::new(
        LiminalConnectionServices::from_config(&liminal_config).map_err(|error| {
            ServerError::Config {
                message: format!("liminal connection services build failed: {error}"),
            }
        })?,
    );
    // (4) Supervisor WITH the services + notifier (the cycle's forward edge).
    let supervisor = ConnectionSupervisor::with_services_and_notifier(services, notifier.clone())
        .map_err(|error| ServerError::Config {
        message: format!("liminal connection supervisor build failed: {error}"),
    })?;
    // (5) Bind the supervisor back into the notifier (the cycle's back edge); a
    // failure here is a wiring bug, surfaced rather than silently ignored.
    if !notifier.bind_supervisor(supervisor.clone()) {
        return Err(ServerError::Config {
            message: "liminal notifier supervisor handle was already bound during boot".to_owned(),
        });
    }
    // (5b) Commission the connection dead-man switch over the SAME notifier. It
    // pings every connected worker on a derived quarter-window cadence: the
    // answers keep a healthy IDLE connection's lease alive (so the idle expiry
    // cannot fire on a live worker), and the pings themselves are what a worker
    // measures silence against (so a wedged half-open socket becomes a declared,
    // logged death on the worker side instead of an unbounded blind wait). Not
    // opt-in: liveness detection is a correctness property of this transport.
    // The handle is detached — dropping a tokio `JoinHandle` never cancels the
    // task — exactly as the heartbeat sweeper is spawned.
    drop(state.spawn_liminal_liveness_probe(notifier.clone(), shutdown_rx.clone()));
    // (6) Bind the listener on the configured address — workers connect IN here.
    let listener =
        ServerListener::bind(&liminal_config, supervisor).map_err(|error| ServerError::Config {
            message: format!("liminal worker listener failed to bind {listen_address}: {error}"),
        })?;
    // (7) Reuse the SAME completion callback the gRPC completion path uses, over
    // the live engine, so a liminal completion re-enters aion through the
    // identical terminal-recording seam (`record_fan_out_completion`).
    // (8) The registry-backed dispatch builds its LiminalCompletionSource from the
    // shared callback internally. Attach the SAME short-TTL placement cache the
    // gRPC arm installs (Control-Plane Phase 2, P2-P3), so an unpinned row in a
    // `Prefer{L}` namespace prefers an L-labelled worker (spilling to any live
    // worker) on the cross-node liminal transport too — the cluster-failover
    // demo behaviour. A default-`Unplaced` deployment is byte-identical.
    let placement_cache = crate::worker::PlacementCache::new(
        Arc::clone(state.namespace_store()),
        PLACEMENT_CACHE_TTL,
    );
    // NOI-6: install the SAME attempt-owner back-index the server's intervention
    // router resolves through, so each dispatched agent attempt binds its owning
    // worker and a pushed command reaches the worker this dispatcher sent it to.
    let dispatch: Arc<dyn OutboxRowDispatch> = Arc::new(
        RegistryLiminalDispatch::new(registry, callback, delivery_gate)
            .with_placement_cache(placement_cache)
            .with_attempt_owners(state.attempt_owners().clone()),
    );

    info!(
        listen_address = %listen_address,
        "liminal outbox worker listener commissioned (remote workers connect in and self-register)"
    );
    Ok((
        dispatch,
        OutboxWorkerListener {
            _inner: Some(listener),
        },
    ))
}

/// Feature-off stub: selecting the liminal transport without the
/// `liminal-transport` feature is a configuration error, never a silent
/// fall-through to gRPC.
#[cfg(not(feature = "liminal-transport"))]
fn build_liminal_row_dispatch(
    _state: &ServerState,
    _outbox_config: &OutboxConfig,
    _shutdown_rx: &tokio::sync::watch::Receiver<bool>,
    _delivery_gate: DeliveryGate,
    _delivery_callback: Arc<dyn OutboxDeliveryCallback>,
    _liminal_address_hint: &str,
) -> Result<(Arc<dyn OutboxRowDispatch>, OutboxWorkerListener), ServerError> {
    Err(ServerError::Config {
        message: "outbox.transport=liminal requires the aion-server `liminal-transport` \
                  Cargo feature, which is not enabled in this build"
            .to_owned(),
    })
}

/// Resolve the validated, all-present outbox knobs into the dispatcher's
/// non-optional config. Validation already guaranteed each value is set and in
/// range when `outbox.enabled` is true, so an absent value here is a defensive
/// configuration error, not a default to invent.
fn resolve_outbox_config(outbox: &OutboxConfig) -> Result<OutboxDispatcherConfig, ServerError> {
    let poll_interval_ms = outbox.poll_interval_ms.ok_or_else(|| ServerError::Config {
        message: crate::config::OUTBOX_POLL_INTERVAL_REQUIRED.to_owned(),
    })?;
    let batch_size = outbox.batch_size.ok_or_else(|| ServerError::Config {
        message: crate::config::OUTBOX_BATCH_SIZE_REQUIRED.to_owned(),
    })?;
    let max_attempts = outbox.max_attempts.ok_or_else(|| ServerError::Config {
        message: crate::config::OUTBOX_MAX_ATTEMPTS_REQUIRED.to_owned(),
    })?;
    let backoff_base_ms = outbox.backoff_base_ms.ok_or_else(|| ServerError::Config {
        message: crate::config::OUTBOX_BACKOFF_BASE_REQUIRED.to_owned(),
    })?;
    let backoff_multiplier = outbox
        .backoff_multiplier
        .ok_or_else(|| ServerError::Config {
            message: crate::config::OUTBOX_BACKOFF_MULTIPLIER_REQUIRED.to_owned(),
        })?;
    let backoff_max_ms = outbox.backoff_max_ms.ok_or_else(|| ServerError::Config {
        message: crate::config::OUTBOX_BACKOFF_MAX_REQUIRED.to_owned(),
    })?;
    Ok(OutboxDispatcherConfig {
        poll_interval: std::time::Duration::from_millis(poll_interval_ms),
        batch_size,
        max_attempts,
        backoff_base: std::time::Duration::from_millis(backoff_base_ms),
        backoff_multiplier,
        backoff_max: std::time::Duration::from_millis(backoff_max_ms),
    })
}

pub(super) fn resolve_outbox_reconciler_config(
    outbox: &OutboxConfig,
) -> Result<Option<OutboxReconcilerConfig>, ServerError> {
    let (Some(interval_ms), Some(stale_after_ms)) = (
        outbox.reconcile_interval_ms,
        outbox.reconcile_stale_after_ms,
    ) else {
        return Ok(None);
    };
    let batch_size = outbox.batch_size.ok_or_else(|| ServerError::Config {
        message: crate::config::OUTBOX_BATCH_SIZE_REQUIRED.to_owned(),
    })?;
    Ok(Some(OutboxReconcilerConfig {
        interval: std::time::Duration::from_millis(interval_ms),
        stale_after: std::time::Duration::from_millis(stale_after_ms),
        batch_size,
    }))
}
