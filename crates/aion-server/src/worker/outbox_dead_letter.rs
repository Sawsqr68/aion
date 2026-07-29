//! The outbox dead-letter path: retire the row, tell the workflow, record the judgment.
//!
//! Extracted from [`crate::worker::outbox_dispatcher`] because it is a distinct decision with
//! durable consequences that outlive the dispatch attempt. Three things must happen, in this order,
//! and each one is load-bearing:
//!
//! 1. **Retire the row.** [`OutboxStore::fail_outbox_row`] makes the dead letter durable BEFORE any
//!    delivery is attempted. Delivering first and crashing before the retire would leave the row
//!    `Claimed` for the reconciler to re-arm — a re-execution of an activity the workflow was
//!    already told had failed.
//! 2. **Tell the workflow.** Without this the workflow reports `Running` forever while waiting on
//!    an activity that will never arrive (the liveness lie the delivery-contract fix closed).
//! 3. **Record whether step 2 landed.** This is the durable judgment marker
//!    ([`OutboxRow::failure_delivered`]) that redrive is gated on, and it is the ONLY thing that
//!    distinguishes the two very different dead letters:
//!
//!    - failure **delivered** — the workflow reacted under its own retry/failure semantics and that
//!      reaction is recorded history. Redriving would re-execute a possibly non-idempotent activity
//!      behind a recorded judgment, so redrive refuses it by default.
//!    - failure **undelivered** — no callback was installed, no live workflow accepted it, or the
//!      delivery errored. Nobody was told; the work was never judged, and this is exactly the row
//!      redrive exists for.
//!
//! # The residual window
//!
//! Steps 2 and 3 are two systems (the workflow's history, this row) and cannot be made one atomic
//! act. A crash between them leaves a row that WAS judged carrying `failure_delivered = false`. The
//! window is one store round-trip wide, it is loudly logged at both ends, and it fails in the
//! direction of "looks redrivable" — never "looks judged when it is not". The server-side redrive
//! (`crate::worker::outbox_redrive`) closes it by ALSO reading the workflow's recorded history for
//! a terminal activity failure at the row's ordinal before it redrives anything.

use std::sync::Arc;

use aion_core::ActivityId;
use aion_store::{OutboxRow, OutboxStore};
use tracing::{error, warn};

use crate::error::ServerError;
use crate::worker::OutboxDeliveryCallback;

/// Retire `row` as a dead letter, surface the failure to its workflow, and durably record whether
/// that delivery landed.
///
/// Every branch is logged: a dead letter is an operator-visible event, and the judgment marker
/// decides whether an operator may later redrive the row.
pub(crate) async fn dead_letter_row(
    store: &Arc<dyn OutboxStore>,
    failure_callback: Option<&Arc<dyn OutboxDeliveryCallback>>,
    row: &OutboxRow,
    dispatch_error: &ServerError,
    attempted: u32,
    max_attempts: u32,
) {
    warn!(
        dispatch_key = %row.dispatch_key,
        attempt = row.attempt,
        max_attempts,
        error = %dispatch_error,
        "outbox dispatch exhausted retry budget; dead-lettering row"
    );
    if let Err(error) = store.fail_outbox_row(&row.dispatch_key).await {
        error!(
            dispatch_key = %row.dispatch_key,
            %error,
            "outbox dispatcher failed to dead-letter row"
        );
        return;
    }

    let Some(callback) = failure_callback else {
        warn!(
            dispatch_key = %row.dispatch_key,
            "outbox row dead-lettered without a workflow delivery callback; the workflow was NOT \
             told and the row stays redrivable"
        );
        return;
    };

    let activity_id = ActivityId::from_sequence_position(row.ordinal);
    let reason = format!(
        "infrastructure: delivery to worker failed after {attempted} attempts: {dispatch_error}"
    );
    match callback.deliver_failure(&row.workflow_id, &activity_id, row.run_id.as_ref(), reason) {
        Ok(true) => {
            warn!(
                dispatch_key = %row.dispatch_key,
                attempt = row.attempt,
                "outbox dead-letter failure delivered to workflow"
            );
            record_judgment(store, row).await;
        }
        Ok(false) => {
            // No live workflow accepted the failure: nobody was told, so the row stays redrivable
            // and the judgment marker is deliberately NOT recorded.
            warn!(
                dispatch_key = %row.dispatch_key,
                attempt = row.attempt,
                "outbox dead-letter failure reached callback but no live workflow accepted it; \
                 the row remains redrivable"
            );
        }
        Err(error) => {
            error!(
                dispatch_key = %row.dispatch_key,
                %error,
                "outbox row dead-lettered but failure delivery to workflow failed; \
                 the row remains redrivable"
            );
        }
    }
}

/// Durably record that this dead letter's failure reached its workflow.
///
/// A `false` return means the guarded marker matched nothing — the row is no longer a dead letter,
/// so something moved it between the retire and this write. That is never silent: it is logged at
/// `error` because it leaves a judged row looking redrivable.
async fn record_judgment(store: &Arc<dyn OutboxStore>, row: &OutboxRow) {
    match store
        .record_outbox_failure_delivered(&row.dispatch_key)
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            error!(
                dispatch_key = %row.dispatch_key,
                "outbox dead-letter judgment marker matched no dead-lettered row; the row moved \
                 underneath the dead-letter path and now looks redrivable despite being judged"
            );
        }
        Err(error) => {
            error!(
                dispatch_key = %row.dispatch_key,
                %error,
                "outbox dead-letter failure was delivered but its judgment marker could not be \
                 recorded; the row now looks redrivable despite being judged"
            );
        }
    }
}
