//! Mechanical git for the agent roles and the gate battery.
//!
//! DOCTRINE: agents do not run git — the worker machinery does the
//! mechanical git around them. Four mechanical steps live here, each under
//! its OWN scoped committer identity so the branch history says exactly which
//! piece of machinery produced each commit:
//!
//! - [`commit_dev_work`] (developer role): commit the round's work in the
//!   item worktree on the item branch. Without it the item branch retains
//!   nothing the dev agent did and the panel diffs an empty change. The
//!   harness then assembles the `DevItemResult` activity payload around the
//!   agent's report with [`crate::agent_output::assemble_dev_item_result`].
//! - [`commit_gate_normalization`] (the `gate_item` battery): commit what a
//!   WRITE-MODE gate command left in the tree (a formatter is the expected
//!   case) so the branch stays complete and nothing dirty ever skips green.
//!   `gate_item` re-runs the FULL battery when this moves HEAD — the
//!   reviewed commit must never be recorded green on a verdict it never
//!   earned.
//! - [`commit_integration_fix`] (the `fix_integration` role): commit the
//!   repair the agent made to the integration worktree after a red
//!   integration battery. Distinct from the remediation identity below
//!   because it answers for a different failure: the merge was CLEAN and the
//!   union is still red.
//! - [`conclude_merge`] (remediator role): in the integration worktree,
//!   conclude the in-progress merge the remediator resolved, or commit a
//!   dirty tree as a remediation fix, or record a skip when there is nothing
//!   stageable. Concluding REFUSES when any conflicted file still contains
//!   conflict markers: staging clears the index's unmerged-entry safety, so
//!   without that scan a no-op remediation would COMMIT the literal
//!   `<<<<<<<`/`>>>>>>>` markers as a concluded merge and the run could end
//!   `completed` with corrupted integration content. The refusal keeps the
//!   merge in progress and fails the activity loudly instead.
//!
//! ONE STAGING RULE, SHARED. Every step above stages the same way, and NONE
//! of them uses `git add -A` — that is the defect this module was rebuilt
//! around, measured on the live run of 2026-07-29, where a gate battery's
//! `cargo build` output was swept into three consecutive commits (96, 96 and
//! 94 files under `target/`) and both review lenses correctly rejected the
//! item for breaching its scope fence, both rounds, forever. The rule:
//!
//! 1. Every TRACKED modification and deletion is staged, across the whole
//!    worktree, always.
//! 2. An UNTRACKED file is staged only when the caller's [`ScopeFence`]
//!    admits it.
//! 3. Every untracked path excluded is returned BY NAME, on a commit and on a
//!    skip alike, and the caller puts it in the activity's recorded outcome.
//!
//! See [`ScopeFence`] for why tracked changes are never fenced, and
//! [`survey_worktree`] for asking the same question without staging anything —
//! which is how `gate_item` decides whether a gate command re-dirtied a
//! worktree, so that assertion can never disagree with what the commit did.
//!
//! All commit under scoped machinery identities via `-c`, so they are
//! attributable without touching the workspace's git config, and all are
//! idempotent across activity retries (nothing staged skips green).
//!
//! Module layout (declarations and re-exports only — logic lives in the named
//! children):
//! - [`fence`] — the [`ScopeFence`] type and its containment rule.
//! - [`staging`] — the shared staging core, the worktree survey, the
//!   exclusion-note rendering, and the git plumbing.
//! - [`steps`] — the machinery identities and the three stage-and-commit
//!   steps.
//! - [`conclude`] — the merge conclusion and its conflict-marker refusal.

mod conclude;
mod fence;
mod staging;
mod steps;

#[cfg(test)]
mod tests;

pub use conclude::{ConcludeOutcome, conclude_merge};
pub use fence::ScopeFence;
pub use staging::{WorktreeSurvey, exclusion_note, survey_worktree};
pub use steps::{
    DEVELOPER_COMMIT_EMAIL, DEVELOPER_COMMIT_NAME, FixCommitOutcome, GATES_COMMIT_EMAIL,
    GATES_COMMIT_NAME, INTEGRATION_COMMIT_EMAIL, INTEGRATION_COMMIT_NAME, REMEDIATION_COMMIT_EMAIL,
    REMEDIATION_COMMIT_NAME, commit_dev_work, commit_gate_normalization, commit_integration_fix,
};
