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
//! a harness a build does not compile in lives where the document's declared `kind` is
//! first checked against the catalogue (`super::catalogue::resolve`, reached from
//! `super::settings::resolve`), which already has an error channel and can name what the
//! build does carry.
//!
//! [`AgentHarness`]: aion_integrations::AgentHarness

use std::sync::Arc;

use aion_integrations::DynAgentHarness;

use super::settings::HarnessConfig;

/// How a document spells Norn's degraded-session opt-in, for the advice attached to
/// that refusal: the operator adds Norn's own flag to the `harness` section's `args`
/// list and redeploys. Stated here because the spelling belongs to THIS surface — the
/// adapter's advice machinery deliberately takes it as a parameter rather than guessing
/// at a caller's configuration.
#[cfg(feature = "norn")]
const NORN_DEGRADED_SESSION_OPT_IN: &str =
    "the worker document's `harness` section: add \"--allow-degraded-session\" to its `args`";

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
/// With no binary declared the adapter's own constructor supplies its documented
/// `norn`-on-`PATH` default, so the default lives in the library that owns the
/// behaviour rather than being restated at this boundary. The document's `args` values
/// are applied through the adapter's `with_arg` builder — which is what expands the
/// `{workflow_id}` / `{activity_type}` run-identity placeholders at spawn time — and
/// the document's `env_pass` list through `with_declared_environment`, which makes the
/// spawned child's environment the declared one INSTEAD of an inherited one.
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
        Some(binary) => NornHarness::with_binary(binary, config.workspace.clone()),
        None => NornHarness::new(config.workspace.clone()),
    };
    for arg in &config.args {
        harness = harness.with_arg(arg.clone());
    }
    harness.with_declared_environment(config.environment.clone())
}

/// The ACP adapter for `config`.
///
/// Every argument the adapter requires comes from the document: the command and its
/// arguments verbatim (ACP has no standard launch flag), the absolute session root, the
/// permission policy, and the shutdown grace. The child's environment is the DECLARED
/// one — constructed from the document's `env_pass` names, set on the spawned process
/// only, and never inherited from whatever shell or supervisor started this worker.
#[cfg(feature = "acp")]
fn build_acp(config: &super::settings::AcpConfig) -> aion_integration_acp::AcpHarness {
    use aion_integration_acp::AcpHarness;

    AcpHarness::new(
        config.command.clone(),
        config.args.clone(),
        config.workspace.clone(),
        config.permission,
        config.exit_grace,
    )
    .with_declared_environment(config.environment.clone())
}
