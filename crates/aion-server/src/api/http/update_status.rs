//! `GET /update-status` — what this server runs, and the newest `aion-cli`
//! it has been TOLD exists.
//!
//! The joined read behind the ops console's update pill: `installed` from the
//! compiled-in [`crate::build_identity`], `latest_known`/`checked_at` from
//! the last completed manual update check
//! ([`crate::update_check::UpdateStatusState`]). Serving this endpoint never
//! touches the network — the fetch happens only inside an operator-started
//! check workflow, and this route reports whatever the last one recorded.
//!
//! # Absence is the honest fresh answer
//!
//! On a server where no check has ever completed, `latest_known` and
//! `checked_at` are `null`. Not an epoch, not an empty string, not the
//! installed version echoed back: nobody has measured anything yet, and the
//! response says exactly that.
//!
//! # Authorization
//!
//! Behind [`HttpCaller`] with the same posture as `/build`, and for the same
//! reason: the response embeds the running revision, which is the fact that
//! makes a known vulnerability actionable against a deployment.

use axum::{Json, extract::State};
use chrono::{DateTime, Utc};
use serde::Serialize;

use super::auth::HttpCaller;
use crate::ServerState;
use crate::build_identity::BuildIdentity;

/// Response body for `GET /update-status`.
#[derive(Debug, Serialize)]
pub(crate) struct UpdateStatusResponse {
    /// The version this binary was built as — [`BuildIdentity::version`].
    pub installed: &'static str,
    /// The greatest INSTALLABLE `aion-cli` version the last completed check
    /// found — not yanked and not a prerelease, the newest version a plain
    /// `cargo install` would take — or `null` when no check has ever
    /// completed.
    pub latest_known: Option<String>,
    /// When the last completed check was recorded, or `null` when no check
    /// has ever completed.
    pub checked_at: Option<DateTime<Utc>>,
}

/// `GET /update-status`.
pub(crate) async fn update_status(
    HttpCaller(_caller): HttpCaller,
    State(state): State<ServerState>,
) -> Json<UpdateStatusResponse> {
    let last = state.update_status().last();
    let (latest_known, checked_at) = match last {
        Some(check) => (Some(check.latest_known), Some(check.checked_at)),
        None => (None, None),
    };
    Json(UpdateStatusResponse {
        installed: BuildIdentity::current().version,
        latest_known,
        checked_at,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use aion::EngineBuilder;
    use aion_store::{EventStore, InMemoryStore};
    use axum::{body, http::Request, http::StatusCode};
    use serde_json::Value;
    use tower::ServiceExt;

    use super::super::router::workflow_router;
    use super::super::test_support::{runtime_config, server_state};
    use crate::update_check::status::LastCheck;
    use crate::{
        NamespaceResolver, ServerState, StaticScheduleNamespaces, StaticWorkflowNamespaces,
        config::NamespaceMode,
    };

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    async fn state() -> Result<ServerState, Box<dyn std::error::Error>> {
        let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
        let engine = Arc::new(
            EngineBuilder::new()
                .store_arc(store)
                .in_memory_visibility()
                .scheduler_threads(1)
                .build()
                .await?,
        );
        let resolver = NamespaceResolver::from_parts(
            NamespaceMode::SharedEngine,
            Some(engine),
            Arc::new(StaticWorkflowNamespaces::default()),
            Arc::new(StaticScheduleNamespaces::default()),
        );
        let mut config = runtime_config();
        config.auth.enabled = false;
        server_state(resolver, config).await
    }

    async fn get_update_status(state: &ServerState) -> Result<Value, Box<dyn std::error::Error>> {
        let response = workflow_router(state.clone())
            .oneshot(
                Request::builder()
                    .uri("/update-status")
                    .body(body::Body::empty())?,
            )
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
            "the SPA catch-all answers text/html for an absent route, so a probe that cannot \
             see the content-type cannot tell this endpoint from its absence; got \
             `{content_type}`"
        );
        let bytes = body::to_bytes(response.into_body(), usize::MAX).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// A server that has never checked says so: real installed version,
    /// explicit `null` for both check fields — no fabricated zeros, no epoch.
    #[tokio::test]
    async fn a_fresh_server_reports_installed_and_honest_absence() -> TestResult {
        let state = state().await?;
        let body = get_update_status(&state).await?;
        assert_eq!(
            body["installed"],
            serde_json::json!(crate::BuildIdentity::current().version)
        );
        assert_eq!(body["latest_known"], Value::Null);
        assert_eq!(body["checked_at"], Value::Null);
        Ok(())
    }

    /// After a check is recorded, the same route serves it — value and
    /// timestamp exactly as recorded.
    #[tokio::test]
    async fn a_recorded_check_is_served_verbatim() -> TestResult {
        let state = state().await?;
        let checked_at = chrono::Utc::now();
        state.update_status().record(LastCheck {
            latest_known: "0.14.0".to_owned(),
            checked_at,
        });

        let body = get_update_status(&state).await?;
        assert_eq!(body["latest_known"], serde_json::json!("0.14.0"));
        let served: chrono::DateTime<chrono::Utc> =
            serde_json::from_value(body["checked_at"].clone())?;
        assert_eq!(served, checked_at);
        Ok(())
    }
}
