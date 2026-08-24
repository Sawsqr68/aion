//! aion#25 — a liveness verdict may only judge the transports it can measure.
//!
//! # The defect
//!
//! [`LivenessProbe::publish_reachability_verdict`] forms the published dispatch
//! eligibility set from `HeartbeatTracker::unreachable_workers()` — EVERY
//! tracked worker — with no regard for whether this probe has any way to ping
//! the transport that worker is delivered over. A worker whose transport the
//! probe does not carry can therefore never bank an answer, never serve its
//! probation, and never leave the exclusion set: it is excluded by definition,
//! permanently, by a mechanism that has no path to test it.
//!
//! That is what was reported live: a typed gRPC worker registered on a server
//! whose liveness probe covered the liminal transport only. It ran its first
//! activity, the first probe round published a verdict formed from the whole
//! tracker, and every activity after that parked as though no compatible worker
//! existed.
//!
//! # Why the #197 gRPC arm did not close it
//!
//! #197 taught the probe to ping gRPC-delivered workers over their task streams
//! and to feed those answers into the same probation, so the PRODUCTION wiring
//! ([`ServerState::spawn_liminal_liveness_probe`]) now passes both transports
//! and a healthy gRPC worker clears. But the verdict itself stayed unscoped:
//! its correctness became a property of that one call site passing `Some` twice,
//! not a property of the probe. [`LivenessProbe::new`] — the documented
//! construction "for callers with no gRPC fleet" — still publishes exclusions
//! over gRPC workers it cannot ping, which is the original defect intact behind
//! a wiring accident.
//!
//! # The law these tests state
//!
//! A verdict carries a worker ONLY if this probe covers the transport that
//! worker is delivered over. Coverage is structural — the probe holds a liminal
//! notifier, a gRPC answer registry, or both — never a property of how many
//! targets a particular round happened to enumerate. Scoping to the round's
//! targets instead would silently re-admit exactly the worker
//! [`the_same_verdict_still_excludes_an_unreachable_worker_of_a_covered_transport`]
//! pins: one whose connection is GONE, which is why that control is here.

#![cfg(feature = "liminal-transport")]

use std::error::Error;
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use aion_server::worker::registry::WorkerMessage;
use aion_server::worker::{
    ConnectedWorkerRegistry, GrpcLivenessWaiters, HeartbeatSweeper, HeartbeatTracker,
    LiminalConnectionNotifier, LiminalWorkerDelivery, LivenessProbe, WorkerDelivery, WorkerId,
    WorkerRegistration, sweep_interval,
};
use aion_worker::{ActivityRegistry, RedialTiming, WorkerConfig, serve_with_redial};
use liminal_server::config::{ChannelDef, ServerConfig};
use liminal_server::server::connection::ConnectionSupervisor;
use liminal_server::server::listener::ServerListener;
use serde::{Deserialize, Serialize};

type TestError = Box<dyn Error + Send + Sync>;
type TestResult = Result<(), TestError>;

const NAMESPACE: &str = "remote";
/// The liminal worker's pool. Deliberately DIFFERENT from the gRPC worker's, so
/// `select_worker` on either address names exactly one worker and a pass can
/// never come from the other transport standing in.
const LIMINAL_QUEUE: &str = "liminal-gates";
const GRPC_QUEUE: &str = "grpc-gates";
const ACTIVITY_TYPE: &str = "run-check";

/// The operator's `worker.heartbeat_window` for these pins — the same four
/// seconds the dead-man-switch pins use, which keeps the production ratio: a
/// one-second derived cadence, so a healthy connection is pinged four times per
/// window exactly as a 30s production window is pinged every 7.5s.
const HEARTBEAT_WINDOW: Duration = Duration::from_secs(4);

/// How long a pin waits for a state change before failing. Generously more than
/// two heartbeat windows, so a slow machine never turns a correct
/// implementation red.
const OBSERVE: Duration = Duration::from_secs(20);

/// How long the deterministic fleet's gRPC worker waits for its ping before
/// declaring that none was sent. Bounded so a probe that stops enumerating gRPC
/// deliveries produces a readable FAILURE rather than a hung suite.
const PING_WAIT: Duration = Duration::from_secs(5);

fn test_error(message: impl std::fmt::Display) -> TestError {
    message.to_string().into()
}

// ------------------------------------------------- the deterministic fleet --

/// One worker per transport in one registry, one tracker and one probe, with no
/// sockets anywhere: these pins ask what a ROUND concludes, not how fast a timer
/// fires, so every round is driven by hand.
struct Fleet {
    registry: ConnectedWorkerRegistry,
    tracker: HeartbeatTracker,
    waiters: GrpcLivenessWaiters,
    grpc_worker: WorkerId,
    liminal_worker: WorkerId,
    grpc_stream: tokio::sync::mpsc::Receiver<WorkerMessage>,
    /// Registrations, held so the workers stay in the registry — and dropped one
    /// at a time by [`Fleet::deregister_liminal`] to make a worker DEPART.
    registrations: Vec<WorkerRegistration>,
}

impl Fleet {
    /// Build the fleet. `liminal_worker` is registered only when `with_liminal`
    /// — a pin about an empty liminal census needs a fleet that has no liminal
    /// worker to be honest about.
    fn build(with_liminal: bool) -> Result<Self, TestError> {
        let registry = ConnectedWorkerRegistry::default();
        let tracker = HeartbeatTracker::new(HEARTBEAT_WINDOW);
        let waiters = GrpcLivenessWaiters::new();
        let activity_types = [String::from(ACTIVITY_TYPE)];
        let now = Instant::now();

        let (grpc_sender, grpc_stream) = tokio::sync::mpsc::channel(8);
        let grpc_registration = registry.register_delivery(
            [String::from(NAMESPACE)],
            String::from(GRPC_QUEUE),
            None,
            None,
            activity_types.iter(),
            WorkerDelivery::Grpc(grpc_sender),
        )?;
        let grpc_worker = grpc_registration
            .worker_id()
            .ok_or_else(|| test_error("gRPC registration assigned no worker id"))?;
        tracker.register_connection(grpc_worker, now)?;

        let mut registrations = vec![grpc_registration];
        // A REAL liminal delivery over a real supervisor, deliberately connected
        // to nothing: this is a worker whose dispatch path the server genuinely
        // cannot reach, which is what the control below needs.
        let liminal_worker = if with_liminal {
            let supervisor = ConnectionSupervisor::new()?;
            let liminal_registration = registry.register_delivery(
                [String::from(NAMESPACE)],
                String::from(LIMINAL_QUEUE),
                None,
                None,
                activity_types.iter(),
                WorkerDelivery::Liminal(LiminalWorkerDelivery::new(supervisor, 7)),
            )?;
            let liminal_worker = liminal_registration
                .worker_id()
                .ok_or_else(|| test_error("liminal registration assigned no worker id"))?;
            tracker.register_connection(liminal_worker, now)?;
            registrations.push(liminal_registration);
            liminal_worker
        } else {
            // No liminal worker exists; the id names nobody and is never asserted
            // on in the pins that build this way.
            WorkerId::from_value(u64::MAX)
        };

        Ok(Self {
            registry,
            tracker,
            waiters,
            grpc_worker,
            liminal_worker,
            grpc_stream,
            registrations,
        })
    }

    /// A probe over exactly the transports named: `liminal` gives it a notifier,
    /// `grpc` gives it the answer-correlation registry it must hold to hear a
    /// gRPC answer at all.
    fn probe(&self, liminal: bool, grpc: bool) -> LivenessProbe {
        LivenessProbe::across_transports(
            liminal.then(|| Arc::new(LiminalConnectionNotifier::new(self.registry.clone()))),
            grpc.then(|| self.waiters.clone()),
            self.tracker.clone(),
            self.registry.clone(),
            HEARTBEAT_WINDOW,
        )
    }

    /// Drop the liminal worker's registration, deregistering it while its
    /// tracker entry survives — a worker that has LEFT, whose transport is
    /// therefore no longer knowable.
    fn deregister_liminal(&mut self) {
        // The liminal registration is pushed last by `build`, so this is it.
        drop(self.registrations.pop());
    }

    /// Drive one round while the gRPC worker answers its ping, exactly as a
    /// healthy SDK runtime would.
    async fn round_with_grpc_answering(
        &mut self,
        probe: &LivenessProbe,
        sequence: u64,
    ) -> TestResult {
        let Self {
            waiters,
            grpc_worker,
            grpc_stream,
            ..
        } = self;
        let answering = async {
            let received = tokio::time::timeout(PING_WAIT, grpc_stream.recv())
                .await
                .map_err(|_| {
                    test_error(format!(
                        "NO LIVENESS PING REACHED THE gRPC WORKER within {PING_WAIT:?}: the probe \
                         did not enumerate its delivery, so it can never clear its probation"
                    ))
                })?;
            let Some(WorkerMessage::LivenessPing(ping)) = received else {
                return Err(test_error(
                    "the probe did not ping the gRPC worker in this round",
                ));
            };
            if !waiters.answer(*grpc_worker, ping.liveness_ping)? {
                return Err(test_error("the answer was not matched to the armed ping"));
            }
            Ok(())
        };
        let ((), answered) = tokio::join!(probe.probe_once(sequence), answering);
        answered
    }
}

/// 🔴 THE DEFECT, stated over the transport the probe cannot carry.
///
/// A probe holding no liminal notifier has no wire on which to ask a liminal
/// worker anything. It must therefore not publish a verdict about one: an
/// exclusion no ping can ever lift is permanent, and permanence is the whole
/// injury of aion#25 — the excluded worker stays registered, stays healthy, and
/// is never selected again.
///
/// The gRPC half of the same round is the instrument check. Without it a probe
/// that had simply stopped working would satisfy the liminal assertion by doing
/// nothing at all.
#[tokio::test]
async fn a_probe_that_cannot_reach_a_transport_leaves_its_workers_alone() -> TestResult {
    let mut fleet = Fleet::build(true)?;
    let probe = fleet.probe(false, true);

    // Two rounds: DISPATCH_PROBATION_PINGS consecutive answers is what earns
    // eligibility, so this is the shortest run that proves the probe works.
    fleet.round_with_grpc_answering(&probe, 1).await?;
    fleet.round_with_grpc_answering(&probe, 2).await?;

    let excluded = fleet.registry.dispatch_ineligible()?;
    assert!(
        !excluded.contains(&fleet.grpc_worker),
        "INSTRUMENT CHECK: the gRPC worker answered a full consecutive run over the transport \
         this probe DOES carry, so it must be admitted. If it is not, the assertion below \
         measures a probe that is not working at all: {excluded:?}"
    );
    assert!(
        !excluded.contains(&fleet.liminal_worker),
        "a probe with no liminal notifier has no path to ping a liminal worker, so it must not \
         publish an exclusion for one — that exclusion could never be lifted by any answer, and \
         a healthy worker would be withheld from dispatch for the life of the process: \
         {excluded:?}"
    );
    let selected = fleet
        .registry
        .select_worker(NAMESPACE, LIMINAL_QUEUE, ACTIVITY_TYPE, None)?
        .ok_or_else(|| {
            test_error(
                "and selection must actually see it: the registered worker on this queue was \
                 withheld by a verdict formed on a transport the probe cannot measure",
            )
        })?;
    assert_eq!(selected.id(), fleet.liminal_worker);
    Ok(())
}

/// THE CONTROL, and it is load-bearing. The fix must scope the verdict by the
/// transports the probe CARRIES, never by the targets one round happened to
/// enumerate.
///
/// This liminal worker is registered and tracked but its connection is gone, so
/// it appears in no round's target list — yet a probe that carries the liminal
/// transport has measured it and found it unreachable, and it must stay
/// excluded. A fix that scoped to enumerated targets would drop it here and
/// silently re-admit a worker with no reachable dispatch path, which is a worse
/// defect than the one being fixed.
#[tokio::test]
async fn the_same_verdict_still_excludes_an_unreachable_worker_of_a_covered_transport() -> TestResult
{
    let mut fleet = Fleet::build(true)?;
    let probe = fleet.probe(true, true);

    fleet.round_with_grpc_answering(&probe, 1).await?;
    fleet.round_with_grpc_answering(&probe, 2).await?;

    let excluded = fleet.registry.dispatch_ineligible()?;
    assert!(
        excluded.contains(&fleet.liminal_worker),
        "this probe CARRIES the liminal transport, so its silence about this worker is a \
         measurement: the worker has no live connection, the server cannot reach its dispatch \
         path, and it must stay excluded: {excluded:?}"
    );
    assert!(
        !excluded.contains(&fleet.grpc_worker),
        "and the worker that answered over its own transport must be admitted from the SAME \
         set: {excluded:?}"
    );
    assert!(
        fleet
            .registry
            .select_worker(NAMESPACE, LIMINAL_QUEUE, ACTIVITY_TYPE, None)?
            .is_none(),
        "selection must honour that exclusion, not merely be told about it"
    );
    assert_eq!(
        fleet
            .registry
            .select_worker(NAMESPACE, GRPC_QUEUE, ACTIVITY_TYPE, None)?
            .map(|handle| handle.id()),
        Some(fleet.grpc_worker),
        "and the gRPC worker stays selectable"
    );
    Ok(())
}

/// 🔴 An empty round must still publish, and what it publishes must not include
/// a transport it cannot re-measure (the issue's fourth acceptance criterion).
///
/// A verdict is published BY REPLACEMENT, so a round that publishes nothing
/// leaves the previous one standing. When the previous one names a gRPC worker
/// this probe cannot ping, "leaves it standing" means the worker is excluded for
/// ever with no mechanism that could ever clear it. Publishing the scoped set —
/// even when it is empty — is what clears it.
#[tokio::test]
async fn an_empty_round_clears_a_stale_verdict_it_cannot_re_measure() -> TestResult {
    let fleet = Fleet::build(false)?;
    // The liminal-only construction, with no liminal connections: this round has
    // no target of any kind.
    let probe = fleet.probe(true, false);
    fleet
        .registry
        .set_dispatch_ineligible([fleet.grpc_worker].into_iter().collect())?;

    probe.probe_once(1).await;

    let excluded = fleet.registry.dispatch_ineligible()?;
    assert!(
        !excluded.contains(&fleet.grpc_worker),
        "an empty liminal round measured nothing about a gRPC worker, so it must not leave a \
         stale exclusion standing over one — nothing this probe can do would ever lift it: \
         {excluded:?}"
    );
    assert_eq!(
        fleet
            .registry
            .select_worker(NAMESPACE, GRPC_QUEUE, ACTIVITY_TYPE, None)?
            .map(|handle| handle.id()),
        Some(fleet.grpc_worker),
        "and the worker is dispatchable again, which is the fact the operator cares about"
    );
    // The tracker still holds the gRPC worker's unserved probation: the verdict
    // was scoped, not the tracker rewritten.
    assert!(
        !fleet
            .tracker
            .is_dispatch_reachable(fleet.grpc_worker, Instant::now())?,
        "the scoping must happen at the VERDICT. Forging tracker proof the probe never obtained \
         would make a later gRPC-carrying probe believe this worker had answered"
    );
    Ok(())
}

/// A worker that has LEFT the registry has no transport, and a verdict must not
/// name it.
///
/// The scoping question is "does this probe hold a wire to this worker's
/// transport"; a departed worker has no transport to answer it with. Guessing
/// one — treating absence as covered — would put a name selection can no longer
/// see into an operator-facing set, and would make the departure look like an
/// exclusion the operator should act on. The tracker's own entry is untouched:
/// this is a scope on the VERDICT, not a second reaper.
#[tokio::test]
async fn a_worker_that_left_the_registry_is_named_by_no_verdict() -> TestResult {
    let mut fleet = Fleet::build(true)?;
    let probe = fleet.probe(true, true);

    fleet.round_with_grpc_answering(&probe, 1).await?;
    assert!(
        fleet
            .registry
            .dispatch_ineligible()?
            .contains(&fleet.liminal_worker),
        "precondition: while it was registered, the unreachable liminal worker was excluded — so \
         the change below is a change and not a standing condition"
    );

    fleet.deregister_liminal();
    fleet.round_with_grpc_answering(&probe, 2).await?;

    let excluded = fleet.registry.dispatch_ineligible()?;
    assert!(
        !excluded.contains(&fleet.liminal_worker),
        "a departed worker has no transport, so no probe is entitled to judge it and no verdict \
         may name it: {excluded:?}"
    );
    assert!(
        fleet
            .tracker
            .unreachable_workers(Instant::now())?
            .iter()
            .any(|entry| entry.worker_id == fleet.liminal_worker),
        "and the tracker still holds its standing evidence — the scope is on the verdict, not a \
         second reaper quietly rewriting what was measured"
    );
    Ok(())
}

// --------------------------------------------------- the real-transport pin --

/// 🔴 THE LIVE SHAPE, over real sockets. A registered gRPC worker must stay
/// dispatchable across repeated REAL liminal probe rounds (acceptance criteria
/// one and five).
///
/// Everything here is production machinery: a real liminal listener with the
/// aion notifier installed, a real remote `aion-worker` connected through the
/// production redial entry point, the real [`LivenessProbe`] on its derived
/// cadence, and the real [`HeartbeatSweeper`]. The gRPC worker is registered
/// into the SAME registry and tracker and beats its connection lease exactly as
/// a connected SDK does.
///
/// Red before the fix: the first round's verdict is formed from the whole
/// tracker, the gRPC worker has banked no answer to a ping it was never sent,
/// and selection stops returning it for the rest of the process.
#[test]
fn a_grpc_worker_stays_dispatchable_across_real_liminal_probe_rounds() -> TestResult {
    let mut harness = Harness::start()?;
    let worker = WorkerThread::spawn(vec![harness.address.to_string()])?;
    harness.wait_for_liminal_worker()?;

    let grpc_worker = harness.register_grpc_worker()?;
    let beater = LeaseBeater::start(harness.tracker.clone(), grpc_worker);

    // BEFORE any probe round has concluded: the worker is selectable. Stating
    // this first is what makes the assertion after the rounds a CHANGE rather
    // than a standing condition that was never true.
    assert_eq!(
        harness
            .registry
            .select_worker(NAMESPACE, GRPC_QUEUE, ACTIVITY_TYPE, None)?
            .map(|handle| handle.id()),
        Some(grpc_worker),
        "precondition: a freshly registered gRPC worker is dispatchable"
    );

    // Wait for PROOF that real rounds ran, rather than for a duration: the
    // liminal worker becomes dispatch-reachable only by answering a full
    // consecutive run of real pings pushed over its real connection.
    harness.wait_for_liminal_reachability()?;
    // And then let several more verdicts publish, so the pin is about the
    // steady state and not about one lucky round.
    std::thread::sleep(sweep_interval(HEARTBEAT_WINDOW) * 3);

    assert!(
        !harness.registry.is_dispatch_ineligible(grpc_worker)?,
        "the liminal probe published its verdict over a gRPC worker it never pinged and could \
         never ping: the worker is registered, healthy and beating, and has been withdrawn from \
         dispatch by a mechanism with no path to test it"
    );
    assert_eq!(
        harness
            .registry
            .select_worker(NAMESPACE, GRPC_QUEUE, ACTIVITY_TYPE, None)?
            .map(|handle| handle.id()),
        Some(grpc_worker),
        "and the activity that would have parked instead finds its worker"
    );
    // Both transports are serving in one server: the liminal worker earned its
    // eligibility over the wire in the same window (acceptance criterion three —
    // an answered run RESTORES eligibility, with no separate trigger).
    assert_eq!(
        harness
            .registry
            .select_worker(NAMESPACE, LIMINAL_QUEUE, ACTIVITY_TYPE, None)?
            .map(|handle| handle.id()),
        harness.liminal_worker(),
        "the real liminal worker answered its pings and must be selectable too"
    );
    // And the eligibility above was NOT bought by quietly probing the gRPC
    // worker after all: this probe carries no gRPC answer registry, so nothing
    // may have gone down that stream. Without this, a probe that had grown a
    // gRPC arm would satisfy every assertion here and the pin would no longer
    // be about scoping at all.
    assert_eq!(
        harness.grpc_frames_pushed(),
        0,
        "a probe with no gRPC wire pushed a frame down the gRPC worker's stream"
    );

    beater.stop();
    worker.stop();
    harness.shutdown()
}

// -------------------------------------------------------------- harness ----

/// A running liminal listener with the aion notifier, the liveness probe and the
/// expiry sweeper — the production trio, built the way `run.rs` builds it.
struct Harness {
    listener: Option<ServerListener>,
    registry: ConnectedWorkerRegistry,
    tracker: HeartbeatTracker,
    address: SocketAddr,
    runtime: Option<tokio::runtime::Runtime>,
    shutdown: tokio::sync::watch::Sender<bool>,
    /// The gRPC registration, held so the worker stays registered.
    grpc_registration: Option<WorkerRegistration>,
    /// The gRPC worker's stream, held open so its delivery channel stays live —
    /// and read at the end of the pin, because a probe with no gRPC wire must
    /// have pushed NOTHING down it.
    grpc_stream: Option<tokio::sync::mpsc::Receiver<WorkerMessage>>,
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

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(test_error)?;
        let (shutdown, shutdown_rx) = tokio::sync::watch::channel(false);
        // The LIMINAL-ONLY probe: the documented construction for a boot with no
        // gRPC fleet, and the exact shape the defect was reported against.
        let probe = LivenessProbe::new(
            notifier,
            tracker.clone(),
            registry.clone(),
            HEARTBEAT_WINDOW,
        );
        let sweeper = HeartbeatSweeper::new(
            tracker.clone(),
            registry.clone(),
            aion_server::worker::PendingActivities::new(HEARTBEAT_WINDOW),
            aion_server::shutdown::DrainState::default(),
            HEARTBEAT_WINDOW,
        );
        runtime.spawn(probe.run(shutdown_rx.clone()));
        runtime.spawn(sweeper.run(shutdown_rx));

        Ok(Self {
            listener: Some(listener),
            registry,
            tracker,
            address,
            runtime: Some(runtime),
            shutdown,
            grpc_registration: None,
            grpc_stream: None,
        })
    }

    /// Register a gRPC-delivered worker into the SAME registry and tracker the
    /// liminal fleet uses — the mixed-transport server the issue describes.
    fn register_grpc_worker(&mut self) -> Result<WorkerId, TestError> {
        let activity_types = [String::from(ACTIVITY_TYPE)];
        let (sender, stream) = tokio::sync::mpsc::channel(8);
        let registration = self.registry.register_delivery(
            [String::from(NAMESPACE)],
            String::from(GRPC_QUEUE),
            None,
            None,
            activity_types.iter(),
            WorkerDelivery::Grpc(sender),
        )?;
        let worker_id = registration
            .worker_id()
            .ok_or_else(|| test_error("gRPC registration assigned no worker id"))?;
        self.tracker
            .register_connection(worker_id, Instant::now())?;
        self.grpc_registration = Some(registration);
        self.grpc_stream = Some(stream);
        Ok(worker_id)
    }

    /// How many frames this server has pushed down the gRPC worker's stream.
    ///
    /// Counted by draining what is queued rather than by watching the push: the
    /// stream has no other consumer in this pin, so anything the server sent is
    /// still sitting there.
    fn grpc_frames_pushed(&mut self) -> usize {
        let Some(stream) = self.grpc_stream.as_mut() else {
            return 0;
        };
        let mut pushed = 0;
        while stream.try_recv().is_ok() {
            pushed += 1;
        }
        pushed
    }

    /// The liminal worker's registry id, once it has connected.
    fn liminal_worker(&self) -> Option<WorkerId> {
        self.registry
            .all_workers()
            .unwrap_or_default()
            .into_iter()
            .find(|handle| matches!(handle.delivery(), WorkerDelivery::Liminal(_)))
            .map(|handle| handle.id())
    }

    fn wait_for_liminal_worker(&self) -> TestResult {
        let deadline = Instant::now() + OBSERVE;
        while Instant::now() < deadline {
            if self.liminal_worker().is_some() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Err(test_error("the liminal worker never registered"))
    }

    /// Wait until the server has PROVED it can reach the liminal worker's
    /// dispatch path — which only a consecutive run of answered real pings can
    /// establish, and is therefore direct evidence that probe rounds ran.
    fn wait_for_liminal_reachability(&self) -> TestResult {
        let deadline = Instant::now() + OBSERVE;
        while Instant::now() < deadline {
            if let Some(worker) = self.liminal_worker()
                && self.tracker.is_dispatch_reachable(worker, Instant::now())?
            {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Err(test_error(
            "the liminal worker never became dispatch-reachable, so no probe round can be shown \
             to have concluded and the pin below would measure nothing",
        ))
    }

    fn shutdown(mut self) -> TestResult {
        self.shutdown.send(true).map_err(test_error)?;
        drop(self.grpc_registration.take());
        if let Some(listener) = self.listener.take() {
            listener.shutdown().map_err(test_error)?;
        }
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_timeout(Duration::from_secs(5));
        }
        Ok(())
    }
}

/// Beats a gRPC worker's CONNECTION lease on the quarter-window cadence a
/// connected SDK heartbeats at.
///
/// Without it the expiry sweeper reaps the worker for silence and the pin would
/// go red for a reason that has nothing to do with the liveness verdict. It
/// advances only the lease — never dispatch reachability — so the fact under
/// test is untouched: an inbound frame proves the worker's process is alive and
/// says nothing about whether the server can push to it.
struct LeaseBeater {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
    failures: Arc<AtomicUsize>,
}

impl LeaseBeater {
    fn start(tracker: HeartbeatTracker, worker: WorkerId) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let failures = Arc::new(AtomicUsize::new(0));
        let thread_stop = Arc::clone(&stop);
        let thread_failures = Arc::clone(&failures);
        let cadence = sweep_interval(HEARTBEAT_WINDOW);
        let handle = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::SeqCst) {
                match tracker.record_connection_activity(worker, Instant::now()) {
                    // `false` means the worker was already deregistered; the
                    // beater stops rather than looping on a dead id, and the
                    // pin's own assertions report what that meant.
                    Ok(true) => {}
                    Ok(false) => return,
                    Err(_) => {
                        thread_failures.fetch_add(1, Ordering::SeqCst);
                        return;
                    }
                }
                std::thread::sleep(cadence / 2);
            }
        });
        Self {
            stop,
            handle: Some(handle),
            failures,
        }
    }

    fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            // A panicked beater is reported by the pin's assertions failing; the
            // join result carries nothing else to act on.
            drop(handle.join());
        }
        assert_eq!(
            self.failures.load(Ordering::SeqCst),
            0,
            "the lease beater hit a tracker error, so the gRPC worker's lease was not being kept \
             alive and any conclusion drawn above about its eligibility is unsafe"
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CheckInput {
    label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CheckOutput {
    ran: String,
}

/// The remote liminal worker, driven through the PRODUCTION redial entry point
/// on its own OS thread (the push receive is blocking).
struct WorkerThread {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl WorkerThread {
    fn spawn(candidates: Vec<String>) -> Result<Self, TestError> {
        let stop = Arc::new(AtomicBool::new(false));
        let config = WorkerConfig::builder()
            .endpoint("unused-direct-address")
            .namespace(NAMESPACE)
            .task_queue(LIMINAL_QUEUE)
            .identity("mixed-transport-liminal-worker")
            .max_concurrency(1)
            .reconnect_initial_backoff(Duration::from_millis(20))
            .reconnect_max_backoff(Duration::from_millis(100))
            .reconnect_max_attempts(3)
            .build()
            .map_err(test_error)?;
        let registry = Arc::new(
            ActivityRegistry::new()
                .register_activity(ACTIVITY_TYPE, move |input: CheckInput, _context| {
                    Box::pin(async move { Ok(CheckOutput { ran: input.label }) })
                })
                .map_err(test_error)?,
        );
        let thread_stop = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            // A serve error here is the worker giving up entirely; the pin
            // asserts on registry state, which such a give-up makes fail loudly.
            if let Err(error) = serve_with_redial(
                candidates,
                &config,
                &registry,
                RedialTiming::new(Duration::from_millis(20), Duration::from_millis(100)),
                &thread_stop,
                None,
                || {},
            ) {
                eprintln!("mixed-transport liminal worker stopped: {error}");
            }
        });
        Ok(Self {
            stop,
            handle: Some(handle),
        })
    }

    fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            drop(handle.join());
        }
    }
}

fn reserve_loopback_port() -> Result<SocketAddr, TestError> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(test_error)?;
    let address = listener.local_addr().map_err(test_error)?;
    drop(listener);
    Ok(address)
}
