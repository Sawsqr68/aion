//! A minimal tool service used to exercise the transport and dispatcher.

use std::sync::Arc;

use aion_mcp::protocol::headers::HeaderView;
use aion_mcp::protocol::meta::Implementation;
use aion_mcp::tools::descriptor::{
    Destructiveness, Idempotence, Tool, ToolAnnotations, WorldScope,
};
use aion_mcp::tools::service::{
    ToolCall, ToolCatalog, ToolFailure, ToolOutcome, ToolService, ToolTaskDecision,
};
use aion_mcp::{HttpRequest, McpServer, McpServerConfig};
use serde_json::{Value, json};

/// A caller identity: the host's, carried through untouched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FakeCaller {
    pub subject: String,
}

pub struct FakeService {
    catalog: ToolCatalog,
}

impl FakeService {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let echo = Tool {
            name: "echo".to_owned(),
            title: Some("Echo".to_owned()),
            description: Some("Return the text it was given.".to_owned()),
            input_schema: json!({
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"],
                "additionalProperties": false,
            }),
            output_schema: Some(json!({
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"],
                "additionalProperties": false,
            })),
            annotations: ToolAnnotations::read_only(WorldScope::Closed),
        };
        let refuse = Tool {
            name: "refuse".to_owned(),
            title: None,
            description: Some("Always fails at the tool level.".to_owned()),
            input_schema: json!({ "type": "object", "additionalProperties": false }),
            output_schema: None,
            annotations: ToolAnnotations::mutating(
                Destructiveness::Destructive,
                Idempotence::Repeating,
                WorldScope::Closed,
            ),
        };
        let slow = Tool {
            name: "await_thing".to_owned(),
            title: None,
            description: Some("Runs as a task.".to_owned()),
            input_schema: json!({ "type": "object", "additionalProperties": false }),
            output_schema: Some(json!({
                "type": "object",
                "properties": { "done": { "type": "boolean" } },
                "required": ["done"],
                "additionalProperties": false,
            })),
            annotations: ToolAnnotations::mutating(
                Destructiveness::Additive,
                Idempotence::Repeating,
                WorldScope::Closed,
            ),
        };
        // A tool that DOES annotate its arguments, so the `Mcp-Param-{Name}`
        // contract is exercised end to end. `shard` is an integer precisely so
        // the numeric-comparison rule has something to be numeric about.
        let routed = Tool {
            name: "routed".to_owned(),
            title: None,
            description: Some("Carries a routable shard in a header.".to_owned()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "shard": { "type": "integer", "x-mcp-header": "Shard" },
                    "label": { "type": "string", "x-mcp-header": "Label" },
                },
                "required": ["shard"],
                "additionalProperties": false,
            }),
            output_schema: Some(json!({
                "type": "object",
                "properties": { "shard": { "type": "integer" } },
                "required": ["shard"],
                "additionalProperties": false,
            })),
            annotations: ToolAnnotations::read_only(WorldScope::Closed),
        };
        Ok(Self {
            catalog: ToolCatalog::new(vec![echo, refuse, slow, routed])?,
        })
    }
}

#[async_trait::async_trait]
impl ToolService for FakeService {
    type Caller = FakeCaller;

    fn catalog(&self) -> &ToolCatalog {
        &self.catalog
    }

    fn instructions(&self) -> &'static str {
        "call echo first"
    }

    fn task_decision(&self, call: &ToolCall) -> ToolTaskDecision {
        if call.name == "await_thing" {
            ToolTaskDecision::Task
        } else {
            ToolTaskDecision::Inline
        }
    }

    async fn call(
        &self,
        caller: &Self::Caller,
        call: ToolCall,
    ) -> Result<ToolOutcome, ToolFailure> {
        match call.name.as_str() {
            "echo" => {
                let text = call
                    .arguments
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                Ok(ToolOutcome {
                    structured: json!({ "text": format!("{}:{text}", caller.subject) }),
                    summary: "echoed".to_owned(),
                })
            }
            "refuse" => Err(ToolFailure::new(
                "the tool refused",
                json!({ "code": "refused" }),
            )),
            "routed" => Ok(ToolOutcome {
                structured: json!({
                    "shard": call.arguments.get("shard").and_then(Value::as_i64).unwrap_or(-1),
                }),
                summary: "routed".to_owned(),
            }),
            "await_thing" => {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                Ok(ToolOutcome {
                    structured: json!({ "done": true }),
                    summary: "done".to_owned(),
                })
            }
            other => Err(ToolFailure::new(
                format!("unreachable tool {other}"),
                Value::Null,
            )),
        }
    }
}

pub fn server() -> Result<McpServer<FakeService>, Box<dyn std::error::Error>> {
    Ok(McpServer::new(
        Arc::new(FakeService::new()?),
        McpServerConfig::new(
            Implementation::new("aion-test", "0.12.0"),
            60_000,
            300_000,
            Some(3_600_000),
            250,
            vec!["http://localhost:8080".to_owned()],
        ),
    ))
}

pub fn caller() -> FakeCaller {
    FakeCaller {
        subject: "operator".to_owned(),
    }
}

/// Build a POST request with the standard headers derived from the body, so a
/// test only states the headers it wants to be WRONG.
pub fn post(body: &Value, overrides: &[(&str, &str)]) -> HttpRequest {
    let method = body
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let version = body
        .pointer("/params/_meta/io.modelcontextprotocol~1protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut headers: Vec<(String, String)> = vec![
        ("mcp-protocol-version".to_owned(), version.to_owned()),
        ("mcp-method".to_owned(), method.to_owned()),
    ];
    let name = body
        .pointer("/params/name")
        .or_else(|| body.pointer("/params/taskId"))
        .and_then(Value::as_str);
    if let Some(name) = name {
        headers.push(("mcp-name".to_owned(), name.to_owned()));
    }
    for (key, value) in overrides {
        headers.retain(|(existing, _)| existing != &key.to_ascii_lowercase());
        if !value.is_empty() {
            headers.push(((*key).to_ascii_lowercase(), (*value).to_owned()));
        }
    }
    HttpRequest::new(
        "POST",
        HeaderView::new(headers),
        serde_json::to_vec(body).unwrap_or_default(),
    )
}

/// Standard `_meta` for a request, optionally declaring the tasks extension.
pub fn meta(declare_tasks: bool) -> Value {
    let mut capabilities = json!({});
    if declare_tasks {
        capabilities = json!({ "extensions": { "io.modelcontextprotocol/tasks": {} } });
    }
    json!({
        "io.modelcontextprotocol/protocolVersion": aion_mcp::PROTOCOL_VERSION,
        "io.modelcontextprotocol/clientCapabilities": capabilities,
        "io.modelcontextprotocol/clientInfo": { "name": "test-client", "version": "1.0.0" },
    })
}

/// Decode a response body as JSON.
pub fn body_of(response: &aion_mcp::HttpResponse) -> Result<Value, serde_json::Error> {
    serde_json::from_slice(&response.body)
}
