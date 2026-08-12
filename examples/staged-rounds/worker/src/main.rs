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
//!
//! THE ADVERTISEMENT. Every one of the eleven activities is registered with the
//! wire DESCRIPTOR the document declares for it — the shell handlers through
//! `register_activity_with_descriptor`, the agent activities through
//! `AgentHarnessConfig::with_activity_descriptors`. Both sets come from
//! [`Declaration`], which compiles the embedded document once at startup, so the
//! schemas this worker advertises are the very bytes the server deployed and
//! admits against. A handler committed with no descriptor advertises nothing and
//! the server refuses the connection outright (`WORKER_CONTRACT_MISMATCH ... but
//! worker advertised <missing>`), which is exactly how all five connections were
//! refused on every dial on 2026-07-30.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::Duration;

use aion_integration_norn::NornHarness;
use aion_integrations::{DynAgentHarness, InterventionCapabilities, InterventionPrimitive};
use aion_package::ActivityDescriptor;
use aion_worker::{
    ActivityRegistry, AgentHarnessConfig, RedialTiming, WorkerConfig, WorkerConfigBuildError,
};

use staged_rounds_worker::declaration::{Declaration, TASK_QUEUE};
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
    // The declaration is compiled BEFORE anything is served: it is the source of
    // every advertised schema, and both startup guards read it.
    let declaration = Declaration::compile().map_err(|error| anyhow::anyhow!(error))?;
    let connections = agent_connections(profiles, args.max_parallel)?;
    let advertised = agent_advertisements(&declaration, &connections)?;
    let shell = Arc::new(shell_registry(Shell::inherited(), &declaration)?);
    require_every_declared_action_served(&declaration, &connections, &shell)?;
    tracing::info!(
        candidates = ?args.candidates,
        norn_bin = %args.norn_bin,
        profiles_dir = %args.profiles_dir,
        max_parallel = args.max_parallel,
        task_queue = TASK_QUEUE,
        agent_connections = connections.len(),
        declared_actions = declaration.contract().actions.len(),
        "staged-rounds-worker starting: 5 driven agent activities + 6 shell activities"
    );

    // Spawn one connection thread per agent node. Each owns its config, an
    // empty registry, its stop flag, its composed harness, and the declared
    // descriptors it advertises, and blocks in serve_with_redial (redialling
    // internally) until the process is killed.
    // The directory every agent process is started in, resolved HERE once and
    // handed to each connection, so no spawned child is left to inherit
    // whichever directory this worker happened to be launched from.
    let agent_cwd = std::env::current_dir()
        .map_err(|error| anyhow::anyhow!("cannot read this worker's working directory: {error}"))?;

    let mut threads = Vec::new();
    for (connection, descriptors) in connections.into_iter().zip(advertised) {
        let candidates = args.candidates.clone();
        let norn_bin = args.norn_bin.clone();
        let norn_account = args.norn_account.clone();
        let cwd = agent_cwd.clone();
        let identity = format!("{}-{}", args.identity_prefix, connection.node);
        threads.push(thread::spawn(move || {
            serve_agent_connection(
                &candidates,
                &identity,
                &norn_bin,
                &norn_account,
                &cwd,
                &connection,
                descriptors,
            );
        }));
    }

    // Serve the shell activities on the main thread; it writes the readiness
    // file once connected (a signal the whole worker is up enough to accept
    // work — the agent connections come up alongside it).
    serve_shell(&args, &shell)?;

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

/// The DECLARED descriptors each agent connection advertises, one list per
/// connection in connection order.
///
/// GUARD ONE lives here: an activity this binary serves that the document
/// declares no matching action for — on that connection's node — is a startup
/// failure naming the activity, never a handler registered with no
/// advertisement.
fn agent_advertisements(
    declaration: &Declaration,
    connections: &[Connection],
) -> anyhow::Result<Vec<Vec<ActivityDescriptor>>> {
    connections
        .iter()
        .map(|connection| connection_advertisement(declaration, connection))
        .collect()
}

/// The declared descriptors for ONE agent connection's activity table.
fn connection_advertisement(
    declaration: &Declaration,
    connection: &Connection,
) -> anyhow::Result<Vec<ActivityDescriptor>> {
    connection
        .activities
        .iter()
        .map(|activity| {
            declaration
                .descriptor(activity.activity_type, connection.node)
                .map_err(|error| anyhow::anyhow!(error))
        })
        .collect()
}

/// GUARD TWO: every action the document declares WITHOUT a body requires an
/// out-of-band worker, and this binary is it. One that no connection and no
/// shell handler serves is a startup failure naming the action and its declared
/// node — never a queue with a permanently parked dispatch.
///
/// Actions that DO carry a declarative body are executed by the server itself,
/// so no worker serves them; the declaration excludes them, mirroring the
/// `body.is_none()` filter `aion_package::contract_diffs` admits with.
fn require_every_declared_action_served(
    declaration: &Declaration,
    connections: &[Connection],
    shell: &ActivityRegistry,
) -> anyhow::Result<()> {
    let mut served: BTreeSet<String> = shell.activity_types();
    for connection in connections {
        for activity in &connection.activities {
            served.insert(activity.activity_type.to_owned());
        }
    }
    declaration
        .require_every_bodyless_action_served(&served)
        .map_err(|error| anyhow::anyhow!(error))
}

/// Compose one connection's harness and serve it on its own liminal
/// connection, registered on the connection's node so only the activities
/// pinned to that node are routed here.
fn serve_agent_connection(
    candidates: &[String],
    identity: &str,
    norn_bin: &str,
    norn_account: &str,
    cwd: &Path,
    connection: &Connection,
    descriptors: Vec<ActivityDescriptor>,
) {
    let config = match worker_config(identity, connection.node, connection.max_concurrency) {
        Ok(config) => config,
        Err(error) => {
            tracing::error!(%identity, %error, "could not build agent worker config");
            return;
        }
    };
    let registry = Arc::new(ActivityRegistry::new());
    let agent = composed_agent_harness(norn_bin, norn_account, cwd, connection, descriptors);
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
///
/// `descriptors` are the DECLARED wire schemas for this connection's activities
/// (see [`connection_advertisement`]). An agent activity's concrete output is
/// harness-owned, so this is the only path by which its advertisement can be
/// exact — without it the connection registers a set of activity NAMES and no
/// schemas, and the server's contract gate refuses it.
fn composed_agent_harness(
    norn_bin: &str,
    norn_account: &str,
    cwd: &Path,
    connection: &Connection,
    descriptors: Vec<ActivityDescriptor>,
) -> AgentHarnessConfig {
    let harness = ProfiledNornHarness::new(
        inner_norn_harness(norn_bin, norn_account, cwd, connection),
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
    .with_activity_descriptors(descriptors)
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
fn inner_norn_harness(
    norn_bin: &str,
    norn_account: &str,
    cwd: &Path,
    connection: &Connection,
) -> NornHarness {
    let mut harness = NornHarness::with_binary(norn_bin, cwd);
    for arg in connection_norn_args(connection) {
        harness = harness.with_arg(arg);
    }
    // Every run this harness starts carries `--session-id` and
    // `--resume-if-exists` (appended per run in `ProfiledNornHarness::start`),
    // and Norn refuses an open-or-resume run with no `--account` so that a
    // long-lived session's credential affinity can never change silently. The
    // account is therefore not optional here: without it the connection
    // registers cleanly and then fails EVERY agent dispatch with an auth
    // error, which is exactly how the 2026-07-30 first flight died on the
    // planner. It is a LITERAL argument, never a template: an operator-supplied
    // alias must reach Norn as typed, not be expanded as a placeholder.
    harness
        .with_arg("--account")
        .with_literal_arg(norn_account)
        .without_env("OPENAI_API_KEY")
}

/// Serve the six shell-node activities from a typed registry on one
/// connection, registered on the `shell` node so only shell activities are
/// routed here. The registry is built (and its descriptors taken from the
/// declaration) before any connection is served, so a mis-declared handler is a
/// startup failure rather than a refused dial.
fn serve_shell(args: &Args, registry: &Arc<ActivityRegistry>) -> anyhow::Result<()> {
    let identity = format!("{}-shell", args.identity_prefix);
    let config = worker_config(&identity, SHELL_NODE, SHELL_CONCURRENCY)?;
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
        registry,
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

#[cfg(test)]
#[path = "main_advertisement_tests.rs"]
mod advertisement_tests;
