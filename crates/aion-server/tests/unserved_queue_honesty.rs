//! R1 exhibit — unserved-queue honesty at the bridge seam.
//!
//! The live defect this file pins: `select_worker_or_wait` collapses "being
//! served slowly" and "served by nobody". A dispatch addressed to a queue no
//! worker serves emits ONE `tracing::info!` ("no connected worker; waiting for
//! a matching worker to register") and then blocks forever — no typed service
//! state, no reason, no deadline, nothing an operator can query or alert on.
//! The committed 3.5-hour hang triage (`docs/evidence/handshake-red-green.md`,
//! defect #3) is that behaviour observed in production shape.
//!
//! The assertion below is the DESIRED behaviour (R1): while a dispatch is
//! parked on an unserved queue the seam states, at WARN, WHICH of the four
//! taxonomy states it is in and how old the last compatible poller is. It
//! fails against the pre-R1 seam because only an unreasoned INFO is emitted.
//!
//! This exhibit owns its whole test binary deliberately. It reads the seam's
//! real tracing output through a thread-scoped subscriber, and `tracing`'s
//! default-dispatcher state is process-global: a sibling test running
//! concurrently in the same binary can leave this thread's capture empty. The
//! typed refusal pins live next door in `unserved_queue_policies.rs`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use aion::{ActivityDispatch, ActivityDispatcher as _};
use aion_core::{ActivityId, RunId, WorkflowId};
use aion_server::worker::{ConnectedWorkerRegistry, HeartbeatTracker, WorkerActivityDispatcher};
use tracing::Level;
use tracing::field::{Field, Visit};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::{Context, SubscriberExt as _};
use tracing_subscriber::registry::Registry;

type TestError = Box<dyn std::error::Error>;

/// How long the exhibit observes the parked dispatch before reading the
/// captured tracing output. The pre-R1 seam re-emits its unreasoned INFO every
/// 500ms, so this window covers several park iterations.
const OBSERVATION_WINDOW: Duration = Duration::from_millis(2_500);

/// One captured `tracing` event: level, message, and rendered field values.
#[derive(Clone, Debug)]
struct CapturedEvent {
    level: Level,
    message: String,
    fields: BTreeMap<String, String>,
}

#[derive(Default)]
struct FieldVisitor {
    message: String,
    fields: BTreeMap<String, String>,
}

impl Visit for FieldVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.store(field.name(), value.to_owned());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.store(field.name(), format!("{value:?}"));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.store(field.name(), value.to_string());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.store(field.name(), value.to_string());
    }
}

impl FieldVisitor {
    fn store(&mut self, name: &str, value: String) {
        if name == "message" {
            self.message = value;
        } else {
            self.fields.insert(name.to_owned(), value);
        }
    }
}

/// Collects every event emitted on the subscriber's thread into a shared log.
struct CaptureLayer {
    events: Arc<Mutex<Vec<CapturedEvent>>>,
}

impl<S: tracing::Subscriber> Layer<S> for CaptureLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _context: Context<'_, S>) {
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        let captured = CapturedEvent {
            level: *event.metadata().level(),
            message: visitor.message,
            fields: visitor.fields,
        };
        if let Ok(mut events) = self.events.lock() {
            events.push(captured);
        }
    }
}

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
        advisory: false,
        labels: BTreeMap::new(),
    }
}

/// A dispatch parked on a queue nobody serves must say so: WARN level, the
/// taxonomy reason, and the age of the last compatible poller.
#[test]
fn parked_dispatch_on_an_unserved_queue_states_its_reason_at_warn() -> Result<(), TestError> {
    let events: Arc<Mutex<Vec<CapturedEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let registry = ConnectedWorkerRegistry::default();
    let dispatcher = WorkerActivityDispatcher::new(
        registry.clone(),
        "default",
        HeartbeatTracker::new(Duration::from_secs(5)),
    );

    let sink = Arc::clone(&events);
    let parked = std::thread::spawn(move || {
        let subscriber = Registry::default().with(CaptureLayer { events: sink });
        tracing::subscriber::with_default(subscriber, || {
            dispatcher.dispatch(dispatch_to_unserved_queue())
        })
    });

    std::thread::sleep(OBSERVATION_WINDOW);
    let captured = events.lock().map_err(|_| "capture log poisoned")?.clone();

    // Release the parked dispatch so the exhibit never leaves a wedged thread:
    // a worker arrives whose receiver is already gone, so the seam resolves the
    // dispatch with its existing closed-transport failure.
    let (worker_tx, worker_rx) = tokio::sync::mpsc::channel(1);
    drop(worker_rx);
    let registration = registry.register_namespaces(
        [String::from("default")],
        "nobody-serves-this",
        None,
        [String::from("greet")].iter(),
        worker_tx,
    )?;
    let outcome = parked
        .join()
        .map_err(|_| "parked dispatch thread panicked")?;
    assert!(
        outcome.is_err(),
        "the released dispatch must resolve, not return a result: {outcome:?}"
    );
    registration.deregister()?;

    let reasoned: Vec<&CapturedEvent> = captured
        .iter()
        .filter(|event| event.fields.contains_key("queue_service_reason"))
        .collect();
    assert!(
        !reasoned.is_empty(),
        "a dispatch parked on an unserved queue emitted no reasoned event; \
         captured instead: {captured:?}"
    );
    assert!(
        reasoned
            .iter()
            .any(|event| event.level == Level::WARN && !event.message.is_empty()),
        "the unserved park was never escalated to a WARN carrying a message: {reasoned:?}"
    );
    assert!(
        reasoned.iter().any(|event| event
            .fields
            .get("queue_service_reason")
            .is_some_and(|reason| reason == "NO_LIVE_POLLERS")),
        "the park did not classify the queue as NO_LIVE_POLLERS: {reasoned:?}"
    );
    assert!(
        reasoned
            .iter()
            .any(|event| event.fields.contains_key("last_compatible_poller_age_ms")),
        "the park carried no last-compatible-poller age: {reasoned:?}"
    );
    Ok(())
}
