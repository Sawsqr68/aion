//! The MCP surface end to end, over the REAL public HTTP router.
//!
//! Everything here drives `http_router` — the same router `aion server` mounts —
//! so the mount, the caller extractor, the dispatcher, the catalog, and the
//! tool implementations are all exercised as one piece. Nothing is stubbed
//! except the store, which is the in-memory reference backend.
//!
//! The suite is the executable form of the 2026-07-28 conformance checklist:
//! each test names one rule and drives a whole request/response cycle at it.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use aion::EngineBuilder;
use aion_core::{
    ActivityEvent, ActivityEventKind, ActivityId, ContentType, Event, EventEnvelope, MessageRole,
    PackageVersion, Payload, RunId, WorkflowId,
};
use aion_server::api::http::http_router;
use aion_server::config::{
    AuthConfig, AuthoringConfig, DeployConfig, ListenConfig, MetricsConfig, NamespaceConfig,
    NamespaceMode, OpsConsoleAssetSource, OpsConsoleConfig, ResolvedMcpConfig, RuntimeConfig,
    WebSocketConfig, WorkerConfig,
};
use aion_server::{
    NamespaceResolver, ServerState, StaticScheduleNamespaces, StaticWorkflowNamespaces,
};
use aion_store::{EventStore, InMemoryStore, WriteToken, visibility::VisibilityStore};
use axum::{
    Router,
    body::{self, Body},
    http::{Request, StatusCode},
};
use chrono::Utc;
use serde_json::{Value, json};
use tower::ServiceExt as _;
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const NAMESPACE: &str = "tenant-a";
const PROTOCOL_VERSION: &str = "2026-07-28";
const TASKS_EXTENSION: &str = "io.modelcontextprotocol/tasks";
const ORIGIN: &str = "http://localhost:8080";

fn workflow_id() -> WorkflowId {
    WorkflowId::new(Uuid::from_u128(0x5eed))
}

fn first_run() -> RunId {
    RunId::new(Uuid::from_u128(0xa1))
}

fn second_run() -> RunId {
    RunId::new(Uuid::from_u128(0xa2))
}

fn envelope(seq: u64) -> EventEnvelope {
    EventEnvelope {
        seq,
        recorded_at: Utc::now(),
        workflow_id: workflow_id(),
    }
}

fn payload() -> Payload {
    Payload::new(ContentType::Json, b"{}".to_vec())
}

/// A continue-as-new chain in which BOTH generations dispatch activity ordinal
/// 0, attempt 1.
///
/// This is not a contrived shape: it is what any chain that runs an agent
/// activity produces, because the activity ordinal is minted per-run and the
/// attempt restarts at one for each dispatch. The run axis of the durable
/// stream key is the only thing separating the two generations' transcripts,
/// which is precisely what the tests below hold to.
fn colliding_chain() -> Vec<Event> {
    vec![
        Event::WorkflowStarted {
            envelope: envelope(1),
            workflow_type: "agent_chain".to_owned(),
            input: payload(),
            run_id: first_run(),
            parent_run_id: None,
            package_version: PackageVersion::new("a".repeat(64)),
        },
        Event::ActivityScheduled {
            envelope: envelope(2),
            activity_id: ActivityId::from_sequence_position(0),
            activity_type: "dev_review".to_owned(),
            input: payload(),
            task_queue: "default".to_owned(),
            node: None,
        },
        Event::ActivityStarted {
            envelope: envelope(3),
            activity_id: ActivityId::from_sequence_position(0),
            attempt: 1,
        },
        Event::ActivityCompleted {
            envelope: envelope(4),
            activity_id: ActivityId::from_sequence_position(0),
            result: payload(),
            attempt: 1,
        },
        Event::WorkflowContinuedAsNew {
            envelope: envelope(5),
            input: payload(),
            workflow_type: None,
            parent_run_id: first_run(),
        },
        Event::WorkflowStarted {
            envelope: envelope(6),
            workflow_type: "agent_chain".to_owned(),
            input: payload(),
            run_id: second_run(),
            parent_run_id: Some(first_run()),
            package_version: PackageVersion::new("a".repeat(64)),
        },
        Event::ActivityScheduled {
            envelope: envelope(7),
            activity_id: ActivityId::from_sequence_position(0),
            activity_type: "dev_review".to_owned(),
            input: payload(),
            task_queue: "default".to_owned(),
            node: None,
        },
        // Generation two's ordinal 0, attempt 1 — byte-identical stream key to
        // generation one's, and still in flight.
        Event::ActivityStarted {
            envelope: envelope(8),
            activity_id: ActivityId::from_sequence_position(0),
            attempt: 1,
        },
    ]
}

fn transcript_event(run_id: RunId, worker_seq: u64, text: &str) -> ActivityEvent {
    ActivityEvent {
        workflow_id: workflow_id(),
        run_id,
        activity_id: ActivityId::from_sequence_position(0),
        attempt: 1,
        agent_id: Uuid::from_u128(7),
        agent_role: "orchestrator".to_owned(),
        emitted_at: Utc::now(),
        worker_seq,
        store_seq: None,
        ephemeral: false,
        kind: ActivityEventKind::Message {
            role: MessageRole::Assistant,
            text: text.to_owned(),
        },
    }
}

fn runtime_config(mcp_enabled: bool) -> RuntimeConfig {
    RuntimeConfig {
        listen: ListenConfig {
            grpc: SocketAddr::from(([127, 0, 0, 1], 0)),
            http: SocketAddr::from(([127, 0, 0, 1], 0)),
        },
        tls: None,
        auth: AuthConfig {
            enabled: false,
            jwks_url: None,
            jwks_refresh_seconds: 300,
        },
        ops_console: OpsConsoleConfig {
            source: OpsConsoleAssetSource::Embedded,
        },
        namespace: NamespaceConfig {
            mode: NamespaceMode::SharedEngine,
        },
        worker: WorkerConfig {
            heartbeat_window: Duration::from_secs(30),
            ..Default::default()
        },
        websocket: WebSocketConfig {
            outbound_buffer_bound: 32,
            event_broadcast_capacity: Some(64),
            cluster_broadcast_capacity: Some(64),
        },
        workflow_packages: Vec::new(),
        deploy: DeployConfig::default(),
        authoring: AuthoringConfig::default(),
        dev: aion_server::config::DevConfig::default(),
        outbox: aion_server::config::OutboxConfig::default(),
        observability: aion_server::config::ObservabilityConfig::default(),
        mcp: ResolvedMcpConfig {
            enabled: mcp_enabled,
            allowed_origins: vec![ORIGIN.to_owned()],
            ..ResolvedMcpConfig::default()
        },
        scheduler_threads: 1,
        query_timeout: Some(Duration::from_secs(10)),
        default_namespace: NAMESPACE.to_owned(),
        auto_create: aion_server::config::AutoCreate::Open,
        max_in_flight_activities: aion_server::config::DEFAULT_MAX_IN_FLIGHT_ACTIVITIES,
        drain_timeout: Duration::from_secs(30),
        metrics: MetricsConfig { enabled: false },
        owned_shards: Vec::new(),
        cors_allowed_origins: Vec::new(),
    }
}

/// A server whose store already holds the colliding chain, and whose transcript
/// keyspace already holds two events on the shared stream.
struct Harness {
    router: Router,
}

impl Harness {
    async fn start(mcp_enabled: bool) -> Result<Self, Box<dyn std::error::Error>> {
        let backing = Arc::new(InMemoryStore::default());
        let store: Arc<dyn EventStore> = backing.clone();
        let visibility: Arc<dyn VisibilityStore> = backing;
        store
            .append(
                WriteToken::recorder(),
                &workflow_id(),
                &colliding_chain(),
                0,
            )
            .await?;
        let engine = Arc::new(
            EngineBuilder::new()
                .store_arc(Arc::clone(&store))
                .visibility_store_arc(Arc::clone(&visibility))
                .scheduler_threads(1)
                .build()
                .await?,
        );
        let ownership = StaticWorkflowNamespaces::default();
        ownership.record(workflow_id(), NAMESPACE)?;
        let resolver = NamespaceResolver::from_parts(
            NamespaceMode::SharedEngine,
            Some(engine),
            Arc::new(ownership),
            Arc::new(StaticScheduleNamespaces::default()),
        );
        let state = ServerState::from_parts(resolver, runtime_config(mcp_enabled));
        // Generation one's transcript sits on ITS OWN stream: the run is an
        // axis of the durable key, so this event must never surface in a read
        // addressed to generation two. The commit-allocated `store_seq` is
        // asserted rather than discarded: if the publisher stopped persisting,
        // the transcript assertions below would otherwise fail with a
        // confusing empty page instead of here.
        let gen_one_seq = state
            .transcript_publisher()
            .publish(&transcript_event(first_run(), 1, "generation one planning"))
            .await?;
        assert_eq!(gen_one_seq, Some(0), "durable transcript sequencing");
        // Two transcript events on generation two's stream, sequenced from
        // zero on its own key: the sibling stream above contributes nothing.
        for (index, text) in [(1, "planning the review"), (2, "review complete")] {
            let store_seq = state
                .transcript_publisher()
                .publish(&transcript_event(second_run(), index, text))
                .await?;
            assert_eq!(store_seq, Some(index - 1), "durable transcript sequencing");
        }
        Ok(Self {
            router: http_router(state)?,
        })
    }

    /// POST a JSON-RPC body with the standard headers derived from it, applying
    /// any overrides (an empty override value removes the header).
    async fn post(
        &self,
        body: &Value,
        overrides: &[(&str, &str)],
    ) -> Result<(StatusCode, Value), Box<dyn std::error::Error>> {
        let mut headers: Vec<(String, String)> = vec![
            ("content-type".to_owned(), "application/json".to_owned()),
            ("origin".to_owned(), ORIGIN.to_owned()),
            (
                "mcp-protocol-version".to_owned(),
                body.pointer("/params/_meta/io.modelcontextprotocol~1protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            ),
            (
                "mcp-method".to_owned(),
                body.get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            ),
        ];
        if let Some(name) = body
            .pointer("/params/name")
            .or_else(|| body.pointer("/params/taskId"))
            .and_then(Value::as_str)
        {
            headers.push(("mcp-name".to_owned(), name.to_owned()));
        }
        for (key, value) in overrides {
            let key = key.to_ascii_lowercase();
            headers.retain(|(existing, _)| existing != &key);
            if !value.is_empty() {
                headers.push((key, (*value).to_owned()));
            }
        }
        let mut builder = Request::builder().method("POST").uri("/mcp");
        for (name, value) in headers {
            builder = builder.header(name, value);
        }
        let request = builder.body(Body::from(serde_json::to_vec(body)?))?;
        self.send(request).await
    }

    async fn send(
        &self,
        request: Request<Body>,
    ) -> Result<(StatusCode, Value), Box<dyn std::error::Error>> {
        let response = self.router.clone().oneshot(request).await?;
        let status = response.status();
        let echoed_session = response
            .headers()
            .keys()
            .any(|name| name.as_str().eq_ignore_ascii_case("mcp-session-id"));
        assert!(
            !echoed_session,
            "the 2026-07-28 revision removed protocol-level sessions: a server must never mint \
             or echo Mcp-Session-Id"
        );
        let bytes = body::to_bytes(response.into_body(), usize::MAX).await?;
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes)?
        };
        Ok((status, value))
    }
}

fn meta(declare_tasks: bool) -> Value {
    let capabilities = if declare_tasks {
        json!({ "extensions": { TASKS_EXTENSION: {} } })
    } else {
        json!({})
    };
    json!({
        "io.modelcontextprotocol/protocolVersion": PROTOCOL_VERSION,
        "io.modelcontextprotocol/clientCapabilities": capabilities,
        "io.modelcontextprotocol/clientInfo": { "name": "conformance", "version": "1.0.0" },
    })
}

fn rpc(method: &str, params: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": "c-1", "method": method, "params": params.clone() })
}

fn call(tool: &str, arguments: &Value, declare_tasks: bool) -> Value {
    rpc(
        "tools/call",
        &json!({ "_meta": meta(declare_tasks), "name": tool, "arguments": arguments.clone() }),
    )
}

/// The `structuredContent` of a successful call, or an error naming what came
/// back instead — so a failing assertion shows the refusal rather than a bare
/// `None`.
fn structured(value: &Value) -> Result<Value, Box<dyn std::error::Error>> {
    if value["result"]["isError"] == json!(true) {
        return Err(format!("the tool refused: {}", value["result"]["content"]).into());
    }
    value["result"]["structuredContent"]
        .as_object()
        .map(|object| Value::Object(object.clone()))
        .ok_or_else(|| format!("no structuredContent in {value}").into())
}

// ---------------------------------------------------------------- transport

#[tokio::test]
async fn a_dark_mcp_surface_is_a_plain_404() -> TestResult {
    let harness = Harness::start(false).await?;
    let (status, _body) = harness
        .post(&rpc("tools/list", &json!({ "_meta": meta(false) })), &[])
        .await?;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "an unmounted MCP surface must be indistinguishable from a build without the route"
    );
    Ok(())
}

#[tokio::test]
async fn get_and_delete_on_the_endpoint_are_405() -> TestResult {
    let harness = Harness::start(true).await?;
    for method in ["GET", "DELETE"] {
        let request = Request::builder()
            .method(method)
            .uri("/mcp")
            .header("origin", ORIGIN)
            .body(Body::empty())?;
        let (status, _body) = harness.send(request).await?;
        assert_eq!(
            status,
            StatusCode::METHOD_NOT_ALLOWED,
            "{method} on the MCP endpoint must be 405: the GET stream endpoint and \
             session termination were both removed in this revision"
        );
    }
    Ok(())
}

#[tokio::test]
async fn a_session_id_header_is_neither_honoured_nor_echoed() -> TestResult {
    let harness = Harness::start(true).await?;
    // `Harness::send` asserts no `Mcp-Session-Id` on ANY response, so every
    // test in this file carries the check; this one proves a request bearing
    // the retired headers is still served normally rather than refused.
    let (status, body) = harness
        .post(
            &rpc("tools/list", &json!({ "_meta": meta(false) })),
            &[("mcp-session-id", "abc123"), ("last-event-id", "17")],
        )
        .await?;
    assert_eq!(status, StatusCode::OK);
    assert!(body["result"]["tools"].is_array());
    Ok(())
}

#[tokio::test]
async fn a_foreign_origin_is_refused_before_dispatch() -> TestResult {
    let harness = Harness::start(true).await?;
    let (status, _body) = harness
        .post(
            &rpc("tools/list", &json!({ "_meta": meta(false) })),
            &[("origin", "http://evil.example")],
        )
        .await?;
    assert_eq!(status, StatusCode::FORBIDDEN);
    Ok(())
}

#[tokio::test]
async fn a_header_mismatch_is_400_and_minus_32020() -> TestResult {
    let harness = Harness::start(true).await?;
    let body = call("describe_run", &json!({}), false);
    let (status, value) = harness.post(&body, &[("mcp-name", "cancel")]).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(value["error"]["code"], -32020);

    // A missing required header is the same refusal.
    let (status, value) = harness.post(&body, &[("mcp-method", "")]).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(value["error"]["code"], -32020);
    Ok(())
}

/// `=?base64?…?=` must be decoded BEFORE the header is compared with the body.
/// A server comparing the raw sentinel string would reject a perfectly
/// conformant request.
#[tokio::test]
async fn a_base64_sentinel_mcp_name_is_decoded_before_comparison() -> TestResult {
    let harness = Harness::start(true).await?;
    let body = call(
        "describe_run",
        &json!({ "namespace": NAMESPACE, "workflow_id": workflow_id().to_string() }),
        false,
    );
    // "describe_run" base64-encoded.
    let (status, value) = harness
        .post(&body, &[("mcp-name", "=?base64?ZGVzY3JpYmVfcnVu?=")])
        .await?;
    assert_eq!(status, StatusCode::OK, "got {value}");

    // The sentinel is not a bypass: a decoded value that disagrees still fails.
    let (status, value) = harness
        .post(&body, &[("mcp-name", "=?base64?Y2FuY2Vs?=")])
        .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(value["error"]["code"], -32020);
    Ok(())
}

#[tokio::test]
async fn an_unsupported_protocol_version_is_400_and_minus_32022() -> TestResult {
    let harness = Harness::start(true).await?;
    let body = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/list",
        "params": { "_meta": {
            "io.modelcontextprotocol/protocolVersion": "2025-06-18",
            "io.modelcontextprotocol/clientCapabilities": {},
        }},
    });
    let (status, value) = harness.post(&body, &[]).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(value["error"]["code"], -32022);
    assert_eq!(value["error"]["data"]["requested"], "2025-06-18");
    assert_eq!(value["error"]["data"]["supported"][0], PROTOCOL_VERSION);
    Ok(())
}

#[tokio::test]
async fn an_unimplemented_method_is_404_with_minus_32601() -> TestResult {
    let harness = Harness::start(true).await?;
    for method in ["subscriptions/listen", "initialize", "tasks/list"] {
        let (status, value) = harness
            .post(&rpc(method, &json!({ "_meta": meta(true) })), &[])
            .await?;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method}");
        assert_eq!(value["error"]["code"], -32601, "{method}");
    }
    Ok(())
}

// ---------------------------------------------------------------- discovery

#[tokio::test]
async fn server_discover_carries_capabilities_instructions_and_cache_hints() -> TestResult {
    let harness = Harness::start(true).await?;
    let (status, value) = harness
        .post(
            &rpc("server/discover", &json!({ "_meta": meta(true) })),
            &[],
        )
        .await?;
    assert_eq!(status, StatusCode::OK);
    let result = &value["result"];
    assert_eq!(result["resultType"], "complete");
    assert_eq!(result["supportedVersions"][0], PROTOCOL_VERSION);
    assert!(result["capabilities"]["tools"].is_object());
    assert!(result["capabilities"]["extensions"][TASKS_EXTENSION].is_object());
    assert!(result["ttlMs"].as_u64().is_some());
    assert_eq!(result["cacheScope"], "private");
    assert_eq!(
        result["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        "aion"
    );
    let instructions = result["instructions"].as_str().unwrap_or_default();
    assert!(instructions.contains("describe_run"));
    assert!(instructions.contains("NEVER INVENT AN IDENTIFIER"));
    Ok(())
}

#[tokio::test]
async fn the_tasks_extension_is_advertised_only_to_a_declaring_client() -> TestResult {
    let harness = Harness::start(true).await?;
    let (_status, value) = harness
        .post(
            &rpc("server/discover", &json!({ "_meta": meta(false) })),
            &[],
        )
        .await?;
    assert!(value["result"]["capabilities"].get("extensions").is_none());
    Ok(())
}

#[tokio::test]
async fn tools_list_publishes_eight_fully_annotated_tools() -> TestResult {
    let harness = Harness::start(true).await?;
    let (status, value) = harness
        .post(&rpc("tools/list", &json!({ "_meta": meta(false) })), &[])
        .await?;
    assert_eq!(status, StatusCode::OK);
    let tools = value["result"]["tools"]
        .as_array()
        .ok_or("tools must be an array")?;
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert_eq!(
        names,
        vec![
            "describe_run",
            "read_transcript",
            "read_history",
            "list_runs",
            "query",
            "start_run",
            "signal",
            "cancel",
        ]
    );
    for tool in tools {
        assert_eq!(tool["inputSchema"]["type"], "object", "{}", tool["name"]);
        assert!(tool["outputSchema"].is_object(), "{}", tool["name"]);
        for hint in [
            "readOnlyHint",
            "destructiveHint",
            "idempotentHint",
            "openWorldHint",
        ] {
            assert!(
                tool["annotations"][hint].is_boolean(),
                "{} is missing {hint}",
                tool["name"]
            );
        }
        let read_only = tool["annotations"]["readOnlyHint"] == json!(true);
        let is_read_tool = matches!(
            tool["name"].as_str().unwrap_or_default(),
            "describe_run" | "read_transcript" | "read_history" | "list_runs" | "query"
        );
        assert_eq!(read_only, is_read_tool, "{}", tool["name"]);
    }
    assert_eq!(value["result"]["cacheScope"], "private");
    assert!(value["result"]["ttlMs"].as_u64().is_some());
    Ok(())
}

/// Aion's tools deliberately annotate nothing: a header value is visible to
/// every intermediary on the path, and every argument these tools take is
/// either an identity or a cursor with no routing value.
#[tokio::test]
async fn no_aion_tool_schema_annotates_an_argument_into_a_header() -> TestResult {
    let harness = Harness::start(true).await?;
    let (_status, value) = harness
        .post(&rpc("tools/list", &json!({ "_meta": meta(false) })), &[])
        .await?;
    let rendered = serde_json::to_string(&value["result"]["tools"])?;
    assert!(!rendered.contains("x-mcp-header"), "{rendered}");
    Ok(())
}

// ------------------------------------------------------------- read tools

#[tokio::test]
async fn describe_run_joins_status_current_step_and_transcript_handles() -> TestResult {
    let harness = Harness::start(true).await?;
    let (status, value) = harness
        .post(
            &call(
                "describe_run",
                &json!({ "namespace": NAMESPACE, "workflow_id": workflow_id().to_string() }),
                false,
            ),
            &[],
        )
        .await?;
    assert_eq!(status, StatusCode::OK);
    let projected = structured(&value)?;
    // An omitted run_id resolves to the LATEST generation, and the resolved id
    // comes back so the caller can carry it into a transcript read.
    assert_eq!(projected["run_id"], second_run().to_string());
    assert_eq!(projected["workflow_type"], "agent_chain");
    assert_eq!(projected["status"], "Running");
    assert_eq!(projected["current_step"]["activity_id"], 0);
    assert_eq!(projected["current_step"]["activity_type"], "dev_review");
    assert_eq!(projected["current_step"]["attempt"], 1);
    assert!(projected["unserved"].is_array());

    // Every emitted transcript handle is COMPLETE: it carries the run. The
    // enumeration is run-scoped by the storage key, so generation one's stream
    // — which exists — is not in this run's list.
    let transcripts = projected["transcripts"]
        .as_array()
        .ok_or("transcripts must be an array")?;
    assert_eq!(transcripts.len(), 1);
    assert_eq!(transcripts[0]["run_id"], second_run().to_string());
    assert_eq!(transcripts[0]["activity_id"], 0);
    assert_eq!(transcripts[0]["attempt"], 1);
    Ok(())
}

#[tokio::test]
async fn describe_run_refuses_a_run_that_is_not_this_workflows() -> TestResult {
    let harness = Harness::start(true).await?;
    let (status, value) = harness
        .post(
            &call(
                "describe_run",
                &json!({
                    "namespace": NAMESPACE,
                    "workflow_id": workflow_id().to_string(),
                    "run_id": Uuid::from_u128(0xdead).to_string(),
                }),
                false,
            ),
            &[],
        )
        .await?;
    // A tool-level refusal: a RESULT with isError, so the model can read it and
    // correct itself, never a protocol error.
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["result"]["isError"], true);
    let text = value["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(text.contains("is not a run of workflow"), "{text}");
    Ok(())
}

/// THE regression test for the run-id collision, phrased as the invariant.
///
/// Two generations, each with its own stream under the run-scoped key. The
/// read addressed to generation two returns generation two's conversation and
/// NOTHING of generation one's — the sibling is excluded by the key range, not
/// by a filter that could be dropped.
#[tokio::test]
async fn read_transcript_requires_a_run_and_serves_only_that_runs_stream() -> TestResult {
    let harness = Harness::start(true).await?;
    let handle = json!({
        "namespace": NAMESPACE,
        "workflow_id": workflow_id().to_string(),
        "run_id": second_run().to_string(),
        "activity_id": 0,
        "attempt": 1,
    });
    let (status, value) = harness
        .post(&call("read_transcript", &handle, false), &[])
        .await?;
    assert_eq!(status, StatusCode::OK);
    let projected = structured(&value)?;
    assert_eq!(projected["run_id"], second_run().to_string());
    assert_eq!(projected["events"].as_array().map(Vec::len), Some(2));
    assert_eq!(projected["head_seq"], 2);
    // Generation one published to the same (workflow, ordinal, attempt) under
    // ITS run; not one byte of it may appear in generation two's page.
    let rendered = serde_json::to_string(&projected["events"])?;
    assert!(
        !rendered.contains("generation one planning"),
        "a sibling generation's transcript leaked into this run's read: {rendered}"
    );

    // The handle is not optional and not decorative: omitting the run is an
    // argument-schema refusal, because the schema REQUIRES it.
    let mut without_run = handle.clone();
    if let Some(object) = without_run.as_object_mut() {
        drop(object.remove("run_id"));
    }
    let (status, value) = harness
        .post(&call("read_transcript", &without_run, false), &[])
        .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(value["error"]["code"], -32602);
    Ok(())
}

#[tokio::test]
async fn read_transcript_refuses_an_attempt_the_named_run_never_dispatched() -> TestResult {
    let harness = Harness::start(true).await?;
    let (status, value) = harness
        .post(
            &call(
                "read_transcript",
                &json!({
                    "namespace": NAMESPACE,
                    "workflow_id": workflow_id().to_string(),
                    "run_id": second_run().to_string(),
                    "activity_id": 9,
                    "attempt": 1,
                }),
                false,
            ),
            &[],
        )
        .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["result"]["isError"], true);
    let text = value["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(text.contains("never dispatched"), "{text}");
    Ok(())
}

#[tokio::test]
async fn read_transcript_pages_with_an_immediate_cursor() -> TestResult {
    let harness = Harness::start(true).await?;
    let (_status, value) = harness
        .post(
            &call(
                "read_transcript",
                &json!({
                    "namespace": NAMESPACE,
                    "workflow_id": workflow_id().to_string(),
                    "run_id": second_run().to_string(),
                    "activity_id": 0,
                    "attempt": 1,
                    "limit": 1,
                }),
                false,
            ),
            &[],
        )
        .await?;
    let projected = structured(&value)?;
    assert_eq!(projected["events"].as_array().map(Vec::len), Some(1));
    assert_eq!(projected["next_from_seq"], 1);
    Ok(())
}

/// A cursor past the end must report the stream's REAL head, not echo the
/// cursor back. An agent that trusted the echo would believe the transcript had
/// run further than it has and would keep asking for events that do not exist.
#[tokio::test]
async fn a_transcript_cursor_past_the_end_reports_the_real_head() -> TestResult {
    let harness = Harness::start(true).await?;
    let (_status, value) = harness
        .post(
            &call(
                "read_transcript",
                &json!({
                    "namespace": NAMESPACE,
                    "workflow_id": workflow_id().to_string(),
                    "run_id": second_run().to_string(),
                    "activity_id": 0,
                    "attempt": 1,
                    "from_seq": 500,
                }),
                false,
            ),
            &[],
        )
        .await?;
    let projected = structured(&value)?;
    assert_eq!(projected["events"].as_array().map(Vec::len), Some(0));
    assert_eq!(
        projected["head_seq"], 2,
        "the stream holds two records, whatever cursor the caller guessed"
    );
    Ok(())
}

#[tokio::test]
async fn read_history_pages_and_reports_page_immutability() -> TestResult {
    let harness = Harness::start(true).await?;
    let (status, value) = harness
        .post(
            &call(
                "read_history",
                &json!({
                    "namespace": NAMESPACE,
                    "workflow_id": workflow_id().to_string(),
                    "limit": 3,
                }),
                false,
            ),
            &[],
        )
        .await?;
    assert_eq!(status, StatusCode::OK);
    let projected = structured(&value)?;
    assert_eq!(projected["events"].as_array().map(Vec::len), Some(3));
    assert_eq!(projected["head_seq"], 8);
    assert_eq!(projected["next_from_seq"], 4);
    assert_eq!(
        projected["page_is_immutable"], true,
        "every event on this page sits below the head of an append-only history"
    );

    // The page that touches the head is honestly reported as not immutable.
    let (_status, value) = harness
        .post(
            &call(
                "read_history",
                &json!({
                    "namespace": NAMESPACE,
                    "workflow_id": workflow_id().to_string(),
                    "from_seq": 8,
                }),
                false,
            ),
            &[],
        )
        .await?;
    let head_page = structured(&value)?;
    assert_eq!(head_page["page_is_immutable"], false);
    Ok(())
}

#[tokio::test]
async fn read_history_refuses_a_workflow_the_caller_cannot_reach() -> TestResult {
    let harness = Harness::start(true).await?;
    let (status, value) = harness
        .post(
            &call(
                "read_history",
                &json!({
                    "namespace": "someone-elses-namespace",
                    "workflow_id": workflow_id().to_string(),
                }),
                false,
            ),
            &[],
        )
        .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["result"]["isError"], true);
    Ok(())
}

#[tokio::test]
async fn list_runs_enumerates_and_refuses_an_unspelled_status() -> TestResult {
    let harness = Harness::start(true).await?;
    let (status, value) = harness
        .post(
            &call("list_runs", &json!({ "namespace": NAMESPACE }), false),
            &[],
        )
        .await?;
    assert_eq!(status, StatusCode::OK);
    let projected = structured(&value)?;
    assert!(projected["runs"].is_array());
    assert_eq!(projected["namespace"], NAMESPACE);

    // A status outside the published enumeration is refused by the input
    // schema, not silently dropped — a dropped filter returns rows the caller
    // believes were filtered.
    let (status, value) = harness
        .post(
            &call(
                "list_runs",
                &json!({ "namespace": NAMESPACE, "status": "running" }),
                false,
            ),
            &[],
        )
        .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(value["error"]["code"], -32602);
    Ok(())
}

#[tokio::test]
async fn query_refuses_a_workflow_with_no_live_execution() -> TestResult {
    let harness = Harness::start(true).await?;
    let (status, value) = harness
        .post(
            &call(
                "query",
                &json!({
                    "namespace": NAMESPACE,
                    "workflow_id": workflow_id().to_string(),
                    "query_name": "state",
                }),
                false,
            ),
            &[],
        )
        .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        value["result"]["isError"], true,
        "a query that cannot be answered is a failure the model can see and act on, \
         never an empty success"
    );
    Ok(())
}

// ---------------------------------------------------------- mutating tools

#[tokio::test]
async fn an_unknown_tool_is_a_protocol_error_not_a_tool_failure() -> TestResult {
    let harness = Harness::start(true).await?;
    let (status, value) = harness
        .post(&call("delete_everything", &json!({}), false), &[])
        .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(value["error"]["code"], -32602);
    Ok(())
}

#[tokio::test]
async fn start_run_refuses_an_undeployed_workflow_type() -> TestResult {
    let harness = Harness::start(true).await?;
    let (status, value) = harness
        .post(
            &call(
                "start_run",
                &json!({ "namespace": NAMESPACE, "workflow_type": "never_deployed" }),
                false,
            ),
            &[],
        )
        .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["result"]["isError"], true);
    Ok(())
}

#[tokio::test]
async fn signal_refuses_a_workflow_with_no_live_execution() -> TestResult {
    let harness = Harness::start(true).await?;
    let (status, value) = harness
        .post(
            &call(
                "signal",
                &json!({
                    "namespace": NAMESPACE,
                    "workflow_id": workflow_id().to_string(),
                    "signal_name": "approve",
                }),
                false,
            ),
            &[],
        )
        .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        value["result"]["isError"], true,
        "a signal has nowhere to be delivered when no execution is resident, and the model \
         must be told rather than shown a success it can act on"
    );
    let text = value["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(!text.is_empty(), "a refusal must say what went wrong");
    Ok(())
}

/// `cancel` is the mutating tool whose whole path is exercisable without a
/// deployed package: the cancellation is recorded durably in history, and the
/// projection the NEXT `describe_run` reads is what proves it landed.
#[tokio::test]
async fn cancel_records_a_terminal_the_next_describe_run_projects() -> TestResult {
    let harness = Harness::start(true).await?;
    let describe = call(
        "describe_run",
        &json!({ "namespace": NAMESPACE, "workflow_id": workflow_id().to_string() }),
        false,
    );
    let (_status, before) = harness.post(&describe, &[]).await?;
    assert_eq!(
        structured(&before)?["status"],
        "Running",
        "the fixture starts on a live generation"
    );

    let (status, value) = harness
        .post(
            &call(
                "cancel",
                &json!({
                    "namespace": NAMESPACE,
                    "workflow_id": workflow_id().to_string(),
                    "reason": "superseded",
                }),
                false,
            ),
            &[],
        )
        .await?;
    assert_eq!(status, StatusCode::OK);
    let projected = structured(&value)?;
    assert_eq!(projected["cancelled"], true);
    assert_eq!(projected["reason"], "superseded");
    assert_eq!(projected["workflow_id"], workflow_id().to_string());

    let (_status, after) = harness.post(&describe, &[]).await?;
    assert_eq!(
        structured(&after)?["status"],
        "Cancelled",
        "status is a projection of history, so the cancel must be visible in the next read"
    );
    Ok(())
}

#[tokio::test]
async fn a_malformed_argument_is_refused_before_the_tool_runs() -> TestResult {
    let harness = Harness::start(true).await?;
    let (status, value) = harness
        .post(
            &call(
                "cancel",
                &json!({ "namespace": NAMESPACE, "workflow_id": "not-a-uuid" }),
                false,
            ),
            &[],
        )
        .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(value["error"]["code"], -32602);

    // A misspelled argument name is refused too: the schemas are closed, so a
    // filter that was never applied cannot be mistaken for one that was.
    let (status, _value) = harness
        .post(
            &call(
                "describe_run",
                &json!({
                    "namespace": NAMESPACE,
                    "workflow_id": workflow_id().to_string(),
                    "runid": second_run().to_string(),
                }),
                false,
            ),
            &[],
        )
        .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    Ok(())
}

// ---------------------------------------------------------------- tasks

#[tokio::test]
async fn an_awaited_start_from_a_non_declaring_client_is_minus_32021() -> TestResult {
    let harness = Harness::start(true).await?;
    let (status, value) = harness
        .post(
            &call(
                "start_run",
                &json!({
                    "namespace": NAMESPACE,
                    "workflow_type": "never_deployed",
                    "await_completion": true,
                }),
                false,
            ),
            &[],
        )
        .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        value["error"]["code"], -32021,
        "the core revision's MissingRequiredClientCapability, NOT the tasks draft's stale -32003"
    );
    assert_eq!(
        value["error"]["data"]["requiredCapabilities"][0],
        format!("extensions.{TASKS_EXTENSION}")
    );
    Ok(())
}

#[tokio::test]
async fn an_awaited_start_from_a_declaring_client_is_a_durable_task() -> TestResult {
    let harness = Harness::start(true).await?;
    let (status, value) = harness
        .post(
            &call(
                "start_run",
                &json!({
                    "namespace": NAMESPACE,
                    "workflow_type": "never_deployed",
                    "await_completion": true,
                }),
                true,
            ),
            &[],
        )
        .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["result"]["resultType"], "task");
    assert_eq!(value["result"]["status"], "working");
    let task_id = value["result"]["taskId"]
        .as_str()
        .ok_or("taskId must be a string")?
        .to_owned();

    // Durably created BEFORE the response was written: a tasks/get issued the
    // instant the client reads the result already resolves.
    let get = rpc(
        "tasks/get",
        &json!({ "_meta": meta(true), "taskId": task_id.clone() }),
    );
    let (status, value) = harness.post(&get, &[]).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        value["result"]["resultType"], "complete",
        "tasks/get answers `complete`; `task` marks a CreateTaskResult and nothing else"
    );
    assert_eq!(value["result"]["taskId"], task_id);

    // The start could not succeed (the type is not deployed), so the task
    // settles COMPLETED carrying an isError tool result — never `failed`, which
    // is reserved for a JSON-RPC error.
    let mut settled = Value::Null;
    for _ in 0..200 {
        let (_status, value) = harness.post(&get, &[]).await?;
        if value["result"]["status"] != json!("working") {
            settled = value;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(settled["result"]["status"], "completed", "{settled}");
    assert_eq!(settled["result"]["result"]["isError"], true);
    assert!(settled["result"].get("error").is_none());
    Ok(())
}

#[tokio::test]
async fn the_mcp_name_header_on_a_tasks_method_must_equal_the_task_id() -> TestResult {
    let harness = Harness::start(true).await?;
    let (status, value) = harness
        .post(
            &rpc(
                "tasks/get",
                &json!({ "_meta": meta(true), "taskId": "some-task" }),
            ),
            &[("mcp-name", "a-different-task")],
        )
        .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(value["error"]["code"], -32020);
    Ok(())
}

#[tokio::test]
async fn tasks_methods_are_gated_and_unknown_ids_are_minus_32602() -> TestResult {
    let harness = Harness::start(true).await?;
    for method in ["tasks/get", "tasks/update", "tasks/cancel"] {
        let (status, value) = harness
            .post(
                &rpc(
                    method,
                    &json!({
                        "_meta": meta(false),
                        "taskId": "no-such-task",
                        "inputResponses": {},
                    }),
                ),
                &[],
            )
            .await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{method}");
        assert_eq!(value["error"]["code"], -32021, "{method}");

        let (status, value) = harness
            .post(
                &rpc(
                    method,
                    &json!({
                        "_meta": meta(true),
                        "taskId": "no-such-task",
                        "inputResponses": {},
                    }),
                ),
                &[],
            )
            .await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{method}");
        assert_eq!(value["error"]["code"], -32602, "{method}");
    }
    Ok(())
}

#[tokio::test]
async fn tasks_cancel_is_acknowledged_empty_and_stops_the_wait() -> TestResult {
    let harness = Harness::start(true).await?;
    let (_status, value) = harness
        .post(
            &call(
                "start_run",
                &json!({
                    "namespace": NAMESPACE,
                    "workflow_type": "never_deployed",
                    "await_completion": true,
                }),
                true,
            ),
            &[],
        )
        .await?;
    let task_id = value["result"]["taskId"]
        .as_str()
        .ok_or("taskId must be a string")?
        .to_owned();
    let (status, value) = harness
        .post(
            &rpc(
                "tasks/cancel",
                &json!({ "_meta": meta(true), "taskId": task_id }),
            ),
            &[],
        )
        .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["result"]["resultType"], "complete");
    assert!(
        value["result"].get("taskId").is_none(),
        "the acknowledgement is EMPTY beyond the envelope"
    );
    Ok(())
}
