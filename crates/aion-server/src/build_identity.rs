//! What code is this server (#123).
//!
//! # The gap this closes
//!
//! A running server could not be asked what code it was. The only build
//! identity anywhere was a startup log line carrying `CARGO_PKG_VERSION` — a
//! crate version, which cannot distinguish two builds from different commits of
//! the same version, and which is gone from the terminal by the time anyone
//! needs it.
//!
//! That was measured, not supposed. On 2026-07-31 a live server's running image
//! was found to differ from every preserved copy of "the same" binary — a
//! different inode and a different size — and there was no way to establish
//! which revision was actually serving. The mistake available at that moment
//! was to restart it and call the result a restoration. **An artefact's
//! identity is its content, never its path**; a path is a label recording where
//! a file used to be. This is the one identity a server can state about itself.
//!
//! # 🔴 A STATUS CODE CANNOT VERIFY THIS ENDPOINT EXISTS
//!
//! The server mounts an ops-console SPA whose catch-all serves the app shell for
//! any unmatched path, so that client-side routing works. A consequence, found
//! the hard way on a live probe: **`GET /version` returns `200` on a server that
//! has no such route** — the fallback answered, with `text/html`.
//!
//! So a probe that checks only the status code would report this endpoint
//! present on every image ever built, including the ones it exists to
//! distinguish. **An instrument that cannot fail is not an instrument.**
//!
//! Any probe for build identity must therefore assert on the BODY:
//!
//! ```text
//! curl -s http://host/build | jq -e .commit     # fails on an image without it
//! ```
//!
//! not on `%{http_code}`. [`BuildIdentity`] is `application/json` with a
//! required `commit` field precisely so that check is available and cheap. The
//! same discipline as grepping an artifact for its own verdict rather than
//! trusting a summarised exit.

use serde::Serialize;

/// The stamped absence, and the one word every unestablished field carries.
///
/// A word rather than an empty string or a `null`: an empty value reads as a
/// formatting bug at the far end, where "unknown" reads as the measurement it
/// is. Nothing here is ever defaulted to something that looks like an answer.
const UNKNOWN: &str = "unknown";

/// The source revision this binary was built from, stamped by `build.rs`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BuildIdentity {
    /// Crate version. Coarse on its own — retained because it is the version an
    /// operator matches against a release, a changelog, and a crates.io page.
    pub version: &'static str,
    /// Full commit hash of the source tree, or `"unknown"` for a build with no
    /// repository (a crates.io tarball, a vendored copy, an export). Its rerun
    /// triggers are registered in the build script, so unlike [`Self::dirty`]
    /// it carries no staleness caveat.
    pub commit: &'static str,
    /// Whether uncommitted changes were observed when the build script last
    /// ran, or `"unknown"` when it could not be established.
    ///
    /// **`"true"` is trustworthy. `"false"` means "no uncommitted change was
    /// observed", NOT "the tree was provably clean."** Cargo cannot watch a
    /// working tree, so a tree dirtied in a different workspace crate after
    /// this script ran is reported clean. Stated here rather than left for a
    /// reader to assume away — a limitation named is worth more than a boolean
    /// that is silently wrong in one direction.
    pub dirty: &'static str,
    /// When the build script ran, RFC 3339, or `"unknown"`. Honours
    /// `SOURCE_DATE_EPOCH`, so a reproducible build gets a reproducible stamp
    /// rather than this being the one field that defeats reproducibility.
    pub built_at: String,
}

impl BuildIdentity {
    /// The identity compiled into this binary.
    #[must_use]
    pub fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            commit: env!("AION_BUILD_COMMIT"),
            dirty: env!("AION_BUILD_DIRTY"),
            built_at: built_at(),
        }
    }

    /// The one-line form for the startup banner, so the identity is also in the
    /// log a crashed server leaves behind — not only on an endpoint that needs
    /// the server alive to answer.
    #[must_use]
    pub fn line(&self) -> String {
        let dirty = if self.dirty == "true" { "-dirty" } else { "" };
        format!(
            "{}+{}{} built {}",
            self.version, self.commit, dirty, self.built_at
        )
    }
}

/// The stamped epoch rendered as RFC 3339, or `"unknown"`.
///
/// The build script carries an integer across the boundary rather than a
/// formatted string, so it needs no date dependency of its own; this is where
/// that integer becomes readable. A value that does not parse, or does not
/// name a real instant, degrades to the same honest absence as never having
/// been established — the three are indistinguishable to an operator and
/// pretending otherwise would invent precision.
fn built_at() -> String {
    let Ok(seconds) = env!("AION_BUILD_EPOCH").parse::<i64>() else {
        return UNKNOWN.to_owned();
    };
    chrono::DateTime::from_timestamp(seconds, 0)
        .map_or_else(|| UNKNOWN.to_owned(), |stamp| stamp.to_rfc3339())
}

#[cfg(test)]
#[path = "build_identity_tests.rs"]
mod tests;
