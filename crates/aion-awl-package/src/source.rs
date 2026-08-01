//! Collecting the AWL provenance an archive carries: the authored document
//! and every schema file it imports, read from the same root the checker
//! resolved those imports against.

use std::collections::BTreeMap;
use std::path::{Component, Path};

use aion_awl::{TypeBody, parse};
use aion_package::{AwlSource, builder::is_safe_logical_name};

/// A typed refusal from [`collect_awl_source`].
#[derive(Debug, thiserror::Error)]
pub enum AwlSourceError {
    /// The document does not parse, so its schema imports cannot be read.
    ///
    /// Unreachable on the compile-then-assemble path — the compiler parses
    /// first and refuses ahead of this — and reported rather than assumed away
    /// so a future caller that assembles unchecked source is not silently
    /// handed an archive with no schemas in it.
    #[error("the AWL document could not be parsed while collecting its source: {message}")]
    Parse {
        /// The parser's diagnostic, verbatim.
        message: String,
    },

    /// The document's filename cannot name an archive entry.
    #[error("document filename `{name}` cannot be archived as `awl/document/{name}`")]
    InvalidDocumentName {
        /// The refused filename.
        name: String,
    },

    /// A `schema(…)` import names a file the archive layout cannot hold.
    ///
    /// The `awl/` tree is rooted at the document's own directory, so an import
    /// that climbs above that root (or is absolute) has no document-relative
    /// path to be archived under. Refusing is the honest outcome: an archive
    /// that silently dropped the file would claim a provenance it does not
    /// carry, and a consumer re-checking the document would fail on a missing
    /// import with no explanation.
    #[error(
        "schema import `{path}` resolves outside the document's own directory and cannot be \
         archived with it; move the schema into the document's directory"
    )]
    UnarchivableImport {
        /// The refused import path, verbatim from the document.
        path: String,
    },

    /// An imported schema file could not be read from the document's root.
    #[error("failed to read imported schema `{path}`: {source}")]
    UnreadableImport {
        /// The import path, verbatim from the document.
        path: String,
        /// The filesystem failure.
        source: std::io::Error,
    },
}

/// Collects the archivable provenance of one AWL document: its source text
/// under its original filename, plus every `schema(…)` import it names, keyed
/// by the document-relative path a consumer must stage the file at.
///
/// `schema_root` is the document's own directory — the exact root
/// `aion_awl::compile` resolved the imports against — so the bytes archived
/// here are the bytes that were compiled.
///
/// # Errors
///
/// Returns [`AwlSourceError`] when the document does not parse, when its
/// filename or an import path cannot be represented as an archive entry, or
/// when an imported schema file cannot be read.
pub fn collect_awl_source(
    document_filename: &str,
    source: &str,
    schema_root: &Path,
) -> Result<AwlSource, AwlSourceError> {
    if document_filename.contains('/') || !is_safe_logical_name(document_filename) {
        return Err(AwlSourceError::InvalidDocumentName {
            name: document_filename.to_owned(),
        });
    }

    let document = parse(source).map_err(|error| AwlSourceError::Parse {
        message: error.message,
    })?;

    let mut schemas: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for declaration in &document.types {
        let TypeBody::SchemaImport { path, .. } = &declaration.body else {
            continue;
        };
        let entry_path = archive_relative_path(path)
            .ok_or_else(|| AwlSourceError::UnarchivableImport { path: path.clone() })?;
        if schemas.contains_key(&entry_path) {
            // Two declarations importing one file name the same archive entry
            // and the same bytes; reading it twice would change nothing.
            continue;
        }
        // Read through the LITERAL import path, exactly as the compiler did,
        // so the archived bytes are the compiled bytes even when the literal
        // and the normalised entry path differ (`./x.json` vs `x.json`).
        let bytes = std::fs::read(schema_root.join(path)).map_err(|source| {
            AwlSourceError::UnreadableImport {
                path: path.clone(),
                source,
            }
        })?;
        schemas.insert(entry_path, bytes);
    }

    Ok(AwlSource::new(document_filename, source, schemas))
}

/// Normalises an import path into the archive-relative path it is stored
/// under, or `None` when the layout cannot represent it.
///
/// `.` components are dropped (they resolve to the same file and are not
/// representable as an entry component). A `..`, root, or prefix component
/// leaves the document's directory, which the `awl/` tree has no root for.
fn archive_relative_path(path: &str) -> Option<String> {
    let mut components = Vec::new();
    for component in Path::new(path).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => components.push(part.to_str()?),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if components.is_empty() {
        return None;
    }
    let joined = components.join("/");
    if is_safe_logical_name(&joined) {
        Some(joined)
    } else {
        None
    }
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
