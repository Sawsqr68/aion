//! Incarnation identity: proving that the process wearing a recorded pid is
//! the server that recorded it.
//!
//! A pid alone is a name the operating system recycles; signalling by bare
//! pid is how a stop verb kills a stranger. The discriminator is the process
//! START INSTANT, read through the same instrument ([`sysinfo`]) on both the
//! write side (the server records its own start instant at boot) and the
//! verify side (the stop/status verbs read the live process table), so the
//! comparison is exact equality on one instrument's answer — never clock
//! arithmetic between two instruments. A reused pid wears a different start
//! instant; a dead pid wears none.

use std::path::PathBuf;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

use super::pid_file::PidRecord;
use crate::error::ServerError;

/// What this process knows about itself at boot: the fields of the pid
/// record that come from the process table and the binary on disk.
#[derive(Clone, Debug)]
pub struct SelfIdentity {
    /// This process's id.
    pub pid: u32,
    /// This process's start instant in whole seconds since the Unix epoch,
    /// as the process-table instrument reports it.
    pub started_at_unix_secs: u64,
    /// SHA-256 of the executable's bytes at the path the OS reports for
    /// this process.
    pub binary_sha256: String,
}

/// The live process table's answer about a recorded incarnation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IncarnationProbe {
    /// No process wears the recorded pid: the recorded server is gone.
    ProcessGone,
    /// A process wears the pid and its start instant MATCHES the record:
    /// this is the recorded server, still running.
    Verified {
        /// Executable path the process table reports for the live process,
        /// carried for the report; `None` when the table does not know it.
        exe: Option<PathBuf>,
    },
    /// A process wears the pid but its start instant does not match the
    /// record: the pid was recycled onto a different process after the
    /// recorded server died. Never signal it.
    DifferentIncarnation {
        /// Start instant of the live process now wearing the pid.
        live_started_at_unix_secs: u64,
        /// Executable path of the live process, for the refusal report.
        exe: Option<PathBuf>,
    },
}

/// Read this process's own identity at boot.
///
/// # Errors
///
/// Returns [`ServerError::PidFile`] when the process table does not answer
/// for our own pid (which would leave the pid record unable to carry the
/// start instant every later verification depends on), when the executable
/// path cannot be resolved, or when the binary cannot be read for hashing.
pub fn self_identity() -> Result<SelfIdentity, ServerError> {
    let pid = std::process::id();
    let started_at_unix_secs = process_start_instant(pid).ok_or_else(|| ServerError::PidFile {
        message: format!(
            "the process table has no entry for this process (pid {pid}); \
                 cannot record a verifiable incarnation"
        ),
    })?;
    let exe = std::env::current_exe().map_err(|io_error| ServerError::PidFile {
        message: format!("cannot resolve this process's executable path: {io_error}"),
    })?;
    let bytes = std::fs::read(&exe).map_err(|io_error| ServerError::PidFile {
        message: format!(
            "cannot read this process's executable `{}` for content hashing: {io_error}",
            exe.display()
        ),
    })?;
    Ok(SelfIdentity {
        pid,
        started_at_unix_secs,
        binary_sha256: sha256_hex(&bytes),
    })
}

/// Probe the live process table for the incarnation a record names.
#[must_use]
pub fn probe(record: &PidRecord) -> IncarnationProbe {
    let mut system = System::new();
    let pid = sysinfo::Pid::from_u32(record.pid);
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().with_exe(sysinfo::UpdateKind::Always),
    );
    let Some(process) = system.process(pid) else {
        return IncarnationProbe::ProcessGone;
    };
    let live_started_at_unix_secs = process.start_time();
    let exe = process.exe().map(std::path::Path::to_path_buf);
    if live_started_at_unix_secs == record.started_at_unix_secs {
        IncarnationProbe::Verified { exe }
    } else {
        IncarnationProbe::DifferentIncarnation {
            live_started_at_unix_secs,
            exe,
        }
    }
}

/// The start instant the process table reports for `pid`, or `None` when no
/// such process exists.
fn process_start_instant(pid: u32) -> Option<u64> {
    let mut system = System::new();
    let pid = sysinfo::Pid::from_u32(pid);
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing(),
    );
    system.process(pid).map(sysinfo::Process::start_time)
}

/// Lowercase hex SHA-256 of `bytes`.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = sha2::Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push(char::from(HEX[usize::from(byte >> 4)]));
        hex.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::{IncarnationProbe, probe, self_identity};
    use crate::control::pid_file::PidRecord;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn record_with(
        pid: u32,
        started_at_unix_secs: u64,
    ) -> Result<PidRecord, std::net::AddrParseError> {
        Ok(PidRecord {
            pid,
            started_at_unix_secs,
            binary_sha256: "0".repeat(64),
            version: "0.0.0-test".to_owned(),
            commit: "test".to_owned(),
            http_address: "127.0.0.1:0".parse()?,
            grpc_address: "127.0.0.1:0".parse()?,
        })
    }

    /// The green path proven against the ONLY process a test can trust to
    /// exist: itself. Self-identity and a probe of the resulting record must
    /// agree — the same-instrument property the module exists for.
    #[test]
    fn a_record_of_this_process_verifies() -> TestResult {
        let me = self_identity()?;
        let record = record_with(me.pid, me.started_at_unix_secs)?;
        assert!(
            matches!(probe(&record), IncarnationProbe::Verified { .. }),
            "this process's own record must verify"
        );
        Ok(())
    }

    /// Red first: the SAME live pid with a different recorded start instant
    /// is a different incarnation — the reused-pid face, which must never
    /// verify and must carry the live instant for the refusal report.
    #[test]
    fn a_wrong_start_instant_on_a_live_pid_is_a_different_incarnation() -> TestResult {
        let me = self_identity()?;
        let record = record_with(me.pid, me.started_at_unix_secs.wrapping_add(7))?;
        match probe(&record) {
            IncarnationProbe::DifferentIncarnation {
                live_started_at_unix_secs,
                ..
            } => {
                assert_eq!(
                    live_started_at_unix_secs, me.started_at_unix_secs,
                    "the refusal must carry the LIVE process's instant"
                );
            }
            other => {
                return Err(format!(
                    "a mismatched start instant must read as a different incarnation, got {other:?}"
                )
                .into());
            }
        }
        Ok(())
    }

    /// Red first: a pid nothing wears is `ProcessGone`. Probing our own pid
    /// space for a vacancy is inherently racy, so the specimen takes the
    /// maximum pid-adjacent value a real allocator will not have handed out
    /// during the test's lifetime — and asserts only the gone/alive axis.
    #[test]
    fn a_vacant_pid_is_process_gone() -> TestResult {
        // Spawn a child and wait it to completion: its pid is proven
        // vacated by the reap, not guessed.
        let child = std::process::Command::new("true").spawn();
        let mut child = match child {
            Ok(child) => child,
            Err(io_error) => return Err(format!("cannot spawn probe child: {io_error}").into()),
        };
        let pid = child.id();
        child.wait()?;
        let record = record_with(pid, 1)?;
        // The reaped pid COULD have been recycled between wait and probe; a
        // recycled wearer has a different start instant, so both honest
        // answers are accepted — the specimen refuses only a false Verified.
        match probe(&record) {
            IncarnationProbe::ProcessGone | IncarnationProbe::DifferentIncarnation { .. } => Ok(()),
            IncarnationProbe::Verified { .. } => {
                Err("a reaped child's record must never verify".into())
            }
        }
    }
}
