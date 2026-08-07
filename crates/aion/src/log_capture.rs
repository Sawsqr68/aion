//! Thread-scoped capture of this crate's own tracing output, shared by every
//! test whose subject is what the operator is TOLD.
//!
//! One copy, deliberately — the same trade `store_faults` makes. A capture layer
//! is a rule about which events a test may see at all and about how a captured
//! event is interrogated, and a rule known in two places with nothing forcing
//! agreement has already drifted: the second copy is where one call site starts
//! matching a level with a trailing space while the other matches it with a
//! delimiter, and a test that should have gone red goes green on a rendering
//! difference.
//!
//! "One copy" is a measured claim about THIS crate, and it was false when it was
//! first written: `durability/recorder.rs` still carried a third capture — a
//! `Vec<u8>` writer behind `fmt().with_max_level(WARN)`, interrogated with
//! `output.contains(..)` over the joined rendering — which is exactly the shape
//! condemned two paragraphs down, and which never installed the floor below.
//! It was migrated to this module rather than documented, because the cure for
//! a rule known in two places is SUBTRACTION and a note is not subtraction.
//! Every `tracing` capture inside `aion` now goes through [`LogCapture`]; if a
//! fourth appears, delete it, do not annotate it.
//!
//! 🔴 A captured event is a LEVEL AND A LIST OF FIELDS, not a joined string, and
//! that is the point rather than a convenience. An earlier revision stored the
//! rendering `LEVEL|field=value|…` and left callers to recover a field by
//! splitting on `|` — which is wrong the moment a value contains one, and a
//! store's error text is exactly the value most likely to. The delimiter now
//! exists only in [`CapturedEvent`]'s `Display`, which no assertion parses.
//!
//! 🔴 THREAD-SCOPED ON PURPOSE. The CAPTURE is installed with
//! [`tracing::subscriber::with_default`] or [`tracing::subscriber::set_default`],
//! never as the global default: unit tests run in parallel on one process, and a
//! capturing global subscriber would let any other test's events land in this
//! test's log — and let this test's assertions be satisfied by an emission it
//! did not cause.
//!
//! Thread scope cuts BOTH ways, and only one direction was written down here
//! until 2026-08-06. Outward: an emission made on a DIFFERENT thread (a spawned
//! task, an executor-owned runtime) is invisible here, so a test whose subject
//! is such an emission must drive the emitting code on its own thread rather
//! than through the production spawn. Inward — the direction that actually bit —
//! ANOTHER THREAD CAN MAKE THIS THREAD'S EMISSIONS INVISIBLE, and that is what
//! [`InterestFloor`] exists to prevent.
//!
//! 🔴 WHY A SILENT GLOBAL SUBSCRIBER IS INSTALLED ANYWAY. `tracing` caches one
//! `Interest` per callsite for the whole PROCESS, while a subscriber installed
//! with `set_default` is scoped to one THREAD. `tracing-core`'s
//! `rebuild_callsite_interest` folds the interest of every registered dispatcher
//! and ends `interest.unwrap_or_else(Interest::never)` — so a callsite that
//! registers while the dispatcher registry is EMPTY caches `never` and the
//! `warn!` macro short-circuits on every thread from then on, this one included.
//! `DefaultCallsite::register` takes its snapshot of the registry before it
//! stores the result, so a sibling test hitting a callsite for the first time
//! can overwrite an `always` this module's own installation had just written.
//! Measured 2026-08-06: `at_the_ceiling_every_failing_attempt_states_itself`
//! captured NOTHING from `lifecycle::completion_retry` in 5 of 5 runs under
//! `cargo test -p aion-rs --lib -- lifecycle::completion`, green alone and green
//! under `--test-threads=1`, with `LevelFilter::current()` reading `TRACE` and a
//! canary at a callsite in the test's own file captured normally.
//!
//! [`InterestFloor`] closes it by SUBTRACTION rather than by a second rule: it
//! is registered for the life of the process, so the registry is never empty and
//! no fold can ever reach the `never` default. It answers
//! [`Interest::sometimes`] for every callsite — which survives the fold in both
//! orders — and `false` from `enabled`, so it decides nothing and captures
//! nothing. Every event is then referred to whatever subscriber the EMITTING
//! thread has, which is exactly the thread scoping above.
//!
//! ⚠️ One window is outside this module's reach: a callsite whose one and only
//! registration began before the floor was installed and stores its result after
//! [`LogCapture::new`] has rebuilt the cache. That is a race inside
//! `tracing-core`, it can happen at most once per callsite per process, and it
//! fails LOUD — a capture that comes back empty asserts, it does not pass.

use std::sync::{Arc, Mutex};

use tracing::field::{Field, Visit};
use tracing::subscriber::Interest;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::{Context, SubscriberExt as _};
use tracing_subscriber::registry::Registry;

/// Renders one event's fields, enough to pin what the operator is told and
/// which identity is named.
#[derive(Default)]
struct EventFields(Vec<String>);

impl Visit for EventFields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0.push(format!("{}={value:?}", field.name()));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.push(format!("{}={value}", field.name()));
    }
}

/// One captured event: its level and its fields, unjoined.
///
/// Nothing here is a parsed rendering. A field name cannot contain `=` — tracing
/// field names are identifiers — so [`Self::field`] recovers a value by exact
/// prefix over the field list, and no assertion depends on a separator that a
/// value could itself contain.
#[derive(Clone, Debug)]
pub(crate) struct CapturedEvent {
    level: String,
    fields: Vec<String>,
}

impl CapturedEvent {
    /// The value recorded under `name`, or `None` if the event has no such
    /// field.
    pub(crate) fn field(&self, name: &str) -> Option<&str> {
        let prefix = format!("{name}=");
        self.fields
            .iter()
            .find_map(|field| field.strip_prefix(prefix.as_str()))
    }

    /// Whether any recorded value — including the message, which tracing records
    /// as the field `message` — contains `needle`.
    pub(crate) fn mentions(&self, needle: &str) -> bool {
        self.fields.iter().any(|field| field.contains(needle))
    }
}

impl std::fmt::Display for CapturedEvent {
    /// For assertion messages only. Nothing reads this back.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}|{}", self.level, self.fields.join("|"))
    }
}

/// Collects this thread's events.
struct CaptureLayer(Arc<Mutex<Vec<CapturedEvent>>>);

impl<S: tracing::Subscriber> Layer<S> for CaptureLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _context: Context<'_, S>) {
        let mut fields = EventFields::default();
        event.record(&mut fields);
        let captured_event = CapturedEvent {
            level: event.metadata().level().to_string(),
            fields: fields.0,
        };
        if let Ok(mut captured) = self.0.lock() {
            captured.push(captured_event);
        }
    }
}

/// The captured log, readable after the subscriber has been installed and the
/// code under test has run.
#[derive(Clone)]
pub(crate) struct LogCapture(Arc<Mutex<Vec<CapturedEvent>>>);

/// A dispatcher that decides nothing, captures nothing, and exists only so the
/// callsite-interest fold described in this module's header can never run over
/// an empty registry.
///
/// `register_callsite` answers [`Interest::sometimes`] rather than
/// [`Interest::always`] deliberately: `always` would let the macro skip
/// `enabled` and dispatch every event process-wide, while `sometimes` refers
/// each event back to the EMITTING thread's own subscriber — which is the whole
/// point of the thread scoping. `enabled` answers `false`, so on a thread with
/// no capture installed the event is dropped here and reaches nothing.
struct InterestFloor;

impl tracing::Subscriber for InterestFloor {
    fn register_callsite(&self, _metadata: &'static tracing::Metadata<'static>) -> Interest {
        Interest::sometimes()
    }

    fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
        false
    }

    /// `None`, stated rather than inherited, because it is a PROCESS-WIDE side
    /// effect of installing this floor and the one thing in this module that
    /// reaches beyond the callsite fold.
    ///
    /// `Callsites::rebuild_interest` reads each dispatcher's hint as
    /// `unwrap_or(LevelFilter::TRACE)` (tracing-core-0.1.36 `callsite.rs:412`)
    /// and takes the MAX, so `None` pins `LevelFilter::current()` at `TRACE`
    /// for the whole test binary: every `trace!` and `debug!` in the crate now
    /// reaches callsite interest and a per-event `enabled` call it would
    /// otherwise have skipped. That cost is accepted deliberately.
    ///
    /// 🔴 The alternative is worse, and it is worse in the direction this
    /// module exists to prevent. Returning `Some(LevelFilter::OFF)` would be
    /// truthful about this subscriber — it does answer `false` to everything —
    /// but the max is taken across REGISTERED dispatchers, and between
    /// [`ensure_interest_floor`] and the caller's own `set_default` the floor is
    /// the only one registered. The global max would sit at `OFF` for that
    /// window, and any emission from another thread inside it would be dropped
    /// before interest was ever consulted: a NEW silent-loss window, opened by
    /// the fix for a silent-loss window. `None` cannot lower anything.
    fn max_level_hint(&self) -> Option<tracing::level_filters::LevelFilter> {
        None
    }

    /// Never reached: `enabled` refuses every callsite, and `tracing` does not
    /// open a span this subscriber declined. The id is still a valid one rather
    /// than a placeholder, because returning an invalid id would be a contract
    /// violation waiting for the first caller that does reach it.
    fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::Id {
        tracing::Id::from_u64(1)
    }

    fn record(&self, _span: &tracing::Id, _values: &tracing::span::Record<'_>) {}

    fn record_follows_from(&self, _span: &tracing::Id, _follows: &tracing::Id) {}

    fn event(&self, _event: &tracing::Event<'_>) {}

    fn enter(&self, _span: &tracing::Id) {}

    fn exit(&self, _span: &tracing::Id) {}
}

/// Installs [`InterestFloor`] once per process, remembering how it went.
static FLOOR: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();

/// Put the floor under the callsite-interest fold, and rebuild the cache so a
/// callsite already poisoned by an empty fold is healed.
///
/// The result is remembered rather than recomputed: `set_global_default` may be
/// called at most once, so a second attempt would fail for a reason that says
/// nothing about whether the floor is in place. A failure is REPORTED rather
/// than ignored — if some other global default owns the process, this module's
/// central claim does not hold and a caller must not be told it does.
fn ensure_interest_floor() -> Result<(), String> {
    FLOOR
        .get_or_init(|| {
            tracing::subscriber::set_global_default(InterestFloor).map_err(|error| {
                format!("captured-log interest floor could not be installed: {error}")
            })?;
            tracing::callsite::rebuild_interest_cache();
            Ok(())
        })
        .clone()
}

impl LogCapture {
    /// A fresh log and the subscriber that fills it.
    ///
    /// The subscriber is returned separately rather than installed here because
    /// the two installation shapes a test needs — `with_default` around a
    /// closure, `set_default` returning a guard across `await` points — differ
    /// in lifetime, and picking one for the caller would push half the callers
    /// into working around it.
    ///
    /// Fallible because [`ensure_interest_floor`] is: a caller that got a
    /// capture back without the floor under it would be holding an instrument
    /// that can silently record nothing, and that is the exact failure this
    /// module was changed to end.
    pub(crate) fn new() -> Result<(Self, impl tracing::Subscriber + Send + Sync + 'static), String>
    {
        ensure_interest_floor()?;
        let events = Arc::new(Mutex::new(Vec::new()));
        let subscriber = Registry::default().with(CaptureLayer(Arc::clone(&events)));
        // Run once more per capture rather than only once per process. The
        // floor makes every fold answer `sometimes`, so this heals any callsite
        // a race had left at `never` since the last capture — and the fold can
        // no longer produce `never` at all, so nothing re-poisons it after.
        tracing::callsite::rebuild_interest_cache();
        Ok((Self(events), subscriber))
    }

    /// Every event captured so far, in emission order.
    ///
    /// The poisoned-lock case is mapped to a typed error rather than unwrapped:
    /// a panic inside a `with_default` closure poisons this mutex, and reporting
    /// that as a failure of the code under test would send the reader to the
    /// wrong place.
    pub(crate) fn events(&self) -> Result<Vec<CapturedEvent>, String> {
        self.0
            .lock()
            .map(|captured| captured.clone())
            .map_err(|error| format!("captured log mutex poisoned: {error}"))
    }

    /// Every captured event at one level, e.g. `"WARN"`.
    ///
    /// Compared against the recorded level itself, so `"WARN"` can never also
    /// select an `INFO` event whose message happens to contain the word.
    pub(crate) fn at_level(&self, level: &str) -> Result<Vec<CapturedEvent>, String> {
        Ok(self
            .events()?
            .into_iter()
            .filter(|event| event.level == level)
            .collect())
    }
}
