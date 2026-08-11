//! Proofs about what a boot install does — and pointedly does NOT do — to a
//! catalog.
//!
//! The three routing cases mirror the assistant's install rule: an empty
//! catalog is claimed, the embedded version already routed is left alone, and
//! a catalog routed somewhere else is NOT re-pointed. On top of those sits
//! the manual-only pin: installing the check STARTS nothing. A boot that
//! fetched the index by itself would be the server phoning home, which the
//! ruled design forbids.

use std::sync::Arc;

use aion::{Engine, EngineBuilder};
use aion_store::visibility::{ListWorkflowsFilter, VisibilityStore};
use aion_store::{EventStore, InMemoryStore};

use super::super::document::EMBEDDED_UPDATE_CHECK_DOCUMENT;
use super::super::document::EmbeddedUpdateCheck;
use super::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Engine over one in-memory backing store shared by events and visibility,
/// with the visibility handle kept so tests can ask "did anything RUN?".
async fn engine() -> Result<(Arc<Engine>, Arc<dyn VisibilityStore>), Box<dyn std::error::Error>> {
    let backing = Arc::new(InMemoryStore::default());
    let store: Arc<dyn EventStore> = backing.clone();
    let visibility: Arc<dyn VisibilityStore> = backing;
    let engine = Arc::new(
        EngineBuilder::new()
            .store_arc(store)
            .visibility_store_arc(Arc::clone(&visibility))
            .scheduler_threads(1)
            .build()
            .await?,
    );
    Ok((engine, visibility))
}

/// A catalog holding no update-check version is claimed, and the install
/// starts NOTHING: the check is startable afterwards, never started.
#[tokio::test]
async fn a_fresh_catalog_gets_the_document_installed_routed_and_nothing_started() -> TestResult {
    let (engine, visibility) = engine().await?;
    let embedded = EmbeddedUpdateCheck::load()?;

    // The engine's own plumbing (the schedule coordinator) may already have
    // executions; the pin below is the DELTA the install causes, not a global
    // gauge that would blame the install for the engine's own runs.
    let executions_before = visibility
        .count_workflows(ListWorkflowsFilter::default())
        .await?;

    let outcome = install_embedded_update_check(engine.as_ref()).await;
    assert_eq!(
        outcome,
        UpdateCheckInstall::Installed {
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

    // THE MANUAL-ONLY PIN, both ways. The install caused zero new executions
    // of ANYTHING, and zero executions of the check's own type exist: making
    // the check startable and running it are different acts, and the second
    // belongs to the operator alone.
    let executions_after = visibility
        .count_workflows(ListWorkflowsFilter::default())
        .await?;
    assert_eq!(
        executions_after, executions_before,
        "a boot install must start nothing — every check is an explicit operator act"
    );
    let check_runs = visibility
        .count_workflows(ListWorkflowsFilter {
            workflow_type: Some(embedded.workflow_type().to_owned()),
            ..ListWorkflowsFilter::default()
        })
        .await?;
    assert_eq!(check_runs, 0, "no update-check execution may exist at boot");
    Ok(())
}

/// A second install over the same catalog is a no-op that says so — the
/// restart case on a home this binary already installed into.
#[tokio::test]
async fn a_second_install_reports_already_current_and_changes_nothing() -> TestResult {
    let (engine, _visibility) = engine().await?;
    let embedded = EmbeddedUpdateCheck::load()?;

    let first = install_embedded_update_check(engine.as_ref()).await;
    assert!(matches!(first, UpdateCheckInstall::Installed { .. }));
    let before = engine.list_workflow_versions()?;

    let second = install_embedded_update_check(engine.as_ref()).await;
    assert_eq!(
        second,
        UpdateCheckInstall::AlreadyCurrent {
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

/// A catalog whose update-check route points at a DIFFERENT version is left
/// untouched, and the outcome names both hashes — a restart must never move a
/// route an operator chose.
#[tokio::test]
async fn an_install_never_repoints_a_route_it_did_not_place() -> TestResult {
    let (engine, _visibility) = engine().await?;
    let embedded = EmbeddedUpdateCheck::load()?;

    // The stand-in for the operator's own deploy: the embedded document with a
    // renamed step — same workflow type, same verified contract surface,
    // different content hash.
    let other_source = EMBEDDED_UPDATE_CHECK_DOCUMENT.replace("step fetch", "step fetch_again");
    let other = EmbeddedUpdateCheck::from_source(&other_source)?;
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

    let outcome = install_embedded_update_check(engine.as_ref()).await;
    assert_eq!(
        outcome,
        UpdateCheckInstall::Deferred {
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
    let installed = UpdateCheckInstall::Installed {
        workflow_type: String::from("update_check"),
        content_hash: String::from("hash"),
    };
    let current = UpdateCheckInstall::AlreadyCurrent {
        workflow_type: String::from("update_check"),
        content_hash: String::from("hash"),
    };
    let deferred = UpdateCheckInstall::Deferred {
        workflow_type: String::from("update_check"),
        embedded_hash: String::from("hash"),
        routed_hash: None,
    };
    let failed = UpdateCheckInstall::Failed {
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
