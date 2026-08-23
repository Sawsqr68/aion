//! The working tree the front reads through, and the receipt's measurement
//! of it.
//!
//! This is one of the two ports the language front declares and deliberately
//! does not implement. Every address a declaration names is resolved against
//! ONE root and is refused if it escapes it — a census that could be pointed
//! at `../../etc` would measure a tree nobody declared.

use std::path::{Path, PathBuf};

use aion_awl::command::recipe::{ArtifactSource, MeasuredTree, ReceiptError, SourceUnreadable};

/// A real working tree, rooted.
pub(crate) struct WorkingTree {
    root: PathBuf,
}

impl WorkingTree {
    /// Root the tree at `root`.
    pub(crate) fn rooted(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    /// Resolve `address` inside the root, refusing anything that escapes it.
    fn resolve(&self, address: &str) -> Result<PathBuf, SourceUnreadable> {
        let candidate = self.root.join(address);
        // Lexical containment: `..` in a declared address is refused rather
        // than normalised away, because an address that climbs out of the
        // root names bytes the declaration did not.
        if Path::new(address).is_absolute()
            || Path::new(address)
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(SourceUnreadable::Refused {
                address: address.to_owned(),
                detail: format!(
                    "the address leaves the root {}; a declared artifact names bytes inside the \
                     tree the recipe was declared against",
                    self.root.display()
                ),
            });
        }
        Ok(candidate)
    }

    /// Measure the tree this run is about to read: its head, its path, and
    /// whether it is clean.
    ///
    /// # Errors
    ///
    /// Returns [`ReceiptError`] when the measurement cannot name what it
    /// measured — which is the one state a receipt may not be built in.
    pub(crate) fn measure(&self) -> Result<MeasuredTree, ReceiptError> {
        let head = git(&self.root, &["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".to_owned());
        let clean = git(&self.root, &["status", "--porcelain"])
            .is_some_and(|status| status.trim().is_empty());
        MeasuredTree::new(head, self.root.display().to_string(), clean)
    }
}

impl ArtifactSource for WorkingTree {
    fn read(&self, address: &str) -> Result<Vec<u8>, SourceUnreadable> {
        let path = self.resolve(address)?;
        if !path.exists() {
            return Err(SourceUnreadable::Absent {
                address: address.to_owned(),
            });
        }
        std::fs::read(&path).map_err(|error| SourceUnreadable::Refused {
            address: address.to_owned(),
            detail: error.to_string(),
        })
    }
}

/// One `git` question, answered as trimmed stdout, or `None` when git could
/// not answer it.
///
/// `None` rather than a guess: a head this cannot resolve is recorded as
/// `unknown` by the caller and a receipt saying `unknown` is honest, while a
/// receipt naming a head it invented is not.
fn git(root: &Path, arguments: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
