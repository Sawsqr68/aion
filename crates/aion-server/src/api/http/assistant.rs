//! `GET /assistant` and `GET /assistant/document` — the built-in assistant.
//!
//! The ops console is served from the binary at `/`; this is the same doctrine
//! for the assistant workflow. `/assistant` answers what assistant this binary
//! carries and whether it is the version the engine would run right now;
//! `/assistant/document` serves the document's own bytes, so an operator with
//! no repository checkout can still stand up the worker that serves its queue.
//!
//! # Authorization
//!
//! Behind the same [`HttpCaller`] extractor as every data route, for the reason
//! `/build` is: the description carries the engine's routing standing for the
//! assistant type, which is deployment state, not public information.
//!
//! # 🔴 Probe the BODY, never the status
//!
//! The ops-console SPA catch-all answers `200 text/html` for any unmatched
//! path, so a binary WITHOUT these routes still answers 200 here. A probe that
//! reads only the status code cannot tell an assistant-carrying build from one
//! without it; read the content type and the body.

use aion_proto::WireError;
use axum::{
    Json,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};

use super::auth::HttpCaller;
use super::error::HttpWireError;
use crate::ServerState;
use crate::assistant::{AssistantDescriptor, describe, embedded_assistant};

/// `GET /assistant`.
pub(crate) async fn assistant_descriptor(
    State(state): State<ServerState>,
    HttpCaller(_caller): HttpCaller,
) -> Result<Json<AssistantDescriptor>, HttpWireError> {
    let embedded = embedded_assistant().map_err(|error| {
        HttpWireError(WireError::backend(format!(
            "this server carries no usable built-in assistant: {error}"
        )))
    })?;
    let engine = state.engine().map_err(|error| {
        HttpWireError(
            WireError::backend(format!(
                "the engine is not available to describe the built-in assistant: {error}"
            ))
            .with_error_type("Engine"),
        )
    })?;
    Ok(Json(describe(embedded, engine.as_ref())))
}

/// `GET /assistant/document` — the embedded AWL source, verbatim.
pub(crate) async fn assistant_document(HttpCaller(_caller): HttpCaller) -> Response {
    match embedded_assistant() {
        // The bytes are served even though the descriptor is derived from the
        // same source: an operator standing up a worker needs the document
        // itself, and it is the artifact every other answer is derived from.
        Ok(embedded) => (
            [(
                header::CONTENT_TYPE,
                axum::http::HeaderValue::from_static("text/plain; charset=utf-8"),
            )],
            embedded.source(),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(WireError::backend(format!(
                "this server carries no usable built-in assistant: {error}"
            ))),
        )
            .into_response(),
    }
}

#[cfg(test)]
#[path = "assistant_tests.rs"]
mod assistant_tests;
