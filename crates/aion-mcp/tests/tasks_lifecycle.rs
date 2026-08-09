//! The `io.modelcontextprotocol/tasks` lifecycle, end to end.

mod support;

use aion_mcp::TASKS_EXTENSION;
use aion_mcp::protocol::error::error_codes;
use serde_json::{Value, json};
use support::{body_of, caller, meta, post, server};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn request(id: &str, method: &str, params: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params.clone() })
}

fn call_task(declare_tasks: bool) -> Value {
    request(
        "call-1",
        "tools/call",
        &json!({ "_meta": meta(declare_tasks), "name": "await_thing", "arguments": {} }),
    )
}

/// Poll `tasks/get` until the task is terminal, or give up after a bounded
/// number of attempts so a hung task fails the test instead of hanging it.
async fn poll_until_terminal(
    server: &aion_mcp::McpServer<support::FakeService>,
    task_id: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    for _ in 0..200 {
        let body = request(
            "get-1",
            "tasks/get",
            &json!({ "_meta": meta(true), "taskId": task_id }),
        );
        let value = body_of(&server.handle_http(&post(&body, &[]), &caller()).await)?;
        let status = value["result"]["status"].as_str().unwrap_or_default();
        if matches!(status, "completed" | "failed" | "cancelled") {
            return Ok(value);
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    Err("the task never reached a terminal status".into())
}

#[tokio::test]
async fn a_task_is_created_durably_and_resolves_immediately() -> TestResult {
    let server = server()?;
    let created = body_of(
        &server
            .handle_http(&post(&call_task(true), &[]), &caller())
            .await,
    )?;
    let result = &created["result"];
    assert_eq!(result["resultType"], "task");
    assert_eq!(result["status"], "working");
    assert!(result["ttlMs"].as_u64().is_some());
    assert!(result["pollIntervalMs"].as_u64().is_some());
    let task_id = result["taskId"].as_str().ok_or("taskId must be a string")?;
    // A `tasks/get` the instant the creation result is in hand already resolves.
    let get = request(
        "get-0",
        "tasks/get",
        &json!({ "_meta": meta(true), "taskId": task_id }),
    );
    let read = body_of(&server.handle_http(&post(&get, &[]), &caller()).await)?;
    assert_eq!(read["result"]["resultType"], "complete");
    assert_eq!(read["result"]["taskId"], task_id);
    Ok(())
}

#[tokio::test]
async fn a_completed_task_embeds_the_result_the_call_would_have_returned() -> TestResult {
    let server = server()?;
    let created = body_of(
        &server
            .handle_http(&post(&call_task(true), &[]), &caller())
            .await,
    )?;
    let task_id = created["result"]["taskId"]
        .as_str()
        .ok_or("taskId must be a string")?
        .to_owned();
    let terminal = poll_until_terminal(&server, &task_id).await?;
    assert_eq!(terminal["result"]["status"], "completed");
    assert_eq!(terminal["result"]["resultType"], "complete");
    assert_eq!(terminal["result"]["result"]["isError"], false);
    assert_eq!(
        terminal["result"]["result"]["structuredContent"]["done"],
        true
    );
    assert!(terminal["result"].get("error").is_none());
    Ok(())
}

#[tokio::test]
async fn a_non_declaring_client_is_refused_with_the_core_capability_code() -> TestResult {
    let server = server()?;
    let response = server
        .handle_http(&post(&call_task(false), &[]), &caller())
        .await;
    assert_eq!(response.status, 400);
    let value = body_of(&response)?;
    assert_eq!(
        value["error"]["code"],
        error_codes::MISSING_REQUIRED_CLIENT_CAPABILITY
    );
    assert_ne!(
        value["error"]["code"],
        error_codes::TASKS_DRAFT_MISSING_CAPABILITY
    );
    assert_eq!(
        value["error"]["data"]["requiredCapabilities"][0],
        format!("extensions.{TASKS_EXTENSION}")
    );
    Ok(())
}

#[tokio::test]
async fn tasks_methods_are_gated_on_the_declared_extension() -> TestResult {
    let server = server()?;
    for method in ["tasks/get", "tasks/update", "tasks/cancel"] {
        let body = request(
            "gate-1",
            method,
            &json!({ "_meta": meta(false), "taskId": "whatever", "inputResponses": {} }),
        );
        let response = server.handle_http(&post(&body, &[]), &caller()).await;
        assert_eq!(response.status, 400, "{method} must be gated");
        assert_eq!(
            body_of(&response)?["error"]["code"],
            error_codes::MISSING_REQUIRED_CLIENT_CAPABILITY,
            "{method} must use the core capability code"
        );
    }
    Ok(())
}

#[tokio::test]
async fn an_unknown_task_id_is_invalid_params_on_every_tasks_method() -> TestResult {
    let server = server()?;
    for method in ["tasks/get", "tasks/update", "tasks/cancel"] {
        let body = request(
            "unknown-1",
            method,
            &json!({ "_meta": meta(true), "taskId": "no-such-task", "inputResponses": {} }),
        );
        let response = server.handle_http(&post(&body, &[]), &caller()).await;
        assert_eq!(response.status, 400, "{method}");
        assert_eq!(
            body_of(&response)?["error"]["code"],
            error_codes::INVALID_PARAMS,
            "{method}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn cancellation_is_acknowledged_empty_and_settles_the_task() -> TestResult {
    let server = server()?;
    let created = body_of(
        &server
            .handle_http(&post(&call_task(true), &[]), &caller())
            .await,
    )?;
    let task_id = created["result"]["taskId"]
        .as_str()
        .ok_or("taskId must be a string")?
        .to_owned();
    let cancel = request(
        "cancel-1",
        "tasks/cancel",
        &json!({ "_meta": meta(true), "taskId": task_id }),
    );
    let response = server.handle_http(&post(&cancel, &[]), &caller()).await;
    assert_eq!(response.status, 200);
    let acknowledgement = body_of(&response)?;
    assert_eq!(acknowledgement["result"]["resultType"], "complete");
    // The acknowledgement is EMPTY beyond the envelope: no task fields ride on it.
    assert!(acknowledgement["result"].get("taskId").is_none());
    assert!(acknowledgement["result"].get("status").is_none());

    let terminal = poll_until_terminal(&server, &task_id).await?;
    assert_eq!(terminal["result"]["status"], "cancelled");
    Ok(())
}

#[tokio::test]
async fn tasks_update_acknowledges_and_requires_input_responses() -> TestResult {
    let server = server()?;
    let created = body_of(
        &server
            .handle_http(&post(&call_task(true), &[]), &caller())
            .await,
    )?;
    let task_id = created["result"]["taskId"]
        .as_str()
        .ok_or("taskId must be a string")?
        .to_owned();

    let missing = request(
        "update-0",
        "tasks/update",
        &json!({ "_meta": meta(true), "taskId": task_id }),
    );
    let response = server.handle_http(&post(&missing, &[]), &caller()).await;
    assert_eq!(response.status, 400);
    assert_eq!(
        body_of(&response)?["error"]["code"],
        error_codes::INVALID_PARAMS
    );

    let update = request(
        "update-1",
        "tasks/update",
        &json!({
            "_meta": meta(true),
            "taskId": task_id,
            "inputResponses": { "never_issued": { "action": "accept" } },
        }),
    );
    let response = server.handle_http(&post(&update, &[]), &caller()).await;
    assert_eq!(response.status, 200);
    let value = body_of(&response)?;
    assert_eq!(value["result"]["resultType"], "complete");
    assert!(value["result"].get("taskId").is_none());
    Ok(())
}

#[tokio::test]
async fn the_mcp_name_header_must_equal_the_task_id() -> TestResult {
    let server = server()?;
    let created = body_of(
        &server
            .handle_http(&post(&call_task(true), &[]), &caller())
            .await,
    )?;
    let task_id = created["result"]["taskId"]
        .as_str()
        .ok_or("taskId must be a string")?
        .to_owned();
    let body = request(
        "get-2",
        "tasks/get",
        &json!({ "_meta": meta(true), "taskId": task_id }),
    );
    let response = server
        .handle_http(&post(&body, &[("mcp-name", "some-other-task")]), &caller())
        .await;
    assert_eq!(response.status, 400);
    assert_eq!(
        body_of(&response)?["error"]["code"],
        error_codes::HEADER_MISMATCH
    );
    Ok(())
}

#[tokio::test]
async fn there_is_no_tasks_list_method() -> TestResult {
    let server = server()?;
    let body = request("list-1", "tasks/list", &json!({ "_meta": meta(true) }));
    let response = server.handle_http(&post(&body, &[]), &caller()).await;
    assert_eq!(response.status, 404);
    assert_eq!(
        body_of(&response)?["error"]["code"],
        error_codes::METHOD_NOT_FOUND
    );
    Ok(())
}
