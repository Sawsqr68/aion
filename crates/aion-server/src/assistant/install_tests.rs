//! Proofs about what a boot install does to a catalog.
//!
//! The three cases are the whole rule: an empty catalog is claimed, the
//! embedded version already routed is left alone, and a catalog routed
//! somewhere else is NOT re-pointed. The third is the one that matters — it is
//! the property that stops a restart from undoing an operator's rollback, and
//! it is invisible to a test that only ever boots against an empty store.

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
        }
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

/// The outcome labels are stable strings, since logs and operators read them.
#[test]
fn outcome_labels_are_distinct() {
    let installed = AssistantInstall::Installed {
        workflow_type: String::from("assistant"),
        content_hash: String::from("hash"),
    };
    let current = AssistantInstall::AlreadyCurrent {
        workflow_type: String::from("assistant"),
        content_hash: String::from("hash"),
    };
    let deferred = AssistantInstall::Deferred {
        workflow_type: String::from("assistant"),
        embedded_hash: String::from("hash"),
        routed_hash: None,
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
