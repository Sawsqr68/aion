//! gRPC half of the worker dead-man switch: the transport liveness ping and
//! the correlation registry its answers land in (#197).
//!
//! # The gap this closes
//!
//! Dispatch eligibility is a verdict of the liveness probe. Until this module
//! existed the probe enumerated only liminal connections
//! ([`LiminalConnectionNotifier::liveness_targets`](super::LiminalConnectionNotifier::liveness_targets)),
//! so a `WorkerDelivery::Grpc` worker could never be asked a single question:
//! its probation opened at registration, no answer could ever be banked against
//! it, and [`ConnectedWorkerRegistry::select_worker`](super::ConnectedWorkerRegistry::select_worker)
//! filtered it out for the life of the process. Registration succeeded and
//! nothing was ever dispatched.
//!
//! # Why the ping rides the task stream
//!
//! The property the probe measures is "the server can reach THIS worker's
//! dispatch path". Only a frame that travels the exact path a dispatch travels
//! can measure it — which is why the ping is queued onto the same
//! [`WorkerTaskSender`](super::registry::WorkerTaskSender) an
//! [`ActivityTask`](aion_proto::ProtoActivityTask) is queued onto, and is
//! answered by the SDK RUNTIME rather than by action code.
//!
//! The rejected alternative was channel-state-only liveness: treating an open
//! sender, or an in-flight per-activity `Heartbeat`, as proof. Both re-create
//! the exact failure this machinery was built against — run `dfd2117c`, where a
//! worker's background pump kept its lease perfectly fresh while the server had
//! been unable to push to it for fifteen minutes. A sender being open is a
//! property of this process's memory; it is not a worker answering.
//!
//! # What a full channel means
//!
//! Admission is bounded by the probe cadence rather than refused instantly. A
//! momentarily busy channel is not the same fact as a worker that has stopped
//! reading its stream, and only the second is a dispatch-path failure — so the
//! ping waits a whole cadence for a slot and reports
//! [`PingFailure::Unaskable`](super::liveness::PingFailure::Unaskable)
//! only if the channel never opened one. That is the same admission-refused
//! class the liminal push reports when its per-connection pending-push cap is
//! exhausted, and it says the same thing: a dispatch queued right now would be
//! refused for the same reason.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use aion_proto::ProtoLivenessPing;
use tokio::sync::oneshot;

use super::liveness::PingFailure;
use super::registry::{WorkerId, WorkerMessage, WorkerTaskSender};
use crate::error::ServerError;

/// One gRPC-delivered worker the probe pings on a round.
///
/// The liminal counterpart ([`LivenessTarget`](super::LivenessTarget)) carries a
/// connection pid because a liminal worker is addressed by its connection. A
/// gRPC worker has no such handle: its stream lives inside a tonic task and the
/// registration's sender is the only way to reach it, so that sender IS the
/// address.
#[derive(Clone, Debug)]
pub struct GrpcLivenessTarget {
    /// Registry identity of the worker being probed.
    pub worker_id: WorkerId,
    /// The worker's stream delivery channel — the same one dispatches use.
    pub sender: WorkerTaskSender,
}

/// One armed ping: the sequence the server is waiting to hear echoed, and the
/// channel the inbound stream handler hands the echo back on.
#[derive(Debug)]
struct ArmedPing {
    sequence: u64,
    answer: oneshot::Sender<u64>,
}

/// Correlation registry joining a pushed [`ProtoLivenessPing`] to the
/// `LivenessAnswer` frame that comes back up the worker's inbound stream.
///
/// The two halves run in different tasks and neither can see the other: the
/// probe pushes from its own timer loop, and the answer arrives inside the
/// tonic stream handler that owns the worker's inbound direction. This is the
/// only object both hold.
///
/// At most ONE ping per worker is armed at a time. The probe's rounds never
/// overlap and each waits out its own answer, so a second arm for the same
/// worker means the previous round abandoned its ping — arming replaces it, and
/// the abandoned receiver observes a closed channel rather than being left to
/// accumulate.
#[derive(Clone, Debug, Default)]
pub struct GrpcLivenessWaiters {
    inner: Arc<Mutex<HashMap<WorkerId, ArmedPing>>>,
}

impl GrpcLivenessWaiters {
    /// Build an empty correlation registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Arm a waiter for `sequence` on `worker_id`, returning the receiver the
    /// probe awaits.
    ///
    /// Must be called BEFORE the ping is queued: arming afterwards leaves a
    /// window in which a fast worker's answer arrives with nobody listening,
    /// and the probe would then time out against a worker that answered.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the waiter map cannot be trusted.
    pub fn arm(
        &self,
        worker_id: WorkerId,
        sequence: u64,
    ) -> Result<oneshot::Receiver<u64>, ServerError> {
        let (answer, wait) = oneshot::channel();
        self.waiters()?
            .insert(worker_id, ArmedPing { sequence, answer });
        Ok(wait)
    }

    /// Deliver an answer frame's echoed sequence to the armed waiter.
    ///
    /// Returns whether the answer was MATCHED to an armed ping. A `false`
    /// return is one of two facts, both of which the caller logs rather than
    /// swallows:
    ///
    /// - nothing was armed — an answer that arrived after its ping's cadence
    ///   expired and the probe gave up, or after the worker was deregistered;
    /// - an answer whose sequence is not the armed one — a stale echo from an
    ///   earlier round.
    ///
    /// A stale echo deliberately does NOT consume the armed waiter. The
    /// sequence exists so that a late answer cannot be counted as a fresh one;
    /// consuming the waiter with a stale value would convert "this worker
    /// answered late" into "this worker answered wrongly" and withdraw its
    /// eligibility for the server's own timing.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the waiter map cannot be trusted.
    pub fn answer(&self, worker_id: WorkerId, sequence: u64) -> Result<bool, ServerError> {
        let mut waiters = self.waiters()?;
        let Some(armed) = waiters.get(&worker_id) else {
            return Ok(false);
        };
        if armed.sequence != sequence {
            return Ok(false);
        }
        let Some(armed) = waiters.remove(&worker_id) else {
            // Unreachable while the guard is held; handled rather than
            // unwrapped because a probe that panics stops being a dead-man
            // switch for the whole fleet.
            return Ok(false);
        };
        drop(waiters);
        // A send error means the probe already gave up and dropped its
        // receiver: the ping is genuinely unanswered-in-time, and reporting it
        // as matched would be a lie about a race we just lost.
        Ok(armed.answer.send(sequence).is_ok())
    }

    /// Drop any armed waiter for a worker: its stream ended, or its ping's
    /// cadence expired.
    ///
    /// Without this the map would grow one entry per departed worker forever on
    /// a never-dying server, and a re-registered worker id could inherit a
    /// stale sequence.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the waiter map cannot be trusted.
    pub fn disarm(&self, worker_id: WorkerId) -> Result<(), ServerError> {
        self.waiters()?.remove(&worker_id);
        Ok(())
    }

    fn waiters(&self) -> Result<MutexGuard<'_, HashMap<WorkerId, ArmedPing>>, ServerError> {
        self.inner
            .lock()
            .map_err(|_| ServerError::lock_poisoned("grpc worker liveness waiters"))
    }
}

/// Push one ping down a worker's task stream and wait for its correlated
/// answer, with both halves bounded by `cadence`.
///
/// Ordering is load-bearing: admission is reserved first (so a full channel is
/// diagnosed as `Unaskable` without a waiter ever being armed), then the waiter
/// is armed, and only then does the frame go out. Arming after the send would
/// race a fast worker's answer against an unarmed map.
pub(super) async fn ping_grpc_worker(
    waiters: &GrpcLivenessWaiters,
    target: &GrpcLivenessTarget,
    ping: ProtoLivenessPing,
    cadence: Duration,
) -> Result<u64, PingFailure> {
    let sequence = ping.liveness_ping;
    let permit = tokio::time::timeout(cadence, target.sender.reserve())
        .await
        .map_err(|_| {
            PingFailure::Unaskable(format!(
                "the worker's stream delivery channel stayed full for the whole {cadence:?} probe \
                 cadence; a dispatch queued now would wait behind the same backlog"
            ))
        })?
        .map_err(|error| {
            PingFailure::Unaskable(format!(
                "the worker's stream delivery channel is closed: {error}"
            ))
        })?;
    let wait = waiters.arm(target.worker_id, sequence).map_err(|error| {
        PingFailure::Unaskable(format!("could not arm the answer waiter: {error}"))
    })?;
    permit.send(WorkerMessage::LivenessPing(ping));
    let answered = tokio::time::timeout(cadence, wait).await;
    // Whatever happened, this round's waiter is spent: a later answer must find
    // nothing armed rather than satisfy a ping the probe has already judged.
    if let Err(error) = waiters.disarm(target.worker_id) {
        tracing::warn!(
            %error,
            worker_id = target.worker_id.value(),
            liveness_ping = sequence,
            "could not disarm a spent gRPC liveness waiter; a later answer may be matched \
             against a ping this round already judged"
        );
    }
    match answered {
        Ok(Ok(echoed)) => Ok(echoed),
        Ok(Err(_)) => Err(PingFailure::Unanswered(String::from(
            "the answer channel closed before the worker replied",
        ))),
        Err(_) => Err(PingFailure::Unanswered(format!(
            "no answer arrived within the {cadence:?} probe cadence"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use aion_proto::ProtoLivenessPing;

    use super::{GrpcLivenessTarget, GrpcLivenessWaiters, ping_grpc_worker};
    use crate::worker::registry::{WorkerId, WorkerMessage};

    /// Tests propagate with `?` rather than unwrapping: a poisoned waiter map is
    /// a real failure mode of the code under test and must surface as the typed
    /// error it is, not as a panic message the test wrote.
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    const CADENCE: Duration = Duration::from_millis(200);

    fn ping(sequence: u64) -> ProtoLivenessPing {
        ProtoLivenessPing {
            liveness_ping: sequence,
            silence_window_ms: 800,
        }
    }

    /// The happy path through the real correlation seam: the ping lands on the
    /// worker's stream channel and the echoed sequence comes back.
    #[tokio::test]
    async fn an_echoed_sequence_answers_the_armed_ping() -> TestResult {
        let waiters = GrpcLivenessWaiters::new();
        let (sender, mut stream) = tokio::sync::mpsc::channel(4);
        let target = GrpcLivenessTarget {
            worker_id: WorkerId::from_value(1),
            sender,
        };

        let answering = {
            let waiters = waiters.clone();
            tokio::spawn(async move {
                let Some(WorkerMessage::LivenessPing(received)) = stream.recv().await else {
                    return Ok(false);
                };
                waiters.answer(WorkerId::from_value(1), received.liveness_ping)
            })
        };

        let echoed = ping_grpc_worker(&waiters, &target, ping(9), CADENCE)
            .await
            .map_err(|failure| format!("ping failed: {failure:?}"))?;
        assert_eq!(echoed, 9, "the probe must observe the sequence it sent");
        assert!(
            answering.await??,
            "the answer must MATCH the armed ping, not merely be delivered"
        );
        Ok(())
    }

    /// A stale echo from an earlier round must not consume the armed waiter —
    /// otherwise a late answer would be reported as a WRONG answer and withdraw
    /// a healthy worker's eligibility for the server's own timing.
    #[tokio::test]
    async fn a_stale_sequence_neither_matches_nor_consumes_the_armed_waiter() -> TestResult {
        let waiters = GrpcLivenessWaiters::new();
        let worker = WorkerId::from_value(4);
        let wait = waiters.arm(worker, 12)?;

        assert!(
            !waiters.answer(worker, 11)?,
            "an answer echoing an earlier sequence is not an answer to this ping"
        );
        assert!(
            waiters.answer(worker, 12)?,
            "the armed ping must still be answerable after a stale echo was rejected"
        );
        assert_eq!(wait.await?, 12);
        Ok(())
    }

    /// An answer for a worker with nothing armed is reported unmatched rather
    /// than silently dropped: it is the caller's cue to log a frame the probe
    /// had already given up on.
    #[tokio::test]
    async fn an_answer_with_nothing_armed_is_reported_unmatched() -> TestResult {
        let waiters = GrpcLivenessWaiters::new();
        assert!(!waiters.answer(WorkerId::from_value(2), 1)?);
        Ok(())
    }

    /// A worker that never answers fails as UNANSWERED — the push was admitted,
    /// so the fact is about the worker, not about this server's ability to ask.
    #[tokio::test]
    async fn silence_after_an_admitted_push_is_unanswered_not_unaskable() -> TestResult {
        let waiters = GrpcLivenessWaiters::new();
        let (sender, _stream) = tokio::sync::mpsc::channel(4);
        let target = GrpcLivenessTarget {
            worker_id: WorkerId::from_value(3),
            sender,
        };

        let Err(failure) =
            ping_grpc_worker(&waiters, &target, ping(1), Duration::from_millis(60)).await
        else {
            return Err("a worker that never answers must not report success".into());
        };
        assert!(
            matches!(failure, super::PingFailure::Unanswered(_)),
            "an admitted push that goes unanswered is evidence about the WORKER: {failure:?}"
        );
        Ok(())
    }

    /// A stream channel that never opens a slot fails as UNASKABLE — nothing
    /// left the server, so the fact is about this server's reach, and the
    /// waiter map must be left clean (arming happens only after admission).
    #[tokio::test]
    async fn a_channel_that_never_admits_is_unaskable_and_arms_nothing() -> TestResult {
        let waiters = GrpcLivenessWaiters::new();
        let (sender, _held) = tokio::sync::mpsc::channel(1);
        sender.try_send(WorkerMessage::DrainRequest)?;
        let worker = WorkerId::from_value(5);
        let target = GrpcLivenessTarget {
            worker_id: worker,
            sender,
        };

        let Err(failure) =
            ping_grpc_worker(&waiters, &target, ping(1), Duration::from_millis(60)).await
        else {
            return Err("a channel with no free slot cannot carry a ping".into());
        };
        assert!(
            matches!(failure, super::PingFailure::Unaskable(_)),
            "a refused admission is evidence about THIS SERVER's reach: {failure:?}"
        );
        assert!(
            !waiters.answer(worker, 1)?,
            "a ping that was never sent must leave no armed waiter behind"
        );
        Ok(())
    }
}
