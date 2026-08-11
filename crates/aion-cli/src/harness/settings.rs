//! One harness's configuration, built from the DOCUMENT that declares it.
//!
//! # Where these settings used to live, and why they moved
//!
//! Until this landing they were a dozen flags on `aion worker agent`, retyped at every
//! shell and drifting between a runbook and the command somebody actually ran. Two of
//! them were genuinely un-guessable and said so at length — an ACP agent's launch
//! command is not standardised, and its permission policy is a security decision — and
//! every one of them had to be repeated identically on every box that served the queue.
//!
//! They now live in the worker document's `harness` section, which is reviewed,
//! deployed and read by the checker. The flags are GONE: keeping both would be a
//! hand-maintained drift pair, and a launch setting that can come from two places has
//! already come from the wrong one.
//!
//! So this module reads a checked [`CompiledHarness`] and produces the configuration an
//! adapter is constructed from. It reads no flag, no config file and no ambient
//! variable — the one thing it does read from the launching context is the VALUES of
//! the variables the document named in `env_pass`, which is the whole point of naming
//! them.
//!
//! # Two settings still have no default, and now cannot
//!
//! `permission` and `cwd` are demanded by the checker, by name, for exactly the reasons
//! the flags used to carry: a default permission policy silently decides whether an
//! unattended agent may write files and run commands, and a defaulted working directory
//! silently scopes an agent somewhere nobody chose. Moving them into the document did
//! not soften either rule; it moved the refusal earlier, to `aion awl check`.

#[cfg(any(feature = "norn", feature = "acp"))]
use std::path::PathBuf;
#[cfg(feature = "acp")]
use std::time::Duration;

use aion_awl::{CompiledHarness, CompiledHarnessKind};
use aion_integrations::{ChildEnvironment, EnvironmentDeclaration};
use anyhow::{Context, Result};

use super::catalogue::{self, HarnessKind};

/// One harness's configuration, VALIDATED and ready to construct an adapter from.
///
/// The variants are cfg-gated like [`HarnessKind`]'s, and for the same reason: a
/// configuration for an adapter this build does not link is not a thing that can exist,
/// so the construction in [`super::compose`] needs no unreachable arm and cannot fail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum HarnessConfig {
    /// The `norn` harness.
    #[cfg(feature = "norn")]
    Norn(NornConfig),
    /// The `acp` harness.
    #[cfg(feature = "acp")]
    Acp(AcpConfig),
}

/// The `norn` harness's validated configuration.
#[cfg(feature = "norn")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NornConfig {
    /// The binary to launch, or `None` to take the adapter's `norn`-on-`PATH` default.
    pub(crate) binary: Option<PathBuf>,
    /// Extra arguments appended to every spawned `norn`, in the order declared. The
    /// adapter expands `{workflow_id}` / `{activity_type}` in each at spawn time.
    pub(crate) args: Vec<String>,
    /// The child's WHOLE environment, resolved from the document's `env_pass` names.
    pub(crate) environment: ChildEnvironment,
}

/// The `acp` harness's validated configuration.
///
/// Carries the adapter's own [`PermissionPolicy`] rather than a CLI-side copy of it:
/// this type only exists in a build that links the adapter, so there is one vocabulary
/// for the policy and no mapping to keep honest.
///
/// [`PermissionPolicy`]: aion_integration_acp::PermissionPolicy
#[cfg(feature = "acp")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AcpConfig {
    /// The agent executable, verbatim.
    pub(crate) command: PathBuf,
    /// The agent's arguments, in the order declared.
    pub(crate) args: Vec<String>,
    /// The absolute session working directory.
    pub(crate) cwd: PathBuf,
    /// How `session/request_permission` is answered.
    pub(crate) permission: aion_integration_acp::PermissionPolicy,
    /// The child's WHOLE environment, resolved from the document's `env_pass` names.
    pub(crate) environment: ChildEnvironment,
    /// The grace the agent gets to exit once its stdin closes.
    pub(crate) exit_grace: Duration,
}

/// A harness that is both SELECTED and CONFIGURED — everything needed to compose one.
///
/// The two halves travel together because neither is usable alone: the configuration
/// says how to launch an adapter, and the kind says which adapter that is and what it
/// advertises at registration.
///
/// In a build compiling no adapter in, [`HarnessKind`] is uninhabited and so is this:
/// there is no such thing as a resolved harness, which is exactly the truth.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedHarness {
    /// Which compiled-in harness this is.
    pub(crate) kind: HarnessKind,
    /// How to launch it.
    pub(crate) config: HarnessConfig,
}

/// Resolves the document's declared harness against this build, and builds the launch.
///
/// `Ok(None)` means this build compiles NO adapter in. That is not an error here — the
/// action surface is still derived and reported — and the serve path refuses with a
/// message naming the missing piece of the build.
///
/// # Errors
///
/// Returns an error when the document names a harness this build does not carry
/// (naming what it does carry), or when its `env_pass` list is not a list of variable
/// names.
pub(crate) fn resolve(
    declared: &CompiledHarness,
    document: &std::path::Path,
) -> Result<Option<ResolvedHarness>> {
    let Some(kind) = catalogue::resolve(declared.kind.word(), document)? else {
        return Ok(None);
    };
    let environment = child_environment(declared, document)?;
    let config = build(kind, declared, environment)?;
    Ok(Some(ResolvedHarness { kind, config }))
}

/// The child's whole environment, resolved from the document's `env_pass` names against
/// THIS process — the launching context, whether that process was started by a
/// supervisor, by a console click, or by an operator at a shell.
///
/// The document names the variables; the box supplies their values. A name the box does
/// not carry contributes nothing and is reported, so a missing credential surfaces here
/// rather than three layers down inside an agent's own error message.
fn child_environment(
    declared: &CompiledHarness,
    document: &std::path::Path,
) -> Result<ChildEnvironment> {
    let declaration = EnvironmentDeclaration::new(declared.env_pass.clone()).with_context(|| {
        format!(
            "the `harness` section of {} declares an unusable `env_pass` list",
            document.display()
        )
    })?;
    let environment = declaration.resolve_from_process();
    if !environment.absent().is_empty() {
        tracing::warn!(
            absent = %environment.absent().join(", "),
            document = %document.display(),
            "these variables are named in the document's `env_pass` but are not set on this \
             machine, so they will be absent from the agent child"
        );
    }
    Ok(environment)
}

/// The per-kind configuration, over the adapter this build actually links.
///
/// The arms are cfg-gated on the features that link the adapters. A kind that resolved
/// against the catalogue is by construction one this build carries, so the refusals
/// below are unreachable — they are written rather than unwrapped so a path made
/// reachable by a future edit reports itself instead of taking a worker down.
fn build(
    kind: HarnessKind,
    declared: &CompiledHarness,
    environment: ChildEnvironment,
) -> Result<HarnessConfig> {
    match (kind, &declared.kind) {
        #[cfg(feature = "norn")]
        (HarnessKind::Norn, CompiledHarnessKind::Norn { binary }) => {
            Ok(HarnessConfig::Norn(NornConfig {
                binary: binary.clone(),
                args: declared.args.clone(),
                environment,
            }))
        }
        #[cfg(feature = "acp")]
        (
            HarnessKind::Acp,
            CompiledHarnessKind::Acp {
                command,
                cwd,
                permission,
                exit_grace,
            },
        ) => Ok(HarnessConfig::Acp(AcpConfig {
            command: command.clone(),
            args: declared.args.clone(),
            cwd: cwd.clone(),
            permission: permission_policy(*permission),
            environment,
            exit_grace: *exit_grace,
        })),
        // Unreachable by construction: the kind resolved against the catalogue FROM the
        // same word this configuration is built from, so the two cannot name different
        // adapters. Written rather than unwrapped, and carrying the declaration it was
        // handed, so a path a future edit makes reachable reports itself instead of
        // taking a worker down mid-run.
        (resolved, declared_kind) => anyhow::bail!(
            "the document declares the `{}` harness, passing {} environment variable(s), but \
             this build resolved `{}`; the two are chosen from one table and cannot disagree, \
             so this is a composition-root defect",
            declared_kind.word(),
            environment.declared().len(),
            resolved.name()
        ),
    }
}

/// The adapter's permission policy for the document's declared one.
///
/// A total mapping over a two-value vocabulary the checker has already ruled on, so
/// there is no unknown-policy case left to refuse here.
#[cfg(feature = "acp")]
const fn permission_policy(
    declared: aion_awl::HarnessPermission,
) -> aion_integration_acp::PermissionPolicy {
    match declared {
        aion_awl::HarnessPermission::Deny => aion_integration_acp::PermissionPolicy::Deny,
        aion_awl::HarnessPermission::AllowOnce => aion_integration_acp::PermissionPolicy::AllowOnce,
    }
}

/// A resolved child environment for a test that needs a COMPOSED harness rather than a
/// particular environment.
///
/// Deliberately EMPTY: nothing is spawned by any test that uses it, and an empty
/// declaration is a real, expressible state — so a fixture cannot accidentally depend on
/// a variable the box happens to export.
#[cfg(test)]
pub(crate) fn test_environment() -> ChildEnvironment {
    EnvironmentDeclaration::empty().resolve(&std::collections::BTreeMap::new())
}
