//! HTTP handlers for the read-only deployed-AWL studio surface.
//!
//! Two GETs and nothing else. There is no POST, PUT, PATCH, or DELETE on this
//! route family — not disabled, not guarded: absent, so a mutation attempt is
//! a 405 from the router rather than a handler decision that could regress.
//! `deployed_routes_reject_every_mutating_method` pins that.

use aion_proto::WireError;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use super::auth::HttpCaller;
use super::error::HttpWireError;
use crate::ServerState;
use crate::awl::deployed::{DeployedDocument, DeployedError, DeployedVersion};

/// `GET /awl/deployed` — every loaded workflow version with the honest state
/// of its archived AWL source.
pub(crate) async fn list_deployed(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
) -> Result<Json<Vec<DeployedVersion>>, Response> {
    let engine = authorized_engine(&state, &caller).map_err(|error| refusal(&error))?;
    crate::awl::deployed::list_versions(&engine)
        .await
        .map(Json)
        .map_err(|error| DeployedHttpError(error).into_response())
}

/// `GET /awl/deployed/{workflow_type}/{content_hash}` — the archived AWL
/// document of one deployed version, with its schema files and projection.
pub(crate) async fn get_deployed_document(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Path((workflow_type, content_hash)): Path<(String, String)>,
) -> Result<Json<DeployedDocument>, Response> {
    let engine = authorized_engine(&state, &caller).map_err(|error| refusal(&error))?;
    crate::awl::deployed::read_document(&engine, &workflow_type, &content_hash)
        .await
        .map(Json)
        .map_err(|error| DeployedHttpError(error).into_response())
}

/// Authorizes a deployed-surface read and borrows the engine.
///
/// The gate is the DEPLOY guard, not the studio's plain authenticated check:
/// this surface reads the deployed catalog and the source of deployed code,
/// which is what `GET /deploy/versions` gates the same way. Using the weaker
/// studio check would let any authenticated caller enumerate deployed versions
/// through the studio that the deploy API refuses to show them.
fn authorized_engine(
    state: &ServerState,
    caller: &crate::CallerIdentity,
) -> Result<std::sync::Arc<aion::Engine>, crate::ServerError> {
    let guard = state.deploy_guard();
    guard.authorize(caller)?;
    guard.engine().map(std::sync::Arc::clone)
}

/// Renders a server refusal through the standard wire-error mapping.
fn refusal(error: &crate::ServerError) -> Response {
    HttpWireError(error.to_wire_error()).into_response()
}

pub(crate) struct DeployedHttpError(pub(crate) DeployedError);

impl IntoResponse for DeployedHttpError {
    fn into_response(self) -> Response {
        let (status, error_type) = match &self.0 {
            DeployedError::NotFound { .. } => (StatusCode::NOT_FOUND, "DeployedVersionNotFound"),
            // 404, not a 5xx and not an empty 200: the archived document is the
            // resource, and for this version it does not exist. The message
            // names both reasons it can be missing so the operator is not left
            // reading absence as breakage.
            DeployedError::NoArchivedSource { .. } => {
                (StatusCode::NOT_FOUND, "DeployedAwlSourceAbsent")
            }
            DeployedError::Unreadable { .. } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "DeployedArchiveUnreadable",
            ),
            DeployedError::UnsafeSchemaPath { .. } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "DeployedArchiveSchemaPathRefused",
            ),
            DeployedError::Staging(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "DeployedStagingFailed")
            }
            DeployedError::Catalog(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "DeployedCatalogUnavailable",
            ),
        };
        (
            status,
            Json(WireError::invalid_input(self.0.to_string()).with_error_type(error_type)),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::sync::Arc;

    use aion::EngineBuilder;
    use aion_package::AwlSource;
    use aion_store::{EventStore, InMemoryStore, PackageRecord};
    use axum::{Router, body, http::Request, http::StatusCode};
    use tower::ServiceExt;

    use super::super::router::workflow_router;
    use super::super::test_support::{read_json, runtime_config, server_state};
    use crate::awl::deployed::fixtures::{DOCUMENT, SCHEMA_BYTES, SCHEMA_PATH, manifest, record};
    use crate::config::{DeployConfig, NamespaceMode};
    use crate::{NamespaceResolver, StaticScheduleNamespaces, StaticWorkflowNamespaces};

    /// A router over a real engine, with `deploy.enabled` per case and
    /// authentication OFF — which makes the caller the operator, who holds the
    /// deploy grant server-side. The archive rows are persisted AFTER the
    /// engine is built, so they are exactly the "persisted but not loaded"
    /// state, which is what this surface has to render honestly.
    async fn deployed_router(
        deploy_enabled: bool,
        auth_enabled: bool,
        rows: Vec<PackageRecord>,
        workspace: Option<&Path>,
    ) -> Result<(Router, Arc<dyn EventStore>), Box<dyn std::error::Error>> {
        let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
        let engine = Arc::new(
            EngineBuilder::new()
                .store_arc(Arc::clone(&store))
                .in_memory_visibility()
                .scheduler_threads(1)
                .build()
                .await?,
        );
        for row in rows {
            store.put_package(row).await?;
        }
        let resolver = NamespaceResolver::from_parts(
            NamespaceMode::SharedEngine,
            Some(engine),
            Arc::new(StaticWorkflowNamespaces::default()),
            Arc::new(StaticScheduleNamespaces::default()),
        );
        let mut config = runtime_config();
        config.auth.enabled = auth_enabled;
        config.deploy = DeployConfig {
            enabled: deploy_enabled,
            max_archive_bytes: Some(1024 * 1024),
            max_inflated_bytes: Some(4 * 1024 * 1024),
        };
        config.authoring.workspace_dir = workspace.map(Path::to_path_buf);
        Ok((
            workflow_router(server_state(resolver, config).await?),
            store,
        ))
    }

    fn operator_request(method: &str, uri: &str) -> Result<Request<body::Body>, axum::http::Error> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("x-aion-subject", "operator")
            .body(body::Body::empty())
    }

    /// An archive carrying a document and the schema it imports.
    fn archived_document() -> Result<PackageRecord, Box<dyn std::error::Error>> {
        record(
            manifest("deployed_probe"),
            Some(AwlSource::new(
                "deployed_probe.awl",
                DOCUMENT,
                [(SCHEMA_PATH.to_owned(), SCHEMA_BYTES.to_vec())],
            )),
            1_700_000_000,
        )
    }

    /// Every file under `root`, by relative path, with its bytes.
    fn snapshot(root: &Path) -> std::io::Result<BTreeMap<String, Vec<u8>>> {
        let mut found = BTreeMap::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(directory) = pending.pop() {
            for entry in std::fs::read_dir(&directory)? {
                let path = entry?.path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    let key = path.strip_prefix(root).map_or_else(
                        |_| path.to_string_lossy().into_owned(),
                        |relative| relative.to_string_lossy().into_owned(),
                    );
                    found.insert(key, std::fs::read(&path)?);
                }
            }
        }
        Ok(found)
    }

    /// 🔴 THE READ-ONLY PIN AT THE TRANSPORT. Both endpoints answer (survival),
    /// and neither the operator's workspace nor the package store differs by a
    /// single byte afterwards (absence). The workspace half is the ruled
    /// invariant made testable: a deployed view must never write into the
    /// directory the operator is editing in.
    #[tokio::test]
    async fn reading_the_deployed_surface_changes_neither_workspace_nor_store()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = crate::test_support::private_tempdir()?;
        std::fs::write(
            workspace.path().join("deployed_probe.awl"),
            b"//! A DIFFERENT document the operator is editing.\nworkflow deployed_probe\n",
        )?;
        let row = archived_document()?;
        let content_hash = row.content_hash.clone();
        let (router, store) =
            deployed_router(true, false, vec![row.clone()], Some(workspace.path())).await?;

        let workspace_before = snapshot(workspace.path())?;
        let store_before = store.list_packages().await?;

        let listing = router
            .clone()
            .oneshot(operator_request("GET", "/awl/deployed")?)
            .await?;
        assert_eq!(listing.status(), StatusCode::OK);
        let versions: serde_json::Value = read_json(listing).await?;
        let versions = versions.as_array().ok_or("listing was not an array")?;
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0]["workflow_type"], "deployed_probe");
        assert_eq!(versions[0]["content_hash"], content_hash);
        assert_eq!(versions[0]["source"]["state"], "available");
        assert_eq!(versions[0]["source"]["document_name"], "deployed_probe.awl");
        assert_eq!(versions[0]["source"]["schema_count"], 1);
        assert_eq!(versions[0]["loaded"], false);

        let detail = router
            .clone()
            .oneshot(operator_request(
                "GET",
                &format!("/awl/deployed/deployed_probe/{content_hash}"),
            )?)
            .await?;
        assert_eq!(detail.status(), StatusCode::OK);
        let document: serde_json::Value = read_json(detail).await?;
        assert_eq!(
            document["source"], DOCUMENT,
            "the deployed surface must serve the ARCHIVED document, not the \
             same-named one in the operator's workspace"
        );
        assert_eq!(document["projection"]["ok"], true);

        assert_eq!(
            snapshot(workspace.path())?,
            workspace_before,
            "reading the deployed surface wrote into the operator's workspace"
        );
        assert_eq!(
            store.list_packages().await?,
            store_before,
            "reading the deployed surface rewrote the package store"
        );
        Ok(())
    }

    /// 🔴 NO MUTATION PATH EXISTS. The routes carry a single `get`, so every
    /// other method is a 405 the router itself answers — there is no handler
    /// for a write to reach, guarded or otherwise.
    #[tokio::test]
    async fn deployed_routes_reject_every_mutating_method() -> Result<(), Box<dyn std::error::Error>>
    {
        let row = archived_document()?;
        let content_hash = row.content_hash.clone();
        let (router, store) = deployed_router(true, false, vec![row], None).await?;
        let before = store.list_packages().await?;

        let detail = format!("/awl/deployed/deployed_probe/{content_hash}");
        for method in ["POST", "PUT", "PATCH", "DELETE"] {
            for uri in ["/awl/deployed", detail.as_str()] {
                let response = router
                    .clone()
                    .oneshot(operator_request(method, uri)?)
                    .await?;
                assert_eq!(
                    response.status(),
                    StatusCode::METHOD_NOT_ALLOWED,
                    "{method} {uri} reached something"
                );
            }
        }
        assert_eq!(
            store.list_packages().await?,
            before,
            "a refused method still changed the store"
        );
        Ok(())
    }

    /// The surface follows the deploy switch: with `[deploy]` off it is a
    /// plain 404, not an empty listing that would read as "nothing deployed".
    #[tokio::test]
    async fn the_deployed_surface_is_dark_when_deploy_is_disabled()
    -> Result<(), Box<dyn std::error::Error>> {
        let row = archived_document()?;
        let content_hash = row.content_hash.clone();
        let (router, _) = deployed_router(false, false, vec![row], None).await?;

        for uri in [
            "/awl/deployed".to_owned(),
            format!("/awl/deployed/deployed_probe/{content_hash}"),
        ] {
            let response = router
                .clone()
                .oneshot(operator_request("GET", &uri)?)
                .await?;
            assert_eq!(
                response.status(),
                StatusCode::NOT_FOUND,
                "{uri} answered with the deploy surface disabled"
            );
        }
        Ok(())
    }

    /// A version that carries no archived source is a 404 whose body STATES
    /// the absence and both of its causes — never a 200 with an empty
    /// document, and never a 500 that reads as breakage.
    #[tokio::test]
    async fn a_version_without_archived_source_answers_with_the_absence_stated()
    -> Result<(), Box<dyn std::error::Error>> {
        let row = record(manifest("gleam_authored"), None, 1_700_000_000)?;
        let content_hash = row.content_hash.clone();
        let (router, _) = deployed_router(true, false, vec![row], None).await?;

        let listing = router
            .clone()
            .oneshot(operator_request("GET", "/awl/deployed")?)
            .await?;
        let versions: serde_json::Value = read_json(listing).await?;
        assert_eq!(versions[0]["source"]["state"], "absent");

        let detail = router
            .oneshot(operator_request(
                "GET",
                &format!("/awl/deployed/gleam_authored/{content_hash}"),
            )?)
            .await?;
        assert_eq!(detail.status(), StatusCode::NOT_FOUND);
        let body: serde_json::Value = read_json(detail).await?;
        assert_eq!(body["error_type"], "DeployedAwlSourceAbsent");
        let message = body["message"]
            .as_str()
            .ok_or("refusal carried no message")?;
        assert!(message.contains("Gleam"), "{message}");
        assert!(
            message.contains("before deploys archived their source"),
            "{message}"
        );
        Ok(())
    }

    /// A GET carrying (or withholding) the deploy grant, expressed through
    /// whichever credential path is compiled: the JWT `deploy` claim under
    /// `feature = "auth"`, the development `x-aion-deploy` header otherwise.
    /// One shape, so the grant pin runs on BOTH builds rather than only the
    /// one the declared gate does not compile.
    fn deploy_grant_request(
        uri: &str,
        granted: bool,
    ) -> Result<Request<body::Body>, Box<dyn std::error::Error>> {
        #[cfg(feature = "auth")]
        let token = if granted {
            crate::auth::test_support::mint_token_with_deploy("alice", "tenant-a", true)?
        } else {
            crate::auth::test_support::mint_token("alice", "tenant-a")?
        };
        #[cfg(not(feature = "auth"))]
        let token = super::super::test_support::TOKEN.to_owned();

        let mut builder = Request::builder()
            .method("GET")
            .uri(uri)
            .header("authorization", format!("Bearer {token}"))
            .header("x-aion-subject", "alice")
            .header("x-aion-namespaces", "tenant-a");
        #[cfg(not(feature = "auth"))]
        if granted {
            builder = builder.header("x-aion-deploy", "true");
        }
        Ok(builder.body(body::Body::empty())?)
    }

    /// The gate is the DEPLOY grant, not the studio's plain authenticated
    /// check: a caller the deploy API would refuse must not enumerate deployed
    /// versions — or read deployed source — through the studio instead. The
    /// granted arm is the control: without it a surface that refused everyone
    /// would pass.
    #[tokio::test]
    async fn a_caller_without_the_deploy_grant_is_refused() -> Result<(), Box<dyn std::error::Error>>
    {
        let row = archived_document()?;
        let content_hash = row.content_hash.clone();
        let (router, _) = deployed_router(true, true, vec![row], None).await?;
        let detail = format!("/awl/deployed/deployed_probe/{content_hash}");

        for uri in ["/awl/deployed", detail.as_str()] {
            let denied = router
                .clone()
                .oneshot(deploy_grant_request(uri, false)?)
                .await?;
            assert_eq!(
                denied.status(),
                StatusCode::FORBIDDEN,
                "{uri} served deployed state to a caller with no deploy grant"
            );

            let granted = router
                .clone()
                .oneshot(deploy_grant_request(uri, true)?)
                .await?;
            assert_eq!(
                granted.status(),
                StatusCode::OK,
                "{uri} refused a caller who does hold the deploy grant"
            );
        }
        Ok(())
    }
}
