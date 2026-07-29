//! Hit B pin: an invisible refusal is a refusal that never happened.
//!
//! Live, 2026-07-29: a worker whose `.v4` contract field-mismatched the deployed
//! one was refused on EVERY dial. The server logged nothing on the `Rejected`
//! ack, and the worker's redial loop dropped the error value unlogged and slept.
//! Both sides mute; a process sample showed 2674 of 2675 samples asleep in
//! backoff. From the outside the system looked merely idle, and the actual
//! cause — a named field, in a named schema, of a named package version — was
//! sitting in an ack nobody printed.
//!
//! This pin stands up a REAL liminal listener with a REAL deployed AWL contract
//! and dials it with a REAL worker whose activity input type is incompatible,
//! then asserts on what BOTH sides actually logged. It owns its whole test
//! binary and installs a GLOBAL tracing subscriber deliberately: the worker runs
//! on its own OS thread inside the production `serve_with_redial`, so a
//! thread-scoped subscriber would capture the server's half and miss the
//! worker's — which is the half that was silent in the incident.
#![cfg(feature = "liminal-transport")]

use std::collections::BTreeMap;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use aion_package::{ExtractionLimits, Package};
use aion_server::worker::{ConnectedWorkerRegistry, LiminalConnectionNotifier};
use aion_worker::{ActivityRegistry, RedialTiming, WorkerConfig, serve_with_redial};
use liminal_server::config::{ChannelDef, ServerConfig};
use liminal_server::server::connection::ConnectionSupervisor;
use liminal_server::server::listener::ServerListener;
use serde::{Deserialize, Serialize};
use tracing::Level;
use tracing::field::{Field, Visit};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::{Context, SubscriberExt as _};
use tracing_subscriber::registry::Registry;
use tracing_subscriber::util::SubscriberInitExt as _;

type TestError = Box<dyn Error + Send + Sync>;
type TestResult = Result<(), TestError>;

/// The deployed contract: `charge` takes an INTEGER amount.
const SOURCE: &str = r"//! Dead-man-switch refusal fixture.
workflow refusal_pin
  input amount: Int
  outcome completed: type Result, route success

type Result { approved: Bool }

worker payments
  action charge(amount: Int) -> Result

step run
  charge(amount: amount) -> result
  route completed(approved: result.approved)
";

/// The worker's INCOMPATIBLE surface: `charge` takes a STRING amount. This is
/// the exact class of drift the live incident hit.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
struct WrongChargeInput {
    amount: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
struct ChargeOutput {
    approved: bool,
}

fn test_error(message: impl std::fmt::Display) -> TestError {
    message.to_string().into()
}

/// THE HIT B GATE. A contract-mismatched worker must name its own refusal in
/// its own log on the FIRST dial, and the server must name the refusal it sent.
///
/// Red on main: the server returns the rejection through the ack and logs
/// nothing at all, and the worker's redial loop swallows the error value before
/// sleeping — so neither assertion below finds anything to read.
#[tokio::test(flavor = "multi_thread")]
async fn a_refused_registration_is_named_in_both_logs_on_the_first_dial() -> TestResult {
    let events: Arc<Mutex<Vec<CapturedEvent>>> = Arc::new(Mutex::new(Vec::new()));
    Registry::default()
        .with(CaptureLayer {
            events: Arc::clone(&events),
        })
        .try_init()
        .map_err(test_error)?;

    let server = RefusingServer::start().await?;
    let address = server.address;
    let worker = WorkerThread::spawn(address)?;

    // Observe dial attempts until BOTH halves have spoken, or give the incident
    // its full chance to stay silent. The redial backoff is deliberately tight
    // here so this is a couple of seconds, not the live incident's minutes.
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let both_spoke = {
            let captured = events.lock().map_err(|_| "capture log poisoned")?;
            captured.iter().any(|event| {
                event
                    .message
                    .contains("REFUSED liminal worker registration")
            }) && captured
                .iter()
                .any(|event| event.message.contains("liminal worker dial failed"))
        };
        if both_spoke {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    worker.stop();
    server.shutdown()?;
    let captured = events.lock().map_err(|_| "capture log poisoned")?.clone();

    // ---- the SERVER half: the refusal it sent is the refusal it logged ----
    let refusals: Vec<&CapturedEvent> = captured
        .iter()
        .filter(|event| {
            event
                .message
                .contains("REFUSED liminal worker registration")
        })
        .collect();
    let refusal = refusals.first().ok_or_else(|| {
        test_error(format!(
            "the server refused every dial and logged NONE of them; captured: {}",
            render(&captured)
        ))
    })?;
    assert_eq!(
        refusal.level,
        Level::WARN,
        "a refusal an operator must act on belongs at WARN: {refusal:?}"
    );
    let reason = refusal.fields.get("reason").map_or("", String::as_str);
    assert!(
        reason.contains("WORKER_CONTRACT_MISMATCH"),
        "the server's refusal must carry the STRUCTURED reason, not just 'rejected': {refusal:?}"
    );
    assert!(
        reason.contains("input_schema.properties.amount.type"),
        "the field-level detail exists at the refusal site and must be surfaced: {refusal:?}"
    );
    assert_eq!(
        refusal.fields.get("task_queue").map(String::as_str),
        Some("payments"),
        "the refusal must name the queue: {refusal:?}"
    );
    assert_eq!(
        refusal.fields.get("identity").map(String::as_str),
        Some("refusal-pin-worker"),
        "the refusal must name the worker build: {refusal:?}"
    );

    // ---- the WORKER half: it names its own refusal, on its first dial ----
    let dials: Vec<&CapturedEvent> = captured
        .iter()
        .filter(|event| event.message.contains("liminal worker dial failed"))
        .collect();
    let dial = dials.first().ok_or_else(|| {
        test_error(format!(
            "the worker was refused on every dial and said NOTHING; captured: {}",
            render(&captured)
        ))
    })?;
    assert_eq!(dial.level, Level::WARN, "{dial:?}");
    let error = dial.fields.get("error").map_or("", String::as_str);
    assert!(
        error.contains("WORKER_CONTRACT_MISMATCH"),
        "the worker must print the server's structured refusal reason verbatim, so a \
         mismatched build is diagnosable from the worker's own log: {dial:?}"
    );
    assert_eq!(
        dial.fields.get("attempt").map(String::as_str),
        Some("1"),
        "the FIRST dial is already logged — not the tenth, not after a threshold: {dial:?}"
    );
    assert_eq!(
        dial.fields.get("address").map(String::as_str),
        Some(address.to_string().as_str()),
        "the dial log must name the endpoint it failed against: {dial:?}"
    );
    Ok(())
}

/// Renders captured events for an assertion message.
fn render(events: &[CapturedEvent]) -> String {
    if events.is_empty() {
        return "<nothing at all>".to_owned();
    }
    events
        .iter()
        .map(|event| format!("[{}] {} {:?}", event.level, event.message, event.fields))
        .collect::<Vec<_>>()
        .join(" | ")
}

/// A liminal listener whose contract catalog refuses the test worker.
struct RefusingServer {
    listener: Option<ServerListener>,
    address: SocketAddr,
}

impl RefusingServer {
    async fn start() -> Result<Self, TestError> {
        let root = tempfile::tempdir().map_err(test_error)?;
        let prepared =
            aion_awl_package::compile_and_assemble_awl(SOURCE, root.path()).map_err(test_error)?;
        let package = Package::load_from_bytes(prepared.archive, ExtractionLimits::unbounded())
            .map_err(test_error)?;
        let engine = aion::EngineBuilder::new()
            .store(aion_store::InMemoryStore::default())
            .in_memory_visibility()
            .build()
            .await
            .map_err(test_error)?;
        engine.load_package(package).await.map_err(test_error)?;

        let config = ServerConfig {
            listen_address: "127.0.0.1:0".parse().map_err(test_error)?,
            health_listen_address: reserve_loopback_port()?,
            channels: Vec::<ChannelDef>::new(),
            routing_rules: Vec::new(),
            persistence_path: None,
            cluster: None,
            auth: None,
            drain_timeout_ms: 30_000,
            services: liminal_server::config::ServicesConfig::default(),
            limits: liminal_server::config::LimitsConfig::default(),
            websocket: None,
            participant: None,
        };
        let notifier = Arc::new(
            LiminalConnectionNotifier::new(ConnectedWorkerRegistry::default())
                .with_contract_catalog(Arc::new(engine)),
        );
        let supervisor = {
            use liminal_server::server::connection::LiminalConnectionServices;
            let services =
                Arc::new(LiminalConnectionServices::from_config(&config).map_err(test_error)?);
            ConnectionSupervisor::with_services_and_notifier(services, notifier.clone())
                .map_err(test_error)?
        };
        if !notifier.bind_supervisor(supervisor.clone()) {
            return Err(test_error("notifier supervisor was already bound"));
        }
        let listener = ServerListener::bind(&config, supervisor).map_err(test_error)?;
        let address = listener.local_addr();
        Ok(Self {
            listener: Some(listener),
            address,
        })
    }

    fn shutdown(mut self) -> TestResult {
        if let Some(listener) = self.listener.take() {
            listener.shutdown().map_err(test_error)?;
        }
        Ok(())
    }
}

/// The mismatched worker, driven through the PRODUCTION redial entry point.
struct WorkerThread {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl WorkerThread {
    fn spawn(address: SocketAddr) -> Result<Self, TestError> {
        let stop = Arc::new(AtomicBool::new(false));
        let config = WorkerConfig::builder()
            .endpoint("unused-direct-address")
            .namespace("default")
            .task_queue("payments")
            .identity("refusal-pin-worker")
            .max_concurrency(1)
            .reconnect_initial_backoff(Duration::from_millis(20))
            .reconnect_max_backoff(Duration::from_millis(50))
            .reconnect_max_attempts(3)
            .build()
            .map_err(test_error)?;
        let registry = Arc::new(
            ActivityRegistry::new()
                // The contract-CARRYING registration, so the worker advertises
                // a real schema and the refusal is the field-level schema
                // mismatch the incident hit — not a missing declaration.
                .register_activity_with_contract("charge", |input: WrongChargeInput, _context| {
                    Box::pin(async move {
                        Ok(ChargeOutput {
                            approved: !input.amount.is_empty(),
                        })
                    })
                })
                .map_err(test_error)?,
        );
        let thread_stop = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            // A refused worker retries for ever in production (the incident's
            // shape); the pin stops it once it has said its piece.
            if let Err(error) = serve_with_redial(
                vec![address.to_string()],
                &config,
                &registry,
                RedialTiming::new(Duration::from_millis(20), Duration::from_millis(50)),
                &thread_stop,
                None,
                || {},
            ) {
                tracing::info!(%error, "refusal-pin worker gave up");
            }
        });
        Ok(Self {
            stop,
            handle: Some(handle),
        })
    }

    fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn reserve_loopback_port() -> Result<SocketAddr, TestError> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(test_error)?;
    let address = listener.local_addr().map_err(test_error)?;
    drop(listener);
    Ok(address)
}

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

/// Collects every event emitted anywhere in the process into a shared log.
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
