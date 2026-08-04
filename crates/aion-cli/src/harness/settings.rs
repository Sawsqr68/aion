//! The per-harness configuration vocabulary, and its resolution.
//!
//! # Why a harness needs configuration at all
//!
//! Both first-party adapters launch a CHILD PROCESS, and neither launch is derivable:
//!
//! * The Norn adapter names its binary. Its own default is `norn` on `PATH`
//!   (`aion_integration_norn::NornHarness::new`), which is the LIBRARY's default and
//!   not one invented here — but an operator whose `norn` lives elsewhere could not
//!   use the verb at all without a way to say so.
//! * An ACP agent's launch command is genuinely not standardised. Verified against
//!   primary sources 2026-07-30 and recorded in that crate's own documentation: Claude
//!   Code is `npx @agentclientprotocol/claude-agent-acp` with no arguments of its own,
//!   Gemini CLI is `npx @google/gemini-cli --acp`, `OpenCode` is `opencode acp`, and
//!   others use `acp serve`, `--acp=true`, `agent stdio`, or
//!   `exec --output-format acp-daemon`. **Nothing may be inferred from an agent's
//!   name**, and a wrong guess spawns the agent in its interactive mode, which speaks
//!   no JSON-RPC at all and hangs the handshake until the activity times out.
//!
//! Every setting therefore follows [`crate::settings`]' contract exactly — flag wins
//! over environment variable, a blank value counts as missing, and every missing
//! setting is reported at once, together with the connection settings.
//!
//! # Two settings deliberately have NO default
//!
//! 1. **`--acp-permission`.** `aion_integration_acp::PermissionPolicy` has no
//!    [`Default`] impl on purpose, and this boundary does not add one. A default here
//!    is an assumed default on a SECURITY decision: it either silently approves an
//!    agent's file writes and shell commands, or silently blocks work the operator
//!    meant to allow. The operator states it out loud.
//! 2. **`--acp-cwd`.** Defaulting it to the worker's own current directory would
//!    silently scope an agent somewhere nobody chose — and `session/new` requires an
//!    absolute path, so a relative one is refused here rather than resolved against an
//!    unstated base.
//!
//! `--acp-exit-grace` is required for the same reason: how long an agent needs to
//! flush its logs and release what it holds once its stdin closes is a property of
//! THAT agent, so a "sensible default" here would be a guess about somebody else's
//! process. Zero is a legitimate setting and means "close stdin, then kill at once";
//! there is no wait-forever setting, because a worker must not be hung by an agent
//! that ignores EOF.
//!
//! # Why the repeatable settings have no environment variable
//!
//! `--acp-arg`, `--acp-env`, and `--norn-arg` are repeatable and ORDERED, and every
//! other setting's environment form is a single string. Packing an ordered list of
//! arbitrary strings into one variable needs a separator that no value may contain,
//! and neither an agent argument nor an environment value carries that guarantee — a
//! wrongly split argument would launch the agent in the wrong mode, which is the exact
//! failure the "nothing is inferred" rule exists to prevent. So they are supplied as
//! repeated flags, where the shell has already done the quoting correctly.
//!
//! `--norn-unset-env` is a repeatable set of NAMES rather than an ordered list, but a
//! comma-separated encoding is still not unambiguous: an environment variable name may
//! contain any byte except `=` and NUL, commas included, so a split could silently
//! turn one exotic name into two wrong ones — and a variable the operator believes is
//! stripped from the agent's environment would still reach it. Flag only, for the same
//! reason.
//!
//! # A setting for the harness you are NOT serving is refused
//!
//! A flag that silently does nothing is how an operator comes to believe a policy is
//! in force when it is not — `--acp-permission deny` accepted and ignored reads
//! exactly like a denial that is being enforced. So a harness FLAG supplied when a
//! different harness is resolved is refused by name.
//!
//! An environment VARIABLE for another harness is not refused. A flag is a deliberate
//! act on this invocation; an exported variable is ambient configuration the box
//! carries, and a box that runs an ACP worker and a Norn worker legitimately exports
//! both. Refusing on ambient state would make a working deployment fail when an
//! unrelated worker's variable is present.

// Both are types of a VALIDATED harness configuration, and each configuration exists
// only in a build that links its adapter — so the imports follow the same features.
#[cfg(any(feature = "norn", feature = "acp"))]
use std::path::PathBuf;
#[cfg(feature = "acp")]
use std::time::Duration;

use anyhow::{Result, bail};
use clap::Args;

use super::catalogue::{self, HarnessKind};
use crate::settings::{self, Environment, Missing};

/// The selector for which compiled-in harness to serve.
const HARNESS_FLAG: &str = "--harness";
/// Env: the harness selection.
const HARNESS_ENV: &str = "AION_AGENT_HARNESS";
/// The `norn` binary to launch.
const NORN_BINARY_FLAG: &str = "--norn-binary";
/// Env: the `norn` binary to launch.
const NORN_BINARY_ENV: &str = "AION_NORN_BINARY";
/// One `norn` argument (repeatable, order preserved).
const NORN_ARG_FLAG: &str = "--norn-arg";
/// One variable name removed from the spawned `norn`'s environment (repeatable).
const NORN_UNSET_ENV_FLAG: &str = "--norn-unset-env";
/// The ACP agent executable.
const ACP_COMMAND_FLAG: &str = "--acp-command";
/// Env: the ACP agent executable.
const ACP_COMMAND_ENV: &str = "AION_ACP_COMMAND";
/// One ACP agent argument (repeatable, order preserved).
const ACP_ARG_FLAG: &str = "--acp-arg";
/// The absolute ACP session working directory.
const ACP_CWD_FLAG: &str = "--acp-cwd";
/// Env: the absolute ACP session working directory.
const ACP_CWD_ENV: &str = "AION_ACP_CWD";
/// How the adapter answers `session/request_permission`.
const ACP_PERMISSION_FLAG: &str = "--acp-permission";
/// Env: how the adapter answers `session/request_permission`.
const ACP_PERMISSION_ENV: &str = "AION_ACP_PERMISSION";
/// One `KEY=VALUE` set on the spawned agent (repeatable).
const ACP_ENV_FLAG: &str = "--acp-env";
/// Seconds the agent may take to exit once its stdin closes.
const ACP_EXIT_GRACE_FLAG: &str = "--acp-exit-grace";
/// Env: seconds the agent may take to exit once its stdin closes.
const ACP_EXIT_GRACE_ENV: &str = "AION_ACP_EXIT_GRACE_SECONDS";

/// The accepted `--acp-permission` value that refuses every gated action.
#[cfg(feature = "acp")]
const PERMISSION_DENY: &str = "deny";
/// The accepted `--acp-permission` value that approves one-shot requests.
#[cfg(feature = "acp")]
const PERMISSION_ALLOW_ONCE: &str = "allow-once";

/// The harness-selection and per-harness configuration flags.
///
/// Flattened into `aion worker agent`, so the verb itself never names a concrete
/// adapter: it knows only that a harness was selected and configured. Every field is
/// optional at PARSE time — the environment may supply it, and `--check` needs none of
/// them — so what is actually required is decided by the resolution below, where the
/// resolved harness is known and every missing setting can be named at once.
///
/// The flags exist in EVERY build, including one that compiles no adapter at all. That
/// is deliberate: an operator who passes `--acp-command` to a binary without the `acp`
/// feature gets a refusal naming the harnesses the build does carry, rather than
/// clap's "unexpected argument", which says nothing about why.
#[derive(Clone, Debug, Default, Args)]
pub(crate) struct HarnessArgs {
    /// Serve this agent harness instead of deriving it from the build.
    ///
    /// A build compiling exactly one harness in leaves nothing to choose, so the
    /// harness is derived from it. This selects among several, and a build carrying
    /// several refuses to serve without it. Env: `AION_AGENT_HARNESS`.
    #[arg(long)]
    pub(crate) harness: Option<String>,
    /// The `norn` binary the `norn` harness launches.
    ///
    /// Optional: without it the adapter uses its own documented default of `norn` on
    /// `PATH`. Supply it when `norn` is installed somewhere that is not on the
    /// worker's `PATH`. Env: `AION_NORN_BINARY`.
    #[arg(long = "norn-binary")]
    pub(crate) norn_binary: Option<String>,
    /// One argument for the spawned `norn`. Repeat it; order is preserved.
    ///
    /// Flag only, deliberately: an ordered list of arbitrary strings has no safe
    /// single-variable encoding. Hyphen-leading values are accepted verbatim, because
    /// the arguments an operator most needs to pass ARE flags (`--session-id`,
    /// `--resume-if-exists`, `--account`): without that they would be swallowed as
    /// unknown arguments of `aion` itself.
    ///
    /// The adapter expands two run-identity placeholders in each argument at spawn
    /// time — `{workflow_id}` and `{activity_type}`; any other `{...}` text passes
    /// through literally. So `--norn-arg --session-id --norn-arg
    /// "{workflow_id}-{activity_type}" --norn-arg --resume-if-exists` pins one
    /// continuous norn session per (run, round), across restarts and weeks of sleep.
    #[arg(long = "norn-arg", allow_hyphen_values = true)]
    pub(crate) norn_args: Vec<String>,
    /// An environment variable REMOVED from the spawned `norn`. Repeatable.
    ///
    /// Applied to the CHILD only, and removals win over inherited and explicitly set
    /// values alike. Use it to strip ambient configuration the agent must not see —
    /// `--norn-unset-env OPENAI_API_KEY` forces norn onto its OAuth login when a stray
    /// ambient key would otherwise take precedence and fail.
    ///
    /// Flag only, deliberately: a variable name may contain any byte except `=` and
    /// NUL, so even a comma-separated single-variable encoding could split one exotic
    /// name into two wrong ones — and a variable believed stripped would still reach
    /// the agent.
    #[arg(long = "norn-unset-env", value_name = "KEY")]
    pub(crate) norn_unset_env: Vec<String>,
    /// The ACP agent executable, passed to the child verbatim.
    ///
    /// Required by the `acp` harness: ACP has no standard launch flag and nothing can
    /// be inferred from an agent's name. Env: `AION_ACP_COMMAND`.
    #[arg(long = "acp-command")]
    pub(crate) acp_command: Option<String>,
    /// One argument for the ACP agent. Repeat it; order is preserved.
    ///
    /// Flag only, deliberately: an ordered list of arbitrary strings has no safe
    /// single-variable encoding. For example Gemini CLI needs `--acp-arg --acp`,
    /// `OpenCode` needs `--acp-arg acp`, and Claude Code needs none.
    ///
    /// Hyphen-leading values are accepted verbatim, because the most common real ACP
    /// argument IS a flag (`--acp`): without that, the one thing an operator most needs
    /// to pass would be swallowed as an unknown argument of `aion` itself.
    #[arg(long = "acp-arg", allow_hyphen_values = true)]
    pub(crate) acp_args: Vec<String>,
    /// The ABSOLUTE working directory every ACP session is rooted at.
    ///
    /// Required by the `acp` harness, and never defaulted to the worker's own
    /// directory: that would silently scope an agent somewhere nobody chose.
    /// `session/new` requires an absolute path, so a relative one is refused.
    /// Env: `AION_ACP_CWD`.
    #[arg(long = "acp-cwd")]
    pub(crate) acp_cwd: Option<String>,
    /// How the ACP adapter answers `session/request_permission`: `deny` or
    /// `allow-once`.
    ///
    /// Required by the `acp` harness, with NO default, because it is a security
    /// decision: an unattended activity has no operator to ask, so the answer is
    /// decided up front. `deny` selects the agent's narrowest reject option;
    /// `allow-once` selects a one-shot approval and never a "remember this choice"
    /// grant. The decision is recorded on the activity transcript (as `deny` /
    /// `allow_once`) for every request. Env: `AION_ACP_PERMISSION`.
    #[arg(long = "acp-permission")]
    pub(crate) acp_permission: Option<String>,
    /// A `KEY=VALUE` environment variable set on the spawned ACP agent. Repeatable.
    ///
    /// Applied to the CHILD only. Flag only, for the same reason as `--acp-arg`.
    #[arg(long = "acp-env", value_name = "KEY=VALUE")]
    pub(crate) acp_env: Vec<String>,
    /// Seconds the ACP agent may take to exit on its own once the turn is over and its
    /// stdin has closed, before it is killed.
    ///
    /// Required by the `acp` harness: how long a given agent needs to flush and
    /// release what it holds is a property of that agent, not something to guess. `0`
    /// means "close stdin, then kill at once". Env: `AION_ACP_EXIT_GRACE_SECONDS`.
    #[arg(long = "acp-exit-grace")]
    pub(crate) acp_exit_grace: Option<String>,
}

/// Which harness a group of settings configures.
///
/// Unconditional, unlike [`HarnessKind`]: the FLAGS exist in every build even when the
/// adapter does not, so a setting can always be attributed, parsed, and refused. It is
/// also what keeps the cfg-gated matches confined to [`HarnessKind`]'s own methods.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HarnessSettings {
    /// The `norn` harness's settings.
    Norn,
    /// The `acp` harness's settings.
    Acp,
}

impl HarnessSettings {
    /// The harness name these settings configure, as `--harness` spells it.
    pub(crate) const fn harness_name(self) -> &'static str {
        match self {
            Self::Norn => "norn",
            Self::Acp => "acp",
        }
    }
}

/// One harness's configuration as SUPPLIED — every value still text.
///
/// The two-phase shape is what lets one refusal name every missing setting: the
/// connection settings and this are collected first, the whole set of misses is
/// reported together, and only then is any value parsed. Parsing as we went would let
/// an unusable `--acp-cwd` pre-empt the report that `--identity` was missing too.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Supplied {
    /// The `norn` harness's supplied settings.
    Norn {
        /// The `norn` binary, or `None` for the adapter's own `PATH` default.
        binary: Option<String>,
        /// The extra `norn` arguments, in order.
        args: Vec<String>,
        /// The variable names to remove from the spawned child's environment.
        unset_env: Vec<String>,
    },
    /// The `acp` harness's supplied settings.
    Acp {
        /// The agent executable.
        command: Option<String>,
        /// The agent's arguments, in order.
        args: Vec<String>,
        /// The session working directory.
        cwd: Option<String>,
        /// The permission policy's name.
        permission: Option<String>,
        /// The `KEY=VALUE` pairs to set on the child.
        env: Vec<String>,
        /// The shutdown grace, in seconds.
        exit_grace: Option<String>,
    },
}

/// One harness's configuration, VALIDATED and ready to construct an adapter from.
///
/// The variants are cfg-gated like [`HarnessKind`]'s, and for the same reason: a
/// configuration for an adapter this build does not link is not a thing that can exist,
/// so the construction in [`super::compose`] needs no unreachable arm and cannot fail.
/// The unvalidated [`Supplied`] form above stays unconditional, which is what lets the
/// FLAGS be parsed and refused in every build.
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
    /// Extra arguments appended to every spawned `norn`, in the order supplied. The
    /// adapter expands `{workflow_id}` / `{activity_type}` in each at spawn time.
    pub(crate) args: Vec<String>,
    /// Environment variables removed from every spawned `norn` child.
    pub(crate) unset_env: Vec<String>,
}

/// The `acp` harness's validated configuration.
///
/// Carries the adapter's own [`PermissionPolicy`] rather than a CLI-side copy of it:
/// this type only exists in a build that links the adapter, so there is one vocabulary
/// for the policy and no mapping to keep honest. The flag's SPELLINGS are still parsed
/// in every build (see [`Supplied::parse`]).
///
/// [`PermissionPolicy`]: aion_integration_acp::PermissionPolicy
#[cfg(feature = "acp")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AcpConfig {
    /// The agent executable, verbatim.
    pub(crate) command: PathBuf,
    /// The agent's arguments, in the order supplied.
    pub(crate) args: Vec<String>,
    /// The absolute session working directory.
    pub(crate) cwd: PathBuf,
    /// How `session/request_permission` is answered.
    pub(crate) permission: aion_integration_acp::PermissionPolicy,
    /// Environment variables set on the spawned agent.
    pub(crate) env: Vec<(String, String)>,
    /// The grace the agent gets to exit once its stdin closes.
    pub(crate) exit_grace: Duration,
}

/// A harness that is both SELECTED and CONFIGURED — everything needed to compose one.
///
/// The two halves travel together because neither is usable alone: the configuration
/// says how to launch an adapter, and the kind says which adapter that is and what it
/// advertises at registration. Deriving one from the other would mean mapping an
/// unconditional [`HarnessConfig`] back onto a cfg-gated [`HarnessKind`], which has no
/// honest answer for a harness this build does not carry.
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

impl HarnessArgs {
    /// Resolves WHICH harness this invocation serves, and refuses a setting meant for
    /// a different one.
    ///
    /// Both halves belong together: the second is only answerable once the first is,
    /// and a caller that resolved the harness without checking the settings would
    /// accept a flag that cannot take effect.
    ///
    /// `None` means this build compiles no harness in. That is not an error here — the
    /// action surface is still derived and reported — and the serve path refuses with a
    /// message naming the missing piece of the build.
    ///
    /// # Errors
    ///
    /// Returns an error when the selection is ambiguous or names a harness this build
    /// does not carry, or when a harness flag was supplied for a harness other than the
    /// resolved one.
    pub(crate) fn select(&self, environment: &Environment) -> Result<Option<HarnessKind>> {
        let requested = settings::optional(self.harness.clone(), HARNESS_ENV, environment);
        let resolved = catalogue::resolve(requested.as_deref(), HARNESS_FLAG)?;
        self.refuse_foreign_flags(resolved)?;
        Ok(resolved)
    }

    /// Refuses every harness flag that configures something other than `resolved`.
    ///
    /// See the module documentation for why a FLAG is refused and an ambient
    /// environment variable is not.
    fn refuse_foreign_flags(&self, resolved: Option<HarnessKind>) -> Result<()> {
        let serving = resolved.map(HarnessKind::name);
        let foreign = self
            .supplied_flags()
            .into_iter()
            .filter(|(settings, _)| Some(settings.harness_name()) != serving)
            .map(|(settings, flag)| {
                format!(
                    "  {flag} configures the `{}` harness",
                    settings.harness_name()
                )
            })
            .collect::<Vec<_>>();
        if foreign.is_empty() {
            return Ok(());
        }
        let listed = foreign.join("\n");
        match serving {
            Some(name) => bail!(
                "these flags configure a harness this invocation is not serving (`{name}`):\n\
                 {listed}\n\
                 A flag that silently does nothing is how an operator comes to believe a policy \
                 is in force when it is not, so it is refused rather than ignored. Select the \
                 harness they configure with {HARNESS_FLAG}, or drop them."
            ),
            None => bail!(
                "these flags configure an agent harness, but this `aion` build compiles none in \
                 (--no-default-features without `norn`/`acp`):\n{listed}"
            ),
        }
    }

    /// Every harness FLAG actually supplied on this invocation, with the settings group
    /// it belongs to.
    ///
    /// A blank flag value is not a supplied setting — it is the same unset-template
    /// mistake a blank environment variable is — so it is reported missing by the
    /// resolution rather than refused as a foreign flag here.
    ///
    /// EVERY configuration field above must appear in one of these two tables. A field
    /// left out would be silently accepted while serving a different harness, which is
    /// precisely what the refusal exists to prevent.
    fn supplied_flags(&self) -> Vec<(HarnessSettings, &'static str)> {
        let scalars = [
            (HarnessSettings::Norn, NORN_BINARY_FLAG, &self.norn_binary),
            (HarnessSettings::Acp, ACP_COMMAND_FLAG, &self.acp_command),
            (HarnessSettings::Acp, ACP_CWD_FLAG, &self.acp_cwd),
            (
                HarnessSettings::Acp,
                ACP_PERMISSION_FLAG,
                &self.acp_permission,
            ),
            (
                HarnessSettings::Acp,
                ACP_EXIT_GRACE_FLAG,
                &self.acp_exit_grace,
            ),
        ];
        let lists = [
            (HarnessSettings::Norn, NORN_ARG_FLAG, &self.norn_args),
            (
                HarnessSettings::Norn,
                NORN_UNSET_ENV_FLAG,
                &self.norn_unset_env,
            ),
            (HarnessSettings::Acp, ACP_ARG_FLAG, &self.acp_args),
            (HarnessSettings::Acp, ACP_ENV_FLAG, &self.acp_env),
        ];
        scalars
            .into_iter()
            .filter(|(_, _, value)| {
                value
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty())
            })
            .map(|(settings, flag, _)| (settings, flag))
            .chain(
                lists
                    .into_iter()
                    .filter(|(_, _, values)| !values.is_empty())
                    .map(|(settings, flag, _)| (settings, flag)),
            )
            .collect()
    }

    /// Collects the resolved harness's settings, recording every missing one by name.
    ///
    /// Only the RESOLVED harness's settings are demanded: an ACP worker is not asked
    /// for a `norn` binary, and a Norn worker is not asked for a permission policy it
    /// has no use for.
    pub(crate) fn collect(
        &self,
        harness: HarnessSettings,
        environment: &Environment,
        missing: &mut Vec<Missing>,
    ) -> Supplied {
        match harness {
            HarnessSettings::Norn => Supplied::Norn {
                // No miss recorded: the absence is legitimate because the ADAPTER owns
                // the default (`norn` on `PATH`), so nothing is invented here. The two
                // repeatable settings are optional too — a norn that needs no extra
                // arguments and no environment stripping is a complete configuration.
                binary: settings::optional(self.norn_binary.clone(), NORN_BINARY_ENV, environment),
                args: self.norn_args.clone(),
                unset_env: self.norn_unset_env.clone(),
            },
            HarnessSettings::Acp => Supplied::Acp {
                command: settings::resolve_one(
                    self.acp_command.clone(),
                    ACP_COMMAND_ENV,
                    ACP_COMMAND_FLAG,
                    environment,
                    missing,
                ),
                args: self.acp_args.clone(),
                cwd: settings::resolve_one(
                    self.acp_cwd.clone(),
                    ACP_CWD_ENV,
                    ACP_CWD_FLAG,
                    environment,
                    missing,
                ),
                permission: settings::resolve_one(
                    self.acp_permission.clone(),
                    ACP_PERMISSION_ENV,
                    ACP_PERMISSION_FLAG,
                    environment,
                    missing,
                ),
                env: self.acp_env.clone(),
                exit_grace: settings::resolve_one(
                    self.acp_exit_grace.clone(),
                    ACP_EXIT_GRACE_ENV,
                    ACP_EXIT_GRACE_FLAG,
                    environment,
                    missing,
                ),
            },
        }
    }
}

impl Supplied {
    /// Validates every supplied value into the configuration an adapter is built from.
    ///
    /// Called only after the combined missing-settings report has passed, so a value
    /// recorded as present really is present; the `let else` refusals name the setting
    /// rather than unwrapping, so a future edit that drops one from the missing-list is
    /// a reported fault instead of a panic.
    ///
    /// A harness this build does not link is refused here rather than in the
    /// construction: the arms below are the one place a supplied configuration meets the
    /// features that decide whether the adapter exists, and this function already has an
    /// error channel. Those refusals are unreachable by construction — the selection
    /// resolves only over compiled-in harnesses — but a path that becomes reachable
    /// through a future edit reports itself instead of taking the process down.
    ///
    /// # Errors
    ///
    /// Returns an error naming the one setting whose value is unusable: a relative
    /// `--acp-cwd`, an unknown `--acp-permission`, a malformed `--acp-env` or
    /// `--norn-unset-env` entry, or a grace that is not a representable non-negative
    /// number of seconds; or naming the harness this build does not compile in.
    pub(crate) fn parse(self) -> Result<HarnessConfig> {
        match self {
            #[cfg(feature = "norn")]
            Self::Norn {
                binary,
                args,
                unset_env,
            } => Ok(HarnessConfig::Norn(NornConfig {
                binary: binary.map(PathBuf::from),
                args,
                unset_env: removed_environment(&unset_env)?,
            })),
            #[cfg(not(feature = "norn"))]
            Self::Norn { .. } => bail!(
                "this `aion` build does not compile the `norn` harness in, so it cannot be \
                 configured"
            ),
            #[cfg(not(feature = "acp"))]
            Self::Acp { .. } => bail!(
                "this `aion` build does not compile the `acp` harness in, so it cannot be \
                 configured"
            ),
            #[cfg(feature = "acp")]
            Self::Acp {
                command,
                args,
                cwd,
                permission,
                env,
                exit_grace,
            } => {
                let (Some(command), Some(cwd)) = (command, cwd) else {
                    bail!("{ACP_COMMAND_FLAG} and {ACP_CWD_FLAG} must both be resolved before use");
                };
                let (Some(permission), Some(exit_grace)) = (permission, exit_grace) else {
                    bail!(
                        "{ACP_PERMISSION_FLAG} and {ACP_EXIT_GRACE_FLAG} must both be resolved \
                         before use"
                    );
                };
                Ok(HarnessConfig::Acp(AcpConfig {
                    command: PathBuf::from(command),
                    args,
                    cwd: absolute_cwd(&cwd)?,
                    permission: permission_policy(&permission)?,
                    env: child_environment(&env)?,
                    exit_grace: settings::non_negative_seconds(
                        &exit_grace,
                        ACP_EXIT_GRACE_FLAG,
                        ACP_EXIT_GRACE_ENV,
                    )?,
                }))
            }
        }
    }
}

/// The session working directory, refused unless it is absolute.
///
/// ACP's `session/new` REQUIRES an absolute `cwd`, and the adapter refuses a relative
/// one when it starts a session. Checking it here as well is not a duplicate for its
/// own sake: at this boundary the mistake is a typo an operator can fix while looking
/// at the command, whereas at the adapter it surfaces on the first dispatched activity,
/// long after the worker has registered and been selected for work.
///
/// The path is NOT made absolute for the operator. Resolving it against the worker's
/// current directory would root an agent somewhere nobody chose, which is the same
/// silent scoping the missing default exists to prevent.
#[cfg(feature = "acp")]
fn absolute_cwd(supplied: &str) -> Result<PathBuf> {
    let cwd = PathBuf::from(supplied);
    if !cwd.is_absolute() {
        bail!(
            "{ACP_CWD_FLAG} / ${ACP_CWD_ENV} must be an ABSOLUTE path, got `{supplied}`. \
             ACP's session/new requires one, and resolving a relative path against the worker's \
             own directory would root the agent somewhere nobody chose."
        );
    }
    Ok(cwd)
}

/// The permission policy named by `supplied`.
///
/// Case-insensitive: an operator who writes `DENY` in a unit file means `deny`, and
/// there is no second reading of it. Exactly the two documented spellings are accepted
/// — a refusal names both, because guessing at a third would be guessing at a security
/// decision.
#[cfg(feature = "acp")]
fn permission_policy(supplied: &str) -> Result<aion_integration_acp::PermissionPolicy> {
    use aion_integration_acp::PermissionPolicy;

    if supplied.eq_ignore_ascii_case(PERMISSION_DENY) {
        return Ok(PermissionPolicy::Deny);
    }
    if supplied.eq_ignore_ascii_case(PERMISSION_ALLOW_ONCE) {
        return Ok(PermissionPolicy::AllowOnce);
    }
    bail!(
        "{ACP_PERMISSION_FLAG} / ${ACP_PERMISSION_ENV} must be `{PERMISSION_DENY}` or \
         `{PERMISSION_ALLOW_ONCE}`, got `{supplied}`. There is no default: the policy decides \
         whether an unattended agent may write files and run commands, so it is stated rather \
         than assumed. (It appears on the activity transcript as `deny` / `allow_once`.)"
    );
}

/// The variable names to remove from the spawned `norn` child, validated.
///
/// A blank name is refused because no process environment can carry one, so the entry
/// is an unset-template mistake rather than a removal. A name containing `=` is
/// refused because it is the `KEY=VALUE` shape of a SET: this flag REMOVES, and
/// silently splitting the pair would leave the operator believing a value was either
/// applied or stripped when neither happened.
#[cfg(feature = "norn")]
fn removed_environment(supplied: &[String]) -> Result<Vec<String>> {
    for entry in supplied {
        if entry.trim().is_empty() {
            bail!("{NORN_UNSET_ENV_FLAG} entry has an empty variable name");
        }
        if entry.contains('=') {
            bail!(
                "{NORN_UNSET_ENV_FLAG} takes a variable NAME, got `{entry}` containing `=`. \
                 This flag removes a variable from the spawned norn's environment; it never \
                 sets one, so a KEY=VALUE pair here is a mistake to correct rather than \
                 reinterpret."
            );
        }
    }
    Ok(supplied.to_vec())
}

/// The `KEY=VALUE` pairs to set on the spawned agent, in the order supplied.
///
/// Split at the FIRST `=`, so a value containing `=` survives. An empty value is
/// allowed — setting a variable to the empty string is a real, different thing from
/// leaving it unset — but an empty key is not, because no process can carry one.
#[cfg(feature = "acp")]
fn child_environment(supplied: &[String]) -> Result<Vec<(String, String)>> {
    supplied
        .iter()
        .map(|entry| {
            let Some((key, value)) = entry.split_once('=') else {
                bail!(
                    "{ACP_ENV_FLAG} takes `KEY=VALUE`, got `{entry}` with no `=`. \
                     The child's environment is set from these pairs verbatim."
                );
            };
            if key.trim().is_empty() {
                bail!("{ACP_ENV_FLAG} entry `{entry}` has an empty variable name");
            }
            Ok((key.to_owned(), value.to_owned()))
        })
        .collect()
}
