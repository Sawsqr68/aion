//! Async wiring: the engine's loaded-version catalog plus the store's
//! persisted archives, read once per request.

use std::sync::Arc;

use aion::Engine;
use aion_store::PackageRecord;

use super::document::read_document;
use super::list::project_versions;
use super::types::{DeployedDocument, DeployedError, DeployedVersion};

/// Lists every loaded workflow version with the state of its archived AWL
/// source.
///
/// # Errors
///
/// Returns [`DeployedError::Catalog`] when the engine's version catalog or the
/// store's archive set cannot be read.
pub async fn versions(engine: &Engine) -> Result<Vec<DeployedVersion>, DeployedError> {
    let catalog = engine
        .list_workflow_versions()
        .map_err(|error| DeployedError::Catalog(error.to_string()))?;
    let archives = persisted_archives(engine).await?;
    // Reading archives back is CPU work over potentially large buffers, so it
    // leaves the reactor thread even though nothing here touches the disk.
    tokio::task::spawn_blocking(move || project_versions(catalog, &archives))
        .await
        .map_err(|error| DeployedError::Catalog(format!("deployed listing task failed: {error}")))
}

/// Reads the archived AWL document of one deployed version.
///
/// # Errors
///
/// Returns [`DeployedError::Catalog`] when the store's archive set cannot be
/// read, and otherwise the refusals of the document reader: unknown version,
/// unreadable archive, no archived source, or a staging failure.
pub async fn document(
    engine: &Engine,
    workflow_type: &str,
    content_hash: &str,
) -> Result<DeployedDocument, DeployedError> {
    let archives = persisted_archives(engine).await?;
    let workflow_type = workflow_type.to_owned();
    let content_hash = content_hash.to_owned();
    tokio::task::spawn_blocking(move || read_document(&archives, &workflow_type, &content_hash))
        .await
        .map_err(|error| {
            DeployedError::Catalog(format!("deployed document task failed: {error}"))
        })?
}

/// Reads every persisted deployed-package archive.
///
/// `PackageStore` exposes no single-row read, so both surfaces load the set
/// and select from it in memory; the alternative is a store-contract change
/// across every backend for a read that an operator console makes by hand.
async fn persisted_archives(engine: &Engine) -> Result<Vec<PackageRecord>, DeployedError> {
    let store: Arc<dyn aion_store::EventStore> = engine.store();
    store
        .list_packages()
        .await
        .map_err(|error| DeployedError::Catalog(error.to_string()))
}
