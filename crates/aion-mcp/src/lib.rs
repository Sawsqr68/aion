//! Model Context Protocol server protocol layer, built to the **2026-07-28**
//! stateless revision plus the `io.modelcontextprotocol/tasks` draft extension.
//!
//! This crate is transport-agnostic and knows nothing about Aion. It owns the
//! JSON-RPC framing, the per-request `_meta` contract, the Streamable-HTTP
//! header rules, version negotiation, the tool descriptor/​catalog types, the
//! tasks extension state machine, and the method dispatcher. A host wires it up
//! by implementing [`ToolService`] and calling [`McpServer::handle_http`] from
//! whatever HTTP surface it already runs.
//!
//! # Specification pins
//!
//! * Core: `modelcontextprotocol/modelcontextprotocol`, `schema/2026-07-28/schema.ts`
//!   at repository HEAD `9d4a9115126f1356f4b189af3266c1839a4e9bbb`
//!   (`LATEST_PROTOCOL_VERSION = "2026-07-28"`).
//! * Tasks extension: `modelcontextprotocol/ext-tasks`,
//!   `specification/draft/tasks.md`, read at repository HEAD
//!   `2c1425d9a288b9b1f489430fe1e00bb392b47e48`. The extension has **no git
//!   tags and no version number** — it self-describes as experimental — so the
//!   commit SHA above is the only pinnable identity, and it is exported as
//!   [`TASKS_EXTENSION_PIN`].
//!
//! # Deliberate non-implementations
//!
//! * **No progress notifications.** `progressToken` is accepted and ignored,
//!   which the progress pattern explicitly permits ("the server MAY ignore it
//!   entirely"). Every request is therefore answered with a single
//!   `application/json` body and never `text/event-stream`, which in turn makes
//!   the SSE-stream obligations (keep-alives, stream-closure-as-cancellation,
//!   final-response termination) vacuous rather than half-built.
//! * **No `subscriptions/listen`.** The server declares no `listChanged`
//!   capability and pushes no task notifications, so the method is answered
//!   with `404` + `-32601` like any other unimplemented method.
//! * **No `Mcp-Session-Id`, no `Last-Event-ID`.** Both were removed in this
//!   revision; the server neither mints, echoes, nor reads them.

/// Streamable-HTTP request/response adaptation: the plain-data HTTP shapes the
/// dispatcher consumes and produces.
pub mod http;
/// Wire-level protocol types: JSON-RPC framing, `_meta`, headers, errors,
/// versioning, and the cacheable-result hints.
pub mod protocol;
/// The dispatcher: method routing, `server/discover`, `tools/call`, and the
/// tasks methods.
pub mod server;
/// The tasks extension (`io.modelcontextprotocol/tasks`): task model, store,
/// and the cooperative execution driver.
pub mod tasks;
/// Tool descriptors, annotations, catalog, and the host-implemented service
/// trait.
pub mod tools;

pub use http::{HttpRequest, HttpResponse};
pub use protocol::error::{ProtocolError, error_codes};
pub use protocol::jsonrpc::{JsonRpcError, JsonRpcResponse, RequestId};
pub use protocol::meta::{ClientCapabilities, Implementation, RequestMeta};
pub use protocol::result::CacheHints;
pub use protocol::version::{
    PROTOCOL_VERSION, SUPPORTED_PROTOCOL_VERSIONS, TASKS_EXTENSION, TASKS_EXTENSION_PIN,
};
pub use server::McpServer;
pub use server::config::McpServerConfig;
pub use tasks::model::{DetailedTask, Task, TaskStatus};
pub use tasks::store::TaskStore;
pub use tools::descriptor::{Tool, ToolAnnotations};
pub use tools::service::{
    ToolCall, ToolCatalog, ToolFailure, ToolOutcome, ToolService, ToolTaskDecision,
};
