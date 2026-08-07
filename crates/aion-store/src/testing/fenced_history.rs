//! A store whose *history* can be made to refuse with [`StoreError::NotOwner`].
//!
//! `NotOwner` is the shard fence: this node is not the owner of the shard the
//! request landed on, so the answer is "ask a different owner", not "this failed".
//! It is the one store refusal a caller can act on by ROUTING, which is why every
//! layer above the store is supposed to carry it through as its own class rather
//! than collapsing it into a generic server error.
//!
//! A caller that reads the store DIRECTLY — rather than through an engine API
//! that wraps store failures in its own error type — has no wrapper to inspect
//! and so is the easiest place for that distinction to get dropped. The
//! [`InMemoryStore`] can never produce `NotOwner` (it owns everything), and a
//! distributed backend produces it only under a real ownership change, so the
//! drop is invisible to every test that uses an honest store. This double makes
//! it visible.
//!
//! # Why the refusal is armed rather than always on
//!
//! An engine reads its store while it is being built (the recovery scan) and
//! while a fixture seeds history. A store that refused from construction could
//! not be got into a useful state at all, so the refusal is switched on with
//! [`FencedHistoryStore::arm_fence`] once setup is done — which also gives a
//! test a genuine before/after control rather than a single measurement.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::memory::InMemoryStore;
use crate::package::{PackageRecord, PackageRouteRecord, PackageStore};
use crate::{
    Event, OutboxRow, ReadableEventStore, RunSummary, StoreError, TimerEntry, TimerId,
    WorkflowFilter, WorkflowId, WorkflowSummary, WritableEventStore, WriteToken,
};

/// The shard number the armed refusal reports.
///
/// Any value works — a caller can only route on the fact of `NotOwner`, not on
/// which shard it names — so this is fixed rather than configurable, and stated
/// here so a test can assert on the rendered message without guessing.
pub const REFUSED_SHARD: usize = 3;

/// [`crate::EventStore`] double whose history access refuses with
/// [`StoreError::NotOwner`] once armed.
///
/// Construct with [`FencedHistoryStore::new`], seed history through it exactly as
/// through any store, then call [`FencedHistoryStore::arm_fence`].
///
/// 🔴 THE FENCE COVERS HISTORY IN BOTH DIRECTIONS — the three reads AND the two
/// appends — because shard ownership governs both and this double answers
/// [`Self::is_current_owner`] with `false` while armed. Fencing only the reads
/// left it accepting an append for a shard it was simultaneously disclaiming,
/// which is a state no backend can occupy: a node that does not own the shard
/// cannot write to it either. Nothing on today's test path writes while armed
/// (fixtures seed first, then arm), so this cost nothing to get right and would
/// have been a trap for the next user, who would have read a successful append
/// as evidence the fence was not up.
///
/// Listings, timers, and the package store keep delegating to the inner
/// [`InMemoryStore`], so a fixture can still observe what was written while
/// history is fenced. Those are not history and are not claimed to be fenced.
pub struct FencedHistoryStore {
    inner: InMemoryStore,
    refusing: Arc<AtomicBool>,
}

impl FencedHistoryStore {
    /// Construct a double wrapping a fresh, empty [`InMemoryStore`] that refuses
    /// nothing. Until [`FencedHistoryStore::arm_fence`] is called it is
    /// indistinguishable from the inner store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: InMemoryStore::default(),
            refusing: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Arm the fence: every subsequent history read and history append refuses
    /// with [`StoreError::NotOwner`] naming [`REFUSED_SHARD`], and
    /// [`Self::is_current_owner`] answers `false` for every shard.
    pub fn arm_fence(&self) {
        self.refusing.store(true, Ordering::SeqCst);
    }

    /// Disarm the fence, restoring honest history access and ownership answers.
    pub fn disarm_fence(&self) {
        self.refusing.store(false, Ordering::SeqCst);
    }

    fn fence(&self) -> Result<(), StoreError> {
        if self.refusing.load(Ordering::SeqCst) {
            return Err(StoreError::NotOwner {
                shard: REFUSED_SHARD,
            });
        }
        Ok(())
    }
}

impl Default for FencedHistoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ReadableEventStore for FencedHistoryStore {
    async fn read_history(&self, workflow_id: &WorkflowId) -> Result<Vec<Event>, StoreError> {
        self.fence()?;
        self.inner.read_history(workflow_id).await
    }

    async fn read_history_from(
        &self,
        workflow_id: &WorkflowId,
        from_seq: u64,
    ) -> Result<Vec<Event>, StoreError> {
        self.fence()?;
        self.inner.read_history_from(workflow_id, from_seq).await
    }

    async fn read_run_chain(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<RunSummary>, StoreError> {
        self.fence()?;
        self.inner.read_run_chain(workflow_id).await
    }

    async fn list_active(&self) -> Result<Vec<WorkflowId>, StoreError> {
        self.inner.list_active().await
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

    /// Ownership answers consistently with the armed refusal: while refusing,
    /// this double does NOT claim [`REFUSED_SHARD`].
    ///
    /// 🔴 Delegating this one wholesale would put the double in a state no real
    /// backend can occupy. [`InMemoryStore`] owns every shard, so a plain
    /// forward has it answering "yes, I own shard 3" to an ownership query
    /// while answering `NotOwner { shard: 3 }` to every read of it — the exact
    /// contradiction a shard-ownership fixture exists to model the absence of.
    /// Nothing consults ownership on today's test path, which is what makes
    /// this cheap to get right now and a trap for the next user who does: they
    /// would route around a refusal the double insists cannot be happening.
    ///
    /// 🔴 AND IT MUST DISCLAIM EVERY SHARD, NOT JUST [`REFUSED_SHARD`] —
    /// because [`Self::fence`] is not shard-aware. An earlier revision denied
    /// only shard 3 and delegated the rest, on the reasoning that a double
    /// owning nothing could not distinguish "this node lost one shard" from
    /// "this node is not serving". True in principle, and it does not describe
    /// this double: `fence` refuses EVERY history read with
    /// `NotOwner { shard: 3 }` whatever shard the workflow actually maps to, so
    /// what the armed fence models is precisely "this node is not serving". A
    /// narrowed ownership answer left the same contradiction the paragraph
    /// above removes, one shard smaller: "yes, I own shard 5" beside
    /// `NotOwner { shard: 3 }` on a read of a shard-5 workflow.
    ///
    /// Modelling a single lost shard would need `fence` to know which shard a
    /// workflow hashes to, and that function lives in `aion-store-haematite` —
    /// the wrong direction for this crate to depend. Until it is available
    /// here, the honest double is the not-serving one, and this answer says so.
    fn is_current_owner(&self, shard: usize) -> bool {
        if self.refusing.load(Ordering::SeqCst) {
            return false;
        }
        self.inner.is_current_owner(shard)
    }

    fn publish_shard_owner(&self, shard: usize) -> Result<(), StoreError> {
        self.inner.publish_shard_owner(shard)
    }
}

#[async_trait]
impl WritableEventStore for FencedHistoryStore {
    /// Fenced for the same reason the reads are: the armed double owns no
    /// shard, and an append accepted by a non-owner is the contradiction this
    /// fixture exists to model the absence of. See the type doc.
    async fn append(
        &self,
        token: WriteToken,
        workflow_id: &WorkflowId,
        events: &[Event],
        expected_seq: u64,
    ) -> Result<(), StoreError> {
        self.fence()?;
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
        self.fence()?;
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
impl PackageStore for FencedHistoryStore {
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
mod tests {
    use super::{FencedHistoryStore, REFUSED_SHARD};

    /// F4: while the fence is armed the double must not claim ANY shard.
    ///
    /// `fence` is not shard-aware — it refuses every history read with
    /// `NotOwner { shard: 3 }` regardless of which shard the workflow maps to.
    /// So a double that still answered "yes, I own shard 5" would occupy a
    /// state no real backend can: owning a shard it refuses every read of.
    ///
    /// Killing mutation: restore the `shard == REFUSED_SHARD &&` conjunct in
    /// `is_current_owner`. The non-refused-shard assertion then fails.
    #[test]
    fn an_armed_fence_claims_no_shard_at_all() {
        let store = FencedHistoryStore::new();
        let other_shard = REFUSED_SHARD + 2;

        // CONTROL: unarmed, the double owns what the inner store owns — so the
        // assertions below measure the ARMING, not a fixture that never owned
        // anything.
        assert!(
            store.is_current_owner(REFUSED_SHARD),
            "control: an unarmed double must own the shard it will later refuse"
        );
        assert!(
            store.is_current_owner(other_shard),
            "control: an unarmed double must own the other shard too"
        );

        store.arm_fence();

        assert!(
            !store.is_current_owner(REFUSED_SHARD),
            "the refused shard must not be claimed while the fence is armed"
        );
        assert!(
            !store.is_current_owner(other_shard),
            "no OTHER shard may be claimed either: the fence refuses reads of every shard, so \
             claiming one is a state no real backend occupies"
        );

        // And it must be reversible, or the double could only ever be used once.
        store.disarm_fence();
        assert!(
            store.is_current_owner(other_shard),
            "disarming must restore honest ownership"
        );
    }

    use crate::{ReadableEventStore, StoreError, WorkflowId};

    /// The double must be honest until armed and refuse only afterwards — the
    /// before half is what makes an armed refusal attributable to the arming
    /// rather than to the fixture never having worked.
    #[tokio::test]
    async fn reads_are_honest_until_armed_and_refuse_after()
    -> Result<(), Box<dyn std::error::Error>> {
        let store = FencedHistoryStore::new();
        let workflow_id = WorkflowId::new_v4();

        assert!(
            store.read_history(&workflow_id).await.is_ok(),
            "an unarmed double must be indistinguishable from an honest store"
        );

        store.arm_fence();
        assert!(
            matches!(
                store.read_history(&workflow_id).await,
                Err(StoreError::NotOwner { shard }) if shard == REFUSED_SHARD
            ),
            "an armed double must refuse history reads with NotOwner"
        );
        assert!(
            matches!(
                store.read_run_chain(&workflow_id).await,
                Err(StoreError::NotOwner { shard }) if shard == REFUSED_SHARD
            ),
            "an armed double must refuse run-chain reads with NotOwner"
        );
        assert!(
            store.list_active().await.is_ok(),
            "only history reads are fenced — a fixture still needs the listings"
        );

        store.disarm_fence();
        assert!(
            store.read_history(&workflow_id).await.is_ok(),
            "disarming must restore honest reads"
        );
        Ok(())
    }

    /// The fence must cover APPENDS, not only reads.
    ///
    /// A node that does not own the shard cannot write to it, and this double
    /// answers `is_current_owner` with `false` for every shard while armed. An
    /// append that succeeded there would put the double in a state no backend
    /// can occupy — and because nothing on today's test path writes while
    /// armed, that state would be invisible until the user who did write read
    /// the success as evidence the fence was down.
    ///
    /// Killing mutation: delete `self.fence()?;` from `append`. The armed
    /// assertion then sees `Ok`.
    #[tokio::test]
    async fn appends_are_fenced_as_well_as_reads() -> Result<(), Box<dyn std::error::Error>> {
        use crate::{WritableEventStore, WriteToken};

        let store = FencedHistoryStore::new();
        let workflow_id = WorkflowId::new_v4();

        // CONTROL: unarmed, an empty append is accepted. Without this the armed
        // refusal below would be consistent with a double that never accepted
        // an append at all.
        assert!(
            store
                .append(WriteToken::recorder(), &workflow_id, &[], 0)
                .await
                .is_ok(),
            "control: an unarmed double must accept an append"
        );

        store.arm_fence();
        assert!(
            matches!(
                store
                    .append(WriteToken::recorder(), &workflow_id, &[], 0)
                    .await,
                Err(StoreError::NotOwner { shard }) if shard == REFUSED_SHARD
            ),
            "an armed double owns no shard, so it must refuse the append with NotOwner"
        );
        assert!(
            matches!(
                store
                    .append_with_outbox(WriteToken::recorder(), &workflow_id, &[], 0, &[])
                    .await,
                Err(StoreError::NotOwner { shard }) if shard == REFUSED_SHARD
            ),
            "the outbox append is the same history write and must refuse identically"
        );

        // SURVIVAL: the refusal was the fence and not a broken store.
        store.disarm_fence();
        assert!(
            store
                .append(WriteToken::recorder(), &workflow_id, &[], 0)
                .await
                .is_ok(),
            "disarming must restore honest appends"
        );
        Ok(())
    }
}
