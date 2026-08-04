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

/// How this verb spells Norn's degraded-session opt-in, for the advice attached to
/// that refusal: the operator restarts the worker passing Norn's own flag through
/// `--norn-arg`. Stated here because the spelling belongs to THIS command line — the
/// adapter's advice machinery deliberately takes it as a parameter rather than
/// guessing at a caller's flags.
#[cfg(feature = "norn")]
const NORN_DEGRADED_SESSION_OPT_IN: &str = "--norn-arg --allow-degraded-session";

/// Builds the configured harness, erased for the worker serve path to hold.
///
/// The Norn adapter is wrapped in its own [`AdvisingHarness`] so the one Norn refusal
/// an operator cannot otherwise get past — the degraded-session refusal after a
/// provider-epoch change — surfaces carrying the restart flag that gets past it.
/// Whether the opt-in was given is DERIVED from the composed argv itself
/// (`argv_allows_degraded_session`), never tracked separately: the advice goes quiet
/// exactly when `--allow-degraded-session` reaches the spawned `norn`.
///
/// [`AdvisingHarness`]: aion_integration_norn::AdvisingHarness
pub(crate) fn build(config: &HarnessConfig) -> Arc<dyn DynAgentHarness> {
    match config {
        #[cfg(feature = "norn")]
        HarnessConfig::Norn(norn) => Arc::new(aion_integration_norn::AdvisingHarness::new(
            Arc::new(build_norn(norn)),
            aion_integration_norn::argv_allows_degraded_session(&norn.args),
            NORN_DEGRADED_SESSION_OPT_IN,
        )),
        #[cfg(feature = "acp")]
        HarnessConfig::Acp(acp) => Arc::new(build_acp(acp)),
    }
}

/// The Norn adapter for `config`.
///
/// With no binary configured the adapter's own constructor supplies its documented
/// `norn`-on-`PATH` default, so the default lives in the library that owns the
/// behaviour rather than being restated at this boundary. The operator's `--norn-arg`
/// values are applied through the adapter's `with_arg` builder — which is what expands
/// the `{workflow_id}` / `{activity_type}` run-identity placeholders at spawn time —
/// and every `--norn-unset-env` name through `without_env`, which strips the variable
/// from the SPAWNED child only and wins over inherited and set values alike.
///
/// `pub(super)` rather than private so the module's tests can assert the composed
/// child invocation (`NornHarness::invocation`) — a flag only observed through a live
/// agent run is untested in the case that matters.
#[cfg(feature = "norn")]
pub(super) fn build_norn(
    config: &super::settings::NornConfig,
) -> aion_integration_norn::NornHarness {
    use aion_integration_norn::NornHarness;

    let mut harness = match &config.binary {
        Some(binary) => NornHarness::with_binary(binary),
        None => NornHarness::new(),
    };
    for arg in &config.args {
        harness = harness.with_arg(arg.clone());
    }
    for key in &config.unset_env {
        harness = harness.without_env(key.clone());
    }
    harness
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
