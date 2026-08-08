//! `LibSqlStore` struct and `EventStore` implementation wiring.

use std::{path::PathBuf, sync::Arc};

use aion_store::{
    ClaimScope, DesiredState, Event, MintOutcome, NamespaceOrigin, NamespacePlacement,
    NamespaceRecord, NamespaceStore, OutboxRow, OutboxStore, PackageRecord, PackageRouteRecord,
    PackageStore, ReadableEventStore, RedriveMode, RedriveOutcome, RunSummary, StoreError,
    TimerEntry, TimerId, WorkerDeployment, WorkerDeploymentDeleteOutcome, WorkerDeploymentListing,
    WorkerDeploymentPutResult, WorkerDeploymentStore, WorkflowFilter, WorkflowId, WorkflowSummary,
    WritableEventStore, WriteToken,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::sync::Mutex;

use crate::config::{LibSqlConfig, LibSqlMode};
use crate::outbox::OutboxRowState;

/// Durable `EventStore` over a dedicated write connection and a read pool.
///
/// # Why the shape is this and not one shared connection (#122)
///
/// `SQLite` transaction state is a property of the **connection**, and a
/// `libsql::Connection` is an `Arc` over one raw `sqlite3*`. When every caller
/// shared one connection, a statement issued while another task held an open
/// `BEGIN IMMEDIATE` executed *inside that task's transaction* — so a write
/// could be rolled back after returning `Ok`, a read could observe uncommitted
/// rows, `sqlite3_changes64` could report another statement's count, and six
/// entry points could issue a second `BEGIN` on a connection already inside one.
///
/// The old defence was a mutex over `()` sitting *beside* a connection that
/// everyone could also reach through a `connection()` accessor. Taking it was a
/// convention, and most methods did not.
///
/// Here the writer mutex **owns** its connection. There is no way to obtain a
/// `&Connection` except by holding a guard, so forgetting is not expressible —
/// the code that forgets does not compile. Reads come from their own
/// connections and never touch the writer's, so a read cannot land inside
/// anyone's transaction. `changes64` is per-operation because each operation
/// holds its connection exclusively.
#[derive(Clone)]
pub struct LibSqlStore {
    factory: Arc<crate::connection::ConnectionFactory>,
    /// The ONE write connection. Every durable write serialises on this mutex,
    /// which cannot time out — unlike a file lock, which can. That is why one
    /// write connection beats N: WAL admits a single writer anyway, so N
    /// connections would convert an in-process queue into `SQLITE_BUSY`
    /// contention without adding throughput.
    writer: Arc<Mutex<libsql::Connection>>,
    readers: Arc<crate::pool::ReadPool>,
}

impl LibSqlStore {
    /// Open a store from operator-provided libSQL configuration.
    ///
    /// # Errors
    ///
    /// Returns `StoreError::Backend` when the connection cannot be opened or when the idempotent
    /// schema DDL cannot be applied.
    pub async fn connect(config: LibSqlConfig) -> Result<Self, StoreError> {
        let read_pool_size = config.read_pool_size;
        let factory = Arc::new(crate::connection::open_factory(&config).await?);
        let writer = factory.connect().await?;
        crate::schema::ensure_schema(&writer).await?;

        Ok(Self {
            readers: Arc::new(crate::pool::ReadPool::new(
                Arc::clone(&factory),
                read_pool_size,
            )),
            factory,
            writer: Arc::new(Mutex::new(writer)),
        })
    }

    /// Take the write connection for one durable operation.
    ///
    /// Holding the returned guard **is** the exclusion — there is no separate
    /// lock to remember, because there is no other way to reach the connection.
    pub(crate) async fn writer(&self) -> tokio::sync::MutexGuard<'_, libsql::Connection> {
        self.writer.lock().await
    }

    /// Take a read connection for one operation, returned to the pool on drop.
    ///
    /// # Errors
    ///
    /// Returns `StoreError::Backend` when a new connection cannot be opened or
    /// configured.
    pub(crate) async fn reader(&self) -> Result<crate::pool::ReadGuard, StoreError> {
        self.readers.acquire().await
    }

    /// The read pool, for the pool module's own tests. Not a general accessor:
    /// it exposes retention state, never a connection.
    #[cfg(test)]
    pub(crate) fn readers(&self) -> &crate::pool::ReadPool {
        &self.readers
    }

    /// Open an embedded local-file store at `path`.
    ///
    /// Operator tunables remain unset; this convenience constructor only selects embedded mode.
    ///
    /// # Errors
    ///
    /// Returns `StoreError::Backend` when the connection cannot be opened or when the idempotent
    /// schema DDL cannot be applied.
    pub async fn open(path: impl Into<PathBuf>) -> Result<Self, StoreError> {
        Self::connect(LibSqlConfig {
            mode: LibSqlMode::Embedded { path: path.into() },
            journal_mode: None,
            synchronous: None,
            sync_interval_seconds: None,
            busy_timeout_ms: None,
            read_pool_size: None,
        })
        .await
    }

    /// Validate stored event blobs against the current Aion event schema.
    ///
    /// # Errors
    ///
    /// Returns `StoreError::Serialization` when any stored event cannot be decoded by the current
    /// event schema, or `StoreError::Backend` for libSQL scan failures.
    pub async fn validate_event_compatibility(&self) -> Result<(), StoreError> {
        crate::read::validate_all_events(self.reader().await?.read()).await
    }

    /// Trigger and await a libSQL replica synchronization cycle.
    ///
    /// # Errors
    ///
    /// Returns `StoreError::Backend` when the current libSQL database mode does not support sync or
    /// when the replica sync operation fails.
    pub async fn sync(&self) -> Result<(), StoreError> {
        // A database operation, not a connection one, so it belongs to the
        // factory's handle rather than to any pooled connection.
        self.factory
            .database()
            .sync()
            .await
            .map(|_| ())
            .map_err(|error| crate::error::libsql_error(&error))
    }

    /// Atomically append `events` and, when `outbox_rows` is `Some`, the outbox rows in the same
    /// `IMMEDIATE` transaction, under the expected-head sequence guard.
    ///
    /// This is the durable fan-out write: the scheduling events and their outbox rows commit
    /// together or not at all. Passing `None` is equivalent to
    /// [`WritableEventStore::append`](aion_store::WritableEventStore::append).
    ///
    /// # Errors
    ///
    /// Returns `StoreError::SequenceConflict` when the stored head differs from `expected_seq`,
    /// `StoreError::Serialization` when an event or outbox payload cannot be serialized, and
    /// `StoreError::Backend` for libSQL boundary failures.
    pub async fn append_with_outbox(
        &self,
        _token: WriteToken,
        workflow_id: &WorkflowId,
        events: &[Event],
        expected_seq: u64,
        outbox_rows: Option<&[OutboxRow]>,
    ) -> Result<(), StoreError> {
        crate::append::append_with_outbox(
            &*self.writer().await,
            workflow_id,
            events,
            expected_seq,
            outbox_rows,
        )
        .await
    }

    /// Read the persisted lifecycle state of one outbox row by its dispatch key.
    ///
    /// Returns the `(status, attempt, visible_after)` triple, or `None` when no
    /// row carries `dispatch_key` (the dedup guard may have ignored it). This is
    /// an out-of-band inspection helper — the [`OutboxStore`] dispatch contract
    /// itself keys terminal transitions off `dispatch_key` and never needs to
    /// read a row back — used by the dispatcher's tests and by operators auditing
    /// dead-lettered (`failed`) rows.
    ///
    /// # Errors
    ///
    /// Returns `StoreError::Backend` for libSQL boundary failures and
    /// `StoreError::Serialization` when the stored status token or timestamp
    /// cannot be decoded.
    pub async fn outbox_row_state(
        &self,
        dispatch_key: &str,
    ) -> Result<Option<OutboxRowState>, StoreError> {
        crate::outbox::outbox_row_state(self.reader().await?.read(), dispatch_key).await
    }

    /// Write worker-deployment backend bytes directly for persistence
    /// conformance testing.
    ///
    /// This bypasses the entity codec so the shared suite can create a poisoned
    /// row and verify listing, replacement, and deletion behavior.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] for libSQL write failures.
    pub async fn write_raw_worker_deployment(
        &self,
        name: &str,
        bytes: Vec<u8>,
    ) -> Result<(), StoreError> {
        crate::worker_deployment::write_raw(&*self.writer().await, name, bytes).await
    }
}

#[async_trait]
impl PackageStore for LibSqlStore {
    async fn put_package(&self, record: PackageRecord) -> Result<(), StoreError> {
        crate::package::put_package(&*self.writer().await, record).await
    }

    async fn put_package_with_routes(
        &self,
        record: PackageRecord,
        route_workflow_types: &[String],
    ) -> Result<(), StoreError> {
        crate::package::put_package_with_routes(&*self.writer().await, record, route_workflow_types)
            .await
    }

    async fn list_packages(&self) -> Result<Vec<PackageRecord>, StoreError> {
        crate::package::list_packages(self.reader().await?.read()).await
    }

    async fn delete_package(
        &self,
        workflow_type: &str,
        content_hash: &str,
    ) -> Result<(), StoreError> {
        crate::package::delete_package(&*self.writer().await, workflow_type, content_hash).await
    }

    async fn put_package_route(
        &self,
        workflow_type: &str,
        content_hash: &str,
    ) -> Result<(), StoreError> {
        crate::package::put_package_route(&*self.writer().await, workflow_type, content_hash).await
    }

    async fn list_package_routes(&self) -> Result<Vec<PackageRouteRecord>, StoreError> {
        crate::package::list_package_routes(self.reader().await?.read()).await
    }
}

#[async_trait]
impl NamespaceStore for LibSqlStore {
    async fn register_namespace(
        &self,
        name: &str,
        origin: NamespaceOrigin,
    ) -> Result<MintOutcome, StoreError> {
        crate::namespace::register_namespace(&*self.writer().await, name, origin).await
    }

    async fn put_namespace(&self, record: NamespaceRecord) -> Result<MintOutcome, StoreError> {
        crate::namespace::put_namespace(&*self.writer().await, record).await
    }

    async fn list_namespaces(&self) -> Result<Vec<NamespaceRecord>, StoreError> {
        crate::namespace::list_namespaces(self.reader().await?.read()).await
    }

    async fn get_namespace(&self, name: &str) -> Result<Option<NamespaceRecord>, StoreError> {
        crate::namespace::get_namespace(self.reader().await?.read(), name).await
    }

    async fn set_namespace_placement(
        &self,
        name: &str,
        placement: NamespacePlacement,
    ) -> Result<Option<()>, StoreError> {
        crate::namespace::set_namespace_placement(&*self.writer().await, name, placement).await
    }

    async fn deprecate_namespace(&self, name: &str) -> Result<(), StoreError> {
        crate::namespace::deprecate_namespace(&*self.writer().await, name).await
    }
}

#[async_trait]
impl WorkerDeploymentStore for LibSqlStore {
    async fn put_worker_deployment(
        &self,
        record: WorkerDeployment,
    ) -> Result<WorkerDeploymentPutResult, StoreError> {
        crate::worker_deployment::put(&*self.writer().await, record).await
    }

    async fn get_worker_deployment(
        &self,
        name: &str,
    ) -> Result<Option<WorkerDeployment>, StoreError> {
        crate::worker_deployment::get(self.reader().await?.read(), name).await
    }

    async fn list_worker_deployments(&self) -> Result<WorkerDeploymentListing, StoreError> {
        crate::worker_deployment::list(self.reader().await?.read()).await
    }

    async fn set_desired_state(
        &self,
        name: &str,
        desired: DesiredState,
    ) -> Result<Option<WorkerDeployment>, StoreError> {
        crate::worker_deployment::set_desired_state(&*self.writer().await, name, desired).await
    }

    async fn delete_worker_deployment(
        &self,
        name: &str,
    ) -> Result<WorkerDeploymentDeleteOutcome, StoreError> {
        crate::worker_deployment::delete(&*self.writer().await, name).await
    }
}

#[async_trait]
impl WritableEventStore for LibSqlStore {
    async fn append(
        &self,
        _token: WriteToken,
        workflow_id: &WorkflowId,
        events: &[Event],
        expected_seq: u64,
    ) -> Result<(), StoreError> {
        crate::append::append(&*self.writer().await, workflow_id, events, expected_seq).await
    }

    async fn append_with_outbox(
        &self,
        _token: WriteToken,
        workflow_id: &WorkflowId,
        events: &[Event],
        expected_seq: u64,
        outbox_rows: &[OutboxRow],
    ) -> Result<(), StoreError> {
        crate::append::append_with_outbox(
            &*self.writer().await,
            workflow_id,
            events,
            expected_seq,
            Some(outbox_rows),
        )
        .await
    }

    async fn rearm_outbox_pending(&self, rows: &[OutboxRow]) -> Result<(), StoreError> {
        crate::outbox::rearm_outbox_pending(&*self.writer().await, rows).await
    }

    async fn settle_outbox_row_cancelled(&self, dispatch_key: &str) -> Result<(), StoreError> {
        crate::outbox::settle_outbox_row_cancelled(&*self.writer().await, dispatch_key).await
    }

    async fn settle_workflow_outbox_rows_cancelled(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<String>, StoreError> {
        crate::outbox::settle_workflow_outbox_rows_cancelled(&*self.writer().await, workflow_id)
            .await
    }
}

#[async_trait]
impl OutboxStore for LibSqlStore {
    async fn append_outbox_batch(&self, rows: &[OutboxRow]) -> Result<(), StoreError> {
        crate::outbox::append_outbox_batch(&*self.writer().await, rows).await
    }

    async fn claim_outbox_rows(&self, limit: u32) -> Result<Vec<OutboxRow>, StoreError> {
        crate::outbox::claim_outbox_rows(&*self.writer().await, limit).await
    }

    async fn claim_outbox_rows_excluding(
        &self,
        limit: u32,
        held: &std::collections::HashSet<WorkflowId>,
    ) -> Result<Vec<OutboxRow>, StoreError> {
        crate::outbox::claim_outbox_rows_excluding(&*self.writer().await, limit, held).await
    }

    async fn claim_outbox_rows_scoped(
        &self,
        scope: &ClaimScope,
        limit: u32,
    ) -> Result<Vec<OutboxRow>, StoreError> {
        crate::outbox::claim_outbox_rows_scoped(&*self.writer().await, scope, limit).await
    }

    async fn claim_outbox_rows_scoped_excluding(
        &self,
        scope: &ClaimScope,
        limit: u32,
        held: &std::collections::HashSet<WorkflowId>,
    ) -> Result<Vec<OutboxRow>, StoreError> {
        crate::outbox::claim_outbox_rows_scoped_excluding(&*self.writer().await, scope, limit, held)
            .await
    }

    async fn rearm_stale_claimed_outbox_rows(
        &self,
        older_than: DateTime<Utc>,
        visible_after: DateTime<Utc>,
        limit: u32,
        excluded: &std::collections::HashSet<String>,
    ) -> Result<Vec<OutboxRow>, StoreError> {
        crate::outbox::rearm_stale_claimed_outbox_rows(
            &*self.writer().await,
            older_than,
            visible_after,
            limit,
            excluded,
        )
        .await
    }

    async fn list_stale_claimed_outbox_rows(
        &self,
        older_than: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<OutboxRow>, StoreError> {
        crate::outbox::list_stale_claimed_outbox_rows(
            self.reader().await?.read(),
            older_than,
            limit,
        )
        .await
    }

    async fn list_unsettled_outbox_workflow_ids(&self) -> Result<Vec<WorkflowId>, StoreError> {
        crate::outbox::list_unsettled_outbox_workflow_ids(self.reader().await?.read()).await
    }

    async fn cancel_outbox_rows_for_workflow(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<String>, StoreError> {
        crate::outbox::settle_workflow_outbox_rows_cancelled(&*self.writer().await, workflow_id)
            .await
    }

    async fn complete_outbox_row(&self, dispatch_key: &str) -> Result<(), StoreError> {
        crate::outbox::complete_outbox_row(&*self.writer().await, dispatch_key).await
    }

    async fn retry_outbox_row(
        &self,
        dispatch_key: &str,
        next_attempt: u32,
        visible_after: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        crate::outbox::retry_outbox_row(
            &*self.writer().await,
            dispatch_key,
            next_attempt,
            visible_after,
        )
        .await
    }

    async fn fail_outbox_row(&self, dispatch_key: &str) -> Result<(), StoreError> {
        crate::outbox::fail_outbox_row(&*self.writer().await, dispatch_key).await
    }

    async fn record_outbox_failure_delivered(
        &self,
        dispatch_key: &str,
    ) -> Result<bool, StoreError> {
        crate::outbox::record_outbox_failure_delivered(&*self.writer().await, dispatch_key).await
    }

    async fn list_dead_lettered_outbox_rows(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<OutboxRow>, StoreError> {
        crate::outbox::list_dead_lettered_outbox_rows(self.reader().await?.read(), workflow_id)
            .await
    }

    async fn redrive_outbox_row(
        &self,
        dispatch_key: &str,
        visible_after: DateTime<Utc>,
        mode: RedriveMode,
    ) -> Result<RedriveOutcome, StoreError> {
        // Serialised with every other IMMEDIATE-transaction writer on this single connection, the
        // same way the workflow-terminal settle is.
        crate::outbox::redrive_outbox_row(&*self.writer().await, dispatch_key, visible_after, mode)
            .await
    }

    async fn count_inflight_outbox_rows(&self, namespace: &str) -> Result<u64, StoreError> {
        crate::outbox::count_inflight_outbox_rows(self.reader().await?.read(), namespace).await
    }

    async fn count_claimed_outbox_rows(&self, namespace: &str) -> Result<u64, StoreError> {
        crate::outbox::count_claimed_outbox_rows(self.reader().await?.read(), namespace).await
    }

    async fn count_claimed_outbox_rows_by_namespace(
        &self,
        namespaces: &[&str],
    ) -> Result<std::collections::BTreeMap<String, u64>, StoreError> {
        crate::outbox::count_claimed_outbox_rows_by_namespace(
            self.reader().await?.read(),
            namespaces,
        )
        .await
    }

    async fn pending_outbox_routes(&self) -> Result<Vec<ClaimScope>, StoreError> {
        crate::outbox::pending_outbox_routes(self.reader().await?.read()).await
    }
}

#[async_trait]
impl ReadableEventStore for LibSqlStore {
    async fn read_history(&self, workflow_id: &WorkflowId) -> Result<Vec<Event>, StoreError> {
        crate::read::read_history(self.reader().await?.read(), workflow_id).await
    }

    async fn read_history_from(
        &self,
        workflow_id: &WorkflowId,
        from_seq: u64,
    ) -> Result<Vec<Event>, StoreError> {
        crate::read::read_history_from(self.reader().await?.read(), workflow_id, from_seq).await
    }

    async fn read_run_chain(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<RunSummary>, StoreError> {
        crate::read::read_run_chain(self.reader().await?.read(), workflow_id).await
    }

    async fn list_workflow_ids(&self) -> Result<Vec<WorkflowId>, StoreError> {
        crate::read::list_workflow_ids(self.reader().await?.read()).await
    }

    async fn list_active(&self) -> Result<Vec<WorkflowId>, StoreError> {
        crate::read::list_active(self.reader().await?.read()).await
    }

    async fn list_paused(&self) -> Result<Vec<WorkflowId>, StoreError> {
        crate::read::list_paused(self.reader().await?.read()).await
    }

    async fn query(&self, filter: &WorkflowFilter) -> Result<Vec<WorkflowSummary>, StoreError> {
        crate::read::query(self.reader().await?.read(), filter).await
    }

    async fn schedule_timer(
        &self,
        workflow_id: &WorkflowId,
        timer_id: &TimerId,
        fire_at: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        // WRITER, not reader: `schedule_timer` UPSERTs a timer row. The name
        // reads like scheduling and the first pass classified it by name — it
        // is a durable write and must take the writer's mutex like every other.
        crate::timer::schedule_timer(&*self.writer().await, workflow_id, timer_id, fire_at).await
    }

    async fn expired_timers(&self, as_of: DateTime<Utc>) -> Result<Vec<TimerEntry>, StoreError> {
        crate::timer::expired_timers(self.reader().await?.read(), as_of).await
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use aion_store::{EventStore, StoreError};

    use super::LibSqlStore;

    #[test]
    fn libsql_store_is_send_sync_static() {
        fn assert_send_sync_static<T: Send + Sync + 'static>() {}

        assert_send_sync_static::<LibSqlStore>();
    }

    #[tokio::test]
    async fn open_creates_schema() -> Result<(), StoreError> {
        let store = LibSqlStore::open(unique_temp_path("open-schema")).await?;

        assert_schema_object(store.reader().await?.read(), "table", "events").await?;
        assert_schema_object(store.reader().await?.read(), "table", "timers").await?;
        assert_schema_object(store.reader().await?.read(), "table", "visibility").await?;

        Ok(())
    }

    #[tokio::test]
    async fn store_can_be_used_as_event_store_trait_object() -> Result<(), StoreError> {
        let store = LibSqlStore::open(unique_temp_path("trait-object")).await?;
        let store: Arc<dyn EventStore> = Arc::new(store);

        assert_eq!(Arc::strong_count(&store), 1);
        Ok(())
    }

    #[tokio::test]
    /// Renamed with the accessor it used to name (#122).
    ///
    /// The old name was `connection_accessor_reuses_same_database_handle`, and
    /// `connection()` no longer exists — so the name claimed coverage of a
    /// surface the run could not touch, which is the #119 defect one level down.
    ///
    /// What it proves NOW is a property the pool makes non-obvious and that
    /// nothing else asserts: the writer connection and a pooled read connection
    /// are DIFFERENT connections onto the SAME database. Before this change one
    /// handle made that trivially true; now it is a real claim, and a pool that
    /// somehow opened a separate database would pass every other test in the
    /// crate while failing this one.
    async fn a_write_is_visible_from_a_different_pooled_connection() -> Result<(), StoreError> {
        let store = LibSqlStore::open(unique_temp_path("shared-handle")).await?;

        store
            .writer()
            .await
            .execute(
                "INSERT INTO timers (workflow_id, timer_id, fire_at) VALUES (?1, ?2, ?3)",
                ("workflow-a", "timer-a", "2026-06-03T00:00:00Z"),
            )
            .await
            .map_err(|error| crate::error::libsql_error(&error))?;

        let count = timer_count(store.reader().await?.read()).await?;
        if count == 1 {
            Ok(())
        } else {
            Err(StoreError::Backend(format!(
                "a row written on the writer connection must be readable from a \
                 pooled read connection — expected 1 timer, found {count}"
            )))
        }
    }

    async fn assert_schema_object(
        conn: crate::pool::ReadOnly<'_>,
        object_type: &str,
        name: &str,
    ) -> Result<(), StoreError> {
        let mut rows = conn
            .query(
                "SELECT name FROM sqlite_master WHERE type = ?1 AND name = ?2",
                (object_type, name),
            )
            .await
            .map_err(|error| crate::error::libsql_error(&error))?;
        let found = rows
            .next()
            .await
            .map_err(|error| crate::error::libsql_error(&error))?
            .is_some();

        if found {
            Ok(())
        } else {
            Err(StoreError::Backend(format!(
                "schema object {object_type} {name} was not created"
            )))
        }
    }

    async fn timer_count(conn: crate::pool::ReadOnly<'_>) -> Result<i64, StoreError> {
        let mut rows = conn
            .query("SELECT COUNT(*) FROM timers", ())
            .await
            .map_err(|error| crate::error::libsql_error(&error))?;
        let row = rows
            .next()
            .await
            .map_err(|error| crate::error::libsql_error(&error))?
            .ok_or_else(|| {
                StoreError::Backend(String::from("timer count query returned no row"))
            })?;

        row.get(0)
            .map_err(|error| crate::error::libsql_error(&error))
    }

    fn unique_temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        std::env::temp_dir().join(format!(
            "aion-store-libsql-store-{name}-{}-{nanos}.db",
            std::process::id()
        ))
    }
}
