//! In-memory `EventStore` conformance coverage.

use std::sync::Arc;

use aion_store::{
    EventStore, InMemoryStore, StoreError, WorkerDeploymentStore,
    conformance::{run_event_store_suite, run_worker_deployment_suite},
};

#[tokio::test]
async fn inmemory_store_satisfies_event_store_conformance_suite() -> Result<(), StoreError> {
    run_event_store_suite(|| async {
        let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
        store
    })
    .await
}

#[tokio::test]
async fn inmemory_store_satisfies_worker_deployment_conformance_suite() -> Result<(), StoreError> {
    let store = Arc::new(InMemoryStore::default());
    let trait_store: Arc<dyn WorkerDeploymentStore> = store.clone();
    let raw_store = store.clone();
    run_worker_deployment_suite(
        trait_store,
        None, // Explicit restart exemption: in-memory has no reopenable storage.
        Box::new(move |name, bytes| {
            let raw_store = raw_store.clone();
            Box::pin(async move { raw_store.write_raw_worker_deployment(&name, bytes) })
        }),
    )
    .await
}
