//! Golden tests for the rev-2 JSON Schema derivation: shorthand records,
//! `?` optionality (never null), enums, doc-comment descriptions, `$defs`
//! references, verbatim re-emission of both schema doors (constraints ride
//! through), and the workflow start contract.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use aion_awl::{
    Document, SchemaError, parse, schema_for_type, schema_for_type_in, schema_for_workflow,
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rev2")
}

fn parse_fixture(relative: &str) -> Result<(Document, PathBuf), Box<dyn Error>> {
    let path = corpus_root().join(relative);
    let source = fs::read_to_string(&path)?;
    let document = parse(&source)?;
    let dir = path.parent().ok_or("fixture has a parent")?.to_path_buf();
    Ok((document, dir))
}

fn required_names(schema: &Value) -> Result<Vec<String>, Box<dyn Error>> {
    Ok(schema["required"]
        .as_array()
        .ok_or("required array")?
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect())
}

#[test]
fn optional_fields_are_omitted_from_required_and_never_nullable() -> TestResult {
    let (document, _) = parse_fixture("schema-doors/valid/optional_shorthand.awl")?;
    let schema = schema_for_type(&document, "Note")?;
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["properties"]["body"]["type"], "string");
    let required = required_names(&schema)?;
    assert!(required.contains(&"title".to_owned()));
    assert!(required.contains(&"tags".to_owned()));
    assert!(!required.contains(&"body".to_owned()));
    // Absence is the optional mechanism: no null, no anyOf, anywhere.
    let rendered = serde_json::to_string(&schema)?;
    assert!(!rendered.contains("null"));
    assert!(!rendered.contains("anyOf"));
    Ok(())
}

#[test]
fn enums_derive_string_enum_schemas() -> TestResult {
    let (document, _) = parse_fixture("header-types/valid/combined.awl")?;
    let schema = schema_for_type(&document, "Stage")?;
    assert_eq!(
        schema,
        json!({ "type": "string", "enum": ["Drafted", "Approved", "Shipped"] })
    );
    Ok(())
}

#[test]
fn doc_comments_flow_to_descriptions_at_type_and_field_level() -> TestResult {
    let (document, _) = parse_fixture("header-types/valid/doc_comments.awl")?;
    let schema = schema_for_type(&document, "Document")?;
    assert_eq!(
        schema["description"],
        "A fetched document, exactly as retrieved. The body is raw bytes decoded as UTF-8."
    );
    assert_eq!(
        schema["properties"]["url"]["description"],
        "Where the document was fetched from."
    );
    assert!(schema["properties"]["body"].get("description").is_none());
    Ok(())
}

#[test]
fn builtins_lists_and_optionals_map_to_json_types() -> TestResult {
    let (document, _) = parse_fixture("header-types/valid/builtins.awl")?;
    let schema = schema_for_type(&document, "Report")?;
    let properties = &schema["properties"];
    assert_eq!(properties["path"]["type"], "string");
    assert_eq!(properties["score"]["type"], "number");
    assert_eq!(properties["attempts"]["type"], "integer");
    assert_eq!(properties["clean"]["type"], "boolean");
    assert_eq!(properties["labels"]["type"], "array");
    assert_eq!(properties["labels"]["items"]["type"], "string");
    assert_eq!(properties["detail"]["type"], "string");
    assert_eq!(properties["extra_labels"]["type"], "array");
    let required = required_names(&schema)?;
    assert!(!required.contains(&"detail".to_owned()));
    assert!(!required.contains(&"extra_labels".to_owned()));
    Ok(())
}

#[test]
fn named_type_references_become_local_defs() -> TestResult {
    let (document, _) = parse_fixture("header-types/valid/combined.awl")?;
    let schema = schema_for_type(&document, "Draft")?;
    assert_eq!(
        schema["properties"]["sections"]["items"]["$ref"],
        "#/$defs/Section"
    );
    assert_eq!(schema["properties"]["stage"]["$ref"], "#/$defs/Stage");
    let section = &schema["$defs"]["Section"];
    assert_eq!(section["type"], "object");
    let section_required = section["required"]
        .as_array()
        .ok_or("Section required array")?;
    assert!(!section_required.iter().any(|name| name == "trimmed_note"));
    assert_eq!(
        schema["$defs"]["Stage"]["enum"],
        json!(["Drafted", "Approved", "Shipped"])
    );
    Ok(())
}

#[test]
fn recursive_records_self_reference_the_schema_root() -> TestResult {
    let source = "\
//! A linked list of nodes.
workflow recursive
  input head: Node
  outcome done: type Node, route success

type Node { label: String, next: Node? }

worker w
  action touch(item: Node) -> Node

step only
  head |> touch |> route done
";
    let document = parse(source)?;
    let schema = schema_for_type(&document, "Node")?;
    assert_eq!(schema["properties"]["next"]["$ref"], "#");
    let required = required_names(&schema)?;
    assert!(!required.contains(&"next".to_owned()));
    Ok(())
}

#[test]
fn imported_schemas_reemit_verbatim_with_constraints_preserved() -> TestResult {
    let (document, dir) = parse_fixture("schema-doors/valid/import_constraints.awl")?;
    let schema = schema_for_type_in(&document, &dir, "Profile")?;
    let raw: Value = serde_json::from_str(&fs::read_to_string(dir.join("profile.schema.json"))?)?;
    assert_eq!(
        schema, raw,
        "the imported schema must re-emit byte-for-value"
    );
    // Constraint keywords ride through untouched.
    assert_eq!(schema["properties"]["handle"]["minLength"], 3);
    assert_eq!(
        schema["properties"]["handle"]["pattern"],
        "^[a-z][a-z0-9_]*$"
    );
    assert_eq!(schema["properties"]["age"]["maximum"], 150);
    assert_eq!(schema["properties"]["email"]["format"], "email");
    Ok(())
}

#[test]
fn inline_schemas_reemit_their_pasted_json_with_constraints() -> TestResult {
    let (document, _) = parse_fixture("schema-doors/valid/mixed_doors.awl")?;
    let schema = schema_for_type(&document, "IntakeConfig")?;
    assert_eq!(
        schema,
        json!({
            "type": "object",
            "required": ["max_attachments"],
            "properties": {
                "max_attachments": { "type": "integer", "minimum": 0 },
                "archive_dir":     { "type": "string" }
            }
        })
    );
    Ok(())
}

#[test]
fn workflow_contract_covers_inputs_with_optionality_and_narration() -> TestResult {
    let (document, _) = parse_fixture("header-types/valid/builtins.awl")?;
    let schema = schema_for_workflow(&document)?;
    assert_eq!(schema["type"], "object");
    assert_eq!(
        schema["description"],
        "Analyze a workspace snapshot and report on it, exercising every builtin type."
    );
    let properties = &schema["properties"];
    assert_eq!(properties["dir"]["type"], "string");
    assert_eq!(properties["limit"]["type"], "number");
    assert_eq!(properties["tries"]["type"], "integer");
    assert_eq!(properties["tags"]["type"], "array");
    assert_eq!(properties["note"]["type"], "string");
    let required = required_names(&schema)?;
    assert_eq!(required, ["dir", "limit", "tries", "tags"]);
    Ok(())
}

#[test]
fn import_derivation_without_a_root_refuses() -> TestResult {
    let (document, _) = parse_fixture("schema-doors/valid/import_constraints.awl")?;
    let error = match schema_for_type(&document, "Profile") {
        Ok(schema) => return Err(format!("derived without a root: {schema}").into()),
        Err(error) => error,
    };
    assert!(matches!(error, SchemaError::ImportUnresolved { .. }));
    assert!(error.to_string().contains("profile.schema.json"));
    Ok(())
}

#[test]
fn unknown_types_are_refused_by_name() -> TestResult {
    let (document, _) = parse_fixture("header-types/valid/builtins.awl")?;
    let error = match schema_for_type(&document, "Ghost") {
        Ok(schema) => return Err(format!("derived a ghost type: {schema}").into()),
        Err(error) => error,
    };
    assert!(matches!(error, SchemaError::UnknownType { .. }));
    assert!(error.to_string().contains("Ghost"));
    Ok(())
}

/// A schema door is a COMPLETE JSON Schema document written on its own terms,
/// so its internal `$ref`s are rooted at ITS root. Composed into a workflow's
/// start contract the door lands at `#/$defs/<Type>`, and an unrebased
/// `#/$defs/Line` then points at the CONTRACT's root, where there is no such
/// entry.
///
/// The consequence is not a loud one, which is why this test asserts the
/// compiled validator rather than only the pointer text: the engine's answer
/// to a schema it cannot compile is to log and admit the payload UNCHECKED
/// (`aion/src/loader/catalog_admission.rs`). An unresolvable contract is
/// therefore a workflow running with its input validation silently off.
#[test]
fn an_imported_schemas_internal_refs_survive_composition() -> TestResult {
    let (document, dir) = parse_fixture("schema-doors/valid/import_nested_defs.awl")?;
    let schema = aion_awl::schema_for_workflow_in(&document, &dir)?;

    let door = schema
        .pointer("/$defs/Order")
        .ok_or("the imported door lands at #/$defs/Order")?;
    assert_eq!(
        door.pointer("/properties/lines/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/Order/$defs/Line"),
        "the door's own `#/$defs/Line` must be re-rooted under where the door \
         now lives, not left pointing at the contract root:\n{schema:#}"
    );
    assert!(
        schema.pointer("/$defs/Order/$defs/Line").is_some(),
        "the target the rebased pointer names must actually be there:\n{schema:#}"
    );

    // The property this whole thing exists for: the derived contract compiles
    // into a validator, so the engine can actually enforce it.
    aion_package::schema_is_usable(&schema)
        .map_err(|reason| format!("derived contract does not compile: {reason}"))?;

    // Non-vacuity: the assertion above would pass on a contract with no refs
    // at all, so prove the pointer being resolved is one the door really wrote.
    let raw: Value = serde_json::from_str(&fs::read_to_string(dir.join("order.schema.json"))?)?;
    assert_eq!(
        raw.pointer("/properties/lines/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/Line"),
        "the fixture must still be the door-rooted form this rebases FROM"
    );
    Ok(())
}

/// A door whose regex the engine's validator cannot compile is refused when
/// the document is checked, rather than deploying and validating nothing.
#[test]
fn a_door_the_validator_cannot_compile_is_refused_at_check() -> TestResult {
    let source = "\
//! A door whose pattern the engine cannot compile.
workflow uncompilable_pattern
  input reading: Reading
  outcome filed: type Filed, route success

type Reading = schema {
  \"type\": \"object\",
  \"required\": [\"tag\"],
  \"properties\": { \"tag\": { \"type\": \"string\", \"pattern\": \"^{[0-9]+}$\" } }
}

type Filed { note: String }

worker doors
  action file_reading(reading: Reading) -> Filed

step file
  reading |> file_reading |> route filed
";
    let document = parse(source)?;
    let errors = aion_awl::check(&document);
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("cannot be compiled into a validator")),
        "an uncompilable door must be a check-time diagnostic, not a silent \
         deploy: {errors:?}"
    );

    // Control: the SAME document with the braces escaped checks clean, so the
    // refusal is about the pattern and not about the surrounding shape.
    let escaped = source.replace("^{[0-9]+}$", "^\\\\{[0-9]+\\\\}$");
    assert_ne!(escaped, source, "the control must actually differ");
    let ok_document = parse(&escaped)?;
    assert!(
        aion_awl::check(&ok_document).is_empty(),
        "the escaped control must check clean: {:?}",
        aion_awl::check(&ok_document)
    );
    Ok(())
}

/// The contract is more than the workflow header, and `check` must prove ALL
/// of it.
///
/// A type that appears only as a signal payload, a child's parameter or an
/// activity's result never reaches the workflow input, outcome or query
/// schemas — so a checker that derived only those three would pass this
/// document and let the compiler refuse it later, at `emit` or `deploy`. That
/// is a type error met at the wrong door: the author is no longer looking at
/// the file.
///
/// Each surface below carries its OWN uncompilable type, so a check that
/// covers some of them and not others fails here rather than passing on the
/// one it happens to cover.
#[test]
fn check_proves_the_signal_child_and_activity_schemas_too() -> TestResult {
    const BAD: &str = "\"type\": \"string\", \"pattern\": \"^{[0-9]+}$\"";
    const GOOD: &str = "\"type\": \"string\"";
    let source = format!(
        "\
//! Uncompilable types reachable ONLY through signal, child and activity.
workflow contract_surfaces
  input label: String
  signal control: Control
  outcome filed: type Filed, route success

type Control {{ token: Token }}
type Token = schema {{ {BAD} }}
type Filed {{ note: String }}
type Ticket = schema {{ {BAD} }}
type Receipt = schema {{ {BAD} }}

worker doors
  action file_reading(ticket: Ticket) -> Filed

child sub(note: String) -> Receipt

step file
  wait control -> answer
  label |> file_reading |> route filed
"
    );

    let document = parse(&source)?;
    let errors = aion_awl::check(&document);
    let refusals = errors
        .iter()
        .filter(|error| {
            error
                .message
                .contains("cannot be compiled into a validator")
        })
        .count();
    assert!(
        refusals >= 3,
        "each of the signal payload, the activity parameter and the child result \
         must be proven; got {refusals} refusals from: {errors:?}"
    );

    // CONTROL. The same document with every pattern removed checks clean, so
    // the refusals above are about the schemas and not about the shape of a
    // document that declares signals, children and workers at all.
    let clean = source.replace(BAD, GOOD);
    assert_ne!(clean, source, "the control must actually differ");
    let clean_document = parse(&clean)?;
    assert!(
        aion_awl::check(&clean_document).is_empty(),
        "the control must check clean: {:?}",
        aion_awl::check(&clean_document)
    );
    Ok(())
}
