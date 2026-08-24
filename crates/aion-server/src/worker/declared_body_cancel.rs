//! Stopping the declared bodies THIS server is executing.
//!
//! [`super::activity_cancel`] asks the workers holding a cancelled run's
//! activities to stop. A declared action body has no worker to ask: it is a
//! process this server started itself, at the dispatch seam, before any
//! task-queue routing happened — so it appears in no heartbeat tracker, is held
//! by no connected worker, and nothing in the cancel path could see it. A
//! cancelled run's server-run command therefore kept a machine busy while the
//! console truthfully reported `Cancelled`, which is the same defect #233 fixed
//! for remote workers, one execution path over.
//!
//! This module is the registry that makes those attempts visible while they
//! run, and the stop the server performs ITSELF rather than asks for.
//!
//! # This one is not a request
//!
//! A worker cancel establishes only that the server asked. Signalling here
//! reaches [`aion_worker::ActivityContext`]'s cooperative cancellation, which
//! the declared-body executor has already handed to
//! [`aion_worker::run_cancellable_command`]: `SIGTERM` →
//! [`aion_worker::PROCESS_GROUP_TERMINATION_GRACE`] → `SIGKILL` across the whole
//! process group, with the `Cancelled` verdict withheld until the group has been
//! PROVEN gone. What this module returns is still an ASKING — the signal has
//! been raised, not yet acted on — but the attempt it names cannot report back
//! until its process tree is gone, so the stop is witnessed rather than assumed,
//! by the attempt itself.
//!
//! # An unregistered attempt is uncancellable
//!
//! Registration is therefore not bookkeeping. An attempt that fails to enter
//! this registry has no cancellation path at all, so the executor refuses the
//! dispatch instead of running a command nothing could stop.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use aion_core::WorkflowId;
use aion_worker::ActivityCancellationHandle;

use super::intervention::AttemptKey;
use crate::error::ServerError;
use crate::shutdown::DrainState;

/// The lock resource name carried by a [`ServerError::LockPoisoned`] raised
/// here, so an operator reading the error knows which state could not be read.
const RESOURCE: &str = "declared command attempts";

/// The declared-command attempts this server is executing right now.
///
/// An entry exists for exactly as long as one attempt's command is running:
/// [`Self::register`] returns a guard that removes it, so an attempt that
/// finished, failed, or panicked its way out cannot be signalled afterwards.
/// Keyed on the full [`AttemptKey`] — including the run — because a
/// continue-as-new chain reuses one workflow id across generations while
/// activity ordinals and attempt numbers restart, so a shorter key genuinely
/// COLLIDES: two generations' attempts would be one entry, and registering the
/// second would replace the first's handle with nothing left to stop it. The
/// run axis is what keeps them distinct here and what lets the cancel report
/// name which generation it stopped. Cancelling is still workflow-scoped, as it
/// is for workers ([`super::HeartbeatTracker::in_flight_for_workflow`]): a
/// cancelled workflow's work stops in every generation of it.
#[derive(Clone, Debug)]
pub struct DeclaredCommandAttempts {
    inner: Arc<Mutex<HashMap<AttemptKey, ActivityCancellationHandle>>>,
    /// The drain latch this registry wakes when an attempt finishes. The
    /// graceful drain waits for this census as well as the heartbeat tracker's
    /// (a declared body is in-flight work no worker holds), and a waiter that
    /// is never woken outlives every command by the full drain window.
    drain: DrainState,
}

impl DeclaredCommandAttempts {
    /// Build an empty registry that wakes `drain`'s activity-drained latch
    /// whenever an executing attempt finishes.
    ///
    /// The latch is required rather than optional for the same reason the
    /// dispatcher requires the registry itself: a construction path that could
    /// silently skip the wiring would recreate the drained-over-live-work bug
    /// the wiring exists to prevent, invisibly.
    #[must_use]
    pub fn new(drain: DrainState) -> Self {
        Self {
            inner: Arc::default(),
            drain,
        }
    }

    /// Record that this server is executing `key`'s declared command, and hand
    /// back the guard that keeps the entry alive.
    ///
    /// The entry lives exactly as long as the returned
    /// [`DeclaredAttemptRegistration`]. A second registration of the same key —
    /// which would mean one attempt executing twice at once, and is a defect
    /// wherever it came from — is refused rather than allowed to overwrite the
    /// handle of a command still running, because an overwritten handle is an
    /// attempt nothing can stop.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] when the registry cannot be read,
    /// and [`ServerError::DeclaredAttemptCollision`] when `key` is already
    /// executing.
    pub fn register(
        &self,
        key: AttemptKey,
        cancellation: ActivityCancellationHandle,
    ) -> Result<DeclaredAttemptRegistration, ServerError> {
        let mut attempts = self
            .inner
            .lock()
            .map_err(|_poisoned| ServerError::lock_poisoned(RESOURCE))?;
        if attempts.contains_key(&key) {
            return Err(ServerError::DeclaredAttemptCollision {
                workflow_id: key.workflow_id.clone(),
                activity_id: key.activity_id.clone(),
                attempt: key.attempt,
            });
        }
        attempts.insert(key.clone(), cancellation);
        drop(attempts);
        Ok(DeclaredAttemptRegistration {
            attempts: self.clone(),
            key,
        })
    }

    /// Signal every declared command this server is executing for `workflow_id`.
    ///
    /// Returns the attempts signalled, in activity-then-attempt order, so the
    /// caller reports a stable list rather than whatever order the map yielded.
    /// An empty result means this server is executing none of the run's bodies,
    /// which is the common case and is not a failure.
    ///
    /// The entries are NOT removed here. Removing them is the executing
    /// attempt's own act, on the guard it holds, once its process group is gone
    /// — and a cancel that deregistered an attempt it had merely signalled would
    /// make a second cancel a silent no-op against a command still dying.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] when the registry cannot be read.
    /// Not survivable: answering "this server is executing nothing for that run"
    /// out of state that could not be read is exactly how a cancelled run keeps
    /// a machine.
    pub fn cancel_workflow(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<AttemptKey>, ServerError> {
        let attempts = self
            .inner
            .lock()
            .map_err(|_poisoned| ServerError::lock_poisoned(RESOURCE))?;
        let mut signalled = Vec::new();
        for (key, cancellation) in attempts.iter() {
            if key.workflow_id == *workflow_id {
                cancellation.cancel();
                signalled.push(key.clone());
            }
        }
        drop(attempts);
        signalled.sort_by_key(|key| (key.activity_id.sequence_position(), key.attempt));
        Ok(signalled)
    }

    /// Every declared command this server is executing right now, in
    /// activity-then-attempt order.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] when the registry cannot be read.
    pub fn executing(&self) -> Result<Vec<AttemptKey>, ServerError> {
        let attempts = self
            .inner
            .lock()
            .map_err(|_poisoned| ServerError::lock_poisoned(RESOURCE))?;
        let mut keys = attempts.keys().cloned().collect::<Vec<_>>();
        drop(attempts);
        keys.sort_by_key(|key| (key.activity_id.sequence_position(), key.attempt));
        Ok(keys)
    }

    /// Remove one entry, whatever state the lock is in.
    ///
    /// The poisoned lock is RECOVERED rather than reported, because this runs
    /// from a drop guard that has no way to return a failure and because the
    /// alternative is worse: a retained entry outlives the command it names, and
    /// a later cancel would then signal a handle whose activity has already
    /// ended. The poisoning itself is stated once, here, at ERROR.
    fn release(&self, key: &AttemptKey) {
        let mut attempts = match self.inner.lock() {
            Ok(attempts) => attempts,
            Err(poisoned) => {
                tracing::error!(
                    resource = RESOURCE,
                    "the declared-command attempt registry's lock is poisoned; recovering it \
                     to release the finished attempt rather than leaving a stale entry a \
                     later cancel could signal"
                );
                PoisonError::into_inner(poisoned)
            }
        };
        attempts.remove(key);
        drop(attempts);
        // Wake the graceful drain's census loop: this may have been the last
        // in-flight declared body it was waiting out. Waking with no waiter is
        // a no-op, so the ordinary (non-draining) case costs nothing.
        self.drain.notify_activity_drained();
    }
}

/// Keeps one executing declared command visible to the cancel path.
///
/// Dropping it deregisters the attempt, so the entry cannot outlive the command
/// it names — including when the executing thread unwinds.
#[derive(Debug)]
pub struct DeclaredAttemptRegistration {
    attempts: DeclaredCommandAttempts,
    key: AttemptKey,
}

impl DeclaredAttemptRegistration {
    /// The attempt this registration keeps visible.
    #[must_use]
    pub const fn key(&self) -> &AttemptKey {
        &self.key
    }
}

impl Drop for DeclaredAttemptRegistration {
    fn drop(&mut self) {
        self.attempts.release(&self.key);
    }
}

#[cfg(test)]
mod tests {
    use aion_core::{ActivityId, RunId, WorkflowId};
    use aion_worker::ActivityContext;

    use super::{AttemptKey, DeclaredCommandAttempts, DrainState};

    /// What a test returns. Every fallible step is carried rather than
    /// unwrapped, because the workspace denies panicking accessors in test code
    /// as firmly as in library code.
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn key(workflow_id: &WorkflowId, position: u64, attempt: u32) -> AttemptKey {
        AttemptKey::new(
            workflow_id.clone(),
            RunId::new_v4(),
            ActivityId::from_sequence_position(position),
            attempt,
        )
    }

    /// One executing attempt is signalled, and the signal reaches the context
    /// the executor is running under — not a copy of it.
    #[tokio::test]
    async fn a_registered_attempt_is_signalled_on_its_own_context() -> TestResult {
        let attempts = DeclaredCommandAttempts::new(DrainState::default());
        let workflow_id = WorkflowId::new_v4();
        let target = key(&workflow_id, 3, 1);
        let (context, cancellation) = ActivityContext::new(
            target.workflow_id.clone(),
            target.run_id.clone(),
            target.activity_id.clone(),
            target.attempt,
        );
        let registration = attempts.register(target.clone(), cancellation)?;

        let signalled = attempts.cancel_workflow(&workflow_id)?;

        assert_eq!(signalled, vec![target]);
        assert!(
            context.is_cancelled(),
            "the registry must signal the context the command is running under"
        );
        drop(registration);
        Ok(())
    }

    /// Another run's attempt is never signalled — a cancel must not reach a
    /// bystander sharing this server.
    #[tokio::test]
    async fn another_workflows_attempt_is_never_signalled() -> TestResult {
        let attempts = DeclaredCommandAttempts::new(DrainState::default());
        let cancelled = WorkflowId::new_v4();
        let bystander = WorkflowId::new_v4();
        let target = key(&cancelled, 1, 1);
        let spectator = key(&bystander, 1, 1);
        let (_target_context, target_cancellation) = ActivityContext::new(
            target.workflow_id.clone(),
            target.run_id.clone(),
            target.activity_id.clone(),
            target.attempt,
        );
        let (spectator_context, spectator_cancellation) = ActivityContext::new(
            spectator.workflow_id.clone(),
            spectator.run_id.clone(),
            spectator.activity_id.clone(),
            spectator.attempt,
        );
        let target_registration = attempts.register(target.clone(), target_cancellation)?;
        let spectator_registration = attempts.register(spectator, spectator_cancellation)?;

        let signalled = attempts.cancel_workflow(&cancelled)?;

        assert_eq!(signalled, vec![target]);
        assert!(
            !spectator_context.is_cancelled(),
            "a cancel must not reach another run's declared body"
        );
        drop(target_registration);
        drop(spectator_registration);
        Ok(())
    }

    /// A finished attempt leaves nothing behind: the guard's drop is what makes
    /// the entry's life exactly the command's life.
    #[tokio::test]
    async fn a_finished_attempt_is_no_longer_visible() -> TestResult {
        let attempts = DeclaredCommandAttempts::new(DrainState::default());
        let workflow_id = WorkflowId::new_v4();
        let target = key(&workflow_id, 1, 1);
        let (_context, cancellation) = ActivityContext::new(
            target.workflow_id.clone(),
            target.run_id.clone(),
            target.activity_id.clone(),
            target.attempt,
        );

        let registration = attempts.register(target, cancellation)?;
        assert_eq!(attempts.executing()?.len(), 1);
        drop(registration);

        assert!(
            attempts.executing()?.is_empty(),
            "a finished attempt must not stay signallable"
        );
        assert!(attempts.cancel_workflow(&workflow_id)?.is_empty());
        Ok(())
    }

    /// One attempt cannot execute twice at once. The refusal exists because the
    /// second registration would replace the first command's cancellation
    /// handle, and a replaced handle is a running command nothing can stop.
    #[tokio::test]
    async fn one_attempt_cannot_register_twice() -> TestResult {
        let attempts = DeclaredCommandAttempts::new(DrainState::default());
        let target = key(&WorkflowId::new_v4(), 1, 1);
        let (_first_context, first) = ActivityContext::new(
            target.workflow_id.clone(),
            target.run_id.clone(),
            target.activity_id.clone(),
            target.attempt,
        );
        let (_second_context, second) = ActivityContext::new(
            target.workflow_id.clone(),
            target.run_id.clone(),
            target.activity_id.clone(),
            target.attempt,
        );
        let registration = attempts.register(target.clone(), first)?;

        let Err(refusal) = attempts.register(target, second) else {
            return Err("a second execution of one attempt must be refused".into());
        };

        assert!(
            refusal.to_string().contains("already executing"),
            "the refusal must name what it refused: {refusal}"
        );
        drop(registration);
        Ok(())
    }
}
