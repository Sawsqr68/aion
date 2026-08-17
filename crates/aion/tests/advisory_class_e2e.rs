//! End-to-end exhibit for the ADVISORY action class (RUNTIME-OPERATIONS.md
//! R5).
//!
//! R5: "Side-channel actions (heartbeats, notifications) declare `advisory`:
//! retry exhaustion emits a visible warning event on the run and surfaces in
//! the console; it never faults the step."
//!
//! The failure R5 exists to kill is live in this repository: the remediation
//! packet's flagship AWL document puts five meridian-heartbeat `run_command`
//! calls on its critical path, so a meridian outage sinks an otherwise
//! healthy run. Both documents below are that shape, reduced to its
//! essentials — one side-channel action that can never succeed, followed by
//! the real work — and they differ in exactly one token:
//!
//! * [`ADVISORY`] declares `advisory` on the heartbeat action. Its run must
//!   COMPLETE: the exhausted heartbeat records its honest terminal
//!   `ActivityFailed` plus a warning-class `ActivityAdvisoryExhausted` event
//!   on the run, the step is not faulted, and the real work still runs.
//! * [`NON_ADVISORY`] is byte-identical minus the marker. Its run must FAIL,
//!   exactly as today — the non-advisory posture is unchanged by R5 and this
//!   test is its regression pin.
//!
//! RED-FIRST, and the record of it: at 65973b59 the first test FAILED (the
//! document did not parse — `advisory` was not yet a config key) while the
//! second passed, which is why the pair is written as one exhibit differing in
//! one token. The warning event is still asserted through its serialized
//! `type` discriminant rather than a Rust variant, so the exhibit reads the
//! same name the store, the event stream, and the console read.
//!
//! The third test is the replay pin: the run's history — warning event and
//! all — replays from history alone with no divergence.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use aion::activity::bridge::{ActivityDispatch, ActivityDispatcher};
use aion::signal::ConcreteSignalRouter;
use aion::{EngineBuilder, RuntimeHandle, SignalRouter};
use aion_awl_package::compile_and_assemble_awl;
use aion_core::{ActivityErrorKind, Event, Payload, WorkflowStatus};
use aion_package::{ExtractionLimits, Package};
use aion_store::{EventStore, InMemoryStore};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;
type TestError = Box<dyn std::error::Error>;

/// The serialized `type` discriminant of the warning-class event R5 requires
/// on the run. Asserted as a string so the exhibit compiles before the
/// variant exists (see the module docs).
const ADVISORY_EVENT_TYPE: &str = "ActivityAdvisoryExhausted";

const ADVISORY_MODULE: &str = "advisory_probe";

/// The heartbeat is declared `advisory`: its exhaustion must not fault the
/// step that calls it.
const ADVISORY: &str = r"//! A side-channel heartbeat outage must not sink an otherwise healthy run.
workflow advisory_probe
  input subject: String
  outcome done: type Done, route success

type Done { reply: String }

worker probe
  action send_heartbeat(subject: String) -> Nil
    advisory, retry 2 every 1s
  action do_work(subject: String) -> Done
    timeout 30s

step announce
  send_heartbeat(subject: subject)

step work after announce
  do_work(subject: subject) -> result
  route done(reply: result.reply)
";

const NON_ADVISORY_MODULE: &str = "non_advisory_probe";

/// The control: identical to [`ADVISORY`] minus the `advisory` marker. Its
/// exhaustion faults the step and fails the run — today's behaviour, which
/// R5 must leave untouched.
const NON_ADVISORY: &str = r"//! A side-channel heartbeat outage must not sink an otherwise healthy run.
workflow non_advisory_probe
  input subject: String
  outcome done: type Done, route success

type Done { reply: String }

worker probe
  action send_heartbeat(subject: String) -> Nil
    retry 2 every 1s
  action do_work(subject: String) -> Done
    timeout 30s

step announce
  send_heartbeat(subject: subject)

step work after announce
  do_work(subject: subject) -> result
  route done(reply: result.reply)
";

/// Stands in for a meridian outage: every `send_heartbeat` delivery fails
/// retryably, forever. `do_work` — the run's real purpose — always succeeds,
/// so any run that fails here failed BECAUSE of the side channel.
struct OutageDispatcher {
    heartbeat_deliveries: AtomicU32,
    work_deliveries: AtomicU32,
}

impl OutageDispatcher {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            heartbeat_deliveries: AtomicU32::new(0),
            work_deliveries: AtomicU32::new(0),
        })
    }

    fn heartbeat_deliveries(&self) -> u32 {
        self.heartbeat_deliveries.load(Ordering::SeqCst)
    }

    fn work_deliveries(&self) -> u32 {
        self.work_deliveries.load(Ordering::SeqCst)
    }
}

impl ActivityDispatcher for OutageDispatcher {
    fn dispatch(&self, request: ActivityDispatch) -> Result<String, String> {
        match request.name.as_str() {
            "send_heartbeat" => {
                self.heartbeat_deliveries.fetch_add(1, Ordering::SeqCst);
                Err(format!(
                    "retryable:meridian unreachable (delivery attempt {})",
                    request.attempt
                ))
            }
            "do_work" => {
                self.work_deliveries.fetch_add(1, Ordering::SeqCst);
                let value: Value = serde_json::from_str(&request.input)
                    .map_err(|error| format!("terminal:bad do_work input: {error}"))?;
                let subject = value
                    .get("subject")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "terminal:do_work input has no subject".to_owned())?;
                serde_json::to_string(&json!({ "reply": format!("work done for {subject}") }))
                    .map_err(|error| format!("terminal:{error}"))
            }
            other => Err(format!("terminal:unknown probe activity {other}")),
        }
    }
}

/// One run of a DIRECT-compiled AWL document against a real engine, with the
/// outage dispatcher serving both actions.
struct RunRecord {
    outcome: Result<Payload, aion_core::WorkflowError>,
    history: Vec<Event>,
    run_id: aion_core::RunId,
    heartbeat_deliveries: u32,
    work_deliveries: u32,
}

fn direct_package(source: &str) -> Result<Package, TestError> {
    let prepared = compile_and_assemble_awl(source, Path::new("."), "advisory_probe.awl")?;
    Ok(Package::load_from_bytes(
        prepared.archive,
        ExtractionLimits::unbounded(),
    )?)
}

async fn run_probe(source: &str, module: &str) -> Result<RunRecord, TestError> {
    let package = direct_package(source)?;
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    let dispatcher = OutageDispatcher::new();
    let engine = EngineBuilder::new()
        .in_process_activity_serving()
        .store_arc(Arc::clone(&store))
        .in_memory_visibility()
        .scheduler_threads(2)
        .signal_router_factory(|runtime: Arc<RuntimeHandle>, handoff| {
            Arc::new(ConcreteSignalRouter::new(runtime, handoff)) as Arc<dyn SignalRouter>
        })
        .activity_dispatcher(Arc::clone(&dispatcher) as Arc<dyn ActivityDispatcher>)
        .load_workflows(package)
        .build()
        .await?;
    let handle = engine
        .start_workflow(
            module,
            Payload::from_json(&json!({ "subject": "packet run" }))?,
            std::collections::HashMap::new(),
            "default".to_owned(),
        )
        .await?;
    let workflow_id = handle.workflow_id().clone();
    let run_id = handle.run_id().clone();
    let inspected_run = run_id.clone();
    let outcome = tokio::time::timeout(
        Duration::from_secs(60),
        engine.result(&workflow_id, &run_id),
    )
    .await??;
    let history = store.read_history(&workflow_id).await?;
    engine.shutdown()?;
    Ok(RunRecord {
        outcome,
        history,
        run_id: inspected_run,
        heartbeat_deliveries: dispatcher.heartbeat_deliveries(),
        work_deliveries: dispatcher.work_deliveries(),
    })
}

/// The serialized `type` discriminants of a history, in order. `Event` is
/// `#[serde(tag = "type", content = "data")]`, so this is the same name the
/// store, the event stream, and the console see.
fn event_types(history: &[Event]) -> Result<Vec<String>, TestError> {
    let mut types = Vec::with_capacity(history.len());
    for event in history {
        let value = serde_json::to_value(event)?;
        let name = value
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("event has no `type` discriminant: {value}"))?;
        types.push(name.to_owned());
    }
    Ok(types)
}

fn terminal_activity_failures(history: &[Event]) -> Vec<(u32, String)> {
    history
        .iter()
        .filter_map(|event| match event {
            Event::ActivityFailed { error, attempt, .. }
                if error.kind == ActivityErrorKind::Terminal =>
            {
                Some((*attempt, error.message.clone()))
            }
            _ => None,
        })
        .collect()
}

/// R5, the whole ruling in one run: an advisory heartbeat that can never
/// succeed exhausts its budget, records its honest terminal failure AND a
/// warning-class event on the run, does not fault the calling step, and the
/// run completes with the real work done.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn advisory_exhaustion_warns_and_the_run_completes() -> TestResult {
    let record = run_probe(ADVISORY, ADVISORY_MODULE).await?;

    let payload = record.outcome.map_err(|error| {
        format!("an advisory side-channel outage must NOT fail the run (R5), but it did: {error:?}")
    })?;
    let result: Value = serde_json::from_slice(payload.bytes())?;
    assert_eq!(
        result,
        json!({
            "outcome": "done",
            "payload": { "reply": "work done for packet run" }
        }),
        "the run's real work must still produce the outcome payload"
    );

    assert_eq!(
        aion_core::status_from_events(&record.history),
        WorkflowStatus::Completed,
        "history must project Completed: {:#?}",
        record.history
    );

    // The budget really was exhausted: `retry 2` = two FURTHER attempts, so
    // three deliveries total, then it stopped.
    assert_eq!(
        record.heartbeat_deliveries, 3,
        "the advisory heartbeat must exhaust its declared budget"
    );
    // The step after the advisory call still ran — "never faults the step".
    assert_eq!(
        record.work_deliveries, 1,
        "the real work must run after an advisory exhaustion"
    );

    // The activity honestly failed and history says so — advisory suppresses
    // the FAULT, never the RECORD.
    let terminals = terminal_activity_failures(&record.history);
    assert!(
        terminals.iter().any(|(attempt, message)| *attempt == 3
            && message
                == "exhausted: spent 3 of 3 attempts; last refusal: \
                    retryable:meridian unreachable (delivery attempt 3)"),
        "the exhausted advisory activity must still record its terminal \
         ActivityFailed naming exhaustion and the last refusal: {terminals:?}"
    );

    // And the run carries the visible warning R5 requires.
    let types = event_types(&record.history)?;
    assert!(
        types.iter().any(|name| name == ADVISORY_EVENT_TYPE),
        "the run must carry a visible `{ADVISORY_EVENT_TYPE}` warning event; recorded: {types:?}"
    );

    Ok(())
}

/// The control and the regression pin: without the marker, the same outage
/// faults the step and fails the run — today's behaviour, which R5 leaves
/// untouched for non-advisory actions.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn non_advisory_exhaustion_still_fails_the_run() -> TestResult {
    let record = run_probe(NON_ADVISORY, NON_ADVISORY_MODULE).await?;

    let error = match record.outcome {
        Err(error) => error,
        Ok(payload) => {
            return Err(format!(
                "a non-advisory exhaustion must fail the run, but it completed with {:?}",
                String::from_utf8_lossy(payload.bytes())
            )
            .into());
        }
    };
    let detail = error
        .details
        .as_ref()
        .map(|payload| String::from_utf8_lossy(payload.bytes()).into_owned())
        .unwrap_or_default();
    assert!(
        detail.contains("meridian unreachable (delivery attempt 3)")
            || error
                .message
                .contains("meridian unreachable (delivery attempt 3)"),
        "the last attempt's reason must surface verbatim on the run failure: {error:?}"
    );

    assert_eq!(
        aion_core::status_from_events(&record.history),
        WorkflowStatus::Failed,
        "history must project Failed: {:#?}",
        record.history
    );
    assert_eq!(
        record.heartbeat_deliveries, 3,
        "the non-advisory heartbeat exhausts the same declared budget"
    );
    assert_eq!(
        record.work_deliveries, 0,
        "the faulted step must stop the run before the real work"
    );

    // No warning event: the warning class is the advisory class's alone.
    let types = event_types(&record.history)?;
    assert!(
        !types.iter().any(|name| name == ADVISORY_EVENT_TYPE),
        "a non-advisory failure must not emit the advisory warning: {types:?}"
    );

    Ok(())
}

/// Replay determinism after an advisory exhaustion: the recorded history —
/// which now carries the warning event WEDGED BETWEEN the last
/// `ActivityStarted` and the activity's terminal `ActivityFailed` — replays
/// from history alone with no divergence, and the advisory ordinal still
/// resolves to the same recorded terminal it resolved to live.
///
/// This is the pin the ordering choice needs. The warning is recorded by the
/// dispatch-side retry loop just before the failure is delivered, so it lands
/// BEFORE the terminal the workflow thread records on receipt. If it landed
/// after, it would be the last event of the ordinal's matched range and replay
/// would read the run's outcome off a warning instead of off the failure.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn advisory_exhaustion_replays_from_history_without_divergence() -> TestResult {
    let record = run_probe(ADVISORY, ADVISORY_MODULE).await?;
    let inspection =
        aion::durability::replay_inspect::inspect_run(record.history.clone(), &record.run_id)?;

    assert!(
        inspection.divergence.is_none(),
        "an advisory exhaustion must replay from history alone with no divergence: {:#?}",
        inspection.divergence
    );

    // The warning is present in the projected trail...
    assert!(
        inspection
            .steps
            .iter()
            .any(|step| step.event_kind == ADVISORY_EVENT_TYPE),
        "the replay projection must carry the advisory warning: {:?}",
        inspection
            .steps
            .iter()
            .map(|step| step.event_kind)
            .collect::<Vec<_>>()
    );
    // ...and it did not displace the terminal the ordinal resolves to.
    let resolved_terminal = inspection.steps.iter().any(|step| {
        matches!(
            &step.projection,
            aion::durability::replay_inspect::StepProjection::Resolved(
                aion::durability::Resolution::ActivityFailedTerminal(error)
            ) if error.message
                == "exhausted: spent 3 of 3 attempts; last refusal: \
                    retryable:meridian unreachable (delivery attempt 3)"
        )
    });
    assert!(
        resolved_terminal,
        "the advisory ordinal must still resolve to its recorded terminal failure: {:#?}",
        inspection.steps
    );

    Ok(())
}
