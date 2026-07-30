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
//! THE INTEGRATION BASE — `{base_commit}` AND THE DIFF. The document passes
//! `base_branch`, and the base this stage resolves is `git merge-base
//! <base_branch> HEAD` in the integration worktree: the commit the integration
//! branch was cut from before any accepted branch merged in, which is genuinely
//! where the whole run started. It resolves `{base_commit}` in every gate argv —
//! so a scope-fence gate is MEANINGFUL here, fencing the union of the run's work
//! rather than nothing — and it is the base of the recorded `diff`, which is
//! therefore the run's real project change and lands in
//! `Completed.verification.diff`.
//!
//! An unresolvable merge-base is TERMINAL, naming the branch and the reason.
//! There is deliberately no fallback to HEAD: a HEAD-relative fence over a clean
//! tree compares nothing against nothing and reports GREEN, so a silent fallback
//! would turn the operator's scope fence into a rubber stamp — exactly the class
//! of hole this whole stage exists to close.

use std::path::Path;

use aion_worker::ActivityFailure;

use crate::shell::Shell;
use crate::types::{GateOutcome, VerifyIntegrationInput};

use super::gate::{run_gate_battery, summarize, worktree_status};
use super::progress::GateProgress;
use super::support::{clip, require_can_run, require_run};

/// `verify_integration`: resolve the run's integration base, assert the merged
/// worktree is clean, re-run the battery on it, and assert it is still clean.
///
/// # Errors
///
/// Terminal when the integration worktree does not exist, when it is not
/// checked out on `integration_branch` (a battery run on the wrong tree would
/// be a false verdict, not a red one), when the integration base cannot be
/// resolved against `base_branch`, when a gate has an empty argv or cannot RUN,
/// or when `git` fails as infrastructure. A RED battery is never an error — it
/// is the recorded [`GateOutcome`] the workflow routes on.
pub fn verify_integration(
    shell: &Shell,
    progress: &dyn GateProgress,
    input: VerifyIntegrationInput,
) -> Result<GateOutcome, ActivityFailure> {
    let VerifyIntegrationInput {
        run_root,
        repo_root,
        base_branch,
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
    let base = require_integration_base(shell, &repo_root, &workspace, &base_branch)?;

    let mut notes: Vec<String> = vec![format!(
        "battery re-run on the integration branch {integration_branch} at {head} \
         ({workspace}). The run's integration base is {base} — the merge-base of \
         {base_branch} and that head, i.e. the commit the integration branch was cut \
         from before any accepted branch merged in. `{{base_commit}}` in a gate argv \
         resolved to it, so a scope-fence gate fences the union of the run's work; \
         and the recorded diff is the WHOLE run's project change against it."
    )];

    let pre_status = worktree_status(
        shell,
        &workspace,
        "git status (integration pre-clean assertion)",
    )?;
    if !pre_status.is_empty() {
        notes.push(format!(
            "the integration worktree was NOT clean going in (git status --porcelain \
             was not empty) — the directory did not equal the branch head, so this \
             battery did not test exactly what merges; recorded as a verification \
             failure. Outstanding paths:\n{}",
            clip(&pre_status)
        ));
    }

    let raw = run_gate_battery(shell, progress, &workspace, &base, &gates)?;

    let post_status = worktree_status(
        shell,
        &workspace,
        "git status (integration post-clean assertion)",
    )?;
    if !post_status.is_empty() {
        notes.push(format!(
            "a gate command DIRTIED the integration worktree — there is no \
             normalization commit at this stage, so a gate that mutates the merged \
             tree is itself a verification failure. Paths left \
             changed:\n{}\nThe recorded diff is worktree-vs-base, so it carries this \
             uncommitted mutation on top of the run's committed change.",
            clip(&post_status)
        ));
    }

    // The run's real project diff: everything the whole run changed, against the
    // commit it started from. This is what lands in
    // `Completed.verification.diff` and what an operator actually wants to read.
    let diff = project_diff(shell, &workspace, &repo_root, &base)?;

    // `summarize` owns the one pass rule and the never-silent diagnostics: a
    // tree that does not equal the branch head forces `pass = false` here for
    // the same reason a re-dirtied item worktree does in `gate_item`.
    Ok(summarize(
        &raw,
        notes,
        !pre_status.is_empty() || !post_status.is_empty(),
        diff,
    ))
}

/// The run's integration base: `git merge-base <base_branch> HEAD` in the
/// integration worktree.
///
/// Refused loudly rather than fallen back on. `merge-base` exits non-zero both
/// when `base_branch` names nothing resolvable and when the two commits share no
/// ancestor; either way the run's starting point is unknown, and continuing with
/// HEAD would leave `{base_commit}` pointing at the very tree being examined —
/// a fence comparing a clean tree against itself, which reports green no matter
/// what the run did. An empty success is treated the same way: an empty base
/// would expand into gate argv as nothing at all.
fn require_integration_base(
    shell: &Shell,
    repo_root: &str,
    workspace: &str,
    base_branch: &str,
) -> Result<String, ActivityFailure> {
    let refusal = |reason: &str| {
        ActivityFailure::terminal(format!(
            "cannot resolve the run's integration base: `git merge-base {base_branch} \
             HEAD` in {workspace} {reason}. The base resolves `{{base_commit}}` in \
             every gate argv and is the diff's base, and there is deliberately NO \
             fallback to HEAD — a HEAD-relative fence over a clean tree compares \
             nothing and reports green, which would turn the configured scope fence \
             into a rubber stamp. Check that {base_branch} still exists in \
             {repo_root} and shares history with the integration branch."
        ))
    };
    let run = require_can_run(
        shell,
        "git",
        &["-C", workspace, "merge-base", base_branch, "HEAD"],
        repo_root,
        "git merge-base (integration base)",
    )?;
    classify_merge_base(run.succeeded(), run.exit_status, &run.output, &run.stdout)
        .map_err(|reason| refusal(&reason))
}

/// Classify a `git merge-base` invocation into the run's integration base, or
/// the reason it cannot be trusted.
///
/// Split out as a PURE function so every branch is reachable from a test. The
/// empty-success case in particular cannot be provoked through real `git` — no
/// `merge-base` invocation returns exit 0 with no commit — so behind the shell
/// it was an untestable claim, and a silent HEAD fallback could be introduced
/// there without any test noticing. That is the same shape of hole this whole
/// stage exists to close, so it does not get to hide in a defensive branch.
///
/// # Errors
///
/// The `reason` fragment for the caller's refusal message: a non-zero exit with
/// git's own output, or a success that printed no commit.
fn classify_merge_base(
    succeeded: bool,
    exit_status: i32,
    output: &str,
    stdout: &str,
) -> Result<String, String> {
    if !succeeded {
        return Err(format!("exited {exit_status}: {}", clip(output.trim())));
    }
    let base = stdout.trim();
    if base.is_empty() {
        return Err("succeeded but printed no commit".to_owned());
    }
    Ok(base.to_owned())
}

/// The run's project diff: `git diff --no-ext-diff <base>` in the integration
/// worktree, so every accepted branch's committed work — and anything a gate
/// left uncommitted — is visible in one place.
///
/// `--no-ext-diff` is load-bearing for the same reason as in `gate_item`: this
/// repository configures an external diff driver that emits no `+`/`-`
/// prefixes, so without the flag the recorded evidence would not be a unified
/// diff at all.
fn project_diff(
    shell: &Shell,
    workspace: &str,
    repo_root: &str,
    base: &str,
) -> Result<String, ActivityFailure> {
    let diff = require_run(
        shell,
        "git",
        &["-C", workspace, "diff", "--no-ext-diff", base],
        repo_root,
        "git diff (integration project change)",
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

#[cfg(test)]
mod merge_base_tests {
    use super::classify_merge_base;

    /// A resolved base is returned trimmed — git prints a trailing newline.
    #[test]
    fn a_resolved_base_is_trimmed() {
        assert_eq!(
            classify_merge_base(true, 0, "", "e472907a1f\n"),
            Ok("e472907a1f".to_owned())
        );
    }

    /// A non-zero exit — `base_branch` unresolvable, or no common ancestor —
    /// carries the exit code AND git's own words, never a bare failure.
    #[test]
    fn a_non_zero_exit_reports_the_code_and_gits_own_words() {
        // `err().unwrap_or_default()` rather than `expect_err`: the house lints
        // deny `expect_used` in tests too. An unexpected `Ok` yields an empty
        // reason, which the assertions below then report by name.
        let reason = classify_merge_base(false, 128, "fatal: Not a valid object name gone\n", "")
            .err()
            .unwrap_or_default();
        assert!(reason.contains("exited 128"), "{reason}");
        assert!(reason.contains("Not a valid object name gone"), "{reason}");
    }

    /// The branch real git will not produce, and therefore the one an
    /// integration test cannot reach: exit 0 with nothing on stdout. It must
    /// refuse, because an empty base would expand into gate argv as nothing at
    /// all — `{base_commit}` would vanish and the fence would compare nothing.
    #[test]
    fn a_success_that_printed_no_commit_is_refused_not_defaulted() {
        assert_eq!(
            classify_merge_base(true, 0, "", "   \n"),
            Err("succeeded but printed no commit".to_owned())
        );
    }
}
