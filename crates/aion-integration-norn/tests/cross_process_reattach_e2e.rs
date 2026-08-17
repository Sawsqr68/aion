//! Cross-process #36 acceptance: kill the worker process while its real Norn
//! child is executing, redeliver the same workflow/activity through a
//! replacement worker, and prove both invocations name one persisted session.
//!
//! Runtime-gated like `norn_e2e`: set `NORN_BIN` to a real Norn executable.
//! The provider and every Norn state directory are isolated inside this test;
//! no network service, credentials, or user Norn home is used.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use aion_integration_norn::NornHarness;
use aion_integrations::{
    ActivityId, AgentHarness, AgentRunSpec, AgentSession, ContentType, HarnessWorkspace, Payload,
    RunId, WorkflowId,
};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::process::Command;
use tokio::sync::Notify;

const NORN_BIN_ENV: &str = "NORN_BIN";
const HELPER_ROLE_ENV: &str = "AION_NORN_REATTACH_HELPER";
const TREE_ENV: &str = "AION_NORN_REATTACH_TREE";
const HOME_ENV: &str = "AION_NORN_REATTACH_HOME";
const BASE_URL_ENV: &str = "AION_NORN_REATTACH_BASE_URL";
const WORKFLOW_ENV: &str = "AION_NORN_REATTACH_WORKFLOW";
const RUN_ENV: &str = "AION_NORN_REATTACH_RUN";
const READY_ENV: &str = "AION_NORN_REATTACH_READY";
const DONE_ENV: &str = "AION_NORN_REATTACH_DONE";
const KEY_ENV: &str = "AION_NORN_REATTACH_KEY";
const MARKER: &str = ".aion-held-execution.json";
const ACTIVITY_TYPE: &str = "norn-reattach";

const SSE_BODY: &str = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"reattached\"},\"finish_reason\":null}]}\n\n\
     data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":1}}\n\n\
     data: [DONE]\n\n";

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct ProviderGate {
    base_url: String,
    accepted: Arc<AtomicUsize>,
    released: Arc<AtomicBool>,
    release: Arc<Notify>,
}

async fn gated_provider() -> TestResult<ProviderGate> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let base_url = format!("http://{}", listener.local_addr()?);
    let accepted = Arc::new(AtomicUsize::new(0));
    let released = Arc::new(AtomicBool::new(false));
    let release = Arc::new(Notify::new());
    let task_accepted = Arc::clone(&accepted);
    let task_released = Arc::clone(&released);
    let task_release = Arc::clone(&release);
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let accepted = Arc::clone(&task_accepted);
            let released = Arc::clone(&task_released);
            let release = Arc::clone(&task_release);
            tokio::spawn(async move {
                let _ = serve_after_release(stream, accepted, released, release).await;
            });
        }
    });
    Ok(ProviderGate {
        base_url,
        accepted,
        released,
        release,
    })
}

async fn serve_after_release(
    mut stream: TcpStream,
    accepted: Arc<AtomicUsize>,
    released: Arc<AtomicBool>,
    release: Arc<Notify>,
) -> std::io::Result<()> {
    let mut request = Vec::new();
    let mut buf = [0_u8; 4096];
    loop {
        let read = stream.read(&mut buf).await?;
        if read == 0 {
            return Ok(());
        }
        request.extend_from_slice(&buf[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    accepted.fetch_add(1, Ordering::SeqCst);
    loop {
        let notified = release.notified();
        if released.load(Ordering::SeqCst) {
            break;
        }
        notified.await;
    }
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        SSE_BODY.len(),
        SSE_BODY
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await
}

fn configured_binary() -> Option<PathBuf> {
    std::env::var_os(NORN_BIN_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn session_id(workflow_id: &WorkflowId) -> String {
    format!("{workflow_id}-{ACTIVITY_TYPE}")
}

fn harness(binary: &Path, tree: &Path, home: &Path, base_url: &str) -> NornHarness {
    NornHarness::with_binary(binary, HarnessWorkspace::Fixed(tree.to_path_buf()))
        .with_arg("--provider")
        .with_arg("openai-compatible")
        .with_arg("--model")
        .with_arg("mock-model")
        .with_arg("-c")
        .with_arg(format!("base_url={base_url}"))
        .with_arg("-c")
        .with_arg(format!("api_key_env={KEY_ENV}"))
        .with_arg("-c")
        .with_arg("max_retries=0")
        .with_arg("-c")
        .with_arg("context_window=131072")
        .with_arg("--session-id")
        .with_arg(format!("{{workflow_id}}-{ACTIVITY_TYPE}"))
        .with_arg("--resume-if-exists")
        .with_env(KEY_ENV, "unused-mock-key")
        .with_env("HOME", home.to_string_lossy())
        .with_env("XDG_CONFIG_HOME", home.join(".config").to_string_lossy())
        .with_env("XDG_DATA_HOME", home.join(".local/share").to_string_lossy())
}

async fn worker_helper() -> TestResult {
    let binary = PathBuf::from(required_env(NORN_BIN_ENV)?);
    let tree = PathBuf::from(required_env(TREE_ENV)?);
    let home = PathBuf::from(required_env(HOME_ENV)?);
    let base_url = required_env(BASE_URL_ENV)?;
    let workflow_id = WorkflowId::new(uuid::Uuid::parse_str(&required_env(WORKFLOW_ENV)?)?);
    let run_id = RunId::new(uuid::Uuid::parse_str(&required_env(RUN_ENV)?)?);
    let ready = PathBuf::from(required_env(READY_ENV)?);
    let done = PathBuf::from(required_env(DONE_ENV)?);
    let spec = AgentRunSpec::new(
        workflow_id.clone(),
        run_id,
        ActivityId::from_sequence_position(1),
        1,
        ACTIVITY_TYPE,
        Payload::new(ContentType::Json, b"say reattached".to_vec()),
    );
    let session = harness(&binary, &tree, &home, &base_url)
        .start(spec)
        .await?;
    std::fs::write(&ready, session_id(&workflow_id))?;
    let result = session.wait_result().await?;
    std::fs::write(done, result.bytes())?;
    Ok(())
}

fn required_env(name: &str) -> TestResult<String> {
    std::env::var(name).map_err(|error| format!("missing helper env {name}: {error}").into())
}

#[derive(Clone, Copy)]
struct HelperLaunch<'a> {
    binary: &'a Path,
    tree: &'a Path,
    home: &'a Path,
    base_url: &'a str,
    workflow_id: &'a WorkflowId,
    run_id: &'a RunId,
    ready: &'a Path,
    done: &'a Path,
}

fn helper_command(launch: HelperLaunch<'_>) -> TestResult<Command> {
    let HelperLaunch {
        binary,
        tree,
        home,
        base_url,
        workflow_id,
        run_id,
        ready,
        done,
    } = launch;
    let current = std::env::current_exe()?;
    let mut command = Command::new(current);
    command
        .arg("--exact")
        .arg("replacement_worker_reattaches_the_surviving_norn_session")
        .arg("--nocapture")
        .env(HELPER_ROLE_ENV, "1")
        .env(NORN_BIN_ENV, binary)
        .env(TREE_ENV, tree)
        .env(HOME_ENV, home)
        .env(BASE_URL_ENV, base_url)
        .env(WORKFLOW_ENV, workflow_id.to_string())
        .env(RUN_ENV, run_id.to_string())
        .env(READY_ENV, ready)
        .env(DONE_ENV, done)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    Ok(command)
}

async fn wait_for_path(path: &Path, what: &str) -> TestResult {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if path.exists() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Err(format!("timed out waiting for {what} at {}", path.display()).into())
}

async fn wait_for_accepted(accepted: &AtomicUsize, minimum: usize) -> TestResult {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if accepted.load(Ordering::SeqCst) >= minimum {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Err(format!("provider never accepted request {minimum}").into())
}

fn marker_pid(tree: &Path) -> TestResult<u32> {
    let marker: Value = serde_json::from_slice(&std::fs::read(tree.join(MARKER))?)?;
    let pid = marker
        .get("pid")
        .and_then(Value::as_u64)
        .ok_or("spawn marker has no numeric pid")?;
    Ok(u32::try_from(pid)?)
}

fn process_is_live(pid: u32) -> bool {
    std::process::Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .status()
        .is_ok_and(|status| status.success())
}

fn listed_session_ids(binary: &Path, tree: &Path, home: &Path) -> TestResult<Vec<String>> {
    let output = std::process::Command::new(binary)
        .args(["session", "list", "--all", "--format", "json"])
        .current_dir(tree)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_DATA_HOME", home.join(".local/share"))
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "norn session list failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let rows: Value = serde_json::from_slice(&output.stdout)?;
    let array = rows
        .as_array()
        .ok_or("norn session list was not an array")?;
    Ok(array
        .iter()
        .filter_map(|row| row.get("id").and_then(Value::as_str).map(str::to_owned))
        .collect())
}

fn stop_orphan(pid: u32) {
    if process_is_live(pid) {
        let _ = std::process::Command::new("/bin/kill")
            .args(["-KILL", &pid.to_string()])
            .status();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn replacement_worker_reattaches_the_surviving_norn_session() -> TestResult {
    if std::env::var_os(HELPER_ROLE_ENV).is_some() {
        return worker_helper().await;
    }
    let Some(binary) = configured_binary() else {
        tracing::info!("skipping cross-process reattach e2e: set {NORN_BIN_ENV}");
        return Ok(());
    };

    let root = tempfile::tempdir()?;
    let tree = root.path().join("tree");
    let home = root.path().join("home");
    std::fs::create_dir_all(&tree)?;
    std::fs::create_dir_all(&home)?;
    let provider = gated_provider().await?;
    let workflow_id = WorkflowId::new_v4();
    let run_id = RunId::new_v4();
    let expected_session = session_id(&workflow_id);

    let first_ready = root.path().join("first.ready");
    let first_done = root.path().join("first.done");
    let mut first = helper_command(HelperLaunch {
        binary: &binary,
        tree: &tree,
        home: &home,
        base_url: &provider.base_url,
        workflow_id: &workflow_id,
        run_id: &run_id,
        ready: &first_ready,
        done: &first_done,
    })?
    .spawn()?;
    wait_for_path(&first_ready, "the first worker to start Norn").await?;
    wait_for_accepted(&provider.accepted, 1).await?;
    let pre_session = std::fs::read_to_string(&first_ready)?;
    let surviving_pid = marker_pid(&tree)?;

    first.kill().await?;
    let first_status = first.wait().await?;
    assert!(
        !first_status.success(),
        "fixture control: worker one must be killed"
    );
    assert!(
        process_is_live(surviving_pid),
        "the Norn child must survive worker kill-9 before replacement dispatch"
    );

    let second_ready = root.path().join("second.ready");
    let second_done = root.path().join("second.done");
    let mut second = helper_command(HelperLaunch {
        binary: &binary,
        tree: &tree,
        home: &home,
        base_url: &provider.base_url,
        workflow_id: &workflow_id,
        run_id: &run_id,
        ready: &second_ready,
        done: &second_done,
    })?
    .spawn()?;
    wait_for_path(&second_ready, "the replacement worker to reattach").await?;
    let post_session = std::fs::read_to_string(&second_ready)?;

    let ids = listed_session_ids(&binary, &tree, &home)?;
    assert_eq!(pre_session, expected_session, "pre-kill session capture");
    assert_eq!(
        post_session, expected_session,
        "post-reattach session capture"
    );
    assert_eq!(
        ids.iter().filter(|id| *id == &expected_session).count(),
        1,
        "exactly one persisted Norn session must exist after adoption: {ids:?}"
    );

    provider.released.store(true, Ordering::SeqCst);
    provider.release.notify_waiters();
    let status = tokio::time::timeout(Duration::from_secs(30), second.wait()).await??;
    assert!(
        status.success(),
        "replacement worker must finish the resumed run"
    );
    wait_for_path(&second_done, "replacement result").await?;
    stop_orphan(surviving_pid);
    Ok(())
}
