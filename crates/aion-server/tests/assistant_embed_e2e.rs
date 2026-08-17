//! Out-of-box proof for the built-in assistant (#139 leg C).
//!
//! The claim under test is the product one: a server booted from STOCK
//! configuration, on a home that has never seen a deploy, serves the assistant
//! and would run it. No repository checkout, no example crate, no operator
//! deploy step.
//!
//! # The home is a temporary directory, and it is named through the config API
//!
//! [`ServerConfig::from_slice_with_home`] is the crate's own way to root a
//! configuration at a home — the same assertion `AION_HOME` makes, without
//! mutating process-global environment state that a parallel test could observe
//! (and which this workspace cannot write anyway: `set_var` is `unsafe`, and
//! unsafe code is denied). Everything home-derived in the boot therefore points
//! at the temp directory, and the operator's real `~/.aion` is untouched.
//!
//! # Read the BODY
//!
//! The router under test here carries no ops-console fallback, so an unmatched
//! path is a real 404 — but the SHIPPED router merges the console, whose
//! catch-all answers `200 text/html` for anything unmatched. Assertions read
//! the content type and the payload, never the status alone.

use std::path::Path;
use std::sync::Arc;

use aion::EngineBuilder;
use aion_server::api::http::http_router;
use aion_server::assistant::{
    AssistantInstall, EMBEDDED_ASSISTANT_DOCUMENT, EmbeddedAssistant,
    install_embedded_assistant_for_server,
};
use aion_server::config::{NamespaceConfig, ServerConfig};
use aion_server::{NamespaceResolver, ServerState};
use aion_store::{EventStore, InMemoryStore};
use axum::{
    body,
    http::{Request, StatusCode},
    response::Response,
};
use serde_json::Value;
use tower::ServiceExt;

type TestError = Box<dyn std::error::Error>;

/// Stock configuration: an empty document, so every value is the built-in
/// default resolved against the given home. A test that hand-built a config
/// would prove nothing about what an operator gets out of the box.
/// The stock config a first-run server boots on, plus the two keys that have
/// no default: `observability.max_batch_events` / `max_batch_hold_ms` are the
/// transcript drain's flush policy, and the boot path refuses to build a
/// publisher without an operator ruling on them (the scaffolded `aion.toml`
/// ships both). A hold of 0 keeps this test's transcript timing unchanged.
const STOCK_CONFIG: &[u8] = b"[observability]\nmax_batch_events = 64\nmax_batch_hold_ms = 0\n";

struct Harness {
    _home: tempfile::TempDir,
    state: ServerState,
    router: axum::Router,
}

impl Harness {
    /// Boots a server on a fresh temp home and runs the real boot install.
    async fn boot() -> Result<Self, TestError> {
        let home = private_tempdir()?;
        let config = ServerConfig::from_slice_with_home(STOCK_CONFIG, home.path())?;
        let (_, runtime) = config.into_parts();

        let store = Arc::new(InMemoryStore::default());
        let event_store: Arc<dyn EventStore> = store;
        let engine = Arc::new(
            EngineBuilder::new()
                .store_arc(event_store)
                .in_memory_visibility()
                .scheduler_threads(1)
                .build()
                .await?,
        );
        let resolver =
            NamespaceResolver::from_config(NamespaceConfig::default(), Arc::clone(&engine));
        let state = ServerState::from_parts(resolver, runtime);
        let router = http_router(state.clone())?;
        Ok(Self {
            _home: home,
            state,
            router,
        })
    }
}

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

fn authorized(path: &str) -> Result<Request<body::Body>, TestError> {
    Ok(Request::builder()
        .uri(path)
        .method("GET")
        .header("x-aion-subject", "assistant-embed-test")
        .header("x-aion-namespaces", "default")
        .body(body::Body::empty())?)
}

async fn read_json(response: Response) -> Result<Value, TestError> {
    let bytes = body::to_bytes(response.into_body(), usize::MAX).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

async fn read_text(response: Response) -> Result<String, TestError> {
    let bytes = body::to_bytes(response.into_body(), usize::MAX).await?;
    Ok(String::from_utf8(bytes.to_vec())?)
}

fn content_type(response: &Response) -> String {
    response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}

/// A stock server on a fresh home installs the embedded assistant, routes it,
/// and serves its description — the whole out-of-box claim in one pass.
#[tokio::test]
async fn a_stock_server_on_a_fresh_home_serves_a_routed_assistant() -> Result<(), TestError> {
    let harness = Harness::boot().await?;
    let embedded = EmbeddedAssistant::load()?;

    let install = install_embedded_assistant_for_server(
        &harness.state,
        "add `liminal_listen_address` to `[outbox]` in the e2e config",
    )
    .await;
    assert_eq!(
        install,
        AssistantInstall::Installed {
            workflow_type: embedded.workflow_type().to_owned(),
            content_hash: embedded.content_hash().to_string(),
            task_queue: embedded.task_queue().to_owned(),
        },
        "a home that has never seen a deploy must be claimed by the boot install"
    );

    let response = harness
        .router
        .clone()
        .oneshot(authorized("/assistant")?)
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let media = content_type(&response);
    assert!(
        media.starts_with("application/json"),
        "the console catch-all answers text/html for an absent route; got `{media}`"
    );
    let body = read_json(response).await?;
    assert_eq!(body["workflow_type"], embedded.workflow_type());
    assert_eq!(body["content_hash"], embedded.content_hash().to_string());
    assert_eq!(
        body["residency"]["state"], "routed",
        "a fresh boot must leave the embedded assistant startable: {body}"
    );
    assert_eq!(body["session"]["continue_signal"], "assistant_continue");
    assert_eq!(
        body["task_queue"],
        embedded.task_queue(),
        "an operator with no checkout learns the queue to serve from here and nowhere else: {body}"
    );
    Ok(())
}

/// The served document is the embedded document, byte for byte — the artifact
/// an operator with no checkout feeds to `aion worker agent`.
#[tokio::test]
async fn a_stock_server_serves_the_assistant_document_itself() -> Result<(), TestError> {
    let harness = Harness::boot().await?;
    let response = harness
        .router
        .clone()
        .oneshot(authorized("/assistant/document")?)
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let media = content_type(&response);
    assert!(media.starts_with("text/plain"), "got `{media}`");
    let text = read_text(response).await?;
    assert_eq!(text, EMBEDDED_ASSISTANT_DOCUMENT);
    assert!(
        text.contains("workflow assistant"),
        "the served document must be the assistant workflow, not an empty file"
    );
    Ok(())
}

/// PACKAGING-INTEGRITY GUARD.
///
/// The assistant is served ENTIRELY from the compile-time embedded document, so
/// a plain `cargo install` must carry the real file. If
/// `crates/aion-server/assistant-embed/assistant.awl` ever slips out of git
/// tracking, or a stray `exclude` drops it from the packaged crate, the embed
/// would compile from whatever remained. This walks the same tracked path the
/// packaged crate would carry and compares it with what the binary holds — the
/// ops-console packaging guard's discipline, applied to the document.
#[test]
fn the_tracked_document_is_what_the_binary_embeds() -> Result<(), TestError> {
    let tracked = Path::new(env!("CARGO_MANIFEST_DIR")).join("assistant-embed/assistant.awl");
    assert!(
        tracked.is_file(),
        "the embedded document must exist at {} — the embed reads it with include_str!, and a \
         packaged crate that drops it ships a binary with no assistant",
        tracked.display()
    );
    let on_disk = std::fs::read_to_string(&tracked)?;
    assert_eq!(on_disk, EMBEDDED_ASSISTANT_DOCUMENT);
    Ok(())
}
