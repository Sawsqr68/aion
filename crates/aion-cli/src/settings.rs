//! The ONE resolution contract every worker setting follows: flag, then environment
//! variable, then a refusal that names the setting.
//!
//! # Why this lives in one place
//!
//! Two independent groups of settings now share it — the agent worker's CONNECTION
//! settings ([`crate::worker_agent`]) and the per-harness CONFIGURATION the
//! composition root resolves after a harness is selected ([`crate::harness`]) — and
//! an operator cannot tell the two apart from the command line. If each resolved its
//! own way, one shell invocation could be told "supply `--acp-permission`" on one run
//! and "supply `--identity`" on the next, when both were missing all along. So the
//! collection of misses is shared, and the refusal is rendered once from the whole
//! set.
//!
//! # The rules, and why each one is the rule
//!
//! * **A flag wins over its environment variable.** The flag is a deliberate act on
//!   this invocation; the variable is ambient configuration the box carries.
//! * **A blank value counts as missing.** An exported-but-empty variable is a
//!   deployment mistake — an unset template placeholder — not a setting. Silently
//!   accepting it produces a worker with an empty identity, or an agent launched from
//!   an empty command, and neither failure names the variable that caused it.
//! * **Every missing setting is reported at once.** A worker started from a shell
//!   should need one correction rather than seven, so the misses accumulate and the
//!   refusal lists all of them, each named by BOTH the ways it could have been
//!   supplied.
//! * **Nothing is defaulted here.** A value that has a default has it in the LIBRARY
//!   that owns the behaviour (the worker SDK's hostname `node`, the Norn adapter's
//!   `norn`-on-`PATH` binary), never invented at this boundary.

use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{Result, bail};

/// One setting with no value, named by both the ways it could have been supplied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Missing {
    /// The flag that supplies it.
    pub(crate) flag: &'static str,
    /// The environment variable that supplies it when the flag is absent.
    pub(crate) variable: &'static str,
}

/// The environment a resolution reads, injected so tests never mutate the process's
/// own variables (which no test can do safely while others run).
#[derive(Clone, Debug, Default)]
pub(crate) struct Environment {
    /// The variables this resolution can see.
    variables: BTreeMap<String, String>,
}

impl Environment {
    /// The real process environment.
    pub(crate) fn from_process() -> Self {
        Self {
            variables: std::env::vars().collect(),
        }
    }

    /// An explicit environment, for tests and for a caller that must not inherit.
    #[cfg(test)]
    pub(crate) fn from_pairs<'pair>(
        pairs: impl IntoIterator<Item = (&'pair str, &'pair str)>,
    ) -> Self {
        Self {
            variables: pairs
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value.to_owned()))
                .collect(),
        }
    }

    /// The variable's value, treating blank as absent — an exported-but-empty
    /// variable is a deployment mistake, not a setting.
    pub(crate) fn read(&self, name: &str) -> Option<String> {
        self.variables
            .get(name)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    }
}

/// One setting from its flag, else its variable, recording a miss by name.
pub(crate) fn resolve_one(
    flag_value: Option<String>,
    variable: &'static str,
    flag: &'static str,
    environment: &Environment,
    missing: &mut Vec<Missing>,
) -> Option<String> {
    let resolved = optional(flag_value, variable, environment);
    if resolved.is_none() {
        missing.push(Missing { flag, variable });
    }
    resolved
}

/// One setting from its flag, else its variable, with no miss recorded.
///
/// For a setting whose absence is legitimate because the LIBRARY that consumes it
/// owns the default — never for one this boundary would have to invent.
pub(crate) fn optional(
    flag_value: Option<String>,
    variable: &str,
    environment: &Environment,
) -> Option<String> {
    flag_value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .or_else(|| environment.read(variable))
}

/// Refuses with every setting that has no value, or returns when nothing is missing.
///
/// The list is sorted so two runs of the same broken invocation produce the same
/// message, and each entry names the flag AND the variable: an operator fixing this
/// in a shell reaches for the flag, one fixing it in a unit file reaches for the
/// variable, and the message serves both without either having to guess.
///
/// # Errors
///
/// Returns an error listing every entry in `missing` when it is non-empty.
pub(crate) fn refuse_if_missing(mut missing: Vec<Missing>) -> Result<()> {
    if missing.is_empty() {
        return Ok(());
    }
    missing.sort_unstable();
    missing.dedup();
    let listed = missing
        .iter()
        .map(|item| format!("  {} or ${}", item.flag, item.variable))
        .collect::<Vec<_>>()
        .join("\n");
    bail!(
        "these worker settings have no value; supply each as a flag or an environment \
         variable:\n{listed}"
    );
}

/// A whole number greater than zero, or a refusal naming the setting and the value.
///
/// # Errors
///
/// Returns an error when `raw` is not a whole number, or is zero.
pub(crate) fn positive_usize(raw: &str, flag: &str, variable: &str) -> Result<usize> {
    let value: usize = raw
        .parse()
        .map_err(|_| anyhow::anyhow!("{flag} / ${variable} must be a whole number, got `{raw}`"))?;
    if value == 0 {
        bail!("{flag} / ${variable} must be greater than zero, got {value}");
    }
    Ok(value)
}

/// A duration in seconds, greater than zero, or a refusal naming the setting.
///
/// Fractional seconds are accepted: a reconnect backoff under a second is a
/// legitimate operator choice, and the Python host's own settings are floats.
///
/// # Errors
///
/// Returns an error when `raw` is not a number, is not finite, is not greater than
/// zero, or is too large to be a [`Duration`].
pub(crate) fn positive_seconds(raw: &str, flag: &str, variable: &str) -> Result<Duration> {
    let seconds = finite_seconds(raw, flag, variable)?;
    if seconds <= 0.0 {
        bail!("{flag} / ${variable} must be a finite number greater than zero, got `{raw}`");
    }
    duration(seconds, raw, flag, variable)
}

/// A duration in seconds that MAY be zero, or a refusal naming the setting.
///
/// Zero is a real choice for a setting whose consumer documents it as one — the ACP
/// agent's shutdown grace means "close stdin, then kill at once" at zero — so the
/// floor is separate from [`positive_seconds`] rather than a parameter, and each
/// caller picks the parser whose rule matches the setting it is reading.
///
/// Gated on the `acp` feature because that grace is currently the only setting in the
/// binary whose floor is zero: a build without the ACP adapter has nothing to read with
/// this rule, and carrying it anyway would be code no configuration can reach.
///
/// # Errors
///
/// Returns an error when `raw` is not a number, is not finite, is negative, or is too
/// large to be a [`Duration`].
#[cfg(feature = "acp")]
pub(crate) fn non_negative_seconds(raw: &str, flag: &str, variable: &str) -> Result<Duration> {
    let seconds = finite_seconds(raw, flag, variable)?;
    if seconds < 0.0 {
        bail!("{flag} / ${variable} must not be negative, got `{raw}`");
    }
    duration(seconds, raw, flag, variable)
}

/// A finite number of seconds, or a refusal naming the setting and the value.
///
/// `NaN` and the infinities are rejected here rather than by the sign comparisons
/// that follow, because `NaN` compares false against every bound and would otherwise
/// slip past a `< 0.0` test.
fn finite_seconds(raw: &str, flag: &str, variable: &str) -> Result<f64> {
    let seconds: f64 = raw
        .parse()
        .map_err(|_| anyhow::anyhow!("{flag} / ${variable} must be a number, got `{raw}`"))?;
    if !seconds.is_finite() {
        bail!("{flag} / ${variable} must be a finite number of seconds, got `{raw}`");
    }
    Ok(seconds)
}

/// The [`Duration`] for an already-validated number of seconds.
fn duration(seconds: f64, raw: &str, flag: &str, variable: &str) -> Result<Duration> {
    Duration::try_from_secs_f64(seconds).map_err(|source| {
        anyhow::anyhow!("{flag} / ${variable} is not a representable duration (`{raw}`): {source}")
    })
}
