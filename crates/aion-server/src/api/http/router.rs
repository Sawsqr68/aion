//! Public HTTP router construction.

use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{HeaderName, HeaderValue, Method, StatusCode, header},
    routing::{any, get, post},
};
use tower_http::cors::CorsLayer;

use super::assistant::{assistant_descriptor, assistant_document};
use super::authoring::compile_source;
use super::awl::{
    bind_run, check, create_document, deploy_authoring, edit, emit, format, get_document,
    get_layout, get_revision, get_run_status, list_documents, put_document, put_layout, scaffold,
    worker_availability,
};
use super::awl_deployed::{get_deployed_document, list_deployed};
use super::build::build_identity;
use super::changelog::changelog;
use super::children::list_children;
use super::cluster_command::cluster_command;
use super::deploy::{list_versions, route_version, unload_version, upload_package};
use super::describe_live::describe_live;
use super::dev_ui::{dev_register_mock, dev_replay_run, dev_trigger_run};
use super::events::subscribe_events_socket;
use super::history::{fetch_event, fetch_history};
use super::intervene::{intervene, list_attempts};
use super::managed_workers::{
    list_managed_workers, restart_managed_worker, start_managed_worker, stop_managed_worker,
};
use super::outbox::list_dead_letters;
use super::queues::list_unserved_queues;
use super::schedules::{
    create_schedule, delete_schedule, describe_schedule, list_schedules, pause_schedule,
    resume_schedule, update_schedule,
};
use super::transcripts::{fetch_transcript, list_transcript_streams};
use super::unrecoverable::list_unrecoverable_runs;
use super::update_status::update_status;
use super::whoami::whoami;
use super::worker_deployments::{
    delete_worker_deployment, get_worker_deployment, list_worker_deployments,
    put_worker_deployment, set_worker_deployment_desired_state,
};
use super::workers::{drain_worker, stop_worker};
use super::workflows::{
    cancel_workflow, count_workflows, describe_workflow, get_workflows, list_namespace_records,
    list_namespaces, post_list_workflows, post_namespace, query_workflow, rename_workflow,
    reopen_workflow, set_namespace_placement, signal_workflow, start_workflow,
};
use crate::mcp::{McpRuntime, mcp_disabled_router, mcp_router};
use crate::{ServerError, ServerState, observability, ops_console::assets};

/// Build the public HTTP application: workflow-management routes first, then
/// the ops-console static asset fallback. The ops console adds no data API.
///
/// # Errors
///
/// Returns [`ServerError::Config`] when ops-console assets are misconfigured, or
/// when the MCP surface is enabled and its published tool catalog cannot be
/// constructed.
pub fn http_router(state: ServerState) -> Result<Router, ServerError> {
    let ops_console = assets::ops_console_router(&state.runtime_config().ops_console)?;
    let cors = cors_layer(&state.runtime_config().cors_allowed_origins)?;
    let metrics = state.metrics().cloned();
    let health = state.health().cloned();
    let mut router = workflow_router(state.clone());
    // The MCP endpoint is another route on THIS listener: one port, one
    // process. It is merged here rather than inside `workflow_router` because
    // it is a top-level protocol surface alongside `/metrics` and `/health/*`,
    // not a workflow-management route.
    router = router.merge(mcp_family(&state)?.with_state(state));
    if let Some(metrics) = metrics {
        router = router.merge(Router::new().route(
            "/metrics",
            get(observability::metrics::metrics_handler).with_state(metrics),
        ));
    }
    if let Some(health) = health {
        router = router.merge(
            Router::new()
                .route("/health/live", get(observability::health::live))
                .route(
                    "/health/ready",
                    get(observability::health::ready).with_state(health),
                ),
        );
    }
    let router = router.merge(ops_console);
    // CORS is applied last so it wraps every public route the browser ops console
    // calls (the workflow API, /metrics, /health/*, the ops-console fallback).
    // With no configured origins `cors_layer` returns None and the router is
    // byte-identical to before — no cross-origin request is allowed (the secure
    // default). With origins set the layer also answers OPTIONS preflight.
    Ok(match cors {
        Some(cors) => router.layer(cors),
        None => router,
    })
}

/// Build the CORS layer for the public HTTP router from the operator-configured
/// allowed origins.
///
/// Returns `Ok(None)` when no origins are configured — the secure default:
/// the layer is not installed and no cross-origin request is permitted. When
/// origins are configured the layer is scoped to exactly those origins (never
/// `Any`, so it is safe to pair with credentialed requests), permits the
/// methods the ops console uses (GET, POST, PUT, DELETE, and OPTIONS preflight), and allows
/// exactly the request headers the API consumes.
///
/// # Errors
///
/// Returns [`ServerError::Config`] when a configured origin is not a valid HTTP
/// header value. Startup validation already rejects malformed origins, so this
/// is defense in depth.
fn cors_layer(allowed_origins: &[String]) -> Result<Option<CorsLayer>, ServerError> {
    if allowed_origins.is_empty() {
        return Ok(None);
    }
    let mut origins = Vec::with_capacity(allowed_origins.len());
    for origin in allowed_origins {
        let value = origin
            .parse::<HeaderValue>()
            .map_err(|source| ServerError::Config {
                message: format!("invalid CORS origin `{origin}`: {source}"),
            })?;
        origins.push(value);
    }
    let layer = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            HeaderName::from_static("x-aion-namespaces"),
            HeaderName::from_static("x-aion-subject"),
        ]);
    Ok(Some(layer))
}

/// Disabled deploy surface: a plain 404 with no body, indistinguishable
/// from an unmounted route family.
async fn deploy_disabled() -> StatusCode {
    StatusCode::NOT_FOUND
}

/// Disabled authoring surface: a plain 404 with no body, indistinguishable
/// from an unmounted route family. When `[authoring].gleam_path` is absent the
/// server compiles no Gleam and deploys pre-built `.aion` files only (CN7).
async fn authoring_disabled() -> StatusCode {
    StatusCode::NOT_FOUND
}

/// Disabled dev surface: a plain 404 with no body, indistinguishable from an
/// unmounted route family. When `[dev].enabled` is false the server mounts no
/// dev endpoints and installs no activity-mock decorator (CN4).
async fn dev_disabled() -> StatusCode {
    StatusCode::NOT_FOUND
}

/// Disabled deployment surfaces: a plain 404 with no body for the deployed-AWL
/// reader, worker-deployment management, and managed-worker lifecycle
/// families. All follow the deploy surface's switch and stay dark when the
/// operator closes it.
async fn deploy_surface_disabled() -> StatusCode {
    StatusCode::NOT_FOUND
}

/// The read-only deployed-AWL route family.
///
/// GET only: both routes carry a single `get`, so every other method is a 405
/// the router answers — there is no mutation handler on this family to reach,
/// guarded or otherwise. With the deploy surface off the family is a plain 404,
/// covering the bare path and everything under it (a `{*rest}` wildcard does
/// not match the bare path).
fn deployed_awl_router(deploy_enabled: bool) -> Router<ServerState> {
    if deploy_enabled {
        Router::new()
            .route("/awl/deployed", get(list_deployed))
            .route(
                "/awl/deployed/{workflow_type}/{content_hash}",
                get(get_deployed_document),
            )
    } else {
        Router::new()
            .route("/awl/deployed", any(deploy_surface_disabled))
            .route("/awl/deployed/{*rest}", any(deploy_surface_disabled))
    }
}

fn worker_deployment_router(deploy_enabled: bool) -> Router<ServerState> {
    if deploy_enabled {
        Router::new()
            .route("/worker-deployments", get(list_worker_deployments))
            .route(
                "/worker-deployments/{name}",
                get(get_worker_deployment)
                    .put(put_worker_deployment)
                    .delete(delete_worker_deployment),
            )
            .route(
                "/worker-deployments/{name}/desired-state",
                post(set_worker_deployment_desired_state),
            )
    } else {
        Router::new()
            .route("/worker-deployments", any(deploy_surface_disabled))
            .route("/worker-deployments/{*rest}", any(deploy_surface_disabled))
    }
}

/// The managed-worker route family: the always-mounted status join, and the
/// three lifecycle commands mounted only when `[deploy].enabled` is set.
///
/// The mount condition matches the surfaces these commands belong to: their
/// gRPC counterparts live on `DeployService`, which joins the listener only
/// under `[deploy].enabled` (`crate::run`), and the sibling
/// `/worker-deployments/*` family follows the same switch. With deploy off,
/// every path under `/workers/managed/` is a plain 404 — a server that is not
/// a deploy target exposes no lifecycle mutation on any transport. Within a
/// mounted route, authorization is the handlers' deploy-grant check (decided
/// per caller; with auth disabled the single-tenant operator holds the grant
/// server-side). The read stays mounted regardless, exactly as it always has
/// been: reporting the fleet is not a deploy-surface mutation.
fn managed_worker_router(deploy_enabled: bool) -> Router<ServerState> {
    let read = Router::new().route("/workers/managed", get(list_managed_workers));
    if deploy_enabled {
        read.route("/workers/managed/{name}/start", post(start_managed_worker))
            .route("/workers/managed/{name}/stop", post(stop_managed_worker))
            .route(
                "/workers/managed/{name}/restart",
                post(restart_managed_worker),
            )
    } else {
        read.route("/workers/managed/{*rest}", any(deploy_surface_disabled))
    }
}

/// The MCP route family, mounted on the SERVER'S OWN listener.
///
/// One port, one process: the MCP endpoint is another route on the same HTTP
/// surface the console and the workflow API are served from, not a second
/// listener. It is dark unless `[mcp].enabled` is set, and a dark surface is a
/// plain 404 — indistinguishable from a build that never had the route.
///
/// A construction failure (a published tool schema that will not compile) is
/// NOT downgraded to a dark surface: it is a server defect, and an operator who
/// asked for the endpoint would otherwise get a 404 and no idea why.
///
/// Each call builds one runtime, and a runtime owns one task store. That is
/// correct because a task is reachable only through the identifier its creator
/// was handed: two routers are two disjoint sets of handles, never a split view
/// of one set. The serving path builds the router exactly once.
fn mcp_family(state: &ServerState) -> Result<Router<ServerState>, ServerError> {
    if !state.runtime_config().mcp.enabled {
        return Ok(mcp_disabled_router());
    }
    let runtime = McpRuntime::build(state.clone(), &state.runtime_config().mcp)?;
    Ok(mcp_router(std::sync::Arc::new(runtime)))
}

/// Mount the development workflow controls only when explicitly enabled.
fn dev_router(enabled: bool) -> Router<ServerState> {
    if enabled {
        Router::new()
            .route("/dev/runs", post(dev_trigger_run))
            .route("/dev/mocks", post(dev_register_mock))
            .route("/dev/replay", post(dev_replay_run))
    } else {
        Router::new().route("/dev/{*rest}", any(dev_disabled))
    }
}

/// Build the public workflow-management HTTP router.
pub fn workflow_router(state: ServerState) -> Router {
    // The deploy surface is dark by default: when `[deploy].enabled` is
    // false the routes are not mounted and every `/deploy/*` path is a
    // plain 404 (the explicit catch-all keeps the ops-console SPA fallback
    // from answering for the deploy namespace). The archive upload route
    // disables the default body limit because `read_archive_body` enforces
    // the operator-configured `deploy.max_archive_bytes` ceiling while
    // streaming.
    let deploy = if state.runtime_config().deploy.enabled {
        Router::new()
            .route(
                "/deploy/packages",
                post(upload_package).layer(DefaultBodyLimit::disable()),
            )
            .route("/deploy/versions", get(list_versions))
            .route("/deploy/route", post(route_version))
            .route("/deploy/unload", post(unload_version))
    } else {
        Router::new().route("/deploy/{*rest}", any(deploy_disabled))
    };
    // The authoring surface is dark by default, gated on
    // `[authoring].gleam_path`: when it is unset the routes are not mounted and
    // every `/authoring/*` path is a plain 404 (the explicit catch-all keeps
    // the ops-console SPA fallback from answering for the authoring namespace).
    // With it absent the server compiles no Gleam and deploys pre-built `.aion`
    // files only (CN7).
    let authoring = if state.runtime_config().authoring.gleam_path.is_some() {
        Router::new().route("/authoring/compile", post(compile_source))
    } else {
        Router::new().route("/authoring/{*rest}", any(authoring_disabled))
    };
    // The full AWL studio is always mounted. Stock config supplies the
    // `aion-authoring` workspace; the typed unconfigured refusal remains for
    // manually constructed runtime configs that explicitly omit it.
    let awl_documents = Router::new()
        .route("/awl/documents", get(list_documents).post(create_document))
        .route(
            "/awl/documents/{*path}",
            get(get_document).put(put_document),
        )
        .route("/awl/layout/{*path}", get(get_layout).put(put_layout));
    let awl_deployed = deployed_awl_router(state.runtime_config().deploy.enabled);
    let awl = Router::new()
        .route("/awl/check", post(check))
        .route("/awl/emit", post(emit))
        .route("/awl/deploy", post(deploy_authoring))
        .route("/awl/revisions/{hash}", get(get_revision))
        .route("/awl/workers/availability", post(worker_availability))
        .route("/awl/runs/{deployment_id}", get(get_run_status))
        .route("/awl/runs/{deployment_id}/binding", post(bind_run))
        .route("/awl/edit", post(edit))
        .route("/awl/fmt", post(format))
        .route("/awl/scaffold", post(scaffold))
        .merge(awl_documents)
        .merge(awl_deployed);
    // The dev surface is dark by default, gated on `[dev].enabled`: when off the
    // routes are not mounted and every `/dev/*` path is a plain 404 (the
    // explicit catch-all keeps the ops-console SPA fallback from answering for
    // the dev namespace), and the engine runs the bare production dispatcher.
    let dev = dev_router(state.runtime_config().dev.enabled);
    deploy
        .merge(authoring)
        .merge(awl)
        .merge(dev)
        .merge(worker_deployment_router(
            state.runtime_config().deploy.enabled,
        ))
        .route("/whoami", get(whoami))
        .route("/build", get(build_identity))
        // The installed version joined with the last completed manual update
        // check — the ops console's update pill reads this. Serving it never
        // touches the network.
        .route("/update-status", get(update_status))
        // What changed in the version this server runs — embedded in the
        // binary, consumed by the console's "What's new" panel.
        .route("/changelog", get(changelog))
        // The built-in assistant, always mounted: it ships in the binary like
        // the ops console, so there is no configuration under which this server
        // has one and does not say so.
        .route("/assistant", get(assistant_descriptor))
        .route("/assistant/document", get(assistant_document))
        .route("/namespaces", get(list_namespaces).post(post_namespace))
        .route("/namespaces/records", get(list_namespace_records))
        .route(
            "/namespaces/{name}/placement",
            axum::routing::put(set_namespace_placement),
        )
        .route("/workflows", get(get_workflows))
        .route("/workflows/count", get(count_workflows))
        .route("/workflows/start", post(start_workflow))
        .route("/workflows/signal", post(signal_workflow))
        .route("/workflows/query", post(query_workflow))
        .route("/workflows/cancel", post(cancel_workflow))
        .route("/workflows/rename", post(rename_workflow))
        .route("/workflows/reopen", post(reopen_workflow))
        .route("/workflows/list", post(post_list_workflows))
        .route("/workflows/describe", post(describe_workflow))
        .route("/workflows/describe-live", post(describe_live))
        .route("/workflows/children", post(list_children))
        .route("/workflows/history", post(fetch_history))
        .route("/workflows/event", post(fetch_event))
        .route("/workflows/intervene", post(intervene))
        .route("/workflows/attempts", post(list_attempts))
        .route("/workflows/transcript", post(fetch_transcript))
        .route("/workflows/transcripts", post(list_transcript_streams))
        .route("/workflows/unrecoverable", get(list_unrecoverable_runs))
        .route("/events/stream", get(subscribe_events_socket))
        .route("/cluster/command", post(cluster_command))
        .route("/outbox/dead-letters", post(list_dead_letters))
        .route("/queues/unserved", get(list_unserved_queues))
        .merge(managed_worker_router(state.runtime_config().deploy.enabled))
        .route("/workers/{worker_id}/drain", post(drain_worker))
        .route("/workers/{worker_id}/stop", post(stop_worker))
        .route("/schedules", post(create_schedule).get(list_schedules))
        .route(
            "/schedules/{id}",
            get(describe_schedule)
                .put(update_schedule)
                .delete(delete_schedule),
        )
        .route("/schedules/{id}/pause", post(pause_schedule))
        .route("/schedules/{id}/resume", post(resume_schedule))
        .with_state(state)
}

#[cfg(test)]
#[path = "router_tests.rs"]
mod tests;
