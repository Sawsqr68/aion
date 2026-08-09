//! Wire types and the typed failure set for the deployed-AWL read surface.

use serde::Serialize;

/// One deployed workflow version, with the honest state of its archived AWL
/// source.
///
/// The listing is the UNION of two sets that are not the same set: the
/// engine's loaded-version catalog, and the archives the store persists.
/// A version can be in either without being in the other — an operator-file
/// startup package is loaded and never persisted; a persisted archive that
/// failed to reload is persisted and not loaded — and collapsing them would
/// hide exactly the states an operator needs to see.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeployedVersion {
    /// Logical workflow type this version is registered under.
    pub workflow_type: String,
    /// Canonical 64-hex content hash identifying the version.
    pub content_hash: String,
    /// Whether the engine currently holds this version in its catalog.
    pub loaded: bool,
    /// Whether new dispatches of this type currently route to this version.
    /// Always false for a version the engine does not hold.
    pub route_active: bool,
    /// RFC 3339 instant the engine loaded this version, when it holds it.
    pub loaded_at: Option<String>,
    /// RFC 3339 instant the archive row was persisted, when one exists.
    pub deployed_at: Option<String>,
    /// What the deployed archive carries by way of authored AWL source.
    pub source: DeployedSourceState,
}

/// The four states a deployed version's archived AWL source can be in.
///
/// Absence is first class in three distinguishable forms, because the remedies
/// differ: a package with no AWL source was authored another way (or predates
/// source archiving) and nothing is wrong; a version with no persisted archive
/// is an operator-file load whose bytes the server never held; an archive that
/// will not read back is a fault that must be named, not rendered as "no
/// source".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DeployedSourceState {
    /// The persisted archive carries the authored AWL document.
    Available {
        /// The document's original filename, extension included.
        document_name: String,
        /// How many schema files the document imported and the archive carries.
        schema_count: usize,
    },
    /// The persisted archive read back cleanly and carries no AWL document:
    /// a Gleam-authored package, or one deployed before source archiving.
    Absent,
    /// The engine holds this version but the store persists no archive for it
    /// (an operator-file startup package), so its source cannot be read at all.
    NotPersisted,
    /// An archive is persisted but could not be read back; the reason is
    /// carried verbatim rather than collapsed into "no source".
    Unreadable {
        /// The package error, verbatim.
        reason: String,
    },
}

/// The archived AWL document of one deployed version, with its schema imports
/// and the projection the studio renders.
///
/// # No `queries` field, deliberately
///
/// The `/assistant` descriptor serves query names because it derives them from
/// the document it embeds and executes. This surface cannot make the same
/// claim: the archived AWL source is PROVENANCE, not identity —
/// [`aion_package::PackageBuilder::with_awl_source`] cannot change the package
/// version hash — so a query list derived from that source would not be
/// attributable to `content_hash`, while the contract-committed `input_schema`
/// and `signals` are. The field's absence therefore means "not reported,
/// because not attributable to this version", never "the document declares no
/// queries"; a consumer that needs the declared queries reads them from
/// `source` on its own authority.
#[derive(Debug, Serialize)]
pub struct DeployedDocument {
    /// Logical workflow type the requested version is registered under.
    pub workflow_type: String,
    /// Canonical 64-hex content hash identifying the version.
    pub content_hash: String,
    /// The document's original filename, extension included.
    pub document_name: String,
    /// The authored AWL source, byte-identical to what the archive carries.
    pub source: String,
    /// The schema files the document imports, keyed by the document-relative
    /// path the source itself names.
    pub schemas: Vec<DeployedSchema>,
    /// The start input schema of `workflow_type`, draft 2020-12, read from the
    /// archive's identity-committed contract ([`aion_package::Package::contract`]
    /// — the same seam the `/assistant` descriptor derives its form surface
    /// from), so a console can build a start form for this exact version.
    ///
    /// `None` exactly when the archived contract genuinely carries none: the
    /// package's stored identity predates contract commitment (a legacy
    /// pre-contract deploy, which [`aion_package::Package::contract`] refuses
    /// with `RedeployRequired`), or — defence in depth the deploy path never
    /// produces — the committed contract does not schema the requested entry.
    pub input_schema: Option<serde_json::Value>,
    /// The signals the archived contract commits to `workflow_type`, each with
    /// its payload schema.
    ///
    /// Attributed with the same standard as `input_schema` — only what the
    /// version hash commits to THIS entry:
    /// - `Some(signals)` (possibly empty — the contract genuinely commits
    ///   none) when `workflow_type` is the archive's primary entry, whose
    ///   contract the signal declarations belong to.
    /// - `None` when the contract commits no signal set for this entry: an
    ///   additional workflow entry
    ///   ([`aion_package::AdditionalWorkflowContract`] carries no signals), or
    ///   a pre-contract package (the case `input_schema` also reports as
    ///   `None`). Absent is not empty: a console must not offer the primary's
    ///   signals to an entry whose contract never accepted them.
    pub signals: Option<Vec<DeployedSignal>>,
    /// The check/semantic projection of `source`, resolved against `schemas`.
    ///
    /// Computed here rather than by a second call to `/awl/check`, because
    /// that endpoint resolves `schema("…")` imports against the OPERATOR'S
    /// workspace: a deployed document checked through it would be rendered
    /// from deployed source and workspace schemas, which is a claim about an
    /// artifact that does not exist.
    pub projection: super::super::CheckResponse,
}

/// One declared signal of a deployed version, from its archived contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeployedSignal {
    /// Signal name.
    pub name: String,
    /// Payload schema, draft 2020-12, as committed into the version identity.
    pub input_schema: serde_json::Value,
}

/// One archived schema file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeployedSchema {
    /// The document-relative path the document's `schema("…")` line names.
    pub path: String,
    /// The file's text, or `None` when the archived bytes are not valid UTF-8.
    ///
    /// A schema that reached the archive passed the checker, which reads schema
    /// files as text, so `None` is unreachable through the deploy path — it is
    /// represented rather than unwrapped so a corrupt archive states what it is
    /// instead of failing the whole document or silently serving nothing.
    pub text: Option<String>,
    /// The archived file's length in bytes, whatever its encoding.
    pub byte_length: usize,
}

/// Everything the deployed-AWL read surface can refuse or fail with.
#[derive(Debug, thiserror::Error)]
pub enum DeployedError {
    /// No persisted archive registers the requested `(type, version)` pair.
    #[error(
        "no deployed package version `{content_hash}` of workflow type `{workflow_type}` is \
         persisted on this server"
    )]
    NotFound {
        /// The requested workflow type.
        workflow_type: String,
        /// The requested content hash.
        content_hash: String,
    },
    /// The version exists and carries no archived AWL document.
    #[error(
        "deployed package version `{content_hash}` of workflow type `{workflow_type}` carries no \
         archived AWL source: it was authored in Gleam, or deployed before deploys archived their \
         source"
    )]
    NoArchivedSource {
        /// The requested workflow type.
        workflow_type: String,
        /// The requested content hash.
        content_hash: String,
    },
    /// A persisted archive could not be read back.
    #[error(
        "the persisted archive for workflow type `{workflow_type}` version `{content_hash}` could \
         not be read: {reason}"
    )]
    Unreadable {
        /// The requested workflow type.
        workflow_type: String,
        /// The requested content hash.
        content_hash: String,
        /// The package error, verbatim.
        reason: String,
    },
    /// An archived schema entry names a path that cannot be staged.
    ///
    /// The archive writer already refuses such names, so this is defence in
    /// depth over bytes that reach a filesystem: it refuses the document rather
    /// than staging a path that escapes the staging directory.
    #[error(
        "archived schema path `{path}` is not a relative in-document path, so the deployed \
         document cannot be checked"
    )]
    UnsafeSchemaPath {
        /// The offending archive-relative path.
        path: String,
    },
    /// Staging the archived schemas for the checker failed.
    #[error("staging the deployed document's archived schemas failed: {0}")]
    Staging(#[from] std::io::Error),
    /// The deployed-package catalog or archive set could not be read.
    #[error("the deployed package catalog could not be read: {0}")]
    Catalog(String),
}
