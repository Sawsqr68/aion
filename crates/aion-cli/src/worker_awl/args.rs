//! The AWL worker's flag and environment vocabulary, and its resolution.
//!
//! # Nothing is invented
//!
//! [`aion_worker::WorkerConfigBuilder`] refuses to build without
//! `max_concurrency` and all three reconnect settings, deliberately: those are
//! operator policy and the SDK has no business guessing them. This verb does
//! not guess either. Every setting comes from a flag or from the named
//! environment variable, the flag wins, and every missing one is reported at
//! once, by name — the same contract `aion worker agent` publishes, through
//! the same [`crate::settings`] mechanics and the same variable names, so a
//! deployment already running an Aion worker can run this one unchanged.
//!
//! The one setting that is DERIVED rather than demanded is the worker's
//! identity, and only because the document names the queue it serves: a
//! worker document declares exactly one queue, so `<queue>-awl-worker` is a
//! reading of the document rather than a choice made here. `--identity`
//! overrides it for a fleet that names its workers itself.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Result, bail};
use clap::Args;

use crate::settings::{self, Environment};

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

/// `aion worker awl` command-line surface.
#[derive(Debug, Args)]
pub(crate) struct AwlWorkerArgs {
    /// The `.awl` WORKER document to serve: a file whose opening declaration
    /// is `worker <queue>`. A workflow document is refused by name.
    pub(crate) document: PathBuf,
    /// The identity this worker registers under.
    ///
    /// Derived from the document as `<queue>-awl-worker` when absent, which
    /// is a reading of the document rather than an invented default.
    #[arg(long)]
    pub(crate) identity: Option<String>,
    /// Maximum activities served at once.
    #[arg(long)]
    pub(crate) concurrency: Option<String>,
    /// Seconds before the first reconnect attempt.
    #[arg(long)]
    pub(crate) reconnect_initial_backoff: Option<String>,
    /// Reconnect backoff cap, in seconds.
    #[arg(long)]
    pub(crate) reconnect_max_backoff: Option<String>,
    /// Cumulative session-drop budget.
    #[arg(long)]
    pub(crate) reconnect_max_attempts: Option<String>,
    /// Print the exact action surface this worker would advertise, with the
    /// command each action runs, then exit without connecting.
    ///
    /// Those descriptors are the whole admission decision on a queue carrying
    /// a deployed contract, so reading them before dialling turns a remote
    /// contract-mismatch refusal into something checkable at the desk.
    #[arg(long)]
    pub(crate) check: bool,
}

/// Every connection setting, resolved from flags and environment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ConnectionSettings {
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
}

/// Resolve every connection setting for the worker serving `task_queue`.
///
/// # Errors
///
/// Returns an error listing EVERY setting that has no source, each named by
/// its flag and its variable, or one naming the single setting whose supplied
/// value is unusable (not a number, not greater than zero).
pub(crate) fn resolve(
    args: &AwlWorkerArgs,
    task_queue: &str,
    environment: &Environment,
) -> Result<ConnectionSettings> {
    let mut missing = Vec::new();
    let identity = settings::optional(args.identity.clone(), IDENTITY_ENV, environment)
        .unwrap_or_else(|| format!("{task_queue}-awl-worker"));
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
    settings::refuse_if_missing(missing)?;

    // Every value below is present: the missing-list was empty, so each
    // `resolve_one` returned `Some`. The `let else` refusal names the
    // settings rather than unwrapping, so a future edit that drops one from
    // the missing-list is a reported fault instead of a panic.
    let (Some(concurrency), Some(initial_backoff), Some(max_backoff), Some(max_attempts)) =
        (concurrency, initial_backoff, max_backoff, max_attempts)
    else {
        bail!("every worker connection setting must be resolved before use");
    };
    Ok(ConnectionSettings {
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
    })
}
