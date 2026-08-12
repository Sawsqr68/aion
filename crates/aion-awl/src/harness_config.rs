//! The `harness` section, validated into the launch a worker process performs.
//!
//! The AST records what was written; the checker rules on it; this is the seam that
//! hands a launcher a value it can act on without re-deciding anything. Nothing here
//! supplies a setting, and nothing here re-reads a flag, a config file or an
//! environment variable: a document carrying a harness section IS the launch truth.
//!
//! # Why this is not part of the package contract
//!
//! A queue's CONTRACT is what a caller and a worker must agree on — the action names and
//! their schemas — and it is committed into package identity. How a particular operator
//! launches their agent on their box is not a contract: two deployments can serve the
//! same queue with different concurrency and different environments and be equally
//! correct. Putting it in the contract would make an unrelated concurrency change mint a
//! new package hash and refuse every worker still running the old one.

use std::path::PathBuf;
use std::time::Duration;

use crate::Span;
use crate::ast::{
    HARNESS_BINARY_FIELD, HARNESS_COMMAND_FIELD, HARNESS_CONCURRENCY_FIELD, HARNESS_CWD_FIELD,
    HARNESS_ENV_PASS_FIELD, HARNESS_EXIT_GRACE_FIELD, HARNESS_KIND_ACP, HARNESS_KIND_FIELD,
    HARNESS_KIND_NORN, HARNESS_PERMISSION_ALLOW_ONCE, HARNESS_PERMISSION_DENY,
    HARNESS_PERMISSION_FIELD, HARNESS_RECONNECT_INITIAL_BACKOFF_FIELD,
    HARNESS_RECONNECT_MAX_ATTEMPTS_FIELD, HARNESS_RECONNECT_MAX_BACKOFF_FIELD, HarnessDecl,
    WorkerDecl,
};

/// A `harness` section that passed the checker and could still not be turned into a
/// launch.
///
/// Every variant is a CHECKER DEFECT, not an operator mistake: the checker refuses each
/// of these before compilation reaches here. They are reported rather than unwrapped so
/// a future edit that drops a rule from the checker names itself instead of panicking in
/// a worker process at three in the morning.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HarnessConfigError {
    /// A setting the declared kind requires reached this seam absent.
    #[error(
        "the `harness` section of worker `{queue}` reached the launch seam with no `{field}` \
         after checking clean; this is a checker defect"
    )]
    Missing {
        /// The queue whose section is incomplete.
        queue: String,
        /// The setting that is absent.
        field: &'static str,
        /// Where the section stands.
        span: Span,
    },
    /// A setting reached this seam carrying a value the checker should have refused.
    #[error(
        "the `harness` section of worker `{queue}` reached the launch seam with `{field}` set \
         to `{value}`, which is unusable: {reason}; this is a checker defect"
    )]
    Unusable {
        /// The queue whose section is unusable.
        queue: String,
        /// The setting carrying the value.
        field: &'static str,
        /// The value, as written.
        value: String,
        /// Why it cannot be used.
        reason: &'static str,
        /// Where the value stands.
        span: Span,
    },
}

impl HarnessConfigError {
    /// Where the fault stands in the source.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Missing { span, .. } | Self::Unusable { span, .. } => *span,
        }
    }
}

/// How an agent worker answers an ACP permission request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessPermission {
    /// Refuse every gated action.
    Deny,
    /// Approve one request, once — never a remember-this-choice grant.
    AllowOnce,
}

/// The per-kind half of a launch: what is true of an ACP agent and not of a Norn one,
/// and the reverse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompiledHarnessKind {
    /// An Agent Client Protocol agent, launched as a subprocess.
    Acp {
        /// The agent executable, an absolute path.
        command: PathBuf,
        /// The absolute working directory every session is rooted at.
        cwd: PathBuf,
        /// How `session/request_permission` is answered.
        permission: HarnessPermission,
        /// How long the agent may take to exit once its stdin closes.
        exit_grace: Duration,
    },
    /// A Norn session.
    Norn {
        /// The binary to launch, or `None` to take the adapter's own documented
        /// `norn`-on-`PATH` default — the library's value, not one invented here.
        binary: Option<PathBuf>,
        /// The absolute directory the agent works in. Norn has no session to root, so
        /// unlike [`Self::Acp`]'s this is the child process's working directory and
        /// nothing else — but it decides which tree the agent reads and edits, which is
        /// why it is declared rather than inherited from wherever the worker was started.
        cwd: PathBuf,
    },
}

impl CompiledHarnessKind {
    /// The kind's word, as `kind` spells it.
    #[must_use]
    pub const fn word(&self) -> &'static str {
        match self {
            Self::Acp { .. } => HARNESS_KIND_ACP,
            Self::Norn { .. } => HARNESS_KIND_NORN,
        }
    }
}

/// One worker document's complete launch declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledHarness {
    /// Maximum activities served at once.
    pub concurrency: usize,
    /// Delay before the first reconnect attempt.
    pub reconnect_initial_backoff: Duration,
    /// Reconnect backoff cap.
    pub reconnect_max_backoff: Duration,
    /// Cumulative session-drop budget.
    pub reconnect_max_attempts: usize,
    /// The environment variable NAMES carried into the harness child, in declaration
    /// order. This list is the child's WHOLE environment: a variable not named here is
    /// absent in the child, whoever the parent is.
    pub env_pass: Vec<String>,
    /// Extra arguments handed to the launched agent, in order. Empty when the section
    /// declares none, which is the literal reading of a section that omits `args`.
    pub args: Vec<String>,
    /// The per-kind launch.
    pub kind: CompiledHarnessKind,
}

/// The launch `worker` declares, when it declares one.
///
/// `Ok(None)` is a first-class answer: a queue served by a worker SDK build, by
/// `aion worker shell`, or by declared `run` bodies has no agent harness and declares
/// none.
///
/// # Errors
///
/// Returns [`HarnessConfigError`] when a section that passed the checker cannot be
/// turned into a launch — every such case is a checker defect, named rather than
/// panicked on.
pub fn compile_harness(worker: &WorkerDecl) -> Result<Option<CompiledHarness>, HarnessConfigError> {
    let Some(harness) = &worker.harness else {
        return Ok(None);
    };
    Ok(Some(build(&worker.name, harness)?))
}

/// Builds the launch from a checked section.
fn build(queue: &str, harness: &HarnessDecl) -> Result<CompiledHarness, HarnessConfigError> {
    let concurrency = required_count(queue, harness, HARNESS_CONCURRENCY_FIELD, || {
        harness.concurrency.as_ref()
    })?;
    let reconnect_max_attempts =
        required_count(queue, harness, HARNESS_RECONNECT_MAX_ATTEMPTS_FIELD, || {
            harness.reconnect_max_attempts.as_ref()
        })?;
    let reconnect_initial_backoff = required_duration(
        queue,
        harness,
        HARNESS_RECONNECT_INITIAL_BACKOFF_FIELD,
        harness.reconnect_initial_backoff.as_ref(),
    )?;
    let reconnect_max_backoff = required_duration(
        queue,
        harness,
        HARNESS_RECONNECT_MAX_BACKOFF_FIELD,
        harness.reconnect_max_backoff.as_ref(),
    )?;
    let Some(env_pass) = &harness.env_pass else {
        return Err(HarnessConfigError::Missing {
            queue: queue.to_owned(),
            field: HARNESS_ENV_PASS_FIELD,
            span: harness.span,
        });
    };
    let env_pass = env_pass
        .value
        .iter()
        .map(|entry| entry.value.clone())
        .collect();
    let args = harness.args.as_ref().map_or_else(Vec::new, |args| {
        args.value.iter().map(|entry| entry.value.clone()).collect()
    });
    let kind = build_kind(queue, harness)?;
    Ok(CompiledHarness {
        concurrency,
        reconnect_initial_backoff,
        reconnect_max_backoff,
        reconnect_max_attempts,
        env_pass,
        args,
        kind,
    })
}

/// The per-kind half.
fn build_kind(
    queue: &str,
    harness: &HarnessDecl,
) -> Result<CompiledHarnessKind, HarnessConfigError> {
    match harness.kind_word() {
        Some(HARNESS_KIND_ACP) => {
            let command = required_path(
                queue,
                harness,
                HARNESS_COMMAND_FIELD,
                harness.command.as_ref(),
            )?;
            let cwd = required_path(queue, harness, HARNESS_CWD_FIELD, harness.cwd.as_ref())?;
            let Some(permission) = &harness.permission else {
                return Err(HarnessConfigError::Missing {
                    queue: queue.to_owned(),
                    field: HARNESS_PERMISSION_FIELD,
                    span: harness.span,
                });
            };
            let permission = match permission.value.value.as_str() {
                HARNESS_PERMISSION_DENY => HarnessPermission::Deny,
                HARNESS_PERMISSION_ALLOW_ONCE => HarnessPermission::AllowOnce,
                other => {
                    return Err(HarnessConfigError::Unusable {
                        queue: queue.to_owned(),
                        field: HARNESS_PERMISSION_FIELD,
                        value: other.to_owned(),
                        reason: "it names no permission policy",
                        span: permission.value.span,
                    });
                }
            };
            let exit_grace = required_duration(
                queue,
                harness,
                HARNESS_EXIT_GRACE_FIELD,
                harness.exit_grace.as_ref(),
            )?;
            Ok(CompiledHarnessKind::Acp {
                command,
                cwd,
                permission,
                exit_grace,
            })
        }
        Some(HARNESS_KIND_NORN) => Ok(CompiledHarnessKind::Norn {
            // OPTIONAL, so it goes through `optional_path` rather than `required_path` —
            // but through the same absoluteness net. The checker rules `binary` absolute
            // exactly as it rules `command` and `cwd`; carrying the value here with a bare
            // `PathBuf::from` would have left that one rule with no backstop, so a future
            // checker edit that dropped it would surface as a `norn` spawned from whatever
            // directory a supervisor happened to be in, at three in the morning, instead of
            // as the named defect this error type exists to report.
            binary: optional_path(queue, HARNESS_BINARY_FIELD, harness.binary.as_ref())?,
            // REQUIRED, and through the same net for the same reason. The failure this
            // backstop catches is the one the whole setting exists to prevent: without a
            // declared directory the agent works in whichever one the worker process was
            // launched from, and that fact appears in no document, no argv and no log —
            // so a lost rule here would not announce itself as a defect at all, it would
            // read as an agent quietly doing its work against the wrong tree.
            cwd: required_path(queue, harness, HARNESS_CWD_FIELD, harness.cwd.as_ref())?,
        }),
        Some(other) => Err(HarnessConfigError::Unusable {
            queue: queue.to_owned(),
            field: HARNESS_KIND_FIELD,
            value: other.to_owned(),
            reason: "it names no agent harness",
            span: harness
                .kind
                .as_ref()
                .map_or(harness.span, |kind| kind.value.span),
        }),
        None => Err(HarnessConfigError::Missing {
            queue: queue.to_owned(),
            field: HARNESS_KIND_FIELD,
            span: harness.span,
        }),
    }
}

/// A required whole-number setting, as a count.
fn required_count<'decl>(
    queue: &str,
    harness: &HarnessDecl,
    field: &'static str,
    setting: impl Fn() -> Option<&'decl crate::ast::HarnessSetting<crate::ast::HarnessInteger>>,
) -> Result<usize, HarnessConfigError> {
    let Some(setting) = setting() else {
        return Err(HarnessConfigError::Missing {
            queue: queue.to_owned(),
            field,
            span: harness.span,
        });
    };
    usize::try_from(setting.value.value).map_err(|_| HarnessConfigError::Unusable {
        queue: queue.to_owned(),
        field,
        value: setting.value.value.to_string(),
        reason: "it does not fit this machine's count type",
        span: setting.value.span,
    })
}

/// A required duration setting.
fn required_duration(
    queue: &str,
    harness: &HarnessDecl,
    field: &'static str,
    setting: Option<&crate::ast::HarnessSetting<crate::ast::DurationLiteral>>,
) -> Result<Duration, HarnessConfigError> {
    let Some(setting) = setting else {
        return Err(HarnessConfigError::Missing {
            queue: queue.to_owned(),
            field,
            span: harness.span,
        });
    };
    setting
        .value
        .checked_duration()
        .ok_or_else(|| HarnessConfigError::Unusable {
            queue: queue.to_owned(),
            field,
            value: format!("{}", setting.value.magnitude),
            reason: "it is too large to represent",
            span: setting.value.span,
        })
}

/// A required absolute-path setting.
fn required_path(
    queue: &str,
    harness: &HarnessDecl,
    field: &'static str,
    setting: Option<&crate::ast::HarnessSetting<crate::ast::HarnessText>>,
) -> Result<PathBuf, HarnessConfigError> {
    let Some(setting) = setting else {
        return Err(HarnessConfigError::Missing {
            queue: queue.to_owned(),
            field,
            span: harness.span,
        });
    };
    absolute_path(queue, field, setting)
}

/// An OPTIONAL absolute-path setting, held to the same rule as a required one.
///
/// `Ok(None)` is the setting's absence, which is a first-class state for `binary` — the
/// adapter's own documented `norn`-on-`PATH` default stands. What absence is NOT is an
/// excuse to skip the check on a value that IS present: the checker rules this field
/// absolute exactly as it rules `command` and `cwd`, and this is that rule's backstop.
fn optional_path(
    queue: &str,
    field: &'static str,
    setting: Option<&crate::ast::HarnessSetting<crate::ast::HarnessText>>,
) -> Result<Option<PathBuf>, HarnessConfigError> {
    setting
        .map(|setting| absolute_path(queue, field, setting))
        .transpose()
}

/// The path rule's backstop, reading the SAME accept-set the checker refuses by
/// ([`crate::harness_path_form`]) so the required and optional forms — and the two layers
/// — cannot come to disagree about what an acceptable path is.
///
/// Reaching here with a value in neither accepted form is a CHECKER DEFECT, not an
/// operator mistake: the checker refuses one by name, with a written reason. Reported
/// rather than resolved, because resolving it against the launching process's directory
/// is precisely the silent scoping the whole rule exists to prevent.
///
/// A `{workspace_root}`-leading value is carried through VERBATIM, placeholder and all.
/// This layer is the language's, and where the box's workspace root lives is not a fact
/// the language has or should have; the composition root that launches the harness
/// expands it and demands the result be absolute before anything is spawned.
fn absolute_path(
    queue: &str,
    field: &'static str,
    setting: &crate::ast::HarnessSetting<crate::ast::HarnessText>,
) -> Result<PathBuf, HarnessConfigError> {
    match crate::harness_path_form(&setting.value.value) {
        Ok(_) => Ok(PathBuf::from(&setting.value.value)),
        Err(refusal) => Err(HarnessConfigError::Unusable {
            queue: queue.to_owned(),
            field,
            value: setting.value.value.clone(),
            reason: refusal.reason(),
            span: setting.value.span,
        }),
    }
}

#[cfg(test)]
mod tests;
