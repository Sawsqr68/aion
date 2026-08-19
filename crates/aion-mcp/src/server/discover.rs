//! The `server/discover` result.
//!
//! There is no `initialize` handshake in this revision — discovery is a plain
//! request like any other, and a server MUST implement it.

use serde_json::{Map, Value, json};

use crate::protocol::meta::ClientCapabilities;
use crate::protocol::result::{RESULT_TYPE_COMPLETE, result_envelope};
use crate::protocol::version::{SUPPORTED_PROTOCOL_VERSIONS, TASKS_EXTENSION, TASKS_EXTENSION_PIN};

use super::config::McpServerConfig;

/// Build the discover result for one request.
///
/// The tasks extension is advertised **only** to a client that declared it on
/// this request. Two reasons, and both matter: the server may not return a
/// `CreateTaskResult` to a non-declaring client anyway, so advertising it to
/// one would be an offer that cannot be taken; and the extension is an
/// unversioned experimental draft, so a client that has not opted in should not
/// be nudged toward it. The pinned source revision travels in the settings
/// object so a client can see exactly which draft it would be agreeing to.
///
/// The result varies with the caller's declared capabilities, which is
/// precisely why its cache scope is `private`.
#[must_use]
pub fn discover_result(
    config: &McpServerConfig,
    capabilities: &ClientCapabilities,
    instructions: &str,
) -> Value {
    let mut result = result_envelope(RESULT_TYPE_COMPLETE, &config.server_info);
    result.insert(
        "supportedVersions".to_owned(),
        json!(SUPPORTED_PROTOCOL_VERSIONS),
    );
    result.insert(
        "capabilities".to_owned(),
        Value::Object(server_capabilities(capabilities)),
    );
    result.insert("instructions".to_owned(), json!(instructions));
    config.discover_cache.apply(&mut result);
    Value::Object(result)
}

fn server_capabilities(client: &ClientCapabilities) -> Map<String, Value> {
    let mut capabilities = Map::new();
    // `tools: {}` and NOT `tools: { listChanged: true }`: this server serves no
    // `subscriptions/listen`, so a `listChanged` declaration would promise a
    // notification that can never be delivered.
    capabilities.insert("tools".to_owned(), json!({}));
    if client.declares_tasks() {
        capabilities.insert(
            "extensions".to_owned(),
            json!({
                TASKS_EXTENSION: {
                    // The extension defines no settings; the pin is diagnostic
                    // metadata under an unreserved, vendor-prefixed key.
                    "com.ablative.aion/specRevision": TASKS_EXTENSION_PIN,
                }
            }),
        );
    }
    capabilities
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value, json};

    use super::{TASKS_EXTENSION, discover_result};
    use crate::protocol::meta::{ClientCapabilities, Implementation};
    use crate::server::config::McpServerConfig;

    fn config() -> McpServerConfig {
        McpServerConfig::new(
            Implementation::new("aion", "0.12.0"),
            60_000,
            300_000,
            1_000,
            Vec::new(),
        )
    }

    fn capabilities(declares_tasks: bool) -> Result<ClientCapabilities, serde_json::Error> {
        let raw: Map<String, Value> = if declares_tasks {
            serde_json::from_value(json!({ "extensions": { TASKS_EXTENSION: {} } }))?
        } else {
            Map::new()
        };
        Ok(ClientCapabilities::from_object(raw))
    }

    #[test]
    fn the_result_carries_every_required_discover_field() -> Result<(), Box<dyn std::error::Error>>
    {
        let result = discover_result(&config(), &capabilities(true)?, "drive aion like so");
        assert_eq!(result["resultType"], "complete");
        assert_eq!(result["supportedVersions"][0], "2026-07-28");
        assert_eq!(result["capabilities"]["tools"], json!({}));
        assert_eq!(result["ttlMs"], 60_000);
        assert_eq!(result["cacheScope"], "private");
        assert_eq!(
            result["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
            "aion"
        );
        assert_eq!(result["instructions"], "drive aion like so");
        Ok(())
    }

    #[test]
    fn the_tasks_extension_is_advertised_only_to_a_declaring_client()
    -> Result<(), Box<dyn std::error::Error>> {
        let declared = discover_result(&config(), &capabilities(true)?, "");
        assert!(declared["capabilities"]["extensions"][TASKS_EXTENSION].is_object());
        let silent = discover_result(&config(), &capabilities(false)?, "");
        assert!(silent["capabilities"].get("extensions").is_none());
        Ok(())
    }

    #[test]
    fn tools_capability_does_not_claim_list_changed() -> Result<(), Box<dyn std::error::Error>> {
        let result = discover_result(&config(), &capabilities(false)?, "");
        assert!(result["capabilities"]["tools"].get("listChanged").is_none());
        Ok(())
    }
}
