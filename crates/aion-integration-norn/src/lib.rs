//! The first-party **Norn** adapter for Aion — the first concrete [`AgentHarness`] integration.
//!
//! This crate implements the `aion-integrations` [`AgentHarness`] / [`AgentSession`] seam against a
//! real Norn process driven in its `--protocol jsonrpc` mode. It is the analogue of `aion-worker`'s
//! concrete `liminal` adapter: the ONE place in the workspace that names Norn's on-wire contract
//! and translates the neutral intervention primitives onto Norn's native control channel (§3.4).
//!
//! # The invariant this crate exists to preserve
//!
//! The aion platform crates (`aion-core`, `aion-worker`, `aion-server`, …) stay **Norn-blind**:
//! everything Norn-specific — the JSON-RPC method namespace, the `event/*` shapes, the
//! `intervene/*` requests — is confined to [`protocol`] and [`translate`] here, behind the neutral
//! trait. Nothing above this adapter references a Norn type (§3A.4). **Norn itself does not depend
//! on aion**: this adapter maps Norn's documented JSON-RPC contract, so Norn stays a standalone
//! harness.
//!
//! # Shape
//!
//! - [`NornHarness`] implements [`AgentHarness`]: [`AgentHarness::start`] spawns
//!   `norn --protocol jsonrpc`, performs the `initialize` handshake, issues `run/execute`, and
//!   returns a [`NornSession`].
//! - [`NornSession`] implements [`AgentSession`]: [`AgentSession::capabilities`] returns the
//!   parsed `initialize` capabilities, [`AgentSession::events`] streams live translated
//!   [`aion_core::ActivityEvent`]s, [`AgentSession::intervene`] maps a neutral command onto an
//!   `intervene/*` request (capability-gated), and [`AgentSession::wait_result`] returns the
//!   id-matched `run/execute` Response as the terminal [`aion_core::Payload`].
//! - [`registration_capabilities`] publishes the neutral [`InterventionCapabilities`] this adapter
//!   can honour, for a worker that must advertise them when it REGISTERS — before any `norn`
//!   process exists to be asked. It is the adapter's translation ceiling, not a guess about a
//!   binary; the per-session handshake reads what the spawned binary actually advertises, and the
//!   function's own documentation is explicit about how the two can disagree.
//! - [`protocol`] names Norn's wire contract; [`translate`] is the pure, independently tested
//!   translation between that contract and the neutral types.
//!
//! [`InterventionCapabilities`]: aion_core::InterventionCapabilities
//!
//! [`AgentHarness`]: aion_integrations::AgentHarness
//! [`AgentSession`]: aion_integrations::AgentSession
//! [`AgentHarness::start`]: aion_integrations::AgentHarness::start
//! [`AgentSession::capabilities`]: aion_integrations::AgentSession::capabilities
//! [`AgentSession::events`]: aion_integrations::AgentSession::events
//! [`AgentSession::intervene`]: aion_integrations::AgentSession::intervene
//! [`AgentSession::wait_result`]: aion_integrations::AgentSession::wait_result

/// The Norn adapter implementing [`aion_integrations::AgentHarness`].
pub mod harness;
/// Norn's on-wire JSON-RPC contract — the ONLY module naming Norn's method/param strings.
pub mod protocol;
/// The live Norn session implementing [`aion_integrations::AgentSession`].
pub mod session;
/// The pure §3.4 translation between Norn's wire shapes and the neutral `aion-core` types.
pub mod translate;

pub use harness::{NornHarness, registration_capabilities};
pub use session::NornSession;
