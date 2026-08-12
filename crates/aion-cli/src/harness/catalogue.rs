//! Which agent harnesses this binary carries, and which one an invocation serves.
//!
//! # Harness identity is DECLARED, never assumed
//!
//! A build can carry several adapters, so "the" harness is not a compile-time constant
//! — and it is not a flag either. The document's `harness` section names its `kind`,
//! because the harness a queue is served by is a property of that queue and not of
//! whoever happened to type the command. This module's whole job is to check the
//! declared name against what the running binary actually links:
//!
//! * the build carries it → resolved;
//! * a name this build does not carry → refused, naming what it does carry, because a
//!   harness is compiled in by a cargo feature and cannot be supplied at runtime;
//! * none compiled in → `None`, which is NOT an error here: the action surface is
//!   still derived and reported (`--check` must work on a build that could not serve),
//!   and the serve path refuses with the build-shaped message it already had.

use anyhow::{Result, bail};

/// One agent-harness adapter compiled into this binary.
///
/// The variants are cfg-gated on the features that link the adapters, so this type
/// cannot name a harness the running binary does not actually carry — a
/// `--no-default-features` build makes it uninhabited, which is the type-level
/// statement that such a build can serve no agent at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HarnessKind {
    /// The first-party Norn adapter: `norn --protocol jsonrpc` as a child process.
    #[cfg(feature = "norn")]
    Norn,
    /// The first-party Agent Client Protocol adapter: any ACP agent over stdio.
    #[cfg(feature = "acp")]
    Acp,
}

impl HarnessKind {
    /// The name a document's `harness kind` declares, and the name every report prints.
    ///
    /// Short and stable: it is what an operator writes into a deployed document, so it
    /// must not carry a crate name or a version that could change under them.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            #[cfg(feature = "norn")]
            Self::Norn => "norn",
            #[cfg(feature = "acp")]
            Self::Acp => "acp",
        }
    }

    /// What this harness drives, and the crate that implements it — for the
    /// composition report and for a refusal that has to describe a choice.
    pub(crate) const fn description(self) -> &'static str {
        match self {
            #[cfg(feature = "norn")]
            Self::Norn => {
                "norn: a `norn --protocol jsonrpc` child process \
                           (aion-integration-norn)"
            }
            #[cfg(feature = "acp")]
            Self::Acp => {
                "acp: any agent speaking the Agent Client Protocol over its stdio \
                          (aion-integration-acp)"
            }
        }
    }

    /// The neutral intervention primitives this harness's ADAPTER publishes for a
    /// worker's registration-time advertisement.
    ///
    /// Nothing here guesses: each adapter publishes the set it can honour and this
    /// root only injects which one is being served. Read those functions'
    /// documentation — `aion_integration_norn::registration_capabilities` and
    /// `aion_integration_acp::registration_capabilities` — for how a
    /// registration-time set and a per-session handshake set can disagree, and why an
    /// honest advertisement is still the adapter's translation ceiling.
    pub(crate) fn registration_capabilities(self) -> aion_core::InterventionCapabilities {
        match self {
            #[cfg(feature = "norn")]
            Self::Norn => aion_integration_norn::registration_capabilities(),
            #[cfg(feature = "acp")]
            Self::Acp => aion_integration_acp::registration_capabilities(),
        }
    }
}

#[cfg(all(test, any(feature = "norn", feature = "acp")))]
impl HarnessKind {
    /// A minimal valid configuration for this harness, for a test that needs a COMPOSED
    /// harness rather than a particular configuration.
    ///
    /// Shared by the composition-root tests and the verb's surface tests so neither grows
    /// its own fixture that could drift. A method taking nothing but `self` because that
    /// is what keeps the cfg-gated match exhaustive in every build. Nothing here is
    /// spawned: an adapter's constructor only captures its configuration.
    pub(crate) fn test_config(self) -> super::settings::HarnessConfig {
        use super::settings::HarnessConfig;

        match self {
            #[cfg(feature = "norn")]
            Self::Norn => HarnessConfig::Norn(super::settings::NornConfig {
                binary: None,
                workspace: aion_integrations::HarnessWorkspace::Fixed(std::path::PathBuf::from(
                    "/srv/workspace",
                )),
                args: Vec::new(),
                environment: super::test_environment(),
            }),
            #[cfg(feature = "acp")]
            Self::Acp => HarnessConfig::Acp(super::settings::AcpConfig {
                command: std::path::PathBuf::from("/usr/local/bin/opencode"),
                args: vec![String::from("acp")],
                workspace: aion_integrations::HarnessWorkspace::Fixed(std::path::PathBuf::from(
                    "/srv/workspace",
                )),
                permission: aion_integration_acp::PermissionPolicy::Deny,
                environment: super::test_environment(),
                exit_grace: std::time::Duration::from_secs(2),
            }),
        }
    }
}

/// Every harness compiled into this binary, in a stable order.
///
/// A `Vec` rather than a `const` slice because the entries are cfg-gated and the
/// order they are listed in is the order every refusal names them in; two runs of the
/// same build must produce the same message.
pub(crate) fn compiled() -> Vec<HarnessKind> {
    [
        #[cfg(feature = "norn")]
        HarnessKind::Norn,
        #[cfg(feature = "acp")]
        HarnessKind::Acp,
    ]
    .to_vec()
}

/// The names of every compiled-in harness, comma-separated, or `none`.
fn carried() -> String {
    let names = compiled()
        .iter()
        .map(|harness| harness.name())
        .collect::<Vec<_>>()
        .join(", ");
    if names.is_empty() {
        "none".to_owned()
    } else {
        names
    }
}

/// Resolves the harness `document` declares against the adapters this binary links.
///
/// `declared` is the `kind` word from the document's `harness` section — always
/// present, because the checker refuses a section without one. See the module
/// documentation for why each case resolves the way it does.
///
/// # Errors
///
/// Returns an error when `declared` names a harness this build does not carry, naming
/// what it does carry.
pub(crate) fn resolve(declared: &str, document: &std::path::Path) -> Result<Option<HarnessKind>> {
    let compiled = compiled();
    // Nothing to serve with. Deliberately not an error: the caller still derives and
    // reports the action surface, and refuses to SERVE with a message naming the
    // missing piece of the build.
    if compiled.is_empty() {
        return Ok(None);
    }
    match compiled
        .iter()
        .copied()
        .find(|kind| kind.name() == declared)
    {
        Some(kind) => Ok(Some(kind)),
        None => bail!(
            "{} declares `harness kind {declared}`, which this `aion` build does not carry (it \
             carries: {}). A harness is compiled in by a cargo feature, so a name this binary \
             does not know cannot be supplied at runtime — rebuild with the feature, or declare \
             one of the harnesses above.",
            document.display(),
            carried()
        ),
    }
}

/// The neutral intervention primitives the resolved harness advertises at
/// registration — the empty set when no harness is compiled in.
///
/// A worker must declare its capabilities at registration, because the server routes
/// an operator command only for a primitive the worker advertised, and registration
/// happens long before any agent session exists to be asked.
///
/// The empty set is a first-class advertisement, not a degenerate one: a build with no
/// harness supports no interventions, and the ops console offers none.
pub(crate) fn registration_capabilities(
    harness: Option<HarnessKind>,
) -> aion_core::InterventionCapabilities {
    harness.map_or_else(
        aion_core::InterventionCapabilities::none,
        HarnessKind::registration_capabilities,
    )
}

/// Announces the agent harnesses composed into this binary at server start.
///
/// Reports the whole compiled-in SET rather than one harness, because at server start
/// nothing has been selected: which harness a worker serves is DECLARED by the worker
/// document it is started from, and that document also carries the launch settings no
/// server-start report has. Constructing one here would therefore mean inventing a
/// launch command, so this reports what the build carries and names where the choice is
/// written.
pub fn announce_composed_harness() {
    let compiled = compiled();
    if compiled.is_empty() {
        eprintln!(
            "aion server: no agent harness compiled into this build \
             (--no-default-features without `norn`/`acp`); agent activities are unavailable \
             until a harness is composed in."
        );
        return;
    }
    for harness in &compiled {
        eprintln!(
            "aion server: agent harness composed at the binary root: {}. \
             The worker drives it through the neutral AgentHarness trait.",
            harness.description()
        );
    }
    if compiled.len() > 1 {
        eprintln!(
            "aion server: {} harnesses are composed in, so a worker document\'s `harness` \
             section selects one by name with `kind <{}>`.",
            compiled.len(),
            carried()
        );
    }
}
