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
#[test]
fn a_dead_servers_file_reconciles_as_already_gone() -> TestResult {
    let home = tempfile::tempdir()?;
    let stale = dead_record()?;
    strand_record(home.path(), &stale)?;

    match stop(home.path(), Duration::from_millis(50))?.into_outcome() {
        Some(StopOutcome::AlreadyGone {
            record,
            fate,
            pid_file_reconciled,
        }) => {
            assert_eq!(
                record, stale,
                "the outcome must name the recorded incarnation"
            );
            assert_eq!(
                fate,
                NoteFate::NoNote,
                "an empty home's note absence is a state, not a guess"
            );
            assert!(
                pid_file_reconciled,
                "the stale file must be reconciled away"
            );
        }
        other => return Err(format!("expected AlreadyGone, got {other:?}").into()),
    }
    assert_eq!(
        pid_file::read(home.path())?,
        None,
        "the stale file must be gone after reconciliation"
    );
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

    match stop(home.path(), Duration::from_millis(50))?.into_outcome() {
        Some(StopOutcome::AlreadyGone { fate, .. }) => match fate {
            NoteFate::Disarmed { outcome, .. } => {
                assert_eq!(
                    outcome.as_ref(),
                    Some(&outcome_record),
                    "the drain outcome must ride the stop verdict"
                );
            }
            other => return Err(format!("expected the Disarmed fate, got {other:?}").into()),
        },
        other => return Err(format!("expected AlreadyGone, got {other:?}").into()),
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
