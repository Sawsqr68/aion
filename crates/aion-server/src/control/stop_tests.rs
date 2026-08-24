//! Red-first specimens for the stop verb's unit-testable faces: the refusals
//! and the already-gone reconciliation. The signal-and-wait faces (Stopped,
//! `StillDraining`) act on a live server and are proven by the e2e specimens.

use std::path::Path;
use std::time::Duration;

use super::{StopOutcome, StopRefusal, StopVerdict, stop, wait_cadence};
use crate::control::incarnation;
use crate::control::outcome::{NoteFate, OutcomeRecord, render_entry};
use crate::control::pid_file::{self, PidRecord, claim};
use crate::shutdown::ShutdownOutcome;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A record for THIS process with its real incarnation — the only live
/// incarnation a test can mint honestly.
fn own_record() -> Result<PidRecord, Box<dyn std::error::Error>> {
    let me = incarnation::self_identity()?;
    Ok(PidRecord {
        pid: me.pid,
        started_at_unix_secs: me.started_at_unix_secs,
        binary_sha256: me.binary_sha256,
        version: env!("CARGO_PKG_VERSION").to_owned(),
        commit: "test-commit".to_owned(),
        http_address: "127.0.0.1:8080".parse()?,
        grpc_address: "127.0.0.1:50051".parse()?,
        drain_timeout_seconds: 30,
    })
}

/// A record whose pid is proven vacated: a reaped child.
fn dead_record() -> Result<PidRecord, Box<dyn std::error::Error>> {
    let mut child = std::process::Command::new("true").spawn()?;
    let pid = child.id();
    child.wait()?;
    let mut record = own_record()?;
    record.pid = pid;
    // A recycled pid would wear a real start instant; zero is a value no
    // live process reports, so the probe cannot accidentally verify.
    record.started_at_unix_secs = 0;
    Ok(record)
}

/// Leave `record` sitting in `home`'s pid file, as a crashed server would.
fn strand_record(home: &Path, record: &PidRecord) -> Result<(), Box<dyn std::error::Error>> {
    let guard = claim(home, record)?;
    std::mem::forget(guard);
    Ok(())
}

fn write_note(home: &Path, lines: &[String]) -> std::io::Result<()> {
    let logs = home.join("logs");
    std::fs::create_dir_all(&logs)?;
    std::fs::write(logs.join("aion-server.death.log"), lines.join("\n") + "\n")
}

fn stamped(body: &str) -> String {
    format!("2026-08-24T08:00:00+00:00 {body}")
}

/// An empty home refuses with the no-pid-file face, naming the path the
/// operator would look at.
#[test]
fn an_unclaimed_home_refuses_with_no_pid_file() -> TestResult {
    let home = tempfile::tempdir()?;
    match stop(home.path(), Duration::from_millis(50))? {
        StopVerdict::Refusal(StopRefusal::NoPidFile { path }) => {
            assert_eq!(
                path,
                pid_file::pid_file_path(home.path()),
                "the refusal must name the path it looked at"
            );
        }
        other => return Err(format!("expected the NoPidFile refusal, got {other:?}").into()),
    }
    Ok(())
}

/// Red first — the recorded process is gone: the verb signals nothing,
/// reconciles the stale file away, and reports the note's honest absence.
///
/// The reaped pid COULD have been recycled by the kernel between
/// `dead_record`'s wait and the verb's probe; the verb then honestly
/// refuses on a stale incarnation instead. Both answers are honest — this
/// specimen refuses only a stop that pretends the dead record is running
/// (the same guard its siblings in `incarnation.rs` and `server_stop.rs`
/// carry, and a false red is not a safe failure).
#[test]
fn a_dead_servers_file_reconciles_as_already_gone() -> TestResult {
    let home = tempfile::tempdir()?;
    let stale = dead_record()?;
    strand_record(home.path(), &stale)?;

    match stop(home.path(), Duration::from_millis(50))? {
        StopVerdict::Outcome(outcome) => match *outcome {
            StopOutcome::AlreadyGone {
                record,
                fate,
                pid_file_reconciled,
            } => {
                assert_eq!(
                    record, stale,
                    "the outcome must name the recorded incarnation"
                );
                assert_eq!(
                    fate,
                    Ok(NoteFate::NoNote),
                    "an empty home's note absence is a state, not a guess"
                );
                assert_eq!(
                    pid_file_reconciled,
                    Ok(true),
                    "the stale file must be reconciled away"
                );
                assert_eq!(
                    pid_file::read(home.path())?,
                    None,
                    "the stale file must be gone after reconciliation"
                );
            }
            other => return Err(format!("expected AlreadyGone, got {other:?}").into()),
        },
        StopVerdict::Refusal(StopRefusal::StaleIncarnation { pid, .. }) => {
            // The kernel re-handed the reaped pid to a stranger before the
            // probe ran. The refusal is the honest answer for that world,
            // and it must leave the stranger's claim alone.
            assert_eq!(pid, stale.pid, "the refusal must name the recorded pid");
            assert_eq!(
                pid_file::read(home.path())?,
                Some(stale),
                "a refusal must leave the file untouched"
            );
        }
        other @ StopVerdict::Refusal(_) => {
            return Err(format!("expected AlreadyGone or StaleIncarnation, got {other:?}").into());
        }
    }
    Ok(())
}

/// `AlreadyGone` carries the death note's account: a disarmed bracket with an
/// outcome record is read back whole through the stop verb.
#[test]
fn already_gone_carries_the_death_notes_account() -> TestResult {
    let home = tempfile::tempdir()?;
    let stale = dead_record()?;
    let outcome_record = OutcomeRecord {
        pid: stale.pid,
        outcome: ShutdownOutcome::Clean,
        drain_timeout_seconds: 30,
        delivered_drain_requests: 1,
        parked: Vec::new(),
        parked_declared_commands: Vec::new(),
        managed_workers_stopped: Vec::new(),
        managed_workers_unstopped: Vec::new(),
    };
    write_note(
        home.path(),
        &[
            stamped(&format!("ARMED pid={} version=test build=test", stale.pid)),
            stamped(&render_entry(&outcome_record)?),
            stamped("DISARMED clean run-loop exit: shutdown outcome Clean"),
        ],
    )?;
    strand_record(home.path(), &stale)?;

    match stop(home.path(), Duration::from_millis(50))? {
        StopVerdict::Outcome(outcome) => match *outcome {
            StopOutcome::AlreadyGone { fate, .. } => match fate {
                Ok(NoteFate::Disarmed { outcome, .. }) => {
                    assert_eq!(
                        outcome.as_ref(),
                        Some(&outcome_record),
                        "the drain outcome must ride the stop verdict"
                    );
                }
                other => return Err(format!("expected the Disarmed fate, got {other:?}").into()),
            },
            other => return Err(format!("expected AlreadyGone, got {other:?}").into()),
        },
        // The reaped pid recycled onto a stranger before the probe — the
        // honest refusal for that world; the note assertion is unreachable
        // there and the specimen refuses only a false Verified.
        StopVerdict::Refusal(StopRefusal::StaleIncarnation { pid, .. }) => {
            assert_eq!(pid, stale.pid, "the refusal must name the recorded pid");
        }
        other @ StopVerdict::Refusal(_) => {
            return Err(format!("expected AlreadyGone or StaleIncarnation, got {other:?}").into());
        }
    }
    Ok(())
}

/// Red first — the reviewer's mixed-ownership shape: the server is proven
/// gone, but BOTH bookkeeping layers fail (the death note is unreadable and
/// the pid file cannot be reconciled). The verb must still report the goal
/// state — `AlreadyGone` with each failure carried as its own layer's fact —
/// never a verb error that hides "the server is down" behind a bookkeeping
/// `?`. Before this specimen, a deploy script keying on exit 2 escalated
/// against a server that was already stopped.
#[test]
fn bookkeeping_failures_ride_the_outcome_instead_of_destroying_it() -> TestResult {
    let home = tempfile::tempdir()?;
    let stale = dead_record()?;
    strand_record(home.path(), &stale)?;

    // The death note's path is occupied by a DIRECTORY: reading it fails
    // with a real I/O error, not the honest-absence NotFound arm.
    std::fs::create_dir_all(crate::death_note::note_path(home.path()))?;
    // The run directory is read-only: the pid file can be read but not
    // removed — the root-owned-home shape at unit scale.
    let run_dir = pid_file::pid_file_path(home.path())
        .parent()
        .ok_or("pid file path has no parent")?
        .to_path_buf();
    let writable = std::fs::metadata(&run_dir)?.permissions();
    let mut read_only = writable.clone();
    read_only.set_readonly(true);
    std::fs::set_permissions(&run_dir, read_only)?;

    let verdict = stop(home.path(), Duration::from_millis(50));
    // Restore before asserting so a red never strands a read-only tempdir.
    std::fs::set_permissions(&run_dir, writable)?;

    match verdict? {
        StopVerdict::Outcome(outcome) => match *outcome {
            StopOutcome::AlreadyGone {
                record,
                fate,
                pid_file_reconciled,
            } => {
                assert_eq!(record, stale, "the goal-state fact must name the record");
                let Err(fate_error) = fate else {
                    return Err("a directory at the note path must be unreadable".into());
                };
                assert!(
                    fate_error.contains("death note"),
                    "the fate failure must name the note layer: {fate_error}"
                );
                let Err(reconcile_error) = pid_file_reconciled else {
                    return Err("a read-only run dir must fail reconciliation".into());
                };
                assert!(
                    !reconcile_error.is_empty(),
                    "the reconciliation failure must carry its reason"
                );
                assert_eq!(
                    pid_file::read(home.path())?,
                    Some(stale),
                    "the file the verb could not remove must still be there — \
                     the Err is a fact, not a shrug"
                );
            }
            other => return Err(format!("expected AlreadyGone, got {other:?}").into()),
        },
        StopVerdict::Refusal(StopRefusal::StaleIncarnation { pid, .. }) => {
            // The reaped pid recycled onto a stranger — honest, rare, and
            // outside this specimen's subject.
            assert_eq!(pid, stale.pid);
        }
        other @ StopVerdict::Refusal(_) => {
            return Err(format!("expected AlreadyGone, got {other:?}").into());
        }
    }
    Ok(())
}

/// Red first — a live pid wearing a foreign start instant is the recycled-pid
/// face: the verb refuses, names both incarnations, and touches NOTHING.
#[test]
fn a_recycled_pid_refuses_and_touches_nothing() -> TestResult {
    let home = tempfile::tempdir()?;
    let mut stale = own_record()?;
    stale.started_at_unix_secs = stale.started_at_unix_secs.wrapping_add(31);
    strand_record(home.path(), &stale)?;

    match stop(home.path(), Duration::from_millis(50))? {
        StopVerdict::Refusal(StopRefusal::StaleIncarnation {
            pid,
            recorded_started_at,
            live_started_at,
            path,
            ..
        }) => {
            assert_eq!(pid, stale.pid, "the refusal must name the recycled pid");
            assert_eq!(
                recorded_started_at, stale.started_at_unix_secs,
                "the refusal must name the recorded start instant"
            );
            assert_ne!(
                live_started_at, recorded_started_at,
                "the refusal must carry the LIVE instant that contradicts the record"
            );
            assert_eq!(
                path,
                pid_file::pid_file_path(home.path()),
                "the refusal must name the file for the operator's own reconciliation"
            );
        }
        other => {
            return Err(format!("expected the StaleIncarnation refusal, got {other:?}").into());
        }
    }
    assert_eq!(
        pid_file::read(home.path())?.as_ref(),
        Some(&stale),
        "a refusal must leave the file exactly as it found it"
    );
    Ok(())
}

/// A corrupt pid file surfaces as the typed read error — the verb never
/// converts an unreadable record into a verdict.
#[test]
fn a_corrupt_pid_file_is_a_typed_error_not_a_verdict() -> TestResult {
    let home = tempfile::tempdir()?;
    let path = pid_file::pid_file_path(home.path());
    std::fs::create_dir_all(path.parent().ok_or("pid path must have a parent")?)?;
    std::fs::write(&path, "93400\n")?;
    match stop(home.path(), Duration::from_millis(50)) {
        Err(error) => {
            assert!(
                error.to_string().contains("does not parse"),
                "the error must say why the file refused: {error}"
            );
        }
        Ok(verdict) => {
            return Err(format!("a corrupt file must refuse as an error, got {verdict:?}").into());
        }
    }
    Ok(())
}

/// The wait cadence derives from the patience — one two-hundredth of the
/// window — and clamps to [25ms, 250ms] at the extremes.
#[test]
fn wait_cadence_derives_from_the_patience_and_clamps() {
    assert_eq!(
        wait_cadence(Duration::from_secs(1)),
        Duration::from_millis(25),
        "a short patience clamps up to the 25ms floor"
    );
    assert_eq!(
        wait_cadence(Duration::from_secs(10)),
        Duration::from_millis(50),
        "an in-range patience divides by 200"
    );
    assert_eq!(
        wait_cadence(Duration::from_secs(600)),
        Duration::from_millis(250),
        "a long patience clamps down to the 250ms ceiling"
    );
}
