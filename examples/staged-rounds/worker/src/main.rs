//! Composition root for the staged-rounds worker.
//!
//! It serves ELEVEN activity types on ONE task queue (`staged_rounds`) across
//! FIVE liminal connections in this single process, each connection
//! registered on its OWN node (the third routing dimension: namespace ×
//! `task_queue` × node) so the server — which routes by those three
//! dimensions only, never by activity type — can land every dispatched
//! activity on a connection that holds its handler. The node strings here MUST
//! equal the `node` lines in `../awl/staged_rounds.awl` — the document itself
//! is the authoritative node table:
//!
//! - FOUR DRIVEN AGENT connections (`planner`, `developer`, `reviewer`,
//!   `remediation`) carrying FIVE activity types, each with its OWN composed
//!   [`ProfiledNornHarness`]: a distinct `--output-schema`, a per-run DERIVED
//!   `--session-id` (`<workflow_id>-<suffix>` — the arg template cannot
//!   express per-item ids, so the wrapper appends it at start), a per-run
//!   `--workspace-root` read from the activity input, and the role's profile
//!   markdown (loaded once at startup from `--profiles-dir`) assembled with
//!   the per-run context by that activity's ONE prompt function. The
//!   `remediation` connection serves TWO activity types (`remediate` and
//!   `fix_integration`) because the document pins both to that node — see
//!   [`connections`] for why one connection per NODE is the only correct
//!   grouping.
//! - one SHELL connection, registered on node `shell`, serving
//!   `provision_item`, `gate_item`, the pure `plan_reviews` and `fold_phase`,
//!   `merge_branches`, and `verify_integration` from a typed registry, with no
//!   harness.
//!
//! PARALLELISM: the engine fans items out via `workflow.map`; genuine
//! N-wide execution is this worker's `max_concurrency` on the developer and
//! reviewer connections (`--max-parallel`, default 4) — the SDK holds a
//! semaphore permit per dispatched activity. The reviewer connection now
//! carries the whole PANEL (item × lens), so its concurrency bounds
//! `items × lenses` concurrent reviewers, not `items`.
//!
//! SESSION TOPOLOGY: see `harness/context.rs`, which owns it in one place —
//! the planner runs once per workflow, each item's dev agent resumes across
//! that item's feedback rounds, each PANEL SEAT is its own session
//! (`review-<item>-<lens>`, so concurrent lenses over one item never collide),
//! and the remediator AND the integration fixer both key the planner's session.
//!
//! Norn runs with `OPENAI_API_KEY` REMOVED from its child environment so it
//! uses the operator's `ChatGPT` OAuth login, exactly like the dev-brief
//! worker. No secret is ever read, stored, or passed by this worker.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::Duration;

use aion_integration_norn::NornHarness;
use aion_integrations::{DynAgentHarness, InterventionCapabilities, InterventionPrimitive};
use aion_worker::{
    ActivityRegistry, AgentHarnessConfig, RedialTiming, WorkerConfig, WorkerConfigBuildError,
};

use staged_rounds_worker::harness::ProfiledNornHarness;
use staged_rounds_worker::profiles::{self, Profiles};
use staged_rounds_worker::shell::Shell;

#[path = "main_args.rs"]
mod args;
#[path = "main_connections.rs"]
mod connections_table;
#[path = "main_shell_node.rs"]
mod shell_node;

use args::{Args, parse_args};
#[cfg(test)]
use args::{DEFAULT_ADDRESS, parse_args_from};
#[cfg(test)]
use connections_table::Role;
use connections_table::{Connection, connections, roles};
use shell_node::shell_registry;

/// The one task queue every staged-rounds activity is dispatched on. MUST
/// equal the document's first `worker` name (`compile.rs` makes it the
/// deploy task queue).
const TASK_QUEUE: &str = "staged_rounds";
/// The node id the SHELL connection registers (see the module doc: the node
/// strings mirror the `node` lines in `../awl/staged_rounds.awl`).
const SHELL_NODE: &str = "shell";
/// The exact Norn file-mutating tool names denied to the read-only roles'
/// sessions (a comma-separated `--disallowed-tools` value). The planner and
/// the reviewer are READ-ONLY: rooted at real trees they must inspect but
/// never write. `read`, `search`, `bash`, and `lsp` stay available.
const READ_ONLY_DISALLOWED_TOOLS: &str = "write,edit,apply_patch";
/// Concurrency for the planner and remediation connections (single-flight
/// roles; 2 leaves headroom for a retry overlapping a redial).
const COORDINATOR_CONCURRENCY: usize = 2;
/// Concurrency for the shell connection. It serves the cheap git/pure handlers
/// AND the per-item gate battery, which fans out as wide as the dev fan-out
/// did, so it must not be narrower than the developer connection.
const SHELL_CONCURRENCY: usize = 8;
const REDIAL_INITIAL_BACKOFF: Duration = Duration::from_millis(100);
const REDIAL_MAX_BACKOFF: Duration = Duration::from_secs(5);

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = parse_args()?;
    let profiles =
        profiles::load(Path::new(&args.profiles_dir)).map_err(|error| anyhow::anyhow!(error))?;
    let connections = agent_connections(profiles, args.max_parallel)?;
    tracing::info!(
        candidates = ?args.candidates,
        norn_bin = %args.norn_bin,
        profiles_dir = %args.profiles_dir,
        max_parallel = args.max_parallel,
        task_queue = TASK_QUEUE,
        agent_connections = connections.len(),
        "staged-rounds-worker starting: 5 driven agent activities + 6 shell activities"
    );

    // Spawn one connection thread per agent node. Each owns its config, an
    // empty registry, its stop flag, and its composed harness, and blocks in
    // serve_with_redial (redialling internally) until the process is killed.
    let mut threads = Vec::new();
    for connection in connections {
        let candidates = args.candidates.clone();
        let norn_bin = args.norn_bin.clone();
        let identity = format!("{}-{}", args.identity_prefix, connection.node);
        threads.push(thread::spawn(move || {
            serve_agent_connection(&candidates, &identity, &norn_bin, &connection);
        }));
    }

    // Serve the shell activities on the main thread; it writes the readiness
    // file once connected (a signal the whole worker is up enough to accept
    // work — the agent connections come up alongside it).
    serve_shell(&args)?;

    for handle in threads {
        let _ = handle.join();
    }
    Ok(())
}

/// The agent connections: the role table folded to one connection per node.
/// A disagreement between two roles sharing a node is a STARTUP failure, not a
/// silent mis-wiring at run time.
fn agent_connections(profiles: Profiles, max_parallel: usize) -> anyhow::Result<Vec<Connection>> {
    connections(roles(profiles, max_parallel, COORDINATOR_CONCURRENCY))
        .map_err(|error| anyhow::anyhow!(error))
}

/// Compose one connection's harness and serve it on its own liminal
/// connection, registered on the connection's node so only the activities
/// pinned to that node are routed here.
fn serve_agent_connection(
    candidates: &[String],
    identity: &str,
    norn_bin: &str,
    connection: &Connection,
) {
    let config = match worker_config(identity, connection.node, connection.max_concurrency) {
        Ok(config) => config,
        Err(error) => {
            tracing::error!(%identity, %error, "could not build agent worker config");
            return;
        }
    };
    let registry = Arc::new(ActivityRegistry::new());
    let agent = composed_agent_harness(norn_bin, connection);
    let stop = AtomicBool::new(false);
    let node = connection.node;
    let served = connection
        .activities
        .iter()
        .map(|activity| activity.activity_type)
        .collect::<Vec<_>>()
        .join(", ");
    if let Err(error) = aion_worker::serve_with_redial(
        candidates.to_vec(),
        &config,
        &registry,
        RedialTiming::new(REDIAL_INITIAL_BACKOFF, REDIAL_MAX_BACKOFF),
        &stop,
        Some(&agent),
        move || tracing::info!(%node, %served, "agent connection registered"),
    ) {
        tracing::error!(%node, %error, "agent connection ended");
    }
}

/// Build the composed harness for one connection: the inner [`NornHarness`]
/// carries the driven-mode wiring (`--output-schema`, the connection's profile
/// as its `--append-system-prompt` doctrine, env hygiene); the
/// [`ProfiledNornHarness`] wrapper dispatches on the activity type, assembles
/// that activity's per-turn context into the prompt, and appends the per-run
/// `--workspace-root` and derived `--session-id` at start. This is the ONE
/// place a concrete adapter is named; the serve path drives it only through
/// the erased [`DynAgentHarness`] trait.
fn composed_agent_harness(norn_bin: &str, connection: &Connection) -> AgentHarnessConfig {
    let harness = ProfiledNornHarness::new(
        inner_norn_harness(norn_bin, connection),
        connection.activities.clone(),
    );
    let erased: Arc<dyn DynAgentHarness> = Arc::new(harness);
    AgentHarnessConfig::new(
        erased,
        connection
            .activities
            .iter()
            .map(|activity| activity.activity_type),
        InterventionCapabilities::from_primitives([
            InterventionPrimitive::InjectMessage,
            InterventionPrimitive::Cancel,
        ]),
    )
}

/// The exact norn argv one connection's inner harness is composed from, in
/// order.
///
/// Kept as a typed list rather than assembled inline so the composition-root
/// tests can assert on the REAL argument structure — which flags are set, and
/// which value follows which flag — instead of scraping a `Debug` rendering
/// that changes whenever the adapter's internals do.
///
/// `--fast` / `--reasoning-effort high` are the operator's speed/effort
/// settings. The profile rides as `--append-system-prompt`, which APPENDS the
/// doctrine to Norn's own system instructions (never `--system-prompt`, which
/// would OVERWRITE them).
///
/// NEITHER `--session-id` NOR `--workspace-root` appears here: both are
/// per-run data the [`ProfiledNornHarness`] derives from each activity input
/// and appends at start time (the arg template expands only `{workflow_id}`
/// and `{activity_type}`, which cannot express a per-item — let alone a
/// per-item-per-lens — session id).
fn connection_norn_args(connection: &Connection) -> Vec<String> {
    let mut args = vec![
        "--fast".to_owned(),
        "--reasoning-effort".to_owned(),
        "high".to_owned(),
        "--append-system-prompt".to_owned(),
        connection.profile.clone(),
        "--output-schema".to_owned(),
        connection.output_schema.trim_start().to_owned(),
    ];
    if let Some(disallowed) = connection.disallowed_tools {
        args.push("--disallowed-tools".to_owned());
        args.push(disallowed.to_owned());
    }
    args
}

/// Build the inner [`NornHarness`] for one connection from
/// [`connection_norn_args`], plus the env hygiene that forces the `ChatGPT`
/// OAuth login: a stray ambient API key would take precedence and fail. No
/// secret is ever set here.
fn inner_norn_harness(norn_bin: &str, connection: &Connection) -> NornHarness {
    let mut harness = NornHarness::with_binary(norn_bin);
    for arg in connection_norn_args(connection) {
        harness = harness.with_arg(arg);
    }
    harness.without_env("OPENAI_API_KEY")
}

/// Serve the six shell-node activities from a typed registry on one
/// connection, registered on the `shell` node so only shell activities are
/// routed here.
fn serve_shell(args: &Args) -> anyhow::Result<()> {
    let identity = format!("{}-shell", args.identity_prefix);
    let config = worker_config(&identity, SHELL_NODE, SHELL_CONCURRENCY)?;
    let registry = Arc::new(shell_registry(Shell::inherited())?);
    let served = registry
        .activity_types()
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let stop = AtomicBool::new(false);
    let ready_file = args.ready_file.clone();
    aion_worker::serve_with_redial(
        args.candidates.clone(),
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
            tracing::info!(served = %served.join(", "), "shell connection registered");
        },
    )?;
    Ok(())
}

/// The shared worker config for one connection: one identity, the
/// staged-rounds task queue, the connection's DISTINCT node (the routing key
/// that separates this process's five same-queue connections), the
/// connection's concurrency (the developer/reviewer connections take
/// `--max-parallel` — N permits on the SDK's dispatch semaphore = N item
/// agents genuinely in flight), and an effectively unbounded reconnect
/// budget (a long-lived worker must outwait server restarts).
fn worker_config(
    identity: &str,
    node: &str,
    max_concurrency: usize,
) -> Result<WorkerConfig, WorkerConfigBuildError> {
    WorkerConfig::builder()
        .endpoint("unused-direct-address")
        .namespace("default")
        .task_queue(TASK_QUEUE)
        .node(node)
        .identity(identity)
        .max_concurrency(max_concurrency)
        .reconnect_initial_backoff(REDIAL_INITIAL_BACKOFF)
        .reconnect_max_backoff(REDIAL_MAX_BACKOFF)
        .reconnect_max_attempts(usize::MAX)
        .build()
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
