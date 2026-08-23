//! `aion-fleet-chain-step` — advance the chain by EXACTLY ONE recorded
//! boundary, durably, and exit.
//!
//! This is the binary the sealed `fleet-chain-step.sh` seam calls once per
//! boundary. One invocation is one recorded fact, which is what makes
//! design §6's seven boundaries seven separately-recorded events rather
//! than one collapsed decision.
//!
//! # What one invocation does
//!
//! 1. Write its own pid to `--pid-file` and fsync it. **This is the
//!    flight's own dispatch receipt for this process**, and it is the only
//!    thing that makes a kill traceable to a process this flight started.
//! 2. Read the normalized graph and the recorded ledger.
//! 3. Decide the next action — a pure function of those two
//!    ([`aion_fleet_plan::chain::next_action`]).
//! 4. If the action needs an external result, WAIT AT THE BARRIER for the
//!    sealed script to deposit it in `--inbox`. The wait is a poll with a
//!    TOTAL DEADLINE and a DISTINCT timeout exit — never one blocking sleep
//!    sized to an expected duration.
//! 5. Append the resulting fact and write the ledger ATOMICALLY (temp file,
//!    fsync, rename). A crash before the rename leaves the previous ledger
//!    exactly as it was, so the restart re-derives the same next action.
//! 6. Print the applied fact and the new next action as JSON. Exit 0.
//!
//! # Exit codes
//!
//! | Code | Meaning |
//! |---|---|
//! | 0 | one boundary advanced, or the chain is already finished |
//! | 2 | a refusal this binary can name (bad input, undecodable ledger) |
//! | 75 | the barrier wait hit its total deadline — a DISTINCT outcome, never silently retried or treated as success |

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use aion_fleet_plan::chain::apply::{External, apply};
use aion_fleet_plan::chain::{ChainAction, ChainLedger, next_action};
use aion_fleet_plan::graph::NormalizedGraph;
use clap::Parser;

/// Exit code for a barrier wait that reached its total deadline.
const EXIT_BARRIER_TIMEOUT: u8 = 75;

/// Advance the staged chain by one recorded boundary.
#[derive(Debug, Parser)]
#[command(name = "aion-fleet-chain-step", version, about, long_about = None)]
struct Cli {
    /// The normalized typed graph — the run's ordering authority.
    #[arg(long)]
    graph: PathBuf,
    /// The recorded ledger, read and rewritten atomically.
    #[arg(long)]
    ledger: PathBuf,
    /// The chain's root base commit, resolved once at chain start.
    #[arg(long)]
    root_base: String,
    /// Where this process writes its own pid before doing any work.
    #[arg(long)]
    pid_file: Option<PathBuf>,
    /// The barrier directory sealed scripts deposit external results in.
    #[arg(long)]
    inbox: PathBuf,
    /// Total deadline for a barrier wait, in seconds.
    #[arg(long, default_value_t = 30)]
    deadline_secs: u64,
    /// Poll interval for a barrier wait, in milliseconds.
    #[arg(long, default_value_t = 50)]
    poll_millis: u64,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(code) => code,
        Err(Failure::Refused(message)) => {
            emit(&serde_json::json!({ "ok": false, "error": message }));
            ExitCode::from(2)
        }
        Err(Failure::BarrierTimeout { waited_for, secs }) => {
            emit(&serde_json::json!({
                "ok": false,
                "timeout": true,
                "waited_for": waited_for,
                "deadline_secs": secs,
                "detail": "the barrier wait reached its TOTAL DEADLINE; this is a distinct \
                           outcome, not a failure of the chain and not a success"
            }));
            ExitCode::from(EXIT_BARRIER_TIMEOUT)
        }
    }
}

enum Failure {
    Refused(String),
    BarrierTimeout { waited_for: String, secs: u64 },
}

fn run(cli: &Cli) -> Result<ExitCode, Failure> {
    if let Some(path) = &cli.pid_file {
        write_pid(path)?;
    }

    let graph: NormalizedGraph = read_json(&cli.graph)?;
    let mut ledger: ChainLedger = if cli.ledger.exists() {
        read_json(&cli.ledger)?
    } else {
        ChainLedger::new()
    };

    let action = next_action(&graph, &ledger, &cli.root_base)
        .map_err(|e| Failure::Refused(e.to_string()))?;

    if let ChainAction::Finish {
        outcome,
        result_commit,
    } = &action
    {
        emit(&serde_json::json!({
            "ok": true,
            "finished": true,
            "outcome": outcome,
            "result_commit": result_commit,
        }));
        return Ok(ExitCode::SUCCESS);
    }

    let external = match External::inbox_name(&action) {
        None => External::None,
        Some(name) => await_barrier(cli, &name)?,
    };

    let fact = apply(&action, &external).map_err(|e| Failure::Refused(e.to_string()))?;
    ledger.record(fact.clone());
    write_ledger_atomically(&cli.ledger, &ledger)?;

    let next = next_action(&graph, &ledger, &cli.root_base)
        .map_err(|e| Failure::Refused(e.to_string()))?;
    emit(&serde_json::json!({
        "ok": true,
        "finished": false,
        "applied": fact,
        "next": next,
    }));
    Ok(ExitCode::SUCCESS)
}

/// Wait for a sealed script to deposit an external result.
///
/// **Poll + total deadline + timeout as a distinct outcome.** There is
/// deliberately no single blocking sleep sized to how long a stage flight
/// is expected to take: a stage is hours long and a sleep that guessed its
/// duration would be both wrong and unobservable.
fn await_barrier(cli: &Cli, name: &str) -> Result<External, Failure> {
    let path = cli.inbox.join(name);
    let deadline = Duration::from_secs(cli.deadline_secs);
    let interval = Duration::from_millis(cli.poll_millis.max(1));
    let started = Instant::now();
    loop {
        if path.exists() {
            return read_json(&path);
        }
        if started.elapsed() >= deadline {
            return Err(Failure::BarrierTimeout {
                waited_for: name.to_owned(),
                secs: cli.deadline_secs,
            });
        }
        std::thread::sleep(interval);
    }
}

/// Write the pid, fsync it, and only then return — so a reader that sees
/// the file sees a complete pid.
fn write_pid(path: &Path) -> Result<(), Failure> {
    let pid = std::process::id();
    let mut file = std::fs::File::create(path)
        .map_err(|e| Failure::Refused(format!("cannot write pid file {}: {e}", path.display())))?;
    writeln!(file, "{pid}").map_err(|e| Failure::Refused(format!("cannot write pid: {e}")))?;
    file.sync_all()
        .map_err(|e| Failure::Refused(format!("cannot fsync pid file: {e}")))?;
    Ok(())
}

/// Write the ledger atomically: temp file in the same directory, fsync,
/// rename. A crash at any instant leaves either the old ledger or the new
/// one, never a torn one — which is what makes the resume re-derive the
/// same decision.
fn write_ledger_atomically(path: &Path, ledger: &ChainLedger) -> Result<(), Failure> {
    let text = serde_json::to_string_pretty(ledger)
        .map_err(|e| Failure::Refused(format!("cannot encode ledger: {e}")))?;
    let parent = path.parent().unwrap_or(Path::new("."));
    let temp = parent.join(format!(".{}.tmp", file_name(path)));
    {
        let mut file = std::fs::File::create(&temp)
            .map_err(|e| Failure::Refused(format!("cannot create {}: {e}", temp.display())))?;
        file.write_all(text.as_bytes())
            .map_err(|e| Failure::Refused(format!("cannot write ledger: {e}")))?;
        file.write_all(b"\n")
            .map_err(|e| Failure::Refused(format!("cannot write ledger: {e}")))?;
        file.sync_all()
            .map_err(|e| Failure::Refused(format!("cannot fsync ledger: {e}")))?;
    }
    std::fs::rename(&temp, path)
        .map_err(|e| Failure::Refused(format!("cannot rename ledger into place: {e}")))?;
    Ok(())
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || "ledger.json".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    )
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, Failure> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| Failure::Refused(format!("cannot read {}: {e}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|e| Failure::Refused(format!("cannot decode {}: {e}", path.display())))
}

fn emit(value: &serde_json::Value) {
    println!("{value}");
}
