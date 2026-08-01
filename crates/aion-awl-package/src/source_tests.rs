//! Unit coverage for the AWL provenance collector.

use std::path::Path;

use super::{AwlSourceError, collect_awl_source};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fixture_root(relative: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../aion-awl/tests/fixtures/rev2")
        .join(relative)
}

/// A document whose imports sit in a SUBDIRECTORY keeps that nesting: the
/// archived key is the path the document's own `schema("…")` line names, so a
/// consumer can stage the tree and re-check the document unchanged.
#[test]
fn nested_schema_imports_keep_their_document_relative_paths() -> TestResult {
    let root = fixture_root("flagship/valid");
    let source = std::fs::read_to_string(root.join("dev_brief.awl"))?;

    let collected = collect_awl_source("dev_brief.awl", &source, &root)?;

    assert_eq!(collected.document_name(), "dev_brief.awl");
    assert_eq!(collected.document(), source);
    let paths: Vec<&str> = collected.schemas().keys().map(String::as_str).collect();
    assert_eq!(
        paths,
        vec![
            "schemas/brief.schema.json",
            "schemas/run_config.schema.json"
        ]
    );
    assert_eq!(
        collected.schemas().get("schemas/brief.schema.json"),
        Some(&std::fs::read(root.join("schemas/brief.schema.json"))?)
    );
    Ok(())
}

/// A sibling import is archived under its bare filename.
#[test]
fn sibling_schema_import_is_archived_beside_the_document() -> TestResult {
    let root = fixture_root("schema-doors/valid");
    let source = std::fs::read_to_string(root.join("mixed_doors.awl"))?;

    let collected = collect_awl_source("mixed_doors.awl", &source, &root)?;

    let paths: Vec<&str> = collected.schemas().keys().map(String::as_str).collect();
    assert_eq!(paths, vec!["intake.schema.json"]);
    Ok(())
}

/// A document with no `schema(…)` import archives its text and nothing else —
/// an empty schema set, not a missing record.
#[test]
fn a_document_without_imports_collects_no_schemas() -> TestResult {
    let root = fixture_root("flagship/valid");
    let source = std::fs::read_to_string(root.join("awl_hello.awl"))?;

    let collected = collect_awl_source("awl_hello.awl", &source, &root)?;

    assert!(collected.schemas().is_empty());
    assert_eq!(collected.document(), source);
    Ok(())
}

/// An import that climbs out of the document's directory has no
/// document-relative path, so it is REFUSED rather than silently dropped —
/// an archive that omitted it would claim a provenance it does not carry.
#[test]
fn an_import_above_the_document_root_is_refused() -> TestResult {
    let directory = tempfile::tempdir()?;
    std::fs::write(directory.path().join("shared.schema.json"), b"{}")?;
    let nested = directory.path().join("flow");
    std::fs::create_dir(&nested)?;
    let source = concat!(
        "//! Collector probe.\n",
        "workflow escaping\n",
        "  input ticket: Ticket\n",
        "  outcome done: type Ticket, route success\n",
        "\n",
        "type Ticket = schema(\"../shared.schema.json\")\n",
    );

    let result = collect_awl_source("escaping.awl", source, &nested);

    assert!(
        matches!(
            &result,
            Err(AwlSourceError::UnarchivableImport { path }) if path == "../shared.schema.json"
        ),
        "expected an unarchivable-import refusal, got {result:?}"
    );
    Ok(())
}

/// A `./` prefix names the same file and is normalised away, so the archived
/// key is the plain relative path a consumer stages at.
#[test]
fn a_current_directory_prefix_normalises_to_the_plain_path() -> TestResult {
    let directory = tempfile::tempdir()?;
    std::fs::write(
        directory.path().join("ticket.schema.json"),
        br#"{"type":"object"}"#,
    )?;
    let source = concat!(
        "//! Collector probe.\n",
        "workflow dotted\n",
        "  input ticket: Ticket\n",
        "  outcome done: type Ticket, route success\n",
        "\n",
        "type Ticket = schema(\"./ticket.schema.json\")\n",
    );

    let collected = collect_awl_source("dotted.awl", source, directory.path())?;

    let paths: Vec<&str> = collected.schemas().keys().map(String::as_str).collect();
    assert_eq!(paths, vec!["ticket.schema.json"]);
    assert_eq!(
        collected.schemas().get("ticket.schema.json"),
        Some(&br#"{"type":"object"}"#.to_vec())
    );
    Ok(())
}

/// A filename carrying a directory part cannot name the document entry: the
/// document is always at the root of its own directory.
#[test]
fn a_document_filename_with_a_directory_part_is_refused() {
    let result = collect_awl_source("nested/doc.awl", "workflow noop\n", Path::new("."));

    assert!(
        matches!(
            &result,
            Err(AwlSourceError::InvalidDocumentName { name }) if name == "nested/doc.awl"
        ),
        "expected an invalid-document-name refusal, got {result:?}"
    );
}

/// An import the compiler read but that has since vanished is reported, never
/// swallowed into an archive with a missing schema.
#[test]
fn an_unreadable_import_is_reported() -> TestResult {
    let directory = tempfile::tempdir()?;
    let source = concat!(
        "//! Collector probe.\n",
        "workflow missing\n",
        "  input ticket: Ticket\n",
        "  outcome done: type Ticket, route success\n",
        "\n",
        "type Ticket = schema(\"gone.schema.json\")\n",
    );

    let result = collect_awl_source("missing.awl", source, directory.path());

    assert!(
        matches!(
            &result,
            Err(AwlSourceError::UnreadableImport { path, .. }) if path == "gone.schema.json"
        ),
        "expected an unreadable-import refusal, got {result:?}"
    );
    Ok(())
}
