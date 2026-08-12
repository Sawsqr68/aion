//! Module declarations.

/// The authoring tools: `list_documents`, `read_document`, `check_document`,
/// `save_document`, `deploy_document`.
pub(crate) mod authoring;
/// Catalog assembly and the published tool-name constants.
pub(crate) mod build;
/// The mutating tools: `start_run`, `signal`, `cancel`.
pub(crate) mod mutations;
/// The #211 label rule, checked mechanically against the published schemas.
#[cfg(test)]
mod name_is_not_an_address_tests;
/// The read tools: `describe_run`, `read_transcript`, `read_history`,
/// `list_runs`, `query`.
pub(crate) mod reads;
/// Schema fragments shared across tools.
pub(crate) mod shared;

pub(crate) use build::aion_tool_catalog;
