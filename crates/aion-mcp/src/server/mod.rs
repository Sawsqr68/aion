//! Module declarations.

/// `CallToolResult` construction and output-schema enforcement.
pub mod call_result;
/// Server configuration: identity, cache hints, and task lifetimes.
pub mod config;
/// The `server/discover` result and the agent-facing instructions.
pub mod discover;
/// Method routing over a validated request.
pub mod dispatch;
/// The server handle that owns the service, the task store, and the config.
pub mod handle;
/// The tasks methods: `tasks/get`, `tasks/update`, `tasks/cancel`.
pub mod tasks_methods;
/// `tools/list` and `tools/call`.
pub mod tools_methods;

pub use handle::McpServer;
