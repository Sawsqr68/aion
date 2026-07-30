//! The three stage-and-commit steps, their scoped machinery identities, and
//! the outcome they record.
//!
//! Each commits under its OWN `-c user.name`/`-c user.email` so the branch
//! history says exactly which piece of machinery produced it, without touching
//! the workspace's git config, and each is idempotent across activity retries
//! (nothing staged skips green with the unchanged head).

use super::fence::ScopeFence;
use super::staging::{require_git_ok, rev_parse_head, stage_fenced, staged_paths};
use crate::shell::Shell;

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
///
/// BOTH variants also carry `excluded`. A skip is exactly where a silent drop
/// would hide best — "nothing to commit" is the honest answer only if the
/// thing that was in the tree really was nobody's work — so the untracked
/// paths the fence turned away are recorded on the skip as fully as on the
/// commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FixCommitOutcome {
    /// The work was committed.
    Committed {
        /// The created commit's hash (the branch head afterwards).
        commit: String,
        /// The staged paths the commit carries (from git, not from claims).
        paths: Vec<String>,
        /// The untracked paths the fence excluded, by name.
        excluded: Vec<String>,
    },
    /// Nothing was staged; the branch head is unchanged.
    Skipped {
        /// The branch head at skip time.
        head: String,
        /// Why nothing was committed.
        reason: String,
        /// The untracked paths the fence excluded, by name. Non-empty here
        /// means the step found something in the tree and deliberately did
        /// not take it — never "there was nothing there".
        excluded: Vec<String>,
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

    /// The untracked paths this step excluded, by name.
    #[must_use]
    pub fn excluded(&self) -> &[String] {
        match self {
            Self::Committed { excluded, .. } | Self::Skipped { excluded, .. } => excluded,
        }
    }
}

/// Commit the dev round's work in `workspace_path` on the item branch under
/// the scoped machinery identity, message `chore(staged dev): item <id> round
/// work`. Nothing staged skips green with the unchanged head — idempotent
/// across activity retries and honest for a round that changed nothing.
///
/// `fence` is the work item's own `scope_in`. The dev agent legitimately
/// creates new files and the item says where, so this is the one step whose
/// fence is not empty by construction; anything untracked outside it (a gate
/// battery's `target/` above all) is left alone and recorded.
///
/// # Errors
///
/// `git` unable to run, or any staging/commit command exiting non-zero.
pub fn commit_dev_work(
    shell: &Shell,
    workspace_path: &str,
    item_id: &str,
    fence: &ScopeFence,
) -> Result<FixCommitOutcome, String> {
    stage_and_commit(
        shell,
        workspace_path,
        fence,
        DEVELOPER_COMMIT_NAME,
        DEVELOPER_COMMIT_EMAIL,
        &format!("chore(staged dev): item {item_id} round work"),
        "no changes in the worktree; nothing to commit (already committed, \
         or the round changed nothing)",
    )
}

/// Commit whatever the GATE COMMANDS left in `workspace_path` under the gate
/// machinery identity, message `chore(staged gates): mechanical normalization
/// from gate commands`.
///
/// A write-mode formatter used as a gate (Tom's standing rule — never
/// format-CHECK, always autoformat) leaves normalization the dev agent never
/// saw. That change belongs on the branch: without this commit the branch is
/// incomplete, the panel's diff disagrees with the branch head, and the
/// item's next re-provisioning refuses a dirty worktree. A clean tree skips
/// green with the unchanged head.
///
/// `fence` is the SAME item's `scope_in`. The legitimate normalization case is
/// a formatter rewriting files that already exist, which the tracked half of
/// the rule covers unconditionally; a gate command that CREATES a new file
/// outside the item's declared fence is build output far more often than it is
/// work, so it is left untracked and named in the record for the operator to
/// judge.
///
/// # Errors
///
/// `git` unable to run, or any staging/commit command exiting non-zero.
pub fn commit_gate_normalization(
    shell: &Shell,
    workspace_path: &str,
    fence: &ScopeFence,
) -> Result<FixCommitOutcome, String> {
    stage_and_commit(
        shell,
        workspace_path,
        fence,
        GATES_COMMIT_NAME,
        GATES_COMMIT_EMAIL,
        "chore(staged gates): mechanical normalization from gate commands",
        "gate commands left the worktree clean; nothing to commit",
    )
}

/// Commit the integration repair the `fix_integration` agent made in the
/// integration worktree under the integration machinery identity, message
/// `fix(staged integration): repair the red integration battery`.
///
/// The agent runs no git, so without this the repair would be discarded by
/// the very next `verify_integration` (which asserts a clean tree). A clean
/// tree skips green: an agent turn that changed nothing is reported honestly
/// rather than manufacturing an empty commit.
///
/// TRACKED-ONLY, deliberately: this step repairs an existing UNION of already
/// accepted, already reviewed branches. There is no item and therefore no
/// declared intent to fence against, and a brand-new untracked file appearing
/// at integration time is not something this machinery should silently adopt
/// into the run's deliverable — the batteries that run here also leave build
/// output in the tree, and it is the same output for the same reason. Anything
/// untracked is recorded by name instead.
///
/// # Errors
///
/// `git` unable to run, or any staging/commit command exiting non-zero.
pub fn commit_integration_fix(
    shell: &Shell,
    workspace_path: &str,
) -> Result<FixCommitOutcome, String> {
    stage_and_commit(
        shell,
        workspace_path,
        &ScopeFence::tracked_only(),
        INTEGRATION_COMMIT_NAME,
        INTEGRATION_COMMIT_EMAIL,
        "fix(staged integration): repair the red integration battery",
        "the integration fix turn left the worktree clean; nothing to commit",
    )
}

/// Stage under the rule, then commit under a scoped identity — or skip green
/// when nothing was staged, still carrying what the fence turned away.
fn stage_and_commit(
    shell: &Shell,
    workspace_path: &str,
    fence: &ScopeFence,
    name: &str,
    email: &str,
    message: &str,
    skip_reason: &str,
) -> Result<FixCommitOutcome, String> {
    let excluded = stage_fenced(shell, workspace_path, fence)?;

    let staged = staged_paths(shell, workspace_path)?;
    if staged.is_empty() {
        return Ok(FixCommitOutcome::Skipped {
            head: rev_parse_head(shell, workspace_path)?,
            reason: skip_reason.to_owned(),
            excluded,
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
        excluded,
    })
}
