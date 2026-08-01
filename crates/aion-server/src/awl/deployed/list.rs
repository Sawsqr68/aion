//! Projecting the engine's loaded-version catalog and the store's persisted
//! archives into one honest listing.

use std::collections::BTreeMap;

use aion::WorkflowVersionInfo;
use aion_package::{ExtractionLimits, Package};
use aion_store::PackageRecord;

use super::types::{DeployedSourceState, DeployedVersion};

/// Unions the loaded-version catalog with the persisted archive set.
///
/// Neither set contains the other. A catalog entry with no archive row is an
/// operator-file startup package, deliberately never persisted; an archive row
/// with no catalog entry is a persisted deploy the engine did not load this
/// boot (a reload the engine skipped loudly). Both are listed, each stating
/// which side it came from through `loaded` and `deployed_at`.
///
/// The SOURCE state, unlike listing membership, is keyed on the content hash
/// alone: a multi-entry archive persists one row under its primary workflow
/// type while registering every additional entry in the catalog, so keying the
/// lookup on the pair would report a synthesized child entry as unpersisted
/// when its archive — and its authored document — are right there. The hash is
/// a safe key because it is computed over the manifest, which names the
/// primary entry module: two rows cannot share a hash and disagree about what
/// they contain.
///
/// Each archive is read back at most once however many entries resolve to it.
pub(super) fn project_versions(
    catalog: Vec<WorkflowVersionInfo>,
    archives: &[PackageRecord],
) -> Vec<DeployedVersion> {
    let mut by_hash: BTreeMap<&str, &PackageRecord> = BTreeMap::new();
    for record in archives {
        by_hash
            .entry(record.content_hash.as_str())
            .or_insert(record);
    }
    let mut merged: BTreeMap<(String, String), DeployedVersion> = BTreeMap::new();
    for info in catalog {
        let content_hash = info.content_hash.to_string();
        merged.insert(
            (info.workflow_type.clone(), content_hash.clone()),
            DeployedVersion {
                workflow_type: info.workflow_type,
                content_hash,
                loaded: true,
                route_active: info.route_active,
                loaded_at: Some(info.loaded_at.to_rfc3339()),
                deployed_at: None,
                source: DeployedSourceState::NotPersisted,
            },
        );
    }
    for record in archives {
        let key = (record.workflow_type.clone(), record.content_hash.clone());
        merged.entry(key).or_insert_with(|| DeployedVersion {
            workflow_type: record.workflow_type.clone(),
            content_hash: record.content_hash.clone(),
            loaded: false,
            route_active: false,
            loaded_at: None,
            deployed_at: None,
            source: DeployedSourceState::NotPersisted,
        });
    }
    let mut evaluated: BTreeMap<String, DeployedSourceState> = BTreeMap::new();
    let mut versions: Vec<DeployedVersion> = merged.into_values().collect();
    for version in &mut versions {
        let Some(record) = by_hash.get(version.content_hash.as_str()).copied() else {
            continue;
        };
        version.deployed_at = Some(record.deployed_at.to_rfc3339());
        version.source = evaluated
            .entry(record.content_hash.clone())
            .or_insert_with(|| evaluate_archive(record))
            .clone();
    }
    // Newest first within a type, with a total order so the listing is stable
    // across calls: two versions can carry the same instant, and one side of
    // the union carries no instant at all.
    versions.sort_by(|left, right| {
        left.workflow_type
            .cmp(&right.workflow_type)
            .then_with(|| newest(right).cmp(&newest(left)))
            .then_with(|| left.content_hash.cmp(&right.content_hash))
    });
    versions
}

/// The most recent instant known for a version, from whichever side of the
/// union carries one. RFC 3339 with a fixed offset sorts lexicographically in
/// instant order, which is what both producers emit.
fn newest(version: &DeployedVersion) -> Option<&str> {
    match (version.deployed_at.as_deref(), version.loaded_at.as_deref()) {
        (Some(deployed), Some(loaded)) => Some(deployed.max(loaded)),
        (Some(only), None) | (None, Some(only)) => Some(only),
        (None, None) => None,
    }
}

/// Reads one persisted archive back far enough to state whether it carries an
/// authored AWL document.
///
/// Extraction is unbounded, matching the engine's own reload of these same
/// rows (`aion::loader::persistence`): these bytes are the engine's persisted
/// state, already admitted under the operator's `deploy.max_inflated_bytes`
/// ceiling when they were deployed. Re-imposing a ceiling that may since have
/// been lowered would report a healthy deployed workflow as unreadable.
fn evaluate_archive(record: &PackageRecord) -> DeployedSourceState {
    match Package::load_from_bytes(&record.archive, ExtractionLimits::unbounded()) {
        Ok(package) => package.awl().map_or(DeployedSourceState::Absent, |awl| {
            DeployedSourceState::Available {
                document_name: awl.document_name().to_owned(),
                schema_count: awl.schemas().len(),
            }
        }),
        Err(error) => DeployedSourceState::Unreadable {
            reason: error.to_string(),
        },
    }
}
