//! End-to-end conformance of the MCP 2026-07-28 transport and dispatcher.
//!
//! Each test states one rule from the revision and drives a whole HTTP
//! request/response cycle through the server, so the rule is checked where it
//! actually has to hold rather than on a helper in isolation.

mod support;

use aion_mcp::protocol::error::error_codes;
use aion_mcp::protocol::headers::HeaderView;
use aion_mcp::{HttpRequest, PROTOCOL_VERSION, TASKS_EXTENSION};
use serde_json::{Value, json};
use support::{body_of, caller, meta, post, server};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn request(method: &str, params: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": "r-1", "method": method, "params": params.clone() })
}

#[tokio::test]
async fn server_discover_returns_the_full_cacheable_shape() -> TestResult {
    let server = server()?;
    let body = request("server/discover", &json!({ "_meta": meta(true) }));
    let response = server.handle_http(&post(&body, &[]), &caller()).await;
    assert_eq!(response.status, 200);
    let value = body_of(&response)?;
    let result = &value["result"];
    assert_eq!(result["resultType"], "complete");
    assert_eq!(result["supportedVersions"][0], PROTOCOL_VERSION);
    assert!(result["capabilities"]["tools"].is_object());
    assert!(result["capabilities"]["extensions"][TASKS_EXTENSION].is_object());
    assert!(result["ttlMs"].as_u64().is_some());
    assert_eq!(result["cacheScope"], "private");
    assert_eq!(
        result["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        "aion-test"
    );
    assert!(
        result["instructions"]
            .as_str()
            .is_some_and(|text| !text.is_empty())
    );
    Ok(())
}

#[tokio::test]
async fn a_non_declaring_client_is_not_offered_the_tasks_extension() -> TestResult {
    let server = server()?;
    let body = request("server/discover", &json!({ "_meta": meta(false) }));
    let response = server.handle_http(&post(&body, &[]), &caller()).await;
    let value = body_of(&response)?;
    assert!(value["result"]["capabilities"].get("extensions").is_none());
    Ok(())
}

#[tokio::test]
async fn tools_list_is_deterministic_cacheable_and_fully_annotated() -> TestResult {
    let server = server()?;
    let body = request("tools/list", &json!({ "_meta": meta(false) }));
    let first = body_of(&server.handle_http(&post(&body, &[]), &caller()).await)?;
    let second = body_of(&server.handle_http(&post(&body, &[]), &caller()).await)?;
    assert_eq!(first["result"]["tools"], second["result"]["tools"]);
    let tools = first["result"]["tools"]
        .as_array()
        .ok_or("tools must be an array")?;
    assert_eq!(tools.len(), 4);
    for tool in tools {
        assert_eq!(tool["inputSchema"]["type"], "object");
        for hint in [
            "readOnlyHint",
            "destructiveHint",
            "idempotentHint",
            "openWorldHint",
        ] {
            assert!(
                tool["annotations"][hint].is_boolean(),
                "tool {} is missing {hint}",
                tool["name"]
            );
        }
    }
    assert_eq!(first["result"]["cacheScope"], "private");
    assert_eq!(first["result"]["ttlMs"], 300_000);
    Ok(())
}

/// The header carries `42.0` and the body carries `42`. Those are the same
/// integer and different strings; a server comparing spellings would refuse a
/// request whose header and body agree perfectly.
#[tokio::test]
async fn an_integer_param_header_is_compared_numerically_not_as_a_string() -> TestResult {
    let server = server()?;
    let body = request(
        "tools/call",
        &json!({ "_meta": meta(false), "name": "routed", "arguments": { "shard": 42 } }),
    );
    for spelling in ["42", "42.0", "+42"] {
        let response = server
            .handle_http(&post(&body, &[("mcp-param-shard", spelling)]), &caller())
            .await;
        assert_eq!(
            response.status, 200,
            "header spelling `{spelling}` denotes the same integer as the body value 42"
        );
    }
    // A genuinely different number is still a mismatch: the numeric comparison
    // is a comparison, not an acceptance.
    let response = server
        .handle_http(&post(&body, &[("mcp-param-shard", "43")]), &caller())
        .await;
    assert_eq!(response.status, 400);
    assert_eq!(
        body_of(&response)?["error"]["code"],
        error_codes::HEADER_MISMATCH
    );
    Ok(())
}

#[tokio::test]
async fn a_param_header_obeys_the_full_three_armed_matrix() -> TestResult {
    let server = server()?;
    // Body carries the value, header absent -> reject.
    let present = request(
        "tools/call",
        &json!({ "_meta": meta(false), "name": "routed", "arguments": { "shard": 7 } }),
    );
    let response = server.handle_http(&post(&present, &[]), &caller()).await;
    assert_eq!(response.status, 400);
    assert_eq!(
        body_of(&response)?["error"]["code"],
        error_codes::HEADER_MISMATCH
    );

    // Body omits an annotated optional value, header sent anyway -> reject.
    let response = server
        .handle_http(
            &post(
                &present,
                &[("mcp-param-shard", "7"), ("mcp-param-label", "west")],
            ),
            &caller(),
        )
        .await;
    assert_eq!(response.status, 400);

    // Body omits it and the header is absent too -> accepted.
    let response = server
        .handle_http(&post(&present, &[("mcp-param-shard", "7")]), &caller())
        .await;
    assert_eq!(response.status, 200);
    Ok(())
}

#[tokio::test]
async fn an_unrecognised_param_header_is_forwarded_and_ignored() -> TestResult {
    let server = server()?;
    let body = request(
        "tools/call",
        &json!({ "_meta": meta(false), "name": "echo", "arguments": { "text": "hi" } }),
    );
    let response = server
        .handle_http(
            &post(&body, &[("mcp-param-proxy-hop", "edge-3")]),
            &caller(),
        )
        .await;
    assert_eq!(
        response.status, 200,
        "an intermediary's own Mcp-Param header must not break a server it fronts"
    );
    Ok(())
}

/// Aion's OWN tools annotate nothing, deliberately: a header value is visible
/// to every intermediary on the path. This pins that choice for the fake
/// catalog's unannotated tools, so a later edit that starts leaking a namespace
/// or an id into headers has to change this test on purpose.
#[tokio::test]
async fn only_the_deliberately_annotated_tool_carries_x_mcp_header() -> TestResult {
    let server = server()?;
    let body = request("tools/list", &json!({ "_meta": meta(false) }));
    let value = body_of(&server.handle_http(&post(&body, &[]), &caller()).await)?;
    let tools = value["result"]["tools"]
        .as_array()
        .ok_or("tools must be an array")?;
    for tool in tools {
        let rendered = serde_json::to_string(tool)?;
        let annotated = rendered.contains("x-mcp-header");
        assert_eq!(
            annotated,
            tool["name"] == "routed",
            "unexpected x-mcp-header state on {}",
            tool["name"]
        );
    }
    Ok(())
}

#[tokio::test]
async fn a_tool_call_returns_content_and_structured_content_without_cache_hints() -> TestResult {
    let server = server()?;
    let body = request(
        "tools/call",
        &json!({ "_meta": meta(false), "name": "echo", "arguments": { "text": "hi" } }),
    );
    let response = server.handle_http(&post(&body, &[]), &caller()).await;
    assert_eq!(response.status, 200);
    let value = body_of(&response)?;
    let result = &value["result"];
    assert_eq!(result["resultType"], "complete");
    assert_eq!(result["isError"], false);
    assert_eq!(result["structuredContent"]["text"], "operator:hi");
    assert_eq!(result["content"][0]["type"], "text");
    assert!(result.get("ttlMs").is_none());
    assert!(result.get("cacheScope").is_none());
    Ok(())
}

#[tokio::test]
async fn a_tool_level_failure_is_a_result_not_a_protocol_error() -> TestResult {
    let server = server()?;
    let body = request(
        "tools/call",
        &json!({ "_meta": meta(false), "name": "refuse", "arguments": {} }),
    );
    let response = server.handle_http(&post(&body, &[]), &caller()).await;
    assert_eq!(response.status, 200);
    let value = body_of(&response)?;
    assert!(value.get("error").is_none());
    assert_eq!(value["result"]["isError"], true);
    assert_eq!(value["result"]["content"][0]["text"], "the tool refused");
    Ok(())
}

#[tokio::test]
async fn an_unknown_tool_is_invalid_params() -> TestResult {
    let server = server()?;
    let body = request(
        "tools/call",
        &json!({ "_meta": meta(false), "name": "invent", "arguments": {} }),
    );
    let response = server.handle_http(&post(&body, &[]), &caller()).await;
    assert_eq!(response.status, 400);
    let value = body_of(&response)?;
    assert_eq!(value["error"]["code"], error_codes::INVALID_PARAMS);
    assert!(
        value["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("Unknown tool: invent"))
    );
    Ok(())
}

#[tokio::test]
async fn arguments_violating_the_input_schema_are_refused_before_the_tool_runs() -> TestResult {
    let server = server()?;
    let body = request(
        "tools/call",
        &json!({ "_meta": meta(false), "name": "echo", "arguments": { "text": 7 } }),
    );
    let response = server.handle_http(&post(&body, &[]), &caller()).await;
    assert_eq!(response.status, 400);
    assert_eq!(
        body_of(&response)?["error"]["code"],
        error_codes::INVALID_PARAMS
    );
    Ok(())
}

#[tokio::test]
async fn a_header_disagreeing_with_the_body_is_refused() -> TestResult {
    let server = server()?;
    let body = request(
        "tools/call",
        &json!({ "_meta": meta(false), "name": "echo", "arguments": { "text": "hi" } }),
    );
    let response = server
        .handle_http(&post(&body, &[("mcp-name", "refuse")]), &caller())
        .await;
    assert_eq!(response.status, 400);
    let value = body_of(&response)?;
    assert_eq!(value["error"]["code"], error_codes::HEADER_MISMATCH);
    Ok(())
}

#[tokio::test]
async fn a_missing_required_header_is_refused() -> TestResult {
    let server = server()?;
    let body = request("tools/list", &json!({ "_meta": meta(false) }));
    // An empty override value removes the header entirely.
    let response = server
        .handle_http(&post(&body, &[("mcp-method", "")]), &caller())
        .await;
    assert_eq!(response.status, 400);
    assert_eq!(
        body_of(&response)?["error"]["code"],
        error_codes::HEADER_MISMATCH
    );
    Ok(())
}

#[tokio::test]
async fn a_base64_sentinel_header_matches_the_plain_body_value() -> TestResult {
    let server = server()?;
    let body = request(
        "tools/call",
        &json!({ "_meta": meta(false), "name": "echo", "arguments": { "text": "hi" } }),
    );
    let response = server
        .handle_http(
            &post(&body, &[("mcp-name", "=?base64?ZWNobw==?=")]),
            &caller(),
        )
        .await;
    assert_eq!(response.status, 200);
    Ok(())
}

#[tokio::test]
async fn an_mcp_name_header_on_a_method_that_has_no_name_is_refused() -> TestResult {
    let server = server()?;
    let body = request("tools/list", &json!({ "_meta": meta(false) }));
    let response = server
        .handle_http(&post(&body, &[("mcp-name", "echo")]), &caller())
        .await;
    assert_eq!(response.status, 400);
    assert_eq!(
        body_of(&response)?["error"]["code"],
        error_codes::HEADER_MISMATCH
    );
    Ok(())
}

#[tokio::test]
async fn an_unimplemented_version_lists_the_supported_ones() -> TestResult {
    let server = server()?;
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": { "_meta": {
            "io.modelcontextprotocol/protocolVersion": "2025-11-25",
            "io.modelcontextprotocol/clientCapabilities": {},
        }},
    });
    let response = server.handle_http(&post(&body, &[]), &caller()).await;
    assert_eq!(response.status, 400);
    let value = body_of(&response)?;
    assert_eq!(
        value["error"]["code"],
        error_codes::UNSUPPORTED_PROTOCOL_VERSION
    );
    assert_eq!(value["error"]["data"]["requested"], "2025-11-25");
    assert_eq!(value["error"]["data"]["supported"][0], PROTOCOL_VERSION);
    Ok(())
}

#[tokio::test]
async fn a_request_without_client_capabilities_is_invalid_params() -> TestResult {
    let server = server()?;
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": { "_meta": {
            "io.modelcontextprotocol/protocolVersion": PROTOCOL_VERSION,
        }},
    });
    let response = server.handle_http(&post(&body, &[]), &caller()).await;
    assert_eq!(response.status, 400);
    assert_eq!(
        body_of(&response)?["error"]["code"],
        error_codes::INVALID_PARAMS
    );
    Ok(())
}

#[tokio::test]
async fn an_unimplemented_method_is_a_404_carrying_minus_32601() -> TestResult {
    let server = server()?;
    let body = request("subscriptions/listen", &json!({ "_meta": meta(false) }));
    let response = server.handle_http(&post(&body, &[]), &caller()).await;
    assert_eq!(response.status, 404);
    assert_eq!(
        body_of(&response)?["error"]["code"],
        error_codes::METHOD_NOT_FOUND
    );
    Ok(())
}

#[tokio::test]
async fn get_and_delete_are_405_and_no_session_header_is_ever_minted() -> TestResult {
    let server = server()?;
    for method in ["GET", "DELETE"] {
        let request = HttpRequest::new(
            method,
            HeaderView::new::<[(&str, &str); 0], _, _>([]),
            Vec::new(),
        );
        let response = server.handle_http(&request, &caller()).await;
        assert_eq!(response.status, 405, "{method} must not be served");
        assert!(
            response
                .headers
                .iter()
                .any(|(name, value)| name == "allow" && value == "POST")
        );
    }
    // A POST carrying the retired session and resumption headers is served
    // normally, and the response never echoes either of them.
    let body = request("tools/list", &json!({ "_meta": meta(false) }));
    let response = server
        .handle_http(
            &post(&body, &[("mcp-session-id", "abc"), ("last-event-id", "42")]),
            &caller(),
        )
        .await;
    assert_eq!(response.status, 200);
    assert!(
        !response
            .headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("mcp-session-id"))
    );
    Ok(())
}

#[tokio::test]
async fn a_notification_is_acknowledged_with_202_and_no_body() -> TestResult {
    let server = server()?;
    let body = json!({ "jsonrpc": "2.0", "method": "notifications/progress", "params": {} });
    let request = HttpRequest::new(
        "POST",
        HeaderView::new([("mcp-method", "notifications/progress")]),
        serde_json::to_vec(&body)?,
    );
    let response = server.handle_http(&request, &caller()).await;
    assert_eq!(response.status, 202);
    assert!(response.body.is_empty());
    Ok(())
}

#[tokio::test]
async fn a_disallowed_origin_is_403_before_anything_is_dispatched() -> TestResult {
    let server = server()?;
    let body = request("tools/list", &json!({ "_meta": meta(false) }));
    let response = server
        .handle_http(
            &post(&body, &[("origin", "http://evil.example")]),
            &caller(),
        )
        .await;
    assert_eq!(response.status, 403);
    let allowed = server
        .handle_http(
            &post(&body, &[("origin", "http://localhost:8080")]),
            &caller(),
        )
        .await;
    assert_eq!(allowed.status, 200);
    Ok(())
}

#[tokio::test]
async fn a_malformed_body_is_a_parse_error() -> TestResult {
    let server = server()?;
    let request = HttpRequest::new(
        "POST",
        HeaderView::new([("mcp-method", "tools/list")]),
        b"{not json".to_vec(),
    );
    let response = server.handle_http(&request, &caller()).await;
    assert_eq!(response.status, 400);
    assert_eq!(
        body_of(&response)?["error"]["code"],
        error_codes::PARSE_ERROR
    );
    Ok(())
}
