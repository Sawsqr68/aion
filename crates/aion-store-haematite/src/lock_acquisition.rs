use std::path::Path;
use std::time::{Duration, Instant};

use haematite::{Database, DatabaseError};

/// Caller-owned ruling for retrying a transient data-directory writer lock.
///
/// Both spans are required because the acceptable boot delay and the retry
/// cadence are deployment decisions. A retry is attempted only after
/// haematite returns [`DatabaseError::DataDirLocked`]; every other open error
/// is returned immediately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LockAcquisitionRuling {
    patience: Duration,
    retry_cadence: Duration,
}

impl LockAcquisitionRuling {
    /// State the maximum acquisition span and the cadence between attempts.
    #[must_use]
    pub const fn new(patience: Duration, retry_cadence: Duration) -> Self {
        Self {
            patience,
            retry_cadence,
        }
    }

    /// State both acquisition spans in milliseconds.
    #[must_use]
    pub const fn from_millis(patience_ms: u64, retry_cadence_ms: u64) -> Self {
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

    use super::{LockAcquisitionRuling, open_database, open_database_with_wait};

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
            LockAcquisitionRuling::new(Duration::from_millis(100), Duration::from_millis(5)),
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
            LockAcquisitionRuling::new(Duration::from_millis(20), Duration::from_millis(5)),
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
