//! The Agent Client Protocol wire contract — the ONLY module naming ACP's method strings, its
//! protocol version, and the client capabilities this adapter advertises.
//!
//! Everything ACP-specific about the channel is named here (and consumed by [`crate::translate`],
//! [`crate::harness`], and [`crate::session`]). Nothing above the adapter references these
//! strings; that is the `no-harness-in-platform` invariant the neutral seam exists to preserve.
//!
//! The method names are **not** re-spelled here: they are taken from the published
//! `agent-client-protocol-schema` constants ([`AGENT_METHOD_NAMES`], [`CLIENT_METHOD_NAMES`],
//! [`PROTOCOL_LEVEL_METHOD_NAMES`]), so a protocol rename in a future schema release is a compile
//! error in this file rather than a silent wire mismatch. A hand-written mirror of a moving
//! external protocol is a fossil waiting to happen.
//!
//! # The client capabilities we advertise, and why they are all `false`
//!
//! ACP lets a client offer `fs/read_text_file`, `fs/write_text_file`, and the `terminal/*` family
//! back to the agent. This adapter advertises **none** of them. That is a ruling, not an omission:
//!
//! - The `fs/*` callbacks exist so an **editor** can serve the agent its *unsaved buffers* — the
//!   file as the user currently sees it, which is not what is on disk. An Aion worker has no
//!   buffers and no editor state; the agent runs as a child process with direct filesystem access
//!   under the session `cwd`, so it reads and writes the disk itself. Proxying disk IO back
//!   through the worker would add a hop that changes nothing.
//! - The `terminal/*` family exists so an editor can host the agent's shell in an embedded panel.
//!   A worker has no panel; the agent spawns its own processes.
//! - ACP v2 removes both surfaces in favour of MCP, which confirms the direction. (Checked against
//!   the pinned schema crate's own draft `v2` module: its `ClientMethodNames` lists neither
//!   `fs/read_text_file`, `fs/write_text_file`, nor any `terminal/*` method, and carries
//!   MCP-over-ACP methods in their place.)
//!
//! Advertising a capability we cannot serve would be worse than advertising none: the agent would
//! call it and block. But a *non-advertised* method can still arrive from a non-conforming agent,
//! and an unanswered peer request wedges the run forever — so [`crate::session`] answers every
//! such request with the `-32601` [`method_not_found_code`] and keeps the run alive.
//!
//! [`method_not_found_code`]: crate::protocol::method_not_found_code
//!
//! [`AGENT_METHOD_NAMES`]: agent_client_protocol_schema::v1::AGENT_METHOD_NAMES
//! [`CLIENT_METHOD_NAMES`]: agent_client_protocol_schema::v1::CLIENT_METHOD_NAMES
//! [`PROTOCOL_LEVEL_METHOD_NAMES`]: agent_client_protocol_schema::v1::PROTOCOL_LEVEL_METHOD_NAMES

use agent_client_protocol_schema::ProtocolVersion;
use agent_client_protocol_schema::v1::{
    AGENT_METHOD_NAMES, CLIENT_METHOD_NAMES, ClientCapabilities, ErrorCode, FileSystemCapabilities,
    Implementation, PROTOCOL_LEVEL_METHOD_NAMES,
};
use aion_integrations::jsonrpc::{JsonRpcError, JsonRpcId, JsonRpcResponse};

/// The ACP protocol version this adapter speaks: the integer `1`.
///
/// ACP versions are a `u16` bumped only for breaking changes (non-breaking additions arrive as
/// capabilities) — never a semver string, and unrelated to this crate's own version. The client
/// sends its newest supported version; the agent echoes it when supported, or answers with its own
/// latest. A client that cannot speak the echoed version MUST disconnect, which is what
/// [`gate_protocol_version`] enforces.
///
/// [`gate_protocol_version`]: crate::translate::gate_protocol_version
pub const PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion::V1;

/// The `initialize` method — the versioned capability handshake, sent once before anything else.
pub const METHOD_INITIALIZE: &str = AGENT_METHOD_NAMES.initialize;
/// The `session/new` method — mints the session every later request is scoped to.
pub const METHOD_SESSION_NEW: &str = AGENT_METHOD_NAMES.session_new;
/// The `session/prompt` method — the ONE long-running request whose Response is the terminal
/// result. It stays open for the whole turn.
pub const METHOD_SESSION_PROMPT: &str = AGENT_METHOD_NAMES.session_prompt;
/// The `session/cancel` **notification** (never a request) — the client's cancellation signal.
///
/// The agent must stop, may emit final `session/update`s, and MUST then answer the still-open
/// `session/prompt` with `stopReason: "cancelled"` — never a JSON-RPC error.
pub const METHOD_SESSION_CANCEL: &str = AGENT_METHOD_NAMES.session_cancel;

/// The `session/update` **notification** — the agent's transcript stream.
pub const METHOD_SESSION_UPDATE: &str = CLIENT_METHOD_NAMES.session_update;
/// The `session/request_permission` request — the agent asking the client to authorise a tool call.
///
/// It MUST be answered or the run wedges: see [`crate::permission`].
pub const METHOD_SESSION_REQUEST_PERMISSION: &str = CLIENT_METHOD_NAMES.session_request_permission;
/// The `fs/read_text_file` request — an unadvertised capability this adapter refuses.
pub const METHOD_FS_READ_TEXT_FILE: &str = CLIENT_METHOD_NAMES.fs_read_text_file;
/// The `fs/write_text_file` request — an unadvertised capability this adapter refuses.
pub const METHOD_FS_WRITE_TEXT_FILE: &str = CLIENT_METHOD_NAMES.fs_write_text_file;

/// The `$/cancel_request` notification either side may send to cancel one in-flight request.
///
/// Notifications in the `$/` namespace are implementation-dependent and may be ignored. This
/// adapter answers every inbound peer request synchronously, so it never has one pending to
/// cancel; the notification is recorded on the transcript rather than acted on.
pub const METHOD_CANCEL_REQUEST: &str = PROTOCOL_LEVEL_METHOD_NAMES.cancel_request;

/// The JSON-RPC code answering a method this adapter did not advertise (`-32601`).
///
/// The honest answer to an `fs/*` or `terminal/*` request from an agent we told `false`: the method
/// does not exist on this peer. It is an answer, which is what matters — silence would block the
/// agent forever.
#[must_use]
pub fn method_not_found_code() -> i64 {
    error_code(ErrorCode::MethodNotFound)
}

/// The ACP code (`-32000`) an agent returns to say authentication is required first.
///
/// This is how an agent asks the client to call `authenticate`. An unattended Aion activity cannot
/// run an interactive auth flow, so the adapter surfaces it as a harness failure naming the
/// authentication methods the agent advertised, rather than guessing at credentials.
#[must_use]
pub fn auth_required_code() -> i64 {
    error_code(ErrorCode::AuthRequired)
}

/// The JSON-RPC code (`-32800`) for a request cancelled before it completed.
///
/// A conforming agent answers a cancelled `session/prompt` with `stopReason: "cancelled"` and
/// never with this error, but the code is recognised so a non-conforming agent's cancellation is
/// still reported as a cancellation rather than as an opaque failure.
#[must_use]
pub fn request_cancelled_code() -> i64 {
    error_code(ErrorCode::RequestCancelled)
}

/// Widens an ACP [`ErrorCode`] to the `i64` the JSON-RPC envelope carries.
///
/// The number itself comes from the schema crate's own `ErrorCode -> i32` mapping, so no error
/// code is ever respelled in this workspace.
fn error_code(code: ErrorCode) -> i64 {
    i64::from(i32::from(code))
}

/// The one `-32601` response this adapter uses to refuse a peer request for a capability it does not
/// advertise.
///
/// Built here rather than at each call site so the refusal an agent reads is identical whether the
/// request arrived during the pre-prompt handshake ([`crate::session`]) or mid-turn
/// ([`crate::pump`]) — two spellings of the same refusal would be two things to keep in step.
#[must_use]
pub fn unadvertised_method_response(id: JsonRpcId, method: &str) -> JsonRpcResponse {
    JsonRpcResponse::failure(
        id,
        JsonRpcError::new(
            method_not_found_code(),
            format!(
                "`{method}` is not available: this client advertises no filesystem or terminal \
                 capability"
            ),
        ),
    )
}

/// The `name` this adapter identifies itself by in the `initialize` handshake.
pub const CLIENT_NAME: &str = "aion-integration-acp";

/// The `agent_role` label stamped on every event a session produces.
///
/// ACP has no sub-agent attribution: one session is one agent, and no frame carries a producer
/// identity. The label is therefore a constant rather than a value read off the wire — inventing a
/// per-event role would be fabrication. For the same reason every event carries the nil
/// [`uuid::Uuid`] as its `agent_id`.
pub const AGENT_ROLE: &str = "agent";

/// The prefix every cancellation-derived terminal error message starts with.
///
/// `cancelled` is a distinct terminal outcome, not an ordinary failure: an operator who cancelled
/// a run must be able to tell it apart from a refusal or a token-limit stop. The neutral
/// [`HarnessError`] taxonomy is `#[non_exhaustive]` and owned by `aion-integrations`, so this
/// adapter cannot add a variant for it; the discriminating signal is this stable, documented
/// prefix, which callers may match on.
///
/// [`HarnessError`]: aion_integrations::HarnessError
pub const CANCELLED_DETAIL_PREFIX: &str = "the prompt turn was cancelled";

/// The `clientInfo` this adapter sends in `initialize`.
///
/// The version is this crate's own `CARGO_PKG_VERSION`, which is unrelated to
/// [`PROTOCOL_VERSION`]; agents log it when diagnosing a client, so it must be the real build.
#[must_use]
pub fn client_info() -> Implementation {
    Implementation::new(CLIENT_NAME, env!("CARGO_PKG_VERSION"))
}

/// The `clientCapabilities` this adapter advertises: `fs.readTextFile: false`,
/// `fs.writeTextFile: false`, `terminal: false`.
///
/// See the module docs for why all three are `false`. They are written out explicitly rather than
/// left to [`ClientCapabilities::default`] so the advertisement is a stated decision in the code,
/// not a default that could quietly change under us.
#[must_use]
pub fn client_capabilities() -> ClientCapabilities {
    ClientCapabilities::new()
        .fs(FileSystemCapabilities::new()
            .read_text_file(false)
            .write_text_file(false))
        .terminal(false)
}

#[cfg(test)]
mod tests {
    use super::{
        METHOD_CANCEL_REQUEST, METHOD_FS_READ_TEXT_FILE, METHOD_FS_WRITE_TEXT_FILE,
        METHOD_INITIALIZE, METHOD_SESSION_CANCEL, METHOD_SESSION_NEW, METHOD_SESSION_PROMPT,
        METHOD_SESSION_REQUEST_PERMISSION, METHOD_SESSION_UPDATE, PROTOCOL_VERSION,
        auth_required_code, client_capabilities, client_info, method_not_found_code,
        request_cancelled_code,
    };

    #[test]
    fn protocol_version_is_the_integer_one() {
        // ACP versions are a u16, not a semver string: the wire form of version 1 is `1`.
        assert_eq!(PROTOCOL_VERSION.as_u16(), 1);
    }

    #[test]
    fn method_names_match_the_published_wire_strings() {
        // These assertions pin the SCHEMA constants to the spec's literal method names. If a
        // schema release renames one, this test fails loudly instead of the adapter silently
        // speaking a method no agent implements.
        assert_eq!(METHOD_INITIALIZE, "initialize");
        assert_eq!(METHOD_SESSION_NEW, "session/new");
        assert_eq!(METHOD_SESSION_PROMPT, "session/prompt");
        assert_eq!(METHOD_SESSION_CANCEL, "session/cancel");
        assert_eq!(METHOD_SESSION_UPDATE, "session/update");
        assert_eq!(
            METHOD_SESSION_REQUEST_PERMISSION,
            "session/request_permission"
        );
        assert_eq!(METHOD_FS_READ_TEXT_FILE, "fs/read_text_file");
        assert_eq!(METHOD_FS_WRITE_TEXT_FILE, "fs/write_text_file");
        assert_eq!(METHOD_CANCEL_REQUEST, "$/cancel_request");
    }

    #[test]
    fn error_codes_match_the_specified_numbers() {
        assert_eq!(method_not_found_code(), -32601);
        assert_eq!(auth_required_code(), -32000);
        assert_eq!(request_cancelled_code(), -32800);
    }

    #[test]
    fn advertised_client_capabilities_are_all_false() -> Result<(), serde_json::Error> {
        // The ruling, asserted on the WIRE form the agent actually reads.
        let wire = serde_json::to_value(client_capabilities())?;
        assert_eq!(wire["fs"]["readTextFile"], serde_json::json!(false));
        assert_eq!(wire["fs"]["writeTextFile"], serde_json::json!(false));
        assert_eq!(wire["terminal"], serde_json::json!(false));
        Ok(())
    }

    #[test]
    fn client_info_names_this_crate_and_its_real_version() {
        let info = client_info();
        assert_eq!(info.name, "aion-integration-acp");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    }
}
