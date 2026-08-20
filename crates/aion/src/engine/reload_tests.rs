//! Unload pin-check tests, built from REAL compiled AWL packages on a real
//! engine and driven through the REAL completion path.
//!
//! The window under test is the one issue #51 named: `handle_process_exit_attempt`
//! appends the terminal, rings the completion doorbell — which releases
//! `Engine::result` — and only afterwards reconciles the registry, so a
//! still-registered handle caches `Running` over a durably terminal history.
//!
//! #51 was fixed at the `reopen` caller and closed. `verify_unload_member_unpinned`
//! reads the same stale value and was never touched, which is what
//! `deploy_api_e2e$http_unload_refusals_success_and_manifest_mismatch` was
//! reporting when it failed `left: 409, right: 200` (aion#94).
//!
//! 🔴 The window is held OPEN with `terminal_reconcile_gate`, never raced. #51's
//! own acceptance shape demanded exactly that — "pin the window deterministically
//! …, not with load" — and 90 executions under CPU load up to 3x the load the
//! original failure sat at reproduced nothing, because contention is not what
//! opens this window.

use std::collections::HashMap;
use std::sync::Arc;

use aion_core::{ContentType, Payload, WorkflowStatus, status_from_events};
use aion_package::{ContentHash, ExtractionLimits, Package};

use crate::engine::api::Engine;
use crate::lifecycle::terminal_reconcile_gate;
use crate::signal::ConcreteSignalRouter;
use crate::{EngineBuilder, RuntimeHandle, SignalRouter};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Parks on a signal, then completes with no action at all — so the run reaches
/// a terminal with no worker in the picture and nothing to time out on.
const PIN_V1: &str = r"//! Unload pin-window fixture, first deploy.
workflow unload_pin_window
  input tag: String
  signal go: Ruling
  outcome completed: type Result, route success

type Result { tag: String }
type Ruling { proceed: Bool }

step hold
  wait go -> decision

step finish
  route completed(tag: tag)
";

/// The same workflow type, second deploy. Loading it moves the route off v1, so
/// v1 is unload-eligible except for whatever pin check says otherwise — which is
/// the only thing this test wants to exercise.
const PIN_V2: &str = r"//! Unload pin-window fixture, second deploy.
workflow unload_pin_window
  input tag: String
  input note: String
  signal go: Ruling
  outcome completed: type Result, route success

type Result { tag: String }
type Ruling { proceed: Bool }

step hold
  wait go -> decision

step finish
  route completed(tag: tag)
";

async fn engine() -> TestResult<Engine> {
    // A signal seam is REQUIRED here, not incidental: without it `signal` fails
    // with "signal routing seam is not configured" and the test never reaches
    // the window at all — a red for the wrong reason, which proves nothing about
    // the defect. The first run of this test failed exactly that way.
    Ok(EngineBuilder::new()
        .in_process_activity_serving()
        .store(aion_store::InMemoryStore::default())
        .in_memory_visibility()
        .scheduler_threads(1)
        .signal_router_factory(|runtime: Arc<RuntimeHandle>, handoff| {
            Arc::new(ConcreteSignalRouter::new(runtime, handoff)) as Arc<dyn SignalRouter>
        })
        .build()
        .await?)
}

async fn deploy(engine: &Engine, source: &str, file: &str) -> TestResult<ContentHash> {
    let root = tempfile::tempdir()?;
    let prepared = aion_awl_package::compile_and_assemble_awl(source, root.path(), file)?;
    let package = Package::load_from_bytes(prepared.archive, ExtractionLimits::unbounded())?;
    let version = package.content_hash().clone();
    engine.load_package(package).await?;
    Ok(version)
}

/// THE DEFECT (aion#94). An unload issued after the run's result is observable
/// must succeed: history is terminal, so nothing is live to pin the version. The
/// pin check consults `handle.cached_status()`, which has not been reconciled
/// yet, and refuses a legitimate unload as `VersionPinned { LiveRun }`.
#[tokio::test(flavor = "multi_thread")]
async fn unload_after_result_before_terminal_registry_reconcile_succeeds() -> TestResult {
    let engine = engine().await?;
    let v1 = deploy(&engine, PIN_V1, "pin_v1.awl").await?;

    let handle = engine
        .start_workflow(
            "unload_pin_window",
            Payload::new(ContentType::Json, br#"{"tag":"pin"}"#.to_vec()),
            HashMap::new(),
            "default".to_owned(),
        )
        .await?;
    assert_eq!(handle.loaded_version(), &v1);
    let workflow_id = handle.workflow_id().clone();
    let run_id = handle.run_id().clone();

    // Move the route off v1 so RouteActive cannot be the refusal under test.
    let v2 = deploy(&engine, PIN_V2, "pin_v2.awl").await?;
    assert_ne!(v1, v2);

    // Arm BEFORE the terminal so the completion path stops inside the window
    // rather than being caught in it by luck.
    let gate = terminal_reconcile_gate::arm(&workflow_id, &run_id);
    assert!(
        !gate.was_reached(),
        "the gate starts before the terminal path"
    );

    engine
        .signal(
            &workflow_id,
            &run_id,
            "go",
            Payload::new(ContentType::Json, br#"{"proceed":true}"#.to_vec()),
        )
        .await?;

    gate.wait_until_reached().await;

    // The window, asserted rather than assumed: history terminal, handle stale.
    let history = engine.store().read_history(&workflow_id).await?;
    assert_eq!(
        status_from_events(&history),
        WorkflowStatus::Completed,
        "the terminal is durable before the doorbell releases the result"
    );
    let stale = engine
        .registry()
        .get(&workflow_id, &run_id)?
        .ok_or("the handle disappeared before the unload")?;
    assert_eq!(
        stale.cached_status(),
        WorkflowStatus::Running,
        "this test is only meaningful inside the un-reconciled window"
    );

    let unloaded = engine
        .unload_workflow_version("unload_pin_window", &v1)
        .await;
    drop(gate);

    if let Err(error) = unloaded {
        engine.shutdown()?;
        return Err(format!(
            "unload must succeed while history is terminal and the lagging handle caches Running; got {error}"
        )
        .into());
    }
    engine.shutdown()?;
    Ok(())
}

/// The check keeps its teeth. A genuinely live run — parked on its signal, never
/// released — must still pin its version against unload. Without this, "fixing"
/// the window by deleting the handle check would pass the test above while
/// destroying the guarantee it exists for.
#[tokio::test(flavor = "multi_thread")]
async fn unload_still_refuses_while_a_run_is_genuinely_live() -> TestResult {
    let engine = engine().await?;
    let v1 = deploy(&engine, PIN_V1, "pin_v1.awl").await?;

    let handle = engine
        .start_workflow(
            "unload_pin_window",
            Payload::new(ContentType::Json, br#"{"tag":"parked"}"#.to_vec()),
            HashMap::new(),
            "default".to_owned(),
        )
        .await?;
    assert_eq!(handle.loaded_version(), &v1);

    let v2 = deploy(&engine, PIN_V2, "pin_v2.awl").await?;
    assert_ne!(v1, v2);

    // Never signalled: the run is parked on `wait go`, non-terminal in history,
    // so the refusal is correct and must survive the fix.
    let refused = engine
        .unload_workflow_version("unload_pin_window", &v1)
        .await;
    let error = refused
        .err()
        .ok_or("unload must refuse while a run is live")?;
    assert!(
        format!("{error}").contains("pinned")
            || matches!(error, crate::EngineError::VersionPinned { .. }),
        "the refusal must be the version pin, not something incidental: {error}"
    );
    engine.shutdown()?;
    Ok(())
}
