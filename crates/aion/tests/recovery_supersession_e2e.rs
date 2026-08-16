//! End-to-end tests for the #266 Defect-B supersession record: an attempt
//! left in flight by a dead server gets an honest, non-terminal
//! `ActivityFailed{attempt, reason: server-death}` recorded by the recovery
//! re-dispatch BEFORE the next attempt starts — closing the forever-dangling
//! attempt trail that made an orphaned run read as silently still working.
//!
//! Ruled semantics (honesty-only, ratified 2026-08-17): the record is an
//! HONESTY record, not a budget event. Re-dispatch proceeds unconditionally
//! — infrastructure death is attempt-neutral at the budget layer, exactly as
//! the live retry loop's TRANSPORT-domain worker-loss arm already treats a
//! worker dying mid-attempt. Terminality by retry policy governs
//! action-authored failures only. The record must be BUDGET-INERT: writing
//! it consumes nothing beyond the pre-existing attempt-number inflation
//! (which is its own ticketed design question, untouched here).
//!
//! The corpse in every test is produced without killing anything: the parked
//! sentinel (`parked:server-draining`) stands the retry loop down with
//! NOTHING recorded and NOTHING delivered, leaving the durable log at the
//! dangling scheduled/started trail — byte-equivalent to a kill -9 (#207's
//! documented equivalence). Engine shutdown then plays the death; a second
//! engine over the same store plays the recovering server.

#[path = "test_support/gleam.rs"]
mod gleam_test_support;

#[path = "common/example_build.rs"]
mod example_build;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use aion::EngineBuilder;
use aion::activity::bridge::{ActivityDispatch, ActivityDispatcher};
use aion_core::{ActivityErrorKind, Event, Payload, WorkflowStatus};
use aion_store::{EventStore, InMemoryStore};
use serde_json::json;

/// The exact recorded reason the supersession record must carry — asserted
/// as a literal so a constant rename can never silently change the recorded
/// bytes.
const SERVER_DEATH_REASON: &str = "superseded:server-death";

/// Scripted per-delivery dispatcher: each delivery consumes the next step;
/// deliveries past the script's end succeed. `Park` answers the parked
/// sentinel (loop stands down, nothing recorded — the corpse maker),
/// `FailRetryable` answers a transient wire failure, `Succeed` replies
/// normally.
#[derive(Clone, Copy)]
enum Step {
    Park,
    FailRetryable,
    Succeed,
}

struct ScriptedDispatcher {
    script: Vec<Step>,
    calls: std::sync::Mutex<Vec<u32>>,
    deliveries: AtomicU32,
}

impl ScriptedDispatcher {
    fn new(script: Vec<Step>) -> Arc<Self> {
        Arc::new(Self {
            script,
            calls: std::sync::Mutex::new(Vec::new()),
            deliveries: AtomicU32::new(0),
        })
    }

    fn seen_attempts(&self) -> Vec<u32> {
        self.calls
            .lock()
            .map(|calls| calls.clone())
            .unwrap_or_default()
    }
}

impl ActivityDispatcher for ScriptedDispatcher {
    fn dispatch(&self, request: ActivityDispatch) -> Result<String, String> {
        if request.name != "flaky_call" {
            return Err(format!("terminal:unknown activity {}", request.name));
        }
        let delivery = self.deliveries.fetch_add(1, Ordering::SeqCst) as usize;
        self.calls
            .lock()
            .map_err(|_| "terminal:calls lock poisoned".to_owned())?
            .push(request.attempt);
        match self.script.get(delivery).copied().unwrap_or(Step::Succeed) {
            Step::Park => Err(aion::PARKED_ACTIVITY_REASON.to_owned()),
            Step::FailRetryable => Err(format!(
                "retryable:upstream stream reset (delivery attempt {})",
                request.attempt
            )),
            Step::Succeed => {
                let value: serde_json::Value = serde_json::from_str(&request.input)
                    .map_err(|error| format!("terminal:bad input: {error}"))?;
                let who = value["name"].as_str().unwrap_or("stranger");
                Ok(json!({ "reply": format!("steady hello, {who}") }).to_string())
            }
        }
    }
}

async fn engine_with(
    dispatcher: Arc<dyn ActivityDispatcher>,
    store: &Arc<dyn EventStore>,
) -> Result<aion::Engine, Box<dyn std::error::Error>> {
    let package = example_build::built_package("examples/retry-policy", "retry_policy")?;
    Ok(EngineBuilder::new()
        .in_process_activity_serving()
        .store_arc(Arc::clone(store))
        .in_memory_visibility()
        .scheduler_threads(1)
        .activity_dispatcher(dispatcher)
        .load_workflows(package)
        .build()
        .await?)
}

/// Poll the durable history until `predicate` holds or the bounded wait
/// expires — the corpse must actually exist before the death is played.
async fn await_history(
    store: &Arc<dyn EventStore>,
    workflow_id: &aion_core::WorkflowId,
    what: &str,
    predicate: impl Fn(&[Event]) -> bool,
) -> Result<(), Box<dyn std::error::Error>> {
    for _ in 0_u32..200 {
        let history = store.read_history(workflow_id).await?;
        if predicate(&history) {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let history = store.read_history(workflow_id).await?;
    Err(format!("timed out waiting for {what}: {history:#?}").into())
}

fn started_attempts(history: &[Event]) -> Vec<u32> {
    history
        .iter()
        .filter_map(|event| match event {
            Event::ActivityStarted { attempt, .. } => Some(*attempt),
            _ => None,
        })
        .collect()
}

/// Every supersession record in the trail: `(seq, attempt)` of each
/// non-terminal `ActivityFailed` carrying the server-death reason.
fn supersession_records(history: &[Event]) -> Vec<(u64, u32)> {
    history
        .iter()
        .filter_map(|event| match event {
            Event::ActivityFailed {
                envelope,
                error,
                attempt,
                ..
            } if error.kind == ActivityErrorKind::Retryable
                && error.message == SERVER_DEATH_REASON =>
            {
                Some((envelope.seq, *attempt))
            }
            _ => None,
        })
        .collect()
}

fn seq_of_started(history: &[Event], wanted: u32) -> Option<u64> {
    history.iter().find_map(|event| match event {
        Event::ActivityStarted {
            envelope, attempt, ..
        } if *attempt == wanted => Some(envelope.seq),
        _ => None,
    })
}

/// THE DEFECT: a server that dies with attempt 1 in flight leaves the
/// attempt dangling forever — no terminal, no failure, nothing. The
/// recovering server's re-dispatch must first record the honest
/// `ActivityFailed{attempt: 1, Retryable, superseded:server-death}` and only
/// then start attempt 2. A fresh run on the recovered engine is the control:
/// it must carry NO supersession record.
#[tokio::test]
async fn recovery_records_the_superseded_attempt_before_redispatch()
-> Result<(), Box<dyn std::error::Error>> {
    if crate::gleam_test_support::skip_if_unavailable() {
        return Ok(());
    }
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());

    // Life 1: attempt 1 parks — the loop stands down recording nothing, the
    // durable log ends at the dangling Scheduled/Started{1} trail.
    let parked = ScriptedDispatcher::new(vec![Step::Park]);
    let engine = engine_with(Arc::clone(&parked) as Arc<dyn ActivityDispatcher>, &store).await?;
    let handle = engine
        .start_workflow(
            "retry_policy",
            Payload::from_json(&json!({ "name": "Ada" }))?,
            std::collections::HashMap::new(),
            String::from("default"),
        )
        .await?;
    await_history(&store, handle.workflow_id(), "the dangling Started{1}", |h| {
        started_attempts(h) == vec![1]
    })
    .await?;
    engine.shutdown()?;

    // Death check: the corpse's trail must end dangling — attempt 1 started,
    // nothing settled it, run still projects Running.
    let corpse = store.read_history(handle.workflow_id()).await?;
    assert_eq!(
        aion_core::status_from_events(&corpse),
        WorkflowStatus::Running,
        "the corpse must be an orphaned Running run: {corpse:#?}"
    );

    // Life 2: recovery replays, re-dispatches, and the run completes.
    let serving = ScriptedDispatcher::new(vec![Step::Succeed]);
    let recovered =
        engine_with(Arc::clone(&serving) as Arc<dyn ActivityDispatcher>, &store).await?;
    let result = recovered
        .result(handle.workflow_id(), handle.run_id())
        .await?;
    let payload = result.map_err(|error| format!("recovered run failed: {error:?}"))?;
    let reply: serde_json::Value = serde_json::from_slice(payload.bytes())?;
    assert_eq!(reply, json!("steady hello, Ada"));

    let history = store.read_history(handle.workflow_id()).await?;
    assert_eq!(
        started_attempts(&history),
        vec![1, 2],
        "the re-dispatch must continue the attempt trail: {history:#?}"
    );

    // The record itself — the defect's cure. Exactly one, for attempt 1,
    // sequenced BEFORE attempt 2's ActivityStarted.
    let records = supersession_records(&history);
    assert_eq!(
        records.len(),
        1,
        "the superseded attempt must get exactly one honest failure record: {history:#?}"
    );
    let (record_seq, record_attempt) = records[0];
    assert_eq!(
        record_attempt, 1,
        "the record names the DANGLING attempt, never the new one: {history:#?}"
    );
    let started_2 = seq_of_started(&history, 2)
        .ok_or_else(|| format!("attempt 2 must record its start: {history:#?}"))?;
    assert!(
        record_seq < started_2,
        "the supersession record ({record_seq}) must precede attempt 2's start \
         ({started_2}): the old attempt ends before the new one begins: {history:#?}"
    );

    // Control: a fresh run on the recovered engine carries no supersession
    // record — the fix must never spray records on fresh dispatches.
    let fresh = recovered
        .start_workflow(
            "retry_policy",
            Payload::from_json(&json!({ "name": "Grace" }))?,
            std::collections::HashMap::new(),
            String::from("default"),
        )
        .await?;
    let result = recovered.result(fresh.workflow_id(), fresh.run_id()).await?;
    let payload = result.map_err(|error| format!("fresh control run failed: {error:?}"))?;
    let reply: serde_json::Value = serde_json::from_slice(payload.bytes())?;
    assert_eq!(reply, json!("steady hello, Grace"));
    let fresh_history = store.read_history(fresh.workflow_id()).await?;
    assert!(
        supersession_records(&fresh_history).is_empty(),
        "a fresh ordinal must never receive a supersession record: {fresh_history:#?}"
    );

    recovered.shutdown()?;
    Ok(())
}

/// NO DOUBLE RECORD: an attempt whose failure is ALREADY recorded (the #197
/// crash-mid-backoff shape — a real retryable failure at attempt 1, then the
/// server dies with attempt 2 in flight) must not receive a second failure
/// record. Recovery writes the supersession record for the DANGLING attempt
/// (2) only; attempt 1 keeps exactly its one real failure.
#[tokio::test]
async fn an_already_failed_attempt_is_not_recorded_twice()
-> Result<(), Box<dyn std::error::Error>> {
    if crate::gleam_test_support::skip_if_unavailable() {
        return Ok(());
    }
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());

    // Life 1: attempt 1 fails retryably (recorded by the live loop), the
    // retry delivery of attempt 2 parks — dangling Started{2}.
    let script = ScriptedDispatcher::new(vec![Step::FailRetryable, Step::Park]);
    let engine = engine_with(Arc::clone(&script) as Arc<dyn ActivityDispatcher>, &store).await?;
    let handle = engine
        .start_workflow(
            "retry_policy",
            Payload::from_json(&json!({ "name": "Ada" }))?,
            std::collections::HashMap::new(),
            String::from("default"),
        )
        .await?;
    await_history(&store, handle.workflow_id(), "the dangling Started{2}", |h| {
        started_attempts(h) == vec![1, 2]
    })
    .await?;
    engine.shutdown()?;

    // Life 2: recovery completes the run at attempt 3.
    let serving = ScriptedDispatcher::new(vec![Step::Succeed]);
    let recovered =
        engine_with(Arc::clone(&serving) as Arc<dyn ActivityDispatcher>, &store).await?;
    let result = recovered
        .result(handle.workflow_id(), handle.run_id())
        .await?;
    let payload = result.map_err(|error| format!("recovered run failed: {error:?}"))?;
    let reply: serde_json::Value = serde_json::from_slice(payload.bytes())?;
    assert_eq!(reply, json!("steady hello, Ada"));

    let history = store.read_history(handle.workflow_id()).await?;
    assert_eq!(
        started_attempts(&history),
        vec![1, 2, 3],
        "the trail continues through both lives: {history:#?}"
    );
    let records = supersession_records(&history);
    assert_eq!(
        records.iter().map(|(_, attempt)| *attempt).collect::<Vec<_>>(),
        vec![2],
        "ONLY the dangling attempt gets a supersession record — attempt 1's \
         real failure already closed it: {history:#?}"
    );
    let attempt_1_failures = history
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::ActivityFailed { attempt: 1, .. }
            )
        })
        .count();
    assert_eq!(
        attempt_1_failures, 1,
        "attempt 1 keeps exactly its one real failure record: {history:#?}"
    );

    recovered.shutdown()?;
    Ok(())
}

/// BUDGET-INERT (the ruled term): writing the supersession record consumes
/// nothing beyond the pre-existing attempt-number inflation. Under the
/// example's 3-attempt policy, a run whose attempt 1 died with the server
/// still gets its remaining numbered attempts: a real retryable failure at
/// attempt 2 retries (2 < 3), and attempt 3 completes the run. A reader that
/// made the record consume budget would refuse that final attempt.
#[tokio::test]
async fn the_supersession_record_is_budget_inert() -> Result<(), Box<dyn std::error::Error>> {
    if crate::gleam_test_support::skip_if_unavailable() {
        return Ok(());
    }
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());

    let parked = ScriptedDispatcher::new(vec![Step::Park]);
    let engine = engine_with(Arc::clone(&parked) as Arc<dyn ActivityDispatcher>, &store).await?;
    let handle = engine
        .start_workflow(
            "retry_policy",
            Payload::from_json(&json!({ "name": "Ada" }))?,
            std::collections::HashMap::new(),
            String::from("default"),
        )
        .await?;
    await_history(&store, handle.workflow_id(), "the dangling Started{1}", |h| {
        started_attempts(h) == vec![1]
    })
    .await?;
    engine.shutdown()?;

    // Life 2: the re-dispatch (attempt 2) fails for real, retryably; the
    // policy's remaining number budget must carry it to attempt 3, which
    // succeeds.
    let serving = ScriptedDispatcher::new(vec![Step::FailRetryable, Step::Succeed]);
    let recovered =
        engine_with(Arc::clone(&serving) as Arc<dyn ActivityDispatcher>, &store).await?;
    let result = recovered
        .result(handle.workflow_id(), handle.run_id())
        .await?;
    let payload = result.map_err(|error| format!("recovered run failed: {error:?}"))?;
    let reply: serde_json::Value = serde_json::from_slice(payload.bytes())?;
    assert_eq!(reply, json!("steady hello, Ada"));

    let history = store.read_history(handle.workflow_id()).await?;
    assert_eq!(
        started_attempts(&history),
        vec![1, 2, 3],
        "the record must not eat the remaining numbered attempts: {history:#?}"
    );
    assert_eq!(
        supersession_records(&history)
            .iter()
            .map(|(_, attempt)| *attempt)
            .collect::<Vec<_>>(),
        vec![1],
        "one supersession record, for the one superseded attempt: {history:#?}"
    );
    assert!(
        history
            .iter()
            .any(|event| matches!(event, Event::ActivityCompleted { attempt: 3, .. })),
        "the final numbered attempt completes the run: {history:#?}"
    );
    assert!(
        seq_of_started(&history, 2).is_some(),
        "attempt 2's start is recorded: {history:#?}"
    );

    recovered.shutdown()?;
    Ok(())
}
