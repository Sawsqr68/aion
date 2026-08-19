//! Unit tests for reopen validation, activity selection, and timer rearming.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use aion_core::{
    ActivityError, ActivityErrorKind, ActivityId, Event, EventEnvelope, Payload, RunId,
    SearchAttributeSchema, TimerId, WorkflowError, WorkflowFilter, WorkflowId, WorkflowStatus,
    WorkflowSummary, status_from_events,
};
use aion_package::ContentHash;
use aion_store::visibility::VisibilityStore;
use aion_store::{
    EventStore, InMemoryStore, PackageRecord, PackageRouteRecord, ReadableEventStore, RunSummary,
    StoreError, TimerEntry, WritableEventStore, WriteToken,
};
use chrono::{DateTime, Utc};
use serde_json::json;
use tokio::sync::Barrier;

use super::{
    ReopenWorkflowContext, reopen, reopened_failed_activities, validate_and_compute_reopened,
};
use crate::EngineError;
use crate::durability::{DurabilityError, Recorder, WorkflowStartRecord};
use crate::lifecycle::completion::{ProcessExitContext, handle_process_exit_attempt};
use crate::lifecycle::completion_retry::{TerminalIntent, TerminalProgress};
use crate::lifecycle::terminal_reconcile_gate;
use crate::loader::WorkflowCatalog;
use crate::registry::{
    CompletionNotifier, HandleResidency, Registry, WorkflowHandle, WorkflowHandleParts,
};
use crate::runtime::{RuntimeConfig, RuntimeHandle, WorkflowProcessOutcome};
use crate::supervision::SupervisionTree;

fn wf() -> WorkflowId {
    WorkflowId::new(uuid::Uuid::from_u128(1))
}

fn run() -> RunId {
    RunId::new(uuid::Uuid::from_u128(1))
}

fn envelope(seq: u64) -> EventEnvelope {
    EventEnvelope {
        seq,
        recorded_at: Utc::now(),
        workflow_id: wf(),
    }
}

fn payload() -> Payload {
    // Non-fallible: a fixed valid JSON byte string, so no expect/unwrap.
    Payload::new(aion_core::ContentType::Json, b"null".to_vec())
}

fn started() -> Event {
    Event::WorkflowStarted {
        envelope: envelope(1),
        workflow_type: String::from("stacked_dev"),
        input: payload(),
        run_id: run(),
        parent_run_id: None,
        parent_workflow_id: None,
        package_version: aion_core::PackageVersion::new("a".repeat(64)),
    }
}

fn scheduled(seq: u64, ordinal: u64) -> Event {
    Event::ActivityScheduled {
        envelope: envelope(seq),
        activity_id: ActivityId::from_sequence_position(ordinal),
        activity_type: String::from("dev_review"),
        input: payload(),
        task_queue: String::from("default"),
        node: None,
    }
}

fn activity_failed(seq: u64, ordinal: u64) -> Event {
    Event::ActivityFailed {
        envelope: envelope(seq),
        activity_id: ActivityId::from_sequence_position(ordinal),
        error: ActivityError {
            kind: ActivityErrorKind::Terminal,
            message: String::from("boom"),
            details: None,
        },
        attempt: 1,
    }
}

fn workflow_failed(seq: u64) -> Event {
    Event::WorkflowFailed {
        envelope: envelope(seq),
        error: WorkflowError {
            message: String::from("failed"),
            details: None,
        },
    }
}

#[test]
fn failed_run_computes_the_terminally_failed_step() -> Result<(), Box<dyn std::error::Error>> {
    let segment = vec![
        started(),
        scheduled(2, 0),
        activity_failed(3, 0),
        workflow_failed(4),
    ];
    let reopened = validate_and_compute_reopened(&wf(), &run(), &segment)?;
    assert_eq!(reopened, vec![ActivityId::from_sequence_position(0)]);
    Ok(())
}

#[test]
fn in_flight_activity_is_never_reopened() -> Result<(), Box<dyn std::error::Error>> {
    // Activity 1 was scheduled but had no terminal at crash time: it is
    // handled by ordinary recovery, not listed in the reopened set.
    let segment = vec![
        started(),
        scheduled(2, 0),
        activity_failed(3, 0),
        scheduled(4, 1),
        workflow_failed(5),
    ];
    let reopened = validate_and_compute_reopened(&wf(), &run(), &segment)?;
    assert_eq!(
        reopened,
        vec![ActivityId::from_sequence_position(0)],
        "only the terminally-failed step is reopened; the in-flight sibling is not"
    );
    Ok(())
}

#[test]
fn failed_then_succeeded_step_is_not_reopened() {
    let segment = vec![
        started(),
        scheduled(2, 0),
        activity_failed(3, 0),
        Event::ActivityCompleted {
            envelope: envelope(4),
            activity_id: ActivityId::from_sequence_position(0),
            result: payload(),
            attempt: 2,
        },
        workflow_failed(5),
    ];
    assert!(
        reopened_failed_activities(&segment).is_empty(),
        "a step that recovered before the failure is not re-driven"
    );
}

#[test]
fn concurrent_fan_out_reopens_every_failed_key() {
    let segment = vec![
        started(),
        scheduled(2, 0),
        scheduled(3, 1),
        activity_failed(4, 0),
        activity_failed(5, 1),
        workflow_failed(6),
    ];
    assert_eq!(
        reopened_failed_activities(&segment),
        vec![
            ActivityId::from_sequence_position(0),
            ActivityId::from_sequence_position(1),
        ]
    );
}

#[test]
fn cancelled_run_reopens_with_an_empty_set() -> Result<(), Box<dyn std::error::Error>> {
    let segment = vec![
        started(),
        scheduled(2, 0),
        Event::WorkflowCancelled {
            envelope: envelope(3),
            reason: String::from("operator stop"),
        },
    ];
    let reopened = validate_and_compute_reopened(&wf(), &run(), &segment)?;
    assert!(
        reopened.is_empty(),
        "a cancel records no terminal activity failure to re-drive"
    );
    Ok(())
}

#[test]
fn completed_run_is_rejected_as_invalid_state() {
    let segment = vec![
        started(),
        Event::WorkflowCompleted {
            envelope: envelope(2),
            result: payload(),
        },
    ];
    assert!(matches!(
        validate_and_compute_reopened(&wf(), &run(), &segment),
        Err(EngineError::InvalidState { .. })
    ));
}

#[test]
fn timed_out_run_is_rejected_as_invalid_state() {
    let segment = vec![
        started(),
        Event::WorkflowTimedOut {
            envelope: envelope(2),
            timeout: String::from("execution"),
        },
    ];
    assert!(matches!(
        validate_and_compute_reopened(&wf(), &run(), &segment),
        Err(EngineError::InvalidState { .. })
    ));
}

#[test]
fn running_run_is_rejected_as_invalid_state() {
    let segment = vec![started(), scheduled(2, 0)];
    assert!(matches!(
        validate_and_compute_reopened(&wf(), &run(), &segment),
        Err(EngineError::InvalidState { .. })
    ));
}

fn timer_started(seq: u64, timer_id: &aion_core::TimerId, fire_at_offset: i64) -> Event {
    Event::TimerStarted {
        envelope: envelope(seq),
        timer_id: timer_id.clone(),
        fire_at: Utc::now() + chrono::Duration::seconds(fire_at_offset),
    }
}

fn timer_cancelled(
    seq: u64,
    timer_id: &aion_core::TimerId,
    cause: aion_core::TimerCancelCause,
) -> Event {
    Event::TimerCancelled {
        envelope: envelope(seq),
        timer_id: timer_id.clone(),
        cause,
    }
}

fn workflow_cancelled(seq: u64) -> Event {
    Event::WorkflowCancelled {
        envelope: envelope(seq),
        reason: String::from("operator stop"),
    }
}

#[test]
fn teardown_cancelled_timer_is_rearmed_with_a_restart_marker() {
    use aion_core::{TimerCancelCause, TimerId};
    let named = TimerId::anonymous(1);
    let segment = vec![
        started(),
        timer_started(2, &named, 3600),
        timer_cancelled(3, &named, TimerCancelCause::CancelTeardown),
        workflow_cancelled(4),
    ];
    let rearm = super::rearmable_timers(&segment);
    assert_eq!(rearm.len(), 1);
    assert_eq!(rearm[0].timer_id, named);
    assert!(
        rearm[0].needs_restart_marker,
        "a teardown-cancelled timer needs a fresh TimerStarted to be live again"
    );
}

#[test]
fn workflow_intent_cancellation_is_never_resurrected() {
    use aion_core::{TimerCancelCause, TimerId};
    let named = TimerId::anonymous(1);
    let segment = vec![
        started(),
        timer_started(2, &named, 3600),
        timer_cancelled(3, &named, TimerCancelCause::WorkflowIntent),
        workflow_cancelled(4),
    ];
    assert!(
        super::rearmable_timers(&segment).is_empty(),
        "a timer the workflow retired is a settled business fact"
    );
}

#[test]
fn fired_timer_is_not_rearmed() {
    use aion_core::TimerId;
    let named = TimerId::anonymous(1);
    let segment = vec![
        started(),
        timer_started(2, &named, -5),
        Event::TimerFired {
            envelope: envelope(3),
            timer_id: named.clone(),
        },
        workflow_cancelled(4),
    ];
    assert!(super::rearmable_timers(&segment).is_empty());
}

#[test]
fn outstanding_timer_rearms_without_touching_history() {
    // The failed-run case: a failure tears no timers down, so the timer is
    // still outstanding by last-event-wins — only the wheel/row re-arms.
    use aion_core::TimerId;
    let named = TimerId::anonymous(1);
    let segment = vec![
        started(),
        timer_started(2, &named, 3600),
        workflow_failed(3),
    ];
    let rearm = super::rearmable_timers(&segment);
    assert_eq!(rearm.len(), 1);
    assert!(
        !rearm[0].needs_restart_marker,
        "an outstanding timer must not gain a duplicate TimerStarted"
    );
}

#[test]
fn rearm_keeps_the_original_fire_at_and_covers_multiple_timers() {
    use aion_core::{TimerCancelCause, TimerId};
    let deadline = TimerId::anonymous(1);
    let scope = TimerId::anonymous(2);
    let expected_deadline = Utc::now() + chrono::Duration::seconds(120);
    let segment = vec![
        started(),
        Event::TimerStarted {
            envelope: envelope(2),
            timer_id: deadline.clone(),
            fire_at: expected_deadline,
        },
        timer_started(3, &scope, 120),
        timer_cancelled(4, &deadline, TimerCancelCause::CancelTeardown),
        timer_cancelled(5, &scope, TimerCancelCause::CancelTeardown),
        workflow_cancelled(6),
    ];
    let rearm = super::rearmable_timers(&segment);
    assert_eq!(rearm.len(), 2, "both teardown-cancelled timers re-arm");
    let recovered = rearm
        .iter()
        .find(|timer| timer.timer_id == deadline)
        .map(|timer| timer.fire_at);
    assert_eq!(
        recovered,
        Some(expected_deadline),
        "reopen never moves a business deadline"
    );
}

#[test]
fn a_reopened_run_that_reterminated_reopens_from_the_new_failure() {
    // Failed -> Reopened -> Failed: the current lease's failure drives the set.
    let segment = vec![
        started(),
        scheduled(2, 0),
        activity_failed(3, 0),
        workflow_failed(4),
        Event::WorkflowReopened {
            envelope: envelope(5),
            run_id: run(),
            reopened: vec![ActivityId::from_sequence_position(0)],
        },
        scheduled(6, 1),
        activity_failed(7, 1),
        workflow_failed(8),
    ];
    let reopened = validate_and_compute_reopened(&wf(), &run(), &segment);
    assert!(
        matches!(
            reopened.as_deref(),
            Ok([id]) if *id == ActivityId::from_sequence_position(1)
        ),
        "only the current lease's failed step is reopened, not the superseded one: {reopened:?}"
    );
}

struct ResidentRun {
    context: ProcessExitContext,
    handle: WorkflowHandle,
    same_head_store: Arc<SameHeadStore>,
}

/// An in-memory store whose next two full-history reads return their snapshots
/// before either caller can proceed. This makes the concurrency twin genuinely
/// start from one terminal head instead of merely releasing two tasks together.
struct SameHeadStore {
    inner: Arc<InMemoryStore>,
    armed: AtomicBool,
    reads: AtomicUsize,
    barrier: Barrier,
}

impl SameHeadStore {
    fn new(inner: Arc<InMemoryStore>) -> Self {
        Self {
            inner,
            armed: AtomicBool::new(false),
            reads: AtomicUsize::new(0),
            barrier: Barrier::new(2),
        }
    }

    fn arm(&self) {
        self.reads.store(0, Ordering::SeqCst);
        self.armed.store(true, Ordering::SeqCst);
    }
}

#[async_trait::async_trait]
impl ReadableEventStore for SameHeadStore {
    async fn read_history(&self, workflow_id: &WorkflowId) -> Result<Vec<Event>, StoreError> {
        let history = self.inner.read_history(workflow_id).await?;
        if self.armed.load(Ordering::SeqCst) {
            let read = self.reads.fetch_add(1, Ordering::SeqCst);
            if read < 2 {
                self.barrier.wait().await;
                if read == 1 {
                    self.armed.store(false, Ordering::SeqCst);
                }
            }
        }
        Ok(history)
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

    async fn list_active(&self) -> Result<Vec<WorkflowId>, StoreError> {
        self.inner.list_active().await
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

#[async_trait::async_trait]
impl WritableEventStore for SameHeadStore {
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
}

#[async_trait::async_trait]
impl aion_store::PackageStore for SameHeadStore {
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

async fn resident_run() -> Result<ResidentRun, Box<dyn std::error::Error>> {
    let backing = Arc::new(InMemoryStore::default());
    let same_head_store = Arc::new(SameHeadStore::new(Arc::clone(&backing)));
    let store = Arc::clone(&same_head_store) as Arc<dyn EventStore>;
    let visibility_store = backing as Arc<dyn VisibilityStore>;
    let registry = Arc::new(Registry::default());
    let workflow_id = WorkflowId::new_v4();
    let run_id = RunId::new_v4();
    let package_hash = ContentHash::from_bytes([0xaa; 32]);
    let mut recorder = Recorder::new(workflow_id.clone(), Arc::clone(&store));
    recorder
        .record_workflow_started(
            Utc::now(),
            WorkflowStartRecord {
                workflow_type: "checkout".to_owned(),
                input: Payload::from_json(&json!({ "order": 51 }))?,
                run_id: run_id.clone(),
                parent_run_id: None,
                parent_workflow_id: None,
                package_version: aion_core::PackageVersion::new(package_hash.to_string()),
            },
        )
        .await?;
    let handle = WorkflowHandle::new(WorkflowHandleParts {
        workflow_id: workflow_id.clone(),
        run_id: run_id.clone(),
        // Keep the synthetic terminal leftover outside the runtime's pid range.
        // The runtime's first spawned test process can legitimately be pid 1,
        // and `remove_if_pid` identifies leases by pid.
        pid: u64::MAX,
        workflow_type: "checkout".to_owned(),
        namespace: String::from("default"),
        loaded_version: package_hash.clone(),
        cached_status: WorkflowStatus::Running,
        residency: HandleResidency::Resident,
        recorder,
        completion: CompletionNotifier::new(),
    });
    registry.insert((workflow_id, run_id), handle.clone())?;

    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    runtime.register_waiting_test_module("checkout_deployed", "run");
    let catalog = Arc::new(WorkflowCatalog::new());
    catalog.note_loaded_workflow_for_test("checkout", "checkout_deployed", "run", package_hash);

    Ok(ResidentRun {
        context: ProcessExitContext {
            store,
            visibility_store,
            registry,
            catalog,
            runtime,
            supervision: Arc::new(SupervisionTree::new()),
            tokio_handle: tokio::runtime::Handle::current(),
            search_attribute_schema: Arc::new(SearchAttributeSchema::new()),
        },
        handle,
        same_head_store,
    })
}

fn reopen_context(context: &ProcessExitContext) -> ReopenWorkflowContext<'_> {
    ReopenWorkflowContext {
        store: Arc::clone(&context.store),
        visibility_store: Arc::clone(&context.visibility_store),
        catalog: Arc::clone(&context.catalog),
        runtime: &context.runtime,
        supervision: Arc::clone(&context.supervision),
        registry: &context.registry,
        search_attribute_schema: Arc::clone(&context.search_attribute_schema),
    }
}

fn failed_intent() -> TerminalIntent {
    TerminalIntent::from_outcome(Ok(WorkflowProcessOutcome::Failed(WorkflowError {
        message: String::from("resident workflow failed"),
        details: None,
    })))
}

async fn drive_failed_terminal(run: &ResidentRun) -> Result<(), EngineError> {
    let mut progress = TerminalProgress::NotRecorded;
    handle_process_exit_attempt(&run.context, &run.handle, &failed_intent(), &mut progress).await
}

fn reopened_count(history: &[Event]) -> usize {
    history
        .iter()
        .filter(|event| matches!(event, Event::WorkflowReopened { .. }))
        .count()
}

#[tokio::test(flavor = "multi_thread")]
async fn reopen_after_result_before_terminal_registry_reconcile_succeeds()
-> Result<(), Box<dyn std::error::Error>> {
    let resident = resident_run().await?;
    let workflow_id = resident.handle.workflow_id().clone();
    let run_id = resident.handle.run_id().clone();
    let gate = terminal_reconcile_gate::arm(&workflow_id, &run_id);
    assert!(
        !gate.was_reached(),
        "the gate starts before the terminal path"
    );

    let terminal_context = resident.context.clone();
    let terminal_handle = resident.handle.clone();
    let terminal_task = tokio::spawn(async move {
        let mut progress = TerminalProgress::NotRecorded;
        handle_process_exit_attempt(
            &terminal_context,
            &terminal_handle,
            &failed_intent(),
            &mut progress,
        )
        .await
    });

    gate.wait_until_reached().await;
    assert!(
        gate.was_reached(),
        "the completion path reached the armed hold"
    );
    let history_at_hold = resident.context.store.read_history(&workflow_id).await?;
    assert_eq!(status_from_events(&history_at_hold), WorkflowStatus::Failed);
    let registered_at_hold = resident
        .context
        .registry
        .get(&workflow_id, &run_id)?
        .ok_or("the resident handle disappeared before reopen")?;
    assert_eq!(registered_at_hold.cached_status(), WorkflowStatus::Running);

    let reopen_result = reopen(reopen_context(&resident.context), &workflow_id, &run_id).await;
    drop(gate);
    terminal_task.await??;

    let reopened = match reopen_result {
        Ok(handle) => handle,
        Err(error) => {
            resident.context.runtime.shutdown()?;
            return Err(format!(
                "reopen must succeed while history is Failed and the lagging handle caches Running; got {error}"
            )
            .into());
        }
    };
    assert_eq!(reopened.cached_status(), WorkflowStatus::Running);
    let history = resident.context.store.read_history(&workflow_id).await?;
    assert_eq!(status_from_events(&history), WorkflowStatus::Running);
    assert_eq!(reopened_count(&history), 1);
    resident.context.runtime.shutdown()?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn two_reopens_from_one_terminal_head_admit_exactly_one_writer()
-> Result<(), Box<dyn std::error::Error>> {
    let resident = resident_run().await?;
    let workflow_id = resident.handle.workflow_id().clone();
    let run_id = resident.handle.run_id().clone();
    drive_failed_terminal(&resident).await?;

    resident.same_head_store.arm();
    let barrier = Arc::new(Barrier::new(3));
    let first_context = resident.context.clone();
    let first_id = workflow_id.clone();
    let first_run = run_id.clone();
    let first_barrier = Arc::clone(&barrier);
    let first = tokio::spawn(async move {
        first_barrier.wait().await;
        reopen(reopen_context(&first_context), &first_id, &first_run).await
    });
    let second_context = resident.context.clone();
    let second_id = workflow_id.clone();
    let second_run = run_id.clone();
    let second_barrier = Arc::clone(&barrier);
    let second = tokio::spawn(async move {
        second_barrier.wait().await;
        reopen(reopen_context(&second_context), &second_id, &second_run).await
    });
    barrier.wait().await;

    let first_result = first.await?;
    let second_result = second.await?;
    let history = resident.context.store.read_history(&workflow_id).await?;
    assert_eq!(
        reopened_count(&history),
        1,
        "one terminal history head admits exactly one WorkflowReopened writer: {history:#?}"
    );

    let successes = usize::from(first_result.is_ok()) + usize::from(second_result.is_ok());
    assert_eq!(successes, 1, "exactly one reopen caller succeeds");
    let (winner, loser) = match (first_result, second_result) {
        (Ok(winner), Err(loser)) | (Err(loser), Ok(winner)) => (winner, loser),
        (first, second) => {
            resident.context.runtime.shutdown()?;
            return Err(
                format!("expected one winner and one loser, got {first:?} and {second:?}").into(),
            );
        }
    };
    let legitimate_loser = match &loser {
        EngineError::InvalidState { reason } => reason.contains("concurrent reopen"),
        EngineError::Durability(DurabilityError::Store(
            aion_store::StoreError::SequenceConflict { .. },
        )) => true,
        _ => false,
    };
    assert!(
        legitimate_loser,
        "the losing reopen must see a replaced lease or a sequence conflict, got {loser}"
    );

    let registered = resident
        .context
        .registry
        .get(&workflow_id, &run_id)?
        .ok_or("the winning reopen handle was evicted")?;
    assert_eq!(registered.pid(), winner.pid());
    resident.context.runtime.shutdown()?;
    Ok(())
}
