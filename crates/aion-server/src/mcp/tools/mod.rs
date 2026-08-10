//! Module declarations.

/// `list_documents`, `read_document`, `check_document`, `save_document`, and
/// `deploy_document`.
pub(crate) mod authoring;
/// `signal`, `query`, and `cancel`.
pub(crate) mod control;
/// `describe_run` and the current-step projection.
pub(crate) mod describe;
/// Wire-error to tool-failure mapping.
pub(crate) mod errors;
/// `read_history`.
pub(crate) mod history;
/// `list_runs`.
pub(crate) mod list;
/// The shared run read: one namespace-gated history read every tool projects
/// from.
pub(crate) mod run_view;
/// `start_run`, including the awaited (task-shaped) variant.
pub(crate) mod start;
/// `read_transcript`.
pub(crate) mod transcript;
