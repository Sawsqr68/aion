//! Operator-driven redrive of dead-lettered outbox rows.
//!
//! # Why a dead letter is not simply retryable
//!
//! A dead letter is a row whose dispatch spent its whole retry budget on infrastructure failures.
//! Two of them can look identical in the table and mean opposite things:
//!
//! - the failure was **delivered** to the owning workflow, which reacted under its own
//!   retry/failure semantics — that reaction is recorded history, judgment has passed, and
//!   re-running the activity would re-execute possibly non-idempotent work behind a recorded
//!   verdict;
//! - the failure was **never delivered** — no live workflow accepted it, the delivery errored, or
//!   no callback was installed — so the workflow is still waiting on an activity that will never
//!   arrive. Nothing was judged. This is the row redrive exists for.
//!
//! The durable marker [`OutboxRow::failure_delivered`] records which is which at dead-letter time
//! (see [`outbox_dead_letter`](super::outbox_dead_letter)), and the store's own guarded transition
//! enforces it. This module adds the two checks that need workflow HISTORY, which the store cannot
//! see:
//!
//! 1. **Liveness.** A terminal workflow's rows are settled, never re-armed — the same rule the
//!    boot/adoption sweep in [`outbox_settle`](super::outbox_settle) enforces, projected from the
//!    same [`status_from_events`] projection. Redriving a dead letter for a workflow that has
//!    already Completed/Failed/Cancelled/TimedOut would dispatch a zombie round for a dead
//!    workflow, which is precisely the incident that sweep exists to prevent.
//! 2. **Recorded outcome.** The judgment marker is written one store round-trip AFTER the failure
//!    is handed to the workflow, so a crash in that window can leave a judged row marked
//!    unjudged. History is the durable authority that closes it: if the current lease already
//!    records ANY terminal outcome for the row's activity, judgment has passed regardless of what
//!    the marker says.
//!
//! Both checks refuse by default and are overridable only by an explicit
//! [`RedriveMode::Forced`], which is logged at `warn` with the full reason it overrode.
//!
//! # Which gate is atomic, and which is not
//!
//! Stated exactly, because a gate believed stronger than it is is worse than a gate known to be
//! advisory.
//!
//! **The judgment marker is ATOMIC and it is the control.** `failure_delivered` is never read
//! here and acted on there: the store's `redrive_outbox_row` applies `status = 'failed'` and (in
//! eligible mode) `failure_delivered = 0` as guards INSIDE the same transition that performs the
//! write — on haematite, one read-modify-write on the row's own shard actor whose refusal writes
//! nothing. A concurrent dead-letter, re-arm, or second redrive cannot interleave
//! between the test and the write, and the `was_judged` an override reports comes from the
//! store's own pre-state rather than from anything this module observed earlier.
//!
//! **The two history gates above are check-then-act, and structurally cannot be otherwise.** The
//! workflow's event history and the outbox row are separate stores with no shared transaction, so
//! nothing can read history and transition a row under one lock. They are advisory pre-checks
//! that narrow a window, not controls that close it. The residual races, named rather than
//! implied:
//!
//! - a workflow that goes terminal between the history read and the transition leaves a row
//!   re-armed to Pending for a workflow that can never consume it — a zombie row, cleaned up by
//!   the next [`outbox_settle`](super::outbox_settle) sweep on boot or shard adoption, and inert
//!   until then because delivery into a terminal workflow is refused at the delivery seam;
//! - an outcome recorded between the history read and the transition means a redrive proceeds for
//!   an activity judged microseconds ago.
//!
//! Both windows are bounded by one operator-initiated call and neither can corrupt a row: the
//! atomic guard still decides what the row BECOMES. The history gates exist to catch the crash
//! window described in check 2, which the marker alone cannot see — not to be a second lock.

use aion_core::{ActivityId, Event, WorkflowId, WorkflowStatus, status_from_events};
use aion_store::{
    EventStore, OutboxRow, OutboxStore, RedriveMode, RedriveOutcome, RedriveRefusal, StoreError,
};
use chrono::Utc;
use tracing::{info, warn};

use super::outbox_settle::is_settle_terminal;

/// Why a redrive did not happen, or could not be attempted.
///
/// Every variant is an explicit, typed refusal: a redrive request never resolves to a silent
/// no-op, so an operator is never told "done" about work that was not re-queued.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RedriveRefused {
    /// The store refused the transition (absent row, not a dead letter, or already judged).
    #[error(transparent)]
    Row(#[from] RedriveRefusal),
    /// The owning workflow already reached a hard terminal, so its rows must be settled, never
    /// re-armed — re-dispatching one would serve a zombie round for a dead workflow.
    #[error(
        "workflow {workflow_id} is terminal ({status:?}); a terminal workflow's outbox rows are \
         settled, never redriven"
    )]
    WorkflowTerminal {
        /// The workflow that owns the dead letter.
        workflow_id: WorkflowId,
        /// The status projected from its recorded history.
        status: WorkflowStatus,
    },
    /// The workflow's recorded history already carries a terminal outcome for this activity, so
    /// judgment has passed even if the row's judgment marker says otherwise (the crash window
    /// between delivering a failure and recording that it was delivered).
    #[error(
        "workflow {workflow_id} already records a terminal outcome for activity {activity_id} \
         (ordinal {ordinal}); redriving it would re-execute an activity whose outcome is recorded \
         history"
    )]
    HistoryRecordsOutcome {
        /// The workflow that owns the dead letter.
        workflow_id: WorkflowId,
        /// The activity the row dispatches.
        activity_id: ActivityId,
        /// The row's fan-out ordinal.
        ordinal: u64,
    },
    /// The store or history read failed; nothing was redriven.
    #[error("redrive could not read durable state: {0}")]
    Store(#[from] StoreError),
}

/// List every dead-lettered outbox row of `workflow_id` — the operator's discovery surface.
///
/// Each row carries [`OutboxRow::failure_delivered`], so an operator can see which dead letters are
/// redrivable and which were already judged before choosing to act.
///
/// # Errors
///
/// Returns [`StoreError`] when the enumeration fails at the backend boundary.
pub async fn list_dead_letters(
    outbox_store: &dyn OutboxStore,
    workflow_id: &WorkflowId,
) -> Result<Vec<OutboxRow>, StoreError> {
    outbox_store
        .list_dead_lettered_outbox_rows(workflow_id)
        .await
}

/// Return the dead-lettered row for `(workflow_id, ordinal)` to the pending claim path.
///
/// Applies, in order: the workflow-liveness gate, the recorded-outcome gate, and then the store's
/// own status-guarded transition (which enforces the dead-letter status and the judgment marker
/// inside its own atomic operation). Every refusal is typed and logged; a successful redrive is
/// logged with the workflow, ordinal, and mode.
///
/// # Errors
///
/// Returns [`RedriveRefused`] when any gate refuses, when the store refuses the transition, or when
/// durable state could not be read. Nothing is written unless the row actually moves.
pub async fn redrive_dead_lettered_row(
    event_store: &dyn EventStore,
    outbox_store: &dyn OutboxStore,
    workflow_id: &WorkflowId,
    ordinal: u64,
    mode: RedriveMode,
) -> Result<OutboxRow, RedriveRefused> {
    let dispatch_key = OutboxRow::dispatch_key_for(workflow_id, ordinal);
    let history = event_store.read_history(workflow_id).await?;

    // Gate 1 — liveness. Identical rule and projection to the terminal-workflow settlement sweep:
    // a terminal workflow's rows are retired, never re-armed. Not overridable: forcing a dispatch
    // for a workflow that can never consume it is not an operator decision, it is a zombie round.
    let status = status_from_events(&history);
    if is_settle_terminal(status) {
        warn!(
            workflow_id = %workflow_id,
            ordinal,
            projected_status = ?status,
            "refusing outbox redrive: the owning workflow is terminal"
        );
        return Err(RedriveRefused::WorkflowTerminal {
            workflow_id: workflow_id.clone(),
            status,
        });
    }

    // Gate 2 — recorded outcome. History is the durable authority on whether judgment passed, and
    // it closes the crash window between delivering a dead letter's failure and recording that the
    // delivery landed.
    let activity_id = ActivityId::from_sequence_position(ordinal);
    if history_records_outcome(&history, &activity_id) {
        if !mode.admits_judged() {
            warn!(
                workflow_id = %workflow_id,
                ordinal,
                activity_id = %activity_id,
                "refusing outbox redrive: history already records a terminal outcome for this activity"
            );
            return Err(RedriveRefused::HistoryRecordsOutcome {
                workflow_id: workflow_id.clone(),
                activity_id,
                ordinal,
            });
        }
        warn!(
            workflow_id = %workflow_id,
            ordinal,
            activity_id = %activity_id,
            "FORCED outbox redrive of an activity whose terminal outcome is already recorded \
             history: the activity will run again behind a recorded judgment"
        );
    }

    match outbox_store
        .redrive_outbox_row(&dispatch_key, Utc::now(), mode)
        .await?
    {
        RedriveOutcome::Redriven { row, was_judged } => {
            if was_judged {
                warn!(
                    dispatch_key = %dispatch_key,
                    workflow_id = %workflow_id,
                    ordinal,
                    "FORCED outbox redrive of a dead letter whose failure was already delivered to \
                     the workflow"
                );
            }
            info!(
                dispatch_key = %dispatch_key,
                workflow_id = %workflow_id,
                ordinal,
                mode = ?mode,
                "outbox dead letter redriven to pending"
            );
            Ok(*row)
        }
        RedriveOutcome::Refused(refusal) => {
            warn!(
                dispatch_key = %dispatch_key,
                workflow_id = %workflow_id,
                ordinal,
                refusal = %refusal,
                "outbox redrive refused by the store"
            );
            Err(RedriveRefused::Row(refusal))
        }
    }
}

/// Whether the workflow's CURRENT lease already records a terminal outcome for `activity_id`.
///
/// The lease is everything after the last `WorkflowStarted`/`WorkflowReopened` — the same scoping
/// the failed-step projection uses — because a continue-as-new or reopened chain reuses ordinals,
/// so a prior lease's recorded outcome says nothing about this row.
fn history_records_outcome(history: &[Event], activity_id: &ActivityId) -> bool {
    let lease_start = history
        .iter()
        .rposition(|event| {
            matches!(
                event,
                Event::WorkflowStarted { .. } | Event::WorkflowReopened { .. }
            )
        })
        .map_or(0, |index| index + 1);
    history[lease_start..].iter().any(|event| {
        matches!(
            event,
            Event::ActivityFailed { activity_id: id, .. }
                | Event::ActivityCompleted { activity_id: id, .. }
                | Event::ActivityCancelled { activity_id: id, .. }
            if id == activity_id
        )
    })
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashSet};
    use std::sync::Arc;

    use aion_core::{
        ActivityError, ActivityErrorKind, ActivityId, ContentType, Event, EventEnvelope,
        PackageVersion, Payload, RunId, WorkflowId,
    };
    use aion_store::{
        ClaimScope, InMemoryStore, OutboxRow, OutboxStore, RedriveMode, StoreError,
        WritableEventStore, WriteToken,
    };
    use chrono::{DateTime, Utc};

    use super::{RedriveRefused, history_records_outcome, redrive_dead_lettered_row};

    /// An outbox store that refuses EVERY call.
    ///
    /// The gates in this module must decide before the store is ever touched, so a test that
    /// expects a gate refusal proves it by handing the redrive a store that cannot be used without
    /// the failure showing up in the result.
    #[derive(Debug, Default)]
    struct RefusingOutbox;

    impl RefusingOutbox {
        fn refusal<T>() -> Result<T, StoreError> {
            Err(StoreError::Backend(String::from(
                "the redrive gates must refuse before touching the outbox store",
            )))
        }
    }

    #[async_trait::async_trait]
    impl OutboxStore for RefusingOutbox {
        async fn append_outbox_batch(&self, _rows: &[OutboxRow]) -> Result<(), StoreError> {
            Self::refusal()
        }

        async fn claim_outbox_rows(&self, _limit: u32) -> Result<Vec<OutboxRow>, StoreError> {
            Self::refusal()
        }

        async fn claim_outbox_rows_scoped(
            &self,
            _scope: &ClaimScope,
            _limit: u32,
        ) -> Result<Vec<OutboxRow>, StoreError> {
            Self::refusal()
        }

        async fn rearm_stale_claimed_outbox_rows(
            &self,
            _older_than: DateTime<Utc>,
            _visible_after: DateTime<Utc>,
            _limit: u32,
            _excluded: &HashSet<String>,
        ) -> Result<Vec<OutboxRow>, StoreError> {
            Self::refusal()
        }

        async fn complete_outbox_row(&self, _dispatch_key: &str) -> Result<(), StoreError> {
            Self::refusal()
        }

        async fn retry_outbox_row(
            &self,
            _dispatch_key: &str,
            _next_attempt: u32,
            _visible_after: DateTime<Utc>,
        ) -> Result<(), StoreError> {
            Self::refusal()
        }

        async fn fail_outbox_row(&self, _dispatch_key: &str) -> Result<(), StoreError> {
            Self::refusal()
        }

        async fn count_inflight_outbox_rows(&self, _namespace: &str) -> Result<u64, StoreError> {
            Self::refusal()
        }

        async fn count_claimed_outbox_rows(&self, _namespace: &str) -> Result<u64, StoreError> {
            Self::refusal()
        }

        async fn count_claimed_outbox_rows_by_namespace(
            &self,
            _namespaces: &[&str],
        ) -> Result<BTreeMap<String, u64>, StoreError> {
            Self::refusal()
        }

        async fn pending_outbox_routes(&self) -> Result<Vec<ClaimScope>, StoreError> {
            Self::refusal()
        }
    }

    /// A redrive that was supposed to refuse but did not: report it as a typed failure rather
    /// than unwrapping.
    fn refused_expected(detail: &str) -> StoreError {
        StoreError::Backend(format!("redrive contract violated: {detail}"))
    }

    fn envelope(workflow_id: &WorkflowId, seq: u64) -> EventEnvelope {
        EventEnvelope {
            seq,
            recorded_at: Utc::now(),
            workflow_id: workflow_id.clone(),
        }
    }

    fn started(workflow_id: &WorkflowId, seq: u64) -> Event {
        Event::WorkflowStarted {
            envelope: envelope(workflow_id, seq),
            workflow_type: String::from("charge"),
            input: Payload::new(ContentType::Json, b"{}".to_vec()),
            run_id: RunId::new_v4(),
            parent_run_id: None,
            parent_workflow_id: None,
            package_version: PackageVersion::new("a".repeat(64)),
        }
    }

    fn failed_activity(workflow_id: &WorkflowId, seq: u64, ordinal: u64) -> Event {
        Event::ActivityFailed {
            envelope: envelope(workflow_id, seq),
            activity_id: ActivityId::from_sequence_position(ordinal),
            error: ActivityError {
                kind: ActivityErrorKind::Terminal,
                message: String::from("infrastructure: delivery to worker failed"),
                details: None,
            },
            attempt: 1,
        }
    }

    fn completed_workflow(workflow_id: &WorkflowId, seq: u64) -> Event {
        Event::WorkflowCompleted {
            envelope: envelope(workflow_id, seq),
            result: Payload::new(ContentType::Json, b"{}".to_vec()),
        }
    }

    async fn store_with(events: Vec<Event>) -> Result<Arc<InMemoryStore>, StoreError> {
        let store = Arc::new(InMemoryStore::default());
        let Some(first) = events.first() else {
            return Ok(store);
        };
        let workflow_id = first.workflow_id().clone();
        store
            .append(WriteToken::recorder(), &workflow_id, &events, 0)
            .await?;
        Ok(store)
    }

    #[tokio::test]
    async fn a_terminal_workflow_is_refused_before_the_store_is_touched() -> Result<(), StoreError>
    {
        let workflow_id = WorkflowId::new_v4();
        let store = store_with(vec![
            started(&workflow_id, 1),
            completed_workflow(&workflow_id, 2),
        ])
        .await?;

        let outcome = redrive_dead_lettered_row(
            store.as_ref(),
            &RefusingOutbox,
            &workflow_id,
            0,
            // Even a FORCED redrive may not serve a zombie round for a dead workflow.
            RedriveMode::Forced,
        )
        .await;
        let Err(refusal) = outcome else {
            return Err(refused_expected(
                "a terminal workflow's dead letter must never redrive",
            ));
        };
        assert!(
            matches!(refusal, RedriveRefused::WorkflowTerminal { .. }),
            "expected a WorkflowTerminal refusal, got {refusal:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_recorded_activity_outcome_is_refused_before_the_store_is_touched()
    -> Result<(), StoreError> {
        let workflow_id = WorkflowId::new_v4();
        let store = store_with(vec![
            started(&workflow_id, 1),
            failed_activity(&workflow_id, 2, 0),
        ])
        .await?;

        let outcome = redrive_dead_lettered_row(
            store.as_ref(),
            &RefusingOutbox,
            &workflow_id,
            0,
            RedriveMode::Eligible,
        )
        .await;
        let Err(refusal) = outcome else {
            return Err(refused_expected(
                "history that already records the activity's failure must refuse the redrive",
            ));
        };
        assert!(
            matches!(refusal, RedriveRefused::HistoryRecordsOutcome { .. }),
            "expected a HistoryRecordsOutcome refusal, got {refusal:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_forced_redrive_passes_the_recorded_outcome_gate() -> Result<(), StoreError> {
        let workflow_id = WorkflowId::new_v4();
        let store = store_with(vec![
            started(&workflow_id, 1),
            failed_activity(&workflow_id, 2, 0),
        ])
        .await?;

        // The refusing store proves the call reached it: the force overrode the history gate, and
        // the ONLY way to observe a store error here is to have passed that gate.
        let outcome = redrive_dead_lettered_row(
            store.as_ref(),
            &RefusingOutbox,
            &workflow_id,
            0,
            RedriveMode::Forced,
        )
        .await;
        let Err(refusal) = outcome else {
            return Err(refused_expected(
                "the refusing store must surface its error",
            ));
        };
        assert!(
            matches!(refusal, RedriveRefused::Store(_)),
            "a forced redrive must reach the store, got {refusal:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn an_unknown_workflow_reaches_the_store_and_is_refused_there() -> Result<(), StoreError>
    {
        // No history at all: nothing is terminal and nothing is recorded, so both gates pass and
        // the store's own status guard is the authority (here, the refusing double).
        let store = store_with(Vec::new()).await?;
        let outcome = redrive_dead_lettered_row(
            store.as_ref(),
            &RefusingOutbox,
            &WorkflowId::new_v4(),
            0,
            RedriveMode::Eligible,
        )
        .await;
        let Err(refusal) = outcome else {
            return Err(refused_expected(
                "the refusing store must surface its error",
            ));
        };
        assert!(
            matches!(refusal, RedriveRefused::Store(_)),
            "expected the store to be consulted, got {refusal:?}"
        );
        Ok(())
    }

    #[test]
    fn a_recorded_outcome_matches_only_the_row_s_own_activity() {
        let workflow_id = WorkflowId::new_v4();
        let history = vec![
            started(&workflow_id, 1),
            failed_activity(&workflow_id, 2, 3),
        ];
        assert!(history_records_outcome(
            &history,
            &ActivityId::from_sequence_position(3)
        ));
        assert!(!history_records_outcome(
            &history,
            &ActivityId::from_sequence_position(4)
        ));
    }

    #[test]
    fn a_prior_lease_s_outcome_never_judges_the_current_lease() {
        let workflow_id = WorkflowId::new_v4();
        // Ordinals are reused across leases, so a failure recorded BEFORE the current lease's
        // WorkflowStarted says nothing about this row.
        let history = vec![
            started(&workflow_id, 1),
            failed_activity(&workflow_id, 2, 0),
            started(&workflow_id, 3),
        ];
        assert!(!history_records_outcome(
            &history,
            &ActivityId::from_sequence_position(0)
        ));
    }

    #[test]
    fn a_completed_or_cancelled_activity_also_counts_as_judged() {
        let workflow_id = WorkflowId::new_v4();
        let activity_id = ActivityId::from_sequence_position(0);
        for terminal in [
            Event::ActivityCompleted {
                envelope: envelope(&workflow_id, 2),
                activity_id: activity_id.clone(),
                result: Payload::new(ContentType::Json, b"{}".to_vec()),
                attempt: 1,
            },
            Event::ActivityCancelled {
                envelope: envelope(&workflow_id, 2),
                activity_id: activity_id.clone(),
                attempt: 1,
            },
        ] {
            let history = vec![started(&workflow_id, 1), terminal];
            assert!(history_records_outcome(&history, &activity_id));
        }
    }

    #[test]
    fn a_scheduled_but_unfinished_activity_is_not_judged() {
        let workflow_id = WorkflowId::new_v4();
        let activity_id = ActivityId::from_sequence_position(0);
        let history = vec![
            started(&workflow_id, 1),
            Event::ActivityScheduled {
                envelope: envelope(&workflow_id, 2),
                activity_id: activity_id.clone(),
                activity_type: String::from("charge"),
                input: Payload::new(ContentType::Json, b"{}".to_vec()),
                task_queue: String::from("default"),
                node: None,
            },
        ];
        assert!(!history_records_outcome(&history, &activity_id));
    }
}
