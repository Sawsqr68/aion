//! End-to-end tests for recovery adoption: an activity attempt left in flight
//! across an engine generation retains its identity, records an explicit
//! `ActivityAdoptionOffered`, and redelivers to the worker adoption path.
//!
//! The engine does not claim the worker found a holder; it truthfully records
//! the offer and leaves holder liveness to worker single-flight/spawn-gate
//! adoption. It never writes `superseded:server-death` for that dangling
//! execution, and the offer itself consumes no retry budget.
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

fn adoption_offer_records(history: &[Event]) -> Vec<(u64, u32)> {
    history
        .iter()
        .filter_map(|event| {
            let value = serde_json::to_value(event).ok()?;
            if value.get("type")?.as_str()? != "ActivityAdoptionOffered" {
                return None;
            }
            let data = value.get("data")?;
            Some((
                data.get("envelope")?.get("seq")?.as_u64()?,
                u32::try_from(data.get("attempt")?.as_u64()?).ok()?,
            ))
        })
        .collect()
}

async fn recovery_offers_dangling_attempt_for_adoption(
    paused: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if crate::gleam_test_support::skip_if_unavailable() {
        return Ok(());
    }
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());

    // Life 1 leaves attempt 1 dangling exactly as a remote agent still owned by
    // its worker does when only the server disappears. `Park` stands the old
    // completion task down without manufacturing a terminal record.
    let holder = ScriptedDispatcher::new(vec![Step::Park]);
    let engine = engine_with(Arc::clone(&holder) as Arc<dyn ActivityDispatcher>, &store).await?;
    let handle = engine
        .start_workflow(
            "retry_policy",
            Payload::from_json(&json!({ "name": "Ada" }))?,
            std::collections::HashMap::new(),
            String::from("default"),
        )
        .await?;
    await_history(
        &store,
        handle.workflow_id(),
        "the live holder's dangling Started{1}",
        |history| started_attempts(history) == vec![1],
    )
    .await?;
    if paused {
        engine
            .pause_workflow(handle.workflow_id(), handle.run_id(), None, None)
            .await?;
        let history = store.read_history(handle.workflow_id()).await?;
        assert_eq!(
            aion_core::status_from_events(&history),
            WorkflowStatus::Paused,
            "fixture control: the dangling holder must cross boot as Paused: {history:#?}"
        );
    }
    engine.shutdown()?;

    // Life 2 replays through the same Recorder-owned dispatch seam. The
    // adoption offer must retain attempt 1: no false failure and no invented
    // attempt 2 are allowed before the surviving holder is joined.
    let serving = ScriptedDispatcher::new(vec![Step::Succeed]);
    let recovered =
        engine_with(Arc::clone(&serving) as Arc<dyn ActivityDispatcher>, &store).await?;
    if paused {
        recovered
            .resume_paused_workflow(handle.workflow_id(), handle.run_id(), None)
            .await?;
    }
    let result = recovered
        .result(handle.workflow_id(), handle.run_id())
        .await?;
    let payload = result.map_err(|error| format!("adopted run failed: {error:?}"))?;
    let reply: serde_json::Value = serde_json::from_slice(payload.bytes())?;
    assert_eq!(reply, json!("steady hello, Ada"));

    let history = store.read_history(handle.workflow_id()).await?;
    assert!(
        supersession_records(&history).is_empty(),
        "a live holder must never be recorded as superseded by server death: {history:#?}"
    );
    assert_eq!(
        adoption_offer_records(&history),
        vec![(
            adoption_offer_records(&history)
                .first()
                .map(|(seq, _)| *seq)
                .unwrap_or_default(),
            1,
        )],
        "recovery must record exactly one explicit adoption offer for attempt 1: {history:#?}"
    );
    assert_eq!(
        started_attempts(&history),
        vec![1],
        "adoption redelivers the dangling attempt; it does not invent a successor: {history:#?}"
    );
    assert_eq!(holder.seen_attempts(), vec![1]);
    assert_eq!(serving.seen_attempts(), vec![1]);

    recovered.shutdown()?;
    Ok(())
}

/// #36 Running face: a server generation must offer its dangling remote
/// attempt for adoption, not write the false server-death supersession that
/// killed the observed Running incidents.
#[tokio::test]
async fn running_recovery_records_adoption_without_supersession()
-> Result<(), Box<dyn std::error::Error>> {
    recovery_offers_dangling_attempt_for_adoption(false).await
}

/// #36 Paused face: Paused histories admitted by the startup sweep have the
/// same truthful adoption contract as Running histories.
#[tokio::test]
async fn paused_recovery_records_adoption_without_supersession()
-> Result<(), Box<dyn std::error::Error>> {
    recovery_offers_dangling_attempt_for_adoption(true).await
}

/// A dangling attempt is offered once under the SAME attempt identity. A fresh
/// run is the negative control: it receives neither an adoption offer nor a
/// server-death supersession record.
#[tokio::test]
async fn recovery_records_one_same_attempt_adoption_offer() -> Result<(), Box<dyn std::error::Error>>
{
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
    await_history(
        &store,
        handle.workflow_id(),
        "the dangling Started{1}",
        |h| started_attempts(h) == vec![1],
    )
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
        vec![1],
        "adoption must retain the dangling attempt identity: {history:#?}"
    );
    assert!(
        supersession_records(&history).is_empty(),
        "adoption must not record server death: {history:#?}"
    );
    let records = adoption_offer_records(&history);
    assert_eq!(
        records.len(),
        1,
        "the dangling attempt gets one adoption offer: {history:#?}"
    );
    assert_eq!(
        records[0].1, 1,
        "the offer names the dangling attempt: {history:#?}"
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
    let result = recovered
        .result(fresh.workflow_id(), fresh.run_id())
        .await?;
    let payload = result.map_err(|error| format!("fresh control run failed: {error:?}"))?;
    let reply: serde_json::Value = serde_json::from_slice(payload.bytes())?;
    assert_eq!(reply, json!("steady hello, Grace"));
    let fresh_history = store.read_history(fresh.workflow_id()).await?;
    assert!(
        supersession_records(&fresh_history).is_empty(),
        "a fresh ordinal must never receive a server-death record: {fresh_history:#?}"
    );
    assert!(
        adoption_offer_records(&fresh_history).is_empty(),
        "a fresh ordinal must never receive an adoption offer: {fresh_history:#?}"
    );

    // Life 1 and recovery both deliver attempt 1; the second is the adoption
    // redelivery. The final 1 belongs to the fresh control.
    assert_eq!(parked.seen_attempts(), vec![1]);
    assert_eq!(serving.seen_attempts(), vec![1, 1]);

    recovered.shutdown()?;
    Ok(())
}

/// A prior retryable failure stays exactly once in history. If the following
/// attempt dangles, recovery offers only that attempt for adoption and does not
/// duplicate or rewrite the earlier terminal for attempt 1.
#[tokio::test]
async fn adoption_does_not_duplicate_an_earlier_attempt_failure()
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
    await_history(
        &store,
        handle.workflow_id(),
        "the dangling Started{2}",
        |h| started_attempts(h) == vec![1, 2],
    )
    .await?;
    engine.shutdown()?;

    // Life 2: recovery redelivers and completes dangling attempt 2.
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
        "the trail continues through both lives: {history:#?}"
    );
    assert!(
        supersession_records(&history).is_empty(),
        "recovery writes no false server-death record: {history:#?}"
    );
    assert_eq!(
        adoption_offer_records(&history)
            .iter()
            .map(|(_, attempt)| *attempt)
            .collect::<Vec<_>>(),
        vec![2],
        "only the dangling attempt gets an adoption offer: {history:#?}"
    );
    let attempt_1_failures = history
        .iter()
        .filter(|event| matches!(event, Event::ActivityFailed { attempt: 1, .. }))
        .count();
    assert_eq!(
        attempt_1_failures, 1,
        "attempt 1 keeps exactly its one real failure record: {history:#?}"
    );

    // Wire trail: recovery redelivers the retained attempt 2 identity.
    assert_eq!(script.seen_attempts(), vec![1, 2]);
    assert_eq!(serving.seen_attempts(), vec![2]);

    recovered.shutdown()?;
    Ok(())
}

/// BUDGET-INERT: an adoption offer spends no attempt. Recovery redelivers
/// attempt 1; if that retained attempt fails retryably, the ordinary policy
/// advances once to attempt 2 and succeeds.
#[tokio::test]
async fn the_adoption_offer_is_budget_inert() -> Result<(), Box<dyn std::error::Error>> {
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
    await_history(
        &store,
        handle.workflow_id(),
        "the dangling Started{1}",
        |h| started_attempts(h) == vec![1],
    )
    .await?;
    engine.shutdown()?;

    // Life 2: adopted attempt 1 fails retryably, then attempt 2 succeeds.
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
        vec![1, 2],
        "the offer must not invent or consume an attempt: {history:#?}"
    );
    assert!(
        supersession_records(&history).is_empty(),
        "the adoption path writes no server-death failure: {history:#?}"
    );
    assert_eq!(
        adoption_offer_records(&history)
            .iter()
            .map(|(_, attempt)| *attempt)
            .collect::<Vec<_>>(),
        vec![1],
        "one adoption offer for the dangling attempt: {history:#?}"
    );
    assert!(
        history
            .iter()
            .any(|event| matches!(event, Event::ActivityCompleted { attempt: 2, .. })),
        "the next ordinary numbered attempt completes the run: {history:#?}"
    );

    // Wire trail: life 1 delivered attempt 1; recovery redelivered attempt 1,
    // then ordinary retry delivered attempt 2.
    assert_eq!(parked.seen_attempts(), vec![1]);
    assert_eq!(serving.seen_attempts(), vec![1, 2]);

    recovered.shutdown()?;
    Ok(())
}
