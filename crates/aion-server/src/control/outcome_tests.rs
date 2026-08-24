//! Red-first specimens for the outcome record and the death-note reader.

use std::path::Path;

use super::{NoteFate, OutcomeRecord, ParkedWorkerRecord, read_fate, render_entry};
use crate::shutdown::ShutdownOutcome;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn parked_record(pid: u32) -> OutcomeRecord {
    OutcomeRecord {
        pid,
        outcome: ShutdownOutcome::Parked,
        drain_timeout_seconds: 30,
        delivered_drain_requests: 2,
        parked_declared_commands: vec![
            "11111111-1111-1111-1111-111111111111/activity:0#1".to_owned(),
        ],
        parked: vec![ParkedWorkerRecord {
            worker: "worker-7".to_owned(),
            queue: Some("fleet_dev".to_owned()),
            tasks: vec!["wf-1/act-3#1".to_owned(), "wf-2/act-1#4".to_owned()],
        }],
        managed_workers_stopped: vec!["managed-a".to_owned()],
        managed_workers_unstopped: Vec::new(),
    }
}

fn write_note(home: &Path, lines: &[String]) -> std::io::Result<()> {
    let logs = home.join("logs");
    std::fs::create_dir_all(&logs)?;
    std::fs::write(logs.join("aion-server.death.log"), lines.join("\n") + "\n")
}

fn stamped(body: &str) -> String {
    format!("2026-08-24T08:00:00+00:00 {body}")
}

/// The record survives its trip through the note entry byte-exactly.
#[test]
fn outcome_record_roundtrips_through_its_entry() -> TestResult {
    let record = parked_record(4242);
    let entry = render_entry(&record)?;
    let json = entry
        .strip_prefix("OUTCOME ")
        .ok_or("entry must carry the OUTCOME kind")?;
    let read_back: OutcomeRecord = serde_json::from_str(json)?;
    assert_eq!(read_back, record, "the record must roundtrip unchanged");
    Ok(())
}

/// A record written before the declared-command census existed still reads:
/// the death note is shared with older builds, and their records carry no
/// `parked_declared_commands` field. The honest reading of that absence is
/// "none named" — never a parse refusal that erases the rest of the record.
#[test]
fn a_record_without_the_declared_field_reads_as_none_named() -> TestResult {
    let json = r#"{"pid":7,"outcome":"Clean","drain_timeout_seconds":30,"delivered_drain_requests":0,"parked":[],"managed_workers_stopped":[],"managed_workers_unstopped":[]}"#;
    let record: OutcomeRecord = serde_json::from_str(json)?;
    assert_eq!(record.pid, 7);
    assert!(
        record.parked_declared_commands.is_empty(),
        "an absent field must read as an empty census, not an error"
    );
    Ok(())
}

/// The ordinary shutdown shape: ARMED, OUTCOME, DISARMED — read back as
/// Disarmed with the record and the reason.
#[test]
fn a_disarmed_bracket_with_a_record_reads_whole() -> TestResult {
    let home = tempfile::tempdir()?;
    let record = parked_record(100);
    write_note(
        home.path(),
        &[
            stamped("ARMED pid=100 version=0.23.0 build=test"),
            stamped(&render_entry(&record)?),
            stamped("DISARMED clean run-loop exit: shutdown outcome Parked"),
        ],
    )?;
    match read_fate(home.path(), 100)? {
        NoteFate::Disarmed {
            outcome,
            outcome_unreadable,
            reason,
        } => {
            assert_eq!(
                outcome.as_ref(),
                Some(&record),
                "the record must be read back"
            );
            assert_eq!(outcome_unreadable, None, "a read-back record is readable");
            assert!(
                reason.contains("Parked"),
                "the DISARMED reason must be carried: {reason}"
            );
        }
        other => return Err(format!("expected Disarmed, got {other:?}").into()),
    }
    Ok(())
}

/// Red first — the kill -9 honesty face: an ARMED bracket with no OUTCOME
/// and no DISARMED reads as `ArmedNotDisarmed` with NO record, never a
/// fabricated summary.
#[test]
fn a_killed_server_reads_as_armed_not_disarmed_with_no_record() -> TestResult {
    let home = tempfile::tempdir()?;
    write_note(
        home.path(),
        &[stamped("ARMED pid=200 version=0.23.0 build=test")],
    )?;
    match read_fate(home.path(), 200)? {
        NoteFate::ArmedNotDisarmed {
            outcome,
            outcome_unreadable,
        } => {
            assert_eq!(outcome, None, "no record must mean NO record");
            assert_eq!(
                outcome_unreadable, None,
                "an absent record is an absence, not an unreadable presence"
            );
        }
        other => return Err(format!("expected ArmedNotDisarmed, got {other:?}").into()),
    }
    Ok(())
}

/// The reader binds to the LAST bracket for the pid: an earlier clean
/// shutdown's record must not answer for the CURRENT incarnation.
#[test]
fn the_last_bracket_for_the_pid_wins() -> TestResult {
    let home = tempfile::tempdir()?;
    let old_record = parked_record(300);
    write_note(
        home.path(),
        &[
            stamped("ARMED pid=300 version=0.23.0 build=test"),
            stamped(&render_entry(&old_record)?),
            stamped("DISARMED clean run-loop exit: shutdown outcome Parked"),
            stamped("ARMED pid=300 version=0.23.0 build=test"),
        ],
    )?;
    match read_fate(home.path(), 300)? {
        NoteFate::ArmedNotDisarmed {
            outcome,
            outcome_unreadable,
        } => {
            assert_eq!(
                outcome, None,
                "the previous bracket's record must not leak into the new bracket"
            );
            assert_eq!(outcome_unreadable, None, "nor may its unreadable face");
        }
        other => return Err(format!("expected ArmedNotDisarmed, got {other:?}").into()),
    }
    Ok(())
}

/// A bracket that closed without a record reads as Disarmed with an honest
/// absence — the pre-record-binary and error-return shapes.
#[test]
fn a_disarmed_bracket_without_a_record_reports_the_absence() -> TestResult {
    let home = tempfile::tempdir()?;
    write_note(
        home.path(),
        &[
            stamped("ARMED pid=400 version=0.22.0 build=test"),
            stamped("DISARMED run scope exited without an explicit disarm"),
        ],
    )?;
    match read_fate(home.path(), 400)? {
        NoteFate::Disarmed { outcome, .. } => {
            assert_eq!(outcome, None, "absence must be reported as absence");
        }
        other => return Err(format!("expected Disarmed, got {other:?}").into()),
    }
    Ok(())
}

/// No note and no bracket are DISTINCT states, and another pid's bracket is
/// not ours.
#[test]
fn absence_states_are_distinct() -> TestResult {
    let home = tempfile::tempdir()?;
    assert_eq!(read_fate(home.path(), 1)?, NoteFate::NoNote);
    write_note(
        home.path(),
        &[stamped("ARMED pid=555 version=0.23.0 build=test")],
    )?;
    assert_eq!(
        read_fate(home.path(), 1)?,
        NoteFate::NoBracketForPid,
        "another pid's bracket must not answer for ours"
    );
    Ok(())
}

/// A torn OUTCOME line (a crash mid-write) never becomes a fabricated
/// record — and it is reported as an unreadable PRESENCE, not silently
/// collapsed into "the drain never wrote one".
#[test]
fn a_torn_outcome_line_reads_as_an_unreadable_presence() -> TestResult {
    let home = tempfile::tempdir()?;
    write_note(
        home.path(),
        &[
            stamped("ARMED pid=600 version=0.23.0 build=test"),
            stamped("OUTCOME {\"pid\":600,\"outcome\":\"Par"),
        ],
    )?;
    match read_fate(home.path(), 600)? {
        NoteFate::ArmedNotDisarmed {
            outcome,
            outcome_unreadable,
        } => {
            assert_eq!(
                outcome, None,
                "a torn record must never be guessed into a summary"
            );
            assert!(
                outcome_unreadable.is_some(),
                "a torn OUTCOME line is an UNREADABLE PRESENCE, distinct from the \
                 honest absence: the drain wrote a record even if this binary \
                 cannot read it"
            );
        }
        other => return Err(format!("expected ArmedNotDisarmed, got {other:?}").into()),
    }
    Ok(())
}

/// Both mixed orders keep BOTH facts. The renderers' rule — "a readable
/// record does not un-happen the torn line beside it" — is enforced at the
/// reader: whichever order the lines arrived in, a torn line and a readable
/// record inside one bracket are both carried.
#[test]
fn a_torn_line_survives_beside_a_readable_record_in_both_orders() -> TestResult {
    let record = parked_record(600);
    let good = stamped(&render_entry(&record)?);
    let torn = stamped("OUTCOME {\"pid\":600,\"outcome\":\"Par");

    for (label, lines) in [
        ("good-then-torn", [good.clone(), torn.clone()]),
        ("torn-then-good", [torn, good]),
    ] {
        let home = tempfile::tempdir()?;
        let mut note = vec![stamped("ARMED pid=600 version=0.23.0 build=test")];
        note.extend(lines);
        write_note(home.path(), &note)?;
        match read_fate(home.path(), 600)? {
            NoteFate::ArmedNotDisarmed {
                outcome,
                outcome_unreadable,
            } => {
                assert_eq!(
                    outcome.as_ref(),
                    Some(&record),
                    "{label}: the readable record must be carried"
                );
                assert!(
                    outcome_unreadable.is_some(),
                    "{label}: the torn line must be carried BESIDE the record, \
                     never erased by it"
                );
            }
            other => {
                return Err(format!("{label}: expected ArmedNotDisarmed, got {other:?}").into());
            }
        }
    }
    Ok(())
}
