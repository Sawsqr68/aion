//! `aion server stop`: resolve the recorded server, signal it, wait bounded,
//! and print the drain outcome as a typed report.
//!
//! The verb's decisions all live in [`aion_server::control::stop`]; this
//! module owns only the CLI surface — patience resolution (flag first, then
//! the config's own `drain.timeout_seconds`, never an invented value) and the
//! rendering of each verdict face. Exit codes: 0 when the server is stopped
//! or already gone, 1 when it is still draining at patience (a script must
//! not proceed while the process lives) or the verb itself failed, 2 for a
//! refusal (nothing was signalled).

use std::num::NonZeroU64;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use aion_server::config::{CliOverrides, ServerConfig, aion_home};
use aion_server::control::{NoteFate, OutcomeRecord, StopOutcome, StopRefusal, StopVerdict};
use clap::Args;

/// Arguments for `aion server stop`.
#[derive(Args, Clone, Debug)]
pub struct StopArgs {
    /// Path to the TOML server configuration file. Used only to resolve the
    /// wait patience when `--patience` is absent (the config's
    /// `drain.timeout_seconds`).
    #[arg(long)]
    config: Option<PathBuf>,
    /// Seconds to wait for the server to exit before reporting it still
    /// draining. Absent, the config's own `drain.timeout_seconds` governs —
    /// the verb never invents a value.
    #[arg(long)]
    patience: Option<NonZeroU64>,
}

/// Run `aion server stop`.
pub fn run(args: &StopArgs) -> ExitCode {
    let home = match aion_home() {
        Ok(home) => home.path,
        Err(error) => {
            eprintln!("aion server stop: could not resolve the Aion home: {error}");
            return ExitCode::from(2);
        }
    };
    let patience = match resolve_patience(args) {
        Ok(patience) => patience,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    match aion_server::control::stop::stop(&home, patience) {
        Ok(StopVerdict::Outcome(outcome)) => render_outcome(&outcome),
        Ok(StopVerdict::Refusal(refusal)) => {
            eprintln!("aion server stop: {refusal}");
            render_refusal_note(&refusal);
            ExitCode::from(2)
        }
        Err(error) => {
            eprintln!("aion server stop: {error}");
            ExitCode::FAILURE
        }
    }
}

/// The wait patience: the flag when given, otherwise the config's own drain
/// timeout. Loading the config for the fallback NEVER scaffolds one — a stop
/// verb must not create server state.
fn resolve_patience(args: &StopArgs) -> Result<Duration, String> {
    if let Some(seconds) = args.patience {
        return Ok(Duration::from_secs(seconds.get()));
    }
    let overrides = CliOverrides {
        config_path: args.config.clone(),
        ..CliOverrides::default()
    };
    match ServerConfig::load(&overrides) {
        Ok(config) => Ok(Duration::from_secs(config.drain.timeout_seconds)),
        Err(error) => Err(format!(
            "aion server stop: could not resolve the wait patience from configuration \
             ({error}); pass `--patience <seconds>` to rule it at invocation"
        )),
    }
}

fn render_outcome(outcome: &StopOutcome) -> ExitCode {
    match outcome {
        StopOutcome::Stopped {
            record,
            fate,
            waited,
            pid_file_reconciled,
        } => {
            println!(
                "stopped: pid {} (version {}, commit {}) exited after {:.1}s",
                record.pid,
                record.version,
                record.commit,
                waited.as_secs_f64()
            );
            render_fate(record.pid, fate);
            if *pid_file_reconciled {
                println!("the pid file the exiting server left behind was reconciled away");
            }
            ExitCode::SUCCESS
        }
        StopOutcome::AlreadyGone {
            record,
            fate,
            pid_file_reconciled,
        } => {
            println!(
                "already gone: recorded pid {} (version {}, commit {}) is not running; \
                 nothing was signalled",
                record.pid, record.version, record.commit
            );
            render_fate(record.pid, fate);
            if *pid_file_reconciled {
                println!("the stale pid file was reconciled away");
            }
            ExitCode::SUCCESS
        }
        StopOutcome::StillDraining { record, waited } => {
            println!(
                "still draining: pid {} is still running after {:.1}s of patience. The \
                 server owns its drain; run `aion server stop` again to force immediate \
                 exit (the server treats a second termination signal as force), or wait \
                 and re-run `aion server status`",
                record.pid,
                waited.as_secs_f64()
            );
            ExitCode::FAILURE
        }
    }
}

/// Render what the death note records for the stopped incarnation. Public to
/// the crate: `aion server status` renders the same account for a dead
/// recorded server.
pub(crate) fn render_fate(pid: u32, fate: &NoteFate) {
    match fate {
        NoteFate::NoNote => {
            println!("no death note exists under this home; the exit left no recorded account");
        }
        NoteFate::NoBracketForPid => {
            println!(
                "the death note has no record for pid {pid}; the exit left no recorded \
                 account"
            );
        }
        NoteFate::ArmedNotDisarmed { outcome } => {
            println!(
                "the death note's bracket for pid {pid} never closed: the process was \
                 destroyed without its run loop seeing the end (the `kill -9` shape)"
            );
            match outcome {
                Some(record) => render_outcome_record(record),
                None => println!(
                    "no drain outcome was recorded — the drain never got far enough to \
                     write one; in-flight work recovers from durable state on the next \
                     boot"
                ),
            }
        }
        NoteFate::Disarmed { outcome, reason } => {
            println!("exit recorded: {reason}");
            match outcome {
                Some(record) => render_outcome_record(record),
                None => println!(
                    "no drain outcome record was written for this exit (an error return \
                     before serving, or a pre-record binary); reporting that absence, \
                     not a summary"
                ),
            }
        }
    }
}

fn render_outcome_record(record: &OutcomeRecord) {
    println!(
        "drain outcome: {:?} (window {}s, drain requests delivered to {} worker(s))",
        record.outcome, record.drain_timeout_seconds, record.delivered_drain_requests
    );
    for parked in &record.parked {
        let queue = parked.queue.as_deref().unwrap_or("unknown queue");
        println!(
            "  parked on worker {} ({queue}): {}",
            parked.worker,
            parked.tasks.join(", ")
        );
    }
    for declared in &record.parked_declared_commands {
        println!(
            "  declared command still executing at drain end: {declared} — its process \
             ended with the server; the next boot re-dispatches the attempt"
        );
    }
    if !record.managed_workers_stopped.is_empty() {
        println!(
            "  managed workers stopped: {}",
            record.managed_workers_stopped.join(", ")
        );
    }
    for unstopped in &record.managed_workers_unstopped {
        println!("  managed worker NOT proven stopped: {unstopped}");
    }
}

/// One extra line for the refusal faces whose remedy benefits from being
/// spelled out beyond the error's own text.
fn render_refusal_note(refusal: &StopRefusal) {
    if matches!(refusal, StopRefusal::StaleIncarnation { .. }) {
        eprintln!(
            "nothing was signalled and nothing was removed: the recorded server is \
             not the running process, and killing by number is exactly what this \
             verb exists to prevent"
        );
    }
}
