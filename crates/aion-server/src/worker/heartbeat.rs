//! Heartbeat window tracking and lost-worker failure surfacing.

use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tokio::sync::{Notify, watch};
use tracing::{error, info, warn};

use aion_core::{ActivityId, Payload, WorkflowId};
use aion_proto::{ProtoHeartbeat, WireError};

use crate::error::ServerError;
use crate::shutdown::DrainState;
use crate::worker::dispatch::{
    ActivityCompletion, ActivityCompletionOutcome, ActivityCompletionSink,
};
use crate::worker::envelope::CompletionToken;
use crate::worker::registry::{ConnectedWorkerRegistry, WorkerId};

/// In-flight activity assigned to a connected worker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InFlightActivity {
    /// Owning workflow id.
    pub workflow_id: WorkflowId,
    /// Correlating activity id.
    pub activity_id: ActivityId,
    /// One-based delivery attempt this dispatch carries — the third axis of the
    /// `(workflow, activity, attempt)` identity the transcript, the intervention
    /// index, and history all key on.
    ///
    /// It is NOT part of the tracker's key: a completion is addressed by the
    /// worker, workflow, and activity together, and widening the key would
    /// break that addressing. It is carried so a reader asking about a SPECIFIC
    /// attempt — the live describe join asking whose progress note this is —
    /// can tell a tracked entry for the attempt it asked about from a lingering
    /// entry for a superseded one, instead of attributing one attempt's note to
    /// another.
    pub attempt: u32,
    /// Generation authorized to receive a result or synthesized loss.
    pub completion_token: CompletionToken,
}

/// Observable liveness state for a single in-flight activity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskLiveness {
    /// Worker currently responsible for the task.
    pub worker_id: WorkerId,
    /// Owning workflow id.
    pub workflow_id: WorkflowId,
    /// Correlating activity id.
    pub activity_id: ActivityId,
    /// One-based delivery attempt this tracked dispatch carries (see
    /// [`InFlightActivity::attempt`]).
    pub attempt: u32,
    /// Generation authorized to receive a result or synthesized loss.
    pub completion_token: CompletionToken,
    /// Operator-configured heartbeat window used for expiry checks.
    pub heartbeat_window: Duration,
    /// Monotonic timestamp of assignment or the most recent heartbeat.
    pub last_heartbeat_at: Instant,
    /// Optional worker progress from the most recent heartbeat.
    pub last_progress: Option<Payload>,
    /// Wall-clock instant this process received the heartbeat that carried
    /// [`Self::last_progress`]. `None` while no progress has been reported.
    ///
    /// A wall clock rather than the tracked [`Instant`] because this one is
    /// REPORTED to operators, and a monotonic instant means nothing outside the
    /// process that minted it. Server-side observability stamping only — the
    /// determinism boundary governs workflow-visible time, and nothing here is
    /// workflow-visible.
    pub last_progress_at: Option<DateTime<Utc>>,
}

/// Result of accepting a heartbeat for an in-flight task.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeartbeatUpdate {
    /// Updated liveness after recording the heartbeat.
    pub liveness: TaskLiveness,
}

/// Tasks removed from tracking because a worker was declared lost.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LostWorkerReport {
    /// Lost worker removed from the connected-worker registry.
    pub worker_id: WorkerId,
    /// In-flight activities swept off the tracker: surfaced to the engine as
    /// retryable failures on the `fail_*` paths, or parked for restart
    /// recovery (nothing recorded, nothing delivered) on the graceful-drain
    /// `park_*` paths (#207).
    pub tasks: Vec<InFlightActivity>,
    /// Task queue the lost worker was serving, captured from the registry
    /// BEFORE deregistration (afterwards the handle is gone and the queue is
    /// unknowable). `None` when the worker was already absent from the registry.
    ///
    /// This is what lets the deregistration log name the queue an operator has
    /// to act on, rather than an opaque worker id.
    pub task_queue: Option<String>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct TaskKey(WorkerId, WorkflowId, ActivityId);

#[derive(Debug, Default)]
struct HeartbeatState {
    tasks: HashMap<TaskKey, TaskLiveness>,
    /// Last frame observed on each live worker connection.
    ///
    /// This is the PROCESS-IS-ALIVE fact, and only that. It is advanced by
    /// anything the worker sends — including the worker-side liveness pump,
    /// which beats from a background task regardless of what its serve loop is
    /// doing. A fresh entry here means "that process is running and its
    /// worker-to-server direction works". It does NOT mean the server can
    /// reach it.
    connections: HashMap<WorkerId, Instant>,
    /// Last time the server PROVED it can reach this worker's dispatch path,
    /// i.e. the last answered liveness ping.
    ///
    /// This is the SERVER-CAN-REACH-THE-WORKER fact, and it is the only one
    /// that is a dispatch precondition. It is advanced ONLY by an answered
    /// ping, never by an inbound frame, because only the ping rides the same
    /// server-to-worker leg a dispatch does.
    ///
    /// The two facts are separate because collapsing them hid a total outage:
    /// on run `dfd2117c` the server could not push to a worker for fifteen
    /// minutes while the worker's pump kept the single old lease perfectly
    /// fresh, so the dead-man switch could not fire for the one failure it
    /// exists to detect. `liminal_transport`'s own doc already forbids this —
    /// the ping proves "the exact path a dispatch would take, not a parallel
    /// one that could be healthy while the real one is not" — and the pump
    /// feeding the same lease was exactly that parallel channel.
    reachability: HashMap<WorkerId, Reachability>,
}

/// How many CONSECUTIVE answered pings re-admit a worker to dispatch.
///
/// A connection is a channel to prove reachability ON, never proof of it: the
/// registration handshake's ack is SENT by the server, and a sent ack is not a
/// received one — exactly the inference this whole lane exists to stop making.
/// So eligibility is earned by measurement, on every connection including the
/// first, and a redial re-seeds the measurement OPPORTUNITY rather than the
/// verdict.
///
/// Why two and not one: one success re-admits a link that answered once by luck
/// — a race, a buffer that happened to drain — so a link answering one probe in
/// three would flap in and out of eligibility indefinitely, which is the defect
/// this constant exists to remove rather than slow down. Two consecutive
/// successes is the smallest number that distinguishes "answered" from
/// "answering".
///
/// Why not three or more: the cost is paid on EVERY connect, in probe cadences.
/// At the probe's cadence a fresh worker is undispatchable for `K` cadences
/// while its first dispatches park, and that latency is charged to every honest
/// worker to catch a dishonest one. Two buys the discrimination; three buys
/// only delay.
pub(crate) const DISPATCH_PROBATION_PINGS: u32 = 2;

/// A worker's dispatch-path standing: how many consecutive pings it has
/// answered, and when the most recent one landed.
///
/// `proved_at` is `None` until the probation is served, so a worker on
/// probation is not merely stale — it has no proof at all, which is the honest
/// description of a connection nothing has been measured on yet.
#[derive(Clone, Copy, Debug, Default)]
struct Reachability {
    consecutive_answers: u32,
    proved_at: Option<Instant>,
    /// Whether this worker has EVER held dispatch eligibility on this
    /// connection. Not a duplicate of the two fields above: they describe the
    /// current standing, this describes the connection's history, and only the
    /// history separates a worker still serving its opening probation from one
    /// that earned eligibility and then lost it.
    ///
    /// Deliberately NOT cleared by [`HeartbeatTracker::record_dispatch_unreachable`]
    /// — a failed ping ends the current proof, it does not un-happen the proof
    /// that came before it. Cleared only by
    /// [`HeartbeatTracker::register_connection`], because a new connection is a
    /// new measurement and nothing earned on the old one carries across.
    ever_proved: bool,
}

impl Reachability {
    /// Whether the probation is served and the proof is still inside `window`.
    fn is_proved(self, now: Instant, window: Duration) -> bool {
        self.consecutive_answers >= DISPATCH_PROBATION_PINGS
            && self.proved_at.is_some_and(|proved_at| {
                now.checked_duration_since(proved_at)
                    .is_none_or(|elapsed| elapsed <= window)
            })
    }

    /// Why this standing does not currently permit dispatch.
    ///
    /// Only meaningful when [`Self::is_proved`] is false; the caller pairs them
    /// so the classification and the membership test can never disagree about
    /// which workers are excluded.
    fn exclusion(self) -> DispatchExclusion {
        if self.ever_proved {
            DispatchExclusion::ReachabilityLost
        } else {
            DispatchExclusion::OpeningProbation {
                answers: self.consecutive_answers,
            }
        }
    }
}

/// Why a worker is currently excluded from dispatch selection.
///
/// These are DIFFERENT FACTS and an operator must be able to tell them apart —
/// the same standard this module already holds the two ping failures to. One is
/// the ordinary cost of connecting; the other is an incident. Reported as one
/// value alongside the exclusion itself so nothing has to re-derive the reason
/// from a second reading of the same state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DispatchExclusion {
    /// The worker registered and has not yet answered
    /// [`DISPATCH_PROBATION_PINGS`] consecutive pings, so eligibility has never
    /// been earned on this connection. Expected on EVERY connect, including a
    /// perfectly healthy one — this is the probation being served, not a fault.
    OpeningProbation {
        /// Consecutive answers banked so far, out of [`DISPATCH_PROBATION_PINGS`].
        answers: u32,
    },
    /// The worker held dispatch eligibility on this connection and no longer
    /// does: either a ping failed and restarted its probation, or the last
    /// proof aged out of the heartbeat window. This one is an incident.
    ReachabilityLost,
}

/// One worker excluded from dispatch, with the reason it is excluded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExcludedWorker {
    /// The worker selection must skip.
    pub worker_id: WorkerId,
    /// Why it is being skipped.
    pub exclusion: DispatchExclusion,
}

/// Per-task liveness tracker for remote-worker streams.
///
/// It is also the server's ONLY store of worker progress notes, and that store
/// is VOLATILE: notes live here and nowhere else, so a restart loses every note
/// ever reported. [`Self::notes_held_since`] is the evidence a reader needs to
/// tell "this worker has said nothing" from "this process was not running when
/// the attempt began" — see `crate::worker::attempt_progress`.
#[derive(Clone, Debug)]
pub struct HeartbeatTracker {
    heartbeat_window: Duration,
    inner: Arc<Mutex<HeartbeatState>>,
    empty: Arc<Notify>,
    notes_held_since: DateTime<Utc>,
}

impl HeartbeatTracker {
    /// Build a tracker using the operator-supplied heartbeat window.
    #[must_use]
    pub fn new(heartbeat_window: Duration) -> Self {
        Self {
            heartbeat_window,
            inner: Arc::new(Mutex::new(HeartbeatState::default())),
            empty: Arc::new(Notify::new()),
            // The instant this volatile note store began holding notes. Read
            // here rather than injected because it is exactly the tracker's own
            // construction instant — there is no other value it could be, and a
            // caller passing a different one would be reporting a fiction.
            notes_held_since: Utc::now(),
        }
    }

    /// When this tracker began holding progress notes.
    ///
    /// An attempt dispatched before this instant left its notes in a process
    /// that no longer exists, so their absence here says nothing about what the
    /// worker reported.
    #[must_use]
    pub const fn notes_held_since(&self) -> DateTime<Utc> {
        self.notes_held_since
    }

    /// Start the connection-level lease for a newly registered worker, and open
    /// its dispatch probation.
    ///
    /// The connection lease starts fresh — the worker's process is plainly
    /// alive, it just registered. Dispatch reachability does NOT: a new
    /// connection is a channel to prove reachability on, not proof of it.
    ///
    /// This deliberately reverses an earlier reading of mine, that the
    /// registration handshake is "itself a completed server-to-worker round
    /// trip". The ack is SENT by the server; nothing reports that it was
    /// RECEIVED. Treating a send as a delivery is the same inference this lane
    /// exists to stop making, and left unfixed it meant a worker the server
    /// could never reach would re-seed itself on every redial and cycle in and
    /// out of eligibility forever instead of settling out.
    ///
    /// See [`DISPATCH_PROBATION_PINGS`].
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if tracker state cannot be trusted.
    pub fn register_connection(
        &self,
        worker_id: WorkerId,
        now: Instant,
    ) -> Result<(), ServerError> {
        let mut state = self.state()?;
        state.connections.insert(worker_id, now);
        // A fresh, UNSERVED probation: zero answers, no proof. Inserted rather
        // than left absent so the worker is carried by `unreachable_workers`
        // and is therefore explicitly excluded, not merely unknown.
        state
            .reachability
            .insert(worker_id, Reachability::default());
        Ok(())
    }

    /// Advance a worker's connection lease after receiving any frame.
    ///
    /// Records ONLY that the worker's process is alive. It deliberately does
    /// NOT advance dispatch reachability: an inbound frame — a heartbeat, a
    /// pump beat, a completion — proves the worker-to-server direction and
    /// says nothing about whether the server can push to it. Use
    /// [`Self::record_dispatch_reachability`] for the fact that gates dispatch.
    ///
    /// Returns `false` if the worker has already been removed from lease tracking;
    /// a frame racing deregistration must not resurrect it.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if tracker state cannot be trusted.
    pub fn record_connection_activity(
        &self,
        worker_id: WorkerId,
        now: Instant,
    ) -> Result<bool, ServerError> {
        let mut state = self.state()?;
        let Some(last_activity) = state.connections.get_mut(&worker_id) else {
            return Ok(false);
        };
        *last_activity = now;
        Ok(true)
    }

    /// Record proof that the server can reach this worker's dispatch path — an
    /// ANSWERED liveness ping, and nothing else.
    ///
    /// Advances both facts, because an answered ping proves both: the worker
    /// received a server push (reachability) and replied to it (alive). It also
    /// serves one ping of the dispatch probation; eligibility returns once
    /// [`DISPATCH_PROBATION_PINGS`] consecutive answers have landed.
    ///
    /// Returns `false` if the worker has already been removed from lease
    /// tracking; a pong racing a reap must not resurrect it.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if tracker state cannot be trusted.
    pub fn record_dispatch_reachability(
        &self,
        worker_id: WorkerId,
        now: Instant,
    ) -> Result<bool, ServerError> {
        let mut state = self.state()?;
        let Some(last_activity) = state.connections.get_mut(&worker_id) else {
            return Ok(false);
        };
        *last_activity = now;
        let standing = state.reachability.entry(worker_id).or_default();
        standing.consecutive_answers = standing.consecutive_answers.saturating_add(1);
        standing.proved_at = Some(now);
        if standing.consecutive_answers >= DISPATCH_PROBATION_PINGS {
            // The probation is served. Recording it here — at the one place a
            // probation can complete — is what lets a later exclusion say
            // whether eligibility was ever held, without a second copy of the
            // threshold anywhere else.
            standing.ever_proved = true;
        }
        Ok(true)
    }

    /// Record that a liveness ping went UNANSWERED: the probation restarts.
    ///
    /// This is what makes the probation consecutive rather than cumulative. A
    /// link answering one probe in three would otherwise accumulate its way to
    /// eligibility and keep it, which is the flapping this design removes.
    ///
    /// Returns `false` if the worker has already been removed from lease
    /// tracking.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if tracker state cannot be trusted.
    pub fn record_dispatch_unreachable(&self, worker_id: WorkerId) -> Result<bool, ServerError> {
        let mut state = self.state()?;
        if !state.connections.contains_key(&worker_id) {
            return Ok(false);
        }
        let standing = state.reachability.entry(worker_id).or_default();
        standing.consecutive_answers = 0;
        standing.proved_at = None;
        // `ever_proved` deliberately survives: this connection DID earn
        // eligibility once, and that is what makes the loss an incident rather
        // than the ordinary cost of connecting.
        Ok(true)
    }

    /// Whether the server has proved, within the heartbeat window, that it can
    /// reach this worker's dispatch path — probation served AND the proof still
    /// fresh.
    ///
    /// An untracked worker is not reachable: absence of proof is not proof.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if tracker state cannot be trusted.
    pub fn is_dispatch_reachable(
        &self,
        worker_id: WorkerId,
        now: Instant,
    ) -> Result<bool, ServerError> {
        let state = self.state()?;
        Ok(state
            .reachability
            .get(&worker_id)
            .is_some_and(|standing| standing.is_proved(now, self.heartbeat_window)))
    }

    /// Every tracked worker the server has NOT been able to reach within the
    /// heartbeat window, regardless of how alive its process looks, each paired
    /// with WHY it is excluded.
    ///
    /// The reason travels with the membership rather than being recomputed by
    /// the caller, so the set that gates dispatch and the reason an operator is
    /// told can never describe different states.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if tracker state cannot be trusted.
    pub fn unreachable_workers(&self, now: Instant) -> Result<Vec<ExcludedWorker>, ServerError> {
        let state = self.state()?;
        let mut workers = state
            .reachability
            .iter()
            .filter(|(_, standing)| !standing.is_proved(now, self.heartbeat_window))
            .map(|(worker_id, standing)| ExcludedWorker {
                worker_id: *worker_id,
                exclusion: standing.exclusion(),
            })
            .collect::<Vec<_>>();
        workers.sort_unstable_by_key(|excluded| excluded.worker_id);
        Ok(workers)
    }

    /// End connection-lease tracking when a transport closes normally.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if tracker state cannot be trusted.
    pub fn unregister_connection(&self, worker_id: WorkerId) -> Result<(), ServerError> {
        let mut state = self.state()?;
        state.connections.remove(&worker_id);
        state.reachability.remove(&worker_id);
        Ok(())
    }

    /// Track a newly accepted in-flight activity for heartbeat expiry.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if tracker state cannot be trusted.
    pub fn track_task(
        &self,
        worker_id: WorkerId,
        task: InFlightActivity,
        now: Instant,
    ) -> Result<(), ServerError> {
        let key = TaskKey::new(
            worker_id,
            task.workflow_id.clone(),
            task.activity_id.clone(),
        );
        let liveness = TaskLiveness {
            worker_id,
            workflow_id: task.workflow_id,
            activity_id: task.activity_id,
            attempt: task.attempt,
            completion_token: task.completion_token,
            heartbeat_window: self.heartbeat_window,
            last_heartbeat_at: now,
            last_progress: None,
            last_progress_at: None,
        };
        let mut state = self.state()?;
        state.tasks.insert(key, liveness);
        state.connections.insert(worker_id, now);
        Ok(())
    }

    /// Stop tracking a completed activity and wake drain waiters if this was the last task.
    ///
    /// Returns whether the task was still tracked when this ran: `true` means
    /// THIS call retired the in-flight entry, `false` means another path (the
    /// expiry sweep, a disconnect teardown, shutdown, or a completed dispatch)
    /// already did. The liminal reply router uses that bool as its structural
    /// gate for synthesizing a lost-worker failure — the exact mirror of the
    /// gRPC sweep failing only still-tracked tasks.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if tracker state cannot be trusted.
    pub fn complete_task(
        &self,
        worker_id: WorkerId,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
    ) -> Result<bool, ServerError> {
        let key = TaskKey::new(worker_id, workflow_id.clone(), activity_id.clone());
        let (was_tracked, became_empty) = {
            let mut state = self.state()?;
            let was_tracked = state.tasks.remove(&key).is_some();
            (was_tracked, state.tasks.is_empty())
        };
        if became_empty {
            self.empty.notify_waiters();
        }
        Ok(was_tracked)
    }

    /// Whether the given in-flight task is still tracked (not yet completed,
    /// swept, or drained). The liminal reply router polls this to bound its
    /// wait: once the entry is gone the dispatch was resolved by another path,
    /// so the router exits instead of parking on the connection forever.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if tracker state cannot be trusted.
    pub fn is_tracked(
        &self,
        worker_id: WorkerId,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
    ) -> Result<bool, ServerError> {
        let key = TaskKey::new(worker_id, workflow_id.clone(), activity_id.clone());
        Ok(self.state()?.tasks.contains_key(&key))
    }

    /// Refresh the liveness stamp of an in-flight task from a transport-level
    /// liveness beat that carries no progress payload (the liminal worker's
    /// automatic pump). Returns `true` when the task was tracked and refreshed,
    /// `false` when it is not in flight — a benign outcome for a beat racing a
    /// completion or covering an outbox dispatch the tracker never held.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if tracker state cannot be trusted.
    pub fn record_liveness(
        &self,
        worker_id: WorkerId,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
        now: Instant,
    ) -> Result<bool, ServerError> {
        let key = TaskKey::new(worker_id, workflow_id.clone(), activity_id.clone());
        let mut state = self.state()?;
        if !state.tasks.contains_key(&key) {
            return Ok(false);
        }
        if let Some(last_activity) = state.connections.get_mut(&worker_id) {
            *last_activity = now;
        }
        let Some(liveness) = state.tasks.get_mut(&key) else {
            return Ok(false);
        };
        liveness.last_heartbeat_at = now;
        Ok(true)
    }

    /// The operator-configured heartbeat window this tracker expires against.
    /// The bridge stamps it onto each liminal dispatch so the worker's
    /// automatic liveness pump beats at the matching quarter-window cadence.
    #[must_use]
    pub const fn heartbeat_window(&self) -> Duration {
        self.heartbeat_window
    }

    /// Number of currently tracked in-flight activities.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if tracker state cannot be trusted.
    pub fn in_flight_count(&self) -> Result<usize, ServerError> {
        Ok(self.state()?.tasks.len())
    }

    /// Record a worker heartbeat without completing the activity.
    ///
    /// Every heartbeat refreshes the task's liveness stamp. The progress
    /// payload is only overwritten when the heartbeat CARRIES one: the worker
    /// runtime's automatic liveness beats are payload-free and interleave
    /// with explicit handler progress heartbeats, and a liveness beat must
    /// never erase the handler's most recent progress report.
    ///
    /// # Errors
    ///
    /// Returns a stable wire error for malformed heartbeats or unknown in-flight tasks.
    pub fn record_heartbeat(
        &self,
        worker_id: WorkerId,
        heartbeat: ProtoHeartbeat,
        now: Instant,
    ) -> Result<HeartbeatUpdate, ServerError> {
        let decoded = DecodedHeartbeat::try_from(heartbeat)?;
        let key = TaskKey::new(worker_id, decoded.workflow_id, decoded.activity_id);
        let mut state = self.state()?;
        if !state.tasks.contains_key(&key) {
            return Err(wire_error("heartbeat task is not in flight"));
        }
        if let Some(last_activity) = state.connections.get_mut(&worker_id) {
            *last_activity = now;
        }
        let Some(liveness) = state.tasks.get_mut(&key) else {
            return Err(wire_error("heartbeat task is not in flight"));
        };
        liveness.last_heartbeat_at = now;
        if decoded.progress.is_some() {
            liveness.last_progress = decoded.progress;
            liveness.last_progress_at = Some(Utc::now());
        }
        Ok(HeartbeatUpdate {
            liveness: liveness.clone(),
        })
    }

    /// Return whether an in-flight task is still within its configured heartbeat window.
    ///
    /// # Errors
    ///
    /// Returns a stable wire error if the task is not tracked, or lock poison if state cannot be trusted.
    pub fn is_live(
        &self,
        worker_id: WorkerId,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
        now: Instant,
    ) -> Result<bool, ServerError> {
        let key = TaskKey::new(worker_id, workflow_id.clone(), activity_id.clone());
        let state = self.state()?;
        let Some(liveness) = state.tasks.get(&key) else {
            return Err(wire_error("heartbeat task is not in flight"));
        };
        Ok(!is_expired(liveness, now))
    }

    /// Return the workers that have at least one task beyond the configured heartbeat window.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if tracker state cannot be trusted.
    pub fn expired_workers(&self, now: Instant) -> Result<Vec<WorkerId>, ServerError> {
        let state = self.state()?;
        let mut seen = HashSet::new();
        let mut workers = Vec::new();
        for (worker_id, last_activity) in &state.connections {
            if now
                .checked_duration_since(*last_activity)
                .is_some_and(|elapsed| elapsed > self.heartbeat_window)
                && seen.insert(*worker_id)
            {
                workers.push(*worker_id);
            }
        }
        for liveness in state.tasks.values() {
            if is_expired(liveness, now) && seen.insert(liveness.worker_id) {
                workers.push(liveness.worker_id);
            }
        }
        workers.sort_unstable();
        Ok(workers)
    }

    /// Mark all currently expired workers lost and fail their in-flight tasks through the engine sink.
    ///
    /// # Errors
    ///
    /// Returns registry, tracker, or sink errors without retrying or rescheduling activities.
    pub fn fail_expired_workers(
        &self,
        registry: &ConnectedWorkerRegistry,
        sink: &impl ActivityCompletionSink,
        now: Instant,
    ) -> Result<Vec<LostWorkerReport>, ServerError> {
        let mut reports = Vec::new();
        for worker_id in self.expired_workers(now)? {
            let report = self.fail_lost_worker(worker_id, registry, sink)?;
            reports.push(report);
        }
        Ok(reports)
    }

    /// Mark a disconnected worker lost and fail its in-flight tasks through the engine sink.
    ///
    /// # Errors
    ///
    /// Returns registry, tracker, or sink errors without retrying or rescheduling activities.
    pub fn fail_disconnected_worker(
        &self,
        worker_id: WorkerId,
        registry: &ConnectedWorkerRegistry,
        sink: &impl ActivityCompletionSink,
    ) -> Result<LostWorkerReport, ServerError> {
        self.fail_lost_worker(worker_id, registry, sink)
    }

    /// Mark every currently in-flight worker lost and fail all remaining tasks through the sink.
    ///
    /// # Errors
    ///
    /// Returns registry, tracker, or sink errors without retrying or rescheduling activities.
    pub fn fail_all_in_flight_workers(
        &self,
        registry: &ConnectedWorkerRegistry,
        sink: &impl ActivityCompletionSink,
    ) -> Result<Vec<LostWorkerReport>, ServerError> {
        let worker_ids = {
            let state = self.state()?;
            let mut worker_ids = state
                .tasks
                .values()
                .map(|liveness| liveness.worker_id)
                .collect::<HashSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            worker_ids.sort_unstable();
            worker_ids
        };
        let mut reports = Vec::new();
        for worker_id in worker_ids {
            let report = self.fail_lost_worker(worker_id, registry, sink)?;
            if !report.tasks.is_empty() {
                reports.push(report);
            }
        }
        self.empty.notify_waiters();
        Ok(reports)
    }

    /// Park a drain-disconnected worker's in-flight tasks for restart recovery
    /// (#207): deregister the worker, remove its tracked tasks, and resolve
    /// each pending waiter through [`ActivityCompletionSink::park_activity`].
    ///
    /// The graceful-drain counterpart of [`Self::fail_disconnected_worker`]:
    /// same deregister-before-collect ordering (same closed dispatch/disconnect
    /// race), but NO completion is synthesized — the durable log keeps its
    /// dangling scheduled/started trail, byte-equivalent to a kill -9, and
    /// restart recovery re-dispatches it. Deregistered with the honest
    /// [`WorkerDeathReason::Disconnect`](aion_core::WorkerDeathReason::Disconnect):
    /// the transport genuinely dropped (the worker obeyed the drain request).
    ///
    /// # Errors
    ///
    /// Returns registry, tracker, or sink errors without retrying or rescheduling activities.
    pub fn park_disconnected_worker(
        &self,
        worker_id: WorkerId,
        registry: &ConnectedWorkerRegistry,
        sink: &impl ActivityCompletionSink,
    ) -> Result<LostWorkerReport, ServerError> {
        self.park_lost_worker(
            worker_id,
            registry,
            sink,
            aion_core::WorkerDeathReason::Disconnect,
        )
    }

    /// Park EVERY currently in-flight worker's tasks for restart recovery
    /// (#207) — the drain-timeout backstop's bulk counterpart of
    /// [`Self::fail_all_in_flight_workers`].
    ///
    /// Deregistered with the honest
    /// [`WorkerDeathReason::Timeout`](aion_core::WorkerDeathReason::Timeout):
    /// the drain window genuinely expired on these workers. Wakes drain waiters
    /// after the sweep so `wait_for_empty` observes the emptied tracker.
    ///
    /// # Errors
    ///
    /// Returns registry, tracker, or sink errors without retrying or rescheduling activities.
    pub fn park_all_in_flight_workers(
        &self,
        registry: &ConnectedWorkerRegistry,
        sink: &impl ActivityCompletionSink,
    ) -> Result<Vec<LostWorkerReport>, ServerError> {
        let worker_ids = {
            let state = self.state()?;
            let mut worker_ids = state
                .tasks
                .values()
                .map(|liveness| liveness.worker_id)
                .collect::<HashSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            worker_ids.sort_unstable();
            worker_ids
        };
        let mut reports = Vec::new();
        for worker_id in worker_ids {
            let report = self.park_lost_worker(
                worker_id,
                registry,
                sink,
                aion_core::WorkerDeathReason::Timeout,
            )?;
            if !report.tasks.is_empty() {
                reports.push(report);
            }
        }
        self.empty.notify_waiters();
        Ok(reports)
    }

    /// Shared park core (#207), structured exactly like [`Self::fail_lost_worker`]
    /// — deregister BEFORE collecting tasks (see that method's race note) — but
    /// resolving each waiter with the ephemeral parked sentinel instead of
    /// synthesizing a lost-worker `ActivityFailed`. Idempotent for the same
    /// reasons: `deregister_with_reason` no-ops on an already-removed worker and
    /// each task is removed as it parks, so a second sweep (park or fail) sees
    /// an empty report and resolves nothing.
    fn park_lost_worker(
        &self,
        worker_id: WorkerId,
        registry: &ConnectedWorkerRegistry,
        sink: &impl ActivityCompletionSink,
        reason: aion_core::WorkerDeathReason,
    ) -> Result<LostWorkerReport, ServerError> {
        let task_queue = task_queue_of(registry, worker_id);
        registry.deregister_with_reason(worker_id, reason)?;
        self.state()?.connections.remove(&worker_id);
        let tasks = self.remove_worker_tasks(worker_id)?;
        for task in &tasks {
            sink.park_activity(&task.workflow_id, &task.activity_id)?;
            info!(
                worker_id = ?worker_id,
                workflow_id = %task.workflow_id,
                activity_id = %task.activity_id,
                "activity parked for restart recovery"
            );
        }
        Ok(LostWorkerReport {
            worker_id,
            tasks,
            task_queue,
        })
    }

    fn fail_lost_worker(
        &self,
        worker_id: WorkerId,
        registry: &ConnectedWorkerRegistry,
        sink: &impl ActivityCompletionSink,
    ) -> Result<LostWorkerReport, ServerError> {
        // Deregister BEFORE collecting tasks: the dispatch path tracks its
        // task, sends, and then checks `registry.is_registered`. With this
        // ordering, a dispatch that still sees the worker registered is
        // guaranteed its tracked task is visible to any later sweep, so the
        // unbounded completion wait always gets a lost-worker failure. (The
        // reverse order leaves a window where a task tracked between the
        // collection and the deregistration is never failed by anyone.)
        // This is the liveness-timeout sweep: the proven reason is Timeout, the
        // one finer-grained WS3 distinction this call site can honestly assert.
        // The queue is read BEFORE the deregistration below, because afterwards
        // the handle is gone and the log could no longer name it.
        let task_queue = task_queue_of(registry, worker_id);
        registry.deregister_with_reason(worker_id, aion_core::WorkerDeathReason::Timeout)?;
        self.state()?.connections.remove(&worker_id);
        let tasks = self.remove_worker_tasks(worker_id)?;
        for task in &tasks {
            sink.complete_activity(ActivityCompletion {
                workflow_id: task.workflow_id.clone(),
                activity_id: task.activity_id.clone(),
                run_id: None,
                completion_token: task.completion_token.clone(),
                // A TRANSPORT-domain loss, not an activity failure: the
                // activity never executed to a result. The sink classifies it
                // (and applies the transport's own re-dispatch budget); this
                // sweep only reports what it observed.
                outcome: ActivityCompletionOutcome::WorkerLost { worker_id },
            })?;
        }
        Ok(LostWorkerReport {
            worker_id,
            tasks,
            task_queue,
        })
    }

    fn remove_worker_tasks(
        &self,
        worker_id: WorkerId,
    ) -> Result<Vec<InFlightActivity>, ServerError> {
        let mut state = self.state()?;
        let keys = state
            .tasks
            .keys()
            .filter(|key| key.worker_id() == worker_id)
            .cloned()
            .collect::<Vec<_>>();
        let mut tasks = Vec::with_capacity(keys.len());
        for key in keys {
            if let Some(liveness) = state.tasks.remove(&key) {
                tasks.push(InFlightActivity {
                    workflow_id: liveness.workflow_id,
                    activity_id: liveness.activity_id,
                    attempt: liveness.attempt,
                    completion_token: liveness.completion_token,
                });
            }
        }
        Ok(tasks)
    }

    /// Every tracked in-flight entry for `(workflow, activity, attempt)`.
    ///
    /// Read from the SAME map the heartbeat path writes, so what a reader is
    /// told and what the dispatch path knows cannot drift. Usually zero or one
    /// entry; a within-attempt failover can briefly have a dying owner and its
    /// adopter both tracked, which is why this returns them all and leaves the
    /// choice to the caller (`crate::worker::attempt_progress`) rather than
    /// silently picking one here.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if tracker state cannot be trusted.
    pub(in crate::worker) fn attempt_entries(
        &self,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
        attempt: u32,
    ) -> Result<Vec<TaskLiveness>, ServerError> {
        Ok(self
            .state()?
            .tasks
            .values()
            .filter(|liveness| {
                &liveness.workflow_id == workflow_id
                    && &liveness.activity_id == activity_id
                    && liveness.attempt == attempt
            })
            .cloned()
            .collect())
    }

    fn state(&self) -> Result<MutexGuard<'_, HeartbeatState>, ServerError> {
        self.inner
            .lock()
            .map_err(|_| ServerError::lock_poisoned("worker heartbeat tracker"))
    }
}

/// Sweep cadence derived from the operator's `worker.heartbeat_window`: a
/// quarter of the window, clamped to `[1s, window]` (the default 30s window
/// sweeps every 7.5s).
///
/// Deliberately derived rather than a separate config knob: the window is the
/// operational contract ("a silent worker is dead after this long"), and the
/// sweep cadence is an implementation detail of enforcing it — a quarter-window
/// cadence bounds detection latency at `window + window/4` while keeping the
/// sweep cheap. A window shorter than one second (test configurations) sweeps
/// once per window rather than sub-second-spinning, and a zero window is
/// floored at one millisecond because `tokio::time::interval` rejects a zero
/// period.
#[must_use]
pub fn sweep_interval(heartbeat_window: Duration) -> Duration {
    /// `tokio::time::interval` panics on a zero period, so even a
    /// (misconfigured) zero window gets a positive cadence.
    const MINIMUM_PERIOD: Duration = Duration::from_millis(1);
    /// Target lower bound: sweeping more often than once a second buys no
    /// meaningful detection latency against real heartbeat windows.
    const TARGET_FLOOR: Duration = Duration::from_secs(1);
    let ceiling = heartbeat_window.max(MINIMUM_PERIOD);
    // The floor never exceeds the ceiling, so `clamp` cannot panic.
    (heartbeat_window / 4).clamp(TARGET_FLOOR.min(ceiling), ceiling)
}

/// Production driver of [`HeartbeatTracker::fail_expired_workers`] (#176).
///
/// The tracker records connection and per-task liveness, while the stream-teardown
/// sweep fails a worker whose stream ENDS. A worker whose stream stays open while
/// its process wedges is caught by the connection lease even when it is idle.
/// This interval task expires every silent connection or task, deregistering it
/// with the provable
/// [`WorkerDeathReason::Timeout`](aion_core::WorkerDeathReason::Timeout) and
/// surfacing its tasks as TRANSPORT losses through the shared completion sink
/// — the `lost:` class the engine re-dispatches attempt-neutrally, never the
/// action's retry vocabulary. It shares the server's shutdown watch, so it drains with
/// the transports (mirroring
/// [`OutboxDispatcher::run`](crate::worker::OutboxDispatcher::run)).
///
/// Double-fail safety: this sweep and the stream-teardown path
/// ([`HeartbeatTracker::fail_disconnected_worker`]) can both observe the same
/// dead worker. Both funnel into the same idempotent core —
/// `deregister_with_reason` is a no-op for an already-removed worker (no
/// duplicate WS3 delta, no metrics double-count) and the tracker removes each
/// task as it fails it — so whichever path runs second sees an empty report and
/// never double-completes an activity.
pub struct HeartbeatSweeper<S> {
    tracker: HeartbeatTracker,
    registry: ConnectedWorkerRegistry,
    sink: S,
    drain: DrainState,
    heartbeat_window: Duration,
    interval: Duration,
    /// Live unserved-queue state, read only to state the CONSEQUENCE of a
    /// deregistration in the same log line as its cause: how many dispatches are
    /// already parked on the queue the reaped worker was serving. Default-empty
    /// in wirings that have no queue service, where the count reads zero.
    queue_state: crate::worker::QueueServiceState,
}

impl<S> HeartbeatSweeper<S>
where
    S: ActivityCompletionSink + Send + Sync + 'static,
{
    /// Build a sweeper over the server's shared liveness tracker, worker
    /// registry, completion sink, and drain gate. The cadence is derived from
    /// `heartbeat_window` by [`sweep_interval`].
    #[must_use]
    pub fn new(
        tracker: HeartbeatTracker,
        registry: ConnectedWorkerRegistry,
        sink: S,
        drain: DrainState,
        heartbeat_window: Duration,
    ) -> Self {
        let interval = sweep_interval(heartbeat_window);
        Self {
            tracker,
            registry,
            sink,
            drain,
            heartbeat_window,
            interval,
            queue_state: crate::worker::QueueServiceState::default(),
        }
    }

    /// Share the live unserved-queue state so a deregistration log can state how
    /// many dispatches are already parked on the queue the reaped worker served.
    ///
    /// Without it the count reads zero — honest for a wiring with no queue
    /// service, and never a reason to withhold the deregistration itself.
    #[must_use]
    pub fn with_queue_state(mut self, queue_state: crate::worker::QueueServiceState) -> Self {
        self.queue_state = queue_state;
        self
    }

    /// Run the expiry sweep until `shutdown` flips to `true`.
    ///
    /// A tracker/registry error during a sweep is logged and retried next tick
    /// rather than tearing the task down — a transient failure must not
    /// silently stop dead-worker detection. Shutdown is observed both while
    /// waiting for the next tick and re-checked before each sweep, exactly like
    /// the outbox dispatcher's run loop.
    pub async fn run(self, mut shutdown: watch::Receiver<bool>) {
        info!(
            sweep_interval_ms = self.interval.as_millis(),
            heartbeat_window_ms = self.heartbeat_window.as_millis(),
            "worker heartbeat sweeper started"
        );
        let mut ticks = tokio::time::interval(self.interval);
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = ticks.tick() => {
                    if *shutdown.borrow() {
                        break;
                    }
                    self.sweep_once(Instant::now());
                }
                changed = shutdown.changed() => {
                    // A receive error means every sender dropped; treat that as
                    // a shutdown request rather than spinning.
                    if changed.is_err() || *shutdown.borrow() {
                        break;
                    }
                }
            }
        }
        info!("worker heartbeat sweeper stopped");
    }

    /// Fail every currently-expired worker once, logging each lost-worker
    /// report at warn (mirroring the stream-teardown sweep's logging).
    fn sweep_once(&self, now: Instant) {
        let reports = match self
            .tracker
            .fail_expired_workers(&self.registry, &self.sink, now)
        {
            Ok(reports) => reports,
            Err(sweep_error) => {
                error!(
                    error = %sweep_error,
                    "heartbeat expiry sweep failed; retrying next tick"
                );
                return;
            }
        };
        for report in &reports {
            let task_queue = report.task_queue.as_deref().unwrap_or("<unregistered>");
            // The consequence, stated with the cause: a queue whose worker just
            // died and which already holds parked dispatches is an alertable
            // condition, and the operator should not have to join two log lines
            // to see it. A read failure reports `None` rather than suppressing
            // the deregistration line.
            let parked = report
                .task_queue
                .as_deref()
                .map(|queue| self.queue_state.parked_on_queue(queue))
                .transpose()
                .unwrap_or_else(|error| {
                    error!(%error, "could not read parked-dispatch count for a reaped worker");
                    None
                })
                .unwrap_or(0);
            if report.tasks.is_empty() {
                warn!(
                    worker_id = ?report.worker_id,
                    task_queue,
                    parked_dispatches = parked,
                    heartbeat_window_ms = self.heartbeat_window.as_millis(),
                    "idle worker connection lease expired; worker deregistered"
                );
            } else {
                warn!(
                    worker_id = ?report.worker_id,
                    task_queue,
                    parked_dispatches = parked,
                    failed_tasks = report.tasks.len(),
                    heartbeat_window_ms = self.heartbeat_window.as_millis(),
                    "worker heartbeat window expired with in-flight activities; \
                     deregistered and surfaced as transport losses, to be \
                     re-dispatched attempt-neutrally"
                );
            }
        }
        if !reports.is_empty() {
            // In-flight accounting may have just reached zero; wake any drain
            // waiter so shutdown does not sit out its full timeout (mirrors
            // the stream-teardown sweep).
            self.drain.notify_activity_drained();
        }
    }
}

impl TaskKey {
    fn new(worker_id: WorkerId, workflow_id: WorkflowId, activity_id: ActivityId) -> Self {
        Self(worker_id, workflow_id, activity_id)
    }

    const fn worker_id(&self) -> WorkerId {
        self.0
    }
}

struct DecodedHeartbeat {
    workflow_id: WorkflowId,
    activity_id: ActivityId,
    progress: Option<Payload>,
}

impl TryFrom<ProtoHeartbeat> for DecodedHeartbeat {
    type Error = ServerError;

    fn try_from(value: ProtoHeartbeat) -> Result<Self, Self::Error> {
        let workflow_id = value
            .workflow_id
            .ok_or_else(|| wire_error("heartbeat workflow id is missing"))
            .and_then(|id| WorkflowId::try_from(id).map_err(ServerError::from))?;
        let activity_id = value
            .activity_id
            .ok_or_else(|| wire_error("heartbeat activity id is missing"))
            .map(ActivityId::from)?;
        let progress = value
            .progress
            .map(Payload::try_from)
            .transpose()
            .map_err(ServerError::from)?;
        Ok(Self {
            workflow_id,
            activity_id,
            progress,
        })
    }
}

/// The task queue a still-registered worker serves, for the deregistration log.
///
/// A registry read failure (poisoned lock) yields `None` rather than aborting
/// the sweep: losing the queue NAME must never stop a dead worker being reaped.
fn task_queue_of(registry: &ConnectedWorkerRegistry, worker_id: WorkerId) -> Option<String> {
    registry
        .worker_by_id(worker_id)
        .ok()
        .flatten()
        .map(|handle| handle.task_queue().to_owned())
}

fn is_expired(liveness: &TaskLiveness, now: Instant) -> bool {
    now.checked_duration_since(liveness.last_heartbeat_at)
        .is_some_and(|elapsed| elapsed > liveness.heartbeat_window)
}

fn wire_error(message: &'static str) -> ServerError {
    ServerError::Wire {
        wire: WireError::backend(message),
    }
}

#[cfg(test)]
mod reachability_tests {
    use std::time::{Duration, Instant};

    use super::{
        DISPATCH_PROBATION_PINGS, DispatchExclusion, ExcludedWorker, HeartbeatTracker, ServerError,
        WorkerId,
    };

    const WINDOW: Duration = Duration::from_secs(30);

    /// Every test returns `Result` and uses `?` rather than unwrapping: a lock
    /// fault inside the tracker is a real failure mode of the code under test,
    /// and it should surface as a failed test carrying the typed error, not as a
    /// panic message written by the test.
    type TestResult = Result<(), ServerError>;

    fn tracker_with_worker(now: Instant) -> Result<(HeartbeatTracker, WorkerId), ServerError> {
        let tracker = HeartbeatTracker::new(WINDOW);
        let worker = WorkerId::from_value(1);
        tracker.register_connection(worker, now)?;
        Ok((tracker, worker))
    }

    /// Answer the full probation, so the worker is genuinely eligible. Tests
    /// about staleness, pump beats, or withdrawal must start from a worker that
    /// HAS eligibility — otherwise they pass on a worker that never had any and
    /// prove nothing about the behaviour they name.
    fn serve_probation(
        tracker: &HeartbeatTracker,
        worker: WorkerId,
        at: Instant,
    ) -> Result<(), ServerError> {
        for _ in 0..DISPATCH_PROBATION_PINGS {
            assert!(
                tracker.record_dispatch_reachability(worker, at)?,
                "the worker must still be tracked while it serves its probation"
            );
        }
        Ok(())
    }

    /// THE REGRESSION. This is the defect that made run `dfd2117c` invisible:
    /// the worker's liveness pump beat from a background task, refreshed the one
    /// shared lease, and the dead-man switch could not fire while the server had
    /// been unable to push to that worker for fifteen minutes.
    ///
    /// An inbound frame must prove the worker is ALIVE and must NOT prove the
    /// server can REACH it.
    #[test]
    fn an_inbound_frame_cannot_prove_dispatch_reachability() -> TestResult {
        let start = Instant::now();
        let (tracker, worker) = tracker_with_worker(start)?;
        // The worker STARTS eligible, earned honestly. Without this the test
        // would pass on a worker that never had eligibility to lose, which says
        // nothing about whether a pump beat can preserve it.
        serve_probation(&tracker, worker, start)?;
        assert!(
            tracker.is_dispatch_reachable(worker, start)?,
            "precondition: the worker is eligible before the connection goes one-way"
        );

        // Well past the window, with the pump beating throughout — exactly what
        // a busy worker on a poisoned connection looks like.
        let much_later = start + WINDOW * 4;
        assert!(
            tracker.record_connection_activity(worker, much_later)?,
            "the worker is still tracked"
        );

        assert!(
            !tracker.is_dispatch_reachable(worker, much_later)?,
            "a pump beat must NOT make a worker the server cannot push to look reachable"
        );
        assert_eq!(
            tracker.unreachable_workers(much_later)?,
            vec![ExcludedWorker {
                worker_id: worker,
                // It HELD eligibility (served above) and lost it to a stale
                // proof. Classifying this as an opening probation would tell an
                // operator a poisoned connection is an ordinary worker start.
                exclusion: DispatchExclusion::ReachabilityLost,
            }],
            "the worker must be named unreachable however alive its process looks"
        );
        Ok(())
    }

    /// The control for the test above: without it, a tracker that reported
    /// EVERYTHING unreachable would satisfy that assertion and prove nothing.
    #[test]
    fn an_answered_ping_does_prove_dispatch_reachability() -> TestResult {
        let start = Instant::now();
        let (tracker, worker) = tracker_with_worker(start)?;

        let much_later = start + WINDOW * 4;
        serve_probation(&tracker, worker, much_later)?;

        assert!(
            tracker.is_dispatch_reachable(worker, much_later)?,
            "answered pings are the one thing that proves the push leg works"
        );
        assert!(
            tracker.unreachable_workers(much_later)?.is_empty(),
            "a worker answering pings is never unreachable"
        );
        Ok(())
    }

    /// Registration opens a PROBATION and grants nothing. The handshake ack is
    /// SENT by this server; nothing reports that it was RECEIVED, so a
    /// connection is a channel, not proof that the channel carries. Eligibility
    /// is earned by answered pings only.
    #[test]
    fn registration_opens_a_probation_and_does_not_grant_eligibility() -> TestResult {
        let start = Instant::now();
        let (tracker, worker) = tracker_with_worker(start)?;

        assert!(
            !tracker.is_dispatch_reachable(worker, start)?,
            "a brand-new connection has proved nothing about the push leg"
        );
        assert_eq!(
            tracker.unreachable_workers(start)?,
            vec![ExcludedWorker {
                worker_id: worker,
                // And it is carried as a PROBATION, not as a reachability
                // failure. This is the distinction that stopped an ordinary
                // worker start from being announced to the operator as an
                // unreachable dispatch path.
                exclusion: DispatchExclusion::OpeningProbation { answers: 0 },
            }],
            "a worker serving its probation is carried in the census as unreachable"
        );
        Ok(())
    }

    /// The case that produced a FALSE ALARM on every healthy worker start.
    ///
    /// One answer banked out of two: the server has demonstrably reached this
    /// worker — moments ago — and is merely waiting for the second consecutive
    /// answer. Reporting that as a reachability failure told Tom's operator log
    /// his worker's dispatch path was dead when the opposite had just been
    /// measured. The exclusion is real; the REASON is an opening probation.
    #[test]
    fn a_part_served_probation_is_a_probation_and_not_a_reachability_failure() -> TestResult {
        let start = Instant::now();
        let (tracker, worker) = tracker_with_worker(start)?;
        const {
            assert!(
                DISPATCH_PROBATION_PINGS > 1,
                "this test is only meaningful while the probation takes more than one answer"
            );
        }
        assert!(tracker.record_dispatch_reachability(worker, start)?);

        assert_eq!(
            tracker.unreachable_workers(start)?,
            vec![ExcludedWorker {
                worker_id: worker,
                exclusion: DispatchExclusion::OpeningProbation { answers: 1 },
            }],
            "a worker that has answered part of its opening probation is still excluded, but it \
             must not be described as one the server cannot reach — it answered"
        );
        Ok(())
    }

    /// The other side of the same discrimination, and the control for the test
    /// above: once eligibility has actually been HELD, losing it is an incident
    /// and must classify differently. Without this, a classifier that answered
    /// `OpeningProbation` unconditionally would satisfy the test above.
    #[test]
    fn losing_held_eligibility_is_reported_as_a_loss_not_as_a_fresh_probation() -> TestResult {
        let start = Instant::now();
        let (tracker, worker) = tracker_with_worker(start)?;
        serve_probation(&tracker, worker, start)?;
        assert!(
            tracker.is_dispatch_reachable(worker, start)?,
            "precondition: eligibility was genuinely held before it was lost"
        );

        assert!(
            tracker.record_dispatch_unreachable(worker)?,
            "the worker is still tracked when its ping fails"
        );

        assert_eq!(
            tracker.unreachable_workers(start)?,
            vec![ExcludedWorker {
                worker_id: worker,
                exclusion: DispatchExclusion::ReachabilityLost,
            }],
            "a failed ping on a worker that HAD eligibility is an incident, and must not be \
             filed as the ordinary probation every fresh connection serves"
        );
        Ok(())
    }

    /// A REDIAL is a new measurement. The previous connection's proof must not
    /// make the new connection's ordinary probation look like an incident —
    /// otherwise every reconnect of a healthy worker would raise the alarm that
    /// is supposed to mean something has gone wrong.
    #[test]
    fn a_reconnect_starts_a_fresh_probation_not_a_lost_eligibility() -> TestResult {
        let start = Instant::now();
        let (tracker, worker) = tracker_with_worker(start)?;
        serve_probation(&tracker, worker, start)?;

        tracker.unregister_connection(worker)?;
        tracker.register_connection(worker, start)?;

        assert_eq!(
            tracker.unreachable_workers(start)?,
            vec![ExcludedWorker {
                worker_id: worker,
                exclusion: DispatchExclusion::OpeningProbation { answers: 0 },
            }],
            "nothing earned on the old connection carries across to the new one"
        );
        Ok(())
    }

    /// The probation must be SERVED IN FULL. One answered ping can be luck — a
    /// link that answers one probe in three would otherwise accrue eligibility
    /// and then flap. This pins the boundary from below: K-1 answers is not
    /// enough, and the very next one is.
    #[test]
    fn one_ping_short_of_the_probation_earns_nothing() -> TestResult {
        let start = Instant::now();
        let (tracker, worker) = tracker_with_worker(start)?;

        for _ in 0..DISPATCH_PROBATION_PINGS - 1 {
            assert!(tracker.record_dispatch_reachability(worker, start)?);
            assert!(
                !tracker.is_dispatch_reachable(worker, start)?,
                "eligibility must not be granted before the probation is served in full"
            );
        }

        assert!(tracker.record_dispatch_reachability(worker, start)?);
        assert!(
            tracker.is_dispatch_reachable(worker, start)?,
            "the ping that completes the probation must grant eligibility — otherwise this test \
             would pass on a tracker that never grants it at all"
        );
        Ok(())
    }

    /// A failed probe RESETS the run. Eligibility is withdrawn immediately, not
    /// when the window later expires: an unanswered probe is direct evidence
    /// about the push leg, and direct negative evidence must weigh at least as
    /// much as silence.
    #[test]
    fn a_failed_probe_withdraws_eligibility_at_once_and_restarts_the_probation() -> TestResult {
        let start = Instant::now();
        let (tracker, worker) = tracker_with_worker(start)?;
        serve_probation(&tracker, worker, start)?;
        assert!(
            tracker.is_dispatch_reachable(worker, start)?,
            "precondition"
        );

        assert!(
            tracker.record_dispatch_unreachable(worker)?,
            "the worker is still tracked"
        );
        assert!(
            !tracker.is_dispatch_reachable(worker, start)?,
            "a failed probe withdraws eligibility on the spot, inside the window"
        );

        // And the run restarts from zero rather than resuming: one answer does
        // not restore what a full probation earned.
        assert!(tracker.record_dispatch_reachability(worker, start)?);
        assert!(
            !tracker.is_dispatch_reachable(worker, start)?,
            "a single answer after a failure must not restore eligibility"
        );
        Ok(())
    }

    /// 🔴 THE FLAPPING PIN. A link that answers every other probe must NEVER
    /// become eligible. Cumulative counting would let it accrue, and eligibility
    /// would switch on and off under a running fleet — the intermittent evidence
    /// that costs hours to attribute. Consecutiveness is what forbids it.
    #[test]
    fn a_link_that_answers_every_other_probe_never_becomes_eligible() -> TestResult {
        let start = Instant::now();
        let (tracker, worker) = tracker_with_worker(start)?;

        // Far more probes than the probation demands, alternating.
        for probe in 0..DISPATCH_PROBATION_PINGS * 10 {
            let now = start + Duration::from_millis(u64::from(probe));
            if probe % 2 == 0 {
                assert!(tracker.record_dispatch_reachability(worker, now)?);
            } else {
                assert!(tracker.record_dispatch_unreachable(worker)?);
            }
            assert!(
                !tracker.is_dispatch_reachable(worker, now)?,
                "a flapping link must never hold dispatch eligibility, at any probe (probe {probe})"
            );
        }

        // The control: the same worker, answering consecutively, DOES become
        // eligible — so this test cannot pass on a tracker that grants nothing.
        let now = start + Duration::from_secs(1);
        serve_probation(&tracker, worker, now)?;
        assert!(
            tracker.is_dispatch_reachable(worker, now)?,
            "consecutive answers must still earn eligibility"
        );
        Ok(())
    }

    /// Reachability must EXPIRE on its own clock. If it were only ever advanced
    /// and never allowed to go stale, eligibility could never be withdrawn.
    #[test]
    fn reachability_goes_stale_once_the_window_passes() -> TestResult {
        let start = Instant::now();
        let (tracker, worker) = tracker_with_worker(start)?;
        serve_probation(&tracker, worker, start)?;

        assert!(
            tracker.is_dispatch_reachable(worker, start + WINDOW)?,
            "still inside the window"
        );
        assert!(
            !tracker.is_dispatch_reachable(worker, start + WINDOW + Duration::from_millis(1))?,
            "one millisecond past the window is stale"
        );
        Ok(())
    }

    /// An untracked worker is not reachable: absence of proof is not proof. A
    /// pong racing a reap must not resurrect it either.
    #[test]
    fn an_unregistered_worker_is_never_reachable_and_cannot_be_resurrected() -> TestResult {
        let start = Instant::now();
        let (tracker, worker) = tracker_with_worker(start)?;
        tracker.unregister_connection(worker)?;

        assert!(
            !tracker.is_dispatch_reachable(worker, start)?,
            "a deregistered worker is not reachable"
        );
        assert!(
            !tracker.record_dispatch_reachability(worker, start)?,
            "a late pong must not resurrect a deregistered worker"
        );
        assert!(
            !tracker.record_dispatch_unreachable(worker)?,
            "a late probe FAILURE must not resurrect a deregistered worker either — the reset \
             path allocates an entry, so it has to refuse an untracked worker as firmly as the \
             success path does"
        );
        assert!(
            tracker.unreachable_workers(start)?.is_empty(),
            "an untracked worker is not carried in the census either"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use aion_core::ContentType;
    use aion_proto::{ProtoActivityId, ProtoPayload, ProtoWorkflowId};
    use serde_json::json;
    use uuid::Uuid;

    use crate::worker::registry::WorkerRegistration;

    use super::*;

    #[derive(Default)]
    struct RecordingSink {
        completions: Mutex<Vec<ActivityCompletion>>,
        parks: Mutex<Vec<(WorkflowId, ActivityId)>>,
    }

    impl ActivityCompletionSink for RecordingSink {
        fn complete_activity(&self, completion: ActivityCompletion) -> Result<(), ServerError> {
            self.completions
                .lock()
                .map_err(|_| ServerError::lock_poisoned("recording completion sink"))?
                .push(completion);
            Ok(())
        }

        fn park_activity(
            &self,
            workflow_id: &WorkflowId,
            activity_id: &ActivityId,
        ) -> Result<(), ServerError> {
            self.parks
                .lock()
                .map_err(|_| ServerError::lock_poisoned("recording completion sink"))?
                .push((workflow_id.clone(), activity_id.clone()));
            Ok(())
        }
    }

    fn workflow_id() -> WorkflowId {
        WorkflowId::new(Uuid::nil())
    }

    fn activity_id(position: u64) -> ActivityId {
        ActivityId::from_sequence_position(position)
    }

    fn payload(value: &serde_json::Value) -> Result<Payload, Box<dyn std::error::Error>> {
        Ok(Payload::from_json(value)?)
    }

    fn heartbeat(
        workflow_id: WorkflowId,
        activity_id: ActivityId,
        progress: Option<Payload>,
    ) -> ProtoHeartbeat {
        ProtoHeartbeat {
            workflow_id: Some(ProtoWorkflowId::from(workflow_id)),
            activity_id: Some(ProtoActivityId::from(activity_id)),
            progress: progress.map(ProtoPayload::from),
        }
    }

    fn registry_with_worker()
    -> Result<(ConnectedWorkerRegistry, WorkerRegistration, WorkerId), ServerError> {
        let registry = ConnectedWorkerRegistry::default();
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let activity_types = [String::from("charge-card")];
        let registration = registry.register("tenant-a", activity_types.iter(), tx)?;
        let worker_id = registration
            .worker_id()
            .ok_or_else(|| ServerError::lock_poisoned("test worker registration"))?;
        Ok((registry, registration, worker_id))
    }

    #[test]
    fn heartbeat_refresh_keeps_task_live_across_window() -> Result<(), Box<dyn std::error::Error>> {
        let window = Duration::from_secs(5);
        let tracker = HeartbeatTracker::new(window);
        let worker_id = WorkerIdForTest::registered()?;
        let workflow_id = workflow_id();
        let activity_id = activity_id(10);
        let start = Instant::now();

        tracker.track_task(
            worker_id,
            InFlightActivity {
                workflow_id: workflow_id.clone(),
                activity_id: activity_id.clone(),
                attempt: 1,
                completion_token: crate::worker::CompletionToken::for_test(),
            },
            start,
        )?;
        assert!(tracker.is_live(worker_id, &workflow_id, &activity_id, start + window)?);

        let progress = payload(&json!({"percent": 50}))?;
        let update = tracker.record_heartbeat(
            worker_id,
            heartbeat(
                workflow_id.clone(),
                activity_id.clone(),
                Some(progress.clone()),
            ),
            start + window,
        )?;

        assert_eq!(update.liveness.last_progress, Some(progress));
        assert!(tracker.is_live(
            worker_id,
            &workflow_id,
            &activity_id,
            start + window + window
        )?);
        assert!(tracker.expired_workers(start + window + window)?.is_empty());
        Ok(())
    }

    #[test]
    fn missed_heartbeat_deregisters_worker_and_fails_in_flight_once()
    -> Result<(), Box<dyn std::error::Error>> {
        let (registry, _registration, worker_id) = registry_with_worker()?;
        let sink = RecordingSink::default();
        let tracker = HeartbeatTracker::new(Duration::from_secs(5));
        let workflow_id = workflow_id();
        let activity_id = activity_id(11);
        let start = Instant::now();

        tracker.track_task(
            worker_id,
            InFlightActivity {
                workflow_id: workflow_id.clone(),
                activity_id: activity_id.clone(),
                attempt: 1,
                completion_token: crate::worker::CompletionToken::for_test(),
            },
            start,
        )?;

        let reports =
            tracker.fail_expired_workers(&registry, &sink, start + Duration::from_secs(6))?;
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].worker_id, worker_id);
        assert_eq!(reports[0].tasks.len(), 1);
        assert!(
            registry
                .workers_for("tenant-a", "default", "charge-card", None)?
                .is_empty()
        );

        let second = tracker.fail_disconnected_worker(worker_id, &registry, &sink)?;
        assert!(second.tasks.is_empty());
        let completions = sink
            .completions
            .lock()
            .map_err(|_| ServerError::lock_poisoned("recording completion sink"))?;
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].workflow_id, workflow_id);
        assert_eq!(completions[0].activity_id, activity_id);
        // The sweep reports a TRANSPORT-domain loss, not an activity failure:
        // the activity never executed to a result, so the sink (not this sweep)
        // classifies it and applies the transport's own re-dispatch budget.
        // Before this distinction existed the sweep synthesized a `Retryable`
        // `ActivityError` that the engine then delivered as a TERMINAL failure
        // whenever the activity carried no authored retry policy.
        match &completions[0].outcome {
            ActivityCompletionOutcome::WorkerLost { worker_id: lost } => {
                assert_eq!(*lost, worker_id);
            }
            other => {
                return Err(format!("expected a lost-worker outcome, got {other:?}").into());
            }
        }
        Ok(())
    }

    #[test]
    fn disconnected_worker_fails_each_in_flight_task_once() -> Result<(), Box<dyn std::error::Error>>
    {
        let (registry, _registration, worker_id) = registry_with_worker()?;
        let sink = RecordingSink::default();
        let tracker = HeartbeatTracker::new(Duration::from_secs(5));
        let workflow_id = workflow_id();
        let start = Instant::now();

        tracker.track_task(
            worker_id,
            InFlightActivity {
                workflow_id: workflow_id.clone(),
                activity_id: activity_id(21),
                attempt: 1,
                completion_token: crate::worker::CompletionToken::for_test(),
            },
            start,
        )?;
        tracker.track_task(
            worker_id,
            InFlightActivity {
                workflow_id,
                activity_id: activity_id(22),
                attempt: 1,
                completion_token: crate::worker::CompletionToken::for_test(),
            },
            start,
        )?;

        let report = tracker.fail_disconnected_worker(worker_id, &registry, &sink)?;
        assert_eq!(report.tasks.len(), 2);
        assert!(
            registry
                .workers_for("tenant-a", "default", "charge-card", None)?
                .is_empty()
        );

        let completions = sink
            .completions
            .lock()
            .map_err(|_| ServerError::lock_poisoned("recording completion sink"))?;
        assert_eq!(completions.len(), 2);
        assert!(completions.iter().all(|completion| matches!(
            &completion.outcome,
            ActivityCompletionOutcome::WorkerLost { .. }
        )));
        Ok(())
    }

    /// #207: parking a drain-disconnected worker removes its tasks, deregisters
    /// it, and PARKS each task through the sink — zero completions synthesized,
    /// so the durable log stays byte-equivalent to a kill -9. A second park (or
    /// a later fail sweep) finds nothing: the idempotent-deregister discipline
    /// the fail path already proves holds for parks too.
    #[test]
    fn park_disconnected_worker_parks_tasks_without_synthesizing_completions()
    -> Result<(), Box<dyn std::error::Error>> {
        let (registry, _registration, worker_id) = registry_with_worker()?;
        let sink = RecordingSink::default();
        let tracker = HeartbeatTracker::new(Duration::from_secs(5));
        let workflow_id = workflow_id();
        let start = Instant::now();
        tracker.track_task(
            worker_id,
            InFlightActivity {
                workflow_id: workflow_id.clone(),
                activity_id: activity_id(60),
                attempt: 1,
                completion_token: crate::worker::CompletionToken::for_test(),
            },
            start,
        )?;
        tracker.track_task(
            worker_id,
            InFlightActivity {
                workflow_id: workflow_id.clone(),
                activity_id: activity_id(61),
                attempt: 1,
                completion_token: crate::worker::CompletionToken::for_test(),
            },
            start,
        )?;

        let report = tracker.park_disconnected_worker(worker_id, &registry, &sink)?;
        assert_eq!(report.tasks.len(), 2);
        assert_eq!(
            tracker.in_flight_count()?,
            0,
            "parking must remove every tracked task so drain accounting reaches zero"
        );
        assert!(
            registry
                .workers_for("tenant-a", "default", "charge-card", None)?
                .is_empty(),
            "the parked worker must be deregistered from routing"
        );
        let parks = sink
            .parks
            .lock()
            .map_err(|_| ServerError::lock_poisoned("recording completion sink"))?;
        assert_eq!(parks.len(), 2, "each task must be parked exactly once");
        drop(parks);
        assert!(
            sink.completions
                .lock()
                .map_err(|_| ServerError::lock_poisoned("recording completion sink"))?
                .is_empty(),
            "parking must never synthesize an activity completion"
        );

        // Double-park and park-after-fail are no-ops: the idempotent core.
        let second = tracker.park_disconnected_worker(worker_id, &registry, &sink)?;
        assert!(second.tasks.is_empty());
        let third = tracker.fail_disconnected_worker(worker_id, &registry, &sink)?;
        assert!(third.tasks.is_empty());
        assert_eq!(
            sink.parks
                .lock()
                .map_err(|_| ServerError::lock_poisoned("recording completion sink"))?
                .len(),
            2,
            "re-sweeping a parked worker must park nothing further"
        );
        assert!(
            sink.completions
                .lock()
                .map_err(|_| ServerError::lock_poisoned("recording completion sink"))?
                .is_empty(),
            "a fail sweep after the park must fail nothing"
        );
        Ok(())
    }

    /// #207 drain-timeout backstop: the bulk park removes every worker's tasks,
    /// parks each through the sink, and wakes drain waiters — never
    /// synthesizing a completion.
    #[tokio::test]
    async fn park_all_in_flight_workers_parks_everything_and_wakes_drain_waiters()
    -> Result<(), Box<dyn std::error::Error>> {
        let (registry, _registration, worker_id) = registry_with_worker()?;
        let sink = RecordingSink::default();
        let tracker = HeartbeatTracker::new(Duration::from_secs(5));
        let workflow_id = workflow_id();
        tracker.track_task(
            worker_id,
            InFlightActivity {
                workflow_id: workflow_id.clone(),
                activity_id: activity_id(70),
                attempt: 1,
                completion_token: crate::worker::CompletionToken::for_test(),
            },
            Instant::now(),
        )?;
        // Arm a waiter on the tracker's empty notify BEFORE the bulk park.
        let notified = tracker.empty.notified();
        tokio::pin!(notified);

        let reports = tracker.park_all_in_flight_workers(&registry, &sink)?;
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].worker_id, worker_id);
        assert_eq!(reports[0].tasks.len(), 1);
        assert_eq!(tracker.in_flight_count()?, 0);
        assert_eq!(
            sink.parks
                .lock()
                .map_err(|_| ServerError::lock_poisoned("recording completion sink"))?
                .len(),
            1
        );
        assert!(
            sink.completions
                .lock()
                .map_err(|_| ServerError::lock_poisoned("recording completion sink"))?
                .is_empty(),
            "the bulk park must never synthesize a completion"
        );
        tokio::time::timeout(Duration::from_millis(200), notified)
            .await
            .map_err(|_| "the bulk park must wake drain waiters")?;
        Ok(())
    }

    /// The worker runtime's AUTOMATIC liveness beats carry no payload and
    /// interleave with explicit handler progress heartbeats: a payload-free
    /// beat must refresh the liveness stamp WITHOUT erasing the handler's
    /// most recent progress report.
    #[test]
    fn payload_free_heartbeat_refreshes_liveness_without_clearing_progress()
    -> Result<(), Box<dyn std::error::Error>> {
        let window = Duration::from_secs(5);
        let tracker = HeartbeatTracker::new(window);
        let worker_id = WorkerIdForTest::registered()?;
        let workflow_id = workflow_id();
        let activity_id = activity_id(12);
        let start = Instant::now();

        tracker.track_task(
            worker_id,
            InFlightActivity {
                workflow_id: workflow_id.clone(),
                activity_id: activity_id.clone(),
                attempt: 1,
                completion_token: crate::worker::CompletionToken::for_test(),
            },
            start,
        )?;
        let progress = payload(&json!({"percent": 80}))?;
        tracker.record_heartbeat(
            worker_id,
            heartbeat(
                workflow_id.clone(),
                activity_id.clone(),
                Some(progress.clone()),
            ),
            start + Duration::from_secs(1),
        )?;

        // An automatic liveness beat: no payload, later timestamp.
        let update = tracker.record_heartbeat(
            worker_id,
            heartbeat(workflow_id.clone(), activity_id.clone(), None),
            start + Duration::from_secs(4),
        )?;

        assert_eq!(
            update.liveness.last_progress,
            Some(progress),
            "a payload-free liveness beat must not erase handler progress"
        );
        assert!(
            tracker.is_live(
                worker_id,
                &workflow_id,
                &activity_id,
                start + Duration::from_secs(8)
            )?,
            "the payload-free beat must still refresh the liveness stamp"
        );
        Ok(())
    }

    #[test]
    fn malformed_heartbeat_missing_ids_is_wire_error() -> Result<(), Box<dyn std::error::Error>> {
        let worker_id = WorkerIdForTest::registered()?;
        let tracker = HeartbeatTracker::new(Duration::from_secs(5));
        let missing = ProtoHeartbeat {
            workflow_id: None,
            activity_id: Some(ProtoActivityId::from(activity_id(30))),
            progress: None,
        };

        let result = tracker.record_heartbeat(worker_id, missing, Instant::now());
        assert!(matches!(result, Err(ServerError::Wire { .. })));
        Ok(())
    }

    #[test]
    fn heartbeat_progress_is_not_reported_as_activity_result()
    -> Result<(), Box<dyn std::error::Error>> {
        let sink = RecordingSink::default();
        let worker_id = WorkerIdForTest::registered()?;
        let tracker = HeartbeatTracker::new(Duration::from_secs(5));
        let workflow_id = workflow_id();
        let activity_id = activity_id(40);
        let now = Instant::now();

        tracker.track_task(
            worker_id,
            InFlightActivity {
                workflow_id: workflow_id.clone(),
                activity_id: activity_id.clone(),
                attempt: 1,
                completion_token: crate::worker::CompletionToken::for_test(),
            },
            now,
        )?;
        tracker.record_heartbeat(
            worker_id,
            heartbeat(
                workflow_id,
                activity_id,
                Some(Payload::new(
                    ContentType::Json,
                    b"{\"progress\":1}".to_vec(),
                )),
            ),
            now,
        )?;

        let completions = sink
            .completions
            .lock()
            .map_err(|_| ServerError::lock_poisoned("recording completion sink"))?;
        assert!(completions.is_empty());
        Ok(())
    }

    struct WorkerIdForTest;

    impl WorkerIdForTest {
        fn registered() -> Result<WorkerId, ServerError> {
            let (_registry, _registration, worker_id) = registry_with_worker()?;
            Ok(worker_id)
        }
    }

    /// `complete_task` reports whether THIS call retired the entry — the
    /// structural gate the liminal reply router uses to synthesize a
    /// lost-worker failure only for a dispatch nobody else resolved.
    #[test]
    fn complete_task_reports_whether_the_entry_was_tracked()
    -> Result<(), Box<dyn std::error::Error>> {
        let tracker = HeartbeatTracker::new(Duration::from_secs(5));
        let worker_id = WorkerIdForTest::registered()?;
        let workflow_id = workflow_id();
        let id = activity_id(50);
        tracker.track_task(
            worker_id,
            InFlightActivity {
                workflow_id: workflow_id.clone(),
                activity_id: id.clone(),
                attempt: 1,
                completion_token: crate::worker::CompletionToken::for_test(),
            },
            Instant::now(),
        )?;

        assert!(tracker.is_tracked(worker_id, &workflow_id, &id)?);
        assert!(
            tracker.complete_task(worker_id, &workflow_id, &id)?,
            "the first completion retires the tracked entry"
        );
        assert!(!tracker.is_tracked(worker_id, &workflow_id, &id)?);
        assert!(
            !tracker.complete_task(worker_id, &workflow_id, &id)?,
            "a second completion finds nothing to retire"
        );
        Ok(())
    }

    /// A liveness beat (the liminal worker's automatic pump) refreshes the
    /// task's expiry stamp — keeping a genuinely-running over-window activity
    /// out of the sweep — and reports an untracked task benignly.
    #[test]
    fn record_liveness_refreshes_stamp_and_ignores_untracked_tasks()
    -> Result<(), Box<dyn std::error::Error>> {
        let window = Duration::from_secs(5);
        let tracker = HeartbeatTracker::new(window);
        let worker_id = WorkerIdForTest::registered()?;
        let workflow_id = workflow_id();
        let id = activity_id(51);
        let start = Instant::now();
        tracker.track_task(
            worker_id,
            InFlightActivity {
                workflow_id: workflow_id.clone(),
                activity_id: id.clone(),
                attempt: 1,
                completion_token: crate::worker::CompletionToken::for_test(),
            },
            start,
        )?;

        // Beaten at the window edge, the task survives past the original expiry.
        assert!(tracker.record_liveness(worker_id, &workflow_id, &id, start + window)?);
        assert!(tracker.is_live(worker_id, &workflow_id, &id, start + window + window)?);
        assert!(tracker.expired_workers(start + window + window)?.is_empty());

        // An untracked beat (an outbox dispatch, or a beat racing completion)
        // is a benign false, never an error.
        assert!(!tracker.record_liveness(
            worker_id,
            &workflow_id,
            &activity_id(52),
            start + window
        )?);
        Ok(())
    }

    #[test]
    fn sweep_interval_is_quarter_window_clamped_to_one_second_and_window() {
        // The default 30s window sweeps every 7.5s (quarter-window).
        assert_eq!(
            sweep_interval(Duration::from_secs(30)),
            Duration::from_millis(7_500)
        );
        // A short window's quarter (500ms) is floored at 1s.
        assert_eq!(
            sweep_interval(Duration::from_secs(2)),
            Duration::from_secs(1)
        );
        // A very long window's quarter stays within the [1s, window] band.
        assert_eq!(
            sweep_interval(Duration::from_secs(3_600)),
            Duration::from_secs(900)
        );
        // A sub-second (test) window sweeps once per window, never spinning
        // sub-window nor waiting longer than the window itself.
        assert_eq!(
            sweep_interval(Duration::from_millis(200)),
            Duration::from_millis(200)
        );
        // A zero window is floored at the minimum positive period rather than
        // producing the zero interval `tokio::time::interval` rejects.
        assert_eq!(sweep_interval(Duration::ZERO), Duration::from_millis(1));
    }
}
