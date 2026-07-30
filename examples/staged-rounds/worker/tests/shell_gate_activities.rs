//! Hermetic execution proofs for the two gate batteries: `gate_item` in a real
//! provisioned item worktree and `verify_integration` on a real merged
//! integration worktree.
//!
//! REAL EVERYTHING: real `git` in scratch repositories, and real gate commands
//! (`sh -c ...`, which is a perfectly ordinary operator-supplied gate) that
//! genuinely exit non-zero, genuinely mutate the worktree, and genuinely record
//! what they were invoked with. Nothing is mocked at the process boundary, so
//! these tests prove the behaviour by EXECUTION: commits are read back from
//! git, tree state from `git status`, and the number of battery passes from a
//! file the gate itself appends to.

use std::path::Path;
use std::sync::{Mutex, PoisonError};

use staged_rounds_worker::commit::GATES_COMMIT_NAME;
use staged_rounds_worker::handlers::progress::{GateProgress, SilentProgress};
use staged_rounds_worker::handlers::{
    gate_item, merge_branches, provision_item, verify_integration,
};
use staged_rounds_worker::shell::Shell;
use staged_rounds_worker::types::{
    DevItemResult, GateCommand, GateItemInput, GateOutcome, GatedItem, ItemReport,
    MergeBranchesInput, VerifyIntegrationInput,
};

mod fixtures;
use fixtures::{commit_file_in, git, item, provision_input, scratch_repo};

// --- local fixtures -------------------------------------------------------

/// A gate command that runs `sh -c <script>` in the worktree. Real process,
/// real exit status.
fn gate(name: &str, script: &str) -> GateCommand {
    GateCommand {
        name: name.to_owned(),
        argv: vec!["sh".to_owned(), "-c".to_owned(), script.to_owned()],
    }
}

/// A progress sink that RECORDS what the battery reported, so the progress
/// plumbing is proven by observation rather than assumed.
///
/// Lock poisoning is handled explicitly: a poisoned recorder still yields its
/// lines (a panic here would replace a real assertion failure with a confusing
/// one).
#[derive(Default)]
struct RecordingProgress {
    lines: Mutex<Vec<String>>,
}

impl RecordingProgress {
    fn reported(&self) -> Vec<String> {
        self.lines
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl GateProgress for RecordingProgress {
    fn report(&self, detail: &str) {
        self.lines
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(detail.to_owned());
    }
}

/// A provisioned item worktree with one committed round of dev work, plus the
/// `DevItemResult` the battery judges. Returns (tempdir, `repo_root`,
/// `run_root`, the dev result).
fn dev_round() -> anyhow::Result<(tempfile::TempDir, String, String, DevItemResult)> {
    let (dir, repo, run_root) = scratch_repo()?;
    let provisioned = provision_item(
        &Shell::inherited(),
        provision_input(&repo, &run_root, item("it-a"), vec![]),
    )
    .map_err(|error| anyhow::anyhow!(error.message().to_owned()))?;
    commit_file_in(
        &provisioned.workspace_path,
        "it-a.txt",
        "the dev agent's work\n",
        "round 1",
    )?;
    let work = DevItemResult {
        item: provisioned.item,
        workspace_path: provisioned.workspace_path,
        branch: provisioned.branch,
        base_commit: provisioned.base_commit,
        report: ItemReport {
            item_id: "it-a".to_owned(),
            summary: "did it-a".to_owned(),
            commits: vec!["head".to_owned()],
            claims: vec![],
        },
    };
    Ok((dir, repo, run_root, work))
}

/// Run `gate_item` over a dev round with `gates`, discarding progress (these
/// tests assert on the battery's OUTCOME; the progress plumbing has its own
/// test below).
fn run_gates(work: &DevItemResult, gates: Vec<GateCommand>) -> anyhow::Result<GatedItem> {
    run_gates_with(work, gates, &SilentProgress)
}

fn run_gates_with(
    work: &DevItemResult,
    gates: Vec<GateCommand>,
    progress: &dyn GateProgress,
) -> anyhow::Result<GatedItem> {
    gate_item(
        &Shell::inherited(),
        progress,
        GateItemInput {
            work: work.clone(),
            gates,
        },
    )
    .map_err(|error| anyhow::anyhow!(error.message().to_owned()))
}

/// Create the run's integration worktree exactly as the pipeline does (a merge
/// pass over an empty done set), and return its path and branch.
fn integration_worktree(repo: &str, run_root: &str) -> anyhow::Result<(String, String)> {
    let state = merge_branches(
        &Shell::inherited(),
        MergeBranchesInput {
            run_root: run_root.to_owned(),
            repo_root: repo.to_owned(),
            base_branch: "main".to_owned(),
            done: vec![],
            prior_evidence: String::new(),
            remediation: String::new(),
        },
    )
    .map_err(|error| anyhow::anyhow!(error.message().to_owned()))?;
    Ok((state.workspace_path, state.integration_branch))
}

fn verify(
    repo: &str,
    run_root: &str,
    branch: &str,
    gates: Vec<GateCommand>,
) -> anyhow::Result<GateOutcome> {
    verify_integration(
        &Shell::inherited(),
        &SilentProgress,
        VerifyIntegrationInput {
            run_root: run_root.to_owned(),
            repo_root: repo.to_owned(),
            integration_branch: branch.to_owned(),
            gates,
        },
    )
    .map_err(|error| anyhow::anyhow!(error.message().to_owned()))
}

// --- gate_item ------------------------------------------------------------

#[test]
fn a_green_battery_records_every_exit_code_and_the_panel_diff() -> anyhow::Result<()> {
    let (_dir, _repo, _run_root, work) = dev_round()?;
    let gated = run_gates(
        &work,
        vec![
            gate("first", "echo MARKER_FIRST"),
            gate("second", "echo MARKER_SECOND"),
        ],
    )?;

    assert!(gated.gate.pass, "{}", gated.gate.diagnostics);
    let recorded: Vec<(&str, i64, bool)> = gated
        .gate
        .runs
        .iter()
        .map(|run| (run.name.as_str(), run.exit_code, run.passed))
        .collect();
    assert_eq!(
        recorded,
        vec![("first", 0, true), ("second", 0, true)],
        "every command's exit status is recorded, in configured order"
    );
    assert!(gated.gate.runs[0].output_tail.contains("MARKER_FIRST"));
    assert!(
        gated
            .gate
            .diagnostics
            .contains("all 2 gate command(s) green"),
        "{}",
        gated.gate.diagnostics
    );
    // The panel's diff is the real change against the provisioned base.
    assert!(
        gated.gate.diff.contains("+the dev agent's work"),
        "the panel diff must carry the dev round's change: {}",
        gated.gate.diff
    );
    // And the gated item carries the item's own coordinates untouched.
    assert_eq!(gated.item.id, "it-a");
    assert_eq!(gated.branch, work.branch);
    assert_eq!(gated.base_commit, work.base_commit);
    assert_eq!(gated.report.summary, "did it-a");
    Ok(())
}

/// A non-zero exit is RECORDED DATA, never an activity error: the pipeline acts
/// on a red battery, it does not crash on one.
#[test]
fn a_red_gate_is_recorded_data_not_an_error() -> anyhow::Result<()> {
    let (_dir, _repo, _run_root, work) = dev_round()?;
    let gated = run_gates(
        &work,
        vec![
            gate("green", "true"),
            gate("red", "echo MARKER_WALL >&2; exit 3"),
        ],
    )?;

    assert!(!gated.gate.pass);
    assert_eq!(
        gated.gate.runs.len(),
        2,
        "ALL commands run, not just up to the first red"
    );
    assert!(gated.gate.runs[0].passed);
    assert_eq!(gated.gate.runs[1].exit_code, 3);
    assert!(!gated.gate.runs[1].passed);
    assert!(
        gated.gate.runs[1].output_tail.contains("MARKER_WALL"),
        "stderr must be captured: {}",
        gated.gate.runs[1].output_tail
    );
    assert!(
        gated.gate.diagnostics.contains("gate `red` exited 3"),
        "{}",
        gated.gate.diagnostics
    );
    Ok(())
}

/// `{base_commit}` is substituted in EVERY argv element before execution, and
/// the RESOLVED argv is what gets recorded — proven both ways: the recorded
/// argv carries the real SHA, and so does the file the gate itself wrote.
#[test]
fn the_base_commit_token_is_substituted_in_every_argv_element() -> anyhow::Result<()> {
    let (dir, _repo, _run_root, work) = dev_round()?;
    let sink = dir.path().join("resolved-argv.txt").display().to_string();
    let gated = run_gates(
        &work,
        vec![GateCommand {
            name: "fence".to_owned(),
            argv: vec![
                "sh".to_owned(),
                "-c".to_owned(),
                format!("printf '%s %s' \"$1\" \"$2\" > {sink}"),
                "fence".to_owned(),
                "--since={base_commit}".to_owned(),
                "{base_commit}".to_owned(),
            ],
        }],
    )?;

    let base = &work.base_commit;
    assert!(gated.gate.pass, "{}", gated.gate.diagnostics);
    assert_eq!(
        gated.gate.runs[0].argv[4],
        format!("--since={base}"),
        "the token must be substituted mid-element and the RESOLVED argv recorded"
    );
    assert_eq!(gated.gate.runs[0].argv[5], *base);
    assert!(
        !gated.gate.runs[0]
            .argv
            .iter()
            .any(|arg| arg.contains("{base_commit}")),
        "no unresolved token may survive into the record: {:?}",
        gated.gate.runs[0].argv
    );
    // Proven by execution: the process really received the substituted values.
    let received = std::fs::read_to_string(&sink)?;
    assert_eq!(received, format!("--since={base} {base}"));
    Ok(())
}

#[test]
fn an_empty_argv_is_a_terminal_error_naming_the_gate() -> anyhow::Result<()> {
    let (_dir, _repo, _run_root, work) = dev_round()?;
    let Err(error) = gate_item(
        &Shell::inherited(),
        &SilentProgress,
        GateItemInput {
            work,
            gates: vec![GateCommand {
                name: "misconfigured".to_owned(),
                argv: vec![],
            }],
        },
    ) else {
        anyhow::bail!("an empty gate argv was silently treated as a verdict");
    };
    assert!(
        error.message().contains("misconfigured"),
        "{}",
        error.message()
    );
    assert!(
        error.message().contains("empty argv"),
        "{}",
        error.message()
    );
    Ok(())
}

/// A write-mode formatter used as a gate (Tom's standing rule — never
/// format-CHECK, always autoformat) leaves normalization the dev agent never
/// saw. It is committed mechanically under the gate identity, and because that
/// commit MOVED HEAD the FULL battery is re-run against it — proven by a gate
/// that appends one line per invocation to a file outside the repo.
#[test]
fn a_write_mode_formatter_gets_a_normalization_commit_and_a_full_battery_rerun()
-> anyhow::Result<()> {
    let (dir, _repo, _run_root, work) = dev_round()?;
    let counter = dir.path().join("battery-passes.txt").display().to_string();
    let head_before = git(&work.workspace_path, &["rev-parse", "HEAD"])?;

    let gated = run_gates(
        &work,
        vec![
            // Idempotent formatter: writes the file only when it is not already
            // there, so the post-normalization re-run leaves the tree clean.
            gate(
                "fmt",
                "[ -s formatted.txt ] || echo normalized > formatted.txt",
            ),
            gate("count", &format!("printf 'pass\\n' >> {counter}")),
        ],
    )?;

    // The battery really ran TWICE — the whole battery, not just the formatter.
    let passes = std::fs::read_to_string(&counter)?;
    assert_eq!(
        passes.lines().count(),
        2,
        "the FULL battery must be re-run against the normalization commit; \
         counter file held: {passes:?}"
    );

    // The normalization was committed, under the gate machinery identity.
    let head_after = git(&work.workspace_path, &["rev-parse", "HEAD"])?;
    assert_ne!(
        head_after, head_before,
        "the normalization must be committed"
    );
    assert_eq!(
        git(&work.workspace_path, &["log", "-1", "--format=%cn"])?,
        GATES_COMMIT_NAME,
        "the normalization commit must be attributable to the gate machinery"
    );
    assert!(
        git(&work.workspace_path, &["log", "-1", "--format=%s"])?
            .contains("mechanical normalization"),
        "the commit message must name what it is"
    );
    assert!(
        git(
            &work.workspace_path,
            &["show", "--stat", "--format=", "HEAD"]
        )?
        .contains("formatted.txt"),
        "the formatter's output must be in the commit"
    );

    // The tree is clean afterwards, so the branch head IS the directory.
    assert!(
        git(&work.workspace_path, &["status", "--porcelain"])?.is_empty(),
        "the worktree must be clean after normalization"
    );
    assert!(gated.gate.pass, "{}", gated.gate.diagnostics);
    assert!(
        gated
            .gate
            .diagnostics
            .contains("normalized the tree; committed mechanically as"),
        "the normalization must be recorded in the diagnostics: {}",
        gated.gate.diagnostics
    );
    // The recorded runs are the POST-normalization pass — the verdict belongs
    // to the commit the panel will actually read.
    assert_eq!(gated.gate.runs.len(), 2);
    assert!(gated.gate.runs.iter().all(|run| run.passed));
    Ok(())
}

/// A NON-idempotent formatter re-dirties the tree on the re-run: the reviewed
/// commit does not embody those changes, so the battery is recorded as FAILED
/// even though every command exited zero. Bounded to one re-run — never an
/// unbounded normalize/re-run loop.
#[test]
fn a_gate_that_redirties_the_tree_fails_even_with_every_command_green() -> anyhow::Result<()> {
    let (_dir, _repo, _run_root, work) = dev_round()?;
    let gated = run_gates(
        &work,
        // Appends every single run, so the tree is dirty again after the
        // post-normalization re-run.
        vec![gate("churn", "printf 'x' >> churn.txt")],
    )?;

    assert!(
        gated.gate.runs.iter().all(|run| run.passed),
        "every command exited zero"
    );
    assert!(
        !gated.gate.pass,
        "a re-dirtied worktree must fail the battery anyway: {}",
        gated.gate.diagnostics
    );
    assert!(
        gated.gate.diagnostics.contains("re-dirtied the worktree"),
        "{}",
        gated.gate.diagnostics
    );
    Ok(())
}

/// An EMPTY battery is the operator's explicit choice — a recorded VACUOUS
/// pass, never a silent one.
#[test]
fn an_empty_battery_is_a_diagnosed_vacuous_pass() -> anyhow::Result<()> {
    let (_dir, _repo, _run_root, work) = dev_round()?;
    let gated = run_gates(&work, vec![])?;

    assert!(gated.gate.pass);
    assert!(gated.gate.runs.is_empty());
    for phrase in [
        "VACUOUS pass",
        "nothing was verified",
        "operator's explicit choice",
    ] {
        assert!(
            gated.gate.diagnostics.contains(phrase),
            "an empty battery must say {phrase:?} on its face: {}",
            gated.gate.diagnostics
        );
    }
    Ok(())
}

/// The batteries run for minutes; each command's START and its RESULT are
/// reported as progress detail so an operator sees which command is in flight.
#[test]
fn every_gate_commands_start_and_result_is_reported_as_progress() -> anyhow::Result<()> {
    let (_dir, _repo, _run_root, work) = dev_round()?;
    let progress = RecordingProgress::default();
    run_gates_with(
        &work,
        vec![gate("green", "true"), gate("red", "exit 3")],
        &progress,
    )?;

    let reported = progress.reported();
    for expected in [
        "gate 1/2 `green` running: sh -c true",
        "gate 1/2 `green` exited 0 (pass)",
        "gate 2/2 `red` exited 3 (RED)",
    ] {
        assert!(
            reported.iter().any(|line| line == expected),
            "the battery must report {expected:?}; it reported:\n{reported:#?}"
        );
    }
    Ok(())
}

// --- verify_integration ---------------------------------------------------

#[test]
fn a_green_integration_battery_passes_on_a_clean_tree() -> anyhow::Result<()> {
    let (_dir, repo, run_root) = scratch_repo()?;
    let (_workspace, branch) = integration_worktree(&repo, &run_root)?;
    let outcome = verify(&repo, &run_root, &branch, vec![gate("build", "true")])?;

    assert!(outcome.pass, "{}", outcome.diagnostics);
    assert!(
        outcome
            .diagnostics
            .contains("battery re-run on the integration branch"),
        "{}",
        outcome.diagnostics
    );
    assert!(
        outcome.diagnostics.contains("has no per-item base commit"),
        "the honest account of what `{{base_commit}}` resolves to here must be \
         recorded, not silently implied: {}",
        outcome.diagnostics
    );
    assert!(
        outcome.diff.is_empty(),
        "a clean verification leaves nothing dirtied to show: {}",
        outcome.diff
    );
    Ok(())
}

/// THE gap this stage closes: N individually-green branches can merge cleanly
/// and still produce a red project. A red integration battery must NOT pass, so
/// the document's `green` outcome cannot fire and the run cannot reach
/// `completed`.
#[test]
fn a_red_integration_battery_does_not_pass() -> anyhow::Result<()> {
    let (_dir, repo, run_root) = scratch_repo()?;
    let (_workspace, branch) = integration_worktree(&repo, &run_root)?;
    let outcome = verify(
        &repo,
        &run_root,
        &branch,
        vec![
            gate("build", "true"),
            gate("test", "echo MARKER_UNION_BREAK; exit 101"),
        ],
    )?;

    assert!(
        !outcome.pass,
        "a red union must never be reported as verified: {}",
        outcome.diagnostics
    );
    assert_eq!(outcome.runs.len(), 2);
    assert_eq!(outcome.runs[1].exit_code, 101);
    assert!(
        outcome.diagnostics.contains("MARKER_UNION_BREAK"),
        "the failing output must be attached for the fix cycle: {}",
        outcome.diagnostics
    );
    Ok(())
}

/// There is NO normalization commit at the integration stage: a gate that
/// dirties the merged tree is itself a verification failure, even when every
/// command exits zero. Proven by reading HEAD back — no commit was made.
#[test]
fn a_gate_that_dirties_the_integration_tree_is_itself_a_failure() -> anyhow::Result<()> {
    let (_dir, repo, run_root) = scratch_repo()?;
    let (workspace, branch) = integration_worktree(&repo, &run_root)?;
    let head_before = git(&workspace, &["rev-parse", "HEAD"])?;

    let outcome = verify(
        &repo,
        &run_root,
        &branch,
        vec![gate("fmt", "echo rewritten > README.md")],
    )?;

    assert!(
        outcome.runs.iter().all(|run| run.passed),
        "the gate exited zero"
    );
    assert!(
        !outcome.pass,
        "a gate that mutates the merged tree must fail the verification: {}",
        outcome.diagnostics
    );
    assert!(
        outcome
            .diagnostics
            .contains("DIRTIED the integration worktree"),
        "{}",
        outcome.diagnostics
    );
    assert!(
        outcome.diff.contains("+rewritten"),
        "the diff must carry exactly what the gate changed: {}",
        outcome.diff
    );
    assert_eq!(
        git(&workspace, &["rev-parse", "HEAD"])?,
        head_before,
        "the integration stage must NOT commit a gate's mutation"
    );
    Ok(())
}

/// A tree that was already dirty going in did not equal the branch head, so the
/// battery did not test what merges. Recorded as a failure, never a green.
#[test]
fn a_dirty_integration_tree_going_in_fails_the_verification() -> anyhow::Result<()> {
    let (_dir, repo, run_root) = scratch_repo()?;
    let (workspace, branch) = integration_worktree(&repo, &run_root)?;
    std::fs::write(Path::new(&workspace).join("stray.txt"), "left behind\n")?;

    let outcome = verify(&repo, &run_root, &branch, vec![gate("build", "true")])?;
    assert!(!outcome.pass, "{}", outcome.diagnostics);
    assert!(
        outcome.diagnostics.contains("NOT clean going in"),
        "{}",
        outcome.diagnostics
    );
    Ok(())
}

/// Verifying the WRONG tree would produce a verdict about work nobody asked
/// about. Both a missing worktree and a branch mismatch are refused, not
/// guessed at.
#[test]
fn verifying_a_missing_or_wrong_branch_worktree_is_terminal() -> anyhow::Result<()> {
    let (_dir, repo, run_root) = scratch_repo()?;

    // Nothing has created the integration worktree yet.
    let Err(error) = verify_integration(
        &Shell::inherited(),
        &SilentProgress,
        VerifyIntegrationInput {
            run_root: run_root.clone(),
            repo_root: repo.clone(),
            integration_branch: "staged/wf-1/integration".to_owned(),
            gates: vec![],
        },
    ) else {
        anyhow::bail!("a missing integration worktree was silently verified");
    };
    assert!(
        error.message().contains("does not exist"),
        "{}",
        error.message()
    );

    // Now it exists, but the workflow names a different branch.
    integration_worktree(&repo, &run_root)?;
    let Err(error) = verify_integration(
        &Shell::inherited(),
        &SilentProgress,
        VerifyIntegrationInput {
            run_root,
            repo_root: repo,
            integration_branch: "staged/other-run/integration".to_owned(),
            gates: vec![],
        },
    ) else {
        anyhow::bail!("the wrong tree was silently verified");
    };
    assert!(
        error
            .message()
            .contains("refusing to verify the wrong tree"),
        "{}",
        error.message()
    );
    Ok(())
}

/// An empty battery at the integration stage is a vacuous pass too, and says so.
#[test]
fn an_empty_integration_battery_is_a_diagnosed_vacuous_pass() -> anyhow::Result<()> {
    let (_dir, repo, run_root) = scratch_repo()?;
    let (_workspace, branch) = integration_worktree(&repo, &run_root)?;
    let outcome = verify(&repo, &run_root, &branch, vec![])?;
    assert!(outcome.pass);
    assert!(
        outcome.diagnostics.contains("VACUOUS pass"),
        "{}",
        outcome.diagnostics
    );
    Ok(())
}
