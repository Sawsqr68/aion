//! Shared `tracing` capture for tests that assert on what the server SAID.
//!
//! Three test binaries need to read the process's own log to decide whether a
//! refusal, a park, or a diagnosis was ever spoken. Before this module each
//! carried its own byte-identical copy of the layer, and the copies had already
//! drifted in the only way a copy can drift silently: their doc comments
//! disagreed about whether the layer captures events from the whole process or
//! only the installing thread. Neither comment described the layer — that is a
//! property of how the subscriber is INSTALLED, which is the caller's choice —
//! so a reader taking either at face value would have drawn the wrong
//! conclusion about their own test's reach.
//!
//! # Installation is the caller's decision, and it is load-bearing
//!
//! The layer records every event it is handed. WHICH events it is handed
//! depends entirely on how the subscriber carrying it was installed:
//!
//! - `.try_init()` installs it GLOBALLY, so events from every thread and every
//!   spawned task reach it. Required whenever the code under test runs off the
//!   test's own thread — a tonic server in a `tokio::spawn`, or a worker on its
//!   own OS thread.
//! - `tracing::subscriber::with_default(...)` scopes it to the current thread.
//!   Enough only when the subject runs inline.
//!
//! Choosing the thread-scoped form for a subject that runs elsewhere yields a
//! test that captures nothing and reports it as "the server said nothing" —
//! a check that cannot distinguish its own failure from its subject's.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use tracing::Level;
use tracing::field::{Field, Visit};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

/// One captured `tracing` event: level, target, message, and field values.
#[derive(Clone, Debug)]
pub struct CapturedEvent {
    /// The event's severity.
    pub level: Level,
    /// The emitting module's target, e.g. `aion_server::worker::contracts`.
    pub target: String,
    /// The event's rendered `message` field.
    pub message: String,
    /// Every other field, rendered to a string and keyed by field name.
    pub fields: BTreeMap<String, String>,
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

/// Collects every event this subscriber is handed into a shared log.
///
/// See the module docs: the reach of "every event" is set by how the subscriber
/// is installed, not by this type.
pub struct CaptureLayer {
    /// The shared log this layer appends to.
    pub events: Arc<Mutex<Vec<CapturedEvent>>>,
}

impl<S: tracing::Subscriber> Layer<S> for CaptureLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _context: Context<'_, S>) {
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        let captured = CapturedEvent {
            level: *event.metadata().level(),
            target: event.metadata().target().to_owned(),
            message: visitor.message,
            fields: visitor.fields,
        };
        if let Ok(mut events) = self.events.lock() {
            events.push(captured);
        }
    }
}

/// A fresh shared log, ready to hand to [`CaptureLayer`].
pub fn capture_log() -> Arc<Mutex<Vec<CapturedEvent>>> {
    Arc::new(Mutex::new(Vec::new()))
}

/// Captured events rendered for a failure message.
///
/// A test asserting on ABSENCE must show what it DID see, or "nothing was
/// logged" and "the wrong thing was logged" produce the same report. But a raw
/// dump is no better: one refusal assertion drowned in ~400 `mio`/`h2`/`hyper`
/// TRACE lines is a diagnostic nobody can read.
///
/// So this renders every event from the code under test IN FULL, and reduces
/// everything else to a per-target count. **The counts are the load-bearing
/// part**, not politeness: they prove the subscriber was alive and receiving,
/// so an empty subject list means the server genuinely said nothing rather than
/// that the instrument was dead. Absence is only evidence when paired with
/// survival.
pub fn render(events: &[CapturedEvent]) -> String {
    if events.is_empty() {
        return "<NO EVENTS CAPTURED AT ALL — the subscriber never received \
                anything, so this proves nothing about the subject>"
            .to_owned();
    }
    let (subject, other): (Vec<_>, Vec<_>) = events
        .iter()
        .partition(|event| event.target.starts_with("aion"));

    let mut out = format!(
        "\n  {} events captured in total ({} from aion targets, {} from \
         elsewhere — the subscriber was demonstrably live)",
        events.len(),
        subject.len(),
        other.len()
    );
    if subject.is_empty() {
        out.push_str("\n  NO aion-target events at all.");
    }
    for event in &subject {
        out.push_str(&format!(
            "\n  [{}] {}: {} {:?}",
            event.level, event.target, event.message, event.fields
        ));
    }
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for event in &other {
        *counts.entry(event.target.as_str()).or_default() += 1;
    }
    for (target, count) in counts {
        out.push_str(&format!("\n  ({count}× {target}, not shown)"));
    }
    out
}
