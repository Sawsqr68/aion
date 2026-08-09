//! Run-scoping for transcript handles.
//!
//! # Why this file exists
//!
//! An `ActivityStreamKey` is `(workflow_id, run_id, activity_id, attempt)`, and
//! the run sits inside the durable `O`-region key itself: a sibling
//! generation's streams are excluded by the key range, so two runs of one
//! workflow can never fuse into one stream. What storage cannot decide is
//! whether the handle a caller constructed names anything at all. A `run_id`
//! that is not this workflow's, or an `(activity, attempt)` the named run never
//! dispatched, would read as an empty stream — and an empty answer to a
//! mis-addressed question is indistinguishable from "that step said nothing".
//!
//! This module refuses those handles instead of serving them empty:
//!
//! * a run that is not a run of this workflow is [`RunScopeError::UnknownRun`];
//! * an `(activity, attempt)` the named run never dispatched is
//!   [`RunScopeError::AttemptNotInRun`].
//!
//! Both verdicts are derived from the workflow's own event history, which is
//! the only record of what each generation actually dispatched.

use std::collections::{BTreeMap, BTreeSet};

use aion_core::{ActivityId, Event, RunId};

/// One `(activity ordinal, attempt)` pair — the two run-scoped axes of a
/// transcript stream key.
pub(crate) type AttemptAxis = (u64, u32);

/// What a workflow's history says about which run dispatched which attempt.
#[derive(Debug, Default)]
pub(crate) struct RunScope {
    /// Attempts dispatched by each run of the chain, in history order.
    by_run: BTreeMap<String, BTreeSet<AttemptAxis>>,
}

/// Why a transcript handle was rejected.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum RunScopeError {
    /// The named run is not a run of this workflow.
    #[error(
        "run {run_id} is not a run of workflow {workflow_id}; \
         call describe_run to obtain a real run_id rather than constructing one"
    )]
    UnknownRun {
        /// The workflow that was read.
        workflow_id: String,
        /// The run the caller named.
        run_id: String,
    },
    /// The run exists but never dispatched that `(activity, attempt)`.
    #[error(
        "run {run_id} never dispatched activity {activity_id} attempt {attempt}; \
         another run of workflow {workflow_id} may have, and its transcript is not this run's"
    )]
    AttemptNotInRun {
        /// The workflow that was read.
        workflow_id: String,
        /// The run the caller named.
        run_id: String,
        /// The activity ordinal the caller named.
        activity_id: u64,
        /// The attempt the caller named.
        attempt: u32,
    },
}

impl RunScope {
    /// Build the scope from a workflow's complete history.
    ///
    /// History is partitioned at each `WorkflowStarted`: that event is what
    /// opens a run, and every event after it belongs to that run until the next
    /// one opens. `ActivityStarted` is the dispatch record and is the only
    /// event that carries both an ordinal and an attempt at dispatch time, so
    /// it is what defines "this run dispatched that attempt".
    pub(crate) fn from_history(history: &[Event]) -> Self {
        let mut by_run: BTreeMap<String, BTreeSet<AttemptAxis>> = BTreeMap::new();
        let mut current: Option<String> = None;
        for event in history {
            match event {
                Event::WorkflowStarted { run_id, .. } => {
                    let key = run_id.to_string();
                    by_run.entry(key.clone()).or_default();
                    current = Some(key);
                }
                Event::ActivityStarted {
                    activity_id,
                    attempt,
                    ..
                } => {
                    if let Some(run) = current.as_ref() {
                        by_run
                            .entry(run.clone())
                            .or_default()
                            .insert((activity_id.sequence_position(), *attempt));
                    }
                }
                _ => {}
            }
        }
        Self { by_run }
    }

    /// Whether this workflow's history knows the run at all.
    pub(crate) fn knows_run(&self, run_id: &RunId) -> bool {
        self.by_run.contains_key(&run_id.to_string())
    }

    /// Validate a transcript handle against this workflow's history.
    ///
    /// # Errors
    ///
    /// [`RunScopeError::UnknownRun`] when the run is not this workflow's, and
    /// [`RunScopeError::AttemptNotInRun`] when the run never dispatched the
    /// named `(activity, attempt)`.
    pub(crate) fn validate_handle(
        &self,
        workflow_id: &aion_core::WorkflowId,
        run_id: &RunId,
        activity_id: &ActivityId,
        attempt: u32,
    ) -> Result<(), RunScopeError> {
        let axis = (activity_id.sequence_position(), attempt);
        let attempts =
            self.by_run
                .get(&run_id.to_string())
                .ok_or_else(|| RunScopeError::UnknownRun {
                    workflow_id: workflow_id.to_string(),
                    run_id: run_id.to_string(),
                })?;
        if !attempts.contains(&axis) {
            return Err(RunScopeError::AttemptNotInRun {
                workflow_id: workflow_id.to_string(),
                run_id: run_id.to_string(),
                activity_id: axis.0,
                attempt,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use aion_core::{
        ActivityId, ContentType, Event, EventEnvelope, PackageVersion, Payload, RunId, WorkflowId,
    };
    use chrono::{DateTime, Utc};
    use uuid::Uuid;

    use super::{RunScope, RunScopeError};

    fn workflow_id() -> WorkflowId {
        WorkflowId::new(Uuid::from_u128(1))
    }

    fn recorded_at(offset: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000 + offset, 0).unwrap_or_default()
    }

    fn envelope(seq: u64) -> EventEnvelope {
        EventEnvelope {
            seq,
            recorded_at: recorded_at(i64::try_from(seq).unwrap_or(0)),
            workflow_id: workflow_id(),
        }
    }

    fn empty_input() -> Payload {
        Payload::new(ContentType::Json, b"{}".to_vec())
    }

    fn started(seq: u64, run_id: &RunId) -> Event {
        Event::WorkflowStarted {
            envelope: envelope(seq),
            workflow_type: "chain".to_owned(),
            input: empty_input(),
            run_id: run_id.clone(),
            parent_run_id: None,
            package_version: PackageVersion::new("hash"),
        }
    }

    fn activity_started(seq: u64, ordinal: u64, attempt: u32) -> Event {
        Event::ActivityStarted {
            envelope: envelope(seq),
            activity_id: ActivityId::from_sequence_position(ordinal),
            attempt,
        }
    }

    /// One workflow, two generations, each dispatching ordinal 0 attempt 1.
    fn continue_as_new_chain(first: &RunId, second: &RunId) -> Vec<Event> {
        vec![
            started(0, first),
            activity_started(1, 0, 1),
            Event::WorkflowContinuedAsNew {
                envelope: envelope(2),
                input: empty_input(),
                workflow_type: None,
                parent_run_id: first.clone(),
            },
            started(3, second),
            activity_started(4, 0, 1),
        ]
    }

    /// Both generations dispatched ordinal 0 attempt 1; each run's own handle
    /// for it is valid. Stream separation across the generations is the storage
    /// key's job and is pinned by the `two_generations` tests in `aion-store`
    /// and `aion-store-haematite`.
    #[test]
    fn each_generation_owns_its_own_dispatched_handle() -> Result<(), RunScopeError> {
        let first = RunId::new(Uuid::from_u128(11));
        let second = RunId::new(Uuid::from_u128(12));
        let scope = RunScope::from_history(&continue_as_new_chain(&first, &second));

        assert!(scope.knows_run(&first));
        assert!(scope.knows_run(&second));

        scope.validate_handle(
            &workflow_id(),
            &first,
            &ActivityId::from_sequence_position(0),
            1,
        )?;
        scope.validate_handle(
            &workflow_id(),
            &second,
            &ActivityId::from_sequence_position(0),
            1,
        )?;
        Ok(())
    }

    #[test]
    fn a_run_that_is_not_this_workflows_is_refused() {
        let only = RunId::new(Uuid::from_u128(21));
        let stranger = RunId::new(Uuid::from_u128(99));
        let scope = RunScope::from_history(&[started(0, &only), activity_started(1, 0, 1)]);
        assert!(matches!(
            scope.validate_handle(
                &workflow_id(),
                &stranger,
                &ActivityId::from_sequence_position(0),
                1
            ),
            Err(RunScopeError::UnknownRun { .. })
        ));
    }

    #[test]
    fn an_attempt_this_run_never_dispatched_is_refused() {
        let first = RunId::new(Uuid::from_u128(11));
        let second = RunId::new(Uuid::from_u128(12));
        let mut history = continue_as_new_chain(&first, &second);
        // Only generation one dispatches ordinal 7.
        history.insert(2, activity_started(9, 7, 1));
        let scope = RunScope::from_history(&history);
        assert!(matches!(
            scope.validate_handle(
                &workflow_id(),
                &second,
                &ActivityId::from_sequence_position(7),
                1
            ),
            Err(RunScopeError::AttemptNotInRun { .. })
        ));
        assert!(
            scope
                .validate_handle(
                    &workflow_id(),
                    &first,
                    &ActivityId::from_sequence_position(7),
                    1
                )
                .is_ok()
        );
    }

    #[test]
    fn a_retry_attempt_is_validated_on_its_own_axis() -> Result<(), RunScopeError> {
        let first = RunId::new(Uuid::from_u128(11));
        let second = RunId::new(Uuid::from_u128(12));
        let mut history = continue_as_new_chain(&first, &second);
        // Generation two retries ordinal 0; attempt 2 exists nowhere else.
        history.push(activity_started(5, 0, 2));
        let scope = RunScope::from_history(&history);
        scope.validate_handle(
            &workflow_id(),
            &second,
            &ActivityId::from_sequence_position(0),
            2,
        )?;
        // Generation one never dispatched attempt 2 of ordinal 0.
        assert!(matches!(
            scope.validate_handle(
                &workflow_id(),
                &first,
                &ActivityId::from_sequence_position(0),
                2
            ),
            Err(RunScopeError::AttemptNotInRun { .. })
        ));
        Ok(())
    }
}
