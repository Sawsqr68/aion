//! The pass-through absence report, proved RECEIVED — through the real binary, off its
//! real stderr.
//!
//! # Why this test exists at all
//!
//! The first version of this report was a `tracing::warn!` raised while the launch was
//! being resolved. The agent worker installs its tracing subscriber inside the serve
//! loop, which is downstream of that point — and on the `--check` path the verb returns
//! before the serve loop exists at all. So the report was constructed on every run and
//! delivered on none of them. A unit test over the message would have stayed green
//! throughout, because the message was never the broken part; the DELIVERY was.
//!
//! This test therefore runs `aion worker agent` as a process and reads what an operator
//! at a terminal would read. It is the only shape that can tell the difference.
//!
//! # The controls
//!
//! Two arms, because either alone could pass for the wrong reason:
//!
//! * a document naming a variable that is not set → the name appears on stderr, and the
//!   `--check` report still arrives INTACT on stdout (the report must not be written
//!   into the JSON an operator pipes to `jq`);
//! * a document naming only variables the child process really has → stderr carries no
//!   warning at all. Without this arm the first could pass on an implementation that
//!   warns unconditionally, which would train an operator to ignore it.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A variable no machine sets, carrying this process's id so a parallel run of this
/// binary cannot collide with it.
fn absent_variable() -> String {
    format!("AION_SLICE1_DEFINITELY_UNSET_{}", std::process::id())
}

/// A workflow document declaring one agent queue whose harness passes `env_pass`.
fn document_source(env_pass: &str) -> String {
    format!(
        "//! Absence-report probe.\nworkflow ask\n  input question: String\n  outcome \
         answered: type Answer, route success\n\ntype Answer {{ reply: String }}\n\ntype \
         Reply {{ text: String, final_message: String, stop_reason: String, session_id: String }}\n\nworker reviewers\n  harness\n    \
         kind norn\n    concurrency 1\n    reconnect_initial_backoff 5s\n    \
         reconnect_max_backoff 1m\n    reconnect_max_attempts 10\n    env_pass \
         {env_pass}\n    cwd \"/srv/agents\"\n  action ask(prompt: String) -> \
         Reply\n    agent\n\nstep ask\n  \
         ask(prompt: question) -> reply\n  route answered(reply: reply.text)\n"
    )
}

/// Writes a document under a path carrying this process's id, so concurrent tests in
/// this binary never read each other's fixture.
fn write_document(label: &str, env_pass: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = std::env::temp_dir().join(format!(
        "aion-absence-report-{label}-{}.awl",
        std::process::id()
    ));
    std::fs::write(&path, document_source(env_pass))?;
    Ok(path)
}

/// Runs `aion worker agent <document> --check` and returns what the operator saw.
///
/// `--check` connects to nothing, which is what makes this safe to run anywhere: no
/// worker is served and no address is dialled. The report is emitted BEFORE the
/// `--check` branch, so this run exercises the same line the serve path does.
fn run_check(document: &Path) -> Result<Output, Box<dyn std::error::Error>> {
    Ok(Command::new(env!("CARGO_BIN_EXE_aion"))
        .args(["worker", "agent"])
        .arg(document)
        .arg("--check")
        .output()?)
}

#[test]
fn an_absent_declared_variable_is_reported_on_stderr_where_an_operator_reads_it() -> TestResult {
    let missing = absent_variable();
    let document = write_document("absent", &format!("\"PATH\", \"{missing}\""))?;
    let output = run_check(&document)?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "an absent pass-through is a warning, not a refusal: stderr was:\n{stderr}"
    );
    assert!(
        stderr.contains(&missing),
        "the operator must be told WHICH variable is missing; stderr was:\n{stderr}"
    );
    assert!(
        stderr.contains("reviewers") && stderr.contains("env_pass"),
        "the report names the queue and the declaration to correct; stderr was:\n{stderr}"
    );
    assert!(
        !stderr.contains("PATH,"),
        "a variable that IS set is not an absence; stderr was:\n{stderr}"
    );

    // The `--check` report itself must survive intact: an operator pipes stdout into
    // `jq`, and a warning written into that stream would break the tool rather than
    // inform the person.
    let report: serde_json::Value = serde_json::from_str(&stdout)?;
    assert_eq!(report["task_queue"], serde_json::json!("reviewers"));
    assert_eq!(report["harness"], serde_json::json!("norn"));

    std::fs::remove_file(&document)?;
    Ok(())
}

#[test]
fn a_declaration_this_machine_can_satisfy_produces_no_warning_at_all() -> TestResult {
    // PATH is set on any machine that could have run this test binary at all; the
    // assertion below re-checks that rather than assuming it, so a stripped environment
    // fails loudly instead of passing this control vacuously.
    assert!(
        std::env::var_os("PATH").is_some(),
        "this control needs a variable the child really inherits from the test runner"
    );
    let document = write_document("present", "\"PATH\"")?;
    let output = run_check(&document)?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "stderr was:\n{stderr}");
    assert!(
        !stderr.contains("environment pass-through"),
        "a complete declaration must produce silence — a warning on every start trains \
         an operator to skip the one that matters; stderr was:\n{stderr}"
    );

    std::fs::remove_file(&document)?;
    Ok(())
}
