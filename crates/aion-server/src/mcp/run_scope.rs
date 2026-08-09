//! Run-scoping for transcript handles.
//!
//! # Why this file exists
//!
//! An `ActivityStreamKey` is `(workflow_id, activity_id, attempt)` and carries
//! **no run**. All three axes are minted per-run: `activity_id` is the
//! scheduling ordinal within a run, and `attempt` restarts at one for each
//! dispatch of that ordinal. So two runs of one workflow — the generations of a
//! continue-as-new chain — routinely produce byte-identical keys for entirely
//! different work. The two streams are not in conflict; they are MERGED. There
//! is no `SequenceConflict`, no error, and no log line: the second generation's
//! events simply append after the first generation's, and a read returns both
//! as though one agent had said all of it.
//!
//! That is the worst failure shape available: an agent handed another
//! generation's conversation reasons over it as if it were the current step.
//!
//! # What this module does about it
//!
//! `run_id` is REQUIRED on every transcript handle the MCP surface accepts and
//! on every transcript handle it emits. This module is what makes that
//! requirement mean something today, before the storage key itself grows a run
//! axis:
//!
//! * It proves the `(activity_id, attempt)` a caller asked for is one **this
//!   run** actually dispatched — a handle naming another generation's ordinal
//!   is refused, not served.
//! * It reports whether that same `(activity_id, attempt)` was ALSO dispatched
//!   by another generation of the same workflow. If it was, the durable stream
//!   is a fusion of both and the read is flagged `transcript_run_ambiguous`
//!   with the colliding runs named.
//!
//! # What it does not do
//!
//! It cannot un-merge a fused stream. Ambiguity is detected and declared, never
//! silently repaired. The repair is the storage change — a run axis on
//! `ActivityEvent`, on `ActivityStreamKey`, and in the `O`-region key encoding
//! — which is a separate, larger cut. The handle shape here is already the
//! post-repair shape, so that cut narrows this module rather than changing any
//! caller.

use std::collections::{BTreeMap, BTreeSet};

use aion_core::{ActivityId, Event, RunId};

/// One `(activity ordinal, attempt)` pair — the two run-scoped axes of a
/// transcript stream key.
pub(crate) type AttemptAxis = (u64, u32);

/// What a workflow's history says about which run owns which attempt.
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

    /// The attempts the named run dispatched.
    pub(crate) fn attempts_of(&self, run_id: &RunId) -> Option<&BTreeSet<AttemptAxis>> {
        self.by_run.get(&run_id.to_string())
    }

    /// Every OTHER run of this workflow that also dispatched `axis`.
    ///
    /// Non-empty means the durable transcript stream for that key is a fusion:
    /// the runs named here wrote into the same stream and nothing in the stored
    /// events distinguishes them.
    pub(crate) fn colliding_runs(&self, run_id: &RunId, axis: AttemptAxis) -> Vec<String> {
        let this = run_id.to_string();
        self.by_run
            .iter()
            .filter(|(run, attempts)| **run != this && attempts.contains(&axis))
            .map(|(run, _)| run.clone())
            .collect()
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
    ) -> Result<TranscriptScope, RunScopeError> {
        let axis = (activity_id.sequence_position(), attempt);
        let attempts = self
            .attempts_of(run_id)
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
        Ok(TranscriptScope {
            colliding_runs: self.colliding_runs(run_id, axis),
        })
    }
}

/// The run-scoping verdict for one validated transcript handle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TranscriptScope {
    /// Other runs of the same workflow that dispatched the same
    /// `(activity, attempt)` and therefore share the durable stream.
    pub(crate) colliding_runs: Vec<String>,
}

impl TranscriptScope {
    /// Whether the stream this handle addresses is shared with another run.
    pub(crate) fn is_ambiguous(&self) -> bool {
        !self.colliding_runs.is_empty()
    }

    /// The operator-facing sentence for an ambiguous read, or `None` when the
    /// stream belongs to this run alone.
    pub(crate) fn warning(&self) -> Option<String> {
        if self.colliding_runs.is_empty() {
            return None;
        }
        Some(format!(
            "this transcript stream is shared with {} other run(s) of the same workflow ({}) \
             because the durable stream key carries no run: the events below may interleave \
             generations and MUST NOT be read as one agent's conversation",
            self.colliding_runs.len(),
            self.colliding_runs.join(", ")
        ))
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

    #[test]
    fn two_generations_dispatching_the_same_ordinal_are_detected_as_a_collision() {
        let first = RunId::new(Uuid::from_u128(11));
        let second = RunId::new(Uuid::from_u128(12));
        let scope = RunScope::from_history(&continue_as_new_chain(&first, &second));

        assert!(scope.knows_run(&first));
        assert!(scope.knows_run(&second));

        let verdict = scope
            .validate_handle(
                &workflow_id(),
                &second,
                &ActivityId::from_sequence_position(0),
                1,
            )
            .ok();
        let verdict = verdict.unwrap_or(super::TranscriptScope {
            colliding_runs: Vec::new(),
        });
        assert!(
            verdict.is_ambiguous(),
            "generation two's ordinal-0 attempt-1 stream is shared with generation one"
        );
        assert_eq!(verdict.colliding_runs, vec![first.to_string()]);
        assert!(
            verdict
                .warning()
                .is_some_and(|warning| warning.contains(&first.to_string()))
        );
    }

    #[test]
    fn a_single_run_workflow_is_never_ambiguous() -> Result<(), RunScopeError> {
        let only = RunId::new(Uuid::from_u128(21));
        let history = vec![started(0, &only), activity_started(1, 3, 2)];
        let scope = RunScope::from_history(&history);
        let verdict = scope.validate_handle(
            &workflow_id(),
            &only,
            &ActivityId::from_sequence_position(3),
            2,
        )?;
        assert!(!verdict.is_ambiguous());
        assert_eq!(verdict.warning(), None);
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
    fn different_attempts_of_one_ordinal_are_different_streams() -> Result<(), RunScopeError> {
        let first = RunId::new(Uuid::from_u128(11));
        let second = RunId::new(Uuid::from_u128(12));
        let mut history = continue_as_new_chain(&first, &second);
        // Generation two retries ordinal 0; attempt 2 exists nowhere else.
        history.push(activity_started(5, 0, 2));
        let scope = RunScope::from_history(&history);
        let verdict = scope.validate_handle(
            &workflow_id(),
            &second,
            &ActivityId::from_sequence_position(0),
            2,
        )?;
        assert!(!verdict.is_ambiguous());
        Ok(())
    }
}
