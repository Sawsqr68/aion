//! The axum mount.
//!
//! One route on the server's EXISTING listener — no second port, no second
//! process. Authentication is the server's own: the caller is resolved by the
//! same [`HttpCaller`] extractor every other route uses, and is handed to the
//! tools unchanged. There is no MCP-specific auth here to drift from it.

use std::sync::Arc;

use axum::{
    Router,
    body::{Body, Bytes},
    http::{HeaderMap, Method, StatusCode, header::HeaderName},
    response::{IntoResponse, Response},
    routing::any,
};

use crate::ServerState;
use crate::api::http::auth::HttpCaller;

use super::runtime::McpRuntime;

/// The single MCP endpoint path.
///
/// A single path supporting POST is what the transport requires; GET and DELETE
/// on it are answered `405` because the standalone SSE stream and the
/// session-termination verb were both removed in this revision.
pub(crate) const MCP_PATH: &str = "/mcp";

/// Build the MCP route family over an already-constructed runtime.
pub(crate) fn mcp_router(runtime: Arc<McpRuntime>) -> Router<ServerState> {
    Router::new()
        .route(MCP_PATH, any(handle))
        .layer(axum::Extension(runtime))
}

/// A dark MCP surface: a plain 404 with no body for the endpoint, matching how
/// every other optional route family reports being unmounted.
pub(crate) fn mcp_disabled_router() -> Router<ServerState> {
    Router::new().route(MCP_PATH, any(|| async { StatusCode::NOT_FOUND }))
}

async fn handle(
    axum::Extension(runtime): axum::Extension<Arc<McpRuntime>>,
    HttpCaller(caller): HttpCaller,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request = aion_mcp::HttpRequest::new(
        method.as_str().to_ascii_uppercase(),
        aion_mcp::protocol::headers::HeaderView::new(header_pairs(&headers)),
        body.to_vec(),
    );
    let response = runtime.server().handle_http(&request, &caller).await;
    into_axum(response)
}

/// Header values that are not valid UTF-8 are dropped rather than lossily
/// decoded: a header the server cannot read exactly is a header it cannot prove
/// agreement with, and a lossy decode would turn "unreadable" into "mismatched"
/// or, worse, into a coincidental match.
fn header_pairs(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_owned(), value.to_owned()))
        })
        .collect()
}

fn into_axum(response: aion_mcp::HttpResponse) -> Response {
    let status = StatusCode::from_u16(response.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut builder = Response::builder().status(status);
    for (name, value) in response.headers {
        match HeaderName::from_bytes(name.as_bytes()) {
            Ok(name) => builder = builder.header(name, value),
            Err(error) => {
                tracing::error!(
                    target: "aion_server::mcp",
                    header = %name,
                    "an MCP response header name was not valid and was dropped: {error}"
                );
            }
        }
    }
    builder
        .body(Body::from(response.body))
        .unwrap_or_else(|error| {
            tracing::error!(
                target: "aion_server::mcp",
                "an MCP response could not be assembled: {error}"
            );
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        })
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue, header::HeaderName};

    use super::header_pairs;

    #[test]
    fn a_non_utf8_header_value_is_dropped_not_lossily_decoded()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut headers = HeaderMap::new();
        drop(headers.insert(
            HeaderName::from_static("mcp-method"),
            HeaderValue::from_static("tools/list"),
        ));
        drop(headers.insert(
            HeaderName::from_static("mcp-name"),
            HeaderValue::from_bytes(&[0xff, 0xfe])?,
        ));
        let pairs = header_pairs(&headers);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, "mcp-method");
        Ok(())
    }
}
