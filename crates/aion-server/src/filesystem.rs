//! Descriptor-relative, no-follow filesystem operations for sensitive server roots.
//!
//! Every path component is opened from a held directory capability. Symlinks and
//! Windows reparse points are refused at component boundaries, and final files are
//! opened with no-follow semantics. A concurrent local actor may replace a name
//! after validation, but the operation remains relative to an already-open parent
//! descriptor: it can redirect a name within that held directory, never expand the
//! operation's authority beyond the configured root.
//!
//! These roots are Aion's own, so Aion provisions them: a missing root is
//! created owner-only in a single `mkdir`, and an existing root the server's
//! user owns is tightened to owner-only rather than refused. The operator is
//! never handed a `chmod` command for a directory the server could fix itself.
//! What Aion cannot make safe — a foreign owner, a symlinked component, a
//! filesystem without Unix modes, or an unsafe ANCESTOR (see `ancestors`) —
//! still refuses, naming the path, the problem, and the remedy.

use std::ffi::{OsStr, OsString};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
mod ancestors;
#[cfg(target_os = "macos")]
mod darwin_acl;

#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
pub(crate) use ancestors::{DataRootAncestorError, validate_ambient_backend_ancestors};

use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, DirBuilder, OpenOptions};
#[cfg(unix)]
use cap_std::fs::{DirBuilderExt, MetadataExt as _, OpenOptionsExt, PermissionsExt};
#[cfg(all(unix, not(target_os = "macos")))]
use std::os::fd::AsRawFd as _;
#[cfg(target_os = "macos")]
use std::os::unix::ffi::OsStringExt as _;

/// Owner-only mode for every directory Aion owns. This is a security constant,
/// not a tunable: these roots hold workflow payloads, signal arguments, and
/// authored source, so group and world get nothing. It is never read from
/// configuration and there is no override.
const PRIVATE_DIR_MODE: u32 = 0o700;

/// Owner-only mode for every file Aion writes under those roots. Same reasoning,
/// same absence of an override.
const PRIVATE_FILE_MODE: u32 = 0o600;

/// A held directory descriptor confining all subsequent operations beneath it.
pub(crate) struct ConfinedDir {
    dir: Dir,
}

impl ConfinedDir {
    /// Open an existing real directory without following any path component,
    /// bringing it to owner-only mode if it is ours and too permissive.
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        let root = Self {
            dir: open_absolute(path, false)?,
        };
        root.ensure_private_mode(path)?;
        root.probe_readable()?;
        Ok(root)
    }

    /// Open or create a real directory, creating every missing component
    /// privately and bringing an existing root to owner-only mode.
    pub(crate) fn open_or_create(path: &Path) -> io::Result<Self> {
        let root = Self {
            dir: open_absolute(path, true)?,
        };
        root.ensure_private_mode(path)?;
        root.probe_readable()?;
        Ok(root)
    }

    /// Refuse at acquisition a root the server cannot actually read.
    ///
    /// Openability is not readability: on Linux the held handle is an
    /// `O_PATH` descriptor and opens even a directory whose mode denies every
    /// read, so without this probe an unreadable root is "successfully"
    /// confined and the `EACCES` surfaces later, at whichever operation first
    /// reads through the handle — or never, when a request happens not to
    /// need the directory's contents (aion#62). macOS refuses the same
    /// directory at `open(2)`. Probing the entry stream here makes an
    /// unusable root fail on every platform at the same place, with its
    /// truthful errno.
    fn probe_readable(&self) -> io::Result<()> {
        self.dir.entries().map(drop)
    }

    /// Read a UTF-8 file without following any component or final symlink.
    pub(crate) fn read_to_string(&self, relative: &Path) -> io::Result<String> {
        let mut file = self.open_file(relative, false)?;
        let mut value = String::new();
        file.read_to_string(&mut value)?;
        Ok(value)
    }

    /// Read a file without following any component or final symlink.
    pub(crate) fn read(&self, relative: &Path) -> io::Result<Vec<u8>> {
        let mut file = self.open_file(relative, false)?;
        let mut value = Vec::new();
        file.read_to_end(&mut value)?;
        Ok(value)
    }

    /// Create a new private file, refusing an existing file or link.
    pub(crate) fn create_new(&self, relative: &Path, bytes: &[u8]) -> io::Result<()> {
        let (parent, name) = self.open_parent(relative, true)?;
        let mut options = private_file_options();
        options.write(true).create_new(true);
        let mut file = parent.open_with(name, &options)?;
        if let Err(error) = write_and_sync(&mut file, bytes) {
            drop(file);
            let _ = parent.remove_file(name);
            return Err(error);
        }
        Ok(())
    }

    /// Atomically replace a private file using an unpredictable `create_new`
    /// temporary in the same held parent directory.
    pub(crate) fn atomic_write(&self, relative: &Path, bytes: &[u8]) -> io::Result<()> {
        let (parent, name) = self.open_parent(relative, true)?;
        match parent.symlink_metadata(name) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "refusing to replace a symbolic link",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }

        let temp_name = OsString::from(format!(".aion-{}.tmp", uuid::Uuid::new_v4()));
        let mut options = private_file_options();
        options.write(true).create_new(true);
        let mut temp = parent.open_with(&temp_name, &options)?;
        if let Err(error) = write_and_sync(&mut temp, bytes) {
            drop(temp);
            let _ = parent.remove_file(&temp_name);
            return Err(error);
        }
        drop(temp);
        if let Err(error) = parent.rename(&temp_name, &parent, name) {
            let _ = parent.remove_file(&temp_name);
            return Err(error);
        }
        Ok(())
    }

    /// Remove a file relative to this capability, without traversing parents.
    pub(crate) fn remove_file(&self, relative: &Path) -> io::Result<()> {
        let (parent, name) = self.open_parent(relative, false)?;
        parent.remove_file(name)
    }

    /// Recursively list `.awl` files while refusing directory links.
    pub(crate) fn list_awl(&self) -> io::Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        visit_awl(&self.dir, Path::new(""), &mut paths)?;
        Ok(paths)
    }

    /// Eagerly create a descendant directory through this capability.
    pub(crate) fn create_dir_all(&self, relative: &Path) -> io::Result<()> {
        drop(self.open_dir(relative, true)?);
        Ok(())
    }

    /// Return the narrowest path bridge the platform offers from this held
    /// descriptor to a backend that accepts only `PathBuf`.
    ///
    /// Linux/Android can traverse descendants through `/proc/self/fd`, so the
    /// returned path remains descriptor-authoritative. macOS and other Unix
    /// targets expose a directory descriptor in `/dev/fd` but do not permit
    /// descendant traversal through that name; there we resolve the descriptor's
    /// current real path immediately before backend startup. Normal backend reads
    /// and commits remain ambient pathname operations after startup on those
    /// platforms, so callers must reject renameable ancestor chains until
    /// Haematite exposes descriptor-relative I/O.
    #[cfg(unix)]
    pub(crate) fn backend_path(&self) -> io::Result<PathBuf> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            // The descriptor bridge only exists where procfs is mounted and
            // readable — containers with a hidden or absent /proc would
            // otherwise hand the backend a path that resolves to nothing.
            // This probe is the arm's real fallibility (aion#59): without it
            // the compiled Linux body was infallible inside the `io::Result`
            // the macOS arm genuinely needs, and the workspace was
            // clippy-red on Linux (`unnecessary_wraps`).
            let bridge = Path::new("/proc/self/fd").join(self.dir.as_raw_fd().to_string());
            std::fs::symlink_metadata(&bridge)?;
            Ok(bridge)
        }
        #[cfg(target_os = "macos")]
        {
            let path = rustix::fs::getpath(&self.dir)?;
            Ok(PathBuf::from(OsString::from_vec(path.into_bytes())))
        }
        #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
        {
            std::fs::canonicalize(Path::new("/dev/fd").join(self.dir.as_raw_fd().to_string()))
        }
    }

    /// Return metadata for this held root directory.
    pub(crate) fn metadata(&self) -> io::Result<cap_std::fs::Metadata> {
        self.dir.dir_metadata()
    }

    /// Apply private modes to every existing descendant without following links.
    /// On non-Unix targets this performs no ACL mutation. Startup permits that
    /// limitation only for roots the operator explicitly configured and emits a
    /// warning that ACL privacy was not verified.
    pub(crate) fn harden_tree(&self) -> io::Result<()> {
        #[cfg(unix)]
        harden_dir(&self.dir)?;
        Ok(())
    }

    /// Bring the held root to owner-only mode, tightening it ourselves when it
    /// is ours to tighten.
    ///
    /// The operator must never be told to go and hand-run `chmod 700` on a
    /// directory Aion created, or would have created, before Aion will start.
    /// That refusal was a real and repeated defect: a `~/.aion` left at 0755 by
    /// a permissive umask stopped a stock server, and the only cure the message
    /// offered was a shell command the server could have run itself. Loosening
    /// the requirement would have been the wrong fix — the directory holds
    /// workflow payloads — so the server now performs the repair instead.
    ///
    /// Every decision is taken against the ALREADY-OPEN descriptor: `fstat` on
    /// the held fd, and the repair addressed through the held capability
    /// (descriptor-relative `.`), never a second resolution of the ambient
    /// `path`. That closes the check-then-act window — there is no interval in
    /// which a concurrent rename could point the inspection at one inode and
    /// the repair at another, because both resolve from the same held
    /// descriptor. `path` is carried purely to make the log and error text
    /// nameable.
    ///
    /// The repair is cap-std's `set_permissions`, not a raw `fchmod` on the
    /// handle: on Linux the held directory handle is an `O_PATH` descriptor,
    /// and `fchmod(2)` refuses those with `EBADF` — which left every
    /// tightenable 0755 root refused on Linux while macOS repaired it
    /// (aion#62). cap-std performs the descriptor-relative mode change
    /// portably on both.
    ///
    /// Refusal is reserved for what Aion genuinely cannot repair: a directory
    /// owned by another principal (not ours to change, and a mode change would
    /// fail anyway), and a filesystem that will not carry Unix modes.
    fn ensure_private_mode(&self, path: &Path) -> io::Result<()> {
        #[cfg(unix)]
        {
            let metadata = self.metadata()?;
            let mode = metadata.permissions().mode() & 0o777;
            let grants_group_or_world = mode & 0o077 != 0;
            if !grants_group_or_world {
                return Ok(());
            }

            let owner = metadata.uid();
            let effective = rustix::process::geteuid().as_raw();
            if owner != effective {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "sensitive root `{}` has mode {mode:04o}, which grants group or world \
                         access, and is owned by uid {owner} rather than the uid {effective} this \
                         server runs as. Aion will not change another principal's directory. \
                         Either run the server as uid {owner}, or point this root at a directory \
                         owned by uid {effective}.",
                        path.display()
                    ),
                ));
            }

            self.dir
                .set_permissions(
                    Path::new("."),
                    cap_std::fs::Permissions::from_mode(PRIVATE_DIR_MODE),
                )
                .map_err(|error| {
                    io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        format!(
                            "sensitive root `{}` has mode {mode:04o}, which grants group or world \
                             access, and Aion could not tighten it to 0700: {error}. Move this \
                             root onto a filesystem that carries Unix permissions, or pre-create \
                             it with mode 0700.",
                            path.display()
                        ),
                    )
                })?;

            // Re-stat the same descriptor rather than assume the write took.
            // Filesystems exist that accept a mode change and discard it
            // (network and FAT-family mounts among them); without this the
            // server would
            // log that it had made the root private while the payloads stayed
            // world-readable.
            let applied = self.metadata()?.permissions().mode() & 0o777;
            if applied & 0o077 != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "sensitive root `{}` still reports mode {applied:04o} after Aion set it \
                         to 0700, so this filesystem does not honour Unix permissions and Aion \
                         cannot keep workflow state private here. Move this root onto a \
                         filesystem that does.",
                        path.display()
                    ),
                ));
            }

            let previous_mode = format!("{mode:04o}");
            let applied_mode = format!("{PRIVATE_DIR_MODE:04o}");
            tracing::info!(
                sensitive_root = %path.display(),
                %previous_mode,
                %applied_mode,
                "tightened a sensitive root to owner-only: it granted group or world access and \
                 the server's own user owns it"
            );
        }
        #[cfg(not(unix))]
        let _ = path;
        Ok(())
    }

    fn open_file(&self, relative: &Path, create_parents: bool) -> io::Result<cap_std::fs::File> {
        let (parent, name) = self.open_parent(relative, create_parents)?;
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        parent.open_with(name, &options)
    }

    fn open_parent<'a>(&self, relative: &'a Path, create: bool) -> io::Result<(Dir, &'a OsStr)> {
        validate_relative(relative)?;
        let name = relative.file_name().ok_or_else(invalid_relative)?;
        let parent = relative.parent().unwrap_or_else(|| Path::new(""));
        self.open_dir(parent, create).map(|dir| (dir, name))
    }

    fn open_dir(&self, relative: &Path, create: bool) -> io::Result<Dir> {
        validate_relative_or_empty(relative)?;
        let mut current = self.dir.try_clone()?;
        for component in relative.components() {
            let Component::Normal(name) = component else {
                return Err(invalid_relative());
            };
            current = open_child_dir(&current, name, create)?;
        }
        Ok(current)
    }
}

fn open_absolute(path: &Path, create: bool) -> io::Result<Dir> {
    let absolute = std::path::absolute(path)?;
    let (anchor, names) = split_absolute(&absolute)?;
    let mut current = Dir::open_ambient_dir(&anchor, ambient_authority())?;
    for (index, name) in names.into_iter().enumerate() {
        match open_child_dir(&current, &name, create) {
            Ok(child) => current = child,
            Err(error) if index == 0 => {
                // macOS exposes root-owned compatibility aliases such as
                // `/var -> /private/var`. Following only a filesystem-root
                // entry preserves those platform paths; every user-controlled
                // component below it remains descriptor-relative and no-follow.
                let alias = anchor.join(&name);
                let metadata = std::fs::symlink_metadata(&alias)?;
                if !metadata.file_type().is_symlink() {
                    return Err(component_error(&name, &error));
                }
                let canonical = std::fs::canonicalize(alias)?;
                current = Dir::open_ambient_dir(canonical, ambient_authority())?;
            }
            Err(error) => return Err(component_error(&name, &error)),
        }
    }
    Ok(current)
}

fn split_absolute(path: &Path) -> io::Result<(PathBuf, Vec<OsString>)> {
    let mut anchor = PathBuf::new();
    let mut names = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => anchor.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if names.pop().is_none() {
                    return Err(invalid_relative());
                }
            }
            Component::Normal(name) => names.push(name.to_owned()),
        }
    }
    if anchor.as_os_str().is_empty() {
        return Err(invalid_relative());
    }
    Ok((anchor, names))
}

fn component_error(name: &OsStr, error: &io::Error) -> io::Error {
    io::Error::new(
        error.kind(),
        format!(
            "failed to open real directory component `{}`: {error}",
            name.to_string_lossy()
        ),
    )
}

fn open_child_dir(parent: &Dir, name: &OsStr, create: bool) -> io::Result<Dir> {
    match parent.open_dir_nofollow(name) {
        Ok(dir) => Ok(dir),
        Err(error) if create && error.kind() == io::ErrorKind::NotFound => {
            let mut builder = DirBuilder::new();
            #[cfg(unix)]
            builder.mode(PRIVATE_DIR_MODE);
            match parent.create_dir_with(name, &builder) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
            parent.open_dir_nofollow(name)
        }
        Err(error) => Err(error),
    }
}

fn private_file_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.follow(FollowSymlinks::No);
    #[cfg(unix)]
    options.mode(PRIVATE_FILE_MODE);
    options
}

fn write_and_sync(file: &mut cap_std::fs::File, bytes: &[u8]) -> io::Result<()> {
    file.write_all(bytes)?;
    file.sync_all()
}

fn visit_awl(dir: &Dir, relative: &Path, paths: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in dir.entries()? {
        let entry = entry?;
        let name = entry.file_name();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        let child_relative = relative.join(&name);
        if file_type.is_dir() {
            let child = dir.open_dir_nofollow(&name)?;
            visit_awl(&child, &child_relative, paths)?;
        } else if file_type.is_file() && child_relative.extension() == Some(OsStr::new("awl")) {
            paths.push(child_relative);
        }
    }
    Ok(())
}

#[cfg(unix)]
fn harden_dir(dir: &Dir) -> io::Result<()> {
    dir.set_permissions(
        Path::new("."),
        cap_std::fs::Permissions::from_mode(PRIVATE_DIR_MODE),
    )?;
    for entry in dir.entries()? {
        let entry = entry?;
        let name = entry.file_name();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "sensitive state contains symbolic link `{}`",
                    name.to_string_lossy()
                ),
            ));
        }
        if file_type.is_dir() {
            let child = dir.open_dir_nofollow(&name)?;
            harden_dir(&child)?;
        } else if file_type.is_file() {
            let mut options = OpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            let file = dir.open_with(&name, &options)?;
            file.set_permissions(cap_std::fs::Permissions::from_mode(PRIVATE_FILE_MODE))?;
        }
    }
    Ok(())
}

fn validate_relative(path: &Path) -> io::Result<()> {
    if path.as_os_str().is_empty() {
        return Err(invalid_relative());
    }
    validate_relative_or_empty(path)
}

fn validate_relative_or_empty(path: &Path) -> io::Result<()> {
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid_relative());
    }
    Ok(())
}

fn invalid_relative() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "path must be relative and contain only normal components",
    )
}

#[cfg(all(test, target_os = "macos"))]
pub(crate) fn darwin_user_uuid_for_test(uid: u32) -> io::Result<uuid::Uuid> {
    darwin_acl::user_uuid_for_test(uid)
}

/// Verify that an existing sensitive root is a real directory, on the targets
/// where Aion cannot express or install an owner-only ACL.
///
/// Unix does not use this: there, [`ConfinedDir::open_or_create`] both creates
/// the root privately and repairs an existing loose one, so there is nothing
/// left for a separate pathname inspection to do. Non-Unix targets have no mode
/// Aion can set, so the configuration boundary additionally refuses default
/// roots, requires the operator to pre-provision and name the directory, and
/// warns that ACL privacy is not verified.
#[cfg(not(unix))]
pub(crate) fn validate_real_directory_root(path: &Path, label: &str) -> io::Result<()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} `{}` is not a real directory", path.display()),
        ));
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    #[test]
    fn nested_sensitive_roots_and_files_ignore_a_permissive_umask()
    -> Result<(), Box<dyn std::error::Error>> {
        const PROBE: &str = "AION_PRIVATE_MODE_UMASK_PROBE";
        if let Some(path) = std::env::var_os(PROBE) {
            return assert_private_creation(Path::new(&path));
        }

        let sandbox = crate::test_support::private_tempdir()?;
        let executable = std::env::current_exe()?;
        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(
                "umask 000; exec \"$1\" --exact \
                 filesystem::tests::nested_sensitive_roots_and_files_ignore_a_permissive_umask \
                 --nocapture",
            )
            .arg("aion-private-mode-probe")
            .arg(executable)
            .env(PROBE, sandbox.path())
            .status()?;
        assert!(status.success(), "private-mode umask probe failed");
        Ok(())
    }

    fn assert_private_creation(sandbox: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let home = sandbox.join("aion-home");
        let authoring = home.join("authoring");
        let root = ConfinedDir::open_or_create(&authoring)?;
        root.create_new(Path::new("private.txt"), b"secret")?;
        assert_eq!(
            std::fs::metadata(&home)?.permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(&authoring)?.permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(authoring.join("private.txt"))?
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        Ok(())
    }

    /// A root that does not exist yet is created owner-only, in one step. The
    /// umask probe above proves the mode comes from the `mkdir` itself rather
    /// than a follow-up `chmod`, so there is no window in which the directory
    /// exists while still group-readable.
    #[test]
    fn a_missing_root_is_created_owner_only() -> Result<(), Box<dyn std::error::Error>> {
        let sandbox = crate::test_support::private_tempdir()?;
        let root = sandbox.path().join("nested").join("aion-home");

        let (captured, opened) = crate::test_support::CapturedLogs::capture(|| {
            ConfinedDir::open_or_create(&root).map(drop)
        });
        opened?;

        assert_eq!(
            std::fs::metadata(&root)?.permissions().mode() & 0o777,
            0o700
        );
        assert!(
            !captured.text()?.contains("tightened a sensitive root"),
            "a freshly created root must not need tightening"
        );
        Ok(())
    }

    /// The defect this whole surface exists to close: a `~/.aion` left at 0755
    /// by a conventional umask used to refuse startup and hand the operator a
    /// `chmod` command. It is our directory and our user owns it, so we fix it.
    #[test]
    fn a_permissive_root_we_own_is_tightened_and_logged() -> Result<(), Box<dyn std::error::Error>>
    {
        let sandbox = crate::test_support::private_tempdir()?;
        let root = sandbox.path().join("aion-home");
        std::fs::create_dir(&root)?;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755))?;

        let (captured, opened) =
            crate::test_support::CapturedLogs::capture(|| ConfinedDir::open(&root).map(drop));
        opened?;

        assert_eq!(
            std::fs::metadata(&root)?.permissions().mode() & 0o777,
            0o700
        );
        let logs = captured.text()?;
        assert!(logs.contains("tightened a sensitive root to owner-only"));
        assert!(logs.contains(&root.display().to_string()));
        assert!(logs.contains("0755"), "the previous mode was not logged");
        assert!(logs.contains("0700"), "the applied mode was not logged");
        Ok(())
    }

    /// World-writable is the same defect one notch worse, and gets the same
    /// answer. The leaf being world-writable is repairable; only an unsafe
    /// ANCESTOR is not (pinned in `ancestors::tests`).
    #[test]
    fn a_world_writable_root_we_own_is_tightened() -> Result<(), Box<dyn std::error::Error>> {
        let sandbox = crate::test_support::private_tempdir()?;
        let root = sandbox.path().join("aion-data");
        std::fs::create_dir(&root)?;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o777))?;

        ConfinedDir::open_or_create(&root)?;

        assert_eq!(
            std::fs::metadata(&root)?.permissions().mode() & 0o777,
            0o700
        );
        Ok(())
    }

    /// An already-private root is left exactly as found, and says nothing.
    #[test]
    fn an_already_private_root_is_untouched_and_silent() -> Result<(), Box<dyn std::error::Error>> {
        let sandbox = crate::test_support::private_tempdir()?;
        let root = sandbox.path().join("aion-home");
        std::fs::create_dir(&root)?;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))?;

        let (captured, opened) =
            crate::test_support::CapturedLogs::capture(|| ConfinedDir::open(&root).map(drop));
        opened?;

        assert_eq!(
            std::fs::metadata(&root)?.permissions().mode() & 0o777,
            0o700
        );
        assert!(captured.text()?.is_empty());
        Ok(())
    }

    /// A permissive root owned by someone else is refused, not repaired: it is
    /// not ours to change, and the mode change would fail regardless. The refusal has
    /// to carry the path, the mode, both uids, and what the operator can do.
    ///
    /// `/usr` is a stable stand-in for "root-owned and group/world readable" on
    /// every Unix. The euid guard is not decoration: running as root, the
    /// ownership branch would not fire and the test would try to tighten a
    /// system directory. Gated at runtime rather than with `#[ignore]` so the
    /// skip is visible in the log.
    #[test]
    fn a_permissive_root_owned_by_another_user_refuses_with_remediation()
    -> Result<(), Box<dyn std::error::Error>> {
        let effective = rustix::process::geteuid().as_raw();
        if effective == 0 {
            tracing::info!(
                "skipping the foreign-owner refusal pin: running as root, which owns every \
                 candidate directory"
            );
            return Ok(());
        }
        let foreign = Path::new("/usr");
        let metadata = std::fs::symlink_metadata(foreign)?;
        let owner = std::os::unix::fs::MetadataExt::uid(&metadata);
        let mode = metadata.permissions().mode() & 0o777;
        let grants_group_or_world = mode & 0o077 != 0;
        if owner == effective || !grants_group_or_world {
            tracing::info!(
                path = %foreign.display(),
                "skipping the foreign-owner refusal pin: this system's /usr is not a \
                 foreign-owned, group/world-readable directory"
            );
            return Ok(());
        }

        let error = ConfinedDir::open(foreign)
            .err()
            .ok_or("a permissive foreign-owned root was accepted")?;
        let message = error.to_string();
        assert!(message.contains("/usr"), "the path was not named");
        assert!(
            message.contains(&format!("mode {mode:04o}")),
            "the offending mode was not named"
        );
        assert!(message.contains(&format!("uid {owner}")));
        assert!(message.contains(&format!("uid {effective}")));
        assert!(message.contains("run the server as"), "no remediation");
        assert_eq!(
            std::fs::symlink_metadata(foreign)?.permissions().mode() & 0o777,
            mode,
            "a foreign-owned directory must never be modified"
        );
        Ok(())
    }

    /// A symlinked root refuses rather than resolving. Aion's privacy claim is
    /// about an inode it opened by no-follow walk; honouring a link would let
    /// whoever can write the link's parent redirect the entire root, and
    /// tightening the link's target would be modifying a directory we were
    /// never pointed at directly.
    #[test]
    fn a_symlinked_root_refuses_and_leaves_its_target_alone()
    -> Result<(), Box<dyn std::error::Error>> {
        let sandbox = crate::test_support::private_tempdir()?;
        let target = sandbox.path().join("elsewhere");
        let link = sandbox.path().join("aion-home");
        std::fs::create_dir(&target)?;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))?;
        std::os::unix::fs::symlink(&target, &link)?;

        let error = ConfinedDir::open(&link)
            .err()
            .ok_or("a symlinked root was accepted")?;
        assert!(
            error.to_string().contains("aion-home"),
            "the refusal did not name the offending component"
        );
        assert!(ConfinedDir::open_or_create(&link).is_err());
        assert_eq!(
            std::fs::metadata(&target)?.permissions().mode() & 0o777,
            0o755,
            "a symlink target must never be tightened"
        );
        Ok(())
    }

    /// A root whose name is taken by a file is refused, not silently replaced.
    #[test]
    fn a_root_occupied_by_a_file_refuses() -> Result<(), Box<dyn std::error::Error>> {
        let sandbox = crate::test_support::private_tempdir()?;
        let occupied = sandbox.path().join("aion-home");
        std::fs::write(&occupied, b"not a directory")?;

        let error = ConfinedDir::open_or_create(&occupied)
            .err()
            .ok_or("a file standing in for a root was accepted")?;
        assert!(error.to_string().contains("aion-home"));
        Ok(())
    }
}
