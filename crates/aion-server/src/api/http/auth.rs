//! Caller-identity extraction from HTTP request headers.

use std::collections::HashMap;

#[cfg(feature = "auth")]
use axum::http::header;
use axum::{
    extract::{FromRequestParts, Query},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, request::Parts},
    response::{IntoResponse, Response},
};

use crate::{CallerIdentity, ServerState};

pub(crate) struct HttpCaller(pub(crate) CallerIdentity);

impl FromRequestParts<ServerState> for HttpCaller {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &ServerState,
    ) -> Result<Self, Self::Rejection> {
        let caller = caller_from_headers(&parts.headers, state)
            .await
            .map_err(axum::response::IntoResponse::into_response)?;
        Ok(Self(caller))
    }
}

/// Caller identity for the `/events/stream` WebSocket handshake.
///
/// Browsers cannot attach custom request headers (`x-aion-namespaces`,
/// `x-aion-subject`, `Authorization`) to a WebSocket handshake, so the same
/// credentials the REST API takes as headers are also accepted here as query
/// parameters and promoted to their header form before the single shared
/// header-based resolution ([`caller_from_headers`]) runs. An explicit header,
/// when present, always wins over its query-parameter fallback. This is the
/// standard browser-WebSocket authorization pattern; it introduces no second
/// auth code path.
pub(crate) struct WsCaller(pub(crate) CallerIdentity);

impl FromRequestParts<ServerState> for WsCaller {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &ServerState,
    ) -> Result<Self, Self::Rejection> {
        let query = Query::<HashMap<String, String>>::from_request_parts(parts, state)
            .await
            .map_or_else(|_error| HashMap::new(), |Query(params)| params);
        let mut headers = parts.headers.clone();
        promote_query_credentials(&query, &mut headers);
        let caller = caller_from_headers(&headers, state)
            .await
            .map_err(axum::response::IntoResponse::into_response)?;
        Ok(Self(caller))
    }
}

/// Every request header this boundary reads as a caller credential.
///
/// Kept in step with what [`promote_query_credentials`] resolves its aliases to
/// — and with what [`caller_from_headers`] actually authorizes with — by
/// `credential_headers_cover_every_promoted_alias` in this module's tests, not
/// by this sentence. That test walks [`CREDENTIAL_QUERY_ALIASES`] itself rather
/// than a copy of it, so an alias added to the promotion table and not to this
/// constant fails there. A forwarded request that carried a different notion of
/// "the caller's credentials" than the one this server authorizes with would
/// refuse on the owner for reasons the caller could not see.
const CREDENTIAL_HEADERS: [&str; 4] = [
    "authorization",
    "x-aion-namespaces",
    "x-aion-subject",
    "x-aion-deploy",
];

/// The caller's credentials as outbound gRPC metadata, for a request this node
/// forwards to a shard owner (`api/http/routing.rs`).
///
/// Only [`CREDENTIAL_HEADERS`] are copied. Nothing else is: the forward-hop
/// counter in particular must NOT be inherited from an inbound HTTP request —
/// an HTTP request begins a forward chain rather than continuing one, and the
/// forwarder stamps the hop itself.
///
/// # Errors
///
/// Returns an invalid-input [`WireError`] naming the header — never its value,
/// which is a credential — when a present credential header cannot be
/// represented as gRPC metadata. Dropping it instead would forward an
/// unauthenticated request and surface as an unexplained refusal from the
/// owner.
pub(crate) fn caller_credentials_metadata(
    headers: &HeaderMap,
) -> Result<tonic::metadata::MetadataMap, aion_proto::WireError> {
    let mut metadata = tonic::metadata::MetadataMap::new();
    for name in CREDENTIAL_HEADERS {
        let Some(value) = headers.get(name) else {
            continue;
        };
        let key = tonic::metadata::MetadataKey::from_bytes(name.as_bytes())
            .map_err(|_error| credential_header_rejected(name))?;
        let value = tonic::metadata::MetadataValue::try_from(value.as_bytes())
            .map_err(|_error| credential_header_rejected(name))?;
        metadata.insert(key, value);
    }
    Ok(metadata)
}

fn credential_header_rejected(name: &str) -> aion_proto::WireError {
    aion_proto::WireError::invalid_input(format!(
        "the `{name}` header is not a valid credential value and cannot be carried to the \
         workflow shard's owner"
    ))
}

/// One credential query parameter the WebSocket handshake recognizes.
struct CredentialAlias {
    /// The query-parameter name accepted on the handshake URL.
    param: &'static str,
    /// The request header it promotes to.
    header: &'static str,
    /// Whether the value must be wrapped in the `Bearer` scheme to match the
    /// `Authorization` header form.
    bearer_scheme: bool,
}

/// The promotion table: every credential query parameter, and the header it
/// becomes.
///
/// A TABLE rather than `match` arms because it has to be DATA. Both
/// [`promote_query_credentials`] and
/// `credential_headers_cover_every_promoted_alias` read THIS, so an alias added
/// here is covered by that test by construction. A test that restated the
/// aliases as its own list would have been checking a copy: a ninth alias would
/// have promoted to an unforwarded header with every assertion still green.
///
/// It carries `bearer_scheme` for the same reason — the wrapping rule was a
/// second enumeration of the same three names, and two enumerations of one fact
/// are one drift away from disagreeing.
const CREDENTIAL_QUERY_ALIASES: &[CredentialAlias] = &[
    CredentialAlias {
        param: "x-aion-namespaces",
        header: "x-aion-namespaces",
        bearer_scheme: false,
    },
    CredentialAlias {
        param: "namespaces",
        header: "x-aion-namespaces",
        bearer_scheme: false,
    },
    CredentialAlias {
        param: "x-aion-subject",
        header: "x-aion-subject",
        bearer_scheme: false,
    },
    CredentialAlias {
        param: "subject",
        header: "x-aion-subject",
        bearer_scheme: false,
    },
    CredentialAlias {
        param: "x-aion-deploy",
        header: "x-aion-deploy",
        bearer_scheme: false,
    },
    CredentialAlias {
        param: "authorization",
        header: "authorization",
        bearer_scheme: false,
    },
    CredentialAlias {
        param: "access_token",
        header: "authorization",
        bearer_scheme: true,
    },
    CredentialAlias {
        param: "token",
        header: "authorization",
        bearer_scheme: true,
    },
];

/// Promote recognized credential query parameters into their request-header
/// equivalents so [`caller_from_headers`] resolves the caller identically to a
/// header-bearing REST request. A header already present on the handshake is
/// never overwritten. `access_token` / `token` are wrapped in the `Bearer`
/// scheme to match the `Authorization` header form.
fn promote_query_credentials(params: &HashMap<String, String>, headers: &mut HeaderMap) {
    for (key, value) in params {
        let Some(alias) = CREDENTIAL_QUERY_ALIASES
            .iter()
            .find(|candidate| candidate.param == key.as_str())
        else {
            continue;
        };
        let header_name = alias.header;
        if headers.contains_key(header_name) {
            continue;
        }
        let header_value = if alias.bearer_scheme {
            format!("Bearer {value}")
        } else {
            value.clone()
        };
        let Ok(header_value) = HeaderValue::from_str(&header_value) else {
            continue;
        };
        headers.insert(HeaderName::from_static(header_name), header_value);
    }
}

async fn caller_from_headers(
    headers: &axum::http::HeaderMap,
    state: &ServerState,
) -> Result<CallerIdentity, HttpAuthError> {
    let auth = &state.runtime_config().auth;
    if !auth.enabled {
        return Ok(development_caller_from_headers(headers));
    }
    #[cfg(feature = "auth")]
    {
        let bearer = headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(parse_bearer)
            .ok_or(HttpAuthError)?;
        let Some(cache) = state.jwks_cache() else {
            return Err(HttpAuthError);
        };
        return cache
            .validate(&bearer)
            .await
            .map(|claims| claims.caller_identity())
            .map_err(|_error| HttpAuthError);
    }
    #[cfg(not(feature = "auth"))]
    {
        // Yield to preserve the async signature required by the auth-feature branch.
        tokio::task::yield_now().await;
        Ok(development_token_caller_from_headers(headers, auth))
    }
}

/// Auth-off single-tenant operator mode: when no auth is configured the server
/// decides server-side, at request time, that the caller IS the operator and
/// holds full access (every namespace + the deployment-wide deploy grant). No
/// development header is required for access; the `x-aion-subject` header is
/// honored only as the audit label when present and non-empty.
///
/// The `x-aion-namespaces` / `x-aion-deploy` headers are intentionally NOT read
/// here — the operator already has all access, so they would assert nothing.
fn development_caller_from_headers(headers: &axum::http::HeaderMap) -> CallerIdentity {
    let subject = headers
        .get("x-aion-subject")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .unwrap_or("operator");
    CallerIdentity::operator(subject)
}

/// Deployment-wide deploy grant from the development `x-aion-deploy` header,
/// the dev-mode analog of the JWT `deploy` claim. Absent or non-true = no
/// grant.
#[cfg(not(feature = "auth"))]
fn deploy_header_granted(headers: &axum::http::HeaderMap) -> bool {
    headers
        .get("x-aion-deploy")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("true"))
}

/// Development-mode token authentication used when `auth.enabled` is `true` but
/// the `auth` crate feature is not compiled.  Validates bearer tokens against the
/// configured `jwks_url` value (treated as a static shared secret) and returns
/// [`CallerIdentity::denied`] with a specific reason on each failure mode so the
/// namespace guard surfaces actionable 403 error messages.
#[cfg(not(feature = "auth"))]
fn development_token_caller_from_headers(
    headers: &axum::http::HeaderMap,
    auth: &crate::config::AuthConfig,
) -> CallerIdentity {
    let subject = headers
        .get("x-aion-subject")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty());
    let namespaces = headers
        .get("x-aion-namespaces")
        .and_then(|value| value.to_str().ok())
        .map_or_else(Vec::new, parse_namespaces);

    let bearer_token = auth.jwks_url.as_deref().unwrap_or_default();
    let expected = format!("Bearer {bearer_token}");
    let Some(authorization) = headers.get("authorization") else {
        return CallerIdentity::denied(
            subject.unwrap_or("anonymous"),
            "missing Authorization header with Bearer token; \
             set authorization to `Bearer <token>` for this server",
        );
    };
    let authorization = authorization.to_str().ok();
    if authorization != Some(expected.as_str()) {
        return CallerIdentity::denied(
            subject.unwrap_or("anonymous"),
            "invalid or expired bearer token; \
             refresh the token and send authorization as `Bearer <token>`",
        );
    }

    let Some(subject) = subject else {
        return CallerIdentity::denied(
            "anonymous",
            "missing required header: x-aion-subject; \
             set x-aion-subject to the caller identity",
        );
    };

    CallerIdentity::new(subject, namespaces).with_deploy(deploy_header_granted(headers))
}

#[cfg(feature = "auth")]
fn parse_bearer(value: &str) -> Option<String> {
    let token = value.strip_prefix("Bearer ")?.trim();
    if token.is_empty() {
        return None;
    }
    Some(token.to_owned())
}

struct HttpAuthError;

impl IntoResponse for HttpAuthError {
    fn into_response(self) -> Response {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

#[cfg(not(feature = "auth"))]
fn parse_namespaces(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use aion::EngineBuilder;
    use aion_proto::{ProtoListWorkflowsRequest, WireError, WireErrorCode};
    use aion_store::{EventStore, InMemoryStore};
    #[cfg(not(feature = "auth"))]
    use axum::response::Response;
    use axum::{body, http::HeaderMap, http::Request, http::StatusCode};
    use tower::ServiceExt;

    use super::super::router::workflow_router;
    #[cfg(not(feature = "auth"))]
    use super::super::test_support::TOKEN;
    use super::super::test_support::{NAMESPACE, read_json, runtime_config, server_state};
    use crate::{
        NamespaceResolver, StaticScheduleNamespaces, StaticWorkflowNamespaces,
        config::NamespaceMode,
    };

    async fn list_router() -> Result<axum::Router, Box<dyn std::error::Error>> {
        router_with(runtime_config()).await
    }

    async fn router_with(
        config: crate::config::RuntimeConfig,
    ) -> Result<axum::Router, Box<dyn std::error::Error>> {
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
        Ok(workflow_router(server_state(resolver, config).await?))
    }

    #[tokio::test]
    async fn awl_documents_revisions_and_runs_enforce_auth_on_every_method()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_workspace, router, endpoints) = awl_auth_fixture().await?;

        for (method, uri, value) in &endpoints {
            let missing = router
                .clone()
                .oneshot(awl_request(
                    method,
                    uri,
                    value.as_ref(),
                    AwlCredential::Missing,
                )?)
                .await?;
            assert!(
                matches!(
                    missing.status(),
                    StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
                ),
                "missing credentials reached {method} {uri}: {}",
                missing.status()
            );
            let invalid = router
                .clone()
                .oneshot(awl_request(
                    method,
                    uri,
                    value.as_ref(),
                    AwlCredential::Invalid,
                )?)
                .await?;
            assert!(
                matches!(
                    invalid.status(),
                    StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
                ),
                "invalid credentials reached {method} {uri}: {}",
                invalid.status()
            );
        }

        for (method, uri, value) in &endpoints {
            let valid = router
                .clone()
                .oneshot(awl_request(
                    method,
                    uri,
                    value.as_ref(),
                    AwlCredential::Valid,
                )?)
                .await?;
            assert!(
                valid.status().is_success(),
                "valid credentials failed {method} {uri}: {}",
                valid.status()
            );
        }
        Ok(())
    }

    type AwlEndpoint = (axum::http::Method, String, Option<serde_json::Value>);

    async fn awl_auth_fixture()
    -> Result<(tempfile::TempDir, axum::Router, Vec<AwlEndpoint>), Box<dyn std::error::Error>> {
        use axum::http::Method;
        use serde_json::json;

        let workspace = crate::test_support::private_tempdir()?;
        crate::awl::documents::write(
            workspace.path(),
            "existing.awl",
            crate::awl::documents::PutDocumentRequest {
                source: "workflow existing\n".to_owned(),
            },
        )
        .await?;
        let revision =
            crate::awl::revisions::store(workspace.path(), "workflow existing\n").await?;
        crate::awl::revisions::record_deployment(
            workspace.path(),
            &crate::awl::revisions::DeploymentRecord {
                deployment_id: "auth-deployment".to_owned(),
                document_path: "existing.awl".to_owned(),
                content_hash: revision.content_hash.clone(),
                package_id: "package".to_owned(),
                workflow_type: "existing".to_owned(),
                task_queue: "worker".to_owned(),
                workflow_id: None,
                run_id: None,
            },
        )
        .await?;
        let mut config = runtime_config();
        config.authoring.workspace_dir = Some(workspace.path().to_owned());
        let router = router_with(config).await?;
        let endpoints = vec![
            (Method::GET, "/awl/documents".to_owned(), None),
            (
                Method::POST,
                "/awl/documents".to_owned(),
                Some(json!({ "name": "created" })),
            ),
            (Method::GET, "/awl/documents/existing.awl".to_owned(), None),
            (
                Method::PUT,
                "/awl/documents/existing.awl".to_owned(),
                Some(json!({ "source": "workflow existing\n" })),
            ),
            (
                Method::GET,
                format!("/awl/revisions/{}", revision.content_hash),
                None,
            ),
            (Method::GET, "/awl/runs/auth-deployment".to_owned(), None),
            (
                Method::POST,
                "/awl/runs/auth-deployment/binding".to_owned(),
                Some(json!({ "workflow_id": "workflow-1", "run_id": "run-1" })),
            ),
        ];
        Ok((workspace, router, endpoints))
    }

    #[derive(Clone, Copy)]
    enum AwlCredential {
        Missing,
        Invalid,
        Valid,
    }

    fn awl_request(
        method: &axum::http::Method,
        uri: &str,
        value: Option<&serde_json::Value>,
        credential: AwlCredential,
    ) -> Result<Request<body::Body>, Box<dyn std::error::Error>> {
        let mut builder = Request::builder().method(method).uri(uri);
        if value.is_some() {
            builder = builder.header("content-type", "application/json");
        }
        match credential {
            AwlCredential::Missing => {}
            AwlCredential::Invalid => {
                builder = builder
                    .header("authorization", "Bearer invalid")
                    .header("x-aion-subject", "alice")
                    .header("x-aion-deploy", "true");
            }
            AwlCredential::Valid => {
                #[cfg(feature = "auth")]
                let bearer =
                    crate::auth::test_support::mint_token_with_deploy("alice", NAMESPACE, true)?;
                #[cfg(not(feature = "auth"))]
                let bearer = TOKEN.to_owned();
                builder = builder
                    .header("authorization", format!("Bearer {bearer}"))
                    .header("x-aion-subject", "alice")
                    .header("x-aion-namespaces", NAMESPACE)
                    .header("x-aion-deploy", "true");
            }
        }
        let bytes = match value {
            Some(value) => serde_json::to_vec(value)?,
            None => Vec::new(),
        };
        Ok(builder.body(body::Body::from(bytes))?)
    }

    /// Auth-off single-tenant operator mode: a caller with NO development
    /// headers at all is the operator and is authorized for an arbitrary
    /// namespace (cross-namespace access) AND holds the deploy grant. This is
    /// the request-time, server-side authorization decision the operator
    /// experience depends on — the client asserts nothing.
    #[tokio::test]
    async fn auth_off_operator_authorizes_namespace_and_deploy()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut config = runtime_config();
        config.auth.enabled = false;
        let router = router_with(config).await?;

        // A namespace the caller never enumerated, with no x-aion-* headers.
        let list = ProtoListWorkflowsRequest {
            namespace: "some-other-tenant".to_owned(),
            filter: None,
        };
        let body = serde_json::to_vec(&list)?;
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/workflows/list")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(body::Body::from(body))?,
            )
            .await?;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "auth-off operator must be authorized for any namespace with no headers"
        );

        // And the resolved identity carries the deploy grant.
        let resolved = super::development_caller_from_headers(&HeaderMap::new());
        assert!(resolved.deploy_granted());
        assert!(resolved.all_namespaces());
        assert_eq!(resolved.subject(), "operator");
        Ok(())
    }

    /// The `x-aion-subject` header is honored only as the audit label in
    /// operator mode; it is never required, and never narrows access.
    #[tokio::test]
    async fn auth_off_operator_honors_subject_as_audit_label()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut headers = HeaderMap::new();
        headers.insert("x-aion-subject", "ci-bot".parse()?);
        let resolved = super::development_caller_from_headers(&headers);
        assert_eq!(resolved.subject(), "ci-bot");
        assert!(resolved.all_namespaces());
        assert!(resolved.deploy_granted());
        Ok(())
    }

    /// JWT-path failure modes: missing, malformed, and expired bearers are
    /// redacted 401s (no oracle for why validation failed), while an
    /// authenticated subject lacking the requested grant gets the specific
    /// namespace denial.
    #[cfg(feature = "auth")]
    #[tokio::test]
    async fn http_auth_failure_messages_are_specific() -> Result<(), Box<dyn std::error::Error>> {
        use crate::auth::test_support::{mint_expired_token, mint_token};

        let router = list_router().await?;
        let request = ProtoListWorkflowsRequest {
            namespace: NAMESPACE.to_owned(),
            filter: None,
        };

        let missing = router.clone().oneshot(jwt_request(&request, None)?).await?;
        assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

        let malformed = router
            .clone()
            .oneshot(jwt_request(&request, Some("not-a-jwt".to_owned()))?)
            .await?;
        assert_eq!(malformed.status(), StatusCode::UNAUTHORIZED);

        let expired = router
            .clone()
            .oneshot(jwt_request(
                &request,
                Some(mint_expired_token("alice", NAMESPACE)?),
            )?)
            .await?;
        assert_eq!(expired.status(), StatusCode::UNAUTHORIZED);

        let foreign = router
            .oneshot(jwt_request(
                &request,
                Some(mint_token("alice", "tenant-b")?),
            )?)
            .await?;
        assert_eq!(foreign.status(), StatusCode::FORBIDDEN);
        let error: WireError = read_json(foreign).await?;
        assert_eq!(error.code, WireErrorCode::NamespaceDenied);
        assert!(
            error
                .message
                .contains("subject not authorized for namespace tenant-a"),
            "denial must name the ungranted namespace: {}",
            error.message
        );
        assert!(
            error.message.contains("namespace claim"),
            "JWT-path denial must hint the token's namespace claim: {}",
            error.message
        );
        assert!(
            !error.message.contains("x-aion-namespaces"),
            "JWT-path denial must not hint the development header: {}",
            error.message
        );

        Ok(())
    }

    #[cfg(feature = "auth")]
    fn jwt_request<T>(
        value: &T,
        bearer: Option<String>,
    ) -> Result<Request<body::Body>, Box<dyn std::error::Error>>
    where
        T: serde::Serialize,
    {
        let body = serde_json::to_vec(value)?;
        let mut builder = Request::builder()
            .uri("/workflows/list")
            .method("POST")
            .header("content-type", "application/json");
        if let Some(bearer) = bearer {
            builder = builder.header("authorization", format!("Bearer {bearer}"));
        }
        Ok(builder.body(body::Body::from(body))?)
    }

    /// Development-token-path failure modes: each failure surfaces a specific,
    /// actionable denial message.
    #[cfg(not(feature = "auth"))]
    #[tokio::test]
    async fn http_auth_failure_messages_are_specific() -> Result<(), Box<dyn std::error::Error>> {
        let router = list_router().await?;
        let request = ProtoListWorkflowsRequest {
            namespace: NAMESPACE.to_owned(),
            filter: None,
        };

        assert_auth_error(
            router
                .clone()
                .oneshot(unauthorized_json_request(
                    &request,
                    HeaderCase::MissingAuthorization,
                )?)
                .await?,
            "missing Authorization header with Bearer token",
            "set authorization",
        )
        .await?;
        assert_auth_error(
            router
                .clone()
                .oneshot(unauthorized_json_request(
                    &request,
                    HeaderCase::InvalidToken,
                )?)
                .await?,
            "invalid or expired bearer token",
            "refresh the token",
        )
        .await?;
        assert_auth_error(
            router
                .clone()
                .oneshot(unauthorized_json_request(
                    &request,
                    HeaderCase::MissingSubject,
                )?)
                .await?,
            "missing required header: x-aion-subject",
            "set x-aion-subject",
        )
        .await?;
        assert_auth_error(
            router
                .oneshot(unauthorized_json_request(
                    &request,
                    HeaderCase::WrongNamespace,
                )?)
                .await?,
            "subject not authorized for namespace tenant-a",
            "x-aion-namespaces",
        )
        .await?;

        Ok(())
    }

    #[cfg(not(feature = "auth"))]
    async fn assert_auth_error(
        response: Response,
        expected_phrase: &str,
        expected_hint: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let error: WireError = read_json(response).await?;
        assert_eq!(error.code, WireErrorCode::NamespaceDenied);
        assert!(
            error.message.contains(expected_phrase),
            "message `{}` did not contain `{expected_phrase}`",
            error.message
        );
        assert!(
            error.message.contains(expected_hint),
            "message `{}` did not contain hint `{expected_hint}`",
            error.message
        );
        Ok(())
    }

    #[cfg(not(feature = "auth"))]
    #[derive(Clone, Copy)]
    enum HeaderCase {
        MissingAuthorization,
        InvalidToken,
        MissingSubject,
        WrongNamespace,
    }

    #[cfg(not(feature = "auth"))]
    fn unauthorized_json_request<T>(
        value: &T,
        header_case: HeaderCase,
    ) -> Result<Request<body::Body>, Box<dyn std::error::Error>>
    where
        T: serde::Serialize,
    {
        let body = serde_json::to_vec(value)?;
        let mut builder = Request::builder()
            .uri("/workflows/list")
            .method("POST")
            .header("content-type", "application/json");
        if !matches!(header_case, HeaderCase::MissingAuthorization) {
            let token = match header_case {
                HeaderCase::InvalidToken => "wrong",
                HeaderCase::MissingAuthorization
                | HeaderCase::MissingSubject
                | HeaderCase::WrongNamespace => TOKEN,
            };
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        if !matches!(header_case, HeaderCase::MissingSubject) {
            builder = builder.header("x-aion-subject", "alice");
        }
        let namespace = if matches!(header_case, HeaderCase::WrongNamespace) {
            "tenant-b"
        } else {
            NAMESPACE
        };
        Ok(builder
            .header("x-aion-namespaces", namespace)
            .body(body::Body::from(body))?)
    }

    /// #211 forward path: the credential headers this boundary authorizes with
    /// are exactly the ones carried to a shard owner — and nothing else is.
    ///
    /// The forward-hop counter is the one that matters: inheriting it from an
    /// inbound HTTP request would let a caller start a forward chain at the cap
    /// and be told `NotOwner` for a workflow this node cannot serve.
    #[test]
    fn only_credential_headers_are_carried_to_a_shard_owner()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer token".parse()?);
        headers.insert("x-aion-subject", "alice".parse()?);
        headers.insert("x-aion-namespaces", NAMESPACE.parse()?);
        headers.insert("x-aion-deploy", "true".parse()?);
        headers.insert("content-type", "application/json".parse()?);
        headers.insert(crate::routing::FORWARD_HOPS_METADATA, "2".parse()?);

        let metadata = super::caller_credentials_metadata(&headers)?;

        for name in super::CREDENTIAL_HEADERS {
            assert!(
                metadata.get(name).is_some(),
                "the `{name}` credential must reach the owner, or it authorizes a different caller"
            );
        }
        assert_eq!(metadata.len(), super::CREDENTIAL_HEADERS.len());
        assert!(
            metadata.get("content-type").is_none(),
            "a non-credential header must not be relayed"
        );
        assert_eq!(
            crate::routing::current_hops(&metadata),
            0,
            "an HTTP request STARTS a forward chain; the inbound hop header must not be inherited"
        );
        Ok(())
    }

    /// An absent credential is absent, not empty: nothing is invented for a
    /// caller that sent nothing.
    #[test]
    fn absent_credentials_are_not_invented() -> Result<(), Box<dyn std::error::Error>> {
        let mut headers = HeaderMap::new();
        headers.insert("x-aion-subject", "alice".parse()?);

        let metadata = super::caller_credentials_metadata(&headers)?;

        assert_eq!(metadata.len(), 1);
        assert!(metadata.get("authorization").is_none());
        Ok(())
    }

    /// Every alias the WebSocket handshake promotes resolves to a header in
    /// [`super::CREDENTIAL_HEADERS`] — the mechanism behind that constant's
    /// claim, so a credential added to the promotion table without being added
    /// there fails here instead of silently not being forwarded.
    ///
    /// The aliases are read from [`super::CREDENTIAL_QUERY_ALIASES`], the table
    /// the production path itself resolves through — NOT restated here. A
    /// hand-copied list would have made this an assertion about the copy: a
    /// ninth alias would have promoted to an unforwarded header with this test
    /// still green, which is exactly the failure the constant's doc promises is
    /// impossible.
    #[test]
    fn credential_headers_cover_every_promoted_alias() {
        // Vacuity control: an emptied table would satisfy every loop below.
        assert!(
            !super::CREDENTIAL_QUERY_ALIASES.is_empty(),
            "the promotion table is empty, so this test measures nothing"
        );
        for alias in super::CREDENTIAL_QUERY_ALIASES {
            let params =
                std::collections::HashMap::from([(alias.param.to_owned(), "v".to_owned())]);
            let mut headers = HeaderMap::new();
            super::promote_query_credentials(&params, &mut headers);
            for name in headers.keys() {
                assert!(
                    super::CREDENTIAL_HEADERS.contains(&name.as_str()),
                    "`{}` promotes to `{name}`, which is not carried onto a forward",
                    alias.param
                );
            }
            assert_eq!(
                headers.len(),
                1,
                "`{}` must promote to exactly one header",
                alias.param
            );
            assert!(
                headers.contains_key(alias.header),
                "`{}` must promote to the header its table row declares (`{}`)",
                alias.param,
                alias.header
            );
        }
    }
}
