//! The agent worker's flag and environment vocabulary, and its resolution.
//!
//! # Nothing is invented
//!
//! [`aion_worker::WorkerConfigBuilder`] refuses to build without `max_concurrency`
//! and all three reconnect settings, deliberately: those are operator policy and
//! the SDK has no business guessing them. So this verb does not guess either. Every
//! setting is supplied by a flag or by the named environment variable, the flag
//! wins, and a missing one is reported BY NAME — with every missing setting listed
//! at once, because a worker started from a shell should need one correction rather
//! than seven. That is the same contract the Python worker host publishes, and the
//! variable names are the ones the rest of the product already reads, so a
//! deployment that runs an Aion worker today can run this one with the same
//! environment. The mechanics of that contract live in [`crate::settings`], shared
//! with the harness configuration so ONE refusal covers both.
//!
//! # Why the harness settings are not declared here
//!
//! `--harness` and the per-harness configuration (`--acp-command`, `--norn-binary`, …)
//! are flattened in from [`crate::harness`], the composition root. This verb is
//! harness-NEUTRAL: it derives a surface, resolves a connection, and serves whatever
//! harness was composed. Declaring a concrete adapter's flags here would put the name
//! `acp` in a verb that must not know which adapters exist — so the flags live with the
//! code that names those adapters anyway, and this module only carries them through.
//!
//! # Why the address is not `--endpoint`
//!
//! The global `--endpoint` is a gRPC target, and the agent-harness seam does not
//! exist on the gRPC worker transport: `AgentHarnessConfig` is threaded only
//! through the liminal serve path, and the server's own bridge notes that the gRPC
//! arm carries no agent binding. An agent worker therefore dials LIMINAL candidate
//! addresses, which is what `--liminal-address` supplies. Repeating it gives the
//! redial driver an ordered candidate list, so a worker whose server is `kill -9`'d
//! migrates to a survivor and re-registers there.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Result, bail};
use clap::Args;

use crate::harness::{HarnessArgs, HarnessKind, ResolvedHarness};
use crate::settings::{self, Environment, Missing};

/// Ordered liminal candidate addresses, comma-separated.
const LIMINAL_ADDRESSES_ENV: &str = "AION_WORKER_LIMINAL_ADDRESSES";
/// The queue selector, when the document declares more than one `worker` block.
const TASK_QUEUE_ENV: &str = "AION_TASK_QUEUE";
/// The worker's stable build identity.
const IDENTITY_ENV: &str = "AION_WORKER_IDENTITY";
/// Maximum activities served at once.
const CONCURRENCY_ENV: &str = "AION_WORKER_CONCURRENCY";
/// Seconds before the first reconnect attempt.
const INITIAL_BACKOFF_ENV: &str = "AION_RECONNECT_INITIAL_BACKOFF_SECONDS";
/// Reconnect backoff cap, in seconds.
const MAX_BACKOFF_ENV: &str = "AION_RECONNECT_MAX_BACKOFF_SECONDS";
/// Cumulative session-drop budget.
const MAX_ATTEMPTS_ENV: &str = "AION_RECONNECT_MAX_ATTEMPTS";

/// `aion worker agent` command-line surface.
#[derive(Clone, Debug, Args)]
pub(crate) struct AgentArgs {
    /// The `.awl` document declaring the queue and the actions to serve.
    ///
    /// The document is the only source of the worker's advertised schemas: an
    /// agent activity's output is harness-owned, so there is no Rust type to
    /// derive one from. Without the document the worker advertises nothing and
    /// every queue carrying a deployed contract refuses it.
    pub(crate) awl: PathBuf,
    /// Liminal listen address to dial, for example `127.0.0.1:50061`. Repeat it
    /// to supply ordered redial candidates: when the connected server dies, the
    /// worker migrates to the next candidate and re-registers there.
    ///
    /// Env: `AION_WORKER_LIMINAL_ADDRESSES` (comma-separated, same order). Not
    /// the global `--endpoint`: that is a gRPC target, and the agent-harness
    /// seam exists only on the liminal worker transport.
    #[arg(long = "liminal-address")]
    pub(crate) liminal_addresses: Vec<String>,
    /// Serve this queue instead of deriving it from the document.
    ///
    /// A document declaring exactly one `worker` block leaves nothing to choose,
    /// so the queue is derived from it. This selects among several.
    /// Env: `AION_TASK_QUEUE`.
    #[arg(long)]
    pub(crate) task_queue: Option<String>,
    /// Stable worker-build identity, used to correlate compatibility decisions.
    /// Env: `AION_WORKER_IDENTITY`.
    #[arg(long)]
    pub(crate) identity: Option<String>,
    /// Maximum activities served at once. Env: `AION_WORKER_CONCURRENCY`.
    #[arg(long)]
    pub(crate) concurrency: Option<String>,
    /// Seconds before the first reconnect attempt.
    /// Env: `AION_RECONNECT_INITIAL_BACKOFF_SECONDS`.
    #[arg(long)]
    pub(crate) reconnect_initial_backoff: Option<String>,
    /// Reconnect backoff cap, in seconds.
    /// Env: `AION_RECONNECT_MAX_BACKOFF_SECONDS`.
    #[arg(long)]
    pub(crate) reconnect_max_backoff: Option<String>,
    /// Cumulative session-drop budget before the run fails.
    /// Env: `AION_RECONNECT_MAX_ATTEMPTS`.
    #[arg(long)]
    pub(crate) reconnect_max_attempts: Option<String>,
    /// Locality affinity advertised at registration. Defaults to this machine's
    /// hostname, the worker SDK's own documented default.
    #[arg(long)]
    pub(crate) node: Option<String>,
    /// Print the typed action surface this worker would advertise, then exit
    /// without connecting to anything.
    ///
    /// Those descriptors are the whole admission decision on a contract-bearing
    /// queue, so seeing them before dialling turns a remote contract-mismatch
    /// refusal into something readable at the desk.
    #[arg(long)]
    pub(crate) check: bool,
    /// Which agent harness to serve, and how to launch it.
    #[command(flatten)]
    pub(crate) harness: HarnessArgs,
}

/// Every connection setting, resolved from flags and environment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ConnectionSettings {
    /// Ordered liminal candidate addresses the redial driver cycles.
    pub(crate) addresses: Vec<String>,
    /// The worker's advertised identity.
    pub(crate) identity: String,
    /// Maximum activities served at once.
    pub(crate) concurrency: usize,
    /// Delay before the first reconnect attempt.
    pub(crate) initial_backoff: Duration,
    /// Reconnect backoff cap.
    pub(crate) max_backoff: Duration,
    /// Cumulative session-drop budget.
    pub(crate) max_attempts: usize,
    /// Locality affinity, or `None` to take the SDK's hostname default.
    pub(crate) node: Option<String>,
}

impl AgentArgs {
    /// The queue selection, if the operator made one explicitly.
    ///
    /// `None` leaves the queue to be DERIVED from the document, which is
    /// unambiguous only when the document declares exactly one `worker` block.
    pub(crate) fn requested_queue(&self, environment: &Environment) -> Option<String> {
        settings::optional(self.task_queue.clone(), TASK_QUEUE_ENV, environment)
    }
}

/// Resolves every connection setting from `args` and `environment`, plus the
/// configuration of the already-resolved `harness`.
///
/// Flags win over the environment. `node` falls through to the worker SDK's own
/// documented hostname default when absent, because that default is the library's
/// and not one invented here; every other setting must be supplied.
///
/// The two groups are resolved together on purpose: a shell invocation missing both
/// `--identity` and `--acp-permission` is one mistake with two parts, and an operator
/// should see both parts at once rather than discover the second only after fixing the
/// first. `harness` is `None` for a build that compiles no adapter in, which demands no
/// harness configuration at all — the serve path then refuses, naming the build.
///
/// # Errors
///
/// Returns an error listing EVERY setting that has no source, each named by its
/// flag and its variable; or one naming the single setting whose supplied value is
/// unusable (not a number, not greater than zero, a relative ACP working directory, an
/// unknown permission policy).
pub(crate) fn resolve(
    args: &AgentArgs,
    environment: &Environment,
    harness: Option<HarnessKind>,
) -> Result<(ConnectionSettings, Option<ResolvedHarness>)> {
    let mut missing = Vec::new();
    let addresses = resolve_addresses(args, environment, &mut missing);
    let identity = settings::resolve_one(
        args.identity.clone(),
        IDENTITY_ENV,
        "--identity",
        environment,
        &mut missing,
    );
    let concurrency = settings::resolve_one(
        args.concurrency.clone(),
        CONCURRENCY_ENV,
        "--concurrency",
        environment,
        &mut missing,
    );
    let initial_backoff = settings::resolve_one(
        args.reconnect_initial_backoff.clone(),
        INITIAL_BACKOFF_ENV,
        "--reconnect-initial-backoff",
        environment,
        &mut missing,
    );
    let max_backoff = settings::resolve_one(
        args.reconnect_max_backoff.clone(),
        MAX_BACKOFF_ENV,
        "--reconnect-max-backoff",
        environment,
        &mut missing,
    );
    let max_attempts = settings::resolve_one(
        args.reconnect_max_attempts.clone(),
        MAX_ATTEMPTS_ENV,
        "--reconnect-max-attempts",
        environment,
        &mut missing,
    );
    let supplied_harness = harness.map(|kind| {
        (
            kind,
            args.harness
                .collect(kind.settings(), environment, &mut missing),
        )
    });
    settings::refuse_if_missing(missing)?;

    // Every value below is present: the missing-list was empty, so each `resolve_one`
    // returned `Some`. The `let else` refusals name the setting rather than
    // unwrapping, so a future edit that drops one from the missing-list is a
    // reported fault instead of a panic.
    let (Some(identity), Some(concurrency)) = (identity, concurrency) else {
        bail!("--identity and --concurrency must both be resolved before use");
    };
    let (Some(initial_backoff), Some(max_backoff), Some(max_attempts)) =
        (initial_backoff, max_backoff, max_attempts)
    else {
        bail!("every reconnect setting must be resolved before use");
    };
    let settings = ConnectionSettings {
        addresses,
        identity,
        concurrency: settings::positive_usize(&concurrency, "--concurrency", CONCURRENCY_ENV)?,
        initial_backoff: settings::positive_seconds(
            &initial_backoff,
            "--reconnect-initial-backoff",
            INITIAL_BACKOFF_ENV,
        )?,
        max_backoff: settings::positive_seconds(
            &max_backoff,
            "--reconnect-max-backoff",
            MAX_BACKOFF_ENV,
        )?,
        max_attempts: settings::positive_usize(
            &max_attempts,
            "--reconnect-max-attempts",
            MAX_ATTEMPTS_ENV,
        )?,
        node: args.node.clone(),
    };
    let harness = supplied_harness
        .map(|(kind, supplied)| {
            supplied
                .parse()
                .map(|config| ResolvedHarness { kind, config })
        })
        .transpose()?;
    Ok((settings, harness))
}

/// The ordered liminal candidates: the repeated flag when given, otherwise the
/// comma-separated variable. Blank entries are dropped, so a trailing comma or a
/// stray space never becomes an address nobody can dial.
fn resolve_addresses(
    args: &AgentArgs,
    environment: &Environment,
    missing: &mut Vec<Missing>,
) -> Vec<String> {
    let from_flags = args
        .liminal_addresses
        .iter()
        .map(|address| address.trim().to_owned())
        .filter(|address| !address.is_empty())
        .collect::<Vec<_>>();
    if !from_flags.is_empty() {
        return from_flags;
    }
    let from_environment = environment
        .read(LIMINAL_ADDRESSES_ENV)
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|address| !address.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if from_environment.is_empty() {
        missing.push(Missing {
            flag: "--liminal-address",
            variable: LIMINAL_ADDRESSES_ENV,
        });
    }
    from_environment
}
