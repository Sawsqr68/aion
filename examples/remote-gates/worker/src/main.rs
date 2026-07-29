//! Composition root for the remote-gates activity worker.
//!
//! Registration is CONTRACT-TRUE: every action is registered with
//! `register_activity_with_contract`, so the worker advertises the schemas the
//! deployed `.aion` package declares. A worker registered without contracts
//! advertises `activities: []` and the server refuses it with
//! `WORKER_CONTRACT_MISMATCH`.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use aion_worker::{
    ActivityContext, ActivityRegistry, HandlerFuture, RedialTiming, WorkerConfig,
    WorkerConfigBuildError, WorkerError,
};
use remote_gates_worker::legs::Leg;
use remote_gates_worker::settings::Settings;
use remote_gates_worker::types::{
    Checkout, CloneInput, LegInput, LegResult, NotifyInput, NotifyResult,
};
use remote_gates_worker::{checkout, legs, notify};

/// Node class the AWL document pins every action to.
const NODE: &str = "shell";
/// Actions this worker serves; also its concurrency, since the workflow can have
/// at most this many of them in flight at once.
const ACTIONS: usize = 6;
/// Reconnect bounds, mirroring the landed `cargo-gates` worker.
const REDIAL_INITIAL_BACKOFF: Duration = Duration::from_millis(100);
const REDIAL_MAX_BACKOFF: Duration = Duration::from_secs(5);

struct Args {
    candidates: Vec<String>,
    task_queue: String,
    identity: String,
    ready_file: Option<String>,
    settings: Settings,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = parse_args()?;
    let settings = Arc::new(args.settings);
    let config = worker_config(&args.task_queue, &args.identity)?;
    let registry = Arc::new(registry(&settings)?);
    let stop = AtomicBool::new(false);
    let ready_file = args.ready_file.clone();
    tracing::info!(
        task_queue = args.task_queue,
        node = NODE,
        workspace_dir = %settings.workspace_dir().display(),
        notify_channel = settings.notify_channel().unwrap_or("<unset: notify skips>"),
        progress_interval_seconds = settings.progress_interval().as_secs(),
        "remote-gates worker starting"
    );
    aion_worker::serve_with_redial(
        args.candidates,
        &config,
        &registry,
        RedialTiming::new(REDIAL_INITIAL_BACKOFF, REDIAL_MAX_BACKOFF),
        &stop,
        None,
        move || {
            if let Some(path) = ready_file.as_ref()
                && let Err(error) = std::fs::write(path, b"connected")
            {
                tracing::error!(%error, "failed to write worker readiness file");
            }
            tracing::info!("connection registered; serving the clone, four gate legs, and notify");
        },
    )?;
    Ok(())
}

fn registry(settings: &Arc<Settings>) -> Result<ActivityRegistry, WorkerError> {
    ActivityRegistry::new()
        .register_activity_with_contract("clone_repo", clone_handler(Arc::clone(settings)))?
        .register_activity_with_contract(
            "run_check",
            leg_handler(Leg::Check, Arc::clone(settings)),
        )?
        .register_activity_with_contract(
            "run_clippy",
            leg_handler(Leg::Clippy, Arc::clone(settings)),
        )?
        .register_activity_with_contract(
            "run_nextest",
            leg_handler(Leg::Nextest, Arc::clone(settings)),
        )?
        .register_activity_with_contract(
            "run_fmt_check",
            leg_handler(Leg::Fmt, Arc::clone(settings)),
        )?
        .register_activity_with_contract("notify_result", notify_handler(Arc::clone(settings)))
}

fn clone_handler(
    settings: Arc<Settings>,
) -> impl for<'context> Fn(CloneInput, &'context ActivityContext) -> HandlerFuture<'context, Checkout>
+ Send
+ Sync
+ 'static {
    move |input, context| {
        let settings = Arc::clone(&settings);
        Box::pin(async move { checkout::clone_repo(input, &settings, context).await })
    }
}

fn leg_handler(
    leg: Leg,
    settings: Arc<Settings>,
) -> impl for<'context> Fn(LegInput, &'context ActivityContext) -> HandlerFuture<'context, LegResult>
+ Send
+ Sync
+ 'static {
    move |input, context| {
        let settings = Arc::clone(&settings);
        Box::pin(async move { legs::run(leg, input, &settings, context).await })
    }
}

fn notify_handler(
    settings: Arc<Settings>,
) -> impl for<'context> Fn(
    NotifyInput,
    &'context ActivityContext,
) -> HandlerFuture<'context, NotifyResult>
+ Send
+ Sync
+ 'static {
    move |input, context| {
        let settings = Arc::clone(&settings);
        Box::pin(async move { notify::notify_result(input, &settings, context).await })
    }
}

fn worker_config(task_queue: &str, identity: &str) -> Result<WorkerConfig, WorkerConfigBuildError> {
    WorkerConfig::builder()
        .endpoint("unused-direct-address")
        .namespace("default")
        .task_queue(task_queue)
        .node(NODE)
        .identity(identity)
        .max_concurrency(ACTIONS)
        .reconnect_initial_backoff(REDIAL_INITIAL_BACKOFF)
        .reconnect_max_backoff(REDIAL_MAX_BACKOFF)
        .reconnect_max_attempts(usize::MAX)
        .build()
}

/// Parses the operator's argv. Nothing that describes the gate HOST has a
/// default: an unset `--workspace-dir` or `--progress-interval` is a refusal to
/// start, not a guess.
fn parse_args() -> anyhow::Result<Args> {
    let mut candidates = Vec::new();
    let mut task_queue = None;
    let mut workspace_dir = None;
    let mut progress_interval = None;
    let mut notify_channel = None;
    let mut identity = "remote-gates-worker".to_owned();
    let mut ready_file = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--address" => candidates.push(next_value(&mut args, "--address")?),
            "--task-queue" => task_queue = Some(next_value(&mut args, "--task-queue")?),
            "--workspace-dir" => {
                workspace_dir = Some(PathBuf::from(next_value(&mut args, "--workspace-dir")?));
            }
            "--progress-interval" => {
                progress_interval = Some(progress_interval_value(&next_value(
                    &mut args,
                    "--progress-interval",
                )?)?);
            }
            "--notify-channel" => {
                notify_channel = Some(next_value(&mut args, "--notify-channel")?);
            }
            "--identity" => identity = next_value(&mut args, "--identity")?,
            "--ready-file" => ready_file = Some(next_value(&mut args, "--ready-file")?),
            other => anyhow::bail!("unknown argument `{other}`"),
        }
    }
    anyhow::ensure!(
        !candidates.is_empty(),
        "--address is required (repeat it to give the worker redial candidates)"
    );
    let workspace_dir = workspace_dir
        .ok_or_else(|| anyhow::anyhow!("--workspace-dir is required and has no default"))?;
    let progress_interval = progress_interval
        .ok_or_else(|| anyhow::anyhow!("--progress-interval is required and has no default"))?;
    let task_queue =
        task_queue.ok_or_else(|| anyhow::anyhow!("--task-queue is required and has no default"))?;
    Ok(Args {
        candidates,
        task_queue,
        identity,
        ready_file,
        settings: Settings::new(workspace_dir, notify_channel, progress_interval),
    })
}

/// Reads the progress cadence in whole seconds. Zero is refused: a zero-interval
/// ticker is not a cadence.
fn progress_interval_value(raw: &str) -> anyhow::Result<Duration> {
    let seconds = raw
        .parse::<u64>()
        .map_err(|error| anyhow::anyhow!("--progress-interval must be whole seconds: {error}"))?;
    anyhow::ensure!(seconds > 0, "--progress-interval must be greater than zero");
    Ok(Duration::from_secs(seconds))
}

fn next_value(args: &mut impl Iterator<Item = String>, flag: &str) -> anyhow::Result<String> {
    args.next()
        .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))
}

#[cfg(test)]
mod contract_tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use remote_gates_worker::settings::Settings;

    use super::registry;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn settings() -> Arc<Settings> {
        Arc::new(Settings::new(
            PathBuf::from("/srv/gates"),
            None,
            Duration::from_secs(30),
        ))
    }

    /// The worker advertises exactly what `remote_gates.awl` declares — the
    /// lsub5b lesson: a drifting advertisement is a `WORKER_CONTRACT_MISMATCH`
    /// refusal at registration, discovered on the gate host instead of here.
    #[test]
    fn contract_true_registration_matches_the_awl_document() -> TestResult {
        let schema_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or("remote-gates worker manifest has no parent")?;
        let compiled = aion_awl::compile(include_str!("../../remote_gates.awl"), schema_root)?;
        let contract = compiled
            .contract
            .workers
            .first()
            .ok_or("compiled remote-gates contract declares no worker")?;
        let advertised = registry(&settings())?.activity_descriptors();

        let diffs = aion_package::contract_diffs("remote-gates-test", contract, &advertised);
        assert!(diffs.is_empty(), "descriptor drift: {diffs:#?}");
        Ok(())
    }

    /// The queue name the README and runbook tell operators to pass.
    #[test]
    fn the_document_declares_the_queue_the_runbook_names() -> TestResult {
        let schema_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or("remote-gates worker manifest has no parent")?;
        let compiled = aion_awl::compile(include_str!("../../remote_gates.awl"), schema_root)?;
        let queues = compiled
            .contract
            .workers
            .iter()
            .map(|worker| worker.task_queue.clone())
            .collect::<Vec<_>>();
        assert_eq!(queues, vec!["remote_gates".to_owned()]);
        Ok(())
    }
}
