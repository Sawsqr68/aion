//! `aion server stop`: resolve the recorded server, signal it, wait bounded,
//! and print the drain outcome as a typed report.
//!
//! The verb's decisions all live in [`aion_server::control::stop`]; this
//! module owns only the CLI surface — patience resolution (flag first, then
//! the VERIFIED-RUNNING incarnation's recorded drain window, then the
//! config's `drain.timeout_seconds`, never an invented value) and the
//! rendering of each verdict face. Exit codes: 0 when the goal state holds —
//! the server stopped, was already gone, or no server has claimed the home
//! at all (stopping is idempotent: "nothing is running" is success, exactly
//! as `systemctl stop` rules it); 1 exclusively when it is still draining at
//! patience (the one face a script keys on — the process lives, wait or
//! force); 2 when the verb could not complete (a refusal before signalling,
//! an unresolvable home, or an error mid-verb — the message on stderr says
//! which). A configuration that cannot load is NOT by itself exit 2: the
//! patience it would have resolved is only needed to wait on a verified
//! RUNNING server, so the failure is stated as a note and carried to the
//! point of need — a home where the goal state already holds still answers
//! 0, and only a running server that genuinely needs the window turns it
//! into a refusal. `aion server status` answers a different question —
//! "is it up?" is a predicate, so DOWN is its exit 1 — which is why the two
//! verbs give the same situation different codes on purpose.

use std::num::NonZeroU64;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use aion_server::config::{CliOverrides, ServerConfig, aion_home};
use aion_server::control::{
    IncarnationProbe, NoteFate, OutcomeRecord, StopOutcome, StopRefusal, StopVerdict,
};
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
    let running = verified_running_record(&home);
    let patience = resolve_patience(args, running.as_ref());
    // A resolution failure is stated as its own layer's fact, never allowed
    // to destroy the verb's answer: the patience is only NEEDED to wait on a
    // verified-running server, and whether one exists is the stop flow's own
    // finding. Erroring out here reported "the verb could not complete"
    // (exit 2) over homes where the goal state already held — a deploy
    // script's idempotent pre-stop escalated against a stopped server
    // because a config file was broken.
    if let Err(message) = &patience {
        eprintln!(
            "aion server stop: the wait patience could not be resolved from \
             configuration ({message}); it is only needed to wait on a running \
             server — if one is found, the stop will refuse and name the remedy"
        );
    }
    // Announce the wait only when one can actually happen: a verified-running
    // recorded server AND a resolved window. Announcing over an empty home,
    // a stale record, or an unresolved patience would claim an action the
    // verb is not going to perform.
    if let (Some(_), Ok((patience, patience_source))) = (running.as_ref(), patience.as_ref()) {
        println!(
            "waiting up to {}s for the drain ({})",
            patience.as_secs(),
            patience_source.describe()
        );
    }
    match aion_server::control::stop::stop(&home, patience.map(|(patience, _)| patience)) {
        Ok(StopVerdict::Outcome(outcome)) => render_outcome(&outcome),
        // The goal state — nothing running at this home — already holds:
        // stopping is idempotent, so an unclaimed home is success, not a
        // failure a deploy script must special-case against debris presence.
        Ok(StopVerdict::Refusal(refusal @ StopRefusal::NoPidFile { .. })) => {
            println!("{refusal}");
            println!("nothing to stop: no server has claimed this home");
            ExitCode::SUCCESS
        }
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

/// The recorded incarnation, but only when the record's process is verified
/// LIVE with the recorded start instant — the only case in which the record
/// speaks for a running server rather than for debris.
fn verified_running_record(home: &std::path::Path) -> Option<aion_server::control::PidRecord> {
    // An unreadable pid file is the stop verb's own business to report in
    // full; for patience resolution it simply means the record cannot answer.
    let record = aion_server::control::pid_file::read(home).ok().flatten()?;
    match aion_server::control::incarnation::probe(&record) {
        IncarnationProbe::Verified { .. } => Some(record),
        IncarnationProbe::ProcessGone | IncarnationProbe::DifferentIncarnation { .. } => None,
    }
}

/// Where the wait patience came from, named in the verb's own output so the
/// operator can see which value governed the wait.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PatienceSource {
    /// `--patience` on this invocation.
    Flag,
    /// The VERIFIED-RUNNING incarnation's pid record — the drain window the
    /// server is ACTUALLY running with, which its own `--drain-timeout` or a
    /// config file edited since boot can have moved away from today's
    /// config. A record whose process is gone or recycled never reaches this
    /// source: a stale record's window governs nothing.
    Record,
    /// The config file's `drain.timeout_seconds`, when no verified-running
    /// record exists to ask (nothing is running, or the record is stale).
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

/// The wait patience: the flag when given, otherwise the VERIFIED-RUNNING
/// incarnation's own recorded drain window, otherwise the config's drain
/// timeout. The record outranks the config for the same reason `status`
/// probes the record's addresses: the running server's window may have come
/// from its own `--drain-timeout`, and a re-derived value would wait out the
/// wrong window — misreporting a healthy long drain as "still draining", or
/// converting it to a forced exit on a second invocation. The caller proves
/// the record's liveness ([`verified_running_record`]) precisely so a stale
/// record's window can never govern a wait. Loading the config for the last
/// fallback NEVER scaffolds one — a stop verb must not create server state.
fn resolve_patience(
    args: &StopArgs,
    running: Option<&aion_server::control::PidRecord>,
) -> Result<(Duration, PatienceSource), String> {
    if let Some(seconds) = args.patience {
        return Ok((Duration::from_secs(seconds.get()), PatienceSource::Flag));
    }
    // A zero window in a record would make the verb give up instantly. No
    // server writes one (the config validates the timeout non-zero), but a
    // record written before the field existed reads as 0 (`serde(default)`),
    // and both shapes honestly mean "no recorded window": fall to config.
    if let Some(record) = running
        && record.drain_timeout_seconds > 0
    {
        return Ok((
            Duration::from_secs(record.drain_timeout_seconds),
            PatienceSource::Record,
        ));
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
        // The carried account is the configuration's OWN error, bare — no
        // sentence frame, no remedy. Each surface that states this failure
        // supplies its own sentence exactly once: the CLI's note names the
        // verb and when the patience matters, and the stop flow's refusal
        // (the one place the failure actually blocks) wraps it in the
        // refusal sentence and names `--patience` as the remedy. A framed
        // account here would be printed inside a second frame there.
        Err(error) => Err(error.to_string()),
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
            render_fate_reading(record.pid, fate);
            render_reconciliation(record.pid, pid_file_reconciled, "the exiting server");
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
            render_fate_reading(record.pid, fate);
            render_reconciliation(record.pid, pid_file_reconciled, "the dead server");
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

/// Render the death note's account, or the fact that it could not be read.
///
/// The verb's doctrine: a bookkeeping layer's failure is stated beside the
/// action layer's fact, never allowed to stand for it. A note that cannot be
/// read is reported as exactly that — the server's exit account is unknown,
/// the stop itself is not in question.
fn render_fate_reading(pid: u32, fate: &Result<NoteFate, String>) {
    match fate {
        Ok(fate) => render_fate(pid, fate),
        Err(error) => println!(
            "the death note could not be read ({error}); the server's exit account is \
             unknown — the stop itself is not in question"
        ),
    }
}

/// Render the pid-file reconciliation's fact: reconciled, nothing to
/// reconcile, or failed with the file possibly still naming the dead pid.
fn render_reconciliation(pid: u32, reconciled: &Result<bool, String>, whose: &str) {
    match reconciled {
        Ok(true) => println!("the pid file {whose} left behind was reconciled away"),
        Ok(false) => {}
        Err(error) => println!(
            "the pid file could not be reconciled ({error}); if it still names pid {pid}, \
             remove it by hand"
        ),
    }
}

/// Render what the death note records for the stopped incarnation. Public to
/// the crate: `aion server status` renders the same account for a dead
/// recorded server.
pub(crate) fn render_fate(pid: u32, fate: &NoteFate) {
    for line in fate_lines(pid, fate) {
        println!("{line}");
    }
}

/// The fate's rendered lines, built pure so the mixed shapes are
/// unit-provable — in particular that a readable record and a torn line in
/// EITHER order both get stated (the renderers' rule, now testable instead
/// of inspected).
fn fate_lines(pid: u32, fate: &NoteFate) -> Vec<String> {
    let mut lines = Vec::new();
    match fate {
        NoteFate::NoNote => {
            lines.push(
                "no death note exists under this home; the exit left no recorded account"
                    .to_owned(),
            );
        }
        NoteFate::NoBracketForPid => {
            lines.push(format!(
                "the death note has no record for pid {pid}; the exit left no recorded \
                 account"
            ));
        }
        NoteFate::ArmedNotDisarmed {
            outcome,
            outcome_unreadable,
        } => {
            lines.push(format!(
                "the death note's bracket for pid {pid} never closed: the process was \
                 destroyed without its run loop seeing the end (the `kill -9` shape)"
            ));
            match (outcome, outcome_unreadable) {
                (Some(record), unreadable) => {
                    push_outcome_record(&mut lines, record);
                    // A readable record does not un-happen the torn line
                    // beside it: both facts are stated.
                    if let Some(unreadable) = unreadable {
                        push_unreadable_outcome(&mut lines, unreadable);
                    }
                }
                (None, Some(unreadable)) => push_unreadable_outcome(&mut lines, unreadable),
                (None, None) => lines.push(
                    "no drain outcome was recorded — the drain never got far enough to \
                     write one; in-flight work recovers from durable state on the next \
                     boot"
                        .to_owned(),
                ),
            }
        }
        NoteFate::Disarmed {
            outcome,
            outcome_unreadable,
            reason,
        } => {
            lines.push(format!("exit recorded: {reason}"));
            match (outcome, outcome_unreadable) {
                (Some(record), unreadable) => {
                    push_outcome_record(&mut lines, record);
                    if let Some(unreadable) = unreadable {
                        push_unreadable_outcome(&mut lines, unreadable);
                    }
                }
                (None, Some(unreadable)) => push_unreadable_outcome(&mut lines, unreadable),
                (None, None) => lines.push(
                    "no drain outcome record was written for this exit (an error return \
                     before serving, or a pre-record binary); reporting that absence, \
                     not a summary"
                        .to_owned(),
                ),
            }
        }
    }
    lines
}

/// The unreadable-presence face: a drain outcome record EXISTS but this
/// binary could not parse it. Reported as exactly that — never as "no
/// record", which would send the operator chasing a drain that in fact ran
/// far enough to write its receipt.
fn push_unreadable_outcome(lines: &mut Vec<String>, unreadable: &str) {
    lines.push(format!(
        "a drain outcome record was written but could not be read ({unreadable}); \
         a torn line from a mid-write kill, or a record from a build this binary \
         cannot parse"
    ));
}

fn push_outcome_record(lines: &mut Vec<String>, record: &OutcomeRecord) {
    lines.push(format!(
        "drain outcome: {:?} (window {}s, drain requests delivered to {} worker(s))",
        record.outcome, record.drain_timeout_seconds, record.delivered_drain_requests
    ));
    for parked in &record.parked {
        let queue = parked.queue.as_deref().unwrap_or("unknown queue");
        lines.push(format!(
            "  parked on worker {} ({queue}): {}",
            parked.worker,
            parked.tasks.join(", ")
        ));
    }
    for declared in &record.parked_declared_commands {
        lines.push(format!(
            "  declared command still executing at drain end: {declared} — its process \
             ended with the server; the next boot re-dispatches the attempt"
        ));
    }
    if !record.managed_workers_stopped.is_empty() {
        lines.push(format!(
            "  managed workers stopped: {}",
            record.managed_workers_stopped.join(", ")
        ));
    }
    for unstopped in &record.managed_workers_unstopped {
        lines.push(format!("  managed worker NOT proven stopped: {unstopped}"));
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
    use super::{PatienceSource, StopArgs, fate_lines, resolve_patience, verified_running_record};
    use aion_server::control::{NoteFate, OutcomeRecord, PidRecord};
    use aion_server::shutdown::ShutdownOutcome;
    use std::num::NonZeroU64;
    use std::time::Duration;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// A record for THIS process with its real incarnation — the only live
    /// incarnation a test can mint honestly — carrying a known drain window.
    fn live_record(drain_timeout_seconds: u64) -> Result<PidRecord, Box<dyn std::error::Error>> {
        let me = aion_server::control::incarnation::self_identity()?;
        Ok(PidRecord {
            pid: me.pid,
            started_at_unix_secs: me.started_at_unix_secs,
            binary_sha256: me.binary_sha256,
            version: "0.0.0-test".to_owned(),
            commit: "test".to_owned(),
            http_address: "127.0.0.1:8080".parse()?,
            grpc_address: "127.0.0.1:50051".parse()?,
            drain_timeout_seconds,
        })
    }

    /// Write `record` where the verb reads.
    fn write_record(home: &std::path::Path, record: &PidRecord) -> TestResult {
        let run_dir = home.join("run");
        std::fs::create_dir_all(&run_dir)?;
        std::fs::write(
            run_dir.join("aion-server.pid"),
            format!("{}\n", serde_json::to_string(record)?),
        )?;
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

    /// R2: the flag outranks everything, including a verified-live record.
    #[test]
    fn the_patience_flag_outranks_the_record_and_the_config() -> TestResult {
        let record = live_record(300)?;
        let args = StopArgs {
            config: None,
            patience: NonZeroU64::new(7),
        };
        let (patience, source) = resolve_patience(&args, Some(&record))
            .map_err(|message| -> Box<dyn std::error::Error> { message.into() })?;
        assert_eq!(patience, Duration::from_secs(7));
        assert_eq!(source, PatienceSource::Flag);
        Ok(())
    }

    /// Absent the flag, the VERIFIED-RUNNING incarnation's recorded window
    /// governs — proven against a record whose pid and start instant are this
    /// very process's, so [`verified_running_record`] genuinely verifies a
    /// live incarnation rather than trusting the file.
    #[test]
    fn an_absent_flag_reads_the_running_servers_recorded_window() -> TestResult {
        let home = tempfile::tempdir()?;
        write_record(home.path(), &live_record(300)?)?;
        let running = verified_running_record(home.path())
            .ok_or("a record naming THIS live process must verify as running")?;
        let config = write_config(home.path(), 30)?;
        let args = StopArgs {
            config: Some(config),
            patience: None,
        };
        let (patience, source) = resolve_patience(&args, Some(&running))
            .map_err(|message| -> Box<dyn std::error::Error> { message.into() })?;
        assert_eq!(
            patience,
            Duration::from_secs(300),
            "the record's window must outrank the config's"
        );
        assert_eq!(source, PatienceSource::Record);
        Ok(())
    }

    /// Red-first for the stale-record hole: a record whose process is GONE
    /// must not verify as running, so its window governs nothing and the
    /// config rules the wait.
    #[test]
    fn a_dead_records_window_never_governs_the_wait() -> TestResult {
        let home = tempfile::tempdir()?;
        // A pid proven vacated: a reaped child, with a start instant (zero)
        // no live process reports, so the probe cannot accidentally verify.
        let mut child = std::process::Command::new("true").spawn()?;
        let dead_pid = child.id();
        child.wait()?;
        let mut record = live_record(300)?;
        record.pid = dead_pid;
        record.started_at_unix_secs = 0;
        write_record(home.path(), &record)?;

        assert_eq!(
            verified_running_record(home.path()),
            None,
            "a dead record must not read as a running server"
        );
        let config = write_config(home.path(), 45)?;
        let args = StopArgs {
            config: Some(config),
            patience: None,
        };
        let (patience, source) = resolve_patience(&args, None)
            .map_err(|message| -> Box<dyn std::error::Error> { message.into() })?;
        assert_eq!(
            patience,
            Duration::from_secs(45),
            "the config must govern when the record is stale"
        );
        assert_eq!(source, PatienceSource::Config);
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
        assert_eq!(
            verified_running_record(home.path()),
            None,
            "an empty home has no running record"
        );
        let (patience, source) = resolve_patience(&args, None)
            .map_err(|message| -> Box<dyn std::error::Error> { message.into() })?;
        assert_eq!(patience, Duration::from_secs(45));
        assert_eq!(source, PatienceSource::Config);
        Ok(())
    }

    /// The mixed fate shapes state BOTH facts — the renderers' rule ("a
    /// readable record does not un-happen the torn line beside it"), proven
    /// on the rendered lines instead of by inspection. Covers both bracket
    /// kinds, since each has its own match.
    #[test]
    fn a_readable_record_and_a_torn_line_are_both_rendered() {
        let record = OutcomeRecord {
            pid: 600,
            outcome: ShutdownOutcome::Clean,
            drain_timeout_seconds: 30,
            delivered_drain_requests: 1,
            parked: Vec::new(),
            parked_declared_commands: Vec::new(),
            managed_workers_stopped: Vec::new(),
            managed_workers_unstopped: Vec::new(),
        };
        for fate in [
            NoteFate::ArmedNotDisarmed {
                outcome: Some(record.clone()),
                outcome_unreadable: Some("an OUTCOME line does not parse: torn".to_owned()),
            },
            NoteFate::Disarmed {
                outcome: Some(record.clone()),
                outcome_unreadable: Some("an OUTCOME line does not parse: torn".to_owned()),
                reason: "clean run-loop exit".to_owned(),
            },
        ] {
            let rendered = fate_lines(600, &fate).join("\n");
            assert!(
                rendered.contains("drain outcome: Clean"),
                "the readable record must be stated: {rendered}"
            );
            assert!(
                rendered.contains("could not be read"),
                "the torn line must be stated BESIDE the record: {rendered}"
            );
        }
    }

    /// The absence faces stay distinct from each other and from the torn
    /// presence — an operator must never chase a drain that wrote nothing,
    /// or dismiss one that wrote a record this binary cannot parse.
    #[test]
    fn fate_absence_faces_are_distinct() {
        let no_note = fate_lines(600, &NoteFate::NoNote).join("\n");
        assert!(no_note.contains("no death note exists"));

        let torn_only = fate_lines(
            600,
            &NoteFate::ArmedNotDisarmed {
                outcome: None,
                outcome_unreadable: Some("an OUTCOME line does not parse: torn".to_owned()),
            },
        )
        .join("\n");
        assert!(
            torn_only.contains("could not be read") && !torn_only.contains("never got far enough"),
            "an unreadable presence must never read as an honest absence: {torn_only}"
        );
    }
}
