//! The remediator's mechanical step: conclude the in-progress merge it
//! resolved, commit a dirty tree as a remediation fix, or record a skip.

use super::fence::ScopeFence;
use super::staging::{require_git_ok, rev_parse_head, stage_fenced, staged_paths};
use super::steps::{REMEDIATION_COMMIT_EMAIL, REMEDIATION_COMMIT_NAME};
use crate::shell::Shell;

/// What [`conclude_merge`] did in the integration worktree. Every variant
/// carries the untracked paths the tracked-only fence turned away, for the
/// same reason [`super::FixCommitOutcome`] does: a step that declined to take
/// something must say so where the run's record can see it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConcludeOutcome {
    /// An in-progress merge was concluded with the remediator's resolutions.
    Concluded {
        /// The merge commit's hash.
        commit: String,
        /// The untracked paths the fence excluded, by name.
        excluded: Vec<String>,
    },
    /// No merge was in progress but there were tracked changes; the
    /// remediation was committed as a fix.
    Committed {
        /// The fix commit's hash.
        commit: String,
        /// The untracked paths the fence excluded, by name.
        excluded: Vec<String>,
    },
    /// No merge was in progress and nothing stageable was in the tree.
    Skipped {
        /// The head at skip time.
        head: String,
        /// The untracked paths the fence excluded, by name. Non-empty here is
        /// the case that most needs saying out loud: the tree was NOT empty,
        /// the machinery simply declined to adopt what was in it.
        excluded: Vec<String>,
    },
}

impl ConcludeOutcome {
    /// The untracked paths this step excluded, by name.
    #[must_use]
    pub fn excluded(&self) -> &[String] {
        match self {
            Self::Concluded { excluded, .. }
            | Self::Committed { excluded, .. }
            | Self::Skipped { excluded, .. } => excluded,
        }
    }
}

/// Conclude the remediator's turn mechanically: if a merge is in progress
/// (`MERGE_HEAD` resolves), verify every still-unmerged file is genuinely
/// resolved (no conflict markers left in the worktree), then stage under the
/// shared rule and `git commit --no-edit` (concluding the conflicted merge
/// with the agent's resolutions); otherwise stage and commit the remediation
/// fix, or record a skip when nothing stageable is there.
///
/// TRACKED-ONLY, deliberately, for the reason spelled out on
/// [`super::commit_integration_fix`]: this step concludes an existing union,
/// and `git add -u` is exactly right for it. A merge puts an index entry on
/// EVERY path it touches — including a file only the incoming branch had, and
/// including an add/add conflict resolved by hand — so `-u` stages the whole
/// resolution; the only thing it leaves behind is a file that is untracked on
/// both sides, which is not part of the merge and not the machinery's to
/// adopt. That is recorded by name rather than swept in.
///
/// The marker scan is load-bearing and runs BEFORE any staging: staging clears
/// the unmerged index entries, so without it a remediator that resolved some
/// (or zero) files would get the literal conflict markers COMMITTED as a
/// concluded merge — a silent wrong output on the run's flagship remediation
/// mechanic.
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
    let fence = ScopeFence::tracked_only();
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
        let excluded = stage_fenced(shell, workspace_path, &fence)?;
        require_git_ok(
            shell,
            workspace_path,
            &["-c", &user_name, "-c", &user_email, "commit", "--no-edit"],
            "git commit --no-edit (conclude merge)",
        )?;
        return Ok(ConcludeOutcome::Concluded {
            commit: rev_parse_head(shell, workspace_path)?,
            excluded,
        });
    }

    // No merge in progress. The dirty check is the STAGED SET rather than
    // `git status --porcelain`: under the tracked-only fence a tree holding
    // nothing but untracked build output stages nothing, and asking status
    // would send an empty commit to `git commit` and fail the activity on a
    // tree the machinery had correctly decided to leave alone.
    let excluded = stage_fenced(shell, workspace_path, &fence)?;
    if staged_paths(shell, workspace_path)?.is_empty() {
        return Ok(ConcludeOutcome::Skipped {
            head: rev_parse_head(shell, workspace_path)?,
            excluded,
        });
    }
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
        excluded,
    })
}

/// The still-conflicted files of the in-progress merge (read BEFORE any
/// staging — staging would erase the unmerged index entries this reads) whose
/// worktree content still contains conflict markers. A file deleted from the
/// worktree is a resolution (the delete side was chosen), not a marker hit.
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
