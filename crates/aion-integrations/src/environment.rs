//! The DECLARED environment a harness subprocess is launched with.
//!
//! # Why inheritance is not a policy
//!
//! A spawned process inherits its parent's whole environment unless something stops it,
//! and that inheritance is invisible: nobody wrote it down, nobody reviewed it, and it
//! changes with whatever shell happened to start the worker. On 2026-08-11 that cost a
//! live run — an agent's own shell exports `CLAUDECODE`, the worker inherited it, the
//! spawned `claude-code-acp` saw it, refused to launch nested, and every `session/new`
//! came back `-32603`. Nothing in the worker's configuration mentioned `CLAUDECODE`;
//! nothing could have, because the variable was never declared anywhere.
//!
//! So a harness child's environment is CONSTRUCTED, never inherited. The document
//! declares an allow-list of variable NAMES; the launcher resolves those names — and
//! only those — against its own context; the child starts from empty and receives
//! exactly the resolved pairs. A variable that is not named is absent in the child,
//! whoever the parent is.
//!
//! # Names, not values
//!
//! The declaration carries names alone. A value belongs to the box the worker runs on
//! (a credential, a home directory, a `PATH` assembled by that machine's login shell),
//! and writing it into a document would put a secret in a file that gets committed,
//! diffed and deployed. The document says WHICH variables cross the boundary; the
//! launching context says what they are worth.
//!
//! # A missing pass-through is named, never discovered at exec
//!
//! An allow-list that omits `PATH` is a legitimate, honest declaration — an absolute
//! command needs no lookup — but a program named without a path cannot be found without
//! it, and the kernel's answer to that is a bare `No such file or directory` against a
//! command the operator can see is spelled correctly. [`ChildEnvironment::require_for_program`]
//! makes that refusal say `PATH` instead, before anything is spawned.

use std::collections::BTreeMap;
use std::path::Path;

use crate::error::HarnessError;

/// The variable a program without a path separator needs in order to be found.
const PATH_VARIABLE: &str = "PATH";

/// A fault in an environment DECLARATION, or in launching against one.
///
/// Separate from [`HarnessError`] because a declaration is validated where it is read —
/// in a document checker, in a CLI, in a launcher — long before any harness exists to
/// report through. [`From`] carries it onto the seam when a launch is what failed.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EnvironmentError {
    /// A declared entry has an empty variable name.
    ///
    /// No process environment can carry one, so the entry is an unfilled template rather
    /// than a variable anybody meant to pass through.
    #[error(
        "an environment pass-through entry has an empty variable name; every entry names one \
         variable to carry into the harness child"
    )]
    EmptyName,
    /// A declared entry is a `KEY=VALUE` pair rather than a name.
    ///
    /// The declaration passes a variable THROUGH: its value comes from the launching
    /// context. Splitting the pair silently would leave the operator believing a value
    /// they wrote is in force when the box's value is.
    #[error(
        "the environment pass-through entry `{entry}` contains `=`; entries are variable NAMES \
         only — the value is taken from the launching context, so a KEY=VALUE pair here is a \
         mistake to correct rather than reinterpret"
    )]
    NameContainsEquals {
        /// The offending entry, verbatim.
        entry: String,
    },
    /// The declaration omits a variable this particular launch cannot proceed without.
    ///
    /// Raised BEFORE the spawn, so the refusal names the variable instead of arriving as
    /// the kernel's `No such file or directory` against a correctly spelled command.
    #[error(
        "cannot launch `{program}`: it is named without a path, so it is looked up on \
         ${variable} — and ${variable} is not in the harness `env_pass` declaration, which \
         carries {declared}. Add \"{variable}\" to `env_pass`, or give `command` an absolute path."
    )]
    MissingForExec {
        /// The variable the launch needs and the declaration omits.
        variable: &'static str,
        /// The program that cannot be resolved without it.
        program: String,
        /// What the declaration does carry, rendered for the refusal.
        declared: String,
    },
}

impl From<EnvironmentError> for HarnessError {
    fn from(error: EnvironmentError) -> Self {
        // A declaration fault surfaces on the seam as a TRANSPORT failure: the channel to
        // the agent could not be opened. It is deliberately not `Protocol` — nothing was
        // malformed on a wire — and not `Contract` — a corrected declaration makes the very
        // same run succeed, so it must stay retryable.
        Self::transport(error.to_string())
    }
}

/// The environment pass-through allow-list a document declares: variable NAMES, in the
/// order they were written.
///
/// Order is preserved because it is the operator's own; duplicates are preserved for the
/// same reason and collapse when the environment is built, where a later name simply
/// resolves to the same value. Nothing here reads a value: this is the declaration, and
/// [`Self::resolve`] is where it meets a launching context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvironmentDeclaration {
    names: Vec<String>,
}

impl EnvironmentDeclaration {
    /// The declaration carrying `names`.
    ///
    /// # Errors
    ///
    /// Returns [`EnvironmentError::EmptyName`] for a blank entry and
    /// [`EnvironmentError::NameContainsEquals`] for a `KEY=VALUE` pair — the two ways an
    /// entry can look like a declaration without being one.
    pub fn new(
        names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, EnvironmentError> {
        let names = names.into_iter().map(Into::into).collect::<Vec<_>>();
        for name in &names {
            if name.trim().is_empty() {
                return Err(EnvironmentError::EmptyName);
            }
            if name.contains('=') {
                return Err(EnvironmentError::NameContainsEquals {
                    entry: name.clone(),
                });
            }
        }
        Ok(Self { names })
    }

    /// The declaration naming NO variable.
    ///
    /// Infallible, because there is no entry to be malformed. It is a real, expressible
    /// state and not a degenerate one: a child launched under it gets an empty
    /// environment, which is legitimate for an absolute command that needs nothing.
    #[must_use]
    pub fn empty() -> Self {
        Self { names: Vec::new() }
    }

    /// The declared names, in the order they were written.
    #[must_use]
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Resolves the declaration against `context` — a snapshot of the launching process's
    /// environment — into the exact, total environment a child receives.
    ///
    /// A declared name absent from `context` contributes NO variable: an unset variable
    /// and a variable set to the empty string are different things, and inventing one for
    /// the other would be a value nobody wrote. Which names came up absent is kept on the
    /// result so a launcher can say so rather than leave the operator guessing.
    #[must_use]
    pub fn resolve(&self, context: &BTreeMap<String, String>) -> ChildEnvironment {
        let mut pairs = Vec::new();
        let mut absent = Vec::new();
        let mut seen = Vec::new();
        for name in &self.names {
            if seen.iter().any(|already| already == name) {
                continue;
            }
            seen.push(name.clone());
            match context.get(name) {
                Some(value) => pairs.push((name.clone(), value.clone())),
                None => absent.push(name.clone()),
            }
        }
        ChildEnvironment {
            declared: self.names.clone(),
            pairs,
            absent,
        }
    }

    /// Resolves the declaration against THIS process's environment.
    ///
    /// The launching context of a worker that spawns its own harness children. Non-UTF-8
    /// names and values are skipped rather than lossily transcoded: a mangled credential
    /// is worse than an absent one, and an absent one is reported.
    #[must_use]
    pub fn resolve_from_process(&self) -> ChildEnvironment {
        self.resolve(&process_environment())
    }
}

/// A snapshot of this process's environment, UTF-8 entries only.
#[must_use]
pub fn process_environment() -> BTreeMap<String, String> {
    std::env::vars_os()
        .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
        .collect()
}

/// The complete environment one harness child is launched with.
///
/// "Complete" is the whole point: a spawner applying this clears the child's environment
/// first, so [`Self::pairs`] is not an overlay on an inherited set — it IS the set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChildEnvironment {
    declared: Vec<String>,
    pairs: Vec<(String, String)>,
    absent: Vec<String>,
}

impl ChildEnvironment {
    /// The resolved `(name, value)` pairs — the child's entire environment.
    #[must_use]
    pub fn pairs(&self) -> &[(String, String)] {
        &self.pairs
    }

    /// Every name the declaration carried, in declaration order.
    #[must_use]
    pub fn declared(&self) -> &[String] {
        &self.declared
    }

    /// The declared names that the launching context did not carry, so a launcher can
    /// report them rather than let a silently-absent credential surface as an agent
    /// failure three layers down.
    #[must_use]
    pub fn absent(&self) -> &[String] {
        &self.absent
    }

    /// Whether `name` is carried into the child.
    #[must_use]
    pub fn carries(&self, name: &str) -> bool {
        self.pairs.iter().any(|(key, _)| key == name)
    }

    /// Refuses the launch when `program` cannot be executed under this environment,
    /// naming the variable that is missing.
    ///
    /// The one mechanically decidable case is program lookup: a program written without a
    /// path separator is resolved on `PATH`, and a child with no `PATH` cannot resolve it.
    /// An absolute or relative path needs no lookup and is not gated here — the operator's
    /// hermetic, `PATH`-free declaration is legitimate and stays legitimate.
    ///
    /// # Errors
    ///
    /// Returns [`EnvironmentError::MissingForExec`] naming `PATH` when `program` needs a
    /// lookup this environment cannot perform.
    pub fn require_for_program(&self, program: &Path) -> Result<(), EnvironmentError> {
        if program.components().count() > 1 || program.is_absolute() {
            return Ok(());
        }
        if self.carries(PATH_VARIABLE) {
            return Ok(());
        }
        Err(EnvironmentError::MissingForExec {
            variable: PATH_VARIABLE,
            program: program.display().to_string(),
            declared: self.rendered_declaration(),
        })
    }

    /// The declaration as a refusal reads it out.
    fn rendered_declaration(&self) -> String {
        if self.declared.is_empty() {
            return "no variables".to_owned();
        }
        self.declared.join(", ")
    }
}

#[cfg(test)]
mod tests;
