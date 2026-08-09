//! Shared handler layer over Engine.
//!
//! Module layout:
//! - `deploy` — operator deploy (load/list/route/unload) handlers.
//! - `workflows` — start/signal/query/cancel workflow operation handlers.
//! - `visibility` — list/count handlers and namespace filter scoping.
//! - `describe` — describe-workflow handler.
//! - `runs` — run-id resolution and terminal-status reads.
//! - `payload` — required-field and envelope encode/decode helpers.
//! - `managed_workers` — the managed-worker projection and failure mapping
//!   shared by `GET /workers/managed` and `DeployService`.
//! - `error` — engine-error-to-wire-error mapping.

/// Operator deploy handlers shared by both transports.
pub mod deploy;
mod describe;
mod error;
/// The one managed-worker projection and failure mapping both transports use.
pub(crate) mod managed_workers;
mod payload;
mod runs;
mod visibility;
mod workflows;

#[cfg(test)]
mod test_support;

pub use describe::describe;
pub use visibility::{count, list};
pub use workflows::{cancel, pause, query, reopen, resume, signal, start, start_with_placement};
