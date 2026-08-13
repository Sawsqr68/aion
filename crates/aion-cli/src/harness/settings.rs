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

#[cfg(any(feature = "norn", feature = "acp"))]
use aion_awl::CwdSource;
use aion_awl::{CompiledHarness, CompiledHarnessKind};
#[cfg(any(feature = "norn", feature = "acp"))]
use aion_integrations::HarnessWorkspace;
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
    /// Where each agent's working directory comes from. Required and never defaulted:
    /// the only value a default could carry is this worker's own launch directory,
    /// which is the inheritance the setting exists to end. A document naming a path
    /// yields the fixed form; a document binding an action parameter yields the per-run
    /// form, and the directory is then read from each job's own input.
    pub(crate) workspace: HarnessWorkspace,
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
    /// Where each session's working directory comes from — a fixed absolute path, or
    /// the job's own input under a named parameter.
    pub(crate) workspace: HarnessWorkspace,
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

impl ResolvedHarness {
    /// The child environment this launch was resolved with — the same value the adapter
    /// clears the child's environment down to.
    ///
    /// Reached THROUGH the resolved harness rather than returned alongside it so there is
    /// one environment in play: a caller cannot report on one value and launch with
    /// another.
    ///
    /// Gated exactly as [`Self::launch_workspace`] is, and for a reason the compiler
    /// enforces rather than a stylistic one: with no adapter compiled in, every
    /// [`HarnessConfig`] variant is gated away and the enum is uninhabited, so this
    /// method has no arm to take. A zero-arm match is only accepted on the place
    /// itself — `&self.config` is a reference, and a reference to an uninhabited type
    /// is perfectly inhabited — so the body cannot be written at all in that build.
    /// Nothing is lost: a `ResolvedHarness` cannot be constructed without a variant to
    /// construct it from, so there is no caller to serve.
    #[cfg(any(feature = "norn", feature = "acp"))]
    pub(crate) const fn environment(&self) -> &ChildEnvironment {
        match &self.config {
            #[cfg(feature = "norn")]
            HarnessConfig::Norn(norn) => &norn.environment,
            #[cfg(feature = "acp")]
            HarnessConfig::Acp(acp) => &acp.environment,
        }
    }

    /// The absolute directory every agent this launch spawns will stand in — the RESOLVED
    /// one, after any `{workspace_root}` expansion.
    ///
    /// This exists so the value can be SAID. The defect this setting was added to close was
    /// never only that a directory was undeclared; it was that the directory an agent worked
    /// in "appears in no document, no argv, and no log", so there was no artifact to check
    /// the behaviour against. Requiring `cwd` closes the document. This closes the log.
    ///
    /// It matters most for the form that made the requirement satisfiable at all: a document
    /// writing `{workspace_root}` is deliberately silent about the actual path, because the
    /// path differs per box. The document cannot tell an operator where their agent stands;
    /// only the worker that resolved it can.
    ///
    /// Read THROUGH the resolved harness, for the same reason [`Self::environment`] is: a
    /// caller that re-derived this for its report could report one directory while the
    /// adapter spawned in another, and the report would be worth less than nothing.
    #[cfg(any(feature = "norn", feature = "acp"))]
    pub(crate) const fn launch_workspace(&self) -> &HarnessWorkspace {
        match &self.config {
            #[cfg(feature = "norn")]
            HarnessConfig::Norn(norn) => &norn.workspace,
            #[cfg(feature = "acp")]
            HarnessConfig::Acp(acp) => &acp.workspace,
        }
    }

    /// The same fact, worded for the one line an operator reads at startup.
    ///
    /// A per-run workspace has no directory to report before a job arrives, and naming
    /// one would be stating a fact that does not exist — so it reports the PARAMETER the
    /// directory will arrive in, which is the thing an operator can actually check
    /// against the workflow that dispatches the work.
    #[cfg(any(feature = "norn", feature = "acp"))]
    pub(crate) fn launch_directory_report(&self) -> String {
        match self.launch_workspace() {
            HarnessWorkspace::Fixed(directory) => directory.display().to_string(),
            HarnessWorkspace::PerRun(parameter) => {
                format!("each job names its own directory in its `{parameter}` parameter")
            }
        }
    }
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
    resolve_against(
        declared,
        document,
        &aion_server::worker::WorkspaceRoot::resolve(),
    )
}

/// The same resolution, against a workspace root the caller states.
///
/// [`resolve`] is the production entry and derives the root the one way it may be derived
/// — `aion_server::worker::WorkspaceRoot::resolve()`, the server's own rule, called rather
/// than copied. This seam exists because a test that pins what `{workspace_root}` expands
/// to needs a root it KNOWS, and reading the box's real one would make the assertion a
/// restatement of the environment instead of a pin on the behaviour.
///
/// # Errors
///
/// As [`resolve`], plus a refusal when a `{workspace_root}`-bearing setting cannot be
/// expanded against the given root, or expands to something that is not absolute.
pub(crate) fn resolve_against(
    declared: &CompiledHarness,
    document: &std::path::Path,
    workspace_root: &aion_server::worker::WorkspaceRoot,
) -> Result<Option<ResolvedHarness>> {
    let Some(kind) = catalogue::resolve(declared.kind.word(), document)? else {
        return Ok(None);
    };
    let environment = child_environment(declared, document)?;
    let config = build(kind, declared, environment, workspace_root, document)?;
    Ok(Some(ResolvedHarness { kind, config }))
}

/// The child's whole environment, resolved from the document's `env_pass` names against
/// THIS process — the launching context, whether that process was started by a
/// supervisor, by a console click, or by an operator at a shell.
///
/// The document names the variables; the box supplies their values. A name the box does
/// not carry contributes nothing, and WHICH names those were rides on the returned
/// [`ChildEnvironment`] for the verb to report — see
/// [`crate::worker_agent::environment_report`] for why that report is a written value
/// and not a log line. Reporting it from HERE would emit it before any tracing
/// subscriber exists, which is exactly how it went to the floor the first time.
fn child_environment(
    declared: &CompiledHarness,
    document: &std::path::Path,
) -> Result<ChildEnvironment> {
    let declaration =
        EnvironmentDeclaration::new(declared.env_pass.clone()).with_context(|| {
            format!(
                "the `harness` section of {} declares an unusable `env_pass` list",
                document.display()
            )
        })?;
    Ok(declaration.resolve_from_process())
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
    workspace_root: &aion_server::worker::WorkspaceRoot,
    document: &std::path::Path,
) -> Result<HarnessConfig> {
    match (kind, &declared.kind) {
        #[cfg(feature = "norn")]
        (HarnessKind::Norn, CompiledHarnessKind::Norn { binary, cwd }) => {
            Ok(HarnessConfig::Norn(NornConfig {
                binary: binary
                    .as_ref()
                    .map(|binary| {
                        launch_path(
                            aion_awl::HARNESS_BINARY_FIELD,
                            binary,
                            workspace_root,
                            document,
                        )
                    })
                    .transpose()?,
                workspace: launch_workspace(
                    aion_awl::HARNESS_CWD_FIELD,
                    cwd,
                    workspace_root,
                    document,
                )?,
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
            command: launch_path(
                aion_awl::HARNESS_COMMAND_FIELD,
                command,
                workspace_root,
                document,
            )?,
            args: declared.args.clone(),
            workspace: launch_workspace(
                aion_awl::HARNESS_CWD_FIELD,
                cwd,
                workspace_root,
                document,
            )?,
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

/// One path-valued harness setting, as the LAUNCH will use it: the document's
/// `{workspace_root}` expanded against THIS box, then held to the absolute rule.
///
/// This is the second half of a division of labour, and neither half is complete alone.
/// The checker accepts exactly two forms — an absolute path, or a value leading with
/// `{workspace_root}` — because a document that must be correct on more than one machine
/// (the assistant compiled into the server binary is one) cannot name any absolute
/// literal that is right on the second. What the language layer cannot do is know where
/// this box keeps its workspaces, so it carries the placeholder through verbatim and the
/// composition root resolves it here.
///
/// The absolute requirement is not softened by that: it is re-imposed on the RESULT.
/// A placeholder that expands to something relative, or one on a box whose home cannot be
/// resolved at all, is refused right here — before a worker registers, before a dispatch
/// arrives, before anything is spawned.
///
/// The root itself comes from `aion_server::worker::WorkspaceRoot::resolve()` — the
/// server's own derivation, called rather than copied. A worker started by hand and a
/// worker started by the server's supervisor read the one rule, so they cannot come to
/// disagree about where this box's workspaces live.
/// Where one launch's working directory will come from, as the LAUNCH will use it.
///
/// The two forms are resolved at different times and that is the whole point. A written
/// path can be settled here, once, against this box — expanded and held to the absolute
/// rule before a worker registers. A bound parameter cannot be settled here at all: the
/// directory it names does not exist until a job arrives carrying it, so what travels is
/// the parameter NAME and the adapter reads it per attempt.
///
/// # Errors
///
/// As [`launch_path`], for the written form. The bound form cannot fail here — the
/// checker has already proven every action this harness serves declares the parameter as
/// a `String`, and whether a given job supplied a usable value is a fact about that job,
/// answered when that job arrives.
#[cfg(any(feature = "norn", feature = "acp"))]
fn launch_workspace(
    field: &str,
    declared: &CwdSource,
    workspace_root: &aion_server::worker::WorkspaceRoot,
    document: &std::path::Path,
) -> Result<HarnessWorkspace> {
    match declared {
        CwdSource::Literal(path) => {
            launch_path(field, path, workspace_root, document).map(HarnessWorkspace::Fixed)
        }
        CwdSource::Bound(parameter) => Ok(HarnessWorkspace::PerRun(parameter.clone())),
    }
}

#[cfg(any(feature = "norn", feature = "acp"))]
fn launch_path(
    field: &str,
    declared: &std::path::Path,
    workspace_root: &aion_server::worker::WorkspaceRoot,
    document: &std::path::Path,
) -> Result<PathBuf> {
    let written = declared.to_str().ok_or_else(|| {
        anyhow::anyhow!(
            "the `harness` section of {} sets `{field}` to a path with no UTF-8 spelling, so it \
             cannot be checked for the `{placeholder}` the document may carry",
            document.display(),
            placeholder = aion_server::worker::WORKSPACE_ROOT_PLACEHOLDER,
        )
    })?;

    let expanded = workspace_root.expand_setting(written).with_context(|| {
        format!(
            "the `harness` section of {} sets `{field}` to `{written}`, which asks this box for \
             its workspace root",
            document.display()
        )
    })?;

    let resolved = match expanded {
        Some(expanded) => PathBuf::from(expanded),
        None => declared.to_path_buf(),
    };
    anyhow::ensure!(
        resolved.is_absolute(),
        "the `harness` section of {} sets `{field}` to `{written}`, which this box expands to \
         `{}` — and that is not an absolute path. A worker is started from whatever directory a \
         supervisor or a shell happens to be in, so a relative path names a different place on \
         every launch; the launch is refused here rather than rooted somewhere nobody chose.",
        document.display(),
        resolved.display()
    );
    Ok(resolved)
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
