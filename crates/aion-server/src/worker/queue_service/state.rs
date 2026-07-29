//! The live, queryable unserved-queue state.
//!
//! A parked dispatch is no longer invisible. While one waits, its address sits
//! in here with the taxonomy reason, the policy it is parked under, the poller
//! census behind the verdict, and every run waiting on it — so "which runs are
//! stuck, on what, since when" is a server-side read rather than a log
//! archaeology exercise.
//!
//! Entries exist only while a dispatch is actually parked: the wait loop clears
//! its own entry on every exit path (served, refused, drained), so the state
//! never accumulates addresses that are fine.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use aion_core::{ActivityId, WorkflowId};

use super::census::PoolCensus;
use super::policy::QueueServicePolicy;
use super::taxonomy::{QueueServiceReason, ServiceAddress};
use crate::error::ServerError;

/// Identity of an unserved address: the pool plus the activity type on it.
///
/// The node pin is per-dispatch, not per-address, so it lives on the waiting
/// entries rather than the key.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct UnservedKey {
    /// Correctness/isolation boundary.
    pub namespace: String,
    /// Pool selector within the namespace.
    pub task_queue: String,
    /// Activity type that is unserved on it.
    pub activity_type: String,
}

impl UnservedKey {
    /// The key an address belongs to.
    #[must_use]
    pub fn of(address: &ServiceAddress) -> Self {
        Self {
            namespace: address.namespace.clone(),
            task_queue: address.task_queue.clone(),
            activity_type: address.activity_type.clone(),
        }
    }
}

/// One run's activity waiting on an unserved address.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnservedDispatch {
    /// Owning workflow.
    pub workflow_id: WorkflowId,
    /// Activity ordinal recorded in history.
    pub activity_id: ActivityId,
    /// Node this dispatch is pinned to, if any.
    pub node: Option<String>,
    /// How long this dispatch has been parked.
    pub waiting_for: Duration,
}

/// The unserved state of one address, as read by an operator surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnservedQueue {
    /// Which address is unserved.
    pub key: UnservedKey,
    /// Taxonomy state of the address at the last classification.
    pub reason: QueueServiceReason,
    /// Policy the parked dispatches are being held under.
    pub policy: QueueServicePolicy,
    /// Live poller census behind the verdict.
    pub census: PoolCensus,
    /// How long this address has been continuously unserved.
    pub unserved_for: Duration,
    /// Every dispatch currently parked on it, oldest first.
    pub waiting: Vec<UnservedDispatch>,
}

#[derive(Clone, Debug)]
struct Entry {
    reason: QueueServiceReason,
    policy: QueueServicePolicy,
    census: PoolCensus,
    since: Instant,
    /// Keyed by execution identity; `WorkflowId`/`ActivityId` are hashable but
    /// not ordered, so the read path sorts explicitly rather than relying on
    /// map order.
    waiting: HashMap<(WorkflowId, ActivityId), Waiting>,
}

#[derive(Clone, Debug)]
struct Waiting {
    node: Option<String>,
    since: Instant,
}

/// Shared handle onto the live unserved-queue state.
#[derive(Clone, Debug, Default)]
pub struct QueueServiceState {
    inner: Arc<Mutex<BTreeMap<UnservedKey, Entry>>>,
}

impl QueueServiceState {
    /// Record (or refresh) one dispatch parked on an unserved address.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the state lock is poisoned.
    pub fn mark(&self, parked: Parked<'_>) -> Result<(), ServerError> {
        let key = UnservedKey::of(parked.address);
        let now = Instant::now();
        let mut state = self.state()?;
        let entry = state.entry(key).or_insert_with(|| Entry {
            reason: parked.reason,
            policy: parked.policy,
            census: parked.census,
            since: now,
            waiting: HashMap::new(),
        });
        entry.reason = parked.reason;
        entry.policy = parked.policy;
        entry.census = parked.census;
        entry
            .waiting
            .entry((parked.workflow_id.clone(), parked.activity_id.clone()))
            .or_insert_with(|| Waiting {
                node: parked.address.node.clone(),
                since: now,
            });
        Ok(())
    }

    /// Drop one dispatch from the unserved state, dropping the address itself
    /// when nothing waits on it any more.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the state lock is poisoned.
    pub fn clear(
        &self,
        address: &ServiceAddress,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
    ) -> Result<(), ServerError> {
        let key = UnservedKey::of(address);
        let mut state = self.state()?;
        let Some(entry) = state.get_mut(&key) else {
            return Ok(());
        };
        entry
            .waiting
            .remove(&(workflow_id.clone(), activity_id.clone()));
        if entry.waiting.is_empty() {
            state.remove(&key);
        }
        Ok(())
    }

    /// Every address currently unserved, in stable address order.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the state lock is poisoned.
    pub fn unserved(&self) -> Result<Vec<UnservedQueue>, ServerError> {
        let now = Instant::now();
        let state = self.state()?;
        Ok(state
            .iter()
            .map(|(key, entry)| {
                let mut waiting: Vec<UnservedDispatch> = entry
                    .waiting
                    .iter()
                    .map(|((workflow_id, activity_id), held)| UnservedDispatch {
                        workflow_id: workflow_id.clone(),
                        activity_id: activity_id.clone(),
                        node: held.node.clone(),
                        waiting_for: now.saturating_duration_since(held.since),
                    })
                    .collect();
                waiting.sort_by(|left, right| {
                    right.waiting_for.cmp(&left.waiting_for).then_with(|| {
                        left.workflow_id
                            .to_string()
                            .cmp(&right.workflow_id.to_string())
                    })
                });
                UnservedQueue {
                    key: key.clone(),
                    reason: entry.reason,
                    policy: entry.policy,
                    census: entry.census,
                    unserved_for: now.saturating_duration_since(entry.since),
                    waiting,
                }
            })
            .collect())
    }

    /// How many dispatches are currently parked across every address on
    /// `task_queue`, in any namespace.
    ///
    /// Read by the worker-expiry sweep so the deregistration of a dead worker
    /// states the operational consequence in the same breath as the cause: "this
    /// worker is gone AND seven dispatches are already parked on the queue it
    /// was serving" is an alertable sentence; "worker deregistered" alone is
    /// not. Queue-scoped rather than address-scoped because a worker serves a
    /// whole queue, not one activity type on it.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the state lock is poisoned.
    pub fn parked_on_queue(&self, task_queue: &str) -> Result<usize, ServerError> {
        Ok(self
            .state()?
            .iter()
            .filter(|(key, _)| key.task_queue == task_queue)
            .map(|(_, entry)| entry.waiting.len())
            .sum())
    }

    /// Whether one run's activity is currently parked on an unserved queue.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the state lock is poisoned.
    pub fn is_unserved(
        &self,
        workflow_id: &WorkflowId,
        activity_id: &ActivityId,
    ) -> Result<bool, ServerError> {
        let held = (workflow_id.clone(), activity_id.clone());
        Ok(self
            .state()?
            .values()
            .any(|entry| entry.waiting.contains_key(&held)))
    }

    fn state(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, BTreeMap<UnservedKey, Entry>>, ServerError> {
        self.inner
            .lock()
            .map_err(|_| ServerError::lock_poisoned("queue service state"))
    }
}

/// One parked dispatch, as handed to [`QueueServiceState::mark`].
#[derive(Clone, Copy, Debug)]
pub struct Parked<'a> {
    /// Address the dispatch is parked on.
    pub address: &'a ServiceAddress,
    /// Taxonomy state of that address.
    pub reason: QueueServiceReason,
    /// Policy the dispatch is parked under.
    pub policy: QueueServicePolicy,
    /// Poller census behind the verdict.
    pub census: PoolCensus,
    /// Owning workflow.
    pub workflow_id: &'a WorkflowId,
    /// Activity ordinal.
    pub activity_id: &'a ActivityId,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address() -> ServiceAddress {
        ServiceAddress {
            namespace: "default".to_owned(),
            task_queue: "general".to_owned(),
            activity_type: "greet".to_owned(),
            node: None,
        }
    }

    fn parked<'a>(
        address: &'a ServiceAddress,
        workflow_id: &'a WorkflowId,
        activity_id: &'a ActivityId,
    ) -> Parked<'a> {
        Parked {
            address,
            reason: QueueServiceReason::NoLivePollers,
            policy: QueueServicePolicy::DurablePending,
            census: PoolCensus::default(),
            workflow_id,
            activity_id,
        }
    }

    #[test]
    fn a_parked_dispatch_is_visible_immediately() -> Result<(), ServerError> {
        let state = QueueServiceState::default();
        let address = address();
        let workflow_id = WorkflowId::new_v4();
        let activity_id = ActivityId::from_sequence_position(3);
        state.mark(parked(&address, &workflow_id, &activity_id))?;

        let unserved = state.unserved()?;
        assert_eq!(unserved.len(), 1);
        assert_eq!(unserved[0].reason, QueueServiceReason::NoLivePollers);
        assert_eq!(unserved[0].policy, QueueServicePolicy::DurablePending);
        assert_eq!(unserved[0].key.task_queue, "general");
        assert_eq!(unserved[0].waiting.len(), 1);
        assert_eq!(unserved[0].waiting[0].workflow_id, workflow_id);
        assert!(state.is_unserved(&workflow_id, &activity_id)?);
        Ok(())
    }

    #[test]
    fn clearing_the_last_waiter_removes_the_address() -> Result<(), ServerError> {
        let state = QueueServiceState::default();
        let address = address();
        let first = WorkflowId::new_v4();
        let second = WorkflowId::new_v4();
        let activity_id = ActivityId::from_sequence_position(0);
        state.mark(parked(&address, &first, &activity_id))?;
        state.mark(parked(&address, &second, &activity_id))?;
        assert_eq!(state.unserved()?.len(), 1);
        assert_eq!(state.unserved()?[0].waiting.len(), 2);

        state.clear(&address, &first, &activity_id)?;
        assert!(!state.is_unserved(&first, &activity_id)?);
        assert_eq!(state.unserved()?[0].waiting.len(), 1);

        state.clear(&address, &second, &activity_id)?;
        assert!(state.unserved()?.is_empty(), "the address must disappear");
        Ok(())
    }

    #[test]
    fn clearing_an_unknown_dispatch_is_a_no_op() -> Result<(), ServerError> {
        let state = QueueServiceState::default();
        state.clear(
            &address(),
            &WorkflowId::new_v4(),
            &ActivityId::from_sequence_position(0),
        )?;
        assert!(state.unserved()?.is_empty());
        Ok(())
    }

    #[test]
    fn re_marking_refreshes_the_verdict_without_duplicating_the_waiter() -> Result<(), ServerError>
    {
        let state = QueueServiceState::default();
        let address = address();
        let workflow_id = WorkflowId::new_v4();
        let activity_id = ActivityId::from_sequence_position(0);
        state.mark(parked(&address, &workflow_id, &activity_id))?;
        state.mark(Parked {
            reason: QueueServiceReason::PollersIncompatible,
            census: PoolCensus {
                workers_in_pool: 2,
                ..PoolCensus::default()
            },
            ..parked(&address, &workflow_id, &activity_id)
        })?;

        let unserved = state.unserved()?;
        assert_eq!(unserved.len(), 1);
        assert_eq!(unserved[0].waiting.len(), 1);
        assert_eq!(unserved[0].reason, QueueServiceReason::PollersIncompatible);
        assert_eq!(unserved[0].census.workers_in_pool, 2);
        Ok(())
    }
}
