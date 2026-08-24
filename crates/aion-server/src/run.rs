//! Run loop for the Aion workflow server: tracing initialization,
//! configuration load, transport startup, and signal-driven graceful
//! shutdown.
//!
//! This is the library entry point behind the `aion server` command. It
//! preserves the operational contract of the former standalone
//! `aion-server` binary: exit code 2 for configuration errors, the drain
//! outcome's exit code on shutdown, and 130 when a second termination
//! signal forces immediate exit.

use std::{net::SocketAddr, process::ExitCode};

use tokio::net::TcpListener;
use tonic::transport::Server as TonicServer;
use tracing::{error, info, warn};

use std::sync::Arc;

use crate::{
    ServerConfig, ServerError, ServerState, api,
    config::{CliOverrides, NamespaceMode, OutboxConfig, OutboxTransport, StoreBackend},
    observability,
    shutdown::ShutdownOutcome,
    worker::{
        ActivityDispatcher, DeliveryGate, OutboxDeliveryCallback, OutboxDispatcher,
        OutboxDispatcherConfig, OutboxReconciler, OutboxReconcilerConfig, OutboxRowDispatch,
        ServerOutboxDeliveryCallback, WorkerOutboxDispatch,
    },
};

mod doors;

use doors::{bind_and_claim, serve_until_shutdown};

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
struct BackpressureSettings {
    /// The `[namespaces] max_in_flight_activities` platform default, applied to any
    /// namespace carrying no explicit per-tenant override.
    platform_default: u32,
    /// This node's owned-shard fraction of the cluster's virtual shard space,
    /// derived from `[store] owned_shards` and `[store] shard_count`.
    fraction: crate::worker::OwnedShardFraction,
}

impl BackpressureSettings {
    /// Derive the backpressure inputs from the merged server config.
    ///
    /// An empty `[store] owned_shards` means own-all (the single-node default), so
    /// the fraction is 1 and per-node ceilings equal the cluster-wide quota. A
    /// declared owned set enforces the proportional per-node slice
    /// `|owned| / shard_count` (CP-Phase-2 §3.6).
    fn from_config(config: &ServerConfig) -> Self {
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
struct OutboxWorkerListener {
    /// Held purely for its `Drop` side-effect (stopping the accept worker on
    /// server shutdown); never read after construction, hence the leading
    /// underscore.
    #[cfg(feature = "liminal-transport")]
    _inner: Option<liminal_server::server::listener::ServerListener>,
}

/// Run the Aion workflow server until it shuts down, returning the process
/// exit code.
///
/// Initializes the JSON tracing subscriber, loads and validates the merged
/// configuration (file, environment, then `overrides`), serves the gRPC and
/// HTTP transports, and drains gracefully after the first termination
/// signal. Every failure is logged through tracing and mapped to the exit
/// code contract above; the caller only has to exit with the returned code.
pub async fn run(overrides: CliOverrides) -> ExitCode {
    match run_server(overrides).await {
        Ok(code) => code,
        Err(error) => {
            error!(%error, "aion-server failed");
            if error.is_config() {
                ExitCode::from(2)
            } else {
                ExitCode::FAILURE
            }
        }
    }
}

/// The where-to-edit half of the missing `outbox.liminal_listen_address`
/// refusal: a liminal outbox refusal must name the FILE to edit, not just the
/// key — the operator reading it is exactly the operator who did not write
/// the config (a scaffolded or setup-script home).
fn liminal_address_hint(source: &crate::config::ConfigSource) -> String {
    match source {
        crate::config::ConfigSource::BuiltInDefaults => {
            "set AION_OUTBOX_LIMINAL_LISTEN_ADDRESS, or add `liminal_listen_address = \
             \"127.0.0.1:50061\"` to `[outbox]` in a config file"
                .to_owned()
        }
        source => format!(
            "add `liminal_listen_address = \"127.0.0.1:50061\"` to `[outbox]` in the {source}"
        ),
    }
}

async fn run_server(cli: CliOverrides) -> Result<ExitCode, ServerError> {
    observability::tracing::init()?;

    // #180: a boot that discovers no config anywhere first scaffolds
    // `<AION_HOME>/config.toml` from the embedded template (claim-only-when-
    // empty), then loads it — config LOAD itself stays pure and read-only.
    let loaded = crate::config::load_or_scaffold(&cli)?;
    loaded.resolution.ensure_private_home()?;
    // Arm the death note as early as the home exists, so every later failure
    // path — including config validation and state build — runs inside the
    // ARMED/DISARMED bracket. Two anonymous server deaths on 2026-08-16 are
    // why this exists; see the module docs for the exact coverage.
    let death_note = crate::death_note::DeathNote::arm(&loaded.resolution.home)?;
    let home = loaded.resolution.home.clone();
    loaded.resolution.log_startup();
    let liminal_address_hint = liminal_address_hint(&loaded.resolution.source);
    let config = loaded.config;
    reject_auth_without_feature(&config)?;
    let store_backend = config.store.backend;
    // Static shard assignment (SS-1): read the operator's pinned shard set from
    // `[store] owned_shards`. Empty means own ALL shards (single-node default).
    // The set is carried into `RuntimeConfig` by `into_parts` and applied to the
    // `EngineBuilder` during state construction; surface it here so the boot
    // banner records which shards this node serves. No election is performed.
    let owned_shards = config.store.owned_shards.clone();
    // Capture the outbox settings before `build` consumes `config`, so the
    // (default-off) outbox dispatcher can be wired after state is up. The
    // dispatcher shares the engine's already-opened haematite store via
    // `state.outbox_store()`, so no store settings are needed.
    let outbox_config = config.outbox.clone();
    // Control-Plane Phase 2 (P2-Q2): capture the keyed-backpressure inputs — the
    // generous platform-default ceiling and this node's owned-shard fraction —
    // before `build` consumes `config`. On a single-node / own-all boot the fraction
    // is 1, so per-node ceilings equal the cluster-wide quota and, with the generous
    // default and no tenant override, the ceiling never engages (byte-identical claim).
    let backpressure_settings = BackpressureSettings::from_config(&config);
    // Capture the SS-5b failover supervisor knobs before `build` consumes config.
    // Only a distributed haematite boot carries a `[store.cluster]` section; this
    // is `None` for every single-node boot, so no supervisor is ever spawned.
    let cluster_config = config.store.cluster.clone();
    // Capture the managed-worker supervision policy before `build` consumes
    // `config`. Resolution already happened during config validation, so this
    // cannot surprise an operator at boot; it is re-read here because the
    // policy is COMMISSIONED onto the supervisor built into state below, and a
    // server without the section supervises nothing.
    let supervision_policy = config.worker_supervision.resolve()?;
    let state = ServerState::build(config).await?;
    reject_tls_until_supported(&state)?;

    let runtime = state.runtime_config();
    let grpc_address = runtime.listen.grpc;
    let http_address = runtime.listen.http;
    let workflow_packages: Vec<String> = runtime
        .workflow_packages
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    // The revision, not just the version. A crate version cannot distinguish
    // two builds from different commits of the same version, and that is the
    // distinction an operator needs when deciding whether a restart restores
    // what was running or substitutes something else (#123). The endpoint
    // answers this too, but a crashed server leaves only its log.
    let build = crate::build_identity::BuildIdentity::current();
    // #139: the server-resolved workspace root (the aion home's `clones/`
    // directory) that declared bodies expand `{workspace_root}` with. Reported
    // here so composition points (setup.sh today, the workspace verb later)
    // READ the value from the server that will use it instead of re-deriving
    // it. An unresolvable root is reported as exactly that — never fabricated;
    // a placeholder-bearing dispatch will refuse terminally with this reason.
    // The rendering itself is `WorkspaceRoot::banner_value`, pinned by its own
    // two-case test, so the banner and the tests cannot drift apart.
    let workspace_root = state.workspace_root().banner_value();
    info!(
        version = env!("CARGO_PKG_VERSION"),
        build = %build.line(),
        commit = build.commit,
        grpc_address = %grpc_address,
        http_address = %http_address,
        default_namespace = %runtime.default_namespace,
        namespace_mode = namespace_mode_label(&runtime.namespace.mode),
        store_backend = store_backend_label(store_backend),
        auth_enabled = runtime.auth.enabled,
        deploy_enabled = runtime.deploy.enabled,
        metrics_enabled = runtime.metrics.enabled,
        workspace_root = %workspace_root,
        death_note = %death_note.path().display(),
        workflow_package_count = workflow_packages.len(),
        workflow_packages = ?workflow_packages,
        owned_shards = ?owned_shards,
        owns_all_shards = owned_shards.is_empty(),
        "aion-server startup banner"
    );
    // #139 leg C: the assistant ships IN aion. The embedded document is
    // installed here — after the engine has reloaded every persisted package,
    // so the install can see what is already resident, and before the
    // transports accept traffic, so the first caller finds it. It claims only a
    // catalog holding no version of the assistant type; anything else is the
    // operator's cut to make, and the outcome says so in the log either way.
    crate::assistant::install_embedded_assistant_for_server(&state, &liminal_address_hint).await;
    // #189 slice one: the built-in update check ships the same way, under the
    // same only-the-empty-case install rule. Installing makes it STARTABLE
    // and nothing else — no check runs without an explicit operator act.
    crate::update_check::install_embedded_update_check_for_server(&state).await;
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    // LSUB-4-1: a distributed haematite boot carries a `[store.cluster]` section.
    // The single outbox dispatcher task is spawned in BOTH modes; the difference
    // is only how ownership is enforced. Single-node (`None`) owns all shards by
    // construction (`owned_shard_scope() == None`), so its claim sweeps see every
    // row. Clustered (`Some`) relies on `claim_outbox_rows`' `owned_shard_scope()`
    // filter — already seeded by `set_owned_shards` during `ServerState::build`,
    // which runs before this point — so each node only ever claims rows on the
    // shards it owns. Compute the flag here where the cluster section is in
    // scope; pass it to the gate so the boot banner records the mode.
    let outbox_clustered = cluster_config.is_some();
    // Dormant by default: only when `outbox.enabled` is set does the
    // non-replayed outbox dispatcher task start. With the flag off (the
    // default) nothing here runs and server behaviour is unchanged.
    // Hold the liminal worker listener (if any) for the server's lifetime: it is
    // dropped at the end of `run_server`, after the serve `select!` completes, so
    // its accept worker stops cleanly on shutdown via the listener's own `Drop`.
    // #204/#253: rebuild the pause dispatch-hold and settle terminal
    // workflows' stranded outbox rows BEFORE the dispatcher's first claim.
    rebuild_outbox_boot_state(&state, &outbox_config).await;
    let _outbox_worker_listener = maybe_spawn_outbox_dispatcher(
        &state,
        &outbox_config,
        outbox_clustered,
        backpressure_settings,
        &shutdown_rx,
        &liminal_address_hint,
    )?;
    // SS-5b: a distributed boot whose peers declare owned shards runs the cluster
    // supervisor — automatic failover detection. A single-node boot spawns
    // nothing here (the method returns `false`), so default behaviour is
    // unchanged.
    maybe_spawn_cluster_supervisor(&state, cluster_config.as_ref(), &shutdown_rx)?;
    // #176: the worker heartbeat expiry sweeper is ALWAYS commissioned —
    // dead-worker detection is a liveness correctness property, not an opt-in
    // feature. It is the production caller of `fail_expired_workers`: a worker
    // whose stream stays open while its process wedges (stops heartbeating
    // without disconnecting) is expired, deregistered with the provable Timeout
    // reason, and its in-flight tasks surface as TRANSPORT losses, re-dispatched
    // attempt-neutrally rather than charged to the action's retry budget.
    // Cadence derives from `worker.heartbeat_window` (quarter-window, clamped to
    // [1s, window]; the default 30s window sweeps every 7.5s) — deliberately no
    // separate config knob. It drains on the same shutdown watch as the
    // transports; dropping the JoinHandle only detaches the task.
    drop(state.spawn_heartbeat_sweeper(shutdown_rx.clone()));
    commission_worker_supervision(&state, supervision_policy).await;
    // Bind both listeners and claim the home FIRST: the pid record the stop
    // verb trusts is written only once both doors are provably ours.
    let doors = bind_and_claim(
        &home,
        grpc_address,
        http_address,
        build.commit,
        state.runtime_config().drain_timeout,
    )
    .await?;
    let identity_pid = doors.identity_pid;
    let pid_file_guard = doors.pid_file_guard;
    // Instant doors: the startup catch-up legs (owed timer fires, schedule
    // catch-up) run as a background task CONCURRENT with the transports —
    // the backlog has no upper bound, and a boot that blocks on it keeps the
    // doors shut for the whole sweep (the 2026-08-24 estate outage shape:
    // 37+ minutes of healthy catch-up with every listener refusing).
    // Workflow-residency recovery already ran inside `ServerState::build`,
    // so every surface the transports serve answers correctly while the
    // catch-up drains behind them.
    drop(state.spawn_startup_catchup(shutdown_rx.clone())?);
    let mut grpc = tokio::spawn(serve_grpc(
        state.clone(),
        doors.grpc_listener,
        doors.bound_grpc,
        shutdown_rx.clone(),
    ));
    let mut http = tokio::spawn(serve_http(
        state.clone(),
        doors.http_listener,
        doors.bound_http,
        shutdown_rx,
    ));

    let report = serve_until_shutdown(&state, &shutdown_tx, &mut grpc, &mut http).await?;

    let outcome = report.outcome;
    let exit_code = outcome.exit_code();
    // The rich outcome crosses the process boundary through the death note
    // (one file, one writer); the exit code keeps the #207 contract.
    death_note.record_outcome(&crate::control::outcome::OutcomeRecord::from_report(
        identity_pid,
        &report,
    ));
    death_note.disarm(&format!(
        "clean run-loop exit: shutdown outcome {outcome:?}"
    ));
    drop(pid_file_guard);
    Ok(exit_code)
}

/// Install the operator's supervision policy and converge the fleet.
///
/// Uncommissioned is a first-class but never SILENT state: a server with no
/// `[worker_supervision]` section supervises nothing, and every deployment that
/// wanted to be running is named in the warning, so the gap between "the
/// operator deployed a worker" and "nothing is running it" is never quiet.
async fn commission_worker_supervision(
    state: &ServerState,
    policy: Option<crate::worker::SupervisionPolicy>,
) {
    let supervisor = state.worker_supervisor();
    let Some(policy) = policy else {
        match supervisor.report().await {
            Ok(report) => {
                let wanted: Vec<&str> = report
                    .workers
                    .iter()
                    .filter(|worker| worker.desired == aion_store::DesiredState::Running)
                    .map(|worker| worker.name.as_str())
                    .collect();
                if wanted.is_empty() {
                    info!("managed-worker supervision is not configured; no deployment wants it");
                } else {
                    warn!(
                        deployments = wanted.join(", "),
                        remedy = crate::worker::supervisor::UNCOMMISSIONED_REMEDY,
                        "worker deployments want to be running but supervision is not configured"
                    );
                }
            }
            Err(error) => error!(
                %error,
                "managed-worker supervision is not configured and the deployment records \
                 could not be read to say what that costs"
            ),
        }
        return;
    };
    if !supervisor.commission(policy, crate::worker::ManagedExecutable::CurrentServer) {
        error!("managed-worker supervision was already commissioned before boot completed");
        return;
    }
    match supervisor.reconcile().await {
        Ok(0) => info!("managed-worker supervision commissioned; no deployment wants to run"),
        Ok(supervised) => info!(supervised, "managed-worker supervision commissioned"),
        Err(error) => error!(%error, "managed-worker fleet could not be converged at boot"),
    }
}

fn transport_result(
    transport: &'static str,
    result: Result<Result<(), ServerError>, tokio::task::JoinError>,
) -> Result<(), ServerError> {
    match result {
        Ok(transport_outcome) => transport_outcome,
        Err(join_error) => Err(ServerError::Transport {
            transport,
            message: join_error.to_string(),
        }),
    }
}

async fn serve_grpc(
    state: ServerState,
    listener: TcpListener,
    address: SocketAddr,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<(), ServerError> {
    let workflow = api::grpc::workflow_service(state.clone());
    let worker = api::worker_grpc::worker_service(state.clone());
    let mut router = TonicServer::builder()
        .add_service(workflow)
        .add_service(worker);
    // Dark by default: the deploy service joins the listener only when the
    // operator commissioned it; otherwise the surface answers Unimplemented.
    if state.runtime_config().deploy.enabled {
        router = router.add_service(api::deploy_grpc::deploy_service(state)?);
    }
    // The pre-bound listener rides in as a `TcpIncoming`, NOT a bare
    // `TcpListenerStream`: tonic applies its socket options (`tcp_nodelay`
    // defaults to true) only in `bind_incoming`, and documents that a raw
    // incoming stream ignores them — a bare stream would silently re-enable
    // Nagle's algorithm on every worker-protocol and client connection.
    let incoming = tonic::transport::server::TcpIncoming::from(listener).with_nodelay(Some(true));
    router
        .serve_with_incoming_shutdown(incoming, shutdown_requested(shutdown))
        .await
        .map_err(|source| transport_bind("grpc", address, source))?;
    Ok(())
}

async fn serve_http(
    state: ServerState,
    listener: TcpListener,
    address: SocketAddr,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<(), ServerError> {
    axum::serve(listener, api::http::http_router(state)?)
        .with_graceful_shutdown(shutdown_requested(shutdown))
        .await
        .map_err(|source| transport_bind("http", address, source))?;
    Ok(())
}

async fn shutdown_requested(mut shutdown: tokio::sync::watch::Receiver<bool>) {
    while !*shutdown.borrow_and_update() {
        if shutdown.changed().await.is_err() {
            break;
        }
    }
}

async fn shutdown_signal() -> Result<(), ServerError> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut terminate = signal(SignalKind::terminate())
            .map_err(|source| signal_listener("SIGTERM", &source))?;
        let mut interrupt =
            signal(SignalKind::interrupt()).map_err(|source| signal_listener("SIGINT", &source))?;
        tokio::select! {
            _ = terminate.recv() => Ok(()),
            _ = interrupt.recv() => Ok(()),
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .map_err(|source| signal_listener("shutdown signal", &source))
    }
}

fn signal_listener(listener: &'static str, source: &std::io::Error) -> ServerError {
    ServerError::SignalListener {
        listener,
        message: source.to_string(),
    }
}

fn reject_auth_without_feature(config: &ServerConfig) -> Result<(), ServerError> {
    if cfg!(not(feature = "auth")) && config.auth.enabled {
        return Err(ServerError::Config {
            message: "auth.enabled=true but binary compiled without auth feature".to_owned(),
        });
    }
    Ok(())
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
async fn rebuild_outbox_boot_state(state: &ServerState, outbox_config: &OutboxConfig) {
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
fn maybe_spawn_outbox_dispatcher(
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
fn maybe_spawn_cluster_supervisor(
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

fn resolve_outbox_reconciler_config(
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

fn reject_tls_until_supported(state: &ServerState) -> Result<(), ServerError> {
    if state.runtime_config().tls.is_some() {
        return Err(ServerError::Config {
            message: "configured TLS material cannot be served until transport TLS is wired"
                .to_owned(),
        });
    }
    Ok(())
}

fn store_backend_label(backend: StoreBackend) -> &'static str {
    match backend {
        StoreBackend::Memory => "memory",
        StoreBackend::Haematite => "haematite",
    }
}

fn namespace_mode_label(mode: &NamespaceMode) -> &'static str {
    match mode {
        NamespaceMode::SharedEngine => "SharedEngine",
        NamespaceMode::SingleTenant { .. } => "SingleTenant",
    }
}

fn transport_bind<E>(transport: &'static str, address: SocketAddr, source: E) -> ServerError
where
    E: std::error::Error,
{
    ServerError::TransportBind {
        transport,
        address,
        message: source.to_string(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{
        BackpressureSettings, OutboxConfig, OutboxTransport, maybe_spawn_outbox_dispatcher,
        resolve_outbox_reconciler_config,
    };
    use crate::ServerState;
    use crate::config::RuntimeConfig;
    use aion_store::InMemoryStore;
    use std::net::SocketAddr;
    use std::time::Duration;

    /// Own-all, generous-default backpressure settings for the gate tests (the
    /// single-node default: fraction 1, so the ceiling never engages).
    fn test_backpressure_settings() -> BackpressureSettings {
        BackpressureSettings {
            platform_default: crate::config::DEFAULT_MAX_IN_FLIGHT_ACTIVITIES,
            fraction: crate::worker::OwnedShardFraction::own_all(),
        }
    }

    /// A minimal `RuntimeConfig` for building an in-memory `ServerState` in unit
    /// tests (mirrors `state.rs`'s test `runtime_config`).
    fn runtime_config() -> RuntimeConfig {
        use crate::config::{
            AuthConfig, AuthoringConfig, DeployConfig, DevConfig, ListenConfig, MetricsConfig,
            NamespaceConfig, NamespaceMode, OpsConsoleAssetSource, OpsConsoleConfig,
            WebSocketConfig, WorkerConfig,
        };
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

    /// An `OutboxConfig` with `enabled = true` and every required knob present, so
    /// the only remaining gate is the store-backend / outbox-table availability.
    fn enabled_outbox_config() -> OutboxConfig {
        OutboxConfig {
            enabled: true,
            poll_interval_ms: Some(250),
            batch_size: Some(64),
            max_attempts: Some(5),
            backoff_base_ms: Some(100),
            backoff_multiplier: Some(2),
            backoff_max_ms: Some(30_000),
            reconcile_interval_ms: None,
            reconcile_stale_after_ms: None,
            transport: OutboxTransport::Grpc,
            liminal_listen_address: None,
        }
    }

    /// LSUB-4-2 / LSUB-4-6 (Memory-backend guard): commissioning the outbox
    /// dispatcher against the in-memory backend (which has no outbox table, so
    /// `outbox_store()` is `None`) is a configuration error, and the message names
    /// haematite as the required durable backend.
    #[tokio::test]
    async fn outbox_enabled_on_memory_backend_is_a_config_error() {
        let state = ServerState::build_with_store(InMemoryStore::default(), runtime_config())
            .await
            .expect("build in-memory state");
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let error = maybe_spawn_outbox_dispatcher(
            &state,
            &enabled_outbox_config(),
            false,
            test_backpressure_settings(),
            &rx,
            "set outbox.liminal_listen_address in the test config",
        )
        .expect_err("outbox.enabled on the memory backend must be a config error");
        assert!(
            error.is_config(),
            "memory-backend outbox error must be Config"
        );
        let message = error.to_string();
        assert!(
            message.contains("store.backend=haematite"),
            "message must name the durable backend, got: {message}"
        );
    }

    /// LSUB-4-1 (Fork-B fast path): with the outbox disabled (the default), the
    /// gate is a no-op even on a memory backend — nothing is spawned and no error
    /// is produced, so a default single-node boot is unchanged.
    #[tokio::test]
    async fn disabled_outbox_is_a_noop_on_any_backend() {
        let state = ServerState::build_with_store(InMemoryStore::default(), runtime_config())
            .await
            .expect("build in-memory state");
        let (_tx, rx) = tokio::sync::watch::channel(false);
        maybe_spawn_outbox_dispatcher(
            &state,
            &OutboxConfig::default(),
            false,
            test_backpressure_settings(),
            &rx,
            "set outbox.liminal_listen_address in the test config",
        )
        .expect("disabled outbox gate must be an infallible no-op");
    }

    /// LSUB-4-4: the reconciler config resolves to `None` unless BOTH knobs are
    /// set — the condition under which the clustered-boot WARN fires.
    #[test]
    fn reconciler_config_absent_unless_both_knobs_set() {
        let mut config = enabled_outbox_config();
        // Neither knob: absent.
        assert!(
            resolve_outbox_reconciler_config(&config)
                .expect("resolve")
                .is_none()
        );
        // Only interval: still absent (the silent-backstop-absent default).
        config.reconcile_interval_ms = Some(1_000);
        assert!(
            resolve_outbox_reconciler_config(&config)
                .expect("resolve")
                .is_none()
        );
        // Both set: present.
        config.reconcile_stale_after_ms = Some(60_000);
        assert!(
            resolve_outbox_reconciler_config(&config)
                .expect("resolve")
                .is_some()
        );
    }

    /// LSUB-PROD (13-6): the liminal transport requires `liminal_listen_address`.
    /// Commissioning the dispatcher with `transport = liminal` but no listen
    /// address is a configuration error naming the missing knob, rather than a
    /// panic or a silent fall-through to gRPC. Built over haematite (so
    /// the outbox-store gate passes and the missing-address check is actually
    /// reached). (Feature-gated: the liminal arm of `build_liminal_row_dispatch`
    /// only exists with `liminal-transport` on; in a feature-off build the same
    /// selection is the missing-feature error instead, covered by the type system
    /// rather than this test.)
    #[cfg(feature = "liminal-transport")]
    #[tokio::test]
    async fn liminal_transport_requires_listen_address() {
        use crate::config::{
            RuntimeSection, ServerConfig, StoreBackend, StoreConfig, WebSocketConfig,
        };

        let data_dir = std::env::temp_dir().join(format!(
            "aion-lsub-prod-listen-guard-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        let mut outbox = enabled_outbox_config();
        outbox.transport = OutboxTransport::Liminal;
        outbox.liminal_listen_address = None;
        let config = ServerConfig {
            store: StoreConfig {
                backend: StoreBackend::Haematite,
                data_dir: Some(data_dir.to_string_lossy().into_owned()),
                // Required, no default: the haematite boot path refuses a config
                // that does not rule on the node cache's byte ceiling.
                node_cache_budget: Some(haematite::NodeCacheBudget::Unlimited),
                ..StoreConfig::default()
            },
            runtime: RuntimeSection {
                scheduler_threads: 1,
                jit_threshold: None,
                query_timeout_ms: Some(10_000),
            },
            websocket: WebSocketConfig {
                outbound_buffer_bound: 32,
                event_broadcast_capacity: Some(64),
                cluster_broadcast_capacity: Some(64),
            },
            outbox: outbox.clone(),
            // Required, no default: the transcript drain's flush policy.
            observability: crate::config::ObservabilityConfig::with_flush_policy(64, 0),
            ..ServerConfig::default()
        };
        let state = ServerState::build(config)
            .await
            .expect("build haematite state");
        let (_tx, rx) = tokio::sync::watch::channel(false);

        let error = maybe_spawn_outbox_dispatcher(
            &state,
            &outbox,
            false,
            test_backpressure_settings(),
            &rx,
            "add `liminal_listen_address = \"127.0.0.1:50061\"` to `[outbox]` in the test config",
        )
        .expect_err("liminal transport without a listen address must be a config error");
        assert!(
            error.is_config(),
            "missing-listen-address error must be Config"
        );
        assert!(
            error.to_string().contains("liminal_listen_address"),
            "error must name the missing knob, got: {error}"
        );
        // #180 review MAJ-4: the refusal must carry the caller's threaded
        // where-to-edit hint, so the production message names the resolved
        // config FILE, not just the key.
        assert!(
            error.to_string().contains("in the test config"),
            "error must carry the threaded config-location hint, got: {error}"
        );
    }
}

/// LSUB-PROD (13-6): production-boot cross-node round-trip over the REAL wiring.
///
/// This is the proof that the production boot now does the full round-trip the
/// retired stub could not. It drives the EXACT production commissioning function
/// `run_server` calls — [`maybe_spawn_outbox_dispatcher`] — over a real
/// [`ServerState`] built with `outbox.enabled`, `transport = liminal`, and a
/// `liminal_listen_address`. That function lifts the full push wiring
/// (`build_liminal_row_dispatch`): it hosts the liminal worker listener, builds
/// [`RegistryLiminalDispatch`](crate::worker::RegistryLiminalDispatch) over the
/// SAME registry the gRPC path uses and the SAME
/// [`ServerOutboxDeliveryCallback`](crate::worker::ServerOutboxDeliveryCallback)
/// (over the live engine), and spawns the real [`OutboxDispatcher`].
///
/// A REAL remote [`LiminalActivityWorker`](aion_worker::LiminalActivityWorker)
/// connects IN to the listener and self-registers in-band. A `collect_four`
/// fan-out is started over the REAL HTTP transport, which stages four pending
/// outbox rows; the production-wired dispatcher claims and pushes each to the
/// worker, the worker executes it, and its completion re-enters aion through the
/// production engine callback — `record_fan_out_completion` — driving the
/// workflow to a recorded terminal. The proof asserts BOTH: the worker observably
/// executed the activities, AND the terminals were recorded in history (four
/// `ActivityCompleted` + one `WorkflowCompleted`), which the stub's
/// publish-and-mark-done path never achieved.
#[cfg(all(test, feature = "liminal-transport"))]
mod lsub_prod_xnode_e2e {
    #![allow(clippy::expect_used)]

    use std::net::SocketAddr;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    use aion_core::Event;
    use aion_package::{
        ActionContract, BeamModule, BeamSet, CURRENT_FORMAT_VERSION, DeclaredActivity, Manifest,
        ManifestVersion, PackageBuilder, PackageContract, WorkerContract,
    };
    use aion_worker::{ActivityRegistry, LiminalActivityWorker, WorkerConfig};
    use axum::body;
    use axum::http::{Request, StatusCode};
    use serde_json::json;
    use tower::ServiceExt;

    use super::{BackpressureSettings, maybe_spawn_outbox_dispatcher};
    use crate::ServerState;
    use crate::api::http::http_router;
    use crate::config::{
        OutboxConfig, OutboxTransport, RuntimeSection, ServerConfig, StoreBackend, StoreConfig,
        WebSocketConfig,
    };

    type TestError = Box<dyn std::error::Error + Send + Sync>;

    /// The `collect_four` fixture passes each member the JSON string `"in"` as
    /// activity input, so the worker handler decodes a [`String`], not a struct.
    type FanInput = String;

    const NAMESPACE: &str = "default";
    const TASK_QUEUE: &str = "default";
    const OUTBOX_MODULE: &str = "aion_outbox_fixture";
    const OUTBOX_BEAM: &[u8] = include_bytes!("../tests/fixtures/aion_outbox_fixture.beam");
    const OUTBOX_SOURCE: &[u8] = include_bytes!("../tests/fixtures/aion_outbox_fixture.erl");
    const FAN_OUT: usize = 4;
    const FAN_ACTIVITY_TYPES: [&str; FAN_OUT] = ["fan:0", "fan:1", "fan:2", "fan:3"];
    const POLL_DEADLINE: Duration = Duration::from_secs(20);
    /// The one fan-out member the reconnect pin holds. Any of the four would do —
    /// they are dispatched independently and served by identical handlers.
    const HELD_ACTIVITY_TYPE: &str = FAN_ACTIVITY_TYPES[0];

    fn test_error(message: impl std::fmt::Display) -> TestError {
        message.to_string().into()
    }

    /// Reserve a loopback port and return it: the liminal listener binds this exact
    /// address (the production path binds the configured `liminal_listen_address`,
    /// so the test must commit to a concrete port the worker can also dial).
    fn reserve_loopback_port() -> Result<SocketAddr, TestError> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(test_error)?;
        let address = listener.local_addr().map_err(test_error)?;
        drop(listener);
        Ok(address)
    }

    /// The fixture's queue-scoped `.v4` contract: the four `fan:N` activities
    /// `collect_four` schedules, declared on the queue its worker actually polls.
    ///
    /// Why the archive cannot just carry the manifest-derived record: by design
    /// `PackageContract::from_manifest` "never invents a queue", so a manifest's
    /// bare activity names land in `unscoped_activities` — and this server boots
    /// queue-routed, where an unscoped catalog is a terminal
    /// `NO_QUEUE_DECLARATION` at start admission
    /// (`aion::lifecycle::start_admission`). That refusal is EARNED: an unserved
    /// queue would otherwise wait silently forever. So the derived record is
    /// amended rather than bypassed — the same four names move out of
    /// `unscoped_activities` and onto the queue that serves them — and the
    /// package still loads through the production boot path with the `.v4`
    /// identity `PackageBuilder` stamps over this exact contract.
    ///
    /// The action schemas come from the SAME generator the worker's typed
    /// registry uses, for the SAME Rust types: `collect_four` passes each member
    /// the JSON string `"in"` and the handler returns a [`String`]. Deriving both
    /// sides from `activity_descriptor::<FanInput, String>` means the package's
    /// declaration and the worker's advertisement cannot drift apart, so
    /// registration admission (`WORKER_CONTRACT_MISMATCH`) compares two schemas
    /// with one source.
    fn fixture_contract(manifest: &Manifest) -> Result<PackageContract, TestError> {
        let mut actions = Vec::with_capacity(FAN_ACTIVITY_TYPES.len());
        for activity_type in FAN_ACTIVITY_TYPES {
            let descriptor = aion_worker::activity_descriptor::<FanInput, String>(activity_type)
                .map_err(test_error)?;
            actions.push(ActionContract {
                name: descriptor.name,
                input_schema: descriptor.input_schema,
                output_schema: descriptor.output_schema,
                node: None,
                timeout: None,
                retry: None,
                advisory: false,
                // A typed `String -> String` handler serves these, not an agent
                // harness — the fan fixture's shape merely coincides with an
                // agent seam's, and marking it would route it somewhere no
                // handler is.
                agent: false,
                // A connected worker serves this fixture's queue, so the
                // declaration carries no body of its own.
                body: None,
            });
        }
        let mut contract = PackageContract::from_manifest(manifest);
        contract.workers = vec![WorkerContract {
            task_queue: TASK_QUEUE.to_owned(),
            actions,
        }];
        contract.unscoped_activities.clear();
        Ok(contract)
    }

    /// Build the `collect_four` package on disk so the production state-build path
    /// loads it exactly as it loads operator-supplied `workflow_packages`.
    fn write_package_archive(dir: &std::path::Path) -> Result<PathBuf, TestError> {
        let beams =
            BeamSet::new(vec![BeamModule::new(OUTBOX_MODULE, OUTBOX_BEAM)]).map_err(test_error)?;
        let manifest = Manifest {
            entry_module: OUTBOX_MODULE.to_owned(),
            entry_function: "collect_four".to_owned(),
            input_schema: json!({ "type": "object" }),
            output_schema: json!({}),
            timeout: Some(Duration::from_secs(30)),
            // The four ordinals `collect_four` actually fans out. This manifest
            // used to name one invented activity, `fixture_activity`, that the
            // fixture never schedules and no worker ever served.
            activities: FAN_ACTIVITY_TYPES
                .iter()
                .map(|activity_type| DeclaredActivity {
                    activity_type: (*activity_type).to_owned(),
                })
                .collect(),
            version: ManifestVersion::new("stamped-by-builder"),
            format_version: CURRENT_FORMAT_VERSION,
            additional_workflows: Vec::new(),
        };
        let contract = fixture_contract(&manifest)?;
        let archive =
            PackageBuilder::with_source(manifest, beams, [(OUTBOX_MODULE, OUTBOX_SOURCE.to_vec())])
                .with_contract(contract)
                .write_to_bytes()
                .map_err(test_error)?;
        let path = dir.join("collect_four.aion");
        std::fs::write(&path, archive).map_err(test_error)?;
        Ok(path)
    }

    /// A production-shaped `ServerConfig`: the haematite backend (so the boot store
    /// path shares the leaf as the dispatcher's outbox store, exactly as
    /// `ServerState::build` does in production), `outbox.enabled`,
    /// `transport = liminal`, the reserved `liminal_listen_address`, and the
    /// `collect_four` package. Built through `ServerState::build` (not
    /// `build_with_store`), so this is the real boot store seam, not a test stand-in.
    fn server_config(
        data_dir: &std::path::Path,
        package_path: PathBuf,
        listen_address: SocketAddr,
    ) -> ServerConfig {
        ServerConfig {
            store: StoreConfig {
                backend: StoreBackend::Haematite,
                data_dir: Some(data_dir.to_string_lossy().into_owned()),
                // Required, no default: the haematite boot path refuses a config
                // that does not rule on the node cache's byte ceiling.
                node_cache_budget: Some(haematite::NodeCacheBudget::Unlimited),
                ..StoreConfig::default()
            },
            runtime: RuntimeSection {
                scheduler_threads: 1,
                jit_threshold: None,
                query_timeout_ms: Some(10_000),
            },
            websocket: WebSocketConfig {
                outbound_buffer_bound: 32,
                event_broadcast_capacity: Some(64),
                cluster_broadcast_capacity: Some(64),
            },
            workflow_packages: vec![package_path],
            outbox: OutboxConfig {
                enabled: true,
                poll_interval_ms: Some(20),
                batch_size: Some(16),
                max_attempts: Some(5),
                backoff_base_ms: Some(50),
                backoff_multiplier: Some(2),
                backoff_max_ms: Some(1_000),
                reconcile_interval_ms: None,
                reconcile_stale_after_ms: None,
                transport: OutboxTransport::Liminal,
                liminal_listen_address: Some(listen_address.to_string()),
            },
            // Required, no default: the transcript drain's flush policy.
            observability: crate::config::ObservabilityConfig::with_flush_policy(64, 0),
            ..ServerConfig::default()
        }
    }

    /// The remote worker self-describes for the fixture's pool `(default, default)`
    /// and registers a handler for every `fan:N` activity type, counting executions
    /// so the test proves it genuinely ran the pushed dispatches.
    fn worker_config() -> Result<WorkerConfig, TestError> {
        WorkerConfig::builder()
            .endpoint("unused-direct-address")
            .namespace(NAMESPACE)
            .task_queue(TASK_QUEUE)
            .identity("lsub-prod-worker")
            .max_concurrency(4)
            .reconnect_initial_backoff(Duration::from_millis(5))
            .reconnect_max_backoff(Duration::from_millis(20))
            .reconnect_max_attempts(3)
            .build()
            .map_err(test_error)
    }

    fn worker_registry(executions: &Arc<AtomicUsize>) -> Result<Arc<ActivityRegistry>, TestError> {
        let mut registry = ActivityRegistry::new();
        for activity_type in FAN_ACTIVITY_TYPES {
            let executions = Arc::clone(executions);
            // `register_activity_with_contract`, not `register_activity`: the
            // bare form registers a handler with NO descriptor, so the worker
            // advertises four names and zero typed contracts, and admission —
            // which compares CONTRACTS — refuses the registration outright
            // (`WORKER_CONTRACT_MISMATCH`). Deriving the advertisement from
            // `<FanInput, String>` is what makes it the same source the
            // package's `fixture_contract` declares from, so the two sides
            // cannot drift.
            registry = registry
                .register_activity_with_contract(
                    activity_type,
                    move |_input: FanInput, _context| {
                        let executions = Arc::clone(&executions);
                        Box::pin(async move {
                            executions.fetch_add(1, Ordering::SeqCst);
                            Ok(activity_type.to_owned())
                        })
                    },
                )
                .map_err(test_error)?;
        }
        Ok(Arc::new(registry))
    }

    /// Spawns the remote worker on its own OS thread with a current-thread runtime
    /// (the push receive is blocking), connecting IN to the production listener.
    struct WorkerThread {
        stop: Arc<std::sync::atomic::AtomicBool>,
        handle: Option<std::thread::JoinHandle<()>>,
    }

    impl WorkerThread {
        fn spawn(address: String, config: WorkerConfig, registry: Arc<ActivityRegistry>) -> Self {
            let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let thread_stop = Arc::clone(&stop);
            let handle = std::thread::spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        eprintln!("worker runtime build failed: {error}");
                        return;
                    }
                };
                runtime.block_on(async move {
                    let worker = match LiminalActivityWorker::connect(&address, &config, registry) {
                        Ok(worker) => worker,
                        Err(error) => {
                            eprintln!("worker connect failed: {error}");
                            return;
                        }
                    };
                    if let Err(error) = worker
                        .serve_until(|| thread_stop.load(Ordering::SeqCst))
                        .await
                    {
                        eprintln!("worker serve loop ended with error: {error}");
                    }
                });
            });
            Self {
                stop,
                handle: Some(handle),
            }
        }

        /// Spawn the worker through [`aion_worker::serve_with_redial`] — the entry
        /// point every REAL worker uses — so a broken link is survivable.
        ///
        /// [`Self::spawn`] uses `LiminalActivityWorker::serve_until`, which returns
        /// the first transport error by design: a single-connection serve has no
        /// survivor to migrate to. That is the right shape for a test whose link
        /// never breaks, and the wrong instrument entirely for one whose link is
        /// broken on purpose — a worker that dies at the break can only ever show
        /// that outstanding work fails, whoever is at fault.
        ///
        /// The redial driver is SYNCHRONOUS and builds its own current-thread
        /// runtime, so it runs on the bare thread rather than inside one.
        fn spawn_redialing(
            address: String,
            config: WorkerConfig,
            registry: Arc<ActivityRegistry>,
            timing: aion_worker::RedialTiming,
        ) -> Self {
            let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let thread_stop = Arc::clone(&stop);
            let handle = std::thread::spawn(move || {
                if let Err(error) = aion_worker::serve_with_redial(
                    vec![address],
                    &config,
                    &registry,
                    timing,
                    &thread_stop,
                    None,
                    || {},
                ) {
                    eprintln!("redialing worker ended with error: {error}");
                }
            });
            Self {
                stop,
                handle: Some(handle),
            }
        }

        fn stop(mut self) {
            self.stop.store(true, Ordering::SeqCst);
            if let Some(handle) = self.handle.take() {
                handle.join().ok();
            }
        }
    }

    fn count_completed(history: &[Event]) -> usize {
        history
            .iter()
            .filter(|event| matches!(event, Event::ActivityCompleted { .. }))
            .count()
    }

    fn count_workflow_completed(history: &[Event]) -> usize {
        history
            .iter()
            .filter(|event| matches!(event, Event::WorkflowCompleted { .. }))
            .count()
    }

    async fn wait_for_history<F>(
        store: &dyn aion_store::ReadableEventStore,
        workflow_id: &aion_core::WorkflowId,
        description: &str,
        predicate: F,
    ) -> Result<Vec<Event>, TestError>
    where
        F: Fn(&[Event]) -> bool,
    {
        let deadline = Instant::now() + POLL_DEADLINE;
        loop {
            let history = store.read_history(workflow_id).await.map_err(test_error)?;
            if predicate(&history) {
                return Ok(history);
            }
            if Instant::now() > deadline {
                return Err(test_error(format!(
                    "timed out waiting for {description}: {history:#?}"
                )));
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    /// Start the loaded `collect_four` workflow over the REAL HTTP transport.
    async fn start_over_http(router: &axum::Router) -> Result<aion_core::WorkflowId, TestError> {
        let build_request = || -> Result<Request<body::Body>, TestError> {
            Request::builder()
                .uri("/workflows/start")
                .method("POST")
                .header("content-type", "application/json")
                .header("x-aion-subject", "ci")
                .header("x-aion-namespaces", NAMESPACE)
                .body(body::Body::from(
                    serde_json::to_vec(&json!({
                        "namespace": NAMESPACE,
                        "workflow_type": OUTBOX_MODULE,
                        "input": { "fixture": "input" },
                    }))
                    .map_err(test_error)?,
                ))
                .map_err(test_error)
        };
        let response = router
            .clone()
            .oneshot(build_request()?)
            .await
            .map_err(test_error)?;
        let status = response.status();
        let bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .map_err(test_error)?
            .to_vec();
        if status != StatusCode::OK {
            return Err(test_error(format!(
                "workflow start over HTTP must succeed, got {status}: {}",
                String::from_utf8_lossy(&bytes)
            )));
        }
        let body: serde_json::Value = serde_json::from_slice(&bytes).map_err(test_error)?;
        // The HTTP wire contract (`clean_dtos::StartWorkflowResponse`) serializes
        // `workflow_id` as a plain UUID string, not a nested `{ uuid }` object.
        let workflow_id = body["workflow_id"]
            .as_str()
            .ok_or_else(|| test_error("start response missing workflow id"))?
            .parse::<uuid::Uuid>()
            .map_err(test_error)?;
        Ok(aion_core::WorkflowId::new(workflow_id))
    }

    /// How long a freshly connected worker needs before the dispatch path may
    /// select it, DERIVED from the same two facts the server derives it from.
    ///
    /// A worker is dispatch-ineligible until it serves an OPENING PROBATION:
    /// [`Reachability::is_proved`] requires `DISPATCH_PROBATION_PINGS` consecutive
    /// answered liveness pings, at the probe's cadence of
    /// [`sweep_interval`](crate::worker::sweep_interval)`(heartbeat_window)`. The
    /// constant's own documentation states the cost — *"at the probe's cadence a
    /// fresh worker is undispatchable for K cadences while its first dispatches
    /// park"* — so this is designed behaviour a test must wait out, not a delay to
    /// be shortened.
    ///
    /// One extra cadence is allowed because the first round lands at an arbitrary
    /// offset inside the first interval: the worker connects between rounds, so it
    /// can miss up to one whole cadence before its first answer is even counted.
    ///
    /// # Why this is not a raised timeout
    ///
    /// It was 5 seconds, fixed, and that is how this test became one of four
    /// documented carriers of a load-sensitive flake
    /// (`gate-logs/lock-race-attribution/VERDICT.md`). The mechanism, measured:
    /// `dispatch_ineligible` starts EMPTY and `select_worker` filters only against
    /// what the probe has published, so a run in which **no probe round lands
    /// inside the window** selects the worker immediately and passes, while a run
    /// in which one does correctly withholds it for ~2 cadences and fails. On the
    /// default 30s window that is 7.5s per cadence against a 5s wait.
    ///
    /// 🔴 The passing runs were the WRONG ones. They dispatched to a worker that
    /// had not served its probation — a path production does not permit, because
    /// production parks those dispatches. Waiting for genuine eligibility makes
    /// this test MORE production-shaped, not more lenient, and that is the reason
    /// to do it. Raising a bound until a flake stops is how a liveness bug gets
    /// buried; deriving the bound from the mechanism that sets it is not the same
    /// act, and the register warns about the first for good reason.
    fn eligibility_patience(config: &ServerConfig) -> Duration {
        let cadence = crate::worker::sweep_interval(config.worker.heartbeat_window);
        cadence * (crate::worker::heartbeat::DISPATCH_PROBATION_PINGS + 1)
    }

    /// Wait until the worker's in-band registration lands in the SAME registry the
    /// dispatch path selects from, with every fan-out activity type eligible.
    ///
    /// On the deadline this reports the state that DISCRIMINATES the worlds a
    /// missed registration can be in, because the bare sentence it replaced —
    /// "worker never registered in-band for the pool" — is equally true in at
    /// least three of them, and they want different fixes:
    ///
    /// 1. the liminal listener never bound, so nothing could dial in;
    /// 2. the worker never connected, or died dialling;
    /// 3. it connected and registration was merely slow;
    /// 4. it connected, registered correctly, and the SELECTOR refused it anyway —
    ///    because the liveness probe published it as unreachable, or because it is
    ///    not indexed for the activity type it advertises.
    ///
    /// The fourth was not in the first version of this report, and it is the world
    /// a real occurrence turned out to be in: the listener was bound, a worker was
    /// registered under the right namespace and queue advertising all four activity
    /// types, and every `select_worker` still returned nothing. A report that
    /// cannot separate "not registered" from "registered and refused" names the
    /// wrong half of the system.
    ///
    /// That is not a hypothetical distinction here. This module's e2e is one of
    /// four documented carriers of a load-sensitive flake
    /// (`gate-logs/lock-race-attribution/VERDICT.md`), it fails through THIS wait,
    /// and the reason the carrier has never been explained is that the failure
    /// named the fact and withheld the cause.
    async fn wait_for_registration(
        registry: &crate::worker::ConnectedWorkerRegistry,
        heartbeat: &crate::worker::HeartbeatTracker,
        listen_address: SocketAddr,
        patience: Duration,
    ) -> Result<(), TestError> {
        let deadline = Instant::now() + patience;
        loop {
            let now = Instant::now();
            let mut ready = true;
            for activity_type in FAN_ACTIVITY_TYPES {
                let Some(worker) = registry
                    .select_worker(NAMESPACE, TASK_QUEUE, activity_type, None)
                    .map_err(test_error)?
                else {
                    ready = false;
                    break;
                };
                if !heartbeat
                    .is_dispatch_reachable(worker.id(), now)
                    .map_err(test_error)?
                {
                    ready = false;
                    break;
                }
            }
            if ready {
                return Ok(());
            }
            if Instant::now() > deadline {
                return Err(test_error(format!(
                    "worker never registered in-band for the pool within {patience:?}{}",
                    registration_diagnosis(registry, listen_address)
                )));
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// The discriminator behind [`wait_for_registration`]'s failure: enough of the
    /// world to tell those three apart, gathered at the moment of the failure.
    fn registration_diagnosis(
        registry: &crate::worker::ConnectedWorkerRegistry,
        listen_address: SocketAddr,
    ) -> String {
        let mut lines = vec![String::from("--- registration diagnosis ---")];
        // World 1, PROBED rather than assumed. The port was reserved by binding a
        // listener and dropping it, so losing the race for it is a real
        // possibility rather than a theoretical one, and it is indistinguishable
        // from every other failure unless something asks.
        lines.push(
            match std::net::TcpStream::connect_timeout(&listen_address, Duration::from_millis(500))
            {
                Ok(stream) => {
                    drop(stream);
                    format!("listener {listen_address}: ACCEPTS — the port is bound and dialable")
                }
                Err(error) => format!(
                    "listener {listen_address}: NOT connectable ({error}) — nothing could have \
                     registered, so this is not a timing problem"
                ),
            },
        );
        // Worlds 2 and 3: did any worker arrive at all, and if one did, what does
        // the registry hold for it against what the dispatch path asks of it? A
        // worker present under a different pool or advertising different activity
        // types is a contract mismatch wearing a timeout's clothes.
        match registry.all_workers() {
            Err(error) => lines.push(format!("registry: UNREADABLE ({error})")),
            Ok(workers) if workers.is_empty() => lines.push(String::from(
                "registry: EMPTY — no worker of any pool registered, so no connection ever \
                 completed an in-band registration",
            )),
            Ok(workers) => {
                lines.push(format!("registry: {} worker(s) registered", workers.len()));
                for worker in &workers {
                    lines.push(format!(
                        "  id={:?} namespaces={:?} task_queue={:?} node={:?} types={:?}",
                        worker.id(),
                        worker.namespaces(),
                        worker.task_queue(),
                        worker.node(),
                        worker.activity_types()
                    ));
                }
            }
        }
        lines.push(format!(
            "asked of it: namespace={NAMESPACE:?} task_queue={TASK_QUEUE:?}"
        ));
        // The liveness probe's reachability verdict. `select_worker` skips every
        // worker in this set, so a registered, correctly-advertised worker that is
        // listed here is refused for a reason nothing else in this report shows.
        lines.push(match registry.dispatch_ineligible() {
            Ok(ineligible) if ineligible.is_empty() => {
                String::from("dispatch-ineligible: none — reachability is not refusing anyone")
            }
            Ok(ineligible) => format!(
                "dispatch-ineligible: {ineligible:?} — the liveness probe has published these \
                 as unreachable and select_worker skips them"
            ),
            Err(error) => format!("dispatch-ineligible: UNREADABLE ({error})"),
        });
        // Which of the four the selector could not satisfy, and — the part that
        // discriminates — the pool census beside each refusal.
        //
        // `select_worker` filters on THREE things: the activity index for
        // `(namespace, task_queue) + activity_type`, the node pin, and the
        // dispatch-ineligible set. The census counts the first two and does NOT
        // apply the third, so the pair of answers separates the remaining worlds
        // that a registry dump alone leaves fused:
        //
        // - census serves it, selector refuses  ⇒ REACHABILITY, not registration;
        // - census serves 0 for the activity    ⇒ the worker is in the pool but not
        //   indexed for this activity type;
        // - census serves 0 for the pool        ⇒ it is not in this pool at all,
        //   whatever `all_workers` shows.
        //
        // Written after the bare registry dump above failed to close a real case:
        // it proved the listener was bound and a worker with all four activity
        // types was registered, and still could not say why every selection
        // returned nothing.
        for activity_type in FAN_ACTIVITY_TYPES {
            let outcome = match registry.select_worker(NAMESPACE, TASK_QUEUE, activity_type, None) {
                Ok(Some(handle)) => format!("worker {:?}", handle.id()),
                Ok(None) => String::from("NO worker"),
                Err(error) => format!("error: {error}"),
            };
            let census = match registry.pool_census(NAMESPACE, TASK_QUEUE, activity_type, None) {
                Ok(census) => format!(
                    "in_pool={} serving_activity={} compatible={} last_compatible_age={:?}",
                    census.workers_in_pool,
                    census.workers_serving_activity,
                    census.compatible_workers,
                    census.last_compatible_poller_age
                ),
                Err(error) => format!("census UNREADABLE ({error})"),
            };
            lines.push(format!(
                "select_worker({activity_type}) -> {outcome}  [census: {census}]"
            ));
        }
        format!("\n  {}", lines.join("\n  "))
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn production_boot_dispatches_executes_and_records_over_liminal() -> Result<(), TestError>
    {
        let dir = crate::test_support::private_tempdir().map_err(test_error)?;
        let db_path = dir.path().join("aion.db");
        let package_path = write_package_archive(dir.path())?;
        // The production path binds the CONFIGURED listen address, so commit to a
        // concrete reserved loopback port the worker can also dial.
        let listen_address = reserve_loopback_port()?;

        // (A) Build a real ServerState through the production boot path
        // (ServerState::build over a haematite ServerConfig): outbox enabled,
        // transport = liminal, the listen address set, collect_four loaded. This
        // shares the haematite leaf as the dispatcher's outbox store (the real boot
        // store seam) and installs the production ServerOutboxDeliveryCallback over
        // the live engine (gated on outbox.enabled).
        let config = server_config(&db_path, package_path, listen_address);
        let outbox_config = config.outbox.clone();
        // Captured before `build` consumes the config: the wait below is derived
        // from the very window this server is about to run its liveness probe on.
        let patience = eligibility_patience(&config);
        let state = ServerState::build(config).await.map_err(test_error)?;

        // (B) Drive the EXACT production commissioning function run_server calls:
        // it hosts the liminal listener, builds RegistryLiminalDispatch over the
        // shared registry + engine callback, and spawns the real OutboxDispatcher.
        // Hold the returned listener guard for the test's lifetime, exactly as
        // run_server holds it.
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        // Own-all, generous-default backpressure (single-node e2e): fraction 1 and
        // the platform default, so the ceiling never engages — the claim behaves
        // exactly as before, proving the production path is byte-identical on default.
        let backpressure_settings = BackpressureSettings {
            platform_default: crate::config::DEFAULT_MAX_IN_FLIGHT_ACTIVITIES,
            fraction: crate::worker::OwnedShardFraction::own_all(),
        };
        let listener_guard = maybe_spawn_outbox_dispatcher(
            &state,
            &outbox_config,
            false,
            backpressure_settings,
            &shutdown_rx,
            "set outbox.liminal_listen_address in the test config",
        )
        .map_err(test_error)?;

        // (C) A REAL remote worker connects IN to the production listener and
        // self-registers in-band for the fixture's pool.
        let executions = Arc::new(AtomicUsize::new(0));
        let worker = WorkerThread::spawn(
            listen_address.to_string(),
            worker_config()?,
            worker_registry(&executions)?,
        );

        // Wait until the in-band registration landed in the SAME registry the
        // dispatch path selects from (every fan-out activity type is eligible).
        let registry = state.worker_registry().clone();
        if let Err(error) = wait_for_registration(
            &registry,
            state.heartbeat_tracker(),
            listen_address,
            patience,
        )
        .await
        {
            worker.stop();
            return Err(error);
        }

        // (D) Start collect_four over the REAL HTTP transport: the engine stages
        // four pending outbox rows; the production-wired dispatcher claims and
        // pushes each to the worker.
        let router = http_router(state.clone()).map_err(test_error)?;
        let workflow_id = start_over_http(&router).await?;

        // (E) THE PROOF: the worker executed all four activities AND every terminal
        // was recorded through the production engine callback (record_fan_out_completion)
        // — four ActivityCompleted + one WorkflowCompleted in durable history. This
        // is the full round-trip the retired stub never achieved.
        let reader = state.engine().map_err(test_error)?.store();
        let settled =
            wait_for_history(reader.as_ref(), &workflow_id, "fan-out settled", |events| {
                count_completed(events) == FAN_OUT && count_workflow_completed(events) == 1
            })
            .await?;
        assert_eq!(
            count_completed(&settled),
            FAN_OUT,
            "every fan-out member must record a terminal through the production callback"
        );
        assert_eq!(
            count_workflow_completed(&settled),
            1,
            "the workflow must complete exactly once"
        );
        assert_eq!(
            executions.load(Ordering::SeqCst),
            FAN_OUT,
            "the remote worker must have executed every pushed dispatch exactly once"
        );

        // Teardown: stop the dispatcher + worker, drop the listener guard (its Drop
        // stops the accept worker), shut the engine down so durable appends finish.
        shutdown_tx.send(true).ok();
        worker.stop();
        drop(listener_guard);
        state.shutdown().map_err(test_error)?;
        Ok(())
    }

    /// One dispatch as the WORKER saw it: the identity the server sent it under,
    /// and when it arrived.
    #[derive(Clone, Debug)]
    struct SeenDispatch {
        activity_type: String,
        activity_id: String,
        attempt: u32,
        at: Instant,
    }

    /// A loopback TCP relay the test can BREAK, sitting between the worker and the
    /// production liminal listener.
    ///
    /// The worker dials this instead of the listener, so the test owns a socket it
    /// can shut from the outside. That is the only way to make a REAL
    /// [`LiminalActivityWorker`] lose its connection mid-flight without reaching
    /// inside either the worker or the server — and a link broken from the inside
    /// would be a different experiment, because the code under test would be the
    /// code doing the breaking.
    ///
    /// # Why this is not the relay in `tests/dead_man_switch_e2e.rs`
    ///
    /// That file has `WedgeableRelay`, which can both wedge and sever, and this is
    /// deliberately not it. The two cannot be one, for a structural reason rather
    /// than a matter of taste: an integration test links this crate as an ordinary
    /// dependency, so it can see neither `#[cfg(test)] pub(crate) mod test_support`
    /// nor the private `maybe_spawn_outbox_dispatcher` this harness is built on,
    /// and `src/` cannot see `tests/`. Sharing one instrument would mean exporting
    /// a public, feature-gated test surface from a production crate.
    ///
    /// So the split is stated rather than hidden, and this half is a strict subset:
    /// it only severs. Wedging — which leaves both sockets open and merely discards
    /// bytes, so writes keep succeeding into the kernel buffer — is a DIFFERENT
    /// instrument answering a different question. #69 is about a broken link, not
    /// a silent one.
    struct SeverableRelay {
        address: SocketAddr,
        /// Every relayed socket, held so [`Self::sever`] can break them.
        sockets: Arc<std::sync::Mutex<Vec<std::net::TcpStream>>>,
        stop: Arc<std::sync::atomic::AtomicBool>,
        handle: Option<std::thread::JoinHandle<()>>,
    }

    impl SeverableRelay {
        /// Bind a loopback port and relay every accepted connection to `upstream`.
        fn spawn(upstream: SocketAddr) -> Result<Self, TestError> {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(test_error)?;
            let address = listener.local_addr().map_err(test_error)?;
            // Non-blocking accept so the relay can be shut down deterministically
            // rather than by parking a thread in `accept` until something happens
            // to connect. Accepted sockets are put back into blocking mode
            // explicitly: on this platform they would otherwise inherit the flag
            // and every pump would spin on `WouldBlock`.
            listener.set_nonblocking(true).map_err(test_error)?;
            let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let sockets: Arc<std::sync::Mutex<Vec<std::net::TcpStream>>> =
                Arc::new(std::sync::Mutex::new(Vec::new()));
            let accept_stop = Arc::clone(&stop);
            let accept_sockets = Arc::clone(&sockets);
            let handle = std::thread::spawn(move || {
                while !accept_stop.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((downstream, _)) => {
                            if let Err(error) =
                                Self::relay_one(&downstream, upstream, &accept_sockets)
                            {
                                // The worker redials, so a connection this relay
                                // fails to carry surfaces as a slower recovery
                                // rather than as a wrong answer — but silence here
                                // would make that indistinguishable from the
                                // server never pushing, which is exactly the
                                // confusion this pin exists to resolve.
                                eprintln!("relay could not carry a connection: {error}");
                            }
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(2));
                        }
                        Err(error) => {
                            eprintln!("relay accept failed: {error}");
                            return;
                        }
                    }
                }
            });
            Ok(Self {
                address,
                sockets,
                stop,
                handle: Some(handle),
            })
        }

        /// Dial upstream for one accepted connection and pump both directions.
        fn relay_one(
            downstream: &std::net::TcpStream,
            upstream: SocketAddr,
            sockets: &Arc<std::sync::Mutex<Vec<std::net::TcpStream>>>,
        ) -> Result<(), TestError> {
            downstream.set_nonblocking(false).map_err(test_error)?;
            let up = std::net::TcpStream::connect(upstream).map_err(test_error)?;
            let down_read = downstream.try_clone().map_err(test_error)?;
            let down_write = downstream.try_clone().map_err(test_error)?;
            let up_read = up.try_clone().map_err(test_error)?;
            let up_write = up.try_clone().map_err(test_error)?;
            let held = downstream.try_clone().map_err(test_error)?;
            let mut parked = sockets
                .lock()
                .map_err(|_| test_error("relay socket register poisoned"))?;
            parked.push(held);
            parked.push(up);
            drop(parked);
            for (from, to) in [(down_read, up_write), (up_read, down_write)] {
                std::thread::spawn(move || Self::pump(from, to));
            }
            Ok(())
        }

        /// Copy one direction until the connection ends.
        ///
        /// A read or write error here IS the severed link in the expected case, and
        /// in every case it means the peer this pump exists to serve is gone: there
        /// is no party left to propagate to, so ending the pump is the handling,
        /// not an omission of it.
        fn pump(mut from: std::net::TcpStream, mut to: std::net::TcpStream) {
            use std::io::{Read, Write};
            let mut buffer = [0_u8; 8192];
            loop {
                match from.read(&mut buffer) {
                    Ok(0) | Err(_) => return,
                    Ok(read) => {
                        if to.write_all(&buffer[..read]).is_err() {
                            return;
                        }
                    }
                }
            }
        }

        const fn address(&self) -> SocketAddr {
            self.address
        }

        /// BREAK every relayed socket, and report how many were broken.
        ///
        /// The count is returned, and asserted non-zero by the caller, so that a
        /// sever which severed nothing can never masquerade as a measurement — the
        /// pin would otherwise pass by never having run its own experiment.
        fn sever(&self) -> Result<usize, TestError> {
            let mut parked = self
                .sockets
                .lock()
                .map_err(|_| test_error("relay socket register poisoned"))?;
            let mut severed = 0;
            for socket in parked.iter() {
                if socket.shutdown(std::net::Shutdown::Both).is_ok() {
                    severed += 1;
                }
            }
            parked.clear();
            Ok(severed)
        }

        fn shutdown(mut self) {
            self.stop.store(true, Ordering::SeqCst);
            if let Some(handle) = self.handle.take() {
                handle.join().ok();
            }
        }
    }

    /// Registry for the reconnect pin: every dispatch is RECORDED with the identity
    /// the server sent it under, and [`HELD_ACTIVITY_TYPE`]'s FIRST dispatch holds
    /// — the work is finished, its reply is not yet on the wire — until released.
    ///
    /// Only the first is held. A blanket hold would stall the re-delivery this pin
    /// exists to observe, and the pin would then measure its own instrument.
    fn recording_registry(
        seen: &Arc<std::sync::Mutex<Vec<SeenDispatch>>>,
        release: &Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Arc<ActivityRegistry>, TestError> {
        let mut registry = ActivityRegistry::new();
        for activity_type in FAN_ACTIVITY_TYPES {
            let seen = Arc::clone(seen);
            let release = Arc::clone(release);
            let arrivals = Arc::new(AtomicUsize::new(0));
            registry = registry
                .register_activity_with_contract(
                    activity_type,
                    move |_input: FanInput, context: &aion_worker::ActivityContext| {
                        let seen = Arc::clone(&seen);
                        let release = Arc::clone(&release);
                        let arrivals = Arc::clone(&arrivals);
                        let record = SeenDispatch {
                            activity_type: activity_type.to_owned(),
                            activity_id: context.activity_id().to_string(),
                            attempt: context.attempt(),
                            at: Instant::now(),
                        };
                        Box::pin(async move {
                            // Recorded BEFORE the hold: a dispatch that arrives and
                            // is never answered must still be visible, or the pin
                            // cannot tell "never re-delivered" from "re-delivered
                            // and lost again".
                            match seen.lock() {
                                Ok(mut log) => log.push(record),
                                Err(_) => {
                                    return Err(aion_worker::ActivityFailure::terminal(
                                        "the pin's dispatch log is poisoned, so this run can \
                                         observe nothing — failing loudly rather than \
                                         returning a result no assertion could trust",
                                    ));
                                }
                            }
                            let first = arrivals.fetch_add(1, Ordering::SeqCst) == 0;
                            if activity_type == HELD_ACTIVITY_TYPE && first {
                                while !release.load(Ordering::SeqCst) {
                                    tokio::time::sleep(Duration::from_millis(5)).await;
                                }
                            }
                            Ok(activity_type.to_owned())
                        })
                    },
                )
                .map_err(test_error)?;
        }
        Ok(Arc::new(registry))
    }

    fn dispatches_of(
        seen: &Arc<std::sync::Mutex<Vec<SeenDispatch>>>,
        activity_type: &str,
    ) -> Result<Vec<SeenDispatch>, TestError> {
        let log = seen
            .lock()
            .map_err(|_| test_error("the pin's dispatch log is poisoned"))?;
        Ok(log
            .iter()
            .filter(|record| record.activity_type == activity_type)
            .cloned()
            .collect())
    }

    fn dispatch_log(seen: &Arc<std::sync::Mutex<Vec<SeenDispatch>>>) -> String {
        match seen.lock() {
            Ok(log) => format!("{:#?}", *log),
            Err(_) => String::from("<poisoned>"),
        }
    }

    /// aion #69 at the ENGINE level: what the system DOES after an activity's
    /// completion is lost to a broken link.
    ///
    /// # What this measures, and why the transport-level pin cannot
    ///
    /// #69's existing red-first pin lives on its fix branch rather than here (it
    /// is red on purpose and lands with the fix), and it establishes that the
    /// completion is DISCARDED: the server abandons the correlated reply-wait the
    /// moment the delivering connection closes. It drives `WorkerDelivery`
    /// directly, with no engine, no store and no workflow behind it, so it can say
    /// nothing at all about what happens NEXT. That gap is the whole severity of
    /// #69: "the work is repeated once" and "the work is lost" are priced very
    /// differently, and nothing in-tree could tell them apart.
    ///
    /// So this pin observes four things, and asserts only what must hold in EVERY
    /// world — including the one a #69 fix creates:
    ///
    /// - **O4, ASSERTED** — the workflow still reaches a recorded terminal. This is
    ///   the invariant: a broken link must not cost the workflow. It is not a weak
    ///   assertion, because `collect_four` consumes all four members, so the
    ///   workflow cannot complete while any member's work is missing;
    /// - **O1, REPORTED** — whether the held activity is dispatched a SECOND time.
    ///   This is the MECHANISM, and the mechanism is what a fix changes: a fix that
    ///   carries the completion across the reconnect would produce NO re-delivery,
    ///   and a pin asserting one would read that fix as a regression.
    ///   regression;
    /// - **O2, asserted CONDITIONALLY** — if a re-delivery happened it must carry
    ///   the activity's OWN identity. That is what makes the finished work
    ///   discarded rather than recovered; a re-delivery under a different identity
    ///   is a different defect and must not pass quietly;
    /// - **O3, REPORTED** — the elapsed time from the break to the re-delivery, as
    ///   a NUMBER asserted against nothing. No threshold is invented here: the
    ///   right bound is a conversation to have with the measurement in hand.
    ///
    /// ⚠️ **O3 is recovery LATENCY, and latency is not COST.** The number is
    /// measured on a fixture activity that is a pure `String -> String`, so its
    /// repeat costs microseconds. The real cost of a repeat is the repeated
    /// activity's own runtime plus its repeated SIDE EFFECTS, which this pin does
    /// not measure and structurally cannot: #69's own exhibit was an *agent*
    /// activity, whose repeat is minutes of compute and files written twice.
    /// Quote the finding — *repeated work, not lost work, one repeat per in-flight
    /// activity* — rather than the milliseconds, which carry their premise (a
    /// trivial activity) only for as long as someone remembers to attach it.
    ///
    /// The settle-wait below is bounded by [`POLL_DEADLINE`], so this pin cannot
    /// hang; but that bound is ~100x the observed recovery, so it is a liveness
    /// guard and NOT a latency guard. A large latency regression would still pass
    /// here, reported in O3 and asserted by nothing — deliberately, because the
    /// correct bound is not derivable from the samples taken so far.
    ///
    /// Executions are REPORTED, never asserted equal to the fan-out. A transport
    /// that can lose a reply gives at-least-once delivery, so the sibling test's
    /// `executions == FAN_OUT` is the wrong shape here and must not be copied
    /// across.
    ///
    /// # The world this models
    ///
    /// One server process with its transport-loss ledger live in memory, a worker
    /// that redials the SAME address, and a SINGLE loss — well inside
    /// `TRANSPORT_LOSS_BUDGET_WINDOWS`. It is NOT a server restart and NOT budget
    /// exhaustion, both of which are different worlds with different recoveries.
    /// The re-delivery this venue can produce is the outbox dispatcher's re-claim
    /// under the `max_attempts`/backoff this test's config sets, not the #266
    /// recovery replay — which is what gives O3's number a slot to mean anything in.
    ///
    /// The relay's own accept poll (2ms) sits inside the measured elapsed.
    ///
    /// ⚠️ This pin shares a venue with
    /// `production_boot_dispatches_executes_and_records_over_liminal`, one of four
    /// documented carriers of a load-sensitive flake — 2/24 on a base that
    /// predates it (`gate-logs/lock-race-attribution/VERDICT.md`). It inherits that
    /// sensitivity, and a red here should be read against that register first.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_completion_lost_to_a_severed_link_is_re_dispatched_and_the_workflow_settles()
    -> Result<(), TestError> {
        let dir = crate::test_support::private_tempdir().map_err(test_error)?;
        let db_path = dir.path().join("aion.db");
        let package_path = write_package_archive(dir.path())?;
        let listen_address = reserve_loopback_port()?;

        let config = server_config(&db_path, package_path, listen_address);
        let outbox_config = config.outbox.clone();
        // Captured before `build` consumes the config: the wait below is derived
        // from the very window this server is about to run its liveness probe on.
        let patience = eligibility_patience(&config);
        let state = ServerState::build(config).await.map_err(test_error)?;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let backpressure_settings = BackpressureSettings {
            platform_default: crate::config::DEFAULT_MAX_IN_FLIGHT_ACTIVITIES,
            fraction: crate::worker::OwnedShardFraction::own_all(),
        };
        let listener_guard = maybe_spawn_outbox_dispatcher(
            &state,
            &outbox_config,
            false,
            backpressure_settings,
            &shutdown_rx,
            "set outbox.liminal_listen_address in the test config",
        )
        .map_err(test_error)?;

        // The worker dials the RELAY, which carries it to the production listener.
        let relay = SeverableRelay::spawn(listen_address)?;
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let release = Arc::new(std::sync::atomic::AtomicBool::new(false));
        // The redial timings are the ones this module's `worker_config` already
        // declares, read off it rather than re-chosen here: a reconnect pin that
        // picked its own recovery timings would be measuring a world of its own.
        let config = worker_config()?;
        let timing = aion_worker::RedialTiming::new(
            config.reconnect.initial_backoff,
            config.reconnect.max_backoff,
        );
        let worker = WorkerThread::spawn_redialing(
            relay.address().to_string(),
            config,
            recording_registry(&seen, &release)?,
            timing,
        );

        let outcome =
            observe_reconnect(&state, &relay, &seen, &release, listen_address, patience).await;

        // Teardown runs on EVERY path, including a failing one: a leaked worker
        // thread or listener poisons whatever runs next, and this venue is already
        // load-sensitive enough without the pin adding to it.
        shutdown_tx.send(true).ok();
        release.store(true, Ordering::SeqCst);
        worker.stop();
        relay.shutdown();
        drop(listener_guard);
        state.shutdown().map_err(test_error)?;
        outcome
    }

    /// The measurement behind
    /// [`a_completion_lost_to_a_severed_link_is_re_dispatched_and_the_workflow_settles`],
    /// split out so its many early returns cannot skip the harness teardown.
    /// Wait until the held member is dispatched and holding — the moment the link
    /// can be broken — and report how many of its siblings had already settled.
    ///
    /// The split at the break is REPORTED, never required. An earlier draft
    /// demanded that the other three settle first, for a single-variable
    /// experiment. Measured across runs it simply varies: the four pushes land
    /// within microseconds of each other and which records a terminal first is a
    /// race, so requiring a particular split would fail the pin for a reason that
    /// has nothing to do with what it measures.
    async fn await_held_dispatch(
        reader: &dyn aion_store::ReadableEventStore,
        workflow_id: &aion_core::WorkflowId,
        seen: &Arc<std::sync::Mutex<Vec<SeenDispatch>>>,
    ) -> Result<(SeenDispatch, usize), TestError> {
        let deadline = Instant::now() + POLL_DEADLINE;
        loop {
            if let Some(first) = dispatches_of(seen, HELD_ACTIVITY_TYPE)?.first() {
                let at_the_break = reader.read_history(workflow_id).await.map_err(test_error)?;
                return Ok((first.clone(), count_completed(&at_the_break)));
            }
            if Instant::now() > deadline {
                let history = reader.read_history(workflow_id).await.map_err(test_error)?;
                return Err(test_error(format!(
                    "{HELD_ACTIVITY_TYPE} was never dispatched at all within {POLL_DEADLINE:?}, \
                     so there was no held completion to lose and this run measured nothing.\n\
                     dispatch log: {}\nhistory: {history:#?}",
                    dispatch_log(seen),
                )));
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    async fn observe_reconnect(
        state: &ServerState,
        relay: &SeverableRelay,
        seen: &Arc<std::sync::Mutex<Vec<SeenDispatch>>>,
        release: &Arc<std::sync::atomic::AtomicBool>,
        listen_address: SocketAddr,
        patience: Duration,
    ) -> Result<(), TestError> {
        wait_for_registration(
            state.worker_registry(),
            state.heartbeat_tracker(),
            listen_address,
            patience,
        )
        .await?;

        let router = http_router(state.clone()).map_err(test_error)?;
        let workflow_id = start_over_http(&router).await?;
        let reader = state.engine().map_err(test_error)?.store();

        let (first, settled_before) =
            await_held_dispatch(reader.as_ref(), &workflow_id, seen).await?;

        // BREAK the link while the finished work is still holding its reply.
        let severed = relay.sever()?;
        let severed_at = Instant::now();
        if severed == 0 {
            return Err(test_error(
                "the relay severed NOTHING, so no link was ever broken and this run measured \
                 nothing — a pass here would have been an artefact of the instrument",
            ));
        }
        // Release the hold: the worker now writes its reply into a dead socket.
        release.store(true, Ordering::SeqCst);

        // O4 FIRST, because it is the INVARIANT: a broken link must not cost the
        // workflow. Every other observable here describes the MECHANISM by which
        // that holds, and the mechanism is exactly what a #69 fix is expected to
        // change — so asserting today's mechanism would make the fix read as a
        // regression, and would be asserting the enumeration rather than the
        // invariant.
        //
        // O4 is load-bearing rather than weak because `collect_four` CONSUMES all
        // four members: the workflow cannot reach a completed terminal while any
        // member's work is missing, so "the workflow settled" is not a state that
        // silently lost work can also produce.
        let settled = wait_for_history(
            reader.as_ref(),
            &workflow_id,
            "the workflow to settle after the severed link",
            |events| count_completed(events) == FAN_OUT && count_workflow_completed(events) == 1,
        )
        .await
        .map_err(|error| {
            test_error(format!(
                "O4 FAILED — the workflow did not settle after the link broke ({severed} \
                 socket(s) severed), so the lost completion cost the workflow rather than \
                 costing a repeat of the work.\n{error}\ndispatch log: {}",
                dispatch_log(seen),
            ))
        })?;
        assert_eq!(
            count_completed(&settled),
            FAN_OUT,
            "every fan-out member must still record a terminal after the link broke"
        );
        assert_eq!(
            count_workflow_completed(&settled),
            1,
            "the workflow must complete exactly once even though a completion was lost"
        );

        // O1/O2/O3 — the MECHANISM, reported. O2 is asserted only CONDITIONALLY:
        // if a re-delivery happened it must have carried the activity's own
        // identity, because a re-delivery under a different identity would be a
        // different defect entirely and must not pass quietly. If no re-delivery
        // happened, the completion survived the reconnect — which is what a fixed
        // #69 looks like, and this pin should report it, not fail on it.
        let held = dispatches_of(seen, HELD_ACTIVITY_TYPE)?;
        match held.get(1) {
            None => println!(
                "aion#69 — {HELD_ACTIVITY_TYPE} ({}) was NOT re-dispatched and the workflow \
                 still settled, so the held completion survived the break; {settled_before} of \
                 {FAN_OUT} members had settled when it broke, {severed} socket(s) severed",
                first.activity_id,
            ),
            Some(second) => {
                if second.activity_id != first.activity_id {
                    return Err(test_error(format!(
                        "O2 FAILED — the re-delivery carried a DIFFERENT activity identity. The \
                         first dispatch was {} (attempt {}) and the second was {} (attempt {}), \
                         so the work was not re-run under its own identity and #69's framing \
                         does not describe what happened here.",
                        first.activity_id, first.attempt, second.activity_id, second.attempt,
                    )));
                }
                let recovery = second.at.saturating_duration_since(severed_at);
                println!(
                    "aion#69 O3 — re-delivery of {} ({}) took {}ms from the link breaking; \
                     first attempt {}, second attempt {}; {settled_before} of {FAN_OUT} members \
                     had already recorded a terminal when the link broke; {severed} socket(s) \
                     severed",
                    HELD_ACTIVITY_TYPE,
                    first.activity_id,
                    recovery.as_millis(),
                    first.attempt,
                    second.attempt,
                );
            }
        }

        let all = seen
            .lock()
            .map_err(|_| test_error("the pin's dispatch log is poisoned"))?
            .len();
        println!(
            "aion#69 — {all} dispatch(es) served for {FAN_OUT} activities; the transport is \
             at-least-once, so the excess is the repeated work a broken link costs"
        );
        Ok(())
    }
}
