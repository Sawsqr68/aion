//! The never-alive cancellation path (#117(c)), from both sides of every door.

use std::sync::Arc;

use aion_core::{Event, PackageVersion, Payload, RunId, WorkflowId, WorkflowStatus};
use aion_package::ContentHash;
use aion_store::visibility::VisibilityStore;
use aion_store::{EventStore, InMemoryStore};
use chrono::Utc;
use serde_json::json;

use crate::EngineError;
use crate::durability::{Recorder, WorkflowStartRecord};
use crate::loader::WorkflowCatalog;
use crate::registry::{Registry, UnrecoverableRun};
use crate::runtime::{RuntimeConfig, RuntimeHandle};

use crate::lifecycle::terminate::{TerminateWorkflowContext, cancel};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const WORKFLOW_TYPE: &str = "rig";
const PINNED: u8 = 0x0c;

/// A run that exists in the store, is projected `Running`, and holds NO handle —
/// the exact shape startup recovery leaves behind when a run's pinned package
/// cannot be loaded.
struct NeverAlive {
    store: Arc<dyn EventStore>,
    visibility_store: Arc<dyn VisibilityStore>,
    registry: Registry,
    runtime: RuntimeHandle,
    catalog: WorkflowCatalog,
    workflow_id: WorkflowId,
    run_id: RunId,
}

impl NeverAlive {
    fn context(&self) -> TerminateWorkflowContext<'_> {
        TerminateWorkflowContext {
            runtime: &self.runtime,
            store: Arc::clone(&self.store),
            visibility_store: Arc::clone(&self.visibility_store),
            registry: &self.registry,
            catalog: &self.catalog,
        }
    }

    /// Records the boot-time verdict startup recovery would have recorded.
    fn record_verdict(&self) -> Result<(), EngineError> {
        self.registry.unrecoverable().record(
            self.workflow_id.clone(),
            UnrecoverableRun {
                workflow_type: WORKFLOW_TYPE.to_owned(),
                reason: format!(
                    "active workflow is pinned to package version `{}`, which is not loaded",
                    pinned_version()
                ),
                observed_at: Utc::now(),
            },
        )
    }

    /// Loads the pinned version into the catalog — a redeploy landing.
    fn deploy_the_pinned_version(&self) {
        self.catalog.note_loaded_workflow_for_test(
            WORKFLOW_TYPE,
            "rig$deployed",
            "run",
            pinned_version(),
        );
    }

    async fn history(&self) -> Result<Vec<Event>, Box<dyn std::error::Error>> {
        Ok(self.store.read_history(&self.workflow_id).await?)
    }
}

fn pinned_version() -> ContentHash {
    ContentHash::from_bytes([PINNED; 32])
}

async fn never_alive_run() -> TestResult<NeverAlive> {
    let backing = Arc::new(InMemoryStore::default());
    let store: Arc<dyn EventStore> = Arc::clone(&backing) as Arc<dyn EventStore>;
    let visibility_store: Arc<dyn VisibilityStore> = backing;
    let workflow_id = WorkflowId::new_v4();
    let run_id = RunId::new_v4();

    // Seeded through a Recorder, not by hand-building events: the run's history
    // must be the shape the engine actually writes, or the path under test is
    // reading a fixture rather than a run.
    let mut recorder = Recorder::new(workflow_id.clone(), Arc::clone(&store));
    recorder
        .record_workflow_started(
            Utc::now(),
            WorkflowStartRecord {
                workflow_type: WORKFLOW_TYPE.to_owned(),
                input: Payload::from_json(&json!({ "label": "input" }))?,
                run_id: run_id.clone(),
                parent_run_id: None,
                parent_workflow_id: None,
                package_version: PackageVersion::new(pinned_version().to_string()),
            },
        )
        .await?;

    Ok(NeverAlive {
        store,
        visibility_store,
        registry: Registry::default(),
        runtime: RuntimeHandle::new(RuntimeConfig::new(Some(1)))?,
        // Empty: the pinned version is NOT loaded, which is the whole condition.
        catalog: WorkflowCatalog::new(),
        workflow_id,
        run_id,
    })
}

fn cancelled_in(history: &[Event]) -> bool {
    history
        .iter()
        .any(|event| matches!(event, Event::WorkflowCancelled { .. }))
}

/// The case #117 was raised for. Before this path existed, `cancel` answered
/// `WorkflowNotFound` for a run whose full history the read plane was happily
/// serving, and the run stayed `Running` with no operator lever at all.
#[tokio::test]
async fn a_run_that_was_never_alive_can_be_cancelled() -> TestResult {
    let run = never_alive_run().await?;
    run.record_verdict()?;

    // Non-vacuity: the run really is `Running` and really holds no handle, so
    // the cancellation below is exercising the never-alive branch and not some
    // resident one.
    assert_eq!(
        aion_core::status_from_events(&run.history().await?),
        WorkflowStatus::Running
    );
    assert_eq!(run.registry.live_pid(&run.workflow_id)?, None);

    cancel(
        run.context(),
        &run.workflow_id,
        &run.run_id,
        "operator cancelled an unrecoverable run",
    )
    .await?;

    let history = run.history().await?;
    assert!(
        cancelled_in(&history),
        "the cancellation must be DURABLE — an in-memory acknowledgement would \
         evaporate at the next boot and leave the run Running again"
    );
    assert_eq!(
        aion_core::status_from_events(&history),
        WorkflowStatus::Cancelled
    );

    // Requirement (1)'s other half: the reservation ended with the append, so
    // the workflow is not wedged against future writers.
    assert_eq!(run.registry.terminal_writer_run(&run.workflow_id)?, None);
    run.runtime.shutdown()?;
    Ok(())
}

/// Requirement (2), the STOP. The verdict is from an earlier boot; the catalog
/// is read NOW. A redeploy that landed in between makes the run recoverable, and
/// this path must refuse rather than take the extraordinary route past a working
/// ordinary one.
#[tokio::test]
async fn a_run_whose_pinned_package_now_loads_is_refused() -> TestResult {
    let run = never_alive_run().await?;
    run.record_verdict()?;
    run.deploy_the_pinned_version();

    let result = cancel(
        run.context(),
        &run.workflow_id,
        &run.run_id,
        "operator cancelled",
    )
    .await;

    assert!(
        matches!(result, Err(EngineError::RunIsRecoverable { .. })),
        "a run whose package resolves is recoverable, whatever an earlier boot recorded: {result:?}"
    );
    assert!(
        !cancelled_in(&run.history().await?),
        "a refused cancellation must append NOTHING"
    );
    assert_eq!(
        run.registry.terminal_writer_run(&run.workflow_id)?,
        None,
        "the reservation must be released on the refusal path too, or a STOP wedges the workflow"
    );
    run.runtime.shutdown()?;
    Ok(())
}

/// The control for the test above, and the discriminating half of the pair: the
/// ONLY difference between them is whether the catalog holds the pinned version.
/// Without this, a `cancel` that refused everything would pass that test.
#[tokio::test]
async fn the_same_run_is_cancellable_when_the_package_is_absent() -> TestResult {
    let run = never_alive_run().await?;
    run.record_verdict()?;

    cancel(
        run.context(),
        &run.workflow_id,
        &run.run_id,
        "operator cancelled",
    )
    .await?;

    assert!(cancelled_in(&run.history().await?));
    run.runtime.shutdown()?;
    Ok(())
}

/// Requirement (4). The run exists and its history is readable, so "not found"
/// would be a lie — but this engine has no verdict to cite, so the cancellation
/// is refused. The refusal names the true state.
#[tokio::test]
async fn a_run_with_no_recorded_verdict_is_refused_without_calling_it_missing() -> TestResult {
    let run = never_alive_run().await?;
    // Deliberately no `record_verdict`.

    let result = cancel(
        run.context(),
        &run.workflow_id,
        &run.run_id,
        "operator cancelled",
    )
    .await;

    match result {
        Err(EngineError::NoResidencyVerdict { .. }) => {}
        other => {
            return Err(format!("expected NoResidencyVerdict, got {other:?}").into());
        }
    }
    assert!(!cancelled_in(&run.history().await?));
    run.runtime.shutdown()?;
    Ok(())
}

/// Requirement (4) pointed the other way. A workflow id nothing was ever started
/// under really IS missing, and the honest answer for it is unchanged. Accuracy
/// is a demand in both directions: renaming this one would be the same
/// dishonesty as calling a live run missing.
#[tokio::test]
async fn a_workflow_that_never_existed_is_still_not_found() -> TestResult {
    let run = never_alive_run().await?;
    let unknown = WorkflowId::new_v4();

    let result = cancel(run.context(), &unknown, &run.run_id, "operator cancelled").await;

    assert!(
        matches!(result, Err(EngineError::WorkflowNotFound { .. })),
        "an id with no history at all is genuinely not found: {result:?}"
    );
    run.runtime.shutdown()?;
    Ok(())
}

/// A cancellation that names a run the history does not carry must not append a
/// `WorkflowCancelled` claiming to terminate it.
#[tokio::test]
async fn cancelling_a_run_the_history_does_not_carry_is_refused() -> TestResult {
    let run = never_alive_run().await?;
    run.record_verdict()?;
    let stale_run = RunId::new_v4();

    let result = cancel(
        run.context(),
        &run.workflow_id,
        &stale_run,
        "operator cancelled",
    )
    .await;

    assert!(
        matches!(result, Err(EngineError::InvalidState { .. })),
        "expected a refusal naming the run mismatch, got {result:?}"
    );
    assert!(!cancelled_in(&run.history().await?));
    run.runtime.shutdown()?;
    Ok(())
}

/// Requirement (3): exactly one terminal append. The second attempt meets a run
/// that already recorded a terminal and is refused — the same guard every other
/// terminal writer passes through, reached here through the reservation.
#[tokio::test]
async fn a_second_cancellation_of_the_same_run_is_refused() -> TestResult {
    let run = never_alive_run().await?;
    run.record_verdict()?;

    cancel(run.context(), &run.workflow_id, &run.run_id, "first").await?;
    let second = cancel(run.context(), &run.workflow_id, &run.run_id, "second").await;

    assert!(
        matches!(second, Err(EngineError::Runtime { .. })),
        "expected a refusal on the already-terminal run, got {second:?}"
    );
    let terminals = run
        .history()
        .await?
        .iter()
        .filter(|event| matches!(event, Event::WorkflowCancelled { .. }))
        .count();
    assert_eq!(terminals, 1, "exactly one terminal append, not two");
    run.runtime.shutdown()?;
    Ok(())
}

/// A resident run is untouched by any of this: it still takes the ordinary path,
/// which is what the never-alive branch is careful never to become.
#[tokio::test]
async fn a_workflow_holding_a_handle_never_reaches_the_never_alive_branch() -> TestResult {
    let run = never_alive_run().await?;
    run.record_verdict()?;

    // A handle for a DIFFERENT run of this workflow. The ordinary path will not
    // find a handle for the run being cancelled, so control reaches the
    // never-alive branch — where the registry refuses, because the workflow
    // already has a writer.
    let other_run = RunId::new_v4();
    run.registry.insert(
        (run.workflow_id.clone(), other_run.clone()),
        crate::registry::WorkflowHandle::new(crate::registry::WorkflowHandleParts {
            workflow_id: run.workflow_id.clone(),
            run_id: other_run,
            pid: 1,
            workflow_type: WORKFLOW_TYPE.to_owned(),
            namespace: String::from("default"),
            loaded_version: pinned_version(),
            cached_status: WorkflowStatus::Running,
            residency: crate::registry::HandleResidency::Resident,
            recorder: Recorder::new(run.workflow_id.clone(), Arc::clone(&run.store)),
            completion: crate::registry::CompletionNotifier::new(),
        }),
    )?;

    let result = cancel(
        run.context(),
        &run.workflow_id,
        &run.run_id,
        "operator cancelled",
    )
    .await;

    assert!(
        matches!(result, Err(EngineError::TerminalWriterUnavailable { .. })),
        "a workflow with a live handle must not get a second writer: {result:?}"
    );
    assert!(!cancelled_in(&run.history().await?));
    run.runtime.shutdown()?;
    Ok(())
}
