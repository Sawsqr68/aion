//! Composition root for the `lane_gates` activity worker.
//!
//! Dials the server's LIMINAL worker address: the server dispatches a
//! deployed AWL workflow's activities through its liminal outbox, and
//! dispatch ELIGIBILITY is earned by answering the liminal liveness pings —
//! so a liminal connection is the plane a worker for this queue must register
//! on (the #197 re-diagnosis in this lane's report states the mechanism).
//! Slice two (Dean's box) is this same binary run on another host with
//! `--address` pointed at this server — the workflow document does not
//! change.
//!
//! NOTHING IS INVENTED: every connection and execution setting is named by
//! the operator — the liminal address(es), the PATH the gates resolve tools
//! from, the concurrency ceiling, and both redial backoffs — and every
//! missing one is reported at once, by flag name (the same contract the
//! `aion worker` verbs publish). The one derived value is the identity,
//! `lane_gates-worker`, a reading of the queue this binary serves; override
//! it with `--identity`.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use aion_worker::{
    ActivityContext, ActivityRegistry, HandlerFuture, RedialTiming, WorkerConfig,
    WorkerConfigBuildError,
};
use lane_gates_worker::activities::{
    self, GateInput, GateResult, ManifestInput, ManifestReceipt, RunnerEnvironment,
};

const TASK_QUEUE: &str = "lane_gates";
/// `WorkerConfigBuilder` requires a gRPC endpoint, and the liminal transport
/// never reads it: the address a connection dials is the liminal candidate.
/// Named the same way `aion worker agent` names its own unused endpoint
/// (`crates/aion-cli/src/worker_agent/serve.rs`), so the placeholder cannot
/// be mistaken for a real address.
const UNUSED_GRPC_ENDPOINT: &str =
    "unused: a liminal worker dials liminal candidates, not a gRPC endpoint";

struct Args {
    candidates: Vec<String>,
    identity: String,
    tool_path: String,
    concurrency: usize,
    redial_initial: Duration,
    redial_max: Duration,
    ready_file: Option<String>,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = parse_args()?;
    let config = worker_config(&args)?;
    let environment = Arc::new(RunnerEnvironment {
        tool_path: args.tool_path.clone(),
    });
    let registry = Arc::new(registry(&environment)?);
    let stop = AtomicBool::new(false);
    let ready_file = args.ready_file.clone();
    tracing::info!(
        task_queue = TASK_QUEUE,
        identity = %args.identity,
        tool_path = %args.tool_path,
        concurrency = args.concurrency,
        "lane-gates worker starting"
    );
    aion_worker::serve_with_redial(
        args.candidates,
        &config,
        &registry,
        RedialTiming::new(args.redial_initial, args.redial_max),
        &stop,
        None,
        move || {
            if let Some(path) = ready_file.as_ref()
                && let Err(error) = std::fs::write(path, b"connected")
            {
                tracing::error!(%error, "failed to write worker readiness file");
            }
            tracing::info!("connection registered; serving all eight lane_gates actions");
        },
    )?;
    Ok(())
}

/// One gate action's handler: the shared runner invocation bound to its
/// subcommand and the operator-named environment.
fn gate_handler(
    environment: &Arc<RunnerEnvironment>,
    subcommand: &'static str,
) -> impl for<'context> Fn(GateInput, &'context ActivityContext) -> HandlerFuture<'context, GateResult>
+ Send
+ Sync
+ 'static {
    let environment = Arc::clone(environment);
    move |input, context| {
        let environment = Arc::clone(&environment);
        Box::pin(
            async move { activities::run_gate(&environment, subcommand, input, context).await },
        )
    }
}

/// The manifest action's handler.
fn manifest_handler(
    environment: &Arc<RunnerEnvironment>,
) -> impl for<'context> Fn(
    ManifestInput,
    &'context ActivityContext,
) -> HandlerFuture<'context, ManifestReceipt>
+ Send
+ Sync
+ 'static {
    let environment = Arc::clone(environment);
    move |input, context| {
        let environment = Arc::clone(&environment);
        Box::pin(async move { activities::write_manifest(&environment, input, context).await })
    }
}

fn registry(
    environment: &Arc<RunnerEnvironment>,
) -> Result<ActivityRegistry, aion_worker::WorkerError> {
    ActivityRegistry::new()
        .register_activity_with_contract("fmt_gate", gate_handler(environment, "fmt"))?
        .register_activity_with_contract(
            "clippy_workspace_gate",
            gate_handler(environment, "clippy_workspace"),
        )?
        .register_activity_with_contract(
            "clippy_server_gate",
            gate_handler(environment, "clippy_server"),
        )?
        .register_activity_with_contract("test_gate", gate_handler(environment, "test"))?
        .register_activity_with_contract("bun_gate", gate_handler(environment, "bun"))?
        .register_activity_with_contract("embed_gate", gate_handler(environment, "embed"))?
        .register_activity_with_contract(
            "gates_worker_gate",
            gate_handler(environment, "gates_worker"),
        )?
        .register_activity_with_contract("write_manifest", manifest_handler(environment))
}

fn worker_config(args: &Args) -> Result<WorkerConfig, WorkerConfigBuildError> {
    WorkerConfig::builder()
        .endpoint(UNUSED_GRPC_ENDPOINT)
        .namespace("default")
        .task_queue(TASK_QUEUE)
        .identity(&args.identity)
        .max_concurrency(args.concurrency)
        .reconnect_initial_backoff(args.redial_initial)
        .reconnect_max_backoff(args.redial_max)
        // Not a tuning default: `serve_with_redial` serves until stopped, and
        // an unbounded drop budget IS that semantic — bounding it would turn
        // a long outage into a silent worker exit.
        .reconnect_max_attempts(usize::MAX)
        .build()
}

fn parse_args() -> anyhow::Result<Args> {
    let mut candidates = Vec::new();
    let mut identity: Option<String> = None;
    let mut tool_path: Option<String> = None;
    let mut concurrency: Option<String> = None;
    let mut redial_initial: Option<String> = None;
    let mut redial_max: Option<String> = None;
    let mut ready_file = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--address" => candidates.push(next_value(&mut args, "--address")?),
            "--identity" => identity = Some(next_value(&mut args, "--identity")?),
            "--path" => tool_path = Some(next_value(&mut args, "--path")?),
            "--concurrency" => concurrency = Some(next_value(&mut args, "--concurrency")?),
            "--reconnect-initial-backoff" => {
                redial_initial = Some(next_value(&mut args, "--reconnect-initial-backoff")?);
            }
            "--reconnect-max-backoff" => {
                redial_max = Some(next_value(&mut args, "--reconnect-max-backoff")?);
            }
            "--ready-file" => ready_file = Some(next_value(&mut args, "--ready-file")?),
            other => anyhow::bail!("unknown argument `{other}`"),
        }
    }

    // Every missing setting is reported at once, by name — never guessed.
    let mut missing = Vec::new();
    if candidates.is_empty() {
        missing.push("--address (the server's liminal worker address, repeatable)");
    }
    if tool_path.is_none() {
        missing
            .push("--path (the PATH the gates resolve their tools from; recorded in the manifest)");
    }
    if concurrency.is_none() {
        missing.push("--concurrency (maximum activities served at once)");
    }
    if redial_initial.is_none() {
        missing.push("--reconnect-initial-backoff (seconds before the first redial)");
    }
    if redial_max.is_none() {
        missing.push("--reconnect-max-backoff (redial backoff cap, seconds)");
    }
    if !missing.is_empty() {
        anyhow::bail!(
            "every worker setting is named by the operator; missing:\n  {}",
            missing.join("\n  ")
        );
    }
    let (Some(tool_path), Some(concurrency), Some(redial_initial), Some(redial_max)) =
        (tool_path, concurrency, redial_initial, redial_max)
    else {
        anyhow::bail!("every worker setting must be resolved before use");
    };

    Ok(Args {
        candidates,
        // Derived from the one queue this binary serves — a reading of the
        // program, not an invented default.
        identity: identity.unwrap_or_else(|| format!("{TASK_QUEUE}-worker")),
        tool_path,
        concurrency: positive_usize(&concurrency, "--concurrency")?,
        redial_initial: positive_seconds(&redial_initial, "--reconnect-initial-backoff")?,
        redial_max: positive_seconds(&redial_max, "--reconnect-max-backoff")?,
        ready_file,
    })
}

fn positive_usize(value: &str, flag: &str) -> anyhow::Result<usize> {
    let parsed: usize = value
        .parse()
        .map_err(|error| anyhow::anyhow!("{flag} `{value}` is not a whole number: {error}"))?;
    anyhow::ensure!(parsed > 0, "{flag} must be greater than zero, got {value}");
    Ok(parsed)
}

fn positive_seconds(value: &str, flag: &str) -> anyhow::Result<Duration> {
    let parsed: u64 = value
        .parse()
        .map_err(|error| anyhow::anyhow!("{flag} `{value}` is not whole seconds: {error}"))?;
    anyhow::ensure!(parsed > 0, "{flag} must be greater than zero, got {value}");
    Ok(Duration::from_secs(parsed))
}

fn next_value(args: &mut impl Iterator<Item = String>, flag: &str) -> anyhow::Result<String> {
    args.next()
        .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))
}

#[cfg(test)]
mod contract_tests {
    use std::sync::Arc;

    use lane_gates_worker::activities::RunnerEnvironment;

    use super::{TASK_QUEUE, registry};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// The worker's advertised descriptors must match the CONTRACT the
    /// in-tree workflow document derives — the same field-by-field comparison
    /// the server's admission gate applies at registration, run at the desk.
    /// (Admission against the DEPLOYED contract happens at connect; this
    /// pins the advertisement to the document the deploy comes from.)
    #[test]
    fn mechanical_descriptors_match_the_real_awl_contract() -> TestResult {
        let schema_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or("lane-gates worker manifest has no parent")?;
        let compiled = aion_awl::compile(include_str!("../../gates.awl"), schema_root)?;
        let contract = compiled
            .contract
            .workers
            .iter()
            .find(|contract| contract.task_queue == TASK_QUEUE)
            .ok_or("compiled gates contract is missing the lane_gates task queue")?;
        let environment = Arc::new(RunnerEnvironment {
            tool_path: String::new(),
        });
        let advertised = registry(&environment)?.activity_descriptors();

        let diffs = aion_package::contract_diffs("lane-gates-test", contract, None, &advertised);
        assert!(diffs.is_empty(), "descriptor drift: {diffs:#?}");
        Ok(())
    }
}
