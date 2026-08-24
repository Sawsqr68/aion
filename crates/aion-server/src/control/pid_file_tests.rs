//! Red-first specimens for the pid file: claim, stale reconciliation, and
//! the guard's remove-only-our-record discipline.

use super::{PidRecord, StaleReconciliation, claim, pid_file_path, read, remove_if_matches};
use crate::control::incarnation;

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

/// The ordinary boot: no file, claim writes it, all fields round-trip, and
/// the guard's drop removes it.
#[test]
fn claim_writes_and_the_guard_removes_on_drop() -> TestResult {
    let home = tempfile::tempdir()?;
    let record = own_record()?;
    let guard = claim(home.path(), &record)?;
    assert_eq!(
        guard.reconciliation(),
        &StaleReconciliation::NonePresent,
        "an empty home has nothing to reconcile"
    );
    assert_eq!(
        read(home.path())?.as_ref(),
        Some(&record),
        "the file must hold the claimed record"
    );
    drop(guard);
    assert_eq!(
        read(home.path())?,
        None,
        "the guard must remove the file on clean exit"
    );
    Ok(())
}

/// Red first: a stale file whose pid is DEAD is reconciled as a dead
/// incarnation and replaced — never trusted, never an error.
#[test]
fn a_dead_incarnations_file_is_reconciled_and_replaced() -> TestResult {
    let home = tempfile::tempdir()?;
    let stale = dead_record()?;
    // Simulate the crash: the guard is leaked so the file stays behind.
    std::mem::forget(claim(home.path(), &stale)?);

    let record = own_record()?;
    let guard = claim(home.path(), &record)?;
    assert_eq!(
        guard.reconciliation(),
        &StaleReconciliation::DeadIncarnation(stale),
        "the dead predecessor must be named"
    );
    assert_eq!(
        read(home.path())?.as_ref(),
        Some(&record),
        "the new incarnation's record must replace the stale one"
    );
    Ok(())
}

/// Red first: a stale file naming a LIVE pid with the wrong start instant is
/// the reused-pid face — reconciled as such, the stranger untouched.
#[test]
fn a_reused_pids_file_is_reconciled_as_reused() -> TestResult {
    let home = tempfile::tempdir()?;
    let mut stale = own_record()?;
    stale.started_at_unix_secs = stale.started_at_unix_secs.wrapping_add(31);
    let leaked = claim(home.path(), &stale)?;
    std::mem::forget(leaked);

    let record = own_record()?;
    let guard = claim(home.path(), &record)?;
    assert_eq!(
        guard.reconciliation(),
        &StaleReconciliation::ReusedPid(stale),
        "a live pid with a foreign start instant is a reused pid"
    );
    Ok(())
}

/// An unparseable file (the pre-verb hand-written ritual wrote bare pids) is
/// reconciled as unreadable and replaced.
#[test]
fn a_hand_written_pid_file_is_reconciled_as_unreadable() -> TestResult {
    let home = tempfile::tempdir()?;
    let path = pid_file_path(home.path());
    std::fs::create_dir_all(path.parent().ok_or("pid path must have a parent")?)?;
    std::fs::write(&path, "93400\n")?;

    let record = own_record()?;
    let guard = claim(home.path(), &record)?;
    assert!(
        matches!(
            guard.reconciliation(),
            StaleReconciliation::Unreadable { .. }
        ),
        "a bare-pid file must reconcile as unreadable, got {:?}",
        guard.reconciliation()
    );
    assert_eq!(
        read(home.path())?.as_ref(),
        Some(&record),
        "the record must replace the hand-written file"
    );
    Ok(())
}

/// A live matching incarnation recorded on DIFFERENT addresses makes the new
/// claimant boot UNCLAIMED: the bind-success argument covers only the
/// recorded addresses, and a server live on other ports may be serving right
/// now — overwriting its record would leave a running server no verb can
/// address, while refusing to boot would break every legitimate multi-server
/// home. The first claimant keeps the record; the unclaimed guard's exit
/// removes nothing.
#[test]
fn a_live_incarnation_on_different_addresses_keeps_its_claim() -> TestResult {
    let home = tempfile::tempdir()?;
    let first_record = own_record()?;
    let first_guard = claim(home.path(), &first_record)?;

    let mut second_record = own_record()?;
    second_record.http_address = "127.0.0.1:18080".parse()?;
    second_record.grpc_address = "127.0.0.1:15005".parse()?;
    let second_guard = claim(home.path(), &second_record)?;
    assert!(
        matches!(
            second_guard.reconciliation(),
            crate::control::pid_file::StaleReconciliation::LiveIncarnation { .. }
        ),
        "the unclaimed boot must still report what it found: {:?}",
        second_guard.reconciliation()
    );
    assert_eq!(
        read(home.path())?.as_ref(),
        Some(&first_record),
        "the first claimant's record must survive the second boot"
    );
    drop(second_guard);
    assert_eq!(
        read(home.path())?.as_ref(),
        Some(&first_record),
        "an unclaimed guard's exit must remove NOTHING — the live claimant's \
         record survives"
    );
    drop(first_guard);
    assert_eq!(
        read(home.path())?,
        None,
        "the claim holder still removes its own record on exit"
    );
    Ok(())
}

/// The guard must NOT remove a successor's file: after a new incarnation
/// claims over a leaked guard's record, dropping the old guard leaves the
/// successor's claim in place.
#[test]
fn a_lingering_guard_leaves_a_successors_file_alone() -> TestResult {
    let home = tempfile::tempdir()?;
    let mut first_record = own_record()?;
    first_record.commit = "first".to_owned();
    let first_guard = claim(home.path(), &first_record)?;

    let mut second_record = own_record()?;
    second_record.commit = "second".to_owned();
    let second_guard = claim(home.path(), &second_record)?;
    assert!(
        matches!(
            second_guard.reconciliation(),
            crate::control::pid_file::StaleReconciliation::LiveIncarnation { .. }
        ),
        "claiming over a live incarnation's record must be REPORTED as exactly \
         that face, not silently absorbed: {:?}",
        second_guard.reconciliation()
    );

    drop(first_guard);
    assert_eq!(
        read(home.path())?.as_ref(),
        Some(&second_record),
        "the first guard must not delete the second incarnation's record"
    );
    drop(second_guard);
    assert_eq!(read(home.path())?, None, "the owner removes its own record");
    Ok(())
}

/// `remove_if_matches` removes exactly its own record and reports which.
#[test]
fn remove_if_matches_is_exact() -> TestResult {
    let home = tempfile::tempdir()?;
    let record = own_record()?;
    let guard = claim(home.path(), &record)?;
    std::mem::forget(guard);

    let mut other = record.clone();
    other.commit = "someone-else".to_owned();
    assert!(
        !remove_if_matches(home.path(), &other)?,
        "a differing record must not remove the file"
    );
    assert!(
        remove_if_matches(home.path(), &record)?,
        "the matching record removes it"
    );
    assert!(
        !remove_if_matches(home.path(), &record)?,
        "an absent file removes nothing"
    );
    Ok(())
}

/// Reading distinguishes absence from unreadability: absent is `Ok(None)`,
/// present-but-corrupt is a typed error naming the remedy.
#[test]
fn read_distinguishes_absent_from_unreadable() -> TestResult {
    let home = tempfile::tempdir()?;
    assert_eq!(read(home.path())?, None, "no file is a state, not an error");
    let path = pid_file_path(home.path());
    std::fs::create_dir_all(path.parent().ok_or("pid path must have a parent")?)?;
    std::fs::write(&path, "not json")?;
    let error = match read(home.path()) {
        Err(error) => error,
        Ok(record) => return Err(format!("corrupt file must refuse, got {record:?}").into()),
    };
    assert!(
        error.to_string().contains("does not parse"),
        "the refusal must say why: {error}"
    );
    Ok(())
}
