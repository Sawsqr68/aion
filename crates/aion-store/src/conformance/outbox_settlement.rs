//! Workflow-terminal outbox settlement scenarios (#253) for outbox-bearing stores.
//!
//! Generic over the concrete store (not `Arc<dyn EventStore>`) because the
//! contract under test spans BOTH the [`OutboxStore`] surface (settle twin,
//! enumeration, stale probe, re-arm pins) and the [`WritableEventStore`]
//! writer seam (`settle_workflow_outbox_rows_cancelled`,
//! `rearm_outbox_pending`), which no single trait object carries.

use chrono::{Duration, Utc};

use crate::{
    OutboxRow, OutboxStatus, OutboxStore, RedriveMode, RedriveOutcome, RedriveRefusal, StoreError,
    WorkflowId, WritableEventStore,
};

use super::contract_error;

fn pending_row(workflow_id: &WorkflowId, ordinal: u64) -> Result<OutboxRow, StoreError> {
    Ok(OutboxRow::pending(
        workflow_id.clone(),
        ordinal,
        String::from("charge"),
        aion_core::Payload::from_json(&serde_json::json!({ "ordinal": ordinal }))
            .map_err(|error| StoreError::Serialization(error.to_string()))?,
        Utc::now(),
    ))
}

/// Append one pending row and immediately claim it (it is the only claimable
/// row at that instant, so the claim is deterministic).
async fn append_and_claim<S>(
    store: &S,
    workflow_id: &WorkflowId,
    ordinal: u64,
) -> Result<OutboxRow, StoreError>
where
    S: OutboxStore + WritableEventStore,
{
    store
        .append_outbox_batch(&[pending_row(workflow_id, ordinal)?])
        .await?;
    let claimed = store.claim_outbox_rows(1).await?;
    match claimed.into_iter().next() {
        Some(row) if row.ordinal == ordinal && &row.workflow_id == workflow_id => Ok(row),
        other => Err(contract_error(&format!(
            "expected to claim the just-appended row (ordinal {ordinal}), got {other:?}"
        ))),
    }
}

/// The core settle contract: only the target workflow's live (Pending|Claimed)
/// rows flip to Cancelled; Done/Failed rows and other workflows' rows are
/// untouched; the settled keys come back; the settle is idempotent; and the
/// unsettled-workflow enumeration reflects it.
pub(super) async fn settle_flips_only_live_rows_and_is_idempotent<S>(
    store: S,
) -> Result<(), StoreError>
where
    S: OutboxStore + WritableEventStore,
{
    let dead = super::workflow_id();
    let live = super::workflow_id();

    // dead:0 → Done, dead:1 → Failed (terminal rows the settle must not touch).
    let done = append_and_claim(&store, &dead, 0).await?;
    store.complete_outbox_row(&done.dispatch_key).await?;
    let failed = append_and_claim(&store, &dead, 1).await?;
    store.fail_outbox_row(&failed.dispatch_key).await?;
    // dead:2 → Claimed, dead:3/4 → Pending, live:0 → Pending.
    let claimed = append_and_claim(&store, &dead, 2).await?;
    store
        .append_outbox_batch(&[
            pending_row(&dead, 3)?,
            pending_row(&dead, 4)?,
            pending_row(&live, 0)?,
        ])
        .await?;

    let unsettled = store.list_unsettled_outbox_workflow_ids().await?;
    for workflow in [&dead, &live] {
        if !unsettled.contains(workflow) {
            return Err(contract_error(
                "both workflows own live rows, so both must enumerate as unsettled",
            ));
        }
    }

    let mut settled = store.cancel_outbox_rows_for_workflow(&dead).await?;
    settled.sort();
    let mut expected = vec![
        claimed.dispatch_key.clone(),
        OutboxRow::dispatch_key_for(&dead, 3),
        OutboxRow::dispatch_key_for(&dead, 4),
    ];
    expected.sort();
    super::expect_eq(
        settled,
        expected,
        "the settle must return exactly the live (Pending|Claimed) keys it retired",
    )?;

    // Idempotent: a second settle finds nothing live.
    super::expect_empty(
        store.cancel_outbox_rows_for_workflow(&dead).await?,
        "a second settle of the same workflow must retire nothing",
    )?;

    // The other workflow's row is untouched and still the ONLY claimable row.
    let claimable = store.claim_outbox_rows(16).await?;
    super::expect_eq(
        claimable
            .iter()
            .map(|row| row.dispatch_key.clone())
            .collect::<Vec<_>>(),
        vec![OutboxRow::dispatch_key_for(&live, 0)],
        "after the settle only the live workflow's row may be claimable — \
         Cancelled/Done/Failed rows must never be claimed",
    )?;

    // Enumeration now sees only the (re-claimed) live workflow.
    let unsettled = store.list_unsettled_outbox_workflow_ids().await?;
    super::expect_eq(
        unsettled,
        vec![live],
        "after the settle only the live workflow may own unsettled rows",
    )
}

/// The stale-claim probe is read-only and selects exactly what the re-arm
/// would take.
pub(super) async fn stale_probe_is_readonly_and_matches_rearm_selection<S>(
    store: S,
) -> Result<(), StoreError>
where
    S: OutboxStore + WritableEventStore,
{
    let workflow = super::workflow_id();
    let first = append_and_claim(&store, &workflow, 0).await?;
    let second = append_and_claim(&store, &workflow, 1).await?;
    // Both claims happened "now"; probe with a threshold in the future so both
    // are stale relative to it.
    let older_than = Utc::now() + Duration::hours(1);

    let keys_of = |rows: &[OutboxRow]| {
        rows.iter()
            .map(|row| row.dispatch_key.clone())
            .collect::<Vec<_>>()
    };
    let probed = store.list_stale_claimed_outbox_rows(older_than, 16).await?;
    let probed_again = store.list_stale_claimed_outbox_rows(older_than, 16).await?;
    super::expect_eq(
        keys_of(&probed),
        keys_of(&probed_again),
        "the stale probe must be read-only: probing twice must observe the same rows",
    )?;

    let rearmed = store
        .rearm_stale_claimed_outbox_rows(
            older_than,
            Utc::now(),
            16,
            &std::collections::HashSet::new(),
        )
        .await?;
    let mut rearmed_keys = keys_of(&rearmed);
    rearmed_keys.sort();
    let mut expected = vec![first.dispatch_key, second.dispatch_key];
    expected.sort();
    let mut selected_keys = keys_of(&probed);
    selected_keys.sort();
    super::expect_eq(
        selected_keys,
        expected.clone(),
        "the probe must select exactly the stale claimed rows",
    )?;
    super::expect_eq(
        rearmed_keys,
        expected,
        "the re-arm must take exactly the probe's selection",
    )?;
    super::expect_empty(
        store.list_stale_claimed_outbox_rows(older_than, 16).await?,
        "after the re-arm no stale claimed row may remain",
    )
}

/// A live delivery key is excluded inside the same status-guarded operation
/// that re-arms other stale claims. The excluded row remains Claimed while a
/// non-excluded peer re-arms to Pending.
pub(super) async fn stale_rearm_excludes_live_delivery_keys<S>(store: S) -> Result<(), StoreError>
where
    S: OutboxStore + WritableEventStore,
{
    let workflow = super::workflow_id();
    let held = append_and_claim(&store, &workflow, 0).await?;
    let recoverable = append_and_claim(&store, &workflow, 1).await?;
    let older_than = Utc::now() + Duration::hours(1);
    let excluded = std::collections::HashSet::from([held.dispatch_key.clone()]);

    let rearmed = store
        .rearm_stale_claimed_outbox_rows(older_than, Utc::now(), 16, &excluded)
        .await?;
    super::expect_eq(
        rearmed
            .iter()
            .map(|row| row.dispatch_key.clone())
            .collect::<Vec<_>>(),
        vec![recoverable.dispatch_key.clone()],
        "the non-excluded stale row must re-arm while the live delivery stays claimed",
    )?;

    let still_stale = store.list_stale_claimed_outbox_rows(older_than, 16).await?;
    super::expect_eq(
        still_stale
            .iter()
            .map(|row| row.dispatch_key.clone())
            .collect::<Vec<_>>(),
        vec![held.dispatch_key],
        "the excluded live delivery must remain Claimed after the guarded re-arm",
    )?;
    let claimed = store.claim_outbox_rows(16).await?;
    super::expect_eq(
        claimed
            .iter()
            .map(|row| row.dispatch_key.clone())
            .collect::<Vec<_>>(),
        vec![recoverable.dispatch_key],
        "only the non-excluded row may return to the pending claim path",
    )
}

/// Pin: neither the stale re-arm nor the claim path ever touches a Cancelled
/// row — the contract the reconciler's settle-then-rearm ordering depends on.
pub(super) async fn rearm_and_claim_never_touch_cancelled_rows<S>(
    store: S,
) -> Result<(), StoreError>
where
    S: OutboxStore + WritableEventStore,
{
    let workflow = super::workflow_id();
    let _claimed = append_and_claim(&store, &workflow, 0).await?;
    let settled = store.cancel_outbox_rows_for_workflow(&workflow).await?;
    super::expect_eq(
        settled,
        vec![OutboxRow::dispatch_key_for(&workflow, 0)],
        "the claimed row must settle to Cancelled",
    )?;

    super::expect_empty(
        store
            .rearm_stale_claimed_outbox_rows(
                Utc::now() + Duration::hours(1),
                Utc::now(),
                16,
                &std::collections::HashSet::new(),
            )
            .await?,
        "the stale re-arm must never resurrect a Cancelled row",
    )?;
    super::expect_empty(
        store.claim_outbox_rows(16).await?,
        "the claim path must never claim a Cancelled row",
    )
}

/// Reopen interplay (#253-I9): `rearm_outbox_pending` — the reopen/recovery
/// re-stage — forcibly returns ANY existing row, including a Cancelled one, to
/// Pending, so a reopened workflow's re-dispatches still deliver after its
/// earlier terminal settled them.
pub(super) async fn reopen_rearm_resurrects_a_cancelled_row<S>(store: S) -> Result<(), StoreError>
where
    S: OutboxStore + WritableEventStore,
{
    let workflow = super::workflow_id();
    store
        .append_outbox_batch(&[pending_row(&workflow, 0)?])
        .await?;
    let settled = store.cancel_outbox_rows_for_workflow(&workflow).await?;
    super::expect_eq(
        settled,
        vec![OutboxRow::dispatch_key_for(&workflow, 0)],
        "the pending row must settle to Cancelled",
    )?;

    store
        .rearm_outbox_pending(&[pending_row(&workflow, 0)?])
        .await?;
    let claimed = store.claim_outbox_rows(16).await?;
    super::expect_eq(
        claimed
            .iter()
            .map(|row| row.dispatch_key.clone())
            .collect::<Vec<_>>(),
        vec![OutboxRow::dispatch_key_for(&workflow, 0)],
        "rearm_outbox_pending must resurrect the Cancelled row to claimable Pending \
         (reopen supersedes the terminal settle)",
    )
}

/// Dead-letter a claimed row and return it.
async fn append_claim_and_dead_letter<S>(
    store: &S,
    workflow_id: &WorkflowId,
    ordinal: u64,
) -> Result<OutboxRow, StoreError>
where
    S: OutboxStore + WritableEventStore,
{
    let row = append_and_claim(store, workflow_id, ordinal).await?;
    store.fail_outbox_row(&row.dispatch_key).await?;
    Ok(row)
}

fn keys_of(rows: &[OutboxRow]) -> Vec<String> {
    rows.iter()
        .map(|row| row.dispatch_key.clone())
        .collect::<Vec<_>>()
}

/// Redrive returns an UNJUDGED dead letter to the pending claim path with its attempt budget reset,
/// and the redriven row is claimable again.
pub(super) async fn redrive_returns_an_unjudged_dead_letter_to_pending<S>(
    store: S,
) -> Result<(), StoreError>
where
    S: OutboxStore + WritableEventStore,
{
    let workflow = super::workflow_id();
    let row = append_and_claim(&store, &workflow, 0).await?;
    // Spend part of the retry budget, then dead-letter the row from a claimed state — the exact
    // shape the dispatcher produces when delivery fails on the final attempt.
    store
        .retry_outbox_row(&row.dispatch_key, 4, Utc::now())
        .await?;
    let claimed = append_and_claim_existing(&store, &row.dispatch_key).await?;
    super::expect_eq(
        claimed.attempt,
        4,
        "the re-claimed row must carry the spent attempt budget",
    )?;
    store.fail_outbox_row(&row.dispatch_key).await?;

    let outcome = store
        .redrive_outbox_row(&row.dispatch_key, Utc::now(), RedriveMode::Eligible)
        .await?;
    let redriven = match outcome {
        RedriveOutcome::Redriven { row, was_judged } => {
            if was_judged {
                return Err(contract_error(
                    "an unjudged dead letter must not report a judged redrive",
                ));
            }
            *row
        }
        RedriveOutcome::Refused(refusal) => {
            return Err(contract_error(&format!(
                "an unjudged dead letter must redrive, got refusal: {refusal}"
            )));
        }
    };
    super::expect_eq(
        redriven.status,
        OutboxStatus::Pending,
        "a redriven row must return to Pending",
    )?;
    super::expect_eq(
        redriven.attempt,
        0,
        "a redriven row must have its attempt budget reset",
    )?;
    if redriven.claimed_at.is_some() {
        return Err(contract_error(
            "a redriven row must have its claim instant cleared",
        ));
    }

    super::expect_eq(
        keys_of(&store.claim_outbox_rows(16).await?),
        vec![row.dispatch_key.clone()],
        "the redriven row must be claimable again",
    )?;
    super::expect_empty(
        store.list_dead_lettered_outbox_rows(&workflow).await?,
        "a redriven row must no longer enumerate as a dead letter",
    )
}

/// Claim the single claimable row and assert it is `dispatch_key`.
async fn append_and_claim_existing<S>(
    store: &S,
    dispatch_key: &str,
) -> Result<OutboxRow, StoreError>
where
    S: OutboxStore + WritableEventStore,
{
    let claimed = store.claim_outbox_rows(1).await?;
    match claimed.into_iter().next() {
        Some(row) if row.dispatch_key == dispatch_key => Ok(row),
        other => Err(contract_error(&format!(
            "expected to re-claim {dispatch_key}, got {other:?}"
        ))),
    }
}

/// The status guard: redrive moves a row ONLY from the dead-lettered state. A `Done`, `Cancelled`,
/// or live `Claimed` row is refused with a typed reason and left byte-identical, and an unknown key
/// is an explicit refusal rather than a silent no-op.
pub(super) async fn redrive_refuses_every_non_dead_lettered_row<S>(
    store: S,
) -> Result<(), StoreError>
where
    S: OutboxStore + WritableEventStore,
{
    let workflow = super::workflow_id();
    let done = append_and_claim(&store, &workflow, 0).await?;
    store.complete_outbox_row(&done.dispatch_key).await?;
    let cancelled = append_and_claim(&store, &workflow, 1).await?;
    store.cancel_outbox_rows_for_workflow(&workflow).await?;
    let live = append_and_claim(&store, &workflow, 2).await?;

    for (dispatch_key, expected) in [
        (done.dispatch_key.clone(), OutboxStatus::Done),
        (cancelled.dispatch_key.clone(), OutboxStatus::Cancelled),
        (live.dispatch_key.clone(), OutboxStatus::Claimed),
    ] {
        match store
            .redrive_outbox_row(&dispatch_key, Utc::now(), RedriveMode::Forced)
            .await?
        {
            RedriveOutcome::Refused(RedriveRefusal::NotDeadLettered { status, .. })
                if status == expected => {}
            other => {
                return Err(contract_error(&format!(
                    "redrive of a '{expected}' row must be refused as NotDeadLettered, got {other:?}"
                )));
            }
        }
    }

    // Not even a forced redrive may resurrect any of them.
    super::expect_empty(
        store.claim_outbox_rows(16).await?,
        "no Done/Cancelled/Claimed row may become claimable through a redrive",
    )?;

    let absent = OutboxRow::dispatch_key_for(&super::workflow_id(), 7);
    match store
        .redrive_outbox_row(&absent, Utc::now(), RedriveMode::Forced)
        .await?
    {
        RedriveOutcome::Refused(RedriveRefusal::NoSuchRow { dispatch_key }) => super::expect_eq(
            dispatch_key,
            absent,
            "the NoSuchRow refusal must name the key it could not find",
        ),
        other => Err(contract_error(&format!(
            "redrive of an unknown key must be an explicit NoSuchRow refusal, got {other:?}"
        ))),
    }
}

/// THE judgment gate: a dead letter whose failure was DELIVERED to the workflow is refused by an
/// eligible redrive (the workflow already reacted; re-running would re-execute an activity whose
/// failure is recorded history) and moves only under an explicit forced redrive. A dead letter
/// whose failure was NOT delivered redrives normally.
pub(super) async fn redrive_refuses_a_judged_dead_letter_unless_forced<S>(
    store: S,
) -> Result<(), StoreError>
where
    S: OutboxStore + WritableEventStore,
{
    let workflow = super::workflow_id();
    let judged = append_claim_and_dead_letter(&store, &workflow, 0).await?;
    if !store
        .record_outbox_failure_delivered(&judged.dispatch_key)
        .await?
    {
        return Err(contract_error(
            "the judgment marker must be recorded on a dead-lettered row",
        ));
    }

    let dead_letters = store.list_dead_lettered_outbox_rows(&workflow).await?;
    match dead_letters.as_slice() {
        [row] if row.failure_delivered => {}
        other => {
            return Err(contract_error(&format!(
                "the dead-letter enumeration must report the judgment marker, got {other:?}"
            )));
        }
    }

    match store
        .redrive_outbox_row(&judged.dispatch_key, Utc::now(), RedriveMode::Eligible)
        .await?
    {
        RedriveOutcome::Refused(RedriveRefusal::AlreadyJudged { .. }) => {}
        other => {
            return Err(contract_error(&format!(
                "an eligible redrive must refuse a judged dead letter, got {other:?}"
            )));
        }
    }
    super::expect_empty(
        store.claim_outbox_rows(16).await?,
        "a refused redrive must leave the judged dead letter unclaimable",
    )?;

    match store
        .redrive_outbox_row(&judged.dispatch_key, Utc::now(), RedriveMode::Forced)
        .await?
    {
        // The store reports the PRE-state judgment so a forced override is auditable: the
        // post-state row always has the marker cleared.
        RedriveOutcome::Redriven { row, was_judged } if !row.failure_delivered && was_judged => {}
        other => {
            return Err(contract_error(&format!(
                "a forced redrive must move the judged row, clear its marker, and report that it                  was judged, got {other:?}"
            )));
        }
    }
    super::expect_eq(
        keys_of(&store.claim_outbox_rows(16).await?),
        vec![judged.dispatch_key],
        "the forcibly redriven row must be claimable again",
    )
}

/// The judgment marker is status-guarded and reset by a fresh dead letter.
pub(super) async fn judgment_marker_is_status_guarded_and_reset_by_a_new_dead_letter<S>(
    store: S,
) -> Result<(), StoreError>
where
    S: OutboxStore + WritableEventStore,
{
    let workflow = super::workflow_id();
    let live = append_and_claim(&store, &workflow, 0).await?;
    if store
        .record_outbox_failure_delivered(&live.dispatch_key)
        .await?
    {
        return Err(contract_error(
            "the judgment marker must never be recorded on a live (Claimed) row",
        ));
    }
    let absent = OutboxRow::dispatch_key_for(&super::workflow_id(), 9);
    if store.record_outbox_failure_delivered(&absent).await? {
        return Err(contract_error(
            "the judgment marker must report false for an unknown dispatch key",
        ));
    }

    // Mark it, redrive it, dead-letter it again: the new dead letter starts a fresh judgment cycle.
    store.fail_outbox_row(&live.dispatch_key).await?;
    if !store
        .record_outbox_failure_delivered(&live.dispatch_key)
        .await?
    {
        return Err(contract_error(
            "the judgment marker must be recorded on the dead-lettered row",
        ));
    }
    match store
        .redrive_outbox_row(&live.dispatch_key, Utc::now(), RedriveMode::Forced)
        .await?
    {
        RedriveOutcome::Redriven { was_judged, .. } if was_judged => {}
        other => {
            return Err(contract_error(&format!(
                "the forced redrive must move the judged row and report it as judged, got {other:?}"
            )));
        }
    }
    store.fail_outbox_row(&live.dispatch_key).await?;
    let dead_letters = store.list_dead_lettered_outbox_rows(&workflow).await?;
    match dead_letters.as_slice() {
        [row] if !row.failure_delivered => {}
        other => {
            return Err(contract_error(&format!(
                "a fresh dead letter must clear the earlier judgment marker, got {other:?}"
            )));
        }
    }
    // …and therefore redrives again without a force.
    match store
        .redrive_outbox_row(&live.dispatch_key, Utc::now(), RedriveMode::Eligible)
        .await?
    {
        RedriveOutcome::Redriven { was_judged, .. } if !was_judged => Ok(()),
        other => Err(contract_error(&format!(
            "the re-dead-lettered row must be eligible for redrive again as unjudged, got {other:?}"
        ))),
    }
}

/// The dead-letter enumeration reports exactly the target workflow's `Failed` rows.
pub(super) async fn dead_letter_enumeration_is_scoped_to_the_workflow_and_to_failed_rows<S>(
    store: S,
) -> Result<(), StoreError>
where
    S: OutboxStore + WritableEventStore,
{
    let workflow = super::workflow_id();
    let other = super::workflow_id();
    let first = append_claim_and_dead_letter(&store, &workflow, 1).await?;
    let second = append_claim_and_dead_letter(&store, &workflow, 0).await?;
    let done = append_and_claim(&store, &workflow, 2).await?;
    store.complete_outbox_row(&done.dispatch_key).await?;
    let foreign = append_claim_and_dead_letter(&store, &other, 0).await?;
    store
        .append_outbox_batch(&[pending_row(&workflow, 3)?])
        .await?;

    super::expect_eq(
        keys_of(&store.list_dead_lettered_outbox_rows(&workflow).await?),
        vec![second.dispatch_key, first.dispatch_key],
        "the enumeration must return only this workflow's dead letters, ordered by ordinal",
    )?;
    super::expect_eq(
        keys_of(&store.list_dead_lettered_outbox_rows(&other).await?),
        vec![foreign.dispatch_key],
        "another workflow's dead letters must never bleed into the enumeration",
    )
}

/// The writer seam (`WritableEventStore::settle_workflow_outbox_rows_cancelled`,
/// the Recorder's hook) shares the [`OutboxStore`] twin's exact semantics.
pub(super) async fn writer_seam_settle_matches_the_outbox_twin<S>(
    store: S,
) -> Result<(), StoreError>
where
    S: OutboxStore + WritableEventStore,
{
    let workflow = super::workflow_id();
    store
        .append_outbox_batch(&[pending_row(&workflow, 0)?, pending_row(&workflow, 1)?])
        .await?;

    let mut settled = store
        .settle_workflow_outbox_rows_cancelled(&workflow)
        .await?;
    settled.sort();
    let mut expected = vec![
        OutboxRow::dispatch_key_for(&workflow, 0),
        OutboxRow::dispatch_key_for(&workflow, 1),
    ];
    expected.sort();
    super::expect_eq(
        settled,
        expected,
        "the writer-seam settle must retire the workflow's live rows and return their keys",
    )?;
    super::expect_empty(
        store.claim_outbox_rows(16).await?,
        "rows settled through the writer seam must never be claimable",
    )?;
    super::expect_empty(
        store
            .settle_workflow_outbox_rows_cancelled(&workflow)
            .await?,
        "the writer-seam settle must be idempotent",
    )
}
