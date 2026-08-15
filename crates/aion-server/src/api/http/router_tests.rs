use std::{fs, sync::Arc};

use aion::EngineBuilder;
use aion_store::{EventStore, InMemoryStore};
use axum::{body, http::Request, http::StatusCode};
use tower::ServiceExt;

use super::super::test_support::{
    NAMESPACE, json_request, read_json, read_text, runtime_config, server_state,
};
use super::*;
use crate::{
    NamespaceResolver, StaticScheduleNamespaces, StaticWorkflowNamespaces,
    config::{NamespaceConfig, NamespaceMode, OpsConsoleAssetSource, OpsConsoleConfig},
};

#[tokio::test]
async fn ops_console_assets_serve_index_asset_and_do_not_shadow_public_api()
-> Result<(), Box<dyn std::error::Error>> {
    let bundle = crate::test_support::private_tempdir()?;
    fs::write(
        bundle.path().join("index.html"),
        "<!doctype html><title>Aion</title><script src=\"/app.js\"></script>",
    )?;
    fs::write(bundle.path().join("app.js"), "window.AION = true;")?;

    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = Arc::new(
        EngineBuilder::new()
            .store_arc(Arc::clone(&store))
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
    config.ops_console = OpsConsoleConfig {
        source: OpsConsoleAssetSource::FileSystem {
            asset_path: bundle.path().to_path_buf(),
        },
    };
    let router = http_router(server_state(resolver, config).await?)?;

    let root = router
        .clone()
        .oneshot(Request::builder().uri("/").body(body::Body::empty())?)
        .await?;
    assert_eq!(root.status(), StatusCode::OK);
    assert!(read_text(root).await?.contains("<title>Aion</title>"));

    let asset = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/app.js")
                .body(body::Body::empty())?,
        )
        .await?;
    assert_eq!(asset.status(), StatusCode::OK);
    assert_eq!(read_text(asset).await?, "window.AION = true;");

    let spa = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/ops-console/workflows/demo")
                .body(body::Body::empty())?,
        )
        .await?;
    assert_eq!(spa.status(), StatusCode::OK);
    assert!(read_text(spa).await?.contains("<title>Aion</title>"));

    let list = serde_json::json!({
        "namespace": NAMESPACE,
        "filter": { "workflow_type": "nonexistent" },
    });
    let list_response = router
        .oneshot(json_request("/workflows/list", &list)?)
        .await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body: serde_json::Value = read_json(list_response).await?;
    assert!(
        list_body["summaries"]
            .as_array()
            .ok_or("summaries missing")?
            .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn cors_preflight_and_actual_request_carry_allow_headers_for_configured_origin()
-> Result<(), Box<dyn std::error::Error>> {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = Arc::new(
        EngineBuilder::new()
            .store_arc(Arc::clone(&store))
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
    config.cors_allowed_origins = vec!["http://localhost:5173".to_owned()];
    let router = http_router(server_state(resolver, config).await?)?;

    // Preflight: the browser sends OPTIONS with the requested method/header;
    // the layer must answer with the matching allow-origin and allow-methods.
    let preflight = router
        .clone()
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/workflows/list")
                .header("origin", "http://localhost:5173")
                .header("access-control-request-method", "POST")
                .header("access-control-request-headers", "x-aion-namespaces")
                .body(body::Body::empty())?,
        )
        .await?;
    assert_eq!(
        preflight
            .headers()
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some("http://localhost:5173")
    );

    // Actual request from the allowed origin echoes the allow-origin header.
    let actual = router
        .oneshot(
            Request::builder()
                .uri("/health/live")
                .header("origin", "http://localhost:5173")
                .body(body::Body::empty())?,
        )
        .await?;
    assert_eq!(actual.status(), StatusCode::OK);
    assert_eq!(
        actual
            .headers()
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some("http://localhost:5173")
    );
    Ok(())
}

#[tokio::test]
async fn cors_absent_origins_install_no_layer() -> Result<(), Box<dyn std::error::Error>> {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = Arc::new(
        EngineBuilder::new()
            .store_arc(Arc::clone(&store))
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
    // runtime_config() leaves cors_allowed_origins empty (the secure default).
    let router = http_router(server_state(resolver, runtime_config()).await?)?;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/health/live")
                .header("origin", "http://localhost:5173")
                .body(body::Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_none(),
        "no CorsLayer must be installed when no origins are configured"
    );
    Ok(())
}

#[cfg(feature = "auth")]
#[tokio::test]
async fn worker_availability_allows_granted_jwt_namespace_and_denies_foreign_namespace()
-> Result<(), Box<dyn std::error::Error>> {
    let (engine, _, _) = super::super::test_support::shared_engine().await?;
    let resolver = NamespaceResolver::from_config(
        NamespaceConfig {
            mode: NamespaceMode::SharedEngine,
        },
        engine,
    );
    let router = http_router(server_state(resolver, runtime_config()).await?)?;

    let allowed = router
        .clone()
        .oneshot(json_request(
            "/awl/workers/availability",
            &serde_json::json!({
                "namespace": NAMESPACE,
                "task_queue": "orders",
            }),
        )?)
        .await?;
    assert_eq!(allowed.status(), StatusCode::OK);

    let foreign = router
        .oneshot(json_request(
            "/awl/workers/availability",
            &serde_json::json!({
                "namespace": "tenant-b",
                "task_queue": "orders",
            }),
        )?)
        .await?;
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);
    let body: serde_json::Value = read_json(foreign).await?;
    assert_eq!(body["code"], "namespace_denied");
    Ok(())
}

#[tokio::test]
async fn worker_availability_keeps_auth_off_operator_access()
-> Result<(), Box<dyn std::error::Error>> {
    let (engine, _, _) = super::super::test_support::shared_engine().await?;
    let resolver = NamespaceResolver::from_config(
        NamespaceConfig {
            mode: NamespaceMode::SharedEngine,
        },
        engine,
    );
    let mut config = runtime_config();
    config.auth.enabled = false;
    config.auth.jwks_url = None;
    let router = http_router(crate::ServerState::from_parts(resolver, config))?;
    let request = Request::builder()
        .method("POST")
        .uri("/awl/workers/availability")
        .header("content-type", "application/json")
        .body(body::Body::from(serde_json::to_vec(&serde_json::json!({
            "namespace": "operator-selected-namespace",
            "task_queue": "orders",
        }))?))?;

    let response = router.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = read_json(response).await?;
    assert_eq!(body["connected_workers"], 0);
    Ok(())
}

#[tokio::test]
async fn observability_routes_are_public_and_expose_expected_payloads()
-> Result<(), Box<dyn std::error::Error>> {
    // This test rides the production `build_with_store` startup path, so
    // under `feature = "auth"` the configured jwks_url must be a live
    // endpoint for the initial JWKS fetch.
    #[cfg(feature = "auth")]
    let config = {
        let mut config = runtime_config();
        config.auth.jwks_url = Some(crate::auth::test_support::serve_jwks()?);
        config
    };
    #[cfg(not(feature = "auth"))]
    let config = runtime_config();
    let router =
        http_router(crate::ServerState::build_with_store(InMemoryStore::default(), config).await?)?;

    let metrics_response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(body::Body::empty())?,
        )
        .await?;
    assert_eq!(metrics_response.status(), StatusCode::OK);
    assert_eq!(
        metrics_response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/plain; version=0.0.4; charset=utf-8")
    );
    let metrics_body = read_text(metrics_response).await?;
    assert!(metrics_body.contains("# HELP aion_workflows_started_total"));
    assert!(metrics_body.contains("# TYPE aion_workflows_started_total counter"));
    assert!(metrics_body.contains("# HELP aion_activity_duration_seconds"));
    assert!(metrics_body.contains("# TYPE aion_activity_duration_seconds histogram"));
    assert!(metrics_body.contains("aion_activity_duration_seconds_bucket"));
    assert!(metrics_body.contains("aion_store_operation_duration_seconds_bucket"));

    let live_response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health/live")
                .body(body::Body::empty())?,
        )
        .await?;
    assert_eq!(live_response.status(), StatusCode::OK);

    let ready_response = router
        .oneshot(
            Request::builder()
                .uri("/health/ready")
                .body(body::Body::empty())?,
        )
        .await?;
    assert_eq!(ready_response.status(), StatusCode::OK);
    Ok(())
}
