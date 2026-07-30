//! The staging core every mechanical commit shares, and the git plumbing the
//! steps run on.
//!
//! ONE staging rule, applied identically at every commit site:
//!
//! 1. `git add -u --` stages every TRACKED modification and deletion across
//!    the whole worktree, fence or no fence (see [`super::ScopeFence`] for
//!    why hiding a tracked change would be worse than recording it).
//! 2. Every UNTRACKED file the fence admits is staged explicitly.
//! 3. Every untracked file the fence does NOT admit is returned BY NAME, so
//!    the caller can put it in the activity's recorded outcome. Nothing is
//!    dropped silently — not on a commit, and not on a skip.
//!
//! `git ls-files --others --exclude-standard -z` is the untracked listing, and
//! both flags are load-bearing:
//!
//! - `--exclude-standard` respects `.gitignore`, so a repository that already
//!   ignores its build output loses nothing to this rule and the fence only
//!   ever bites on what git itself considers untracked-and-not-ignored. An
//!   ignored file is not excluded — it was never a candidate — so it appears
//!   in neither the commit nor the exclusion record.
//! - `-z` (NUL-separated, quoting disabled) for exactly the reason the merge
//!   conclusion's conflict-marker scan needs it: without it git C-quotes
//!   unusual pathnames (non-ASCII under the default `core.quotepath=true`,
//!   plus quotes, backslashes and control characters), and the quoted string
//!   names no file on disk. Staging it would fail, and recording it would name
//!   a path nobody can find.

use super::ScopeFence;
use crate::shell::{CliRun, Shell};

/// What the staging rule WOULD do to a worktree right now, asked without
/// staging anything: the tracked changes it would stage, the untracked files
/// the fence admits, and the untracked files it does not.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorktreeSurvey {
    tracked: Vec<String>,
    admitted: Vec<String>,
    excluded: Vec<String>,
}

impl WorktreeSurvey {
    /// Everything the staging rule would still stage — tracked changes plus
    /// fence-admitted untracked files. Non-empty means the worktree does NOT
    /// equal the branch head in any way the machinery was supposed to embody.
    #[must_use]
    pub fn divergent(&self) -> Vec<String> {
        let mut paths = self.tracked.clone();
        paths.extend(self.admitted.iter().cloned());
        paths
    }

    /// Whether the worktree diverges from what the last mechanical commit was
    /// supposed to embody. Untracked files OUTSIDE the fence deliberately do
    /// NOT count: they were never going to be committed, so calling them a
    /// divergence would fail every battery in a repository whose build output
    /// is not gitignored — the very defect the fence exists to fix.
    #[must_use]
    pub fn diverges(&self) -> bool {
        !self.tracked.is_empty() || !self.admitted.is_empty()
    }

    /// The untracked paths the fence does not admit, by name.
    #[must_use]
    pub fn excluded(&self) -> &[String] {
        &self.excluded
    }
}

/// Survey `workspace_path` against `fence` WITHOUT staging anything.
///
/// The tracked half is `git diff --name-only -z HEAD`: index-and-worktree
/// against the branch head, which is precisely the question "does this
/// directory still equal the commit the panel will read".
///
/// # Errors
///
/// `git` unable to run, or either listing exiting non-zero.
pub fn survey_worktree(
    shell: &Shell,
    workspace_path: &str,
    fence: &ScopeFence,
) -> Result<WorktreeSurvey, String> {
    let tracked = require_git_ok(
        shell,
        workspace_path,
        &["diff", "--name-only", "-z", "HEAD", "--"],
        "git diff --name-only HEAD (tracked divergence survey)",
    )?;
    let (admitted, excluded) = partition_untracked(shell, workspace_path, fence)?;
    Ok(WorktreeSurvey {
        tracked: nul_paths(&tracked.stdout),
        admitted,
        excluded,
    })
}

/// Apply the staging rule in `workspace_path`, returning the untracked paths
/// that were EXCLUDED by `fence` (empty when nothing was excluded).
///
/// # Errors
///
/// `git` unable to run, or any listing/staging command exiting non-zero.
pub(super) fn stage_fenced(
    shell: &Shell,
    workspace_path: &str,
    fence: &ScopeFence,
) -> Result<Vec<String>, String> {
    // The partition is read BEFORE anything is staged: `git add` moves files
    // out of the untracked listing, so a survey taken afterwards could not say
    // what was admitted and what was dropped.
    let (admitted, excluded) = partition_untracked(shell, workspace_path, fence)?;

    // Rule 1. `-u` with no pathspec is the whole worktree, not the current
    // directory (git 2.0 and later); the trailing `--` states that explicitly
    // and keeps the argument list unambiguous.
    require_git_ok(shell, workspace_path, &["add", "-u", "--"], "git add -u")?;

    // Rule 2. Named one by one under `:(literal)` magic so a real filename
    // containing `*`, `?` or `[` is staged as itself and can never glob a
    // second file in, and after `--` so a filename beginning with `-` is never
    // read as an option. A pathological worktree could make this argument list
    // exceed the operating system's limit; that surfaces as a loud spawn
    // failure from the shell boundary, never as a quietly partial commit.
    if !admitted.is_empty() {
        let pathspecs: Vec<String> = admitted
            .iter()
            .map(|path| format!(":(literal){path}"))
            .collect();
        let mut args: Vec<&str> = vec!["add", "--"];
        args.extend(pathspecs.iter().map(String::as_str));
        require_git_ok(
            shell,
            workspace_path,
            &args,
            "git add (untracked files inside the fence)",
        )?;
    }

    Ok(excluded)
}

/// The untracked-and-not-ignored files of `workspace_path`, split into the
/// ones `fence` admits and the ones it does not.
fn partition_untracked(
    shell: &Shell,
    workspace_path: &str,
    fence: &ScopeFence,
) -> Result<(Vec<String>, Vec<String>), String> {
    let listing = require_git_ok(
        shell,
        workspace_path,
        &["ls-files", "--others", "--exclude-standard", "-z"],
        "git ls-files --others (untracked survey)",
    )?;
    Ok(nul_paths(&listing.stdout)
        .into_iter()
        .partition(|path| fence.admits(path)))
}

/// The ONE rendering of an exclusion record: [`None`] when nothing was
/// excluded, otherwise a note naming the step, the fence that decided, and
/// EVERY excluded path in full.
///
/// Nothing here is summarised or elided. A count alone would let build output
/// and a legitimately misplaced source file look identical in the record, and
/// the whole point of the rule is that what the machinery declined to commit
/// is visible to the operator and to the review lenses.
#[must_use]
pub fn exclusion_note(step: &str, fence: &ScopeFence, excluded: &[String]) -> Option<String> {
    if excluded.is_empty() {
        return None;
    }
    let listed = excluded
        .iter()
        .map(|path| format!("  {path}"))
        .collect::<Vec<_>>()
        .join("\n");
    Some(format!(
        "{step}: {} untracked path(s) were NOT staged and are therefore not on \
         the branch — they fall outside this step's staging fence, {}. Tracked \
         changes are always committed, so this list is new files only. Nothing \
         is dropped silently; every excluded path, in full:\n{listed}",
        excluded.len(),
        fence.describe()
    ))
}

/// Split a git `-z` listing into its paths. `-z` output is NUL-TERMINATED, so
/// the final field is empty and is dropped along with any other empty.
fn nul_paths(stdout: &str) -> Vec<String> {
    stdout
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The currently staged paths (index vs HEAD), `-z` for the same pathname
/// reason as every other listing here: these names ride in the commit message
/// and in the activity's recorded outcome, and a C-quoted one would name a
/// file that does not exist.
pub(super) fn staged_paths(shell: &Shell, workspace_path: &str) -> Result<Vec<String>, String> {
    let staged = require_git_ok(
        shell,
        workspace_path,
        &["diff", "--cached", "--name-only", "-z"],
        "git diff --cached (staged set)",
    )?;
    Ok(nul_paths(&staged.stdout))
}

/// The branch head.
///
/// # Errors
///
/// `git` unable to run, or `rev-parse` exiting non-zero.
pub(super) fn rev_parse_head(shell: &Shell, workspace_path: &str) -> Result<String, String> {
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
pub(super) fn require_git_ok(
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
