//! Catalog assembly.

use aion_mcp::tools::service::{CatalogError, ToolCatalog};

use super::{mutations, reads};

/// Build the Aion tool catalog.
///
/// Reads come first: `describe_run` is the call an agent should make before any
/// other, and a model reading a tool list top-down should meet it first.
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
        mutations::start_run(),
        mutations::signal(),
        mutations::cancel(),
    ])
}

#[cfg(test)]
mod tests {
    use aion_mcp::tools::descriptor::Modification;
    use aion_mcp::tools::service::CatalogError;

    use super::aion_tool_catalog;

    /// The eight published tool names, in publication order.
    ///
    /// Written out independently of the builder so the order assertion is
    /// checking something rather than comparing the builder to itself.
    const TOOL_NAMES: &[&str] = &[
        "describe_run",
        "read_transcript",
        "read_history",
        "list_runs",
        "query",
        "start_run",
        "signal",
        "cancel",
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
        for name in ["start_run", "signal", "cancel"] {
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
