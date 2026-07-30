//! Emitted artifacts are canonical by construction.
//!
//! Every byte AWL emits — the Gleam source, the packaging sidecar, and the
//! `schema of <Type>` text frozen into that source — is derived from JSON
//! documents held in `serde_json` maps. A `serde_json::Map` is an `IndexMap`
//! (insertion-ordered) when the `preserve_order` feature is resolved anywhere
//! in the build graph and a `BTreeMap` (key-sorted) when it is not, so any
//! emitter that let map iteration order reach its output would emit different
//! bytes depending on which unrelated crates happen to share the build. That
//! would make recompiling the same `.awl` document produce a different package
//! version, because the content hash covers the compiled BEAM bytes.
//!
//! These tests pin the fix from both directions:
//!
//! - permuting the authored key order of a schema door must not change one byte
//!   of the emitted source or sidecar (fails under `preserve_order` if any
//!   emitter reads map iteration order); and
//! - the emitted bytes must match hand-written key-sorted expectations (fails
//!   in either feature configuration if the sort is dropped).

use aion_awl::{emit_artifact, parse, schema_for_type};

/// A document whose schema door authors its properties in a deliberately
/// non-alphabetical order, with `PROPERTIES` the substitution point so the same
/// document can be re-authored in a different order.
const DOCUMENT: &str = r#"
//! Canonical-emission proof.
workflow canon_flow
  input ticket: Ticket
  outcome filed: type Filed, route success

const ticket_schema = schema of Ticket

type Ticket = schema {
  "type": "object",
  "required": ["zulu", "alpha"],
  "properties": {
PROPERTIES
  }
}

type Filed { note: String }

worker desk
  action file(ticket: Ticket, contract: String) -> Filed

step submit
  file(ticket: ticket, contract: ticket_schema) -> filed
  route filed(note: filed.note)
"#;

/// The three properties, authored alphabetically.
const SORTED_PROPERTIES: &str = r#"    "alpha": { "type": "string" },
    "mike":  { "type": "integer" },
    "zulu":  { "type": "boolean" }"#;

/// The same three properties, authored in reverse.
const REVERSED_PROPERTIES: &str = r#"    "zulu":  { "type": "boolean" },
    "mike":  { "type": "integer" },
    "alpha": { "type": "string" }"#;

/// The emitted artifact for one authored property order.
fn emitted(properties: &str) -> Result<aion_awl::EmittedArtifact, Box<dyn std::error::Error>> {
    let source = DOCUMENT.replace("PROPERTIES", properties);
    Ok(emit_artifact(&parse(&source)?)?)
}

#[test]
fn authored_key_order_cannot_change_the_emitted_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let sorted = emitted(SORTED_PROPERTIES)?;
    let reversed = emitted(REVERSED_PROPERTIES)?;

    assert_eq!(
        sorted.source, reversed.source,
        "permuting a schema door's authored key order changed the emitted Gleam source"
    );
    assert_eq!(
        serde_json::to_vec_pretty(&sorted.project_metadata())?,
        serde_json::to_vec_pretty(&reversed.project_metadata())?,
        "permuting a schema door's authored key order changed the emitted sidecar"
    );
    Ok(())
}

#[test]
fn projected_record_fields_are_emitted_in_key_order() -> Result<(), Box<dyn std::error::Error>> {
    // The projected field order is program structure: it is the generated Gleam
    // record's field order and its decoder's field sequence. Pinned as the
    // schema's key order, which is the only order both `serde_json` map
    // representations can agree on.
    for properties in [SORTED_PROPERTIES, REVERSED_PROPERTIES] {
        let source = emitted(properties)?.source;
        let record = source
            .split("pub type Ticket {")
            .nth(1)
            .ok_or("the emitted source declares no `Ticket` record")?;
        let alpha = record.find("alpha:").ok_or("no `alpha` field")?;
        let mike = record.find("mike:").ok_or("no `mike` field")?;
        let zulu = record.find("zulu:").ok_or("no `zulu` field")?;
        assert!(
            alpha < mike && mike < zulu,
            "projected record fields are not in key order:\n{record}"
        );
    }
    Ok(())
}

#[test]
fn folded_schema_text_is_key_sorted_in_the_emitted_source() -> Result<(), Box<dyn std::error::Error>>
{
    // `const ticket_schema = schema of Ticket` folds to a JSON string literal
    // baked into the emitted Gleam — and therefore into the compiled BEAM bytes
    // the package content hash covers. The expectation below is written by hand
    // in key order, so it holds with `serde_json/preserve_order` on and off.
    let expected = "{\\\"properties\\\":{\\\"alpha\\\":{\\\"type\\\":\\\"string\\\"},\
                    \\\"mike\\\":{\\\"type\\\":\\\"integer\\\"},\
                    \\\"zulu\\\":{\\\"type\\\":\\\"boolean\\\"}},\
                    \\\"required\\\":[\\\"zulu\\\",\\\"alpha\\\"],\
                    \\\"type\\\":\\\"object\\\"}";
    for properties in [SORTED_PROPERTIES, REVERSED_PROPERTIES] {
        let source = emitted(properties)?.source;
        assert!(
            source.contains(expected),
            "the folded `schema of` text is not key-sorted; emitted source:\n{source}"
        );
    }
    Ok(())
}

#[test]
fn the_sidecar_matches_a_key_sorted_expectation() -> Result<(), Box<dyn std::error::Error>> {
    // A `distribute` step synthesizes a child entry, so this document emits a
    // sidecar with nested schema documents. The expectation is authored in key
    // order at every depth.
    let source = r"
//! Sidecar canonicality proof.
workflow canon_fan
  input items: [String]
  outcome done: type Done, route success

type Done { count: Int }

worker desk
  action stamp(item: String) -> String
  action seal(item: String) -> String

step fan
  distribute item in items
step one
  stamp(item: item) -> stamped
step two
  seal(item: stamped) -> sealed
step gather
  collect sealed -> seals
  seals |> count -> total
  route done(count: total)
";
    let artifact = emit_artifact(&parse(source)?)?;
    let sidecar = String::from_utf8(serde_json::to_vec_pretty(&artifact.project_metadata())?)?;
    let expected = r#"{
  "entry_module": "canon_fan",
  "format_version": 1,
  "synthesized_workflows": [
    {
      "entry_function": "aion_internal_awl_child_canon_fan_fan_0_run",
      "entry_module": "canon_fan",
      "input_schema": {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "additionalProperties": false,
        "properties": {
          "item": {
            "type": "string"
          }
        },
        "required": [
          "item"
        ],
        "type": "object"
      },
      "internal": true,
      "output_schema": {
        "type": "string"
      },
      "timeout_seconds": null,
      "workflow_type": "aion_internal_awl_child_canon_fan_fan_0"
    }
  ]
}"#;
    assert_eq!(sidecar, expected, "the emitted sidecar is not key-sorted");
    Ok(())
}

#[test]
fn derived_schemas_serialize_key_sorted() -> Result<(), Box<dyn std::error::Error>> {
    // The `aion awl schema` surface and the manifest/contract records all
    // serialize derived schemas. Authored field order is the record's content,
    // so `required` keeps it; the object keys are sorted.
    let document = parse(
        r"
//! Derived-schema canonicality proof.
workflow canon_derive
  input note: Note
  outcome done: type String, route success

type Note {
  zulu: String,
  alpha: Int,
}

worker desk
  action stamp(note: Note) -> String

step submit
  stamp(note: note) -> stamped
  route done(stamped)
",
    )?;
    let schema = schema_for_type(&document, "Note")?;
    assert_eq!(
        serde_json::to_string(&aion_package::CanonicalJson::new(schema))?,
        r#"{"properties":{"alpha":{"type":"integer"},"zulu":{"type":"string"}},"required":["zulu","alpha"],"type":"object"}"#
    );
    Ok(())
}
