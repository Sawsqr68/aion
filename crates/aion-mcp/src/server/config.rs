//! Server configuration.
//!
//! Every value here is supplied by the host. Nothing in this crate invents a
//! timeout or a cache lifetime: the host owns those, because the host is what
//! knows how fast its work moves.
//!
//! There is no task lifetime here, because there is nothing to expire. A task
//! is a projection of durable work the host already stores; it holds no
//! executor and no memory, so no number bounds it and none has to be guessed.

use crate::protocol::meta::Implementation;
use crate::protocol::result::{CacheHints, CacheScope};

/// Configuration for one MCP server instance.
#[derive(Clone, Debug)]
pub struct McpServerConfig {
    /// The server's self-reported identity, echoed in every result's `_meta`.
    pub server_info: Implementation,
    /// Cache hints for `server/discover`.
    pub discover_cache: CacheHints,
    /// Cache hints for `tools/list`.
    pub tools_list_cache: CacheHints,
    /// The poll interval a client is asked to honour for a created task.
    ///
    /// A hint, and only a hint: no server behaviour depends on it, and a client
    /// that ignores it is not doing anything the server has to defend against.
    pub task_poll_interval_ms: u64,
    /// Origins permitted on the `Origin` header of an MCP request.
    ///
    /// A request whose `Origin` is present and not listed here is refused with
    /// HTTP 403 — the DNS-rebinding defence. An EMPTY list means no
    /// cross-origin request is accepted at all: a browser-originated request
    /// always carries `Origin`, so the empty list refuses every one of them
    /// while leaving header-less programmatic clients unaffected. That is the
    /// secure default and it is a real policy, not an unset value.
    pub allowed_origins: Vec<String>,
}

impl McpServerConfig {
    /// Build a configuration with the two cache scopes fixed to `private`.
    ///
    /// Both cacheable results this server returns vary with the caller: the
    /// discover result advertises the tasks extension only to a client that
    /// declared it, and the tool set is served under the caller's own
    /// authorization. `private` is the only scope that cannot leak one
    /// caller's answer to another — and `cacheScope` is a sharing hint, never
    /// an access control, so the per-request authorization still runs.
    #[must_use]
    pub fn new(
        server_info: Implementation,
        discover_ttl_ms: u64,
        tools_list_ttl_ms: u64,
        task_poll_interval_ms: u64,
        allowed_origins: Vec<String>,
    ) -> Self {
        Self {
            server_info,
            discover_cache: CacheHints::new(discover_ttl_ms, CacheScope::Private),
            tools_list_cache: CacheHints::new(tools_list_ttl_ms, CacheScope::Private),
            task_poll_interval_ms,
            allowed_origins,
        }
    }

    /// Whether `origin` may issue an MCP request.
    #[must_use]
    pub fn origin_allowed(&self, origin: &str) -> bool {
        self.allowed_origins.iter().any(|allowed| allowed == origin)
    }
}

#[cfg(test)]
mod tests {
    use super::{CacheScope, Implementation, McpServerConfig};

    fn config(origins: Vec<String>) -> McpServerConfig {
        McpServerConfig::new(
            Implementation::new("aion", "0.12.0"),
            60_000,
            300_000,
            1_000,
            origins,
        )
    }

    #[test]
    fn both_cacheable_results_are_private() {
        let config = config(Vec::new());
        assert_eq!(config.discover_cache.scope, CacheScope::Private);
        assert_eq!(config.tools_list_cache.scope, CacheScope::Private);
        assert_eq!(config.tools_list_cache.ttl_ms, 300_000);
    }

    #[test]
    fn an_empty_origin_list_allows_nothing() {
        assert!(!config(Vec::new()).origin_allowed("http://localhost:8080"));
        let permissive = config(vec!["http://localhost:8080".to_owned()]);
        assert!(permissive.origin_allowed("http://localhost:8080"));
        assert!(!permissive.origin_allowed("http://evil.example"));
    }
}
