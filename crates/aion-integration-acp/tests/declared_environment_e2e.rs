//! The pinned regression for the CLAUDECODE incident, proved against a REAL exec'd child.
//!
//! # The incident
//!
//! 2026-08-11, the `mm_fleet_loop` oversight seat. An agent's own shell exports `CLAUDECODE`.
//! The worker was started from that shell, inherited the variable, and passed it — invisibly,
//! because nothing anywhere had written it down — to the `claude-code-acp` it spawned. That agent
//! refuses to launch nested, so every `session/new` came back `-32603` and the run's agent seats
//! were dead until somebody read the JSON-RPC error code by hand.
//!
//! # What this test proves, and how it avoids proving nothing
//!
//! The harness is given a declared environment naming exactly two variables and an agent that is
//! a real executable: it writes its own environment to the file named by its first ARGUMENT (an
//! argument, not a variable, so the report channel cannot itself depend on the thing under test)
//! and exits. `AgentHarness::start` then fails its handshake, which is expected and irrelevant —
//! the evidence is the file the child wrote before dying.
//!
//! Two arms, because one alone would be weak:
//!
//! * **the incident's variable** — `CLAUDECODE` is in the launching context handed to the
//!   declaration and is NOT in the child. This is the reproduction the ruling asked for.
//! * **a variable that is genuinely in the real parent** — the test process runs under Cargo,
//!   which sets `CARGO_PKG_NAME`; it is not declared, and it is absent from the child. Without
//!   this arm the first could pass on a harness that simply never read the launching context at
//!   all, and the test would be measuring nothing.
//!
//! The positive control is the same file: both declared variables ARE present with their declared
//! values, so an "absent" reading cannot come from an empty or unwritten report.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use aion_integration_acp::{AcpHarness, PermissionPolicy};
use aion_integrations::{
    ActivityId, AgentHarness, AgentRunSpec, ContentType, EnvironmentDeclaration, Payload, RunId,
    WorkflowId,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The variable whose inheritance broke the live run.
const INCIDENT_VARIABLE: &str = "CLAUDECODE";
/// A variable Cargo really does set on a test process — the control for "the parent's own
/// environment does not cross".
const REAL_PARENT_VARIABLE: &str = "CARGO_PKG_NAME";

/// Writes an executable that reports its whole environment to the path in `$1`.
///
/// `/usr/bin/env` is named ABSOLUTELY on purpose: the point of the test is a child with a
/// constructed environment, and a `PATH` lookup for the reporting tool would make the report
/// depend on the very variable under examination.
fn report_environment_agent(directory: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let script = directory.join("report-environment");
    std::fs::write(&script, "#!/bin/sh\n/usr/bin/env > \"$1\"\n")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(script)
}

/// One activity attempt's neutral identity, with a prompt the agent never reads.
fn spec() -> AgentRunSpec {
    AgentRunSpec::new(
        WorkflowId::new_v4(),
        RunId::new_v4(),
        ActivityId::from_sequence_position(1),
        1,
        "ask".to_owned(),
        Payload::new(ContentType::Json, b"\"prompt\"".to_vec()),
    )
}

/// The launching context: the incident's variable, plus the two the document declares.
fn launching_context(home: &Path) -> BTreeMap<String, String> {
    let mut context = aion_integrations::process_environment();
    context.insert(INCIDENT_VARIABLE.to_owned(), "1".to_owned());
    context.insert("HOME".to_owned(), home.display().to_string());
    context
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
async fn a_declared_environment_keeps_claudecode_out_of_the_agent_child() -> TestResult {
    let sandbox = tempfile::tempdir()?;
    let agent = report_environment_agent(sandbox.path())?;
    let report = sandbox.path().join("child-environment.txt");

    let declaration = EnvironmentDeclaration::new(["PATH", "HOME"])?;
    let context = launching_context(sandbox.path());
    let environment = declaration.resolve(&context);

    // The parent really does carry both variables the child must not see.
    assert!(
        context.contains_key(INCIDENT_VARIABLE),
        "the launching context must carry the incident's variable, or the test proves nothing"
    );
    assert!(
        context.contains_key(REAL_PARENT_VARIABLE),
        "this test process must really carry {REAL_PARENT_VARIABLE}, or the control proves nothing"
    );

    let harness = AcpHarness::new(
        &agent,
        [report.display().to_string()],
        sandbox.path(),
        PermissionPolicy::Deny,
        Duration::from_secs(0),
    )
    .with_declared_environment(environment);

    // The agent writes its report and exits, so the ACP handshake cannot complete. That failure is
    // expected and is not what is under test; the report on disk is.
    let outcome = harness.start(spec()).await;
    assert!(
        outcome.is_err(),
        "an agent that only reports its environment cannot complete an ACP handshake"
    );

    let text = std::fs::read_to_string(&report)
        .map_err(|error| format!("the agent must have written {}: {error}", report.display()))?;
    let child = reported(&text);

    assert!(
        !child.contains_key(INCIDENT_VARIABLE),
        "{INCIDENT_VARIABLE} was in the launching context and is not declared, so it must be \
         absent from the child; the child carried: {:?}",
        child.keys().collect::<Vec<_>>()
    );
    assert!(
        !child.contains_key(REAL_PARENT_VARIABLE),
        "{REAL_PARENT_VARIABLE} is really set on this process and is not declared, so it must be \
         absent from the child; the child carried: {:?}",
        child.keys().collect::<Vec<_>>()
    );
    // The positive control: an absent reading must not come from an empty report.
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
async fn without_a_declaration_the_child_still_inherits_which_is_what_makes_the_pin_meaningful()
-> TestResult {
    let sandbox = tempfile::tempdir()?;
    let agent = report_environment_agent(sandbox.path())?;
    let report = sandbox.path().join("child-environment.txt");

    let harness = AcpHarness::new(
        &agent,
        [report.display().to_string()],
        sandbox.path(),
        PermissionPolicy::Deny,
        Duration::from_secs(0),
    );
    let outcome = harness.start(spec()).await;
    assert!(
        outcome.is_err(),
        "the reporting agent completes no handshake"
    );

    let text = std::fs::read_to_string(&report)?;
    let child = reported(&text);
    assert!(
        child.contains_key(REAL_PARENT_VARIABLE),
        "a harness with NO declared environment inherits, so this is the world the declaration \
         has to change; without this arm the pin above could be passing because nothing is \
         inherited anywhere"
    );
    Ok(())
}

#[tokio::test]
async fn a_bare_command_with_no_path_declared_is_refused_naming_path() -> TestResult {
    let sandbox = tempfile::tempdir()?;
    let declaration = EnvironmentDeclaration::new(["HOME"])?;
    let mut context = BTreeMap::new();
    context.insert("HOME".to_owned(), sandbox.path().display().to_string());

    let harness = AcpHarness::new(
        "claude-code-acp",
        Vec::<String>::new(),
        sandbox.path(),
        PermissionPolicy::Deny,
        Duration::from_secs(0),
    )
    .with_declared_environment(declaration.resolve(&context));

    let Err(error) = harness.start(spec()).await else {
        return Err("a program looked up on an absent PATH cannot be launched".into());
    };
    let message = error.to_string();
    assert!(
        message.contains("PATH"),
        "the refusal names the missing pass-through rather than dying generic at exec: {message}"
    );
    assert!(
        message.contains("env_pass"),
        "the refusal names the declaration to correct: {message}"
    );
    assert!(
        message.contains("claude-code-acp"),
        "the refusal names the program it could not resolve: {message}"
    );
    Ok(())
}
