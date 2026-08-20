//! Issue #36, verbatim, against real processes: a LIVE builder is paused, the
//! server it was dispatched from dies, a fresh server boots over the same
//! durable store, the run is resumed — and it completes through the SAME agent
//! session, with no `superseded:server-death` record anywhere in its history.
//!
//! # Why this gate exists
//!
//! Every other proof in this lane observes one seam. This one observes the
//! incident: `aion server` + `aion worker agent` + a real `norn` child + the
//! durable store, driven only through the product's own commands. The incident
//! record (issue #36) is the script:
//!
//! 1. a run's agent activity is dispatched and the agent goes live;
//! 2. the run is paused;
//! 3. the server is `kill -9`'d and a fresh one boots over the same store;
//! 4. the run is resumed;
//! 5. **then**: attempt 1 was failed as `superseded:server-death` while its
//!    builder was still working, the resume re-dispatched into that same live
//!    builder, the spawn gate refused it as an occupied tree, the retry ladder
//!    exhausted, and the run Failed with its agent still running.
//!    **Now**: recovery offers the dangling attempt for adoption, the worker
//!    joins the execution it already owns, and the run completes.
//!
//! Both faces of the incident run here. The PAUSED face is the recorded one
//! (paused at boot, which is why recovery never examined it). The RUNNING face
//! is the same incident's scope-widening observation — no pause anywhere, just
//! a server generation ending under a live builder — and it is recovered with
//! no operator action at all. The record must be identical either way: that
//! equality is the leg-4 requirement.
//!
//! # Isolation
//!
//! Nothing here can reach the estate. The server binary is the one cargo built
//! from THIS tree (`CARGO_BIN_EXE_aion`), never an installed `aion`. Its
//! `AION_HOME`, its store, its three ports, the agent's `HOME`/XDG dirs and the
//! agent's working tree are all created fresh under this test's own directory
//! inside the workspace target dir. The agent talks to a mock provider bound on
//! loopback by this test — no network service, no credentials, no shared state.
//!
//! # The provider gate is the fixture's clock
//!
//! The mock provider accepts the agent's request and WITHHOLDS its response
//! until the test releases it. That is what makes the builder verifiably live
//! across the pause and the restart: the agent is mid-call, holding its tree,
//! exactly as the estate's builders were.
//!
//! # Runtime gate
//!
//! A real `norn` executable is the one input this test cannot build: set
//! `NORN_BIN` to it, as the sibling `aion-integration-norn` e2es do. Without it
//! the gate announces, by name, the claim it did not establish.

#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

type TestError = Box<dyn std::error::Error>;

/// The environment variable naming the real `norn` executable.
const NORN_BIN_ENV: &str = "NORN_BIN";
/// The token the announcement carries when the agent binary is absent.
const SKIP_TOKEN: &str = "ISSUE36_E2E_SKIPPED";
/// The workflow type this document declares, and its start target.
const WORKFLOW_TYPE: &str = "issue36_resume";
/// The agent action's name — and therefore the `{activity_type}` half of the
/// pinned session id.
const ACTIVITY_TYPE: &str = "ask";
/// The variable the mock provider's key is read from. Named, not inherited: the
/// agent's environment is the declared one.
const KEY_ENV: &str = "AION_ISSUE36_KEY";
/// The exact recorded reason #36's seq 6 carried. Asserted as literal bytes so
/// a constant rename cannot quietly change what history says.
const SERVER_DEATH_REASON: &str = "superseded:server-death";
/// The spawn gate's tree marker, whose pid names the live builder.
const MARKER: &str = ".aion-held-execution.json";

/// How long a server gets to answer its liveness probe.
const BOOT_DEADLINE: Duration = Duration::from_secs(90);
/// How long the worker gets to register and the run to start.
const REGISTER_DEADLINE: Duration = Duration::from_secs(120);
/// How long a state transition (agent live, status change, completion) gets.
const PHASE_DEADLINE: Duration = Duration::from_secs(120);
/// How long a reaped child gets to exit.
const EXIT_DEADLINE: Duration = Duration::from_secs(30);

/// The mock provider's streamed reply: one content chunk, then a stop.
const SSE_BODY: &str = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"adopted\"},\"finish_reason\":null}]}\n\n\
     data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":1}}\n\n\
     data: [DONE]\n\n";

/// A loopback provider that counts what it accepted and answers only once
/// released.
struct GatedProvider {
    /// The base URL the agent is configured with.
    base_url: String,
    /// How many requests have been read in full.
    accepted: Arc<AtomicUsize>,
    /// Whether responses may be written.
    released: Arc<AtomicBool>,
}

impl GatedProvider {
    /// Bind the provider and start serving in the background.
    fn start() -> Result<Self, TestError> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let base_url = format!("http://{}", listener.local_addr()?);
        let accepted = Arc::new(AtomicUsize::new(0));
        let released = Arc::new(AtomicBool::new(false));
        let thread_accepted = Arc::clone(&accepted);
        let thread_released = Arc::clone(&released);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let accepted = Arc::clone(&thread_accepted);
                let released = Arc::clone(&thread_released);
                std::thread::spawn(move || serve_when_released(stream, &accepted, &released));
            }
        });
        Ok(Self {
            base_url,
            accepted,
            released,
        })
    }

    /// Let every held and future request answer.
    fn release(&self) {
        self.released.store(true, Ordering::SeqCst);
    }

    /// How many requests the agent has made.
    fn accepted(&self) -> usize {
        self.accepted.load(Ordering::SeqCst)
    }
}

/// Read one HTTP request, count it, hold until released, then stream the reply.
///
/// Errors are dropped deliberately: this runs on a detached connection thread,
/// and every observable it feeds (the accepted count, the agent's own progress)
/// is asserted by the test with its own deadline and its own message. A panic
/// here would report a broken pipe instead of the state the test is waiting for.
fn serve_when_released(mut stream: TcpStream, accepted: &AtomicUsize, released: &AtomicBool) {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => return,
            Ok(read) => request.extend_from_slice(&buffer[..read]),
        }
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    accepted.fetch_add(1, Ordering::SeqCst);
    while !released.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        SSE_BODY.len(),
        SSE_BODY
    );
    let _written = stream.write_all(response.as_bytes());
    let _flushed = stream.flush();
}

/// Kills its child on every exit path, so a failed assertion never leaks a
/// server, a worker, or an agent.
struct ChildGuard {
    /// The supervised process.
    child: Child,
    /// What it is, for failure messages.
    name: &'static str,
}

impl ChildGuard {
    /// Adopt `child` under `name`.
    fn new(child: Child, name: &'static str) -> Self {
        Self { child, name }
    }

    /// This process's id.
    fn id(&self) -> u32 {
        self.child.id()
    }

    /// Fail if the process has already exited — a dead worker explains every
    /// later timeout, and saying so beats burning a deadline.
    fn require_alive(&mut self) -> Result<(), TestError> {
        match self.child.try_wait()? {
            Some(status) => Err(format!("{} exited early with {status}", self.name).into()),
            None => Ok(()),
        }
    }

    /// `kill -9` it and wait for the corpse, the way the memory guard did.
    fn kill9(&mut self) -> Result<(), TestError> {
        let status = Command::new("/bin/kill")
            .args(["-KILL", &self.child.id().to_string()])
            .status()?;
        if !status.success() {
            return Err(format!("could not SIGKILL the {}", self.name).into());
        }
        self.child.wait()?;
        Ok(())
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.child.try_wait().is_ok_and(|status| status.is_none()) {
            let _killed = self.child.kill();
            let _reaped = self.child.wait();
        }
    }
}

/// Reserve a loopback port by binding it and letting the listener go.
fn reserve_port() -> Result<u16, TestError> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

/// Poll `predicate` until it answers `Some` or `deadline` elapses.
fn wait_for<T, F: FnMut() -> Result<Option<T>, TestError>>(
    deadline: Duration,
    what: &str,
    mut predicate: F,
) -> Result<T, TestError> {
    let started = Instant::now();
    loop {
        if let Some(value) = predicate()? {
            return Ok(value);
        }
        if started.elapsed() > deadline {
            return Err(format!("timed out after {deadline:?} waiting for {what}").into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Issue a raw `GET /health/live` and return the response text.
fn probe_live(http_port: u16) -> Option<String> {
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

/// The server config: reserved ports, the durable store under this run's own
/// home, the liminal worker channel the agent worker dials, deploy enabled
/// so `aion deploy` can reach it, and the two keys that are required with no
/// default — the server refuses to boot without either: the transcript flush
/// policy (`observability.max_batch_events` / `max_batch_hold_ms`) and the
/// haematite node-cache ceiling (`store.node_cache_budget`).
fn server_config(http: u16, grpc: u16, liminal: u16) -> String {
    format!(
        "[server]\n\
         listen_address = \"127.0.0.1:{http}\"\n\
         grpc_address = \"127.0.0.1:{grpc}\"\n\
         \n[store]\n\
         backend = \"haematite\"\n\
         # Unlimited: this e2e measures adoption across a server death, never caching.\n\
         node_cache_budget = \"unlimited\"\n\
         lock_acquisition_patience_ms = 250\n\
         lock_acquisition_retry_cadence_ms = 5\n\
         \n[runtime]\n\
         query_timeout_ms = 10000\n\
         \n[outbox]\n\
         enabled = true\n\
         transport = \"liminal\"\n\
         liminal_listen_address = \"127.0.0.1:{liminal}\"\n\
         \n[websocket]\n\
         event_broadcast_capacity = 1024\n\
         cluster_broadcast_capacity = 64\n\
         \n[deploy]\n\
         enabled = true\n\
         max_archive_bytes = 16777216\n\
         max_inflated_bytes = 67108864\n\
         \n[observability]\n\
         max_batch_events = 64\n\
         max_batch_hold_ms = 0\n"
    )
}

/// The worker document under test: one agent action on its own queue, launched
/// as a real `norn` against the mock provider, with the session pinned to
/// `{workflow_id}-{activity_type}` and `--resume-if-exists` — the estate's own
/// register (`crates/aion-server/assistant-embed/assistant.awl`), which is what
/// makes "one session" an observable rather than an aspiration.
///
/// The retry ladder is declared because #36's death ran through one: a refused
/// re-dispatch retried until the budget was spent. Short backoffs keep the
/// mutation control's red cheap without changing its shape.
fn worker_document(norn_binary: &Path, agent_tree: &Path, provider_base_url: &str) -> String {
    format!(
        "//! Issue #36 acceptance fixture: one agent action, one pinned session.\n\
         workflow {WORKFLOW_TYPE}\n\
         \x20 timeout 30m\n\
         \x20 input question: String\n\
         \x20 outcome answered: type Answer, route success\n\
         \n\
         type Answer {{ reply: String, session: String }}\n\
         \n\
         type Reply {{ text: String, final_message: String, stop_reason: String, session_id: String }}\n\
         \n\
         worker {WORKFLOW_TYPE}\n\
         \x20 harness\n\
         \x20   kind norn\n\
         \x20   concurrency 1\n\
         \x20   reconnect_initial_backoff 1s\n\
         \x20   reconnect_max_backoff 5s\n\
         \x20   reconnect_max_attempts 100\n\
         \x20   env_pass \"PATH\", \"HOME\", \"XDG_CONFIG_HOME\", \"XDG_DATA_HOME\", \"{KEY_ENV}\"\n\
         \x20   args \"--session-id\", \"{{workflow_id}}-{{activity_type}}\", \"--resume-if-exists\", \"--provider\", \"openai-compatible\", \"--model\", \"mock-model\", \"-c\", \"base_url={provider_base_url}\", \"-c\", \"api_key_env={KEY_ENV}\", \"-c\", \"max_retries=0\", \"-c\", \"context_window=131072\"\n\
         \x20   cwd \"{tree}\"\n\
         \x20   binary \"{binary}\"\n\
         \x20 action {ACTIVITY_TYPE}(prompt: String) -> Reply\n\
         \x20   agent, timeout 20m, retry 3 backoff 1s..5s\n\
         \n\
         step {ACTIVITY_TYPE}\n\
         \x20 {ACTIVITY_TYPE}(prompt: question) -> reply\n\
         \x20 route answered(reply: reply.text, session: reply.session_id)\n",
        tree = agent_tree.display(),
        binary = norn_binary.display(),
    )
}

/// Run the cargo-built `aion` from `dir` with this run's isolated home.
fn aion(dir: &Path, home: &Path, args: &[&str]) -> Result<Output, TestError> {
    Ok(Command::new(env!("CARGO_BIN_EXE_aion"))
        .args(args)
        .current_dir(dir)
        .env("AION_HOME", home)
        .output()?)
}

/// Run `aion` and parse its stdout as JSON, or say why there is none.
fn aion_json(dir: &Path, home: &Path, args: &[&str]) -> Result<serde_json::Value, String> {
    let rendered = args.join(" ");
    let output = aion(dir, home, args).map_err(|error| format!("`aion {rendered}`: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "`aion {rendered}` exited {status}; stderr:\n{stderr}",
            status = output.status,
            stderr = String::from_utf8_lossy(&output.stderr),
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|error| {
        format!(
            "`aion {rendered}` stdout was not JSON ({error}):\n{stdout}",
            stdout = String::from_utf8_lossy(&output.stdout),
        )
    })
}

/// Boot `aion server --config aion.toml` and wait for its liveness probe.
fn boot_server(
    dir: &Path,
    home: &Path,
    http_port: u16,
    log_path: &Path,
) -> Result<ChildGuard, TestError> {
    let log = std::fs::File::create(log_path)?;
    let duplicate = log.try_clone()?;
    let child = Command::new(env!("CARGO_BIN_EXE_aion"))
        .args(["server", "--config", "aion.toml"])
        .current_dir(dir)
        .env("AION_HOME", home)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(duplicate))
        .spawn()?;
    let mut guard = ChildGuard::new(child, "server");
    wait_for(BOOT_DEADLINE, "the server's liveness probe", || {
        guard.require_alive().map_err(|error| -> TestError {
            format!(
                "{error}; server log:\n{}",
                std::fs::read_to_string(log_path).unwrap_or_default()
            )
            .into()
        })?;
        Ok(probe_live(http_port)
            .filter(|response| response.starts_with("HTTP/1.1 200"))
            .map(|_| ()))
    })?;
    Ok(guard)
}

/// The parsed history entries of a described run.
fn history(description: &serde_json::Value) -> Vec<&serde_json::Value> {
    description
        .get("history")
        .and_then(serde_json::Value::as_array)
        .map(|events| events.iter().collect())
        .unwrap_or_default()
}

/// How many history entries carry `event_type`.
fn count_events(description: &serde_json::Value, event_type: &str) -> usize {
    history(description)
        .iter()
        .filter(|event| event.get("type").and_then(serde_json::Value::as_str) == Some(event_type))
        .count()
}

/// One persisted agent session's identity: its id and when it was OPENED.
///
/// `created_at` is what tells adoption from replacement. The session id is
/// templated on the workflow id, so a fresh spawn under the same template would
/// answer the same id — the two outcomes would coincide. The open time does
/// not: a session that was resumed carries the instant the FIRST invocation
/// opened it, and a session that was re-created carries the second's.
type SessionIdentity = (String, String);

/// Every persisted agent session in the isolated agent home, as `(id, opened)`.
fn listed_sessions(
    binary: &Path,
    tree: &Path,
    agent_home: &Path,
) -> Result<Vec<SessionIdentity>, TestError> {
    let output = Command::new(binary)
        .args(["session", "list", "--all", "--format", "json"])
        .current_dir(tree)
        .env("HOME", agent_home)
        .env("XDG_CONFIG_HOME", agent_home.join(".config"))
        .env("XDG_DATA_HOME", agent_home.join(".local/share"))
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "`norn session list` failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let rows: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let rows = rows
        .as_array()
        .ok_or("`norn session list --format json` did not answer an array")?;
    Ok(rows
        .iter()
        .filter_map(|row| {
            let id = row.get("id").and_then(serde_json::Value::as_str)?;
            let opened = row.get("created_at").and_then(serde_json::Value::as_str)?;
            Some((id.to_owned(), opened.to_owned()))
        })
        .collect())
}

/// The pid the spawn gate's marker names, once the agent holds the tree.
fn marker_pid(tree: &Path) -> Result<u32, TestError> {
    let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(tree.join(MARKER))?)?;
    let pid = marker
        .get("pid")
        .and_then(serde_json::Value::as_u64)
        .ok_or("the spawn marker carries no numeric pid")?;
    Ok(u32::try_from(pid)?)
}

/// Whether `pid` is still a live process.
fn process_is_live(pid: u32) -> bool {
    // `output()` rather than `status()`: a liveness probe of an already-reaped
    // pid writes "No such process" to the inherited stderr, and a poll loop's
    // expected answer must not print like a fault.
    Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .output()
        .is_ok_and(|output| output.status.success())
}

/// The status a describe payload reports, if it carries one.
fn status_of(description: &serde_json::Value) -> Option<&str> {
    description
        .get("summary")
        .and_then(|summary| summary.get("status"))
        .and_then(serde_json::Value::as_str)
}

/// The isolated estate one face of the incident runs inside.
///
/// Held together so every step addresses the SAME home, ports, document and
/// agent tree — the isolation is a property of this value, not of each call
/// remembering to pass the right path.
struct Isolation {
    /// The run's directory. Held so it outlives every child that works in it.
    work: tempfile::TempDir,
    /// The server's `AION_HOME`, and therefore its durable store.
    server_home: PathBuf,
    /// The agent's `HOME`, and therefore where its sessions persist.
    agent_home: PathBuf,
    /// The tree the agent stands in, and where the spawn gate's marker lands.
    agent_tree: PathBuf,
    /// The gRPC address every client command is aimed at.
    endpoint: String,
    /// The liminal worker channel the agent worker dials.
    liminal_address: String,
    /// The HTTP port the liveness probe reads.
    http_port: u16,
    /// The real agent executable under test.
    norn_binary: PathBuf,
    /// The worker document, as an argument.
    document_argument: String,
    /// The provider holding the agent mid-call.
    provider: GatedProvider,
}

impl Isolation {
    /// Create the directories, reserve the ports, and write the config and the
    /// worker document. Nothing is started yet.
    fn provision(norn_binary: PathBuf) -> Result<Self, TestError> {
        let work = tempfile::TempDir::new_in(env!("CARGO_TARGET_TMPDIR"))?;
        let root = work.path().to_path_buf();
        let server_home = root.join("server-home");
        let agent_home = root.join("agent-home");
        let agent_tree = root.join("agent-tree");
        for directory in [&server_home, &agent_home, &agent_tree] {
            std::fs::create_dir_all(directory)?;
        }

        let http_port = reserve_port()?;
        let grpc_port = reserve_port()?;
        let liminal_port = reserve_port()?;
        std::fs::write(
            root.join("aion.toml"),
            server_config(http_port, grpc_port, liminal_port),
        )?;

        let provider = GatedProvider::start()?;
        let document = root.join("issue36.awl");
        std::fs::write(
            &document,
            worker_document(&norn_binary, &agent_tree, &provider.base_url),
        )?;

        Ok(Self {
            work,
            server_home,
            agent_home,
            agent_tree,
            endpoint: format!("127.0.0.1:{grpc_port}"),
            liminal_address: format!("127.0.0.1:{liminal_port}"),
            http_port,
            norn_binary,
            document_argument: document.to_string_lossy().into_owned(),
            provider,
        })
    }

    /// This run's directory.
    fn root(&self) -> &Path {
        self.work.path()
    }

    /// Run one `aion` client command against this run's server.
    fn cli(&self, args: &[&str]) -> Result<serde_json::Value, String> {
        let mut full = vec!["--endpoint", self.endpoint.as_str()];
        full.extend_from_slice(args);
        aion_json(self.root(), &self.server_home, &full)
    }

    /// Boot a server generation, logging to `log_name` in the run directory.
    fn boot(&self, log_name: &str) -> Result<ChildGuard, TestError> {
        boot_server(
            self.root(),
            &self.server_home,
            self.http_port,
            &self.root().join(log_name),
        )
    }

    /// Start the agent worker: this run's document, this run's liminal address,
    /// and the agent's own isolated environment.
    fn spawn_worker(&self) -> Result<ChildGuard, TestError> {
        let child = Command::new(env!("CARGO_BIN_EXE_aion"))
            .args([
                "worker",
                "agent",
                &self.document_argument,
                "--liminal-address",
                &self.liminal_address,
                "--identity",
                "issue36-agent-worker",
            ])
            .current_dir(self.root())
            .env("AION_HOME", &self.server_home)
            .env("HOME", &self.agent_home)
            .env("XDG_CONFIG_HOME", self.agent_home.join(".config"))
            .env("XDG_DATA_HOME", self.agent_home.join(".local/share"))
            .env(KEY_ENV, "unused-mock-key")
            .stdin(Stdio::null())
            .stdout(Stdio::from(std::fs::File::create(
                self.root().join("worker.log"),
            )?))
            .stderr(Stdio::from(std::fs::File::create(
                self.root().join("worker.err.log"),
            )?))
            .spawn()?;
        Ok(ChildGuard::new(child, "agent worker"))
    }

    /// Every persisted agent session in this run's agent home.
    fn sessions(&self) -> Result<Vec<SessionIdentity>, TestError> {
        listed_sessions(&self.norn_binary, &self.agent_tree, &self.agent_home)
    }

    /// Poll `describe` until the run reports `status`.
    fn wait_for_status(
        &self,
        workflow_id: &str,
        status: &str,
        what: &str,
    ) -> Result<(), TestError> {
        wait_for(PHASE_DEADLINE, what, || {
            Ok(self
                .cli(&["describe", workflow_id])
                .ok()
                .filter(|description| status_of(description) == Some(status))
                .map(|_| ()))
        })
    }
}

/// What the live builder looks like once it is holding the tree mid-call.
struct LiveBuilder {
    /// The run the builder belongs to.
    workflow_id: String,
    /// The session id pinned by the harness template.
    session: String,
    /// The agent process the spawn marker names.
    pid: u32,
    /// When the agent OPENED its session — the pre-capture half of the
    /// continuity proof.
    opened_at: String,
}

/// Start the run and wait until its agent is verifiably live: registered,
/// dispatched, and held by the provider mid-call.
///
/// The engine fails an activity terminally when NO worker serves its type, so
/// the run cannot start before the worker registers. The START is retried, not
/// the run: a run that died for want of a worker is not this test's subject.
fn start_under_a_live_builder(
    isolation: &Isolation,
    server: &mut ChildGuard,
    worker: &mut ChildGuard,
) -> Result<LiveBuilder, TestError> {
    let workflow_id = wait_for(
        REGISTER_DEADLINE,
        "the worker to register and a run to start",
        || {
            worker.require_alive()?;
            server.require_alive()?;
            match isolation.cli(&[
                "start",
                WORKFLOW_TYPE,
                "--input",
                "{\"question\":\"say adopted\"}",
            ]) {
                Ok(value) => Ok(value
                    .get("workflow_id")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)),
                Err(refusal) if refusal.contains("no connected worker") => Ok(None),
                Err(refusal) => Err(refusal.into()),
            }
        },
    )?;
    let session = format!("{workflow_id}-{ACTIVITY_TYPE}");

    wait_for(PHASE_DEADLINE, "the agent to reach the provider", || {
        worker.require_alive()?;
        Ok((isolation.provider.accepted() >= 1).then_some(()))
    })?;

    let pid = marker_pid(&isolation.agent_tree)?;
    let opened = isolation.sessions()?;
    let [(opened_id, opened_at)] = opened.as_slice() else {
        return Err(
            format!("the live builder must have opened exactly one session: {opened:?}").into(),
        );
    };
    assert_eq!(
        opened_id, &session,
        "the builder's session must be the pinned one"
    );
    Ok(LiveBuilder {
        workflow_id,
        session,
        pid,
        opened_at: opened_at.clone(),
    })
}

/// Everything #36 demands of the durable record after the crossing.
fn assert_the_adopted_record(
    description: &serde_json::Value,
    builder: &LiveBuilder,
    face: &str,
) -> Result<(), TestError> {
    let rendered = serde_json::to_string_pretty(description)?;
    assert_eq!(
        status_of(description),
        Some("Completed"),
        "the {face} run must complete, not die against its own builder:\n{rendered}"
    );
    assert!(
        !rendered.contains(SERVER_DEATH_REASON),
        "a live builder must never be recorded as superseded by server death:\n{rendered}"
    );
    assert_eq!(
        count_events(description, "ActivityAdoptionOffered"),
        1,
        "recovery must record exactly one adoption offer for the dangling attempt:\n{rendered}"
    );
    assert_eq!(
        count_events(description, "ActivityCompleted"),
        1,
        "the adopted execution must settle exactly once:\n{rendered}"
    );
    // The attempt that settles is the RETAINED one. A false supersession would
    // spend attempt 1 and land the completion on a successor, so this number is
    // the difference between adopting the live builder and replacing it.
    assert_eq!(
        history(description)
            .iter()
            .filter(|event| {
                event.get("type").and_then(serde_json::Value::as_str) == Some("ActivityCompleted")
            })
            .filter_map(|event| event
                .get("data")
                .and_then(|data| data.get("attempt"))
                .and_then(serde_json::Value::as_u64))
            .collect::<Vec<_>>(),
        vec![1],
        "the dangling attempt itself must complete, not a successor:\n{rendered}"
    );
    assert!(
        rendered.contains(&builder.session),
        "the run's own record must carry the pinned session id {session}:\n{rendered}",
        session = builder.session
    );
    Ok(())
}

/// Stop the worker so it drains its own agent, then make sure the agent is
/// really gone before the run directory is dropped.
fn drain(worker: &ChildGuard, builder: &LiveBuilder) -> Result<(), TestError> {
    let _terminated = Command::new("/bin/kill")
        .args(["-TERM", &worker.id().to_string()])
        .output()?;
    let deadline = Instant::now() + EXIT_DEADLINE;
    while Instant::now() < deadline && process_is_live(builder.pid) {
        std::thread::sleep(Duration::from_millis(100));
    }
    if process_is_live(builder.pid) {
        let _killed = Command::new("/bin/kill")
            .args(["-KILL", &builder.pid.to_string()])
            .output()?;
    }
    Ok(())
}

/// #36's Paused face, verbatim: pause → server death → restart → resume.
///
/// This is the incident as it was recorded — the run was Paused at boot, which
/// is why recovery never examined it and the live builder was disowned.
#[test]
fn a_paused_run_resumed_across_a_server_death_adopts_its_live_builder() -> Result<(), TestError> {
    incident_across_a_server_death(true)
}

/// #36's Running face, from the same incident's scope-widening observation: no
/// pause anywhere in the story, just a server generation ending under a live
/// builder. Recovery must offer the dangling attempt for adoption here exactly
/// as it does for a Paused history — the leg-4 requirement, at the layer the
/// incident was observed on.
#[test]
fn a_running_run_recovered_across_a_server_death_adopts_its_live_builder() -> Result<(), TestError>
{
    incident_across_a_server_death(false)
}

/// The incident, with `paused` selecting which face of it runs.
fn incident_across_a_server_death(paused: bool) -> Result<(), TestError> {
    let face = if paused { "paused" } else { "running" };
    let Some(norn_binary) = std::env::var_os(NORN_BIN_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    else {
        let mut stderr = std::io::stderr();
        let _announced = writeln!(
            stderr,
            "{SKIP_TOKEN} (PROVES NOTHING): issue #36's {face} restart repro against a \
             real agent — not established by this run. Set {NORN_BIN_ENV} to a real agent \
             executable to run it."
        );
        return Ok(());
    };

    let isolation = Isolation::provision(norn_binary)?;

    // ---- The first server generation: the one the incident's builder was
    // dispatched from.
    let mut server = isolation.boot("server-1.log")?;
    isolation
        .cli(&["deploy", &isolation.document_argument])
        .map_err(TestError::from)?;
    let mut worker = isolation.spawn_worker()?;
    let builder = start_under_a_live_builder(&isolation, &mut server, &mut worker)?;

    // ---- The incident, step by step. The Paused face crosses the boot as a
    // paused history (the one recovery never examined); the Running face
    // crosses it Running, and is recovered with no operator action at all.
    if paused {
        isolation
            .cli(&["pause", &builder.workflow_id])
            .map_err(TestError::from)?;
    }
    isolation.wait_for_status(
        &builder.workflow_id,
        if paused { "Paused" } else { "Running" },
        "the run to read back the status it must cross the boot with",
    )?;

    server.kill9()?;
    assert!(
        process_is_live(builder.pid),
        "fixture control: the builder must outlive the server that dispatched it"
    );

    let mut restarted = isolation.boot("server-2.log")?;
    if paused {
        isolation
            .cli(&["resume", &builder.workflow_id])
            .map_err(TestError::from)?;
    }

    // The held call is what kept the builder alive across all of that. Let it
    // answer: the run must now finish through the session it already had.
    isolation.provider.release();
    let description = wait_for(
        PHASE_DEADLINE,
        "the recovered run to reach a terminal",
        || {
            worker.require_alive()?;
            restarted.require_alive()?;
            Ok(isolation
                .cli(&["describe", &builder.workflow_id])
                .ok()
                .filter(|description| {
                    matches!(
                        status_of(description),
                        Some("Completed" | "Failed" | "Cancelled" | "TimedOut")
                    )
                }))
        },
    )?;

    assert_the_adopted_record(&description, &builder, face)?;

    // ---- And of the agent: ONE session, opened before the death, still the
    // same session afterwards.
    assert_eq!(
        isolation.sessions()?,
        vec![(builder.session.clone(), builder.opened_at.clone())],
        "the {face} run must continue the session its live builder opened, and open no other"
    );

    drain(&worker, &builder)
}
