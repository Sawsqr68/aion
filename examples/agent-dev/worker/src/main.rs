//! Generic mixed worker for the `agent_dev` proof.
//!
//! Serves two kinds of activity over the liminal server-push transport
//! ([`aion_worker::serve_with_redial`], same wiring as `norn-fan-worker`):
//!
//! - AGENT activities `scout`, `dev`, `review` (the `agent_dev` workflow)
//!   and `assistant` (the `assistant` workflow): routed through the composed
//!   [`NornHarness`] (observable + intervenable). Input is the prompt string,
//!   output the canonical agent-outcome record
//!   (`aion_core::AgentOutcome`: `text` + `final_message` + `stop_reason` +
//!   `session_id`). Each run's Norn session id is
//!   `{workflow_id}-{activity_type}` and its `--workspace-root` AND `-C`
//!   (tool-execution cwd) are the run's own clone
//!   `<workspace root>/{workflow_id}/repo` — the placeholders are expanded per
//!   run by the adapter, and the workspace root is the SAME resolved root the
//!   provision handlers clone under (resolved once here, threaded to both).
//!   The session id carries the activity TYPE, not the attempt: every
//!   dispatch of the same type in one run resumes ONE session, which is what
//!   makes the assistant's multi-round conversation continuous.
//! - PLAIN registry activities `provision`, `gate`, `land`: async handler
//!   bodies in [`agent_dev_worker::handlers`] whose command process groups are
//!   stopped and verified on activity cancellation. The `assistant` workflow's
//!   `assistant_provision` is NOT served here: it carries a declared body, so
//!   the server executes it from the deployed contract.
//!
//! Auth: Norn is invoked with `OPENAI_API_KEY` REMOVED from its child
//! environment (via the adapter's `without_env`) so it uses the operator's
//! `ChatGPT` OAuth login — a stray ambient key would take precedence and fail.
//!
//! Usage:
//!   agent-dev-worker --address 127.0.0.1:PORT [--address 127.0.0.1:PORT2 ...]
//!                    [--identity <id>] [--ready-file <path>] [--norn-bin <path>]
//!                    --norn-account <alias> [--norn-model <model>]
//!                    [--norn-allow-degraded-session]

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

mod args;

use agent_dev_worker::descriptors::advertised_surface;
use agent_dev_worker::handlers;
use agent_dev_worker::norn_invocation::{DEGRADED_SESSION_OPT_IN, norn_args};
use agent_dev_worker::registry::build_registry;
use agent_dev_worker::shell::Shell;
use aion_integration_norn::{AdvisingHarness, NornHarness};
use aion_integrations::{
    DynAgentHarness, HarnessWorkspace, InterventionCapabilities, InterventionPrimitive,
};
use aion_worker::{AgentHarnessConfig, RedialTiming, WorkerConfig};
use args::parse_args;

/// Agent rounds this binary serves for a package that has NO `.awl` document
/// to declare them — the three Norn rounds of the Gleam-authored `agent_dev`
/// workflow. ONE worker binary serves both packages.
///
/// Everything the `assistant` document DOES declare now arrives through the
/// document's own `agent` markers ([`advertised_surface`]), which is why this
/// list no longer contains `assistant`. What remains is the residue of a
/// package that cannot state the fact itself.
///
/// This list is deletable, and the condition is precise: when `agent_dev`
/// gains an `.awl` document, mark its three rounds `agent` there and delete
/// this constant. It is NOT kept because a Rust list is an acceptable place
/// for the fact — the whole point of the marker is that it is not. It is kept
/// because these three have no other home yet, and removing their routing here
/// would quietly settle an open question about the Gleam authoring path rather
/// than answer it.
const UNDECLARED_AGENT_ACTIVITY_TYPES: [&str; 3] = ["scout", "dev", "review"];

/// Lower bound on the reconnect backoff between candidate dials.
const REDIAL_INITIAL_BACKOFF: Duration = Duration::from_millis(50);
/// Upper bound on the reconnect backoff (a survivor may take a moment to
/// adopt the shard and bring its listener up).
const REDIAL_MAX_BACKOFF: Duration = Duration::from_millis(500);

/// Compose the agent harness at the binary root — the ONE place this worker
/// names a concrete [`AgentHarness`](aion_integrations::AgentHarness) adapter,
/// mirroring `norn-fan-worker` and the `aion` binary's composition root. The
/// serve path drives it only through the erased neutral trait
/// ([`DynAgentHarness`]), so swapping the adapter touches this function alone.
///
/// `workspace_root` is the SAME resolved root the `provision` handler clones
/// under: every agent round works inside the run's own clone
/// (`<root>/{workflow_id}/repo`). `--workspace-root` confines the file tools
/// to that clone, and `-C` sets the tool EXECUTION cwd to the same directory —
/// without it the agent's bash commands would run in the worker's own cwd,
/// outside the clone. Its session id (`{workflow_id}-{activity_type}` +
/// `--resume-if-exists`) is stable per `(run, round)` so a re-dispatch after a
/// failover RESUMES the session rather than starting over. `--fast` selects
/// Norn's fast model tier.
///
/// The advertised capabilities are exactly the neutral primitives the Norn
/// adapter's intervention translation supports today (`InjectMessage` +
/// `Cancel`); advertising more would promise interventions the harness
/// rejects.
///
/// The argv itself is built by
/// [`norn_args`](agent_dev_worker::norn_invocation::norn_args) so it can be
/// asserted directly, and the composed harness is wrapped in
/// [`AdvisingHarness`] so norn's degraded-session refusal reaches the operator
/// with the flag that gets past it.
fn composed_agent_harness(
    norn_bin: &str,
    norn_account: &str,
    norn_model: Option<&str>,
    workspace_root: &Path,
    allow_degraded_session: bool,
    agent_descriptors: &[aion_package::ActivityDescriptor],
    declared_agent_types: &[String],
) -> AgentHarnessConfig {
    // The run's own clone — the ONE template both the file-tool confinement
    // (--workspace-root) and the tool-execution cwd (-C) point at.
    let run_clone = format!("{}/{{workflow_id}}/repo", workspace_root.display());
    // The PROCESS working directory is the workspace root, not `run_clone`: the
    // clone is a per-run template that does not exist until `provision` makes
    // it, and a process cannot be started in a directory that is not there yet.
    // What the agent READS and EDITS is confined to the clone by
    // `--workspace-root`/`-C` above; this only decides where the child stands
    // when it is launched, and now it is decided here rather than inherited
    // from whichever shell started this worker.
    let mut norn = NornHarness::with_binary(
        norn_bin,
        HarnessWorkspace::Fixed(workspace_root.to_path_buf()),
    );
    for arg in norn_args(&run_clone, norn_account, norn_model, allow_degraded_session) {
        norn = norn.with_arg(arg);
    }
    let harness: Arc<dyn DynAgentHarness> = Arc::new(AdvisingHarness::new(
        // Force the `ChatGPT` OAuth login: the project does not use API keys,
        // and a stray ambient key would take precedence and fail.
        Arc::new(norn.without_env("OPENAI_API_KEY")),
        allow_degraded_session,
        // The advice names THIS worker's restart flag, spelled by the same
        // constant the argument parser accepts.
        DEGRADED_SESSION_OPT_IN,
    ));
    // The agent set is the union of what the document DECLARES and the
    // undeclared residue named above. Declared names come first because they
    // are the ones a reader can check against a source file.
    let agent_types: Vec<String> = declared_agent_types
        .iter()
        .cloned()
        .chain(
            UNDECLARED_AGENT_ACTIVITY_TYPES
                .iter()
                .map(|&name| name.to_owned()),
        )
        .collect();
    AgentHarnessConfig::new(
        harness,
        agent_types,
        InterventionCapabilities::from_primitives([
            InterventionPrimitive::InjectMessage,
            InterventionPrimitive::Cancel,
        ]),
    )
    // The whole admission decision. Without these the registration carries
    // names and no schemas, and every contract-bearing queue refuses the
    // connection while the refusal log shows the names it DID send.
    .with_activity_descriptors(agent_descriptors.to_vec())
}

fn main() -> anyhow::Result<()> {
    let env_filter = match tracing_subscriber::EnvFilter::try_from_default_env() {
        Ok(filter) => filter,
        Err(_) => tracing_subscriber::EnvFilter::new("info"),
    };
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let args = parse_args()?;

    // Resolve the stable workspace root ONCE and thread it to both consumers:
    // the provision handler (clones under it) and the agent harness (whose
    // --workspace-root template points into the same per-run clone).
    let workspace_root = handlers::resolve_workspace_root()
        .map_err(|failure| anyhow::anyhow!(failure.message().to_owned()))?;

    tracing::info!(
        candidates = ?args.candidates,
        identity = %args.identity,
        norn_bin = %args.norn_bin,
        norn_model = ?args.norn_model,
        workspace_root = %workspace_root.display(),
        allow_degraded_session = args.allow_degraded_session,
        "agent-dev-worker starting"
    );

    // Derive the advertised surface BEFORE anything dials, and before the queue
    // is chosen — the document decides both. A document that does not compile is
    // a startup failure carrying the compiler's own diagnosis, not a remote
    // WORKER_CONTRACT_MISMATCH an operator has to decode from a log.
    let surface = advertised_surface()?;
    let split = surface.split();
    tracing::info!(
        task_queue = %surface.task_queue,
        advertised = ?surface.names(),
        agent_served = ?split.agent.iter().map(|d| &d.name).collect::<Vec<_>>(),
        registry_served = ?split.plain.iter().map(|d| &d.name).collect::<Vec<_>>(),
        undeclared_agent_types = ?UNDECLARED_AGENT_ACTIVITY_TYPES,
        "advertising the typed action surface declared by the embedded assistant document"
    );

    let config = WorkerConfig::builder()
        .endpoint("unused-direct-address")
        .namespace("default")
        .task_queue(&surface.task_queue)
        .identity(&args.identity)
        .max_concurrency(4)
        .reconnect_initial_backoff(REDIAL_INITIAL_BACKOFF)
        .reconnect_max_backoff(REDIAL_MAX_BACKOFF)
        .reconnect_max_attempts(3)
        .build()?;

    let shell = Shell::inherited();
    let registry = build_registry(&shell, &workspace_root, &split.plain)?;
    let agent = composed_agent_harness(
        &args.norn_bin,
        &args.norn_account,
        args.norn_model.as_deref(),
        &workspace_root,
        args.allow_degraded_session,
        &split.agent,
        &surface.agent_names,
    );

    // Never stop on our own; the operator ends the worker with a signal.
    let stop = AtomicBool::new(false);
    let ready_file = args.ready_file.clone();
    aion_worker::serve_with_redial(
        args.candidates,
        &config,
        &registry,
        RedialTiming::new(REDIAL_INITIAL_BACKOFF, REDIAL_MAX_BACKOFF),
        &stop,
        Some(&agent),
        move || {
            if let Some(path) = ready_file.as_ref()
                && let Err(error) = std::fs::write(path, b"connected")
            {
                tracing::error!(%error, "failed to write worker readiness file");
            }
            tracing::info!("agent-dev-worker connected and registered; serving dispatches");
        },
    )?;

    Ok(())
}
