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

    let outcome = install_embedded_assistant(engine.as_ref()).await;
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

    let first = install_embedded_assistant(engine.as_ref()).await;
    assert!(matches!(first, AssistantInstall::Installed { .. }));
    let before = engine.list_workflow_versions()?;

    let second = install_embedded_assistant(engine.as_ref()).await;
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

    let outcome = install_embedded_assistant(engine.as_ref()).await;
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

    let on_default: &'static str = Box::leak(
        EMBEDDED_ASSISTANT_DOCUMENT
            .replace("\nworker assistant\n", "\nworker default\n")
            .into_boxed_str(),
    );
    assert_ne!(
        on_default, EMBEDDED_ASSISTANT_DOCUMENT,
        "the fixture must differ from the shipped document, or it fixes nothing in place"
    );
    // Loaded as a PACKAGE, not through `EmbeddedAssistant`: the embedded loader
    // now refuses a document on `default`, and it is right to — but the store
    // being modelled predates that refusal and holds one anyway.
    let prior = aion_awl_package::compile_and_assemble_awl(
        on_default,
        std::path::Path::new("<existing-store-fixture-has-no-schema-directory>"),
        "assistant.awl",
    )?;
    let prior = aion_package::Package::load_from_bytes(
        &prior.archive,
        aion_package::ExtractionLimits::unbounded(),
    )?;
    let prior_hash = prior.content_hash().to_string();
    assert_ne!(
        prior_hash,
        embedded.content_hash().to_string(),
        "the fixture must be a different version, or the install would report AlreadyCurrent"
    );
    engine.load_package(prior).await?;

    let outcome = install_embedded_assistant(engine.as_ref()).await;
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
    let line = captured_log(|| log_outcome(&moving));
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

    // THE CONTROL. Same deferral, same hashes, routed on the queue the binary
    // declares — no move, so no move is announced.
    let not_moving = AssistantInstall::Deferred {
        workflow_type: String::from("assistant"),
        embedded_hash: String::from("embedded-hash"),
        routed_hash: Some(String::from("routed-hash")),
        embedded_queue: String::from("assistant"),
        routed_queues: RoutedQueues::Declared(vec![String::from("assistant")]),
    };
    let line = captured_log(|| log_outcome(&not_moving));
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
