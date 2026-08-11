//! End-to-end guard for the server-run `assistant_provision` body's stdout
//! purity — ratification condition 2 of #139 (the recorded activity result's
//! `stdout` IS the workspace path, so durable history carries the RESOLVED
//! path and nothing else).
//!
//! The command under test is DERIVED, not copied: it is extracted from the
//! compiled contract of the embedded `assistant.awl` document, so an edit to
//! the shipped body is held here, against a real `git`, before any live
//! dispatch sees it.
//!
//! Expansion is replicated with `str::replace` rather than through
//! `aion_server::worker::WorkspaceRoot`: aion-server would be a legal
//! dev-dependency (this crate is not in its graph, so there is no cycle), but
//! it drags the whole engine, store, and transport stack into this standalone
//! example crate's own build for one line of behaviour. That line is pinned
//! on the aion-server side by its own unit tests (`workspace_root.rs`:
//! expansion replaces every placeholder occurrence with the root text, and
//! the shipped documents' commands pass the placement guard), and this file
//! asserts the placeholder stands as a bare word before replacing it, so the
//! two stay the same seam.

#![cfg(unix)]

use std::collections::BTreeMap;
use std::error::Error;
use std::path::{Path, PathBuf};

use agent_dev_worker::descriptors::ASSISTANT_DOCUMENT;
use aion_core::{ActivityId, RunId, WorkflowId};
use aion_worker::ActivityContext;
use aion_worker::shell::ShellAction;

type TestResult = Result<(), Box<dyn Error>>;

/// The placeholder the server expands — the same literal
/// `aion_server::worker::WORKSPACE_ROOT_PLACEHOLDER` names (see the module
/// note for why the constant is not imported).
const WORKSPACE_ROOT_PLACEHOLDER: &str = "{workspace_root}";

/// Extract the `assistant_provision` body command from the embedded
/// document's COMPILED contract — the same derivation the server dispatches
/// from, so the test cannot drift from the shipped body.
fn declared_provision_command() -> Result<String, Box<dyn Error>> {
    let compiled = aion_awl::compile(ASSISTANT_DOCUMENT, Path::new("."))?;
    compiled
        .contract
        .workers
        .iter()
        .flat_map(|worker| &worker.actions)
        .find_map(|action| match &action.body {
            Some(aion_package::ActionBodyContract::Run { command })
                if action.name == "assistant_provision" =>
            {
                Some(command.clone())
            }
            _ => None,
        })
        .ok_or_else(|| "the assistant document must declare a bodied assistant_provision".into())
}

/// Replicate the server's expansion: every placeholder occurrence replaced by
/// the root's text, after asserting each occurrence stands as a bare argv
/// word (` {workspace_root} ` or at an end) — the property the server's
/// placement guard enforces before it splices.
fn expand(command: &str, root: &Path) -> Result<String, Box<dyn Error>> {
    for (start, _) in command.match_indices(WORKSPACE_ROOT_PLACEHOLDER) {
        let end = start + WORKSPACE_ROOT_PLACEHOLDER.len();
        let bare = command[..start]
            .chars()
            .next_back()
            .is_none_or(char::is_whitespace)
            && command[end..]
                .chars()
                .next()
                .is_none_or(char::is_whitespace);
        if !bare {
            return Err("the shipped body must carry the placeholder as a bare argv word".into());
        }
    }
    let root_text = root
        .to_str()
        .ok_or("the test root must be valid UTF-8")?
        .to_owned();
    Ok(command.replace(WORKSPACE_ROOT_PLACEHOLDER, &root_text))
}

fn activity_context() -> ActivityContext {
    ActivityContext::new(
        WorkflowId::new_v4(),
        RunId::new_v4(),
        ActivityId::from_sequence_position(1),
        1,
    )
    .0
}

fn arguments(run_id: &str, repo_path: &str) -> BTreeMap<String, serde_json::Value> {
    [
        ("run_id".to_owned(), serde_json::json!(run_id)),
        ("repo_path".to_owned(), serde_json::json!(repo_path)),
    ]
    .into_iter()
    .collect()
}

/// Run one git command in `dir`, failing the test loudly on a non-zero exit.
fn git(dir: &Path, args: &[&str]) -> TestResult {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "git {args:?} in {} failed: {}",
            dir.display(),
            String::from_utf8_lossy(&output.stderr)
        )
        .into())
    }
}

/// Build a tiny committed fixture repository to clone from.
fn fixture_repo(parent: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let dir = parent.join("fixture");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("README.md"), "# provision-body fixture\n")?;
    git(&dir, &["init", "-b", "main"])?;
    git(&dir, &["config", "user.email", "provision-body@test"])?;
    git(&dir, &["config", "user.name", "Provision Body"])?;
    git(&dir, &["add", "-A"])?;
    git(&dir, &["commit", "-m", "fixture: initial"])?;
    Ok(dir)
}

/// Execute the derived, expanded body through the worker SDK's own executor —
/// the same `ShellAction` path the server dispatches through.
async fn run_body(
    root: &Path,
    run_id: &str,
    repo_path: &str,
) -> Result<aion_worker::shell::ShellOutcome, Box<dyn Error>> {
    let command = expand(&declared_provision_command()?, root)?;
    let action = ShellAction::new(&command)?;
    action
        .run(&arguments(run_id, repo_path), &activity_context())
        .await
        .map_err(|failure| format!("the provision body failed: {}", failure.message()).into())
}

#[tokio::test]
async fn the_scratch_branch_prints_exactly_the_workspace_path() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let root = scratch.path().join("clones");
    std::fs::create_dir_all(&root)?;

    let outcome = run_body(&root, "wf-scratch-1", "").await?;
    let expected = root.join("wf-scratch-1").join("repo");
    assert_eq!(outcome.exit_code, 0);
    assert_eq!(
        outcome.stdout,
        expected.to_string_lossy(),
        "stdout must be EXACTLY the resolved workspace path — durable history \
         records it verbatim"
    );
    assert!(
        expected.join(".git").is_dir(),
        "scratch mode must git init the workspace"
    );
    Ok(())
}

#[tokio::test]
async fn the_attached_branch_prints_exactly_the_workspace_path() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let root = scratch.path().join("clones");
    std::fs::create_dir_all(&root)?;
    let fixture = fixture_repo(scratch.path())?;

    let outcome = run_body(&root, "wf-attached-1", &fixture.to_string_lossy()).await?;
    let expected = root.join("wf-attached-1").join("repo");
    assert_eq!(outcome.exit_code, 0);
    assert_eq!(
        outcome.stdout,
        expected.to_string_lossy(),
        "git clone's chatter must all be on stderr; stdout is the path alone"
    );
    assert!(
        expected.join("README.md").is_file(),
        "the attached branch must really clone the fixture"
    );
    Ok(())
}

#[tokio::test]
async fn the_rename_aside_branch_prints_exactly_the_workspace_path() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let root = scratch.path().join("clones");
    std::fs::create_dir_all(&root)?;

    // A surviving directory under the SAME run id, with salvageable content.
    let stale = root.join("wf-collide-1");
    std::fs::create_dir_all(&stale)?;
    std::fs::write(stale.join("partial.txt"), "half a workspace\n")?;

    let outcome = run_body(&root, "wf-collide-1", "").await?;
    let expected = root.join("wf-collide-1").join("repo");
    assert_eq!(outcome.exit_code, 0);
    assert_eq!(
        outcome.stdout,
        expected.to_string_lossy(),
        "the mv's own chatter must not reach stdout"
    );

    // The stale directory survives, renamed aside — never deleted.
    let renamed: Vec<PathBuf> = std::fs::read_dir(&root)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name().is_some_and(|name| {
                name.to_string_lossy()
                    .starts_with("wf-collide-1.superseded-")
            })
        })
        .collect();
    assert_eq!(
        renamed.len(),
        1,
        "exactly one renamed stale attempt; got {renamed:?}"
    );
    assert!(
        renamed[0].join("partial.txt").exists(),
        "the stale attempt's contents must survive the rename intact"
    );
    Ok(())
}

/// Extract the body's `sh` script (the single-quoted segment) so the failure
/// branches can be driven with an EXPLICIT argv. This is required for the
/// empty-root branch: an empty root cannot travel through the string splice
/// (the template collapses the resulting empty word away, shifting every
/// argument), which is exactly why the server REFUSES an empty root before
/// splicing and the script's own `$1` guard is defence-in-depth. Driving
/// `sh -c <script> -- <argv…>` directly delivers the empty word for real and
/// lets stdout be observed on failure, which `ShellAction`'s error path (by
/// design) does not return.
fn declared_script() -> Result<String, Box<dyn Error>> {
    let command = declared_provision_command()?;
    let first = command
        .find('\'')
        .ok_or("the body must carry a single-quoted script")?;
    let last = command
        .rfind('\'')
        .ok_or("the body must carry a single-quoted script")?;
    if last <= first {
        return Err("the body's quotes did not delimit a script".into());
    }
    let script = &command[first + 1..last];
    if script.contains('\'') {
        return Err("the script must not itself contain single quotes".into());
    }
    Ok(script.to_owned())
}

/// Run the derived script with an explicit argv, capturing both streams.
fn run_script_argv(
    root: &str,
    run_id: &str,
    repo_path: &str,
) -> std::io::Result<std::process::Output> {
    let script = match declared_script() {
        Ok(script) => script,
        Err(error) => {
            return Err(std::io::Error::other(error.to_string()));
        }
    };
    std::process::Command::new("sh")
        .args(["-c", &script, "--", root, run_id, repo_path])
        .output()
}

#[tokio::test]
async fn a_malformed_run_id_fails_with_empty_stdout() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let root = scratch.path().join("clones");
    std::fs::create_dir_all(&root)?;

    // Through the real executor: the dispatch FAILS, so no result — and
    // therefore no stdout — is ever recorded into durable history.
    let command = expand(&declared_provision_command()?, &root)?;
    let action = ShellAction::new(&command)?;
    let Err(failure) = action.run(&arguments("a/b", ""), &activity_context()).await else {
        return Err("a run_id with a path separator must fail the dispatch".into());
    };
    assert!(
        failure
            .message()
            .contains("run_id must be a single path component"),
        "the failure must carry the script's own guard message: {}",
        failure.message()
    );

    // The same derived script, driven with an explicit argv, proves the
    // failing branch writes NOTHING to stdout.
    let output = run_script_argv(&root.to_string_lossy(), "a/b", "")?;
    assert!(!output.status.success(), "the guard must exit non-zero");
    assert!(
        output.stdout.is_empty(),
        "a failing provision must produce EMPTY stdout, got: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    Ok(())
}

#[tokio::test]
async fn an_empty_root_fails_with_empty_stdout() -> TestResult {
    // See `declared_script`: only an explicit argv can deliver an empty root
    // word, because the server refuses an empty root before ever splicing it.
    let output = run_script_argv("", "wf-empty-root", "")?;
    assert!(
        !output.status.success(),
        "the empty-root guard must exit non-zero"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("workspace_root must not be empty"),
        "the guard must name the defect on stderr"
    );
    assert!(
        output.stdout.is_empty(),
        "a failing provision must produce EMPTY stdout, got: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    Ok(())
}
