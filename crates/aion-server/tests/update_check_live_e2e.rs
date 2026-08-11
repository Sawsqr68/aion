//! LIVE end-to-end proof for the built-in update check (#189 slice one,
//! r1 Blocker B1 discharge, second half).
//!
//! The claim under test is the whole chain, on the REAL boot path: a server
//! built through `ServerState::build_with_store` (the full-boot constructor
//! that assembles the production dispatcher stack — declared-body executor,
//! update-check observer, shared status slot), with the embedded document
//! installed by the real boot install, started through the real
//! `POST /workflows/start` route, executing the real declared `curl` against
//! the real crates.io sparse index, recorded by the real observer, and served
//! by the real `GET /update-status` route.
//!
//! # Runtime-gated, the house way
//!
//! The chain's middle link is a network transfer, and the gates run offline —
//! so this test runs only when `AION_UPDATE_CHECK_LIVE=1` is set, and
//! otherwise reports a skip through `tracing` and returns `Ok(())`. Never
//! `#[ignore]`: an ignored test is invisible in the tally, a skipped one says
//! so in the log. The offline twin of this proof — the same wiring driven
//! with a local stand-in for the transfer — lives in `state.rs`'s
//! `a_completed_check_through_the_built_dispatcher_lands_in_the_returned_slot`
//! and runs in every battery.
//!
//! Run by hand before landing:
//!
//! ```sh
//! AION_UPDATE_CHECK_LIVE=1 cargo test -p aion-server --test update_check_live_e2e
//! ```

use std::time::Duration;

use aion_server::api::http::http_router;
use aion_server::config::ServerConfig;
use aion_server::update_check::{UpdateCheckInstall, install_embedded_update_check_for_server};
use aion_server::{ServerState, update_check::UPDATE_CHECK_WORKFLOW_TYPE};
use aion_store::InMemoryStore;
use axum::{
    body,
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;

type TestError = Box<dyn std::error::Error>;

/// The gate. `1` runs the live chain; anything else skips loudly.
const LIVE_GATE: &str = "AION_UPDATE_CHECK_LIVE";

/// Stock configuration rooted at a private temp home, exactly as the
/// assistant's out-of-box e2e boots — nothing here touches `~/.aion`.
const STOCK_CONFIG: &[u8] = b"";

/// How long the live check may take end to end before this proof fails:
/// generous for one HTTPS GET plus engine scheduling, and a bound so a hung
/// transfer fails the test instead of hanging the suite.
const LIVE_DEADLINE: Duration = Duration::from_secs(120);

/// Poll spacing while waiting for the recorded result.
const POLL_INTERVAL: Duration = Duration::from_millis(500);

fn authorized(path: &str) -> Result<Request<body::Body>, TestError> {
    Ok(Request::builder()
        .uri(path)
        .method("GET")
        .header("x-aion-subject", "update-check-live-e2e")
        .header("x-aion-namespaces", "default")
        .body(body::Body::empty())?)
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

/// The whole chain, live: boot → install → start via HTTP → curl runs →
/// observer records → `/update-status` serves the recorded answer.
///
/// The GATE IS IN THE NAME (r2-m2): cargo captures a passing test's output,
/// so the `eprintln!`/`tracing` skip lines below are invisible in an offline
/// battery log — the name is the one channel that always reaches the tally.
/// An offline reader seeing `…or_skips_without_env … ok` knows that `ok` may
/// be the skip; the dedicated gated invocation (`AION_UPDATE_CHECK_LIVE=1 …
/// -- --nocapture`) is where ran-and-passed is proven, with the run's own
/// log lines visible.
#[tokio::test(flavor = "multi_thread")]
async fn gated_live_check_moves_update_status_or_skips_without_env() -> Result<(), TestError> {
    if std::env::var(LIVE_GATE).as_deref() != Ok("1") {
        tracing::info!(
            gate = LIVE_GATE,
            "skipping the live update-check e2e: the gate variable is not `1` and the chain \
             needs the network (the offline wiring twin runs in every battery)"
        );
        // On stderr for `--nocapture` runs ONLY (r2-m2, corrected r3-m2):
        // cargo captures and DISCARDS a passing test's output, so neither
        // this line nor the tracing line above reaches a normal battery log.
        // The channel that always reaches the tally is the TEST NAME
        // (`…or_skips_without_env`), which is why the gate is spelled there;
        // this line exists so a `--nocapture` or gated invocation shows
        // explicitly which path ran.
        eprintln!(
            "SKIPPED (not a pass of the live chain): {LIVE_GATE} != 1, network-gated e2e \
             returned Ok(()) without running anything"
        );
        return Ok(());
    }

    let home = private_tempdir()?;
    let config = ServerConfig::from_slice_with_home(STOCK_CONFIG, home.path())?;
    let (_, runtime) = config.into_parts();

    // THE REAL BOOT PATH: `build_with_store` assembles the production
    // dispatcher stack — this is what `from_parts` test states deliberately
    // do not do, and what this proof exists to exercise.
    let state = ServerState::build_with_store(InMemoryStore::default(), runtime).await?;
    let install = install_embedded_update_check_for_server(&state).await;
    assert!(
        matches!(install, UpdateCheckInstall::Installed { .. }),
        "a fresh temp home must install the embedded document, got {install:?}"
    );
    let router = http_router(state.clone())?;

    // Before any check: honest absence.
    let before = router
        .clone()
        .oneshot(authorized("/update-status")?)
        .await?;
    assert_eq!(before.status(), StatusCode::OK);
    let before: Value =
        serde_json::from_slice(&body::to_bytes(before.into_body(), usize::MAX).await?)?;
    assert_eq!(before["latest_known"], Value::Null);

    // THE OPERATOR ACT: one plain workflow start through the real route.
    // `input` is the empty object: the document declares no inputs, and the
    // start route requires a payload to bind. The console's check-now sends
    // the same (pinned by its tests — the r2 live run of THIS test is what
    // caught the omission).
    let start_body = serde_json::to_vec(&json!({
        "namespace": "default",
        "workflow_type": UPDATE_CHECK_WORKFLOW_TYPE,
        "input": {},
    }))?;
    let start = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/workflows/start")
                .method("POST")
                .header("content-type", "application/json")
                .header("x-aion-subject", "update-check-live-e2e")
                .header("x-aion-namespaces", "default")
                .body(body::Body::from(start_body))?,
        )
        .await?;
    assert_eq!(
        start.status(),
        StatusCode::OK,
        "the start must be accepted: {}",
        String::from_utf8_lossy(&body::to_bytes(start.into_body(), usize::MAX).await?)
    );

    // The curl runs at the server; poll the route until the observer records.
    let deadline = tokio::time::Instant::now() + LIVE_DEADLINE;
    loop {
        let response = router
            .clone()
            .oneshot(authorized("/update-status")?)
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let status: Value =
            serde_json::from_slice(&body::to_bytes(response.into_body(), usize::MAX).await?)?;
        if let Some(latest) = status["latest_known"].as_str() {
            // The recorded answer is a stable release: three dot-separated
            // numeric parts, no prerelease — the M1 contract, asserted at the
            // served surface.
            assert!(
                !latest.contains('-')
                    && latest.split('.').count() == 3
                    && latest
                        .split('.')
                        .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())),
                "latest_known must be a stable release version, got `{latest}`"
            );
            assert!(
                status["checked_at"].as_str().is_some(),
                "a recorded check carries its timestamp"
            );
            tracing::info!(%latest, "live update check recorded and served");
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(format!(
                "the live check did not record within {LIVE_DEADLINE:?}; last answer: {status}"
            )
            .into());
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
