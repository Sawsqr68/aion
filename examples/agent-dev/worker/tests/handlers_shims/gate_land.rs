use super::*;

// --- gate ------------------------------------------------------------------------

#[tokio::test]
async fn gate_pass_runs_clippy_then_tests() -> TestResult {
    let shims = Shims::new()?;
    shims.write("cargo", "exit 0")?;

    let result = handlers::gate(
        &shims.shell(),
        GateInput {
            path: shims.root_string(),
            branch: "agent-dev-TEST".to_owned(),
        },
        activity_context(),
    )
    .await
    .map_err(|failure| failure.message().to_owned())?;

    assert!(result.pass, "both commands exit zero: the gate passes");
    assert!(
        result.diagnostics.is_empty(),
        "a pass carries no diagnostics"
    );
    let log = shims.log("cargo");
    assert!(
        log.contains("clippy --workspace --all-targets -- -D warnings"),
        "clippy must run with the full strict argv; got log: {log}"
    );
    assert!(
        log.contains("test --workspace"),
        "the test suite must run after clippy; got log: {log}"
    );
    Ok(())
}

#[tokio::test]
async fn gate_failure_is_recorded_data_carrying_the_diagnostics_tail() -> TestResult {
    let shims = Shims::new()?;
    // Clippy fails; the test suite would pass but must not even run.
    shims.write(
        "cargo",
        r#"if [ "$1" = clippy ]; then echo 'error: unused variable `count`'; exit 1; fi
exit 0"#,
    )?;

    let result = handlers::gate(
        &shims.shell(),
        GateInput {
            path: shims.root_string(),
            branch: "agent-dev-TEST".to_owned(),
        },
        activity_context(),
    )
    .await
    .map_err(|failure| failure.message().to_owned())?;

    assert!(!result.pass, "a failing command is a recorded fail verdict");
    assert!(
        result
            .diagnostics
            .contains("cargo clippy failed — exit status 1"),
        "diagnostics must name the failing command; got: {}",
        result.diagnostics
    );
    assert!(
        result
            .diagnostics
            .contains("error: unused variable `count`"),
        "diagnostics must carry the command output; got: {}",
        result.diagnostics
    );
    assert!(
        !shims.log("cargo").contains("test --workspace"),
        "the test suite must not run after a failed clippy"
    );
    Ok(())
}

#[tokio::test]
async fn gate_test_failure_after_clean_clippy_is_recorded_data() -> TestResult {
    let shims = Shims::new()?;
    shims.write(
        "cargo",
        r#"if [ "$1" = test ]; then echo 'test result: FAILED. 1 failed'; exit 101; fi
exit 0"#,
    )?;

    let result = handlers::gate(
        &shims.shell(),
        GateInput {
            path: shims.root_string(),
            branch: "agent-dev-TEST".to_owned(),
        },
        activity_context(),
    )
    .await
    .map_err(|failure| failure.message().to_owned())?;

    assert!(!result.pass);
    assert!(
        result
            .diagnostics
            .contains("cargo test failed — exit status 101"),
        "diagnostics must name the failing command; got: {}",
        result.diagnostics
    );
    assert!(result.diagnostics.contains("test result: FAILED"));
    Ok(())
}

#[tokio::test]
async fn gate_that_cannot_run_at_all_is_a_loud_terminal_failure() -> TestResult {
    // No cargo shim is written, and the shim directory is the entire search
    // path, so cargo is genuinely absent — a broken environment, not a
    // recorded gate verdict.
    let shims = Shims::new()?;

    let failure = handlers::gate(
        &shims.shell(),
        GateInput {
            path: shims.root_string(),
            branch: "agent-dev-TEST".to_owned(),
        },
        activity_context(),
    )
    .await
    .err()
    .ok_or("a missing CLI must fail the activity")?;
    assert_terminal(
        &failure,
        "cargo clippy: executable not found on PATH: cargo",
    );
    Ok(())
}

#[tokio::test]
async fn gate_against_a_missing_workspace_is_the_explicit_diagnostic() -> TestResult {
    let shims = Shims::new()?;
    shims.write("cargo", "exit 0")?;

    let failure = handlers::gate(
        &shims.shell(),
        GateInput {
            path: format!("{}/never-provisioned/repo", shims.root_string()),
            branch: "agent-dev-TEST".to_owned(),
        },
        activity_context(),
    )
    .await
    .err()
    .ok_or("a missing workspace must fail the activity")?;
    assert_terminal(&failure, "workspace missing at");
    assert_terminal(&failure, "run cannot resume");
    assert!(shims.log("cargo").is_empty(), "no gate command may run");
    Ok(())
}

// --- land ------------------------------------------------------------------------

#[tokio::test]
async fn land_commits_and_returns_the_commit_sha() -> TestResult {
    let shims = Shims::new()?;
    shims.write(
        "git",
        r#"if [ "$1" = rev-parse ]; then printf '%s\n' 0123456789abcdef0123456789abcdef01234567; fi
exit 0"#,
    )?;

    let landed = handlers::land(
        &shims.shell(),
        LandInput {
            workspace: Workspace {
                path: shims.root_string(),
                branch: "agent-dev-brief-7".to_owned(),
            },
            brief_id: "brief-7".to_owned(),
        },
        activity_context(),
    )
    .await
    .map_err(|failure| failure.message().to_owned())?;

    assert_eq!(
        landed.commit_sha,
        "0123456789abcdef0123456789abcdef01234567"
    );
    let log = shims.log("git");
    assert!(log.contains("add -A"));
    assert!(
        log.contains("commit -m agent-dev: brief-7"),
        "the commit message must be `agent-dev: <brief_id>`; got log: {log}"
    );
    assert!(log.contains("rev-parse HEAD"));
    Ok(())
}

#[tokio::test]
async fn land_with_nothing_to_commit_is_terminal() -> TestResult {
    let shims = Shims::new()?;
    shims.write(
        "git",
        "if [ \"$1\" = commit ]; then echo 'nothing to commit, working tree clean'; exit 1; fi\nexit 0",
    )?;

    let failure = handlers::land(
        &shims.shell(),
        LandInput {
            workspace: Workspace {
                path: shims.root_string(),
                branch: "agent-dev-brief-7".to_owned(),
            },
            brief_id: "brief-7".to_owned(),
        },
        activity_context(),
    )
    .await
    .err()
    .ok_or("a no-op land must fail the activity")?;
    assert_terminal(&failure, "git commit failed — exit status 1");
    assert_terminal(&failure, "nothing to commit");
    assert!(
        !shims.log("git").contains("rev-parse"),
        "no sha may be read after a failed commit"
    );
    Ok(())
}

#[tokio::test]
async fn land_with_an_empty_rev_parse_is_terminal() -> TestResult {
    let shims = Shims::new()?;
    shims.write("git", "exit 0")?;

    let failure = handlers::land(
        &shims.shell(),
        LandInput {
            workspace: Workspace {
                path: shims.root_string(),
                branch: "agent-dev-brief-7".to_owned(),
            },
            brief_id: "brief-7".to_owned(),
        },
        activity_context(),
    )
    .await
    .err()
    .ok_or("an empty rev-parse must fail the activity")?;
    assert_terminal(&failure, "printed no commit sha");
    Ok(())
}
