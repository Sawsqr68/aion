//! `Package` load path and integrity check.

use std::{
    collections::BTreeMap,
    fs::File,
    io::{Cursor, Read, Seek},
    path::Path,
};

use zip::{ZipArchive, result::ZipError};

use crate::{
    AwlSource, BeamModule, BeamSet, ContentHash, ContractIdentityError, ExtractionLimits, Manifest,
    PackageContract, PackageError,
    awl::{AWL_DOCUMENT_PREFIX, AWL_SCHEMA_PREFIX},
    builder::is_safe_logical_name,
    extraction::ExtractionBudget,
    hash::{has_contract_identity, verified_content_hash_with_contract},
    namespace::deployed_name,
    version::WorkflowVersion,
};

const MANIFEST_ENTRY: &str = "manifest.json";
const CONTRACT_ENTRY: &str = "contract.json";
const BEAM_PREFIX: &str = "beam/";
const BEAM_SUFFIX: &str = ".beam";
const SOURCE_PREFIX: &str = "src/";
const SOURCE_SUFFIX: &str = ".gleam";
const AWL_PREFIX: &str = "awl/";

/// A validated, integrity-checked `.aion` package loaded fully into memory.
///
/// The engine performs actual VM registration. This crate only supplies the
/// validated manifest, canonical beam bytes, optional source, and deployed module
/// names the engine can register.
#[derive(Clone, Debug, PartialEq)]
pub struct Package {
    manifest: Manifest,
    contract: Option<PackageContract>,
    beams: BeamSet,
    source: BTreeMap<String, Vec<u8>>,
    awl: Option<AwlSource>,
    content_hash: ContentHash,
}

/// The entry families one archive read yields, beyond the manifest and
/// contract.
struct ArchiveEntries {
    beams: BeamSet,
    source: BTreeMap<String, Vec<u8>>,
    awl: Option<AwlSource>,
}

impl Package {
    /// Loads a `.aion` package from a filesystem path.
    ///
    /// The caller chooses an explicit [`ExtractionLimits`] inflate budget;
    /// untrusted input must be bounded.
    ///
    /// # Errors
    ///
    /// Returns a typed [`PackageError`] for unreadable archives, malformed
    /// manifests or entries, unsupported format versions, integrity mismatches,
    /// missing entry modules, or contents inflating past `limits`.
    pub fn load_from_path(
        path: impl AsRef<Path>,
        limits: ExtractionLimits,
    ) -> Result<Self, PackageError> {
        let file =
            File::open(path).map_err(|source| PackageError::ArchiveRead(ZipError::Io(source)))?;
        Self::load_from_reader(file, limits)
    }

    /// Loads a `.aion` package from an in-memory byte buffer.
    ///
    /// The caller chooses an explicit [`ExtractionLimits`] inflate budget;
    /// untrusted input must be bounded.
    ///
    /// # Errors
    ///
    /// Returns a typed [`PackageError`] for unreadable archives, malformed
    /// manifests or entries, unsupported format versions, integrity mismatches,
    /// missing entry modules, or contents inflating past `limits`.
    pub fn load_from_bytes(
        bytes: impl AsRef<[u8]>,
        limits: ExtractionLimits,
    ) -> Result<Self, PackageError> {
        Self::load_from_reader(Cursor::new(bytes.as_ref()), limits)
    }

    fn load_from_reader<R>(reader: R, limits: ExtractionLimits) -> Result<Self, PackageError>
    where
        R: Read + Seek,
    {
        let mut archive = ZipArchive::new(reader).map_err(PackageError::ArchiveRead)?;
        let mut budget = limits.budget();
        let manifest = read_manifest(&mut archive, &mut budget)?;
        manifest.check_format_version()?;
        let contract = read_contract(&mut archive, &mut budget)?;

        let entries = read_archive_entries(&mut archive, &mut budget)?;
        let ArchiveEntries { beams, source, awl } = entries;
        let content_hash =
            verified_content_hash_with_contract(&beams, &manifest, contract.as_ref())?;

        if beams.get(&manifest.entry_module).is_none() {
            return Err(PackageError::MissingEntryModule {
                module: manifest.entry_module.clone(),
            });
        }

        Ok(Self {
            manifest,
            contract,
            beams,
            source,
            awl,
            content_hash,
        })
    }

    /// Returns the validated manifest loaded from `manifest.json`.
    #[must_use]
    pub const fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Returns the canonical compiled beam set extracted from `beam/` entries.
    #[must_use]
    pub const fn beams(&self) -> &BeamSet {
        &self.beams
    }

    /// Returns optional Gleam source files extracted verbatim from `src/` entries.
    #[must_use]
    pub const fn source(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.source
    }

    /// Returns the authored AWL document and its imported schema files, when
    /// the archive carries them.
    ///
    /// `None` for every archive built from a Gleam project and for every AWL
    /// archive written before the `awl/` entry families existed — the field is
    /// provenance, never a load requirement.
    ///
    /// This is PROVENANCE ONLY: it never participates in the package's
    /// [`ContentHash`], so a consumer must not treat it as a version input.
    #[must_use]
    pub const fn awl(&self) -> Option<&AwlSource> {
        self.awl.as_ref()
    }

    /// Returns the recomputed content hash that proved package integrity.
    #[must_use]
    pub const fn content_hash(&self) -> &ContentHash {
        &self.content_hash
    }

    /// Returns the durable contract only when the stored identity commits to it.
    ///
    /// # Errors
    ///
    /// Returns [`ContractIdentityError::RedeployRequired`] for every pre-`.v4`
    /// identity, including integrity-valid legacy, `.v1`, and `.v3` packages.
    pub fn contract(&self) -> Result<&PackageContract, ContractIdentityError> {
        if has_contract_identity(
            &self.beams,
            &self.manifest,
            self.contract.as_ref(),
            &self.content_hash,
        ) {
            self.contract
                .as_ref()
                .ok_or_else(|| ContractIdentityError::RedeployRequired {
                    stored_version: self.content_hash.to_string(),
                })
        } else {
            Err(ContractIdentityError::RedeployRequired {
                stored_version: self.content_hash.to_string(),
            })
        }
    }

    /// Whether this package's version identity commits to an explicitly authored
    /// workflow timeout.
    ///
    /// This is the single, tamper-evident authority for "did the author declare
    /// a workflow timeout": it is true only when the manifest carries an
    /// authored `timeout` AND the content hash is the domain-separated
    /// contract-bearing identity that binds it. The `.v4` identity commits to
    /// every package's timeout vector, so identity alone does not mean a
    /// timeout was authored — absence of the value is absence of the
    /// declaration. A legacy (beams-only) archive — even one whose manifest
    /// still carries a defaulted `timeout` value — reads as `false`, so it can
    /// never arm a deadline. Callers pair this with [`Self::manifest`] to read
    /// the declared timeout: the value is trustworthy precisely because it is
    /// bound into the version hash.
    #[must_use]
    pub fn has_declared_timeout(&self) -> bool {
        self.manifest.timeout.is_some()
            && has_contract_identity(
                &self.beams,
                &self.manifest,
                self.contract.as_ref(),
                &self.content_hash,
            )
    }

    /// The explicitly authored workflow timeout of the primary entry, or `None`.
    ///
    /// Returns `Some` only when the package identity commits to a declared
    /// timeout (see [`Self::has_declared_timeout`]); otherwise `None`, so a
    /// legacy or defaulted manifest yields no deadline.
    #[must_use]
    pub fn declared_timeout(&self) -> Option<std::time::Duration> {
        self.declared_entry_timeout(self.manifest.timeout)
    }

    /// The authenticated authored timeout for an entry carrying `entry_timeout`.
    ///
    /// This is the per-entry declaredness authority: the timeout-bearing
    /// identity binds EVERY entry's timeout (primary and additional), so when
    /// [`Self::has_declared_timeout`] is true each entry's manifest `timeout` is
    /// authenticated and returned verbatim. When the identity is legacy
    /// (beams-only) — or does not verify against the full per-entry timeout
    /// vector — every entry reads as undeclared and arms nothing, regardless of
    /// what `timeout` value a manifest entry happens to carry. Callers pass the
    /// primary entry's `manifest.timeout` or an additional
    /// [`crate::WorkflowEntry::timeout`]; the gate is identical for both.
    #[must_use]
    pub fn declared_entry_timeout(
        &self,
        entry_timeout: Option<std::time::Duration>,
    ) -> Option<std::time::Duration> {
        if self.has_declared_timeout() {
            entry_timeout
        } else {
            None
        }
    }

    /// Produces the canonical cross-system version record for this loaded package.
    #[must_use]
    pub fn version_record(&self) -> WorkflowVersion {
        WorkflowVersion {
            entry_module: self.manifest.entry_module.clone(),
            content_hash: self.content_hash.clone(),
            activities: self.manifest.activities.clone(),
            input_schema: self.manifest.input_schema.clone(),
            output_schema: self.manifest.output_schema.clone(),
        }
    }

    /// Returns engine-ready deployed module names paired with their beam bytes.
    ///
    /// The engine performs the actual VM registration; this crate only supplies
    /// the validated namespaced names and exact module bytes.
    #[must_use]
    pub fn deployed_modules(&self) -> Vec<(String, &[u8])> {
        self.beams
            .iter()
            .map(|module| {
                (
                    deployed_name(module.name(), &self.content_hash),
                    module.bytes(),
                )
            })
            .collect()
    }

    /// Returns the deployed namespaced module name for the manifest entry module.
    #[must_use]
    pub fn deployed_entry_module(&self) -> String {
        deployed_name(&self.manifest.entry_module, &self.content_hash)
    }

    /// Re-serialises this validated package into canonical `.aion` archive
    /// bytes.
    ///
    /// The deterministic [`crate::PackageBuilder`] write path is used, so the
    /// output round-trips through [`Self::load_from_bytes`] to a package with
    /// the same legacy or explicit-timeout content hash, canonical manifest
    /// digest, source set, and AWL provenance. This is the persistence form for
    /// runtime-deployed packages: the engine stores these bytes so a deploy
    /// survives restart, and this path rebuilds the archive from FIELDS — so
    /// anything the loaded package does not hold as a field is not persisted.
    ///
    /// # Errors
    ///
    /// Returns [`PackageError`] variants for manifest serialisation or ZIP
    /// writer failures; the entry module is already proven present by load
    /// validation.
    pub fn to_archive_bytes(&self) -> Result<Vec<u8>, PackageError> {
        let mut builder = crate::PackageBuilder::with_source(
            self.manifest.clone(),
            self.beams.clone(),
            self.source.clone(),
        );
        if let Some(awl) = self.awl.clone() {
            builder = builder.with_awl_source(awl);
        }
        builder
            .preserving_loaded_identity(self.content_hash.clone(), self.contract.clone())
            .write_to_bytes()
    }

    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    #[must_use]
    pub fn from_validated_parts_for_test(
        manifest: Manifest,
        beams: BeamSet,
        source: BTreeMap<String, Vec<u8>>,
        content_hash: ContentHash,
    ) -> Self {
        Self {
            manifest,
            contract: None,
            beams,
            source,
            awl: None,
            content_hash,
        }
    }
}

fn read_manifest<R>(
    archive: &mut ZipArchive<R>,
    budget: &mut ExtractionBudget,
) -> Result<Manifest, PackageError>
where
    R: Read + Seek,
{
    let mut manifest_file = match archive.by_name(MANIFEST_ENTRY) {
        Ok(file) => file,
        Err(ZipError::FileNotFound) => return Err(PackageError::MissingManifest),
        Err(error) => return Err(PackageError::ArchiveRead(error)),
    };

    let manifest_bytes = budget.read_entry(&mut manifest_file)?;

    serde_json::from_slice(&manifest_bytes).map_err(|source| PackageError::ManifestParse { source })
}

fn read_contract<R>(
    archive: &mut ZipArchive<R>,
    budget: &mut ExtractionBudget,
) -> Result<Option<PackageContract>, PackageError>
where
    R: Read + Seek,
{
    let mut contract_file = match archive.by_name(CONTRACT_ENTRY) {
        Ok(file) => file,
        Err(ZipError::FileNotFound) => return Ok(None),
        Err(error) => return Err(PackageError::ArchiveRead(error)),
    };
    let contract_bytes = budget.read_entry(&mut contract_file)?;
    let contract = serde_json::from_slice(&contract_bytes)
        .map_err(|source| PackageError::ContractParse { source })?;
    Ok(Some(contract))
}

/// Reads every non-metadata entry into its family.
///
/// Entries outside the families this format defines are skipped, exactly as
/// they always have been — with ONE deliberate exception: an entry under the
/// `awl/` prefix that names no defined family is refused. That prefix is owned
/// by this format, and a rewrite through [`Package::to_archive_bytes`] rebuilds
/// the archive from fields, so silently skipping an unrecognised `awl/` entry
/// would drop it on the next persistence write. Refusing names the problem
/// instead of losing the bytes.
fn read_archive_entries<R>(
    archive: &mut ZipArchive<R>,
    budget: &mut ExtractionBudget,
) -> Result<ArchiveEntries, PackageError>
where
    R: Read + Seek,
{
    let mut modules = Vec::new();
    let mut source = BTreeMap::new();
    let mut document: Option<(String, String)> = None;
    let mut schemas = BTreeMap::new();

    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(PackageError::ArchiveRead)?;
        if file.is_dir() {
            continue;
        }

        let entry = file.name().to_owned();
        if entry == MANIFEST_ENTRY || entry == CONTRACT_ENTRY {
            continue;
        }

        if entry.starts_with(BEAM_PREFIX) {
            let logical = logical_name_from_entry(&entry, BEAM_PREFIX, BEAM_SUFFIX)?;
            let bytes = budget.read_entry(&mut file)?;
            modules.push(BeamModule::new(logical, bytes));
        } else if entry.starts_with(SOURCE_PREFIX) {
            let logical = logical_name_from_entry(&entry, SOURCE_PREFIX, SOURCE_SUFFIX)?;
            let bytes = budget.read_entry(&mut file)?;
            if source.insert(logical, bytes).is_some() {
                return Err(PackageError::MalformedBeamEntry { entry });
            }
        } else if let Some(name) = entry.strip_prefix(AWL_DOCUMENT_PREFIX) {
            // The document sits at the ROOT of its own directory — that is what
            // makes every schema entry's path document-relative — so its entry
            // carries a bare filename and never a directory part.
            if name.contains('/') {
                return Err(PackageError::MalformedAwlEntry { entry });
            }
            let name = awl_relative_path(&entry, name)?;
            let bytes = budget.read_entry(&mut file)?;
            let text =
                String::from_utf8(bytes).map_err(|source| PackageError::AwlDocumentNotUtf8 {
                    entry: entry.clone(),
                    source,
                })?;
            if document.replace((name, text)).is_some() {
                return Err(PackageError::MalformedAwlEntry { entry });
            }
        } else if let Some(path) = entry.strip_prefix(AWL_SCHEMA_PREFIX) {
            let path = awl_relative_path(&entry, path)?;
            let bytes = budget.read_entry(&mut file)?;
            if schemas.insert(path, bytes).is_some() {
                return Err(PackageError::MalformedAwlEntry { entry });
            }
        } else if entry.starts_with(AWL_PREFIX) {
            return Err(PackageError::MalformedAwlEntry { entry });
        }
    }

    let awl = match document {
        Some((name, text)) => Some(AwlSource::new(name, text, schemas)),
        None if schemas.is_empty() => None,
        None => return Err(PackageError::MissingAwlDocument),
    };

    let beams = BeamSet::new(modules)?;
    Ok(ArchiveEntries { beams, source, awl })
}

/// Validates the part of an `awl/` entry name that follows its family prefix.
///
/// The whole remainder is the name — extension and nesting included — because
/// a consumer stages the file back at exactly that relative path.
fn awl_relative_path(entry: &str, relative_path: &str) -> Result<String, PackageError> {
    if is_safe_logical_name(relative_path) {
        Ok(relative_path.to_owned())
    } else {
        Err(PackageError::MalformedAwlEntry {
            entry: entry.to_owned(),
        })
    }
}

fn logical_name_from_entry(
    entry: &str,
    prefix: &str,
    suffix: &str,
) -> Result<String, PackageError> {
    let Some(without_prefix) = entry.strip_prefix(prefix) else {
        return Err(PackageError::MalformedBeamEntry {
            entry: entry.to_owned(),
        });
    };
    let Some(logical) = without_prefix.strip_suffix(suffix) else {
        return Err(PackageError::MalformedBeamEntry {
            entry: entry.to_owned(),
        });
    };

    if is_safe_logical_name(logical) {
        Ok(logical.to_owned())
    } else {
        Err(PackageError::MalformedBeamEntry {
            entry: entry.to_owned(),
        })
    }
}

#[cfg(test)]
#[path = "package_tests.rs"]
mod tests;
