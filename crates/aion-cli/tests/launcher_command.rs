//! End-to-end tests for the bare `aion` launcher: a bare invocation starts a
//! background server and exits 0 while the server keeps answering
//! `/health/live`; a second bare invocation reports the running server and
//! spawns nothing; a foreign process already holding the resolved port draws
//! a clear refusal before anything is spawned; and `--foreground` runs the
//! ordinary in-terminal server path.

#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

type TestError = Box<dyn std::error::Error>;

/// The launcher's own readiness budget is 60s; give the whole invocation
/// (spawn + first haematite boot + assistant install) headroom past it.
const LAUNCH_DEADLINE: Duration = Duration::from_secs(90);

/// A repeat invocation only probes the running server; it must be prompt.
const REPEAT_DEADLINE: Duration = Duration::from_secs(30);

/// How long the spawned background server gets to exit after SIGTERM.
const STOP_DEADLINE: Duration = Duration::from_secs(60);

/// Reserve a loopback port by binding to port 0 and dropping the listener.
fn reserve_port() -> Result<u16, TestError> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

/// Issue a raw `GET /health/live` and return the full HTTP response.
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

/// Spawn `aion` with the given launcher arguments and the server addresses
/// re-pointed through the environment (which both the launcher's resolution
/// and the spawned server overlay identically), so the test never touches
/// the machine's real 8080/50051/50061.
fn spawn_launcher(
    arguments: &[&str],
    working_dir: &Path,
    home: &Path,
    http_port: u16,
    grpc_port: u16,
    outbox_port: u16,
) -> Result<Child, TestError> {
    Ok(Command::new(env!("CARGO_BIN_EXE_aion"))
        .args(arguments)
        .env("AION_HOME", home)
        .env(
            "AION_SERVER_LISTEN_ADDRESS",
            format!("127.0.0.1:{http_port}"),
        )
        .env("AION_SERVER_GRPC_ADDRESS", format!("127.0.0.1:{grpc_port}"))
        .env(
            "AION_OUTBOX_LIMINAL_LISTEN_ADDRESS",
            format!("127.0.0.1:{outbox_port}"),
        )
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?)
}

/// Wait for a launcher invocation to exit within `deadline` and collect its
/// output; kill it and fail loudly when it does not.
fn wait_for_output(mut child: Child, deadline: Duration) -> Result<Output, TestError> {
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return Ok(child.wait_with_output()?);
        }
        if started.elapsed() > deadline {
            child.kill()?;
            let output = child.wait_with_output()?;
            return Err(format!(
                "launcher did not exit within {deadline:?}; stdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Find a server pid in the launcher's report. Both shapes are recognised —
/// the success line (`server process id: <pid> …`) and the never-ready line
/// (`… running as process <pid> …`) — so the teardown guard covers every
/// path on which a server was left running.
fn find_reported_pid(report: &str) -> Option<i32> {
    for marker in ["server process id: ", "running as process "] {
        if let Some(position) = report.find(marker) {
            let digits: String = report[position + marker.len()..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            if let Ok(pid) = digits.parse() {
                return Some(pid);
            }
        }
    }
    None
}

/// Kills the background server on drop so a failed assertion cannot leak a
/// process. Constructed IMMEDIATELY after the launcher's output is collected
/// — before the first assertion — from whatever pid the report carries; the
/// happy path stops the server deliberately through [`ServerGuard::stop`].
struct ServerGuard {
    pid: Option<i32>,
    stopped: bool,
}

impl ServerGuard {
    fn from_report(stdout: &str, stderr: &str) -> Self {
        Self {
            pid: find_reported_pid(stdout).or_else(|| find_reported_pid(stderr)),
            stopped: false,
        }
    }

    /// Deliver SIGTERM and wait for `/health/live` to stop answering.
    fn stop(mut self, http_port: u16) -> Result<(), TestError> {
        let Some(pid) = self.pid else {
            return Err("no server pid was reported to stop".into());
        };
        let term = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()?;
        if !term.success() {
            return Err(format!("failed to deliver SIGTERM to {pid}").into());
        }
        let started = Instant::now();
        while http_get_live(http_port).is_some() {
            if started.elapsed() > STOP_DEADLINE {
                return Err(format!(
                    "server pid {pid} still answered /health/live {STOP_DEADLINE:?} after SIGTERM"
                )
                .into());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        self.stopped = true;
        Ok(())
    }
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        // Best-effort teardown for the failure paths; the happy path already
        // stopped the process, and a signal to a dead pid is harmless.
        if !self.stopped
            && let Some(pid) = self.pid
        {
            let _ = Command::new("kill")
                .args(["-KILL", &pid.to_string()])
                .status();
        }
    }
}

/// Kills a directly spawned child on drop, so a failed assertion cannot leak
/// the in-process (`--foreground`) server either.
struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The launcher's whole newcomer path: a bare `aion` (here with `--no-open`
/// so no browser opens on the test box) exits 0 promptly, leaves a server
/// answering `/health/live`, reports the scaffolded config, the console URL,
/// and the log path in plain language, and a second bare invocation reports
/// the running server without spawning another.
#[test]
fn bare_launch_starts_a_background_server_and_repeats_idempotently() -> Result<(), TestError> {
    use std::os::unix::fs::PermissionsExt as _;

    let temp_dir = tempfile::tempdir()?;
    let home = temp_dir.path().join("aion-home");
    let http_port = reserve_port()?;
    let grpc_port = reserve_port()?;
    let outbox_port = reserve_port()?;

    let child = spawn_launcher(
        &["--no-open"],
        temp_dir.path(),
        &home,
        http_port,
        grpc_port,
        outbox_port,
    )?;
    let output = wait_for_output(child, LAUNCH_DEADLINE)?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    // The kill-on-drop guard exists BEFORE the first assertion: however this
    // test fails from here on, a spawned server does not outlive it.
    let guard = ServerGuard::from_report(&stdout, &stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "bare launch must exit 0; stdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // The plain report: scaffolded config, console URL, log path.
    let home_config = home.join("config.toml");
    assert!(
        stdout.contains("aion is up."),
        "report must open with the plain status; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains(&format!("config: created {}", home_config.display())),
        "report must name the scaffolded config; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains(&format!("console: http://127.0.0.1:{http_port}/")),
        "report must name the console URL; stdout:\n{stdout}"
    );
    let log_path = home.join("server.log");
    assert!(
        stdout.contains(&format!("log: {}", log_path.display())),
        "report must name the log file; stdout:\n{stdout}"
    );

    // The launcher exited, but the server it spawned is live and durable
    // state landed where the report says.
    let response = http_get_live(http_port)
        .ok_or("server stopped answering /health/live after the launcher exited")?;
    assert!(
        response.starts_with("HTTP/1.1 200"),
        "liveness probe must answer 200: {response}"
    );
    assert!(
        home_config.exists(),
        "the server's first boot must scaffold {}",
        home_config.display()
    );
    let config_mode = std::fs::metadata(&home_config)?.permissions().mode() & 0o777;
    assert_eq!(
        config_mode, 0o600,
        "the scaffolded config must be owner-only"
    );
    let log_metadata = std::fs::metadata(&log_path)?;
    assert!(
        log_metadata.len() > 0,
        "the server log must carry the server's own output"
    );
    assert_eq!(
        log_metadata.permissions().mode() & 0o777,
        0o600,
        "the server log must be created owner-only"
    );

    // Second bare invocation: reports the running server, spawns nothing,
    // exits 0 promptly.
    let repeat = spawn_launcher(
        &["--no-open"],
        temp_dir.path(),
        &home,
        http_port,
        grpc_port,
        outbox_port,
    )?;
    let repeat_output = wait_for_output(repeat, REPEAT_DEADLINE)?;
    let repeat_stdout = String::from_utf8_lossy(&repeat_output.stdout);
    assert_eq!(
        repeat_output.status.code(),
        Some(0),
        "repeat launch must exit 0; stdout:\n{repeat_stdout}"
    );
    assert!(
        repeat_stdout.contains("aion is already running."),
        "repeat launch must report the running server; stdout:\n{repeat_stdout}"
    );
    assert!(
        !repeat_stdout.contains("server process id:"),
        "repeat launch must not spawn (or report) a second server; stdout:\n{repeat_stdout}"
    );

    // The same server is still the one serving.
    assert!(
        http_get_live(http_port).is_some_and(|body| body.starts_with("HTTP/1.1 200")),
        "the original server must still be serving after the repeat invocation"
    );

    guard.stop(http_port)?;
    Ok(())
}

/// A foreign process already holding the resolved HTTP port draws a clear
/// refusal naming the port — before any server is spawned, so no config is
/// scaffolded and nothing is left running.
#[test]
fn a_foreign_listener_on_the_resolved_port_is_refused_by_name() -> Result<(), TestError> {
    let temp_dir = tempfile::tempdir()?;
    let home = temp_dir.path().join("aion-home");
    let grpc_port = reserve_port()?;
    let outbox_port = reserve_port()?;

    // A listener that answers HTTP, but not as an Aion server would.
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let http_port = listener.local_addr()?.port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut buffer = [0_u8; 1024];
            let _ = stream.read(&mut buffer);
            let _ = stream.write_all(
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
        }
    });

    let child = spawn_launcher(
        &["--no-open"],
        temp_dir.path(),
        &home,
        http_port,
        grpc_port,
        outbox_port,
    )?;
    let output = wait_for_output(child, REPEAT_DEADLINE)?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    // A regressed refusal path could have spawned a server; guard before the
    // first assertion, exactly as in the happy-path test.
    let _guard = ServerGuard::from_report(&stdout, &stderr);

    assert_eq!(
        output.status.code(),
        Some(2),
        "a held port must be refused with exit 2; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains(&format!("port {http_port}")) && stderr.contains("not an Aion server"),
        "the refusal must name the port and the foreign holder; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("404"),
        "the refusal must quote what the port answered; stderr:\n{stderr}"
    );
    assert!(
        !home.join("config.toml").exists(),
        "a refused launch must not scaffold a config"
    );
    Ok(())
}

/// `aion --foreground` is the ordinary in-terminal server path: the process
/// itself serves (no background spawn), answers `/health/live`, and drains
/// cleanly to exit 0 on SIGTERM.
#[test]
fn foreground_launch_serves_in_process_and_drains_on_sigterm() -> Result<(), TestError> {
    let temp_dir = tempfile::tempdir()?;
    let home = temp_dir.path().join("aion-home");
    let http_port = reserve_port()?;
    let grpc_port = reserve_port()?;
    let outbox_port = reserve_port()?;

    let child = spawn_launcher(
        &["--foreground", "--no-open"],
        temp_dir.path(),
        &home,
        http_port,
        grpc_port,
        outbox_port,
    )?;
    // Kill-on-drop from the moment the child exists: this child IS the
    // server, so no assertion failure below may leak it.
    let mut guard = ChildGuard(child);

    let started = Instant::now();
    loop {
        if let Some(status) = guard.0.try_wait()? {
            return Err(format!("foreground server exited during boot with {status}").into());
        }
        if http_get_live(http_port).is_some_and(|body| body.starts_with("HTTP/1.1 200")) {
            break;
        }
        if started.elapsed() > LAUNCH_DEADLINE {
            return Err("foreground server never answered /health/live".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    let term = Command::new("kill")
        .args(["-TERM", &guard.0.id().to_string()])
        .status()?;
    assert!(term.success(), "failed to deliver SIGTERM");
    let started = Instant::now();
    let code = loop {
        if let Some(status) = guard.0.try_wait()? {
            break status.code();
        }
        if started.elapsed() > STOP_DEADLINE {
            return Err("foreground server did not exit after SIGTERM".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(code, Some(0), "a drained foreground server must exit 0");
    Ok(())
}
