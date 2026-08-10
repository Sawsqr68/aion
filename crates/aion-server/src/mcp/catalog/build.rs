//! Catalog assembly.

use aion_mcp::tools::service::{CatalogError, ToolCatalog};

use super::{authoring, mutations, reads};

/// Build the Aion tool catalog.
///
/// Reads come first: `describe_run` is the call an agent should make before any
/// other, and a model reading a tool list top-down should meet it first. The
/// authoring reads follow the run reads, and the mutations close the list with
/// the authoring pair last — the same order the authoring loop is walked in.
///
/// # Errors
///
/// [`CatalogError`] when a schema fails to compile or a name is malformed —
/// a server defect, surfaced at construction rather than on first call.
pub(crate) fn aion_tool_catalog() -> Result<ToolCatalog, CatalogError> {
    ToolCatalog::new(vec![
        reads::describe_run(),
        reads::read_transcript(),
        reads::read_history(),
        reads::list_runs(),
        reads::query(),
        authoring::list_documents(),
        authoring::read_document(),
        authoring::check_document(),
        mutations::start_run(),
        mutations::signal(),
        mutations::cancel(),
        authoring::save_document(),
        authoring::deploy_document(),
    ])
}

#[cfg(test)]
mod tests {
    use aion_mcp::tools::descriptor::Modification;
    use aion_mcp::tools::service::CatalogError;

    use super::aion_tool_catalog;

    /// The thirteen published tool names, in publication order.
    ///
    /// Written out independently of the builder so the order assertion is
    /// checking something rather than comparing the builder to itself.
    const TOOL_NAMES: &[&str] = &[
        "describe_run",
        "read_transcript",
        "read_history",
        "list_runs",
        "query",
        "list_documents",
        "read_document",
        "check_document",
        "start_run",
        "signal",
        "cancel",
        "save_document",
        "deploy_document",
    ];

    #[test]
    fn the_catalog_publishes_exactly_the_named_tools_in_order() -> Result<(), CatalogError> {
        let catalog = aion_tool_catalog()?;
        let names: Vec<&str> = catalog
            .tools()
            .iter()
            .map(|tool| tool.name.as_str())
            .collect();
        assert_eq!(names, TOOL_NAMES);
        Ok(())
    }

    #[test]
    fn every_tool_declares_an_output_schema() -> Result<(), CatalogError> {
        for tool in aion_tool_catalog()?.tools() {
            assert!(
                tool.output_schema.is_some(),
                "{} must declare an outputSchema",
                tool.name
            );
            assert!(
                tool.description.is_some(),
                "{} needs a description",
                tool.name
            );
        }
        Ok(())
    }

    #[test]
    fn read_tools_are_annotated_read_only_and_mutations_are_not() -> Result<(), CatalogError> {
        let catalog = aion_tool_catalog()?;
        for name in [
            "describe_run",
            "read_transcript",
            "read_history",
            "list_runs",
            "query",
            "list_documents",
            "read_document",
            "check_document",
        ] {
            let tool = catalog.find(name).ok_or(CatalogError::InvalidName {
                name: name.to_owned(),
            })?;
            assert_eq!(
                tool.annotations.modification,
                Modification::ReadOnly,
                "{name} must be annotated read-only"
            );
        }
        for name in [
            "start_run",
            "signal",
            "cancel",
            "save_document",
            "deploy_document",
        ] {
            let tool = catalog.find(name).ok_or(CatalogError::InvalidName {
                name: name.to_owned(),
            })?;
            assert_eq!(
                tool.annotations.modification,
                Modification::Mutating,
                "{name} must NOT be annotated read-only"
            );
        }
        Ok(())
    }

    #[test]
    fn cancel_is_the_only_tool_marked_destructive() -> Result<(), CatalogError> {
        use aion_mcp::tools::descriptor::Destructiveness;
        for tool in aion_tool_catalog()?.tools() {
            let expected = if tool.name == "cancel" {
                Destructiveness::Destructive
            } else {
                Destructiveness::Additive
            };
            assert_eq!(tool.annotations.destructiveness, expected, "{}", tool.name);
        }
        Ok(())
    }

    /// The authoring mutations are additive BY DESIGN — a save stores a
    /// content-addressed revision (nothing is lost) and a deploy loads a
    /// content-hash package (nothing is overwritten) — but they differ on
    /// idempotence, and each hint states its own truth: repeating a save with
    /// the same bytes CONVERGES (same file, same hash, no new state), while
    /// repeating a deploy APPENDS — every call mints a fresh `deployment_id`
    /// and records a new entry in the deployment ledger, even for the same
    /// revision. If either annotation changes, the change must be argued, not
    /// slipped.
    #[test]
    fn save_converges_and_deploy_appends_and_both_are_additive() -> Result<(), CatalogError> {
        use aion_mcp::tools::descriptor::{Destructiveness, Idempotence};
        let catalog = aion_tool_catalog()?;
        for (name, idempotence) in [
            ("save_document", Idempotence::Idempotent),
            ("deploy_document", Idempotence::Repeating),
        ] {
            let tool = catalog.find(name).ok_or(CatalogError::InvalidName {
                name: name.to_owned(),
            })?;
            assert_eq!(
                tool.annotations.destructiveness,
                Destructiveness::Additive,
                "{name}"
            );
            assert_eq!(tool.annotations.idempotence, idempotence, "{name}");
        }
        Ok(())
    }

    /// The save→deploy handoff is a schema-level contract: save's output
    /// REQUIRES `content_hash`, and deploy's input REQUIRES `path` and
    /// `content_hash` — there is no argument through which unsaved source
    /// could be deployed.
    #[test]
    fn deploy_requires_the_hash_save_returns_and_takes_no_source() -> Result<(), CatalogError> {
        let catalog = aion_tool_catalog()?;
        let save = catalog
            .find("save_document")
            .ok_or(CatalogError::InvalidName {
                name: "save_document".to_owned(),
            })?;
        let save_output = save
            .output_schema
            .as_ref()
            .and_then(|schema| schema["required"].as_array())
            .cloned()
            .unwrap_or_default();
        assert!(save_output.iter().any(|value| value == "content_hash"));

        let deploy = catalog
            .find("deploy_document")
            .ok_or(CatalogError::InvalidName {
                name: "deploy_document".to_owned(),
            })?;
        let required = deploy.input_schema["required"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert!(required.iter().any(|value| value == "path"));
        assert!(required.iter().any(|value| value == "content_hash"));
        assert!(
            deploy.input_schema["properties"].get("source").is_none(),
            "a `source` argument on deploy_document would be the deploy-this-string seam \
             the design refuses"
        );
        Ok(())
    }

    #[test]
    fn read_transcript_requires_a_run_id() -> Result<(), CatalogError> {
        let catalog = aion_tool_catalog()?;
        let tool = catalog
            .find("read_transcript")
            .ok_or(CatalogError::InvalidName {
                name: "read_transcript".to_owned(),
            })?;
        let required = tool.input_schema["required"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert!(
            required.iter().any(|value| value == "run_id"),
            "run_id is a REQUIRED axis of a transcript handle, not an option: {required:?}"
        );
        // And the output echoes it, so a handle that comes back out is complete.
        let output_required = tool
            .output_schema
            .as_ref()
            .and_then(|schema| schema["required"].as_array())
            .cloned()
            .unwrap_or_default();
        assert!(output_required.iter().any(|value| value == "run_id"));
        Ok(())
    }

    #[test]
    fn no_published_schema_annotates_a_property_with_x_mcp_header()
    -> Result<(), Box<dyn std::error::Error>> {
        let catalog = aion_tool_catalog()?;
        let rendered = serde_json::to_string(catalog.tools())?;
        assert!(
            !rendered.contains("x-mcp-header"),
            "an x-mcp-header annotation would put a namespace or an id into a header every \
             intermediary on the path can read"
        );
        Ok(())
    }

    #[test]
    fn no_schema_uses_a_ref_or_a_composition_keyword() -> Result<(), Box<dyn std::error::Error>> {
        let catalog = aion_tool_catalog()?;
        let rendered = serde_json::to_string(catalog.tools())?;
        for keyword in [
            "\"$ref\"",
            "\"anyOf\"",
            "\"oneOf\"",
            "\"allOf\"",
            "\"not\"",
            "\"if\"",
            "\"$defs\"",
        ] {
            assert!(
                !rendered.contains(keyword),
                "{keyword} in a published schema is either an unresolvable external reference \
                 or unbounded composition; neither belongs in a tool contract"
            );
        }
        Ok(())
    }
}
