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
}

/// Select a worker for the address, or refuse with a typed reason.
///
/// `accepting` is the drain gate, consulted exactly where it always was —
/// before anything else on a selection miss. `park` performs one bounded wait
/// for a worker arrival: `Some(budget)` must return by `budget` at the latest,
/// `None` may wait indefinitely (the caller owns the runtime plumbing).
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
                let census = match wait.registry.pool_census(
                    &address.namespace,
                    &address.task_queue,
                    &address.activity_type,
                    address.node.as_deref(),
                ) {
                    Ok(census) => census,
                    Err(error) => {
                        break Err(SelectionRefusal::Registry {
                            reason: format!("registry error: {error}"),
                        });
                    }
                };
                let declaration = wait.declarations.declaration_for(&address.task_queue);
                let Some(reason) = classify(declaration, &census) else {
                    // A worker arrived between the miss and the census: the
                    // state we would report is already untrue, so re-select
                    // rather than announce it.
                    continue;
                };
                mark_unserved(wait, reason, policy, census);
                let waited = started_at.elapsed();
                report(&Report {
                    wait,
                    reason,
                    policy,
                    census,
                    waited,
                    deadline,
                    repeat: reported == Some(reason),
                });
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
