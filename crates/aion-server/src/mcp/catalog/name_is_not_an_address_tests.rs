//! The label rule, made an INSTRUMENT rather than a sentence (#211).
//!
//! `aion.display_name` is an operator-facing label over a workflow's UUID
//! identity. The rule is about ACTS, not about whether a name may appear on the
//! wire:
//!
//! 1. **No operation acts on a name.** Every act-verb takes a UUID target.
//! 2. **Name-shaped FILTERING returns lists and acts on nothing.** The display
//!    name is an ordinary search attribute, so filtering on it is supported and
//!    deliberate — it is the first half of a picker.
//! 3. **The composition is prohibited.** Nothing may compose name-filter →
//!    take-first → act.
//!
//! A rule that lives only in prose is a stale comment waiting to happen, so the
//! one surface where the rule is expressed as DATA — the published MCP tool
//! schemas — checks itself here. The predicate below is the same one the
//! narrative sites (`aion_core::DISPLAY_NAME_ATTRIBUTE` and
//! `docs/operations/API.md`) state for the surfaces that are Rust types rather
//! than data:
//!
//! > For every verb, route, RPC, tool, and SDK function that CHANGES state, the
//! > target is identified by a UUID; a display name appears only as a value
//! > being SET, never as the selector; and no single call turns a name into an
//! > act.
//!
//! **Coverage boundary, stated so this file is not mistaken for a whole-system
//! gate:** it covers the MCP tool catalogue only. The HTTP routes, the gRPC
//! service, the CLI verbs, and the caller SDKs are Rust types and function
//! signatures, not enumerable data, and are held to the same predicate by
//! reading. Point 3 — the composition prohibition — is a contract on callers
//! and cannot be measured from inside the server at all; it is written into
//! `docs/design/aion-clients/CLIENT-CONTRACT.md` as a pre-made design decision.

use aion_mcp::tools::service::CatalogError;
use serde_json::Value;

use super::aion_tool_catalog;

/// The property name a display name is carried under, everywhere it appears.
const DISPLAY_NAME_PROPERTY: &str = "display_name";
/// The property name a workflow target is carried under, everywhere it appears.
const WORKFLOW_TARGET_PROPERTY: &str = "workflow_id";

fn properties(schema: &Value) -> Vec<&str> {
    schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|map| map.keys().map(String::as_str).collect())
        .unwrap_or_default()
}

fn required(schema: &Value) -> Vec<&str> {
    schema
        .get("required")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

/// A workflow target must be UUID-shaped, and the schema says so with a
/// `pattern` — a bare `"type": "string"` would accept a name.
fn is_uuid_shaped(schema: &Value, property: &str) -> bool {
    schema
        .get("properties")
        .and_then(|properties| properties.get(property))
        .and_then(|property| property.get("pattern"))
        .and_then(Value::as_str)
        .is_some_and(|pattern| pattern.contains("[0-9a-fA-F]{8}-"))
}

/// VACUITY CONTROL for the whole file.
///
/// Every assertion below searches the schemas for two literal property names.
/// A typo in either — or a rename of the underlying field — would make every
/// other test in this file pass while looking at nothing, which is the failure
/// mode a rule-as-instrument exists to avoid. So the strings are proven to
/// match something real BEFORE they are used to prove an absence.
#[test]
fn the_properties_this_file_reasons_about_exist_in_the_published_schemas()
-> Result<(), CatalogError> {
    let catalog = aion_tool_catalog()?;
    let tools = catalog.tools();

    assert!(!tools.is_empty(), "the catalog published no tools at all");
    assert!(
        tools.iter().any(|tool| tool.annotations.modification
            == aion_mcp::tools::descriptor::Modification::Mutating),
        "the catalog published no mutating tool, so every act-verb assertion below is vacuous"
    );
    assert!(
        tools
            .iter()
            .any(|tool| properties(&tool.input_schema).contains(&DISPLAY_NAME_PROPERTY)),
        "no published tool carries a `{DISPLAY_NAME_PROPERTY}` property; the name this file \
         searches for does not match the schemas, so its absence proves nothing"
    );
    assert!(
        tools
            .iter()
            .any(|tool| properties(&tool.input_schema).contains(&WORKFLOW_TARGET_PROPERTY)),
        "no published tool carries a `{WORKFLOW_TARGET_PROPERTY}` property; the name this file \
         searches for does not match the schemas"
    );
    Ok(())
}

/// Point 1, the target half: wherever a tool addresses an existing workflow, it
/// does so with a REQUIRED, UUID-patterned identifier.
///
/// Required matters as much as UUID-shaped: an optional target is a target the
/// call can proceed without, which would leave something else in the request to
/// choose the referent.
#[test]
fn a_tool_that_addresses_a_workflow_addresses_it_by_a_required_uuid() -> Result<(), CatalogError> {
    let catalog = aion_tool_catalog()?;

    for tool in catalog.tools() {
        if !properties(&tool.input_schema).contains(&WORKFLOW_TARGET_PROPERTY) {
            continue;
        }
        assert!(
            required(&tool.input_schema).contains(&WORKFLOW_TARGET_PROPERTY),
            "tool `{}` carries a workflow target that the call can proceed without",
            tool.name
        );
        assert!(
            is_uuid_shaped(&tool.input_schema, WORKFLOW_TARGET_PROPERTY),
            "tool `{}` accepts a workflow target that is not constrained to a UUID, so a name \
             would satisfy its schema",
            tool.name
        );
    }
    Ok(())
}

/// Point 1, the name half: a display name is never the thing a call cannot
/// proceed without.
///
/// A selector is precisely the argument a call REQUIRES in order to know what it
/// is acting on. A name that can never be required can never be that argument —
/// on any tool, read or write, since a read whose only required argument were a
/// name would be half of the prohibited composition with the choosing step
/// already gone.
#[test]
fn no_published_tool_requires_a_display_name() -> Result<(), CatalogError> {
    let catalog = aion_tool_catalog()?;

    for tool in catalog.tools() {
        assert!(
            !required(&tool.input_schema).contains(&DISPLAY_NAME_PROPERTY),
            "tool `{}` REQUIRES a display name; a required name is a selector, and no tool \
             resolves a workflow by name",
            tool.name
        );
    }
    Ok(())
}

/// Point 1, stated as the thing that actually matters: a MUTATING tool that
/// carries a display name must also carry the UUID that says what it is acting
/// on — the name is a value being SET, never the selector.
///
/// The one shape without a UUID target is a tool that CREATES the workflow, and
/// it is recognised by its schema rather than by its name: a tool that requires
/// a `workflow_type` is minting a run the server has not assigned an id to yet,
/// so there is no pre-existing referent for a name to resolve to.
#[test]
fn a_mutating_tool_that_carries_a_name_still_takes_a_uuid_or_creates_the_run()
-> Result<(), CatalogError> {
    let catalog = aion_tool_catalog()?;

    for tool in catalog.tools() {
        if tool.annotations.modification != aion_mcp::tools::descriptor::Modification::Mutating {
            continue;
        }
        if !properties(&tool.input_schema).contains(&DISPLAY_NAME_PROPERTY) {
            continue;
        }
        let required = required(&tool.input_schema);
        assert!(
            required.contains(&WORKFLOW_TARGET_PROPERTY) || required.contains(&"workflow_type"),
            "mutating tool `{}` carries a display name but neither requires a UUID target nor \
             creates the run; a name would be the only thing left to select what it acts on",
            tool.name
        );
    }
    Ok(())
}
