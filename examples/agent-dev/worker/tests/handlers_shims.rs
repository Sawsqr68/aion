//! Hermetic handler tests with fake-CLI shims, following the
//! stacked-dev-remote suite: each test builds its own directory of stub
//! scripts (`git`, `cargo`) that emit canned output and append their argv to
//! per-executable log files, then constructs a `Shell` whose search path is
//! that directory ALONE. The handlers stay honest — they really shell out —
//! and the shims intercept at the process boundary. A CLI the test did not
//! stub is genuinely absent, which proves a missing CLI is a loud terminal
//! failure, never a silent skip. The `Shell` carries the search path (the
//! global `PATH` is never mutated), so the tests are parallel-safe.

#![cfg(unix)]

use std::error::Error;
use std::path::{Path, PathBuf};

use agent_dev_worker::handlers;
use agent_dev_worker::shell::Shell;
use agent_dev_worker::types::{
    AssistantProvisionInput, GateInput, LandInput, ProvisionInput, Workspace,
};
use aion_core::ActivityId;
use aion_worker::{ActivityContext, ActivityFailure, Classification};

type TestResult = Result<(), Box<dyn Error>>;

fn activity_context() -> ActivityContext {
    ActivityContext::new(ActivityId::from_sequence_position(1), 1).0
}

/// One test's shim directory. A `workspace-root` subdirectory serves as the
/// stable workspace root handed to `provision`.
struct Shims {
    dir: tempfile::TempDir,
}

impl Shims {
    fn new() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            dir: tempfile::tempdir()?,
        })
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn root_string(&self) -> String {
        self.root().to_string_lossy().into_owned()
    }

    /// The stable workspace root threaded to `provision`, as the composition
    /// root would after `resolve_workspace_root()`.
    fn workspace_root(&self) -> PathBuf {
        self.root().join("workspace-root")
    }

    /// A `Shell` resolving executables against the shim directory ALONE.
    fn shell(&self) -> Shell {
        Shell::with_path(self.root())
    }

    /// Write one shim: a `/bin/sh` script that records its argv to
    /// `<root>/<name>.log` and then runs `body`.
    fn write(&self, name: &str, body: &str) -> TestResult {
        use std::os::unix::fs::PermissionsExt;
        let path = self.root().join(name);
        let script = format!(
            "#!/bin/sh\nPATH=/usr/bin:/bin\necho \"$@\" >> \"{}/{name}.log\"\n{body}\n",
            self.root_string()
        );
        std::fs::write(&path, script)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
        Ok(())
    }

    /// Read one shim's argv recording (empty when the shim never ran).
    fn log(&self, name: &str) -> String {
        let path = self.root().join(format!("{name}.log"));
        match std::fs::read_to_string(&path) {
            Ok(log) => log,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(source) => format!("could not read shim log {}: {source}", path.display()),
        }
    }
}

fn assert_terminal(failure: &ActivityFailure, expected_fragment: &str) {
    assert_eq!(
        failure.classification(),
        &Classification::Terminal,
        "failure must be terminal: {failure:?}"
    );
    assert!(
        failure.message().contains(expected_fragment),
        "message {:?} must contain {expected_fragment:?}",
        failure.message()
    );
}

/// The `git` shim shared across provision scenarios: a clone that creates the
/// target directory it is handed (arg 3: `clone <url> <dir>`), everything
/// else a recorded success.
const GIT_PROVISION_SHIM: &str = r#"case "$1" in
  clone) mkdir -p "$3"; exit 0 ;;
  *) exit 0 ;;
esac"#;

fn provision_input(repo_url: String, run_id: &str) -> ProvisionInput {
    ProvisionInput {
        repo_url,
        base_ref: "main".to_owned(),
        brief_id: "brief-7".to_owned(),
        run_id: run_id.to_owned(),
    }
}

// --- provision -----------------------------------------------------------------

#[tokio::test]
async fn provision_clones_into_the_run_keyed_layout_and_creates_the_branch() -> TestResult {
    let shims = Shims::new()?;
    shims.write("git", GIT_PROVISION_SHIM)?;
    let root = shims.workspace_root();

    let workspace = handlers::provision(
        &shims.shell(),
        &root,
        provision_input("https://example.test/repo.git".to_owned(), "wf-123"),
        activity_context(),
    )
    .await
    .map_err(|failure| failure.message().to_owned())?;

    let expected_repo = root.join("wf-123").join("repo");
    assert_eq!(PathBuf::from(&workspace.path), expected_repo);
    assert_eq!(workspace.branch, "agent-dev-brief-7");
    assert!(
        expected_repo.is_dir(),
        "the clone target directory must exist"
    );
    let log = shims.log("git");
    assert!(
        log.contains(&format!(
            "clone https://example.test/repo.git {}",
            expected_repo.display()
        )),
        "git clone must target <root>/<run_id>/repo; got log: {log}"
    );
    assert!(
        log.contains("checkout -b agent-dev-brief-7 main"),
        "the branch must be created off base_ref; got log: {log}"
    );
    Ok(())
}

#[tokio::test]
async fn provision_collision_renames_the_stale_attempt_aside() -> TestResult {
    let shims = Shims::new()?;
    shims.write("git", GIT_PROVISION_SHIM)?;
    let root = shims.workspace_root();

    // An earlier partial attempt of the SAME run id, with salvageable content.
    let stale = root.join("wf-123");
    std::fs::create_dir_all(&stale)?;
    std::fs::write(stale.join("partial-clone.txt"), "half a clone\n")?;

    let workspace = handlers::provision(
        &shims.shell(),
        &root,
        provision_input("https://example.test/repo.git".to_owned(), "wf-123"),
        activity_context(),
    )
    .await
    .map_err(|failure| failure.message().to_owned())?;

    assert_eq!(
        PathBuf::from(&workspace.path),
        root.join("wf-123").join("repo")
    );
    // The stale attempt survives, renamed aside — never deleted.
    let renamed: Vec<PathBuf> = std::fs::read_dir(&root)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("wf-123.superseded-"))
        })
        .collect();
    assert_eq!(
        renamed.len(),
        1,
        "exactly one renamed stale attempt; got {renamed:?}"
    );
    assert!(
        renamed[0].join("partial-clone.txt").exists(),
        "the stale attempt's contents must survive the rename intact"
    );
    Ok(())
}

#[tokio::test]
async fn provision_rejects_a_run_id_that_escapes_the_root() -> TestResult {
    let shims = Shims::new()?;
    shims.write("git", GIT_PROVISION_SHIM)?;
    let root = shims.workspace_root();

    for bad_run_id in ["../escape", "a/b", "/abs", "..", ".", ""] {
        let failure = handlers::provision(
            &shims.shell(),
            &root,
            provision_input("https://example.test/repo.git".to_owned(), bad_run_id),
            activity_context(),
        )
        .await
        .err()
        .ok_or_else(|| format!("run_id {bad_run_id:?} must be refused"))?;
        assert_terminal(&failure, "not a single path component");
    }
    assert!(shims.log("git").is_empty(), "no git call may run");
    Ok(())
}

#[tokio::test]
async fn provision_failing_clone_is_terminal_with_diagnostics() -> TestResult {
    let shims = Shims::new()?;
    shims.write("git", "echo 'fatal: repository not found' >&2\nexit 128")?;
    let root = shims.workspace_root();

    let failure = handlers::provision(
        &shims.shell(),
        &root,
        provision_input("https://example.test/missing.git".to_owned(), "wf-404"),
        activity_context(),
    )
    .await
    .err()
    .ok_or("a failing clone must fail the activity")?;
    assert_terminal(&failure, "git clone failed — exit status 128");
    assert_terminal(&failure, "repository not found");
    Ok(())
}

// --- assistant_provision ---------------------------------------------------------

fn assistant_provision_input(repo_path: &str, run_id: &str) -> AssistantProvisionInput {
    AssistantProvisionInput {
        repo_path: repo_path.to_owned(),
        run_id: run_id.to_owned(),
    }
}

#[tokio::test]
async fn assistant_provision_clones_into_the_run_keyed_layout() -> TestResult {
    let shims = Shims::new()?;
    shims.write("git", GIT_PROVISION_SHIM)?;
    let root = shims.workspace_root();

    let workspace = handlers::assistant_provision(
        &shims.shell(),
        &root,
        assistant_provision_input("/repos/aion", "wf-assist-1"),
        activity_context(),
    )
    .await
    .map_err(|failure| failure.message().to_owned())?;

    let expected_repo = root.join("wf-assist-1").join("repo");
    assert_eq!(PathBuf::from(&workspace.path), expected_repo);
    assert!(
        expected_repo.is_dir(),
        "the clone target directory must exist"
    );
    let log = shims.log("git");
    assert!(
        log.contains(&format!("clone /repos/aion {}", expected_repo.display())),
        "git clone must target <root>/<run_id>/repo; got log: {log}"
    );
    assert!(
        !log.contains("init"),
        "a clone session must not also git init; got log: {log}"
    );
    Ok(())
}

#[tokio::test]
async fn assistant_provision_empty_repo_path_inits_a_scratch_workspace() -> TestResult {
    let shims = Shims::new()?;
    shims.write("git", "exit 0")?;
    let root = shims.workspace_root();

    let workspace = handlers::assistant_provision(
        &shims.shell(),
        &root,
        assistant_provision_input("", "wf-scratch-1"),
        activity_context(),
    )
    .await
    .map_err(|failure| failure.message().to_owned())?;

    let expected_repo = root.join("wf-scratch-1").join("repo");
    assert_eq!(PathBuf::from(&workspace.path), expected_repo);
    assert!(
        expected_repo.is_dir(),
        "the scratch workspace directory must exist"
    );
    let log = shims.log("git");
    assert!(
        log.contains("init"),
        "an empty repo_path must git init the scratch workspace; got log: {log}"
    );
    assert!(
        !log.contains("clone"),
        "an empty repo_path must not clone; got log: {log}"
    );
    Ok(())
}

#[tokio::test]
async fn assistant_provision_collision_renames_the_stale_attempt_aside() -> TestResult {
    let shims = Shims::new()?;
    shims.write("git", GIT_PROVISION_SHIM)?;
    let root = shims.workspace_root();

    // An earlier partial attempt of the SAME run id, with salvageable content.
    let stale = root.join("wf-assist-1");
    std::fs::create_dir_all(&stale)?;
    std::fs::write(stale.join("partial.txt"), "half a workspace\n")?;

    let workspace = handlers::assistant_provision(
        &shims.shell(),
        &root,
        assistant_provision_input("/repos/aion", "wf-assist-1"),
        activity_context(),
    )
    .await
    .map_err(|failure| failure.message().to_owned())?;

    assert_eq!(
        PathBuf::from(&workspace.path),
        root.join("wf-assist-1").join("repo")
    );
    let renamed: Vec<PathBuf> = std::fs::read_dir(&root)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name().is_some_and(|name| {
                name.to_string_lossy()
                    .starts_with("wf-assist-1.superseded-")
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

#[tokio::test]
async fn assistant_provision_rejects_a_run_id_that_escapes_the_root() -> TestResult {
    let shims = Shims::new()?;
    shims.write("git", GIT_PROVISION_SHIM)?;
    let root = shims.workspace_root();

    for bad_run_id in ["../escape", "a/b", "/abs", "..", ".", ""] {
        let failure = handlers::assistant_provision(
            &shims.shell(),
            &root,
            assistant_provision_input("/repos/aion", bad_run_id),
            activity_context(),
        )
        .await
        .err()
        .ok_or_else(|| format!("run_id {bad_run_id:?} must be refused"))?;
        assert_terminal(&failure, "not a single path component");
    }
    assert!(shims.log("git").is_empty(), "no git call may run");
    Ok(())
}

#[tokio::test]
async fn assistant_provision_failing_clone_is_terminal_with_diagnostics() -> TestResult {
    let shims = Shims::new()?;
    shims.write("git", "echo 'fatal: repository not found' >&2\nexit 128")?;
    let root = shims.workspace_root();

    let failure = handlers::assistant_provision(
        &shims.shell(),
        &root,
        assistant_provision_input("/repos/missing", "wf-assist-404"),
        activity_context(),
    )
    .await
    .err()
    .ok_or("a failing clone must fail the activity")?;
    assert_terminal(&failure, "git clone failed — exit status 128");
    assert_terminal(&failure, "repository not found");
    Ok(())
}

#[path = "handlers_shims/gate_land.rs"]
mod gate_land;
