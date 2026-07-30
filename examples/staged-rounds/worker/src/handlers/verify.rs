//! `verify_integration`: re-run the battery on the MERGED integration
//! worktree.
//!
//! WHY IT EXISTS: N individually-green branches can merge cleanly and still
//! produce a red project — two items touching the same callers, a test that
//! only fails in combination, a lint that only fires on the union. Before this
//! stage the pipeline routed a clean merge straight to `completed`. This is the
//! only evidence that the PROJECT, not just each branch, is green, and it is
//! the gap whose blast radius is the entire deliverable.
//!
//! DELIBERATELY NO NORMALIZATION COMMIT. In `gate_item` a write-mode formatter
//! is expected and its output is committed; by the integration stage every
//! accepted branch is already normalized, so a gate that DIRTIES the
//! integration tree is not housekeeping — it is a failure, and it is recorded
//! as one. The clean-tree assertion is the handler's OWN (never an
//! operator-authored gate that could be forgotten) and it is made BEFORE and
//! AFTER the battery: clean before means the directory IS the branch head, so
//! the gates tested exactly what merges; clean after means no gate mutated it.
//! Both must hold for `pass`.
//!
//! THE `{base_commit}` TOKEN AT THIS STAGE: the document's
//! `verify_integration(run_root, repo_root, integration_branch, gates)` carries
//! no per-item base commit, and none exists — the integration tree is the union
//! of every accepted branch. The token therefore resolves to the integration
//! worktree's own HEAD, and `diagnostics` SAYS SO, so a reader can see that a
//! HEAD-relative fence gate is a no-op here rather than silently believing it
//! fenced anything.

use std::path::Path;

use aion_worker::ActivityFailure;

use crate::shell::Shell;
use crate::types::{GateOutcome, VerifyIntegrationInput};

use super::gate::{run_gate_battery, summarize, worktree_clean};
use super::progress::GateProgress;
use super::support::{clip, require_run};

/// `verify_integration`: assert the merged worktree is clean, re-run the
/// battery on it, and assert it is still clean.
///
/// # Errors
///
/// Terminal when the integration worktree does not exist, when it is not
/// checked out on `integration_branch` (a battery run on the wrong tree would
/// be a false verdict, not a red one), when a gate has an empty argv or cannot
/// RUN, or when `git` fails as infrastructure. A RED battery is never an
/// error — it is the recorded [`GateOutcome`] the workflow routes on.
pub fn verify_integration(
    shell: &Shell,
    progress: &dyn GateProgress,
    input: VerifyIntegrationInput,
) -> Result<GateOutcome, ActivityFailure> {
    let VerifyIntegrationInput {
        run_root,
        repo_root,
        integration_branch,
        gates,
    } = input;
    // The integration worktree is exactly where `merge_branches` created it
    // (`<run_root>/integration`); the two derivations must never drift.
    let workspace = format!("{run_root}/integration");
    require_integration_worktree(shell, &repo_root, &workspace, &integration_branch)?;

    let head = require_run(
        shell,
        "git",
        &["-C", &workspace, "rev-parse", "HEAD"],
        &repo_root,
        "git rev-parse HEAD (integration head)",
    )?;
    let head = head.stdout.trim().to_owned();

    let mut notes: Vec<String> = vec![format!(
        "battery re-run on the integration branch {integration_branch} at {head} \
         ({workspace}); `{{base_commit}}` in a gate argv resolved to that head — the \
         integration stage has no per-item base commit, so a HEAD-relative fence gate \
         verifies nothing here"
    )];

    let pre_clean = worktree_clean(
        shell,
        &workspace,
        "git status (integration pre-clean assertion)",
    )?;
    if !pre_clean {
        notes.push(
            "the integration worktree was NOT clean going in (git status --porcelain \
             was not empty) — the directory did not equal the branch head, so this \
             battery did not test exactly what merges; recorded as a verification \
             failure"
                .to_owned(),
        );
    }

    let raw = run_gate_battery(shell, progress, &workspace, &head, &gates)?;

    let post_clean = worktree_clean(
        shell,
        &workspace,
        "git status (integration post-clean assertion)",
    )?;
    let dirtied_diff = if post_clean {
        String::new()
    } else {
        notes.push(
            "a gate command DIRTIED the integration worktree — there is no \
             normalization commit at this stage, so a gate that mutates the merged \
             tree is itself a verification failure; the diff below carries exactly \
             what it changed"
                .to_owned(),
        );
        mutation_diff(shell, &workspace, &repo_root)?
    };

    // `summarize` owns the one pass rule and the never-silent diagnostics: a
    // tree that does not equal the branch head forces `pass = false` here for
    // the same reason a re-dirtied item worktree does in `gate_item`.
    Ok(summarize(
        &raw,
        notes,
        !pre_clean || !post_clean,
        dirtied_diff,
    ))
}

/// The mutation a gate left in the integration tree: `git diff --no-ext-diff
/// HEAD`, so staged AND unstaged changes are both visible (untracked files do
/// not appear here, but `git status --porcelain` already caught them for the
/// clean assertion).
///
/// `--no-ext-diff` is load-bearing for the same reason as in `gate_item`: this
/// repository configures an external diff driver that emits no `+`/`-`
/// prefixes, so without the flag the recorded evidence would not be a unified
/// diff at all.
fn mutation_diff(
    shell: &Shell,
    workspace: &str,
    repo_root: &str,
) -> Result<String, ActivityFailure> {
    let diff = require_run(
        shell,
        "git",
        &["-C", workspace, "diff", "--no-ext-diff", "HEAD"],
        repo_root,
        "git diff HEAD (integration tree mutated by a gate)",
    )?;
    Ok(clip(&diff.stdout))
}

/// Assert the integration worktree exists AND is checked out on the branch the
/// workflow named. A battery run on the wrong tree would produce a verdict
/// about work nobody asked about — refused, never guessed at.
fn require_integration_worktree(
    shell: &Shell,
    repo_root: &str,
    workspace: &str,
    integration_branch: &str,
) -> Result<(), ActivityFailure> {
    if !Path::new(workspace).is_dir() {
        return Err(ActivityFailure::terminal(format!(
            "the integration worktree {workspace} does not exist — \
             verify_integration runs on the tree merge_branches created, so there \
             is nothing to verify"
        )));
    }
    let current = require_run(
        shell,
        "git",
        &["-C", workspace, "rev-parse", "--abbrev-ref", "HEAD"],
        repo_root,
        "git rev-parse --abbrev-ref HEAD (integration worktree branch)",
    )?;
    let current = current.stdout.trim();
    if current != integration_branch {
        return Err(ActivityFailure::terminal(format!(
            "the integration worktree {workspace} is on branch {current:?}, not the \
             run's integration branch {integration_branch:?} — refusing to verify the \
             wrong tree"
        )));
    }
    Ok(())
}
