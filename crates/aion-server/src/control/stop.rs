//! The stop flow behind `aion server stop`: resolve the pid file, verify the
//! incarnation, signal, wait bounded, and read the drain outcome back from
//! the death note.
//!
//! The verb signals ONLY a process the record proves is ours — the estate's
//! kill discipline, mechanized. Wrong-incarnation is a refusal that names
//! both incarnations and touches nothing; a dead process with a lingering
//! file is reconciled and reported; a server that exits without writing an
//! outcome record has that absence reported honestly, never papered over
//! with a fabricated summary.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::incarnation::{self, IncarnationProbe};
use super::outcome::{self, NoteFate};
use super::pid_file::{self, PidRecord};
use crate::error::ServerError;

/// How the stop ended when it was allowed to act.
#[derive(Clone, Debug)]
pub enum StopOutcome {
    /// The server was signalled and exited within the patience window. The
    /// death note's account of the exit rides along.
    Stopped {
        /// The incarnation that was stopped.
        record: PidRecord,
        /// What the death note records for it (the drain outcome lives
        /// here; its absence is a state, not an error).
        fate: NoteFate,
        /// How long the wait took.
        waited: Duration,
        /// Whether a pid file the dying server left behind was reconciled
        /// away (a clean exit removes its own; a forced one cannot).
        pid_file_reconciled: bool,
    },
    /// The recorded process was already gone: nothing to signal. The file
    /// is reconciled away and the note consulted for what happened.
    AlreadyGone {
        /// The incarnation the file named.
        record: PidRecord,
        /// The death note's account — `ArmedNotDisarmed` with no record
        /// here is the `kill -9` shape, reported as exactly that.
        fate: NoteFate,
        /// Whether the stale file was removed.
        pid_file_reconciled: bool,
    },
    /// The server is still draining when the patience elapsed. Not a
    /// failure and not a guess: the server owns its drain, and a second
    /// `stop` is the operator's escalation (the server treats a second
    /// termination signal as "force immediate exit").
    StillDraining {
        /// The incarnation still running.
        record: PidRecord,
        /// How long the verb waited before reporting.
        waited: Duration,
    },
}

/// Why the stop refused to act. Each face is distinct and names its remedy.
#[derive(Clone, Debug, thiserror::Error)]
pub enum StopRefusal {
    /// No pid file exists under the home: no server has claimed it (or the
    /// running server predates the pid-file era).
    #[error(
        "no pid file at `{path}`: no server has claimed this home. If a server is \
         running, it predates `aion server stop` — find it with `ps` and signal it \
         by hand this one time; its next start writes the pid file"
    )]
    NoPidFile {
        /// The path that held no file.
        path: PathBuf,
    },
    /// The file names a live pid whose start instant is not the recorded
    /// one: the pid was recycled. Nothing is signalled.
    #[error(
        "refusing to signal pid {pid}: the pid file records a server started at unix \
         second {recorded_started_at}, but the live process wearing that pid started \
         at {live_started_at}{live_exe} — the pid was recycled onto a different \
         process after the recorded server died. Nothing was signalled and nothing \
         was removed; verify with `ps -p {pid}` and remove `{path}` once satisfied"
    )]
    StaleIncarnation {
        /// The recycled pid.
        pid: u32,
        /// Start instant the file records.
        recorded_started_at: u64,
        /// Start instant the live process wears.
        live_started_at: u64,
        /// Rendered ` (running <exe>)` fragment when the table knows it.
        live_exe: String,
        /// The pid file's path, named for the operator's own reconciliation.
        path: PathBuf,
    },
    /// This build cannot send POSIX signals.
    #[error("`aion server stop` needs POSIX signals, which this platform does not have")]
    UnsupportedPlatform,
    /// The signal could not be sent to a verified-live process.
    #[error("could not signal pid {pid}: {message}")]
    SignalFailed {
        /// The verified pid the signal was aimed at.
        pid: u32,
        /// The OS error.
        message: String,
    },
}

/// The verb's answer: it acted (an outcome) or it refused (a refusal).
#[derive(Clone, Debug)]
pub enum StopVerdict {
    /// The verb acted; here is what happened. Boxed so the verdict's two
    /// arms stay close in size — the outcome carries the whole record and
    /// the note's account.
    Outcome(Box<StopOutcome>),
    /// The verb refused to act; here is the exact face.
    Refusal(StopRefusal),
}

impl StopVerdict {
    /// The outcome, unboxed, when the verb acted; `None` on a refusal.
    #[must_use]
    pub fn into_outcome(self) -> Option<StopOutcome> {
        match self {
            Self::Outcome(outcome) => Some(*outcome),
            Self::Refusal(_) => None,
        }
    }
}

/// Stop the server recorded under `home`, waiting up to `patience` for it to
/// exit. `patience` is operator-ruled at invocation — the CLI passes the
/// `--patience` flag or the config's own `drain_timeout`; this function
/// never invents a value.
///
/// # Errors
///
/// Returns [`ServerError`] only for I/O failures reading the pid file or the
/// death note — every decision the verb itself makes is a typed
/// [`StopVerdict`], never an anonymous error.
pub fn stop(home: &Path, patience: Duration) -> Result<StopVerdict, ServerError> {
    let Some(record) = pid_file::read(home)? else {
        return Ok(StopVerdict::Refusal(StopRefusal::NoPidFile {
            path: pid_file::pid_file_path(home),
        }));
    };
    match incarnation::probe(&record) {
        IncarnationProbe::ProcessGone => {
            let fate = outcome::read_fate(home, record.pid)?;
            let pid_file_reconciled = pid_file::remove_if_matches(home, &record)?;
            Ok(StopVerdict::Outcome(Box::new(StopOutcome::AlreadyGone {
                record,
                fate,
                pid_file_reconciled,
            })))
        }
        IncarnationProbe::DifferentIncarnation {
            live_started_at_unix_secs,
            exe,
        } => Ok(StopVerdict::Refusal(StopRefusal::StaleIncarnation {
            pid: record.pid,
            recorded_started_at: record.started_at_unix_secs,
            live_started_at: live_started_at_unix_secs,
            live_exe: exe.map_or_else(String::new, |path| {
                format!(" (running `{}`)", path.display())
            }),
            path: pid_file::pid_file_path(home),
        })),
        IncarnationProbe::Verified { .. } => signal_and_wait(home, record, patience),
    }
}

/// SIGTERM the verified incarnation and wait for it to leave the process
/// table, then read the death note's account.
#[cfg(unix)]
fn signal_and_wait(
    home: &Path,
    record: PidRecord,
    patience: Duration,
) -> Result<StopVerdict, ServerError> {
    let Ok(pid_i32) = i32::try_from(record.pid) else {
        return Ok(StopVerdict::Refusal(StopRefusal::SignalFailed {
            pid: record.pid,
            message: "pid does not fit a signal target".to_owned(),
        }));
    };
    let target = nix::unistd::Pid::from_raw(pid_i32);
    if let Err(errno) = nix::sys::signal::kill(target, nix::sys::signal::Signal::SIGTERM) {
        // ESRCH here means the process exited between the probe and the
        // signal — the AlreadyGone face, one instant later.
        if errno == nix::errno::Errno::ESRCH {
            let fate = outcome::read_fate(home, record.pid)?;
            let pid_file_reconciled = pid_file::remove_if_matches(home, &record)?;
            return Ok(StopVerdict::Outcome(Box::new(StopOutcome::AlreadyGone {
                record,
                fate,
                pid_file_reconciled,
            })));
        }
        return Ok(StopVerdict::Refusal(StopRefusal::SignalFailed {
            pid: record.pid,
            message: errno.to_string(),
        }));
    }
    let started = Instant::now();
    let cadence = wait_cadence(patience);
    loop {
        match incarnation::probe(&record) {
            IncarnationProbe::Verified { .. } => {
                let waited = started.elapsed();
                if waited >= patience {
                    return Ok(StopVerdict::Outcome(Box::new(StopOutcome::StillDraining {
                        record,
                        waited,
                    })));
                }
                std::thread::sleep(cadence.min(patience.saturating_sub(waited)));
            }
            // Gone, or the pid already recycled onto something else —
            // either way OUR incarnation exited.
            IncarnationProbe::ProcessGone | IncarnationProbe::DifferentIncarnation { .. } => {
                let fate = outcome::read_fate(home, record.pid)?;
                let pid_file_reconciled = pid_file::remove_if_matches(home, &record)?;
                return Ok(StopVerdict::Outcome(Box::new(StopOutcome::Stopped {
                    record,
                    fate,
                    waited: started.elapsed(),
                    pid_file_reconciled,
                })));
            }
        }
    }
}

#[cfg(not(unix))]
fn signal_and_wait(
    _home: &Path,
    _record: PidRecord,
    _patience: Duration,
) -> Result<StopVerdict, ServerError> {
    Ok(StopVerdict::Refusal(StopRefusal::UnsupportedPlatform))
}

/// The wait's poll cadence, derived from the patience rather than invented:
/// one two-hundredth of the window, clamped to [25ms, 250ms] — the same
/// derive-from-the-governing-span discipline as the heartbeat sweeper's
/// quarter-window cadence. This is detection latency, not policy; the
/// operator's ruling is the patience itself.
fn wait_cadence(patience: Duration) -> Duration {
    (patience / 200).clamp(Duration::from_millis(25), Duration::from_millis(250))
}

#[cfg(test)]
#[path = "stop_tests.rs"]
mod tests;
