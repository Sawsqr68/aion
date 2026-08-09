//! Which executable a builtin deployment launches, and what it is.
//!
//! `WorkerArtifactRef::Builtin { verb }` names the server's OWN binary plus an
//! operator-declared argv tail. The tail is the whole launch contract: nothing
//! here appends a flag, an endpoint, or an environment variable the operator
//! did not write. An inferred argument is an argument nobody can read off the
//! deployment record, and this record is meant to be the answer to "what will
//! actually run".
//!
//! Identity is captured at EVERY spawn, not once at deploy: that is what makes
//! a restart across an upgrade measurable rather than assumed.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::build_identity::BuildIdentity;

use super::error::SupervisionError;
use super::status::SpawnedBinary;

/// The executable a managed worker instance launches.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManagedExecutable {
    /// This server's own binary, resolved through the operating system at each
    /// spawn. The production choice, and the only one a `builtin` artifact
    /// reference can mean.
    CurrentServer,
    /// An explicit executable path. The seam supervision tests drive, so a test
    /// never risks re-executing the test binary itself.
    Path(PathBuf),
}

impl ManagedExecutable {
    /// Resolve the path to launch.
    ///
    /// # Errors
    ///
    /// Returns [`SupervisionError::ExecutableUnresolved`] when the operating
    /// system cannot identify this server's own executable.
    pub fn resolve(&self) -> Result<PathBuf, SupervisionError> {
        match self {
            Self::CurrentServer => {
                std::env::current_exe().map_err(|source| SupervisionError::ExecutableUnresolved {
                    detail: source.to_string(),
                })
            }
            Self::Path(path) => Ok(path.clone()),
        }
    }

    /// Capture the identity of `path` as this spawn will run it.
    ///
    /// The build stamp is reported only for this server's own binary: it is the
    /// only executable whose version and commit this process actually knows.
    ///
    /// # Errors
    ///
    /// Returns [`SupervisionError::ExecutableUnreadable`] when the bytes cannot
    /// be read, which is also the honest report for a deployment naming a path
    /// that no longer exists.
    pub fn identify(&self, path: &Path) -> Result<SpawnedBinary, SupervisionError> {
        let bytes =
            std::fs::read(path).map_err(|source| SupervisionError::ExecutableUnreadable {
                path: path.display().to_string(),
                detail: source.to_string(),
            })?;
        let content_hash = lowercase_hex(&Sha256::digest(bytes));
        let stamp = match self {
            Self::CurrentServer => Some(BuildIdentity::current()),
            Self::Path(_) => None,
        };
        Ok(SpawnedBinary {
            path: path.display().to_string(),
            content_hash,
            version: stamp.as_ref().map(|identity| identity.version.to_owned()),
            commit: stamp.as_ref().map(|identity| identity.commit.to_owned()),
            dirty: stamp.as_ref().map(|identity| identity.dirty.to_owned()),
        })
    }
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::ManagedExecutable;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// The hash is of the bytes on disk at the moment of capture, so rewriting
    /// the file between spawns changes it. That is the mechanism the
    /// "restart across an upgrade is visible" ruling rests on; if the hash were
    /// cached, an upgrade would report the old identity and read as calm.
    #[test]
    fn identity_follows_the_bytes_on_disk_at_each_capture() -> TestResult {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("worker");
        let mut file = std::fs::File::create(&path)?;
        file.write_all(b"first build")?;
        drop(file);

        let executable = ManagedExecutable::Path(path.clone());
        let first = executable.identify(&path)?;

        std::fs::write(&path, b"second build")?;
        let second = executable.identify(&path)?;

        assert_ne!(first.content_hash, second.content_hash);
        assert_eq!(first.path, path.display().to_string());
        // An explicit path carries no build stamp: this process cannot know one.
        assert_eq!(first.version, None);
        assert_eq!(first.commit, None);
        assert_eq!(first.dirty, None);
        Ok(())
    }

    #[test]
    fn a_missing_executable_is_a_typed_unreadable_refusal() -> TestResult {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("absent");
        let error = ManagedExecutable::Path(path.clone())
            .identify(&path)
            .err()
            .ok_or("identifying an absent executable must fail")?;
        assert!(
            error.to_string().contains(&path.display().to_string()),
            "the refusal must name the path it could not read: {error}"
        );
        Ok(())
    }

    #[test]
    fn the_server_binary_carries_its_build_stamp() -> TestResult {
        let executable = ManagedExecutable::CurrentServer;
        let path = executable.resolve()?;
        let identity = executable.identify(&path)?;
        assert!(identity.version.is_some());
        assert!(identity.commit.is_some());
        assert!(identity.dirty.is_some());
        assert!(!identity.content_hash.is_empty());
        Ok(())
    }
}
