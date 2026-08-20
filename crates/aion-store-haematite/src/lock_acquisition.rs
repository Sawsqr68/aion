use std::path::Path;
use std::time::{Duration, Instant};

use haematite::{Database, DatabaseError};

/// Refusal of a [`LockAcquisitionRuling`] whose spans cannot bound a real,
/// cadenced retry of the data-directory writer lock.
///
/// The ruling drives REAL [`Database::open`] attempts, each of which takes an
/// advisory `flock`, so a span that reads as a number but does not bound
/// anything is not a weaker ruling — it is a different mechanism (a busy-spin,
/// or a single unwaited attempt) wearing the ruling's name. The invariant is
/// therefore enforced at construction rather than left to each caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LockAcquisitionRulingError {
    /// The patience span was zero: the first refusal is already past the
    /// deadline, so no retry is ever attempted and the ruling silently
    /// degrades to the single unwaited `try_lock` it exists to replace.
    #[error(
        "lock acquisition patience must be positive: a zero span is already spent at the first refusal, so nothing is ever retried"
    )]
    PatienceNotPositive,
    /// The retry cadence was zero: nothing separates the attempts, so the
    /// patience is spent busy-spinning real database opens on a hot loop.
    #[error(
        "lock acquisition retry cadence must be positive: a zero cadence busy-spins real database open attempts for the whole patience"
    )]
    RetryCadenceNotPositive,
    /// The retry cadence exceeded the patience: the single wait is clamped to
    /// what remains of the patience, so the stated cadence never separates two
    /// attempts and does not describe what the loop does.
    #[error(
        "lock acquisition retry cadence ({retry_cadence:?}) must not exceed the patience ({patience:?}): the wait is clamped to the remaining patience, so the stated cadence never governs an attempt"
    )]
    RetryCadenceExceedsPatience {
        /// The refused patience span.
        patience: Duration,
        /// The refused retry cadence.
        retry_cadence: Duration,
    },
}

/// Caller-owned ruling for retrying a transient data-directory writer lock.
///
/// Both spans are required because the acceptable boot delay and the retry
/// cadence are deployment decisions. A retry is attempted only after
/// haematite returns [`DatabaseError::DataDirLocked`]; every other open error
/// is returned immediately.
///
/// The type owns its invariant: both spans are positive and the cadence does
/// not exceed the patience. There is no unchecked constructor, so a value of
/// this type always bounds at least one waited retry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LockAcquisitionRuling {
    patience: Duration,
    retry_cadence: Duration,
}

impl LockAcquisitionRuling {
    /// State the maximum acquisition span and the cadence between attempts.
    ///
    /// # Errors
    ///
    /// Returns [`LockAcquisitionRulingError::PatienceNotPositive`] when
    /// `patience` is zero, [`LockAcquisitionRulingError::RetryCadenceNotPositive`]
    /// when `retry_cadence` is zero, and
    /// [`LockAcquisitionRulingError::RetryCadenceExceedsPatience`] when
    /// `retry_cadence` is longer than `patience`.
    pub fn new(
        patience: Duration,
        retry_cadence: Duration,
    ) -> Result<Self, LockAcquisitionRulingError> {
        if patience.is_zero() {
            return Err(LockAcquisitionRulingError::PatienceNotPositive);
        }
        if retry_cadence.is_zero() {
            return Err(LockAcquisitionRulingError::RetryCadenceNotPositive);
        }
        if retry_cadence > patience {
            return Err(LockAcquisitionRulingError::RetryCadenceExceedsPatience {
                patience,
                retry_cadence,
            });
        }
        Ok(Self {
            patience,
            retry_cadence,
        })
    }

    /// State both acquisition spans in milliseconds.
    ///
    /// # Errors
    ///
    /// Returns the same refusals as [`new`](LockAcquisitionRuling::new) for the
    /// millisecond spans given here.
    pub fn from_millis(
        patience_ms: u64,
        retry_cadence_ms: u64,
    ) -> Result<Self, LockAcquisitionRulingError> {
        Self::new(
            Duration::from_millis(patience_ms),
            Duration::from_millis(retry_cadence_ms),
        )
    }
}

pub(crate) fn open_database(
    data_dir: &Path,
    ruling: LockAcquisitionRuling,
) -> Result<Database, DatabaseError> {
    open_database_with_wait(data_dir, ruling, std::thread::sleep)
}

fn open_database_with_wait(
    data_dir: &Path,
    ruling: LockAcquisitionRuling,
    mut wait: impl FnMut(Duration),
) -> Result<Database, DatabaseError> {
    let started = Instant::now();
    loop {
        match Database::open(data_dir) {
            Ok(database) => return Ok(database),
            Err(error @ DatabaseError::DataDirLocked { .. }) => {
                let elapsed = started.elapsed();
                if elapsed >= ruling.patience {
                    return Err(error);
                }
                let remaining = ruling.patience.saturating_sub(elapsed);
                // The caller-ruled cadence separates real lock attempts.
                wait(ruling.retry_cadence.min(remaining));
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::sync::mpsc;
    use std::time::Duration;

    use super::{
        LockAcquisitionRuling, LockAcquisitionRulingError, open_database, open_database_with_wait,
    };

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn spans_that_bound_no_waited_retry_are_refused_at_construction() -> TestResult {
        assert_eq!(
            LockAcquisitionRuling::from_millis(0, 5).err(),
            Some(LockAcquisitionRulingError::PatienceNotPositive),
            "a zero patience is spent at the first refusal and must not construct"
        );
        assert_eq!(
            LockAcquisitionRuling::from_millis(250, 0).err(),
            Some(LockAcquisitionRulingError::RetryCadenceNotPositive),
            "a zero cadence busy-spins real opens and must not construct"
        );
        assert_eq!(
            LockAcquisitionRuling::from_millis(5, 250).err(),
            Some(LockAcquisitionRulingError::RetryCadenceExceedsPatience {
                patience: Duration::from_millis(5),
                retry_cadence: Duration::from_millis(250),
            }),
            "a cadence longer than the patience never governs an attempt"
        );
        // The accepted case: a well-formed pair keeps both spans verbatim. This
        // is the constructor's own contract, not a ruling on any acquisition.
        let ruling = LockAcquisitionRuling::from_millis(250, 5)?;
        assert_eq!(ruling.patience, Duration::from_millis(250));
        assert_eq!(ruling.retry_cadence, Duration::from_millis(5));
        // The boundary case — cadence exactly equal to the patience — is a
        // single waited retry, which is a ruling, so it is accepted.
        LockAcquisitionRuling::from_millis(5, 5)?;
        Ok(())
    }

    fn prepared_store()
    -> Result<(tempfile::TempDir, std::path::PathBuf), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let data_dir = directory.path().join("store");
        let store =
            crate::HaematiteStore::create(&data_dir, haematite::NodeCacheBudget::Unlimited)?;
        drop(store);
        Ok((directory, data_dir))
    }

    #[test]
    fn transient_lock_is_retried_but_live_writer_still_fails_unchanged() -> TestResult {
        let (_directory, data_dir) = prepared_store()?;
        let lock_path = data_dir.join("writer.lock");
        let lock_file = OpenOptions::new().read(true).write(true).open(&lock_path)?;
        lock_file.lock()?;

        let (release_sender, release_receiver) = mpsc::channel();
        let releaser = std::thread::spawn(move || -> std::io::Result<()> {
            release_receiver.recv().map_err(std::io::Error::other)?;
            lock_file.unlock()
        });
        let mut release_sent = false;
        let mut release_send_failed = false;
        let reopened = open_database_with_wait(
            &data_dir,
            LockAcquisitionRuling::new(Duration::from_millis(100), Duration::from_millis(5))?,
            |_| {
                if !release_sent {
                    release_sent = true;
                    release_send_failed = release_sender.send(()).is_err();
                }
            },
        )?;
        if release_send_failed {
            return Err("failed to signal the lock releaser".into());
        }
        releaser
            .join()
            .map_err(|_| "lock releaser thread panicked")??;
        drop(reopened);

        let genuine_holder = OpenOptions::new().read(true).write(true).open(&lock_path)?;
        genuine_holder.lock()?;
        let error = open_database(
            &data_dir,
            LockAcquisitionRuling::new(Duration::from_millis(20), Duration::from_millis(5))?,
        )
        .err()
        .ok_or("a live writer unexpectedly allowed a second open")?;
        assert!(matches!(
            error,
            haematite::DatabaseError::DataDirLocked { .. }
        ));
        genuine_holder.unlock()?;
        Ok(())
    }
}
