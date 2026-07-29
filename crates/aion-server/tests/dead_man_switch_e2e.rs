//! Dead-man-switch pins: a liminal worker connection may not die in silence.
//!
//! Both pins stand up a REAL `liminal-server` over loopback TCP with the aion
//! [`LiminalConnectionNotifier`] installed, a REAL remote `aion-worker` serving
//! through the production [`serve_with_redial`] entry point, the REAL connection
//! [`LivenessProbe`], and the REAL [`HeartbeatSweeper`]. Nothing is faked at the
//! transport; only the heartbeat window is shortened so a test can observe in
//! seconds what production observes in tens of them.
//!
//! # Hit A — the idle worker that was killed for being idle
//!
//! Live, 2026-07-29: "idle worker connection lease expired; worker deregistered"
//! 37 seconds after a worker's last activity. The connection lease is advanced
//! only by frames a worker SENDS, and an idle worker sends nothing — the
//! activity progress-heartbeat only fires DURING execution, so the two liveness
//! domains never fed each other and idle death was structural at ANY cadence.
//! Confirmed by probe: a worker beating every 10s into a 30s window was still
//! deregistered at exactly 30s, before it ever received a dispatch. The next
//! dispatch then parked forever on a queue nobody was serving.
//!
//! [`idle_worker_survives_past_the_old_lease_window`] is that scenario: connect,
//! then do NOTHING for well over the window. The worker must still be
//! registered, and a dispatch must still reach it.
//!
//! # Hit C — the link that died with both ends blind
//!
//! Live run `3a894965`: the link died at 08:33:41 and the worker blocked on the
//! dead socket for 28 minutes while the server recorded `ActivityStarted` for
//! pushes that went nowhere. A CLOSED socket surfaces promptly; a merely DEAD
//! one never does — reads time out benignly and writes land in the kernel
//! buffer.
//!
//! [`a_wedged_link_is_detected_and_redialed`] reproduces exactly that: a
//! wedgeable TCP relay stops forwarding bytes IN BOTH DIRECTIONS without closing
//! either socket. The worker must notice the silence, tear the connection down,
//! redial its next candidate, re-register there, and serve work again.
#![cfg(feature = "liminal-transport")]

use std::error::Error;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use aion_core::{RunId, WorkflowId};
use aion_server::worker::{
    ConnectedWorkerRegistry, DispatchRequest, HeartbeatSweeper, HeartbeatTracker,
    LiminalConnectionNotifier, LivenessProbe, WorkerDelivery, WorkerHandle,
};
use aion_worker::{ActivityRegistry, RedialTiming, WorkerConfig, serve_with_redial};
use liminal_server::config::{ChannelDef, ServerConfig};
use liminal_server::server::connection::ConnectionSupervisor;
use liminal_server::server::listener::ServerListener;
use serde::{Deserialize, Serialize};

type TestError = Box<dyn Error + Send + Sync>;
type TestResult = Result<(), TestError>;

/// The operator's `worker.heartbeat_window` for these pins. Four seconds is the
/// smallest window that still yields the production ratio: `sweep_interval`
/// derives a one-second cadence from it (a quarter-window, its `[1s, window]`
/// clamp inactive), so a healthy connection is pinged FOUR times per window,
/// exactly as a 30s production window is pinged every 7.5s.
const HEARTBEAT_WINDOW: Duration = Duration::from_secs(4);

/// How long a pin waits for a state change before failing. Generously more than
/// two heartbeat windows, so a slow machine never turns a correct
/// implementation red.
const OBSERVE: Duration = Duration::from_secs(20);

const NAMESPACE: &str = "remote";
const TASK_QUEUE: &str = "gates";
const ACTIVITY_TYPE: &str = "run-check";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CheckInput {
    label: String,
    /// Milliseconds the handler blocks for, so a pin can drive an activity that
    /// deliberately outruns the connection's silence window.
    #[serde(default)]
    hold_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CheckOutput {
    ran: String,
}

fn test_error(message: impl std::fmt::Display) -> TestError {
    message.to_string().into()
}

// ---------------------------------------------------------------- Hit A ----

/// HIT A. A worker that connects and then does NOTHING must still be registered
/// long after the connection lease window has passed, and a dispatch must still
/// reach it.
///
/// Red on main: the lease is advanced only by frames the worker sends, and an
/// idle worker sends none, so the sweep deregisters it one window after
/// registration — "idle worker connection lease expired" — and the assertion
/// below fails with the worker gone.
#[test]
fn idle_worker_survives_past_the_old_lease_window() -> TestResult {
    let harness = Harness::start()?;
    let worker = WorkerThread::spawn(vec![harness.address.to_string()])?;
    harness.wait_for_worker()?;

    // Do NOTHING for well over two lease windows. This is the entire scenario:
    // a healthy, connected, idle worker.
    std::thread::sleep(HEARTBEAT_WINDOW * 2 + Duration::from_secs(1));

    let handle = harness.registered_worker().ok_or_else(|| {
        test_error(
            "the idle worker was deregistered: its connection lease expired while it was alive \
             and connected — nothing kept the lease alive on an idle connection",
        )
    })?;

    // And it is not merely a stale registry row: the connection still carries a
    // real dispatch.
    let response = push_dispatch(&handle, "idle-then-dispatched")?;
    assert_eq!(
        response, r#"{"ran":"idle-then-dispatched"}"#,
        "the surviving registration must still be a live, dispatchable connection"
    );
    assert_eq!(
        worker.executions(),
        1,
        "the worker genuinely executed the dispatch"
    );

    worker.stop();
    harness.shutdown()
}

// ---------------------------------------------------------------- Hit C ----

/// HIT C. A link that goes DEAD without closing — the relay stops forwarding in
/// both directions and closes nothing — must be detected by the worker, torn
/// down, and redialed onto its next candidate, where it re-registers and serves
/// work again.
///
/// Red on main: no frame ever arrives and none is expected, so the worker's
/// receive loop times out benignly for ever. It never redials, never
/// re-registers, and no new worker appears in the registry — exactly the
/// 28-minute blindness observed live.
#[test]
fn a_wedged_link_is_detected_and_redialed() -> TestResult {
    let harness = Harness::start()?;
    let relay = WedgeableRelay::start(harness.address)?;
    // The worker dials the relay FIRST and the server directly second, so a
    // detected death has somewhere honest to migrate to — the production
    // candidate-ring shape.
    let worker = WorkerThread::spawn(vec![relay.address.to_string(), harness.address.to_string()])?;
    harness.wait_for_worker()?;
    let through_relay = harness
        .registered_worker()
        .ok_or_else(|| test_error("the worker never registered through the relay"))?;

    // Let the connection settle into its steady state first: the worker's
    // dead-man switch is UNARMED until the server's first ping tells it the
    // window to expect, which is deliberate — a worker holds no window of its
    // own. One full heartbeat window is several ping cadences, so the switch is
    // certainly armed by now.
    std::thread::sleep(HEARTBEAT_WINDOW);

    // Kill the LINK, not the worker: both directions stop carrying bytes and
    // neither socket is closed. Every read now times out benignly and every
    // write still succeeds into the kernel buffer.
    relay.wedge();

    // The proof: a SECOND, distinct registration appears — the worker declared
    // the link dead, redialed the next candidate, and re-registered there.
    let deadline = Instant::now() + OBSERVE;
    let redialed = loop {
        if let Some(handle) = harness
            .registered_workers()
            .into_iter()
            .find(|handle| handle.id() != through_relay.id())
        {
            break handle;
        }
        if Instant::now() >= deadline {
            return Err(test_error(
                "the worker never noticed its link was dead: no redial, no re-registration. \
                 A wedged connection is indistinguishable from an idle one unless something \
                 is expected to arrive on it",
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    };

    // And the migrated connection genuinely serves: the in-flight work survives.
    let response = push_dispatch(&redialed, "after-the-link-died")?;
    assert_eq!(response, r#"{"ran":"after-the-link-died"}"#);
    assert_eq!(worker.executions(), 1);

    worker.stop();
    relay.shutdown();
    harness.shutdown()
}

// ------------------------------------------------ busy is not silent ----

/// A plain activity executes INLINE on the worker's serve loop, so a handler
/// that outruns the silence window returns to a clock that started when its
/// dispatch arrived. The dead-man switch must NOT read that as a dead link: the
/// worker was busy, not silent, and tearing down a healthy connection after
/// every long activity would be a far worse failure than the one being fixed.
///
/// This pin is a self-inflicted-regression guard, not one of the four hits: it
/// fails against the switch's first cut (which re-armed only on frame ARRIVAL)
/// and passes once the clock also re-arms on completion.
#[test]
fn a_long_activity_is_not_mistaken_for_a_dead_link() -> TestResult {
    let harness = Harness::start()?;
    let worker = WorkerThread::spawn(vec![harness.address.to_string()])?;
    harness.wait_for_worker()?;
    let handle = harness
        .registered_worker()
        .ok_or_else(|| test_error("the worker never registered"))?;
    let worker_id = handle.id();

    // Let the connection settle into its steady state first, exactly as the
    // wedged-link pin does: the worker's switch and its connection-lease pump
    // are both armed by the server's first ping, which arrives within a quarter
    // window of registration. (A dispatch that lands inside that first quarter
    // window is a documented residual — see the evidence doc.)
    std::thread::sleep(HEARTBEAT_WINDOW);

    // One activity that holds the serve loop for well over the silence window.
    let hold = HEARTBEAT_WINDOW * 2;
    let response = push_dispatch_holding(
        &handle,
        "long-but-alive",
        u64::try_from(hold.as_millis()).unwrap_or(u64::MAX),
    )?;
    assert_eq!(response, r#"{"ran":"long-but-alive"}"#);

    // The connection must be the SAME one: no declared death, no teardown, no
    // redial. A second dispatch proves it is still live.
    let still = harness
        .registered_workers()
        .into_iter()
        .find(|candidate| candidate.id() == worker_id)
        .ok_or_else(|| {
            test_error(
                "the worker tore its healthy connection down after a long activity: the \
                 dead-man switch counted execution time as silence",
            )
        })?;
    let response = push_dispatch(&still, "still-here")?;
    assert_eq!(response, r#"{"ran":"still-here"}"#);
    assert_eq!(worker.executions(), 2);

    worker.stop();
    harness.shutdown()
}

// ------------------------------------------------------------- harness ----

/// A running liminal listener with the aion notifier, the liveness probe, and
/// the expiry sweeper — the production trio.
struct Harness {
    listener: Option<ServerListener>,
    registry: ConnectedWorkerRegistry,
    address: SocketAddr,
    runtime: Option<tokio::runtime::Runtime>,
    shutdown: tokio::sync::watch::Sender<bool>,
}

impl Harness {
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
        let tracker = HeartbeatTracker::new(HEARTBEAT_WINDOW);
        let notifier = Arc::new(
            LiminalConnectionNotifier::new(registry.clone())
                .with_heartbeat_tracker(tracker.clone()),
        );
        let supervisor = {
            use liminal_server::server::connection::LiminalConnectionServices;
            let services =
                Arc::new(LiminalConnectionServices::from_config(&config).map_err(test_error)?);
            ConnectionSupervisor::with_services_and_notifier(services, notifier.clone())
                .map_err(test_error)?
        };
        if !notifier.bind_supervisor(supervisor.clone()) {
            return Err(test_error("notifier supervisor was already bound"));
        }
        let listener = ServerListener::bind(&config, supervisor).map_err(test_error)?;
        let address = listener.local_addr();

        // The production pair: the probe keeps a live connection's lease fresh,
        // and the sweeper reaps one whose lease genuinely ran out.
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(test_error)?;
        let (shutdown, shutdown_rx) = tokio::sync::watch::channel(false);
        let probe = LivenessProbe::new(
            notifier,
            tracker.clone(),
            registry.clone(),
            HEARTBEAT_WINDOW,
        );
        let sweeper = HeartbeatSweeper::new(
            tracker,
            registry.clone(),
            aion_server::worker::PendingActivities::default(),
            aion_server::shutdown::DrainState::default(),
            HEARTBEAT_WINDOW,
        );
        runtime.spawn(probe.run(shutdown_rx.clone()));
        runtime.spawn(sweeper.run(shutdown_rx));

        Ok(Self {
            listener: Some(listener),
            registry,
            address,
            runtime: Some(runtime),
            shutdown,
        })
    }

    fn registered_workers(&self) -> Vec<WorkerHandle> {
        self.registry.all_workers().unwrap_or_default()
    }

    fn registered_worker(&self) -> Option<WorkerHandle> {
        self.registered_workers().into_iter().next()
    }

    fn wait_for_worker(&self) -> TestResult {
        let deadline = Instant::now() + OBSERVE;
        while Instant::now() < deadline {
            if self.registered_worker().is_some() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Err(test_error("the worker never registered"))
    }

    fn shutdown(mut self) -> TestResult {
        self.shutdown.send(true).map_err(test_error)?;
        if let Some(listener) = self.listener.take() {
            listener.shutdown().map_err(test_error)?;
        }
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_timeout(Duration::from_secs(5));
        }
        Ok(())
    }
}

/// Pushes one real dispatch to a registered worker over its liminal connection
/// and returns the worker's result string.
fn push_dispatch(handle: &WorkerHandle, label: &str) -> Result<String, TestError> {
    push_dispatch_holding(handle, label, 0)
}

/// [`push_dispatch`], but the worker's handler blocks for `hold_ms` first.
fn push_dispatch_holding(
    handle: &WorkerHandle,
    label: &str,
    hold_ms: u64,
) -> Result<String, TestError> {
    let WorkerDelivery::Liminal(delivery) = handle.delivery() else {
        return Err(test_error("the registered worker is not liminal-delivered"));
    };
    let request = DispatchRequest {
        activity_type: ACTIVITY_TYPE.to_owned(),
        workflow_id: WorkflowId::new_v4(),
        ordinal: 0,
        run_id: Some(RunId::new_v4()),
        completion_token: "dead-man-switch-token".to_owned(),
        idempotency_key: "dead-man-switch-key".to_owned(),
        input: serde_json::to_vec(&CheckInput {
            label: label.to_owned(),
            hold_ms,
        })
        .map_err(test_error)?,
        attempt: 1,
        labels: std::collections::BTreeMap::new(),
        heartbeat_window_ms: 0,
    };
    let response = delivery.dispatch(&request).map_err(test_error)?;
    response
        .outcome
        .map_err(|reason| test_error(format!("the worker failed the dispatch: {reason}")))
}

/// The remote worker, driven through the PRODUCTION redial entry point on its
/// own OS thread (the push receive is blocking).
struct WorkerThread {
    stop: Arc<AtomicBool>,
    executions: Arc<AtomicUsize>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl WorkerThread {
    fn spawn(candidates: Vec<String>) -> Result<Self, TestError> {
        let stop = Arc::new(AtomicBool::new(false));
        let executions = Arc::new(AtomicUsize::new(0));
        let config = WorkerConfig::builder()
            .endpoint("unused-direct-address")
            .namespace(NAMESPACE)
            .task_queue(TASK_QUEUE)
            .identity("dead-man-switch-worker")
            .max_concurrency(1)
            .reconnect_initial_backoff(Duration::from_millis(20))
            .reconnect_max_backoff(Duration::from_millis(100))
            .reconnect_max_attempts(3)
            .build()
            .map_err(test_error)?;
        let counter = Arc::clone(&executions);
        let registry = Arc::new(
            ActivityRegistry::new()
                .register_activity(ACTIVITY_TYPE, move |input: CheckInput, _context| {
                    let counter = Arc::clone(&counter);
                    Box::pin(async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                        // A plain activity runs INLINE on the serve loop, so
                        // this hold is exactly the "worker is busy, not silent"
                        // condition the dead-man switch must not mistake for a
                        // dead link.
                        if input.hold_ms > 0 {
                            tokio::time::sleep(Duration::from_millis(input.hold_ms)).await;
                        }
                        Ok(CheckOutput { ran: input.label })
                    })
                })
                .map_err(test_error)?,
        );
        let thread_stop = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            // A serve error here is the worker giving up entirely; the pins
            // assert on registry state, which such a give-up makes fail loudly.
            if let Err(error) = serve_with_redial(
                candidates,
                &config,
                &registry,
                RedialTiming::new(Duration::from_millis(20), Duration::from_millis(100)),
                &thread_stop,
                None,
                || {},
            ) {
                eprintln!("dead-man-switch worker stopped: {error}");
            }
        });
        Ok(Self {
            stop,
            executions,
            handle: Some(handle),
        })
    }

    fn executions(&self) -> usize {
        self.executions.load(Ordering::SeqCst)
    }

    fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// A TCP relay that can be WEDGED: it stops carrying bytes in both directions
/// while closing neither socket.
///
/// This is the faithful shape of the live failure. A relay that CLOSED would
/// prove nothing — a closed socket already surfaces promptly as a receive error
/// and already triggers a redial. The failure worth pinning is the one where
/// every read times out benignly and every write succeeds into a kernel buffer
/// nobody drains, so both ends believe they are connected.
struct WedgeableRelay {
    address: SocketAddr,
    wedged: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl WedgeableRelay {
    fn start(upstream: SocketAddr) -> Result<Self, TestError> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(test_error)?;
        let address = listener.local_addr().map_err(test_error)?;
        listener.set_nonblocking(true).map_err(test_error)?;
        let wedged = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let accept_wedged = Arc::clone(&wedged);
        let accept_stop = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            // Accepted sockets are PARKED here for the relay's whole life: a
            // dropped stream would emit a FIN and turn the wedge into an
            // ordinary close, which proves nothing.
            let mut parked: Vec<TcpStream> = Vec::new();
            while !accept_stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((downstream, _)) => {
                        // Any failure here leaves the connection unrelayed,
                        // which the pin surfaces as a missing registration
                        // rather than a silent pass.
                        let Ok(up) = TcpStream::connect(upstream) else {
                            continue;
                        };
                        let Ok(down_read) = downstream.try_clone() else {
                            continue;
                        };
                        let Ok(down_write) = downstream.try_clone() else {
                            continue;
                        };
                        let Ok(up_read) = up.try_clone() else {
                            continue;
                        };
                        let Ok(up_write) = up.try_clone() else {
                            continue;
                        };
                        parked.push(downstream);
                        parked.push(up);
                        for (from, to) in [(down_read, up_write), (up_read, down_write)] {
                            let pump_wedged = Arc::clone(&accept_wedged);
                            let pump_stop = Arc::clone(&accept_stop);
                            std::thread::spawn(move || pump(from, to, &pump_wedged, &pump_stop));
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
            // Only now do the parked sockets close.
            drop(parked);
        });
        Ok(Self {
            address,
            wedged,
            stop,
            handle: Some(handle),
        })
    }

    /// Stop carrying bytes in both directions, closing nothing.
    fn wedge(&self) {
        self.wedged.store(true, Ordering::SeqCst);
    }

    fn shutdown(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// One direction of the relay: forward bytes until wedged, then read and DISCARD
/// them, never closing either socket.
fn pump(mut from: TcpStream, mut to: TcpStream, wedged: &AtomicBool, stop: &AtomicBool) {
    if from
        .set_read_timeout(Some(Duration::from_millis(50)))
        .is_err()
    {
        return;
    }
    let mut buffer = [0_u8; 8192];
    while !stop.load(Ordering::SeqCst) {
        match from.read(&mut buffer) {
            Ok(0) => return,
            Ok(read) => {
                if wedged.load(Ordering::SeqCst) {
                    // Swallow the bytes. The socket stays open, so the sender
                    // sees a perfectly healthy write.
                    continue;
                }
                let Some(chunk) = buffer.get(..read) else {
                    return;
                };
                if to.write_all(chunk).is_err() || to.flush().is_err() {
                    return;
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return,
        }
    }
}

fn reserve_loopback_port() -> Result<SocketAddr, TestError> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(test_error)?;
    let address = listener.local_addr().map_err(test_error)?;
    drop(listener);
    Ok(address)
}
