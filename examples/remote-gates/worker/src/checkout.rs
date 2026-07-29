//! Resolving a repository and branch into a working tree on the gate host.
//!
//! Reuse is deliberate: an existing checkout of the SAME remote is fetched,
//! hard-reset onto the requested branch, and cleaned with `git clean -fd` — note
//! the absent `-x`, which is what keeps `.gitignore`d build caches (`target/`)
//! alive between runs and makes the gates incremental. A path holding a DIFFERENT
//! remote is a refusal, never a silent re-clone over somebody's checkout.
//!
//! Every step here is fatal on failure: you cannot gate what you could not
//! check out, and the workflow declares no failure arm for this step.

use std::path::{Path, PathBuf};

use aion_worker::{ActivityContext, ActivityFailure};

use crate::logs::log_path;
use crate::process::{CommandRun, CommandSpec, run_logged};
use crate::settings::Settings;
use crate::types::{Checkout, CloneInput};

/// Version-control tool, resolved from `PATH` on the gate host.
const GIT: &str = "git";
/// POSIX disk-free reporter, resolved from `PATH` on the gate host.
const DF: &str = "df";
/// Field index of "available 1K blocks" in POSIX `df -P` output.
const DF_AVAILABLE_FIELD: usize = 3;
/// Bytes per block in `df -k` output.
const DF_BLOCK_BYTES: i64 = 1024;

/// Clones or refreshes `repo` at `branch` and reports the resolved tree.
///
/// # Errors
///
/// Returns [`ActivityFailure`] when the inputs are not usable as a repository
/// and branch, when the target path holds a different remote, when any Git step
/// exits non-zero, or when free disk space cannot be read.
pub async fn clone_repo(
    input: CloneInput,
    settings: &Settings,
    context: &ActivityContext,
) -> Result<Checkout, ActivityFailure> {
    let repo = validated_repo(&input.repo)?;
    let branch = validated_branch(&input.branch)?;
    let name = repository_name(repo)?;
    let checkouts = settings.checkout_dir();
    std::fs::create_dir_all(&checkouts).map_err(|error| {
        ActivityFailure::terminal(format!(
            "checkout root {} could not be created: {error}",
            checkouts.display()
        ))
    })?;
    let target = checkouts.join(&name);

    let fetched_fresh = if target.exists() {
        refresh(&target, repo, branch, settings, context).await?;
        false
    } else {
        clone(&checkouts, &target, repo, branch, settings, context).await?;
        true
    };

    let commit = resolved_commit(&target, settings, context).await?;
    let free_disk_bytes = free_disk_bytes(&target, settings, context).await?;
    Ok(Checkout {
        path: target.display().to_string(),
        commit,
        branch: branch.to_owned(),
        fetched_fresh,
        free_disk_bytes,
    })
}

async fn clone(
    checkouts: &Path,
    target: &Path,
    repo: &str,
    branch: &str,
    settings: &Settings,
    context: &ActivityContext,
) -> Result<(), ActivityFailure> {
    step(
        Step {
            name: "clone",
            program: GIT,
            args: vec![
                "clone".to_owned(),
                "--branch".to_owned(),
                branch.to_owned(),
                "--".to_owned(),
                repo.to_owned(),
                target.display().to_string(),
            ],
            cwd: checkouts,
            target,
        },
        settings,
        context,
    )
    .await
    .map(drop)
}

async fn refresh(
    target: &Path,
    repo: &str,
    branch: &str,
    settings: &Settings,
    context: &ActivityContext,
) -> Result<(), ActivityFailure> {
    if !target.join(".git").exists() {
        return Err(ActivityFailure::terminal(format!(
            "{} exists but is not a Git checkout; the gate host will not clone over it",
            target.display()
        )));
    }
    let origin = step(
        Step {
            name: "remote",
            program: GIT,
            args: vec![
                "remote".to_owned(),
                "get-url".to_owned(),
                "origin".to_owned(),
            ],
            cwd: target,
            target,
        },
        settings,
        context,
    )
    .await?;
    let existing = origin.stdout.trim();
    if existing != repo {
        return Err(ActivityFailure::terminal(format!(
            "{} already holds a checkout of `{existing}`, not `{repo}`; refusing to overwrite it",
            target.display()
        )));
    }
    let remote_branch = format!("origin/{branch}");
    for (name, args) in [
        (
            "fetch",
            vec![
                "fetch".to_owned(),
                "--prune".to_owned(),
                "origin".to_owned(),
                branch.to_owned(),
            ],
        ),
        (
            // `-B` so a branch this checkout has never seen still resolves,
            // pointed at the remote tip rather than at whatever was here.
            "checkout",
            vec![
                "checkout".to_owned(),
                "-B".to_owned(),
                branch.to_owned(),
                remote_branch.clone(),
            ],
        ),
        (
            "reset",
            vec![
                "reset".to_owned(),
                "--hard".to_owned(),
                remote_branch.clone(),
            ],
        ),
        // No `-x`: ignored build caches survive, so the gates stay incremental.
        ("clean", vec!["clean".to_owned(), "-fd".to_owned()]),
    ] {
        step(
            Step {
                name,
                program: GIT,
                args,
                cwd: target,
                target,
            },
            settings,
            context,
        )
        .await?;
    }
    Ok(())
}

async fn resolved_commit(
    target: &Path,
    settings: &Settings,
    context: &ActivityContext,
) -> Result<String, ActivityFailure> {
    let run = step(
        Step {
            name: "rev-parse",
            program: GIT,
            args: vec!["rev-parse".to_owned(), "HEAD".to_owned()],
            cwd: target,
            target,
        },
        settings,
        context,
    )
    .await?;
    let commit = run.stdout.trim().to_owned();
    if commit.is_empty() {
        return Err(ActivityFailure::terminal(format!(
            "`git rev-parse HEAD` in {} printed no commit; see {}",
            target.display(),
            run.log_path.display()
        )));
    }
    Ok(commit)
}

async fn free_disk_bytes(
    target: &Path,
    settings: &Settings,
    context: &ActivityContext,
) -> Result<i64, ActivityFailure> {
    let run = step(
        Step {
            name: "disk",
            program: DF,
            args: vec![
                "-P".to_owned(),
                "-k".to_owned(),
                target.display().to_string(),
            ],
            cwd: target,
            target,
        },
        settings,
        context,
    )
    .await?;
    parse_available_bytes(&run.stdout).ok_or_else(|| {
        ActivityFailure::terminal(format!(
            "`df -P -k {}` output could not be read for free space; see {}",
            target.display(),
            run.log_path.display()
        ))
    })
}

/// Reads the available-space column out of POSIX `df -P -k` output.
#[must_use]
pub fn parse_available_bytes(stdout: &str) -> Option<i64> {
    stdout
        .lines()
        .nth(1)?
        .split_whitespace()
        .nth(DF_AVAILABLE_FIELD)?
        .parse::<i64>()
        .ok()?
        .checked_mul(DF_BLOCK_BYTES)
}

/// One named checkout step: what to run, where, and which checkout it logs under.
struct Step<'a> {
    name: &'a str,
    program: &'a str,
    args: Vec<String>,
    cwd: &'a Path,
    target: &'a Path,
}

/// Runs one checkout step, failing the activity on any non-zero exit.
async fn step(
    step: Step<'_>,
    settings: &Settings,
    context: &ActivityContext,
) -> Result<CommandRun, ActivityFailure> {
    let name = step.name;
    let log = log_path(settings, step.target, &format!("clone-{name}"), context);
    let spec = CommandSpec {
        program: step.program.to_owned(),
        args: step.args,
        env: Vec::new(),
        cwd: step.cwd.to_path_buf(),
    };
    let run = run_logged(
        &spec,
        &log,
        &format!("clone:{name}"),
        context,
        settings.progress_interval(),
    )
    .await
    .map_err(|error| {
        ActivityFailure::terminal(format!("checkout step `{name}` failed: {error}"))
    })?;
    if run.exit_code == 0 {
        return Ok(run);
    }
    Err(ActivityFailure::terminal(format!(
        "checkout step `{name}` ({}) exited {}; see {}\n{}",
        spec.rendered(),
        run.exit_code,
        run.log_path.display(),
        run.output_tail
    )))
}

fn validated_repo(repo: &str) -> Result<&str, ActivityFailure> {
    let repo = repo.trim();
    if repo.is_empty() {
        return Err(ActivityFailure::terminal(
            "repository must not be empty".to_owned(),
        ));
    }
    if repo.starts_with('-') {
        return Err(ActivityFailure::terminal(format!(
            "repository `{repo}` starts with `-` and would be read as a Git option"
        )));
    }
    Ok(repo)
}

fn validated_branch(branch: &str) -> Result<&str, ActivityFailure> {
    let branch = branch.trim();
    if branch.is_empty() {
        return Err(ActivityFailure::terminal(
            "branch must not be empty".to_owned(),
        ));
    }
    if branch.starts_with('-')
        || !branch
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
    {
        return Err(ActivityFailure::terminal(format!(
            "branch `{branch}` is not a plain Git branch name"
        )));
    }
    Ok(branch)
}

/// Directory name for a remote: its last path segment without `.git`.
fn repository_name(repo: &str) -> Result<String, ActivityFailure> {
    let trimmed = repo.trim_end_matches('/');
    let segment = trimmed
        .rsplit(['/', ':'])
        .next()
        .unwrap_or(trimmed)
        .trim_end_matches(".git");
    if segment.is_empty()
        || segment == "."
        || segment == ".."
        || !segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return Err(ActivityFailure::terminal(format!(
            "repository `{repo}` does not end in a usable directory name"
        )));
    }
    Ok(segment.to_owned())
}

/// Root the checkouts live under, exposed so the runbook and the README can
/// describe the same layout the worker actually builds.
#[must_use]
pub fn checkout_path(settings: &Settings, repo: &str) -> Option<PathBuf> {
    repository_name(repo)
        .ok()
        .map(|name| settings.checkout_dir().join(name))
}

#[cfg(test)]
mod tests {
    use super::{parse_available_bytes, repository_name, validated_branch, validated_repo};

    #[test]
    fn a_remote_resolves_to_its_last_segment_without_dot_git() -> Result<(), String> {
        for (repo, expected) in [
            ("https://github.com/ablative/aion.git", "aion"),
            ("https://github.com/ablative/aion", "aion"),
            ("git@github.com:ablative/aion.git", "aion"),
            ("/Users/tom/Developer/ablative/stack/aion", "aion"),
            ("/Users/tom/Developer/ablative/stack/aion/", "aion"),
        ] {
            let name = repository_name(repo).map_err(|error| format!("{repo}: {error}"))?;
            assert_eq!(name, expected, "for {repo}");
        }
        Ok(())
    }

    #[test]
    fn a_remote_that_would_escape_the_checkout_root_is_refused() {
        for repo in ["https://example.com/..", "..", ".", "/", "", "a/b/c d"] {
            assert!(repository_name(repo).is_err(), "accepted {repo}");
        }
    }

    #[test]
    fn option_shaped_inputs_are_refused_before_git_sees_them() {
        assert!(validated_repo("--upload-pack=touch /tmp/x").is_err());
        assert!(validated_branch("--exec=touch /tmp/x").is_err());
        assert!(validated_branch("main; rm -rf /").is_err());
        assert!(validated_branch("").is_err());
    }

    #[test]
    fn plain_branch_names_survive_validation() -> Result<(), String> {
        for branch in ["main", "build/remote-gates", "v0.10.0", "release_1.2"] {
            validated_branch(branch).map_err(|error| format!("{branch}: {error}"))?;
        }
        Ok(())
    }

    #[test]
    fn free_space_is_read_from_the_posix_df_column() {
        let stdout = concat!(
            "Filesystem 1024-blocks      Used Available Capacity Mounted on\n",
            "/dev/disk3s5 971350180 841000000  61234567      94% /System/Volumes/Data\n"
        );
        assert_eq!(parse_available_bytes(stdout), Some(61_234_567 * 1024));
    }

    #[test]
    fn unreadable_df_output_is_not_guessed_at() {
        assert_eq!(parse_available_bytes(""), None);
        assert_eq!(parse_available_bytes("header only\n"), None);
        assert_eq!(
            parse_available_bytes("header\nnot numbers here at all\n"),
            None
        );
    }
}
