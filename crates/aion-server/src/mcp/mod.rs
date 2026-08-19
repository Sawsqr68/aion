//! Module declarations.

/// Argument extraction shared by the tools.
pub(crate) mod args;
/// The published tool catalog.
pub(crate) mod catalog;
/// The `server/discover` instructions.
pub(crate) mod instructions;
/// The axum mount.
pub(crate) mod route;
/// Run-scoping for transcript handles.
pub(crate) mod run_scope;
/// The MCP runtime constructed at startup.
pub mod runtime;
/// The Aion tool service.
pub mod service;
/// The MCP task handle and the run projection behind it.
pub(crate) mod task_ref;
/// The tool implementations.
pub(crate) mod tools;

pub(crate) use route::{mcp_disabled_router, mcp_router};
pub use runtime::McpRuntime;
pub use service::AionToolService;
