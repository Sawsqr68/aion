//! libSQL outbox terminal, retry, and redrive transition helpers.

use aion_store::{OutboxStatus, RedriveMode, RedriveOutcome, RedriveRefusal, StoreError};
use chrono::{DateTime, SecondsFormat, Utc};
use libsql::{Connection, TransactionBehavior, params};

const COMPLETE_ROW_SQL: &str = "
UPDATE outbox SET status = 'done', claimed_at = NULL WHERE dispatch_key = ?1";

const RETRY_ROW_SQL: &str = "
UPDATE outbox SET status = 'pending', attempt = ?2, visible_after = ?3, claimed_at = NULL WHERE dispatch_key = ?1";

// Dead-lettering opens a FRESH judgment cycle, so the marker is cleared here: a row redriven after
// an earlier judged dead letter must never carry the stale marker into its new one.
const FAIL_ROW_SQL: &str = "
UPDATE outbox SET status = 'failed', claimed_at = NULL, failure_delivered = 0 WHERE dispatch_key = ?1";

// Status-guarded judgment marker: ONLY a row that is currently a dead letter is marked, so a
// concurrent reopen/re-arm that already moved the row on cannot be silently annotated.
const MARK_FAILURE_DELIVERED_SQL: &str = "
UPDATE outbox SET failure_delivered = 1 WHERE dispatch_key = ?1 AND status = 'failed'";

// Status-guarded redrive: a row moves ONLY from 'failed', with the attempt budget reset and the
// judgment marker cleared. The `failure_delivered` conjunct is spliced in for the eligible-only
// mode so the judgment gate is enforced INSIDE the same atomic statement as the transition, never
// by a read-then-write race.
const REDRIVE_ROW_SQL: &str = "
UPDATE outbox SET status = 'pending', attempt = 0, visible_after = ?2, claimed_at = NULL, failure_delivered = 0
WHERE dispatch_key = ?1 AND status = 'failed'";

const REDRIVE_ELIGIBLE_ONLY_CLAUSE: &str = " AND failure_delivered = 0";

const SELECT_ROW_STATE_SQL: &str = "
SELECT status, failure_delivered FROM outbox WHERE dispatch_key = ?1";

const SETTLE_CANCELLED_ROW_SQL: &str = "
UPDATE outbox SET status = 'cancelled', claimed_at = NULL
WHERE dispatch_key = ?1 AND status IN ('pending', 'claimed')";

/// Mark the row identified by `dispatch_key` as `done`.
///
/// # Errors
///
/// Returns `StoreError::Backend` for libSQL boundary failures.
pub(crate) async fn complete_outbox_row(
    conn: &Connection,
    dispatch_key: &str,
) -> Result<(), StoreError> {
    conn.execute(COMPLETE_ROW_SQL, params![dispatch_key.to_string()])
        .await
        .map(|_| ())
        .map_err(|error| crate::error::libsql_error(&error))
}

/// Return the row identified by `dispatch_key` to `pending` with updated attempt and backoff fence.
///
/// # Errors
///
/// Returns `StoreError::Backend` for libSQL boundary failures.
pub(crate) async fn retry_outbox_row(
    conn: &Connection,
    dispatch_key: &str,
    next_attempt: u32,
    visible_after: DateTime<Utc>,
) -> Result<(), StoreError> {
    conn.execute(
        RETRY_ROW_SQL,
        params![
            dispatch_key.to_string(),
            i64::from(next_attempt),
            encode_instant(visible_after)
        ],
    )
    .await
    .map(|_| ())
    .map_err(|error| crate::error::libsql_error(&error))
}

/// Mark the row identified by `dispatch_key` as `failed` (dead letter).
///
/// # Errors
///
/// Returns `StoreError::Backend` for libSQL boundary failures.
pub(crate) async fn fail_outbox_row(
    conn: &Connection,
    dispatch_key: &str,
) -> Result<(), StoreError> {
    conn.execute(FAIL_ROW_SQL, params![dispatch_key.to_string()])
        .await
        .map(|_| ())
        .map_err(|error| crate::error::libsql_error(&error))
}

/// Idempotently settle a live row identified by `dispatch_key` as `cancelled`.
///
/// Only `pending` and `claimed` rows transition. Absent, already-cancelled, `done`, and `failed`
/// rows are safe no-ops.
///
/// # Errors
///
/// Returns `StoreError::Backend` for libSQL boundary failures.
pub(crate) async fn settle_outbox_row_cancelled(
    conn: &Connection,
    dispatch_key: &str,
) -> Result<(), StoreError> {
    conn.execute(SETTLE_CANCELLED_ROW_SQL, params![dispatch_key.to_string()])
        .await
        .map(|_| ())
        .map_err(|error| crate::error::libsql_error(&error))
}

/// Record that a dead letter's failure reached the owning workflow, returning whether the marker
/// was written.
///
/// Status-guarded: only a row that is currently `failed` is marked. `false` means nothing was
/// marked — no such row, or the row is no longer a dead letter.
///
/// # Errors
///
/// Returns `StoreError::Backend` for libSQL boundary failures.
pub(crate) async fn record_outbox_failure_delivered(
    conn: &Connection,
    dispatch_key: &str,
) -> Result<bool, StoreError> {
    conn.execute(
        MARK_FAILURE_DELIVERED_SQL,
        params![dispatch_key.to_string()],
    )
    .await
    .map(|changed| changed > 0)
    .map_err(|error| crate::error::libsql_error(&error))
}

/// Return a dead-lettered row to the pending claim path, or report why it was refused.
///
/// The whole decision runs inside ONE `IMMEDIATE` transaction — the single-writer lock is taken
/// before the state read, so the status/judgment classification the refusal reports and the guarded
/// `UPDATE` that acts on it cannot be separated by another writer. The `UPDATE` itself repeats both
/// guards (`status = 'failed'`, plus `failure_delivered = 0` in eligible mode) so the transition is
/// status-guarded in the statement, not merely in the surrounding read.
///
/// # Errors
///
/// Returns `StoreError::Backend` for libSQL boundary failures and `StoreError::Serialization` when
/// the stored row cannot be decoded.
pub(crate) async fn redrive_outbox_row(
    conn: &Connection,
    dispatch_key: &str,
    visible_after: DateTime<Utc>,
    mode: RedriveMode,
) -> Result<RedriveOutcome, StoreError> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .await
        .map_err(|error| crate::error::libsql_error(&error))?;
    let outcome = match redrive_within(&tx, dispatch_key, visible_after, mode).await {
        Ok(outcome) => outcome,
        Err(error) => {
            super::rollback(tx).await?;
            return Err(error);
        }
    };
    tx.commit()
        .await
        .map_err(|error| crate::error::libsql_error(&error))?;
    Ok(outcome)
}

async fn redrive_within(
    tx: &libsql::Transaction,
    dispatch_key: &str,
    visible_after: DateTime<Utc>,
    mode: RedriveMode,
) -> Result<RedriveOutcome, StoreError> {
    let Some((status, failure_delivered)) = select_row_state(tx, dispatch_key).await? else {
        return Ok(RedriveOutcome::Refused(RedriveRefusal::NoSuchRow {
            dispatch_key: dispatch_key.to_owned(),
        }));
    };
    if status != OutboxStatus::Failed {
        return Ok(RedriveOutcome::Refused(RedriveRefusal::NotDeadLettered {
            dispatch_key: dispatch_key.to_owned(),
            status,
        }));
    }
    if failure_delivered && !mode.admits_judged() {
        return Ok(RedriveOutcome::Refused(RedriveRefusal::AlreadyJudged {
            dispatch_key: dispatch_key.to_owned(),
        }));
    }

    let statement = match mode {
        RedriveMode::Eligible => format!("{REDRIVE_ROW_SQL}{REDRIVE_ELIGIBLE_ONLY_CLAUSE}"),
        RedriveMode::Forced => REDRIVE_ROW_SQL.to_owned(),
    };
    let changed = tx
        .execute(
            &statement,
            params![dispatch_key.to_string(), encode_instant(visible_after)],
        )
        .await
        .map_err(|error| crate::error::libsql_error(&error))?;
    if changed == 0 {
        // Unreachable under the IMMEDIATE write lock taken above: nothing else can move the row
        // between the classification and this guarded UPDATE. Reaching it means the guard and the
        // classification disagree, which is a backend invariant break — never a silent no-op.
        return Err(StoreError::Backend(format!(
            "outbox redrive of {dispatch_key} classified the row as an eligible dead letter but the \
             status-guarded update matched nothing"
        )));
    }

    match super::select_row_by_key(tx, dispatch_key).await? {
        Some(row) => Ok(RedriveOutcome::Redriven {
            row: Box::new(row),
            was_judged: failure_delivered,
        }),
        None => Err(StoreError::Backend(format!(
            "outbox redrive of {dispatch_key} updated the row but it could not be read back"
        ))),
    }
}

/// Read one row's status + judgment marker inside an existing transaction.
async fn select_row_state(
    tx: &libsql::Transaction,
    dispatch_key: &str,
) -> Result<Option<(OutboxStatus, bool)>, StoreError> {
    let mut rows = tx
        .query(SELECT_ROW_STATE_SQL, params![dispatch_key.to_string()])
        .await
        .map_err(|error| crate::error::libsql_error(&error))?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| crate::error::libsql_error(&error))?
    else {
        return Ok(None);
    };
    let status: String = row
        .get(0)
        .map_err(|error| crate::error::libsql_error(&error))?;
    let failure_delivered: i64 = row
        .get(1)
        .map_err(|error| crate::error::libsql_error(&error))?;
    Ok(Some((
        OutboxStatus::parse_token(&status)?,
        failure_delivered != 0,
    )))
}

fn encode_instant(instant: DateTime<Utc>) -> String {
    instant.to_rfc3339_opts(SecondsFormat::Nanos, true)
}
