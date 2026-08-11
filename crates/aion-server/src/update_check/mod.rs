//! The built-in update check: "does a newer `aion-cli` exist?" — slice one,
//! know and show.
//!
//! Four parts, one contract:
//!
//! - [`document`] — the embedded AWL document whose declared `curl` command
//!   the server executes through the declared-body spine, with its output on
//!   the transcript. The binary's default build keeps ZERO outbound-HTTP
//!   dependencies; the transfer is runtime `curl` in a declared command.
//! - [`install`] — boot-time installation of that document into the engine
//!   catalog, claiming only a catalog that holds no version of it. Installing
//!   makes the check STARTABLE; it never starts one.
//! - `observer` + `index`/`semver` (crate-internal) — the server half: when
//!   the check's own dispatch completes, parse the fetched sparse-index lines
//!   (skipping yanked releases and prereleases — the versions a plain
//!   `cargo install` refuses) and record the greatest installable version.
//! - `status` (crate-internal) — the recorded last check, served by
//!   `GET /update-status` beside the installed version from
//!   [`crate::build_identity`].
//!
//! # Manual-only is the design, not a default
//!
//! There is no cadence configuration key, no timer, no check-on-startup.
//! Every check is one explicit operator act — the ops console's "check now"
//! or a plain workflow start. And knowing is ALL this slice does: no install,
//! no upgrade, and no restart machinery exists here.
//!
//! # Visibility: only the document and its install are API
//!
//! The parser, the SemVer implementation, the observer, and the status slot
//! are implementation detail behind `GET /update-status`, and publishing them
//! would make a hand-rolled SemVer part of `aion-server`'s public API — a
//! surface that could never be retired. They are `pub(crate)`; the embedded
//! document and its boot install are the exported face (the CLI and the e2e
//! suites drive the feature through them, exactly as with the assistant).

/// The embedded AWL document, its compiled identity, and the verified names
/// the server half keys on.
pub mod document;
/// The crates.io sparse-index line parse (crate-internal).
pub(crate) mod index;
/// Boot-time installation of the embedded document into the engine catalog.
pub mod install;
/// The dispatch decorator that records completed checks (crate-internal).
pub(crate) mod observer;
/// Strict SemVer 2.0.0 parsing and precedence (crate-internal).
pub(crate) mod semver;
/// The server's memory of its last completed check (crate-internal).
pub(crate) mod status;

pub use document::{
    EMBEDDED_UPDATE_CHECK_DOCUMENT, EMBEDDED_UPDATE_CHECK_FILENAME, EmbeddedUpdateCheck,
    EmbeddedUpdateCheckError, FETCH_ACTION, FETCH_COMMAND, UPDATE_CHECK_QUEUE,
    UPDATE_CHECK_WORKFLOW_TYPE, embedded_update_check,
};
pub use install::{
    UpdateCheckInstall, install_embedded_update_check, install_embedded_update_check_for_server,
};
pub(crate) use observer::UpdateCheckObserver;
pub(crate) use status::UpdateStatusState;
