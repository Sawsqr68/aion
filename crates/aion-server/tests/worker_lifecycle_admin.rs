//! Regression coverage for connection leases and worker drain fencing.

use std::time::{Duration, Instant};

use aion_core::{ActivityId, WorkflowId};
use aion_server::ServerError;
use aion_server::worker::registry::WorkerMessage;
use aion_server::worker::{
    ActivityCompletion, ActivityCompletionSink, CompletionToken, ConnectedWorkerRegistry,
    HeartbeatTracker, InFlightActivity, WorkerDelivery,
};

#[derive(Debug, Default)]
struct NoopSink;

impl ActivityCompletionSink for NoopSink {
    fn complete_activity(&self, completion: ActivityCompletion) -> Result<(), ServerError> {
        std::hint::black_box(completion);
        Ok(())
    }

    fn park_activity(
        &self,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
    ) -> Result<(), ServerError> {
        std::hint::black_box((workflow_id, activity_id));
        Ok(())
    }
}

#[test]
fn idle_worker_connection_lease_expiry_deregisters_worker() -> Result<(), Box<dyn std::error::Error>>
{
    let registry = ConnectedWorkerRegistry::default();
    let tracker = HeartbeatTracker::new(Duration::from_secs(5));
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    let activity_types = [String::from("idle")];
    let registration = registry.register("default", activity_types.iter(), sender)?;
    let worker_id = registration
        .worker_id()
        .ok_or_else(|| std::io::Error::other("registration did not assign a worker id"))?;
    std::hint::black_box(&receiver);
    let now = Instant::now();
    tracker.register_connection(worker_id, now)?;

    let reports =
        tracker.fail_expired_workers(&registry, &NoopSink, now + Duration::from_secs(6))?;

    assert_eq!(reports.len(), 1);
    assert!(reports[0].tasks.is_empty());
    assert!(!registry.is_registered(worker_id)?);
    Ok(())
}

#[test]
fn active_worker_heartbeat_refreshes_connection_lease() -> Result<(), Box<dyn std::error::Error>> {
    let registry = ConnectedWorkerRegistry::default();
    let tracker = HeartbeatTracker::new(Duration::from_secs(5));
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    let activity_types = [String::from("active")];
    let registration = registry.register("default", activity_types.iter(), sender)?;
    let worker_id = registration
        .worker_id()
        .ok_or_else(|| std::io::Error::other("registration did not assign a worker id"))?;
    std::hint::black_box(&receiver);
    let now = Instant::now();
    let workflow_id = WorkflowId::new(uuid::Uuid::from_u128(1));
    let activity_id = ActivityId::from_sequence_position(1);
    tracker.register_connection(worker_id, now)?;
    tracker.track_task(
        worker_id,
        InFlightActivity {
            workflow_id: workflow_id.clone(),
            activity_id: activity_id.clone(),
            completion_token: CompletionToken::from_wire(
                &workflow_id,
                &activity_id,
                "lease-test-generation".to_owned(),
            )?,
        },
        now,
    )?;
    assert!(tracker.record_liveness(
        worker_id,
        &workflow_id,
        &activity_id,
        now + Duration::from_secs(4),
    )?);
    assert!(tracker.record_connection_activity(worker_id, now + Duration::from_secs(4))?);

    let reports =
        tracker.fail_expired_workers(&registry, &NoopSink, now + Duration::from_secs(6))?;

    assert!(reports.is_empty());
    assert!(registry.is_registered(worker_id)?);
    Ok(())
}

#[tokio::test]
async fn targeted_grpc_drain_fences_before_signalling() -> Result<(), Box<dyn std::error::Error>> {
    let registry = ConnectedWorkerRegistry::default();
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    let activity_types = [String::from("drainable")];
    let registration = registry.register("default", activity_types.iter(), sender)?;
    let worker_id = registration
        .worker_id()
        .ok_or_else(|| std::io::Error::other("registration did not assign a worker id"))?;

    assert!(registry.drain_worker(worker_id)?);
    assert!(
        registry
            .select_worker("default", "default", "drainable", None)?
            .is_none()
    );
    let message = receiver
        .recv()
        .await
        .ok_or_else(|| std::io::Error::other("drain signal channel closed"))?;
    assert_eq!(message, WorkerMessage::DrainRequest);
    Ok(())
}

#[cfg(feature = "liminal-transport")]
#[test]
fn shutdown_drain_fences_and_deregisters_liminal_worker() -> Result<(), Box<dyn std::error::Error>>
{
    use aion_server::worker::LiminalWorkerDelivery;

    let registry = ConnectedWorkerRegistry::default();
    let supervisor = liminal_server::server::connection::ConnectionSupervisor::new()?;
    let delivery = WorkerDelivery::Liminal(LiminalWorkerDelivery::new(supervisor, 7));
    let activity_types = [String::from("liminal")];
    let registration = registry.register_delivery(
        [String::from("default")],
        String::from("default"),
        None,
        None,
        activity_types.iter(),
        delivery,
    )?;
    let worker_id = registration
        .worker_id()
        .ok_or_else(|| std::io::Error::other("registration did not assign a worker id"))?;

    assert_eq!(registry.broadcast_drain()?, 0);
    assert!(!registry.is_registered(worker_id)?);
    assert!(
        registry
            .select_worker("default", "default", "liminal", None)?
            .is_none()
    );
    Ok(())
}
