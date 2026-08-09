//! Protocol version constants and extension identifier pins.

/// The single protocol revision this server implements.
///
/// `LATEST_PROTOCOL_VERSION` in `schema/2026-07-28/schema.ts` at core-repo HEAD
/// `9d4a9115126f1356f4b189af3266c1839a4e9bbb`.
pub const PROTOCOL_VERSION: &str = "2026-07-28";

/// Every protocol version this server accepts, newest first.
///
/// Exactly one entry: the handshake-era revisions (`2025-03-26` …
/// `2025-11-25`) are a deliberately deferred compatibility shim, not a silent
/// omission — a request naming one is refused with
/// [`UNSUPPORTED_PROTOCOL_VERSION`](super::error::error_codes::UNSUPPORTED_PROTOCOL_VERSION)
/// and this list, never mis-served as if it were understood.
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[PROTOCOL_VERSION];

/// The tasks extension identifier, used both as the `capabilities.extensions`
/// key and as the `clientCapabilities.extensions` key a client declares.
pub const TASKS_EXTENSION: &str = "io.modelcontextprotocol/tasks";

/// The exact source revision of the tasks extension this server implements.
///
/// `modelcontextprotocol/ext-tasks` has no git tags and its
/// `specification/draft/tasks.md` carries no version number, so a commit SHA is
/// the only identity that can be pinned. Surfaced in `server/discover`'s
/// instructions and in the extension's advertised settings object so a client
/// can see precisely which draft it is talking to.
pub const TASKS_EXTENSION_PIN: &str = "2c1425d9a288b9b1f489430fe1e00bb392b47e48";

/// The `_meta` key carrying the request's protocol version.
pub const META_PROTOCOL_VERSION: &str = "io.modelcontextprotocol/protocolVersion";

/// The `_meta` key carrying the request's client capabilities.
pub const META_CLIENT_CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";

/// The `_meta` key carrying the (optional) client implementation identity.
pub const META_CLIENT_INFO: &str = "io.modelcontextprotocol/clientInfo";

/// The `_meta` key carrying the server implementation identity on every result.
pub const META_SERVER_INFO: &str = "io.modelcontextprotocol/serverInfo";

/// Whether `version` is a revision this server implements.
#[must_use]
pub fn is_supported(version: &str) -> bool {
    SUPPORTED_PROTOCOL_VERSIONS.contains(&version)
}

#[cfg(test)]
mod tests {
    use super::{PROTOCOL_VERSION, SUPPORTED_PROTOCOL_VERSIONS, is_supported};

    #[test]
    fn the_implemented_revision_is_the_only_supported_one() {
        assert!(is_supported(PROTOCOL_VERSION));
        assert!(!is_supported("2025-11-25"));
        assert!(!is_supported("1900-01-01"));
        assert_eq!(SUPPORTED_PROTOCOL_VERSIONS, &[PROTOCOL_VERSION]);
    }
}
