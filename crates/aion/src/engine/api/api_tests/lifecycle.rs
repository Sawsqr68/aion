use super::*;

#[tokio::test]
async fn start_then_cancel_records_started_then_cancelled() -> Result<(), Box<dyn std::error::Error>>
{
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = engine_with_loaded_workflow(Arc::clone(&store), "checkout", "checkout_deployed")?;
    let handle = engine
        .start_workflow(
            "checkout",
            payload("input")?,
            HashMap::new(),
            String::from("default"),
        )
        .await?;

    engine
        .cancel(
            handle.workflow_id(),
            handle.run_id(),
            "caller requested cancellation",
        )
        .await?;

    let history = store.read_history(handle.workflow_id()).await?;
    match history.as_slice() {
        [
            Event::WorkflowStarted { .. },
            Event::WorkflowCancelled { reason, .. },
        ] => {
            assert_eq!(reason, "caller requested cancellation");
        }
        other => return Err(format!("expected started then cancelled, found {other:?}").into()),
    }
    engine.shutdown()?;
    Ok(())
}

fn test_envelope(workflow_id: &WorkflowId, seq: u64) -> EventEnvelope {
    EventEnvelope {
        seq,
        recorded_at: chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap_or_default(),
        workflow_id: workflow_id.clone(),
    }
}

fn started_event(workflow_id: &WorkflowId, seq: u64) -> Event {
    Event::WorkflowStarted {
        envelope: test_envelope(workflow_id, seq),
        workflow_type: String::from("checkout"),
        input: Payload::new(aion_core::ContentType::Json, b"{}".to_vec()),
        run_id: RunId::new_v4(),
        parent_run_id: None,
        parent_workflow_id: None,
        package_version: PackageVersion::new("a".repeat(64)),
    }
}

fn timer_started_event(workflow_id: &WorkflowId, seq: u64, timer_id: &TimerId) -> Event {
    Event::TimerStarted {
        envelope: test_envelope(workflow_id, seq),
        timer_id: timer_id.clone(),
        fire_at: chrono::DateTime::from_timestamp(1_700_000_500, 0).unwrap_or_default(),
    }
}

fn timer_fired_event(workflow_id: &WorkflowId, seq: u64, timer_id: &TimerId) -> Event {
    Event::TimerFired {
        envelope: test_envelope(workflow_id, seq),
        timer_id: timer_id.clone(),
    }
}

fn timer_cancelled_event(workflow_id: &WorkflowId, seq: u64, timer_id: &TimerId) -> Event {
    Event::TimerCancelled {
        envelope: test_envelope(workflow_id, seq),
        timer_id: timer_id.clone(),
        cause: TimerCancelCause::WorkflowIntent,
    }
}

#[test]
fn live_timers_lists_started_and_unterminated() {
    let workflow_id = WorkflowId::new_v4();
    let first = TimerId::anonymous(0);
    let second = TimerId::anonymous(1);
    let history = vec![
        started_event(&workflow_id, 0),
        timer_started_event(&workflow_id, 1, &first),
        timer_started_event(&workflow_id, 2, &second),
    ];
    assert_eq!(
        live_timers_in_active_segment(&history),
        vec![first, second],
        "both started, unterminated timers should be live, in start order"
    );
}

#[test]
fn live_timers_excludes_fired_and_cancelled() {
    let workflow_id = WorkflowId::new_v4();
    let fired = TimerId::anonymous(0);
    let cancelled = TimerId::anonymous(1);
    let live = TimerId::anonymous(2);
    let history = vec![
        started_event(&workflow_id, 0),
        timer_started_event(&workflow_id, 1, &fired),
        timer_started_event(&workflow_id, 2, &cancelled),
        timer_started_event(&workflow_id, 3, &live),
        timer_fired_event(&workflow_id, 4, &fired),
        timer_cancelled_event(&workflow_id, 5, &cancelled),
    ];
    assert_eq!(
        live_timers_in_active_segment(&history),
        vec![live],
        "only the timer with no terminal event remains live"
    );
}

#[test]
fn live_timers_dedups_repeated_start() {
    let workflow_id = WorkflowId::new_v4();
    let timer = TimerId::anonymous(0);
    let history = vec![
        started_event(&workflow_id, 0),
        timer_started_event(&workflow_id, 1, &timer),
        timer_started_event(&workflow_id, 2, &timer),
    ];
    assert_eq!(live_timers_in_active_segment(&history), vec![timer]);
}

#[test]
fn live_timers_scopes_to_active_run_segment() {
    // A timer started in a prior run (before a continue-as-new
    // `WorkflowStarted`) must not be surfaced for the replacement run.
    let workflow_id = WorkflowId::new_v4();
    let prior_run = TimerId::anonymous(0);
    let current_run = TimerId::anonymous(0);
    let history = vec![
        started_event(&workflow_id, 0),
        timer_started_event(&workflow_id, 1, &prior_run),
        started_event(&workflow_id, 2),
        timer_started_event(&workflow_id, 3, &current_run),
    ];
    assert_eq!(
        live_timers_in_active_segment(&history),
        vec![current_run],
        "only timers from the latest WorkflowStarted segment are live"
    );
}

#[test]
fn live_timers_empty_history_is_empty() {
    assert!(live_timers_in_active_segment(&[]).is_empty());
}

/// Build an engine whose runtime has the production timer NIF bridge
/// installed against the given store + registry, so `Engine::cancel`'s timer
/// cleanup exercises the real `TimerService` path (not a fake). Must be
/// called from within a tokio runtime (`Handle::current()`).
fn engine_with_timer_bridge(
    store: Arc<dyn EventStore>,
    registry: Arc<Registry>,
) -> Result<Engine, EngineError> {
    let runtime = RuntimeHandle::new(RuntimeConfig::new(Some(1)))?;
    runtime.register_waiting_test_module("checkout_deployed", "run");
    crate::runtime::nif_timer_bridge::install_timer_nif_bridge(
        runtime.nif_state(),
        Arc::clone(&registry),
        Arc::clone(&store),
        tokio::runtime::Handle::current(),
        crate::runtime::SignalDeliveryConfig::default(),
    );
    let visibility_store: Arc<dyn VisibilityStore> = Arc::new(InMemoryStore::default());
    Ok(Engine::new(EngineComponents {
        store,
        visibility_store,
        runtime: Arc::new(runtime),
        catalog: workflow_catalog("checkout", "checkout_deployed"),
        registry,
        supervision: Arc::new(SupervisionTree::new()),
        delegated: DelegatedSeams::default(),
        signal_handoff: Arc::new(crate::signal::SignalResumeHandoff::new()),
        search_attribute_schema: Arc::new(SearchAttributeSchema::new()),
        visibility_reconciliation_task: None,
        deferred_startup_recovery: None,
    }))
}

/// Root-cause regression: cancelling a workflow with a live durable timer
/// must record `TimerCancelled` (before the terminal `WorkflowCancelled`),
/// so the timer is dead in history and recovery never fires it as an
/// orphan. Drives the real `Engine::cancel` against a runtime with the
/// production timer bridge installed.
#[tokio::test(flavor = "multi_thread")]
async fn cancel_records_timer_cancelled_before_workflow_cancelled()
-> Result<(), Box<dyn std::error::Error>> {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let registry = Arc::new(Registry::default());
    let engine = engine_with_timer_bridge(Arc::clone(&store), Arc::clone(&registry))?;

    let handle = engine
        .start_workflow(
            "checkout",
            payload("input")?,
            HashMap::new(),
            String::from("default"),
        )
        .await?;

    // Arm a live durable timer for the resident run and record its
    // `TimerStarted`, exactly as the resume-live handoff would in production.
    let timer_id = TimerId::anonymous(0);
    let fire_at = chrono::Utc::now() + chrono::Duration::hours(1);
    handle
        .recorder()
        .lock()
        .await
        .record_timer_started(chrono::Utc::now(), timer_id.clone(), fire_at)
        .await?;
    let timer_service =
        crate::runtime::nif_timer_bridge::installed_timer_service(engine.runtime().nif_state())
            .map_err(|error| format!("timer service unavailable: {error}"))?;
    timer_service
        .schedule(handle.workflow_id().clone(), timer_id.clone(), fire_at)
        .await?;

    engine
        .cancel(
            handle.workflow_id(),
            handle.run_id(),
            "caller requested cancellation",
        )
        .await?;

    let history = store.read_history(handle.workflow_id()).await?;
    match history.as_slice() {
        [
            Event::WorkflowStarted { .. },
            Event::TimerStarted {
                timer_id: started, ..
            },
            Event::TimerCancelled {
                timer_id: cancelled,
                ..
            },
            Event::WorkflowCancelled { reason, .. },
        ] => {
            assert_eq!(started, &timer_id);
            assert_eq!(cancelled, &timer_id, "the live timer must be cancelled");
            assert_eq!(reason, "caller requested cancellation");
        }
        other => {
            return Err(format!(
                "expected [started, timer-started, timer-cancelled, cancelled], found {other:?}"
            )
            .into());
        }
    }
    engine.shutdown()?;
    Ok(())
}

/// All live timers (not just one) are cancelled, in start order, before the
/// terminal `WorkflowCancelled`.
#[tokio::test(flavor = "multi_thread")]
async fn cancel_cancels_multiple_live_timers() -> Result<(), Box<dyn std::error::Error>> {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let registry = Arc::new(Registry::default());
    let engine = engine_with_timer_bridge(Arc::clone(&store), Arc::clone(&registry))?;
    let handle = engine
        .start_workflow(
            "checkout",
            payload("input")?,
            HashMap::new(),
            String::from("default"),
        )
        .await?;

    let first = TimerId::anonymous(0);
    let second = TimerId::anonymous(1);
    let fire_at = chrono::Utc::now() + chrono::Duration::hours(1);
    {
        let recorder = handle.recorder();
        let mut recorder = recorder.lock().await;
        recorder
            .record_timer_started(chrono::Utc::now(), first.clone(), fire_at)
            .await?;
        recorder
            .record_timer_started(chrono::Utc::now(), second.clone(), fire_at)
            .await?;
    }

    engine
        .cancel(handle.workflow_id(), handle.run_id(), "stop")
        .await?;

    let history = store.read_history(handle.workflow_id()).await?;
    match history.as_slice() {
        [
            Event::WorkflowStarted { .. },
            Event::TimerStarted {
                timer_id: started_first,
                ..
            },
            Event::TimerStarted {
                timer_id: started_second,
                ..
            },
            Event::TimerCancelled {
                timer_id: cancelled_first,
                ..
            },
            Event::TimerCancelled {
                timer_id: cancelled_second,
                ..
            },
            Event::WorkflowCancelled { .. },
        ] => {
            assert_eq!(started_first, &first);
            assert_eq!(started_second, &second);
            assert_eq!(cancelled_first, &first, "first live timer cancelled first");
            assert_eq!(
                cancelled_second, &second,
                "second live timer cancelled second"
            );
        }
        other => {
            return Err(format!(
                "expected two timer-cancels before workflow-cancel, found {other:?}"
            )
            .into());
        }
    }
    engine.shutdown()?;
    Ok(())
}

/// End-to-end source-of-bug proof: a cancelled workflow leaves no orphan for
/// startup recovery. With a past-due durable timer row (the exact shape that
/// bricked startup before the fix), recovery surfaces no `UnknownWorkflow`
/// and fires nothing — because cancel recorded `TimerCancelled`, so the
/// timer is dead in history. Complements the committed `recover_due` defense
/// test by proving the orphan is gone *at the source*.
#[tokio::test(flavor = "multi_thread")]
async fn cancelled_workflow_leaves_no_orphan_for_recovery() -> Result<(), Box<dyn std::error::Error>>
{
    let concrete: Arc<InMemoryStore> = Arc::new(InMemoryStore::default());
    let store: Arc<dyn EventStore> = concrete.clone();
    let registry = Arc::new(Registry::default());
    let engine = engine_with_timer_bridge(Arc::clone(&store), Arc::clone(&registry))?;
    let handle = engine
        .start_workflow(
            "checkout",
            payload("input")?,
            HashMap::new(),
            String::from("default"),
        )
        .await?;
    let workflow_id = handle.workflow_id().clone();

    // A live timer whose durable row is already past-due, inserted directly
    // (no wheel arm, so nothing races the cancel).
    let timer_id = TimerId::anonymous(0);
    let fire_at = chrono::Utc::now() - chrono::Duration::hours(1);
    handle
        .recorder()
        .lock()
        .await
        .record_timer_started(chrono::Utc::now(), timer_id.clone(), fire_at)
        .await?;
    concrete
        .schedule_timer(&workflow_id, &timer_id, fire_at)
        .await?;

    let timer_service =
        crate::runtime::nif_timer_bridge::installed_timer_service(engine.runtime().nif_state())
            .map_err(|error| format!("timer service unavailable: {error}"))?;

    engine.cancel(&workflow_id, handle.run_id(), "stop").await?;

    // Cancel removed the workflow from the registry and the durable row is
    // now past-due — exactly the orphan scenario. Recovery must handle it
    // cleanly: the recorded `TimerCancelled` makes `fire_timer` a no-op, so
    // no `TimerFired` and (critically) no `UnknownWorkflow`.
    let readable: Arc<dyn ReadableEventStore> = concrete.clone();
    TimerRecovery::new(readable, timer_service, Duration::ZERO)
        .recover_on_startup(chrono::Utc::now())
        .await?;

    let history = concrete.read_history(&workflow_id).await?;
    assert!(
        !history
            .iter()
            .any(|event| matches!(event, Event::TimerFired { .. })),
        "no timer should fire for a cancelled workflow during recovery"
    );
    assert!(
        history
            .iter()
            .any(|event| matches!(event, Event::TimerCancelled { .. })),
        "cancel must have recorded TimerCancelled at the source"
    );
    engine.shutdown()?;
    Ok(())
}

#[tokio::test]
async fn result_returns_completed_payload() -> Result<(), Box<dyn std::error::Error>> {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = engine_with_loaded_workflow(Arc::clone(&store), "checkout", "checkout_deployed")?;
    let handle = engine
        .start_workflow(
            "checkout",
            payload("input")?,
            HashMap::new(),
            String::from("default"),
        )
        .await?;
    let result_payload = payload("result")?;

    terminate::complete(
        termination_context(&engine),
        handle.workflow_id(),
        handle.run_id(),
        result_payload.clone(),
    )
    .await?;

    assert_eq!(
        engine.result(handle.workflow_id(), handle.run_id()).await?,
        Ok(result_payload)
    );
    engine.shutdown()?;
    Ok(())
}

#[tokio::test]
async fn result_returns_failed_workflow_error() -> Result<(), Box<dyn std::error::Error>> {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = engine_with_loaded_workflow(Arc::clone(&store), "checkout", "checkout_deployed")?;
    let handle = engine
        .start_workflow(
            "checkout",
            payload("input")?,
            HashMap::new(),
            String::from("default"),
        )
        .await?;
    let error = workflow_error("workflow failed");

    terminate::fail(
        termination_context(&engine),
        handle.workflow_id(),
        handle.run_id(),
        error.clone(),
    )
    .await?;

    assert_eq!(
        engine.result(handle.workflow_id(), handle.run_id()).await?,
        Err(error)
    );
    engine.shutdown()?;
    Ok(())
}
