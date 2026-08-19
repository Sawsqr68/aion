//! The `io.modelcontextprotocol/tasks` lifecycle, end to end.
//!
//! The service under test holds NO task state — a task is resolved from the
//! handle on every request — so these tests are about the protocol's behaviour
//! over that projection: what a creation answers, what a read answers, and who
//! is allowed to read.

mod support;

use aion_mcp::TASKS_EXTENSION;
use aion_mcp::protocol::error::error_codes;
use serde_json::{Value, json};
use support::{FakeService, body_of, caller, caller_named, meta, post, server};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn request(id: &str, method: &str, params: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params.clone() })
}

/// A task-shaped call. `finished` decides what the work is doing when the
/// handle is read back — the fake's stand-in for a run that is still going
/// versus one that has ended.
fn call_task(declare_tasks: bool, finished: bool) -> Value {
    request(
        "call-1",
        "tools/call",
        &json!({
            "_meta": meta(declare_tasks),
            "name": "await_thing",
            "arguments": { "owner": "operator", "finished": finished },
        }),
    )
}

async fn created_task_id(
    server: &aion_mcp::McpServer<FakeService>,
    finished: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let created = body_of(
        &server
            .handle_http(&post(&call_task(true, finished), &[]), &caller())
            .await,
    )?;
    Ok(created["result"]["taskId"]
        .as_str()
        .ok_or("taskId must be a string")?
        .to_owned())
}

fn get(task_id: &str) -> Value {
    request(
        "get-1",
        "tasks/get",
        &json!({ "_meta": meta(true), "taskId": task_id }),
    )
}

#[tokio::test]
async fn a_created_task_advertises_no_lifetime_and_resolves_at_once() -> TestResult {
    let server = server()?;
    let created = body_of(
        &server
            .handle_http(&post(&call_task(true, false), &[]), &caller())
            .await,
    )?;
    let result = &created["result"];
    assert_eq!(result["resultType"], "task");
    assert_eq!(result["status"], "working");
    // `ttlMs` is REQUIRED and nullable, and `null` is the extension's spelling
    // of unlimited. A projection of durable work has nothing to expire, so any
    // number here would be one somebody had to invent.
    assert_eq!(result["ttlMs"], json!(null), "{result}");
    assert!(result["pollIntervalMs"].as_u64().is_some());
    let task_id = result["taskId"].as_str().ok_or("taskId must be a string")?;

    let read = body_of(
        &server
            .handle_http(&post(&get(task_id), &[]), &caller())
            .await,
    )?;
    assert_eq!(read["result"]["resultType"], "complete");
    assert_eq!(read["result"]["taskId"], task_id);
    assert_eq!(read["result"]["status"], "working");
    assert_eq!(read["result"]["ttlMs"], json!(null));
    Ok(())
}

/// Work that had already finished by the time the handle was minted is
/// reported as finished by the CREATION result too. One projection answers
/// both, so there is no moment where the server claims `working` about work
/// that has stopped.
#[tokio::test]
async fn creation_and_reading_agree_because_one_projection_answers_both() -> TestResult {
    let server = server()?;
    let created = body_of(
        &server
            .handle_http(&post(&call_task(true, true), &[]), &caller())
            .await,
    )?;
    assert_eq!(created["result"]["resultType"], "task");
    assert_eq!(created["result"]["status"], "completed");
    // A CreateTaskResult carries the FLAT task record; the payload is what
    // tasks/get is for.
    assert!(created["result"].get("result").is_none());

    let task_id = created["result"]["taskId"]
        .as_str()
        .ok_or("taskId must be a string")?;
    let read = body_of(
        &server
            .handle_http(&post(&get(task_id), &[]), &caller())
            .await,
    )?;
    assert_eq!(read["result"]["status"], "completed");
    assert_eq!(read["result"]["result"]["isError"], false);
    assert_eq!(read["result"]["result"]["structuredContent"]["done"], true);
    assert!(read["result"].get("error").is_none());
    Ok(())
}

/// 🔴 The handle is DERIVED, not secret. A second caller holding a valid
/// handle — not a bogus one, which would prove nothing — must be refused.
#[tokio::test]
async fn a_valid_handle_does_not_authorize_a_caller_who_may_not_read_the_work() -> TestResult {
    let server = server()?;
    let task_id = created_task_id(&server, false).await?;

    // The owner can read it, so the handle really is valid: without this the
    // refusal below could be a refusal of anything.
    let mine = server
        .handle_http(&post(&get(&task_id), &[]), &caller())
        .await;
    assert_eq!(mine.status, 200);

    for method in ["tasks/get", "tasks/update", "tasks/cancel"] {
        let body = request(
            "other-1",
            method,
            &json!({
                "_meta": meta(true),
                "taskId": task_id.clone(),
                "inputResponses": {},
            }),
        );
        let response = server
            .handle_http(&post(&body, &[]), &caller_named("someone-else"))
            .await;
        assert_eq!(response.status, 400, "{method} must refuse a stranger");
        assert_eq!(
            body_of(&response)?["error"]["code"],
            error_codes::INVALID_PARAMS,
            "{method}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn a_non_declaring_client_is_refused_with_the_core_capability_code() -> TestResult {
    let server = server()?;
    let response = server
        .handle_http(&post(&call_task(false, false), &[]), &caller())
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

/// A task-shaped call whose work never started is answered INLINE with the
/// failure. A task pointing at work that does not exist could never resolve,
/// and an unresolvable handle hides the error that caused it.
#[tokio::test]
async fn a_failed_task_shaped_call_is_answered_inline_and_mints_no_task() -> TestResult {
    let server = server()?;
    let body = request(
        "call-2",
        "tools/call",
        &json!({
            "_meta": meta(true),
            "name": "await_thing",
            "arguments": { "owner": "somebody-else" },
        }),
    );
    let response = server.handle_http(&post(&body, &[]), &caller()).await;
    assert_eq!(response.status, 200);
    let value = body_of(&response)?;
    assert_eq!(value["result"]["resultType"], "complete");
    assert_eq!(value["result"]["isError"], true);
    assert!(
        value["result"].get("taskId").is_none(),
        "a call that failed must not hand back a task"
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

/// Cancellation is cooperative: the server acknowledges and this one never
/// honours it, because the work behind a task outlives the client's interest
/// in it. The task therefore keeps reporting the work's own truth.
#[tokio::test]
async fn cancellation_is_acknowledged_empty_and_changes_nothing() -> TestResult {
    let server = server()?;
    let task_id = created_task_id(&server, false).await?;
    let cancel = request(
        "cancel-1",
        "tasks/cancel",
        &json!({ "_meta": meta(true), "taskId": task_id.clone() }),
    );
    let response = server.handle_http(&post(&cancel, &[]), &caller()).await;
    assert_eq!(response.status, 200);
    let acknowledgement = body_of(&response)?;
    assert_eq!(acknowledgement["result"]["resultType"], "complete");
    // The acknowledgement is EMPTY beyond the envelope: no task fields ride on it.
    assert!(acknowledgement["result"].get("taskId").is_none());
    assert!(acknowledgement["result"].get("status").is_none());

    // And the task still says what the work says. There is no `cancel_requested`
    // status in the extension, so there is nothing a read could show instead.
    let read = body_of(
        &server
            .handle_http(&post(&get(&task_id), &[]), &caller())
            .await,
    )?;
    assert_eq!(read["result"]["status"], "working");
    Ok(())
}

#[tokio::test]
async fn tasks_update_acknowledges_and_requires_input_responses() -> TestResult {
    let server = server()?;
    let task_id = created_task_id(&server, false).await?;

    let missing = request(
        "update-0",
        "tasks/update",
        &json!({ "_meta": meta(true), "taskId": task_id.clone() }),
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
    let task_id = created_task_id(&server, false).await?;
    let response = server
        .handle_http(
            &post(&get(&task_id), &[("mcp-name", "some-other-task")]),
            &caller(),
        )
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
