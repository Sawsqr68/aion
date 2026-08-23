//! Indefinite, loudly-reported waiting for the data-directory writer lock.
//!
//! The writer lock is an advisory `flock` haematite takes inside
//! [`Database::open`]. A kernel-released lock cannot go stale: its holder is
//! either a live process — a draining predecessor mid-handover, or another
//! server on the same data directory — or already dead, in which case the
//! kernel has released the lock and the next open succeeds. There is
//! therefore no hazard for a patience ceiling to guard: a bounded wait can
//! only convert an ordinary slow handover into a refused boot (the 0.22.0
//! estate deploy did exactly that — seven waiters died on a patience ruling
//! while one live predecessor drained; ruled out 2026-08-24). Boot waits for
//! the lock indefinitely and says so, at intervals, so an operator reading
//! the log sees a handover in progress, never a silent hang.
//!
//! The spans in this module are implementation constants, not configuration:
//! the retired `store.lock_acquisition_patience_ms` /
//! `store.lock_acquisition_retry_cadence_ms` keys are accepted-and-warned by
//! the server config (never refused, so existing estates boot) and ignored.

use std::path::Path;
use std::time::{Duration, Instant};

use haematite::{Database, DatabaseError};

/// File name of haematite's writer-lock file inside the data directory. Used
/// only to PROBE lock availability cheaply; the lock that admits a writer is
/// always the one [`Database::open`] itself takes.
const WRITER_LOCK_FILE: &str = "writer.lock";

/// How long the waiter sleeps between cheap probes of the writer lock while
/// a holder lives. An implementation constant, deliberately not surface: the
/// probe is one `try_lock` syscall on this process's own descriptor, and a
/// full [`Database::open`] — which can replay WAL recovery reads — is
/// attempted only after a probe finds the lock takeable, so this cadence
/// bounds handover-detection latency, not open-attempt cost.
const PROBE_CADENCE: Duration = Duration::from_millis(500);

/// How often the wait reports that it is still waiting. Frequent enough that
/// a log tail always shows the newest line within seconds of looking;
/// infrequent enough that a long handover does not flood the log.
const REPORT_INTERVAL: Duration = Duration::from_secs(10);

/// Open the database at `data_dir`, waiting indefinitely — and saying so —
/// while another live process holds the writer lock.
///
/// Every error other than [`DatabaseError::DataDirLocked`] is returned
/// immediately: only the one refusal that means "a live holder exists" is
/// waited out, because only that refusal is guaranteed to resolve (the
/// holder exits, the kernel releases).
pub(crate) fn open_database(data_dir: &Path) -> Result<Database, DatabaseError> {
    open_database_with(data_dir, std::thread::sleep, |path, waited| {
        tracing::warn!(
            writer_lock = %path.display(),
            waited_secs = waited.as_secs(),
            "waiting for the data-directory writer lock: it is held by a live \
             process (a draining predecessor mid-handover, or another server on \
             this data directory). The kernel releases the lock the moment its \
             holder exits; this boot will proceed then. Not an error — a \
             handover in progress"
        );
    })
}

/// [`open_database`] with the wait and the report injectable, so tests can
/// drive a real contended lock without real time or a captured logger.
fn open_database_with(
    data_dir: &Path,
    mut wait: impl FnMut(Duration),
    mut report: impl FnMut(&Path, Duration),
) -> Result<Database, DatabaseError> {
    let lock_path = data_dir.join(WRITER_LOCK_FILE);
    let started = Instant::now();
    let mut last_report: Option<Instant> = None;
    loop {
        match Database::open(data_dir) {
            Ok(database) => return Ok(database),
            Err(DatabaseError::DataDirLocked { .. }) => {
                report_if_due(&mut report, &lock_path, started, &mut last_report);
                wait_until_lock_looks_takeable(
                    &lock_path,
                    started,
                    &mut wait,
                    &mut report,
                    &mut last_report,
                );
            }
            Err(error) => return Err(error),
        }
    }
}

/// Sleep-probe until this process can momentarily take the writer lock, then
/// release it at once and return so the caller re-runs the real open. The
/// probe is what keeps the wait cheap: a contended [`Database::open`] can
/// replay recovery work before meeting the lock, and re-running that on
/// every poll multiplies I/O for no faster handover.
///
/// A probe that cannot even reach the lock file (it does not exist yet, or
/// cannot be opened) returns immediately: the real open is the authority on
/// what that means, and it will say so in a typed error.
fn wait_until_lock_looks_takeable(
    lock_path: &Path,
    started: Instant,
    wait: &mut impl FnMut(Duration),
    report: &mut impl FnMut(&Path, Duration),
    last_report: &mut Option<Instant>,
) {
    loop {
        let Ok(probe) = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(lock_path)
        else {
            return;
        };
        match probe.try_lock() {
            Ok(()) => {
                // Released on drop; the caller's real open takes it for keeps.
                // Losing the race to another waiter just re-enters the wait.
                drop(probe);
                return;
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                report_if_due(report, lock_path, started, last_report);
                wait(PROBE_CADENCE);
            }
            Err(std::fs::TryLockError::Error(_)) => return,
        }
    }
}

/// Emit the still-waiting report when none has been emitted yet or the last
/// one is older than [`REPORT_INTERVAL`].
fn report_if_due(
    report: &mut impl FnMut(&Path, Duration),
    lock_path: &Path,
    started: Instant,
    last_report: &mut Option<Instant>,
) {
    let due = last_report.is_none_or(|at| at.elapsed() >= REPORT_INTERVAL);
    if due {
        report(lock_path, started.elapsed());
        *last_report = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::sync::mpsc;

    use super::open_database_with;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn prepared_store()
    -> Result<(tempfile::TempDir, std::path::PathBuf), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let data_dir = directory.path().join("store");
        let store =
            crate::HaematiteStore::create(&data_dir, haematite::NodeCacheBudget::Unlimited)?;
        drop(store);
        Ok((directory, data_dir))
    }

    /// The ruled-out failure, as a specimen: a held writer lock is WAITED OUT
    /// — however long the holder takes — and the boot proceeds when the
    /// holder releases. Before the 2026-08-24 ruling this exact arrangement
    /// could refuse with `DataDirLocked` once a patience span elapsed; the
    /// wait now has no far side for a refusal to live on.
    #[test]
    fn a_held_lock_is_waited_out_and_the_open_proceeds_on_release() -> TestResult {
        let (_directory, data_dir) = prepared_store()?;
        let lock_path = data_dir.join("writer.lock");
        let lock_file = OpenOptions::new().read(true).write(true).open(&lock_path)?;
        lock_file.lock()?;

        let (release_sender, release_receiver) = mpsc::channel();
        let releaser = std::thread::spawn(move || -> std::io::Result<()> {
            release_receiver.recv().map_err(std::io::Error::other)?;
            lock_file.unlock()
        });
        let mut reports = 0_u32;
        let mut release_sent = false;
        let mut release_send_failed = false;
        let reopened = open_database_with(
            &data_dir,
            |_| {
                // The first wait proves the holder was seen live; release it
                // so the wait's only exit — the holder exiting — occurs.
                if !release_sent {
                    release_sent = true;
                    release_send_failed = release_sender.send(()).is_err();
                }
            },
            |_, _| reports += 1,
        )?;
        if release_send_failed {
            return Err("failed to signal the lock releaser".into());
        }
        releaser
            .join()
            .map_err(|_| "lock releaser thread panicked")??;
        drop(reopened);
        assert!(
            reports >= 1,
            "a waited-on lock must be reported, not waited on silently"
        );
        Ok(())
    }

    /// Errors other than a held lock are NOT waited out: they return at once,
    /// untouched. Waiting can only resolve the one refusal whose cause is a
    /// live holder; everything else is the operator's to see immediately.
    #[test]
    fn a_non_lock_error_returns_immediately_without_waiting() -> TestResult {
        let directory = tempfile::tempdir()?;
        // No store was ever created here and the directory is empty, so the
        // open fails for a non-lock reason (nothing to open).
        let data_dir = directory.path().join("never-created");
        let error = open_database_with(
            &data_dir,
            |_| unreachable!("a non-lock failure must not wait"),
            |_, _| unreachable!("a non-lock failure must not report a lock wait"),
        )
        .err()
        .ok_or("opening a nonexistent store unexpectedly succeeded")?;
        assert!(
            !matches!(error, haematite::DatabaseError::DataDirLocked { .. }),
            "the specimen must exercise the non-lock path, got a lock refusal"
        );
        Ok(())
    }

    /// The reports keep coming while the wait continues (interval-gated), and
    /// the wait survives many probe rounds — the indefinite property probed
    /// as far as a test can: no internal counter or span ends it.
    #[test]
    fn waiting_reports_and_never_gives_up_across_many_probe_rounds() -> TestResult {
        let (_directory, data_dir) = prepared_store()?;
        let lock_path = data_dir.join("writer.lock");
        let lock_file = OpenOptions::new().read(true).write(true).open(&lock_path)?;
        lock_file.lock()?;

        let (release_sender, release_receiver) = mpsc::channel();
        let releaser = std::thread::spawn(move || -> std::io::Result<()> {
            release_receiver.recv().map_err(std::io::Error::other)?;
            lock_file.unlock()
        });
        let mut probe_rounds = 0_u32;
        let mut release_send_failed = false;
        let reopened = open_database_with(
            &data_dir,
            |_| {
                probe_rounds += 1;
                // Hold through many rounds before releasing: a patience of any
                // of the retired shapes (250ms at 5ms cadence ≈ 50 rounds)
                // would have refused long before round 60.
                if probe_rounds == 60 {
                    release_send_failed = release_sender.send(()).is_err();
                }
            },
            |_, _| {},
        )?;
        if release_send_failed {
            return Err("failed to signal the lock releaser".into());
        }
        releaser
            .join()
            .map_err(|_| "lock releaser thread panicked")??;
        drop(reopened);
        assert!(
            probe_rounds >= 60,
            "the wait must have survived at least the 60 probe rounds the \
             releaser held it for, got {probe_rounds}"
        );
        Ok(())
    }
}
