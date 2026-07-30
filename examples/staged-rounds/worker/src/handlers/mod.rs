//! The SIX SHELL-node activity handler bodies: `provision_item`, `gate_item`,
//! `plan_reviews`, `fold_phase`, `merge_branches`, and `verify_integration`.
//! The worktree and battery handlers shell to real `git` and to the operator's
//! real gate commands through [`crate::shell::Shell`]; panel planning and phase
//! folding are pure.
//!
//! Failure taxonomy (the dev-brief discipline, kept): a command that cannot
//! RUN (missing executable, empty argv, dead working directory) is
//! INFRASTRUCTURE failure — a terminal `ActivityFailure`, because retrying a
//! broken environment cannot help. A merge CONFLICT and a RED GATE are
//! RECORDED DATA returned as a successful activity result — never an error, so
//! they land in durable workflow history and the workflow acts on them.
//! Nothing is ever swallowed into a success: every conflict rides back with
//! its captured output, and every gate command's exit code is recorded whether
//! it passed or not.
//!
//! The bodies are plain synchronous `(&Shell, Input) -> Result<Output, _>`
//! (or `(&Shell, &dyn GateProgress, Input) -> ...` for the two long batteries,
//! or pure `Input -> Result<Output, _>`) so the hermetic tests drive them
//! directly; `main.rs` adapts them onto the worker's async handler signature.
//!
//! Module layout (declarations and re-exports only — logic lives in the
//! named children):
//! - [`provision`] — per-item worktree provisioning (reuse, don't reset).
//! - [`gate`] — the gate battery core and the per-item `gate_item`.
//! - [`panel`] — the item × lens panel flattening, and the shared panel
//!   validation `fold_phase` also reads.
//! - [`judge`] — the acceptance rule: red gate ⇒ ineligible, every lens must
//!   report, derive-and-check per verdict, and the feedback rendering.
//! - [`fold_phase`] — the pure phase-state fold.
//! - [`merge`] — the continue-style integration merge.
//! - [`verify`] — the merged-tree battery re-run.
//! - [`progress`] — the heartbeat progress seam the two batteries report on.
//! - [`support`] — the shared shell/clipping helpers ([`clip`] is public API).

mod fold_phase;
mod gate;
mod judge;
mod merge;
mod panel;
pub mod progress;
mod provision;
mod support;
mod verify;

pub use fold_phase::fold_phase;
pub use gate::gate_item;
pub use merge::{MERGE_COMMIT_EMAIL, MERGE_COMMIT_NAME, merge_branches};
pub use panel::plan_reviews;
pub use provision::{PROVISION_COMMIT_EMAIL, PROVISION_COMMIT_NAME, provision_item};
pub use support::clip;
pub use verify::verify_integration;
