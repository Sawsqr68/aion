//! Startup recovery wiring used by `EngineBuilder::build()`: schedule
//! coordinator bootstrap, active-workflow repopulation, and timer recovery.

use std::{collections::HashSet, sync::Arc, time::Duration};

use chrono::Utc;

use aion_core::{Event, Payload, RunId, SearchAttributeSchema, WorkflowStatus, status_from_events};
use aion_store::EventStore;
use aion_store::visibility::VisibilityStore;

use crate::{
    CompletionNotifier, EngineError, HandleResidency, Registry, RuntimeHandle, SupervisionTree,
    WorkflowCatalog, WorkflowHandle, WorkflowHandleParts,
    durability::{
        ActiveWorkflowRecovery, ActiveWorkflowRecoverySeam, ActiveWorkflowRecoverySeamImpl,
        Recorder,
    },
    lifecycle::completion::{ProcessExitContext, handle_process_exit},
    time::TimerRecovery,
};

use super::api_schedule::{
    schedule_coordinator_package_version, schedule_coordinator_run_id,
    schedule_coordinator_workflow_id, schedule_coordinator_workflow_type,
};
use super::startup_sweeps::{
    SweepScope, sweep_continued_as_new_replacements, sweep_recorded_children,
    sweep_uncancelled_terminal_deadlines,
};

pub(super) async fn recover_timers_on_startup(
    nif_state: &crate::runtime::EngineNifState,
    store: Arc<dyn EventStore>,
) -> Result<(), EngineError> {
    let readable_store: Arc<dyn aion_store::ReadableEventStore> = store;
    let timer_service = crate::runtime::nif_timer_bridge::installed_timer_service(nif_state)
        .map_err(|error| EngineError::Runtime {
            reason: format!("timer recovery service unavailable: {error}"),
        })?;
    TimerRecovery::new(readable_store, timer_service, Duration::ZERO)
        .recover_on_startup(Utc::now())
        .await
        .map(|_| ())
        .map_err(|error| EngineError::Runtime {
            reason: format!("timer recovery failed: {error}"),
        })
}

pub(crate) struct StartupRecoveryContext {
    pub(crate) store: Arc<dyn EventStore>,
    pub(crate) visibility_store: Arc<dyn VisibilityStore>,
    pub(crate) runtime: Arc<RuntimeHandle>,
    pub(crate) catalog: Arc<WorkflowCatalog>,
    pub(crate) registry: Arc<Registry>,
    pub(crate) supervision: Arc<SupervisionTree>,
    pub(crate) recovery: Option<Arc<dyn ActiveWorkflowRecoverySeam>>,
    pub(crate) search_attribute_schema: Arc<SearchAttributeSchema>,
    /// When false, skip seeding the schedule-coordinator history at startup
    /// (multi-node: only the coordinator-shard owner seeds it). Default true.
    pub(crate) bootstrap_schedule_coordinator: bool,
}

/// Resolve the build-time startup-recovery decision (#266).
///
/// A deferred build skips the recovery steps and stows the two recovery
/// inputs the constructed engine does not itself hold; the host runs
/// [`super::api::Engine::run_startup_recovery`] once every seam its activity
/// dispatcher consults is installed — recovery replay re-dispatches in-flight
/// activities through that dispatcher the moment it runs. A normal build runs
/// workflow and timer recovery here, exactly as it always has.
///
/// # Errors
///
/// Returns [`EngineError`] when either recovery step fails on the
/// non-deferred path.
pub(super) async fn resolve_startup_recovery(
    defer: bool,
    nif_state: &crate::runtime::EngineNifState,
    context: StartupRecoveryContext,
) -> Result<Option<super::startup_deferred::DeferredStartupRecovery>, EngineError> {
    if defer {
        return Ok(Some(super::startup_deferred::DeferredStartupRecovery {
            recovery: context.recovery,
            bootstrap_schedule_coordinator: context.bootstrap_schedule_coordinator,
        }));
    }
    let store = Arc::clone(&context.store);
    recover_active_workflows_on_startup(context).await?;
    recover_timers_on_startup(nif_state, store).await?;
    Ok(None)
}

pub(super) async fn recover_active_workflows_on_startup(
    context: StartupRecoveryContext,
) -> Result<(), EngineError> {
    if context.bootstrap_schedule_coordinator {
        seed_schedule_coordinator_history(Arc::clone(&context.store)).await?;
    }
    crate::lifecycle::visibility::reconcile_visibility(
        Arc::clone(&context.store),
        Arc::clone(&context.visibility_store),
    )
    .await?;
    // Repair orphaned non-timeout terminal deadlines BEFORE repopulating active
    // workflows and BEFORE starting continue-as-new successors: at this point no
    // local workflow recorder exists, so the sweep's independent recorder is the
    // sole writer and cannot stale a recovered or successor recorder's head.
    sweep_uncancelled_terminal_deadlines(&context, SweepScope::ColdBoot).await?;
    let recovery = context.recovery.clone().unwrap_or_else(|| {
        Arc::new(ActiveWorkflowRecoverySeamImpl::new(Arc::clone(
            &context.runtime,
        ))) as Arc<dyn ActiveWorkflowRecoverySeam>
    });
    repopulate_active_workflows(&context, recovery.as_ref()).await?;
    sweep_continued_as_new_replacements(&context).await
}

/// Re-resident the active workflows on shards this LIVE engine has just adopted
/// from a dead peer (SS-5 failover).
///
/// This is the post-boot counterpart to
/// [`recover_active_workflows_on_startup`]: it re-runs the SAME idempotent
/// repopulation over the (now-widened) owned-shard enumeration, so every
/// workflow on a newly-adopted shard whose history `become_live` union-merged
/// locally is re-spawned and registered through the production recovery seam.
///
/// It deliberately does NOT seed the schedule coordinator (a survivor adopting a
/// peer's shards is not forming the cluster) and skips workflows already resident
/// in this engine's registry (the idempotency guard in
/// [`repopulate_active_workflows`]), so this engine's own in-flight workflows are
/// untouched — only the adopted ones are recovered.
pub(super) async fn recover_adopted_shards(
    context: StartupRecoveryContext,
) -> Result<(), EngineError> {
    crate::lifecycle::visibility::reconcile_visibility(
        Arc::clone(&context.store),
        Arc::clone(&context.visibility_store),
    )
    .await?;
    // Repair a dead owner's orphaned non-timeout terminal deadline on the
    // surviving adopter BEFORE repopulating/spawning the acquired shards'
    // recovered processes — under the established shard fence. Ordering removes
    // every RECOVERY-path writer from this sweep's window (nothing on the acquired
    // shards is repopulated or spawned yet), but it does NOT exclude public
    // post-fence actors: adoption has already published shard ownership, so
    // `reopen_workflow` (and other public writers) can append for an acquired
    // workflow concurrently. The sweep is therefore an OPTIMISTIC writer arbitrated
    // at the store's per-workflow sequence check by the complete repair predicate
    // (see `startup_sweeps::sweep_uncancelled_terminal_deadlines`). `SweepScope::Adoption`
    // skips an already-owned resident workflow (which legitimately holds a live
    // handle) rather than writing around it. Covers both due and future deadline
    // rows, which `rearm_future_from_active_histories` (list_active only) would miss.
    //
    // Known pre-existing race (NOT introduced here, and NOT widened by this sweep):
    // `repopulate_active_workflows` below can observe a durable `WorkflowReopened`
    // while production reopen has appended but not yet registered its handle, and
    // its single `live_pid` check-then-spawn is unarbitrated while `Registry::insert`
    // silently replaces — so adoption and reopen can both publish a resident for one
    // `(workflow, run)`. This is reachable on any reopen-versus-adoption overlap with
    // no sweep candidate at all; it is BOARDED as a separate follow-up lane (atomic
    // resident-publication arbitration: `insert_if_absent`/pre-spawn reservation,
    // loser cancels its spawn, full-path barrier test). The deadline sweep neither
    // creates nor widens that window.
    sweep_uncancelled_terminal_deadlines(&context, SweepScope::Adoption).await?;
    let recovery = context.recovery.clone().unwrap_or_else(|| {
        Arc::new(ActiveWorkflowRecoverySeamImpl::new(Arc::clone(
            &context.runtime,
        ))) as Arc<dyn ActiveWorkflowRecoverySeam>
    });
    repopulate_active_workflows(&context, recovery.as_ref()).await?;
    sweep_continued_as_new_replacements(&context).await
}

async fn seed_schedule_coordinator_history(store: Arc<dyn EventStore>) -> Result<(), EngineError> {
    let workflow_id = schedule_coordinator_workflow_id();
    let history = store.as_ref().read_history(&workflow_id).await?;
    if !history.is_empty() {
        return Ok(());
    }

    let input = Payload::from_json(&serde_json::json!({})).map_err(|error| EngineError::Load {
        reason: format!("failed to build schedule coordinator input payload: {error}"),
    })?;
    let run_id = schedule_coordinator_run_id();
    let mut recorder = Recorder::new(workflow_id, store);
    recorder
        .record_workflow_started(
            Utc::now(),
            crate::durability::WorkflowStartRecord {
                workflow_type: schedule_coordinator_workflow_type().to_owned(),
                input,
                run_id,
                parent_run_id: None,
                parent_workflow_id: None,
                package_version: schedule_coordinator_package_version(),
            },
        )
        .await?;
    Ok(())
}

async fn repopulate_active_workflows(
    context: &StartupRecoveryContext,
    recovery: &dyn ActiveWorkflowRecoverySeam,
) -> Result<(), EngineError> {
    let store = &context.store;
    let catalog = &context.catalog;
    let registry = &context.registry;
    let supervision = &context.supervision;
    let mut recoverable = store.as_ref().list_active().await?;
    recoverable.extend(store.as_ref().list_paused().await?);
    let mut seen = HashSet::new();
    recoverable.retain(|workflow_id| seen.insert(workflow_id.clone()));
    for workflow_id in recoverable {
        // Idempotent repopulation: a workflow already resident in this engine's
        // registry must not be re-spawned. At the boot path the registry is
        // empty, so this never fires; on the SS-5 failover re-run (adopting a
        // dead peer's shards into a LIVE engine) it skips the workflows this node
        // already owns, leaving only the newly-adopted ones to recover.
        if registry.live_pid(&workflow_id)?.is_some() {
            // Already resident, so any degraded verdict this process recorded
            // for it is now false. Clearing here rather than only on the
            // recovery path covers the SS-5 adoption re-run, which reaches
            // workflows that became resident by a route this loop never took.
            registry.unrecoverable().clear(&workflow_id)?;
            continue;
        }
        let history = store.as_ref().read_history(&workflow_id).await?;
        let workflow_type = started_workflow_type(&workflow_id, &history)?;
        let projected_status = status_from_events(&history);
        if projected_status.is_terminal() {
            tracing::warn!(
                workflow_id = %workflow_id,
                status = ?projected_status,
                "store listed terminal workflow as active during startup; skipping resident recovery"
            );
            continue;
        }
        // #36: an ordinary paused run stays non-resident, but Paused is not a
        // blanket recovery exclusion. When a remote attempt is dangling, startup
        // must replay far enough for its dispatcher/harness to adopt the surviving
        // execution; otherwise the later operator resume is the first observer and
        // falsely disowns live work as server death. The paused-runs dispatch hold
        // remains in force, so unrelated held outbox rows are not released here.
        if projected_status == WorkflowStatus::Paused && !history_needs_activity_adoption(&history)
        {
            continue;
        }
        supervision.ensure_type_supervisor(workflow_type.clone())?;

        // Per-workflow isolation (#62): a run whose pinned package version
        // (or replay metadata) cannot be resolved fails its own recovery with
        // a typed error, logged here; it must not abort the engine build or
        // other workflows' recovery.
        let recovered = match recover_active_workflow(
            recovery,
            &workflow_id,
            &workflow_type,
            &history,
            catalog,
        ) {
            Ok(recovered) => {
                // Recovery succeeded, so a verdict recorded by an earlier sweep
                // in this same process (SS-5 adoption re-runs this loop) no
                // longer holds. A degraded flag that outlives the degradation
                // sends an operator to a redeploy for a healthy run.
                registry.unrecoverable().clear(&workflow_id)?;
                recovered
            }
            Err(error) => {
                // #117: retain the verdict as typed data, not only as this log
                // line. A run skipped here never becomes resident, so it can
                // never obtain a Recorder and can never be cancelled through
                // the sanctioned path — and until this was retained, the only
                // record of WHY was one ERROR in a boot stream that cannot be
                // queried afterwards. The operator who needs it is exactly the
                // one who was not watching stdout at boot.
                let reason = error.to_string();
                tracing::error!(
                    workflow_id = %workflow_id,
                    workflow_type = %workflow_type,
                    error = %error,
                    "active workflow failed startup recovery; skipping it and continuing"
                );
                registry.unrecoverable().record(
                    workflow_id.clone(),
                    crate::registry::UnrecoverableRun {
                        workflow_type: workflow_type.clone(),
                        reason,
                        observed_at: Utc::now(),
                    },
                )?;
                continue;
            }
        };
        let history_head = history.last().map(Event::seq).unwrap_or_default();
        match recovered {
            ActiveWorkflowRecovery::Resident {
                run_id,
                loaded_version,
                pid,
            } => {
                register_recovered_resident(
                    context,
                    RecoveredResident {
                        workflow_id: &workflow_id,
                        workflow_type: &workflow_type,
                        history: &history,
                        history_head,
                        projected_status,
                        run_id,
                        loaded_version,
                        pid,
                        // Startup recovery builds its own recorder at the head.
                        recorder: None,
                    },
                )
                .await?;
            }
            ActiveWorkflowRecovery::ScheduleCoordinator { run_id } => {
                registry.reconcile(&workflow_id, &run_id, &history)?;
            }
        }
    }

    Ok(())
}

/// Whether history ends with at least one dispatched activity attempt that has
/// no matching terminal record. These are the remote-agent executions recovery
/// must offer to the adoption path for both Running and Paused workflows (#36).
fn history_needs_activity_adoption(history: &[Event]) -> bool {
    let mut dangling = HashSet::new();
    for event in history {
        match event {
            Event::ActivityStarted {
                activity_id,
                attempt,
                ..
            } => {
                dangling.insert((activity_id.clone(), *attempt));
            }
            Event::ActivityFailed {
                activity_id,
                attempt,
                ..
            }
            | Event::ActivityCompleted {
                activity_id,
                attempt,
                ..
            }
            | Event::ActivityCancelled {
                activity_id,
                attempt,
                ..
            } => {
                dangling.remove(&(activity_id.clone(), *attempt));
            }
            _ => {}
        }
    }
    !dangling.is_empty()
}

/// One resident workflow recovered by the AD seam, ready for registration.
pub(crate) struct RecoveredResident<'a> {
    pub(crate) workflow_id: &'a aion_core::WorkflowId,
    pub(crate) workflow_type: &'a str,
    pub(crate) history: &'a [Event],
    pub(crate) history_head: u64,
    pub(crate) projected_status: WorkflowStatus,
    pub(crate) run_id: RunId,
    pub(crate) loaded_version: aion_package::ContentHash,
    pub(crate) pid: crate::Pid,
    /// The single continuous recorder to register this resident with.
    ///
    /// Startup recovery passes `None` and this flow builds a fresh
    /// `Recorder::resume_at(head)`. The reopen operation passes `Some(recorder)`
    /// — the very recorder that already appended `WorkflowReopened` — so exactly
    /// one recorder spans the reopen append through the respawn (invariant #3);
    /// no second writer is ever constructed for the reopened run.
    pub(crate) recorder: Option<Recorder>,
}

/// Register one recovered resident process: recorder, registry, supervision,
/// completion monitor, and the recorded-children crash-window sweep.
pub(crate) async fn register_recovered_resident(
    context: &StartupRecoveryContext,
    resident: RecoveredResident<'_>,
) -> Result<(), EngineError> {
    register_recovered_resident_with_reconcile(
        context,
        resident,
        |registry, workflow_id, run_id, history| {
            registry.reconcile(workflow_id, run_id, history).map(|_| ())
        },
    )
    .await
}

async fn register_recovered_resident_with_reconcile<F>(
    context: &StartupRecoveryContext,
    resident: RecoveredResident<'_>,
    reconcile: F,
) -> Result<(), EngineError>
where
    F: FnOnce(&Registry, &aion_core::WorkflowId, &RunId, &[Event]) -> Result<(), EngineError>,
{
    let RecoveredResident {
        workflow_id,
        workflow_type,
        history,
        history_head,
        projected_status,
        run_id,
        loaded_version,
        pid,
        recorder,
    } = resident;
    let recorder = recorder.unwrap_or_else(|| {
        Recorder::resume_at(
            workflow_id.clone(),
            Arc::clone(&context.store),
            history_head,
        )
        .with_visibility(run_id.clone(), Arc::clone(&context.visibility_store))
    });
    let completion = CompletionNotifier::new();
    let namespace = namespace_from_history(history);
    let handle = WorkflowHandle::new(WorkflowHandleParts {
        workflow_id: workflow_id.clone(),
        run_id: run_id.clone(),
        pid,
        workflow_type: workflow_type.to_owned(),
        namespace,
        loaded_version,
        cached_status: projected_status,
        residency: HandleResidency::Resident,
        recorder,
        completion,
    });
    if let Err(error) = context
        .registry
        .insert((workflow_id.clone(), run_id.clone()), handle.clone())
        .map(|_| ())
    {
        return Err(rollback_unmonitored_recovered_resident(
            context,
            workflow_id,
            &run_id,
            pid,
            error,
        ));
    }
    let registration = reconcile(&context.registry, workflow_id, &run_id, history).and_then(|()| {
        context
            .supervision
            .place_workflow(workflow_type.to_owned(), pid)
            .map(|_| ())
    });
    if let Err(error) = registration {
        return Err(rollback_unmonitored_recovered_resident(
            context,
            workflow_id,
            &run_id,
            pid,
            error,
        ));
    }
    if let Err(error) = install_recovered_completion_monitor(
        RecoveredMonitorParts {
            store: Arc::clone(&context.store),
            visibility_store: Arc::clone(&context.visibility_store),
            runtime: Arc::clone(&context.runtime),
            registry: Arc::clone(&context.registry),
            catalog: Arc::clone(&context.catalog),
            supervision: Arc::clone(&context.supervision),
            search_attribute_schema: Arc::clone(&context.search_attribute_schema),
        },
        &handle,
    ) {
        rollback_recovered_registry_entry(&context.registry, workflow_id, &run_id, &error);
        return Err(error);
    }
    sweep_recorded_children(context, workflow_id, &run_id, history).await
}

struct RecoveredMonitorParts {
    store: Arc<dyn EventStore>,
    visibility_store: Arc<dyn VisibilityStore>,
    runtime: Arc<RuntimeHandle>,
    registry: Arc<Registry>,
    catalog: Arc<WorkflowCatalog>,
    supervision: Arc<SupervisionTree>,
    search_attribute_schema: Arc<SearchAttributeSchema>,
}

fn install_recovered_completion_monitor(
    parts: RecoveredMonitorParts,
    handle: &WorkflowHandle,
) -> Result<(), EngineError> {
    let pid = handle.pid();
    let runtime = Arc::clone(&parts.runtime);
    let completion_context = ProcessExitContext {
        store: parts.store,
        visibility_store: parts.visibility_store,
        registry: parts.registry,
        catalog: parts.catalog,
        runtime: parts.runtime,
        supervision: parts.supervision,
        tokio_handle: tokio::runtime::Handle::current(),
        search_attribute_schema: parts.search_attribute_schema,
    };
    let completion_handle = handle.clone();
    runtime.monitor_process(pid, move |outcome| {
        if let Err(error) = handle_process_exit(completion_context, completion_handle, outcome) {
            tracing::error!(workflow_pid = pid, error = %error, "recovered workflow process monitor completion failed");
        }
    })?;
    Ok(())
}

fn rollback_unmonitored_recovered_resident(
    context: &StartupRecoveryContext,
    workflow_id: &aion_core::WorkflowId,
    run_id: &RunId,
    pid: crate::Pid,
    cause: EngineError,
) -> EngineError {
    rollback_recovered_registry_entry(&context.registry, workflow_id, run_id, &cause);
    match context.runtime.abort_unmonitored_process(pid) {
        Ok(()) => cause,
        Err(abort_error) => {
            tracing::error!(workflow_id = %workflow_id, pid, error = %abort_error, cause = %cause, "bounded recovered workflow abort failed after registration error");
            abort_error.into_engine_error()
        }
    }
}

fn rollback_recovered_registry_entry(
    registry: &Registry,
    workflow_id: &aion_core::WorkflowId,
    run_id: &RunId,
    cause: &EngineError,
) {
    if let Err(error) = registry.remove(workflow_id, run_id) {
        tracing::warn!(workflow_id = %workflow_id, error = %error, cause = %cause, "failed to roll back recovered workflow registry entry");
    }
}

pub(crate) fn recover_active_workflow(
    recovery: &dyn ActiveWorkflowRecoverySeam,
    workflow_id: &aion_core::WorkflowId,
    workflow_type: &str,
    history: &[Event],
    catalog: &WorkflowCatalog,
) -> Result<ActiveWorkflowRecovery, EngineError> {
    if workflow_id == &schedule_coordinator_workflow_id()
        && workflow_type == schedule_coordinator_workflow_type()
    {
        let run_id = started_run_id(workflow_id, history)?;
        return Ok(ActiveWorkflowRecovery::ScheduleCoordinator { run_id });
    }

    recovery.recover_active_workflow(workflow_id, workflow_type, history, catalog)
}

/// Extract the namespace from the workflow's `SearchAttributesUpdated`
/// event. The `aion.namespace` attribute is set at workflow start and
/// carried through child inheritance. Falls back to `"default"` when
/// no namespace attribute is recorded (pre-namespace workflows).
pub(crate) fn namespace_from_history(history: &[Event]) -> String {
    for event in history {
        if let Event::SearchAttributesUpdated { attributes, .. } = event {
            if let Some(aion_core::SearchAttributeValue::String(ns)) =
                attributes.get("aion.namespace")
            {
                return ns.clone();
            }
        }
    }
    String::from("default")
}

fn started_workflow_type(
    workflow_id: &aion_core::WorkflowId,
    history: &[Event],
) -> Result<String, EngineError> {
    if let Some(workflow_type) = history.iter().find_map(|event| match event {
        Event::WorkflowStarted { workflow_type, .. } => Some(workflow_type.clone()),
        _ => None,
    }) {
        return Ok(workflow_type);
    }

    if workflow_id == &schedule_coordinator_workflow_id() {
        return Ok(schedule_coordinator_workflow_type().to_owned());
    }

    Err(EngineError::Load {
        reason: format!(
            "active workflow `{workflow_id}` has no WorkflowStarted event in durable history"
        ),
    })
}

fn started_run_id(
    workflow_id: &aion_core::WorkflowId,
    history: &[Event],
) -> Result<RunId, EngineError> {
    if let Some(run_id) = history.iter().find_map(|event| match event {
        Event::WorkflowStarted { run_id, .. } => Some(run_id.clone()),
        _ => None,
    }) {
        return Ok(run_id);
    }

    if workflow_id == &schedule_coordinator_workflow_id() {
        return Ok(schedule_coordinator_run_id());
    }

    Err(EngineError::Load {
        reason: format!(
            "active workflow `{workflow_id}` has no WorkflowStarted run id in durable history"
        ),
    })
}

#[cfg(test)]
#[path = "startup_tests.rs"]
mod tests;
