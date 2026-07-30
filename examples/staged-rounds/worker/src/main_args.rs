//! Command-line parsing for the staged-rounds worker binary.

/// The default liminal listen address the worker dials — the server's
/// `[outbox] liminal_listen_address`.
pub(super) const DEFAULT_ADDRESS: &str = "127.0.0.1:50061";

/// The default per-connection concurrency for the developer and reviewer
/// connections (the genuine-parallelism knob: N parallel item agents).
pub(super) const DEFAULT_MAX_PARALLEL: usize = 4;

/// Parsed command-line arguments.
#[derive(Debug)]
pub(super) struct Args {
    pub(super) candidates: Vec<String>,
    pub(super) identity_prefix: String,
    pub(super) ready_file: Option<String>,
    pub(super) norn_bin: String,
    /// The directory the four role profiles are loaded from — required:
    /// this package's `worker/profiles/`.
    pub(super) profiles_dir: String,
    /// `max_concurrency` for the developer and reviewer connections: how
    /// many item agents may run at once.
    pub(super) max_parallel: usize,
    /// The Norn OAuth account every agent run is pinned to — required.
    ///
    /// Every agent activity here runs with `--session-id` and
    /// `--resume-if-exists`, and Norn refuses a resumed, forked or
    /// open-or-resume run without an explicit `--account`, deliberately, so
    /// that credential affinity can never change silently underneath a
    /// long-lived session. So this worker cannot serve a SINGLE agent activity
    /// without one, and a missing value is a startup refusal rather than a
    /// failure on first dispatch — on 2026-07-30 the omission cost a whole
    /// first flight, every connection registered and the run dying on the
    /// planner with `norn: auth error`. Use `default` for Norn's
    /// compatibility slot.
    pub(super) norn_account: String,
}

pub(super) fn parse_args() -> anyhow::Result<Args> {
    let default_norn_bin = std::env::var("NORN_BIN").unwrap_or_else(|_| "norn".to_owned());
    parse_args_from(std::env::args().skip(1), default_norn_bin)
}

/// The argument-parsing core, fed an explicit iterator and defaults so tests
/// exercise production logic without touching process globals.
pub(super) fn parse_args_from(
    args: impl IntoIterator<Item = String>,
    default_norn_bin: String,
) -> anyhow::Result<Args> {
    let mut candidates = Vec::new();
    let mut identity_prefix = "staged-rounds-worker".to_owned();
    let mut ready_file = None;
    let mut norn_bin = default_norn_bin;
    let mut profiles_dir = None;
    let mut norn_account = None;
    let mut max_parallel = DEFAULT_MAX_PARALLEL;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--address" => candidates.push(next_value(&mut args, "--address")?),
            "--identity" => identity_prefix = next_value(&mut args, "--identity")?,
            "--ready-file" => ready_file = Some(next_value(&mut args, "--ready-file")?),
            "--norn-bin" => norn_bin = next_value(&mut args, "--norn-bin")?,
            "--profiles-dir" => profiles_dir = Some(next_value(&mut args, "--profiles-dir")?),
            "--norn-account" => norn_account = Some(next_value(&mut args, "--norn-account")?),
            "--max-parallel" => {
                let value = next_value(&mut args, "--max-parallel")?;
                max_parallel = value.parse().map_err(|error| {
                    anyhow::anyhow!("--max-parallel must be a positive integer: {error}")
                })?;
                if max_parallel == 0 {
                    anyhow::bail!("--max-parallel must be at least 1");
                }
            }
            other => anyhow::bail!("unknown argument `{other}`"),
        }
    }
    if candidates.is_empty() {
        candidates.push(DEFAULT_ADDRESS.to_owned());
    }
    let profiles_dir = profiles_dir.ok_or_else(|| {
        anyhow::anyhow!("--profiles-dir is required (point it at this package's worker/profiles/)")
    })?;
    let norn_account = norn_account.ok_or_else(|| {
        anyhow::anyhow!(
            "--norn-account is required: every agent activity runs with --session-id and \
             --resume-if-exists, and Norn refuses a resumed or open-or-resume run without an \
             explicit --account so credential affinity cannot change silently under a \
             long-lived session. Without it this worker registers fine and then fails every \
             single agent dispatch. Use `default` for Norn's compatibility slot, or an account \
             alias."
        )
    })?;
    Ok(Args {
        candidates,
        identity_prefix,
        ready_file,
        norn_bin,
        profiles_dir,
        max_parallel,
        norn_account,
    })
}

/// Take the value for a value-taking flag, bailing clearly when it is missing.
fn next_value(args: &mut impl Iterator<Item = String>, flag: &str) -> anyhow::Result<String> {
    args.next()
        .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))
}
