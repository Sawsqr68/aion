//! `aion server stop`: resolve the recorded server, signal it, wait bounded,
//! and print the drain outcome as a typed report.
//!
//! The verb's decisions all live in [`aion_server::control::stop`]; this
//! module owns only the CLI surface — patience resolution (flag first, then
//! the running incarnation's recorded drain window, then the config's
//! `drain.timeout_seconds`, never an invented value) and the rendering of
//! each verdict face. Exit codes: 0 when the server is stopped or already
//! gone; 1 exclusively when it is still draining at patience (the one face a
//! script keys on — the process lives, wait or force); 2 when the verb could
//! not complete (a refusal before signalling, an unresolvable home or
//! config, or an error mid-verb — the message on stderr says which).

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
    /// wait patience when `--patience` is absent and no running incarnation's
    /// record answers (the config's `drain.timeout_seconds`).
    #[arg(long)]
    config: Option<PathBuf>,
    /// Seconds to wait for the server to exit before reporting it still
    /// draining. Absent, the running server's own recorded drain window
    /// governs, then the config's `drain.timeout_seconds` — the verb never
    /// invents a value.
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
    let (patience, patience_source) = match resolve_patience(args, &home) {
        Ok(resolved) => resolved,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    println!(
        "waiting up to {}s for the drain ({})",
        patience.as_secs(),
        patience_source.describe()
    );
    match aion_server::control::stop::stop(&home, patience) {
        Ok(StopVerdict::Outcome(outcome)) => render_outcome(&outcome),
        Ok(StopVerdict::Refusal(refusal)) => {
            eprintln!("aion server stop: {refusal}");
            render_refusal_note(&refusal);
            ExitCode::from(2)
        }
        Err(error) => {
            eprintln!("aion server stop: {error}");
            ExitCode::from(2)
        }
    }
}

/// Where the wait patience came from, named in the verb's own output so the
/// operator can see which value governed the wait.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PatienceSource {
    /// `--patience` on this invocation.
    Flag,
    /// The running incarnation's pid record — the drain window the server is
    /// ACTUALLY running with, which its own `--drain-timeout` or a config
    /// file edited since boot can have moved away from today's config.
    Record,
    /// The config file's `drain.timeout_seconds`, when no record exists to
    /// ask (nothing is running, or a stale record was already reconciled).
    Config,
}

impl PatienceSource {
    const fn describe(self) -> &'static str {
        match self {
            Self::Flag => "--patience",
            Self::Record => "the running server's recorded drain window",
            Self::Config => "the config's drain.timeout_seconds",
        }
    }
}

/// The wait patience: the flag when given, otherwise the running
/// incarnation's own recorded drain window, otherwise the config's drain
/// timeout. The record outranks the config for the same reason `status`
/// probes the record's addresses: the running server's window may have come
/// from its own `--drain-timeout`, and a re-derived value would wait out the
/// wrong window — misreporting a healthy long drain as "still draining", or
/// converting it to a forced exit on a second invocation. Loading the config
/// for the last fallback NEVER scaffolds one — a stop verb must not create
/// server state.
fn resolve_patience(
    args: &StopArgs,
    home: &std::path::Path,
) -> Result<(Duration, PatienceSource), String> {
    if let Some(seconds) = args.patience {
        return Ok((Duration::from_secs(seconds.get()), PatienceSource::Flag));
    }
    match aion_server::control::pid_file::read(home) {
        Ok(Some(record)) if record.drain_timeout_seconds > 0 => {
            return Ok((
                Duration::from_secs(record.drain_timeout_seconds),
                PatienceSource::Record,
            ));
        }
        // A zero window in a record would make the verb give up instantly;
        // no server writes one (the config validates the timeout non-zero),
        // so it reads as debris and falls through to the config.
        Ok(Some(_) | None) => {}
        // An unreadable pid file is the stop verb's own business to report
        // in full; for patience resolution it simply means the record cannot
        // answer, and the config still can.
        Err(_unreadable) => {}
    }
    let overrides = CliOverrides {
        config_path: args.config.clone(),
        ..CliOverrides::default()
    };
    match ServerConfig::load(&overrides) {
        Ok(config) => Ok((
            Duration::from_secs(config.drain.timeout_seconds),
            PatienceSource::Config,
        )),
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
        NoteFate::ArmedNotDisarmed {
            outcome,
            outcome_unreadable,
        } => {
            println!(
                "the death note's bracket for pid {pid} never closed: the process was \
                 destroyed without its run loop seeing the end (the `kill -9` shape)"
            );
            match (outcome, outcome_unreadable) {
                (Some(record), _) => render_outcome_record(record),
                (None, Some(unreadable)) => render_unreadable_outcome(unreadable),
                (None, None) => println!(
                    "no drain outcome was recorded — the drain never got far enough to \
                     write one; in-flight work recovers from durable state on the next \
                     boot"
                ),
            }
        }
        NoteFate::Disarmed {
            outcome,
            outcome_unreadable,
            reason,
        } => {
            println!("exit recorded: {reason}");
            match (outcome, outcome_unreadable) {
                (Some(record), _) => render_outcome_record(record),
                (None, Some(unreadable)) => render_unreadable_outcome(unreadable),
                (None, None) => println!(
                    "no drain outcome record was written for this exit (an error return \
                     before serving, or a pre-record binary); reporting that absence, \
                     not a summary"
                ),
            }
        }
    }
}

/// The unreadable-presence face: a drain outcome record EXISTS but this
/// binary could not parse it. Reported as exactly that — never as "no
/// record", which would send the operator chasing a drain that in fact ran
/// far enough to write its receipt.
fn render_unreadable_outcome(unreadable: &str) {
    println!(
        "a drain outcome record was written but could not be read ({unreadable}); \
         a torn line from a mid-write kill, or a record from a build this binary \
         cannot parse"
    );
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

#[cfg(test)]
mod tests {
    use super::{PatienceSource, StopArgs, resolve_patience};
    use std::num::NonZeroU64;
    use std::time::Duration;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// A pid record with a known drain window, written where the verb reads.
    fn write_record(home: &std::path::Path, drain_timeout_seconds: u64) -> TestResult {
        let run_dir = home.join("run");
        std::fs::create_dir_all(&run_dir)?;
        let record = serde_json::json!({
            "pid": 4242,
            "started_at_unix_secs": 1_000_000,
            "binary_sha256": "0".repeat(64),
            "version": "0.0.0-test",
            "commit": "test",
            "http_address": "127.0.0.1:8080",
            "grpc_address": "127.0.0.1:50051",
            "drain_timeout_seconds": drain_timeout_seconds,
        });
        std::fs::write(run_dir.join("aion-server.pid"), format!("{record}\n"))?;
        Ok(())
    }

    /// A config file naming a drain window, for the last-resort fallback.
    fn write_config(
        dir: &std::path::Path,
        timeout_seconds: u64,
    ) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        let path = dir.join("server-config.toml");
        std::fs::write(
            &path,
            format!(
                r#"workflow_packages = []

[server]
listen_address = "127.0.0.1:18080"
grpc_address = "127.0.0.1:15005"

[store]
backend = "memory"

[drain]
timeout_seconds = {timeout_seconds}
"#
            ),
        )?;
        Ok(path)
    }

    /// R2: the flag outranks everything, including a live record.
    #[test]
    fn the_patience_flag_outranks_the_record_and_the_config() -> TestResult {
        let home = tempfile::tempdir()?;
        write_record(home.path(), 300)?;
        let args = StopArgs {
            config: None,
            patience: NonZeroU64::new(7),
        };
        let (patience, source) = resolve_patience(&args, home.path())
            .map_err(|message| -> Box<dyn std::error::Error> { message.into() })?;
        assert_eq!(patience, Duration::from_secs(7));
        assert_eq!(source, PatienceSource::Flag);
        Ok(())
    }

    /// Absent the flag, the RUNNING incarnation's recorded window governs —
    /// not a re-derivation from today's config, which the server's own
    /// `--drain-timeout` or an edited file can have moved away from.
    #[test]
    fn an_absent_flag_reads_the_running_servers_recorded_window() -> TestResult {
        let home = tempfile::tempdir()?;
        write_record(home.path(), 300)?;
        let config = write_config(home.path(), 30)?;
        let args = StopArgs {
            config: Some(config),
            patience: None,
        };
        let (patience, source) = resolve_patience(&args, home.path())
            .map_err(|message| -> Box<dyn std::error::Error> { message.into() })?;
        assert_eq!(
            patience,
            Duration::from_secs(300),
            "the record's window must outrank the config's"
        );
        assert_eq!(source, PatienceSource::Record);
        Ok(())
    }

    /// With no flag and no record, the config's own drain timeout governs.
    #[test]
    fn an_absent_record_falls_back_to_the_config() -> TestResult {
        let home = tempfile::tempdir()?;
        let config = write_config(home.path(), 45)?;
        let args = StopArgs {
            config: Some(config),
            patience: None,
        };
        let (patience, source) = resolve_patience(&args, home.path())
            .map_err(|message| -> Box<dyn std::error::Error> { message.into() })?;
        assert_eq!(patience, Duration::from_secs(45));
        assert_eq!(source, PatienceSource::Config);
        Ok(())
    }
}
