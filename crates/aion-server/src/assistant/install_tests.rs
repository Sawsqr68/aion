//! Proofs about what a boot install does to a catalog.
//!
//! The three cases are the whole rule: an empty catalog is claimed, the
//! embedded version already routed is left alone, and a catalog routed
//! somewhere else is NOT re-pointed. The third is the one that matters — it is
//! the property that stops a restart from undoing an operator's rollback, and
//! it is invisible to a test that only ever boots against an empty store.
//!
//! The QUEUE cases sit on top of that rule rather than beside it (#200). A
//! store carried across the assistant's move off `default` is a deferred
//! install of the third kind — and the one an operator must be told about,
//! because the pending cut changes which queue a worker has to serve. So the
//! deferral is proved twice over: that nothing moved, and that the boot said
//! what would move if the operator deploys.

use std::sync::Arc;

use aion::{Engine, EngineBuilder};
use aion_store::{EventStore, InMemoryStore};

use super::super::document::EMBEDDED_ASSISTANT_DOCUMENT;
use super::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

async fn engine() -> Result<Arc<Engine>, Box<dyn std::error::Error>> {
    let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
    Ok(Arc::new(
        EngineBuilder::new()
            .store_arc(store)
            .in_memory_visibility()
            .scheduler_threads(1)
            .build()
            .await?,
    ))
}

/// A catalog holding no assistant version is claimed: the embedded document is
/// loaded and takes the route, so a fresh home has a working assistant with no
/// operator step.
#[tokio::test]
async fn a_fresh_catalog_gets_the_embedded_assistant_installed_and_routed() -> TestResult {
    let engine = engine().await?;
    let embedded = EmbeddedAssistant::load()?;

    let outcome = install_embedded_assistant(
        engine.as_ref(),
        WorkerListenerAdvice::without_boot_context(),
    )
    .await;
    assert_eq!(
        outcome,
        AssistantInstall::Installed {
            workflow_type: embedded.workflow_type().to_owned(),
            content_hash: embedded.content_hash().to_string(),
            task_queue: embedded.task_queue().to_owned(),
        },
        "a fresh catalog must be claimed"
    );

    let routed: Vec<_> = engine
        .list_workflow_versions()?
        .into_iter()
        .filter(|version| version.workflow_type == embedded.workflow_type() && version.route_active)
        .collect();
    assert_eq!(routed.len(), 1, "exactly one version holds the route");
    assert_eq!(
        routed[0].content_hash.to_string(),
        embedded.content_hash().to_string()
    );
    Ok(())
}

/// A second install over the same catalog is a no-op that says so — the restart
/// case on a home this binary already installed into.
#[tokio::test]
async fn a_second_install_reports_already_current_and_changes_nothing() -> TestResult {
    let engine = engine().await?;
    let embedded = EmbeddedAssistant::load()?;

    let first = install_embedded_assistant(
        engine.as_ref(),
        WorkerListenerAdvice::without_boot_context(),
    )
    .await;
    assert!(matches!(first, AssistantInstall::Installed { .. }));
    let before = engine.list_workflow_versions()?;

    let second = install_embedded_assistant(
        engine.as_ref(),
        WorkerListenerAdvice::without_boot_context(),
    )
    .await;
    assert_eq!(
        second,
        AssistantInstall::AlreadyCurrent {
            workflow_type: embedded.workflow_type().to_owned(),
            content_hash: embedded.content_hash().to_string(),
            task_queue: embedded.task_queue().to_owned(),
        }
    );
    let after = engine.list_workflow_versions()?;
    assert_eq!(
        before.len(),
        after.len(),
        "an already-current install must load nothing"
    );
    Ok(())
}

/// THE ONE THAT MATTERS. A catalog whose assistant route points at a DIFFERENT
/// version is left untouched, and the outcome names both hashes.
///
/// The stand-in for the operator's rollback is a genuinely different document
/// under the same workflow type: a copy of the embedded document with a
/// changed constant, which compiles to the same type and a different hash. If
/// the install ever loaded unconditionally, this route would flip and the test
/// would fail — which is exactly the restart-undoes-the-rollback bug.
#[tokio::test]
async fn an_install_never_repoints_a_route_it_did_not_place() -> TestResult {
    let engine = engine().await?;
    let embedded = EmbeddedAssistant::load()?;

    let other: &'static str = Box::leak(
        EMBEDDED_ASSISTANT_DOCUMENT
            .replace(
                "It is a scratch git workspace",
                "It is a scratch git workspace (operator build)",
            )
            .into_boxed_str(),
    );
    let other = EmbeddedAssistant::from_source(other)?;
    assert_eq!(
        other.workflow_type(),
        embedded.workflow_type(),
        "the stand-in must be the same workflow type"
    );
    assert_ne!(
        other.content_hash(),
        embedded.content_hash(),
        "the stand-in must be a different version, or this test cannot distinguish anything"
    );
    engine.load_package(other.package().clone()).await?;

    let outcome = install_embedded_assistant(
        engine.as_ref(),
        WorkerListenerAdvice::without_boot_context(),
    )
    .await;
    assert_eq!(
        outcome,
        AssistantInstall::Deferred {
            workflow_type: embedded.workflow_type().to_owned(),
            embedded_hash: embedded.content_hash().to_string(),
            routed_hash: Some(other.content_hash().to_string()),
            embedded_queue: embedded.task_queue().to_owned(),
            routed_queues: RoutedQueues::Declared(vec![other.task_queue().to_owned()]),
        }
    );
    assert!(
        !outcome.defers_a_queue_move(),
        "the stand-in serves the same queue, so this deferral is a version cut and not a queue \
         move — reporting one here would cry wolf on every rollback"
    );

    let versions = engine.list_workflow_versions()?;
    let routed: Vec<_> = versions
        .iter()
        .filter(|version| version.workflow_type == embedded.workflow_type() && version.route_active)
        .collect();
    assert_eq!(routed.len(), 1);
    assert_eq!(
        routed[0].content_hash.to_string(),
        other.content_hash().to_string(),
        "the operator's routed version must survive the boot install"
    );
    assert!(
        !versions.iter().any(|version| {
            version.workflow_type == embedded.workflow_type()
                && version.content_hash.to_string() == embedded.content_hash().to_string()
        }),
        "a deferred install must not load the embedded version either — loading is what \
         re-points the route"
    );
    Ok(())
}

/// The literal prior shape (#200): the embedded document with its `worker`
/// block put back on `default`, compiled and loaded as a PACKAGE — the
/// embedded loader now refuses a document on `default`, and it is right to,
/// but the store being modelled predates that refusal and holds one anyway.
/// The hash is proven distinct from the shipped version, so an install over
/// this fixture can never report `AlreadyCurrent`.
fn prior_package_on_default() -> Result<(aion_package::Package, String), Box<dyn std::error::Error>>
{
    let embedded = EmbeddedAssistant::load()?;
    let on_default =
        EMBEDDED_ASSISTANT_DOCUMENT.replace("\nworker assistant\n", "\nworker default\n");
    assert_ne!(
        on_default, EMBEDDED_ASSISTANT_DOCUMENT,
        "the fixture must differ from the shipped document, or it fixes nothing in place"
    );
    let assembled = aion_awl_package::compile_and_assemble_awl(
        &on_default,
        std::path::Path::new("<existing-store-fixture-has-no-schema-directory>"),
        "assistant.awl",
    )?;
    let package = aion_package::Package::load_from_bytes(
        &assembled.archive,
        aion_package::ExtractionLimits::unbounded(),
    )?;
    let hash = package.content_hash().to_string();
    assert_ne!(
        hash,
        embedded.content_hash().to_string(),
        "the fixture must be a different version, or the install would report AlreadyCurrent"
    );
    Ok((package, hash))
}

/// THE EXISTING-STORE CASE (#200). A catalog whose assistant is routed on
/// `default` — the queue the built-in assistant used to ship on — is NOT moved
/// by a boot, and the boot says so, naming the queue it is on and the queue the
/// binary would put it on.
///
/// The stand-in is the embedded document with its `worker` block put back on
/// `default`: the literal prior shape, so what is loaded here is the artifact
/// an operator's existing home actually holds, not an approximation of it.
/// Every assertion is about what survived — the route, the hash, and the queue
/// the live sessions dispatch on.
#[tokio::test]
async fn an_existing_store_on_the_old_queue_is_announced_and_never_moved() -> TestResult {
    let engine = engine().await?;
    let embedded = EmbeddedAssistant::load()?;

    let (prior, prior_hash) = prior_package_on_default()?;
    engine.load_package(prior).await?;

    let outcome = install_embedded_assistant(
        engine.as_ref(),
        WorkerListenerAdvice::without_boot_context(),
    )
    .await;
    assert_eq!(
        outcome,
        AssistantInstall::Deferred {
            workflow_type: embedded.workflow_type().to_owned(),
            embedded_hash: embedded.content_hash().to_string(),
            routed_hash: Some(prior_hash.clone()),
            embedded_queue: embedded.task_queue().to_owned(),
            routed_queues: RoutedQueues::Declared(vec![String::from(
                aion_core::DEFAULT_TASK_QUEUE
            )]),
        },
        "the outcome must name the queue the store is on AND the queue the binary declares"
    );
    assert!(
        outcome.defers_a_queue_move(),
        "an assistant routed on `default` against an embedded assistant on `{}` IS a pending \
         queue move, and a boot that does not say so has moved it silently or hidden it",
        embedded.task_queue()
    );

    // NOTHING MOVED. The operator's version still holds the route, the embedded
    // version was not loaded, and the queue the live sessions dispatch on is
    // still the one they were started against.
    let versions = engine.list_workflow_versions()?;
    let routed: Vec<_> = versions
        .iter()
        .filter(|version| version.workflow_type == embedded.workflow_type() && version.route_active)
        .collect();
    assert_eq!(routed.len(), 1);
    assert_eq!(routed[0].content_hash.to_string(), prior_hash);
    assert!(
        !versions.iter().any(|version| {
            version.workflow_type == embedded.workflow_type()
                && version.content_hash.to_string() == embedded.content_hash().to_string()
        }),
        "a deferred install must not load the embedded version — loading is what re-points"
    );
    assert!(
        engine
            .declared_task_queues()?
            .declares(aion_core::DEFAULT_TASK_QUEUE),
        "the catalog must still declare `default`: the assistant this store holds is served \
         there, and a boot that silently emptied that queue would strand the live sessions"
    );
    Ok(())
}

/// The outcome labels are stable strings, since logs and operators read them.
#[test]
fn outcome_labels_are_distinct() {
    let installed = AssistantInstall::Installed {
        workflow_type: String::from("assistant"),
        content_hash: String::from("hash"),
        task_queue: String::from("assistant"),
    };
    let current = AssistantInstall::AlreadyCurrent {
        workflow_type: String::from("assistant"),
        content_hash: String::from("hash"),
        task_queue: String::from("assistant"),
    };
    let deferred = AssistantInstall::Deferred {
        workflow_type: String::from("assistant"),
        embedded_hash: String::from("hash"),
        routed_hash: None,
        embedded_queue: String::from("assistant"),
        routed_queues: RoutedQueues::NoRoutedVersion,
    };
    let failed = AssistantInstall::Failed {
        reason: String::from("why"),
    };
    let labels = [
        installed.outcome(),
        current.outcome(),
        deferred.outcome(),
        failed.outcome(),
    ];
    let unique: std::collections::BTreeSet<&str> = labels.iter().copied().collect();
    assert_eq!(unique.len(), labels.len(), "labels must be distinguishable");
}

/// A tracing writer that keeps what was written, so a log line can be read back
/// as text.
#[derive(Clone, Default)]
struct Captured(Arc<std::sync::Mutex<Vec<u8>>>);

impl Captured {
    /// The captured text so far. A poisoned lock is recovered rather than
    /// propagated: this is a test's own buffer, and refusing to read it would
    /// report a missing log line as a missing log line.
    fn text(&self) -> String {
        let bytes = match self.0.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        };
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

impl std::io::Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self.0.lock() {
            Ok(mut guard) => guard.extend_from_slice(buf),
            Err(poisoned) => poisoned.into_inner().extend_from_slice(buf),
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'writer> tracing_subscriber::fmt::MakeWriter<'writer> for Captured {
    type Writer = Self;

    fn make_writer(&'writer self) -> Self::Writer {
        self.clone()
    }
}

/// Runs `emit` under a subscriber that captures everything, and returns it.
fn captured_log(emit: impl FnOnce()) -> String {
    let buffer = Captured::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(buffer.clone())
        .with_ansi(false)
        .with_max_level(tracing::Level::TRACE)
        .finish();
    tracing::subscriber::with_default(subscriber, emit);
    buffer.text()
}

/// THE ANNOUNCEMENT. A pending queue move is stated in the boot log, naming the
/// queue the store is on, the queue the binary declares, and why.
///
/// Read from the emitted line, not from the outcome value: the outcome is what
/// the server knows and the log is what the operator gets, and a boot that
/// knows about the move but does not say it is exactly the silent migration
/// this refuses to be. The control below shares every field except the routed
/// queue, so the difference in what is said is caused by the queue alone.
#[test]
fn a_pending_queue_move_is_announced_naming_both_queues() {
    let moving = AssistantInstall::Deferred {
        workflow_type: String::from("assistant"),
        embedded_hash: String::from("embedded-hash"),
        routed_hash: Some(String::from("routed-hash")),
        embedded_queue: String::from("assistant"),
        routed_queues: RoutedQueues::Declared(vec![String::from(aion_core::DEFAULT_TASK_QUEUE)]),
    };
    let line = captured_log(|| {
        log_outcome(
            &moving,
            WorkerListenerAdvice {
                listener: Some("127.0.0.1:50061"),
                config_hint: "add `liminal_listen_address` to `[outbox]` in the test config",
            },
        );
    });
    assert!(
        line.contains("QUEUE MOVE PENDING"),
        "the move must be announced in words an operator can grep for; got: {line}"
    );
    assert!(
        line.contains("from_task_queue=default"),
        "the line must name the queue the store is on; got: {line}"
    );
    assert!(
        line.contains("to_task_queue=assistant"),
        "the line must name the queue the binary declares; got: {line}"
    );
    assert!(
        line.contains("NOTHING WAS MOVED"),
        "the line must say that this boot did not perform the move; got: {line}"
    );
    assert!(
        line.contains("#200"),
        "the line must say WHY the queue changed; got: {line}"
    );
    assert!(
        line.contains("WARN"),
        "an announcement below WARN is one an operator's own filters can drop; got: {line}"
    );
    assert!(
        line.contains(
            "aion worker agent assistant.awl --liminal-address 127.0.0.1:50061 \
             --identity assistant-worker"
        ),
        "the worker step must be the runnable command with this server's own liminal \
         address filled in, not a description the operator completes by hand; got: {line}"
    );

    // THE CONTROL. Same deferral, same hashes, routed on the queue the binary
    // declares — no move, so no move is announced.
    let not_moving = AssistantInstall::Deferred {
        workflow_type: String::from("assistant"),
        embedded_hash: String::from("embedded-hash"),
        routed_hash: Some(String::from("routed-hash")),
        embedded_queue: String::from("assistant"),
        routed_queues: RoutedQueues::Declared(vec![String::from("assistant")]),
    };
    let line = captured_log(|| {
        log_outcome(
            &not_moving,
            WorkerListenerAdvice {
                listener: Some("127.0.0.1:50061"),
                config_hint: "add `liminal_listen_address` to `[outbox]` in the test config",
            },
        );
    });
    assert!(
        !line.contains("QUEUE MOVE PENDING"),
        "a version cut on one queue is not a queue move, and announcing one would teach an \
         operator to ignore the announcement; got: {line}"
    );
    assert!(
        line.contains("routing was NOT changed"),
        "the ordinary deferral must still say it changed nothing; got: {line}"
    );
}

/// A boot with no liminal worker listener cannot print a dialable command, and
/// must not print a plausible-looking one with a hole in it as if it could be
/// run. The announcement instead says the listener is missing and names the
/// exact setting that provides it — the honest arm of the same worker step.
#[test]
fn a_queue_move_on_a_boot_without_a_listener_names_the_missing_setting() {
    let moving = AssistantInstall::Deferred {
        workflow_type: String::from("assistant"),
        embedded_hash: String::from("embedded-hash"),
        routed_hash: Some(String::from("routed-hash")),
        embedded_queue: String::from("assistant"),
        routed_queues: RoutedQueues::Declared(vec![String::from(aion_core::DEFAULT_TASK_QUEUE)]),
    };
    let line = captured_log(|| {
        log_outcome(
            &moving,
            WorkerListenerAdvice {
                listener: None,
                config_hint: "add `liminal_listen_address` to `[outbox]` in the scaffolded \
                              config.toml (file)",
            },
        );
    });
    assert!(
        line.contains("QUEUE MOVE PENDING"),
        "the move is announced regardless of whether a listener exists; got: {line}"
    );
    assert!(
        line.contains("binds NO liminal worker listener"),
        "the line must say why no runnable command is printed; got: {line}"
    );
    // The remedy must be COMPLETE: the listen address alone binds nothing —
    // the dispatcher must be enabled, on the liminal transport, and the
    // outbox needs a durable store. A remedy an operator follows verbatim
    // and still gets no listener is #209's shape again.
    assert!(
        line.contains("`enabled = true`"),
        "the remedy must name the enable switch; got: {line}"
    );
    assert!(
        line.contains("`transport = \"liminal\"`"),
        "the remedy must name the transport; got: {line}"
    );
    assert!(
        line.contains("liminal_listen_address"),
        "the remedy must name the address setting (via the config hint); got: {line}"
    );
    assert!(
        line.contains("AION_OUTBOX_LIMINAL_LISTEN_ADDRESS"),
        "the remedy must name the environment override too; got: {line}"
    );
    assert!(
        line.contains("durable store"),
        "the remedy must say the outbox refuses a memory store; got: {line}"
    );
    // The hint is the boot's resolved where-to-edit text and must be rendered,
    // not merely accepted: the operator reading this is the operator who did
    // not write the config, so the FILE matters as much as the key.
    assert!(
        line.contains("scaffolded config.toml"),
        "the config-source hint must appear in the announcement; got: {line}"
    );
    // The command still appears, in its follow-later form with an explicit
    // placeholder — never with an empty hole where the address should be.
    assert!(
        line.contains("--liminal-address <that address> --identity assistant-worker"),
        "the fallback command must carry the explicit placeholder; got: {line}"
    );
    assert!(
        !line.contains("--liminal-address  "),
        "an empty address hole (double space after the flag) must never be printed; got: {line}"
    );
    assert!(
        !line.contains("--liminal-address --identity"),
        "the address must never collapse into the next flag; got: {line}"
    );
}

/// The predicate behind the announcement's runnable arm: a configured address
/// is NOT a listener. The listener exists only when the outbox dispatcher is
/// enabled on the liminal transport — the same gate the boot applies before
/// building the liminal dispatch path. Each false-positive configuration here
/// is one the first-run template actively produces (its memory-backend
/// instruction is "set `enabled = false`", leaving the address line in place).
#[test]
fn a_configured_address_is_not_a_listener_unless_the_outbox_dispatches_on_liminal() {
    use crate::config::{OutboxConfig, OutboxTransport};

    let commissioned = OutboxConfig {
        enabled: true,
        transport: OutboxTransport::Liminal,
        liminal_listen_address: Some(String::from("127.0.0.1:50061")),
        ..OutboxConfig::default()
    };
    assert_eq!(
        liminal_worker_listener(&commissioned),
        Some("127.0.0.1:50061"),
        "an enabled liminal outbox with an address is the one configuration that binds"
    );

    let disabled = OutboxConfig {
        enabled: false,
        transport: OutboxTransport::Liminal,
        liminal_listen_address: Some(String::from("127.0.0.1:50061")),
        ..OutboxConfig::default()
    };
    assert_eq!(
        liminal_worker_listener(&disabled),
        None,
        "the first-run template's memory-backend shape: address present, outbox off — \
         printing a dial command here is the #209 defect one arm over"
    );

    let wrong_transport = OutboxConfig {
        enabled: true,
        transport: OutboxTransport::Grpc,
        liminal_listen_address: Some(String::from("127.0.0.1:50061")),
        ..OutboxConfig::default()
    };
    assert_eq!(
        liminal_worker_listener(&wrong_transport),
        None,
        "a grpc-transport outbox binds no liminal listener regardless of the address"
    );

    let no_address = OutboxConfig {
        enabled: true,
        transport: OutboxTransport::Liminal,
        liminal_listen_address: None,
        ..OutboxConfig::default()
    };
    assert_eq!(
        liminal_worker_listener(&no_address),
        None,
        "no address, no listener — this shape is a boot-time config refusal anyway"
    );
}

/// A queue move is claimed only from a queue that was actually READ.
///
/// The two unknowns — no routed version, and a routed version whose contract
/// could not be read — must not render as "the assistant is somewhere else".
/// An unknown announced as a move sends an operator to redeploy a catalog
/// nobody has established anything about, and it is the failure mode a
/// convenient default would produce here.
#[test]
fn an_unknown_routed_queue_is_never_reported_as_a_move() {
    assert!(RoutedQueues::Declared(vec![String::from("default")]).moves_to("assistant"));
    assert!(!RoutedQueues::Declared(vec![String::from("assistant")]).moves_to("assistant"));
    assert!(
        !RoutedQueues::Declared(vec![String::from("assistant"), String::from("other")])
            .moves_to("assistant"),
        "a routed version that serves the embedded queue among others is already reachable there"
    );
    assert!(!RoutedQueues::NoRoutedVersion.moves_to("assistant"));
    assert!(!RoutedQueues::Unreadable(String::from("catalog poisoned")).moves_to("assistant"));

    // And each renders distinguishably, because the log line is what an
    // operator reads: an empty declaration must not look like an unread one.
    assert_eq!(
        RoutedQueues::Declared(vec![String::from("default")]).describe(),
        "default"
    );
    assert_eq!(
        RoutedQueues::Declared(Vec::new()).describe(),
        "none declared"
    );
    assert_eq!(
        RoutedQueues::NoRoutedVersion.describe(),
        "no routed version"
    );
    assert_eq!(
        RoutedQueues::Unreadable(String::from("catalog poisoned")).describe(),
        "unreadable: catalog poisoned"
    );
}

/// A complete runtime config for the boot-wiring tests below, varying only the
/// `[outbox]` under test. Everything else is the minimal working state shape;
/// the advice derivation reads nothing but the outbox.
fn boot_runtime_config(outbox: crate::config::OutboxConfig) -> crate::config::RuntimeConfig {
    use crate::config::{
        AuthConfig, AuthoringConfig, DeployConfig, DevConfig, ListenConfig, MetricsConfig,
        NamespaceConfig, NamespaceMode, OpsConsoleAssetSource, OpsConsoleConfig, RuntimeConfig,
        WebSocketConfig, WorkerConfig,
    };
    RuntimeConfig {
        listen: ListenConfig {
            grpc: std::net::SocketAddr::from(([127, 0, 0, 1], 50051)),
            http: std::net::SocketAddr::from(([127, 0, 0, 1], 8080)),
        },
        tls: None,
        auth: AuthConfig {
            enabled: false,
            jwks_url: None,
            jwks_refresh_seconds: 300,
        },
        ops_console: OpsConsoleConfig {
            source: OpsConsoleAssetSource::Embedded,
        },
        namespace: NamespaceConfig {
            mode: NamespaceMode::SharedEngine,
        },
        worker: WorkerConfig {
            heartbeat_window: std::time::Duration::from_secs(30),
            ..WorkerConfig::default()
        },
        websocket: WebSocketConfig {
            outbound_buffer_bound: 32,
            event_broadcast_capacity: Some(64),
            cluster_broadcast_capacity: Some(64),
        },
        workflow_packages: Vec::new(),
        deploy: DeployConfig::default(),
        authoring: AuthoringConfig::default(),
        dev: DevConfig::default(),
        outbox,
        observability: crate::config::ObservabilityConfig::with_flush_policy(64, 0),
        mcp: crate::config::ResolvedMcpConfig::default(),
        scheduler_threads: 1,
        jit_threshold: None,
        query_timeout: Some(std::time::Duration::from_secs(10)),
        default_namespace: "default".to_owned(),
        auto_create: crate::config::AutoCreate::Open,
        max_in_flight_activities: crate::config::DEFAULT_MAX_IN_FLIGHT_ACTIVITIES,
        drain_timeout: std::time::Duration::from_secs(30),
        metrics: MetricsConfig { enabled: true },
        owned_shards: Vec::new(),
        cors_allowed_origins: Vec::new(),
    }
}

/// Runs the boot wiring — [`install_embedded_assistant_for_server`], the line
/// `run.rs` calls — over a real `ServerState` whose catalog already holds the
/// on-`default` prior, and returns the captured announcement. The state is
/// built OUTSIDE the capture so only the install's own log is read, and the
/// outcome is asserted to be the queue-move deferral: on any other outcome the
/// advice under test was never rendered and the log would prove nothing.
async fn captured_boot_install_log(
    outbox: crate::config::OutboxConfig,
    config_hint: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let state =
        crate::ServerState::build_with_store(InMemoryStore::default(), boot_runtime_config(outbox))
            .await?;
    let engine = state.engine()?;
    let (prior, _prior_hash) = prior_package_on_default()?;
    engine.load_package(prior).await?;

    let buffer = Captured::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(buffer.clone())
        .with_ansi(false)
        .with_max_level(tracing::Level::TRACE)
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);
    let outcome = install_embedded_assistant_for_server(&state, config_hint).await;
    drop(guard);

    assert!(
        outcome.defers_a_queue_move(),
        "the fixture catalog is routed on `default`, so this install must defer a queue move; \
         got: {outcome:?}"
    );
    Ok(buffer.text())
}

/// THE BOOT WIRING (#209). What `install_embedded_assistant_for_server` adds
/// over the plain install is exactly one thing: the worker-step advice is
/// derived from the server's OWN `[outbox]`. So the queue-move announcement a
/// real boot prints must carry the address the operator actually configured —
/// an address that exists nowhere but the config under test, which makes its
/// appearance in the log the wiring itself.
#[tokio::test]
async fn the_server_install_derives_the_worker_step_from_the_server_own_outbox() -> TestResult {
    let outbox = crate::config::OutboxConfig {
        enabled: true,
        transport: crate::config::OutboxTransport::Liminal,
        liminal_listen_address: Some(String::from("127.0.0.1:59742")),
        ..crate::config::OutboxConfig::default()
    };
    let line = captured_boot_install_log(
        outbox,
        "add `liminal_listen_address` to `[outbox]` in the boot-wiring test config",
    )
    .await?;
    assert!(
        line.contains("QUEUE MOVE PENDING"),
        "the on-`default` catalog must produce the queue-move announcement; got: {line}"
    );
    assert!(
        line.contains(
            "aion worker agent assistant.awl --liminal-address 127.0.0.1:59742 \
             --identity assistant-worker"
        ),
        "the runnable worker command must carry the address from the server's own `[outbox]`; \
         got: {line}"
    );
    Ok(())
}

/// The dark twin through the same boot wiring: an `[outbox]` that names an
/// address but leaves the dispatcher off binds nothing, so the boot advice
/// must say the listener is missing and must NOT print that address as
/// dialable. This is `for_boot` consulting the commissioning gate rather than
/// the address field — and the first-run template's memory-backend
/// instruction ("set `enabled = false`", address line left in place) produces
/// exactly this config.
#[tokio::test]
async fn the_server_install_with_a_dark_outbox_advises_the_missing_listener() -> TestResult {
    let outbox = crate::config::OutboxConfig {
        enabled: false,
        transport: crate::config::OutboxTransport::Liminal,
        liminal_listen_address: Some(String::from("127.0.0.1:59743")),
        ..crate::config::OutboxConfig::default()
    };
    let line = captured_boot_install_log(
        outbox,
        "add `liminal_listen_address` to `[outbox]` in the boot-wiring test config",
    )
    .await?;
    assert!(
        line.contains("binds NO liminal worker listener"),
        "a dark outbox must be advised as one; got: {line}"
    );
    assert!(
        line.contains("boot-wiring test config"),
        "the hint handed to the boot wiring must surface in the advice verbatim; got: {line}"
    );
    assert!(
        !line.contains("127.0.0.1:59743"),
        "an address the boot does not bind must never be printed as dialable; got: {line}"
    );
    Ok(())
}
