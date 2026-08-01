//! Reading one deployed version's archived AWL document back out of its
//! persisted archive.

use aion_package::{ExtractionLimits, Package};
use aion_store::PackageRecord;

use super::projection;
use super::types::{DeployedDocument, DeployedError, DeployedSchema};

/// Reads the archived AWL document for `(workflow_type, content_hash)`.
///
/// Blocking: extracts an archive and, when the document imports schemas,
/// stages them for the checker. Callers on an async runtime must run this on
/// the blocking pool.
///
/// The `(type, version)` pair is verified, not just the hash: the requested
/// workflow type must be the archive's own entry module or one of its declared
/// additional entries, so a mismatched pair is a refusal rather than a
/// document served under a type that never named it.
///
/// # Errors
///
/// Returns [`DeployedError::NotFound`] when no persisted archive matches the
/// pair, [`DeployedError::Unreadable`] when the archive will not read back,
/// [`DeployedError::NoArchivedSource`] when it carries no AWL document, and
/// the staging failures of [`projection`].
pub(super) fn read_document(
    archives: &[PackageRecord],
    workflow_type: &str,
    content_hash: &str,
) -> Result<DeployedDocument, DeployedError> {
    let not_found = || DeployedError::NotFound {
        workflow_type: workflow_type.to_owned(),
        content_hash: content_hash.to_owned(),
    };
    let record = archives
        .iter()
        .find(|record| record.content_hash == content_hash)
        .ok_or_else(not_found)?;
    // Unbounded for the same reason the engine's own reload is: these are the
    // server's persisted bytes, admitted under the operator's inflate ceiling
    // at deploy time.
    let package = Package::load_from_bytes(&record.archive, ExtractionLimits::unbounded())
        .map_err(|error| DeployedError::Unreadable {
            workflow_type: workflow_type.to_owned(),
            content_hash: content_hash.to_owned(),
            reason: error.to_string(),
        })?;
    if !declares_workflow_type(&package, workflow_type) {
        return Err(not_found());
    }
    let awl = package
        .awl()
        .ok_or_else(|| DeployedError::NoArchivedSource {
            workflow_type: workflow_type.to_owned(),
            content_hash: content_hash.to_owned(),
        })?;
    let projection = projection::project(awl.document(), awl.schemas())?;
    Ok(DeployedDocument {
        workflow_type: workflow_type.to_owned(),
        content_hash: content_hash.to_owned(),
        document_name: awl.document_name().to_owned(),
        source: awl.document().to_owned(),
        schemas: awl
            .schemas()
            .iter()
            .map(|(path, bytes)| DeployedSchema {
                path: path.clone(),
                text: String::from_utf8(bytes.clone()).ok(),
                byte_length: bytes.len(),
            })
            .collect(),
        projection,
    })
}

/// Whether `package` registers `workflow_type` — as its primary entry module
/// or as one of the additional entries a multi-entry archive exports.
fn declares_workflow_type(package: &Package, workflow_type: &str) -> bool {
    package.manifest().entry_module == workflow_type
        || package
            .manifest()
            .additional_workflows
            .iter()
            .any(|entry| entry.workflow_type == workflow_type)
}
