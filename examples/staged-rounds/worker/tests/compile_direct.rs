//! THE direct-path compile proof: the staged-rounds document must compile
//! and assemble on the exact seam `aion deploy <file.awl>` uses
//! (`compile_and_assemble_awl`). `aion awl check` green does NOT imply
//! compile-green — the checker accepts more than the lowering — so this
//! seam test is the gate of record for the document's shape.

use std::path::Path;
use std::time::Duration;

use aion_awl_package::compile_and_assemble_awl;

/// The document under proof, relative to this crate's manifest.
const DOCUMENT: &str = "../awl/staged_rounds.awl";

/// The example start input shipped alongside the document.
const EXAMPLE_INPUT: &str = "../awl/example-input.json";

fn document_source() -> anyhow::Result<(String, std::path::PathBuf)> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest_dir.join(DOCUMENT);
    let source = std::fs::read_to_string(&path)?;
    let doc_dir = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("document path has no parent directory"))?
        .to_path_buf();
    Ok((source, doc_dir))
}

#[test]
fn staged_rounds_document_compiles_and_assembles_on_the_direct_path() -> anyhow::Result<()> {
    let (source, doc_dir) = document_source()?;
    let prepared = compile_and_assemble_awl(&source, &doc_dir)?;
    assert_eq!(prepared.compiled.workflow_name, "staged_rounds");
    assert_eq!(
        prepared.compiled.first_worker.as_deref(),
        Some("staged_rounds"),
        "the first worker name is the document-derived deploy task queue"
    );
    assert_eq!(
        prepared.compiled.timeout,
        Some(Duration::from_hours(12)),
        "the declared 12h workflow timeout rides through compilation"
    );
    assert!(
        !prepared.archive.is_empty(),
        "the assembled .aion archive must be non-empty"
    );
    assert!(
        !prepared.compiled.beam_bytes.is_empty(),
        "the direct lowering must produce bytecode"
    );
    Ok(())
}

/// The document's ELEVEN actions and the node each pins, in document order.
/// This is the CONTRACT the worker's composition root mirrors (`main.rs`'s
/// `roles` + `main_shell_node.rs`'s registry, guarded from the worker side by
/// `node_mapping_is_exhaustive_and_injective`): the server routes by
/// (namespace, `task_queue`, node) and matches these strings blindly, so a
/// drift on either side strands activities on handlerless connections.
///
/// Note that `remediate` and `fix_integration` deliberately SHARE node
/// `remediation` — both resume the planner's session — which is exactly why the
/// worker must serve both from ONE connection registered on that node.
const EXPECTED_ACTIONS: [(&str, &str); 11] = [
    ("planner", "planner"),
    ("provision_item", "shell"),
    ("dev_item", "developer"),
    ("gate_item", "shell"),
    ("plan_reviews", "shell"),
    ("review_item", "reviewer"),
    ("fold_phase", "shell"),
    ("merge_branches", "shell"),
    ("remediate", "remediation"),
    ("verify_integration", "shell"),
    ("fix_integration", "remediation"),
];

#[test]
fn the_eleven_actions_pin_their_nodes() -> anyhow::Result<()> {
    let (source, doc_dir) = document_source()?;
    let prepared = compile_and_assemble_awl(&source, &doc_dir)?;
    let rows: Vec<(String, String, Option<String>)> = prepared
        .compiled
        .actions
        .iter()
        .map(|requirement| {
            (
                requirement.task_queue.clone(),
                requirement.action.clone(),
                requirement.node.clone(),
            )
        })
        .collect();
    let expected: Vec<(String, String, Option<String>)> = EXPECTED_ACTIONS
        .into_iter()
        .map(|(action, node)| {
            (
                "staged_rounds".to_owned(),
                action.to_owned(),
                Some(node.to_owned()),
            )
        })
        .collect();
    assert_eq!(
        rows, expected,
        "every action pins exactly the node table the worker registers"
    );
    Ok(())
}

#[test]
fn the_input_schema_covers_material_and_config() -> anyhow::Result<()> {
    let (source, doc_dir) = document_source()?;
    let prepared = compile_and_assemble_awl(&source, &doc_dir)?;
    let schema = &prepared.compiled.input_schema;
    let required = schema["required"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("input schema carries no required array"))?;
    for name in ["material", "config"] {
        assert!(
            required.iter().any(|entry| entry == name),
            "input schema must require top-level `{name}`; got {required:?}"
        );
        assert!(
            schema["properties"][name].is_object(),
            "input schema must describe top-level `{name}`"
        );
    }
    Ok(())
}

#[test]
fn example_input_validates_against_the_compiled_contract() -> anyhow::Result<()> {
    let (source, doc_dir) = document_source()?;
    let prepared = compile_and_assemble_awl(&source, &doc_dir)?;
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let input_text = std::fs::read_to_string(manifest_dir.join(EXAMPLE_INPUT))?;
    let input: serde_json::Value = serde_json::from_str(&input_text)?;
    let validator = jsonschema::validator_for(&prepared.compiled.input_schema)
        .map_err(|error| anyhow::anyhow!("input schema does not build a validator: {error}"))?;
    let errors: Vec<String> = validator
        .iter_errors(&input)
        .map(|error| format!("{error} at {}", error.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "example-input.json must satisfy the compiled start contract: {errors:?}"
    );
    Ok(())
}
