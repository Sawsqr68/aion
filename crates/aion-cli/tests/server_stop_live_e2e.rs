//! Live end-to-end gate for `aion server stop` / `aion server status`.
//!
//! Drives the REAL `aion` binary against a REAL `aion server` child on
//! ephemeral ports, with a PRIVATE `AION_HOME` per test — the production
//! death note is shared by every build that runs against a home, so these
//! specimens must never write into a real one.
//!
//! Faces proven here: the live clean stop (pid record found, incarnation
//! verified, SIGTERM, drain outcome rendered from the death note), the
//! kill -9 honesty face (bracket never closed, no fabricated summary, stale
//! record reconciled), and the status layers for a running and a stopped
//! server.

#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

type TestError = Box<dyn std::error::Error>;
type TestResult = Result<(), TestError>;

const BOOT_DEADLINE: Duration = Duration::from_secs(60);
/// Ceiling on one control-verb invocation. The verbs bound their own waits
/// (`--patience`); this is the harness's leak detector, not an assertion.
const VERB_DEADLINE: Duration = Duration::from_secs(60);

fn reserve_port() -> Result<u16, TestError> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

/// A real `aion server` child on ephemeral ports with a private home, torn
/// down by the test itself (on `Drop`, so an assertion failure never leaks
/// the process).
struct LiveServer {
    child: Child,
    http_port: u16,
    grpc_port: u16,
    home: PathBuf,
    config: PathBuf,
    server_log: PathBuf,
    workspace: tempfile::TempDir,
}

impl LiveServer {
    fn boot() -> Result<Self, TestError> {
        Self::boot_with_drain_seconds(30)
    }

    fn boot_with_drain_seconds(drain_timeout_seconds: u64) -> Result<Self, TestError> {
        let workspace = tempfile::tempdir()?;
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

[drain]
timeout_seconds = {drain_timeout_seconds}

[namespaces]
default = "default"

[websocket]
event_broadcast_capacity = 1024
cluster_broadcast_capacity = 64

[observability]
max_batch_events = 64
max_batch_hold_ms = 0

[deploy]
enabled = true
max_archive_bytes = 16777216
max_inflated_bytes = 67108864
"#
            ),
        )?;
        let home = workspace.path().join("aion-home");
        let server_log = workspace.path().join("server.log");
        let log_handle = std::fs::File::create(&server_log)?;
        let mut child = Command::new(env!("CARGO_BIN_EXE_aion"))
            .args([
                "server",
                "--config",
                config.to_str().ok_or("non-utf8 config path")?,
            ])
            .env("AION_HOME", &home)
            .current_dir(workspace.path())
            .stdout(Stdio::from(log_handle.try_clone()?))
            .stderr(Stdio::from(log_handle))
            .spawn()?;
        wait_for_boot(&mut child, http_port, &server_log)?;
        Ok(Self {
            child,
            http_port,
            grpc_port,
            home,
            config,
            server_log,
            workspace,
        })
    }

    /// Run a control verb (`server stop …` / `server status …`) with this
    /// server's private home, to completion.
    fn verb(&self, args: &[&str]) -> Result<Output, TestError> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_aion"))
            .args(args)
            .env("AION_HOME", &self.home)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let started = Instant::now();
        loop {
            if child.try_wait()?.is_some() {
                return Ok(child.wait_with_output()?);
            }
            if started.elapsed() > VERB_DEADLINE {
                child.kill()?;
                let _ = child.wait();
                return Err(
                    format!("`aion {args:?}` did not finish within {VERB_DEADLINE:?}").into(),
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Run an ordinary client verb against this server's gRPC endpoint.
    fn aion(&self, args: &[&str]) -> Result<Output, TestError> {
        let endpoint = format!("127.0.0.1:{}", self.grpc_port);
        let mut child = Command::new(env!("CARGO_BIN_EXE_aion"))
            .args(["--endpoint", &endpoint])
            .args(args)
            .current_dir(self.workspace.path())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let started = Instant::now();
        loop {
            if child.try_wait()?.is_some() {
                return Ok(child.wait_with_output()?);
            }
            if started.elapsed() > VERB_DEADLINE {
                child.kill()?;
                let _ = child.wait();
                return Err(
                    format!("`aion {args:?}` did not finish within {VERB_DEADLINE:?}").into(),
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn config_arg(&self) -> Result<&str, TestError> {
        self.config
            .to_str()
            .ok_or_else(|| "non-utf8 config path".into())
    }

    fn pid_file(&self) -> PathBuf {
        self.home.join("run").join("aion-server.pid")
    }

    /// Wait for the reaped child so the log file holds everything it wrote.
    fn wait_reaped(&mut self) -> Result<(), TestError> {
        let started = Instant::now();
        loop {
            if self.child.try_wait()?.is_some() {
                return Ok(());
            }
            if started.elapsed() > VERB_DEADLINE {
                return Err("server child did not exit".into());
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

fn wait_for_boot(child: &mut Child, http_port: u16, server_log: &Path) -> Result<(), TestError> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Err(format!(
                "server exited during boot with {status}{}",
                dying_words(server_log)
            )
            .into());
        }
        if let Some(response) = http_get_live(http_port)
            && response.starts_with("HTTP/1.1 200")
        {
            return Ok(());
        }
        if started.elapsed() > BOOT_DEADLINE {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "server did not answer /health/live within {BOOT_DEADLINE:?}{}",
                dying_words(server_log)
            )
            .into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn dying_words(log: &Path) -> String {
    match std::fs::read_to_string(log) {
        Err(error) => format!(
            "; the server log at {} was unreadable: {error}",
            log.display()
        ),
        Ok(text) if text.trim().is_empty() => {
            format!("; the server log at {} is empty", log.display())
        }
        Ok(text) => {
            let tail: Vec<&str> = text.lines().rev().take(15).collect();
            let tail: Vec<&str> = tail.into_iter().rev().collect();
            format!("; server log tail:\n{}", tail.join("\n"))
        }
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The live clean stop: the verb finds the record, verifies the incarnation,
/// signals, waits out the drain, and renders the drain outcome — and the pid
/// file is gone afterwards.
#[test]
fn a_live_server_is_stopped_and_the_drain_outcome_is_rendered() -> TestResult {
    let mut server = LiveServer::boot()?;
    assert!(
        server.pid_file().exists(),
        "the pid record must exist once the server has bound"
    );

    let config = server.config_arg()?.to_owned();
    let output = server.verb(&["server", "stop", "--config", &config])?;
    let stdout = text(&output.stdout);
    let stderr = text(&output.stderr);
    assert!(
        output.status.success(),
        "stop must succeed against a live server; stdout:\n{stdout}\nstderr:\n{stderr}{}",
        dying_words(&server.server_log)
    );
    assert!(
        stdout.contains("stopped: pid"),
        "the report must name the stopped incarnation: {stdout}"
    );
    assert!(
        stdout.contains("drain outcome: Clean"),
        "an idle server's drain must render as Clean, from the outcome record: {stdout}"
    );
    server.wait_reaped()?;
    assert!(
        !server.pid_file().exists(),
        "the exiting server (or the verb's reconciliation) must remove the pid record"
    );
    Ok(())
}

/// Red first — the kill -9 honesty face: a server destroyed without its run
/// loop seeing the end yields a stop report that says exactly that, never a
/// fabricated drain summary, and reconciles the stale record away.
#[test]
fn a_kill_9_death_is_reported_honestly_never_fabricated() -> TestResult {
    let mut server = LiveServer::boot()?;
    let pid = server.child.id();
    // SIGKILL: the one death the note cannot record. Sent through the
    // platform's own `kill` so the test stays under the workspace's
    // unsafe-code denial.
    let killed = Command::new("kill")
        .args(["-9", &pid.to_string()])
        .status()?;
    assert!(killed.success(), "the SIGKILL must be delivered");
    server.wait_reaped()?;

    let config = server.config_arg()?.to_owned();
    let output = server.verb(&["server", "stop", "--config", &config])?;
    let stdout = text(&output.stdout);
    assert!(
        output.status.success(),
        "reconciling an already-dead server is a success, not an error: {stdout}"
    );
    assert!(
        stdout.contains("already gone"),
        "the report must say nothing was signalled: {stdout}"
    );
    assert!(
        stdout.contains("never closed"),
        "the kill -9 shape must be named (bracket never closed): {stdout}"
    );
    assert!(
        !stdout.contains("drain outcome:"),
        "no drain outcome may be fabricated for a SIGKILL death: {stdout}"
    );
    assert!(
        !server.pid_file().exists(),
        "the stale record must be reconciled away"
    );

    // The second stop faces an unclaimed home and refuses by name.
    let second = server.verb(&["server", "stop", "--config", &config])?;
    assert_eq!(
        second.status.code(),
        Some(2),
        "a home with no record refuses with the no-pid-file face"
    );
    assert!(
        text(&second.stderr).contains("no pid file"),
        "the refusal names itself: {}",
        text(&second.stderr)
    );
    Ok(())
}

/// The Parked face is VISIBLE in the stop report: an in-flight declared
/// command outlives a tiny drain window, the drain parks it, and the verb
/// renders `Parked` — distinguishable from `Clean` — from the outcome
/// record the dying server wrote.
#[test]
fn a_parked_drain_is_visible_in_the_stop_report() -> TestResult {
    let mut server = LiveServer::boot_with_drain_seconds(2)?;
    let fixture = server.workspace.path().join("stop_linger.awl");
    // The command must be findable in the process table afterwards, and the
    // declared executor runs argv directly (no shell), so nothing but sleep's
    // own arguments can carry a marker. A duration unique to this test run IS
    // the marker: long enough to outlive any drain, unique enough that
    // `pgrep -f` can ask about exactly this sleeper and no other test's.
    let unique_duration = 600_000 + u64::from(std::process::id()) % 100_000;
    let marker = format!("sleep {unique_duration}");
    std::fs::write(
        &fixture,
        format!(
            r#"//! One long-lingering declared command, for the parked-drain specimen.
workflow stop_linger
  input name: String
  outcome done: type Done, route success

type Done {{ text: String }}
type RunOutcome {{ exit_code: Int, stdout: String, stderr: String }}

worker stop_linger_box
  action linger(name: String) -> RunOutcome
    run "sleep {unique_duration}"

step linger
  linger(name: name) -> lingered
  route done(text: lingered.stdout)
"#
        ),
    )?;
    let deploy = server.aion(&["deploy", "stop_linger.awl"])?;
    assert!(
        deploy.status.success(),
        "precondition: the linger document deploys; stdout:\n{}\nstderr:\n{}",
        text(&deploy.stdout),
        text(&deploy.stderr)
    );
    let start = server.aion(&["start", "stop_linger", "--input", r#"{"name":"parked"}"#])?;
    assert!(
        start.status.success(),
        "precondition: the linger workflow starts; stdout:\n{}\nstderr:\n{}",
        text(&start.stdout),
        text(&start.stderr)
    );
    // Give the declared command time to be dispatched and spawned; the stop
    // below then finds it in flight. The assertion is on the report, not on
    // this delay.
    std::thread::sleep(Duration::from_secs(3));

    let config = server.config_arg()?.to_owned();
    let output = server.verb(&["server", "stop", "--config", &config, "--patience", "30"])?;
    let stdout = text(&output.stdout);
    assert!(
        output.status.success(),
        "a parked drain is a successful stop; stdout:\n{stdout}\nstderr:\n{}{}",
        text(&output.stderr),
        dying_words(&server.server_log)
    );
    assert!(
        stdout.contains("drain outcome: Parked"),
        "the parked face must be visible in the report, distinct from Clean: {stdout}{}",
        dying_words(&server.server_log)
    );
    assert!(
        stdout.contains("declared command still executing at drain end:"),
        "the report must NAME the declared command the drain parked — an unnamed \
         park is the census gap this specimen exists to close: {stdout}{}",
        dying_words(&server.server_log)
    );
    server.wait_reaped()?;
    // The report's claim — "its process ended with the server; the next boot
    // re-dispatches the attempt" — is asserted against the process table, not
    // taken on faith: engine teardown must have ended the parked command's
    // group. Polled briefly because the group kill and the server's own exit
    // are not one atomic act.
    let orphan_deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let survey = Command::new("pgrep").args(["-f", &marker]).output()?;
        if !survey.status.success() {
            break; // pgrep exits non-zero when nothing matches: the group is gone.
        }
        if Instant::now() >= orphan_deadline {
            return Err(format!(
                "the parked declared command outlived the server: pgrep -f {marker} still \
                 matches:\n{}",
                text(&survey.stdout)
            )
            .into());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Ok(())
}

/// The middle face of the verb's three-value exit contract: patience elapses
/// while the server is still draining → exit 1 and a report that says so.
/// Faces 0 (stopped) and 2 (refusal) have their own specimens; this is the
/// one a deploy script keys on to distinguish "wait longer" from "failed".
#[test]
fn a_drain_outliving_patience_reports_still_draining_with_exit_one() -> TestResult {
    let server = LiveServer::boot_with_drain_seconds(600)?;
    let fixture = server.workspace.path().join("stop_slow.awl");
    std::fs::write(
        &fixture,
        r#"//! One long-lingering declared command, for the still-draining specimen.
workflow stop_slow
  input name: String
  outcome done: type Done, route success

type Done { text: String }
type RunOutcome { exit_code: Int, stdout: String, stderr: String }

worker stop_slow_box
  action linger(name: String) -> RunOutcome
    run "sleep 600"

step linger
  linger(name: name) -> lingered
  route done(text: lingered.stdout)
"#,
    )?;
    let deploy = server.aion(&["deploy", "stop_slow.awl"])?;
    assert!(
        deploy.status.success(),
        "precondition: the slow document deploys; stdout:\n{}\nstderr:\n{}",
        text(&deploy.stdout),
        text(&deploy.stderr)
    );
    let start = server.aion(&["start", "stop_slow", "--input", r#"{"name":"slow"}"#])?;
    assert!(
        start.status.success(),
        "precondition: the slow workflow starts; stdout:\n{}\nstderr:\n{}",
        text(&start.stdout),
        text(&start.stderr)
    );
    std::thread::sleep(Duration::from_secs(3));

    let config = server.config_arg()?.to_owned();
    let output = server.verb(&["server", "stop", "--config", &config, "--patience", "1"])?;
    let stdout = text(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(1),
        "a drain outliving the patience is exit 1 — not success, not a verb failure; \
         stdout:\n{stdout}\nstderr:\n{}{}",
        text(&output.stderr),
        dying_words(&server.server_log)
    );
    assert!(
        stdout.contains("still draining"),
        "the report must say the server is still draining: {stdout}"
    );
    assert!(
        stdout.contains("(--patience)"),
        "the report must name the flag as the patience source: {stdout}"
    );
    // The LiveServer drop ends the still-draining server; the drain itself
    // is not this specimen's subject beyond having outlived the patience.
    Ok(())
}

/// The status layers against a RUNNING server: record present and verified,
/// build identity read from the body, readiness answered.
#[test]
fn status_against_a_running_server_states_every_layer() -> TestResult {
    let server = LiveServer::boot()?;
    let config = server.config_arg()?.to_owned();
    let output = server.verb(&["server", "status", "--config", &config])?;
    let stdout = text(&output.stdout);
    assert!(output.status.success(), "status must compose: {stdout}");
    assert!(
        stdout.contains("pid file: pid"),
        "the record layer must render: {stdout}"
    );
    assert!(
        stdout.contains("RUNNING, incarnation verified"),
        "the incarnation must verify against the live process: {stdout}"
    );
    assert!(
        stdout.contains(&format!("build: {}", env!("CARGO_PKG_VERSION"))),
        "the build layer must carry the version read from the BODY: {stdout}"
    );
    assert!(
        stdout.contains("ready: YES"),
        "the readiness layer must answer: {stdout}"
    );
    // The probe target is the recorded bind, not a config re-derivation.
    assert!(
        stdout.contains(&format!("127.0.0.1:{}", server.http_port)),
        "status must probe the address the record says was bound: {stdout}"
    );
    Ok(())
}

/// The status absence shape against a STOPPED (cleanly exited) server: no
/// record, and the probe against the configured address finds no listener —
/// each stated as its own layer, neither as the other.
#[test]
fn status_against_a_stopped_server_states_the_absences() -> TestResult {
    let mut server = LiveServer::boot()?;
    let config = server.config_arg()?.to_owned();
    let stop = server.verb(&["server", "stop", "--config", &config])?;
    assert!(stop.status.success(), "precondition: the stop succeeds");
    server.wait_reaped()?;

    let output = server.verb(&["server", "status", "--config", &config])?;
    let stdout = text(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(1),
        "no running server is exit 1 — the absence is still REPORTED in full, but \
         a script asking `is the server up?` must get a no: {stdout}"
    );
    assert!(
        stdout.contains("pid file: none"),
        "the record absence must be stated: {stdout}"
    );
    assert!(
        stdout.contains("CONFIGURED address"),
        "with no record, the probe source must be named as the config: {stdout}"
    );
    assert!(
        stdout.contains("build: unreachable"),
        "the dead listener must be its own state: {stdout}"
    );
    Ok(())
}
