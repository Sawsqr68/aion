//! Module declarations.

/// JSON-RPC and MCP error codes, and the typed protocol failure.
pub mod error;
/// Streamable-HTTP standard header extraction and validation.
pub mod headers;
/// JSON-RPC 2.0 request/response framing.
pub mod jsonrpc;
/// Per-request `_meta`: protocol version, client capabilities, client info.
pub mod meta;
/// Result envelopes: `resultType` and the `CacheableResult` hints.
pub mod result;
/// Protocol version constants and the extension identifier pins.
pub mod version;
