//! Proofs about the document this binary carries.
//!
//! Two properties, and they are different: that the EMBEDDED bytes are the
//! tracked artifact and compile into a usable package, and that the
//! session-contract verification can actually fail. The second is not
//! decoration — a check that has never refused anything is a check nobody has
//! measured, and it would pass just as green if the verification loop were
//! deleted.

use std::path::Path;

use super::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The tracked file `include_str!` names.
fn tracked_document() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assistant-embed/assistant.awl")
}

/// The embedded bytes ARE the tracked artifact — not a copy of it, and not a
/// stale snapshot. Read from disk and compared, so editing the document without
/// rebuilding cannot leave the two disagreeing silently.
#[test]
fn the_embedded_document_is_the_tracked_artifact() -> TestResult {
    let on_disk = std::fs::read_to_string(tracked_document())?;
    assert_eq!(
        EMBEDDED_ASSISTANT_DOCUMENT, on_disk,
        "the embedded document must be the bytes of assistant-embed/assistant.awl"
    );
    assert!(
        !EMBEDDED_ASSISTANT_DOCUMENT.is_empty(),
        "an empty embedded document would satisfy every other assertion here"
    );
    Ok(())
}

/// The embedded document compiles, packages, and validates — so a stock binary
/// really carries a deployable assistant rather than a string.
#[test]
fn the_embedded_document_compiles_into_a_validated_package() -> TestResult {
    let embedded = EmbeddedAssistant::load()?;
    assert_eq!(embedded.workflow_type(), "assistant");
    assert_eq!(embedded.source(), EMBEDDED_ASSISTANT_DOCUMENT);
    assert!(
        !embedded.content_hash().to_string().is_empty(),
        "a package with no content hash has no version identity"
    );
    assert_eq!(
        embedded.package().manifest().entry_module,
        embedded.workflow_type(),
        "the workflow type is the manifest's entry module, not a name held beside it"
    );
    Ok(())
}

/// The same source compiles to the same content hash. Identity is the deploy
/// contract: two boots of one binary must not disagree about which version they
/// carry.
#[test]
fn the_embedded_identity_is_deterministic() -> TestResult {
    let first = EmbeddedAssistant::load()?;
    let second = EmbeddedAssistant::load()?;
    assert_eq!(first.content_hash(), second.content_hash());
    Ok(())
}

/// Every surface the operator verbs bind to is declared by the document, with
/// the payload fields the continuation signal really takes.
#[test]
fn the_session_contract_surfaces_are_declared_by_the_document() -> TestResult {
    let embedded = EmbeddedAssistant::load()?;
    // Read the derived shape LITERALLY rather than through the module's own
    // resolver: a test that reuses the implementation to check the
    // implementation would keep passing if both drifted together.
    let schema = embedded.continuation_schema()?;
    let definition = schema["$ref"]
        .as_str()
        .and_then(|reference| reference.strip_prefix("#/$defs/"))
        .map(|name| &schema["$defs"][name])
        .ok_or("the continuation schema must reference its own definition")?;
    for field in [CONTINUE_MESSAGE_FIELD, CONTINUE_END_FIELD] {
        assert!(
            !definition["properties"][field].is_null(),
            "the `{CONTINUE_SIGNAL}` payload schema must declare `{field}`; got {schema}"
        );
    }
    assert!(
        embedded.queries().iter().any(|name| name == STATUS_QUERY),
        "the document must declare the `{STATUS_QUERY}` query; got {:?}",
        embedded.queries()
    );
    let inputs = embedded
        .input_schema()
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .ok_or("the derived input schema must be an object schema")?;
    for input in [OBJECTIVE_INPUT, REPO_PATH_INPUT] {
        assert!(
            inputs.contains_key(input),
            "the start contract must carry `{input}`; got {inputs:?}"
        );
    }
    Ok(())
}

/// THE VACUITY CONTROL. A document that compiles cleanly but renames the
/// objective input is refused BY NAME.
///
/// Without this, every assertion above would pass unchanged if the
/// session-contract verification were deleted outright: the embedded document
/// satisfies it, so it never fires. Renaming the input (and its one use, so the
/// document still typechecks) puts the check on the path and demands it refuse.
#[test]
fn a_document_that_renames_the_objective_input_is_refused_by_name() -> TestResult {
    let renamed: &'static str = Box::leak(
        EMBEDDED_ASSISTANT_DOCUMENT
            .replace("input objective: String", "input goal: String")
            .replace(
                "contract_tail + objective -> opening_prompt",
                "contract_tail + goal -> opening_prompt",
            )
            .into_boxed_str(),
    );
    assert_ne!(
        renamed, EMBEDDED_ASSISTANT_DOCUMENT,
        "the mutation must change the document, or this test measures nothing"
    );
    match EmbeddedAssistant::from_source(renamed) {
        Err(EmbeddedAssistantError::MissingInput { name }) => {
            assert_eq!(name, OBJECTIVE_INPUT);
            Ok(())
        }
        Err(other) => Err(format!(
            "the renamed document must be refused for the MISSING INPUT, not for {other}; a \
             compile failure here would mean the mutation never reached the contract check"
        )
        .into()),
        Ok(_) => Err(format!(
            "a document declaring no `{OBJECTIVE_INPUT}` input was accepted — the session \
             contract verification is not on the load path"
        )
        .into()),
    }
}

/// A document carrying a `schema(…)` import is refused before compilation,
/// naming the import: the binary embeds one file and no directory to resolve
/// against, and resolving one against the server's cwd would be a silent
/// substitution.
#[test]
fn a_document_with_a_schema_import_is_refused_naming_the_import() -> TestResult {
    let with_import: &'static str = Box::leak(
        EMBEDDED_ASSISTANT_DOCUMENT
            .replace(
                "type Provisioned { exit_code: Int, stdout: String }",
                "type Provisioned = schema(\"provisioned.json\")",
            )
            .into_boxed_str(),
    );
    match EmbeddedAssistant::from_source(with_import) {
        Err(EmbeddedAssistantError::SchemaImport { path }) => {
            assert_eq!(path, "provisioned.json");
            Ok(())
        }
        Err(other) => Err(format!("the import must be refused as an import, got: {other}").into()),
        Ok(_) => Err("a document with an unresolvable schema import was accepted".into()),
    }
}

/// The process-wide handle returns the same prepared assistant every call, and
/// it is the one `load` produces.
#[test]
fn the_process_wide_handle_matches_a_direct_load() -> TestResult {
    let shared = embedded_assistant().map_err(ToString::to_string)?;
    let direct = EmbeddedAssistant::load()?;
    assert_eq!(shared.workflow_type(), direct.workflow_type());
    assert_eq!(shared.content_hash(), direct.content_hash());
    assert!(std::ptr::eq(
        shared,
        embedded_assistant().map_err(ToString::to_string)?
    ));
    Ok(())
}

/// The property predicate reads `properties` — directly, or one hop through the
/// local `$defs` reference the deriver emits — and never a bare object key.
#[test]
fn property_detection_reads_the_schema_properties_map() {
    let direct = serde_json::json!({"properties": {"message": {"type": "string"}}});
    assert!(schema_declares_property(&direct, "message"));
    assert!(!schema_declares_property(&direct, "end"));
    assert!(!schema_declares_property(
        &serde_json::json!({"message": {}}),
        "message"
    ));

    let referenced = serde_json::json!({
        "$ref": "#/$defs/Continuation",
        "$defs": {"Continuation": {"properties": {"end": {"type": "boolean"}}}},
    });
    assert!(schema_declares_property(&referenced, "end"));
    assert!(!schema_declares_property(&referenced, "message"));

    // An unresolvable reference is not silently treated as the schema itself.
    let dangling = serde_json::json!({
        "$ref": "#/$defs/Missing",
        "$defs": {},
        "properties": {"end": {"type": "boolean"}},
    });
    assert!(!schema_declares_property(&dangling, "end"));
}
