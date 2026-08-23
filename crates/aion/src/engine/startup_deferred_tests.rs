//! Red-first specimens for the staged deferred-recovery slot: the
//! workflows-before-catchup ordering refusal, the one-shot discipline of
//! each staged entry point, and the composed entry's unchanged contract.

use std::sync::Arc;

use aion_store::{EventStore, InMemoryStore};

use crate::{Engine, EngineBuilder, EngineError};

type TestResult = Result<(), Box<dyn std::error::Error>>;

async fn deferred_engine() -> Result<Engine, EngineError> {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    EngineBuilder::new()
        .store_arc(store)
        .in_memory_visibility()
        .scheduler_threads(1)
        .defer_startup_recovery()
        .build()
        .await
}

/// Red first: catch-up before the workflow-recovery leg is refused with the
/// ordering error, and the refusal leaves the slot intact — the staged pair
/// still runs to completion afterwards.
#[tokio::test]
async fn catchup_before_workflow_recovery_refuses_and_leaves_the_slot_intact() -> TestResult {
    let engine = deferred_engine().await?;
    match engine.run_startup_catchup().await {
        Err(EngineError::StartupCatchupBeforeWorkflowRecovery) => {}
        other => {
            engine.shutdown()?;
            return Err(format!("expected the ordering refusal, got {other:?}").into());
        }
    }
    engine.recover_workflows_on_startup().await?;
    engine.run_startup_catchup().await?;
    engine.shutdown()?;
    Ok(())
}

/// Each staged entry point is one-shot: a second call to either leg after
/// the pair has run answers `StartupRecoveryAlreadyRan`.
#[tokio::test]
async fn each_staged_leg_is_one_shot() -> TestResult {
    let engine = deferred_engine().await?;
    engine.recover_workflows_on_startup().await?;
    match engine.recover_workflows_on_startup().await {
        Err(EngineError::StartupRecoveryAlreadyRan) => {}
        other => {
            engine.shutdown()?;
            return Err(format!("second workflows leg must refuse, got {other:?}").into());
        }
    }
    engine.run_startup_catchup().await?;
    match engine.run_startup_catchup().await {
        Err(EngineError::StartupRecoveryAlreadyRan) => {}
        other => {
            engine.shutdown()?;
            return Err(format!("second catch-up must refuse, got {other:?}").into());
        }
    }
    engine.shutdown()?;
    Ok(())
}

/// A non-deferred build owes nothing: both staged entry points refuse with
/// `StartupRecoveryNotDeferred`, repeatably.
#[tokio::test]
async fn staged_legs_refuse_on_a_non_deferred_build() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let engine = EngineBuilder::new()
        .store_arc(store)
        .in_memory_visibility()
        .scheduler_threads(1)
        .build()
        .await?;
    for _round in 0..2 {
        match engine.recover_workflows_on_startup().await {
            Err(EngineError::StartupRecoveryNotDeferred) => {}
            other => {
                engine.shutdown()?;
                return Err(format!("workflows leg must refuse, got {other:?}").into());
            }
        }
        match engine.run_startup_catchup().await {
            Err(EngineError::StartupRecoveryNotDeferred) => {}
            other => {
                engine.shutdown()?;
                return Err(format!("catch-up must refuse, got {other:?}").into());
            }
        }
    }
    engine.shutdown()?;
    Ok(())
}

/// The composed entry keeps its contract: one call runs both legs, a second
/// call refuses as already-ran.
#[tokio::test]
async fn composed_run_startup_recovery_still_runs_whole_and_once() -> TestResult {
    let engine = deferred_engine().await?;
    engine.run_startup_recovery().await?;
    match engine.run_startup_recovery().await {
        Err(EngineError::StartupRecoveryAlreadyRan) => {}
        other => {
            engine.shutdown()?;
            return Err(format!("second composed run must refuse, got {other:?}").into());
        }
    }
    engine.shutdown()?;
    Ok(())
}
