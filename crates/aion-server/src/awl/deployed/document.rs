//! Reading one deployed version's archived AWL document back out of its
//! persisted archive.

use aion_package::{ContractIdentityError, ExtractionLimits, Package};
use aion_store::PackageRecord;

use super::projection;
use super::types::{DeployedDocument, DeployedError, DeployedSchema};
// Through the module's public re-export deliberately: it is the path the
// surface's consumers name the type by, and using it here keeps that
// re-export load-bearing.
use super::DeployedSignal;

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
    let (input_schema, signals) = contract_surface(&package, workflow_type);
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
        input_schema,
        signals,
        projection,
    })
}

/// The start input schema and committed signals of `workflow_type`, from the
/// archive's identity-committed contract.
///
/// This is the SAME seam the `/assistant` descriptor derives its form surface
/// from — [`Package::contract`] — never a re-derivation from source: the
/// contract is what the version hash commits to, so it is the only schema the
/// server can honestly attribute to this exact deployed version. Both halves
/// are attributed PER ENTRY: an additional workflow entry gets its own
/// committed input schema and `None` for signals, because
/// [`aion_package::AdditionalWorkflowContract`] commits no signal set — the
/// primary's declarations must not be inherited by an entry whose contract
/// never accepted them.
///
/// A pre-contract package (a stored identity [`Package::contract`] refuses
/// with [`ContractIdentityError::RedeployRequired`]) genuinely carries no
/// committed contract, which is a first-class absence here — `(None, None)` —
/// not a failure: the document read itself remains served.
fn contract_surface(
    package: &Package,
    workflow_type: &str,
) -> (Option<serde_json::Value>, Option<Vec<DeployedSignal>>) {
    let contract = match package.contract() {
        Ok(contract) => contract,
        Err(ContractIdentityError::RedeployRequired { .. }) => return (None, None),
    };
    if package.manifest().entry_module == workflow_type {
        let signals = contract
            .signals
            .iter()
            .map(|signal| DeployedSignal {
                name: signal.name.clone(),
                input_schema: signal.input_schema.clone(),
            })
            .collect();
        return (Some(contract.input_schema.clone()), Some(signals));
    }
    let input_schema = contract
        .additional_workflows
        .iter()
        .find(|entry| entry.workflow_type == workflow_type)
        .map(|entry| entry.input_schema.clone());
    (input_schema, None)
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
