//! `GET /changelog` — what changed in the version this server runs.
//!
//! Serves the changelog embedded in the binary ([`crate::changelog`]), never a
//! file on disk: the answer describes the running BINARY, and a crates.io
//! install has no repository to read from. The console's "What's new" panel is
//! the consumer.
//!
//! Behind [`HttpCaller`] like `/build` and every data route — release notes
//! name the exact code a deployment runs, which is the fact that makes a known
//! vulnerability actionable against it.

use axum::Json;
use serde::Serialize;

use super::auth::HttpCaller;
use crate::changelog::changelog_text;

/// Response body for `GET /changelog`.
#[derive(Debug, Serialize)]
pub(crate) struct ChangelogResponse {
    /// The authored changelog markdown, newest release first, exactly as
    /// embedded at compile time.
    pub markdown: &'static str,
}

/// `GET /changelog`.
pub(crate) async fn changelog(HttpCaller(_caller): HttpCaller) -> Json<ChangelogResponse> {
    Json(ChangelogResponse {
        markdown: changelog_text(),
    })
}
