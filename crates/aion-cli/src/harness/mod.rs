//! The agent-harness composition root (NOI-4b, §3A.3).
//!
//! This is the ONE place in the `aion` binary that names a concrete
//! [`AgentHarness`] adapter. Behind the default-on `norn` and `acp` cargo features
//! it composes the first-party `NornHarness` (`aion-integration-norn`) and
//! `AcpHarness` (`aion-integration-acp`) in, so the shipped default binary drives
//! Norn agents AND any agent that speaks the Agent Client Protocol OUT-OF-BOX. The
//! platform library crates never name an adapter — the worker drives ANY
//! `AgentHarness` through the neutral trait (`aion_worker::spawn_agent`); this root
//! only injects which concrete adapters are compiled in, decides which one an
//! invocation serves, and configures it.
//!
//! Mirrors the proven `liminal-transport` split exactly: each concrete adapter is an
//! OPTIONAL dependency, ON in the binary's `default = [...]`, OFF under
//! `--no-default-features` — at which point the binary carries no adapter code and
//! the `no-norn-in-platform` gate proves the library crates never did.
//!
//! # Why selection, and why per-harness configuration
//!
//! With one adapter compiled in, harness identity could be a compile-time constant.
//! With two it cannot: an operator has to say which one, and an ACP agent needs
//! configuration that CANNOT be inferred — it is a subprocess whose launch command is
//! not derivable from anything (see `aion_integration_acp::AcpHarness`' own module
//! documentation for the table of mutually incompatible real-world invocations).
//!
//! The adapter type names above are code spans rather than intra-doc links on purpose:
//! both crates are OPTIONAL dependencies, and a link to one would not resolve in the
//! `--no-default-features` build this module exists to keep working.
//!
//! So the root has three parts:
//!
//! * [`catalogue`] — which harnesses this build carries, and the check of the
//!   document's declared `kind` against them. A name this build does not carry is
//!   refused naming what it does; a build carrying none resolves to `None`, which is
//!   not an error until something tries to serve.
//! * [`settings`] — one harness's configuration, built from the `harness` section of
//!   the document being served. No flag, no config file and no ambient variable
//!   contributes a setting; the only thing read from the launching context is the
//!   VALUE of each variable the document named in `env_pass`.
//! * `compose` — the only code that constructs an adapter, erased behind
//!   `Arc<dyn DynAgentHarness>` for the worker serve path to hold.
//!
//! Naming a concrete adapter's settings HERE rather than in `aion worker agent` is
//! the point: the verb stays harness-neutral and knows only that a harness was
//! declared and configured, while the code that reads `norn` and `acp` out of a
//! document lives with the code that names those adapters anyway.
//!
//! [`AgentHarness`]: aion_integrations::AgentHarness

pub(crate) mod catalogue;
pub(crate) mod settings;

// Constructing an adapter is only ever wanted by the serve path, and the
// agent-harness seam exists only on the liminal worker transport — so a build with no
// adapter, or with no transport able to carry one, carries no construction code.
#[cfg(all(any(feature = "norn", feature = "acp"), feature = "liminal-transport"))]
pub(crate) mod compose;

#[cfg(test)]
mod tests;

pub(crate) use catalogue::{HarnessKind, announce_composed_harness, registration_capabilities};
pub(crate) use settings::{ResolvedHarness, resolve as resolve_declared};
#[cfg(test)]
pub(crate) use settings::test_environment;

