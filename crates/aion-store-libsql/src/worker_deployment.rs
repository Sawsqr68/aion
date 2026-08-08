//! Durable worker-deployment records over libSQL.

use aion_store::{
    DesiredState, PutOutcome, StoreError, UndecodableWorkerDeployment, WorkerDeployment,
    WorkerDeploymentDeleteOutcome, WorkerDeploymentListing, WorkerDeploymentPutResult,
};
use chrono::{SecondsFormat, Utc};
use libsql::{Connection, Row, TransactionBehavior};

const SELECT_SQL: &str = "SELECT record FROM worker_deployments WHERE name = ?1";
const LIST_SQL: &str = "SELECT name, record FROM worker_deployments ORDER BY name ASC";
const UPSERT_SQL: &str = "
INSERT INTO worker_deployments (name, updated_at, record) VALUES (?1, ?2, ?3)
ON CONFLICT(name) DO UPDATE SET updated_at = excluded.updated_at, record = excluded.record";
const UPDATE_SQL: &str =
    "UPDATE worker_deployments SET updated_at = ?2, record = ?3 WHERE name = ?1";
const DELETE_SQL: &str = "DELETE FROM worker_deployments WHERE name = ?1 RETURNING record";

pub(crate) async fn put(
    conn: &Connection,
    mut record: WorkerDeployment,
) -> Result<WorkerDeploymentPutResult, StoreError> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .await
        .map_err(|error| crate::error::libsql_error(&error))?;
    let outcome = if let Some(bytes) = load_bytes(&tx, &record.name).await? {
        match WorkerDeployment::decode(&bytes) {
            Ok(previous) => {
                record = record.preserving_previous(&previous, Utc::now());
            }
            Err(error) => {
                tracing::warn!(
                    deployment_key = %record.name,
                    %error,
                    "replacing undecodable prior worker-deployment bytes"
                );
            }
        }
        PutOutcome::Replaced
    } else {
        PutOutcome::Created
    };
    let result = tx
        .execute(
            UPSERT_SQL,
            libsql::params![
                record.name.clone(),
                encode_instant(record.updated_at),
                record.encode()?
            ],
        )
        .await
        .map(|_| WorkerDeploymentPutResult {
            outcome,
            deployment: record,
        })
        .map_err(|error| crate::error::libsql_error(&error));
    finish(tx, result).await
}

pub(crate) async fn get(
    conn: crate::pool::ReadOnly<'_>,
    name: &str,
) -> Result<Option<WorkerDeployment>, StoreError> {
    let mut rows = conn
        .query(SELECT_SQL, libsql::params![name])
        .await
        .map_err(|error| crate::error::libsql_error(&error))?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| crate::error::libsql_error(&error))?
    else {
        return Ok(None);
    };
    decode(&row).map(Some)
}

pub(crate) async fn list(
    conn: crate::pool::ReadOnly<'_>,
) -> Result<WorkerDeploymentListing, StoreError> {
    let mut rows = conn
        .query(LIST_SQL, ())
        .await
        .map_err(|error| crate::error::libsql_error(&error))?;
    let mut listing = WorkerDeploymentListing::default();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| crate::error::libsql_error(&error))?
    {
        let name: String = row
            .get(0)
            .map_err(|error| crate::error::libsql_error(&error))?;
        let bytes: Vec<u8> = row
            .get(1)
            .map_err(|error| crate::error::libsql_error(&error))?;
        match WorkerDeployment::decode(&bytes) {
            Ok(record) => listing.deployments.push(record),
            Err(error) => listing.undecodable.push(UndecodableWorkerDeployment {
                name,
                error: error.to_string(),
            }),
        }
    }
    Ok(listing)
}

pub(crate) async fn set_desired_state(
    conn: &Connection,
    name: &str,
    desired: DesiredState,
) -> Result<Option<WorkerDeployment>, StoreError> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .await
        .map_err(|error| crate::error::libsql_error(&error))?;
    let result = match load_bytes(&tx, name).await {
        Ok(Some(bytes)) => {
            let mut record = WorkerDeployment::decode(&bytes)?;
            record.change_desired_state(desired, Utc::now());
            let encoded = record.encode()?;
            tx.execute(
                UPDATE_SQL,
                libsql::params![name, encode_instant(record.updated_at), encoded],
            )
            .await
            .map(|_| Some(record))
            .map_err(|error| crate::error::libsql_error(&error))
        }
        Ok(None) => Ok(None),
        Err(error) => Err(error),
    };
    finish(tx, result).await
}

pub(crate) async fn delete(
    conn: &Connection,
    name: &str,
) -> Result<WorkerDeploymentDeleteOutcome, StoreError> {
    let mut rows = conn
        .query(DELETE_SQL, libsql::params![name])
        .await
        .map_err(|error| crate::error::libsql_error(&error))?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| crate::error::libsql_error(&error))?
    else {
        return Ok(WorkerDeploymentDeleteOutcome {
            existed: false,
            deployment: None,
        });
    };
    let bytes: Vec<u8> = row
        .get(0)
        .map_err(|error| crate::error::libsql_error(&error))?;
    Ok(WorkerDeploymentDeleteOutcome {
        existed: true,
        deployment: WorkerDeployment::decode(&bytes).ok(),
    })
}

pub(crate) async fn write_raw(
    conn: &Connection,
    name: &str,
    bytes: Vec<u8>,
) -> Result<(), StoreError> {
    conn.execute(
        UPSERT_SQL,
        libsql::params![name, encode_instant(Utc::now()), bytes],
    )
    .await
    .map(|_| ())
    .map_err(|error| crate::error::libsql_error(&error))
}

async fn load_bytes(tx: &libsql::Transaction, name: &str) -> Result<Option<Vec<u8>>, StoreError> {
    let mut rows = tx
        .query(SELECT_SQL, libsql::params![name])
        .await
        .map_err(|error| crate::error::libsql_error(&error))?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| crate::error::libsql_error(&error))?
    else {
        return Ok(None);
    };
    row.get(0)
        .map(Some)
        .map_err(|error| crate::error::libsql_error(&error))
}

fn decode(row: &Row) -> Result<WorkerDeployment, StoreError> {
    let bytes: Vec<u8> = row
        .get(0)
        .map_err(|error| crate::error::libsql_error(&error))?;
    WorkerDeployment::decode(&bytes)
}

async fn finish<T>(
    tx: libsql::Transaction,
    result: Result<T, StoreError>,
) -> Result<T, StoreError> {
    match result {
        Ok(value) => tx
            .commit()
            .await
            .map(|()| value)
            .map_err(|error| crate::error::libsql_error(&error)),
        Err(error) => {
            tx.rollback()
                .await
                .map_err(|rollback| crate::error::libsql_error(&rollback))?;
            Err(error)
        }
    }
}

fn encode_instant(instant: chrono::DateTime<Utc>) -> String {
    instant.to_rfc3339_opts(SecondsFormat::Nanos, true)
}
