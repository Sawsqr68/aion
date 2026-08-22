//! Wheel-level tests for the deadline fire re-drive (`fire_wheel_timer`).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use aion_core::{
    Event, PackageVersion, Payload, RunId, TimerId, WorkflowFilter, WorkflowId, WorkflowSummary,
};
use aion_store::{
    EventStore, InMemoryStore, PackageRecord, PackageRouteRecord, ReadableEventStore, RunSummary,
    StoreError, TimerEntry, WritableEventStore, WriteToken,
};
use chrono::{DateTime, Utc};

use super::{fire_wheel_timer, install_timer_nif_bridge, register_deadline_handler};
use crate::durability::{Recorder, WorkflowStartRecord};
use crate::registry::Registry;
use crate::runtime::{RuntimeConfig, RuntimeHandle};
use crate::time::{DeadlineHandler, DeadlineHandlerError};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// An [`EventStore`] wrapping an in-memory store with bounded, deterministic
/// fault injection: `read_history` can fail outright for the next N calls, and
/// `append` can either be REFUSED (nothing lands) or suffer an
/// acknowledgement loss (the write LANDS in the inner store and the caller
/// still sees `Err` — the aion#145 incident shape). Every other operation
/// delegates unconditionally.
struct FaultableStore {
    inner: Arc<InMemoryStore>,
    fail_reads: AtomicUsize,
    refuse_appends: AtomicUsize,
    lose_append_acks: AtomicUsize,
}

impl FaultableStore {
    fn new(inner: Arc<InMemoryStore>) -> Self {
        Self {
            inner,
            fail_reads: AtomicUsize::new(0),
            refuse_appends: AtomicUsize::new(0),
            lose_append_acks: AtomicUsize::new(0),
        }
    }

    /// Arms the next `count` `read_history` calls to fail.
    fn fail_next_reads(&self, count: usize) {
        self.fail_reads.store(count, Ordering::SeqCst);
    }

    /// Arms the next `count` `append` calls to be refused with NOTHING landing.
    fn refuse_next_appends(&self, count: usize) {
        self.refuse_appends.store(count, Ordering::SeqCst);
    }

    /// Arms the next `count` `append` calls to LAND in the inner store while
    /// the caller still receives an error — the ack-loss incident shape.
    fn lose_next_append_acks(&self, count: usize) {
        self.lose_append_acks.store(count, Ordering::SeqCst);
    }

    /// Consumes one armed fault from `counter`, returning whether it fired.
    fn take_fault(counter: &AtomicUsize) -> bool {
        let mut current = counter.load(Ordering::SeqCst);
        loop {
            if current == 0 {
                return false;
            }
            match counter.compare_exchange(current, current - 1, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return true,
                Err(actual) => current = actual,
            }
        }
    }
}

#[async_trait::async_trait]
impl ReadableEventStore for FaultableStore {
    async fn read_history(&self, workflow_id: &WorkflowId) -> Result<Vec<Event>, StoreError> {
        if Self::take_fault(&self.fail_reads) {
            return Err(StoreError::Backend(
                "simulated store outage during read_history".to_owned(),
            ));
        }
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
impl WritableEventStore for FaultableStore {
    async fn append(
        &self,
        token: WriteToken,
        workflow_id: &WorkflowId,
        events: &[Event],
        expected_seq: u64,
    ) -> Result<(), StoreError> {
        if Self::take_fault(&self.refuse_appends) {
            return Err(StoreError::Backend(
                "simulated store outage: append refused before any write landed".to_owned(),
            ));
        }
        if Self::take_fault(&self.lose_append_acks) {
            // The write LANDS; only the acknowledgement is lost. A genuine inner
            // failure (e.g. a sequence conflict) still surfaces as itself —
            // masking it would let the fault injector manufacture the incident
            // on top of an append the store never took.
            self.inner
                .append(token, workflow_id, events, expected_seq)
                .await?;
            return Err(StoreError::Backend(
                "simulated acknowledgement loss: the append landed but the caller's ack timed out"
                    .to_owned(),
            ));
        }
        self.inner
            .append(token, workflow_id, events, expected_seq)
            .await
    }
}

#[async_trait::async_trait]
impl aion_store::PackageStore for FaultableStore {
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

/// A deadline handler that counts the fires routed to it.
#[derive(Default)]
struct RecordingDeadlineHandler {
    calls: AtomicUsize,
}

impl RecordingDeadlineHandler {
    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl DeadlineHandler for RecordingDeadlineHandler {
    async fn on_deadline_elapsed(
        &self,
        _workflow_id: WorkflowId,
        _run_id: RunId,
    ) -> Result<(), DeadlineHandlerError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// F1 re-drive: a store outage that fails BOTH the fire-path liveness read AND
/// the immediately-following `deadline_remains_live` read on the first attempt
/// must NOT kill the wheel task. Once the outage clears (before the retry), a
/// later attempt drives the deadline fire to the handler. Mutation-sensitive: the
/// pre-fix code returned on the liveness-read error, so the handler would never
/// be called.
#[tokio::test(flavor = "multi_thread")]
async fn deadline_fire_retries_through_a_store_outage_spanning_both_reads() -> TestResult {
    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    let inner = Arc::new(InMemoryStore::default());
    let workflow_id = WorkflowId::new_v4();
    let run_id = RunId::new_v4();
    let deadline_id = crate::time::deadline_timer_id(&run_id)?;
    // Seed a live deadline in history BEFORE arming the outage.
    let event_store: Arc<dyn EventStore> = Arc::clone(&inner) as Arc<dyn EventStore>;
    let mut recorder = Recorder::new(workflow_id.clone(), event_store);
    recorder
        .record_workflow_started(
            Utc::now(),
            WorkflowStartRecord {
                workflow_type: "sleeper".to_owned(),
                input: Payload::from_json(&serde_json::json!({}))?,
                run_id: run_id.clone(),
                parent_run_id: None,
                parent_workflow_id: None,
                package_version: PackageVersion::new("a".repeat(64)),
            },
        )
        .await?;
    recorder
        .record_timer_started(Utc::now(), deadline_id.clone(), Utc::now())
        .await?;

    let flaky = Arc::new(FaultableStore::new(inner));
    let registry = Arc::new(Registry::default());
    install_timer_nif_bridge(
        runtime.nif_state(),
        Arc::clone(&registry),
        Arc::clone(&flaky) as Arc<dyn EventStore>,
        tokio::runtime::Handle::current(),
        runtime.signal_delivery(),
    );
    let handler = Arc::new(RecordingDeadlineHandler::default());
    // This double ignores the wheel's stand-down latch on purpose: this test is
    // about the retry ladder, and the wheel is never torn down in it.
    register_deadline_handler(runtime.nif_state(), |_| {
        Arc::clone(&handler) as Arc<dyn DeadlineHandler>
    })
    .map_err(|error| format!("failed to register deadline handler: {error}"))?;

    // Fail the next two reads: the first attempt's fire-path liveness read and its
    // immediate deadline_remains_live read. The third read (attempt two) succeeds.
    flaky.fail_next_reads(2);

    fire_wheel_timer(
        &Arc::downgrade(runtime.nif_state()),
        &workflow_id,
        &deadline_id,
        Utc::now(),
    )
    .await;

    assert!(
        handler.call_count() >= 1,
        "a later retry attempt drove the deadline fire to the handler after the double-read outage"
    );
    runtime.shutdown()?;
    Ok(())
}

/// Shared fixture for the aion#145 ordinary-timer ladder tests: a runtime with
/// a REAL inert live process (so a delivered wake goes through the genuine
/// `wake_workflow` path instead of erroring on a fake pid), a resident
/// registered workflow whose Recorder writes THROUGH the [`FaultableStore`],
/// and an ordinary named timer recorded live in durable history through that
/// same recorder — history `[WorkflowStarted(1), TimerStarted(2)]`, recorder
/// head `2`.
struct OrdinaryFireFixture {
    runtime: Arc<crate::runtime::RuntimeHandle>,
    inner: Arc<InMemoryStore>,
    faulted: Arc<FaultableStore>,
    registry: Arc<Registry>,
    workflow_id: WorkflowId,
    run_id: RunId,
    timer_id: TimerId,
}

impl OrdinaryFireFixture {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
        let inner = Arc::new(InMemoryStore::default());
        let faulted = Arc::new(FaultableStore::new(Arc::clone(&inner)));
        let registry = Arc::new(Registry::default());
        let pid = runtime.spawn_test_process()?;
        let store: Arc<dyn EventStore> = Arc::clone(&faulted) as Arc<dyn EventStore>;
        let (workflow_id, run_id) = seed_resident_workflow(&registry, &store, pid).await?;
        install_timer_nif_bridge(
            runtime.nif_state(),
            Arc::clone(&registry),
            Arc::clone(&faulted) as Arc<dyn EventStore>,
            tokio::runtime::Handle::current(),
            runtime.signal_delivery(),
        );
        let timer_id = TimerId::named("ordinary-loop")?;
        let handle = registry
            .get(&workflow_id, &run_id)?
            .ok_or("the seeded workflow must be registered")?;
        let recorder = handle.recorder();
        recorder
            .lock()
            .await
            .record_timer_started(Utc::now(), timer_id.clone(), Utc::now())
            .await?;
        Ok(Self {
            runtime,
            inner,
            faulted,
            registry,
            workflow_id,
            run_id,
            timer_id,
        })
    }

    /// Drive the wheel's fire ladder for the fixture's ordinary timer.
    async fn fire_through_ladder(&self) {
        fire_wheel_timer(
            &Arc::downgrade(self.runtime.nif_state()),
            &self.workflow_id,
            &self.timer_id,
            Utc::now(),
        )
        .await;
    }

    /// Count of durable `TimerFired` events for the fixture's timer.
    async fn fired_count(&self) -> Result<usize, Box<dyn std::error::Error>> {
        Ok(self
            .inner
            .read_history(&self.workflow_id)
            .await?
            .iter()
            .filter(|event| {
                matches!(event, Event::TimerFired { timer_id, .. } if timer_id == &self.timer_id)
            })
            .count())
    }

    /// Assert the registered handle's Recorder tracks `expected_head`, then
    /// prove the workflow's NEXT append through that SAME recorder lands at
    /// `expected_head + 1` — the assertion that catches a stale-low recorder
    /// as the `SequenceConflict` wedge it would become one event later.
    async fn assert_recorder_healthy_at(
        &self,
        expected_head: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let handle = self
            .registry
            .get(&self.workflow_id, &self.run_id)?
            .ok_or("the workflow handle must still be registered")?;
        let recorder = handle.recorder();
        let mut recorder = recorder.lock().await;
        assert_eq!(
            recorder.current_head(),
            expected_head,
            "the recorder's tracked head must match the durable head"
        );
        recorder
            .record_signal_received(
                Utc::now(),
                "after-the-incident".to_owned(),
                Payload::from_json(&serde_json::json!({}))?,
            )
            .await
            .map_err(|error| {
                format!(
                    "the workflow's next append through the same recorder must succeed, got: {error}"
                )
            })?;
        let history = self.inner.read_history(&self.workflow_id).await?;
        let head = history
            .last()
            .ok_or("the history cannot be empty after a successful append")?;
        assert!(
            matches!(head, Event::SignalReceived { .. }),
            "the next append must be the event just recorded: {head:?}"
        );
        assert_eq!(
            head.seq(),
            expected_head + 1,
            "the next append lands at the correct sequence"
        );
        Ok(())
    }
}

/// aion#145, the incident shape end-to-end: an ordinary timer's `TimerFired`
/// append LANDS in the store while the caller's acknowledgement is lost. The
/// ladder retries; the retry finds the fire already recorded, reconciles the
/// recorder's stale sequence forward under the recorder lock
/// (`RecordOutcome::AlreadyRecorded`, nothing appended), and delivers the owed
/// wake through the genuine `wake_workflow` path to the fixture's live
/// process. The closing assertion is the one that pins Part 3: the workflow's
/// NEXT append through the SAME recorder succeeds at the correct sequence.
///
/// This is the unconditional mutant killer for the three parts together:
/// - revert Part 1 (ordinary timers WARN-and-drop): no retry runs, nothing
///   reconciles, the next-append assertion dies on `SequenceConflict`;
/// - revert Part 2 (fired-at-head → silent no-op): the retry never re-enters
///   the recorder seam, nothing reconciles — same red;
/// - revert Part 3 (no already-recorded branch in the bridge): the retry
///   re-appends into a `SequenceConflict` every attempt, the recorder stays
///   stale — same red (and a double append would also redden the
///   exactly-one assertion).
#[tokio::test(flavor = "multi_thread")]
async fn an_ack_lost_ordinary_fire_is_retried_reconciled_and_the_run_keeps_appending() -> TestResult
{
    let fixture = OrdinaryFireFixture::new().await?;
    fixture.faulted.lose_next_append_acks(1);

    fixture.fire_through_ladder().await;

    assert_eq!(
        fixture.fired_count().await?,
        1,
        "exactly one durable TimerFired: the landed-unacknowledged append, never a duplicate"
    );
    let history = fixture.inner.read_history(&fixture.workflow_id).await?;
    assert_eq!(
        history.len(),
        3,
        "WorkflowStarted, TimerStarted, TimerFired and nothing else: {history:#?}"
    );
    fixture.assert_recorder_healthy_at(3).await?;
    fixture.runtime.shutdown()?;
    Ok(())
}

/// aion#145 Part 1 in isolation: a fire attempt that fails OUTRIGHT (the
/// append is refused, nothing lands) must be retried within the same engine
/// epoch, and the retry records exactly one `TimerFired` with the recorder
/// advanced normally. Mutation-sensitive: reverting the ordinary-timer ladder
/// to WARN-and-drop leaves history with no fire at all — the pre-fix wedge.
#[tokio::test(flavor = "multi_thread")]
async fn a_transiently_refused_ordinary_fire_is_retried_to_success() -> TestResult {
    let fixture = OrdinaryFireFixture::new().await?;
    fixture.faulted.refuse_next_appends(1);

    fixture.fire_through_ladder().await;

    assert_eq!(
        fixture.fired_count().await?,
        1,
        "the retry must record the fire exactly once after the refused first attempt"
    );
    fixture.assert_recorder_healthy_at(3).await?;
    fixture.runtime.shutdown()?;
    Ok(())
}

/// aion#145 Part 3's guard rail: reconciliation keys on THIS timer's own last
/// event, and a rogue event at the head — even one of the same KIND, for a
/// different timer id — must NOT trigger it. The genuine double-writer
/// indicator keeps surfacing un-resynced, exactly as the Recorder-level pin
/// `sequence_conflict_surfaces_without_advancing_or_retrying` demands; this is
/// the same law measured at the bridge, where the reconciliation branch lives.
#[tokio::test(flavor = "multi_thread")]
async fn a_rogue_head_event_does_not_reconcile_and_the_conflict_surfaces() -> TestResult {
    use crate::engine_seam::EngineHandle;

    let fixture = OrdinaryFireFixture::new().await?;
    // A rogue writer lands a TimerFired for a DIFFERENT timer behind the
    // registered recorder's back (the recorder still tracks head 2).
    let rogue_id = TimerId::named("rogue-timer")?;
    fixture
        .inner
        .append(
            WriteToken::recorder(),
            &fixture.workflow_id,
            &[Event::TimerFired {
                envelope: aion_core::EventEnvelope {
                    seq: 3,
                    recorded_at: Utc::now(),
                    workflow_id: fixture.workflow_id.clone(),
                },
                timer_id: rogue_id,
            }],
            2,
        )
        .await?;
    let bridge = super::timer_bridge(fixture.runtime.nif_state())
        .map_err(|error| format!("the timer bridge must be installed: {error}"))?;

    let refused = bridge.record_workflow_event(
        &fixture.workflow_id,
        Event::TimerFired {
            envelope: aion_core::EventEnvelope {
                seq: 3,
                recorded_at: Utc::now(),
                workflow_id: fixture.workflow_id.clone(),
            },
            timer_id: fixture.timer_id.clone(),
        },
    );

    let Err(error) = refused else {
        return Err(
            "a rogue head event must NOT be taken for our own landed append: the fire for OUR \
             timer must surface the sequence conflict, not reconcile over a second writer"
                .into(),
        );
    };
    assert!(
        error.to_string().contains("sequence conflict"),
        "the double-writer indicator must surface as itself: {error}"
    );
    let handle = fixture
        .registry
        .get(&fixture.workflow_id, &fixture.run_id)?
        .ok_or("the workflow handle must still be registered")?;
    let recorder = handle.recorder();
    assert_eq!(
        recorder.lock().await.current_head(),
        2,
        "the conflict must leave the recorder's tracked head un-resynced"
    );
    assert_eq!(
        fixture
            .inner
            .read_history(&fixture.workflow_id)
            .await?
            .len(),
        3,
        "the refused append must not have grown the history"
    );
    fixture.runtime.shutdown()?;
    Ok(())
}

/// F1: `arm_timer` must not leave a live durable writer behind when the wheel
/// is torn down *while it is arming*.
///
/// The pre-arm gate was never the hard part — an `arm_timer` that starts after
/// `shutdown_timer_wheel` has returned is refused by the check at the top of
/// the function, and always was. The window is the one BETWEEN that check and
/// the insert: a registry read, a `remove` and a `spawn` all happen in it, and
/// a drain that snapshots `pending_timers` during it takes a snapshot that
/// does not contain this arm. The task then outlives the engine holding an
/// upgradable `Weak<EngineNifState>` (the engine's `Drop` deliberately leaves
/// the seams installed) and records a durable `TimerFired` for a run a
/// successor may already own — the #119 second-writer breach, and a violation
/// of the single-writer invariant.
///
/// The interleave hook fires the teardown from inside that window, which is
/// the only way to put a test on the path of the code being fixed. **A test
/// that merely calls `shutdown_timer_wheel()` and then `arm_timer` stays green
/// with the fix deleted**, because it only ever reaches the pre-arm gate.
///
/// Killing mutation: delete the post-insert re-read block in `arm_timer`. The
/// arm then reports `Ok` and leaves one entry armed, failing both assertions.
#[tokio::test(flavor = "multi_thread")]
async fn a_wheel_teardown_during_arming_leaves_no_armed_timer() -> TestResult {
    use crate::engine_seam::{EngineHandle, TimerWheelEntry};

    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    let store = Arc::new(InMemoryStore::default());
    let registry = Arc::new(Registry::default());
    let pid = 4242;
    let workflow_id = seed_running_workflow(&registry, &store, pid).await?;
    install_timer_nif_bridge(
        runtime.nif_state(),
        Arc::clone(&registry),
        Arc::clone(&store) as Arc<dyn EventStore>,
        tokio::runtime::Handle::current(),
        runtime.signal_delivery(),
    );
    let bridge = super::timer_bridge(runtime.nif_state())
        .map_err(|error| format!("the timer bridge must be installed: {error}"))?;
    let process = crate::engine_seam::WorkflowProcessHandle::new(pid);
    let entry = |name: &str| -> Result<TimerWheelEntry, Box<dyn std::error::Error>> {
        Ok(TimerWheelEntry {
            process,
            timer_id: TimerId::named(name)?,
            // Far enough out that nothing fires while the test runs: this test
            // is about what is ARMED, never about a fire landing.
            fire_at: Utc::now() + chrono::Duration::hours(1),
        })
    };

    // POSITIVE CONTROL. Without the interleave this fixture arms successfully
    // and the count moves. Without this, "no armed timer afterwards" is equally
    // well explained by a fixture that could never arm one — and the refusal
    // below would be measuring nothing.
    bridge
        .arm_timer(entry("control-arm")?)
        .map_err(|error| format!("control: the fixture must be able to arm at all: {error}"))?;
    assert_eq!(
        bridge.armed_wheel_timers(),
        1,
        "control: the fixture must reach the real arming path"
    );
    bridge.disarm_timer(process, &TimerId::named("control-arm")?)?;
    assert_eq!(bridge.armed_wheel_timers(), 0, "control: disarm cleaned up");

    // THE INTERLEAVING. Tear the wheel down from inside the window — after the
    // pre-arm gate has been passed, before the entry exists.
    let torn = Arc::clone(&bridge);
    bridge.set_arm_interleave(Arc::new(move || torn.shutdown_timer_wheel()));

    let refused = bridge.arm_timer(entry("armed-into-a-teardown")?);

    let Err(error) = refused else {
        return Err(
            "a wheel teardown that lands while `arm_timer` is between its gate and its \
                    insert must make the arm REFUSE: the task it spawned appends a durable \
                    `TimerFired` for a run this process may no longer own, which is a second \
                    writer for one workflow"
                .into(),
        );
    };
    assert!(
        error.to_string().contains("torn down"),
        "the refusal must name the cause, or an operator is sent looking for a missing workflow \
         or a bad timer id: {error}"
    );
    assert_eq!(
        bridge.armed_wheel_timers(),
        0,
        "the retracted arm must leave NOTHING in the wheel: an entry inserted into an \
         already-drained map is a live durable writer with no owner left to abort it"
    );
    // The workflow is untouched by a refused arm — no history was written.
    assert_eq!(
        store.read_history(&workflow_id).await?.len(),
        1,
        "a refused arm must not have recorded anything"
    );
    runtime.shutdown()?;
    Ok(())
}

/// F1, second half: once the wheel is torn down, the append boundary itself
/// refuses.
///
/// `shutdown_timer_wheel` aborts the armed tasks, but `JoinHandle::abort` does
/// not stop a task that has already entered a poll — so a fire which reached
/// the recorder before the abort landed would complete its append regardless
/// of any arming-side gate. Without the check inside the recorder lock the
/// wheel is the only durable writer in the crate with no boundary refusal at
/// all.
///
/// Killing mutation: delete the `shut_down` check inside
/// `record_workflow_event`'s blocking future. The append then succeeds, so the
/// call returns `Ok` and the test fails at the `let Err(error) = refused else`
/// guard — before any assertion runs. The assertions are what pin the WORDING;
/// the guard is what pins the refusal existing at all.
#[tokio::test(flavor = "multi_thread")]
async fn a_torn_down_wheel_refuses_to_append() -> TestResult {
    use crate::engine_seam::EngineHandle;

    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    let store = Arc::new(InMemoryStore::default());
    let registry = Arc::new(Registry::default());
    let workflow_id = seed_running_workflow(&registry, &store, 7373).await?;
    install_timer_nif_bridge(
        runtime.nif_state(),
        Arc::clone(&registry),
        Arc::clone(&store) as Arc<dyn EventStore>,
        tokio::runtime::Handle::current(),
        runtime.signal_delivery(),
    );
    let bridge = super::timer_bridge(runtime.nif_state())
        .map_err(|error| format!("the timer bridge must be installed: {error}"))?;
    let fired = |seq: u64| -> Result<Event, Box<dyn std::error::Error>> {
        Ok(Event::TimerFired {
            envelope: aion_core::EventEnvelope {
                seq,
                recorded_at: Utc::now(),
                workflow_id: workflow_id.clone(),
            },
            timer_id: TimerId::named("late-fire")?,
        })
    };

    // POSITIVE CONTROL: before teardown this exact call records.
    let outcome = bridge.record_workflow_event(&workflow_id, fired(2)?)?;
    assert!(
        matches!(outcome, crate::engine_seam::RecordOutcome::Recorded),
        "control: the bridge must be able to record at all, or the refusal below measures nothing"
    );
    let after_control = store.read_history(&workflow_id).await?.len();
    assert_eq!(after_control, 2, "control: the fire landed in history");

    bridge.shutdown_timer_wheel();

    let refused = bridge.record_workflow_event(&workflow_id, fired(3)?);
    let Err(error) = refused else {
        return Err(
            "a torn-down wheel must REFUSE its append: `abort` cannot stop a task already \
                    inside a poll, so without a boundary check a released engine still writes"
                .into(),
        );
    };
    assert!(
        error.to_string().contains("torn down"),
        "the refusal must name the cause: {error}"
    );
    // 🔴 AND IT MUST NOT CLAIM THE TIMER WAS NEVER ARMED. It was armed, it
    // elapsed, and it fired — the append is what was refused. One constructor
    // used to serve this site and the two arming sites, so the wording here
    // said `was not armed` about a timer the operator had watched fire.
    assert!(
        !error.to_string().contains("was not armed"),
        "the append refusal must not report an arming failure for a timer that fired: {error}"
    );
    assert!(
        error
            .to_string()
            .contains("fired, but its append was refused"),
        "the append refusal must say what actually happened: {error}"
    );
    assert_eq!(
        store.read_history(&workflow_id).await?.len(),
        after_control,
        "the refused append must not have grown the history"
    );
    runtime.shutdown()?;
    Ok(())
}

/// 🔴 A REFUSED CANCEL MUST NOT BE REPORTED AS A REFUSED FIRE.
///
/// The append boundary refuses `TimerCancelled` by exactly the same gate that
/// refuses `TimerFired`, and for a while it said exactly the same sentence at
/// both. For a cancel every clause of the fire wording is wrong: nothing fired,
/// and "the timer is still live and the owning engine re-arms it" — the very
/// fact that makes a refused fire cost the run nothing — is the HARM, because
/// the run asked for that timer to stop. An operator reading the fire sentence
/// after a refused cancel is told the outcome they feared is the reassurance.
///
/// This is the identical defect class that `a_torn_down_wheel_refuses_to_append`
/// above pins for the arming sites, reintroduced one boundary along by the fix
/// for it, which is why it is measured rather than reasoned about.
///
/// Killing mutation: collapse `TimerAppendError::into_seam_error`'s two
/// `WheelTornDown` arms back onto `wheel_torn_down_after_firing`. The refusal
/// then says "fired, but its append was refused" about a cancellation and the
/// three wording assertions fail — while
/// `a_torn_down_wheel_refuses_to_append` stays green, because the fire arm is
/// unchanged. The two tests are orthogonal by construction.
#[tokio::test(flavor = "multi_thread")]
async fn a_refused_cancel_is_not_reported_as_a_refused_fire() -> TestResult {
    use aion_core::TimerCancelCause;

    use crate::engine_seam::EngineHandle;

    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    let store = Arc::new(InMemoryStore::default());
    let registry = Arc::new(Registry::default());
    let workflow_id = seed_running_workflow(&registry, &store, 7575).await?;
    install_timer_nif_bridge(
        runtime.nif_state(),
        Arc::clone(&registry),
        Arc::clone(&store) as Arc<dyn EventStore>,
        tokio::runtime::Handle::current(),
        runtime.signal_delivery(),
    );
    let bridge = super::timer_bridge(runtime.nif_state())
        .map_err(|error| format!("the timer bridge must be installed: {error}"))?;
    let cancelled = |seq: u64| -> Result<Event, Box<dyn std::error::Error>> {
        Ok(Event::TimerCancelled {
            envelope: aion_core::EventEnvelope {
                seq,
                recorded_at: Utc::now(),
                workflow_id: workflow_id.clone(),
            },
            timer_id: TimerId::named("late-cancel")?,
            cause: TimerCancelCause::WorkflowIntent,
        })
    };

    // POSITIVE CONTROL: before teardown this exact call records, so the refusal
    // below is the teardown and not a cancel the bridge could never record.
    let outcome = bridge.record_workflow_event(&workflow_id, cancelled(2)?)?;
    assert!(
        matches!(outcome, crate::engine_seam::RecordOutcome::Recorded),
        "control: the bridge must be able to record a cancellation at all, or the refusal \
         below measures nothing"
    );

    bridge.shutdown_timer_wheel();

    let refused = bridge.record_workflow_event(&workflow_id, cancelled(3)?);
    let Err(error) = refused else {
        return Err("a torn-down wheel must REFUSE a cancellation append too".into());
    };
    let message = error.to_string();
    assert!(
        message.contains("torn down"),
        "the refusal must name the cause: {error}"
    );
    // 🔴 THE FIRE WORDING MUST NOT APPEAR HERE. Nothing fired.
    assert!(
        !message.contains("fired"),
        "a refused cancellation must not be reported as a refused fire: {error}"
    );
    assert!(
        message.contains("cancellation of timer"),
        "the refusal must say that a CANCELLATION was what went unrecorded: {error}"
    );
    // The re-arm is stated as the consequence it is for a cancel, conditioned on
    // the run reissuing the cancellation against the owning engine — not left as
    // the closing reassurance it legitimately is for a fire.
    assert!(
        message.contains("reissues it there"),
        "the refusal must say how the run's intent is actually restored, rather than \
         ending on the re-arm as though it were good news: {error}"
    );
    runtime.shutdown()?;
    Ok(())
}

/// 🔴 A REFUSED TEARDOWN CANCEL MUST NOT PROMISE A REISSUE THAT CANNOT HAPPEN.
///
/// The sibling test above pins the wording for a [`TimerCancelCause::WorkflowIntent`]
/// cancel, whose remedy really is "the run reissues it there" — workflow code
/// re-executes on the owning engine and asks again. `Engine::cancel` also
/// retires a run's in-flight timers, with [`TimerCancelCause::CancelTeardown`],
/// and those reach this same append boundary (see the cancel loop in
/// `engine/api_workflow_ops.rs`). That run is being CANCELLED — the terminal is
/// recorded by the very next statement — so nothing re-executes it and nothing
/// reissues anything. Raising the workflow-intent sentence there sends an
/// operator to wait on a reissue that is not coming — which is the F-A defect
/// exactly, one level further down, and it survived F-A because the cause was
/// dropped with a `_`.
///
/// "Being cancelled" is as far as this goes, and the distinction is load-bearing
/// rather than pedantic: the terminal has NOT landed when the refusal is raised,
/// so anything the message says about the run being terminal is a prediction.
/// The assertions below pin it as one.
///
/// The control is the [`aion_core::TimerCancelCause::WorkflowIntent`] path in
/// the test above: same bridge, same
/// boundary, same teardown — only `cause` differs, so a divergence here is
/// attributable to the cause and nothing else.
#[tokio::test]
async fn a_refused_teardown_cancel_does_not_promise_a_reissue() -> TestResult {
    use aion_core::TimerCancelCause;

    use crate::engine_seam::EngineHandle;

    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    let store = Arc::new(InMemoryStore::default());
    let registry = Arc::new(Registry::default());
    let workflow_id = seed_running_workflow(&registry, &store, 7576).await?;
    install_timer_nif_bridge(
        runtime.nif_state(),
        Arc::clone(&registry),
        Arc::clone(&store) as Arc<dyn EventStore>,
        tokio::runtime::Handle::current(),
        runtime.signal_delivery(),
    );
    let bridge = super::timer_bridge(runtime.nif_state())
        .map_err(|error| format!("the timer bridge must be installed: {error}"))?;
    let teardown_cancelled = |seq: u64| -> Result<Event, Box<dyn std::error::Error>> {
        Ok(Event::TimerCancelled {
            envelope: aion_core::EventEnvelope {
                seq,
                recorded_at: Utc::now(),
                workflow_id: workflow_id.clone(),
            },
            timer_id: TimerId::named("teardown-cancel")?,
            cause: TimerCancelCause::CancelTeardown,
        })
    };

    // POSITIVE CONTROL: this exact call records before teardown, so the refusal
    // below is the stand-down and not a cancel the bridge could never take.
    let outcome = bridge.record_workflow_event(&workflow_id, teardown_cancelled(2)?)?;
    assert!(
        matches!(outcome, crate::engine_seam::RecordOutcome::Recorded),
        "control: the bridge must be able to record a teardown cancellation at all, or the \
         refusal below measures nothing"
    );

    bridge.shutdown_timer_wheel();

    let refused = bridge.record_workflow_event(&workflow_id, teardown_cancelled(3)?);
    let Err(error) = refused else {
        return Err("a torn-down wheel must REFUSE a teardown cancellation append".into());
    };
    let message = error.to_string();
    assert!(
        message.contains("torn down"),
        "the refusal must name the cause: {error}"
    );
    assert!(
        !message.contains("fired"),
        "a refused cancellation must not be reported as a refused fire: {error}"
    );
    // 🔴 THE LOAD-BEARING ABSENCE. This is the sentence that is true of the
    // sibling case and false of this one.
    assert!(
        !message.contains("reissues it there"),
        "a teardown cancel's run is terminal and never re-executes, so the refusal must not \
         tell the operator to wait for the run to reissue it: {error}"
    );
    // Absence alone would pass on an empty string, so pair it with what must be
    // SAID: the reason no reissue is coming, and that the fire is inert.
    //
    // 🔴 STATED AS A CONDITION, NOT AS A FACT. An earlier version of this
    // message — and of this assertion — said the run "is terminal and will not
    // re-execute". That is false at the moment the refusal is raised:
    // `Engine::cancel` calls `cancel_inflight_timers` BEFORE `terminate::cancel`
    // (mandatory ordering, stated in its own doc), so `WorkflowCancelled` has
    // not been recorded yet and the run is still `Running`. The assertion
    // passed anyway, because it measured the TEXT and the text was confident.
    assert!(
        message.contains("once that run's terminal lands"),
        "the refusal must state the terminal as the condition it depends on rather than as a \
         fact already in hand: {error}"
    );
    assert!(
        message.contains("post-terminal"),
        "the refusal must say what actually becomes of the uncancelled timer — the owning \
         engine's fire is refused as post-terminal, recording nothing: {error}"
    );
    // 🔴 THE SECOND LOAD-BEARING ABSENCE. A message that names only the branch
    // where everything works reads as unconditional even when it is hedged.
    // `cancel_inflight_timers` swallows every failure into a `warn!`, and
    // `Engine::cancel` can still fail at `terminate::cancel` — leaving the run
    // live with this timer armed, where the fire is NOT post-terminal and DOES
    // wake it. The operator has to be told that branch exists.
    assert!(
        message.contains("check the run's status"),
        "the refusal must tell the operator what to do when the cancel that issued it also \
         failed, because then the run is still live and this timer will fire: {error}"
    );
    runtime.shutdown()?;
    Ok(())
}

/// 🔴 AN ORDERLY STAND-DOWN IS NOT A FAULT, AND MUST BE READABLE AS ONE.
///
/// `fire_wheel_timer` reports a failed fire, and until the refusal carried its
/// own type it could not tell an engine standing down from a store that broke.
/// Both arrived as `EngineSeamError::Recorder` — the boxed error in
/// `record_workflow_event` flattened them — so a teardown was logged as a
/// callback failure to investigate.
///
/// It pins the classification on the REAL error: the refusal is obtained by
/// driving `TimerService::fire_timer` against a live timer on a torn-down
/// wheel, not by constructing one here. A hand-built error would make the
/// treatment its own control.
///
/// 🔴 THE TIMER IS DELIBERATELY NOT A DEADLINE, and the reason is a finding in
/// its own right. Review 15 suspected the bounded deadline ladder would grind
/// six attempts against a torn-down wheel and then emit `tracing::error!` for
/// an orderly shutdown, and marked it PLAUSIBLE. Written as a deadline test
/// first, this REFUTED it: `fire_timer_guarded` demuxes a reserved
/// `deadline:{run}` timer to `fire_deadline` BEFORE the generic
/// record-then-deliver path, so a deadline never reaches the append boundary
/// that raises the teardown refusal and the ladder was never exposed to it. An
/// ordinary timer is the path that does reach it, so that is the path measured.
///
/// Killing mutation: restore the boxed error in `record_workflow_event`, so the
/// teardown arrives as `EngineSeamError::Recorder`. `is_wheel_teardown` then
/// answers `false` and the assertion below fails.
#[tokio::test(flavor = "multi_thread")]
async fn stand_down_is_not_a_fault() -> TestResult {
    use crate::runtime::nif_timer_fire::is_wheel_teardown;
    use crate::time::TimerServiceError;

    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    let store = Arc::new(InMemoryStore::default());
    let registry = Arc::new(Registry::default());
    let workflow_id = seed_running_workflow(&registry, &store, 7474).await?;
    install_timer_nif_bridge(
        runtime.nif_state(),
        Arc::clone(&registry),
        Arc::clone(&store) as Arc<dyn EventStore>,
        tokio::runtime::Handle::current(),
        runtime.signal_delivery(),
    );
    let bridge = super::timer_bridge(runtime.nif_state())
        .map_err(|error| format!("the timer bridge must be installed: {error}"))?;

    // A live ORDINARY timer, so `fire_timer` passes its liveness guard and
    // actually reaches the append boundary. Without the liveness event it
    // returns `Ok(())` early and every assertion below would measure an absent
    // code path; with a deadline id it would demux away before ever arriving.
    let timer_id = TimerId::named("stand-down")?;
    let fire_at = Utc::now();
    store
        .append(
            WriteToken::recorder(),
            &workflow_id,
            &[Event::TimerStarted {
                envelope: aion_core::EventEnvelope {
                    seq: 2,
                    recorded_at: fire_at,
                    workflow_id: workflow_id.clone(),
                },
                timer_id: timer_id.clone(),
                fire_at,
            }],
            1,
        )
        .await?;

    bridge.shutdown_timer_wheel();

    let refused = bridge
        .service()
        .fire_timer(workflow_id.clone(), timer_id.clone(), fire_at)
        .await;
    let Err(error) = refused else {
        return Err("a torn-down wheel must refuse the fire, or this measures nothing".into());
    };
    assert!(
        is_wheel_teardown(&error),
        "a wheel teardown must be recognised as a stand-down, or the deadline ladder \
         retries it six times and then logs an error for an orderly shutdown: {error}"
    );

    // NEGATIVE CONTROL: the predicate must not answer `true` for everything.
    // A recorder failure is a genuine fault and MUST stay in the retry ladder,
    // which is exactly the case a too-wide predicate would silently abandon.
    let recorder_failure =
        TimerServiceError::Engine(crate::engine_seam::EngineSeamError::Recorder {
            reason: String::from("the store refused the append"),
        });
    assert!(
        !is_wheel_teardown(&recorder_failure),
        "a recorder failure is a fault worth retrying and must not be read as a stand-down"
    );

    runtime.shutdown()?;
    Ok(())
}

/// 🔴 A TORN-DOWN WHEEL MUST NOT LET A DEADLINE TIME A RUN OUT.
///
/// The deadline path is the ONE durable writer this engine's append boundary
/// cannot reach. `TimerService::fire_timer_guarded` demuxes a reserved
/// `deadline:{run}` fire to the registered [`DeadlineHandler`] BEFORE the
/// generic record-then-deliver path, so the refusal in
/// `TimerNifBridge::record_workflow_event` — the one thing that stops a task
/// already past its `abort` point from appending — is never on a deadline's
/// route. Without a gate in the handler, a deadline task already inside its poll
/// when `shutdown_timer_wheel` ran goes on to append a durable
/// `WorkflowTimedOut` and tear the run down, for a run a successor engine may
/// already own: a second writer for one workflow, which is invariant 3 and the
/// #119 breach — through the very wheel this lane was hardening.
///
/// 🔴 THE LATCH IS THE REAL ONE. The handler is registered through
/// `register_deadline_handler`, which hands it the installed bridge's own
/// `shut_down` flag; the test never constructs a flag and never sets one.
/// `shutdown_timer_wheel()` is what changes between the control and the
/// treatment, exactly as in production. A test that made its own flag would be
/// its own control: it would pass identically against a handler wired to
/// nothing.
///
/// The control runs FIRST, on the same bridge and the same handler, so the two
/// halves differ in one thing only.
///
/// Killing mutation: delete the `stand_down` check in
/// `WorkflowDeadlineHandler::decide_disposition`. The control still passes; the
/// treatment records `WorkflowTimedOut` and fails.
#[tokio::test(flavor = "multi_thread")]
async fn a_torn_down_wheel_does_not_time_a_run_out() -> TestResult {
    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    let backing = Arc::new(InMemoryStore::default());
    let store: Arc<dyn EventStore> = Arc::clone(&backing) as Arc<dyn EventStore>;
    let registry = Arc::new(Registry::default());
    install_timer_nif_bridge(
        runtime.nif_state(),
        Arc::clone(&registry),
        Arc::clone(&store),
        tokio::runtime::Handle::current(),
        runtime.signal_delivery(),
    );
    register_deadline_handler(runtime.nif_state(), |stand_down| {
        Arc::new(crate::lifecycle::deadline::WorkflowDeadlineHandler::new(
            Arc::downgrade(&runtime),
            Arc::clone(&store),
            Arc::clone(&backing) as Arc<dyn aion_store::visibility::VisibilityStore>,
            Arc::clone(&registry),
            stand_down,
        )) as Arc<dyn DeadlineHandler>
    })
    .map_err(|error| format!("failed to register the deadline handler: {error}"))?;
    let bridge = super::timer_bridge(runtime.nif_state())
        .map_err(|error| format!("the timer bridge must be installed: {error}"))?;

    // POSITIVE CONTROL: wheel intact, an armed deadline elapses, the run times
    // out. Without this the refusal below could not be told from a deadline
    // that never reached the handler at all.
    let (control_id, control_deadline) =
        seed_armed_deadline(&registry, &backing, 8181, Utc::now()).await?;
    bridge
        .service()
        .fire_timer(control_id.clone(), control_deadline, Utc::now())
        .await?;
    assert!(
        timed_out(&store.read_history(&control_id).await?),
        "control: an armed deadline on a live wheel must record WorkflowTimedOut, or the \
         treatment below proves nothing"
    );

    // TREATMENT: same bridge, same handler, wheel torn down.
    let (timed_id, timed_deadline) =
        seed_armed_deadline(&registry, &backing, 8282, Utc::now()).await?;
    let before = store.read_history(&timed_id).await?.len();
    bridge.shutdown_timer_wheel();

    bridge
        .service()
        .fire_timer(timed_id.clone(), timed_deadline, Utc::now())
        .await?;

    let history = store.read_history(&timed_id).await?;
    assert!(
        !timed_out(&history),
        "a torn-down wheel must NOT record WorkflowTimedOut: the run belongs to whichever \
         engine owns it now, and a second writer for one workflow is the #119 breach: \
         {history:#?}"
    );
    assert_eq!(
        history.len(),
        before,
        "the stood-down deadline must append nothing at all — not the terminal, not the \
         ordinary-timer retirements, not the deadline's own cancellation"
    );
    runtime.shutdown()?;
    Ok(())
}

/// 🔴 THE REGISTRY-FREE FINALIZER IS A DURABLE WRITER TOO, AND IT IS GATED.
///
/// `WorkflowDeadlineHandler::finalize_timed_out_without_handle` is the path a
/// deadline takes when no handle is registered — a cold engine or a shard
/// adopter never registers a terminal run, so a recovered deadline row whose
/// history already shows `WorkflowTimedOut` with teardown unfinished lands
/// there. It appends: ordinary-timer retirements, a visibility upsert, and the
/// deadline's own retirement. None of that goes near the recorder lock (there is
/// no handle to take one from), so none of it is covered by the gate in
/// `decide_disposition` — a stand-down gate that stopped only the registered
/// path would leave this one writing.
///
/// Its gate is honestly weaker: check-then-act, stated as such at the site. What
/// this measures is that it exists and bites, which is the difference between a
/// short window and no gate at all.
///
/// Killing mutation: delete the `stand_down` check in
/// `finalize_timed_out_without_handle`. The control still passes; the treatment
/// retires the deadline and fails.
#[tokio::test(flavor = "multi_thread")]
async fn a_torn_down_wheel_does_not_finalize_an_unregistered_timeout() -> TestResult {
    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    let backing = Arc::new(InMemoryStore::default());
    let store: Arc<dyn EventStore> = Arc::clone(&backing) as Arc<dyn EventStore>;
    let registry = Arc::new(Registry::default());
    install_timer_nif_bridge(
        runtime.nif_state(),
        Arc::clone(&registry),
        Arc::clone(&store),
        tokio::runtime::Handle::current(),
        runtime.signal_delivery(),
    );
    register_deadline_handler(runtime.nif_state(), |stand_down| {
        Arc::new(crate::lifecycle::deadline::WorkflowDeadlineHandler::new(
            Arc::downgrade(&runtime),
            Arc::clone(&store),
            Arc::clone(&backing) as Arc<dyn aion_store::visibility::VisibilityStore>,
            Arc::clone(&registry),
            stand_down,
        )) as Arc<dyn DeadlineHandler>
    })
    .map_err(|error| format!("failed to register the deadline handler: {error}"))?;
    let bridge = super::timer_bridge(runtime.nif_state())
        .map_err(|error| format!("the timer bridge must be installed: {error}"))?;

    // POSITIVE CONTROL: an unregistered, already-timed-out run with its deadline
    // still outstanding — the exact shape the finalizer exists for. On a live
    // wheel it retires that deadline.
    let (control_id, control_deadline) = seed_unfinished_timeout(&backing).await?;
    bridge
        .service()
        .fire_timer(control_id.clone(), control_deadline.clone(), Utc::now())
        .await?;
    assert!(
        cancelled(&store.read_history(&control_id).await?, &control_deadline),
        "control: the registry-free finalizer must retire the outstanding deadline, or the \
         treatment below measures a path that never ran"
    );

    // TREATMENT: same bridge, same handler, wheel torn down.
    let (stale_id, stale_deadline) = seed_unfinished_timeout(&backing).await?;
    let before = store.read_history(&stale_id).await?.len();
    bridge.shutdown_timer_wheel();

    bridge
        .service()
        .fire_timer(stale_id.clone(), stale_deadline.clone(), Utc::now())
        .await?;

    let history = store.read_history(&stale_id).await?;
    assert!(
        !cancelled(&history, &stale_deadline),
        "a torn-down wheel must not finalize an unregistered timeout: every append here is \
         durable and the run may already be owned elsewhere: {history:#?}"
    );
    assert_eq!(
        history.len(),
        before,
        "the stood-down finalizer must append nothing at all"
    );
    runtime.shutdown()?;
    Ok(())
}

/// Seed an UNREGISTERED run whose history shows `WorkflowTimedOut` with its
/// deadline still outstanding — an interrupted timeout teardown, which is the
/// only shape `finalize_timed_out_without_handle` acts on.
///
/// Nothing is inserted into the registry: that absence is what routes the fire
/// to the registry-free finalizer rather than to `decide_disposition`.
async fn seed_unfinished_timeout(
    store: &Arc<InMemoryStore>,
) -> Result<(WorkflowId, TimerId), Box<dyn std::error::Error>> {
    let workflow_id = WorkflowId::new_v4();
    let run_id = RunId::new_v4();
    let deadline_id = crate::time::deadline_timer_id(&run_id)?;
    let mut recorder = Recorder::new(workflow_id.clone(), Arc::clone(store) as _);
    recorder
        .record_workflow_started(
            Utc::now(),
            WorkflowStartRecord {
                workflow_type: "sleeper".to_owned(),
                input: Payload::from_json(&serde_json::json!({}))?,
                run_id: run_id.clone(),
                parent_run_id: None,
                parent_workflow_id: None,
                package_version: PackageVersion::new("a".repeat(64)),
            },
        )
        .await?;
    recorder
        .record_timer_started(Utc::now(), deadline_id.clone(), Utc::now())
        .await?;
    recorder
        .record_workflow_timed_out(Utc::now(), String::from("workflow"))
        .await?;
    Ok((workflow_id, deadline_id))
}

/// Whether history retires `timer_id`.
fn cancelled(history: &[Event], timer_id: &TimerId) -> bool {
    history
        .iter()
        .any(|event| matches!(event, Event::TimerCancelled { timer_id: id, .. } if id == timer_id))
}

/// Whether history records a `WorkflowTimedOut`.
fn timed_out(history: &[Event]) -> bool {
    history
        .iter()
        .any(|event| matches!(event, Event::WorkflowTimedOut { .. }))
}

/// Seed a running workflow whose declared-timeout deadline is ARMED in durable
/// history, and return it with its reserved deadline timer id.
///
/// The `TimerStarted` matters: the handler re-checks that this deadline is still
/// outstanding before it records anything, so an unarmed run would lose cleanly
/// and every assertion built on it would measure an absent path.
///
/// It is armed THROUGH THE HANDLE'S OWN RECORDER, not by appending to the store
/// behind it — the single-writer discipline this whole lane is about. Writing
/// straight to the store leaves the handle's recorder at a stale head and the
/// handler's own first append fails with a `SequenceConflict`, which is exactly
/// the double-writer indicator it is supposed to be.
async fn seed_armed_deadline(
    registry: &Registry,
    store: &Arc<InMemoryStore>,
    pid: u64,
    fire_at: DateTime<Utc>,
) -> Result<(WorkflowId, TimerId), Box<dyn std::error::Error>> {
    let workflow_id = seed_running_workflow(registry, store, pid).await?;
    let history = store.read_history(&workflow_id).await?;
    let run_id = history
        .iter()
        .find_map(|event| match event {
            Event::WorkflowStarted { run_id, .. } => Some(run_id.clone()),
            _ => None,
        })
        .ok_or("the seeded workflow must have a WorkflowStarted")?;
    let deadline_id = crate::time::deadline_timer_id(&run_id)?;
    let handle = registry
        .get(&workflow_id, &run_id)?
        .ok_or("the seeded workflow must be registered")?;
    let recorder = handle.recorder();
    let mut recorder = recorder.lock().await;
    recorder
        .record_timer_started(fire_at, deadline_id.clone(), fire_at)
        .await?;
    Ok((workflow_id, deadline_id))
}

/// Seed one running workflow and register a resident handle for `pid`, so the
/// bridge's `workflow_id_for_process` resolves it.
async fn seed_running_workflow(
    registry: &Registry,
    store: &Arc<InMemoryStore>,
    pid: u64,
) -> Result<WorkflowId, Box<dyn std::error::Error>> {
    let store: Arc<dyn EventStore> = Arc::clone(store) as Arc<dyn EventStore>;
    seed_resident_workflow(registry, &store, pid)
        .await
        .map(|(workflow_id, _)| workflow_id)
}

/// [`seed_running_workflow`] over any [`EventStore`] — the fault-injection
/// tests need the registered handle's Recorder writing THROUGH the faultable
/// wrapper, not around it — returning the run id as well so tests can retrieve
/// the handle (and its recorder) back from the registry.
async fn seed_resident_workflow(
    registry: &Registry,
    store: &Arc<dyn EventStore>,
    pid: u64,
) -> Result<(WorkflowId, RunId), Box<dyn std::error::Error>> {
    use crate::registry::{
        CompletionNotifier, HandleResidency, WorkflowHandle, WorkflowHandleParts,
    };
    use aion_core::WorkflowStatus;

    let workflow_id = WorkflowId::new_v4();
    let run_id = RunId::new_v4();
    let started = Event::WorkflowStarted {
        envelope: aion_core::EventEnvelope {
            seq: 1,
            recorded_at: Utc::now(),
            workflow_id: workflow_id.clone(),
        },
        workflow_type: "sleeper".to_owned(),
        input: Payload::from_json(&serde_json::json!({}))?,
        run_id: run_id.clone(),
        parent_run_id: None,
        parent_workflow_id: None,
        package_version: PackageVersion::new("a".repeat(64)),
    };
    store
        .append(WriteToken::recorder(), &workflow_id, &[started], 0)
        .await?;
    let recorder = Recorder::resume_at(workflow_id.clone(), Arc::clone(store), 1);
    let handle = WorkflowHandle::new(WorkflowHandleParts {
        workflow_id: workflow_id.clone(),
        run_id: run_id.clone(),
        pid,
        workflow_type: "sleeper".to_owned(),
        namespace: String::from("default"),
        loaded_version: aion_package::ContentHash::from_bytes([9; 32]),
        cached_status: WorkflowStatus::Running,
        residency: HandleResidency::Resident,
        recorder,
        completion: CompletionNotifier::new(),
    });
    registry.insert((workflow_id.clone(), run_id.clone()), handle)?;
    Ok((workflow_id, run_id))
}
