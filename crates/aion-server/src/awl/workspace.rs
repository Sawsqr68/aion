//! The one derivation of the authoring workspace root.
//!
//! Both surfaces that expose the AWL workspace — the HTTP studio routes and
//! the MCP authoring tools — resolve the root through this function. One
//! derivation in one place is what keeps the two surfaces from ever answering
//! with two different workspaces.

use std::path::PathBuf;

use crate::ServerState;

use super::documents::DocumentError;

/// Resolve the configured authoring workspace root.
///
/// # Errors
///
/// [`DocumentError::WorkspaceUnconfigured`] when the server was started
/// without `authoring.workspace_dir` — a loud refusal, never a guessed path.
pub(crate) fn workspace_root(state: &ServerState) -> Result<PathBuf, DocumentError> {
    state
        .runtime_config()
        .authoring
        .workspace_dir
        .clone()
        .ok_or(DocumentError::WorkspaceUnconfigured)
}
