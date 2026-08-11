//! Head-of-line measurement for the transport liveness ping (#197).
//!
//! # The question this file answers
//!
//! The ping shares ONE channel with dispatch payloads. If the worker runtime
//! answered it from the same serialized path a delivery occupies, a
//! busy-but-healthy worker would read as silent — and post-clearance, silence
//! is a WARN withdrawal of a worker doing its job. So: what happens to a ping
//! that arrives while a delivery is in flight?
//!
//! Two facts are measured here against the REAL serve loop, not argued:
//!
//! 1. **A running activity does not delay the answer.** `handle_task` spawns
//!    the activity onto its own task, so the receive loop returns to its
//!    `select!` immediately and the next frame — the ping — is read and
//!    answered while the handler is still running. This is the ordinary case
//!    and it is safe.
//!
//! 2. **An exhausted concurrency budget DOES delay it.** The loop awaits a
//!    semaphore permit INSIDE its stream arm
//!    (`acquire_permit_or_shutdown`), before spawning. With every permit held,
//!    the loop parks there and reads no further frames, so a ping queued behind
//!    that task is not answered until a permit frees. A worker saturated for
//!    longer than the probe cadence therefore reads as unreachable while it is
//!    in fact working.
//!
//! The second is a REAL hazard and is named as such rather than mitigated here:
//! any fix reorders the receive loop's admission, which is a change to how work
//! is accepted, not to how liveness is measured.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use aion_core::{ActivityId, ContentType, Payload, RunId, WorkflowId};
use aion_proto::{ProtoActivityId, ProtoActivityTask, ProtoPayload, ProtoWorkflowId};
use async_trait::async_trait;
use tokio::sync::{Mutex, Notify, mpsc};

use super::{ActivityDispatcher, DispatchOutcome, ServeEnd, serve_activity_tasks};
use crate::context::ActivityContext;
use crate::error::WorkerError;
use crate::protocol::{
    ActivityTask, UnackedResultTracker, WorkerSession, WorkerSessionEvent, WorkerTaskStream,
    validate_activity_handlers,
};
use crate::{ReconnectConfig, WorkerConfig};

type TestError = Box<dyn std::error::Error>;

const ACTIVITY_TYPE: &str = "charge-card";

/// How long a test waits before concluding an answer did not arrive.
///
/// It is not a cadence and it is not tuned to one: it is simply long enough
/// that a loop which was going to answer would have, on any plausible
/// scheduler. Both the presence assertion and the absence assertion use the
/// SAME window, so neither can be satisfied by a window chosen to suit it.
const OBSERVATION: Duration = Duration::from_millis(300);

/// A session whose inbound events are pushed by the test and whose liveness
/// answers are recorded with the instant they were sent.
struct AnswerRecordingSession {
    receiver: Option<mpsc::Receiver<Result<WorkerSessionEvent, WorkerError>>>,
    answered: Arc<Mutex<Vec<u64>>>,
}

#[async_trait]
impl WorkerSession for AnswerRecordingSession {
    async fn handshake(&mut self, config: &WorkerConfig) -> Result<(), WorkerError> {
        drop(config.clone());
        Ok(())
    }

    async fn register(
        &mut self,
        activity_types: Vec<String>,
        available_handlers: &BTreeSet<String>,
    ) -> Result<(), WorkerError> {
        validate_activity_handlers(&activity_types, available_handlers)
    }

    fn receive_tasks(&mut self) -> WorkerTaskStream {
        match self.receiver.take() {
            Some(receiver) => Box::pin(tokio_stream::wrappers::ReceiverStream::new(receiver)),
            None => Box::pin(futures::stream::empty()),
        }
    }

    async fn report_result(
        &mut self,
        workflow_id: WorkflowId,
        activity_id: ActivityId,
        run_id: Option<RunId>,
        completion_token: String,
        result: Payload,
    ) -> Result<(), WorkerError> {
        drop((workflow_id, activity_id, run_id, completion_token, result));
        Ok(())
    }

    async fn report_failure(
        &mut self,
        workflow_id: WorkflowId,
        activity_id: ActivityId,
        run_id: Option<RunId>,
        completion_token: String,
        failure: aion_core::ActivityError,
    ) -> Result<(), WorkerError> {
        drop((workflow_id, activity_id, run_id, completion_token, failure));
        Ok(())
    }

    async fn send_heartbeat(
        &mut self,
        workflow_id: WorkflowId,
        activity_id: ActivityId,
        progress: Option<Payload>,
    ) -> Result<(), WorkerError> {
        drop((workflow_id, activity_id, progress));
        Ok(())
    }

    async fn answer_liveness_ping(&mut self, sequence: u64) -> Result<(), WorkerError> {
        self.answered.lock().await.push(sequence);
        Ok(())
    }
}

/// A dispatcher whose activity does not finish until it is released — the
/// in-flight delivery every test here is measured against.
struct HeldDispatcher {
    release: Arc<Notify>,
}

#[async_trait]
impl ActivityDispatcher for HeldDispatcher {
    async fn dispatch(
        &self,
        task: ActivityTask,
        context: ActivityContext,
    ) -> Result<DispatchOutcome, WorkerError> {
        drop((task, context));
        self.release.notified().await;
        Ok(DispatchOutcome::Completed {
            output: Payload::new(ContentType::Json, b"{}".to_vec()),
        })
    }

    fn activity_types(&self) -> BTreeSet<String> {
        [String::from(ACTIVITY_TYPE)].into_iter().collect()
    }
}

fn config(max_concurrency: usize) -> WorkerConfig {
    WorkerConfig::new(
        "http://127.0.0.1:50051",
        "payments",
        "worker-a",
        max_concurrency,
        ReconnectConfig::new(Duration::from_millis(5), Duration::from_millis(20), 3),
        None,
    )
}

fn task_event() -> WorkerSessionEvent {
    WorkerSessionEvent::Task(Box::new(ProtoActivityTask {
        workflow_id: Some(ProtoWorkflowId::from(WorkflowId::new_v4())),
        activity_id: Some(ProtoActivityId::from(ActivityId::from_sequence_position(1))),
        activity_type: String::from(ACTIVITY_TYPE),
        // A real input: a task that fails `ActivityTask::try_from` sets the
        // loop's pending error and BREAKS it, so the ping behind it would never
        // be read for a reason that has nothing to do with head-of-line
        // blocking — the measurement would be of a decode failure.
        input: Some(ProtoPayload::from(Payload::new(
            ContentType::Json,
            b"{}".to_vec(),
        ))),
        attempt: 1,
        labels: std::collections::HashMap::new(),
        run_id: Some(aion_proto::ProtoRunId::from(RunId::new_v4())),
        completion_token: String::from("generation-1"),
        idempotency_key: String::from("effect-key"),
    }))
}

const fn ping_event(sequence: u64) -> WorkerSessionEvent {
    WorkerSessionEvent::LivenessPing {
        sequence,
        silence_window: Duration::from_secs(4),
    }
}

/// One serve loop, driven by pushed events, with its answers observable.
struct Fixture {
    events: mpsc::Sender<Result<WorkerSessionEvent, WorkerError>>,
    answered: Arc<Mutex<Vec<u64>>>,
    release: Arc<Notify>,
    serve: tokio::task::JoinHandle<Result<ServeEnd, WorkerError>>,
}

impl Fixture {
    fn start(max_concurrency: usize) -> Self {
        let (events, receiver) = mpsc::channel(8);
        let answered = Arc::new(Mutex::new(Vec::new()));
        let release = Arc::new(Notify::new());
        let mut session = AnswerRecordingSession {
            receiver: Some(receiver),
            answered: Arc::clone(&answered),
        };
        let dispatcher = Arc::new(HeldDispatcher {
            release: Arc::clone(&release),
        });
        let config = config(max_concurrency);
        let serve = tokio::spawn(async move {
            let mut tracker = UnackedResultTracker::default();
            serve_activity_tasks(&config, &mut session, dispatcher, &mut tracker).await
        });
        Self {
            events,
            answered,
            release,
            serve,
        }
    }

    async fn answered(&self) -> Vec<u64> {
        self.answered.lock().await.clone()
    }

    /// Release every held activity and let the loop drain to a clean end.
    async fn finish(self) -> Result<(), TestError> {
        self.release.notify_waiters();
        drop(self.events);
        self.serve.await??;
        Ok(())
    }
}

/// THE ORDINARY CASE, MEASURED: a ping that arrives while an activity is
/// running IS answered, promptly, without waiting for that activity.
///
/// This is the property the whole mechanism rests on. `handle_task` spawns the
/// activity rather than awaiting it, so the receive loop is back in its
/// `select!` before the next frame arrives, and answering takes no concurrency
/// permit. A worker doing its job stays reachable.
#[tokio::test]
async fn a_ping_behind_a_running_activity_is_answered_while_it_runs() -> Result<(), TestError> {
    // Two permits: one for the held activity, one spare — the shape of any
    // worker not saturated at this instant.
    let fixture = Fixture::start(2);
    fixture.events.send(Ok(task_event())).await?;
    fixture.events.send(Ok(ping_event(7))).await?;

    tokio::time::sleep(OBSERVATION).await;
    assert_eq!(
        fixture.answered().await,
        vec![7],
        "the ping must be answered while the delivery is still in flight; a runtime that answered \
         only after the activity finished would report every busy worker as unreachable"
    );

    fixture.finish().await
}

/// 🔴 THE HAZARD, MEASURED: with every concurrency permit held, the receive
/// loop parks on permit acquisition and a ping queued behind a task is NOT
/// answered until a permit frees.
///
/// The loop awaits `acquire_permit_or_shutdown` inside its stream arm, before
/// spawning, so while it waits no further frame is read — including a ping that
/// is already on the wire. A worker saturated for longer than the probe cadence
/// therefore reads as silent, and post-clearance that is a WARN withdrawal of a
/// worker that is working.
///
/// The release half is the vacuity control and is not optional: without it,
/// this test would be satisfied by a runtime that never answers pings at all,
/// which is a different (and far worse) defect wearing the same red.
#[tokio::test]
async fn a_ping_behind_a_task_that_cannot_get_a_permit_waits_for_that_permit()
-> Result<(), TestError> {
    // One permit, already held by an activity that will not finish, and a
    // SECOND task queued behind it — the task the loop parks on.
    let fixture = Fixture::start(1);
    fixture.events.send(Ok(task_event())).await?;
    fixture.events.send(Ok(task_event())).await?;
    fixture.events.send(Ok(ping_event(9))).await?;

    tokio::time::sleep(OBSERVATION).await;
    assert!(
        fixture.answered().await.is_empty(),
        "MEASURED HAZARD: the loop is parked acquiring a permit for the queued task and has not \
         read the ping behind it. This is the head-of-line block a saturated worker suffers"
    );

    // The control: free the permits and the SAME ping is answered. That is what
    // proves the silence above was the permit and not a broken answer path.
    fixture.release.notify_waiters();
    tokio::time::sleep(OBSERVATION).await;
    assert_eq!(
        fixture.answered().await,
        vec![9],
        "once a permit frees, the queued ping is read and answered — so the silence measured \
         above was head-of-line blocking, not an unwired answer"
    );

    fixture.finish().await
}
