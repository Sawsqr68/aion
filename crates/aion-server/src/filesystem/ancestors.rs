//! The path-ambient backend's ancestor-chain gate.
//!
//! This is deliberately separate from the [`super::ConfinedDir`] capability.
//! `ConfinedDir` governs a root Aion OWNS and may therefore repair; this module
//! governs the chain of directories ABOVE that root, which Aion does not own and
//! must never modify. The only correct answer to an unsafe ancestor is to refuse
//! and say precisely which component is wrong.

use std::path::{Path, PathBuf};

/// An unsafe component in a path-ambient backend's resolved ancestor chain.
#[derive(Debug)]
pub(crate) struct DataRootAncestorError {
    component: PathBuf,
    reason: String,
}

impl DataRootAncestorError {
    pub(super) fn new(component: PathBuf, reason: impl Into<String>) -> Self {
        Self {
            component,
            reason: reason.into(),
        }
    }

    pub(crate) fn into_parts(self) -> (PathBuf, String) {
        (self.component, self.reason)
    }
}

impl std::fmt::Display for DataRootAncestorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "ancestor `{}` is not owner-controlled: {}",
            self.component.display(),
            self.reason
        )
    }
}

impl std::error::Error for DataRootAncestorError {}

/// Require every component of a path-ambient Haematite root to be controlled by
/// the server's effective user (or by root for the immutable system prefix) and
/// to deny group/world writes. On macOS, each component's extended ACL is read
/// without following its final name. An allow ACE for any principal other than
/// the effective user is refused when it grants directory traversal, entry
/// creation/removal, child deletion, or ACL/owner mutation. Deny ACEs remain
/// acceptable because they cannot confer the rename authority this gate removes;
/// in particular, this admits the stock macOS home `everyone deny delete` ACE.
///
/// This forecloses another principal renaming a parent after startup, replacing
/// the old name with a symlink, and redirecting Haematite's ambient `DiskStore`
/// reads and commits. Linux and Android are exempt because their
/// `/proc/self/fd/N/...` backend path stays descriptor-authoritative while the
/// retained directory fd lives. Descriptor-relative backend I/O is the long-term
/// Haematite-side fix; until then, Unix platforms without traversable procfs fd
/// paths must fail closed on a renameable ancestor. Sticky directories such as
/// `/tmp` are intentionally refused because the sticky bit does not make an
/// ambient child pathname descriptor-authoritative. A macOS ACL that cannot be
/// read or interpreted is likewise refused rather than treated as absent.
///
/// Nothing here is ever auto-repaired. Aion does not own `/`, `/private`, or a
/// shared `/tmp`, and tightening someone else's directory would be both futile
/// (it usually fails) and hostile (it would break every other user of that
/// path). A failure of this gate is genuinely an operator decision about WHERE
/// the data root lives, so the refusal names the exact offending component.
pub(crate) fn validate_ambient_backend_ancestors(
    data_root: &Path,
) -> Result<(), DataRootAncestorError> {
    use std::os::unix::fs::MetadataExt as _;

    if !data_root.is_absolute() {
        return Err(DataRootAncestorError::new(
            data_root.to_path_buf(),
            "descriptor-resolved backend path is not absolute",
        ));
    }

    let effective_uid = rustix::process::geteuid().as_raw();
    let mut component_path = PathBuf::new();
    for component in data_root.components() {
        component_path.push(component.as_os_str());
        let metadata = std::fs::symlink_metadata(&component_path).map_err(|error| {
            DataRootAncestorError::new(
                component_path.clone(),
                format!("could not inspect ownership and mode: {error}"),
            )
        })?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(DataRootAncestorError::new(
                component_path,
                "component is not a real directory",
            ));
        }

        let mode = metadata.mode() & 0o7777;
        if mode & 0o022 != 0 {
            return Err(DataRootAncestorError::new(
                component_path,
                format!(
                    "mode {mode:04o} grants group/world write access (sticky bit is not accepted)"
                ),
            ));
        }

        let owner_uid = metadata.uid();
        if owner_uid != effective_uid && owner_uid != 0 {
            return Err(DataRootAncestorError::new(
                component_path,
                format!("owner uid {owner_uid} is neither server euid {effective_uid} nor root"),
            ));
        }

        #[cfg(target_os = "macos")]
        super::darwin_acl::validate_extended_acl(&component_path, effective_uid)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression pin. A world-writable ancestor is the one permission failure
    /// Aion must NOT repair on the operator's behalf: the leaf could be made
    /// 0700 and the data would still be exposed, because anyone can rename the
    /// parent out from under the running backend. Auto-tightening sensitive
    /// roots must never soften this.
    #[test]
    fn a_world_writable_ancestor_is_still_refused() -> Result<(), Box<dyn std::error::Error>> {
        let sandbox = crate::test_support::private_tempdir()?;
        let shared = sandbox.path().join("shared");
        std::fs::create_dir(&shared)?;
        std::fs::set_permissions(
            &shared,
            std::os::unix::fs::PermissionsExt::from_mode(0o1777),
        )?;
        let data_root = shared.join("aion-data");
        std::fs::create_dir(&data_root)?;
        std::fs::set_permissions(
            &data_root,
            std::os::unix::fs::PermissionsExt::from_mode(0o700),
        )?;

        // Production passes the descriptor-resolved backend path, which is
        // already canonical; a raw temporary path would trip the symlink refusal
        // at `/var` on macOS long before reaching the case under test.
        let data_root = std::fs::canonicalize(&data_root)?;
        let shared = std::fs::canonicalize(&shared)?;

        let error = validate_ambient_backend_ancestors(&data_root)
            .err()
            .ok_or("a world-writable ancestor was accepted")?;
        let (component, reason) = error.into_parts();
        assert_eq!(component, shared);
        assert!(reason.contains("1777"), "reason did not state the mode");
        assert!(reason.contains("group/world write"));
        Ok(())
    }

    /// The exact shape Tom hit: a data root parked under the shared system
    /// temporary directory. `/private/tmp` is mode 1777 and root-owned, and no
    /// amount of chmod on the leaf makes it safe.
    #[cfg(target_os = "macos")]
    #[test]
    fn the_shared_system_temp_directory_is_still_refused() -> Result<(), Box<dyn std::error::Error>>
    {
        let error = validate_ambient_backend_ancestors(Path::new("/private/tmp/aion-data"))
            .err()
            .ok_or("a data root under /private/tmp was accepted")?;
        let (component, reason) = error.into_parts();
        assert_eq!(component, Path::new("/private/tmp"));
        assert!(reason.contains("group/world write"));
        Ok(())
    }

    #[test]
    fn a_relative_backend_path_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        let error = validate_ambient_backend_ancestors(Path::new("aion-data"))
            .err()
            .ok_or("a relative backend path was accepted")?;
        let (_, reason) = error.into_parts();
        assert!(reason.contains("not absolute"));
        Ok(())
    }
}
