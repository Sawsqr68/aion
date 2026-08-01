//! Checking a deployed document against the schema files its own archive
//! carries.
//!
//! # Why the server projects, instead of the console re-checking
//!
//! `/awl/check` resolves a document's `schema("…")` imports against the
//! OPERATOR'S WORKSPACE. Sending deployed source through it would render a
//! composite that exists nowhere: deployed statements checked against whatever
//! files happen to sit beside a same-named workspace document. The archived
//! schemas are the only schemas that version was ever checked with, so the
//! projection is computed here, from them.
//!
//! # The staging directory
//!
//! The AWL checker resolves imports through the filesystem, so archived schema
//! bytes have to reach one. This module writes them into a process-private
//! temporary directory (0700, unique name) that is created inside this call and
//! removed before it returns. It holds only bytes that are being returned to
//! the caller in the same response. Nothing durable is created or modified:
//! not the operator's workspace, not the package store, no cache. That is the
//! whole of this surface's filesystem use, and
//! `deployed_projection_leaves_no_directory_behind` pins it.

use std::collections::BTreeMap;
use std::path::{Component, Path};

use super::super::handlers::{CheckResponse, check_source_at_root};
use super::types::DeployedError;

/// Projects `source`, resolving its schema imports against `schemas`.
///
/// A document that imports nothing needs no filesystem at all and is checked
/// directly.
///
/// # Errors
///
/// Returns [`DeployedError::UnsafeSchemaPath`] for an archived schema whose
/// path is not a relative in-document path, and [`DeployedError::Staging`]
/// when the staging directory cannot be created or written.
pub(super) fn project(
    source: &str,
    schemas: &BTreeMap<String, Vec<u8>>,
) -> Result<CheckResponse, DeployedError> {
    project_in(source, schemas, &std::env::temp_dir())
}

/// [`project`], with the directory the staging directory is created inside
/// stated explicitly.
///
/// Production passes the system temporary directory. The parameter exists so
/// the staging invariant can be pinned against a directory the pin owns
/// outright — a pin that scanned the shared system temp directory could not
/// tell this call's leavings from a concurrent one's.
///
/// # Errors
///
/// Returns [`DeployedError::UnsafeSchemaPath`] for an archived schema whose
/// path is not a relative in-document path, and [`DeployedError::Staging`]
/// when the staging directory cannot be created or written.
pub(super) fn project_in(
    source: &str,
    schemas: &BTreeMap<String, Vec<u8>>,
    staging_parent: &Path,
) -> Result<CheckResponse, DeployedError> {
    if schemas.is_empty() {
        return Ok(check_source_at_root(source, None));
    }
    let staging = tempfile::Builder::new()
        .prefix("aion-deployed-schema-")
        .tempdir_in(staging_parent)?;
    for (path, bytes) in schemas {
        let relative = relative_in_document(path)?;
        let staged = staging.path().join(relative);
        if let Some(parent) = staged.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(staged, bytes)?;
    }
    let response = check_source_at_root(source, Some(staging.path()));
    // Explicit, not incidental: the staging directory is gone before the
    // projection is handed back, so no caller can observe it and no failure
    // path can outlive it.
    staging.close()?;
    Ok(response)
}

/// Accepts only a non-empty relative path built from ordinary components.
///
/// The archive writer already refuses anything else, so this is defence in
/// depth at the point where archived names become filesystem paths: a `..` or
/// absolute component would place a staged file outside the staging directory.
fn relative_in_document(path: &str) -> Result<&Path, DeployedError> {
    let candidate = Path::new(path);
    if path.is_empty()
        || candidate
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(DeployedError::UnsafeSchemaPath {
            path: path.to_owned(),
        });
    }
    Ok(candidate)
}
