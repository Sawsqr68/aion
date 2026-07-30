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
//!   conclude the in-progress merge the remediator resolved (`git add -A` +
//!   `git commit --no-edit`), or commit a dirty tree as a remediation fix,
//!   or record a skip when the tree is clean. Concluding REFUSES when any
//!   conflicted file still contains conflict markers: `git add -A` clears
//!   the index's unmerged-entry safety, so without that scan a no-op
//!   remediation would COMMIT the literal `<<<<<<<`/`>>>>>>>` markers as a
//!   concluded merge and the run could end `completed` with corrupted
//!   integration content. The refusal keeps the merge in progress and
//!   fails the activity loudly instead.
//!
//! All commit under scoped machinery identities via `-c`, so they are
//! attributable without touching the workspace's git config, and all are
//! idempotent across activity retries (nothing staged skips green).

use crate::shell::{CliRun, Shell};

/// The developer commit's scoped committer name — machinery identity, not
/// an agent, not the operator.
pub const DEVELOPER_COMMIT_NAME: &str = "staged-rounds-developer";

/// The developer commit's scoped committer email.
pub const DEVELOPER_COMMIT_EMAIL: &str = "developer@staged-rounds.local";

/// The gate-normalization commit's scoped committer name.
pub const GATES_COMMIT_NAME: &str = "staged-rounds-gates";

/// The gate-normalization commit's scoped committer email.
pub const GATES_COMMIT_EMAIL: &str = "gates@staged-rounds.local";

/// The integration-fix commit's scoped committer name.
pub const INTEGRATION_COMMIT_NAME: &str = "staged-rounds-integration";

/// The integration-fix commit's scoped committer email.
pub const INTEGRATION_COMMIT_EMAIL: &str = "integration@staged-rounds.local";

/// The remediation commit's scoped committer name.
pub const REMEDIATION_COMMIT_NAME: &str = "staged-rounds-remediator";

/// The remediation commit's scoped committer email.
pub const REMEDIATION_COMMIT_EMAIL: &str = "remediator@staged-rounds.local";

/// What a mechanical commit step did. BOTH variants carry the branch head
/// afterwards: that hash is what the activity result's `commits` is
/// rewritten to (reality wins — on a skip, the head already embodies the
/// reported work: a previous attempt's commit, or the unchanged base when
/// the round changed nothing).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FixCommitOutcome {
    /// The work was committed.
    Committed {
        /// The created commit's hash (the branch head afterwards).
        commit: String,
        /// The staged paths the commit carries (from git, not from claims).
        paths: Vec<String>,
    },
    /// Nothing was staged; the branch head is unchanged.
    Skipped {
        /// The branch head at skip time.
        head: String,
        /// Why nothing was committed.
        reason: String,
    },
}

impl FixCommitOutcome {
    /// The branch head that embodies the reported work after the step.
    #[must_use]
    pub fn head(&self) -> &str {
        match self {
            Self::Committed { commit, .. } => commit,
            Self::Skipped { head, .. } => head,
        }
    }
}

/// Commit the dev round's work in `workspace_path` on the item branch:
/// `git add -A` (the worktree is wholly owned by this item's dev session)
/// then one commit under the scoped machinery identity, message
/// `chore(staged dev): item <id> round work`. Nothing staged skips green
/// with the unchanged head — idempotent across activity retries and honest
/// for a round that changed nothing.
///
/// # Errors
///
/// `git` unable to run, or any staging/commit command exiting non-zero.
pub fn commit_dev_work(
    shell: &Shell,
    workspace_path: &str,
    item_id: &str,
) -> Result<FixCommitOutcome, String> {
    stage_all_and_commit(
        shell,
        workspace_path,
        DEVELOPER_COMMIT_NAME,
        DEVELOPER_COMMIT_EMAIL,
        &format!("chore(staged dev): item {item_id} round work"),
        "no changes in the worktree; nothing to commit (already committed, \
         or the round changed nothing)",
    )
}

/// Commit whatever the GATE COMMANDS left in `workspace_path`: `git add -A`
/// then one commit under the gate machinery identity, message
/// `chore(staged gates): mechanical normalization from gate commands`.
///
/// A write-mode formatter used as a gate (Tom's standing rule — never
/// format-CHECK, always autoformat) leaves normalization the dev agent never
/// saw. That change belongs on the branch: without this commit the branch is
/// incomplete, the panel's diff disagrees with the branch head, and the
/// item's next re-provisioning refuses a dirty worktree. A clean tree skips
/// green with the unchanged head.
///
/// # Errors
///
/// `git` unable to run, or any staging/commit command exiting non-zero.
pub fn commit_gate_normalization(
    shell: &Shell,
    workspace_path: &str,
) -> Result<FixCommitOutcome, String> {
    stage_all_and_commit(
        shell,
        workspace_path,
        GATES_COMMIT_NAME,
        GATES_COMMIT_EMAIL,
        "chore(staged gates): mechanical normalization from gate commands",
        "gate commands left the worktree clean; nothing to commit",
    )
}

/// Commit the integration repair the `fix_integration` agent made in the
/// integration worktree: `git add -A` then one commit under the integration
/// machinery identity, message
/// `fix(staged integration): repair the red integration battery`.
///
/// The agent runs no git, so without this the repair would be discarded by
/// the very next `verify_integration` (which asserts a clean tree). A clean
/// tree skips green: an agent turn that changed nothing is reported honestly
/// rather than manufacturing an empty commit.
///
/// # Errors
///
/// `git` unable to run, or any staging/commit command exiting non-zero.
pub fn commit_integration_fix(
    shell: &Shell,
    workspace_path: &str,
) -> Result<FixCommitOutcome, String> {
    stage_all_and_commit(
        shell,
        workspace_path,
        INTEGRATION_COMMIT_NAME,
        INTEGRATION_COMMIT_EMAIL,
        "fix(staged integration): repair the red integration battery",
        "the integration fix turn left the worktree clean; nothing to commit",
    )
}

/// What [`conclude_merge`] did in the integration worktree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConcludeOutcome {
    /// An in-progress merge was concluded with the remediator's resolutions.
    Concluded {
        /// The merge commit's hash.
        commit: String,
    },
    /// No merge was in progress but the tree was dirty; the remediation was
    /// committed as a fix.
    Committed {
        /// The fix commit's hash.
        commit: String,
    },
    /// The tree was clean and no merge was in progress; nothing to do.
    Skipped {
        /// The head at skip time.
        head: String,
    },
}

/// Conclude the remediator's turn mechanically: if a merge is in progress
/// (`MERGE_HEAD` resolves), verify every still-unmerged file is genuinely
/// resolved (no conflict markers left in the worktree), then stage
/// everything and `git commit --no-edit` (concluding the conflicted merge
/// with the agent's resolutions); if no merge is in progress but the tree
/// is dirty, commit the remediation fix; if clean, record a skip.
///
/// The marker scan is load-bearing: `git add -A` clears the unmerged index
/// entries, so without it a remediator that resolved some (or zero) files
/// would get the literal conflict markers COMMITTED as a concluded merge —
/// a silent wrong output on the run's flagship remediation mechanic.
///
/// # Errors
///
/// `git` unable to run, any staging/commit command exiting non-zero, or a
/// conflicted file still carrying conflict markers (the merge is left in
/// progress) — an unconcludable merge is an activity failure, never a
/// silent pass.
pub fn conclude_merge(shell: &Shell, workspace_path: &str) -> Result<ConcludeOutcome, String> {
    let probe = shell
        .run(
            "git",
            &["rev-parse", "-q", "--verify", "MERGE_HEAD"],
            workspace_path,
        )
        .map_err(|failure| format!("git rev-parse MERGE_HEAD: {}", failure.message()))?;
    let user_name = format!("user.name={REMEDIATION_COMMIT_NAME}");
    let user_email = format!("user.email={REMEDIATION_COMMIT_EMAIL}");
    if probe.succeeded() {
        let unresolved = unresolved_marker_paths(shell, workspace_path)?;
        if !unresolved.is_empty() {
            return Err(format!(
                "refusing to conclude the in-progress merge: conflict markers \
                 remain in [{}] — the merge stays in progress; resolve every \
                 marker and the machinery will conclude it",
                unresolved.join(", ")
            ));
        }
        require_git_ok(shell, workspace_path, &["add", "-A"], "git add -A")?;
        require_git_ok(
            shell,
            workspace_path,
            &["-c", &user_name, "-c", &user_email, "commit", "--no-edit"],
            "git commit --no-edit (conclude merge)",
        )?;
        return Ok(ConcludeOutcome::Concluded {
            commit: rev_parse_head(shell, workspace_path)?,
        });
    }
    let status = require_git_ok(
        shell,
        workspace_path,
        &["status", "--porcelain"],
        "git status (remediation dirty check)",
    )?;
    if status.stdout.trim().is_empty() {
        return Ok(ConcludeOutcome::Skipped {
            head: rev_parse_head(shell, workspace_path)?,
        });
    }
    require_git_ok(shell, workspace_path, &["add", "-A"], "git add -A")?;
    require_git_ok(
        shell,
        workspace_path,
        &[
            "-c",
            &user_name,
            "-c",
            &user_email,
            "commit",
            "-m",
            "fix(staged): remediation",
        ],
        "git commit (remediation fix)",
    )?;
    Ok(ConcludeOutcome::Committed {
        commit: rev_parse_head(shell, workspace_path)?,
    })
}

/// The still-conflicted files of the in-progress merge (read BEFORE any
/// staging — `git add -A` would erase the unmerged index entries this
/// reads) whose worktree content still contains conflict markers. A file
/// deleted from the worktree is a resolution (the delete side was chosen),
/// not a marker hit.
///
/// The listing uses `-z` (NUL-separated, quoting disabled): without it,
/// git C-quotes unusual pathnames (non-ASCII under the default
/// `core.quotepath=true`, plus quotes/backslashes/control chars), the
/// quoted string never exists on disk, and the scan would misread the
/// file as resolved-by-deletion — committing its literal conflict markers.
fn unresolved_marker_paths(shell: &Shell, workspace_path: &str) -> Result<Vec<String>, String> {
    let conflicted = require_git_ok(
        shell,
        workspace_path,
        &["diff", "--name-only", "--diff-filter=U", "-z"],
        "git diff --diff-filter=U (conclusion marker scan)",
    )?;
    let mut unresolved = Vec::new();
    for path in conflicted
        .stdout
        .split('\0')
        .filter(|path| !path.is_empty())
    {
        let absolute = std::path::Path::new(workspace_path).join(path);
        if !absolute.exists() {
            continue;
        }
        let bytes = std::fs::read(&absolute).map_err(|error| {
            format!("could not read conflicted file {path} for the marker scan: {error}")
        })?;
        if contains_conflict_markers(&bytes) {
            unresolved.push(path.to_owned());
        }
    }
    Ok(unresolved)
}

/// Whether any line starts with an unambiguous git conflict marker:
/// `<<<<<<<`, `|||||||`, or `>>>>>>>`. A bare `=======` line is NOT
/// matched (it is legitimate content — setext underlines, comment rules)
/// and an unresolved conflict region always retains the other markers.
fn contains_conflict_markers(bytes: &[u8]) -> bool {
    bytes.split(|byte| *byte == b'\n').any(|line| {
        line.starts_with(b"<<<<<<<") || line.starts_with(b"|||||||") || line.starts_with(b">>>>>>>")
    })
}

fn stage_all_and_commit(
    shell: &Shell,
    workspace_path: &str,
    name: &str,
    email: &str,
    message: &str,
    skip_reason: &str,
) -> Result<FixCommitOutcome, String> {
    require_git_ok(shell, workspace_path, &["add", "-A"], "git add -A")?;

    let staged = staged_paths(shell, workspace_path)?;
    if staged.is_empty() {
        return Ok(FixCommitOutcome::Skipped {
            head: rev_parse_head(shell, workspace_path)?,
            reason: skip_reason.to_owned(),
        });
    }

    let full_message = format!(
        "{message}\n\nStaged paths:\n{}",
        staged
            .iter()
            .map(|path| format!("  {path}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let user_name = format!("user.name={name}");
    let user_email = format!("user.email={email}");
    require_git_ok(
        shell,
        workspace_path,
        &[
            "-c",
            &user_name,
            "-c",
            &user_email,
            "commit",
            "-m",
            &full_message,
        ],
        "git commit (mechanical)",
    )?;

    Ok(FixCommitOutcome::Committed {
        commit: rev_parse_head(shell, workspace_path)?,
        paths: staged,
    })
}

/// The currently staged paths (index vs HEAD).
fn staged_paths(shell: &Shell, workspace_path: &str) -> Result<Vec<String>, String> {
    let staged = require_git_ok(
        shell,
        workspace_path,
        &["diff", "--cached", "--name-only"],
        "git diff --cached (staged set)",
    )?;
    Ok(staged
        .stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

fn rev_parse_head(shell: &Shell, workspace_path: &str) -> Result<String, String> {
    let head = require_git_ok(
        shell,
        workspace_path,
        &["rev-parse", "HEAD"],
        "git rev-parse HEAD",
    )?;
    Ok(head.stdout.trim().to_owned())
}

/// Run one git command in the workspace; unrunnable OR non-zero is an error
/// carrying the context and captured output (the commit steps have no
/// recorded-data exits — every red is a fault).
fn require_git_ok(
    shell: &Shell,
    workspace_path: &str,
    args: &[&str],
    context: &str,
) -> Result<CliRun, String> {
    let run = shell
        .run("git", args, workspace_path)
        .map_err(|failure| format!("{context}: {}", failure.message()))?;
    if run.succeeded() {
        Ok(run)
    } else {
        Err(format!(
            "{context} exited {}: {}",
            run.exit_status,
            run.output.trim()
        ))
    }
}
