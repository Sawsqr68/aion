//! Server half of the liminal connection dead-man switch: the liveness probe.
//!
//! # The gap this closes
//!
//! The server's connection lease ([`HeartbeatTracker`]) is advanced by frames a
//! worker SENDS, and an idle worker sends nothing. So on 2026-07-29 a healthy,
//! connected, idle worker's lease expired 37 seconds after its last activity —
//! "idle worker connection lease expired; worker deregistered" — the worker was
//! never told, kept believing it was connected, and the next dispatch parked
//! forever on a queue nobody was serving. The same absent mechanism cost a live
//! run 28 minutes of mutual blindness when a link died mid-activity: the server
//! pushed into a socket nobody was reading and the worker blocked on a socket
//! nobody was writing.
//!
//! # The probe
//!
//! [`LivenessProbe`] pushes a [`LivenessPing`] to every liminal-connected worker
//! on a fixed cadence and waits for the correlated [`LivenessPong`]. One
//! exchange proves both legs:
//!
//! - The PONG proves the worker is alive to the server, and its arrival advances
//!   the worker's connection lease — so an idle-but-alive connection is no
//!   longer "idle" at the lease layer and the idle expiry cannot fire on it.
//!   That is the structural death of the first failure above.
//! - The PING's arrival proves the server is alive to the worker, whose own
//!   dead-man switch (`aion-worker`'s `liminal_liveness`) declares the link dead
//!   when pings stop. That is the death of the second.
//!
//! A ping that is not answered inside the cadence is logged LOUDLY with the
//! worker's identity and queue, and withdraws the worker's DISPATCH
//! ELIGIBILITY once the silence outlasts the window.
//!
//! # Why eligibility, and not the connection lease
//!
//! This module used to claim that an unanswered ping let "a genuinely dead
//! worker's lease run down" so the expiry sweep would reap it. **That was
//! false, and run `dfd2117c` proved it**: the server could not push to a worker
//! for fifteen minutes and the worker never lost its lease, because the
//! worker-side liveness pump beats from a background task and keeps refreshing
//! it whatever the worker's serve loop is doing.
//!
//! Two different facts were collapsed into one lease:
//!
//! - **the worker process is alive** — proven by anything the worker sends,
//!   pump included, on the worker-to-server direction;
//! - **the server can reach the worker's dispatch path** — proven only by an
//!   answered ping, on the server-to-worker direction.
//!
//! Only the second is a dispatch precondition, and
//! [`LiminalWorkerDelivery::push_payload_with_deadline`] already says why the
//! ping is the only thing that can prove it: it rides "the exact path a
//! dispatch would take, not a parallel one that could be healthy while the real
//! one is not." The pump is exactly such a parallel channel, so it must not
//! feed the fact the ping exists to establish.
//!
//! So the probe advances a SEPARATE reachability clock
//! ([`HeartbeatTracker::record_dispatch_reachability`]) and publishes an
//! eligibility verdict the dispatch selector honours. The connection lease and
//! its expiry sweep are untouched and still own process liveness — there is
//! still deliberately no second reaper, and an unreachable worker is excluded
//! from dispatch rather than torn down.
//!
//! # No knobs
//!
//! Both timings are DERIVED from the operator's existing
//! `worker.heartbeat_window` — the one place they already declared what silence
//! means:
//!
//! - the ping cadence is [`sweep_interval`] of that window (a quarter of it,
//!   clamped to `[1s, window]`), the identical derivation the expiry sweeper
//!   uses, so a healthy connection is refreshed four times per window and the
//!   idle lease has no chance to expire;
//! - the window the worker is told to expect is the heartbeat window ITSELF, so
//!   both ends of the link declare death on exactly the same operator contract.
//!
//! Nothing here is separately configurable, and the worker holds no copy of the
//! window: it is carried on every ping.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tracing::{info, warn};

use super::heartbeat::{
    DISPATCH_PROBATION_PINGS, DispatchExclusion, ExcludedWorker, HeartbeatTracker, sweep_interval,
};
use super::liminal_transport::{LiminalConnectionNotifier, LiminalWorkerDelivery};
use super::registry::{ConnectedWorkerRegistry, WorkerId};

/// Wire liveness ping the server pushes on an established liminal connection.
///
/// Field-for-field mirror of `aion-worker`'s `liminal_liveness::LivenessPing`
/// (same serde field names), the same cross-crate contract the
/// dispatch/response and intervention pairs pin. `liveness_ping` is also the
/// worker's demux discriminator: no other pushed frame carries it, and a ping
/// carries none of the fields a dispatch or intervention requires.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LivenessPing {
    /// Monotonic ping sequence within this connection, echoed on the answer.
    pub liveness_ping: u64,
    /// How long the worker may hear NOTHING on this connection before it must
    /// declare the link dead — this server's `worker.heartbeat_window`, carried
    /// on the wire so the worker never holds a second copy of it.
    pub silence_window_ms: u64,
}

/// Wire answer the worker replies with, echoing the ping's sequence.
///
/// Field-for-field mirror of `aion-worker`'s `liminal_liveness::LivenessPong`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LivenessPong {
    /// The sequence of the ping being answered, echoed verbatim.
    pub liveness_pong: u64,
}

/// One connection the probe pings on a round: the connection pid, the worker it
/// registered, and the push leg to reach it.
#[derive(Clone, Debug)]
pub struct LivenessTarget {
    /// Liminal connection process id the worker is addressed on.
    pub pid: u64,
    /// Registry identity of the worker that registered on this connection.
    pub worker_id: WorkerId,
    /// Push leg used to deliver the ping and await its answer.
    pub delivery: LiminalWorkerDelivery,
}

/// The production driver of the liminal connection dead-man switch.
///
/// Shares the server's shutdown watch, so it drains with the transports exactly
/// like [`HeartbeatSweeper`](super::HeartbeatSweeper) and the outbox dispatcher.
pub struct LivenessProbe {
    notifier: Arc<LiminalConnectionNotifier>,
    tracker: HeartbeatTracker,
    registry: ConnectedWorkerRegistry,
    cadence: Duration,
    silence_window: Duration,
}

impl std::fmt::Debug for LivenessProbe {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LivenessProbe")
            .field("cadence", &self.cadence)
            .field("silence_window", &self.silence_window)
            .finish_non_exhaustive()
    }
}

impl LivenessProbe {
    /// Build a probe over the notifier that owns the liminal connections, the
    /// shared liveness tracker whose leases a pong refreshes, and the registry
    /// the WARN lines resolve a worker's queue through.
    ///
    /// Both timings derive from `heartbeat_window` (see the module docs); there
    /// is no separate configuration surface.
    #[must_use]
    pub fn new(
        notifier: Arc<LiminalConnectionNotifier>,
        tracker: HeartbeatTracker,
        registry: ConnectedWorkerRegistry,
        heartbeat_window: Duration,
    ) -> Self {
        Self {
            notifier,
            tracker,
            registry,
            cadence: sweep_interval(heartbeat_window),
            silence_window: heartbeat_window,
        }
    }

    /// The interval between probe rounds.
    #[must_use]
    pub const fn cadence(&self) -> Duration {
        self.cadence
    }

    /// The silence window this probe declares to every worker it pings.
    #[must_use]
    pub const fn silence_window(&self) -> Duration {
        self.silence_window
    }

    /// Run the probe until `shutdown` flips to `true`.
    ///
    /// Rounds never overlap: each tick's pings are awaited to completion (each
    /// bounded by the cadence) before the next round starts, and a missed tick
    /// is skipped rather than queued.
    ///
    /// That bounds the concurrent WAITS to one per connection. It was once
    /// claimed to bound the outstanding PUSHES to one as well, "so it can never
    /// crowd out real dispatches against liminal's per-connection pending-push
    /// cap." **That claim was false and the failure it denied is exactly what
    /// happened** on run `dfd2117c`: a push slot is not released by the caller
    /// giving up, only by a consumed reply, a deadline expiry, or a connection
    /// close. Awaiting a round to completion ends the wait, not the slot. So
    /// abandoning one unanswered no-deadline ping per round leaked one slot per
    /// round — 32 of them, then total refusal of every push on that connection.
    ///
    /// The bound is now real because
    /// [`LiminalWorkerDelivery::push_payload_with_deadline`] attaches the
    /// cadence as the push's own reply deadline, so an unanswered ping's slot
    /// expires and RELEASES its cap admission instead of accumulating.
    pub async fn run(self, mut shutdown: watch::Receiver<bool>) {
        info!(
            cadence_ms = self.cadence.as_millis(),
            silence_window_ms = self.silence_window.as_millis(),
            "liminal worker liveness probe started"
        );
        let mut sequence = 0_u64;
        let mut ticks = tokio::time::interval(self.cadence);
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = ticks.tick() => {
                    if *shutdown.borrow() {
                        break;
                    }
                    sequence = sequence.saturating_add(1);
                    self.probe_once(sequence).await;
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
        info!("liminal worker liveness probe stopped");
    }

    /// Ping every live liminal connection once, concurrently, and apply each
    /// answer to the worker's connection lease.
    async fn probe_once(&self, sequence: u64) {
        let targets = self.notifier.liveness_targets();
        if targets.is_empty() {
            return;
        }
        let ping = LivenessPing {
            liveness_ping: sequence,
            silence_window_ms: u64::try_from(self.silence_window.as_millis()).unwrap_or(u64::MAX),
        };
        let payload = match serde_json::to_vec(&ping) {
            Ok(payload) => payload,
            Err(error) => {
                // Structurally unreachable (two integers), but a probe that
                // cannot encode its own ping must say so rather than silently
                // stop being a dead-man switch.
                warn!(%error, "liminal liveness probe could not encode its ping; skipping round");
                return;
            }
        };
        let deadline = self.cadence;
        let answers = targets.into_iter().map(|target| {
            let payload = payload.clone();
            async move {
                let outcome = tokio::task::spawn_blocking(move || {
                    ping_one(&target.delivery, payload, deadline)
                })
                .await;
                (target.pid, target.worker_id, outcome)
            }
        });
        for (pid, worker_id, outcome) in futures::future::join_all(answers).await {
            self.apply_answer(pid, worker_id, sequence, outcome);
        }
        self.publish_reachability_verdict(sequence);
    }

    /// Publish this round's reachability verdict to the registry, so dispatch
    /// selection skips workers the server cannot reach.
    ///
    /// Runs after every round, including rounds where every ping succeeded —
    /// that is what RESTORES eligibility to a worker whose pings have started
    /// answering again. Recovery must not need a separate trigger.
    ///
    /// This is the half that makes the switch able to fire at all. A worker's
    /// connection lease is refreshed by anything it sends, including its own
    /// background liveness pump, so a worker whose dispatch path is completely
    /// dead can look perfectly alive indefinitely. Reachability is tracked
    /// separately and only an answered ping advances it, so the pump can no
    /// longer hold dispatch eligibility open against failing pings.
    ///
    /// # What this says out loud, and why it changed
    ///
    /// Until 2026-08-05 this announced exactly one transition — the withdrawal
    /// — in one sentence that was FALSE in the commonest case. Every healthy
    /// worker start logged `WITHDRAWING DISPATCH ELIGIBILITY … the server has
    /// not been able to reach its dispatch path within the window`, because
    /// registration opens an unserved probation and the first round after it
    /// always finds the probation unserved. At that instant the server had
    /// reached the worker — one answer was already banked — and the window had
    /// nothing to do with it. The remedy sentence was wrong too: it promised
    /// eligibility back after "one ping", when
    /// [`DISPATCH_PROBATION_PINGS`](super::heartbeat::DISPATCH_PROBATION_PINGS)
    /// consecutive answers are required and one was already in hand.
    ///
    /// The restoration a few seconds later was silent, so an operator saw the
    /// alarm and never the all-clear. On Tom's server that read as a broken
    /// worker and was reported to him as a caveat on a fix that was in fact
    /// working. An alarm that fires on every ordinary connect carries no
    /// information; a resolution nobody announces cannot cancel it.
    ///
    /// So the three transitions are now distinguished and all three are said:
    /// the probation opening (ordinary, INFO), the loss of eligibility that was
    /// actually held (an incident, WARN), and the recovery (INFO).
    fn publish_reachability_verdict(&self, sequence: u64) {
        let now = Instant::now();
        let unreachable = match self.tracker.unreachable_workers(now) {
            Ok(workers) => workers,
            Err(error) => {
                warn!(
                    %error,
                    liveness_ping = sequence,
                    "could not read liminal worker reachability; leaving the previous dispatch \
                     eligibility verdict in place rather than guessing"
                );
                return;
            }
        };
        let excluded_now: BTreeSet<WorkerId> = unreachable
            .iter()
            .map(|excluded| excluded.worker_id)
            .collect();
        // The previous verdict is what makes a transition a transition. If it
        // cannot be read the announcements are SKIPPED rather than guessed —
        // assuming an empty previous set would re-announce every standing
        // exclusion as though it had just happened. The verdict itself still
        // publishes below: gating dispatch is the load-bearing half, and it
        // must not be dropped because the narration failed.
        match self.registry.dispatch_ineligible() {
            Ok(previously_excluded) => {
                self.announce_transitions(
                    sequence,
                    &previously_excluded,
                    &unreachable,
                    &excluded_now,
                );
            }
            Err(error) => warn!(
                %error,
                liveness_ping = sequence,
                "could not read the published liminal dispatch eligibility set; this round's \
                 eligibility changes go UNANNOUNCED, though the verdict itself is still published"
            ),
        }
        if let Err(error) = self.registry.set_dispatch_ineligible(excluded_now) {
            warn!(
                %error,
                liveness_ping = sequence,
                "could not publish liminal worker dispatch eligibility; selection keeps the \
                 previous verdict"
            );
        }
    }

    /// Say what changed this round — and only what changed, so a persistently
    /// excluded worker does not re-log every cadence.
    fn announce_transitions(
        &self,
        sequence: u64,
        previously_excluded: &BTreeSet<WorkerId>,
        unreachable: &[ExcludedWorker],
        excluded_now: &BTreeSet<WorkerId>,
    ) {
        for excluded in unreachable {
            let Some(announcement) = transition(
                previously_excluded.contains(&excluded.worker_id),
                Some(excluded.exclusion),
            ) else {
                continue;
            };
            self.say(sequence, excluded.worker_id, announcement);
        }
        for worker_id in previously_excluded.difference(excluded_now) {
            // A worker that DEPARTED is not a worker that recovered. The
            // registry read is the discriminator: an entry no longer in it left
            // the fleet, and announcing its recovery would be a fabrication.
            let Some(task_queue) = self.task_queue_of(*worker_id) else {
                continue;
            };
            let Some(announcement) = transition(true, None) else {
                continue;
            };
            self.say_with_queue(sequence, *worker_id, &task_queue, announcement);
        }
    }

    /// Emit one announcement, resolving the worker's queue for the line.
    fn say(&self, sequence: u64, worker_id: WorkerId, announcement: Announcement) {
        let task_queue = self
            .task_queue_of(worker_id)
            .unwrap_or_else(|| "<unregistered>".to_owned());
        self.say_with_queue(sequence, worker_id, &task_queue, announcement);
    }

    /// Emit one announcement against an already-resolved queue.
    fn say_with_queue(
        &self,
        sequence: u64,
        worker_id: WorkerId,
        task_queue: &str,
        announcement: Announcement,
    ) {
        match announcement {
            Announcement::ProbationOpened { answers } => info!(
                worker_id = worker_id.value(),
                task_queue,
                liveness_ping = sequence,
                answers_banked = answers,
                answers_required = DISPATCH_PROBATION_PINGS,
                "liminal worker is SERVING ITS DISPATCH PROBATION: it has answered {answers} of \
                 {DISPATCH_PROBATION_PINGS} consecutive liveness pings since it connected, and is \
                 not selected for dispatch until the run is complete. This is the ordinary cost of \
                 connecting — every healthy worker start passes through it — not a fault, and not \
                 a statement that anything is unreachable"
            ),
            Announcement::EligibilityWithdrawn => warn!(
                worker_id = worker_id.value(),
                task_queue,
                liveness_ping = sequence,
                answers_required = DISPATCH_PROBATION_PINGS,
                silence_window_ms = self.silence_window.as_millis(),
                "WITHDRAWING DISPATCH ELIGIBILITY from liminal worker: it had PROVED its dispatch \
                 path reachable on this connection and the server can no longer prove it — either \
                 a liveness ping failed or the last proof aged out of the window. It stays \
                 registered and keeps its in-flight work, and becomes eligible again after \
                 {DISPATCH_PROBATION_PINGS} consecutive answered pings"
            ),
            Announcement::EligibilityRestored => info!(
                worker_id = worker_id.value(),
                task_queue,
                liveness_ping = sequence,
                "DISPATCH ELIGIBILITY RESTORED to liminal worker: it has answered a full run of \
                 consecutive liveness pings, so the server can again prove it reaches this \
                 worker's dispatch path. Dispatch selection includes it from now"
            ),
        }
    }

    /// Apply one connection's ping outcome: a correct answer advances dispatch
    /// reachability, anything else is logged LOUDLY, proves nothing, and RESETS
    /// the probation — proof of reachability must be a consecutive run.
    fn apply_answer(
        &self,
        pid: u64,
        worker_id: WorkerId,
        sequence: u64,
        outcome: Result<Result<LivenessPong, PingFailure>, tokio::task::JoinError>,
    ) {
        let failure = match outcome {
            Err(join_error) => {
                PingFailure::Unanswered(format!("ping task failed to run: {join_error}"))
            }
            Ok(Err(failure)) => failure,
            Ok(Ok(pong)) if pong.liveness_pong != sequence => PingFailure::Unanswered(format!(
                "worker answered with mismatched sequence {}",
                pong.liveness_pong
            )),
            Ok(Ok(_)) => {
                // The one thing that proves the server can reach this worker's
                // DISPATCH path: a push we sent came back answered. Only this
                // advances reachability — an inbound frame proves the opposite
                // direction and cannot stand in for it. A `false` return means
                // the worker was already deregistered (a pong racing a reap);
                // an answer must never resurrect it.
                if let Err(error) = self
                    .tracker
                    .record_dispatch_reachability(worker_id, Instant::now())
                {
                    warn!(
                        %error,
                        connection_pid = pid,
                        worker_id = worker_id.value(),
                        "failed to record liminal worker dispatch reachability from a liveness answer"
                    );
                }
                return;
            }
        };
        // A probation is CONSECUTIVE, so any failure resets it to zero. Without
        // this the counter would be cumulative, and a link that answers one probe
        // in three would still accrue its way to eligibility and then flap in and
        // out of it forever — which is exactly the defect the probation exists to
        // stop. Both failure classes below reset: whether we could not ask or the
        // worker did not answer, the run of answers is broken either way.
        // `Ok(false)` means the worker was already deregistered; nothing to reset.
        if let Err(error) = self.tracker.record_dispatch_unreachable(worker_id) {
            warn!(
                %error,
                connection_pid = pid,
                worker_id = worker_id.value(),
                "failed to reset liminal worker dispatch probation after a failed liveness ping; \
                 its eligibility may outlive the proof that earned it"
            );
        }
        // The two failures are DIFFERENT FACTS and the operator must be able to
        // tell them apart: one is about the worker, the other is about us.
        let task_queue = self
            .task_queue_of(worker_id)
            .unwrap_or_else(|| "<unregistered>".to_owned());
        match failure {
            PingFailure::Unaskable(reason) => warn!(
                connection_pid = pid,
                worker_id = worker_id.value(),
                task_queue = %task_queue,
                liveness_ping = sequence,
                reason = %reason,
                silence_window_ms = self.silence_window.as_millis(),
                "THE SERVER COULD NOT ASK this liminal worker for liveness — the push itself was \
                 refused, so nothing was sent and the worker has no idea it was probed. A dispatch \
                 would be refused by the same connection for the same reason. This says nothing \
                 about whether the worker is healthy; it says this server cannot currently reach \
                 it. Its dispatch eligibility is withdrawn NOW and it must answer a full run of \
                 consecutive pings to earn it back"
            ),
            PingFailure::Unanswered(reason) => warn!(
                connection_pid = pid,
                worker_id = worker_id.value(),
                task_queue = %task_queue,
                liveness_ping = sequence,
                reason = %reason,
                silence_window_ms = self.silence_window.as_millis(),
                "liminal worker did not answer its liveness ping; the server could not prove it \
                 can reach this worker's dispatch path, so its dispatch eligibility is withdrawn \
                 NOW and it must answer a full run of consecutive pings to earn it back — an \
                 unanswered probe is direct evidence about the push leg, not mere silence. NOTE: \
                 the worker's connection lease may still be fresh — its liveness pump beats from a \
                 background task and keeps proving the process is alive — so do NOT expect an \
                 expiry sweep to reap it"
            ),
        }
    }

    /// The task queue a worker is registered on, for the WARN line. `None` when
    /// the worker is no longer in the registry (already reaped).
    fn task_queue_of(&self, worker_id: WorkerId) -> Option<String> {
        self.registry
            .worker_by_id(worker_id)
            .ok()
            .flatten()
            .map(|handle| handle.task_queue().to_owned())
    }
}

/// Push one ping and block for its correlated answer, bounded by `deadline`.
///
/// Runs on a blocking thread: the liminal push/await pair is thread-based, not
/// async. Every failure is rendered as a reason string, because at this layer
/// the distinction that matters is "answered" vs "did not answer" — the typed
/// error text rides into the WARN verbatim.
/// A change in one worker's dispatch standing that the operator must be told
/// about — never the standing state itself, so a persistently excluded worker
/// does not re-log every cadence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Announcement {
    /// A freshly connected worker began serving its probation. Ordinary.
    ProbationOpened {
        /// Consecutive answers banked when the probation was announced.
        answers: u32,
    },
    /// A worker that HELD eligibility lost it. An incident.
    EligibilityWithdrawn,
    /// A worker that was excluded is dispatchable again.
    EligibilityRestored,
}

/// The whole truth table for one worker's standing between two rounds.
///
/// Pure and total on purpose: the four inputs are exhaustively enumerated in
/// the tests, which is the only way to be sure the alarming half and the
/// reassuring half are both reachable. The old code had no such function — the
/// decision was inlined and only ever produced one of the three lines, so the
/// missing two were invisible.
const fn transition(
    was_excluded: bool,
    now_excluded: Option<DispatchExclusion>,
) -> Option<Announcement> {
    match (was_excluded, now_excluded) {
        // Newly excluded. WHICH exclusion decides whether this is news.
        (false, Some(DispatchExclusion::OpeningProbation { answers })) => {
            Some(Announcement::ProbationOpened { answers })
        }
        (false, Some(DispatchExclusion::ReachabilityLost)) => {
            Some(Announcement::EligibilityWithdrawn)
        }
        // Left the exclusion set: the all-clear.
        (true, None) => Some(Announcement::EligibilityRestored),
        // No CHANGE in standing, by either route: a worker still excluded (its
        // exclusion was announced when it began, and repeating it every cadence
        // is how a log stops being read), or one that was eligible and stayed
        // eligible. Both are silence, for different reasons.
        (true, Some(_)) | (false, None) => None,
    }
}

/// Why one liveness round produced no proof of dispatch reachability.
///
/// The two cases are different facts about different parties and an operator
/// must be able to act on the difference, so they are typed rather than folded
/// into one string. Collapsing them is how the old WARN came to blame a worker
/// for a connection the server had itself exhausted.
#[derive(Debug)]
enum PingFailure {
    /// The server could not ASK. Push admission was refused, so no frame left
    /// the server and the worker was never probed. A dispatch on this
    /// connection would be refused identically. Says nothing about the worker.
    Unaskable(String),
    /// The server asked and got no usable answer. The push was admitted; the
    /// reply did not arrive in the cadence, or did not decode.
    Unanswered(String),
}

fn ping_one(
    delivery: &LiminalWorkerDelivery,
    payload: Vec<u8>,
    deadline: Duration,
) -> Result<LivenessPong, PingFailure> {
    // The deadline is attached to the PUSH, not just to the wait. Without it the
    // reply slot is reclaimed only by a consumed reply or a connection close, so
    // abandoning an unanswered ping every cadence leaks one slot per round until
    // the connection's push cap is exhausted and nothing — ping, dispatch or
    // intervention — can be pushed to that worker again.
    let awaiter = delivery
        .push_payload_with_deadline(payload, deadline)
        .map_err(|error| PingFailure::Unaskable(error.to_string()))?;
    let reply = awaiter
        .receive(deadline)
        .map_err(|error| PingFailure::Unanswered(format!("no answer arrived: {error}")))?;
    serde_json::from_slice(&reply)
        .map_err(|error| PingFailure::Unanswered(format!("answer could not be decoded: {error}")))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use super::super::heartbeat::{DISPATCH_PROBATION_PINGS, HeartbeatTracker};
    use super::super::liminal_transport::LiminalConnectionNotifier;
    use super::super::registry::{ConnectedWorkerRegistry, WorkerId};
    use super::{
        Announcement, DispatchExclusion, LivenessPing, LivenessPong, LivenessProbe, PingFailure,
        transition,
    };

    /// The WHOLE truth table, enumerated. Four inputs, and every one of them is
    /// asserted here — which is the only way to be sure both the alarming and
    /// the reassuring outcomes are reachable.
    ///
    /// The version of this logic that shipped until 2026-08-05 could emit
    /// exactly one of these lines. The other two were not wrong; they did not
    /// exist, so an operator saw the withdrawal on every healthy worker start
    /// and never saw the recovery that followed seconds later.
    #[test]
    fn every_standing_change_has_exactly_one_announcement() {
        assert_eq!(
            transition(
                false,
                Some(DispatchExclusion::OpeningProbation { answers: 1 })
            ),
            Some(Announcement::ProbationOpened { answers: 1 }),
            "a fresh connection serving its probation is ORDINARY and must not be announced as a \
             worker the server cannot reach"
        );
        assert_eq!(
            transition(false, Some(DispatchExclusion::ReachabilityLost)),
            Some(Announcement::EligibilityWithdrawn),
            "losing eligibility that was actually held is the incident the WARN exists for"
        );
        assert_eq!(
            transition(true, None),
            Some(Announcement::EligibilityRestored),
            "the all-clear must be said out loud — an alarm nobody cancels is read as ongoing"
        );
        assert_eq!(
            transition(true, Some(DispatchExclusion::ReachabilityLost)),
            None,
            "a standing exclusion must not re-log every cadence"
        );
        assert_eq!(
            transition(
                true,
                Some(DispatchExclusion::OpeningProbation { answers: 0 })
            ),
            None,
            "including while a still-excluded worker is still serving its probation"
        );
        assert_eq!(
            transition(false, None),
            None,
            "an eligible worker that stayed eligible is not news"
        );
    }

    /// The ping/pong pair round-trips with stable field names — the cross-crate
    /// wire contract with `aion-worker`'s mirror of these types. A drift here is
    /// a wire break, so the exact JSON is pinned.
    #[test]
    fn the_liveness_pair_round_trips_through_json() -> Result<(), serde_json::Error> {
        let ping = LivenessPing {
            liveness_ping: 7,
            silence_window_ms: 30_000,
        };
        let encoded = serde_json::to_string(&ping)?;
        assert_eq!(encoded, r#"{"liveness_ping":7,"silence_window_ms":30000}"#);
        assert_eq!(serde_json::from_str::<LivenessPing>(&encoded)?, ping);

        let answer = LivenessPong { liveness_pong: 7 };
        let encoded = serde_json::to_string(&answer)?;
        assert_eq!(encoded, r#"{"liveness_pong":7}"#);
        assert_eq!(serde_json::from_str::<LivenessPong>(&encoded)?, answer);
        Ok(())
    }

    /// A ping decodes as NEITHER of the other two frames that share the push
    /// channel, and neither of them decodes as a ping — the demux contract the
    /// worker's serve loop relies on.
    #[test]
    fn a_liveness_ping_is_disjoint_from_the_other_pushed_frames() -> Result<(), serde_json::Error> {
        let ping = serde_json::to_vec(&LivenessPing {
            liveness_ping: 1,
            silence_window_ms: 1_000,
        })?;
        assert!(
            serde_json::from_slice::<super::super::liminal_transport::DispatchRequest>(&ping)
                .is_err(),
            "a liveness ping must never decode as a dispatch"
        );
        assert!(
            serde_json::from_slice::<super::super::liminal_transport::InterventionRequest>(&ping)
                .is_err(),
            "a liveness ping must never decode as an intervention"
        );
        Ok(())
    }

    /// Both timings derive from the operator's heartbeat window, with no
    /// separate knob: the cadence is the sweeper's quarter-window derivation and
    /// the declared silence window is the heartbeat window itself.
    #[test]
    fn probe_timings_derive_from_the_heartbeat_window() {
        let window = Duration::from_secs(30);
        assert_eq!(super::sweep_interval(window), Duration::from_millis(7_500));
        // The declared window IS the operator's window; the cadence divides it,
        // so a healthy connection is refreshed four times per window.
        assert!(super::sweep_interval(window) * 4 <= window);
    }

    /// 🔴 THE WIRING PIN. The probation lives in the tracker, but only the probe
    /// can tell it a ping FAILED — and a probe that recorded successes and
    /// dropped failures would compile, log its WARN lines exactly as it does
    /// now, and leave the probation permanently unreset. The eligibility bug
    /// would be silently back with every test in `heartbeat` still green,
    /// because those tests drive the tracker directly and never go through this
    /// seam.
    ///
    /// Every failure class is asserted individually. The fourth arm — a
    /// `JoinError` from the ping task — converges on the same
    /// `PingFailure::Unanswered` path these two take, one line above the reset.
    #[test]
    fn every_failed_probe_class_withdraws_dispatch_eligibility() {
        let window = Duration::from_secs(30);
        let worker = WorkerId::from_value(1);
        let start = Instant::now();

        // Each class gets its own probe and its own served probation, so a class
        // cannot pass by inheriting the withdrawal an earlier class performed.
        let failures = [
            (
                "the push was refused, so nothing was even asked",
                Ok(Err(PingFailure::Unaskable("push refused".to_owned()))),
            ),
            (
                "the ping was sent and no answer came back",
                Ok(Err(PingFailure::Unanswered("no answer arrived".to_owned()))),
            ),
            (
                "an answer came back carrying the wrong sequence",
                Ok(Ok(LivenessPong { liveness_pong: 99 })),
            ),
        ];

        for (class, outcome) in failures {
            let registry = ConnectedWorkerRegistry::default();
            let tracker = HeartbeatTracker::new(window);
            let probe = LivenessProbe::new(
                Arc::new(LiminalConnectionNotifier::new(registry.clone())),
                tracker.clone(),
                registry,
                window,
            );

            assert!(
                tracker.register_connection(worker, start).is_ok(),
                "tracker registration must succeed for {class}"
            );
            for _ in 0..DISPATCH_PROBATION_PINGS {
                assert!(
                    tracker
                        .record_dispatch_reachability(worker, start)
                        .is_ok_and(|tracked| tracked),
                    "the worker serves its probation before {class}"
                );
            }
            assert!(
                tracker
                    .is_dispatch_reachable(worker, start)
                    .is_ok_and(|reachable| reachable),
                "precondition: the worker is eligible before {class}"
            );

            // Sequence 1 is the ping that was sent; the mismatched-answer case
            // deliberately answers 99.
            probe.apply_answer(7, worker, 1, outcome);

            assert!(
                tracker
                    .is_dispatch_reachable(worker, start)
                    .is_ok_and(|reachable| !reachable),
                "the probe must withdraw dispatch eligibility when {class} — otherwise the \
                 probation never resets and a one-way link keeps its eligibility forever"
            );
        }
    }

    /// The control for the pin above: the SUCCESS path through the same seam
    /// must keep eligibility. Without it, a probe that withdrew eligibility on
    /// every outcome — including healthy answers — would satisfy every
    /// assertion above and strand every worker on the fleet.
    #[test]
    fn an_answered_probe_keeps_dispatch_eligibility_through_the_same_seam() {
        let window = Duration::from_secs(30);
        let worker = WorkerId::from_value(1);
        let start = Instant::now();
        let registry = ConnectedWorkerRegistry::default();
        let tracker = HeartbeatTracker::new(window);
        let probe = LivenessProbe::new(
            Arc::new(LiminalConnectionNotifier::new(registry.clone())),
            tracker.clone(),
            registry,
            window,
        );

        assert!(tracker.register_connection(worker, start).is_ok());
        for sequence in 1..=u64::from(DISPATCH_PROBATION_PINGS) {
            probe.apply_answer(
                7,
                worker,
                sequence,
                Ok(Ok(LivenessPong {
                    liveness_pong: sequence,
                })),
            );
        }

        assert!(
            tracker
                .is_dispatch_reachable(worker, start)
                .is_ok_and(|reachable| reachable),
            "answered probes must EARN eligibility through the probe seam, not merely fail to \
             withdraw it"
        );
    }
}
