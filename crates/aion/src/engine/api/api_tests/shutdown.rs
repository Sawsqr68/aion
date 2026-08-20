use super::*;

#[tokio::test]
async fn result_unknown_workflow_returns_not_found() -> Result<(), Box<dyn std::error::Error>> {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = engine_with_loaded_workflow(store, "checkout", "checkout_deployed")?;
    let workflow_id = aion_core::WorkflowId::new_v4();
    let run_id = aion_core::RunId::new_v4();

    let result = engine.result(&workflow_id, &run_id).await;

    assert!(matches!(result, Err(EngineError::WorkflowNotFound { .. })));
    engine.shutdown()?;
    Ok(())
}

/// F5: `Engine::shutdown`'s SECOND failing step must be reported.
///
/// Its two fallible steps both go through `keep`, which returns the first
/// and emits every later one at `error` level. That `else` arm IS the fix
/// for the swallowed-second-error defect, and nothing asserted on it: the
/// sibling below arms only the drain, which fails the SECOND step, so
/// `first_error` is still `None` when `keep` sees it and the `else` is
/// never taken.
///
/// Reaching it needs BOTH steps to fail, which is why `ShutdownGate` gained
/// its own injection seam. The gate's only real failure is mutex poison, so
/// the injected error is `RegistryPoisoned` — a fault wearing the label its
/// injection point can actually issue.
///
/// Killing mutation: replace the `else` body inside `keep` with `{}`. No
/// `error!` is emitted and the capture assertion fails.
#[tokio::test]
async fn a_second_failing_engine_shutdown_step_is_reported_not_swallowed()
-> Result<(), Box<dyn std::error::Error>> {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = engine_with_loaded_workflow(store, "checkout", "checkout_deployed")?;

    // Both steps fail: the gate first (so it is the returned error), the
    // runtime drain second (so it lands in the `else`).
    engine.shutdown_gate.force_close_failure();
    engine.runtime().force_process_exit_drain_failure();

    let (captured, subscriber) = crate::log_capture::LogCapture::new()?;
    let returned = {
        let _installed = tracing::subscriber::set_default(subscriber);
        engine.shutdown()
    };

    let error = returned
        .err()
        .ok_or("both steps failed, so shutdown must not return Ok")?;
    assert!(
        matches!(error, EngineError::RegistryPoisoned),
        "control: the FIRST failure is the one returned, and it is the gate's: {error:?}"
    );

    let reported: Vec<_> = captured
        .at_level("ERROR")?
        .into_iter()
        .filter(|event| event.mentions("a further engine-shutdown step failed"))
        .collect();
    assert!(
        !reported.is_empty(),
        "the second failing step must be reported — a teardown failure with no trace at all \
         is a swallowed Result, which this codebase forbids outright"
    );
    assert!(
        reported
            .iter()
            .any(|event| event.field("step") == Some("runtime.shutdown")),
        "the report must NAME the step, or the operator cannot tell which half failed: \
         {reported:?}"
    );
    Ok(())
}

/// 🔴 A FAILING TEARDOWN STEP DOES NOT CANCEL THE STEPS AFTER IT.
///
/// `shutdown` used to be a chain of `?`, so the first step that failed
/// returned and every later step — the runtime drain, and the three
/// `nif_state` teardowns that release the engine's installed seams — simply
/// never ran. The process then exited with a catalog still installed and an
/// engine reference still reachable from the NIF table: the exact leak the
/// function exists to prevent, produced by the error path of the function
/// itself.
///
/// The reason this went unmeasured is that no drain failure in here can be
/// produced on demand, so no test ever took the error path at all.
///
/// 🔴 WHY THAT SET IS UNREACHABLE IS STATED IN EXACTLY ONE PLACE, AND IT IS
/// NOT HERE. See [`crate::RuntimeHandle::shutdown`].
///
/// This comment has now been wrong twice about it, in two different ways —
/// first "every one is timeout-shaped", then "the shared PRECONDITION: each
/// needs a worker thread or a beamr publisher in a state no test can
/// arrange". The second is false for `ProcessExitOutcomeMissingAfterEvent`,
/// which is a beamr contract breach surfaced through `registry.process_event`
/// and needs neither. It also reasons from a shared property, which is the
/// move `RuntimeHandle::shutdown` explicitly forbids — the set is OPEN, so
/// no property shared by today's members is safe to state about it.
///
/// Two wrong answers in two revisions is what a rule known in two places
/// does, and the cure is subtraction rather than a third attempt: the
/// characterisation lives at the one site that owns the drain, and this one
/// points at it. All that is needed locally is that
/// `force_process_exit_drain_failure` is the named `#[cfg(test)]` seam that
/// makes the path reachable at all, and that it cannot reach a shipped
/// binary.
///
/// **The decisive observable is (c), not (a).** That the error still reaches
/// the caller is true of the old chain too — it is what the old chain did
/// *instead of* finishing. Only `installed_workflow_catalog() == None` can
/// tell "the later steps ran" from "the function returned early", because
/// `clear_engine_seams` is ordered after the failing step. A test asserting
/// only the error would pass against the defect.
#[tokio::test]
async fn a_failing_teardown_step_does_not_skip_the_ones_after_it()
-> Result<(), Box<dyn std::error::Error>> {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = engine_with_loaded_workflow(store, "checkout", "checkout_deployed")?;

    // Installed explicitly, because `engine_with_loaded_workflow` calls
    // `Engine::new` directly and only `EngineBuilder::build` installs the NIF
    // seams. Without this the assertion below would hold on an engine that
    // never had a catalog to clear — a pass measuring nothing. The control
    // that follows is what caught exactly that on the first cut of this test.
    engine
        .runtime()
        .nif_state()
        .set_workflow_catalog(Arc::clone(engine.workflow_catalog()));

    // Control: the seam under (c) is genuinely installed before the call, so
    // a `None` afterwards is the teardown's doing and not the absence of
    // anything to tear down.
    assert!(
        engine
            .runtime()
            .nif_state()
            .installed_workflow_catalog()
            .is_some(),
        "control: the catalog must be installed before shutdown, or asserting it is \
         cleared afterwards measures nothing"
    );

    engine.runtime().force_process_exit_drain_failure();
    let error = engine
        .shutdown()
        .err()
        .ok_or("an injected drain failure must be reported, not swallowed")?;

    assert!(
        matches!(error, EngineError::ProcessExitRegistryPoisoned),
        "(a) the failure must reach the caller as itself: {error:?}"
    );
    assert!(
        !engine.runtime().engine_tasks().is_epoch_open(),
        "(b) the epoch must be closed — it is closed FIRST, so a shutdown that failed \
         later must still leave it shut"
    );
    assert!(
        engine
            .runtime()
            .nif_state()
            .installed_workflow_catalog()
            .is_none(),
        "(c) THE DECISIVE ONE: `clear_engine_seams` is ordered AFTER the step that \
         failed, so a still-installed catalog means the failure returned early and \
         the engine leaked its seams"
    );
    Ok(())
}

#[tokio::test]
async fn continue_as_new_unknown_workflow_returns_not_found()
-> Result<(), Box<dyn std::error::Error>> {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = engine_with_loaded_workflow(store, "checkout", "checkout_deployed")?;
    let workflow_id = aion_core::WorkflowId::new_v4();
    let run_id = aion_core::RunId::new_v4();

    let result = engine
        .continue_as_new(&workflow_id, &run_id, payload("next")?, None)
        .await;

    assert!(matches!(result, Err(EngineError::WorkflowNotFound { .. })));
    engine.shutdown()?;
    Ok(())
}

#[tokio::test]
async fn list_workflows_merges_live_and_terminal_without_duplicates()
-> Result<(), Box<dyn std::error::Error>> {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = engine_with_loaded_workflow(Arc::clone(&store), "checkout", "checkout_deployed")?;
    let running = insert_active_handle(&engine, Arc::clone(&store), "checkout").await?;
    let completed = engine
        .start_workflow(
            "checkout",
            payload("input")?,
            HashMap::new(),
            String::from("default"),
        )
        .await?;
    terminate::complete(
        termination_context(&engine),
        completed.workflow_id(),
        completed.run_id(),
        payload("result")?,
    )
    .await?;

    let summaries = engine.list_workflows(WorkflowFilter::default()).await?;
    assert_eq!(summaries.len(), 2);
    assert!(summaries.iter().any(|summary| {
        &summary.workflow_id == running.workflow_id() && summary.status == WorkflowStatus::Running
    }));
    assert!(summaries.iter().any(|summary| {
        &summary.workflow_id == completed.workflow_id()
            && summary.status == WorkflowStatus::Completed
    }));

    let completed_only = engine
        .list_workflows(WorkflowFilter {
            status: Some(WorkflowStatus::Completed),
            ..WorkflowFilter::default()
        })
        .await?;
    assert_eq!(completed_only.len(), 1);
    assert_eq!(&completed_only[0].workflow_id, completed.workflow_id());
    engine.shutdown()?;
    Ok(())
}

#[tokio::test]
async fn shutdown_rejects_subsequent_starts() -> Result<(), Box<dyn std::error::Error>> {
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
    terminate::complete(
        termination_context(&engine),
        handle.workflow_id(),
        handle.run_id(),
        payload("result")?,
    )
    .await?;

    engine.shutdown()?;
    let result = engine
        .start_workflow(
            "checkout",
            payload("after-shutdown")?,
            HashMap::new(),
            String::from("default"),
        )
        .await;

    assert!(matches!(result, Err(EngineError::ShuttingDown)));
    Ok(())
}

#[tokio::test]
async fn shutdown_is_idempotent() -> Result<(), Box<dyn std::error::Error>> {
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
    terminate::complete(
        termination_context(&engine),
        handle.workflow_id(),
        handle.run_id(),
        payload("result")?,
    )
    .await?;

    engine.shutdown()?;
    let second = engine.shutdown();

    assert!(
        second.is_ok(),
        "double shutdown should succeed; got {second:?}"
    );
    Ok(())
}

#[tokio::test]
async fn shutdown_rejects_schedule_creation() -> Result<(), Box<dyn std::error::Error>> {
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
    terminate::complete(
        termination_context(&engine),
        handle.workflow_id(),
        handle.run_id(),
        payload("result")?,
    )
    .await?;
    engine.shutdown()?;

    let config = aion_core::ScheduleConfig {
        trigger: aion_core::TriggerSpec::Interval {
            period: Duration::from_secs(60),
        },
        overlap_policy: aion_core::OverlapPolicy::Skip,
        catch_up_policy: aion_core::CatchUpPolicy::Skip,
        workflow_type: String::from("checkout"),
        input: payload("scheduled")?,
        search_attributes: HashMap::new(),
    };
    let result = engine.create_schedule(config).await;

    assert!(
        matches!(result, Err(EngineError::ShuttingDown)),
        "create_schedule after shutdown should return ShuttingDown; got {result:?}"
    );
    Ok(())
}

#[test]
fn engine_drop_releases_the_descriptor_owning_task_runtime() -> Result<(), EngineError> {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = engine_with_loaded_workflow(store, "checkout", "checkout_deployed")?;
    let tasks = engine.runtime().engine_tasks();
    assert!(
        tasks.owns_runtime(),
        "control: a live engine owns its task runtime"
    );

    drop(engine);

    assert!(
        !tasks.owns_runtime(),
        "Engine Drop must release the task runtime so its I/O descriptors cannot accumulate"
    );
    Ok(())
}
