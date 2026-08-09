//! `CallToolResult` construction.
//!
//! A `CallToolResult` is NOT a `CacheableResult` in this revision, so nothing
//! here writes `ttlMs` or `cacheScope`. Immutability facts about the data a
//! read tool returns (a history page below the head can never change) are
//! stated inside the structured content instead, where they are honest, rather
//! than as protocol cache hints the revision no longer carries.

use serde_json::{Map, Value, json};

use crate::tools::descriptor::Tool;
use crate::tools::service::{ToolFailure, ToolOutcome, validate_against};

/// Build the `result` object for a successful call.
///
/// The serialized structured content is ALSO emitted as a `TextContent` block,
/// ahead of which sits the tool's one-line summary — a client that reads only
/// `content` still receives the whole answer.
///
/// # Errors
///
/// The schema-violation report when the tool declared an `outputSchema` and its
/// structured content does not conform to it. That is a server defect: the
/// server published a contract and then broke it, and shipping the malformed
/// object to a model would teach the model a shape that does not exist.
pub fn call_tool_success_result(
    tool: Option<&Tool>,
    outcome: &ToolOutcome,
) -> Result<Value, String> {
    if let Some(schema) = tool.and_then(|tool| tool.output_schema.as_ref()) {
        validate_against(schema, &outcome.structured).map_err(|violations| {
            format!(
                "tool `{}` produced structured content violating its declared outputSchema: \
                 {violations}",
                tool.map_or("<unknown>", |tool| tool.name.as_str())
            )
        })?;
    }
    let serialized = serde_json::to_string_pretty(&outcome.structured)
        .unwrap_or_else(|error| format!("<structured content could not be serialized: {error}>"));
    let mut result = Map::new();
    result.insert(
        "content".to_owned(),
        json!([
            { "type": "text", "text": outcome.summary },
            { "type": "text", "text": serialized },
        ]),
    );
    result.insert("structuredContent".to_owned(), outcome.structured.clone());
    result.insert("isError".to_owned(), json!(false));
    Ok(Value::Object(result))
}

/// Build the `result` object for a tool-level failure.
///
/// A tool that ran and failed reports through the RESULT with `isError: true`,
/// not through a JSON-RPC error, so the model can see what went wrong and
/// correct itself. `structuredContent` is deliberately absent: the failure
/// detail does not conform to the tool's success `outputSchema`, and claiming
/// otherwise would violate the contract the catalog published.
#[must_use]
pub fn call_tool_failure_result(failure: &ToolFailure) -> Value {
    let detail = serde_json::to_string_pretty(&failure.detail)
        .unwrap_or_else(|error| format!("<failure detail could not be serialized: {error}>"));
    json!({
        "content": [
            { "type": "text", "text": failure.message },
            { "type": "text", "text": detail },
        ],
        "isError": true,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{call_tool_failure_result, call_tool_success_result};
    use crate::tools::descriptor::{Tool, ToolAnnotations, WorldScope};
    use crate::tools::service::{ToolFailure, ToolOutcome};

    fn tool(output_schema: Option<serde_json::Value>) -> Tool {
        Tool {
            name: "read_history".to_owned(),
            title: None,
            description: None,
            input_schema: json!({ "type": "object" }),
            output_schema,
            annotations: ToolAnnotations::read_only(WorldScope::Closed),
        }
    }

    #[test]
    fn a_success_carries_structured_content_and_its_serialization() -> Result<(), String> {
        let outcome = ToolOutcome {
            structured: json!({ "head_seq": 12 }),
            summary: "12 events".to_owned(),
        };
        let schema = json!({
            "type": "object",
            "properties": { "head_seq": { "type": "integer" } },
            "required": ["head_seq"],
        });
        let result = call_tool_success_result(Some(&tool(Some(schema))), &outcome)?;
        assert_eq!(result["structuredContent"]["head_seq"], 12);
        assert_eq!(result["isError"], false);
        assert_eq!(result["content"][0]["text"], "12 events");
        assert!(
            result["content"][1]["text"]
                .as_str()
                .is_some_and(|text| text.contains("head_seq"))
        );
        // No cache hints: CallToolResult is not a CacheableResult.
        assert!(result.get("ttlMs").is_none());
        assert!(result.get("cacheScope").is_none());
        Ok(())
    }

    #[test]
    fn structured_content_violating_the_declared_schema_is_refused() {
        let outcome = ToolOutcome {
            structured: json!({ "head_seq": "twelve" }),
            summary: "12 events".to_owned(),
        };
        let schema = json!({
            "type": "object",
            "properties": { "head_seq": { "type": "integer" } },
            "required": ["head_seq"],
        });
        let error = call_tool_success_result(Some(&tool(Some(schema))), &outcome)
            .err()
            .unwrap_or_default();
        assert!(error.contains("outputSchema"), "got: {error}");
    }

    #[test]
    fn a_tool_failure_is_an_is_error_result_without_structured_content() {
        let result = call_tool_failure_result(&ToolFailure::new(
            "workflow not found",
            json!({ "code": "not_found" }),
        ));
        assert_eq!(result["isError"], true);
        assert_eq!(result["content"][0]["text"], "workflow not found");
        assert!(result.get("structuredContent").is_none());
    }
}
