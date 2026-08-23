//! Stable out-of-worktree storage for spawn claims.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use aion_integrations::HarnessError;
use sha2::{Digest, Sha256};

pub(super) const CLAIMS_DIRECTORY: &str = "spawn-claims";

#[derive(Debug)]
pub(super) struct ClaimArea {
    pub(super) canonical_tree: PathBuf,
    pub(super) directory: PathBuf,
}

/// Resolve the claim directory for a worktree.
///
/// This mirrors `aion_server::config::home`: non-empty `AION_HOME` wins,
/// otherwise state lives below `$HOME/.aion`. The key is a pure SHA-256 of the
/// canonical tree path, so aliases and harness restarts address one claim.
pub(super) fn for_tree(tree: &Path) -> Result<ClaimArea, HarnessError> {
    let home = resolve_home(
        std::env::var_os("AION_HOME"),
        std::env::var_os("HOME"),
        std::env::current_dir().ok(),
    )?;
    for_tree_under(tree, &home)
}

pub(super) fn for_tree_under(tree: &Path, home: &Path) -> Result<ClaimArea, HarnessError> {
    let canonical_tree = std::fs::canonicalize(tree).map_err(|error| {
        HarnessError::configuration(format!(
            "could not canonicalize spawn-claim tree `{}`: {error}",
            tree.display()
        ))
    })?;
    Ok(ClaimArea {
        directory: home.join(CLAIMS_DIRECTORY).join(tree_key(&canonical_tree)),
        canonical_tree,
    })
}

fn resolve_home(
    aion_home: Option<OsString>,
    home: Option<OsString>,
    current: Option<PathBuf>,
) -> Result<PathBuf, HarnessError> {
    let path = match aion_home {
        Some(value) if value.is_empty() => {
            return Err(HarnessError::configuration("AION_HOME must not be empty"));
        }
        Some(value) => expand_tilde(Path::new(&value), home)?,
        None => home
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .map(|path| path.join(".aion"))
            .ok_or_else(|| {
                HarnessError::configuration("cannot resolve Aion home: set AION_HOME or HOME")
            })?,
    };
    if path.is_absolute() {
        Ok(path)
    } else {
        current.map(|cwd| cwd.join(path)).ok_or_else(|| {
            HarnessError::configuration(
                "cannot resolve relative AION_HOME against the current directory",
            )
        })
    }
}

fn expand_tilde(path: &Path, home: Option<OsString>) -> Result<PathBuf, HarnessError> {
    let mut components = path.components();
    let Some(Component::Normal(first)) = components.next() else {
        return Ok(path.to_owned());
    };
    if first == "~" {
        let home = home.filter(|value| !value.is_empty()).ok_or_else(|| {
            HarnessError::configuration(format!(
                "cannot expand `~` in path `{}`: HOME is not set",
                path.display()
            ))
        })?;
        let rest = components.as_path();
        return Ok(if rest.as_os_str().is_empty() {
            PathBuf::from(home)
        } else {
            PathBuf::from(home).join(rest)
        });
    }
    if first.as_encoded_bytes().starts_with(b"~") {
        return Err(HarnessError::configuration(format!(
            "cannot resolve `{}`: `~user` expansion is not supported",
            path.display()
        )));
    }
    Ok(path.to_owned())
}

fn tree_key(tree: &Path) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(tree.as_os_str().as_encoded_bytes());
    let mut key = String::with_capacity(digest.len() * 2);
    for byte in digest {
        key.push(char::from(HEX[usize::from(byte >> 4)]));
        key.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    key
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn aliases_and_restarts_derive_one_stable_claim_directory()
    -> Result<(), Box<dyn std::error::Error>> {
        // Fails if canonicalization is omitted, or if a pid/clock/random value
        // enters the key: aliases or separate harness processes then diverge.
        let scratch = tempdir()?;
        let real = scratch.path().join("real/tree");
        std::fs::create_dir_all(&real)?;
        let link = scratch.path().join("alias");
        #[cfg(unix)]
        std::os::unix::fs::symlink(scratch.path().join("real"), &link)?;
        assert!(link.is_symlink());
        let alias = link.join("tree/../tree/");
        assert_ne!(alias, std::fs::canonicalize(&alias)?);
        let one = for_tree(&real)?;
        let two = for_tree(&alias)?;
        assert_eq!(one.directory, two.directory);

        let exe = std::env::current_exe()?;
        let mut paths = Vec::new();
        let mut pids = Vec::new();
        for _ in 0..2 {
            let output = Command::new(&exe)
                .args([
                    "--exact",
                    "claim_area::tests::claim_path_probe",
                    "--nocapture",
                ])
                .env("AION_CLAIM_PROBE_TREE", &real)
                .env("AION_HOME", scratch.path().join("state"))
                .output()?;
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8(output.stdout)?;
            let probe_record = stdout
                .lines()
                .find(|line| line.starts_with("CLAIM_PROBE "))
                .ok_or("probe output absent")?;
            let mut fields = probe_record.splitn(3, ' ');
            let _ = fields.next();
            pids.push(fields.next().ok_or("probe pid absent")?.to_owned());
            paths.push(fields.next().ok_or("probe path absent")?.to_owned());
        }
        assert_ne!(pids[0], pids[1]);
        assert_eq!(paths[0], paths[1]);
        Ok(())
    }

    #[test]
    fn claim_path_probe() -> Result<(), Box<dyn std::error::Error>> {
        let Some(tree) = std::env::var_os("AION_CLAIM_PROBE_TREE") else {
            return Ok(());
        };
        let area = for_tree(Path::new(&tree))?;
        println!(
            "CLAIM_PROBE {} {}",
            std::process::id(),
            area.directory.display()
        );
        Ok(())
    }

    #[test]
    fn home_resolution_is_loud_and_matches_the_server_chain()
    -> Result<(), Box<dyn std::error::Error>> {
        // Fails if state falls back to cwd or temp when no home is configured.
        assert_eq!(
            resolve_home(None, Some(OsString::from("/users/probe")), None)?,
            Path::new("/users/probe/.aion")
        );
        assert_eq!(
            resolve_home(
                Some(OsString::from("~/state")),
                Some(OsString::from("/users/probe")),
                None,
            )?,
            Path::new("/users/probe/state")
        );
        assert!(resolve_home(None, None, None).is_err());
        assert!(resolve_home(Some(OsString::new()), None, None).is_err());
        Ok(())
    }

    #[test]
    fn key_is_sha256_hex_of_only_the_canonical_path() {
        let key = tree_key(Path::new("/canonical/tree"));
        assert_eq!(key.len(), 64);
        assert!(key.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(key, tree_key(Path::new("/canonical/tree")));
        assert_ne!(key, tree_key(Path::new("/canonical/other")));
    }
}
