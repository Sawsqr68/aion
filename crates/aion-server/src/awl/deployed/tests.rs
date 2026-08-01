//! Pins for the read-only deployed-AWL surface.
//!
//! Every read pin is paired with the absence it is supposed to preserve: the
//! byte-identity pin also proves the archive it read is unchanged, and the
//! projection pin also proves its staging directory is gone. A read test that
//! only proved the read would pass just as well over a surface that rewrote
//! everything it touched.

use std::collections::BTreeMap;

use aion_package::{AwlSource, WorkflowEntry};
use aion_store::PackageRecord;
use chrono::{TimeZone, Utc};
use serde_json::json;

use super::document::read_document;
use super::fixtures::{
    DOCUMENT, SCHEMA_BYTES, SCHEMA_DOCUMENT, SCHEMA_PATH, catalog_entry, manifest, record,
};
use super::list::project_versions;
use super::projection;
use super::types::{DeployedError, DeployedSourceState};

/// 🔴 THE READ-ONLY PIN, both halves. The archived document is served
/// BYTE-IDENTICAL to what `Package::awl()` carries — and the archive bytes the
/// read consumed are unchanged afterwards. Without the second half this would
/// pass over a surface that re-serialised, re-stamped, or normalised the
/// package it read.
#[test]
fn the_archived_document_is_served_byte_identical_and_the_archive_is_unchanged()
-> Result<(), Box<dyn std::error::Error>> {
    let awl = AwlSource::new(
        "deployed_probe.awl",
        DOCUMENT,
        Vec::<(String, Vec<u8>)>::new(),
    );
    let row = record(manifest("deployed_probe"), Some(awl), 1_700_000_000)?;
    let before = row.archive.clone();
    let archives = vec![row];

    let document = read_document(&archives, "deployed_probe", &archives[0].content_hash)?;

    assert_eq!(
        document.source, DOCUMENT,
        "the served source must be the archived bytes verbatim"
    );
    assert_eq!(document.document_name, "deployed_probe.awl");
    assert_eq!(document.workflow_type, "deployed_probe");
    assert_eq!(document.content_hash, archives[0].content_hash);
    assert!(document.schemas.is_empty());
    assert!(
        document.projection.ok,
        "the deployed document must project: {:?}",
        document.projection.diagnostics
    );
    assert_eq!(document.projection.steps, Some(0));
    assert_eq!(
        archives[0].archive, before,
        "reading a deployed document rewrote the archive it read"
    );
    Ok(())
}

/// 🔴 THE ABSENCE PIN. A package with no archived AWL source is a first-class
/// state, not an error class of its own invention and not an empty document:
/// the refusal names BOTH reasons a version can lack source, because an
/// operator told only "no source" cannot tell a Gleam workflow from a broken
/// one.
#[test]
fn a_package_without_archived_source_refuses_and_states_both_reasons()
-> Result<(), Box<dyn std::error::Error>> {
    let row = record(manifest("gleam_authored"), None, 1_700_000_000)?;
    let archives = vec![row];

    let refusal = read_document(&archives, "gleam_authored", &archives[0].content_hash)
        .err()
        .ok_or("a package with no archived AWL source must refuse")?;

    assert!(
        matches!(refusal, DeployedError::NoArchivedSource { .. }),
        "expected the absence class, got {refusal:?}"
    );
    let message = refusal.to_string();
    assert!(message.contains("Gleam"), "{message}");
    assert!(
        message.contains("before deploys archived their source"),
        "{message}"
    );
    Ok(())
}

/// The pair is verified, not just the hash: a right hash under a workflow type
/// the archive never declared must not resolve. Otherwise the surface would
/// serve a document under a type that has nothing to do with it.
#[test]
fn a_workflow_type_the_archive_does_not_declare_is_not_found()
-> Result<(), Box<dyn std::error::Error>> {
    let awl = AwlSource::new(
        "deployed_probe.awl",
        DOCUMENT,
        Vec::<(String, Vec<u8>)>::new(),
    );
    let archives = vec![record(
        manifest("deployed_probe"),
        Some(awl),
        1_700_000_000,
    )?];

    let refusal = read_document(&archives, "someone_elses_type", &archives[0].content_hash)
        .err()
        .ok_or("a mismatched type/version pair must not resolve")?;
    assert!(
        matches!(refusal, DeployedError::NotFound { .. }),
        "expected not-found, got {refusal:?}"
    );

    let unknown = read_document(&archives, "deployed_probe", &"f".repeat(64))
        .err()
        .ok_or("an unknown version must not resolve")?;
    assert!(
        matches!(unknown, DeployedError::NotFound { .. }),
        "expected not-found, got {unknown:?}"
    );
    Ok(())
}

/// A multi-entry archive persists ONE row under its primary type while the
/// catalog registers every additional entry. An additional entry must resolve
/// to its own archive's document rather than reporting no source.
#[test]
fn an_additional_entry_resolves_to_its_archives_document() -> Result<(), Box<dyn std::error::Error>>
{
    let mut declared = manifest("parent_flow");
    declared.additional_workflows = vec![WorkflowEntry {
        workflow_type: "child_flow".to_owned(),
        entry_module: "parent_flow".to_owned(),
        entry_function: "run".to_owned(),
        input_schema: json!({ "type": "object" }),
        output_schema: json!({ "type": "object" }),
        timeout: None,
        internal: false,
    }];
    let awl = AwlSource::new("parent_flow.awl", DOCUMENT, Vec::<(String, Vec<u8>)>::new());
    let archives = vec![record(declared, Some(awl), 1_700_000_000)?];

    let child = read_document(&archives, "child_flow", &archives[0].content_hash)?;
    assert_eq!(child.workflow_type, "child_flow");
    assert_eq!(child.source, DOCUMENT);
    Ok(())
}

/// The four source states are distinguishable in one listing, and the listing
/// is the union of the two sets: a loaded version with no archive, and an
/// archived version the engine never loaded, both appear.
#[test]
fn the_listing_states_every_source_condition_distinctly() -> Result<(), Box<dyn std::error::Error>>
{
    let with_source = record(
        manifest("with_source"),
        Some(AwlSource::new(
            "with_source.awl",
            DOCUMENT,
            [(SCHEMA_PATH.to_owned(), SCHEMA_BYTES.to_vec())],
        )),
        1_700_000_100,
    )?;
    let without_source = record(manifest("without_source"), None, 1_700_000_200)?;
    let operator_file = record(manifest("operator_file"), None, 1_700_000_300)?;
    let corrupt = PackageRecord {
        workflow_type: "corrupt".to_owned(),
        content_hash: "c".repeat(64),
        archive: b"this is not a zip archive".to_vec(),
        deployed_at: Utc
            .timestamp_opt(1_700_000_400, 0)
            .single()
            .ok_or("fixture instant is not representable")?,
    };

    // `operator_file` is loaded but NOT persisted; `without_source` is
    // persisted but NOT loaded.
    let catalog = vec![
        catalog_entry(&with_source, true)?,
        catalog_entry(&operator_file, true)?,
        catalog_entry(&corrupt, false)?,
    ];
    let archives = vec![with_source.clone(), without_source.clone(), corrupt.clone()];

    let listing = project_versions(catalog, &archives);
    let state = |workflow_type: &str| {
        listing
            .iter()
            .find(|version| version.workflow_type == workflow_type)
            .map(|version| version.source.clone())
    };

    assert_eq!(
        state("with_source"),
        Some(DeployedSourceState::Available {
            document_name: "with_source.awl".to_owned(),
            schema_count: 1,
        })
    );
    assert_eq!(state("without_source"), Some(DeployedSourceState::Absent));
    assert_eq!(
        state("operator_file"),
        Some(DeployedSourceState::NotPersisted)
    );
    assert!(
        matches!(
            state("corrupt"),
            Some(DeployedSourceState::Unreadable { .. })
        ),
        "a corrupt archive must be named unreadable, never rendered as no source"
    );

    let loaded = |workflow_type: &str| {
        listing
            .iter()
            .find(|version| version.workflow_type == workflow_type)
            .map(|version| (version.loaded, version.deployed_at.is_some()))
    };
    assert_eq!(loaded("operator_file"), Some((true, false)));
    assert_eq!(loaded("without_source"), Some((false, true)));
    assert_eq!(loaded("with_source"), Some((true, true)));
    assert_eq!(
        listing.len(),
        4,
        "the listing must be the union, not either side"
    );
    Ok(())
}

/// 🔴 THE STAGING PIN, both halves. Schema imports RESOLVE from the archive's
/// own schema bytes (survival) — and the directory those bytes were staged in
/// is gone when the call returns (absence). The staging parent is a directory
/// this test owns, so an empty listing afterwards is this call's leavings and
/// nobody else's.
#[test]
fn deployed_projection_resolves_archived_schemas_and_leaves_nothing_behind()
-> Result<(), Box<dyn std::error::Error>> {
    let staging_parent = crate::test_support::private_tempdir()?;
    let mut schemas = BTreeMap::new();
    schemas.insert(SCHEMA_PATH.to_owned(), SCHEMA_BYTES.to_vec());

    let projected = projection::project_in(SCHEMA_DOCUMENT, &schemas, staging_parent.path())?;

    assert!(
        projected.ok,
        "the archived schema must resolve: {:?}",
        projected.diagnostics
    );
    assert!(projected.semantic.is_some());
    assert_eq!(
        std::fs::read_dir(staging_parent.path())?.count(),
        0,
        "the staging directory outlived the call that made it"
    );
    Ok(())
}

/// The inverted control for the pin above: WITHOUT the archived schemas the
/// same document does not check. Without this, the staging pin would pass just
/// as well over a projection that ignored schemas entirely.
#[test]
fn the_same_document_without_its_archived_schemas_does_not_resolve()
-> Result<(), Box<dyn std::error::Error>> {
    let staging_parent = crate::test_support::private_tempdir()?;
    let projected =
        projection::project_in(SCHEMA_DOCUMENT, &BTreeMap::new(), staging_parent.path())?;

    assert!(
        !projected.ok,
        "a schema import cannot resolve with no schemas staged; the staging pin would be vacuous"
    );
    Ok(())
}

/// Defence in depth where archived names become filesystem paths: a schema
/// entry that would escape the staging directory refuses the whole document
/// rather than being written, skipped, or sanitised into something else.
#[test]
fn a_schema_path_that_would_escape_staging_refuses_the_document()
-> Result<(), Box<dyn std::error::Error>> {
    let staging_parent = crate::test_support::private_tempdir()?;
    for path in ["../escape.json", "/absolute.json", ""] {
        let mut schemas = BTreeMap::new();
        schemas.insert(path.to_owned(), SCHEMA_BYTES.to_vec());
        let refusal = projection::project_in(SCHEMA_DOCUMENT, &schemas, staging_parent.path())
            .err()
            .ok_or_else(|| format!("schema path `{path}` was staged instead of refused"))?;
        assert!(
            matches!(refusal, DeployedError::UnsafeSchemaPath { .. }),
            "expected an unsafe-path refusal for `{path}`, got {refusal:?}"
        );
    }
    assert_eq!(
        std::fs::read_dir(staging_parent.path())?.count(),
        0,
        "a refused document left a staging directory behind"
    );
    Ok(())
}

/// Archived schemas reach the caller as text alongside the document, so a
/// consumer can show what the deployed version was actually checked against.
#[test]
fn archived_schemas_are_served_with_the_document() -> Result<(), Box<dyn std::error::Error>> {
    let awl = AwlSource::new(
        "schema_probe.awl",
        SCHEMA_DOCUMENT,
        [(SCHEMA_PATH.to_owned(), SCHEMA_BYTES.to_vec())],
    );
    let archives = vec![record(manifest("schema_probe"), Some(awl), 1_700_000_000)?];

    let document = read_document(&archives, "schema_probe", &archives[0].content_hash)?;

    assert_eq!(document.schemas.len(), 1);
    assert_eq!(document.schemas[0].path, SCHEMA_PATH);
    assert_eq!(
        document.schemas[0].text.as_deref(),
        Some(std::str::from_utf8(SCHEMA_BYTES)?)
    );
    assert_eq!(document.schemas[0].byte_length, SCHEMA_BYTES.len());
    assert!(
        document.projection.ok,
        "a schema-importing deployed document must project from its own archived schemas: {:?}",
        document.projection.diagnostics
    );
    Ok(())
}
