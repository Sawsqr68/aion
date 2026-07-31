//! Generic mixed worker for the `agent_dev` proof.
//!
//! Serves two kinds of activity over the liminal server-push transport
//! ([`aion_worker::serve_with_redial`], same wiring as `norn-fan-worker`):
//!
//! - AGENT activities `scout`, `dev`, `review` (the `agent_dev` workflow)
//!   and `assistant` (the `assistant` workflow): routed through the composed
//!   [`NornHarness`] (observable + intervenable). Input is the prompt string,
//!   output the agent's answer string. Each run's Norn session id is
//!   `{workflow_id}-{activity_type}` and its `--workspace-root` AND `-C`
//!   (tool-execution cwd) are the run's own clone
//!   `<workspace root>/{workflow_id}/repo` — the placeholders are expanded per
//!   run by the adapter, and the workspace root is the SAME resolved root the
//!   provision handlers clone under (resolved once here, threaded to both).
//!   The session id carries the activity TYPE, not the attempt: every
//!   dispatch of the same type in one run resumes ONE session, which is what
//!   makes the assistant's multi-round conversation continuous.
//! - PLAIN registry activities `provision`, `assistant_provision`, `gate`,
//!   `land`: async handler bodies in [`agent_dev_worker::handlers`] whose
//!   command process groups are stopped and verified on activity cancellation.
//!
//! Auth: Norn is invoked with `OPENAI_API_KEY` REMOVED from its child
//! environment (via the adapter's `without_env`) so it uses the operator's
//! `ChatGPT` OAuth login — a stray ambient key would take precedence and fail.
//!
//! Usage:
//!   agent-dev-worker --address 127.0.0.1:PORT [--address 127.0.0.1:PORT2 ...]
//!                    [--identity <id>] [--ready-file <path>] [--norn-bin <path>]
//!                    --norn-account <alias> [--norn-allow-degraded-session]

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use agent_dev_worker::advising_harness::AdvisingHarness;
use agent_dev_worker::descriptors::advertised_surface;
use agent_dev_worker::handlers;
use agent_dev_worker::norn_invocation::norn_args;
use agent_dev_worker::registry::build_registry;
use agent_dev_worker::shell::Shell;
use aion_integration_norn::NornHarness;
use aion_integrations::{DynAgentHarness, InterventionCapabilities, InterventionPrimitive};
use aion_worker::{AgentHarnessConfig, RedialTiming, WorkerConfig};

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
    workspace_root: &Path,
    allow_degraded_session: bool,
    agent_descriptors: &[aion_package::ActivityDescriptor],
    declared_agent_types: &[String],
) -> AgentHarnessConfig {
    // The run's own clone — the ONE template both the file-tool confinement
    // (--workspace-root) and the tool-execution cwd (-C) point at.
    let run_clone = format!("{}/{{workflow_id}}/repo", workspace_root.display());
    let mut norn = NornHarness::with_binary(norn_bin);
    for arg in norn_args(&run_clone, norn_account, allow_degraded_session) {
        norn = norn.with_arg(arg);
    }
    let harness: Arc<dyn DynAgentHarness> = Arc::new(AdvisingHarness::new(
        // Force the `ChatGPT` OAuth login: the project does not use API keys,
        // and a stray ambient key would take precedence and fail.
        Arc::new(norn.without_env("OPENAI_API_KEY")),
        allow_degraded_session,
    ));
    // The agent set is the union of what the document DECLARES and the
    // undeclared residue named above. Declared names come first because they
    // are the ones a reader can check against a source file.
    let agent_types: Vec<String> = declared_agent_types
        .iter()
        .cloned()
        .chain(UNDECLARED_AGENT_ACTIVITY_TYPES.iter().map(|&name| name.to_owned()))
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

/// Parsed command-line arguments.
#[derive(Debug)]
struct Args {
    /// One or more candidate liminal listen addresses, in dial-preference order.
    candidates: Vec<String>,
    /// The worker identity announced in-band.
    identity: String,
    /// Optional readiness file written once after the first registration.
    ready_file: Option<String>,
    /// Path to the `norn` binary (default: `NORN_BIN` env, else `norn` on PATH).
    norn_bin: String,
    /// The norn OAuth account alias passed as `--account`.
    ///
    /// Required, with no default. The harness runs norn with
    /// `--resume-if-exists`, and norn refuses to resume, fork, or open an
    /// existing OAuth session without an account — so this is an input the
    /// worker cannot supply for the operator. Guessing `default` here would
    /// turn a clear startup error into three retried activity failures and a
    /// failed workflow, which is exactly how this was found.
    norn_account: String,
    /// Whether to pass norn `--allow-degraded-session`.
    ///
    /// OPT-IN, with no default. A durable session can park far longer than the
    /// agent provider's epoch — one real run parked 28 days, and the engine
    /// resumed it perfectly — and when the epoch has moved on norn refuses to
    /// reopen the session without approval, so every round of that session
    /// fails. Allowing it resumes from a fresh provider epoch because the exact
    /// provider transcript cannot be replayed: a real fidelity tradeoff, and
    /// therefore the operator's call rather than this worker's.
    allow_degraded_session: bool,
}

/// Parse `--address` (repeatable), `--identity`, `--ready-file`, `--norn-bin` from the process
/// arguments, defaulting `--norn-bin` from the `NORN_BIN` environment variable.
fn parse_args() -> anyhow::Result<Args> {
    let default_norn_bin = match std::env::var("NORN_BIN") {
        Ok(value) => value,
        Err(_) => "norn".to_owned(),
    };
    parse_args_from(std::env::args().skip(1), default_norn_bin)
}

/// The argument-parsing core, fed an explicit argument iterator and `--norn-bin` default so
/// tests exercise the exact production logic without touching process globals.
///
/// Every value-taking flag bails with a clear message when its value is missing — a silent
/// fallback to a default would mask an operator typo.
fn parse_args_from(
    args: impl IntoIterator<Item = String>,
    default_norn_bin: String,
) -> anyhow::Result<Args> {
    let mut candidates: Vec<String> = Vec::new();
    let mut identity = "agent-dev-worker".to_owned();
    let mut ready_file: Option<String> = None;
    let mut norn_bin = default_norn_bin;
    let mut norn_account: Option<String> = None;
    let mut allow_degraded_session = false;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--address" => match args.next() {
                Some(value) => candidates.push(value),
                None => anyhow::bail!("--address requires a value"),
            },
            "--identity" => match args.next() {
                Some(value) => identity = value,
                None => anyhow::bail!("--identity requires a value"),
            },
            "--ready-file" => match args.next() {
                Some(value) => ready_file = Some(value),
                None => anyhow::bail!("--ready-file requires a value"),
            },
            "--norn-bin" => match args.next() {
                Some(value) => norn_bin = value,
                None => anyhow::bail!("--norn-bin requires a value"),
            },
            "--norn-account" => match args.next() {
                Some(value) => norn_account = Some(value),
                None => anyhow::bail!("--norn-account requires a value"),
            },
            // A bare switch: it takes no value, because there is nothing to
            // configure — either the operator accepts the fidelity tradeoff
            // for this worker's sessions or they do not.
            "--norn-allow-degraded-session" => allow_degraded_session = true,
            other => anyhow::bail!("unknown argument `{other}`"),
        }
    }
    if candidates.is_empty() {
        candidates.push("127.0.0.1:50061".to_owned());
    }
    let Some(norn_account) = norn_account else {
        anyhow::bail!(
            "--norn-account <alias|default> is required: the agent harness runs norn with \
             --resume-if-exists, and norn refuses to resume or open an existing OAuth session \
             without an account. Omitting it does not fail here — it fails inside every agent \
             activity, three retries deep, as `norn: auth error: authentication failed`"
        );
    };
    Ok(Args {
        candidates,
        identity,
        ready_file,
        norn_bin,
        norn_account,
        allow_degraded_session,
    })
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

#[cfg(test)]
mod tests {
    //! Unit tests of the argument-parsing core: defaults, explicit values, and the strict
    //! missing-value bail for every value-taking flag.

    use super::parse_args_from;

    /// Runs the parsing core on a literal argv with a fixed `--norn-bin` default.
    fn parse(args: &[&str]) -> anyhow::Result<super::Args> {
        parse_args_from(
            args.iter().map(|arg| (*arg).to_owned()),
            "norn-default".to_owned(),
        )
    }

    /// The account has NO default on purpose. Before this was required the
    /// worker started cleanly, logged "connected and registered; serving
    /// dispatches", and then failed every agent activity three times with
    /// `norn: auth error` — a healthy-looking worker that could not do its
    /// job. Failing at parse time is the whole fix.
    #[test]
    fn no_arguments_bails_because_the_norn_account_has_no_default() {
        let outcome = parse(&[]).err().map(|error| error.to_string());
        assert!(
            outcome.is_some(),
            "a missing --norn-account must bail, not default"
        );
        let error = outcome.unwrap_or_default();
        assert!(
            error.contains("--norn-account <alias|default> is required"),
            "the refusal must name the flag and its shape, got: {error}"
        );
        assert!(
            error.contains("norn: auth error"),
            "the refusal must name the failure it prevents, so an operator \
             who has already seen that error connects the two, got: {error}"
        );
    }

    #[test]
    fn the_other_flags_still_default_once_the_account_is_supplied() -> anyhow::Result<()> {
        let args = parse(&["--norn-account", "default"])?;
        assert_eq!(args.candidates, vec!["127.0.0.1:50061".to_owned()]);
        assert_eq!(args.identity, "agent-dev-worker");
        assert_eq!(args.ready_file, None);
        assert_eq!(args.norn_bin, "norn-default");
        assert_eq!(args.norn_account, "default");
        Ok(())
    }

    /// The negative control for the degraded-resume opt-in. Defaulting it on
    /// would silently accept a provider-fidelity tradeoff on the operator's
    /// behalf, which is exactly the kind of assumption this codebase forbids.
    #[test]
    fn degraded_session_resume_is_off_unless_asked_for() -> anyhow::Result<()> {
        let args = parse(&["--norn-account", "default"])?;
        assert!(
            !args.allow_degraded_session,
            "the degraded-resume opt-in must never default on"
        );
        Ok(())
    }

    #[test]
    fn the_degraded_session_switch_takes_no_value() -> anyhow::Result<()> {
        // A bare switch: the argument after it must still parse as its own
        // flag, not be swallowed as this one's value.
        let args = parse(&[
            "--norn-allow-degraded-session",
            "--norn-account",
            "default",
            "--identity",
            "worker-a",
        ])?;
        assert!(args.allow_degraded_session);
        assert_eq!(args.norn_account, "default");
        assert_eq!(args.identity, "worker-a");
        Ok(())
    }

    #[test]
    fn all_flags_with_values_parse_and_addresses_repeat() -> anyhow::Result<()> {
        let args = parse(&[
            "--address",
            "127.0.0.1:1111",
            "--address",
            "127.0.0.1:2222",
            "--identity",
            "worker-a",
            "--ready-file",
            "/tmp/ready",
            "--norn-bin",
            "/opt/norn",
            "--norn-account",
            "work-alias",
        ])?;
        assert_eq!(
            args.candidates,
            vec!["127.0.0.1:1111".to_owned(), "127.0.0.1:2222".to_owned()]
        );
        assert_eq!(args.identity, "worker-a");
        assert_eq!(args.ready_file.as_deref(), Some("/tmp/ready"));
        assert_eq!(args.norn_bin, "/opt/norn");
        assert_eq!(args.norn_account, "work-alias");
        Ok(())
    }

    /// NOTE: this list is an enumeration, so it only covers the flags someone
    /// remembered to add. A new value-taking flag joins the parser without
    /// joining this list and is silently untested — which is exactly how
    /// `--norn-account` came to be missing from the harness invocation in the
    /// first place. Add the flag here whenever you add one to `parse_args_from`.
    #[test]
    fn every_value_taking_flag_bails_when_its_value_is_missing() {
        for flag in [
            "--address",
            "--identity",
            "--ready-file",
            "--norn-bin",
            "--norn-account",
        ] {
            assert_eq!(
                parse(&[flag]).err().map(|error| error.to_string()),
                Some(format!("{flag} requires a value")),
                "a trailing value-less {flag} must bail"
            );
        }
    }

    #[test]
    fn unknown_argument_bails() {
        assert_eq!(
            parse(&["--bogus"]).err().map(|error| error.to_string()),
            Some("unknown argument `--bogus`".to_owned())
        );
    }
}
