//! Recovery's fork at the DECLARED CLASS: what a dangling attempt means when
//! an engine generation dies with it in flight.
//!
//! Two rulings meet here and disagree, for good reasons on both sides:
//!
//! * **#266 Defect B** — an ORDINARY activity's dangling attempt gets an
//!   honest NON-terminal `superseded:server-death` record, so an orphaned run
//!   stops reading as silently still working, and the re-dispatch continues
//!   the trail at the NEXT attempt.
//! * **#36** — a declared AGENT action's holder is its own OS process and can
//!   outlive the engine, so the same record is a LIE about live work. The
//!   attempt identity is RETAINED (the worker's spawn gate and single-flight
//!   recognise the execution they already hold) and an explicit
//!   `ActivityAdoptionOffered` is recorded instead, through the run's one
//!   Recorder.
//!
//! The engine cannot observe a harness process, so the declared class is the
//! only thing at the dispatch seam that can tell them apart. This exhibit is
//! therefore a PAIR whose documents differ in exactly one token — `agent` —
//! written that way for the same reason the advisory exhibit is: a fixture
//! that only exercises the new behaviour cannot tell "adoption arrived" from
//! "supersession was deleted", and the second of those is a regression that
//! took five other gates down when it happened.
//!
//! The corpse in every test is produced without killing anything: the parked
//! sentinel (`parked:server-draining`) stands the retry loop down with NOTHING
//! recorded and NOTHING delivered, leaving the durable log at the dangling
//! scheduled/started trail — byte-equivalent to a kill -9 (#207's documented
//! equivalence). Engine shutdown then plays the death; a second engine over
//! the same store plays the recovering server.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use aion::EngineBuilder;
use aion::activity::bridge::{ActivityDispatch, ActivityDispatcher};
use aion_awl_package::compile_and_assemble_awl;
use aion_core::{ActivityErrorKind, Event, Payload, WorkflowStatus};
use aion_package::{ExtractionLimits, Package};
use aion_store::{EventStore, InMemoryStore};
use serde_json::json;

type TestError = Box<dyn std::error::Error>;
type TestResult = Result<(), TestError>;

/// The exact recorded reason the supersession record carries — asserted as a
/// literal so a constant rename can never silently change the recorded bytes.
const SERVER_DEATH_REASON: &str = "superseded:server-death";

/// The action both documents declare, and the one the dispatcher serves.
const ACTION: &str = "delegate";

/// The AGENT face: the action carries the `agent` marker, so a dangling
/// attempt is offered for adoption.
const AGENT_PROBE: &str = r"//! A dangling agent attempt must be offered for adoption, not disowned.
workflow adoption_agent_probe
  input subject: String
  outcome done: type Done, route success

type Done { reply: String }

type Reply { text: String, final_message: String, stop_reason: String, session_id: String }

worker probe
  action delegate(prompt: String) -> Reply
    agent, retry 3 every 1s

step work
  delegate(prompt: subject) -> reply
  route done(reply: reply.text)
";

/// The CONTROL: byte-identical to [`AGENT_PROBE`] minus the `agent` marker, so
/// a dangling attempt keeps the #266 supersession posture. If this test ever
/// goes green alongside a green agent test that asserts no supersession, the
/// two claims have stopped being distinguishable and the exhibit is broken.
const PLAIN_PROBE: &str = r"//! A dangling ordinary attempt keeps its honest supersession record.
workflow adoption_plain_probe
  input subject: String
  outcome done: type Done, route success

type Done { reply: String }

type Reply { text: String, final_message: String, stop_reason: String, session_id: String }

worker probe
  action delegate(prompt: String) -> Reply
    retry 3 every 1s

step work
  delegate(prompt: subject) -> reply
  route done(reply: reply.text)
";

/// Scripted per-delivery dispatcher: each delivery consumes the next step;
/// deliveries past the script's end succeed. `Park` answers the parked
/// sentinel (loop stands down, nothing recorded — the corpse maker),
/// `FailRetryable` answers a transient wire failure, `Succeed` replies with
/// the canonical agent-outcome record both documents declare.
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

    /// The attempt number stamped on every delivery this dispatcher saw — the
    /// WIRE trail, which must agree with the recorded one.
    fn seen_attempts(&self) -> Vec<u32> {
        self.calls
            .lock()
            .map(|calls| calls.clone())
            .unwrap_or_default()
    }
}

impl ActivityDispatcher for ScriptedDispatcher {
    fn dispatch(&self, request: ActivityDispatch) -> Result<String, String> {
        if request.name != ACTION {
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
            Step::Succeed => Ok(json!({
                "text": "steady hello",
                "final_message": "steady hello",
                "stop_reason": "end_turn",
                "session_id": "probe-session",
            })
            .to_string()),
        }
    }
}

fn package(source: &str) -> Result<Package, TestError> {
    let prepared = compile_and_assemble_awl(source, Path::new("."), "adoption_probe.awl")?;
    Ok(Package::load_from_bytes(
        prepared.archive,
        ExtractionLimits::unbounded(),
    )?)
}

async fn engine_with(
    dispatcher: Arc<dyn ActivityDispatcher>,
    store: &Arc<dyn EventStore>,
    source: &str,
) -> Result<aion::Engine, TestError> {
    Ok(EngineBuilder::new()
        .in_process_activity_serving()
        .store_arc(Arc::clone(store))
        .in_memory_visibility()
        .scheduler_threads(1)
        .activity_dispatcher(dispatcher)
        .load_workflows(package(source)?)
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
) -> TestResult {
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

/// Every supersession record in the trail: the attempt of each non-terminal
/// `ActivityFailed` carrying the server-death reason.
fn supersession_attempts(history: &[Event]) -> Vec<u32> {
    history
        .iter()
        .filter_map(|event| match event {
            Event::ActivityFailed { error, attempt, .. }
                if error.kind == ActivityErrorKind::Retryable
                    && error.message == SERVER_DEATH_REASON =>
            {
                Some(*attempt)
            }
            _ => None,
        })
        .collect()
}

/// Every adoption offer in the trail, read through the SERIALIZED
/// discriminant — the same name the store, the event stream and the console
/// read.
fn adoption_offer_attempts(history: &[Event]) -> Vec<u32> {
    history
        .iter()
        .filter_map(|event| {
            let value = serde_json::to_value(event).ok()?;
            if value.get("type")?.as_str()? != "ActivityAdoptionOffered" {
                return None;
            }
            u32::try_from(value.get("data")?.get("attempt")?.as_u64()?).ok()
        })
        .collect()
}

async fn start(
    engine: &aion::Engine,
    workflow_type: &str,
) -> Result<aion::WorkflowHandle, TestError> {
    Ok(engine
        .start_workflow(
            workflow_type,
            Payload::from_json(&json!({ "subject": "Ada" }))?,
            std::collections::HashMap::new(),
            String::from("default"),
        )
        .await?)
}

/// Life 1 leaves attempt 1 dangling exactly as a holder still owned by its
/// worker does when only the server disappears, then the engine dies.
async fn dangling_corpse(
    store: &Arc<dyn EventStore>,
    source: &str,
    workflow_type: &str,
    paused: bool,
    script: Vec<Step>,
    expect_started: Vec<u32>,
) -> Result<(aion::WorkflowHandle, Arc<ScriptedDispatcher>), TestError> {
    let holder = ScriptedDispatcher::new(script);
    let engine = engine_with(
        Arc::clone(&holder) as Arc<dyn ActivityDispatcher>,
        store,
        source,
    )
    .await?;
    let handle = start(&engine, workflow_type).await?;
    await_history(store, handle.workflow_id(), "the dangling Started trail", {
        let expect_started = expect_started.clone();
        move |history| started_attempts(history) == expect_started
    })
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
    } else {
        let history = store.read_history(handle.workflow_id()).await?;
        assert_eq!(
            aion_core::status_from_events(&history),
            WorkflowStatus::Running,
            "fixture control: the corpse must be an orphaned Running run: {history:#?}"
        );
    }
    engine.shutdown()?;
    Ok((handle, holder))
}

/// Life 2 over the same store, driven to its terminal.
async fn recover(
    store: &Arc<dyn EventStore>,
    source: &str,
    handle: &aion::WorkflowHandle,
    paused: bool,
    script: Vec<Step>,
) -> Result<(Vec<Event>, Arc<ScriptedDispatcher>), TestError> {
    let serving = ScriptedDispatcher::new(script);
    let recovered = engine_with(
        Arc::clone(&serving) as Arc<dyn ActivityDispatcher>,
        store,
        source,
    )
    .await?;
    if paused {
        recovered
            .resume_paused_workflow(handle.workflow_id(), handle.run_id(), None)
            .await?;
    }
    let result = recovered
        .result(handle.workflow_id(), handle.run_id())
        .await?;
    let payload = result.map_err(|error| format!("recovered run failed: {error:?}"))?;
    let reply: serde_json::Value = serde_json::from_slice(payload.bytes())?;
    assert_eq!(
        reply,
        json!({ "outcome": "done", "payload": { "reply": "steady hello" } }),
        "the recovered run routes its declared outcome, carrying the agent's reply text"
    );
    let history = store.read_history(handle.workflow_id()).await?;
    recovered.shutdown()?;
    Ok((history, serving))
}

async fn agent_face_offers_adoption(paused: bool) -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let (handle, holder) = dangling_corpse(
        &store,
        AGENT_PROBE,
        "adoption_agent_probe",
        paused,
        vec![Step::Park],
        vec![1],
    )
    .await?;
    let (history, serving) =
        recover(&store, AGENT_PROBE, &handle, paused, vec![Step::Succeed]).await?;

    assert!(
        supersession_attempts(&history).is_empty(),
        "a holder that can outlive the engine must never be recorded as superseded by \
         server death: {history:#?}"
    );
    assert_eq!(
        adoption_offer_attempts(&history),
        vec![1],
        "recovery records exactly one adoption offer, for the dangling attempt: {history:#?}"
    );
    assert_eq!(
        started_attempts(&history),
        vec![1],
        "adoption redelivers the dangling attempt; it does not invent a successor: {history:#?}"
    );
    assert_eq!(holder.seen_attempts(), vec![1]);
    assert_eq!(
        serving.seen_attempts(),
        vec![1],
        "the wire trail carries the RETAINED identity"
    );
    Ok(())
}

/// #36 Paused face: Paused histories admitted by the startup sweep have the
/// same truthful adoption contract as Running histories.
#[tokio::test]
async fn paused_recovery_records_adoption_without_supersession() -> TestResult {
    agent_face_offers_adoption(true).await
}

/// #36 Running face: a server generation must offer its dangling remote
/// attempt for adoption, not write the false server-death supersession that
/// killed the observed Running incidents.
#[tokio::test]
async fn running_recovery_records_adoption_without_supersession() -> TestResult {
    agent_face_offers_adoption(false).await
}

/// THE CONTROL, one token away: the same corpse under an action that is NOT
/// declared `agent` keeps #266 Defect B's honest supersession record and
/// advances the attempt. Adoption and supersession are therefore
/// distinguishable outcomes of this fixture, not coinciding ones.
#[tokio::test]
async fn an_ordinary_dangling_attempt_is_superseded_not_adopted() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let (handle, holder) = dangling_corpse(
        &store,
        PLAIN_PROBE,
        "adoption_plain_probe",
        false,
        vec![Step::Park],
        vec![1],
    )
    .await?;
    let (history, serving) =
        recover(&store, PLAIN_PROBE, &handle, false, vec![Step::Succeed]).await?;

    assert_eq!(
        supersession_attempts(&history),
        vec![1],
        "the orphaned ordinary attempt keeps its honest supersession record: {history:#?}"
    );
    assert!(
        adoption_offer_attempts(&history).is_empty(),
        "an ordinary action has no holder to adopt: {history:#?}"
    );
    assert_eq!(
        started_attempts(&history),
        vec![1, 2],
        "the re-dispatch continues the trail at the NEXT attempt: {history:#?}"
    );
    assert_eq!(holder.seen_attempts(), vec![1]);
    assert_eq!(
        serving.seen_attempts(),
        vec![2],
        "the wire trail carries the ADVANCED identity"
    );
    Ok(())
}

/// A fresh ordinal is the negative control on the other side: it receives
/// neither an adoption offer nor a supersession record, so neither record is
/// sprayed on ordinary dispatches.
#[tokio::test]
async fn a_fresh_dispatch_receives_neither_record() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let serving = ScriptedDispatcher::new(vec![Step::Succeed]);
    let engine = engine_with(
        Arc::clone(&serving) as Arc<dyn ActivityDispatcher>,
        &store,
        AGENT_PROBE,
    )
    .await?;
    let handle = start(&engine, "adoption_agent_probe").await?;
    let result = engine.result(handle.workflow_id(), handle.run_id()).await?;
    result.map_err(|error| format!("fresh run failed: {error:?}"))?;

    let history = store.read_history(handle.workflow_id()).await?;
    assert!(
        supersession_attempts(&history).is_empty(),
        "a fresh ordinal must never receive a server-death record: {history:#?}"
    );
    assert!(
        adoption_offer_attempts(&history).is_empty(),
        "a fresh ordinal must never receive an adoption offer: {history:#?}"
    );
    assert_eq!(started_attempts(&history), vec![1]);
    engine.shutdown()?;
    Ok(())
}

/// A prior retryable failure stays exactly once in history. If the following
/// attempt dangles, recovery offers only that attempt for adoption and does
/// not duplicate or rewrite the earlier terminal for attempt 1.
#[tokio::test]
async fn adoption_does_not_duplicate_an_earlier_attempt_failure() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let (handle, script) = dangling_corpse(
        &store,
        AGENT_PROBE,
        "adoption_agent_probe",
        false,
        vec![Step::FailRetryable, Step::Park],
        vec![1, 2],
    )
    .await?;
    let (history, serving) =
        recover(&store, AGENT_PROBE, &handle, false, vec![Step::Succeed]).await?;

    assert_eq!(
        started_attempts(&history),
        vec![1, 2],
        "the trail continues through both lives: {history:#?}"
    );
    assert!(
        supersession_attempts(&history).is_empty(),
        "recovery writes no false server-death record: {history:#?}"
    );
    assert_eq!(
        adoption_offer_attempts(&history),
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
    assert_eq!(script.seen_attempts(), vec![1, 2]);
    assert_eq!(serving.seen_attempts(), vec![2]);
    Ok(())
}

/// BUDGET-INERT: an adoption offer spends no attempt. Recovery redelivers
/// attempt 1; if that retained attempt fails retryably, the ordinary policy
/// advances once to attempt 2 and succeeds.
#[tokio::test]
async fn the_adoption_offer_is_budget_inert() -> TestResult {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let (handle, parked) = dangling_corpse(
        &store,
        AGENT_PROBE,
        "adoption_agent_probe",
        false,
        vec![Step::Park],
        vec![1],
    )
    .await?;
    let (history, serving) = recover(
        &store,
        AGENT_PROBE,
        &handle,
        false,
        vec![Step::FailRetryable, Step::Succeed],
    )
    .await?;

    assert_eq!(
        started_attempts(&history),
        vec![1, 2],
        "the offer must not invent or consume an attempt: {history:#?}"
    );
    assert!(
        supersession_attempts(&history).is_empty(),
        "the adoption path writes no server-death failure: {history:#?}"
    );
    assert_eq!(
        adoption_offer_attempts(&history),
        vec![1],
        "one adoption offer for the dangling attempt: {history:#?}"
    );
    assert!(
        history
            .iter()
            .any(|event| matches!(event, Event::ActivityCompleted { attempt: 2, .. })),
        "the next ordinary numbered attempt completes the run: {history:#?}"
    );
    assert_eq!(parked.seen_attempts(), vec![1]);
    assert_eq!(serving.seen_attempts(), vec![1, 2]);
    Ok(())
}
