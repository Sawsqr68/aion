//! Full from-scaffold proof for the default AWL template: `aion new` → boot
//! the generated `aion.toml` → `aion deploy <name>.awl` → `aion run
//! <name>.awl` → completed typed output. Every step drives the real binary.

#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

type TestError = Box<dyn std::error::Error>;

const BOOT_DEADLINE: Duration = Duration::from_secs(60);
const GRPC_DEADLINE: Duration = Duration::from_secs(30);
const EXIT_DEADLINE: Duration = Duration::from_secs(60);

/// Reserve a loopback port by binding to port 0 and dropping the listener
/// (the standard fixture approach used across the workspace).
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

/// Re-point the generated config at reserved loopback ports. The store URL
/// stays the generated relative `aion.db`, which lands in the project
/// directory because the server runs with the project as its working
/// directory — exactly the README workflow.
/// The worker-channel address is rewritten too: the template's fixed
/// `127.0.0.1:50061` collides with any live worker on the machine (and with a
/// concurrent test run), and this server binds it at boot.
fn repoint_config_ports(
    project: &Path,
    http_port: u16,
    grpc_port: u16,
    outbox_port: u16,
) -> Result<(), TestError> {
    let config_path = project.join("aion.toml");
    let config = std::fs::read_to_string(&config_path)?;
    if !config.contains("127.0.0.1:8080")
        || !config.contains("127.0.0.1:50051")
        || !config.contains("127.0.0.1:50061")
    {
        return Err(format!(
            "generated aion.toml must carry the dev addresses and the worker channel:\n{config}"
        )
        .into());
    }
    let config = config
        .replace("127.0.0.1:8080", &format!("127.0.0.1:{http_port}"))
        .replace("127.0.0.1:50051", &format!("127.0.0.1:{grpc_port}"))
        .replace("127.0.0.1:50061", &format!("127.0.0.1:{outbox_port}"));
    std::fs::write(config_path, config)?;
    Ok(())
}

/// Boot `aion server --config aion.toml` from the project directory and wait
/// for the liveness probe.
fn boot_server(project: &Path, http_port: u16) -> Result<Child, TestError> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_aion"))
        .args(["server", "--config", "aion.toml"])
        .env(
            "AION_HOME",
            std::env::temp_dir().join(format!("aion-awl-e2e-home-{}", std::process::id())),
        )
        .current_dir(project)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Err(format!(
                "server exited during boot with {status}; output:\n{}",
                captured_output(&mut child)
            )
            .into());
        }
        if http_get_live(http_port).is_some_and(|response| response.starts_with("HTTP/1.1 200")) {
            return Ok(child);
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
    }
}

fn run_cli(current_dir: &Path, args: &[&str]) -> Result<Output, TestError> {
    Ok(Command::new(env!("CARGO_BIN_EXE_aion"))
        .args(args)
        .current_dir(current_dir)
        .output()?)
}

fn success_json(output: &Output) -> Result<serde_json::Value, TestError> {
    if output.status.code() != Some(0) {
        return Err(format!(
            "expected success, got {:?}; stdout: {} stderr: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn scaffold_project(parent: &Path, name: &str) -> Result<(PathBuf, serde_json::Value), TestError> {
    let output = run_cli(parent, &["new", name])?;
    let report = success_json(&output)?;
    Ok((parent.join(name), report))
}

/// Deploy the scaffolded AWL source, retrying while gRPC finishes binding.
fn deploy_source(project: &Path, endpoint: &str) -> Result<(), TestError> {
    let started = Instant::now();
    loop {
        let output = run_cli(project, &["--endpoint", endpoint, "deploy", "awl_demo.awl"])?;
        if output.status.code() == Some(0) {
            let report = success_json(&output)?;
            if report["workflow_type"] != "awl_demo" {
                return Err(format!("deploy must report the AWL workflow type: {report}").into());
            }
            return Ok(());
        }
        if started.elapsed() > GRPC_DEADLINE {
            return Err(format!(
                "deploy did not succeed within {GRPC_DEADLINE:?}; stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[test]
fn default_awl_scaffold_deploys_and_runs_to_completion() -> Result<(), TestError> {
    let temp_dir = tempfile::tempdir()?;
    let (project, report) = scaffold_project(temp_dir.path(), "awl_demo")?;
    assert_eq!(report["template"], "awl", "no flag must select AWL");
    assert!(project.join("awl_demo.awl").is_file());
    assert!(!project.join("gleam.toml").exists());
    assert!(!project.join("workflow.toml").exists());

    let http_port = reserve_port()?;
    let grpc_port = reserve_port()?;
    let outbox_port = reserve_port()?;
    repoint_config_ports(&project, http_port, grpc_port, outbox_port)?;
    let mut server = boot_server(&project, http_port)?;
    let endpoint = format!("127.0.0.1:{grpc_port}");

    let result = deploy_and_run(&project, &endpoint);

    // Shutdown regardless of the verdict so the failure path reports the
    // assertion, not a leaked child.
    let term = Command::new("kill")
        .args(["-TERM", &server.id().to_string()])
        .status()?;
    assert!(term.success(), "failed to deliver SIGTERM");
    let exit_code = wait_for_exit(&mut server, EXIT_DEADLINE)?;
    result?;
    assert_eq!(exit_code, Some(0), "graceful drain must exit 0");
    Ok(())
}

fn deploy_and_run(project: &Path, endpoint: &str) -> Result<(), TestError> {
    deploy_source(project, endpoint)?;
    let output = run_cli(
        project,
        &[
            "--endpoint",
            endpoint,
            "run",
            "awl_demo.awl",
            "--input",
            r#"{"name":"Ada","enthusiastic":true}"#,
        ],
    )?;
    let report = success_json(&output)?;
    assert_eq!(report["workflow_type"], "awl_demo");
    assert_eq!(report["result"]["outcome"], "greeted");
    assert_eq!(report["result"]["payload"]["message"], "Hello, Ada!");
    Ok(())
}
