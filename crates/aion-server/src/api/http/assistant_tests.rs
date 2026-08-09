//! The served assistant surface.
//!
//! Every assertion reads the BODY. The ops-console catch-all answers
//! `200 text/html` for any path a binary does not serve, so a status-only probe
//! cannot tell a build that carries the assistant from one that does not — and
//! these tests exist precisely to tell those two apart.

use std::sync::Arc;

use aion::EngineBuilder;
use aion_store::{EventStore, InMemoryStore};
use axum::http::StatusCode;
use serde_json::Value;
use tower::ServiceExt;

use super::super::router::workflow_router;
use super::super::test_support::{get_request, read_json, read_text, runtime_config, server_state};
use crate::assistant::{
    CONTINUE_SIGNAL, EMBEDDED_ASSISTANT_DOCUMENT, EmbeddedAssistant, OBJECTIVE_INPUT,
    REPO_PATH_INPUT, STATUS_QUERY, install_embedded_assistant,
};
use crate::{
    NamespaceResolver, StaticScheduleNamespaces, StaticWorkflowNamespaces, config::NamespaceMode,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A router over a fresh engine, optionally with the boot install already run.
async fn router(install: bool) -> Result<axum::Router, Box<dyn std::error::Error>> {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = Arc::new(
        EngineBuilder::new()
            .store_arc(store)
            .in_memory_visibility()
            .scheduler_threads(1)
            .build()
            .await?,
    );
    if install {
        install_embedded_assistant(engine.as_ref()).await;
    }
    let resolver = NamespaceResolver::from_parts(
        NamespaceMode::SharedEngine,
        Some(engine),
        Arc::new(StaticWorkflowNamespaces::default()),
        Arc::new(StaticScheduleNamespaces::default()),
    );
    Ok(workflow_router(
        server_state(resolver, runtime_config()).await?,
    ))
}

/// `GET /assistant` describes the assistant this binary carries: identity,
/// start contract, control surfaces, and — after the boot install — that the
/// embedded version is the one the engine would run.
#[tokio::test]
async fn the_server_describes_the_assistant_it_carries() -> TestResult {
    let embedded = EmbeddedAssistant::load()?;
    let response = router(true)
        .await?
        .oneshot(get_request("/assistant")?)
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert!(
        content_type.starts_with("application/json"),
        "an absent route answers text/html from the SPA catch-all; got `{content_type}`"
    );

    let body: Value = read_json(response).await?;
    assert_eq!(body["workflow_type"], embedded.workflow_type());
    assert_eq!(body["content_hash"], embedded.content_hash().to_string());
    assert_eq!(body["document_filename"], "assistant.awl");
    assert_eq!(body["document_bytes"], EMBEDDED_ASSISTANT_DOCUMENT.len());
    assert_eq!(body["residency"]["state"], "routed");
    assert_eq!(body["session"]["objective_input"], OBJECTIVE_INPUT);
    assert_eq!(body["session"]["repo_path_input"], REPO_PATH_INPUT);
    assert_eq!(body["session"]["continue_signal"], CONTINUE_SIGNAL);
    assert_eq!(body["session"]["status_query"], STATUS_QUERY);
    assert!(
        body["queries"]
            .as_array()
            .is_some_and(|queries| queries.iter().any(|name| name == STATUS_QUERY)),
        "the description must carry the document's own queries: {body}"
    );
    assert!(
        body["signals"].as_array().is_some_and(|signals| signals
            .iter()
            .any(|signal| signal["name"] == CONTINUE_SIGNAL)),
        "the description must carry the document's own signals: {body}"
    );
    for input in [OBJECTIVE_INPUT, REPO_PATH_INPUT] {
        assert!(
            !body["input_schema"]["properties"][input].is_null(),
            "the start contract must name `{input}`: {body}"
        );
    }
    Ok(())
}

/// Without the boot install the SAME description reports the embedded document
/// as not loaded — so `residency` is a live read of the catalog, not a constant
/// the descriptor asserts about itself.
#[tokio::test]
async fn residency_reports_an_uninstalled_catalog_honestly() -> TestResult {
    let response = router(false)
        .await?
        .oneshot(get_request("/assistant")?)
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = read_json(response).await?;
    assert_eq!(
        body["residency"]["state"], "not_loaded",
        "an engine that never installed the document must not be described as routed: {body}"
    );
    assert!(
        body["residency"]["routed_hash"].is_null(),
        "nothing holds the route on a catalog with no assistant: {body}"
    );
    Ok(())
}

/// `GET /assistant/document` serves the document's own bytes, so an operator
/// with no repository checkout can still stand up the worker for its queue.
#[tokio::test]
async fn the_server_serves_the_document_itself() -> TestResult {
    let response = router(true)
        .await?
        .oneshot(get_request("/assistant/document")?)
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert!(content_type.starts_with("text/plain"), "got {content_type}");
    let text = read_text(response).await?;
    assert_eq!(
        text, EMBEDDED_ASSISTANT_DOCUMENT,
        "the served bytes must be the embedded bytes, byte for byte"
    );
    Ok(())
}

/// The discriminating control: this router really does 404 a path it does not
/// serve, so the 200s above are the routes' own answers rather than a router
/// that answers everything.
#[tokio::test]
async fn a_path_this_router_does_not_serve_is_a_404() -> TestResult {
    let response = router(true)
        .await?
        .oneshot(get_request("/assistant/not-a-real-surface")?)
        .await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    Ok(())
}
