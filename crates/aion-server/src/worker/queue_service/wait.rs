//! The selection wait, made honest.
//!
//! Before R1 this loop was: registry miss, one `tracing::info!`, block. It
//! could not tell "served slowly" from "served by nobody", so an operator had
//! nothing to alert on and a run could sit forever with no state.
//!
//! Now every miss is classified against the deployed contract records and the
//! live poller census, published to the queue-service state, logged at WARN
//! with the reason and the last-compatible-poller age, and resolved by the
//! policy: structural unservability refuses at once, a fleet condition refuses
//! when the service-availability deadline expires, and `durable_pending` parks
//! visibly until a worker arrives.
//!
//! What did NOT change: the drain gate still runs first and still wins, and a
//! server with no configured deadline still waits — loudly, and with a state an
//! operator can read.
//!
//! That wait is unbounded only while the server is accepting work. The drain
//! gate here is consulted once per iteration, so it can only refuse a dispatch
//! that is *between* waits; releasing one that is already inside a wait is the
//! caller's `park` contract below, and the caller owes it (#72).

use std::time::{Duration, Instant};

use aion_core::{ActivityId, WorkflowId};

use super::census::{PoolCensus, classify};
use super::declarations::QueueDeclarationSource;
use super::policy::{QueueServiceConfig, QueueServicePolicy};
use super::state::{Parked, QueueServiceState};
use super::taxonomy::{
    ExpiredClock, QueueServiceReason, ServiceAddress, WorkerUnavailable, millis,
};
use crate::worker::registry::{ConnectedWorkerRegistry, WorkerHandle};

/// Age rendering for an address no compatible worker has ever served.
const NEVER: &str = "never";

/// Why selection ended without a worker.
#[derive(Clone, Debug)]
pub enum SelectionRefusal {
    /// The queue is not being served and the policy/clock refused the dispatch.
    Unavailable(Box<WorkerUnavailable>),
    /// The server stopped accepting work — the drain gate's own reason,
    /// untouched.
    NotAccepting {
        /// Reason reported by the drain gate.
        reason: String,
    },
    /// The registry itself could not be read.
    Registry {
        /// Underlying registry failure, rendered.
        reason: String,
    },
}

impl SelectionRefusal {
    /// The failure string handed back at the engine dispatch seam.
    #[must_use]
    pub fn reason_string(&self) -> String {
        match self {
            Self::Unavailable(unavailable) => unavailable.reason_string(),
            Self::NotAccepting { reason } | Self::Registry { reason } => reason.clone(),
        }
    }
}

/// Everything the wait needs that outlives one iteration.
pub struct ServiceWait<'a> {
    /// Live connected-worker registry.
    pub registry: &'a ConnectedWorkerRegistry,
    /// Deployed queue declarations.
    pub declarations: &'a QueueDeclarationSource,
    /// Operator's policies and clocks.
    pub config: &'a QueueServiceConfig,
    /// Queryable unserved-queue state.
    pub state: &'a QueueServiceState,
    /// Address this dispatch needs served.
    pub address: &'a ServiceAddress,
    /// Owning workflow.
    pub workflow_id: &'a WorkflowId,
    /// Activity ordinal recorded in history.
    pub activity_id: &'a ActivityId,
    /// Cluster-event publisher an UNBOUNDED park announces itself on (#266
    /// T4). `None` (isolated tests) emits nothing on the wire; like the
    /// defaulted queue seams on the dispatchers, an unwired publisher loses
    /// only the pushed echo — the WARN and the queryable unserved state can
    /// never be switched off by wiring. Both production dispatchers share the
    /// boot's one publisher here.
    pub publisher: Option<&'a crate::cluster_publisher::ClusterEventPublisher>,
}

/// Select a worker for the address, or refuse with a typed reason.
///
/// `accepting` is the drain gate, consulted exactly where it always was —
/// before anything else on a selection miss. `park` performs one wait for a
/// worker arrival: `Some(budget)` must return by `budget` at the latest, `None`
/// may wait indefinitely (the caller owns the runtime plumbing).
///
/// `park` must ALSO return once the server stops accepting work, in both arms.
/// This loop cannot enforce that itself: it consults `accepting` between waits,
/// never during one, so a `park` that only ends on a worker arrival makes a
/// dispatch to an unserved queue unrefusable and — on the blocking pool tokio's
/// runtime `Drop` joins — unexitable (#72).
///
/// # Errors
///
/// Returns [`SelectionRefusal`] when the dispatch is refused rather than
/// served.
pub fn select_worker_or_refuse(
    wait: &ServiceWait<'_>,
    accepting: &mut dyn FnMut() -> Result<(), String>,
    park: &mut dyn FnMut(Option<Duration>),
) -> Result<WorkerHandle, SelectionRefusal> {
    let started_at = Instant::now();
    let address = wait.address;
    let policy = wait
        .config
        .policy_for(&address.namespace, &address.task_queue);
    let deadline = wait.config.availability_deadline_for(policy);
    let mut reported: Option<QueueServiceReason> = None;
    let outcome = loop {
        // `node` is the OPTIONAL within-pool affinity carried on the dispatch:
        // `Some(n)` pins selection to workers advertising node `n`; `None` is
        // unpinned and reaches any worker in the (namespace, task_queue) pool.
        match wait.registry.select_worker(
            &address.namespace,
            &address.task_queue,
            &address.activity_type,
            address.node.as_deref(),
        ) {
            Ok(Some(worker)) => {
                if let Some(reason) = reported {
                    tracing::info!(
                        namespace = %address.namespace,
                        task_queue = %address.task_queue,
                        activity_type = %address.activity_type,
                        workflow_id = %wait.workflow_id,
                        activity_id = %wait.activity_id,
                        queue_service_reason = reason.as_str(),
                        waited_ms = millis(started_at.elapsed()),
                        "queue service restored; the parked dispatch has a worker"
                    );
                }
                break Ok(worker);
            }
            Ok(None) => {
                if let Err(reason) = accepting() {
                    break Err(SelectionRefusal::NotAccepting { reason });
                }
                let waited = started_at.elapsed();
                let observed =
                    match observe_selection_miss(wait, policy, deadline, waited, reported) {
                        Ok(Some(observed)) => observed,
                        // A worker arrived between the miss and the census: the
                        // state we would report is already untrue, so re-select
                        // rather than announce it.
                        Ok(None) => continue,
                        Err(refusal) => break Err(refusal),
                    };
                let (reason, census) = (observed.reason, observed.census);
                reported = Some(reason);
                if reason.is_structural() {
                    // Unconditional: no policy admits work onto a queue no
                    // deployment declares, and no clock can change the answer.
                    break Err(unavailable(address, reason, None, waited, census));
                }
                match deadline {
                    Some(deadline) => {
                        let Some(remaining) = deadline
                            .checked_sub(waited)
                            .filter(|remaining| !remaining.is_zero())
                        else {
                            break Err(unavailable(
                                address,
                                reason,
                                Some(ExpiredClock::ServiceAvailability),
                                waited,
                                census,
                            ));
                        };
                        park(Some(remaining));
                    }
                    None => park(None),
                }
            }
            Err(error) => {
                break Err(SelectionRefusal::Registry {
                    reason: format!("registry error: {error}"),
                });
            }
        }
    };
    clear_unserved(wait);
    outcome
}

/// What one classified selection miss turned out to be.
pub struct ObservedMiss {
    /// The classified reason this address is not being served.
    pub reason: QueueServiceReason,
    /// The live poller census the classification was made against.
    pub census: PoolCensus,
}

/// Classify one selection miss, publish it to the queryable unserved state, and
/// state it in the log.
///
/// This is the whole of "park VISIBLY", factored out so it has exactly ONE
/// implementation. There are two dispatch loops in this server — the blocking
/// selection wait below, and the async gRPC outbox leg — and the second was
/// written before this machinery existed and never adopted it. A dispatch that
/// parked there was invisible: no state to query, one `info!` line, and no
/// reachable dead-letter path. Duplicating the classification to fix that would
/// have guaranteed the two drifted, so both call this.
///
/// It deliberately decides NOTHING about whether to keep waiting. Bounding an
/// unserved dispatch is a semantics decision that belongs to the operator, and
/// a bound invented here would refuse work nobody asked to have refused.
///
/// `Ok(None)` means a worker arrived between the miss and the census, so there
/// is nothing true left to report — the caller should re-select rather than
/// announce a state that is already untrue.
///
/// # Errors
///
/// Returns [`SelectionRefusal::Registry`] when the census itself cannot be read.
pub fn observe_selection_miss(
    wait: &ServiceWait<'_>,
    policy: QueueServicePolicy,
    deadline: Option<Duration>,
    waited: Duration,
    reported: Option<QueueServiceReason>,
) -> Result<Option<ObservedMiss>, SelectionRefusal> {
    let address = wait.address;
    let census = wait
        .registry
        .pool_census(
            &address.namespace,
            &address.task_queue,
            &address.activity_type,
            address.node.as_deref(),
        )
        .map_err(|error| SelectionRefusal::Registry {
            reason: format!("registry error: {error}"),
        })?;
    let declaration = wait.declarations.declaration_for(&address.task_queue);
    let Some(reason) = classify(declaration, &census) else {
        return Ok(None);
    };
    let repeat = reported == Some(reason);
    mark_unserved(wait, reason, policy, census);
    report(&Report {
        wait,
        reason,
        policy,
        census,
        waited,
        deadline,
        repeat,
    });
    // Only the UNBOUNDED park is pushed to the cluster channel: a
    // deadline-bounded park expires into a typed refusal that reaches the
    // workflow's own history, while this class otherwise ends nowhere an
    // operator is pushed to (#266 T4). Same transition rule as the WARN:
    // first classification and reason changes emit, repeats are silent.
    if deadline.is_none() && !repeat {
        emit_unbounded_park(wait, reason, policy, &census, waited);
    }
    Ok(Some(ObservedMiss { reason, census }))
}

/// Clear one dispatch's unserved entry once it is served or refused.
pub fn clear_selection_miss(wait: &ServiceWait<'_>) {
    clear_unserved(wait);
}

fn unavailable(
    address: &ServiceAddress,
    reason: QueueServiceReason,
    clock: Option<ExpiredClock>,
    waited: Duration,
    census: PoolCensus,
) -> SelectionRefusal {
    SelectionRefusal::Unavailable(Box::new(WorkerUnavailable {
        reason,
        clock,
        waited,
        address: address.clone(),
        census,
    }))
}

fn mark_unserved(
    wait: &ServiceWait<'_>,
    reason: QueueServiceReason,
    policy: QueueServicePolicy,
    census: PoolCensus,
) {
    if let Err(error) = wait.state.mark(Parked {
        address: wait.address,
        reason,
        policy,
        census,
        workflow_id: wait.workflow_id,
        activity_id: wait.activity_id,
    }) {
        tracing::error!(%error, "failed to publish the unserved queue state");
    }
}

/// Announce one unbounded-park transition on the deployment's cluster channel.
///
/// The taxonomy reason and policy cross as their canonical spellings because
/// `aion-core` must not depend on this crate's enums; every other field is the
/// same data the WARN and the queryable state already carry. An unwired
/// publisher (isolated tests) emits nothing — the honesty contract on
/// [`ServiceWait::publisher`] — and a publisher with no live subscribers is
/// the calm single-node case, not an error.
fn emit_unbounded_park(
    wait: &ServiceWait<'_>,
    reason: QueueServiceReason,
    policy: QueueServicePolicy,
    census: &PoolCensus,
    waited: Duration,
) {
    let Some(publisher) = wait.publisher else {
        return;
    };
    let address = wait.address;
    publisher.emit(|meta| aion_core::ClusterEvent::DispatchParked {
        meta,
        namespace: address.namespace.clone(),
        task_queue: address.task_queue.clone(),
        activity_type: address.activity_type.clone(),
        node: address.node.clone(),
        reason: reason.as_str().to_owned(),
        policy: policy.as_str().to_owned(),
        workflow_id: wait.workflow_id.clone(),
        activity_id: wait.activity_id.clone(),
        waited_ms: millis(waited),
        workers_in_pool: census.workers_in_pool,
        workers_serving_activity: census.workers_serving_activity,
        compatible_workers: census.compatible_workers,
        last_compatible_poller_age_ms: census.last_compatible_poller_age.map(millis),
    });
}

fn clear_unserved(wait: &ServiceWait<'_>) {
    if let Err(error) = wait
        .state
        .clear(wait.address, wait.workflow_id, wait.activity_id)
    {
        tracing::error!(%error, "failed to clear the unserved queue state");
    }
}

struct Report<'a> {
    wait: &'a ServiceWait<'a>,
    reason: QueueServiceReason,
    policy: QueueServicePolicy,
    census: PoolCensus,
    waited: Duration,
    deadline: Option<Duration>,
    repeat: bool,
}

/// State the park at WARN the first time, and whenever the verdict changes.
///
/// Repeats drop to DEBUG: a queue nobody serves would otherwise emit a WARN on
/// every poll for as long as the run lives, which is how a real signal becomes
/// noise nobody reads. The escalation an operator sees is INFO (the old,
/// unreasoned line) becoming WARN (this one, with the reason and the age).
fn report(report: &Report<'_>) {
    let Report {
        wait,
        reason,
        policy,
        census,
        waited,
        deadline,
        repeat,
    } = report;
    let address = wait.address;
    let age = census
        .last_compatible_poller_age
        .map_or_else(|| NEVER.to_owned(), |age| millis(age).to_string());
    let deadline_ms = deadline.map_or_else(|| NEVER.to_owned(), |value| millis(value).to_string());
    if *repeat {
        tracing::debug!(
            namespace = %address.namespace,
            task_queue = %address.task_queue,
            activity_type = %address.activity_type,
            node = address.node.as_deref(),
            workflow_id = %wait.workflow_id,
            activity_id = %wait.activity_id,
            queue_service_reason = reason.as_str(),
            queue_service_policy = policy.as_str(),
            last_compatible_poller_age_ms = %age,
            waited_ms = millis(*waited),
            "queue still unserved"
        );
        return;
    }
    tracing::warn!(
        namespace = %address.namespace,
        task_queue = %address.task_queue,
        activity_type = %address.activity_type,
        node = address.node.as_deref(),
        workflow_id = %wait.workflow_id,
        activity_id = %wait.activity_id,
        queue_service_reason = reason.as_str(),
        queue_service_policy = policy.as_str(),
        last_compatible_poller_age_ms = %age,
        service_availability_deadline_ms = %deadline_ms,
        waited_ms = millis(*waited),
        workers_in_pool = census.workers_in_pool,
        workers_serving_activity = census.workers_serving_activity,
        compatible_workers = census.compatible_workers,
        "dispatch is parked on a queue that is not being served"
    );
}
