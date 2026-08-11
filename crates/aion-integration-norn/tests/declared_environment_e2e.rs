//! The Norn adapter's declared environment, proved against a REAL exec'd child.
//!
//! # Why this test exists as well as the ACP one
//!
//! The two adapters build their spawn commands in different files, and the Norn one
//! builds its through [`NornInvocation`]. Before this test, the Norn `env_clear()` sat on
//! no test's path: the invocation-preview test asserts that the DECLARATION rides the
//! preview, and it stays green with the `env_clear()` line deleted, because a preview is
//! not a process. `std::process::Command` cannot be interrogated for its clear flag
//! either — `get_envs()` reports the explicit changes, not whether the inherited set was
//! dropped. So the only oracle that can see the difference is a child that has actually
//! been exec'd and reports what it was given.
//!
//! That is what this does: the harness launches a program that writes its own
//! environment to the file named by its LAST argument — an argument, not a variable, so
//! the report channel cannot depend on the thing under test — and exits. The handshake
//! then fails, which is expected and irrelevant; the evidence is the file on disk.
//!
//! Three arms, because one alone would be weak:
//!
//! * the incident's variable (`CLAUDECODE`) is in the launching context and NOT in the
//!   child;
//! * `CARGO_PKG_NAME`, which Cargo really does set on this test process, is not declared
//!   and is absent from the child — without this the first arm could pass on an
//!   implementation that never read the launching context at all;
//! * the declared variables ARE present with their declared values, so an "absent"
//!   reading cannot come from an empty or unwritten report.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use aion_integration_norn::NornHarness;
use aion_integrations::{
    ActivityId, AgentHarness, AgentRunSpec, ContentType, EnvironmentDeclaration, Payload, RunId,
    WorkflowId,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The variable whose inheritance broke the live run on 2026-08-11.
const INCIDENT_VARIABLE: &str = "CLAUDECODE";
/// A variable Cargo really sets on a test process — the control for "the parent's own
/// environment does not cross".
const REAL_PARENT_VARIABLE: &str = "CARGO_PKG_NAME";

/// Writes an executable that reports its whole environment to the path in its LAST
/// argument.
///
/// The last argument rather than `$1`, because the Norn adapter prepends its own fixed
/// `--protocol jsonrpc` and this test must not break the day that prefix changes.
/// `/usr/bin/env` is named ABSOLUTELY on purpose: the point of the test is a child with a
/// constructed environment, and a `PATH` lookup for the reporting tool would make the
/// report depend on the very variable under examination.
fn report_environment_binary(directory: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let script = directory.join("report-environment");
    std::fs::write(
        &script,
        "#!/bin/sh\nfor argument in \"$@\"; do last=\"$argument\"; done\n/usr/bin/env > \"$last\"\n",
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(script)
}

/// One activity attempt's neutral identity.
fn spec() -> AgentRunSpec {
    AgentRunSpec::new(
        WorkflowId::new_v4(),
        RunId::new_v4(),
        ActivityId::from_sequence_position(1),
        1,
        "ask",
        Payload::new(ContentType::Json, b"\"prompt\"".to_vec()),
    )
}

/// The `KEY=VALUE` lines a child's `env` output carries, as a map.
fn reported(report: &str) -> BTreeMap<String, String> {
    report
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            Some((key.to_owned(), value.to_owned()))
        })
        .collect()
}

#[tokio::test]
async fn a_declared_environment_keeps_claudecode_out_of_the_norn_child() -> TestResult {
    let sandbox = tempfile::tempdir()?;
    let binary = report_environment_binary(sandbox.path())?;
    let report = sandbox.path().join("child-environment.txt");

    let mut context = aion_integrations::process_environment();
    context.insert(INCIDENT_VARIABLE.to_owned(), "1".to_owned());
    context.insert("HOME".to_owned(), sandbox.path().display().to_string());
    assert!(
        context.contains_key(REAL_PARENT_VARIABLE),
        "this test process must really carry {REAL_PARENT_VARIABLE}, or the control below \
         proves nothing"
    );

    let declaration = EnvironmentDeclaration::new(["PATH", "HOME"])?;
    let harness = NornHarness::with_binary(&binary)
        .with_arg(report.display().to_string())
        .with_declared_environment(declaration.resolve(&context));

    // The binary writes its report and exits, so the JSON-RPC handshake cannot complete.
    // That failure is expected and is not what is under test; the report on disk is.
    let outcome = harness.start(spec()).await;
    assert!(
        outcome.is_err(),
        "a binary that only reports its environment completes no handshake"
    );

    let text = std::fs::read_to_string(&report)
        .map_err(|error| format!("the child must have written {}: {error}", report.display()))?;
    let child = reported(&text);

    assert!(
        !child.contains_key(INCIDENT_VARIABLE),
        "{INCIDENT_VARIABLE} was in the launching context and is not declared, so it must \
         be absent from the child; the child carried: {:?}",
        child.keys().collect::<Vec<_>>()
    );
    assert!(
        !child.contains_key(REAL_PARENT_VARIABLE),
        "{REAL_PARENT_VARIABLE} is really set on this process and is not declared, so it \
         must be absent from the child; the child carried: {:?}",
        child.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        child.get("HOME").map(String::as_str),
        Some(sandbox.path().display().to_string().as_str()),
        "a declared variable is carried with the launching context's value"
    );
    assert!(
        child.contains_key("PATH"),
        "a declared variable is carried; the child saw {:?}",
        child.keys().collect::<Vec<_>>()
    );
    Ok(())
}

#[tokio::test]
async fn without_a_declaration_the_norn_child_still_inherits() -> TestResult {
    let sandbox = tempfile::tempdir()?;
    let binary = report_environment_binary(sandbox.path())?;
    let report = sandbox.path().join("child-environment.txt");

    let harness = NornHarness::with_binary(&binary).with_arg(report.display().to_string());
    let outcome = harness.start(spec()).await;
    assert!(
        outcome.is_err(),
        "the reporting binary completes no handshake"
    );

    let child = reported(&std::fs::read_to_string(&report)?);
    assert!(
        child.contains_key(REAL_PARENT_VARIABLE),
        "a harness with NO declared environment inherits, so this is the world the \
         declaration has to change; without this arm the pin above could be passing \
         because nothing is inherited anywhere"
    );
    Ok(())
}

#[tokio::test]
async fn a_binary_on_path_with_no_path_declared_is_refused_naming_path() -> TestResult {
    let mut context = BTreeMap::new();
    context.insert("HOME".to_owned(), "/tmp".to_owned());
    let declaration = EnvironmentDeclaration::new(["HOME"])?;

    // The adapter's own documented default: `norn`, looked up on `PATH`.
    let harness = NornHarness::new().with_declared_environment(declaration.resolve(&context));

    let Err(error) = harness.start(spec()).await else {
        return Err("a program looked up on an absent PATH cannot be launched".into());
    };
    let message = error.to_string();
    assert!(
        message.contains("PATH"),
        "the refusal names the missing pass-through rather than dying generic at exec: \
         {message}"
    );
    assert!(
        message.contains("env_pass"),
        "the refusal names the declaration to correct: {message}"
    );
    assert!(
        error.is_deterministic(),
        "a declaration that omits PATH omits it just as hard on the next attempt, so the \
         refusal must not present as a retryable transport failure: {error:?}"
    );
    Ok(())
}
