//! LEG 1 proof obligation: a deployed AWL archive CARRIES the document it was
//! compiled from, and keeps carrying it across the persistence rewrite the
//! engine performs on every deploy (`Package::to_archive_bytes`).
//!
//! Every document here is a tracked corpus fixture, compiled through the real
//! `compile_and_assemble_awl` seam — the same call `aion deploy <file>.awl`
//! makes — so what is proved is the shipped path, not a bench rig.

use std::fs;
use std::path::{Path, PathBuf};

use aion_awl_package::{AwlAssembleOptions, assemble_awl, compile_and_assemble_awl};
use aion_package::{ExtractionLimits, Package};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fixture_dir(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../aion-awl/tests/fixtures/rev2")
        .join(relative)
}

/// The document with NESTED schema imports (`schemas/brief.schema.json`) —
/// the case that proves relative structure survives, not just a flat filename.
#[test]
fn a_deployed_document_and_its_nested_schemas_survive_the_persistence_rewrite() -> TestResult {
    let root = fixture_dir("flagship/valid");
    let source = fs::read_to_string(root.join("dev_brief.awl"))?;

    let prepared = compile_and_assemble_awl(&source, &root, "dev_brief.awl")?;
    let package = Package::load_from_bytes(&prepared.archive, ExtractionLimits::unbounded())?;

    let carried = package
        .awl()
        .ok_or("the assembled archive carries no AWL source")?;
    assert_eq!(carried.document_name(), "dev_brief.awl");
    assert_eq!(
        carried.document(),
        source,
        "the archived document is not byte-identical to what was compiled"
    );
    let paths: Vec<&str> = carried.schemas().keys().map(String::as_str).collect();
    assert_eq!(
        paths,
        vec![
            "schemas/brief.schema.json",
            "schemas/run_config.schema.json"
        ],
        "the archived schema paths are not the document-relative ones"
    );
    for path in &paths {
        assert_eq!(
            carried.schemas().get(*path),
            Some(&fs::read(root.join(path))?),
            "archived schema `{path}` is not byte-identical to the file on disk"
        );
    }

    // The rewrite the engine performs when it persists a deploy.
    let rewritten = package.to_archive_bytes()?;
    let reloaded = Package::load_from_bytes(&rewritten, ExtractionLimits::unbounded())?;

    assert_eq!(
        reloaded.awl(),
        package.awl(),
        "the persistence rewrite lost or altered the archived AWL source"
    );
    assert_eq!(
        reloaded.content_hash(),
        package.content_hash(),
        "the persistence rewrite changed the package version"
    );
    Ok(())
}

/// A sibling import lands beside the document, under its bare filename.
#[test]
fn a_sibling_schema_import_is_archived_beside_the_document() -> TestResult {
    let root = fixture_dir("schema-doors/valid");
    let source = fs::read_to_string(root.join("mixed_doors.awl"))?;

    let prepared = compile_and_assemble_awl(&source, &root, "mixed_doors.awl")?;
    let package = Package::load_from_bytes(prepared.archive, ExtractionLimits::unbounded())?;

    let carried = package
        .awl()
        .ok_or("the assembled archive carries no AWL source")?;
    assert_eq!(carried.document_name(), "mixed_doors.awl");
    assert_eq!(carried.document(), source);
    assert_eq!(
        carried.schemas().get("intake.schema.json"),
        Some(&fs::read(root.join("intake.schema.json"))?)
    );
    assert_eq!(carried.schemas().len(), 1);
    Ok(())
}

/// A document with no imports still carries itself, with an empty schema set.
#[test]
fn a_document_without_imports_still_carries_its_own_source() -> TestResult {
    let root = fixture_dir("flagship/valid");
    let source = fs::read_to_string(root.join("awl_hello.awl"))?;

    let prepared = compile_and_assemble_awl(&source, &root, "awl_hello.awl")?;
    let package = Package::load_from_bytes(prepared.archive, ExtractionLimits::unbounded())?;

    let carried = package
        .awl()
        .ok_or("the assembled archive carries no AWL source")?;
    assert_eq!(carried.document(), source);
    assert!(carried.schemas().is_empty());
    Ok(())
}

/// THE RIDER at the shipped seam: the archived document is provenance, not
/// identity. The same compiled workflow assembled WITH and WITHOUT its source
/// produces the same package version, so embedding source cannot re-version a
/// deployed workflow or orphan the runs pinned to it.
#[test]
fn archiving_the_document_does_not_change_the_package_version() -> TestResult {
    let root = fixture_dir("flagship/valid");
    let source = fs::read_to_string(root.join("dev_brief.awl"))?;
    let compiled = aion_awl::compile(&source, &root)?;

    let bare = assemble_awl(
        &compiled,
        AwlAssembleOptions {
            timeout: compiled.timeout,
            ..AwlAssembleOptions::default()
        },
    )?;
    let carrying = compile_and_assemble_awl(&source, &root, "dev_brief.awl")?.archive;

    let bare_package = Package::load_from_bytes(bare, ExtractionLimits::unbounded())?;
    let carrying_package = Package::load_from_bytes(carrying, ExtractionLimits::unbounded())?;

    assert_eq!(bare_package.awl(), None);
    assert!(carrying_package.awl().is_some());
    assert_eq!(
        bare_package.content_hash(),
        carrying_package.content_hash(),
        "archiving the document changed the package version"
    );
    assert_eq!(
        bare_package.manifest().version,
        carrying_package.manifest().version,
        "archiving the document changed the stamped manifest version"
    );
    Ok(())
}

/// Assembly stays deterministic with provenance in the archive: the same
/// document compiled and assembled twice is byte-identical.
#[test]
fn assembly_with_archived_source_is_byte_deterministic() -> TestResult {
    let root = fixture_dir("flagship/valid");
    let source = fs::read_to_string(root.join("dev_brief.awl"))?;

    let first = compile_and_assemble_awl(&source, &root, "dev_brief.awl")?.archive;
    let second = compile_and_assemble_awl(&source, &root, "dev_brief.awl")?.archive;

    assert_eq!(first, second);
    Ok(())
}
