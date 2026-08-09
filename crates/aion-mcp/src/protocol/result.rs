//! Result envelopes.
//!
//! Every result carries `resultType`. Only the three cacheable operations carry
//! `ttlMs`/`cacheScope`; a `tools/call` result carries neither, because
//! `CallToolResult` is not a `CacheableResult` in this revision.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::meta::Implementation;
use super::version::META_SERVER_INFO;

/// `resultType: "complete"` — a finished result.
pub const RESULT_TYPE_COMPLETE: &str = "complete";
/// `resultType: "task"` — a `CreateTaskResult`, introduced by the tasks
/// extension. Never set on any other result shape.
pub const RESULT_TYPE_TASK: &str = "task";

/// Cache freshness hints for a `CacheableResult`.
///
/// `ttl_ms` is milliseconds and MUST be `>= 0` — the type enforces that by
/// construction. `scope` is `"public"` or `"private"`; the enum makes the third
/// value unrepresentable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheHints {
    /// Freshness lifetime in milliseconds.
    pub ttl_ms: u64,
    /// Sharing scope.
    pub scope: CacheScope,
}

/// Whether a cached result may be shared across authorization contexts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheScope {
    /// Carries no user-specific data; any cache may serve it to any user.
    Public,
    /// MUST NOT be shared across authorization contexts.
    Private,
}

impl CacheScope {
    /// The wire spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }
}

impl CacheHints {
    /// Build hints.
    #[must_use]
    pub const fn new(ttl_ms: u64, scope: CacheScope) -> Self {
        Self { ttl_ms, scope }
    }

    /// Write `ttlMs` and `cacheScope` into a result object.
    pub fn apply(self, result: &mut Map<String, Value>) {
        result.insert("ttlMs".to_owned(), json!(self.ttl_ms));
        result.insert("cacheScope".to_owned(), json!(self.scope.as_str()));
    }
}

/// Start a result object with its `resultType` and the server's identity in
/// `_meta`.
///
/// `serverInfo` lives INSIDE `_meta` in this revision — it is not a top-level
/// result field as it was on the handshake-era `InitializeResult`.
#[must_use]
pub fn result_envelope(result_type: &str, server_info: &Implementation) -> Map<String, Value> {
    let mut result = Map::new();
    result.insert("resultType".to_owned(), json!(result_type));
    let mut meta = Map::new();
    meta.insert(META_SERVER_INFO.to_owned(), json!(server_info));
    result.insert("_meta".to_owned(), Value::Object(meta));
    result
}

#[cfg(test)]
mod tests {
    use super::{CacheHints, CacheScope, Implementation, RESULT_TYPE_COMPLETE, result_envelope};

    #[test]
    fn the_envelope_places_server_info_inside_meta() {
        let envelope =
            result_envelope(RESULT_TYPE_COMPLETE, &Implementation::new("aion", "0.12.0"));
        assert_eq!(envelope["resultType"], "complete");
        assert_eq!(
            envelope["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
            "aion"
        );
        assert!(envelope.get("serverInfo").is_none());
    }

    #[test]
    fn cache_hints_write_both_fields() {
        let mut envelope =
            result_envelope(RESULT_TYPE_COMPLETE, &Implementation::new("aion", "0.12.0"));
        CacheHints::new(300_000, CacheScope::Private).apply(&mut envelope);
        assert_eq!(envelope["ttlMs"], 300_000);
        assert_eq!(envelope["cacheScope"], "private");
    }
}
