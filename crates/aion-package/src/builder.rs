//! `PackageBuilder` deterministic write path.
//!
//! # Archive layout
//!
//! Entries are written in one fixed order, so identical inputs produce
//! byte-identical archives:
//!
//! ```text
//! manifest.json                the deployment descriptor
//! contract.json                the durable worker contract, when one is bound
//! beam/<module>.beam           the compiled beam closure, canonical order
//! src/<module>.gleam           optional Gleam source
//! awl/document/<filename>      optional authored AWL document
//! awl/schema/<relative path>   the schema files that document imports
//! ```
//!
//! Only `manifest.json`, `contract.json`, and the `beam/` closure carry the
//! package's meaning to the engine. `src/` and `awl/` are PROVENANCE: they
//! record what the package was made from. Neither participates in package
//! identity — the content hash is computed over beams, routing/timeouts, and
//! the contract alone (see [`crate::hash`]), so adding, changing, or removing
//! archived source cannot change a package version. That non-participation is
//! a declared property of this format, pinned by
//! `source_inclusion_does_not_change_manifest_version` and
//! `awl_source_inclusion_does_not_change_manifest_version` below.

use std::{
    collections::BTreeMap,
    fs::File,
    io::{Cursor, Seek, Write},
    path::Path,
};

use zip::{CompressionMethod, DateTime, ZipWriter, write::SimpleFileOptions};

use crate::{
    AwlSource, BeamSet, ContentHash, Manifest, ManifestVersion, PackageContract, PackageError,
    awl::{AWL_DOCUMENT_PREFIX, AWL_SCHEMA_PREFIX},
    content_hash_with_contract,
};

const CONTRACT_ENTRY: &str = "contract.json";

/// Deterministic writer for the `.aion` ZIP container format.
#[derive(Clone, Debug)]
pub struct PackageBuilder {
    manifest: Manifest,
    beams: BeamSet,
    source: BTreeMap<String, Vec<u8>>,
    awl: Option<AwlSource>,
    contract: Option<PackageContract>,
    preserved_identity: Option<ContentHash>,
}

impl PackageBuilder {
    /// Creates a builder without source files.
    #[must_use]
    pub fn new(manifest: Manifest, beams: BeamSet) -> Self {
        let contract = PackageContract::from_manifest(&manifest);
        Self {
            manifest,
            beams,
            source: BTreeMap::new(),
            awl: None,
            contract: Some(contract),
            preserved_identity: None,
        }
    }

    /// Creates a builder with optional source files keyed by logical module name.
    #[must_use]
    pub fn with_source<I, N, B>(manifest: Manifest, beams: BeamSet, source: I) -> Self
    where
        I: IntoIterator<Item = (N, B)>,
        N: Into<String>,
        B: Into<Vec<u8>>,
    {
        let contract = PackageContract::from_manifest(&manifest);
        Self {
            manifest,
            beams,
            source: source
                .into_iter()
                .map(|(name, bytes)| (name.into(), bytes.into()))
                .collect(),
            awl: None,
            contract: Some(contract),
            preserved_identity: None,
        }
    }

    /// Replaces the manifest-derived record with the complete compiled contract.
    #[must_use]
    pub fn with_contract(mut self, contract: PackageContract) -> Self {
        self.contract = Some(contract);
        self
    }

    /// Archives the authored AWL document and its imported schema files as
    /// package provenance.
    ///
    /// Additive: it touches neither the manifest nor the contract, so it is
    /// safe in any order relative to [`Self::with_contract`], and it cannot
    /// change the package version (see this module's archive-layout note).
    #[must_use]
    pub fn with_awl_source(mut self, awl: AwlSource) -> Self {
        self.awl = Some(awl);
        self
    }

    pub(crate) fn preserving_loaded_identity(
        mut self,
        identity: ContentHash,
        contract: Option<PackageContract>,
    ) -> Self {
        self.preserved_identity = Some(identity);
        self.contract = contract;
        self
    }

    /// Returns the manifest after stamping the authoritative beam content hash.
    ///
    /// # Errors
    ///
    /// Returns [`PackageError::MissingEntryModule`] when the manifest entry module
    /// is not present in the supplied beam set.
    pub fn finalise_manifest(&self) -> Result<Manifest, PackageError> {
        self.stamped_manifest()
    }

    /// Writes a deterministic `.aion` archive into memory.
    ///
    /// # Errors
    ///
    /// Returns [`PackageError`] variants for missing entry modules, manifest JSON
    /// serialisation failures, ZIP writer failures, or target I/O failures.
    pub fn write_to_bytes(&self) -> Result<Vec<u8>, PackageError> {
        let cursor = Cursor::new(Vec::new());
        let (manifest_bytes, contract_bytes) = self.archive_metadata()?;
        let cursor = self.write_archive(cursor, &manifest_bytes, contract_bytes.as_deref())?;
        Ok(cursor.into_inner())
    }

    /// Writes a deterministic `.aion` archive to the supplied filesystem path.
    ///
    /// # Errors
    ///
    /// Returns [`PackageError`] variants for missing entry modules, manifest JSON
    /// serialisation failures, ZIP writer failures, or target I/O failures.
    pub fn write_to_path(&self, path: impl AsRef<Path>) -> Result<(), PackageError> {
        let (manifest_bytes, contract_bytes) = self.archive_metadata()?;
        let file = File::create(path).map_err(|source| PackageError::ArchiveWriteIo { source })?;
        self.write_archive(file, &manifest_bytes, contract_bytes.as_deref())?;
        Ok(())
    }

    fn archive_metadata(&self) -> Result<(Vec<u8>, Option<Vec<u8>>), PackageError> {
        let manifest = self.stamped_manifest()?;
        let manifest_bytes = serde_json::to_vec(&manifest)
            .map_err(|source| PackageError::ManifestSerialise { source })?;
        let contract_bytes = self
            .contract
            .as_ref()
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|source| PackageError::ContractSerialise { source })?;
        Ok((manifest_bytes, contract_bytes))
    }

    fn stamped_manifest(&self) -> Result<Manifest, PackageError> {
        if self.beams.get(&self.manifest.entry_module).is_none() {
            return Err(PackageError::MissingEntryModule {
                module: self.manifest.entry_module.clone(),
            });
        }

        let hash = match (&self.preserved_identity, &self.contract) {
            (Some(identity), _) => identity.clone(),
            (None, Some(contract)) => {
                content_hash_with_contract(&self.beams, &self.manifest, contract)
            }
            (None, None) => return Err(PackageError::MissingContractRecord),
        };
        let mut manifest = self.manifest.clone();
        manifest.version = ManifestVersion::new(hash.to_string());
        Ok(manifest)
    }

    fn write_archive<W>(
        &self,
        writer: W,
        manifest_bytes: &[u8],
        contract_bytes: Option<&[u8]>,
    ) -> Result<W, PackageError>
    where
        W: Write + Seek,
    {
        let mut archive = ZipWriter::new(writer);
        let options = deterministic_file_options();

        write_entry(&mut archive, "manifest.json", manifest_bytes, options)?;
        if let Some(contract_bytes) = contract_bytes {
            write_entry(&mut archive, CONTRACT_ENTRY, contract_bytes, options)?;
        }

        for module in self.beams.iter() {
            let entry_name = archive_entry_name("beam", module.name(), "beam")?;
            write_entry(&mut archive, entry_name, module.bytes(), options)?;
        }

        for (name, bytes) in &self.source {
            let entry_name = archive_entry_name("src", name, "gleam")?;
            write_entry(&mut archive, entry_name, bytes, options)?;
        }

        if let Some(awl) = &self.awl {
            let entry_name = awl_entry_name(AWL_DOCUMENT_PREFIX, awl.document_name())?;
            write_entry(&mut archive, entry_name, awl.document().as_bytes(), options)?;
            for (path, bytes) in awl.schemas() {
                let entry_name = awl_entry_name(AWL_SCHEMA_PREFIX, path)?;
                write_entry(&mut archive, entry_name, bytes, options)?;
            }
        }

        archive.finish().map_err(PackageError::ArchiveWrite)
    }
}

fn deterministic_file_options() -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .compression_level(None)
        .last_modified_time(DateTime::DEFAULT)
        .unix_permissions(0o644)
}

fn archive_entry_name(
    prefix: &str,
    logical_name: &str,
    extension: &str,
) -> Result<String, PackageError> {
    if is_safe_logical_name(logical_name) {
        Ok(format!("{prefix}/{logical_name}.{extension}"))
    } else {
        Err(PackageError::MalformedBeamEntry {
            entry: logical_name.to_owned(),
        })
    }
}

/// Builds an `awl/` entry name, keeping the supplied relative path verbatim —
/// extension and nesting included — because a consumer must be able to stage
/// the file back at exactly the path the document names.
fn awl_entry_name(prefix: &str, relative_path: &str) -> Result<String, PackageError> {
    if is_safe_logical_name(relative_path) {
        Ok(format!("{prefix}{relative_path}"))
    } else {
        Err(PackageError::MalformedAwlEntry {
            entry: relative_path.to_owned(),
        })
    }
}

/// Returns whether a logical module name can be represented safely as an
/// archive entry path.
#[must_use]
pub fn is_safe_logical_name(logical_name: &str) -> bool {
    !logical_name.is_empty()
        && !logical_name.starts_with('/')
        && !logical_name.starts_with('\\')
        && !logical_name.contains('\\')
        && !logical_name.contains(crate::namespace::DEPLOYED_NAME_SEPARATOR)
        && logical_name
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

fn write_entry<W>(
    archive: &mut ZipWriter<W>,
    name: impl ToString,
    bytes: &[u8],
    options: SimpleFileOptions,
) -> Result<(), PackageError>
where
    W: Write + Seek,
{
    archive
        .start_file(name, options)
        .map_err(PackageError::ArchiveWrite)?;
    archive
        .write_all(bytes)
        .map_err(|source| PackageError::ArchiveWriteIo { source })
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, io::Cursor, time::Duration};

    use serde_json::json;
    use zip::ZipArchive;

    use super::PackageBuilder;
    use crate::{
        AwlSource, BeamModule, BeamSet, CURRENT_FORMAT_VERSION, DeclaredActivity, Manifest,
        ManifestVersion, PackageContract, PackageError, content_hash_with_contract,
    };

    fn sample_manifest() -> Manifest {
        Manifest {
            entry_module: "workflow/order".to_owned(),
            entry_function: "run".to_owned(),
            input_schema: json!({ "type": "object" }),
            output_schema: json!({ "type": "object" }),
            timeout: Some(Duration::from_secs(30)),
            activities: vec![DeclaredActivity {
                activity_type: "charge_card".to_owned(),
            }],
            version: ManifestVersion::new("caller-supplied-version"),
            format_version: CURRENT_FORMAT_VERSION,
            additional_workflows: Vec::new(),
        }
    }

    fn sample_beams() -> Result<BeamSet, PackageError> {
        BeamSet::new(vec![
            BeamModule::new("workflow/support", vec![4, 5, 6]),
            BeamModule::new("workflow/order", vec![1, 2, 3]),
        ])
    }

    #[test]
    fn finalised_manifest_version_equals_v4_contract_hash() -> Result<(), PackageError> {
        let beams = sample_beams()?;
        let source_manifest = sample_manifest();
        let contract = PackageContract::from_manifest(&source_manifest);
        let expected = content_hash_with_contract(&beams, &source_manifest, &contract).to_string();
        let manifest = PackageBuilder::new(source_manifest, beams).finalise_manifest()?;

        assert_eq!(manifest.version.as_str(), expected);
        Ok(())
    }

    #[test]
    fn caller_supplied_manifest_version_is_overwritten() -> Result<(), PackageError> {
        let beams = sample_beams()?;
        let source_manifest = sample_manifest();
        let contract = PackageContract::from_manifest(&source_manifest);
        let expected = content_hash_with_contract(&beams, &source_manifest, &contract).to_string();
        let manifest = PackageBuilder::new(source_manifest, beams).finalise_manifest()?;

        assert_ne!(manifest.version.as_str(), "caller-supplied-version");
        assert_eq!(manifest.version.as_str(), expected);
        Ok(())
    }

    #[test]
    fn missing_entry_module_returns_typed_error() -> Result<(), PackageError> {
        let beams = BeamSet::new(vec![BeamModule::new("workflow/other", vec![1])])?;
        let result = PackageBuilder::new(sample_manifest(), beams).write_to_bytes();

        assert!(matches!(
            result,
            Err(PackageError::MissingEntryModule { module }) if module == "workflow/order"
        ));
        Ok(())
    }

    #[test]
    fn write_to_bytes_succeeds_without_source_entries() -> Result<(), PackageError> {
        let bytes = PackageBuilder::new(sample_manifest(), sample_beams()?).write_to_bytes()?;
        let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(PackageError::ArchiveRead)?;
        let mut names = Vec::new();

        for index in 0..archive.len() {
            let file = archive.by_index(index).map_err(PackageError::ArchiveRead)?;
            names.push(file.name().to_owned());
        }

        assert_eq!(
            names,
            vec![
                "manifest.json",
                "contract.json",
                "beam/workflow/order.beam",
                "beam/workflow/support.beam",
            ]
        );
        Ok(())
    }

    #[test]
    fn identical_inputs_produce_identical_archive_bytes() -> Result<(), PackageError> {
        let mut source = BTreeMap::new();
        source.insert(
            "workflow/order".to_owned(),
            b"pub fn run() { Nil }".to_vec(),
        );
        let first = PackageBuilder::with_source(sample_manifest(), sample_beams()?, source.clone())
            .write_to_bytes()?;
        let second = PackageBuilder::with_source(sample_manifest(), sample_beams()?, source)
            .write_to_bytes()?;

        assert_eq!(first, second);
        Ok(())
    }

    #[test]
    fn source_inclusion_does_not_change_manifest_version() -> Result<(), PackageError> {
        let mut source = BTreeMap::new();
        source.insert(
            "workflow/order".to_owned(),
            b"pub fn run() { Nil }".to_vec(),
        );
        let without_source = PackageBuilder::new(sample_manifest(), sample_beams()?)
            .finalise_manifest()?
            .version;
        let with_source = PackageBuilder::with_source(sample_manifest(), sample_beams()?, source)
            .finalise_manifest()?
            .version;

        assert_eq!(without_source, with_source);
        Ok(())
    }

    /// THE RIDER, pinned: archived AWL source is provenance and is DECLARED
    /// unbound from package identity. Three packages — one with no AWL source,
    /// one carrying a document, and one carrying a DIFFERENT document under a
    /// different filename with different schemas — all hash identically,
    /// because none of those bytes reach any identity encoding in
    /// [`crate::hash`]. If this ever fails, a workflow's version would change
    /// when only a comment in its source changed, and every run pinned to the
    /// old version would stop resolving.
    #[test]
    fn awl_source_inclusion_does_not_change_manifest_version() -> Result<(), PackageError> {
        let bare = PackageBuilder::new(sample_manifest(), sample_beams()?)
            .finalise_manifest()?
            .version;
        let carried = PackageBuilder::new(sample_manifest(), sample_beams()?)
            .with_awl_source(AwlSource::new(
                "order.awl",
                "workflow order\n",
                std::iter::empty::<(String, Vec<u8>)>(),
            ))
            .finalise_manifest()?
            .version;
        let differing = PackageBuilder::new(sample_manifest(), sample_beams()?)
            .with_awl_source(AwlSource::new(
                "renamed.awl",
                "workflow order\n  timeout 6h\n",
                [("ticket.schema.json", br#"{"type":"object"}"#.to_vec())],
            ))
            .finalise_manifest()?
            .version;

        assert_eq!(bare, carried, "carrying AWL source changed the version");
        assert_eq!(
            carried, differing,
            "changing the archived AWL source changed the version"
        );
        Ok(())
    }

    /// The archived layout, pinned by name and order: the AWL families sit
    /// under their own prefix, keep the document's original filename and each
    /// schema's document-relative path verbatim, and are written last so the
    /// archive stays byte-deterministic.
    #[test]
    fn awl_entries_are_written_under_their_own_prefixes() -> Result<(), PackageError> {
        let bytes = PackageBuilder::new(sample_manifest(), sample_beams()?)
            .with_awl_source(AwlSource::new(
                "dev_brief.awl",
                "workflow dev_brief\n",
                [
                    ("schemas/brief.schema.json", b"{}".to_vec()),
                    ("ticket.schema.json", b"{}".to_vec()),
                ],
            ))
            .write_to_bytes()?;

        let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(PackageError::ArchiveRead)?;
        let mut names = Vec::new();
        for index in 0..archive.len() {
            let file = archive.by_index(index).map_err(PackageError::ArchiveRead)?;
            names.push(file.name().to_owned());
        }

        assert_eq!(
            names,
            vec![
                "manifest.json",
                "contract.json",
                "beam/workflow/order.beam",
                "beam/workflow/support.beam",
                "awl/document/dev_brief.awl",
                "awl/schema/schemas/brief.schema.json",
                "awl/schema/ticket.schema.json",
            ]
        );
        Ok(())
    }

    /// A schema path that could escape its archive root cannot be written: the
    /// same guard that protects module names protects provenance paths.
    #[test]
    fn rejects_unsafe_awl_schema_paths() -> Result<(), PackageError> {
        let result = PackageBuilder::new(sample_manifest(), sample_beams()?)
            .with_awl_source(AwlSource::new(
                "order.awl",
                "workflow order\n",
                [("../escape.json", b"{}".to_vec())],
            ))
            .write_to_bytes();

        assert!(matches!(
            result,
            Err(PackageError::MalformedAwlEntry { entry }) if entry == "../escape.json"
        ));
        Ok(())
    }

    #[test]
    fn rejects_unsafe_source_names() -> Result<(), PackageError> {
        let mut source = BTreeMap::new();
        source.insert("../escape".to_owned(), b"pub fn run() { Nil }".to_vec());

        let result = PackageBuilder::with_source(sample_manifest(), sample_beams()?, source)
            .write_to_bytes();

        assert!(matches!(
            result,
            Err(PackageError::MalformedBeamEntry { entry }) if entry == "../escape"
        ));
        Ok(())
    }

    #[test]
    fn rejects_logical_names_with_deployed_name_separator() -> Result<(), PackageError> {
        let beams = BeamSet::new(vec![
            BeamModule::new("workflow/order", vec![1, 2, 3]),
            BeamModule::new("workflow/order$bad", vec![1]),
        ])?;
        let result = PackageBuilder::new(sample_manifest(), beams).write_to_bytes();

        assert!(matches!(
            result,
            Err(PackageError::MalformedBeamEntry { entry }) if entry == "workflow/order$bad"
        ));
        Ok(())
    }
}
