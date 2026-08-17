//! End-to-end smoke tests for the unified binary's `server` subcommand: the
//! `aion` executable boots the full workflow server from a `--config` file,
//! serves an HTTP request, and drains cleanly on SIGTERM with exit code 0;
//! a boot that discovers no config anywhere scaffolds `<AION_HOME>/config.toml`
//! from the embedded template (#180); and an explicit `--config` naming a
//! missing file still refuses without inventing one.

#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

type TestError = Box<dyn std::error::Error>;

const BOOT_DEADLINE: Duration = Duration::from_secs(60);
const EXIT_DEADLINE: Duration = Duration::from_secs(60);

/// Reserve a loopback port by binding to port 0 and dropping the listener.
///
/// A small race window exists between drop and the server's own bind; the
/// kernel does not reissue recently-bound ephemeral ports eagerly, so this
/// is the standard fixture approach used across the workspace.
fn reserve_port() -> Result<u16, TestError> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

fn write_config(directory: &Path, http_port: u16, grpc_port: u16) -> Result<String, TestError> {
    let config = format!(
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
"#
    );
    let path = directory.join("server-config.toml");
    std::fs::write(&path, config)?;
    Ok(path.to_string_lossy().into_owned())
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

fn captured_output(child: &mut Child) -> String {
    let mut combined = String::new();
    if let Some(mut stdout) = child.stdout.take() {
        let mut buffer = String::new();
        if stdout.read_to_string(&mut buffer).is_ok() {
            combined.push_str(&buffer);
        }
    }
    if let Some(mut stderr) = child.stderr.take() {
        let mut buffer = String::new();
        if stderr.read_to_string(&mut buffer).is_ok() {
            combined.push_str(&buffer);
        }
    }
    combined
}

fn wait_for_exit(child: &mut Child, deadline: Duration) -> Result<Option<i32>, TestError> {
    let started = Instant::now();
    while started.elapsed() < deadline {
        if let Some(status) = child.try_wait()? {
            return Ok(status.code());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    child.kill()?;
    Err("server did not exit within the drain deadline".into())
}

#[test]
fn server_subcommand_boots_serves_and_drains_cleanly() -> Result<(), TestError> {
    let temp_dir = tempfile::tempdir()?;
    let http_port = reserve_port()?;
    let grpc_port = reserve_port()?;
    let config_path = write_config(temp_dir.path(), http_port, grpc_port)?;

    // Run from the temp directory so package auto-discovery cannot pick up
    // stray `.aion` archives from the workspace.
    let mut child = Command::new(env!("CARGO_BIN_EXE_aion"))
        .args(["server", "--config", &config_path])
        .env(
            "AION_HOME",
            std::env::temp_dir().join(format!("aion-e2e-home-{}", std::process::id())),
        )
        .current_dir(temp_dir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Boot: poll the health probe until the HTTP transport answers.
    let started = Instant::now();
    let response = loop {
        if let Some(status) = child.try_wait()? {
            return Err(format!(
                "server exited during boot with {status}; output:\n{}",
                captured_output(&mut child)
            )
            .into());
        }
        if let Some(response) = http_get_live(http_port) {
            break response;
        }
        if started.elapsed() > BOOT_DEADLINE {
            child.kill()?;
            return Err(format!(
                "server did not answer /health/live within {BOOT_DEADLINE:?}; output:\n{}",
                captured_output(&mut child)
            )
            .into());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(
        response.starts_with("HTTP/1.1 200"),
        "liveness probe must answer 200: {response}"
    );

    // Shutdown: first SIGTERM drains gracefully and exits clean (code 0).
    let term = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()?;
    assert!(term.success(), "failed to deliver SIGTERM");
    let exit_code = wait_for_exit(&mut child, EXIT_DEADLINE)?;
    let output = captured_output(&mut child);
    assert_eq!(
        exit_code,
        Some(0),
        "graceful drain must exit 0; output:\n{output}"
    );
    assert!(
        output.contains("aion-server startup banner"),
        "startup banner must be logged; output:\n{output}"
    );
    assert!(
        output.contains("beginning graceful drain"),
        "drain start must be logged; output:\n{output}"
    );
    Ok(())
}

/// Kills a spawned server child on drop, so a failed assertion between spawn
/// and the deliberate SIGTERM teardown cannot leak a live server process.
struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Spawn a bare `aion server` (no `--config`) with its listener addresses
/// re-pointed through the environment, which overlays the scaffolded file,
/// so the test never touches the machine's real 8080/50051/50061.
fn spawn_bare_server(
    working_dir: &Path,
    home: &Path,
    http_port: u16,
    grpc_port: u16,
    outbox_port: u16,
) -> Result<Child, TestError> {
    Ok(Command::new(env!("CARGO_BIN_EXE_aion"))
        .arg("server")
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

/// Poll `/health/live` until the HTTP transport answers 200, failing loudly
/// with the child's output if it exits or the boot deadline passes.
fn wait_until_live(child: &mut Child, http_port: u16) -> Result<(), TestError> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Err(format!(
                "server exited during boot with {status}; output:\n{}",
                captured_output(child)
            )
            .into());
        }
        if http_get_live(http_port).is_some_and(|response| response.starts_with("HTTP/1.1 200")) {
            return Ok(());
        }
        if started.elapsed() > BOOT_DEADLINE {
            child.kill()?;
            return Err(format!(
                "server did not answer /health/live within {BOOT_DEADLINE:?}; output:\n{}",
                captured_output(child)
            )
            .into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Poll the liminal worker-channel port until it accepts a TCP connection.
fn wait_for_outbox_listener(child: &mut Child, outbox_port: u16) -> Result<(), TestError> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if TcpStream::connect_timeout(
            &([127, 0, 0, 1], outbox_port).into(),
            Duration::from_secs(1),
        )
        .is_ok()
        {
            return Ok(());
        }
        if Instant::now() > deadline {
            child.kill()?;
            return Err(format!(
                "the liminal worker listener never accepted on port {outbox_port}; output:\n{}",
                captured_output(child)
            )
            .into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// #180: a bare `aion server` — no `--config`, no `./aion.toml`, no
/// `<AION_HOME>/config.toml` — scaffolds the home config owner-only, boots on
/// it, and the two dark-by-default surfaces the template exists to light
/// (deploy and the outbox worker channel) come up.
#[test]
fn a_bare_boot_scaffolds_the_home_config_and_serves_deploy_and_outbox() -> Result<(), TestError> {
    use std::os::unix::fs::PermissionsExt as _;

    let temp_dir = tempfile::tempdir()?;
    let home = temp_dir.path().join("aion-home");
    let http_port = reserve_port()?;
    let grpc_port = reserve_port()?;
    let outbox_port = reserve_port()?;

    let child = spawn_bare_server(temp_dir.path(), &home, http_port, grpc_port, outbox_port)?;
    // Kill-on-drop from the moment the child exists: however an assertion
    // below fails, the spawned server does not outlive this test.
    let mut guard = ChildGuard(child);
    let child = &mut guard.0;
    wait_until_live(child, http_port)?;

    // The scaffolded file: present, owner-only, and byte-identical to the
    // server's embedded first-run template — no invented values, no partial
    // write. (Body-identity between that template and the `aion new` shared
    // template is pinned in aion-server's scaffold tests.)
    let config_path = home.join("config.toml");
    let scaffolded = std::fs::read_to_string(&config_path)?;
    assert_eq!(
        scaffolded,
        aion_server::config::FIRST_RUN_CONFIG,
        "the scaffolded config must be exactly the embedded first-run template"
    );
    assert_eq!(
        std::fs::metadata(&config_path)?.permissions().mode() & 0o777,
        0o600,
        "the scaffolded config must be owner-only"
    );

    // The outbox worker channel is live: the liminal listener accepts a
    // connection at the (env-re-pointed) address the config commissioned.
    wait_for_outbox_listener(child, outbox_port)?;

    let term = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()?;
    assert!(term.success(), "failed to deliver SIGTERM");
    let exit_code = wait_for_exit(child, EXIT_DEADLINE)?;
    let output = captured_output(child);
    assert_eq!(
        exit_code,
        Some(0),
        "graceful drain must exit 0; output:\n{output}"
    );
    assert!(
        output.contains("scaffolded a first-run config"),
        "the scaffold must announce itself with the path; output:\n{output}"
    );
    assert!(
        output.contains(&config_path.display().to_string()),
        "the scaffold log must name the written path; output:\n{output}"
    );
    assert!(
        output.contains("\"deploy_enabled\":true"),
        "the startup banner must show the deploy surface commissioned; output:\n{output}"
    );
    Ok(())
}

/// An explicit `--config` naming a missing file refuses (exit code 2, the
/// configuration-error contract) and scaffolds NOTHING: the operator named a
/// file, and inventing one would mask a typo.
#[test]
fn an_explicit_missing_config_refuses_and_scaffolds_nothing() -> Result<(), TestError> {
    let temp_dir = tempfile::tempdir()?;
    let home = temp_dir.path().join("aion-home");
    let missing = temp_dir.path().join("absent.toml");

    let output = Command::new(env!("CARGO_BIN_EXE_aion"))
        .args(["server", "--config"])
        .arg(&missing)
        .env("AION_HOME", &home)
        .current_dir(temp_dir.path())
        .output()?;

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "a missing explicit config must exit with the config-error code; output:\n{combined}"
    );
    assert!(
        combined.contains("absent.toml"),
        "the refusal must name the missing file; output:\n{combined}"
    );
    assert!(
        !home.join("config.toml").exists(),
        "a refused explicit config must not scaffold a home config"
    );
    Ok(())
}
