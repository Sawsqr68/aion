//! The differential: BOTH transports publish into ONE eligibility set (#197).
//!
//! The uniform-probation ruling is not satisfied by each transport merely
//! working. It is satisfied only if there is one probation constant, one
//! tracker, and one published verdict — and a verdict is PUBLISHED BY
//! REPLACEMENT, so two probes, or one probe that formed its verdict from one
//! transport's census, would each erase the other transport's findings every
//! cadence.
//!
//! That failure is invisible to a single-transport test. A gRPC-only suite is
//! green while liminal workers are being silently re-admitted, and a
//! liminal-only suite is green while gRPC workers are. Only a fleet holding one
//! of each, judged in ONE round, can see it.
//!
//! These tests drive the probe's rounds directly and never sleep: the question
//! is what one round concludes about two workers, not how fast a timer fires.

#![cfg(feature = "liminal-transport")]

use std::sync::Arc;
use std::time::{Duration, Instant};

use aion_server::worker::registry::WorkerMessage;
use aion_server::worker::{
    ConnectedWorkerRegistry, GrpcLivenessWaiters, HeartbeatTracker, LiminalConnectionNotifier,
    LiminalWorkerDelivery, LivenessProbe, WorkerDelivery, WorkerId,
};

type TestError = Box<dyn std::error::Error>;

const NAMESPACE: &str = "default";
const TASK_QUEUE: &str = "default";
const ACTIVITY_TYPE: &str = "greet";
const HEARTBEAT_WINDOW: Duration = Duration::from_secs(4);

/// A fleet of exactly one worker per transport, sharing one registry, one
/// tracker, one waiter registry and one probe.
struct MixedFleet {
    registry: ConnectedWorkerRegistry,
    probe: LivenessProbe,
    waiters: GrpcLivenessWaiters,
    grpc_worker: WorkerId,
    liminal_worker: WorkerId,
    /// The gRPC worker's stream, held so pings can be read off it.
    grpc_stream: tokio::sync::mpsc::Receiver<WorkerMessage>,
    /// Registrations, held so the workers stay in the registry.
    _registrations: Vec<aion_server::worker::WorkerRegistration>,
}

impl MixedFleet {
    fn build() -> Result<Self, TestError> {
        let registry = ConnectedWorkerRegistry::default();
        let tracker = HeartbeatTracker::new(HEARTBEAT_WINDOW);
        let waiters = GrpcLivenessWaiters::new();
        let activity_types = [String::from(ACTIVITY_TYPE)];

        let (grpc_sender, grpc_stream) = tokio::sync::mpsc::channel(8);
        let grpc_registration = registry.register_delivery(
            [String::from(NAMESPACE)],
            String::from(TASK_QUEUE),
            None,
            None,
            activity_types.iter(),
            WorkerDelivery::Grpc(grpc_sender),
        )?;
        let grpc_worker = grpc_registration
            .worker_id()
            .ok_or("gRPC registration assigned no worker id")?;

        // A REAL liminal delivery over a real supervisor. It is deliberately
        // not connected to anything: this worker's role here is to be the other
        // transport in the same verdict, and a liminal worker that cannot be
        // pushed to is exactly the one that must stay excluded while the gRPC
        // worker clears.
        let supervisor = liminal_server::server::connection::ConnectionSupervisor::new()?;
        let liminal_registration = registry.register_delivery(
            [String::from(NAMESPACE)],
            String::from(TASK_QUEUE),
            None,
            None,
            activity_types.iter(),
            WorkerDelivery::Liminal(LiminalWorkerDelivery::new(supervisor, 7)),
        )?;
        let liminal_worker = liminal_registration
            .worker_id()
            .ok_or("liminal registration assigned no worker id")?;

        let now = Instant::now();
        tracker.register_connection(grpc_worker, now)?;
        tracker.register_connection(liminal_worker, now)?;

        // ONE probe over both. The notifier is real and empty — this boot hosts
        // no liminal connections, which is why the liminal worker is
        // unreachable rather than merely unenumerated.
        let probe = LivenessProbe::across_transports(
            Some(Arc::new(LiminalConnectionNotifier::new(registry.clone()))),
            Some(waiters.clone()),
            tracker,
            registry.clone(),
            HEARTBEAT_WINDOW,
        );

        Ok(Self {
            registry,
            probe,
            waiters,
            grpc_worker,
            liminal_worker,
            grpc_stream,
            _registrations: vec![grpc_registration, liminal_registration],
        })
    }

    /// Run one round while the gRPC worker answers its ping.
    async fn round_with_grpc_answering(&mut self, sequence: u64) -> Result<(), TestError> {
        let Self {
            probe,
            waiters,
            grpc_worker,
            grpc_stream,
            ..
        } = self;
        let answering = async {
            let Some(WorkerMessage::LivenessPing(ping)) = grpc_stream.recv().await else {
                return Err::<(), TestError>(
                    "the probe did not ping the gRPC worker in this round".into(),
                );
            };
            if !waiters.answer(*grpc_worker, ping.liveness_ping)? {
                return Err("the answer was not matched to the armed ping".into());
            }
            Ok(())
        };
        let (_, answered) = tokio::join!(probe.probe_once(sequence), answering);
        answered
    }
}

/// 🔴 THE DIFFERENTIAL. One round, two transports, ONE published set — and the
/// set must name the unreachable liminal worker while the answering gRPC worker
/// is admitted.
///
/// The two halves are both load-bearing. If the verdict were formed from the
/// gRPC census alone, the liminal worker would silently drop out of the
/// exclusion set and be selected despite the server having no way to reach it.
/// If it were formed from the liminal census alone, the gRPC worker would never
/// leave it however faithfully it answered. Only asserting BOTH memberships
/// from ONE round's verdict distinguishes one set from two.
#[tokio::test]
async fn one_round_publishes_one_set_covering_both_transports() -> Result<(), TestError> {
    let mut fleet = MixedFleet::build()?;

    // Round 1: the gRPC worker banks its first answer; neither worker is
    // eligible yet (a probation takes a consecutive RUN).
    fleet.round_with_grpc_answering(1).await?;
    let excluded = fleet.registry.dispatch_ineligible()?;
    assert!(
        excluded.contains(&fleet.grpc_worker) && excluded.contains(&fleet.liminal_worker),
        "after one round both workers are still serving/failing their probation, and both must \
         appear in the one published set: {excluded:?}"
    );

    // Round 2: the gRPC worker completes its run.
    fleet.round_with_grpc_answering(2).await?;
    let excluded = fleet.registry.dispatch_ineligible()?;
    assert!(
        !excluded.contains(&fleet.grpc_worker),
        "the gRPC worker answered a full consecutive run and must be admitted: {excluded:?}"
    );
    assert!(
        excluded.contains(&fleet.liminal_worker),
        "the SAME round's SAME set must still exclude the liminal worker the server cannot reach. \
         A verdict formed from the gRPC census alone would have dropped it here, silently \
         re-admitting a worker with no reachable dispatch path: {excluded:?}"
    );
    assert!(
        !fleet.registry.is_dispatch_ineligible(fleet.grpc_worker)?
            && fleet
                .registry
                .is_dispatch_ineligible(fleet.liminal_worker)?,
        "selection reads the same one set both transports published into"
    );

    // And selection itself honours it: the gRPC worker is the only choice.
    let selected = fleet
        .registry
        .select_worker(NAMESPACE, TASK_QUEUE, ACTIVITY_TYPE, None)?
        .ok_or("the eligible gRPC worker must be selectable")?;
    assert_eq!(
        selected.id(),
        fleet.grpc_worker,
        "the pool holds two workers and exactly one is eligible; selection must pick that one"
    );
    Ok(())
}

/// R3, the empty-vs-excluded fixture: a pool whose only workers are
/// eligibility-excluded is NOT an empty pool, and the two must not be reported
/// the same way.
///
/// The control is the whole point — an empty ADDRESS in the same registry must
/// still report zero excluded workers, or the distinction would be a constant
/// dressed as a measurement.
#[tokio::test]
async fn an_excluded_pool_is_distinguishable_from_an_empty_one() -> Result<(), TestError> {
    let fleet = MixedFleet::build()?;
    // One unanswered round: it publishes the verdict that excludes both.
    fleet.probe.probe_once(1).await;

    assert!(
        fleet
            .registry
            .select_worker(NAMESPACE, TASK_QUEUE, ACTIVITY_TYPE, None)?
            .is_none(),
        "precondition: selection finds nobody, which is the miss R3 is about"
    );
    assert_eq!(
        fleet
            .registry
            .ineligible_workers_for(NAMESPACE, TASK_QUEUE, ACTIVITY_TYPE, None)?,
        2,
        "both registered workers are excluded, and the refusal must be able to say so instead of \
         telling an operator nobody is registered"
    );
    assert_eq!(
        fleet.registry.ineligible_workers_for(
            NAMESPACE,
            "nobody-serves-this",
            ACTIVITY_TYPE,
            None
        )?,
        0,
        "THE CONTROL: a genuinely empty address reports zero excluded workers. Without this, a \
         count that returned the whole fleet regardless of address would satisfy the assertion \
         above and the refusal would misreport every empty pool as an excluded one"
    );
    Ok(())
}
