//! Deployed-package fixtures shared by the domain pins here and the HTTP
//! pins in `crate::api::http::awl_deployed`.
//!
//! One builder, so both layers assert over archives built the same way; a
//! second copy would let the two layers drift and still both be green.

use std::time::Duration;

use aion::WorkflowVersionInfo;
use aion_package::{
    AwlSource, BeamModule, BeamSet, CURRENT_FORMAT_VERSION, DeclaredActivity, Manifest,
    ManifestVersion, PackageBuilder,
};
use aion_store::PackageRecord;
use chrono::{TimeZone, Utc};
use serde_json::json;

/// A minimal valid AWL document with no schema imports.
pub(crate) const DOCUMENT: &str = "//! Deployed probe.\nworkflow deployed_probe\n  outcome done: type Summary, route success\n\ntype Summary { value: String }\n";

/// A valid AWL document whose only type comes from an imported schema file.
pub(crate) const SCHEMA_DOCUMENT: &str = "//! Deployed schema probe.\nworkflow schema_probe\n  outcome done: type Intake, route success\n\ntype Intake = schema(\"schemas/intake.schema.json\")\n";

/// The schema file `SCHEMA_DOCUMENT` imports.
pub(crate) const SCHEMA_BYTES: &[u8] =
    b"{\"type\":\"object\",\"properties\":{\"value\":{\"type\":\"string\"}},\"required\":[\"value\"]}";

/// The document-relative path `SCHEMA_DOCUMENT` names.
pub(crate) const SCHEMA_PATH: &str = "schemas/intake.schema.json";

/// A manifest whose single entry module is `entry_module`.
pub(crate) fn manifest(entry_module: &str) -> Manifest {
    Manifest {
        entry_module: entry_module.to_owned(),
        entry_function: "run".to_owned(),
        input_schema: json!({ "type": "object" }),
        output_schema: json!({ "type": "object" }),
        timeout: Some(Duration::from_secs(30)),
        activities: vec![DeclaredActivity {
            activity_type: "probe".to_owned(),
        }],
        version: ManifestVersion::new("unstamped"),
        format_version: CURRENT_FORMAT_VERSION,
        additional_workflows: Vec::new(),
    }
}

/// A persisted archive row built from `manifest`, carrying `awl` when given.
pub(crate) fn record(
    manifest: Manifest,
    awl: Option<AwlSource>,
    deployed_at_seconds: i64,
) -> Result<PackageRecord, Box<dyn std::error::Error>> {
    let workflow_type = manifest.entry_module.clone();
    let beams = BeamSet::new(vec![BeamModule::new(&workflow_type, vec![1, 2, 3])])?;
    let mut builder = PackageBuilder::new(manifest, beams);
    if let Some(awl) = awl {
        builder = builder.with_awl_source(awl);
    }
    let content_hash = builder.finalise_manifest()?.version.as_str().to_owned();
    Ok(PackageRecord {
        workflow_type,
        content_hash,
        archive: builder.write_to_bytes()?,
        deployed_at: Utc
            .timestamp_opt(deployed_at_seconds, 0)
            .single()
            .ok_or("fixture instant is not representable")?,
    })
}

/// The catalog entry an engine would hold for `record`.
pub(crate) fn catalog_entry(
    record: &PackageRecord,
    route_active: bool,
) -> Result<WorkflowVersionInfo, Box<dyn std::error::Error>> {
    Ok(WorkflowVersionInfo {
        workflow_type: record.workflow_type.clone(),
        content_hash: record.content_hash.parse()?,
        deployed_entry_module: format!("{}${}", record.workflow_type, record.content_hash),
        entry_function: "run".to_owned(),
        manifest_version: ManifestVersion::new(record.content_hash.clone()),
        loaded_at: record.deployed_at,
        route_active,
    })
}
