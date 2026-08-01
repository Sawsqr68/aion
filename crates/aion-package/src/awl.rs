//! The AWL provenance a `.aion` archive can carry: the authored document and
//! the schema files that document imports.
//!
//! # Archive layout
//!
//! Two entry families share the `awl/` prefix, and both are rooted at the
//! DOCUMENT'S OWN DIRECTORY — the same root an AWL checker resolves
//! `schema("…")` imports against:
//!
//! ```text
//! awl/document/<filename>          exactly one entry; the authored `.awl` file
//! awl/schema/<relative path>       zero or more; each imported schema file
//! ```
//!
//! The document entry keeps the original filename verbatim, and every schema
//! entry keeps the document-relative path the document itself names, nesting
//! included. That is what lets a consumer stage the pair back onto a
//! filesystem and re-check the document exactly as it was checked when it was
//! deployed: the source's `schema("schemas/brief.schema.json")` line resolves
//! against the staged tree unchanged.
//!
//! # This is provenance, never identity
//!
//! An archived document does NOT participate in package identity. Two
//! packages whose beams, manifest, and contract agree have the SAME
//! [`crate::ContentHash`] whether one carries AWL source and the other does
//! not, and whether their carried source differs. See [`crate::hash`] for the
//! identity law this is a declared instance of; it is pinned by
//! `awl_source_inclusion_does_not_change_manifest_version` in
//! `crate::builder`.

use std::collections::BTreeMap;

pub(crate) const AWL_DOCUMENT_PREFIX: &str = "awl/document/";
pub(crate) const AWL_SCHEMA_PREFIX: &str = "awl/schema/";

/// The authored AWL document an archive was built from, with the schema files
/// it imports.
///
/// The document is text (an AWL document is UTF-8 by construction — the lexer
/// works on `&str`), so a consumer can parse it without re-validating an
/// encoding. Schema files stay bytes: they are staged back onto a filesystem
/// verbatim, and re-encoding them would change what the checker reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwlSource {
    document_name: String,
    document: String,
    schemas: BTreeMap<String, Vec<u8>>,
}

impl AwlSource {
    /// Creates an AWL provenance record from the document's original filename,
    /// its source text, and its imported schema files keyed by
    /// document-relative path.
    ///
    /// Names are validated when the archive is written, not here: an
    /// unrepresentable name yields [`crate::PackageError::MalformedAwlEntry`]
    /// from the write path, exactly as an unrepresentable module name does.
    #[must_use]
    pub fn new<I, N, B>(
        document_name: impl Into<String>,
        document: impl Into<String>,
        schemas: I,
    ) -> Self
    where
        I: IntoIterator<Item = (N, B)>,
        N: Into<String>,
        B: Into<Vec<u8>>,
    {
        Self {
            document_name: document_name.into(),
            document: document.into(),
            schemas: schemas
                .into_iter()
                .map(|(name, bytes)| (name.into(), bytes.into()))
                .collect(),
        }
    }

    /// Returns the document's original filename, extension included.
    #[must_use]
    pub fn document_name(&self) -> &str {
        &self.document_name
    }

    /// Returns the authored AWL source text, byte-identical to what was
    /// compiled.
    #[must_use]
    pub fn document(&self) -> &str {
        &self.document
    }

    /// Returns the imported schema files keyed by their document-relative
    /// path, which is the path to stage each file at before re-checking the
    /// document.
    #[must_use]
    pub const fn schemas(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.schemas
    }
}
