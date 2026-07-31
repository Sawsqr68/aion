//! `GET /build` — what code is this server (#123).
//!
//! The identity itself, and the reasoning behind every field, lives in
//! [`crate::build_identity`]. This module is only its transport.
//!
//! # Why it is not on `/whoami`
//!
//! `/whoami` answers "who am I to this server". This answers "what IS this
//! server" — a different question with a different audience. The first is
//! fetched by the console on every load to gate affordances; the second is read
//! by an operator once, during an incident. Folding the second into the first
//! would put a field nobody routinely needs into the response everybody
//! routinely fetches.
//!
//! # Authorization
//!
//! Behind the same [`HttpCaller`] extractor as every data route, rather than
//! joining `/health` on the unauthenticated surface. The revision a deployment
//! runs is not a public fact, and it is precisely the fact that makes a known
//! vulnerability actionable against it.
//!
//! # 🔴 Probe the BODY, never the status
//!
//! The ops-console SPA catch-all serves the app shell for any unmatched path,
//! so an image WITHOUT this route answers `200 text/html` here. A probe reading
//! only `%{http_code}` would report the endpoint present on every image ever
//! built — including the ones it exists to tell apart. See
//! [`crate::build_identity`] for the measurement that established this.

use axum::Json;

use super::auth::HttpCaller;
use crate::build_identity::BuildIdentity;

/// `GET /build`.
pub(crate) async fn build_identity(HttpCaller(_caller): HttpCaller) -> Json<BuildIdentity> {
    Json(BuildIdentity::current())
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
    use crate::{
        NamespaceResolver, StaticScheduleNamespaces, StaticWorkflowNamespaces,
        config::NamespaceMode,
    };

    async fn router() -> Result<axum::Router, Box<dyn std::error::Error>> {
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
        Ok(workflow_router(server_state(resolver, config).await?))
    }

    /// The endpoint answers with the compiled-in identity, as JSON.
    ///
    /// The content-type assertion is not decoration. An image lacking this route
    /// answers `200 text/html` from the ops-console catch-all, so
    /// `application/json` plus a present `commit` is what makes a probe against
    /// this contract able to FAIL — and an instrument that cannot fail is not
    /// an instrument.
    #[tokio::test]
    async fn the_server_can_say_what_code_it_is() -> Result<(), Box<dyn std::error::Error>> {
        let response = router()
            .await?
            .oneshot(Request::builder().uri("/build").body(body::Body::empty())?)
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
            "the SPA catch-all answers text/html for an absent route, so a probe that \
             cannot see the content-type cannot tell this endpoint from its absence; got `{content_type}`"
        );

        let bytes = body::to_bytes(response.into_body(), usize::MAX).await?;
        let body: Value = serde_json::from_slice(&bytes)?;
        let identity = crate::build_identity::BuildIdentity::current();
        assert_eq!(body["version"], serde_json::json!(identity.version));
        assert_eq!(body["commit"], serde_json::json!(identity.commit));
        assert_eq!(body["dirty"], serde_json::json!(identity.dirty));
        assert_eq!(body["built_at"], serde_json::json!(identity.built_at));
        Ok(())
    }

    /// The discriminating control: the API router really does 404 a path it does
    /// not serve.
    ///
    /// Without this, the test above proves only that SOMETHING answered `/build`
    /// — which is exactly the false positive the live probe hit on `/version`.
    /// Here there is no console fallback merged, so the 404 is the router's own
    /// answer and the 200 above is therefore the route's own answer.
    #[tokio::test]
    async fn a_path_this_router_does_not_serve_is_a_404() -> Result<(), Box<dyn std::error::Error>>
    {
        let response = router()
            .await?
            .oneshot(
                Request::builder()
                    .uri("/build-that-does-not-exist")
                    .body(body::Body::empty())?,
            )
            .await?;

        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "if this router answered every path, the sibling test would prove nothing"
        );
        Ok(())
    }
}
