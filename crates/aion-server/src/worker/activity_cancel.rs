//! Stopping a run's in-flight activities, wherever they are executing (#233).
//!
//! Cancelling a workflow records `WorkflowCancelled`, kills the workflow's own
//! VM process, and settles its outbox rows so nothing more is dispatched. None
//! of that reaches an activity a remote worker is ALREADY executing: that is a
//! separate OS process, on a separate machine in the general case, and the only
//! thing that can interrupt it is a message down the stream its dispatch came
//! from. This module sends that message.
//!
//! **A cancel is a request, not a guarantee.** Delivering it establishes one
//! fact — that the server asked. Whether the work stops depends on the worker
//! still holding the activity and on the action being interruptible, and
//! neither is observable from here. Every outcome this module reports is
//! therefore phrased as what was asked or what stopped the asking, never as a
//! stop it did not witness. Silently assuming it stopped is the same defect one
//! layer up from the one this exists to fix.
//!
//! Routing comes from [`HeartbeatTracker::in_flight_for_workflow`] — the live
//! registry of which worker holds which activity — rather than from the durable
//! outbox row, which records no worker at all.
//!
//! # Not every in-flight activity has a worker
//!
//! An action whose contract declares a body is executed BY THIS SERVER
//! ([`super::declared_body`]): no worker holds it, no heartbeat tracks it, and
//! the tracker read above therefore cannot see it. Stopping those is
//! [`super::declared_body_cancel`]'s job, and it happens HERE, in the same call,
//! because "stop this run's in-flight work" is one question with two execution
//! paths — and a caller that had to remember the second one would eventually
//! forget it, which is the same silence #233 closed for the first.

use aion_core::{ActivityId, WorkflowId};
use aion_proto::{ProtoActivityId, ProtoCancelActivity, ProtoWorkflowId};
use tracing::{info, warn};

use crate::error::ServerError;
use crate::worker::declared_body_cancel::DeclaredCommandAttempts;
use crate::worker::heartbeat::HeartbeatTracker;
use crate::worker::intervention::AttemptKey;
use crate::worker::registry::{ConnectedWorkerRegistry, WorkerId, WorkerMessage};

/// What happened when the server tried to ask one worker to stop one activity.
///
/// Every variant names an OBSERVATION about the asking. None of them claims the
/// work stopped, because delivering a cancel does not establish that.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancelDelivery {
    /// The cancel was queued on the worker's dispatch stream. The server asked;
    /// nothing more than that is known from here.
    Requested,
    /// The worker was tracked as holding the activity but is no longer
    /// registered — it disconnected between the tracker read and the send.
    /// Its work is already orphaned by the disconnect, not by this.
    WorkerGone,
    /// The worker's transport carries no control messages, so a cancel cannot
    /// be expressed to it at all (the liminal transport, which has no drain
    /// channel either). Named rather than skipped: an operator who cancelled a
    /// run must be able to find out that the ask never left the building.
    TransportCannotCarry,
    /// The worker is registered but its dispatch channel would not accept the
    /// message — full or closed. The transport is failing, and the same channel
    /// carries this worker's dispatches.
    ChannelUnavailable,
}

impl CancelDelivery {
    /// Whether the cancel actually reached the worker's stream.
    ///
    /// Deliberately NOT named `succeeded`: a queued cancel is a delivered
    /// question, not an answered one.
    #[must_use]
    pub const fn was_requested(self) -> bool {
        matches!(self, Self::Requested)
    }
}

/// One activity the server asked a worker to stop, and how that asking went.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancelRequest {
    /// Worker that was holding the activity when the tracker was read.
    pub worker_id: WorkerId,
    /// Workflow the activity belongs to.
    pub workflow_id: WorkflowId,
    /// Activity within that workflow.
    pub activity_id: ActivityId,
    /// Delivery attempt the tracked dispatch carried, for the record.
    pub attempt: u32,
    /// What was observed when the ask was attempted.
    pub delivery: CancelDelivery,
}

/// Everything this node did to stop one run's in-flight work.
///
/// Two lists because there are two execution paths and they are stopped by
/// different means — asking a worker over its stream, and signalling a process
/// this server owns. Kept apart rather than merged into one vocabulary: a
/// worker ask is a question whose answer this node never sees, and a declared
/// body's stop is one the attempt itself witnesses, and calling both "cancel
/// requests" would lose that difference exactly where an operator needs it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InFlightCancellation {
    /// One record per activity a WORKER was tracked as holding, including the
    /// ones that could not be asked.
    pub worker_requests: Vec<CancelRequest>,
    /// The declared bodies THIS SERVER was executing, now signalled. Each named
    /// attempt has had its cancellation raised; the attempt reports back only
    /// once its process group has been proven gone.
    pub declared_attempts: Vec<AttemptKey>,
}

impl InFlightCancellation {
    /// Whether this node stopped nothing at all — the common case for a run with
    /// no activity in flight, and not a failure.
    #[must_use]
    pub fn stopped_nothing(&self) -> bool {
        self.worker_requests.is_empty() && self.declared_attempts.is_empty()
    }
}

/// Stop every in-flight activity of `workflow_id`, by whichever path is
/// executing it (#233).
///
/// Two paths, one call. Every activity a WORKER holds produces a
/// [`CancelRequest`], in the order the tracker yielded them, INCLUDING the ones
/// that could not be asked. Every declared body THIS SERVER is executing is
/// signalled directly, which reaches the command's process group rather than
/// merely a worker's inbox. An empty result on both means nothing was in flight
/// for this workflow — the common case, and not a failure.
///
/// Call this AFTER the terminal is durably recorded, never before: a cancel
/// pushed ahead of the record could stop work for a cancellation that then
/// fails to persist.
///
/// # Errors
///
/// Returns [`ServerError::LockPoisoned`] when the tracker's, the registry's, or
/// the declared-attempt registry's state cannot be trusted. A poisoned lock is
/// not survivable here: it would mean answering "nothing was in flight" from
/// state that cannot be read, and an operator would take that silence for a
/// completed cancellation.
pub fn cancel_in_flight_activities(
    tracker: &HeartbeatTracker,
    registry: &ConnectedWorkerRegistry,
    declared: &DeclaredCommandAttempts,
    workflow_id: &WorkflowId,
) -> Result<InFlightCancellation, ServerError> {
    let in_flight = tracker.in_flight_for_workflow(workflow_id)?;
    let mut worker_requests = Vec::with_capacity(in_flight.len());
    for liveness in in_flight {
        let delivery = ask_worker_to_stop(
            registry,
            liveness.worker_id,
            &liveness.workflow_id,
            &liveness.activity_id,
        )?;
        worker_requests.push(CancelRequest {
            worker_id: liveness.worker_id,
            workflow_id: liveness.workflow_id,
            activity_id: liveness.activity_id,
            attempt: liveness.attempt,
            delivery,
        });
    }
    // The server's own execution path, in the SAME call: an activity is either
    // held by a worker or run here, and a cancel that only covered the first
    // left the second running under a console reporting `Cancelled`.
    let declared_attempts = declared.cancel_workflow(workflow_id)?;
    let cancellation = InFlightCancellation {
        worker_requests,
        declared_attempts,
    };
    report(workflow_id, &cancellation);
    Ok(cancellation)
}

/// Push one cancel down one worker's dispatch stream.
fn ask_worker_to_stop(
    registry: &ConnectedWorkerRegistry,
    worker_id: WorkerId,
    workflow_id: &WorkflowId,
    activity_id: &ActivityId,
) -> Result<CancelDelivery, ServerError> {
    let Some(worker) = registry.worker_by_id(worker_id)? else {
        return Ok(CancelDelivery::WorkerGone);
    };
    let Some(sender) = worker.sender() else {
        return Ok(CancelDelivery::TransportCannotCarry);
    };
    let message = WorkerMessage::CancelActivity(ProtoCancelActivity {
        workflow_id: Some(ProtoWorkflowId {
            uuid: workflow_id.to_string(),
        }),
        activity_id: Some(ProtoActivityId {
            sequence_position: activity_id.sequence_position(),
        }),
    });
    if sender.try_send(message).is_ok() {
        Ok(CancelDelivery::Requested)
    } else {
        Ok(CancelDelivery::ChannelUnavailable)
    }
}

/// Log what was stopped, what was only asked, and what stopped an ask.
///
/// Anything that is not a plain `Requested` is logged at WARN with the worker
/// and activity named, because it means a cancelled run may still be holding a
/// machine and nothing else in the system will say so.
fn report(workflow_id: &WorkflowId, cancellation: &InFlightCancellation) {
    if cancellation.stopped_nothing() {
        return;
    }
    let requests = &cancellation.worker_requests;
    let requested = requests
        .iter()
        .filter(|request| request.delivery.was_requested())
        .count();
    info!(
        workflow_id = %workflow_id,
        in_flight = requests.len(),
        requested,
        declared_attempts = cancellation.declared_attempts.len(),
        "stopped the cancelled run's in-flight activities: asked their workers, and \
         signalled the declared bodies this server was executing"
    );
    for request in requests
        .iter()
        .filter(|request| !request.delivery.was_requested())
    {
        warn!(
            workflow_id = %workflow_id,
            worker_id = request.worker_id.value(),
            activity_id = request.activity_id.sequence_position(),
            attempt = request.attempt,
            outcome = ?request.delivery,
            "could not ask a worker to stop a cancelled run's activity; \
             the work may still be running"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CancelDelivery, CancelRequest, DeclaredCommandAttempts, cancel_in_flight_activities,
    };
    use crate::error::ServerError;
    use crate::worker::heartbeat::{HeartbeatTracker, InFlightActivity};
    use crate::worker::registry::{
        ConnectedWorkerRegistry, WorkerId, WorkerMessage, WorkerRegistration,
    };
    use aion_core::{ActivityId, WorkflowId};
    use std::time::{Duration, Instant};
    use tokio::sync::mpsc::{self, Receiver};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    const WINDOW: Duration = Duration::from_secs(5);

    /// A registered worker whose registration guard is HELD: dropping a
    /// [`WorkerRegistration`] deregisters the worker (the disconnect path), so
    /// the guard must live as long as the test wants the worker present.
    /// Returning only the id — the first draft of these tests — deregistered
    /// every worker before the test body ran, and one test then passed green
    /// for the wrong reason: its explicit deregister had already been done for
    /// it by the drop.
    struct RegisteredWorker {
        registry: ConnectedWorkerRegistry,
        registration: WorkerRegistration,
        worker_id: WorkerId,
        received: Receiver<WorkerMessage>,
    }

    fn registry_with_worker(capacity: usize) -> Result<RegisteredWorker, ServerError> {
        let registry = ConnectedWorkerRegistry::default();
        let (sender, received) = mpsc::channel(capacity);
        let activity_types = [String::from("work")];
        let registration = registry.register("tenant-a", activity_types.iter(), sender)?;
        let worker_id = registration
            .worker_id()
            .ok_or_else(|| ServerError::lock_poisoned("test worker registration"))?;
        Ok(RegisteredWorker {
            registry,
            registration,
            worker_id,
            received,
        })
    }

    fn track(
        tracker: &HeartbeatTracker,
        worker_id: WorkerId,
        workflow_id: &WorkflowId,
        position: u64,
    ) -> Result<(), ServerError> {
        tracker.track_task(
            worker_id,
            InFlightActivity {
                workflow_id: workflow_id.clone(),
                activity_id: ActivityId::from_sequence_position(position),
                attempt: 1,
                completion_token: crate::worker::CompletionToken::for_test(),
            },
            Instant::now(),
        )
    }

    /// A tracked activity produces a cancel on the worker's OWN stream — the
    /// same channel its dispatch travelled.
    ///
    /// `try_recv`, never `recv().await`: the fan-out is synchronous, so once it
    /// returns the cancel is either already queued or was never sent. A wait
    /// here could only hang forever on a missing message instead of failing.
    #[tokio::test]
    async fn a_tracked_activity_is_asked_to_stop_on_its_workers_stream() -> TestResult {
        let mut worker = registry_with_worker(4)?;
        let tracker = HeartbeatTracker::new(WINDOW);
        let workflow_id = WorkflowId::new_v4();
        track(&tracker, worker.worker_id, &workflow_id, 7)?;

        let declared = DeclaredCommandAttempts::new();
        let requests =
            cancel_in_flight_activities(&tracker, &worker.registry, &declared, &workflow_id)?
                .worker_requests;

        assert_eq!(requests.len(), 1, "one tracked activity, one ask");
        assert_eq!(requests[0].delivery, CancelDelivery::Requested);
        assert_eq!(
            requests[0].activity_id,
            ActivityId::from_sequence_position(7)
        );
        let Ok(WorkerMessage::CancelActivity(cancel)) = worker.received.try_recv() else {
            return Err("the worker's stream did not carry a cancel".into());
        };
        assert_eq!(
            cancel.workflow_id.map(|id| id.uuid),
            Some(workflow_id.to_string()),
            "the cancel must name the workflow it is cancelling"
        );
        assert_eq!(
            cancel.activity_id.map(|id| id.sequence_position),
            Some(7),
            "the cancel must name the activity it is cancelling"
        );
        Ok(())
    }

    /// A workflow with nothing in flight asks nothing — the common case, and
    /// not a failure.
    #[tokio::test]
    async fn a_workflow_with_nothing_in_flight_asks_nobody() -> TestResult {
        let worker = registry_with_worker(4)?;
        let tracker = HeartbeatTracker::new(WINDOW);

        let declared = DeclaredCommandAttempts::new();
        let requests = cancel_in_flight_activities(
            &tracker,
            &worker.registry,
            &declared,
            &WorkflowId::new_v4(),
        )?
        .worker_requests;

        assert!(
            requests.is_empty(),
            "an untracked workflow must produce no asks, not a synthesized one"
        );
        Ok(())
    }

    /// Only the named workflow's activities are asked to stop. A cancel must
    /// never reach a bystander sharing the same worker.
    #[tokio::test]
    async fn another_workflows_activity_is_never_asked_to_stop() -> TestResult {
        let mut worker = registry_with_worker(4)?;
        let tracker = HeartbeatTracker::new(WINDOW);
        let cancelled = WorkflowId::new_v4();
        let bystander = WorkflowId::new_v4();
        track(&tracker, worker.worker_id, &cancelled, 1)?;
        track(&tracker, worker.worker_id, &bystander, 1)?;

        let declared = DeclaredCommandAttempts::new();
        let requests =
            cancel_in_flight_activities(&tracker, &worker.registry, &declared, &cancelled)?
                .worker_requests;

        assert_eq!(requests.len(), 1, "only the cancelled run's activity");
        let Ok(WorkerMessage::CancelActivity(cancel)) = worker.received.try_recv() else {
            return Err("the worker's stream did not carry a cancel".into());
        };
        assert_eq!(
            cancel.workflow_id.map(|id| id.uuid),
            Some(cancelled.to_string()),
            "the bystander workflow must not be named in any cancel"
        );
        assert!(
            worker.received.try_recv().is_err(),
            "exactly one cancel; the bystander's activity was asked to stop too"
        );
        Ok(())
    }

    /// A worker that has gone since the tracker entry was written is REPORTED,
    /// not silently skipped.
    #[tokio::test]
    async fn a_departed_worker_is_reported_rather_than_skipped() -> TestResult {
        let worker = registry_with_worker(4)?;
        let tracker = HeartbeatTracker::new(WINDOW);
        let workflow_id = WorkflowId::new_v4();
        track(&tracker, worker.worker_id, &workflow_id, 1)?;
        worker.registration.deregister()?;

        let declared = DeclaredCommandAttempts::new();
        let requests: Vec<CancelRequest> =
            cancel_in_flight_activities(&tracker, &worker.registry, &declared, &workflow_id)?
                .worker_requests;

        assert_eq!(requests.len(), 1, "the entry is reported, not dropped");
        assert_eq!(
            requests[0].delivery,
            CancelDelivery::WorkerGone,
            "an unaskable worker must be named, never counted as asked"
        );
        assert!(
            !requests[0].delivery.was_requested(),
            "a departed worker was never asked"
        );
        Ok(())
    }

    /// A worker on a transport that carries no control messages is reported as
    /// exactly that — never as gone (it is still registered) and never as
    /// asked (the ask cannot be expressed to it).
    ///
    /// The supervisor is real but deliberately connected to nothing: what is
    /// under test is the registry's answer for a liminal delivery, not liminal
    /// itself.
    #[cfg(feature = "liminal-transport")]
    #[tokio::test]
    async fn a_liminal_worker_is_reported_as_transport_cannot_carry() -> TestResult {
        use crate::worker::liminal_transport::LiminalWorkerDelivery;
        use crate::worker::registry::WorkerDelivery;

        let registry = ConnectedWorkerRegistry::default();
        let supervisor = liminal_server::server::connection::ConnectionSupervisor::new()?;
        let activity_types = [String::from("work")];
        // The guard must outlive the fan-out call: dropping it deregisters.
        let registration = registry.register_delivery(
            [String::from("tenant-a")],
            String::from("default"),
            None,
            None,
            activity_types.iter(),
            WorkerDelivery::Liminal(LiminalWorkerDelivery::new(supervisor, 7)),
        )?;
        let worker_id = registration
            .worker_id()
            .ok_or_else(|| ServerError::lock_poisoned("test worker registration"))?;
        let tracker = HeartbeatTracker::new(WINDOW);
        let workflow_id = WorkflowId::new_v4();
        track(&tracker, worker_id, &workflow_id, 1)?;

        let declared = DeclaredCommandAttempts::new();
        let requests = cancel_in_flight_activities(&tracker, &registry, &declared, &workflow_id)?
            .worker_requests;

        assert_eq!(requests.len(), 1, "the entry is reported, not dropped");
        assert_eq!(
            requests[0].delivery,
            CancelDelivery::TransportCannotCarry,
            "a control-less transport must be named as such, not as gone or asked"
        );
        drop(registration);
        Ok(())
    }

    /// A closed dispatch channel is reported as unavailable, not as an ask.
    #[tokio::test]
    async fn a_closed_dispatch_channel_is_reported_as_unavailable() -> TestResult {
        let worker = registry_with_worker(1)?;
        let tracker = HeartbeatTracker::new(WINDOW);
        let workflow_id = WorkflowId::new_v4();
        track(&tracker, worker.worker_id, &workflow_id, 1)?;
        drop(worker.received);

        let declared = DeclaredCommandAttempts::new();
        let requests =
            cancel_in_flight_activities(&tracker, &worker.registry, &declared, &workflow_id)?
                .worker_requests;

        assert_eq!(
            requests[0].delivery,
            CancelDelivery::ChannelUnavailable,
            "a dead channel must not read as a delivered cancel"
        );
        Ok(())
    }
}
