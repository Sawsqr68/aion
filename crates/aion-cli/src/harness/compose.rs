//! Constructing the selected adapter — the only code in the binary that names one.
//!
//! Everything above this module drives the harness through the neutral
//! [`AgentHarness`] trait; the concrete types appear here, behind the cargo features
//! that link them, and leave erased as `Arc<dyn DynAgentHarness>`.
//!
//! # Why the erasure happens here
//!
//! `aion_worker::AgentHarnessConfig` holds an `Arc<dyn DynAgentHarness>` because the
//! typed [`AgentHarness`] is not object-safe. Erasing at the composition root keeps
//! the naming of concrete adapters in the one place allowed to do it: the worker
//! receives something it can drive and cannot tell which adapter it got.
//!
//! # Why this cannot fail
//!
//! [`HarnessConfig`]'s variants are cfg-gated on the same features that link the
//! adapters, so a configuration for a harness this build does not carry is not a value
//! that exists. There is no unreachable arm to write and nothing to refuse: by the time
//! a configuration reaches this module, the adapter it names is present. The refusal for
//! a harness a build does not compile in lives where a supplied configuration is first
//! validated (`super::settings::Supplied::parse`), which already has an error channel.
//!
//! [`AgentHarness`]: aion_integrations::AgentHarness

use std::sync::Arc;

use aion_integrations::DynAgentHarness;

use super::settings::HarnessConfig;

/// Builds the configured harness, erased for the worker serve path to hold.
pub(crate) fn build(config: &HarnessConfig) -> Arc<dyn DynAgentHarness> {
    match config {
        #[cfg(feature = "norn")]
        HarnessConfig::Norn(norn) => Arc::new(build_norn(norn)),
        #[cfg(feature = "acp")]
        HarnessConfig::Acp(acp) => Arc::new(build_acp(acp)),
    }
}

/// The Norn adapter for `config`.
///
/// With no binary configured the adapter's own constructor supplies its documented
/// `norn`-on-`PATH` default, so the default lives in the library that owns the
/// behaviour rather than being restated at this boundary.
#[cfg(feature = "norn")]
fn build_norn(config: &super::settings::NornConfig) -> aion_integration_norn::NornHarness {
    use aion_integration_norn::NornHarness;

    match &config.binary {
        Some(binary) => NornHarness::with_binary(binary),
        None => NornHarness::new(),
    }
}

/// The ACP adapter for `config`.
///
/// Every argument the adapter requires comes from the operator: the command and its
/// arguments verbatim (ACP has no standard launch flag), the absolute session root, the
/// permission policy, and the shutdown grace. The child environment is applied through
/// the adapter's builder, which sets it on the SPAWNED process only and never mutates
/// this one.
#[cfg(feature = "acp")]
fn build_acp(config: &super::settings::AcpConfig) -> aion_integration_acp::AcpHarness {
    use aion_integration_acp::AcpHarness;

    let mut harness = AcpHarness::new(
        config.command.clone(),
        config.args.clone(),
        config.cwd.clone(),
        config.permission,
        config.exit_grace,
    );
    for (key, value) in &config.env {
        harness = harness.with_env(key, value);
    }
    harness
}
