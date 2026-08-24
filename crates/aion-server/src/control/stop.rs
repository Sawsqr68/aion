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
//!
//! One window the proof cannot close: between the incarnation probe and the
//! SIGTERM, the verified process can exit and the kernel can hand its number
//! to a stranger. The verb handles the exit (ESRCH reconciles as
//! already-gone) but a full pid-space wraparound inside those microseconds
//! would land the signal on the recycled pid. Closing it needs a
//! process-handle primitive (`pidfd_open`/`pidfd_send_signal`, Linux-only);
//! until a piece wants that, the window is named here rather than claimed
//! away.

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
        /// here; its absence is a state, not an error), or why the note
        /// could not be read — a bookkeeping failure beside a server that
        /// is PROVEN exited, carried as its own fact, never allowed to
        /// destroy this outcome (see [`exit_bookkeeping`]).
        fate: Result<NoteFate, String>,
        /// How long the wait took.
        waited: Duration,
        /// Whether a pid file the dying server left behind was reconciled
        /// away (a clean exit removes its own; a forced one cannot), or
        /// why reconciliation failed — `Err` means the file may still name
        /// the dead pid and wants a manual look.
        pid_file_reconciled: Result<bool, String>,
    },
    /// The recorded process was already gone: nothing to signal. The file
    /// is reconciled away and the note consulted for what happened.
    AlreadyGone {
        /// The incarnation the file named.
        record: PidRecord,
        /// The death note's account — `ArmedNotDisarmed` with no record
        /// here is the `kill -9` shape, reported as exactly that — or why
        /// the note could not be read (carried, never propagated: the
        /// goal state already holds).
        fate: Result<NoteFate, String>,
        /// Whether the stale file was removed, or why removing it failed.
        pid_file_reconciled: Result<bool, String>,
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
    /// The server is verified RUNNING but no wait patience could be resolved
    /// to govern the stop — no `--patience`, no usable recorded drain window,
    /// and the configuration could not answer. Nothing is signalled:
    /// signalling without a governed wait would leave the verb unable to
    /// report what became of the drain.
    #[error(
        "refusing to signal pid {pid}: the server is RUNNING, but no wait patience \
         could be resolved to govern the stop ({unresolved}). Pass `--patience \
         <seconds>` to rule it at invocation, or repair the configuration"
    )]
    UnresolvedPatience {
        /// The verified-running pid that was NOT signalled.
        pid: u32,
        /// Why patience resolution failed — the configuration's own account.
        unresolved: String,
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

/// Stop the server recorded under `home`, waiting up to `patience` for it to
/// exit. `patience` is operator-ruled at invocation — the CLI passes the
/// `--patience` flag, the verified-running record's drain window, or the
/// config's own `drain_timeout`; this function never invents a value.
///
/// `patience` arrives as a `Result` because its resolution can fail (a config
/// that cannot load) and that failure must be carried to the point of need
/// rather than destroy the verb's answer: only a VERIFIED-RUNNING server
/// needs a wait window, so only the `Verified` arm consults the `Err` — and
/// refuses, naming the running pid and the remedy. Every other face
/// (no pid file, already gone, stale incarnation) decides without a window,
/// exactly as it would with one.
///
/// # Errors
///
/// Returns [`ServerError`] only for an I/O failure reading the pid file —
/// before any action is decided. Every decision the verb itself makes is a
/// typed [`StopVerdict`], and once the server is proven exited, bookkeeping
/// failures (death note, pid-file reconciliation) ride INSIDE the outcome
/// rather than erroring out of it (see [`exit_bookkeeping`]).
pub fn stop(home: &Path, patience: Result<Duration, String>) -> Result<StopVerdict, ServerError> {
    let Some(record) = pid_file::read(home)? else {
        return Ok(StopVerdict::Refusal(StopRefusal::NoPidFile {
            path: pid_file::pid_file_path(home),
        }));
    };
    match incarnation::probe(&record) {
        IncarnationProbe::ProcessGone => {
            let (fate, pid_file_reconciled) = exit_bookkeeping(home, &record);
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
        IncarnationProbe::Verified { .. } => match patience {
            Ok(patience) => signal_and_wait(home, record, patience),
            // The one arm that actually needs the window. Refusing HERE —
            // after the probe, before the signal — keeps both truths: a home
            // where the goal state already holds never sees this refusal,
            // and a running server is never signalled without a governed
            // wait to report on.
            Err(unresolved) => Ok(StopVerdict::Refusal(StopRefusal::UnresolvedPatience {
                pid: record.pid,
                unresolved,
            })),
        },
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
            let (fate, pid_file_reconciled) = exit_bookkeeping(home, &record);
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
                let (fate, pid_file_reconciled) = exit_bookkeeping(home, &record);
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

/// The exit-time bookkeeping beside a proven-exited server: the death
/// note's account and the pid-file reconciliation.
///
/// Both sit AFTER the action layer's fact is established (the process left
/// the table, or the signal found nobody), and either can fail on its own —
/// a root-owned pid file under a mixed-ownership home, an unreadable note.
/// Propagating those failures out of the verb destroyed the outcome: a stop
/// that genuinely stopped the server reported "the verb could not complete"
/// (exit 2) with no line saying the server is down, and a deploy script
/// keying on that code escalated against a server that was already stopped.
/// So each failure is carried in the outcome as its own layer's fact and
/// rendered beside the action's — never allowed to stand for it.
fn exit_bookkeeping(
    home: &Path,
    record: &PidRecord,
) -> (Result<NoteFate, String>, Result<bool, String>) {
    let fate = outcome::read_fate(home, record.pid).map_err(|error| error.to_string());
    let pid_file_reconciled =
        pid_file::remove_if_matches(home, record).map_err(|error| error.to_string());
    (fate, pid_file_reconciled)
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
