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

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tracing::{info, warn};

use super::heartbeat::{HeartbeatTracker, sweep_interval};
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
        for worker_id in &unreachable {
            // Announce the transition only, not the standing state, so a
            // persistently unreachable worker does not re-log every cadence.
            match self.registry.is_dispatch_ineligible(*worker_id) {
                Ok(false) => warn!(
                    worker_id = worker_id.value(),
                    task_queue = self
                        .task_queue_of(*worker_id)
                        .as_deref()
                        .unwrap_or("<unregistered>"),
                    liveness_ping = sequence,
                    silence_window_ms = self.silence_window.as_millis(),
                    "WITHDRAWING DISPATCH ELIGIBILITY from liminal worker: the server has not been \
                     able to reach its dispatch path within the window. It stays registered and \
                     keeps its in-flight work, and becomes eligible again as soon as one ping is \
                     answered"
                ),
                Ok(true) => {}
                Err(error) => warn!(
                    %error,
                    worker_id = worker_id.value(),
                    "could not read a liminal worker's current dispatch eligibility"
                ),
            }
        }
        if let Err(error) = self
            .registry
            .set_dispatch_ineligible(unreachable.into_iter().collect())
        {
            warn!(
                %error,
                liveness_ping = sequence,
                "could not publish liminal worker dispatch eligibility; selection keeps the \
                 previous verdict"
            );
        }
    }

    /// Apply one connection's ping outcome: a correct answer proves dispatch
    /// reachability, anything else is logged LOUDLY and proves nothing.
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
                 it. Its dispatch eligibility is withdrawn while this persists"
            ),
            PingFailure::Unanswered(reason) => warn!(
                connection_pid = pid,
                worker_id = worker_id.value(),
                task_queue = %task_queue,
                liveness_ping = sequence,
                reason = %reason,
                silence_window_ms = self.silence_window.as_millis(),
                "liminal worker did not answer its liveness ping; the server could not prove it \
                 can reach this worker's dispatch path, so dispatch eligibility is withdrawn once \
                 the silence outlasts the window. NOTE: the worker's connection lease may still be \
                 fresh — its liveness pump beats from a background task and keeps proving the \
                 process is alive — so do NOT expect an expiry sweep to reap it"
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
    use std::time::Duration;

    use super::{LivenessPing, LivenessPong};

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
}
