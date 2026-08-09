//! The MCP runtime: the server object and its construction from config.

use std::sync::Arc;
use std::time::Duration;

use aion_mcp::protocol::meta::Implementation;
use aion_mcp::{McpServer, McpServerConfig};

use crate::config::ResolvedMcpConfig;
use crate::{ServerError, ServerState};

use super::service::AionToolService;

/// The MCP server plus everything it needs, constructed once at startup.
pub struct McpRuntime {
    server: McpServer<AionToolService>,
}

impl McpRuntime {
    /// Build the runtime from validated configuration.
    ///
    /// The advertised server identity is this binary's own name and version —
    /// self-reported, unverified, and used for nothing but display, exactly as
    /// the revision says such an identity must be.
    ///
    /// # Errors
    ///
    /// [`ServerError::Config`] when a published tool schema fails to compile.
    pub fn build(state: ServerState, config: &ResolvedMcpConfig) -> Result<Self, ServerError> {
        let service =
            AionToolService::new(state, Duration::from_millis(config.await_poll_interval_ms))?;
        let server = McpServer::new(
            Arc::new(service),
            McpServerConfig::new(
                Implementation::new("aion", env!("CARGO_PKG_VERSION")),
                config.discover_ttl_ms,
                config.tools_list_ttl_ms,
                config.task_ttl_ms,
                config.task_poll_interval_ms,
                config.allowed_origins.clone(),
            ),
        );
        Ok(Self { server })
    }

    /// The MCP server.
    pub(crate) const fn server(&self) -> &McpServer<AionToolService> {
        &self.server
    }
}
