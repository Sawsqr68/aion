//! Delivery-contract pins: the outbox liminal path must span a real activity.
//!
//! Red-first pins for the delivery-contract fix (Option 1, ruling `755737bb`;
//! analysis in `docs/design/DELIVERY-CONTRACT-SCOPING.md`). Each pin drives the
//! REAL `OutboxDispatcher` retry loop over a REAL liminal server and a REAL
//! remote worker — the same harness shape as `lsub1_xnode_dispatch_e2e` — so
//! the pins exercise the delivery contract itself, not a mock of it.
//!
//! The defect being pinned: outbox delivery blocks on the worker's
//! reply-when-done for a bounded `PUSH_REPLY_TIMEOUT` (30s), so an activity
//! longer than the bound times out, re-pushes (the worker re-executes — an
//! unfenced duplicate side effect), exhausts the retry budget against a live
//! worker, dead-letters, and leaves the workflow `Running` in silence.
//!
//! On the defective tree these pins are RED. The fix (push once, wait
//! unbounded gated on row-claimed + worker-registered) turns them green:
//!
//! - [`ninety_second_activity_delivers_exactly_once`] — the contract pin: a
//!   90-second activity is delivered once, executed once, completed once.
//! - [`re_push_while_executing_never_re_executes`] — the mechanism pin: a
//!   re-push while the worker is mid-execution is the duplicate-execution bug.
//! - [`dead_letter_is_genuine_and_loud`] — the honesty pin: dead-letter still
//!   fires on genuine delivery failure (no worker at all), and the workflow is
//!   TOLD (the failure reaches the delivery callback, never silence).
//! - [`wedge_shape_regression_0756ecd5`] — the exhibit pin: one fast leg and
//!   two long legs against one serial worker; every leg completes exactly
//!   once, none dead-letters. Mirrors wedged run
//!   `0756ecd5-9569-4dc9-a3a-...` (fast leg delivered, long legs burned five
//!   attempts each and dead-lettered while the worker ground on doomed
//!   duplicates). Attempt budget and leg durations are scaled down from the
//!   exhibit (3 attempts, 45s legs) purely for test wall-clock; the shape —
//!   bounded awaiter vs. serial long execution — is byte-for-byte the same.
//!
//! Timings are contract-derived, not tuning: every "slow" duration exceeds
//! the 30-second bounded awaiter the defect lives in, and every deadline
//! exceeds the worst-case red path so a red verdict is an assertion, never a
//! test timeout.
#![cfg(feature = "liminal-transport")]

use std::error::Error;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use aion_core::{ActivityId, RunId, WorkflowId};
use aion_server::ServerError;
use aion_server::worker::{
    ConnectedWorkerRegistry, LiminalConnectionNotifier, OutboxDeliveryCallback, OutboxDispatcher,
    OutboxDispatcherConfig, OutboxRowDispatch, RegistryLiminalDispatch,
};
use aion_store::{OutboxRow, OutboxStatus, OutboxStore};
use aion_store_libsql::LibSqlStore;
use aion_worker::{ActivityRegistry, LiminalActivityWorker, WorkerConfig};
use chrono::Utc;
use liminal_server::config::{ChannelDef, ServerConfig};
use liminal_server::server::connection::ConnectionSupervisor;
use liminal_server::server::listener::ServerListener;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use uuid::Uuid;

type TestError = Box<dyn Error + Send + Sync>;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const NAMESPACE: &str = "contract";
const TASK_QUEUE: &str = "legs";
const NODE: &str = "pin-box";
const ACTIVITY_TYPE: &str = "slow-leg";

/// The bounded awaiter the defect lives in (`liminal_transport.rs`
/// `PUSH_REPLY_TIMEOUT`). Pins derive their "slow" durations from it so the
/// contract violation, not a tuned number, is what trips red.
const BOUNDED_AWAITER: Duration = Duration::from_secs(30);

/// One recorded completion: correlation ids plus the worker's result string.
type CompletionRecord = (WorkflowId, ActivityId, Option<RunId>, String);
/// One recorded failure surfaced to the workflow: correlation ids plus reason.
type FailureRecord = (WorkflowId, ActivityId, Option<RunId>, String);

/// The pin activity's input: how long the leg holds the worker, and which
/// staged row it belongs to (for per-row execution accounting).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegInput {
    sleep_ms: u64,
    tag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegOutput {
    tag: String,
    slept_ms: u64,
}

fn test_error(message: impl std::fmt::Display) -> TestError {
    message.to_string().into()
}

/// Recording delivery callback: completions AND failures, so the pins can
/// assert both halves of the honesty contract (results arrive exactly once;
/// dead-letter is never silent to the workflow).
#[derive(Debug, Default)]
struct RecordingCallback {
    completions: Mutex<Vec<CompletionRecord>>,
    failures: Mutex<Vec<FailureRecord>>,
}

impl RecordingCallback {
    fn completion_count(&self) -> usize {
        self.completions.lock().map_or(usize::MAX, |c| c.len())
    }

    fn failure_count(&self) -> usize {
        self.failures.lock().map_or(usize::MAX, |f| f.len())
    }
}

impl OutboxDeliveryCallback for RecordingCallback {
    fn deliver_completion(
        &self,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
        run_id: Option<&RunId>,
        result: String,
    ) -> Result<bool, ServerError> {
        if let Ok(mut completions) = self.completions.lock() {
            completions.push((
                workflow_id.clone(),
                activity_id.clone(),
                run_id.cloned(),
                result,
            ));
        }
        Ok(true)
    }

    fn deliver_failure(
        &self,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
        run_id: Option<&RunId>,
        reason: String,
    ) -> Result<bool, ServerError> {
        if let Ok(mut failures) = self.failures.lock() {
            failures.push((
                workflow_id.clone(),
                activity_id.clone(),
                run_id.cloned(),
                reason,
            ));
        }
        Ok(true)
    }
}

/// Per-tag execution ledger: which staged row's leg the worker started, in
/// order. Duplicate tags are the duplicate-execution defect made visible.
#[derive(Debug, Default)]
struct ExecutionLedger {
    started: Mutex<Vec<String>>,
    total: AtomicUsize,
}

impl ExecutionLedger {
    fn record_start(&self, tag: &str) {
        self.total.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut started) = self.started.lock() {
            started.push(tag.to_owned());
        }
    }

    fn total(&self) -> usize {
        self.total.load(Ordering::SeqCst)
    }

    fn executions_of(&self, tag: &str) -> usize {
        self.started.lock().map_or(usize::MAX, |started| {
            started
                .iter()
                .filter(|started_tag| started_tag.as_str() == tag)
                .count()
        })
    }
}

/// Running liminal server with the aion in-band registration notifier, the
/// same construction as `lsub1_xnode_dispatch_e2e`.
struct RunningServer {
    listener: Option<ServerListener>,
    registry: ConnectedWorkerRegistry,
    address: SocketAddr,
}

impl RunningServer {
    fn start() -> Result<Self, TestError> {
        let config = ServerConfig {
            listen_address: "127.0.0.1:0".parse().map_err(test_error)?,
            health_listen_address: reserve_loopback_port()?,
            channels: Vec::<ChannelDef>::new(),
            routing_rules: Vec::new(),
            persistence_path: None,
            cluster: None,
            auth: None,
            drain_timeout_ms: 30_000,
            services: liminal_server::config::ServicesConfig::default(),
            limits: liminal_server::config::LimitsConfig::default(),
            websocket: None,
            participant: None,
        };
        let registry = ConnectedWorkerRegistry::default();
        let notifier = Arc::new(LiminalConnectionNotifier::new(registry.clone()));
        let supervisor = build_supervisor_with_notifier(&config, notifier.clone())?;
        if !notifier.bind_supervisor(supervisor.clone()) {
            return Err(test_error("notifier supervisor was already bound"));
        }
        let listener = ServerListener::bind(&config, supervisor).map_err(test_error)?;
        let address = listener.local_addr();
        Ok(Self {
            listener: Some(listener),
            registry,
            address,
        })
    }

    fn wait_for_registered_worker(&self) -> Result<(), TestError> {
        let deadline = Instant::now() + CONNECT_TIMEOUT;
        while Instant::now() < deadline {
            let selected = self
                .registry
                .select_worker(NAMESPACE, TASK_QUEUE, ACTIVITY_TYPE, Some(NODE))
                .map_err(test_error)?;
            if selected.is_some() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Err(test_error("server never registered the in-band worker"))
    }

    fn shutdown(mut self) -> Result<(), TestError> {
        if let Some(listener) = self.listener.take() {
            listener.shutdown().map_err(test_error)?;
        }
        Ok(())
    }
}

fn build_supervisor_with_notifier(
    config: &ServerConfig,
    notifier: Arc<LiminalConnectionNotifier>,
) -> Result<ConnectionSupervisor, TestError> {
    use liminal_server::server::connection::LiminalConnectionServices;
    let services = Arc::new(LiminalConnectionServices::from_config(config).map_err(test_error)?);
    ConnectionSupervisor::with_services_and_notifier(services, notifier).map_err(test_error)
}

fn reserve_loopback_port() -> Result<SocketAddr, TestError> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(test_error)?;
    let address = listener.local_addr().map_err(test_error)?;
    drop(listener);
    Ok(address)
}

/// The pin activity: record the start in the ledger, hold the worker for the
/// leg's duration, return. The hold is a plain async sleep on the worker's
/// serve loop — exactly how a real long activity occupies the serial inline
/// executor.
fn worker_registry(ledger: Arc<ExecutionLedger>) -> Result<Arc<ActivityRegistry>, TestError> {
    let registry = ActivityRegistry::new()
        .register_activity(ACTIVITY_TYPE, move |input: LegInput, _context| {
            let ledger = Arc::clone(&ledger);
            Box::pin(async move {
                ledger.record_start(&input.tag);
                tokio::time::sleep(Duration::from_millis(input.sleep_ms)).await;
                Ok(LegOutput {
                    tag: input.tag,
                    slept_ms: input.sleep_ms,
                })
            })
        })
        .map_err(test_error)?;
    Ok(Arc::new(registry))
}

fn worker_config() -> Result<WorkerConfig, TestError> {
    WorkerConfig::builder()
        .endpoint("unused-direct-address")
        .namespace(NAMESPACE)
        .task_queue(TASK_QUEUE)
        .node(NODE)
        .identity("delivery-contract-pin-worker")
        .max_concurrency(1)
        .reconnect_initial_backoff(Duration::from_millis(5))
        .reconnect_max_backoff(Duration::from_millis(20))
        .reconnect_max_attempts(3)
        .build()
        .map_err(test_error)
}

/// The remote worker on its own OS thread. `stop` is DETACHED — it flips the
/// serve-loop flag but does not join: on the defective tree the worker can be
/// mid-way through a multi-minute doomed duplicate when the pin's verdict is
/// already in, and joining would stall the verdict behind the defect being
/// pinned. The thread dies with the test process (nextest: one process per
/// test binary).
struct WorkerThread {
    stop: Arc<AtomicBool>,
}

impl WorkerThread {
    fn spawn(address: String, config: WorkerConfig, registry: Arc<ActivityRegistry>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    eprintln!("worker runtime build failed: {error}");
                    return;
                }
            };
            runtime.block_on(async move {
                let worker = match LiminalActivityWorker::connect(&address, &config, registry) {
                    Ok(worker) => worker,
                    Err(error) => {
                        eprintln!("worker connect failed: {error}");
                        return;
                    }
                };
                if let Err(error) = worker
                    .serve_until(|| thread_stop.load(Ordering::SeqCst))
                    .await
                {
                    eprintln!("worker serve loop ended with error: {error}");
                }
            });
        });
        Self { stop }
    }

    fn stop(self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Dispatcher settings for the pins. Poll and backoff are fast so the
/// defective tree burns its retry budget promptly (red verdicts arrive in
/// minutes, not tens of minutes); `max_attempts` is per-pin because the
/// wedge exhibit's budget (5) and the scaled pins (3) differ deliberately.
fn dispatcher_config(max_attempts: u32) -> OutboxDispatcherConfig {
    OutboxDispatcherConfig {
        poll_interval: Duration::from_millis(250),
        batch_size: 16,
        max_attempts,
        backoff_base: Duration::from_millis(50),
        backoff_multiplier: 2,
        backoff_max: Duration::from_secs(1),
    }
}

/// Stage one pending outbox row for the pin pool carrying a leg of
/// `sleep_ms`, tagged for the execution ledger.
fn pending_row(sleep_ms: u64, tag: &str, ordinal: u64) -> Result<OutboxRow, TestError> {
    let workflow_id = WorkflowId::new(Uuid::new_v4());
    let dispatch_key = format!("{workflow_id}:{ordinal}");
    let input =
        aion_core::Payload::from_json(&serde_json::json!({ "sleep_ms": sleep_ms, "tag": tag }))
            .map_err(test_error)?;
    Ok(OutboxRow {
        dispatch_key,
        workflow_id,
        ordinal,
        run_id: Some(RunId::new(Uuid::new_v4())),
        namespace: NAMESPACE.to_owned(),
        task_queue: TASK_QUEUE.to_owned(),
        node: Some(NODE.to_owned()),
        activity_type: ACTIVITY_TYPE.to_owned(),
        input,
        status: OutboxStatus::Pending,
        attempt: 0,
        visible_after: Utc::now(),
        claimed_at: None,
        failure_delivered: false,
    })
}

async fn open_store(name: &str) -> Result<Arc<LibSqlStore>, TestError> {
    let nanos = Instant::now().elapsed().as_nanos();
    let path = std::env::temp_dir().join(format!(
        "aion-delivery-contract-{name}-{}-{nanos}.db",
        std::process::id()
    ));
    LibSqlStore::open(path)
        .await
        .map(Arc::new)
        .map_err(test_error)
}

/// Everything a pin stands up: liminal server, remote worker, durable store,
/// and the REAL dispatcher loop running on its own task.
struct PinRig {
    server: Option<RunningServer>,
    worker: Option<WorkerThread>,
    store: Arc<LibSqlStore>,
    callback: Arc<RecordingCallback>,
    ledger: Arc<ExecutionLedger>,
    shutdown: watch::Sender<bool>,
    dispatcher: Option<tokio::task::JoinHandle<()>>,
}

impl PinRig {
    /// Stand the rig up. With `with_worker = false` the pool has no worker at
    /// all (the genuine-delivery-failure arm of the honesty pin).
    async fn start(name: &str, max_attempts: u32, with_worker: bool) -> Result<Self, TestError> {
        let server = RunningServer::start()?;
        let ledger = Arc::new(ExecutionLedger::default());
        let worker = if with_worker {
            let registry = worker_registry(Arc::clone(&ledger))?;
            let worker =
                WorkerThread::spawn(server.address.to_string(), worker_config()?, registry);
            server.wait_for_registered_worker()?;
            Some(worker)
        } else {
            None
        };

        let store = open_store(name).await?;
        let callback = Arc::new(RecordingCallback::default());
        let dispatcher_builder = OutboxDispatcher::new(
            Arc::clone(&store) as Arc<dyn OutboxStore>,
            dispatcher_config(max_attempts),
        );
        let dispatch = RegistryLiminalDispatch::new(
            server.registry.clone(),
            Arc::clone(&callback) as Arc<dyn OutboxDeliveryCallback>,
            dispatcher_builder.delivery_gate(),
        );
        let dispatcher = dispatcher_builder
            .with_dispatch(Arc::new(dispatch) as Arc<dyn OutboxRowDispatch>)
            .with_delivery_callback(Arc::clone(&callback) as Arc<dyn OutboxDeliveryCallback>);
        let (shutdown, shutdown_rx) = watch::channel(false);
        let handle = tokio::spawn(dispatcher.run(shutdown_rx));
        Ok(Self {
            server: Some(server),
            worker,
            store,
            callback,
            ledger,
            shutdown,
            dispatcher: Some(handle),
        })
    }

    async fn stage(&self, rows: &[OutboxRow]) -> Result<(), TestError> {
        self.store
            .append_outbox_batch(rows)
            .await
            .map_err(test_error)
    }

    async fn row_status(&self, dispatch_key: &str) -> Result<Option<OutboxStatus>, TestError> {
        Ok(self
            .store
            .outbox_row_state(dispatch_key)
            .await
            .map_err(test_error)?
            .map(|state| state.status))
    }

    async fn teardown(mut self) -> Result<(), TestError> {
        let _ = self.shutdown.send(true);
        if let Some(handle) = self.dispatcher.take() {
            // The dispatcher observes shutdown at the next select arm; on the
            // defective tree an in-flight bounded await can hold the sweep for
            // up to one BOUNDED_AWAITER, so give the join real room rather
            // than leaking the task silently.
            match tokio::time::timeout(BOUNDED_AWAITER + Duration::from_secs(5), handle).await {
                Ok(joined) => joined.map_err(test_error)?,
                Err(_) => eprintln!(
                    "dispatcher did not stop within the teardown window; \
                     abandoning its task (defective-tree hold)"
                ),
            }
        }
        if let Some(worker) = self.worker.take() {
            worker.stop();
        }
        if let Some(server) = self.server.take() {
            server.shutdown()?;
        }
        Ok(())
    }
}

/// One verdict poll: the terminal conditions a pin waits on.
enum Verdict {
    /// Every staged row is Done, each tag executed exactly once, and exactly
    /// one completion per row re-entered aion.
    Green,
    /// Keep polling.
    Pending,
}

/// Polls until every staged row is `Done` with exactly-once execution and
/// delivery, failing FAST — with a named defect, not a timeout — the moment a
/// duplicate execution or a dead-letter of a live leg is observed.
async fn wait_for_exactly_once(
    rig: &PinRig,
    rows: &[OutboxRow],
    tags: &[&str],
    deadline: Duration,
) -> Result<(), TestError> {
    let started = Instant::now();
    while started.elapsed() < deadline {
        // Duplicate execution is the re-push defect — fail immediately.
        for tag in tags {
            let executions = rig.ledger.executions_of(tag);
            if executions > 1 {
                return Err(test_error(format!(
                    "DUPLICATE EXECUTION: leg '{tag}' started {executions} times — \
                     the outbox re-pushed a dispatch the worker was already executing \
                     (unfenced at-least-once execution)"
                )));
            }
        }
        match poll_rows(rig, rows, tags).await? {
            Verdict::Green => return Ok(()),
            Verdict::Pending => {}
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Err(test_error(format!(
        "exactly-once delivery did not complete within {deadline:?}: \
         executions={} completions={} failures={}",
        rig.ledger.total(),
        rig.callback.completion_count(),
        rig.callback.failure_count(),
    )))
}

async fn poll_rows(rig: &PinRig, rows: &[OutboxRow], tags: &[&str]) -> Result<Verdict, TestError> {
    let mut all_done = true;
    for row in rows {
        match rig.row_status(&row.dispatch_key).await? {
            Some(OutboxStatus::Failed) => {
                return Err(test_error(format!(
                    "DEAD-LETTERED A LIVE LEG: row {} reached Failed while its worker \
                     was alive and executing — the bounded awaiter exhausted the retry \
                     budget against slow-but-healthy work",
                    row.dispatch_key
                )));
            }
            Some(OutboxStatus::Done) => {}
            _ => all_done = false,
        }
    }
    if !all_done {
        return Ok(Verdict::Pending);
    }
    for tag in tags {
        if rig.ledger.executions_of(tag) != 1 {
            return Ok(Verdict::Pending);
        }
    }
    if rig.callback.completion_count() != rows.len() {
        return Ok(Verdict::Pending);
    }
    Ok(Verdict::Green)
}

/// PIN (a) — the contract: a 90-second activity through the outbox liminal
/// path is delivered exactly once, executed exactly once, and completed.
///
/// RED today: the 30s bounded awaiter times the delivery out mid-execution,
/// re-pushes (duplicate execution at ~t90s), and dead-letters at ~t150s while
/// the worker is alive and working the whole time.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ninety_second_activity_delivers_exactly_once() -> Result<(), TestError> {
    let rig = PinRig::start("ninety-second", 5, true).await?;
    let row = pending_row(90_000, "ninety", 0)?;
    rig.stage(std::slice::from_ref(&row)).await?;

    // Deadline: the leg itself (90s) + generous delivery/poll margin. The red
    // paths trip far earlier (duplicate at ~90s, dead-letter at ~150s).
    let verdict = wait_for_exactly_once(
        &rig,
        std::slice::from_ref(&row),
        &["ninety"],
        Duration::from_secs(240),
    )
    .await;
    rig.teardown().await?;
    verdict
}

/// PIN (b) — the mechanism: a re-push while the worker is executing the first
/// push re-executes the same activity. The leg only just exceeds the bounded
/// awaiter (40s vs 30s), the minimal case that produces a re-push against an
/// executing worker.
///
/// RED today: the duplicate starts at ~t40s (the moment the serial worker
/// finishes leg one and dequeues the re-push).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn re_push_while_executing_never_re_executes() -> Result<(), TestError> {
    let rig = PinRig::start("re-push", 3, true).await?;
    let row = pending_row(40_000, "minimal-overrun", 0)?;
    rig.stage(std::slice::from_ref(&row)).await?;

    let verdict = wait_for_exactly_once(
        &rig,
        std::slice::from_ref(&row),
        &["minimal-overrun"],
        Duration::from_secs(180),
    )
    .await;
    rig.teardown().await?;
    verdict
}

/// PIN (c) — honesty: dead-letter fires on GENUINE delivery failure (here: no
/// worker exists for the pool at all), and it is LOUD to the workflow — the
/// failure reaches the delivery callback rather than leaving the run
/// `Running` in silence forever.
///
/// Half guard, half red: the dead-letter itself already fires today (guard —
/// the fix must not make it unreachable); the workflow being TOLD is RED
/// today (`handle_dispatch_error` dead-letters into the store and logs, but
/// nothing reaches the callback — the liveness lie). The fix's
/// infrastructure-failed rider closes exactly that gap at this seam.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dead_letter_is_genuine_and_loud() -> Result<(), TestError> {
    let rig = PinRig::start("genuine-failure", 3, false).await?;
    let row = pending_row(1_000, "undeliverable", 0)?;
    rig.stage(std::slice::from_ref(&row)).await?;

    // Genuine failure: no worker for the pool. Budget (3 attempts, ms-scale
    // backoff) exhausts within a few sweeps.
    let started = Instant::now();
    let deadline = Duration::from_secs(30);
    loop {
        if rig.row_status(&row.dispatch_key).await? == Some(OutboxStatus::Failed) {
            break;
        }
        if started.elapsed() >= deadline {
            rig.teardown().await?;
            return Err(test_error(
                "GUARD BROKEN: a genuinely undeliverable row (no worker for the pool) \
                 never dead-lettered — dead-letter must remain reachable for real \
                 delivery failures",
            ));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Never executed, never completed: there was no worker.
    if rig.ledger.total() != 0 {
        rig.teardown().await?;
        return Err(test_error(
            "no worker existed, yet an execution was recorded",
        ));
    }
    if rig.callback.completion_count() != 0 {
        rig.teardown().await?;
        return Err(test_error(
            "no worker existed, yet a completion was recorded",
        ));
    }

    // LOUD: the workflow is told. RED today — dead-letter reaches the store
    // and the log, but no failure ever reaches the delivery callback.
    let failures = rig.callback.failure_count();
    rig.teardown().await?;
    if failures == 0 {
        return Err(test_error(
            "SILENT DEAD-LETTER: the row dead-lettered but nothing reached the \
             delivery callback — the workflow reports Running forever (the \
             liveness lie); dead-letter must surface an explicit \
             infrastructure failure to the workflow",
        ));
    }
    Ok(())
}

/// PIN (d) — the exhibit: the wedge shape of run `0756ecd5`. One fast leg and
/// two long legs staged together against ONE serial worker. Every leg must
/// complete exactly once; none may dead-letter; the fast leg's success must
/// not be survivorship over the long legs' corpses.
///
/// RED today: the fast leg completes inside the bounded awaiter, the long
/// legs burn their whole attempt budgets against the serial worker (each
/// sweep holds the sibling rows behind a 30s bounded await — the same
/// sequential stall the exhibit's timeline shows between 12:35:38 and
/// 12:39:09) and dead-letter while the worker grinds doomed duplicates.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn wedge_shape_regression_0756ecd5() -> Result<(), TestError> {
    let rig = PinRig::start("wedge-shape", 3, true).await?;
    let rows = vec![
        pending_row(5_000, "fast-check", 0)?,
        pending_row(45_000, "long-clippy", 0)?,
        pending_row(45_000, "long-nextest", 0)?,
    ];
    rig.stage(&rows).await?;

    // Deadline: serial execution of all three legs (5+45+45 = 95s) plus
    // delivery margin. Red paths trip earlier: first duplicate at ~t50s,
    // first dead-letter within the scaled 3-attempt budget.
    let verdict = wait_for_exactly_once(
        &rig,
        &rows,
        &["fast-check", "long-clippy", "long-nextest"],
        Duration::from_secs(400),
    )
    .await;
    rig.teardown().await?;
    verdict
}
