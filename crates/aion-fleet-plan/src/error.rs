//! Typed errors. `thiserror` throughout; this crate is a library and never
//! reaches for `anyhow`, `.unwrap()`, or `.expect()`.

use crate::cause::{CauseCode, ExitClass};

/// A single plan refusal: a stable cause code, the stage it is about when
/// one applies, and human-readable detail.
///
/// This is the payload design §5.3 requires exit 4 to carry
/// (`StagePlanRefused`: `cause_code`, `stage_id` when applicable, plus the
/// caller's stdout/stderr, plan reply, and kept root, which the workflow
/// adds around it).
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[error("PLAN GRAPH INVALID [{cause_code}]{}: {detail}", .stage_id.as_deref().map(|s| format!(" stage `{s}`")).unwrap_or_default())]
pub struct PlanRefusal {
    /// The stable cause code.
    pub cause_code: CauseCode,
    /// The stage the refusal is about, when one applies.
    pub stage_id: Option<String>,
    /// Human-readable detail, as design §5.3 requires beside the code.
    pub detail: String,
}

impl PlanRefusal {
    /// A refusal about a named stage.
    pub fn at(
        cause_code: CauseCode,
        stage_id: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            cause_code,
            stage_id: Some(stage_id.into()),
            detail: detail.into(),
        }
    }

    /// A refusal about the plan as a whole.
    pub fn whole(cause_code: CauseCode, detail: impl Into<String>) -> Self {
        Self {
            cause_code,
            stage_id: None,
            detail: detail.into(),
        }
    }

    /// Which exit code this refusal maps to.
    #[must_use]
    pub const fn exit_class(&self) -> ExitClass {
        self.cause_code.exit_class()
    }
}

/// The stable cause codes for a refused stage conclusion (design §2.6, §4).
///
/// Exactly three, and an absent or invalid artifact is NEITHER `unplanned`
/// NOR a silent stage failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConclusionCause {
    /// The canonical path does not exist at the exact recorded commit.
    ConclusionMissing,
    /// The path exists but its bytes are not exactly one declared value
    /// followed by exactly one LF and nothing else.
    ConclusionMalformed,
    /// The bytes are well-formed but the token is not a member of that
    /// stage's declared closed vocabulary.
    ConclusionUndeclared,
}

impl ConclusionCause {
    /// The stable wire rendering. This string is the contract.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConclusionMissing => "conclusion_missing",
            Self::ConclusionMalformed => "conclusion_malformed",
            Self::ConclusionUndeclared => "conclusion_undeclared",
        }
    }
}

impl std::fmt::Display for ConclusionCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why the sealed verifier refused to even look.
///
/// Distinct from [`ConclusionCause`], which describes an artifact the
/// verifier DID read. These are seal breaches: the verifier was handed
/// something that is not an exact object id, or git could not be run at
/// all. Design §6 names resolving a branch, a worktree, or a `git
/// rev-parse` of a name as violations, so being handed one is refused
/// before any git process starts.
#[derive(Debug, thiserror::Error)]
pub enum VerifierError {
    /// The commit the verifier was handed is not exactly 40 lowercase hex
    /// characters, so it is a NAME, not an exact object id.
    #[error(
        "the sealed verifier takes an exact 40-hex object id, never a ref name: {given:?} \
         (design §6: resolving a branch ref, a worktree, or a `git rev-parse` of a name \
         violates the design)"
    )]
    NotAnObjectId {
        /// What the caller handed over.
        given: String,
    },
    /// The stage id failed the charset gate, so the conclusion path cannot
    /// be derived safely.
    #[error(
        "stage id {given:?} is outside `^[a-z0-9][a-z0-9_-]*$`, so no conclusion path is derivable"
    )]
    UnsafeStageId {
        /// What the caller handed over.
        given: String,
    },
    /// `git` could not be executed.
    #[error("could not run `git {argv}` in {cwd}: {source}")]
    GitUnavailable {
        /// The arguments attempted.
        argv: String,
        /// The working directory attempted.
        cwd: String,
        /// The underlying I/O error.
        source: std::io::Error,
    },
    /// `git` ran and refused. Its own stderr is carried, never swallowed.
    #[error("`git {argv}` exited {code}: {stderr}")]
    GitRefused {
        /// The arguments attempted.
        argv: String,
        /// The exit code git returned, or -1 when it was signalled.
        code: i32,
        /// git's verbatim stderr.
        stderr: String,
    },
    /// `git ls-tree` returned a line this crate cannot parse.
    #[error("could not parse `git ls-tree` output: {line:?}")]
    UnparseableTreeEntry {
        /// The offending line.
        line: String,
    },
}

/// Why the chain decision function could not decide.
///
/// Every variant is a defect in the recorded ledger, not an ordinary
/// outcome: the decision function is total over well-formed history.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ChainError {
    /// The ledger names a stage that is not in the normalized graph. The
    /// graph — not any later parse — is the run's ordering authority
    /// (design §5.3 final ¶), so this is a ledger defect.
    #[error(
        "the recorded ledger names stage `{stage_id}`, which the normalized graph does not contain"
    )]
    UnknownStage {
        /// The stage the ledger named.
        stage_id: String,
    },
    /// A recorded fact arrived out of the order design §2.6 fixes.
    #[error("recorded ledger is out of order: {detail}")]
    OutOfOrder {
        /// What was expected versus what was recorded.
        detail: String,
    },
    /// The ledger could not be decoded.
    #[error("could not decode the recorded chain ledger: {detail}")]
    Undecodable {
        /// The decoder's own message.
        detail: String,
    },
}
