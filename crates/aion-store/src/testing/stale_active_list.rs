//! A store that lies in exactly one place: `list_active`.
//!
//! Every real backend filters `list_active` to projected status `Running` —
//! [`InMemoryStore`] at `memory.rs`, the libSQL backend in its read path, the
//! haematite backend in its own. That filter is load-bearing and correct, and it
//! is *also* what makes a reader's second line of defence unreachable: a reader
//! that re-reads each listed run's history and refuses the ones that project
//! terminal (or `Paused`) can never see one through an honest store, so deleting
//! that refusal changes no observable outcome and every test still passes.
//!
//! A second line of defence behind a total filter reads as covered because the
//! OUTCOME is always right. [`StaleActiveListStore`] is the instrument that tells
//! the two apart: it forces chosen workflow ids into the `list_active` answer
//! whatever their history projects, and delegates everything else — including the
//! per-run `read_history` the reader defends itself with — to a real
//! [`InMemoryStore`]. The reader then faces a genuinely inconsistent store and
//! must defend itself or fail.
//!
//! Both of the states it can force are reachable in production, by different
//! routes:
//!
//! - **Terminal.** A backend whose active-set filter is wrong, stale, or racing a
//!   concurrent terminal write hands back a run that has already finished.
//! - **`Paused`.** No filter bug needed. `list_active` is a *snapshot*; a run
//!   paused between that snapshot and the per-run re-read was legitimately active
//!   when listed and is `Paused` by the time it is read (#204). This double
//!   reproduces that interleaving deterministically, without a race.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::{Arc, Mutex};

use crate::memory::InMemoryStore;
use crate::package::{PackageRecord, PackageRouteRecord, PackageStore};
use crate::{
    Event, OutboxRow, ReadableEventStore, RunSummary, StoreError, TimerEntry, TimerId,
    WorkflowFilter, WorkflowId, WorkflowSummary, WritableEventStore, WriteToken,
};

/// [`crate::EventStore`] double whose `list_active` reports ids the inner store's
/// own filter excludes.
///
/// Construct with [`StaleActiveListStore::new`], seed history through it exactly
/// as through any store, then name the ids to force with
/// [`StaleActiveListStore::force_active`]. Every other method delegates to the
/// inner [`InMemoryStore`], so the forced run's history still projects whatever
/// was truthfully written to it — which is the entire point: the reader is handed
/// an id its own re-read will contradict.
pub struct StaleActiveListStore {
    inner: InMemoryStore,
    forced: Arc<Mutex<Vec<WorkflowId>>>,
}

impl StaleActiveListStore {
    /// Construct a double wrapping a fresh, empty [`InMemoryStore`] that forces
    /// nothing. Until [`StaleActiveListStore::force_active`] is called it is
    /// indistinguishable from the inner store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: InMemoryStore::default(),
            forced: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Force `workflow_id` into every subsequent `list_active` answer, whatever
    /// its history projects.
    ///
    /// Forcing an id the inner store would have listed anyway is a no-op on the
    /// answer (the id appears once), so a test that forces the wrong id gets a
    /// silently honest store rather than a duplicate — assert on `list_active`
    /// directly before relying on the injection.
    pub fn force_active(&self, workflow_id: WorkflowId) {
        self.forced
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(workflow_id);
    }
}

impl Default for StaleActiveListStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ReadableEventStore for StaleActiveListStore {
    /// The inner store's honest answer, followed by every forced id it did not
    /// already contain, in the order they were forced.
    async fn list_active(&self) -> Result<Vec<WorkflowId>, StoreError> {
        let mut active = self.inner.list_active().await?;
        let forced = self
            .forced
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        for workflow_id in forced {
            if !active.contains(&workflow_id) {
                active.push(workflow_id);
            }
        }
        Ok(active)
    }

    fn set_owned_shards(&self, shards: Option<&[usize]>) {
        self.inner.set_owned_shards(shards);
    }

    fn acquire_owned_shards(&self, shards: &[usize]) -> Result<(), StoreError> {
        self.inner.acquire_owned_shards(shards)
    }

    fn acquire_owned_shard(&self, shard: usize) -> Result<(), StoreError> {
        self.inner.acquire_owned_shard(shard)
    }

    fn extend_owned_shards(&self, shards: &[usize]) {
        self.inner.extend_owned_shards(shards);
    }

    fn is_current_owner(&self, shard: usize) -> bool {
        self.inner.is_current_owner(shard)
    }

    fn publish_shard_owner(&self, shard: usize) -> Result<(), StoreError> {
        self.inner.publish_shard_owner(shard)
    }

    async fn read_history(&self, workflow_id: &WorkflowId) -> Result<Vec<Event>, StoreError> {
        self.inner.read_history(workflow_id).await
    }

    async fn read_history_from(
        &self,
        workflow_id: &WorkflowId,
        from_seq: u64,
    ) -> Result<Vec<Event>, StoreError> {
        self.inner.read_history_from(workflow_id, from_seq).await
    }

    async fn read_run_chain(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<RunSummary>, StoreError> {
        self.inner.read_run_chain(workflow_id).await
    }

    async fn list_workflow_ids(&self) -> Result<Vec<WorkflowId>, StoreError> {
        self.inner.list_workflow_ids().await
    }

    async fn list_paused(&self) -> Result<Vec<WorkflowId>, StoreError> {
        self.inner.list_paused().await
    }

    async fn query(&self, filter: &WorkflowFilter) -> Result<Vec<WorkflowSummary>, StoreError> {
        self.inner.query(filter).await
    }

    async fn schedule_timer(
        &self,
        workflow_id: &WorkflowId,
        timer_id: &TimerId,
        fire_at: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.inner
            .schedule_timer(workflow_id, timer_id, fire_at)
            .await
    }

    async fn expired_timers(&self, as_of: DateTime<Utc>) -> Result<Vec<TimerEntry>, StoreError> {
        self.inner.expired_timers(as_of).await
    }
}

#[async_trait]
impl WritableEventStore for StaleActiveListStore {
    async fn append(
        &self,
        token: WriteToken,
        workflow_id: &WorkflowId,
        events: &[Event],
        expected_seq: u64,
    ) -> Result<(), StoreError> {
        self.inner
            .append(token, workflow_id, events, expected_seq)
            .await
    }

    async fn append_with_outbox(
        &self,
        token: WriteToken,
        workflow_id: &WorkflowId,
        events: &[Event],
        expected_seq: u64,
        outbox_rows: &[OutboxRow],
    ) -> Result<(), StoreError> {
        self.inner
            .append_with_outbox(token, workflow_id, events, expected_seq, outbox_rows)
            .await
    }

    async fn rearm_outbox_pending(&self, rows: &[OutboxRow]) -> Result<(), StoreError> {
        self.inner.rearm_outbox_pending(rows).await
    }

    async fn settle_outbox_row_cancelled(&self, dispatch_key: &str) -> Result<(), StoreError> {
        self.inner.settle_outbox_row_cancelled(dispatch_key).await
    }

    async fn settle_workflow_outbox_rows_cancelled(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<String>, StoreError> {
        self.inner
            .settle_workflow_outbox_rows_cancelled(workflow_id)
            .await
    }
}

#[async_trait]
impl PackageStore for StaleActiveListStore {
    async fn put_package(&self, record: PackageRecord) -> Result<(), StoreError> {
        self.inner.put_package(record).await
    }

    async fn put_package_with_routes(
        &self,
        record: PackageRecord,
        route_workflow_types: &[String],
    ) -> Result<(), StoreError> {
        self.inner
            .put_package_with_routes(record, route_workflow_types)
            .await
    }

    async fn list_packages(&self) -> Result<Vec<PackageRecord>, StoreError> {
        self.inner.list_packages().await
    }

    async fn delete_package(
        &self,
        workflow_type: &str,
        content_hash: &str,
    ) -> Result<(), StoreError> {
        self.inner.delete_package(workflow_type, content_hash).await
    }

    async fn put_package_route(
        &self,
        workflow_type: &str,
        content_hash: &str,
    ) -> Result<(), StoreError> {
        self.inner
            .put_package_route(workflow_type, content_hash)
            .await
    }

    async fn list_package_routes(&self) -> Result<Vec<PackageRouteRecord>, StoreError> {
        self.inner.list_package_routes().await
    }
}

#[cfg(test)]
mod tests;
