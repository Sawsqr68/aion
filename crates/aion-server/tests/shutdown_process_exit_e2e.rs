//! #72 — `aion server` must EXIT after a graceful drain, not merely drain.
//!
//! Nothing in this suite asserted that before. `graceful_shutdown_park_e2e` and
//! the drain unit tests observe the drain: they prove it parks in-flight work
//! and returns the right `ShutdownOutcome`. All of that passed while the process
//! it belonged to never exited, because the hang happens AFTER the outcome is
//! chosen — after `state.shutdown()`, after `main` returns an exit code — inside
//! tokio's runtime `Drop`, which joins the blocking pool. The last thing an
//! operator ever saw was `activity drain completed cleanly`, and the corpse kept
//! the store's writer lock, so the next deploy into the same data directory was
//! refused by a process that had reported success. The captured log and stack of
//! a real one are in `docs/evidence/samples/`.
//!
//! So this pin is at PROCESS level, and it is deliberately about the one thing a
//! library test cannot see: reaping. A child process arms the exact wedge — one
//! activity dispatch parked on a queue with no worker and no schedule-to-start
//! deadline, sitting on a blocking-pool thread — the parent signals it, and the
//! only assertion is that the operating system reaps it.
//!
//! The child is a re-exec of this same test binary (`current_exe` plus
//! `AION_SHUTDOWN_EXIT_CHILD`), so the wedge is armed by the real
//! `aion-server` types, with no extra binary or example to keep in step. It
//! builds a real `ServerState`, wires the bridge dispatcher over that state's
//! own seams exactly as `state::build_bridge_dispatcher` does at boot, and
//! drains through the real `shutdown::drain_after_first_signal` — the exact
//! function `run_server`'s SIGTERM arm calls. What it does not stand up is the
//! gRPC/HTTP transports: they are not on the path between the drain and the
//! blocking pool, and the captured sample was taken from a process whose
//! transports had already exited.
//!
//! Unix only, and it must stay that way: SIGTERM is the signal the contract is
//! written against, and `run_server`'s own listener is `#[cfg(unix)]` for the
//! same reason.
#![cfg(unix)]

use std::io::BufRead;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use aion::{ActivityDispatch, ActivityDispatcher as _};
use aion_core::{ActivityId, RunId, WorkflowId};
use aion_server::ServerState;
use aion_server::config::{
    AuthConfig, AuthoringConfig, AutoCreate, DEFAULT_MAX_IN_FLIGHT_ACTIVITIES, DeployConfig,
    DevConfig, ListenConfig, MetricsConfig, NamespaceConfig, NamespaceMode, ObservabilityConfig,
    OpsConsoleAssetSource, OpsConsoleConfig, OutboxConfig, ResolvedMcpConfig, RuntimeConfig,
    WebSocketConfig, WorkerConfig,
};
use aion_server::shutdown::{self, ShutdownOutcome};
use aion_server::worker::WorkerActivityDispatcher;
use aion_store::InMemoryStore;

type TestError = Box<dyn std::error::Error>;

/// Set on the child only; selects the child role of [`parked_dispatch_child`].
const CHILD_ENV: &str = "AION_SHUTDOWN_EXIT_CHILD";
/// The child-role test the parent re-execs, by exact libtest name.
const CHILD_TEST: &str = "parked_dispatch_child";
/// Printed by the child once the dispatch is observably parked AND the SIGTERM
/// listener is installed. The parent signals only after reading it.
const ARMED_MARKER: &str = "AION-72-PARKED-DISPATCH-ARMED";

/// A queue no worker serves and no deployed contract declares. With no
/// declarations installed the classification is `NO_LIVE_POLLERS` — a fleet
/// condition, not a structural refusal — which is the state that parks.
const UNSERVED_QUEUE: &str = "nobody-serves-this";
const NAMESPACE: &str = "default";
const ACTIVITY_TYPE: &str = "greet";

/// How long the parent waits for the child to arm the wedge. Generous: the child
/// boots a real engine first.
const ARM_DEADLINE: Duration = Duration::from_secs(90);
/// How long the parent waits for the signalled child to be reaped. This bound
/// belongs to the TEST, not to the server: the fix is that the wait becomes
/// wakeable, not that some timeout expires. A correct child exits in
/// milliseconds; the defect never exits at all, so any bound distinguishes them
/// and this one only decides how long a failure takes to report.
const EXIT_DEADLINE: Duration = Duration::from_secs(30);
/// Reap poll cadence.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// The pin: a server with a dispatch parked on an unserved queue is reaped after
/// SIGTERM.
///
/// Failure modes it separates, so a red run says which one happened: the child
/// never armed the wedge (setup), the child armed it and was never reaped (the
/// #72 defect), or the child was reaped with a failing status (its own
/// assertions, whose output is quoted).
#[test]
fn server_process_exits_after_sigterm_with_a_dispatch_parked_on_an_unserved_queue()
-> Result<(), TestError> {
    if std::env::var_os(CHILD_ENV).is_some() {
        // Belt and braces: the parent re-execs `--exact CHILD_TEST`, so this
        // cannot run in the child. If it ever did, it must not fork again.
        return Err("the parent pin must never run in the child role".into());
    }
    let mut child = Command::new(std::env::current_exe()?)
        .arg(CHILD_TEST)
        .args(["--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or("piped child stdout was not captured")?;
    let stderr = child
        .stderr
        .take()
        .ok_or("piped child stderr was not captured")?;
    // BOTH streams are read, and both for the same two reasons. Diagnostics: a
    // child panic goes to stderr under `--nocapture`, so a report quoting only
    // stdout would drop the child's own account of its failure. Liveness: an
    // unread pipe fills at the OS buffer and blocks the writer, which would wedge
    // the child before it ever armed the wedge this pin exists to arm.
    let (lines_tx, lines) = channel();
    let readers = [
        spawn_reader(stdout, "stdout", lines_tx.clone()),
        spawn_reader(stderr, "stderr", lines_tx),
    ];

    let armed = await_armed(&lines, &mut child);
    if let Err(error) = armed {
        kill_and_reap(&mut child);
        let transcript = drain_transcript(&lines, readers);
        return Err(format!("{error}\nchild output so far:\n{transcript}").into());
    }

    let pid = rustix::process::Pid::from_raw(i32::try_from(child.id())?)
        .ok_or("child reported pid 0, which cannot be signalled")?;
    rustix::process::kill_process(pid, rustix::process::Signal::TERM)?;

    let signalled_at = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if signalled_at.elapsed() >= EXIT_DEADLINE {
            kill_and_reap(&mut child);
            let transcript = drain_transcript(&lines, readers);
            return Err(format!(
                "#72: the server did not exit within {EXIT_DEADLINE:?} of SIGTERM while one \
                 activity dispatch was parked on the unserved queue `{UNSERVED_QUEUE}`. The drain \
                 can report success and the process still never be reaped — that is the whole \
                 defect.\nchild output:\n{transcript}"
            )
            .into());
        }
        std::thread::sleep(POLL_INTERVAL);
    };
    let transcript = drain_transcript(&lines, readers);
    if !status.success() {
        return Err(format!(
            "the child exited with {status} rather than success; its own assertions are the \
             report.\nchild output:\n{transcript}"
        )
        .into());
    }
    Ok(())
}

/// Block until the child prints [`ARMED_MARKER`], or explain what it printed
/// instead.
fn await_armed(lines: &Receiver<String>, child: &mut std::process::Child) -> Result<(), TestError> {
    let started = Instant::now();
    loop {
        let remaining =
            ARM_DEADLINE
                .checked_sub(started.elapsed())
                .ok_or_else(|| -> TestError {
                    format!("the child never armed the parked dispatch within {ARM_DEADLINE:?}")
                        .into()
                })?;
        match lines.recv_timeout(remaining.min(POLL_INTERVAL)) {
            Ok(line) if line.contains(ARMED_MARKER) => return Ok(()),
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout) => {
                if let Some(status) = child.try_wait()? {
                    return Err(format!(
                        "the child exited with {status} before arming the parked dispatch"
                    )
                    .into());
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err("the child's stdout closed before it armed the parked dispatch".into());
            }
        }
    }
}

/// Forward one of the child's streams into the shared transcript channel,
/// labelled with which stream it came from.
fn spawn_reader<R>(stream: R, label: &'static str, lines: Sender<String>) -> JoinHandle<()>
where
    R: std::io::Read + Send + 'static,
{
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stream).lines() {
            let Ok(line) = line else {
                // The pipe closed or produced invalid UTF-8: stop reading. The
                // parent's own deadlines are what decide the verdict, and it
                // must not block here waiting for a stream that has ended.
                break;
            };
            if lines.send(format!("{label}: {line}")).is_err() {
                break;
            }
        }
    })
}

/// Everything the child printed, for a failure report.
///
/// The readers are joined FIRST, and only a closed child makes them finish — so
/// every caller reaps or kills the child before asking for a transcript.
/// Otherwise this drains whatever happened to have arrived, and the line that
/// explains the failure is the one most likely to still be in flight.
fn drain_transcript(lines: &Receiver<String>, readers: [JoinHandle<()>; 2]) -> String {
    for reader in readers {
        drop(reader.join());
    }
    let mut transcript = String::new();
    while let Ok(line) = lines.try_recv() {
        transcript.push_str(&line);
        transcript.push('\n');
    }
    transcript
}

/// Last resort on a failing path: the wedged child must not outlive this test
/// and hold its (temporary, in-memory) state open.
fn kill_and_reap(child: &mut std::process::Child) {
    drop(child.kill());
    drop(child.wait());
}

/// The CHILD role: arm the wedge, drain on SIGTERM, and drop the runtime.
///
/// In the parent's own run this test is a no-op — the role is selected by
/// [`CHILD_ENV`], which only the parent sets. That is a role switch, not a
/// skipped test: the assertion about this code lives in
/// [`server_process_exits_after_sigterm_with_a_dispatch_parked_on_an_unserved_queue`],
/// which fails if this body ever stops reaching its end.
#[test]
fn parked_dispatch_child() -> Result<(), TestError> {
    if std::env::var_os(CHILD_ENV).is_none() {
        println!(
            "{CHILD_TEST}: parent role — the wedge is armed by the child this binary re-execs"
        );
        return Ok(());
    }
    child_main()
}

fn child_main() -> Result<(), TestError> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let state = runtime.block_on(ServerState::build_with_store(
        InMemoryStore::default(),
        runtime_config(),
    ))?;

    // The bridge dispatcher over the STATE's own seams, wired exactly as
    // `state::build_bridge_dispatcher` wires it at boot: the same drain gate the
    // shutdown coordinator flips, the same queue-service state, the same
    // (default: strict, no clocks) queue-service config, and this runtime's
    // handle — which is what makes the park a `block_on` on the blocking pool
    // rather than a plain sleep.
    let dispatcher = Arc::new(
        WorkerActivityDispatcher::new(
            state.worker_registry().clone(),
            NAMESPACE,
            state.heartbeat_tracker().clone(),
        )
        .with_pending(state.pending_activities().clone())
        .with_drain_state(state.drain_state().clone())
        .with_tokio_handle(runtime.handle().clone())
        .with_queue_service(state.runtime_config().worker.queue_service.clone())
        .with_queue_declarations(state.queue_declarations().clone())
        .with_queue_state(state.queue_service_state().clone()),
    );
    let dispatch = runtime
        .handle()
        .spawn_blocking(move || dispatcher.dispatch(unserved_dispatch()));

    // Install the SIGTERM listener BEFORE arming, and fail loudly if the
    // registration does not take. If the process still carried the default
    // disposition when the parent signalled, the kernel would kill it outright
    // and this pin would pass on a defect it never exercised.
    let mut terminate = runtime.block_on(async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
    })?;

    runtime.block_on(await_parked(&state))?;
    println!("{ARMED_MARKER}");

    runtime
        .block_on(async { terminate.recv().await })
        .ok_or("the SIGTERM stream closed without delivering a signal")?;
    let outcome = runtime.block_on(shutdown::drain_after_first_signal(
        state.clone(),
        std::future::pending::<()>(),
    ))?;
    // The drain's own verdict, recorded here because it is the misleading half
    // of the incident: it is honestly `Clean` — the parked dispatch never
    // reached a worker, so the heartbeat tracker it counts has nothing in
    // flight — and the process still has to exit afterwards.
    println!("child: drain outcome {outcome:?}");
    if !matches!(outcome, ShutdownOutcome::Clean) {
        return Err(format!("expected a clean drain, got {outcome:?}").into());
    }

    // THE WEDGE. `Runtime::drop` calls `BlockingPool::shutdown`, which waits for
    // every started blocking task — including the parked dispatch. This is the
    // line the sampled process died on, with `main` already returned.
    drop(runtime);
    // The dispatch must have been released, not abandoned: the pool shutdown
    // above joins every started blocking task, so an unfinished one here would
    // mean the wait ended some other way than by ending.
    if dispatch.is_finished() {
        Ok(())
    } else {
        Err("the parked dispatch survived the blocking pool's shutdown".into())
    }
}

/// Wait until the dispatch is observably parked on the unserved queue.
///
/// Read from the queue-service state — the same live state the ops console and
/// the `unserved_queues` API read — so "armed" means the dispatch really is
/// inside the selection wait, not merely spawned.
async fn await_parked(state: &ServerState) -> Result<(), TestError> {
    let started = Instant::now();
    loop {
        if state
            .queue_service_state()
            .parked_on_queue(UNSERVED_QUEUE)?
            > 0
        {
            return Ok(());
        }
        if started.elapsed() >= ARM_DEADLINE {
            return Err(format!(
                "no dispatch parked on `{UNSERVED_QUEUE}` within {ARM_DEADLINE:?}"
            )
            .into());
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// One dispatch to the unserved queue, carrying no retry policy — so a
/// synthesized failure would be delivered to the workflow as terminal, which is
/// why the drain refusal must wear the parked class instead.
fn unserved_dispatch() -> ActivityDispatch {
    ActivityDispatch {
        namespace: NAMESPACE.to_owned(),
        task_queue: UNSERVED_QUEUE.to_owned(),
        node: None,
        workflow_id: WorkflowId::new_v4(),
        run_id: RunId::new_v4(),
        activity_id: ActivityId::from_sequence_position(0),
        name: ACTIVITY_TYPE.to_owned(),
        input: "{}".to_owned(),
        config: "{}".to_owned(),
        attempt: 1,
        advisory: false,
        labels: std::collections::BTreeMap::new(),
    }
}

/// The child's server configuration: an in-memory store, no transports bound,
/// and — the load-bearing part — `WorkerConfig::default`'s queue service, which
/// is `strict` with NEITHER clock set. That is the shipped default and the
/// precondition for the unbounded park.
fn runtime_config() -> RuntimeConfig {
    RuntimeConfig {
        listen: ListenConfig {
            grpc: std::net::SocketAddr::from(([127, 0, 0, 1], 0)),
            http: std::net::SocketAddr::from(([127, 0, 0, 1], 0)),
        },
        tls: None,
        auth: AuthConfig {
            enabled: false,
            jwks_url: None,
            jwks_refresh_seconds: 300,
        },
        ops_console: OpsConsoleConfig {
            source: OpsConsoleAssetSource::Embedded,
        },
        namespace: NamespaceConfig {
            mode: NamespaceMode::SharedEngine,
        },
        worker: WorkerConfig {
            heartbeat_window: Duration::from_secs(30),
            ..WorkerConfig::default()
        },
        websocket: WebSocketConfig {
            outbound_buffer_bound: 32,
            event_broadcast_capacity: Some(64),
            cluster_broadcast_capacity: Some(64),
        },
        workflow_packages: Vec::new(),
        deploy: DeployConfig::default(),
        authoring: AuthoringConfig::default(),
        dev: DevConfig::default(),
        outbox: OutboxConfig::default(),
        observability: ObservabilityConfig::default(),
        mcp: ResolvedMcpConfig::default(),
        scheduler_threads: 1,
        query_timeout: Some(Duration::from_secs(10)),
        default_namespace: NAMESPACE.to_owned(),
        auto_create: AutoCreate::Open,
        max_in_flight_activities: DEFAULT_MAX_IN_FLIGHT_ACTIVITIES,
        drain_timeout: Duration::from_secs(30),
        metrics: MetricsConfig { enabled: false },
        owned_shards: Vec::new(),
        cors_allowed_origins: Vec::new(),
    }
}
