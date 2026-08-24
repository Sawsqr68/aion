//! The server's pid file: `<AION_HOME>/run/aion-server.pid`.
//!
//! The file is the stop/status verbs' address for the running server. It is
//! written only after BOTH transport listeners have bound successfully — a
//! boot that loses the port race must never overwrite the live server's
//! record — and removed on clean exit by [`PidFileGuard`], which deletes the
//! file only while it still holds this incarnation's record, so a guard
//! outliving its server can never destroy a successor's claim.
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
    /// server holding this home appears to still be running. The claim
    /// proceeds only because binding both listeners succeeded, which the
    /// recorded server could not have allowed while serving the same
    /// addresses — so the record is presumed left over from an incompletely
    /// observed exit and replaced, with the contradiction reported in full.
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
    let reconciliation = reconcile_existing(&path);
    report_reconciliation(&path, &reconciliation);
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
    match read(home)? {
        Some(current) if current == *record => {
            std::fs::remove_file(&path).map_err(|io_error| {
                pid_file_error(format!(
                    "could not remove pid file `{}`: {io_error}",
                    path.display()
                ))
            })?;
            Ok(true)
        }
        Some(_) | None => Ok(false),
    }
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
                "pid file names a LIVE matching incarnation, yet this boot bound the \
                 listeners — a contradiction (the recorded server cannot be serving \
                 these addresses). Replacing the record; if two servers were started \
                 on different addresses against one home, stop one of them"
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
