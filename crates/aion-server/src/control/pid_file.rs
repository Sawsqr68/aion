//! The server's pid file: `<AION_HOME>/run/aion-server.pid`.
//!
//! The file is the stop/status verbs' address for the running server. It is
//! written only after BOTH transport listeners have bound successfully — a
//! boot that loses the port race must never overwrite the live server's
//! record — and removed on clean exit by [`PidFileGuard`], which deletes the
//! file only while it still holds this incarnation's record, so a guard
//! outliving its server can never destroy a successor's claim.
//!
//! Every mutation of the file — the claim's rename-into-place, the stop
//! verb's compare-and-delete, the guard's own exit-time compare-and-delete —
//! runs under an exclusive `flock` on a sibling lock file
//! (`aion-server.pid.lock`). The lock is what makes compare-and-delete
//! atomic: without it, a successor claiming the home between a remover's
//! read and its unlink would have ITS record deleted, leaving a live server
//! no verb can address. The lock file itself is never renamed or removed —
//! locking the pid file directly would be unsound, because the claim
//! replaces that path's inode and a lock on the old inode excludes nobody.
//!
//! The record is an incarnation identity, never a bare pid: pid, start
//! instant, and the serving binary's content hash (plus the bound addresses
//! and build identity, so readers talk to the server that IS running rather
//! than the one today's config would start). A stale file found at claim
//! time is reconciled by incarnation check and reported — never silently
//! overwritten, never trusted. See [`crate::control::incarnation`] for how a
//! record is verified against the live process table.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::error::ServerError;

/// File name of the pid file inside the Aion home's `run/` directory.
const PID_FILE_NAME: &str = "aion-server.pid";

/// File name of the mutation lock beside the pid file. Held exclusively for
/// the duration of every pid-file mutation; never renamed, never removed
/// (unlinking a lock file reintroduces the race the lock exists to close).
const PID_LOCK_FILE_NAME: &str = "aion-server.pid.lock";

/// Take the exclusive pid-file mutation lock for `pid_path`'s home.
///
/// Blocks until the lock is granted; every holder is momentary (a read, a
/// compare, one unlink or rename), so waiting is bounded in practice. The
/// returned guard releases on drop.
///
/// # Errors
///
/// Returns [`ServerError::PidFile`] when the lock file cannot be created or
/// the lock cannot be taken.
fn lock_pid_mutation(pid_path: &Path) -> Result<nix::fcntl::Flock<std::fs::File>, ServerError> {
    let lock_path = pid_path.with_file_name(PID_LOCK_FILE_NAME);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(|io_error| {
            pid_file_error(format!(
                "could not open the pid mutation lock `{}`: {io_error}",
                lock_path.display()
            ))
        })?;
    nix::fcntl::Flock::lock(file, nix::fcntl::FlockArg::LockExclusive).map_err(|(_file, errno)| {
        pid_file_error(format!(
            "could not take the pid mutation lock `{}`: {errno}",
            lock_path.display()
        ))
    })
}

/// Release the mutation lock explicitly, so the failure face is a WARN in our
/// own words rather than the lock guard's drop behaviour. An unlock on an
/// owned, valid descriptor has no realistic failure mode; the arm exists so
/// no path is silent.
fn release_pid_mutation(lock: nix::fcntl::Flock<std::fs::File>, pid_path: &Path) {
    if let Err((_file, errno)) = lock.unlock() {
        warn!(
            path = %pid_path.display(),
            %errno,
            "could not release the pid mutation lock; the kernel releases it when \
             this process exits"
        );
    }
}

/// The pid file's content: one JSON object describing the server incarnation
/// that claimed the home.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PidRecord {
    /// Operating-system process id of the serving process.
    pub pid: u32,
    /// The process start instant in whole seconds since the Unix epoch, read
    /// through the process-table instrument ([`sysinfo`]) at boot. Verifiers
    /// compare this against the SAME instrument's answer for the live pid, so
    /// the check is exact equality, not clock arithmetic: a reused pid wears
    /// a different start instant.
    pub started_at_unix_secs: u64,
    /// SHA-256 of the serving binary's bytes, hashed from the executable path
    /// at boot. This records WHICH build claimed the home; it is not a
    /// liveness discriminator (an upgrade that swaps the binary by rename
    /// changes the file on disk while the process keeps its old image).
    pub binary_sha256: String,
    /// Crate version of the serving binary (`CARGO_PKG_VERSION` at build).
    pub version: String,
    /// Source commit of the serving binary, from
    /// [`crate::build_identity::BuildIdentity`].
    pub commit: String,
    /// HTTP address the server actually bound — recorded so `status` probes
    /// the running server's own listener even when the config file has been
    /// edited since boot.
    pub http_address: SocketAddr,
    /// gRPC address the server actually bound, recorded for the same reason.
    pub grpc_address: SocketAddr,
    /// The drain window this incarnation is actually running with, in whole
    /// seconds — recorded for the same reason as the addresses: the running
    /// server's window may have come from its own command line
    /// (`--drain-timeout`) or from a config file edited since boot, and a
    /// stop verb that waited out a re-derived value would misreport a healthy
    /// long drain as "still draining" (or force it on a second invocation).
    pub drain_timeout_seconds: u64,
}

/// What [`claim`] found already sitting at the pid-file path, reconciled
/// before the new record was written. Reported to the operator through the
/// boot log; returned so tests can assert the reconciliation happened rather
/// than trusting the log.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StaleReconciliation {
    /// No file was present: the ordinary boot.
    NonePresent,
    /// A file was present but its process is gone — the previous server died
    /// without removing its record (crash, `SIGKILL`). The record is replaced.
    DeadIncarnation(PidRecord),
    /// A file was present and its pid is alive, but the live process's start
    /// instant does not match the record — the pid was reused by something
    /// else after the recorded server died. The record is replaced; the
    /// stranger keeps running untouched.
    ReusedPid(PidRecord),
    /// A file was present, its pid is alive, and the incarnation MATCHES: a
    /// server holding this home appears to still be running. The record is
    /// replaced ONLY when the recorded addresses equal the ones this boot
    /// just bound — there the successful binds prove the recorded server is
    /// not serving them, so the record is presumed left over from an
    /// incompletely observed exit and replaced, with the contradiction
    /// reported in full. A live matching incarnation recorded on DIFFERENT
    /// addresses proves nothing of the kind — that server may be serving
    /// them right now — so the claimant boots UNCLAIMED instead: the first
    /// claimant keeps the record and the control verbs, the new incarnation
    /// serves without them, and the warning names the consequence. Neither
    /// server's record is ever destroyed by the other's boot or exit.
    LiveIncarnation(PidRecord),
    /// A file was present but could not be parsed as a [`PidRecord`] (a
    /// hand-written pid from the pre-verb ritual, or corruption). Replaced.
    Unreadable {
        /// Why the existing content did not parse.
        reason: String,
    },
}

/// Absolute path of the pid file under `home`.
#[must_use]
pub fn pid_file_path(home: &Path) -> PathBuf {
    home.join("run").join(PID_FILE_NAME)
}

/// Read and parse the pid file under `home`.
///
/// Returns `Ok(None)` when the file does not exist — the ordinary "no server
/// has claimed this home" state, distinct from every error.
///
/// # Errors
///
/// Returns [`ServerError::PidFile`] when the file exists but cannot be read,
/// or exists and cannot be parsed as a [`PidRecord`].
pub fn read(home: &Path) -> Result<Option<PidRecord>, ServerError> {
    let path = pid_file_path(home);
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(io_error) if io_error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(io_error) => {
            return Err(pid_file_error(format!(
                "could not read pid file `{}`: {io_error}",
                path.display()
            )));
        }
    };
    let record = serde_json::from_str::<PidRecord>(&content).map_err(|parse_error| {
        pid_file_error(format!(
            "pid file `{}` exists but does not parse as a pid record: {parse_error}. \
             If it was written by hand (the pre-verb restart ritual wrote a bare pid), \
             remove it and restart the server so the server writes its own record",
            path.display()
        ))
    })?;
    Ok(Some(record))
}

/// Claim the pid file for `record`: reconcile whatever is already there,
/// write the new record atomically (temp file + rename), and return the
/// guard whose `Drop` removes the file on clean exit.
///
/// Call only after every listener has bound: successful binds are the proof
/// that no live server is serving this home's addresses.
///
/// # Errors
///
/// Returns [`ServerError::PidFile`] when the `run/` directory or the file
/// cannot be created, written, or renamed into place. A stale file that
/// cannot be REASONED about (unreadable) is reconciled and reported, never an
/// error: refusing to boot over a corrupt leftover would turn a crash's
/// debris into an outage.
pub fn claim(home: &Path, record: &PidRecord) -> Result<PidFileGuard, ServerError> {
    let path = pid_file_path(home);
    let run_dir = path.parent().ok_or_else(|| {
        pid_file_error(format!(
            "pid file path `{}` has no parent directory",
            path.display()
        ))
    })?;
    std::fs::create_dir_all(run_dir).map_err(|io_error| {
        pid_file_error(format!(
            "could not create run directory `{}`: {io_error}",
            run_dir.display()
        ))
    })?;
    // The read (reconcile) and the rename are one critical section: a
    // remover running between them could compare against the record this
    // claim is about to replace and unlink the replacement.
    let mutation_lock = lock_pid_mutation(&path)?;
    let reconciliation = reconcile_existing(&path);
    if let StaleReconciliation::LiveIncarnation(existing) = &reconciliation
        && (existing.http_address, existing.grpc_address)
            != (record.http_address, record.grpc_address)
    {
        // The bind-success argument only covers the recorded addresses:
        // this boot bound ITS addresses, which says nothing about whether
        // the live recorded server is still serving DIFFERENT ones.
        // Overwriting here would leave a running server no verb can address,
        // and refusing to boot would break every legitimate multi-server
        // home (concurrent test servers, fleet boxes on one default home).
        // So this boot RUNS UNCLAIMED: the first claimant keeps the record
        // and the verbs, this incarnation serves without either, and the
        // warning names the consequence.
        warn!(
            path = %path.display(),
            recorded_pid = existing.pid,
            recorded_http_address = %existing.http_address,
            this_http_address = %record.http_address,
            "a live server already holds this home's pid file on different \
             addresses; this incarnation boots UNCLAIMED — `aion server \
             stop`/`status` will address the recorded server, not this one. \
             Give each server its own AION_HOME to make both addressable"
        );
        release_pid_mutation(mutation_lock, &path);
        return Ok(PidFileGuard {
            path,
            record: record.clone(),
            reconciliation,
            holds_claim: false,
        });
    }
    report_reconciliation(&path, &reconciliation);
    let content = serde_json::to_string(record).map_err(|serialize_error| {
        pid_file_error(format!(
            "could not serialize the pid record: {serialize_error}"
        ))
    })?;
    let temp_path = path.with_extension("pid.tmp");
    std::fs::write(&temp_path, format!("{content}\n")).map_err(|io_error| {
        pid_file_error(format!(
            "could not write pid file staging `{}`: {io_error}",
            temp_path.display()
        ))
    })?;
    std::fs::rename(&temp_path, &path).map_err(|io_error| {
        pid_file_error(format!(
            "could not move pid file into place at `{}`: {io_error}",
            path.display()
        ))
    })?;
    release_pid_mutation(mutation_lock, &path);
    info!(
        path = %path.display(),
        pid = record.pid,
        started_at_unix_secs = record.started_at_unix_secs,
        "pid file written; this incarnation has claimed the home"
    );
    Ok(PidFileGuard {
        path,
        record: record.clone(),
        reconciliation,
        holds_claim: true,
    })
}

/// Remove the pid file under `home` if — and only if — it still holds
/// exactly `record`. Used by the stop verb to reconcile the file a
/// non-clean exit (forced, killed) left behind, after the process is proven
/// gone.
///
/// Returns whether the file was removed.
///
/// # Errors
///
/// Returns [`ServerError::PidFile`] when the file cannot be read or removed.
pub fn remove_if_matches(home: &Path, record: &PidRecord) -> Result<bool, ServerError> {
    let path = pid_file_path(home);
    if !path.exists() {
        // No pid file means nothing to remove and — since the lock file lives
        // beside it — possibly no run directory to open a lock in either.
        return Ok(false);
    }
    // Read, compare, and unlink under the mutation lock: without it a
    // successor claiming the home between the read and the unlink would have
    // ITS record deleted, leaving a live server no verb can address.
    let mutation_lock = lock_pid_mutation(&path)?;
    let removed = match read(home)? {
        Some(current) if current == *record => {
            std::fs::remove_file(&path).map_err(|io_error| {
                pid_file_error(format!(
                    "could not remove pid file `{}`: {io_error}",
                    path.display()
                ))
            })?;
            true
        }
        Some(_) | None => false,
    };
    release_pid_mutation(mutation_lock, &path);
    Ok(removed)
}

/// Classify what is already at `path` without touching it.
fn reconcile_existing(path: &Path) -> StaleReconciliation {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(io_error) if io_error.kind() == std::io::ErrorKind::NotFound => {
            return StaleReconciliation::NonePresent;
        }
        Err(io_error) => {
            return StaleReconciliation::Unreadable {
                reason: format!("could not read the existing file: {io_error}"),
            };
        }
    };
    let record = match serde_json::from_str::<PidRecord>(&content) {
        Ok(record) => record,
        Err(parse_error) => {
            return StaleReconciliation::Unreadable {
                reason: format!("existing content does not parse as a pid record: {parse_error}"),
            };
        }
    };
    match super::incarnation::probe(&record) {
        super::incarnation::IncarnationProbe::ProcessGone => {
            StaleReconciliation::DeadIncarnation(record)
        }
        super::incarnation::IncarnationProbe::DifferentIncarnation { .. } => {
            StaleReconciliation::ReusedPid(record)
        }
        super::incarnation::IncarnationProbe::Verified { .. } => {
            StaleReconciliation::LiveIncarnation(record)
        }
    }
}

/// Say what the reconciliation found, at the severity it deserves.
fn report_reconciliation(path: &Path, reconciliation: &StaleReconciliation) {
    match reconciliation {
        StaleReconciliation::NonePresent => {}
        StaleReconciliation::DeadIncarnation(record) => {
            warn!(
                path = %path.display(),
                stale_pid = record.pid,
                stale_started_at_unix_secs = record.started_at_unix_secs,
                stale_version = %record.version,
                "stale pid file: the recorded server (pid gone) died without removing \
                 its record; replacing it with this incarnation's"
            );
        }
        StaleReconciliation::ReusedPid(record) => {
            warn!(
                path = %path.display(),
                stale_pid = record.pid,
                stale_started_at_unix_secs = record.started_at_unix_secs,
                "stale pid file: the recorded pid is alive but belongs to a different \
                 process (pid reused after the recorded server died); replacing the \
                 record and leaving that process untouched"
            );
        }
        StaleReconciliation::LiveIncarnation(record) => {
            warn!(
                path = %path.display(),
                recorded_pid = record.pid,
                recorded_http_address = %record.http_address,
                "pid file names a LIVE matching incarnation recorded on THESE SAME \
                 addresses, yet this boot bound them — a contradiction (the recorded \
                 server cannot be serving them). Replacing the record as debris of an \
                 incompletely observed exit"
            );
        }
        StaleReconciliation::Unreadable { reason } => {
            warn!(
                path = %path.display(),
                %reason,
                "pid file present but unreadable as a record (hand-written or \
                 corrupt); replacing it with this incarnation's"
            );
        }
    }
}

/// Removes the pid file on drop — but only while the file still holds the
/// record this guard wrote, so a lingering guard can never delete a
/// successor incarnation's claim.
#[derive(Debug)]
pub struct PidFileGuard {
    path: PathBuf,
    record: PidRecord,
    reconciliation: StaleReconciliation,
    /// Whether this guard's incarnation actually holds the claim. False for
    /// a server that booted UNCLAIMED over another live claimant's record —
    /// such a guard owns nothing on disk and its `Drop` must remove nothing.
    holds_claim: bool,
}

impl PidFileGuard {
    /// What [`claim`] found and reconciled when this guard was created.
    #[must_use]
    pub fn reconciliation(&self) -> &StaleReconciliation {
        &self.reconciliation
    }

    /// The record this guard wrote.
    #[must_use]
    pub fn record(&self) -> &PidRecord {
        &self.record
    }
}

impl Drop for PidFileGuard {
    fn drop(&mut self) {
        if !self.holds_claim {
            // An unclaimed boot owns nothing on disk: the live claimant's
            // record must survive this incarnation's exit untouched.
            return;
        }
        // The compare-and-delete runs under the mutation lock: by the time
        // this guard drops, the listeners are closed and a successor can be
        // mid-claim, and an unlocked read-then-unlink could delete the
        // successor's freshly renamed record. If the lock cannot be taken,
        // the file is LEFT IN PLACE rather than removed without proof of
        // exclusivity — `aion server stop` and the next boot both reconcile
        // a leftover record as stale, while a deleted live record is a
        // running server no verb can address.
        let mutation_lock = match lock_pid_mutation(&self.path) {
            Ok(lock) => lock,
            Err(error) => {
                warn!(
                    path = %self.path.display(),
                    %error,
                    "could not take the pid mutation lock on exit; leaving the pid \
                     file for stale reconciliation"
                );
                return;
            }
        };
        let current = std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|content| serde_json::from_str::<PidRecord>(&content).ok());
        match current {
            Some(record) if record == self.record => {
                if let Err(io_error) = std::fs::remove_file(&self.path) {
                    warn!(
                        path = %self.path.display(),
                        %io_error,
                        "could not remove the pid file on exit; `aion server stop` \
                         and the next boot both reconcile it as stale"
                    );
                } else {
                    info!(path = %self.path.display(), "pid file removed on exit");
                }
            }
            Some(_) => {
                info!(
                    path = %self.path.display(),
                    "pid file now holds a different incarnation's record; leaving it"
                );
            }
            None => {}
        }
        release_pid_mutation(mutation_lock, &self.path);
    }
}

/// Build the [`ServerError`] for a pid-file failure.
fn pid_file_error(message: impl Into<String>) -> ServerError {
    ServerError::PidFile {
        message: message.into(),
    }
}

#[cfg(test)]
#[path = "pid_file_tests.rs"]
mod tests;
