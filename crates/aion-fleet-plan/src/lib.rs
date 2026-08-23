//! The `fleet-stages-v1` staged-chain plan contract.
//!
//! This crate is the machine half of the staged-flight chain coordinator
//! designed in `docs/design/sequential-legs.md` (Resolution D). It owns four
//! things, and nothing else owns any of them:
//!
//! 1. **The plan grammar and `check_plan`'s §5.2 validation** — one
//!    `fleet-stages-v1` block in canonical authored form, all seventeen
//!    clauses plus the derived stage-id charset tightening, each refusal
//!    carrying a stable cause code ([`validate`], [`grammar`], [`cause`]).
//! 2. **The normalized typed graph** — the run's ordering authority
//!    ([`graph`]). Design §5.3, final ¶: the graph returned by a successful
//!    `check_plan`, *"not a later parse of the working tree"*, is what
//!    orders the run.
//! 3. **The sealed conclusion verifier** — one declared token read from an
//!    EXACT recorded pushed commit, never a branch, worktree, or
//!    `git rev-parse` of a name ([`verify`]).
//! 4. **The chain records and the replay-deterministic decision function**
//!    ([`chain`]).
//!
//! # What this crate deliberately does NOT do
//!
//! It changes no existing document, adds no `FleetDevReport` field, adds no
//! multi-token judge admission, and supports no arbitrary DAG. Design §3 and
//! §10 refuse multi-parent stage graphs in v1 and this crate refuses them by
//! name ([`cause::CauseCode::MultipleSuccessors`]).
//!
//! # Determinism
//!
//! [`validate::check_plan`] and [`chain::next_action`] are pure functions of
//! their arguments: no clock, no environment, no filesystem, no randomness.
//! [`verify::verify_conclusion`] is the one function that touches the world,
//! and it touches it only through `git ls-tree` and `git cat-file` against
//! EXACT object ids.

pub mod cause;
pub mod chain;
pub mod error;
pub mod grammar;
pub mod graph;
pub mod ids;
pub mod validate;
pub mod verify;

pub use cause::{CauseCode, ExitClass};
pub use chain::{ChainAction, ChainLedger, ChainOutcome, Recorded, next_action};
pub use error::{ChainError, ConclusionCause, PlanRefusal, VerifierError};
pub use graph::{Admit, Advance, Comparison, NormalizedGraph, Stage};
pub use validate::{CheckOutcome, PlanInputs, check_plan};
pub use verify::{ExactCommit, Verdict, VerifyRequest, verify_conclusion};
