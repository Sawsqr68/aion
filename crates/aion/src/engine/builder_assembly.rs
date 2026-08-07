//! The assembly steps `EngineBuilder::build` runs, kept apart from the builder
//! surface itself.
//!
//! Everything here is a step in standing an engine up — installing NIF seams,
//! collecting the startup catalog, claiming shards, and wiring the child
//! bridge. [`super::builder`] owns the caller-facing configuration and the
//! order these run in; nothing here reads builder state directly.

use std::{sync::Arc, time::Duration};

use aion_package::{ExtractionLimits, Package};
use aion_store::EventStore;
use aion_store::visibility::VisibilityStore;

use crate::{
    ActivityServing, EngineError, Registry, RuntimeHandle, SignalDeliveryConfig, SupervisionTree,
    WorkflowCatalog,
    activity::bridge::ActivityDispatcher,
    runtime::{
        ChildNifBridge, ChildNifBridgeParts, install_child_nif_bridge, install_nif_runtime_context,
        install_query_bridge,
        nif_determinism::{NifContextSource, install_nif_context_source},
    },
    signal::SignalResumeHandoff,
};

use super::builder::WorkflowPackageSource;

/// Install the engine-scoped NIF seams that are available before delegated
/// seams exist: runtime context, timer bridge, deterministic context source,
/// query bridge, and the optional activity dispatcher.
///
/// Returns the query mailbox engine handle installed in the query bridge, so
/// `build()` can wire the concrete query-dispatch seam over the same
/// delivery path the NIF-side `dispatch_query` uses.
pub(super) fn install_engine_nif_seams(
    nif_state: &Arc<crate::runtime::EngineNifState>,
    registry: &Arc<Registry>,
    store: &Arc<dyn EventStore>,
    runtime: &Arc<RuntimeHandle>,
    activity_dispatcher: Option<Arc<dyn ActivityDispatcher>>,
    query_timeout: Option<Duration>,
) -> Arc<dyn crate::engine_seam::EngineHandle> {
    install_nif_runtime_context(
        nif_state,
        Arc::clone(registry),
        Arc::clone(runtime),
        tokio::runtime::Handle::current(),
    );
    crate::runtime::nif_timer_bridge::install_timer_nif_bridge(
        nif_state,
        Arc::clone(registry),
        Arc::clone(store),
        tokio::runtime::Handle::current(),
        runtime.signal_delivery(),
    );
    install_nif_context_source(
        nif_state,
        Arc::new(NifContextSource::new(
            Arc::clone(registry),
            tokio::runtime::Handle::current(),
            Arc::clone(store),
            runtime.signal_delivery(),
        )),
    );
    let query_mailbox_engine = install_query_bridge(
        nif_state,
        Arc::clone(registry),
        runtime,
        tokio::runtime::Handle::current(),
        query_timeout,
    );
    if let Some(dispatcher) = activity_dispatcher {
        nif_state.set_activity_dispatcher(dispatcher);
    }
    query_mailbox_engine
}

/// Assemble the startup catalog: persisted runtime deploys reload first
/// (with their persisted route pointers), then explicit operator-supplied
/// sources load on top.
///
/// The order is the routing-intent precedence: a package named explicitly at
/// THIS boot (`--workflow-package` / builder source) is the operator's newest
/// instruction and wins the route for its type, while every persisted deploy
/// still reloads so startup recovery — which runs after this and resolves
/// each run's recorded pinned version — finds every version it needs.
/// Operator-file sources are not persisted; only the runtime deploy seam
/// writes package rows.
pub(super) async fn assemble_startup_catalog(
    runtime: &RuntimeHandle,
    store: &dyn EventStore,
    sources: Vec<WorkflowPackageSource>,
    serving: ActivityServing,
) -> Result<Arc<WorkflowCatalog>, EngineError> {
    let catalog = Arc::new(WorkflowCatalog::new_with_serving(serving));
    crate::loader::persistence::reload_persisted_packages(runtime, catalog.as_ref(), store).await?;
    for source in sources {
        let package = package_from_source(source)?;
        let outcome = catalog.load_package(runtime, &package).await?;
        tracing::info!(
            workflow_type = outcome.record.workflow_type(),
            content_hash = %outcome.record.version(),
            freshly_loaded = outcome.freshly_loaded,
            "loaded workflow package {}",
            outcome.record.workflow_type()
        );
    }
    Ok(catalog)
}

fn spawn_visibility_reconciliation_task(
    interval: Duration,
    store: Arc<dyn EventStore>,
    visibility_store: Arc<dyn VisibilityStore>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;
            if let Err(error) = crate::lifecycle::visibility::reconcile_visibility(
                Arc::clone(&store),
                Arc::clone(&visibility_store),
            )
            .await
            {
                tracing::warn!(
                    error = %error,
                    "periodic visibility reconciliation failed; crash-consistency window may remain until a later reconciliation repairs visibility"
                );
            }
        }
    })
}

/// Declare and then fence this node's owned shards, in that order.
///
/// SS-2: become the fenced live owner BEFORE any recovery enumerates them, so a
/// distributed backend's `become_live` union-merge has landed every committed
/// write locally first. A no-op for single-node / non-distributed backends, so
/// default boot is unchanged.
pub(super) fn claim_owned_shards(
    store: &dyn EventStore,
    owned_shards: Option<&[usize]>,
) -> Result<(), EngineError> {
    apply_owned_shards(store, owned_shards);
    acquire_owned_shards(store, owned_shards)
}

/// Apply owned-shard scoping to the store BEFORE any recovery or enumeration
/// reads it, so a multi-shard node recovers only its shards.
///
/// `None` leaves the store untouched — the single-node default, where the store
/// owns ALL shards and boot is byte-identical to today (the scoping hook is
/// never called). `Some(set)` forwards through any store decorator to the
/// sharded backend; a single-shard backend ignores it.
fn apply_owned_shards(store: &dyn EventStore, owned_shards: Option<&[usize]>) {
    if let Some(shards) = owned_shards {
        store.set_owned_shards(Some(shards));
    }
}

/// Win the per-shard election and become the live owner of each owned shard
/// BEFORE startup recovery reads them (SS-2).
///
/// Ordering matters: this runs after [`apply_owned_shards`] (so the store is
/// already scoped to this node's shards) and BEFORE
/// [`recover_active_workflows_on_startup`], so a distributed backend's
/// `become_live` union-merge has made every committed write on those shards
/// locally present before recovery enumerates them. The election is driven
/// through the type-erased [`ReadableEventStore::acquire_owned_shards`] seam,
/// whose distributed implementation runs the blocking coordinator on a bare
/// off-runtime thread — so calling it from this async `build()` honours
/// haematite's no-blocking-election-inside-an-async-context constraint.
///
/// `None` (the single-node default) skips election entirely, and the seam is a
/// no-op for every non-distributed backend even when a shard set is configured,
/// so boot stays byte-identical to today.
fn acquire_owned_shards(
    store: &dyn EventStore,
    owned_shards: Option<&[usize]>,
) -> Result<(), EngineError> {
    if let Some(shards) = owned_shards {
        store.acquire_owned_shards(shards)?;
    }
    Ok(())
}

/// Spawn the periodic visibility reconciliation task when an interval is
/// configured, returning its join handle; otherwise return `None`.
pub(super) fn maybe_spawn_visibility_reconciliation(
    interval: Option<Duration>,
    store: &Arc<dyn EventStore>,
    visibility_store: &Arc<dyn VisibilityStore>,
) -> Option<tokio::task::JoinHandle<()>> {
    interval.map(|interval| {
        spawn_visibility_reconciliation_task(
            interval,
            Arc::clone(store),
            Arc::clone(visibility_store),
        )
    })
}

/// Borrowed engine components assembled into the child NIF bridge.
pub(super) struct ChildBridgeAssembly<'a> {
    pub(super) nif_state: &'a Arc<crate::runtime::EngineNifState>,
    pub(super) store: &'a Arc<dyn EventStore>,
    pub(super) visibility_store: &'a Arc<dyn VisibilityStore>,
    pub(super) runtime: &'a Arc<RuntimeHandle>,
    pub(super) catalog: &'a Arc<WorkflowCatalog>,
    pub(super) registry: &'a Arc<Registry>,
    pub(super) supervision: &'a Arc<SupervisionTree>,
    pub(super) signal_handoff: &'a Arc<SignalResumeHandoff>,
    pub(super) search_attribute_schema: &'a Arc<aion_core::SearchAttributeSchema>,
    /// The child-terminal watcher reuses the builder's delivery retry
    /// policy for its registry-miss backoff windows.
    pub(super) watch_backoff: SignalDeliveryConfig,
}

/// Register the `WorkflowDeadlineHandler` on the timer bridge.
///
/// The handler holds the runtime weakly so the runtime → nif-state → bridge →
/// handler chain never cycles back into the runtime (the documented
/// cycle-avoidance the timer bridge observes with its `Weak<EngineNifState>`).
///
/// # Errors
///
/// Returns [`EngineError::Runtime`] when no timer bridge is installed.
fn register_workflow_deadline_handler(
    nif_state: &crate::runtime::EngineNifState,
    runtime: &Arc<RuntimeHandle>,
    store: &Arc<dyn EventStore>,
    visibility_store: &Arc<dyn VisibilityStore>,
    registry: &Arc<Registry>,
) -> Result<(), EngineError> {
    // `stand_down` is the wheel's OWN latch, handed in by the registration seam
    // — there is deliberately no way to pass a different one. See
    // `register_deadline_handler`.
    crate::runtime::nif_timer_bridge::register_deadline_handler(nif_state, |stand_down| {
        Arc::new(crate::lifecycle::deadline::WorkflowDeadlineHandler::new(
            Arc::downgrade(runtime),
            Arc::clone(store),
            Arc::clone(visibility_store),
            Arc::clone(registry),
            stand_down,
        ))
    })
    .map_err(|error| EngineError::Runtime {
        reason: format!("failed to register workflow deadline handler: {error}"),
    })
}

pub(super) fn install_configured_child_nif_bridge(
    assembly: &ChildBridgeAssembly<'_>,
) -> Result<(), EngineError> {
    install_child_nif_bridge(
        assembly.nif_state,
        Arc::new(ChildNifBridge::new(ChildNifBridgeParts {
            store: Arc::clone(assembly.store),
            visibility_store: Arc::clone(assembly.visibility_store),
            runtime: Arc::clone(assembly.runtime),
            catalog: Arc::clone(assembly.catalog),
            registry: Arc::clone(assembly.registry),
            supervision: Arc::clone(assembly.supervision),
            signal_handoff: Arc::clone(assembly.signal_handoff),
            search_attribute_schema: Arc::clone(assembly.search_attribute_schema),
            tokio_handle: tokio::runtime::Handle::current(),
            watch_backoff: assembly.watch_backoff,
        })),
    );
    // The dispatch seam reads an action's declared advisory class off the
    // package contract this catalog carries (RUNTIME-OPERATIONS.md R5).
    assembly
        .nif_state
        .set_workflow_catalog(Arc::clone(assembly.catalog));
    // Register the workflow-deadline handler here too: it needs the same
    // teardown deps this assembly carries, and this runs before startup timer
    // recovery, so an already-due `deadline:{run_id}` swept at boot routes to
    // the engine rather than failing as an unhandled reserved fire.
    register_workflow_deadline_handler(
        assembly.nif_state,
        assembly.runtime,
        assembly.store,
        assembly.visibility_store,
        assembly.registry,
    )
}

pub(super) fn package_from_source(source: WorkflowPackageSource) -> Result<Package, EngineError> {
    match source {
        WorkflowPackageSource::Path(path) => {
            // Operator-local startup packages from config/CLI are trusted
            // input; only the network deploy path extracts bounded.
            Package::load_from_path(&path, ExtractionLimits::unbounded()).map_err(|error| {
                EngineError::Load {
                    reason: format!(
                        "failed to load workflow package `{}`: {error}",
                        path.display()
                    ),
                }
            })
        }
        WorkflowPackageSource::Package(package) => Ok(*package),
    }
}
