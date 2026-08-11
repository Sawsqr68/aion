//! What a start of the update check BINDS on each documented path (r2-m4).
//!
//! The ops doc names two ways to run a check: the console's check-now and "a
//! plain workflow start" from the CLI. The two travel different transports
//! with different payload rules, and the r2 live e2e proved the difference
//! matters (an HTTP start with no payload at all is refused outright). This
//! suite pins all three shapes OFFLINE — no curl completes here; what is
//! under test is start ACCEPTANCE, which resolves before any activity runs:
//!
//! - HTTP `input: {}` — the console's exact body — is accepted.
//! - HTTP `input: null` — serde deserializes JSON `null` into the DTO's
//!   `Option<Value>` as `None`, indistinguishable from an absent field, so it
//!   is refused as "payload is missing" (HTTP 500 today; the
//!   client-shaped-refusal-as-500 half is docketed server-side, r2-m5).
//! - The CLI's default `--input null` — which arrives over gRPC as a PRESENT
//!   payload carrying JSON `null` — is REFUSED by the engine's start-input
//!   schema validation (`StartInputRefused`: null is not an object), typed
//!   and named, never a silent mis-bind. A bare `aion start update_check`
//!   therefore does NOT work; the ops doc names the exact command
//!   (`--input '{}'`), and this pin is what keeps that sentence honest.

use std::collections::HashMap;

use aion_core::{Payload, RunId, WorkflowId};
use aion_server::ServerState;
use aion_server::api::http::http_router;
use aion_server::config::ServerConfig;
use aion_server::update_check::{
    UPDATE_CHECK_WORKFLOW_TYPE, UpdateCheckInstall, install_embedded_update_check_for_server,
};
use aion_store::InMemoryStore;
use axum::{
    body,
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;

type TestError = Box<dyn std::error::Error>;

/// Stock configuration rooted at a private temp home (assistant-e2e pattern).
const STOCK_CONFIG: &[u8] = b"";

/// A temp dir the server's private-root validation accepts.
fn private_tempdir() -> Result<tempfile::TempDir, TestError> {
    let dir = tempfile::tempdir()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(dir)
}

/// Real boot path + real boot install, offline.
async fn booted_state() -> Result<ServerState, TestError> {
    let home = private_tempdir()?;
    let config = ServerConfig::from_slice_with_home(STOCK_CONFIG, home.path())?;
    let (_, runtime) = config.into_parts();
    let state = ServerState::build_with_store(InMemoryStore::default(), runtime).await?;
    let install = install_embedded_update_check_for_server(&state).await;
    assert!(
        matches!(install, UpdateCheckInstall::Installed { .. }),
        "a fresh temp home must install the embedded document, got {install:?}"
    );
    Ok(state)
}

async fn http_start(
    state: &ServerState,
    body_value: &Value,
) -> Result<(StatusCode, Value), TestError> {
    let response = http_router(state.clone())?
        .oneshot(
            Request::builder()
                .uri("/workflows/start")
                .method("POST")
                .header("content-type", "application/json")
                .header("x-aion-subject", "update-check-binding-test")
                .header("x-aion-namespaces", "default")
                .body(body::Body::from(serde_json::to_vec(body_value)?))?,
        )
        .await?;
    let status = response.status();
    let decoded: Value =
        serde_json::from_slice(&body::to_bytes(response.into_body(), usize::MAX).await?)?;
    Ok((status, decoded))
}

/// Cancel a started test run before the test returns (r3-m5): an accepted
/// start of the GENUINE document would, if its dispatch won the race against
/// process teardown, run a real `curl` from an ungated offline test. The
/// review sampled three suite runs and saw zero curls — but sampling is a
/// lower bound, not quiet, so the run is cancelled explicitly. A run that
/// already ended reports `WorkflowNotFound`, which is equally final.
async fn cancel_started_run(
    state: &ServerState,
    workflow_id: &WorkflowId,
    run_id: &RunId,
) -> Result<(), TestError> {
    match state
        .engine()?
        .cancel(workflow_id, run_id, "offline binding test cleanup (r3-m5)")
        .await
    {
        Ok(()) => Ok(()),
        Err(error) if error.to_string().contains("not found") => Ok(()),
        Err(error) => Err(format!("the cleanup cancel failed unexpectedly: {error}").into()),
    }
}

/// Parse the ids an accepted HTTP start names, for the cleanup cancel.
fn started_ids(decoded: &Value) -> Result<(WorkflowId, RunId), TestError> {
    let workflow_id = decoded["workflow_id"]
        .as_str()
        .ok_or("an accepted start names its workflow id")?;
    let run_id = decoded["run_id"]
        .as_str()
        .ok_or("an accepted start names its run id")?;
    Ok((
        WorkflowId::new(workflow_id.parse()?),
        RunId::new(run_id.parse()?),
    ))
}

/// The console's exact shape: an explicit empty `input` object starts.
#[tokio::test(flavor = "multi_thread")]
async fn an_http_start_with_the_empty_object_input_is_accepted() -> Result<(), TestError> {
    let state = booted_state().await?;
    let (status, decoded) = http_start(
        &state,
        &json!({
            "namespace": "default",
            "workflow_type": UPDATE_CHECK_WORKFLOW_TYPE,
            "input": {},
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "refused: {decoded}");
    let (workflow_id, run_id) = started_ids(&decoded)?;
    cancel_started_run(&state, &workflow_id, &run_id).await?;
    Ok(())
}

/// HTTP `input: null` is REFUSED — serde folds JSON `null` into the absent
/// `Option`, so the route cannot tell it from no payload at all. Pinned so
/// the ops doc's claims stay honest about which paths accept what; the
/// 500-for-a-client-shaped-request half is the docketed server-side item.
#[tokio::test(flavor = "multi_thread")]
async fn an_http_start_with_null_input_is_refused_as_missing_payload() -> Result<(), TestError> {
    let state = booted_state().await?;
    let (status, decoded) = http_start(
        &state,
        &json!({
            "namespace": "default",
            "workflow_type": UPDATE_CHECK_WORKFLOW_TYPE,
            "input": null,
        }),
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "the null-input refusal changed shape; re-pin it and re-check the ops doc: {decoded}"
    );
    assert!(
        decoded["message"]
            .as_str()
            .is_some_and(|message| message.contains("payload is missing")),
        "the refusal must name the missing payload: {decoded}"
    );
    Ok(())
}

/// The CLI's default binding: `aion start update_check` sends a PRESENT
/// payload of JSON `null` (gRPC `--input null`). Pinned at the engine call:
/// the start-input schema validation REFUSES it by name — the document's
/// input is an object, and null is not one. This is why the ops doc's CLI
/// command carries `--input '{}'` explicitly.
///
/// MODELLED, NOT DRIVEN (r3-m4, named as the reviewer asked): this calls
/// `Engine::start_workflow` directly rather than running the real CLI, which
/// would need a live gRPC server and network setup an offline battery cannot
/// have. The inference that this is the same join: the CLI's `--input`
/// defaults to the string `"null"`, is parsed to a JSON payload, and arrives
/// at the gRPC handler as a PRESENT `ProtoPayload` that passes
/// `required_payload` and reaches exactly this engine call with exactly this
/// payload shape. The transport hop is the untested residue; the binding
/// decision is not.
#[tokio::test(flavor = "multi_thread")]
async fn a_cli_shaped_null_payload_start_is_refused_by_input_validation() -> Result<(), TestError> {
    let state = booted_state().await?;
    let engine = state.engine()?;
    let refusal = engine
        .start_workflow(
            UPDATE_CHECK_WORKFLOW_TYPE,
            Payload::from_json(&Value::Null)?,
            HashMap::new(),
            "default".to_owned(),
        )
        .await;
    match refusal {
        Err(error) => {
            let rendered = error.to_string();
            assert!(
                rendered.contains("null is not of type"),
                "the refusal must name the schema mismatch: {rendered}"
            );
            Ok(())
        }
        Ok(handle) => Err(format!(
            "a null start input must be refused by schema validation, but run {} started — \
             re-check the ops doc's CLI command if this ever changes",
            handle.workflow_id()
        )
        .into()),
    }
}

/// The engine accepts the CLI shape when the payload is the explicit empty
/// object — the exact input the documented command sends.
#[tokio::test(flavor = "multi_thread")]
async fn a_cli_shaped_empty_object_payload_start_binds_and_starts() -> Result<(), TestError> {
    let state = booted_state().await?;
    let engine = state.engine()?;
    let handle = engine
        .start_workflow(
            UPDATE_CHECK_WORKFLOW_TYPE,
            Payload::from_json(&json!({}))?,
            HashMap::new(),
            "default".to_owned(),
        )
        .await?;
    // Started is the pin; the run's own completion needs the network and is
    // the live e2e's business, not this one's.
    assert!(!handle.workflow_id().to_string().is_empty());
    let workflow_id = handle.workflow_id().clone();
    let run_id = handle.run_id().clone();
    cancel_started_run(&state, &workflow_id, &run_id).await?;
    Ok(())
}
