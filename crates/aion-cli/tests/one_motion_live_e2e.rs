//! Live end-to-end gate for `aion run <file.awl>` — the one-motion pipeline.
//!
//! CHANGELOG.md claims "source to a running workflow in one command". This file
//! is that claim's gate: it drives the REAL `aion` binary against a REAL `aion
//! server` process on ephemeral ports and traverses the whole motion — compile,
//! deploy, start, await, report — for tracked AWL fixtures. The two pre-existing
//! `tests/one_motion_awl.rs` cases are negative tests that abort BEFORE any
//! server is contacted, so nothing here overlaps them.
//!
//! No runtime gate is needed: the one-motion path uses the native AWL bytecode
//! compiler (`aion-awl-package::compile_and_assemble_awl`), never the external
//! `gleam` toolchain, and the server runs the workflow on the in-process VM.

#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

type TestError = Box<dyn std::error::Error>;
type TestResult = Result<(), TestError>;

const BOOT_DEADLINE: Duration = Duration::from_secs(60);
/// Ceiling on a single `aion run` invocation, generous enough that a loaded box
/// never trips it; the assertions are on the command's own report, not on this.
const RUN_DEADLINE: Duration = Duration::from_secs(120);
/// The document timeout stamped onto the derived declared-timeout document, and
/// the `--timeout` bound the client-side wait test passes. Short enough to keep
/// the gate quick, long enough that deploy and start complete first.
const SHORT_BOUND: &str = "5s";

fn fixture(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../aion-awl/tests/fixtures/rev2")
        .join(relative)
}

fn reserve_port() -> Result<u16, TestError> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

/// A real `aion server` child on ephemeral ports, torn down by the test itself
/// (on `Drop`, so an assertion failure never leaks the process).
struct LiveServer {
    child: Child,
    grpc_port: u16,
    /// Held for its lifetime: the config and the server's private `AION_HOME`
    /// live inside it and must outlive the child.
    _workspace: tempfile::TempDir,
}

impl LiveServer {
    fn boot() -> Result<Self, TestError> {
        let workspace = tempfile::tempdir()?;
        // The server validates its resolved home as a private root; `tempdir`
        // inherits the umask, so the mode is set explicitly.
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(workspace.path(), std::fs::Permissions::from_mode(0o700))?;
        }
        let http_port = reserve_port()?;
        let grpc_port = reserve_port()?;
        let config = workspace.path().join("server-config.toml");
        std::fs::write(
            &config,
            format!(
                r#"workflow_packages = []

[server]
listen_address = "127.0.0.1:{http_port}"
grpc_address = "127.0.0.1:{grpc_port}"

[store]
backend = "memory"

[runtime]
query_timeout_ms = 10000

[namespaces]
default = "default"

[websocket]
event_broadcast_capacity = 1024
cluster_broadcast_capacity = 64

[observability]
# Required, no default: the transcript drain's flush policy.
max_batch_events = 64
max_batch_hold_ms = 0

[deploy]
enabled = true
max_archive_bytes = 16777216
max_inflated_bytes = 67108864
"#
            ),
        )?;
        let mut child = Command::new(env!("CARGO_BIN_EXE_aion"))
            .args([
                "server",
                "--config",
                config.to_str().ok_or("non-utf8 config path")?,
            ])
            .env("AION_HOME", workspace.path().join("aion-home"))
            .current_dir(workspace.path())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        wait_for_boot(&mut child, http_port)?;
        Ok(Self {
            child,
            grpc_port,
            _workspace: workspace,
        })
    }

    fn endpoint(&self) -> String {
        format!("127.0.0.1:{}", self.grpc_port)
    }

    /// Runs the real binary to completion with this server as its endpoint.
    fn aion(&self, args: &[&str]) -> Result<Output, TestError> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_aion"))
            .args(["--endpoint", &self.endpoint()])
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let started = Instant::now();
        loop {
            if child.try_wait()?.is_some() {
                return Ok(child.wait_with_output()?);
            }
            if started.elapsed() > RUN_DEADLINE {
                child.kill()?;
                let _ = child.wait();
                return Err(
                    format!("`aion {args:?}` did not finish within {RUN_DEADLINE:?}").into(),
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for LiveServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn http_get_live(http_port: u16) -> Option<String> {
    let mut stream =
        TcpStream::connect_timeout(&([127, 0, 0, 1], http_port).into(), Duration::from_secs(1))
            .ok()?;
    stream
        .write_all(b"GET /health/live HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    Some(response)
}

fn wait_for_boot(child: &mut Child, http_port: u16) -> Result<(), TestError> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Err(format!("server exited during boot with {status}").into());
        }
        if let Some(response) = http_get_live(http_port)
            && response.starts_with("HTTP/1.1 200")
        {
            return Ok(());
        }
        if started.elapsed() > BOOT_DEADLINE {
            child.kill()?;
            return Err("server did not boot within the deadline".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The value of a `  <label>: <value>` line in the command's stderr report.
fn reported_field(stderr: &str, label: &str) -> Result<String, TestError> {
    let needle = format!("{label}: ");
    stderr
        .lines()
        .find_map(|line| line.trim().strip_prefix(&needle).map(str::to_owned))
        .ok_or_else(|| format!("no `{label}` line in the report:\n{stderr}").into())
}

/// Writes `relative`'s tracked source into `directory` with a `timeout`
/// declaration added to its workflow header. The declaration is the ONLY
/// difference from the tracked fixture — no new corpus document is authored.
fn document_with_declared_timeout(
    relative: &str,
    workflow: &str,
    timeout: &str,
    directory: &Path,
) -> Result<PathBuf, TestError> {
    let source = std::fs::read_to_string(fixture(relative))?;
    let header = format!("workflow {workflow}\n");
    let stamped = source.replacen(&header, &format!("{header}  timeout {timeout}\n"), 1);
    if stamped == source {
        return Err(format!("fixture `{relative}` has no `{header:?}` header to stamp").into());
    }
    let path = directory.join(format!("{workflow}_declared.awl"));
    std::fs::write(&path, stamped)?;
    Ok(path)
}

// --- The claim: source to a reported result, in one command. ---

#[test]
fn one_motion_run_compiles_deploys_starts_awaits_and_reports_the_result() -> TestResult {
    let server = LiveServer::boot()?;
    let document = fixture("step-bodies/valid/unicode_payloads.awl");

    let output = server.aion(&[
        "run",
        document.to_str().ok_or("non-utf8 fixture path")?,
        "--input",
        r#"{"name":"Ada"}"#,
    ])?;
    let stdout = String::from_utf8(output.stdout)?;
    let stderr = String::from_utf8(output.stderr)?;
    assert_eq!(
        output.status.code(),
        Some(0),
        "one-motion run failed\nstdout: {stdout}\nstderr: {stderr}"
    );

    // The report is the command's whole product: identifiers AND the decoded
    // terminal result. Asserting the payload proves the run really executed on
    // the server rather than that the command merely exited zero.
    let report: serde_json::Value = serde_json::from_str(&stdout)?;
    assert_eq!(report["workflow_type"], "unicode_payloads");
    assert_eq!(report["result"]["outcome"], "greeted");
    assert_eq!(
        report["result"]["payload"]["message"],
        "こんにちは 🚀 café Ω — Ada"
    );
    assert_eq!(report["result"]["payload"]["who"], "Ada");
    // The identifiers must be the run's real ids, not placeholders: `describe`
    // resolves them against the same server.
    let workflow_id = report["workflow_id"]
        .as_str()
        .ok_or("report carried no workflow_id")?;
    let described = server.aion(&["describe", workflow_id])?;
    assert_eq!(described.status.code(), Some(0));
    let described: serde_json::Value = serde_json::from_slice(&described.stdout)?;
    assert_eq!(described["summary"]["status"], "Completed");
    Ok(())
}

// --- The failure report: a failing run exits non-zero and names the outcome. ---

#[test]
fn one_motion_run_reports_a_failure_outcome_with_its_details_and_exits_nonzero() -> TestResult {
    let server = LiveServer::boot()?;
    let document = fixture("step-bodies/valid/collection_predicates.awl");

    // Empty collections make every existential predicate false, so the document
    // routes its `rejected` outcome, which is declared `route failure`.
    let output = server.aion(&[
        "run",
        document.to_str().ok_or("non-utf8 fixture path")?,
        "--input",
        r#"{"findings":[],"rounds":[]}"#,
    ])?;
    let stdout = String::from_utf8(output.stdout)?;
    let stderr = String::from_utf8(output.stderr)?;

    assert_eq!(
        output.status.code(),
        Some(1),
        "a failed run must exit 1\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.is_empty(), "stdout was: {stdout}");
    assert!(
        stderr.contains("workflow run failed:"),
        "stderr was: {stderr}"
    );
    // The engine's own message is a process ordinal ("workflow N returned
    // error") and names nothing an operator can act on; the recorded details
    // carry the routed outcome, and the report must not drop them.
    assert!(
        stderr.contains("AwlOutcomeFailure"),
        "the failure report dropped its recorded details: {stderr}"
    );
    assert!(
        stderr.contains("\"outcome\":\"rejected\""),
        "the failure report did not name the routed outcome: {stderr}"
    );
    Ok(())
}

// --- A DECLARED document timeout ends the await through the engine deadline. ---

#[test]
fn a_declared_document_timeout_terminates_the_one_motion_await() -> TestResult {
    let server = LiveServer::boot()?;
    let scratch = tempfile::tempdir()?;
    // `wait_and_sleep` parks on a sleep and then on a signal that never arrives,
    // so ONLY the declared deadline can end this run.
    let document = document_with_declared_timeout(
        "step-bodies/valid/wait_and_sleep.awl",
        "wait_and_sleep",
        SHORT_BOUND,
        scratch.path(),
    )?;

    let output = server.aion(&[
        "run",
        document.to_str().ok_or("non-utf8 document path")?,
        "--input",
        r#"{"build_id":"b-1"}"#,
    ])?;
    let stdout = String::from_utf8(output.stdout)?;
    let stderr = String::from_utf8(output.stderr)?;

    assert_eq!(
        output.status.code(),
        Some(1),
        "a timed-out run must exit 1\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.is_empty(), "stdout was: {stdout}");
    // `"workflow"` is the deadline descriptor recorded on `WorkflowTimedOut`:
    // this is the ENGINE's deadline, not any client-side bound.
    assert!(
        stderr.contains("workflow run timed out: workflow"),
        "stderr was: {stderr}"
    );
    assert!(
        !stderr.contains("--timeout"),
        "no client-side bound was passed, so none may be reported: {stderr}"
    );
    Ok(())
}

// --- `--timeout` bounds the WAIT and leaves the run alive on the server. ---

#[test]
fn an_operator_timeout_stops_the_wait_and_leaves_the_run_running() -> TestResult {
    let server = LiveServer::boot()?;
    // The tracked fixture as-is: no declared timeout, parked on a signal that
    // never arrives — exactly the run that made the await loop unbounded.
    let document = fixture("step-bodies/valid/wait_and_sleep.awl");

    let output = server.aion(&[
        "run",
        document.to_str().ok_or("non-utf8 fixture path")?,
        "--input",
        r#"{"build_id":"b-1"}"#,
        "--timeout",
        SHORT_BOUND,
    ])?;
    let stdout = String::from_utf8(output.stdout)?;
    let stderr = String::from_utf8(output.stderr)?;

    assert_eq!(
        output.status.code(),
        Some(1),
        "a bounded wait that produced no result must exit 1\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.is_empty(), "stdout was: {stdout}");
    assert!(
        stderr.contains("stopped waiting after --timeout"),
        "stderr was: {stderr}"
    );

    // Survival check, paired with the absence above: the bound stops the WAIT,
    // never the run. Without this, a client that cancelled the workflow (or a
    // server that had died) would pass the assertions above identically.
    let workflow_id = reported_field(&stderr, "workflow_id")?;
    let run_id = reported_field(&stderr, "run_id")?;
    let described = server.aion(&["describe", &workflow_id, "--run-id", &run_id])?;
    assert_eq!(
        described.status.code(),
        Some(0),
        "describe failed: {}",
        String::from_utf8_lossy(&described.stderr)
    );
    let described: serde_json::Value = serde_json::from_slice(&described.stdout)?;
    assert_eq!(
        described["summary"]["status"], "Running",
        "the bound must stop the wait, not the run: {described}"
    );
    Ok(())
}

#[test]
fn the_run_timeout_flag_has_no_default() -> TestResult {
    // The house standard forbids an invented default. `--help` is the operator's
    // contract surface: clap prints `[default: …]` for any argument that has one.
    // The claim is scoped to `--timeout`'s own section — `aion run --help` also
    // lists the GLOBAL `--namespace`/`--subject`, which legitimately do default.
    let output = Command::new(env!("CARGO_BIN_EXE_aion"))
        .args(["run", "--help"])
        .output()?;
    let help = String::from_utf8(output.stdout)?;
    let section = help
        .split_once("--timeout <TIMEOUT>")
        .ok_or_else(|| format!("`aion run --help` never mentions --timeout:\n{help}"))?
        .1;
    // Everything up to the next option heading belongs to `--timeout`.
    let section = section.split("\n      --").next().unwrap_or(section);
    assert!(
        !section.contains("[default: "),
        "`--timeout` must declare no default: {section}"
    );
    Ok(())
}
