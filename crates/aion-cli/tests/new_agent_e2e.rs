//! From-scaffold structural-admission proof for the `agent` template:
//! `aion new agent` (which now warns at mint) → `gleam build` →
//! `aion package` → boot `aion server` → `aion deploy` succeeds — and
//! `aion start` is REFUSED with the typed `NO_QUEUE_DECLARATION` contract
//! error, because the legacy Gleam template's implicit activity declarations
//! (act, scout, verify) cannot carry a queue-scoped `.v4` contract.
//!
//! This is the ruled behavior, not a regression: pre-`.v4` packages are not
//! grandfathered, and a package that names activities without a durable
//! queue-scoped contract must not start. The scout -> act -> verify -> review
//! run-flow proof this gate used to carry moves to the AWL agent scaffold:
//! the template-AWL-migration lane rewrites the template in AWL, and its gate
//! re-proves the full loop end to end.
//!
//! Every step drives the real `aion` binary. Runtime-gated: a missing `gleam`
//! toolchain prints an explicit skip line and returns `Ok(())`, so the suite
//! passes on a host without the Gleam toolchain rather than failing or
//! silently lying.

#![cfg(unix)]

#[path = "test_support/gleam.rs"]
mod gleam_test_support;

mod common;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::TestError;

const BOOT_DEADLINE: Duration = Duration::from_secs(60);
const GRPC_DEADLINE: Duration = Duration::from_secs(30);
const EXIT_DEADLINE: Duration = Duration::from_secs(60);

/// The project name used throughout this gate.
const PROJECT: &str = "agent_demo";

fn reserve_port() -> Result<u16, TestError> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
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
    Err("process did not exit within the deadline".into())
}

/// Kills the child on every exit path so a failed assertion never leaks a
/// server process.
struct ChildGuard {
    child: Child,
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.child.try_wait().is_ok_and(|s| s.is_none()) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// Re-point the generated config at reserved loopback ports, mirroring the
/// hello-world gate. The store stays the generated relative `aion.db`.
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

fn boot_server(project: &Path, http_port: u16) -> Result<Child, TestError> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_aion"))
        .args(["server", "--config", "aion.toml"])
        .env(
            "AION_HOME",
            std::env::temp_dir().join(format!("aion-e2e-home-{}", std::process::id())),
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

/// Deploy succeeds: the `.v4` archive carries its contract record, and the
/// structural refusal belongs to start admission, not to deploy.
fn deploy_archive(project: &Path, endpoint: &str) -> Result<(), TestError> {
    let started = Instant::now();
    loop {
        let output = common::run_cli(
            project,
            &["--endpoint", endpoint, "deploy", &format!("{PROJECT}.aion")],
        )?;
        if output.status.code() == Some(0) {
            let body: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            if body["workflow_type"] != PROJECT {
                return Err(format!("deploy must report the workflow type: {body}").into());
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

/// `aion start` against the deployed legacy package is refused with the
/// typed `NO_QUEUE_DECLARATION` message naming the unscoped activities and
/// the fix, and the refusal reaches stderr — never stdout.
fn start_is_refused_no_queue_declaration(project: &Path, endpoint: &str) -> Result<(), TestError> {
    let input = serde_json::json!({
        "task_id": "refused-task",
        "scout_prompt": "survey",
        "act_prompt": "do",
        "verify_prompt": "check",
        "review_timeout_ms": 60_000,
    })
    .to_string();
    let output = common::run_cli(
        project,
        &["--endpoint", endpoint, "start", PROJECT, "--input", &input],
    )?;
    if output.status.code() == Some(0) {
        return Err(format!(
            "start must refuse the unscoped legacy package, but succeeded: {}",
            String::from_utf8_lossy(&output.stdout)
        )
        .into());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    for marker in [
        "NO_QUEUE_DECLARATION",
        &format!("workflow `{PROJECT}`"),
        "unscoped activities act,scout,verify",
        "re-deploy from a checked AWL contract",
    ] {
        if !stderr.contains(marker) {
            return Err(format!("the refusal must carry {marker:?}; stderr:\n{stderr}").into());
        }
    }
    if !output.stdout.is_empty() {
        return Err(format!(
            "a refused start must write nothing to stdout, got: {}",
            String::from_utf8_lossy(&output.stdout)
        )
        .into());
    }
    Ok(())
}

#[test]
fn scaffolded_agent_deploys_but_start_is_refused_unscoped() -> Result<(), TestError> {
    // `eprintln!` is captured by libtest and shown only for a FAILING test, so
    // the skip this replaced was indistinguishable from a pass in the log. The
    // shared gate writes to the real stderr handle instead.
    if crate::gleam_test_support::skip_if_unavailable() {
        return Ok(());
    }

    let temp_dir = tempfile::tempdir()?;
    let (project, report) = common::scaffold_project(
        temp_dir.path(),
        PROJECT,
        &["--template", "agent", "--worker", "rust"],
    )?;
    if report["template"] != "agent" || report["worker"] != "rust" {
        return Err(
            format!("scaffold report must name the agent template and worker: {report}").into(),
        );
    }
    // Mint-time loudness through the real binary: the scaffold report itself
    // must carry the this-will-be-refused warning naming the AWL successor.
    let warning = report["warning"]
        .as_str()
        .ok_or("the agent scaffold report must carry the mint-time refusal warning")?;
    for marker in ["NO_QUEUE_DECLARATION", "AWL agent scaffold"] {
        if !warning.contains(marker) {
            return Err(format!("the mint warning must carry {marker:?}: {warning}").into());
        }
    }

    common::patch_aion_flow_to_workspace(&project)?;
    common::gleam_build(&project)?;
    common::package_project(&project, PROJECT)?;

    let http_port = reserve_port()?;
    let grpc_port = reserve_port()?;
    let outbox_port = reserve_port()?;
    repoint_config_ports(&project, http_port, grpc_port, outbox_port)?;
    let mut server = ChildGuard {
        child: boot_server(&project, http_port)?,
    };
    let endpoint = format!("127.0.0.1:{grpc_port}");

    let result = (|| -> Result<(), TestError> {
        deploy_archive(&project, &endpoint)?;
        start_is_refused_no_queue_declaration(&project, &endpoint)
    })();

    let term = Command::new("kill")
        .args(["-TERM", &server.child.id().to_string()])
        .status()?;
    if !term.success() {
        return Err("failed to deliver SIGTERM to the server".into());
    }
    let exit_code = wait_for_exit(&mut server.child, EXIT_DEADLINE)?;
    result?;
    if exit_code != Some(0) {
        return Err(format!("graceful drain must exit 0, got {exit_code:?}").into());
    }
    Ok(())
}
