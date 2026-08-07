//! A store that injects transient durable failures, shared by every test that
//! must prove what a durable call does when the store refuses it.
//!
//! One copy, deliberately. It began private inside the child-watch tests; the
//! process-exit completion retry needs exactly the same fault, and a second copy
//! is how two callers of one rule start disagreeing about what "transient"
//! means. Reaching for it from another module is the cheaper half of the trade.
//!
//! **Not every consumer is a retry path, and calling this a retry fixture was
//! wrong.** Two of the three retry: the child watcher re-attempts its
//! parent-side append, and the process-exit monitor re-attempts its history
//! read. The third does not — the continue-as-new epoch gate
//! ([`crate::runtime::nif_continue_as_new`]) uses an injected append failure to
//! produce a refusal that is REFUSED ONCE and never retried, in order to prove
//! the calling workflow process is SPARED. A fixture described as serving
//! retries only would look inapplicable to exactly the case that needs it most.
//!
//! Both directions are injectable because these paths fail in different places:
//! the child watcher fails on its parent-side APPEND, while the process-exit
//! monitor's first fallible call is a history READ. A fixture that could only
//! fail writes would leave the read untested while looking like coverage.
//!
//! The read fault also comes in three KINDS, because "the store refused" and
//! "the store refused for a reason no retry can repair" must reach the code
//! under test as different values — and there is more than one of the latter. A
//! fixture that could only inject [`aion_store::StoreError::Backend`] would let
//! a classifier that retries everything pass every test it has.

use aion_core::{Event, WorkflowId};
use aion_store::{InMemoryStore, ReadableEventStore};

pub(crate) struct FlakyStore {
    inner: InMemoryStore,
    /// Appends to let through before the refusals below begin.
    append_failures_to_skip: std::sync::atomic::AtomicU32,
    /// Appends still to be refused before the wrapped store is reached.
    remaining_append_failures: std::sync::atomic::AtomicU32,
    /// History reads to let through before the refusals below begin.
    read_failures_to_skip: std::sync::atomic::AtomicU32,
    /// History reads still to be refused before the wrapped store is reached.
    remaining_read_failures: std::sync::atomic::AtomicU32,
    /// History reads still to be refused with a NON-transient conflict.
    remaining_read_conflicts: std::sync::atomic::AtomicU32,
    /// History reads still to be refused with a lost-ownership refusal.
    remaining_read_lost_ownership: std::sync::atomic::AtomicU32,
}

impl FlakyStore {
    pub(crate) fn new() -> Self {
        Self {
            inner: InMemoryStore::default(),
            append_failures_to_skip: std::sync::atomic::AtomicU32::new(0),
            remaining_append_failures: std::sync::atomic::AtomicU32::new(0),
            read_failures_to_skip: std::sync::atomic::AtomicU32::new(0),
            remaining_read_failures: std::sync::atomic::AtomicU32::new(0),
            remaining_read_conflicts: std::sync::atomic::AtomicU32::new(0),
            remaining_read_lost_ownership: std::sync::atomic::AtomicU32::new(0),
        }
    }

    /// Refuse the next `count` appends, then behave normally.
    pub(crate) fn fail_next_appends(&self, count: u32) {
        self.fail_appends_after(0, count);
    }

    /// Let `skip` appends through, then refuse the next `count`.
    ///
    /// The write-side twin of [`Self::fail_reads_after`], and it exists for the
    /// same reason that one does. One durable TRANSITION can be several appends
    /// — continue-as-new writes its `WorkflowContinuedAsNew` terminal and then
    /// retires the predecessor's deadline as two separate `append_with` calls —
    /// and those appends sit on OPPOSITE sides of the terminal. A budget that
    /// can only refuse from the first append can therefore only ever produce
    /// failures BEFORE a terminal lands, which makes "this failure happened
    /// AFTER the terminal was durable" untestable while looking covered. That
    /// gap is not hypothetical: it is precisely the case where a caller must
    /// still end the workflow process, and keying that decision on the error
    /// alone gets it wrong.
    ///
    /// [`Self::fail_next_appends`] is this with `skip = 0`; one budget, not two,
    /// so the two cannot drift apart.
    pub(crate) fn fail_appends_after(&self, skip: u32, count: u32) {
        self.append_failures_to_skip
            .store(skip, std::sync::atomic::Ordering::Release);
        self.remaining_append_failures
            .store(count, std::sync::atomic::Ordering::Release);
    }

    /// Refuse the next `count` history reads, then behave normally.
    pub(crate) fn fail_next_reads(&self, count: u32) {
        self.fail_reads_after(0, count);
    }

    /// Let `skip` history reads through, then refuse the next `count`.
    ///
    /// One operation can read history several times — the attempt's own read,
    /// then the visibility upsert's, then the registry reconcile's — and those
    /// reads sit on OPPOSITE sides of the durable append. A budget that can only
    /// refuse from the first read can therefore only ever produce failures
    /// BEFORE the terminal lands, which makes "this failure happened after the
    /// terminal was recorded" untestable while looking like it is covered.
    ///
    /// [`Self::fail_next_reads`] is this with `skip = 0`; there is one budget,
    /// not two, so the two cannot drift apart.
    pub(crate) fn fail_reads_after(&self, skip: u32, count: u32) {
        self.read_failures_to_skip
            .store(skip, std::sync::atomic::Ordering::Release);
        self.remaining_read_failures
            .store(count, std::sync::atomic::Ordering::Release);
    }

    /// Refuse the next `count` history reads with a sequence conflict.
    ///
    /// [`aion_store::StoreError::SequenceConflict`] is the store contract's
    /// double-writer indicator (CLAUDE.md invariant 3: exactly one `Recorder`
    /// per active workflow). Retrying past one would have a second writer keep
    /// re-attempting a history another writer already owns, so this budget
    /// exists to prove the classifier abandons it instead.
    ///
    /// Kept separate from [`Self::fail_next_reads`] rather than parameterised:
    /// a test sets exactly one budget, and the read site drains this one first
    /// so a test that set both would still see the conflict it asked for.
    pub(crate) fn fail_next_reads_with_conflict(&self, count: u32) {
        self.remaining_read_conflicts
            .store(count, std::sync::atomic::Ordering::Release);
    }

    /// Refuse the next `count` history reads with a lost-ownership refusal.
    ///
    /// [`aion_store::StoreError::NotOwner`] is the third KIND, and it is a
    /// distinct kind for the same reason the conflict is. Its own contract doc
    /// says the caller should *re-resolve the shard's owner* and retry or
    /// forward; a loop that has no re-resolution step and simply sleeps is not
    /// performing that remedy, it is hammering a shard this node has
    /// permanently lost. Injectable so the classifier can be proven to abandon
    /// it rather than spin — a fixture that only offered
    /// [`aion_store::StoreError::Backend`] would let "retry everything that is
    /// not a conflict" pass.
    pub(crate) fn fail_next_reads_with_lost_ownership(&self, count: u32) {
        self.remaining_read_lost_ownership
            .store(count, std::sync::atomic::Ordering::Release);
    }

    /// Ground truth, read PAST the injector.
    ///
    /// An oracle must not be subject to the treatment it measures. Observing a
    /// retry through the faulted interface draws from the same failure budget as
    /// the code under test, so the test's own poll can be refused — which is a
    /// failure of the instrument reported as a failure of the fix. This reads the
    /// wrapped store directly and can therefore only ever report what is durably
    /// recorded.
    pub(crate) async fn recorded_history(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<Event>, aion_store::StoreError> {
        self.inner.read_history(workflow_id).await
    }

    /// How much of the append-failure budget is still unspent.
    ///
    /// An injected fault that is never reached looks exactly like a working
    /// system. A test whose subject is "this call was REFUSED and its caller
    /// survived" therefore has two claims to prove, not one, and the second —
    /// that the call happened at all — has no other witness: a refusal writes
    /// nothing to history, so an unmade call and a refused one leave identical
    /// stores. Draining the budget to zero is the difference.
    ///
    /// 🔴 THE APPEND BUDGET, NOT THE READ BUDGET, AND THE DIFFERENCE IS
    /// ATTRIBUTION. The read budget is spent by whoever reads first, and in a
    /// live-process test that is not the call under test — a fixture polling a
    /// pure NIF to discover when the test has finished arranging its world
    /// drains it long before the durable call is made. The witness then reports
    /// "the fault was consumed" about a completely different caller, and the
    /// call under test proceeds against a healthy store.
    ///
    /// ⚠️ **That is a property of the SETUP, not of this type.** Nothing here
    /// restricts who may append; any caller holding this store drains the same
    /// budget, and a shared store's seeding writes drain it first of all. What
    /// makes the witness attributable is that the *calling test* leaves no other
    /// appender running inside the window it measures — the seeding appends are
    /// counted and skipped with [`Self::fail_appends_after`], and the fixture's
    /// own polling reads cannot touch this budget at all. Reuse it under those
    /// conditions or not at all.
    pub(crate) fn unspent_append_failures(&self) -> u32 {
        self.remaining_append_failures
            .load(std::sync::atomic::Ordering::Acquire)
    }

    /// Decrement one budget, reporting whether this call is refused.
    fn take_failure(budget: &std::sync::atomic::AtomicU32) -> bool {
        budget
            .fetch_update(
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
                |current| current.checked_sub(1),
            )
            .is_ok()
    }
}

#[async_trait::async_trait]
impl aion_store::ReadableEventStore for FlakyStore {
    async fn read_history(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<Event>, aion_store::StoreError> {
        if Self::take_failure(&self.remaining_read_conflicts) {
            return Err(aion_store::StoreError::SequenceConflict {
                expected: 0,
                found: 1,
            });
        }
        if Self::take_failure(&self.remaining_read_lost_ownership) {
            return Err(aion_store::StoreError::NotOwner { shard: 7 });
        }
        if !Self::take_failure(&self.read_failures_to_skip)
            && Self::take_failure(&self.remaining_read_failures)
        {
            return Err(aion_store::StoreError::Backend(
                "transient history read failure injected by FlakyStore".to_owned(),
            ));
        }
        self.inner.read_history(workflow_id).await
    }

    async fn read_history_from(
        &self,
        workflow_id: &WorkflowId,
        from_seq: u64,
    ) -> Result<Vec<Event>, aion_store::StoreError> {
        self.inner.read_history_from(workflow_id, from_seq).await
    }

    async fn read_run_chain(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<aion_store::RunSummary>, aion_store::StoreError> {
        self.inner.read_run_chain(workflow_id).await
    }

    async fn list_workflow_ids(&self) -> Result<Vec<WorkflowId>, aion_store::StoreError> {
        self.inner.list_workflow_ids().await
    }

    async fn list_active(&self) -> Result<Vec<WorkflowId>, aion_store::StoreError> {
        self.inner.list_active().await
    }

    async fn list_paused(&self) -> Result<Vec<WorkflowId>, aion_store::StoreError> {
        self.inner.list_paused().await
    }

    async fn query(
        &self,
        filter: &aion_core::WorkflowFilter,
    ) -> Result<Vec<aion_core::WorkflowSummary>, aion_store::StoreError> {
        self.inner.query(filter).await
    }

    async fn schedule_timer(
        &self,
        workflow_id: &WorkflowId,
        timer_id: &aion_core::TimerId,
        fire_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), aion_store::StoreError> {
        self.inner
            .schedule_timer(workflow_id, timer_id, fire_at)
            .await
    }

    async fn expired_timers(
        &self,
        as_of: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<aion_store::TimerEntry>, aion_store::StoreError> {
        self.inner.expired_timers(as_of).await
    }
}

#[async_trait::async_trait]
impl aion_store::WritableEventStore for FlakyStore {
    async fn append(
        &self,
        token: aion_store::WriteToken,
        workflow_id: &WorkflowId,
        events: &[Event],
        expected_seq: u64,
    ) -> Result<(), aion_store::StoreError> {
        if !Self::take_failure(&self.append_failures_to_skip)
            && Self::take_failure(&self.remaining_append_failures)
        {
            return Err(aion_store::StoreError::Backend(
                "transient append failure injected by FlakyStore".to_owned(),
            ));
        }
        self.inner
            .append(token, workflow_id, events, expected_seq)
            .await
    }
}

/// Package persistence is untouched by the injected failures: forward to the
/// wrapped in-memory store.
#[async_trait::async_trait]
impl aion_store::PackageStore for FlakyStore {
    async fn put_package(
        &self,
        record: aion_store::PackageRecord,
    ) -> Result<(), aion_store::StoreError> {
        self.inner.put_package(record).await
    }

    async fn put_package_with_routes(
        &self,
        record: aion_store::PackageRecord,
        route_workflow_types: &[String],
    ) -> Result<(), aion_store::StoreError> {
        self.inner
            .put_package_with_routes(record, route_workflow_types)
            .await
    }

    async fn list_packages(
        &self,
    ) -> Result<Vec<aion_store::PackageRecord>, aion_store::StoreError> {
        self.inner.list_packages().await
    }

    async fn delete_package(
        &self,
        workflow_type: &str,
        content_hash: &str,
    ) -> Result<(), aion_store::StoreError> {
        self.inner.delete_package(workflow_type, content_hash).await
    }

    async fn put_package_route(
        &self,
        workflow_type: &str,
        content_hash: &str,
    ) -> Result<(), aion_store::StoreError> {
        self.inner
            .put_package_route(workflow_type, content_hash)
            .await
    }

    async fn list_package_routes(
        &self,
    ) -> Result<Vec<aion_store::PackageRouteRecord>, aion_store::StoreError> {
        self.inner.list_package_routes().await
    }
}
