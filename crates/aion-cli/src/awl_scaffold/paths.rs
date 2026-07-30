//! How the generated crate spells the paths it must reach.
//!
//! The generated `src/declaration.rs` embeds the document with `include_str!`
//! and resolves its schema-import root from `CARGO_MANIFEST_DIR`, and a
//! path-sourced `Cargo.toml` reaches the aion crates the same way.
//!
//! WITHIN ONE TREE THE LINK IS RELATIVE. A scaffold generated beside its
//! document stays buildable when the tree is moved, cloned, or checked out
//! somewhere else, which an absolute path burned in at generation time would
//! not.
//!
//! ACROSS UNRELATED TREES IT IS ABSOLUTE. When the only ancestor two paths
//! share is the filesystem root, they are not one tree and no relative path
//! between them means anything — it would be a `../../../../..` chain that
//! encodes the depth of a directory nobody chose. This is a structural test,
//! not a length threshold: either the two paths share a real directory or they
//! do not.
//!
//! The output directory may not exist yet, so paths are normalised LEXICALLY
//! (`.` dropped, `..` popped) rather than canonicalised. Where a path traverses
//! a symlink the two differ — and the difference surfaces as a `cargo build`
//! failure naming the file that could not be found, never as a silently wrong
//! document.

use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

/// How the generated crate reaches something: from where it stands, or from
/// the root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Link {
    /// A path relative to the directory it is written into, `/`-separated.
    Relative(String),
    /// An absolute path, for a target in an unrelated tree.
    Absolute(String),
}

impl Link {
    /// The path text as the generated source spells it.
    pub(super) fn text(&self) -> &str {
        match self {
            Self::Relative(path) | Self::Absolute(path) => path,
        }
    }
}

/// How a file written into `from_dir` reaches `to`.
///
/// # Errors
///
/// Fails when either path cannot be made absolute, climbs above the
/// filesystem root, or is not valid UTF-8.
pub(super) fn link(from_dir: &Path, to: &Path) -> Result<Link> {
    let from = absolute_lexical(from_dir)?;
    let target = absolute_lexical(to)?;
    let from_parts: Vec<Component<'_>> = from.components().collect();
    let target_parts: Vec<Component<'_>> = target.components().collect();
    let shared = from_parts
        .iter()
        .zip(&target_parts)
        .take_while(|(left, right)| left == right)
        .count();
    let shares_a_directory = from_parts[..shared]
        .iter()
        .any(|component| matches!(component, Component::Normal(_)));
    if !shares_a_directory {
        // Nothing but the filesystem root in common: two unrelated trees,
        // where a relative path would encode an accident of depth.
        return Ok(Link::Absolute(text(&target)?));
    }
    let mut parts: Vec<String> = vec!["..".to_owned(); from_parts.len() - shared];
    for component in &target_parts[shared..] {
        parts.push(component_text(*component, &target)?);
    }
    if parts.is_empty() {
        return Ok(Link::Relative(".".to_owned()));
    }
    Ok(Link::Relative(parts.join("/")))
}

/// A whole path as text, refusing one that is not UTF-8: it could not be
/// written into Rust or TOML source.
fn text(path: &Path) -> Result<String> {
    path.to_str().map(str::to_owned).with_context(|| {
        format!(
            "`{}` is not valid UTF-8, so it cannot be written into the generated crate",
            path.display()
        )
    })
}

/// One path component as text, refusing a component that is not UTF-8.
fn component_text(component: Component<'_>, whole: &Path) -> Result<String> {
    component
        .as_os_str()
        .to_str()
        .map(str::to_owned)
        .with_context(|| {
            format!(
                "`{}` is not valid UTF-8, so it cannot be written into the generated crate",
                whole.display()
            )
        })
}

/// Makes `path` absolute and removes `.` and `..` components lexically.
///
/// Lexical, not canonical: the generated crate's directory routinely does not
/// exist yet, and `canonicalize` cannot resolve a path that is not there.
fn absolute_lexical(path: &Path) -> Result<PathBuf> {
    let absolute = std::path::absolute(path).with_context(|| {
        format!(
            "could not resolve `{}` against the working directory",
            path.display()
        )
    })?;
    let mut normalised = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalised.pop() {
                    bail!("`{}` climbs above the filesystem root", absolute.display());
                }
            }
            other => normalised.push(other.as_os_str()),
        }
    }
    Ok(normalised)
}
