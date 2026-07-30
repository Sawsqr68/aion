//! The first-party **Agent Client Protocol** (ACP) adapter for Aion.
//!
//! This crate implements the `aion-integrations` [`AgentHarness`] / [`AgentSession`] seam against
//! any agent that speaks [ACP](https://agentclientprotocol.com) over its stdio. Where
//! `aion-integration-norn` adapts one specific harness and `aion-integration-cli` adapts the
//! observability-only tier, this one adapts a **protocol**: a single implementation reaches every
//! agent that has an ACP mode — Claude Code, Codex, Gemini CLI, `OpenCode`, and whatever ships next.
//!
//! # The invariant this crate exists to preserve
//!
//! The aion platform crates (`aion-core`, `aion-worker`, `aion-server`, …) stay ACP-blind:
//! everything ACP-specific — the method namespace, the `session/update` shapes, the permission
//! handshake — is confined to [`protocol`], [`translate`], and [`messages`] here, behind the neutral
//! trait. Nothing above this adapter references an ACP type.
//!
//! # Shape
//!
//! - [`AcpHarness`] implements [`AgentHarness`]: [`AgentHarness::start`] spawns the configured agent
//!   command with piped stdio (stderr inherited), performs the `initialize` handshake gating on ACP
//!   protocol version `1`, creates a session with `session/new`, issues the one long-running
//!   `session/prompt` request carrying the spec's input as a `text` content block, and returns an
//!   [`AcpSession`].
//! - [`AcpSession`] implements [`AgentSession`]: [`AgentSession::events`] streams translated
//!   [`aion_core::ActivityEvent`]s and **ends the moment the turn is answered**,
//!   [`AgentSession::capabilities`] returns the negotiated set, [`AgentSession::intervene`] maps a
//!   neutral command onto real ACP (capability-gated), and [`AgentSession::wait_result`] interprets
//!   the id-matched `session/prompt` Response as the terminal [`aion_core::Payload`] and then shuts
//!   the agent process down.
//! - [`protocol`] names the wire contract, [`translate`] is the pure translation between it and the
//!   neutral types, [`messages`] reassembles the streamed message text, and [`permission`] is the
//!   required policy for answering `session/request_permission`.
//!
//! # Four rulings worth knowing before you use it
//!
//! 1. **`Cancel` is the only intervention primitive advertised.** It is the only one ACP v1 can
//!    actually deliver; the other four have no protocol mechanism, and advertising a control that
//!    silently does nothing would be worse than advertising none. A live session gates on
//!    [`translate::negotiated_capabilities`]; a composition root advertising this adapter at worker
//!    REGISTRATION — before any agent process exists — publishes the same ceiling through
//!    [`registration_capabilities`], which is documented with why the two coincide for ACP and
//!    pinned against what [`translate::intervention_to_notification`] can actually build.
//! 2. **The permission policy is required, with no default.** An unanswered
//!    `session/request_permission` blocks the agent until the activity times out, and an Aion
//!    activity is unattended, so the answer is decided when the harness is built. See
//!    [`permission`].
//! 3. **No filesystem or terminal callbacks are offered.** Those exist for editors with unsaved
//!    buffers and embedded panels; a worker has neither, and the agent has direct disk access. An
//!    inbound `fs/*` or `terminal/*` request is still ANSWERED (with `-32601`) and recorded, because
//!    an unanswered peer request wedges the run. See [`protocol`].
//! 4. **The event stream ending is the end-of-run signal, and it does not wait for the process.**
//!    A real ACP agent is a persistent stdio server: it answers `session/prompt` and keeps running.
//!    The platform's agent driver takes the terminal result only once the event stream yields
//!    `None`, so the adapter closes the transcript at the answer rather than at the child's exit.
//!    See [`pump`].
//!
//! # Two limitations to read before you rely on this
//!
//! - **The session working directory is per-HARNESS, not per-attempt.** Two agent activities running
//!   concurrently through one [`AcpHarness`] share a workspace root and can overwrite each other's
//!   files. Per-attempt workspace provisioning is a separate design; see [`harness`] for the full
//!   statement and why nothing is invented here.
//! - **A completed turn may legitimately carry no text.** An agent that ends `end_turn` having
//!   emitted only tool calls yields a SUCCESSFUL result whose `text` is `""`. That is honest, not a
//!   bug: the agent finished and said nothing, and its work is on the transcript as tool events. See
//!   [`outcome`].
//!
//! # Wire types
//!
//! The ACP request/response/notification types come from the published
//! [`agent-client-protocol-schema`] crate, pinned
//! exactly. That is deliberate: a hand-written mirror of a moving external protocol is a fossil
//! waiting to happen, and a protocol bump should be a compile error rather than a silent wire
//! mismatch. Only the schema is taken — not the full `agent-client-protocol` SDK, which carries its
//! own async stack and would drag a second runtime into this tokio codebase.
//!
//! [`AgentHarness`]: aion_integrations::AgentHarness
//! [`AgentSession`]: aion_integrations::AgentSession
//! [`AgentHarness::start`]: aion_integrations::AgentHarness::start
//! [`AgentSession::capabilities`]: aion_integrations::AgentSession::capabilities
//! [`AgentSession::events`]: aion_integrations::AgentSession::events
//! [`AgentSession::intervene`]: aion_integrations::AgentSession::intervene
//! [`AgentSession::wait_result`]: aion_integrations::AgentSession::wait_result
//! [`agent-client-protocol-schema`]: https://docs.rs/agent-client-protocol-schema
//!
//! [`AcpHarness`]: crate::harness::AcpHarness
//! [`AcpSession`]: crate::session::AcpSession

/// The ACP adapter implementing [`aion_integrations::AgentHarness`].
pub mod harness;
/// Reassembly of ACP's per-chunk message stream into durable messages and the run's final text.
pub mod messages;
/// Interpretation of the terminal `session/prompt` Response as the activity's result.
pub mod outcome;
/// The required policy for answering `session/request_permission`.
pub mod permission;
/// ACP's on-wire contract — the ONLY module naming its method strings and protocol version.
pub mod protocol;
/// The reader pump that handles every inbound frame for a live session.
pub mod pump;
/// The live ACP session implementing [`aion_integrations::AgentSession`].
pub mod session;
/// The pure translation between ACP's wire shapes and the neutral `aion-core` types.
pub mod translate;

// Protocol tests driving a real `JsonRpcConnection` against a scripted fake ACP agent over an
// in-memory duplex. In-crate rather than under `tests/` because they exercise the adapter's own
// transport-independent seams (`AcpHarness::connect`, `AcpSession::start`) instead of forcing those
// internals into the published API just so a test could reach them.
#[cfg(test)]
mod wire_tests;

pub use harness::{AcpHarness, registration_capabilities};
pub use permission::PermissionPolicy;
pub use session::AcpSession;
