//! R1 acceptance pins — the service policies, the two clocks, and the typed
//! refusal each taxonomy state produces at the live bridge seam.
//!
//! Companion to `unserved_queue_honesty.rs` (the tracing exhibit, which owns
//! its own binary for subscriber isolation). Everything here drives the real
//! `WorkerActivityDispatcher` against the real connected-worker registry.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use aion::{ActivityDispatch, ActivityDispatcher as _};
use aion_core::{ActivityId, ContentType, Payload, RunId, WorkflowId};
use aion_server::worker::queue_service::{
    QueueServiceConfig, QueueServiceOverride, QueueServicePolicy, QueueServiceState,
};
use aion_server::worker::registry::WorkerMessage;
use aion_server::worker::{
    ActivityCompletion, ActivityCompletionOutcome, ActivityCompletionSink as _, CompletionToken,
    ConnectedWorkerRegistry, ExpiredClock, HeartbeatTracker, PendingActivities, QueueDeclaration,
    QueueDeclarationSource, QueueDeclarations, QueueServiceReason, UnavailableSummary,
    WorkerActivityDispatcher, WorkerUnavailable,
};

type TestError = Box<dyn std::error::Error>;

fn dispatch_to_unserved_queue() -> ActivityDispatch {
    ActivityDispatch {
        namespace: "default".to_owned(),
        task_queue: "nobody-serves-this".to_owned(),
        node: None,
        workflow_id: WorkflowId::new_v4(),
        run_id: RunId::new_v4(),
        activity_id: ActivityId::from_sequence_position(0),
        name: "greet".to_owned(),
        input: "{}".to_owned(),
        config: "{}".to_owned(),
        attempt: 1,
        labels: BTreeMap::new(),
    }
}

/// The declared-queue reader, shaped exactly like the engine-backed one: an
/// empty declaration set is UNKNOWABLE (it can contradict nothing), a
/// non-empty one answers precisely.
struct DeclaredQueues(BTreeSet<String>);

impl DeclaredQueues {
    fn declaring(queues: &[&str]) -> Arc<Self> {
        Arc::new(Self(
            queues.iter().map(|queue| (*queue).to_owned()).collect(),
        ))
    }
}

impl QueueDeclarations for DeclaredQueues {
    fn declaration_for(&self, task_queue: &str) -> QueueDeclaration {
        if self.0.is_empty() {
            QueueDeclaration::Unknown
        } else if self.0.contains(task_queue) {
            QueueDeclaration::Declared
        } else {
            QueueDeclaration::NotDeclared
        }
    }
}

/// One assembled dispatcher plus the seams a test drives it through.
struct Seam {
    registry: ConnectedWorkerRegistry,
    pending: PendingActivities,
    state: QueueServiceState,
    dispatcher: Arc<WorkerActivityDispatcher>,
}

fn seam(config: QueueServiceConfig, declarations: Option<Arc<DeclaredQueues>>) -> Seam {
    let registry = ConnectedWorkerRegistry::default();
    let pending = PendingActivities::default();
    let state = QueueServiceState::default();
    let source = QueueDeclarationSource::default();
    if let Some(declarations) = declarations {
        source.install(declarations);
    }
    let dispatcher = WorkerActivityDispatcher::new(
        registry.clone(),
        "default",
        HeartbeatTracker::new(Duration::from_secs(5)),
    )
    .with_pending(pending.clone())
    .with_queue_service(config)
    .with_queue_declarations(source)
    .with_queue_state(state.clone());
    Seam {
        registry,
        pending,
        state,
        dispatcher: Arc::new(dispatcher),
    }
}

fn strict_with_deadline(deadline: Duration) -> QueueServiceConfig {
    QueueServiceConfig {
        service_availability_deadline: Some(deadline),
        ..QueueServiceConfig::default()
    }
}

fn durable_pending_for(task_queue: &str, deadline: Duration) -> QueueServiceConfig {
    QueueServiceConfig {
        overrides: vec![QueueServiceOverride {
            namespace: None,
            task_queue: task_queue.to_owned(),
            policy: QueueServicePolicy::DurablePending,
        }],
        service_availability_deadline: Some(deadline),
        ..QueueServiceConfig::default()
    }
}

/// The typed head of a dispatch failure, or a description of why it was not a
/// queue-service refusal at all.
fn refusal(outcome: &Result<String, String>) -> Result<UnavailableSummary, String> {
    let failure = outcome
        .as_ref()
        .err()
        .ok_or_else(|| "dispatch succeeded; expected a typed refusal".to_owned())?;
    WorkerUnavailable::parse(failure)
        .ok_or_else(|| format!("failure carried no typed queue-service reason: {failure}"))
}

/// Structural unservability refuses UNCONDITIONALLY: no deployment declares the
/// queue, so no policy and no clock can make waiting honest.
#[test]
fn an_undeclared_queue_refuses_immediately_under_both_policies() -> Result<(), TestError> {
    for policy in [
        QueueServicePolicy::Strict,
        QueueServicePolicy::DurablePending,
    ] {
        let config = QueueServiceConfig {
            default_policy: policy,
            // A generous deadline that must NOT be consulted: structural
            // unservability never waits.
            service_availability_deadline: Some(Duration::from_secs(600)),
            ..QueueServiceConfig::default()
        };
        let seam = seam(config, Some(DeclaredQueues::declaring(&["billing"])));
        let started = Instant::now();
        let outcome = seam.dispatcher.dispatch(dispatch_to_unserved_queue());
        let elapsed = started.elapsed();

        let summary = refusal(&outcome)?;
        assert_eq!(
            summary.reason,
            QueueServiceReason::NoQueueDeclaration,
            "policy {policy} misclassified an undeclared queue"
        );
        assert_eq!(summary.clock, None, "a structural refusal expires no clock");
        let failure = outcome.err().unwrap_or_default();
        assert!(
            failure.starts_with("terminal:"),
            "a structural refusal is not retryable: {failure}"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "structural refusal waited {elapsed:?} instead of refusing at once"
        );
        assert!(
            seam.state.unserved()?.is_empty(),
            "a refused dispatch must not stay in the unserved state"
        );
    }
    Ok(())
}

/// Strict + no live pollers: the dispatch waits out the service-availability
/// deadline and then refuses with the typed reason and THAT clock.
#[test]
fn strict_refuses_no_live_pollers_when_the_availability_deadline_expires() -> Result<(), TestError>
{
    let deadline = Duration::from_millis(900);
    let seam = seam(
        strict_with_deadline(deadline),
        Some(DeclaredQueues::declaring(&["nobody-serves-this"])),
    );
    let started = Instant::now();
    let outcome = seam.dispatcher.dispatch(dispatch_to_unserved_queue());
    let elapsed = started.elapsed();

    let summary = refusal(&outcome)?;
    assert_eq!(summary.reason, QueueServiceReason::NoLivePollers);
    assert_eq!(summary.clock, Some(ExpiredClock::ServiceAvailability));
    assert!(
        summary.waited >= deadline,
        "refused after {:?}, before the declared deadline {deadline:?}",
        summary.waited
    );
    assert!(
        elapsed < Duration::from_secs(10),
        "the deadline did not bound the wait: {elapsed:?}"
    );
    let failure = outcome.err().unwrap_or_default();
    assert!(
        failure.starts_with("retryable:"),
        "a fleet condition is transient, not terminal: {failure}"
    );
    assert!(seam.state.unserved()?.is_empty());
    Ok(())
}

/// Workers are connected for the pool but none serves this activity: the state
/// is `POLLERS_INCOMPATIBLE`, not `NO_LIVE_POLLERS` — the distinction the flagship
/// incident could not make.
#[test]
fn connected_workers_that_do_not_serve_the_activity_are_incompatible() -> Result<(), TestError> {
    let seam = seam(
        strict_with_deadline(Duration::from_millis(900)),
        Some(DeclaredQueues::declaring(&["nobody-serves-this"])),
    );
    let (worker_tx, _worker_rx) = tokio::sync::mpsc::channel(4);
    let registration = seam.registry.register_namespaces(
        [String::from("default")],
        "nobody-serves-this",
        None,
        [String::from("settle")].iter(),
        worker_tx,
    )?;

    let outcome = seam.dispatcher.dispatch(dispatch_to_unserved_queue());
    let summary = refusal(&outcome)?;
    assert_eq!(summary.reason, QueueServiceReason::PollersIncompatible);
    assert_eq!(summary.clock, Some(ExpiredClock::ServiceAvailability));
    registration.deregister()?;
    Ok(())
}

/// SATURATED really fires end to end: a compatible worker IS connected and its
/// intake stays full past the schedule-to-start clock — the other clock, named
/// distinctly.
#[test]
fn a_full_intake_on_a_live_worker_refuses_as_saturated() -> Result<(), TestError> {
    let config = QueueServiceConfig {
        schedule_to_start_timeout: Some(Duration::from_millis(700)),
        ..QueueServiceConfig::default()
    };
    let seam = seam(
        config,
        Some(DeclaredQueues::declaring(&["nobody-serves-this"])),
    );
    let (worker_tx, _worker_rx) = tokio::sync::mpsc::channel(1);
    // The worker is live, compatible, and its single intake slot is occupied.
    worker_tx
        .try_send(WorkerMessage::DrainRequest)
        .map_err(|error| format!("fixture fill failed: {error}"))?;
    let registration = seam.registry.register_namespaces(
        [String::from("default")],
        "nobody-serves-this",
        None,
        [String::from("greet")].iter(),
        worker_tx,
    )?;

    let outcome = seam.dispatcher.dispatch(dispatch_to_unserved_queue());
    let summary = refusal(&outcome)?;
    assert_eq!(summary.reason, QueueServiceReason::Saturated);
    assert_eq!(summary.clock, Some(ExpiredClock::ScheduleToStart));
    assert_ne!(
        summary.clock,
        Some(ExpiredClock::ServiceAvailability),
        "saturation must never be reported against the availability clock"
    );
    registration.deregister()?;
    Ok(())
}

/// The written opt-in: `durable_pending` does not refuse when the availability
/// deadline passes. The run is marked UNSERVED immediately and visibly, it
/// survives, and it completes when a worker finally arrives.
#[test]
fn durable_pending_parks_visibly_and_completes_when_a_worker_arrives() -> Result<(), TestError> {
    let seam = seam(
        durable_pending_for("nobody-serves-this", Duration::from_millis(200)),
        Some(DeclaredQueues::declaring(&["nobody-serves-this"])),
    );
    let request = dispatch_to_unserved_queue();
    let workflow_id = request.workflow_id.clone();
    let activity_id = request.activity_id.clone();

    let dispatcher = Arc::clone(&seam.dispatcher);
    let parked = std::thread::spawn(move || dispatcher.dispatch(request));

    // Marked unserved immediately and visibly — well before the deadline that
    // would have refused a strict dispatch.
    std::thread::sleep(Duration::from_millis(700));
    let unserved = seam.state.unserved()?;
    assert_eq!(
        unserved.len(),
        1,
        "the parked run is not visible: {unserved:?}"
    );
    assert_eq!(unserved[0].reason, QueueServiceReason::NoLivePollers);
    assert_eq!(unserved[0].policy, QueueServicePolicy::DurablePending);
    assert_eq!(unserved[0].key.task_queue, "nobody-serves-this");
    assert_eq!(unserved[0].waiting.len(), 1);
    assert_eq!(unserved[0].waiting[0].workflow_id, workflow_id);
    assert!(
        seam.state.is_unserved(&workflow_id, &activity_id)?,
        "the run itself must be answerable as unserved"
    );
    assert!(
        !parked.is_finished(),
        "durable_pending must survive the availability deadline, not refuse at it"
    );

    // The worker finally arrives: the parked dispatch is served and completes.
    let (worker_tx, mut worker_rx) = tokio::sync::mpsc::channel(4);
    let sink = seam.pending.clone();
    let worker = std::thread::spawn(move || -> Result<(), String> {
        let task = loop {
            match worker_rx.try_recv() {
                Ok(WorkerMessage::ActivityTask(task)) => break task,
                // Any other frame on the worker channel is not this dispatch;
                // keep waiting for the real task.
                Ok(_) => {}
                Err(_) => std::thread::sleep(Duration::from_millis(20)),
            }
        };
        let workflow_id = task
            .workflow_id
            .ok_or("task missing workflow id")
            .and_then(|id| WorkflowId::try_from(id).map_err(|_| "bad workflow id"))?;
        let activity_id = task
            .activity_id
            .map(ActivityId::from)
            .ok_or("task missing activity id")?;
        let completion_token =
            CompletionToken::from_wire(&workflow_id, &activity_id, task.completion_token)
                .map_err(|error| error.to_string())?;
        sink.complete_activity(ActivityCompletion {
            workflow_id,
            activity_id,
            run_id: None,
            completion_token,
            outcome: ActivityCompletionOutcome::Succeeded(Payload::new(
                ContentType::Json,
                br#"{"greeting":"served at last"}"#.to_vec(),
            )),
        })
        .map_err(|error| error.to_string())
    });
    let registration = seam.registry.register_namespaces(
        [String::from("default")],
        "nobody-serves-this",
        None,
        [String::from("greet")].iter(),
        worker_tx,
    )?;

    let outcome = parked.join().map_err(|_| "parked dispatch panicked")?;
    worker.join().map_err(|_| "worker thread panicked")??;
    assert_eq!(outcome, Ok(r#"{"greeting":"served at last"}"#.to_owned()));
    assert!(
        seam.state.unserved()?.is_empty(),
        "a served dispatch must leave the unserved state"
    );
    assert!(!seam.state.is_unserved(&workflow_id, &activity_id)?);
    registration.deregister()?;
    Ok(())
}

/// What the run records is typed, and it is NOT mistaken for a park.
///
/// The dispatch seam hands this exact string to the engine, which writes it as
/// the activity's failure reason — so the run's history carries the taxonomy
/// verdict, readable back without string sniffing. The park check is the sharp
/// edge: a refusal that looked like the drain sentinel would be swallowed as a
/// "parked for restart recovery" no-op and the run would silently stall again.
#[test]
fn the_recorded_failure_reason_is_typed_and_never_reads_as_a_park() -> Result<(), TestError> {
    let seam = seam(
        strict_with_deadline(Duration::from_millis(400)),
        Some(DeclaredQueues::declaring(&["nobody-serves-this"])),
    );
    let failure = seam
        .dispatcher
        .dispatch(dispatch_to_unserved_queue())
        .err()
        .ok_or("expected a refusal")?;

    assert!(
        !aion::is_parked_reason(&failure),
        "a queue-service refusal must never be classified as a park: {failure}"
    );
    assert!(
        failure.starts_with("retryable:") || failure.starts_with("terminal:"),
        "the recorded reason must carry the engine's retryability vocabulary: {failure}"
    );
    let summary = WorkerUnavailable::parse(&failure).ok_or_else(|| {
        format!("the recorded reason is not readable back as a typed refusal: {failure}")
    })?;
    assert_eq!(summary.reason, QueueServiceReason::NoLivePollers);
    assert_eq!(summary.clock, Some(ExpiredClock::ServiceAvailability));
    // The human tail names the address an operator has to fix.
    assert!(
        failure.contains("nobody-serves-this") && failure.contains("greet"),
        "the recorded reason does not name the address: {failure}"
    );
    Ok(())
}
