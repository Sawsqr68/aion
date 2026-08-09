//! The host-implemented tool service and the catalog it publishes.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use super::annotations::{AnnotationError, HeaderBinding, header_bindings};
use super::descriptor::Tool;

/// A published set of tools, validated once at construction.
///
/// The order is the order the host supplied and is preserved verbatim, so
/// `tools/list` is deterministic across requests without the catalog having to
/// re-sort on every call.
#[derive(Clone, Debug)]
pub struct ToolCatalog {
    tools: Vec<Tool>,
    /// Each tool's validated `x-mcp-header` bindings, indexed by tool position.
    /// Computed once here so a request never re-walks a schema, and so an
    /// invalid annotation is a startup failure rather than a per-request one.
    bindings: Vec<Vec<HeaderBinding>>,
}

/// Why a proposed catalog was refused.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CatalogError {
    /// A tool name violates the naming rules.
    #[error("tool name `{name}` must be 1-128 characters of A-Za-z0-9_-.")]
    InvalidName {
        /// The offending name.
        name: String,
    },
    /// Two tools share a name.
    #[error("tool name `{name}` is declared more than once")]
    DuplicateName {
        /// The repeated name.
        name: String,
    },
    /// A tool's input schema is not a JSON Schema object schema.
    #[error("tool `{name}` must declare an inputSchema with root type \"object\"")]
    NonObjectInputSchema {
        /// The offending tool.
        name: String,
    },
    /// A schema could not be compiled as JSON Schema.
    #[error("tool `{name}` has an uncompilable {which} schema: {reason}")]
    UnusableSchema {
        /// The offending tool.
        name: String,
        /// `input` or `output`.
        which: &'static str,
        /// The compiler's own reason.
        reason: String,
    },
    /// A tool declares an invalid `x-mcp-header` annotation.
    #[error("tool `{name}` has an invalid x-mcp-header annotation: {source}")]
    InvalidAnnotation {
        /// The offending tool.
        name: String,
        /// What was wrong with the annotation.
        source: AnnotationError,
    },
}

impl ToolCatalog {
    /// Validate and adopt a tool list.
    ///
    /// Every schema is compiled here, at construction, rather than on first
    /// call: a schema that cannot compile is a server defect, and a server
    /// defect must surface at startup instead of as a 500 on somebody's first
    /// `tools/call`.
    ///
    /// # Errors
    ///
    /// [`CatalogError`] when a name is invalid or duplicated, an input schema
    /// is not an object schema, or either schema fails to compile.
    pub fn new(tools: Vec<Tool>) -> Result<Self, CatalogError> {
        let mut seen = BTreeSet::new();
        let mut bindings = Vec::with_capacity(tools.len());
        for tool in &tools {
            if !Tool::is_valid_name(&tool.name) {
                return Err(CatalogError::InvalidName {
                    name: tool.name.clone(),
                });
            }
            if !seen.insert(tool.name.clone()) {
                return Err(CatalogError::DuplicateName {
                    name: tool.name.clone(),
                });
            }
            if !tool.has_object_input_schema() {
                return Err(CatalogError::NonObjectInputSchema {
                    name: tool.name.clone(),
                });
            }
            compile(&tool.name, "input", &tool.input_schema)?;
            if let Some(output_schema) = &tool.output_schema {
                compile(&tool.name, "output", output_schema)?;
            }
            bindings.push(header_bindings(&tool.input_schema).map_err(|source| {
                CatalogError::InvalidAnnotation {
                    name: tool.name.clone(),
                    source,
                }
            })?);
        }
        Ok(Self { tools, bindings })
    }

    /// The published tools, in publication order.
    #[must_use]
    pub fn tools(&self) -> &[Tool] {
        &self.tools
    }

    /// The tool named `name`, if it is published.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&Tool> {
        self.tools.iter().find(|tool| tool.name == name)
    }

    /// The validated `x-mcp-header` bindings of the tool named `name`.
    ///
    /// An empty slice for a published tool that declares none, and for an
    /// unpublished name — an unknown tool is refused by the dispatcher on its
    /// own terms, and reporting "no bindings" here is the answer that cannot
    /// mislead a caller into thinking a header was checked.
    #[must_use]
    pub fn bindings_of(&self, name: &str) -> &[HeaderBinding] {
        self.tools
            .iter()
            .position(|tool| tool.name == name)
            .and_then(|index| self.bindings.get(index))
            .map_or(&[], Vec::as_slice)
    }
}

fn compile(name: &str, which: &'static str, schema: &Value) -> Result<(), CatalogError> {
    jsonschema::validator_for(schema)
        .map(|_validator| ())
        .map_err(|error| CatalogError::UnusableSchema {
            name: name.to_owned(),
            which,
            reason: error.to_string(),
        })
}

/// Validate `value` against `schema`, rendering every violation.
///
/// # Errors
///
/// The joined violation list when `value` does not conform, or the compiler's
/// reason when `schema` itself cannot be compiled.
pub fn validate_against(schema: &Value, value: &Value) -> Result<(), String> {
    let validator = jsonschema::validator_for(schema).map_err(|error| error.to_string())?;
    if validator.is_valid(value) {
        return Ok(());
    }
    let violations = validator
        .iter_errors(value)
        .map(|error| {
            let location = error.instance_path().to_string();
            if location.is_empty() {
                format!("<root>: {error}")
            } else {
                format!("{location}: {error}")
            }
        })
        .collect::<Vec<_>>()
        .join("; ");
    Err(violations)
}

/// One decoded `tools/call`.
#[derive(Clone, Debug)]
pub struct ToolCall {
    /// The tool being called.
    pub name: String,
    /// The `arguments` object, empty when the client sent none.
    pub arguments: Map<String, Value>,
}

/// What a tool produced.
///
/// `structured` is the tool's `structuredContent` and MUST conform to the
/// tool's declared `outputSchema`; the dispatcher checks that before answering,
/// so a schema violation is the server's fault and is reported as one rather
/// than shipped to the model.
#[derive(Clone, Debug)]
pub struct ToolOutcome {
    /// The structured result.
    pub structured: Value,
    /// A one-line human/model-readable summary placed ahead of the serialized
    /// JSON in the content blocks.
    pub summary: String,
}

/// A tool-level failure.
///
/// This is deliberately NOT a protocol error: it is returned to the model
/// inside a `CallToolResult` with `isError: true`, so the model can see what
/// went wrong and correct itself. Only malformed requests and unknown tools are
/// protocol errors.
#[derive(Clone, Debug, thiserror::Error)]
#[error("{message}")]
pub struct ToolFailure {
    /// What went wrong, phrased for the model that will read it.
    pub message: String,
    /// Machine-readable failure detail, surfaced as `structuredContent` so a
    /// caller can branch on it without parsing prose.
    pub detail: Value,
}

impl ToolFailure {
    /// Build a failure with a detail object.
    #[must_use]
    pub fn new(message: impl Into<String>, detail: Value) -> Self {
        Self {
            message: message.into(),
            detail,
        }
    }
}

/// Whether a call should be executed as a task rather than inline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolTaskDecision {
    /// Execute inline and answer with a `CallToolResult`.
    Inline,
    /// Execute as a task: the server answers with a `CreateTaskResult` and the
    /// call runs on in the background.
    ///
    /// The decision is the SERVER's, per the extension: a client never signals
    /// a preference on the request. When the client did not declare the tasks
    /// extension, a call the server can only serve as a task is refused with
    /// `-32021` rather than silently downgraded.
    Task,
}

/// The host's tool implementation.
///
/// One instance serves every request; per-request identity arrives as
/// [`ToolService::Caller`], which the transport supplies from whatever
/// authentication the host already runs. The MCP layer adds no authentication
/// of its own and makes no authorization decision — it only carries the
/// caller through.
#[async_trait::async_trait]
pub trait ToolService: Send + Sync + 'static {
    /// The host's per-request caller identity.
    ///
    /// `Clone + 'static` because a task-shaped call outlives the request that
    /// created it and must keep executing under the SAME identity that was
    /// authorized at creation — never a re-derived or ambient one.
    type Caller: Clone + Send + Sync + 'static;

    /// The published catalog. Must be the same set for every caller on every
    /// request (it may narrow by the authorization presented, but never by
    /// connection or by prior requests on it).
    fn catalog(&self) -> &ToolCatalog;

    /// Prose returned in `server/discover`, teaching a model how to drive this
    /// host: which handles exist, which call to make first, and what it must
    /// never invent.
    fn instructions(&self) -> &str;

    /// Whether this specific call must run as a task.
    ///
    /// Consulted BEFORE the client's declared capabilities are checked, so the
    /// server's own decision is what determines whether the tasks extension is
    /// required for this call.
    fn task_decision(&self, call: &ToolCall) -> ToolTaskDecision;

    /// Execute a call.
    ///
    /// # Errors
    ///
    /// [`ToolFailure`] for a tool-level failure the model should see and be
    /// able to correct.
    async fn call(&self, caller: &Self::Caller, call: ToolCall)
    -> Result<ToolOutcome, ToolFailure>;
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{CatalogError, Tool, ToolCatalog, validate_against};
    use crate::tools::descriptor::{ToolAnnotations, WorldScope};

    fn tool(name: &str, input_schema: serde_json::Value) -> Tool {
        Tool {
            name: name.to_owned(),
            title: None,
            description: None,
            input_schema,
            output_schema: None,
            annotations: ToolAnnotations::read_only(WorldScope::Closed),
        }
    }

    #[test]
    fn a_duplicate_name_is_refused() {
        let object = json!({ "type": "object" });
        let error = ToolCatalog::new(vec![
            tool("read_history", object.clone()),
            tool("read_history", object),
        ])
        .err();
        assert_eq!(
            error,
            Some(CatalogError::DuplicateName {
                name: "read_history".to_owned()
            })
        );
    }

    #[test]
    fn a_non_object_input_schema_is_refused() {
        let error = ToolCatalog::new(vec![tool("x", json!({ "type": "string" }))]).err();
        assert_eq!(
            error,
            Some(CatalogError::NonObjectInputSchema {
                name: "x".to_owned()
            })
        );
    }

    #[test]
    fn an_uncompilable_schema_is_refused_at_construction() {
        let error = ToolCatalog::new(vec![tool(
            "x",
            json!({ "type": "object", "properties": { "a": { "type": 7 } } }),
        )])
        .err();
        assert!(matches!(
            error,
            Some(CatalogError::UnusableSchema { which: "input", .. })
        ));
    }

    #[test]
    fn validation_reports_the_violating_pointer() {
        let schema = json!({
            "type": "object",
            "properties": { "attempt": { "type": "integer" } },
            "required": ["attempt"],
        });
        let violations = validate_against(&schema, &json!({ "attempt": "one" }))
            .err()
            .unwrap_or_default();
        assert!(violations.contains("/attempt"), "got: {violations}");
        assert!(validate_against(&schema, &json!({ "attempt": 1 })).is_ok());
    }

    #[test]
    fn publication_order_is_preserved() -> Result<(), CatalogError> {
        let object = json!({ "type": "object" });
        let catalog = ToolCatalog::new(vec![
            tool("b", object.clone()),
            tool("a", object.clone()),
            tool("c", object),
        ])?;
        let names: Vec<&str> = catalog.tools().iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["b", "a", "c"]);
        assert!(catalog.find("a").is_some());
        assert!(catalog.find("zzz").is_none());
        Ok(())
    }
}
