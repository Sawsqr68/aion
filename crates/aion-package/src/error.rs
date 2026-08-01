//! Error taxonomy for malformed `.aion` packages.

use crate::awl::{AWL_DOCUMENT_PREFIX, AWL_SCHEMA_PREFIX};

/// Errors produced while validating or loading a `.aion` package.
#[derive(thiserror::Error, Debug)]
pub enum PackageError {
    /// The archive could not be read as a ZIP container.
    #[error("failed to read .aion ZIP archive: {0}")]
    ArchiveRead(#[from] zip::result::ZipError),

    /// The archive does not contain the required root manifest.
    #[error("missing required manifest.json entry")]
    MissingManifest,

    /// A new package builder has no contract record to bind into `.v4` identity.
    #[error("missing required contract record for .v4 package identity")]
    MissingContractRecord,

    /// A module uses a namespace owned by the engine's native NIF layer.
    #[error(
        "module `{module}` uses an engine-reserved namespace and must not ship as package bytecode"
    )]
    ReservedModuleName {
        /// The offending logical module name.
        module: String,
    },

    /// The archive could not be written as a ZIP container.
    #[error("failed to write .aion ZIP archive: {0}")]
    ArchiveWrite(zip::result::ZipError),

    /// The archive target could not be written to the filesystem or memory buffer.
    #[error("failed to write .aion archive bytes: {source}")]
    ArchiveWriteIo {
        /// I/O failure reported by the write target.
        source: std::io::Error,
    },

    /// The manifest entry is present but is not valid manifest JSON.
    #[error("failed to parse manifest.json: {source}")]
    ManifestParse {
        /// JSON parsing failure reported by `serde_json`.
        source: serde_json::Error,
    },

    /// The manifest could not be serialised for writing into the archive.
    #[error("failed to serialise manifest.json: {source}")]
    ManifestSerialise {
        /// JSON serialisation failure reported by `serde_json`.
        source: serde_json::Error,
    },

    /// The durable contract record is present but is not valid contract JSON.
    #[error("failed to parse contract.json: {source}")]
    ContractParse {
        /// JSON parsing failure reported by `serde_json`.
        source: serde_json::Error,
    },

    /// The durable contract record could not be serialised.
    #[error("failed to serialise contract.json: {source}")]
    ContractSerialise {
        /// JSON serialisation failure reported by `serde_json`.
        source: serde_json::Error,
    },

    /// The manifest declares a format version this crate does not support.
    #[error("unknown .aion format_version {found}")]
    UnknownFormatVersion {
        /// Unsupported format version found in the manifest.
        found: u32,
    },

    /// The manifest entry module is not present in the beam set.
    #[error("missing entry module `{module}` in beam set")]
    MissingEntryModule {
        /// Logical entry module named by the manifest.
        module: String,
    },

    /// The manifest version does not match the hash recomputed from beams.
    #[error("package integrity mismatch: expected version `{expected}`, computed `{computed}`")]
    IntegrityMismatch {
        /// Version claimed by the manifest.
        expected: String,
        /// Version recomputed from package beams.
        computed: String,
    },

    /// A beam archive entry is malformed or ambiguous.
    #[error("malformed beam entry `{entry}`")]
    MalformedBeamEntry {
        /// Archive entry or logical module name that failed validation.
        entry: String,
    },

    /// An `awl/` archive entry is malformed, duplicated, or names a family
    /// this format does not define.
    #[error("malformed AWL source entry `{entry}`")]
    MalformedAwlEntry {
        /// Archive entry or relative path that failed validation.
        entry: String,
    },

    /// The archived AWL document is not UTF-8 text.
    #[error("archived AWL document `{entry}` is not valid UTF-8: {source}")]
    AwlDocumentNotUtf8 {
        /// Archive entry that failed decoding.
        entry: String,
        /// Decoding failure reported by the standard library.
        source: std::string::FromUtf8Error,
    },

    /// The archive carries imported schema files with no document to own them,
    /// so the provenance it claims is incomplete.
    #[error("archive carries `{AWL_SCHEMA_PREFIX}` entries but no `{AWL_DOCUMENT_PREFIX}` entry")]
    MissingAwlDocument,

    /// The archive's entries inflate past the caller's extraction budget.
    #[error(
        "archive contents inflate past the extraction limit of {limit} bytes; refusing to extract further"
    )]
    InflatedSizeExceeded {
        /// The caller-configured inflate ceiling in bytes.
        limit: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::PackageError;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn package_error_is_send_and_sync() {
        assert_send_sync::<PackageError>();
    }

    #[test]
    fn display_messages_name_the_failed_condition() {
        assert_eq!(
            PackageError::MissingManifest.to_string(),
            "missing required manifest.json entry"
        );
        assert_eq!(
            PackageError::ArchiveWriteIo {
                source: std::io::Error::other("disk full"),
            }
            .to_string(),
            "failed to write .aion archive bytes: disk full"
        );
        assert_eq!(
            PackageError::UnknownFormatVersion { found: 99 }.to_string(),
            "unknown .aion format_version 99"
        );
        assert_eq!(
            PackageError::MissingEntryModule {
                module: "workflow/main".to_owned(),
            }
            .to_string(),
            "missing entry module `workflow/main` in beam set"
        );
        assert_eq!(
            PackageError::IntegrityMismatch {
                expected: "expected".to_owned(),
                computed: "computed".to_owned(),
            }
            .to_string(),
            "package integrity mismatch: expected version `expected`, computed `computed`"
        );
        assert_eq!(
            PackageError::MalformedBeamEntry {
                entry: "beam/workflow.beam".to_owned(),
            }
            .to_string(),
            "malformed beam entry `beam/workflow.beam`"
        );
        assert_eq!(
            PackageError::InflatedSizeExceeded { limit: 1024 }.to_string(),
            "archive contents inflate past the extraction limit of 1024 bytes; refusing to extract further"
        );
    }
}
