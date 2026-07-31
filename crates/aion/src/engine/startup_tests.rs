//! Startup recovery sweep tests (split from `startup.rs`).

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use aion_core::{
    Event, EventEnvelope, Payload, RunId, SearchAttributeSchema, WorkflowId, WorkflowStatus,
};
use aion_store::{EventStore, StoreError};
use chrono::Utc;
use serde_json::json;

use super::{
    RecoveredResident, StartupRecoveryContext, SweepScope, register_recovered_resident,
    register_recovered_resident_with_reconcile, sweep_continued_as_new_replacements,
    sweep_uncancelled_terminal_deadlines,
};
use crate::EngineError;
use crate::loader::WorkflowCatalog;
use crate::registry::{
    CompletionNotifier, HandleResidency, Registry, WorkflowHandle, WorkflowHandleParts,
};
use crate::runtime::{RuntimeConfig, RuntimeHandle, RuntimeInput};
use crate::supervision::SupervisionTree;
use aion_store::InMemoryStore;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Canned store modelling the N-5 race: the first history read shows a
/// stranded continue-as-new run (no successor `WorkflowStarted`); once
/// `successor_appears_after_reads` reads have happened, the racing exit
/// monitor's successor start is visible. Appends are never expected —
/// the sweep's own start fails earlier (no loadable type), standing in
/// for the race loser's `SequenceConflict`.
struct RacingSuccessorStore {
    workflow_id: WorkflowId,
    base_history: Vec<Event>,
    full_history: Vec<Event>,
    successor_appears_after_reads: u32,
    reads: AtomicU32,
    appears: bool,
}

#[async_trait::async_trait]
impl aion_store::ReadableEventStore for RacingSuccessorStore {
    async fn read_history(&self, workflow_id: &WorkflowId) -> Result<Vec<Event>, StoreError> {
        if workflow_id != &self.workflow_id {
            return Ok(Vec::new());
        }
        let read = self.reads.fetch_add(1, Ordering::AcqRel) + 1;
        if self.appears && read > self.successor_appears_after_reads {
            Ok(self.full_history.clone())
        } else {
            Ok(self.base_history.clone())
        }
    }

    async fn read_history_from(
        &self,
        workflow_id: &WorkflowId,
        from_seq: u64,
    ) -> Result<Vec<Event>, StoreError> {
        let _ = (workflow_id, from_seq);
        Err(StoreError::Backend(
            "unexpected read_history_from in the sweep test".to_owned(),
        ))
    }

    async fn read_run_chain(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<aion_store::RunSummary>, StoreError> {
        let _ = workflow_id;
        Err(StoreError::Backend(
            "unexpected read_run_chain in the sweep test".to_owned(),
        ))
    }

    async fn list_workflow_ids(&self) -> Result<Vec<WorkflowId>, StoreError> {
        Ok(vec![self.workflow_id.clone()])
    }

    async fn list_active(&self) -> Result<Vec<WorkflowId>, StoreError> {
        Ok(Vec::new())
    }

    async fn list_paused(&self) -> Result<Vec<WorkflowId>, StoreError> {
        Ok(Vec::new())
    }

    async fn query(
        &self,
        filter: &aion_core::WorkflowFilter,
    ) -> Result<Vec<aion_core::WorkflowSummary>, StoreError> {
        if filter.status != Some(WorkflowStatus::ContinuedAsNew) {
            return Ok(Vec::new());
        }
        Ok(vec![aion_core::WorkflowSummary {
            workflow_id: self.workflow_id.clone(),
            workflow_type: "checkout".to_owned(),
            status: WorkflowStatus::ContinuedAsNew,
            started_at: Utc::now(),
            ended_at: None,
            parent: None,
            failed_step: None,
            failure_reason: None,
        }])
    }

    async fn schedule_timer(
        &self,
        workflow_id: &WorkflowId,
        timer_id: &aion_core::TimerId,
        fire_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), StoreError> {
        let _ = (workflow_id, timer_id, fire_at);
        Err(StoreError::Backend(
            "unexpected schedule_timer in the sweep test".to_owned(),
        ))
    }

    async fn expired_timers(
        &self,
        as_of: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<aion_store::TimerEntry>, StoreError> {
        let _ = as_of;
        Ok(Vec::new())
    }
}

#[async_trait::async_trait]
impl aion_store::WritableEventStore for RacingSuccessorStore {
    async fn append(
        &self,
        token: aion_store::WriteToken,
        workflow_id: &WorkflowId,
        events: &[Event],
        expected_seq: u64,
    ) -> Result<(), StoreError> {
        let _ = (token, workflow_id, events, expected_seq);
        Err(StoreError::SequenceConflict {
            expected: expected_seq,
            found: expected_seq + 1,
        })
    }
}

/// The sweep never touches deployed packages: reads are legitimately empty,
/// mutations are unexpected.
#[async_trait::async_trait]
impl aion_store::PackageStore for RacingSuccessorStore {
    async fn put_package(&self, record: aion_store::PackageRecord) -> Result<(), StoreError> {
        let _ = record;
        Err(StoreError::Backend(
            "unexpected put_package in the sweep test".to_owned(),
        ))
    }

    async fn put_package_with_routes(
        &self,
        record: aion_store::PackageRecord,
        route_workflow_types: &[String],
    ) -> Result<(), StoreError> {
        let _ = (record, route_workflow_types);
        Err(StoreError::Backend(
            "unexpected put_package_with_routes in the sweep test".to_owned(),
        ))
    }

    async fn list_packages(&self) -> Result<Vec<aion_store::PackageRecord>, StoreError> {
        Ok(Vec::new())
    }

    async fn delete_package(
        &self,
        workflow_type: &str,
        content_hash: &str,
    ) -> Result<(), StoreError> {
        let _ = (workflow_type, content_hash);
        Err(StoreError::Backend(
            "unexpected delete_package in the sweep test".to_owned(),
        ))
    }

    async fn put_package_route(
        &self,
        workflow_type: &str,
        content_hash: &str,
    ) -> Result<(), StoreError> {
        let _ = (workflow_type, content_hash);
        Err(StoreError::Backend(
            "unexpected put_package_route in the sweep test".to_owned(),
        ))
    }

    async fn list_package_routes(&self) -> Result<Vec<aion_store::PackageRouteRecord>, StoreError> {
        Ok(Vec::new())
    }
}

/// `(base history, history with the successor, continued run id)`.
type StrandedHistories = (Vec<Event>, Vec<Event>, RunId);

fn stranded_histories(
    workflow_id: &WorkflowId,
) -> Result<StrandedHistories, Box<dyn std::error::Error>> {
    let first_run = RunId::new_v4();
    let second_run = RunId::new_v4();
    let envelope = |seq: u64| EventEnvelope {
        seq,
        recorded_at: Utc::now(),
        workflow_id: workflow_id.clone(),
    };
    let input = Payload::from_json(&json!({"next": true}))?;
    let base = vec![
        Event::WorkflowStarted {
            envelope: envelope(1),
            workflow_type: "checkout".to_owned(),
            input: Payload::from_json(&json!({"first": true}))?,
            run_id: first_run.clone(),
            parent_run_id: None,
            package_version: aion_core::PackageVersion::new("a".repeat(64)),
        },
        Event::WorkflowContinuedAsNew {
            envelope: envelope(2),
            input: input.clone(),
            workflow_type: None,
            parent_run_id: first_run.clone(),
        },
    ];
    let mut full = base.clone();
    full.push(Event::WorkflowStarted {
        envelope: envelope(3),
        workflow_type: "checkout".to_owned(),
        input,
        run_id: second_run,
        parent_run_id: Some(first_run.clone()),
        package_version: aion_core::PackageVersion::new("a".repeat(64)),
    });
    Ok((base, full, first_run))
}

fn recovery_context(
    store: Arc<dyn EventStore>,
    runtime: Arc<RuntimeHandle>,
    catalog: Arc<WorkflowCatalog>,
) -> StartupRecoveryContext {
    StartupRecoveryContext {
        store,
        visibility_store: Arc::new(InMemoryStore::default()),
        runtime,
        catalog,
        registry: Arc::new(Registry::default()),
        supervision: Arc::new(SupervisionTree::new()),
        recovery: None,
        search_attribute_schema: Arc::new(SearchAttributeSchema::new()),
        bootstrap_schedule_coordinator: true,
    }
}

/// N-5: the sweep's start races the recovered run's exit monitor, which
/// starts the same successor concurrently. When the sweep's start fails
/// but a re-read shows the successor `WorkflowStarted` durable (the
/// winner's append), the failure is benign and `EngineBuilder::build`
/// must not fail. Before the fix the sweep propagated the loser's error
/// and the whole build failed on a `SequenceConflict`-class race.
#[tokio::test(flavor = "multi_thread")]
async fn sweep_start_race_lost_to_the_exit_monitor_is_benign() -> TestResult {
    let workflow_id = WorkflowId::new_v4();
    let (base, full, _continued) = stranded_histories(&workflow_id)?;
    let store = Arc::new(RacingSuccessorStore {
        workflow_id,
        base_history: base,
        full_history: full,
        // Read #1 is the sweep's pre-start read (no successor yet); the
        // racing monitor wins during the sweep's failed start, so the
        // post-failure re-read (#2) sees the successor.
        successor_appears_after_reads: 1,
        reads: AtomicU32::new(0),
        appears: true,
    });
    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    let catalog = Arc::new(WorkflowCatalog::new());
    let context = recovery_context(store as Arc<dyn EventStore>, Arc::clone(&runtime), catalog);

    sweep_continued_as_new_replacements(&context)
        .await
        .map_err(|error| format!("a lost start race must be benign (N-5): {error}"))?;
    runtime.shutdown()?;
    Ok(())
}

/// The guard must not swallow real failures: a start failure with NO
/// durable successor is a genuine fault and still fails the build.
#[tokio::test(flavor = "multi_thread")]
async fn sweep_start_failure_without_a_successor_still_fails() -> TestResult {
    let workflow_id = WorkflowId::new_v4();
    let (base, full, _continued) = stranded_histories(&workflow_id)?;
    let store = Arc::new(RacingSuccessorStore {
        workflow_id,
        base_history: base,
        full_history: full,
        successor_appears_after_reads: u32::MAX,
        reads: AtomicU32::new(0),
        appears: false,
    });
    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    let catalog = Arc::new(WorkflowCatalog::new());
    let context = recovery_context(store as Arc<dyn EventStore>, Arc::clone(&runtime), catalog);

    let result = sweep_continued_as_new_replacements(&context).await;
    assert!(
        matches!(result, Err(EngineError::WorkflowNotFound { .. })),
        "a start failure without a durable successor must propagate: {result:?}"
    );
    runtime.shutdown()?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn recovered_monitor_installation_failure_drains_retained_completion() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    let context = recovery_context(
        store,
        Arc::clone(&runtime),
        Arc::new(WorkflowCatalog::new()),
    );
    context
        .supervision
        .ensure_type_supervisor("recovered-monitor-failure")?;
    let workflow_id = WorkflowId::new_v4();
    let run_id = RunId::new_v4();
    let history = vec![Event::WorkflowStarted {
        envelope: EventEnvelope {
            seq: 1,
            recorded_at: Utc::now(),
            workflow_id: workflow_id.clone(),
        },
        workflow_type: "recovered-monitor-failure".to_owned(),
        input: Payload::from_json(&json!({"recovered": true}))?,
        run_id: run_id.clone(),
        parent_run_id: None,
        package_version: aion_core::PackageVersion::new("07".repeat(32)),
    }];
    let pid = runtime.spawn_test_process()?;
    let baseline_gates = runtime.activity_delivery_gate_count();
    runtime.deliver_activity_completion_message_with_attempt(
        pid,
        "activity:43",
        String::from(r#"{"recovered":true}"#),
        Some(4),
    )?;
    assert_eq!(runtime.retained_activity_completions(), 1);
    assert_eq!(runtime.retained_activity_attempt_count_for_test(), 1);
    assert_eq!(runtime.activity_delivery_gate_count(), baseline_gates + 1);

    runtime.force_next_monitor_installation_failure_for_test();
    let error = register_recovered_resident(
        &context,
        RecoveredResident {
            workflow_id: &workflow_id,
            workflow_type: "recovered-monitor-failure",
            history: &history,
            history_head: 1,
            projected_status: WorkflowStatus::Running,
            run_id,
            loaded_version: aion_package::ContentHash::from_bytes([7; 32]),
            pid,
            recorder: None,
        },
    )
    .await
    .err()
    .ok_or("forced recovered monitor installation failure registered a resident")?;

    assert!(error.to_string().contains("forced test failure"));
    assert!(context.registry.live_pid(&workflow_id)?.is_none());
    assert!(!runtime.is_live(pid));
    assert_eq!(runtime.retained_activity_completions(), 0);
    assert_eq!(runtime.retained_activity_attempt_count_for_test(), 0);
    assert_eq!(runtime.activity_delivery_gate_count(), baseline_gates);
    runtime.shutdown()?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn recovered_reconcile_failure_after_publication_runs_observed_abort() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    runtime.register_waiting_test_module("rollback-child", "run");
    let context = recovery_context(
        store,
        Arc::clone(&runtime),
        Arc::new(WorkflowCatalog::new()),
    );
    let workflow_id = WorkflowId::new_v4();
    let run_id = RunId::new_v4();
    let history = vec![Event::WorkflowStarted {
        envelope: EventEnvelope {
            seq: 1,
            recorded_at: Utc::now(),
            workflow_id: workflow_id.clone(),
        },
        workflow_type: "reconcile-failure".to_owned(),
        input: Payload::from_json(&json!({"recovered": true}))?,
        run_id: run_id.clone(),
        parent_run_id: None,
        package_version: aion_core::PackageVersion::new("08".repeat(32)),
    }];
    let pid = runtime.spawn_test_process()?;
    let child_input = RuntimeInput::from_payload(&Payload::from_json(&json!({"child": true}))?)?;
    let child_pid = runtime.spawn_activity(pid, "rollback-child", "run", child_input)?;
    let baseline_gates = runtime.activity_delivery_gate_count();
    let seam_runtime = Arc::clone(&runtime);

    let error = register_recovered_resident_with_reconcile(
        &context,
        RecoveredResident {
            workflow_id: &workflow_id,
            workflow_type: "reconcile-failure",
            history: &history,
            history_head: 1,
            projected_status: WorkflowStatus::Running,
            run_id: run_id.clone(),
            loaded_version: aion_package::ContentHash::from_bytes([8; 32]),
            pid,
            recorder: None,
        },
        move |registry, published_workflow_id, _, _| {
            assert_eq!(registry.live_pid(published_workflow_id)?, Some(pid));
            seam_runtime.deliver_activity_completion_message_with_attempt(
                pid,
                "activity:47",
                String::from(r#"{"recovered":true}"#),
                Some(5),
            )?;
            Err(EngineError::Runtime {
                reason: "forced reconcile failure after publication".to_owned(),
            })
        },
    )
    .await
    .err()
    .ok_or("forced reconcile failure registered a recovered resident")?;

    assert!(error.to_string().contains("forced reconcile failure"));
    assert!(context.registry.live_pid(&workflow_id)?.is_none());
    assert!(!runtime.is_live(pid));
    assert!(!runtime.is_live(child_pid));
    assert!(runtime.process_cleanup_complete_for_test(pid));
    assert_eq!(runtime.retained_activity_completions(), 0);
    assert_eq!(runtime.retained_activity_attempt_count_for_test(), 0);
    assert_eq!(runtime.activity_delivery_gate_count(), baseline_gates);
    runtime.shutdown()?;
    Ok(())
}

/// Seeds a workflow whose run recorded `terminal` but whose armed deadline was
/// never cancelled (the two-write crash window), plus its durable timer row.
async fn seed_terminal_with_uncancelled_deadline(
    store: &Arc<dyn EventStore>,
    workflow_id: &WorkflowId,
    run_id: &RunId,
    terminal: Event,
) -> Result<aion_core::TimerId, Box<dyn std::error::Error>> {
    let deadline_id = crate::time::deadline_timer_id(run_id)?;
    let mut recorder = crate::durability::Recorder::new(workflow_id.clone(), Arc::clone(store));
    recorder
        .record_workflow_started(
            Utc::now(),
            crate::durability::WorkflowStartRecord {
                workflow_type: "sweeper".to_owned(),
                input: Payload::from_json(&json!({}))?,
                run_id: run_id.clone(),
                parent_run_id: None,
                package_version: aion_core::PackageVersion::new("a".repeat(64)),
            },
        )
        .await?;
    recorder
        .record_timer_started(Utc::now(), deadline_id.clone(), Utc::now())
        .await?;
    match terminal {
        Event::WorkflowCompleted { result, .. } => {
            recorder
                .record_workflow_completed(Utc::now(), result)
                .await?;
        }
        Event::WorkflowTimedOut { timeout, .. } => {
            recorder
                .record_workflow_timed_out(Utc::now(), timeout)
                .await?;
        }
        other => return Err(format!("unsupported terminal in seed helper: {other:?}").into()),
    }
    store
        .schedule_timer(workflow_id, &deadline_id, Utc::now())
        .await?;
    Ok(deadline_id)
}

/// The startup sweep retires an uncancelled deadline left behind a NON-timeout
/// terminal (crash-window repair), but leaves a `WorkflowTimedOut` deadline live
/// — that one is owned by the deadline handler's teardown and re-driven by
/// `recover_due`.
#[tokio::test]
async fn startup_sweep_retires_non_timeout_terminal_deadlines_but_skips_timed_out()
-> Result<(), Box<dyn std::error::Error>> {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let completed_id = WorkflowId::new_v4();
    let completed_run = RunId::new_v4();
    seed_terminal_with_uncancelled_deadline(
        &store,
        &completed_id,
        &completed_run,
        Event::WorkflowCompleted {
            envelope: EventEnvelope {
                seq: 0,
                recorded_at: Utc::now(),
                workflow_id: completed_id.clone(),
            },
            result: Payload::from_json(&json!("done"))?,
        },
    )
    .await?;
    let timed_out_id = WorkflowId::new_v4();
    let timed_out_run = RunId::new_v4();
    seed_terminal_with_uncancelled_deadline(
        &store,
        &timed_out_id,
        &timed_out_run,
        Event::WorkflowTimedOut {
            envelope: EventEnvelope {
                seq: 0,
                recorded_at: Utc::now(),
                workflow_id: timed_out_id.clone(),
            },
            timeout: "workflow".to_owned(),
        },
    )
    .await?;

    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    let catalog = Arc::new(WorkflowCatalog::new());
    let context = recovery_context(Arc::clone(&store), Arc::clone(&runtime), catalog);

    sweep_uncancelled_terminal_deadlines(&context, SweepScope::ColdBoot).await?;

    let completed_history = store.read_history(&completed_id).await?;
    assert_eq!(
        crate::time::outstanding_deadline_timer(&completed_history, &completed_run),
        None,
        "the sweep retires the deadline behind the completed terminal: {completed_history:#?}"
    );
    let timed_out_history = store.read_history(&timed_out_id).await?;
    assert!(
        crate::time::outstanding_deadline_timer(&timed_out_history, &timed_out_run).is_some(),
        "the sweep leaves a TimedOut deadline live for its owning teardown: {timed_out_history:#?}"
    );
    runtime.shutdown()?;
    Ok(())
}

/// Finding 2: shard adoption must run the terminal-deadline repair. A dead
/// owner's completed run with an uncancelled FUTURE-dated deadline (the case
/// `rearm_future_from_active_histories` / due-only recovery would miss) is
/// retired on the surviving adopter through `recover_adopted_shards`.
#[tokio::test]
async fn adoption_repairs_orphaned_future_dated_terminal_deadline() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let workflow_id = WorkflowId::new_v4();
    let run_id = RunId::new_v4();
    let deadline_id = crate::time::deadline_timer_id(&run_id)?;
    let future = Utc::now()
        .checked_add_signed(chrono::Duration::hours(1))
        .unwrap_or_else(Utc::now);
    let mut seed = crate::durability::Recorder::new(workflow_id.clone(), Arc::clone(&store));
    seed.record_workflow_started(
        Utc::now(),
        crate::durability::WorkflowStartRecord {
            workflow_type: "checkout".to_owned(),
            input: Payload::from_json(&json!({}))?,
            run_id: run_id.clone(),
            parent_run_id: None,
            package_version: aion_core::PackageVersion::new("a".repeat(64)),
        },
    )
    .await?;
    seed.record_timer_started(Utc::now(), deadline_id.clone(), future)
        .await?;
    seed.record_workflow_completed(Utc::now(), Payload::from_json(&json!("done"))?)
        .await?;
    store
        .schedule_timer(&workflow_id, &deadline_id, future)
        .await?;

    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    let context = recovery_context(
        Arc::clone(&store),
        Arc::clone(&runtime),
        Arc::new(WorkflowCatalog::new()),
    );
    super::recover_adopted_shards(context).await?;

    let history = store.read_history(&workflow_id).await?;
    assert_eq!(
        crate::time::outstanding_deadline_timer(&history, &run_id),
        None,
        "adoption retires the future-dated terminal deadline: {history:#?}"
    );
    runtime.shutdown()?;
    Ok(())
}

/// Builds an orphan-shaped completed run (started + armed deadline + completed,
/// deadline never cancelled) plus its durable timer row, returning the run id.
async fn seed_completed_orphan(
    store: &Arc<dyn EventStore>,
    workflow_id: &WorkflowId,
) -> Result<RunId, Box<dyn std::error::Error>> {
    let run_id = RunId::new_v4();
    seed_terminal_with_uncancelled_deadline(
        store,
        workflow_id,
        &run_id,
        Event::WorkflowCompleted {
            envelope: EventEnvelope {
                seq: 0,
                recorded_at: Utc::now(),
                workflow_id: workflow_id.clone(),
            },
            result: Payload::from_json(&json!("done"))?,
        },
    )
    .await?;
    Ok(run_id)
}

/// Registers a live resident handle for `run_id` of `workflow_id`, its recorder
/// sampled at the current durable head — so `live_run_pid` sees it.
async fn register_live_handle(
    store: &Arc<dyn EventStore>,
    registry: &Arc<Registry>,
    workflow_id: &WorkflowId,
    run_id: &RunId,
) -> Result<(), Box<dyn std::error::Error>> {
    let head = store
        .read_history(workflow_id)
        .await?
        .iter()
        .map(Event::seq)
        .max()
        .unwrap_or_default();
    let handle = WorkflowHandle::new(WorkflowHandleParts {
        workflow_id: workflow_id.clone(),
        run_id: run_id.clone(),
        pid: 1,
        workflow_type: "checkout".to_owned(),
        namespace: String::from("default"),
        loaded_version: aion_package::ContentHash::from_bytes([9; 32]),
        cached_status: WorkflowStatus::Running,
        residency: HandleResidency::Resident,
        recorder: crate::durability::Recorder::resume_at(
            workflow_id.clone(),
            Arc::clone(store),
            head,
        ),
        completion: CompletionNotifier::new(),
    });
    registry.insert((workflow_id.clone(), run_id.clone()), handle)?;
    Ok(())
}

fn context_with_registry(
    store: &Arc<dyn EventStore>,
    runtime: &Arc<RuntimeHandle>,
    registry: &Arc<Registry>,
) -> StartupRecoveryContext {
    StartupRecoveryContext {
        store: Arc::clone(store),
        visibility_store: Arc::new(InMemoryStore::default()),
        runtime: Arc::clone(runtime),
        catalog: Arc::new(WorkflowCatalog::new()),
        registry: Arc::clone(registry),
        supervision: Arc::new(SupervisionTree::new()),
        recovery: None,
        search_attribute_schema: Arc::new(SearchAttributeSchema::new()),
        bootstrap_schedule_coordinator: false,
    }
}

/// Finding 3 (a) — ordering: the full cold-boot recovery runs the
/// terminal-deadline sweep BEFORE starting a stranded continue-as-new successor,
/// so the successor's recorder is built after the predecessor deadline is retired
/// and its subsequent append lands. If the sweep ran after successor start, its
/// `ColdBoot` scope would find the successor's live handle and fail recovery — so
/// a successful recovery itself proves the ordering.
#[tokio::test(flavor = "multi_thread")]
async fn cold_boot_repairs_predecessor_deadline_before_starting_the_successor() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    runtime.register_waiting_test_module("checkout_deployed", "run");
    let catalog = Arc::new(WorkflowCatalog::new());
    catalog.note_loaded_workflow_for_test(
        "checkout",
        "checkout_deployed",
        "run",
        aion_package::ContentHash::from_bytes([3; 32]),
    );
    let workflow_id = WorkflowId::new_v4();
    let predecessor = RunId::new_v4();
    let deadline_id = crate::time::deadline_timer_id(&predecessor)?;
    // Stranded continue-as-new: the predecessor recorded ContinuedAsNew with an
    // uncancelled deadline, but the successor was never started.
    let mut seed = crate::durability::Recorder::new(workflow_id.clone(), Arc::clone(&store));
    seed.record_workflow_started(
        Utc::now(),
        crate::durability::WorkflowStartRecord {
            workflow_type: "checkout".to_owned(),
            input: Payload::from_json(&json!({}))?,
            run_id: predecessor.clone(),
            parent_run_id: None,
            package_version: aion_core::PackageVersion::new("a".repeat(64)),
        },
    )
    .await?;
    seed.record_timer_started(Utc::now(), deadline_id.clone(), Utc::now())
        .await?;
    seed.record_workflow_continued_as_new(
        Utc::now(),
        Payload::from_json(&json!({}))?,
        None,
        predecessor.clone(),
    )
    .await?;
    store
        .schedule_timer(&workflow_id, &deadline_id, Utc::now())
        .await?;

    let registry = Arc::new(Registry::default());
    let mut context = context_with_registry(&store, &runtime, &registry);
    context.catalog = catalog;
    super::recover_active_workflows_on_startup(context).await?;

    // The predecessor deadline is retired ...
    let history = store.read_history(&workflow_id).await?;
    assert_eq!(
        crate::time::outstanding_deadline_timer(&history, &predecessor),
        None,
        "the predecessor deadline is retired before the successor starts: {history:#?}"
    );
    // ... and the started successor's recorder is consistent: its next append lands.
    let (successor_run, _) = registry
        .live_run_pid(&workflow_id)?
        .ok_or("the continue-as-new successor was not started")?;
    assert_ne!(successor_run, predecessor, "a fresh successor run started");
    let handle = registry
        .get(&workflow_id, &successor_run)?
        .ok_or("no successor handle")?;
    {
        let recorder = handle.recorder();
        let mut recorder = recorder.lock().await;
        recorder
            .record_workflow_completed(Utc::now(), Payload::from_json(&json!("succeeded"))?)
            .await
            .map_err(|error| format!("the successor recorder was staled by the sweep: {error}"))?;
    }
    runtime.shutdown()?;
    Ok(())
}

/// Finding 3 (b) — adoption scoping: the adoption sweep retires an acquired
/// workflow's orphan deadline (no local handle) but does NOT touch an
/// already-owned resident workflow that legitimately holds a live handle.
#[tokio::test]
async fn adoption_sweep_repairs_acquired_but_skips_owned_resident() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    let acquired_id = WorkflowId::new_v4();
    let acquired_run = seed_completed_orphan(&store, &acquired_id).await?;
    let owned_id = WorkflowId::new_v4();
    let owned_run = seed_completed_orphan(&store, &owned_id).await?;

    let registry = Arc::new(Registry::default());
    // Only the already-owned workflow is resident (holds a live handle).
    register_live_handle(&store, &registry, &owned_id, &owned_run).await?;
    let context = context_with_registry(&store, &runtime, &registry);

    sweep_uncancelled_terminal_deadlines(&context, SweepScope::Adoption).await?;

    assert_eq!(
        crate::time::outstanding_deadline_timer(
            &store.read_history(&acquired_id).await?,
            &acquired_run
        ),
        None,
        "the acquired workflow's orphan deadline is retired"
    );
    assert!(
        crate::time::outstanding_deadline_timer(&store.read_history(&owned_id).await?, &owned_run)
            .is_some(),
        "the adoption sweep never touches an owned resident workflow's deadline"
    );
    runtime.shutdown()?;
    Ok(())
}

/// Finding 3 (c) — defensive check: a live registered handle for a candidate on a
/// COLD-BOOT sweep is an ordering-invariant breach (the sweep must run before
/// repopulation), surfaced as a typed error rather than an append around the live
/// recorder or a silent skip.
#[tokio::test]
async fn cold_boot_sweep_errors_on_a_live_handle_ordering_breach() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    let workflow_id = WorkflowId::new_v4();
    let run_id = seed_completed_orphan(&store, &workflow_id).await?;
    let registry = Arc::new(Registry::default());
    register_live_handle(&store, &registry, &workflow_id, &run_id).await?;
    let context = context_with_registry(&store, &runtime, &registry);

    let result = sweep_uncancelled_terminal_deadlines(&context, SweepScope::ColdBoot).await;

    assert!(
        matches!(result, Err(EngineError::Runtime { .. })),
        "a cold-boot sweep must surface the ordering breach, got {result:?}"
    );
    runtime.shutdown()?;
    Ok(())
}

/// #117 — the seam, not the set.
///
/// `registry::unrecoverable`'s own unit tests drive the set directly and would
/// stay green if `repopulate_active_workflows` never called it. That is exactly
/// how #58 shipped an inert feature behind eight passing tests, so this test
/// crosses the wiring instead: it runs the real startup sweep and asks the
/// registry afterwards.
///
/// It is written as a BASELINE COMPARISON rather than an enumeration — the same
/// sweep sees one run whose pinned version is loaded and one whose is not, so
/// the assertion is about the DIFFERENCE between them. A version of `record`
/// that fired for every run, or for none, fails one half or the other; a
/// hardcoded expectation of "one entry" would pass for the wrong reasons.
#[tokio::test(flavor = "multi_thread")]
async fn a_run_whose_pinned_version_cannot_load_is_retained_as_unrecoverable() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    runtime.register_waiting_test_module("loadable_deployed", "run");
    let catalog = Arc::new(WorkflowCatalog::new());
    catalog.note_loaded_workflow_for_test(
        "loadable",
        "loadable_deployed",
        "run",
        aion_package::ContentHash::from_bytes([3; 32]),
    );

    // The healthy run: its type is in the catalog, so recovery makes it resident.
    let healthy = WorkflowId::new_v4();
    let mut seed = crate::durability::Recorder::new(healthy.clone(), Arc::clone(&store));
    seed.record_workflow_started(
        Utc::now(),
        crate::durability::WorkflowStartRecord {
            workflow_type: "loadable".to_owned(),
            input: Payload::from_json(&json!({}))?,
            run_id: RunId::new_v4(),
            parent_run_id: None,
            // The pin must MATCH the catalogued version, or this run is degraded
            // too and the comparison has no baseline. `ContentHash` renders as
            // lowercase hex, so `from_bytes([3; 32])` is "03" thirty-two times.
            // The first draft pinned "a" * 64 and the baseline half of this test
            // caught it — which is the assertion earning its place.
            package_version: aion_core::PackageVersion::new("03".repeat(32)),
        },
    )
    .await?;

    // The degraded run: nothing in the catalog can serve its type. This is the
    // shape of 0756ecd5 — an archive minted under a superseded package-identity
    // domain, which this build has no encoder for and therefore cannot load.
    let degraded = WorkflowId::new_v4();
    let mut seed = crate::durability::Recorder::new(degraded.clone(), Arc::clone(&store));
    seed.record_workflow_started(
        Utc::now(),
        crate::durability::WorkflowStartRecord {
            workflow_type: "minted_under_a_superseded_identity_domain".to_owned(),
            input: Payload::from_json(&json!({}))?,
            run_id: RunId::new_v4(),
            parent_run_id: None,
            package_version: aion_core::PackageVersion::new("b".repeat(64)),
        },
    )
    .await?;

    let registry = Arc::new(Registry::default());
    let mut context = context_with_registry(&store, &runtime, &registry);
    context.catalog = catalog;
    super::recover_active_workflows_on_startup(context).await?;

    // The degraded run is retained WITH its reason — the whole point is that the
    // operator can ask afterwards, not that a line scrolled past at boot.
    let retained = registry.unrecoverable().get(&degraded)?.ok_or_else(|| {
        Box::<dyn std::error::Error>::from(
            "the run whose pinned version cannot load must be retained as unrecoverable; \
             without this the only record is one boot-log ERROR nobody can query",
        )
    })?;
    assert_eq!(
        retained.workflow_type, "minted_under_a_superseded_identity_domain",
        "the retained entry must name the run's own type"
    );
    assert!(
        !retained.reason.is_empty(),
        "an entry with an empty reason is a flag, not an answer — the operator \
         needs the cause, which is the difference between a redeploy and a \
         data-loss incident"
    );

    // The baseline half: the healthy run must NOT be reported degraded.
    assert!(
        registry.unrecoverable().get(&healthy)?.is_none(),
        "a run that recovered normally must not be reported unrecoverable — a \
         stale degraded flag sends an operator to a redeploy for a healthy run"
    );

    Ok(())
}

/// #117 requirement (5), the PINNING TEST — built before the feature it protects.
///
/// Waffles ruled the direct cancellation of a never-alive run YES-narrowly, under
/// five binding requirements. (5) is *"boot honors a cancellation record before
/// minting a writer — a cancelled run is terminal at boot, unconditionally."*
///
/// It needs no new code: `list_active` filters `== Running`, and the recovery loop
/// skips a terminal projected status before calling anything that mints a writer.
/// But requirement (1) proves the writer slot is vacant at the WRITE INSTANT,
/// which is spatial — and redeploy, the prescribed remedy for a stranded package,
/// is precisely the event that could make the impossible writer possible later.
/// (5) is what makes the invariant temporal: one writer EVER, not one writer now.
///
/// A load-bearing behaviour with no pinning test is one refactor away from
/// fiction, so this exists to FALSIFY a future change that removes it.
///
/// # 🔴 What this test does NOT cover, stated rather than implied
///
/// There are two independent defences and this reaches only the first:
///
/// ```text
/// layer 1  list_active never returns a non-Running run          <- pinned here
/// layer 2  the recovery loop's own is_terminal() skip, which
///          runs before any writer is minted                     <- NOT reached
/// ```
///
/// Because layer 1 excludes the run entirely, it never arrives at layer 2. A test
/// that only asserted "the cancelled run is not resident" would therefore go green
/// while proving nothing about the guard.
///
/// 🔴 **MEASURED, not assumed — and worse than the limit above claims.** Disabling
/// the layer-2 guard entirely (`if false && projected_status.is_terminal()`) and
/// running the whole package leaves the suite at **exit 0, zero failures**. Not
/// only does this test not reach layer 2 — *nothing in `aion-rs` does*. That guard
/// is load-bearing and could be deleted today without a single test noticing,
/// which is the "one refactor away from fiction" law standing on the very guard
/// requirement (5) names.
///
/// The same is true of the `Paused` exclusion immediately below it: disabling
/// that gives exit 0 across two consecutive runs. Both backends' `list_active`
/// filter `== Running` (`memory.rs:374`, libSQL `read.rs:133`), so neither a
/// terminal nor a paused run reaches the loop through any real store, and both
/// guards sit behind a total filter. See #121 — covering them needs an
/// adversarial `EventStore` whose `list_active` returns such a run anyway.
///
/// Covering it needs an adversarial `EventStore` whose `list_active` returns the
/// cancelled run anyway, forcing the loop to defend itself. Named as the follow-up
/// with its cost known, rather than papered over by this green.
#[tokio::test(flavor = "multi_thread")]
async fn a_cancelled_run_is_terminal_at_boot_and_never_has_a_writer_minted() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    runtime.register_waiting_test_module("loadable_deployed", "run");
    let catalog = Arc::new(WorkflowCatalog::new());
    catalog.note_loaded_workflow_for_test(
        "loadable",
        "loadable_deployed",
        "run",
        aion_package::ContentHash::from_bytes([3; 32]),
    );

    // The control. Identical in every respect except that it is not cancelled —
    // so if the sweep silently did nothing, this half fails and the test cannot
    // certify the cancelled half by accident.
    let healthy = WorkflowId::new_v4();
    let mut seed = crate::durability::Recorder::new(healthy.clone(), Arc::clone(&store));
    seed.record_workflow_started(
        Utc::now(),
        crate::durability::WorkflowStartRecord {
            workflow_type: "loadable".to_owned(),
            input: Payload::from_json(&json!({}))?,
            run_id: RunId::new_v4(),
            parent_run_id: None,
            package_version: aion_core::PackageVersion::new("03".repeat(32)),
        },
    )
    .await?;

    // The cancelled run: same type, same loadable version, so the ONLY difference
    // between it and the control is the cancellation record itself.
    let cancelled = WorkflowId::new_v4();
    let mut seed = crate::durability::Recorder::new(cancelled.clone(), Arc::clone(&store));
    seed.record_workflow_started(
        Utc::now(),
        crate::durability::WorkflowStartRecord {
            workflow_type: "loadable".to_owned(),
            input: Payload::from_json(&json!({}))?,
            run_id: RunId::new_v4(),
            parent_run_id: None,
            package_version: aion_core::PackageVersion::new("03".repeat(32)),
        },
    )
    .await?;
    seed.record_workflow_cancelled(
        Utc::now(),
        "direct cancellation of a never-alive run".to_owned(),
    )
    .await?;

    // Layer 1, pinned directly rather than inferred from the outcome below.
    let active = store.list_active().await?;
    assert!(
        active.contains(&healthy),
        "the control must be in the active set, or the sweep has nothing to do \
         and the cancelled half proves nothing"
    );
    assert!(
        !active.contains(&cancelled),
        "a run carrying a cancellation record must not be listed active — this is \
         requirement (5) at the layer that actually delivers it"
    );

    let registry = Arc::new(Registry::default());
    let mut context = context_with_registry(&store, &runtime, &registry);
    context.catalog = catalog;
    super::recover_active_workflows_on_startup(context).await?;

    // The outcome, through the real boot path.
    assert!(
        registry.live_pid(&healthy)?.is_some(),
        "the control must be made resident — without this the assertion below is \
         satisfied by a sweep that did nothing at all"
    );
    assert!(
        registry.live_pid(&cancelled)?.is_none(),
        "a cancelled run must never be made resident, so no writer is ever minted \
         for it — the direct cancellation write closes the door behind itself, and \
         a redeploy that makes its package loadable again must not reopen it"
    );

    Ok(())
}

/// #121 — the terminal guard, reached for the first time.
///
/// `repopulate_active_workflows` re-reads each listed run's history and refuses
/// the ones that project terminal. Every real backend already filters
/// `list_active` to `Running`, so that refusal is a SECOND line of defence behind
/// a total filter: it can never fire through an honest store, and deleting it
/// changes no observable outcome. Measured directly — `if false &&` on the guard
/// left the whole `aion-rs` suite green at 1022 passed / 0 failed.
///
/// [`StaleActiveListStore`] is what makes it reachable: the completed run is
/// forced into `list_active` while its history still says `Completed`, so the
/// loop meets the contradiction the guard exists for and must refuse residency
/// on its own account.
#[tokio::test(flavor = "multi_thread")]
async fn a_terminal_run_forced_into_the_active_list_is_never_made_resident() -> TestResult {
    let stale = Arc::new(aion_store::testing::StaleActiveListStore::new());
    let store: Arc<dyn EventStore> = Arc::clone(&stale) as Arc<dyn EventStore>;
    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    runtime.register_waiting_test_module("loadable_deployed", "run");
    let catalog = Arc::new(WorkflowCatalog::new());
    catalog.note_loaded_workflow_for_test(
        "loadable",
        "loadable_deployed",
        "run",
        aion_package::ContentHash::from_bytes([3; 32]),
    );

    // The control: identical in every respect except that it never completes.
    // Without it, "not resident" is satisfied by a loop that recovered nothing
    // at all — including one that never ran.
    let healthy = WorkflowId::new_v4();
    let mut seed = crate::durability::Recorder::new(healthy.clone(), Arc::clone(&store));
    seed.record_workflow_started(
        Utc::now(),
        crate::durability::WorkflowStartRecord {
            workflow_type: "loadable".to_owned(),
            input: Payload::from_json(&json!({}))?,
            run_id: RunId::new_v4(),
            parent_run_id: None,
            package_version: aion_core::PackageVersion::new("03".repeat(32)),
        },
    )
    .await?;

    let finished = WorkflowId::new_v4();
    let mut seed = crate::durability::Recorder::new(finished.clone(), Arc::clone(&store));
    seed.record_workflow_started(
        Utc::now(),
        crate::durability::WorkflowStartRecord {
            workflow_type: "loadable".to_owned(),
            input: Payload::from_json(&json!({}))?,
            run_id: RunId::new_v4(),
            parent_run_id: None,
            package_version: aion_core::PackageVersion::new("03".repeat(32)),
        },
    )
    .await?;
    seed.record_workflow_completed(Utc::now(), Payload::from_json(&json!({ "done": true }))?)
        .await?;

    // The store's own filter is correct and is NOT what is under test here —
    // pinned so a future change that starts listing terminal runs honestly does
    // not quietly turn this test into a duplicate of the filter's own coverage.
    assert!(
        !store.list_active().await?.contains(&finished),
        "the inner store must still exclude the completed run, or the injection \
         below is not injecting anything"
    );

    stale.force_active(finished.clone());

    // Non-vacuity, asserted rather than assumed: the loop must actually be handed
    // the terminal run. If this fails the guard was never reached and a green
    // outcome below would mean nothing.
    let active = store.list_active().await?;
    assert!(
        active.contains(&finished),
        "the forced terminal run must reach the recovery loop — this is the whole \
         mechanism, and it is asserted here rather than inferred from the outcome"
    );
    assert!(
        active.contains(&healthy),
        "the control must also be listed, or it cannot discriminate"
    );

    let registry = Arc::new(Registry::default());
    let mut context = context_with_registry(&store, &runtime, &registry);
    context.catalog = catalog;
    super::recover_active_workflows_on_startup(context).await?;

    assert!(
        registry.live_pid(&healthy)?.is_some(),
        "the control must be made resident — without this the assertion below is \
         satisfied by a sweep that did nothing at all"
    );
    assert!(
        registry.live_pid(&finished)?.is_none(),
        "a terminal run must never be made resident even when the store insists it \
         is active: residency mints a writer for a run whose history is closed"
    );

    Ok(())
}

/// #121 — the `Paused` guard (#204), reached for the first time.
///
/// `Paused` is NON-terminal, so the `is_terminal` refusal above lets it through;
/// it needs its own guard. That guard was equally uncovered — `if false` on it
/// left the suite green TWICE. (An earlier single observation of a failure here
/// was a flake of the #85 class, not a causal path: a deterministic cause cannot
/// pass twice.)
///
/// This models the real interleaving rather than a filter bug. `list_active` is a
/// SNAPSHOT; a run paused between that snapshot and the per-run re-read was
/// genuinely active when listed and is `Paused` by the time it is read. The
/// double reproduces that ordering deterministically, with no race to lose.
#[tokio::test(flavor = "multi_thread")]
async fn a_paused_run_forced_into_the_active_list_is_never_made_resident() -> TestResult {
    let stale = Arc::new(aion_store::testing::StaleActiveListStore::new());
    let store: Arc<dyn EventStore> = Arc::clone(&stale) as Arc<dyn EventStore>;
    let runtime = Arc::new(RuntimeHandle::new(RuntimeConfig::new(Some(1)))?);
    runtime.register_waiting_test_module("loadable_deployed", "run");
    let catalog = Arc::new(WorkflowCatalog::new());
    catalog.note_loaded_workflow_for_test(
        "loadable",
        "loadable_deployed",
        "run",
        aion_package::ContentHash::from_bytes([3; 32]),
    );

    let healthy = WorkflowId::new_v4();
    let mut seed = crate::durability::Recorder::new(healthy.clone(), Arc::clone(&store));
    seed.record_workflow_started(
        Utc::now(),
        crate::durability::WorkflowStartRecord {
            workflow_type: "loadable".to_owned(),
            input: Payload::from_json(&json!({}))?,
            run_id: RunId::new_v4(),
            parent_run_id: None,
            package_version: aion_core::PackageVersion::new("03".repeat(32)),
        },
    )
    .await?;

    let held = WorkflowId::new_v4();
    let held_run = RunId::new_v4();
    let mut seed = crate::durability::Recorder::new(held.clone(), Arc::clone(&store));
    seed.record_workflow_started(
        Utc::now(),
        crate::durability::WorkflowStartRecord {
            workflow_type: "loadable".to_owned(),
            input: Payload::from_json(&json!({}))?,
            run_id: held_run.clone(),
            parent_run_id: None,
            package_version: aion_core::PackageVersion::new("03".repeat(32)),
        },
    )
    .await?;
    seed.record_workflow_paused(
        Utc::now(),
        held_run,
        Some("operator hold during recovery".to_owned()),
        Some("tester".to_owned()),
    )
    .await?;

    // The projection, pinned directly: `Paused` is NON-terminal, which is exactly
    // why the terminal guard cannot cover this case and a separate one is needed.
    let projected = aion_core::status_from_events(&store.read_history(&held).await?);
    assert_eq!(projected, WorkflowStatus::Paused);
    assert!(
        !projected.is_terminal(),
        "if Paused ever became terminal the guard under test would be redundant \
         and this test would be silently covering the other one instead"
    );
    assert!(
        !store.list_active().await?.contains(&held),
        "the inner store must exclude the paused run before forcing"
    );

    stale.force_active(held.clone());

    let active = store.list_active().await?;
    assert!(
        active.contains(&held),
        "the forced paused run must reach the recovery loop"
    );
    assert!(
        active.contains(&healthy),
        "the control must also be listed, or it cannot discriminate"
    );

    let registry = Arc::new(Registry::default());
    let mut context = context_with_registry(&store, &runtime, &registry);
    context.catalog = catalog;
    super::recover_active_workflows_on_startup(context).await?;

    assert!(
        registry.live_pid(&healthy)?.is_some(),
        "the control must be made resident, or this proves nothing about the paused one"
    );
    assert!(
        registry.live_pid(&held)?.is_none(),
        "a durably-paused run must NOT be respawned (#204 GATE-2): the dispatch \
         hold rebuilt from list_paused keeps its outbox rows held, and an operator \
         `resume` is the only thing that may bring it back"
    );

    // The pause must survive the recovery pass. A loop that respawned it and then
    // recorded a resume would satisfy the residency check above on a later poll.
    assert!(
        store.list_paused().await?.contains(&held),
        "the run must still be durably paused after recovery"
    );

    Ok(())
}
