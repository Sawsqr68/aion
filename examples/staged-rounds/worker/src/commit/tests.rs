//! The staging rule, proven BY EXECUTION against real git repositories in
//! tempdirs. The regression pin for the measured defect is
//! [`build_output_is_excluded_and_recorded_by_name`]: before the fence, a gate
//! battery's `target/` went into the item's commit and both review lenses
//! correctly rejected the item for breaching its own scope fence, every round,
//! forever.

use super::{
    ConcludeOutcome, FixCommitOutcome, ScopeFence, commit_dev_work, commit_gate_normalization,
    commit_integration_fix, conclude_merge, exclusion_note, survey_worktree,
};
use crate::shell::Shell;

// --- fixtures -----------------------------------------------------------------

/// Run one git command in `dir`, failing the test loudly on any refusal.
fn git(dir: &str, args: &[&str]) -> anyhow::Result<String> {
    let run = Shell::inherited()
        .run("git", args, dir)
        .map_err(|failure| anyhow::anyhow!(failure.message()))?;
    anyhow::ensure!(
        run.succeeded(),
        "git {args:?} exited {}: {}",
        run.exit_status,
        run.output
    );
    Ok(run.stdout.trim().to_owned())
}

/// A real repo on `main` with one commit carrying `src/kept.rs`, and git's
/// DEFAULT pathname quoting pinned on so these repos exhibit the C-quoted
/// listings the `-z` handling must survive even when a host gitconfig turns
/// quoting off.
fn repo() -> anyhow::Result<(tempfile::TempDir, String)> {
    let dir = tempfile::tempdir()?;
    let root = dir.path().display().to_string();
    git(&root, &["init", "-b", "main"])?;
    git(&root, &["config", "user.name", "test"])?;
    git(&root, &["config", "user.email", "test@example.com"])?;
    git(&root, &["config", "core.quotepath", "true"])?;
    write(&dir, "src/kept.rs", "fn kept() {}\n")?;
    git(&root, &["add", "-A"])?;
    git(&root, &["commit", "-m", "base"])?;
    Ok((dir, root))
}

/// Write `content` at `relative`, creating parent directories.
fn write(dir: &tempfile::TempDir, relative: &str, content: &str) -> anyhow::Result<()> {
    let path = dir.path().join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}

/// The item fence most of these tests commit under: `src/` only.
fn src_fence() -> ScopeFence {
    ScopeFence::new(&["src/".to_owned()])
}

/// The paths a commit outcome carries, or a loud failure when it skipped.
fn committed_paths(outcome: &FixCommitOutcome) -> anyhow::Result<Vec<String>> {
    match outcome {
        FixCommitOutcome::Committed { paths, .. } => Ok(paths.clone()),
        FixCommitOutcome::Skipped { reason, .. } => {
            anyhow::bail!("expected a commit, got a skip: {reason}")
        }
    }
}

// --- the measured defect ------------------------------------------------------

/// THE REGRESSION PIN. A repository that does not gitignore its build output
/// runs a gate battery, `cargo build` leaves an untracked `target/`, and the
/// dev commit must neither carry it nor be silent about it.
#[test]
fn build_output_is_excluded_and_recorded_by_name() -> anyhow::Result<()> {
    let (dir, root) = repo()?;
    // The dev agent's real work, and the battery's build output beside it.
    write(&dir, "src/kept.rs", "fn kept() { }\n")?;
    write(&dir, "src/added.rs", "fn added() {}\n")?;
    write(&dir, "target/debug/libthing.rlib", "binary junk\n")?;
    write(&dir, "target/CACHEDIR.TAG", "Signature: 8a477f597d28d\n")?;

    let outcome = commit_dev_work(&Shell::inherited(), &root, "it-a", &src_fence())
        .map_err(anyhow::Error::msg)?;

    let paths = committed_paths(&outcome)?;
    assert_eq!(
        paths,
        vec!["src/added.rs".to_owned(), "src/kept.rs".to_owned()],
        "the commit must carry the item's work and NOTHING under target/"
    );
    // Proven by execution: the commit git actually made carries no build output.
    let committed = git(&root, &["show", "--stat", "--format=", "HEAD"])?;
    assert!(
        !committed.contains("target/"),
        "no build output may reach the branch: {committed}"
    );

    // And every excluded path is recorded BY NAME — not merely absent, not
    // merely counted, not merely logged.
    let mut excluded = outcome.excluded().to_vec();
    excluded.sort();
    assert_eq!(
        excluded,
        vec![
            "target/CACHEDIR.TAG".to_owned(),
            "target/debug/libthing.rlib".to_owned()
        ]
    );
    let note =
        exclusion_note("the dev-work commit", &src_fence(), outcome.excluded()).unwrap_or_default();
    assert!(
        note.contains("target/debug/libthing.rlib") && note.contains("target/CACHEDIR.TAG"),
        "the recorded note must name every excluded path in full: {note}"
    );
    assert!(
        note.contains("scope_in = [src]"),
        "the note must say which fence decided: {note}"
    );
    Ok(())
}

// --- the rules ----------------------------------------------------------------

#[test]
fn an_untracked_file_inside_the_fence_is_committed() -> anyhow::Result<()> {
    let (dir, root) = repo()?;
    write(&dir, "src/nested/deep.rs", "fn deep() {}\n")?;

    let outcome = commit_dev_work(&Shell::inherited(), &root, "it-a", &src_fence())
        .map_err(anyhow::Error::msg)?;

    assert_eq!(committed_paths(&outcome)?, vec!["src/nested/deep.rs"]);
    assert!(outcome.excluded().is_empty());
    assert_eq!(
        git(&root, &["show", "HEAD:src/nested/deep.rs"])?,
        "fn deep() {}"
    );
    Ok(())
}

/// RULE 1. A change to a file that already exists is real work on real source,
/// and it is staged wherever it lives. Hiding an out-of-fence tracked change
/// would make the branch disagree with what the panel reviews — catching it is
/// the REVIEWERS' job, not the machinery's to conceal.
#[test]
fn a_tracked_modification_outside_the_fence_is_still_committed() -> anyhow::Result<()> {
    let (dir, root) = repo()?;
    write(&dir, "docs/manual.md", "the manual\n")?;
    git(&root, &["add", "-A"])?;
    git(&root, &["commit", "-m", "add the manual"])?;
    // Now modify it — outside the item's `src/` fence.
    write(&dir, "docs/manual.md", "the manual, rewritten\n")?;

    let outcome = commit_dev_work(&Shell::inherited(), &root, "it-a", &src_fence())
        .map_err(anyhow::Error::msg)?;

    assert_eq!(committed_paths(&outcome)?, vec!["docs/manual.md"]);
    assert!(
        outcome.excluded().is_empty(),
        "a TRACKED change is never an exclusion: {:?}",
        outcome.excluded()
    );
    assert_eq!(
        git(&root, &["show", "HEAD:docs/manual.md"])?,
        "the manual, rewritten",
        "the reviewers must be able to see the fence breach on the branch"
    );
    Ok(())
}

/// A tracked DELETION outside the fence is staged for the same reason.
#[test]
fn a_tracked_deletion_outside_the_fence_is_still_committed() -> anyhow::Result<()> {
    let (dir, root) = repo()?;
    write(&dir, "docs/manual.md", "the manual\n")?;
    git(&root, &["add", "-A"])?;
    git(&root, &["commit", "-m", "add the manual"])?;
    std::fs::remove_file(dir.path().join("docs/manual.md"))?;

    let outcome = commit_dev_work(&Shell::inherited(), &root, "it-a", &src_fence())
        .map_err(anyhow::Error::msg)?;

    assert_eq!(committed_paths(&outcome)?, vec!["docs/manual.md"]);
    let tree = git(&root, &["ls-tree", "-r", "--name-only", "HEAD"])?;
    assert!(
        !tree.contains("docs/manual.md"),
        "the deletion must be on the branch: {tree}"
    );
    Ok(())
}

/// A `.gitignore`d file was never a candidate: `--exclude-standard` means it
/// appears in neither the commit nor the exclusion record. A repository that
/// already ignores its build output loses nothing whatever to this rule.
#[test]
fn a_gitignored_file_appears_in_neither_the_commit_nor_the_exclusions() -> anyhow::Result<()> {
    let (dir, root) = repo()?;
    write(&dir, ".gitignore", "/target\n")?;
    git(&root, &["add", "-A"])?;
    git(&root, &["commit", "-m", "ignore the build output"])?;
    write(&dir, "target/debug/libthing.rlib", "binary junk\n")?;
    write(&dir, "src/added.rs", "fn added() {}\n")?;

    let outcome = commit_dev_work(&Shell::inherited(), &root, "it-a", &src_fence())
        .map_err(anyhow::Error::msg)?;

    assert_eq!(committed_paths(&outcome)?, vec!["src/added.rs"]);
    assert!(
        outcome.excluded().is_empty(),
        "an IGNORED path is not an exclusion — it was never a candidate: {:?}",
        outcome.excluded()
    );
    assert_eq!(
        exclusion_note("the dev-work commit", &src_fence(), outcome.excluded()),
        None,
        "nothing excluded must render no note at all"
    );
    Ok(())
}

/// The empty fence stages tracked changes ONLY. This is what the integration
/// steps commit under.
#[test]
fn an_empty_fence_stages_tracked_changes_only() -> anyhow::Result<()> {
    let (dir, root) = repo()?;
    write(&dir, "src/kept.rs", "fn kept() { /* repaired */ }\n")?;
    write(&dir, "src/brand_new.rs", "fn brand_new() {}\n")?;

    let outcome = commit_integration_fix(&Shell::inherited(), &root).map_err(anyhow::Error::msg)?;

    assert_eq!(
        committed_paths(&outcome)?,
        vec!["src/kept.rs"],
        "the tracked repair is committed and the brand-new file is not"
    );
    assert_eq!(outcome.excluded(), ["src/brand_new.rs"]);
    Ok(())
}

/// A SKIP carries its exclusions too. "Nothing to commit" is the honest answer
/// only when there really was nothing there; a silent skip over a tree full of
/// build output is the same defect wearing a different hat.
#[test]
fn a_skipped_commit_still_records_what_it_excluded() -> anyhow::Result<()> {
    let (dir, root) = repo()?;
    write(&dir, "target/debug/libthing.rlib", "binary junk\n")?;
    let head_before = git(&root, &["rev-parse", "HEAD"])?;

    let outcome = commit_gate_normalization(&Shell::inherited(), &root, &src_fence())
        .map_err(anyhow::Error::msg)?;

    let FixCommitOutcome::Skipped {
        head,
        reason,
        excluded,
    } = &outcome
    else {
        anyhow::bail!("a tree holding only build output must NOT produce a commit: {outcome:?}");
    };
    assert_eq!(head, &head_before, "the branch head must be unchanged");
    assert!(reason.contains("clean"), "{reason}");
    assert_eq!(excluded, &["target/debug/libthing.rlib".to_owned()]);
    assert!(
        exclusion_note("the gate-normalization commit", &src_fence(), excluded).is_some(),
        "a skip with exclusions must still render a note"
    );
    Ok(())
}

/// `-z` is load-bearing: under the default `core.quotepath=true`, git prints
/// `café.txt` as the C-quoted literal `"caf\303\251.txt"`, a string that names
/// no file on disk. A listing parsed without `-z` would try to stage that
/// literal and fail, or record a path nobody can find.
#[test]
fn an_untracked_non_ascii_or_quote_bearing_path_is_handled_literally() -> anyhow::Result<()> {
    let (dir, root) = repo()?;
    write(&dir, "src/café.txt", "inside the fence\n")?;
    write(&dir, "src/with\"quote.txt", "also inside\n")?;
    write(&dir, "prosé.txt", "outside the fence\n")?;

    let outcome = commit_dev_work(&Shell::inherited(), &root, "it-a", &src_fence())
        .map_err(anyhow::Error::msg)?;

    let mut paths = committed_paths(&outcome)?;
    paths.sort();
    assert_eq!(
        paths,
        vec!["src/café.txt".to_owned(), "src/with\"quote.txt".to_owned()],
        "the real on-disk names must be staged, never their C-quoted renderings"
    );
    assert_eq!(outcome.excluded(), ["prosé.txt".to_owned()]);
    // Proven by execution: git really holds those paths at HEAD.
    assert_eq!(
        git(&root, &["show", "HEAD:src/café.txt"])?,
        "inside the fence"
    );
    assert_eq!(
        git(&root, &["show", "HEAD:src/with\"quote.txt"])?,
        "also inside"
    );
    Ok(())
}

/// A repo-root path beginning with `-` must reach git as a path, never as an
/// option — hence the `--` separator on the staging call. A path containing
/// `*` must be staged as ITSELF and never glob a second file in — hence the
/// `:(literal)` pathspec magic.
#[test]
fn an_untracked_path_that_looks_like_an_option_or_a_glob_is_staged_literally() -> anyhow::Result<()>
{
    let (dir, root) = repo()?;
    write(&dir, "--force.txt", "not an option\n")?;
    write(&dir, "star*.txt", "not a glob\n")?;
    // The decoy a glob-interpreted `star*.txt` pathspec would sweep in.
    write(&dir, "starburst.txt", "must not be swept in\n")?;
    let fence = ScopeFence::new(&["--force.txt".to_owned(), "star*.txt".to_owned()]);

    let outcome =
        commit_dev_work(&Shell::inherited(), &root, "it-a", &fence).map_err(anyhow::Error::msg)?;

    let mut paths = committed_paths(&outcome)?;
    paths.sort();
    assert_eq!(
        paths,
        vec!["--force.txt".to_owned(), "star*.txt".to_owned()],
        "each fenced path is staged as itself and nothing else"
    );
    assert_eq!(
        outcome.excluded(),
        ["starburst.txt".to_owned()],
        "the glob decoy stays untracked, and is recorded"
    );
    assert_eq!(git(&root, &["show", "HEAD:--force.txt"])?, "not an option");
    Ok(())
}

/// The fence is a path fence, not a glob: `src` must not admit `srcery/`, and
/// a `*` in an entry matches nothing rather than everything.
#[test]
fn the_fence_matches_paths_not_prefixes_and_never_globs() {
    let fence = ScopeFence::new(&[
        "src".to_owned(),
        " ./docs/ ".to_owned(),
        "  ".to_owned(),
        "/lib/thing.rs".to_owned(),
    ]);
    assert!(fence.admits("src"));
    assert!(fence.admits("src/deep/nested.rs"));
    assert!(!fence.admits("srcery/other.rs"), "a prefix is not a parent");
    assert!(fence.admits("docs/manual.md"), "./docs/ normalises to docs");
    assert!(
        fence.admits("lib/thing.rs"),
        "/lib/... normalises to lib/..."
    );
    assert!(!fence.admits(""), "a blank entry must have been dropped");

    let globbed = ScopeFence::new(&["src/*.rs".to_owned()]);
    assert!(
        !globbed.admits("src/added.rs"),
        "there is no globbing — an entry that does not match admits nothing"
    );
    assert!(globbed.admits("src/*.rs"), "it is matched literally");

    assert_eq!(
        ScopeFence::tracked_only().describe(),
        "tracked-only (this step adopts no untracked file at all)"
    );
    assert!(!ScopeFence::tracked_only().admits("anything"));
}

// --- the survey the gate battery's clean assertion reads ----------------------

/// The survey answers the SAME question the commit does, so the battery's
/// re-dirty assertion can never disagree with what the commit staged.
#[test]
fn the_survey_counts_tracked_and_in_fence_changes_but_not_excluded_ones() -> anyhow::Result<()> {
    let (dir, root) = repo()?;
    write(&dir, "target/debug/libthing.rlib", "binary junk\n")?;

    let survey =
        survey_worktree(&Shell::inherited(), &root, &src_fence()).map_err(anyhow::Error::msg)?;
    assert!(
        !survey.diverges(),
        "excluded build output must NOT read as a re-dirtied worktree — that \
         would fail every battery in the repositories the fence exists to unblock"
    );
    assert_eq!(survey.excluded(), ["target/debug/libthing.rlib".to_owned()]);
    assert!(survey.divergent().is_empty());

    // A tracked modification does diverge, and is named.
    write(&dir, "src/kept.rs", "fn kept() { /* churned */ }\n")?;
    let survey =
        survey_worktree(&Shell::inherited(), &root, &src_fence()).map_err(anyhow::Error::msg)?;
    assert!(survey.diverges());
    assert_eq!(survey.divergent(), vec!["src/kept.rs".to_owned()]);

    // So does an untracked file INSIDE the fence: the commit was supposed to
    // embody it, so its presence afterwards is a real divergence.
    let (dir, root) = repo()?;
    write(&dir, "src/late.rs", "fn late() {}\n")?;
    let survey =
        survey_worktree(&Shell::inherited(), &root, &src_fence()).map_err(anyhow::Error::msg)?;
    assert!(survey.diverges());
    assert_eq!(survey.divergent(), vec!["src/late.rs".to_owned()]);
    drop(dir);
    Ok(())
}

// --- the merge conclusion under the tracked-only rule -------------------------

/// The flagship remediation mechanic, with the case `git add -u` had to be
/// proven against: a merge that BRINGS IN a file the branch did not have. Such
/// a file has an index entry from the merge itself, so `-u` covers it — proven
/// here by reading the concluded merge commit back out of git.
#[test]
fn conclude_merge_concludes_a_merge_that_adds_a_file_the_branch_lacked() -> anyhow::Result<()> {
    let (dir, root) = repo()?;
    git(&root, &["checkout", "-q", "-b", "side"])?;
    write(&dir, "src/kept.rs", "fn kept() { /* side */ }\n")?;
    write(&dir, "src/only_on_side.rs", "fn only_on_side() {}\n")?;
    git(&root, &["add", "-A"])?;
    git(&root, &["commit", "-m", "side"])?;
    git(&root, &["checkout", "-q", "main"])?;
    write(&dir, "src/kept.rs", "fn kept() { /* main */ }\n")?;
    git(&root, &["add", "-A"])?;
    git(&root, &["commit", "-m", "main"])?;
    let merge = Shell::inherited()
        .run("git", &["merge", "side"], &root)
        .map_err(|failure| anyhow::anyhow!(failure.message()))?;
    anyhow::ensure!(
        !merge.succeeded(),
        "the fixture merge should have conflicted"
    );

    // The remediator resolves in place and runs no git; a stray untracked file
    // sits beside the resolution and must NOT be adopted into the merge.
    write(&dir, "src/kept.rs", "fn kept() { /* resolved */ }\n")?;
    write(&dir, "target/debug/libthing.rlib", "binary junk\n")?;

    let outcome = conclude_merge(&Shell::inherited(), &root).map_err(anyhow::Error::msg)?;
    let ConcludeOutcome::Concluded { commit, excluded } = &outcome else {
        anyhow::bail!("expected the merge to be concluded, got {outcome:?}");
    };
    assert!(!commit.is_empty());
    assert_eq!(excluded, &["target/debug/libthing.rlib".to_owned()]);

    // Proven by execution: the merge really concluded, the incoming-only file
    // is on the branch, and the build output is not.
    let probe = Shell::inherited()
        .run("git", &["rev-parse", "-q", "--verify", "MERGE_HEAD"], &root)
        .map_err(|failure| anyhow::anyhow!(failure.message()))?;
    assert!(!probe.succeeded(), "MERGE_HEAD must be gone");
    assert_eq!(
        git(&root, &["show", "HEAD:src/only_on_side.rs"])?,
        "fn only_on_side() {}",
        "a file the branch did not have, brought in by the merge, must be \
         concluded with it — `git add -u` covers it because the merge gave it \
         an index entry"
    );
    assert_eq!(
        git(&root, &["show", "HEAD:src/kept.rs"])?,
        "fn kept() { /* resolved */ }"
    );
    let tree = git(&root, &["ls-tree", "-r", "--name-only", "HEAD"])?;
    assert!(!tree.contains("target/"), "{tree}");
    Ok(())
}

/// An add/add conflict — the same NEW path on both sides — is resolved by
/// `git add -u` as well: the unmerged entry is an index entry.
#[test]
fn conclude_merge_resolves_an_add_add_conflict() -> anyhow::Result<()> {
    let (dir, root) = repo()?;
    git(&root, &["checkout", "-q", "-b", "side"])?;
    write(&dir, "src/both.rs", "fn both() { /* side */ }\n")?;
    git(&root, &["add", "-A"])?;
    git(&root, &["commit", "-m", "side"])?;
    git(&root, &["checkout", "-q", "main"])?;
    write(&dir, "src/both.rs", "fn both() { /* main */ }\n")?;
    git(&root, &["add", "-A"])?;
    git(&root, &["commit", "-m", "main"])?;
    let merge = Shell::inherited()
        .run("git", &["merge", "side"], &root)
        .map_err(|failure| anyhow::anyhow!(failure.message()))?;
    anyhow::ensure!(
        !merge.succeeded(),
        "the fixture merge should have conflicted"
    );

    write(&dir, "src/both.rs", "fn both() { /* resolved */ }\n")?;
    let outcome = conclude_merge(&Shell::inherited(), &root).map_err(anyhow::Error::msg)?;
    let ConcludeOutcome::Concluded { .. } = &outcome else {
        anyhow::bail!("expected the add/add merge to be concluded, got {outcome:?}");
    };
    assert_eq!(
        git(&root, &["show", "HEAD:src/both.rs"])?,
        "fn both() { /* resolved */ }"
    );
    Ok(())
}

/// A tree holding NOTHING but untracked build output and no merge in progress
/// is a skip that still names what it declined to take — and it must never be
/// sent to `git commit`, which would fail the activity on a tree the machinery
/// had correctly decided to leave alone.
#[test]
fn conclude_merge_skips_over_untracked_only_and_records_it() -> anyhow::Result<()> {
    let (dir, root) = repo()?;
    write(&dir, "target/debug/libthing.rlib", "binary junk\n")?;
    let head_before = git(&root, &["rev-parse", "HEAD"])?;

    let outcome = conclude_merge(&Shell::inherited(), &root).map_err(anyhow::Error::msg)?;
    let ConcludeOutcome::Skipped { head, excluded } = &outcome else {
        anyhow::bail!("expected a skip, got {outcome:?}");
    };
    assert_eq!(head, &head_before);
    assert_eq!(excluded, &["target/debug/libthing.rlib".to_owned()]);
    Ok(())
}
