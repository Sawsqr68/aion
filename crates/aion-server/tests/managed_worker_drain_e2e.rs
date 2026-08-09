//! The shutdown drain stops managed worker processes — proven end to end.
//!
//! The supervisor's own tests prove it CAN stop a fleet. This one proves the
//! server's shutdown path actually asks it to: a real `ServerState`, a real
//! supervised child process, and `drain_after_first_signal` driven exactly as
//! `aion server` drives it. Deleting the call from the drain path fails here
//! and nowhere else, which is the whole point of the file — a wiring claim
//! nothing executes is not a tested claim.
//!
//! The supervised executable is `/bin/sh`, installed through the commissioning
//! seam. Under `cargo test` the server's own executable is the test harness, so
//! a supervisor pointed at it would re-execute this suite inside itself.

use std::collections::BTreeSet;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use aion::{Engine, EngineBuilder};
use aion_server::config::{
    AuthConfig, AuthoringConfig, AutoCreate, DEFAULT_MAX_IN_FLIGHT_ACTIVITIES, DeployConfig,
    DevConfig, ListenConfig, MetricsConfig, NamespaceConfig, NamespaceMode, ObservabilityConfig,
    OpsConsoleAssetSource, OpsConsoleConfig, OutboxConfig, RuntimeConfig, WebSocketConfig,
    WorkerConfig,
};
use aion_server::shutdown::{ShutdownOutcome, drain_after_first_signal};
use aion_server::worker::{ManagedExecutable, SupervisionPolicy};
use aion_server::{
    NamespaceResolver, ServerState, StaticScheduleNamespaces, StaticWorkflowNamespaces,
};
use aion_store::{
    DeployedBinaryIdentity, DesiredState, EventStore, InMemoryStore, NewWorkerDeployment,
    WorkerArtifactRef, WorkerDeployment, visibility::VisibilityStore,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const NAMESPACE: &str = "default";
/// Generous by design: it bounds how long a BROKEN drain takes to report, not
/// whether a working one passes.
const BUDGET: Duration = Duration::from_secs(20);

#[tokio::test]
async fn the_shutdown_drain_stops_every_managed_worker() -> TestResult {
    let state = server_state().await?;
    let directory = tempfile::tempdir()?;
    let marker = directory.path().join("starts");

    state
        .worker_deployment_store()
        .put_worker_deployment(deployment(
            "drained",
            &format!("echo start >> {}; sleep 300", marker.display()),
        )?)
        .await
        .map(drop)?;

    let supervisor = state.worker_supervisor();
    assert!(supervisor.commission(policy()?, ManagedExecutable::Path(PathBuf::from("/bin/sh"))));
    drop(supervisor.start("drained").await?);

    // Wait for the child itself to say it started — the supervisor's own status
    // would be the supervisor grading its own homework.
    wait_until(|| marker.exists(), "the managed worker to start").await?;
    let running = supervisor
        .report()
        .await?
        .workers
        .into_iter()
        .find(|worker| worker.name == "drained")
        .ok_or("the deployment must be reported")?;
    let pid = running.pid.ok_or("a running worker has a pid")?;
    let group = running
        .process_group
        .ok_or("a running worker leads a process group")?;
    assert!(
        pid_alive(pid),
        "the worker should be running before the drain"
    );

    // Exactly how `aion server` drives it: one signal, no second one.
    let outcome = drain_after_first_signal(state.clone(), std::future::pending()).await?;
    assert_eq!(
        outcome,
        ShutdownOutcome::Clean,
        "no activity was in flight, so the drain must complete cleanly"
    );

    assert!(
        !group_alive(group),
        "process group {group} outlived the server drain"
    );
    assert!(!pid_alive(pid), "the managed worker outlived the drain");

    // Shutdown is not an operator stop: durable intent survives, so the next
    // boot reconciles the fleet back up rather than leaving it silently down.
    let record = state
        .worker_deployment_store()
        .get_worker_deployment("drained")
        .await?
        .ok_or("the record must survive shutdown")?;
    assert_eq!(record.desired, DesiredState::Running);
    Ok(())
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

fn deployment(name: &str, script: &str) -> Result<WorkerDeployment, Box<dyn std::error::Error>> {
    Ok(WorkerDeployment::new(
        NewWorkerDeployment {
            name: name.to_owned(),
            artifact: WorkerArtifactRef::Builtin {
                verb: vec!["-c".to_owned(), script.to_owned()],
            },
            binary: DeployedBinaryIdentity {
                version: "test".to_owned(),
                commit: "test".to_owned(),
                dirty: "false".to_owned(),
                content_hash: "deploy-time-hash".to_owned(),
            },
            namespaces: BTreeSet::from([NAMESPACE.to_owned()]),
            task_queue: "shell".to_owned(),
            node: None,
            desired: DesiredState::Running,
        },
        chrono::Utc::now(),
    )?)
}

async fn server_state() -> Result<ServerState, Box<dyn std::error::Error>> {
    let backing = Arc::new(InMemoryStore::default());
    let store: Arc<dyn EventStore> = backing.clone();
    let visibility: Arc<dyn VisibilityStore> = backing;
    let engine: Arc<Engine> = Arc::new(
        EngineBuilder::new()
            .store_arc(Arc::clone(&store))
            .visibility_store_arc(Arc::clone(&visibility))
            .scheduler_threads(1)
            .build()
            .await?,
    );
    let resolver = NamespaceResolver::from_parts(
        NamespaceMode::SharedEngine,
        Some(engine),
        Arc::new(StaticWorkflowNamespaces::default()),
        Arc::new(StaticScheduleNamespaces::default()),
    );
    Ok(ServerState::from_parts(resolver, runtime_config()))
}

async fn wait_until<F>(mut condition: F, what: &str) -> TestResult
where
    F: FnMut() -> bool,
{
    let deadline = Instant::now() + BUDGET;
    while Instant::now() < deadline {
        if condition() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Err(format!("timed out after {BUDGET:?} waiting for {what}").into())
}

/// Liveness comes from the OPERATING SYSTEM, never from the supervisor's cells.
fn pid_alive(pid: u32) -> bool {
    shell_test(&format!("kill -0 {pid} 2>/dev/null"))
}

fn group_alive(process_group: i32) -> bool {
    shell_test(&format!("kill -0 -{process_group} 2>/dev/null"))
}

fn shell_test(script: &str) -> bool {
    std::process::Command::new("/bin/sh")
        .args(["-c", script])
        .status()
        .is_ok_and(|status| status.success())
}

fn runtime_config() -> RuntimeConfig {
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
        observability: ObservabilityConfig::default(),
        mcp: aion_server::config::ResolvedMcpConfig::default(),
        scheduler_threads: 1,
        query_timeout: Some(Duration::from_secs(10)),
        default_namespace: NAMESPACE.to_owned(),
        auto_create: AutoCreate::Open,
        max_in_flight_activities: DEFAULT_MAX_IN_FLIGHT_ACTIVITIES,
        drain_timeout: Duration::from_secs(30),
        metrics: MetricsConfig { enabled: false },
        owned_shards: Vec::new(),
        cors_allowed_origins: Vec::new(),
    }
}
