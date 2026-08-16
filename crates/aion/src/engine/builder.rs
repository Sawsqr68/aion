//! `EngineBuilder` and build wiring.

use std::{num::NonZeroUsize, path::PathBuf, sync::Arc, time::Duration};

use chrono::Utc;

use aion_core::SearchAttributeSchema;
use aion_package::Package;
use aion_store::visibility::VisibilityStore;
use aion_store::{EventStore, InMemoryStore};

use crate::{
    ActivityServing, EngineError, Registry, RuntimeConfig, RuntimeHandle, SignalDeliveryConfig,
    SupervisionTree,
    activity::bridge::ActivityDispatcher,
    durability::ActiveWorkflowRecoverySeam,
    runtime::{NifEntry, NifRegistration},
    signal::SignalResumeHandoff,
};

use super::api::{Engine, EngineComponents};
use super::builder_assembly::{
    ChildBridgeAssembly, assemble_startup_catalog, claim_owned_shards, install_engine_nif_seams,
    install_workflow_nif_bridges, maybe_spawn_visibility_reconciliation,
};
use super::delegated::{DelegatedSeams, EventPublisher, QueryService, SignalRouter};
use super::seams::{
    SeamAssembly, SignalRouterFactory, assemble_delegated_seams, wrap_event_streaming,
};
use super::startup::{StartupRecoveryContext, resolve_startup_recovery};

/// Source for a workflow package collected before `build()` performs fallible
/// loading and runtime registration.
#[derive(Clone, Debug)]
pub enum WorkflowPackageSource {
    /// Load a package from this `.aion` archive path during `build()`.
    Path(PathBuf),
    /// Use an already-loaded package value.
    Package(Box<Package>),
}

impl From<Package> for WorkflowPackageSource {
    fn from(package: Package) -> Self {
        Self::Package(Box::new(package))
    }
}

impl From<PathBuf> for WorkflowPackageSource {
    fn from(path: PathBuf) -> Self {
        Self::Path(path)
    }
}

impl From<&std::path::Path> for WorkflowPackageSource {
    fn from(path: &std::path::Path) -> Self {
        Self::Path(path.to_path_buf())
    }
}

impl From<&str> for WorkflowPackageSource {
    fn from(path: &str) -> Self {
        Self::Path(PathBuf::from(path))
    }
}

impl From<String> for WorkflowPackageSource {
    fn from(path: String) -> Self {
        Self::Path(PathBuf::from(path))
    }
}

/// Tracks which optional engine seams the caller explicitly overrode, so
/// `build()` can detect mutually-exclusive configuration (e.g. an event
/// publisher set both directly and via event streaming). Grouped so the builder
/// keeps its boolean configuration flags few and named.
#[derive(Default)]
struct SeamOverrides {
    /// The caller installed an explicit event-publisher seam.
    event_publisher: bool,
    /// The caller installed an explicit query-service seam.
    query_service: bool,
}

/// Builder for the embedded, transport-agnostic workflow engine.
pub struct EngineBuilder {
    store: Option<Arc<dyn EventStore>>,
    visibility_store: Option<Arc<dyn VisibilityStore>>,
    scheduler_threads: Option<usize>,
    signal_delivery: SignalDeliveryConfig,
    completion_retry: crate::runtime::CompletionRetryConfig,
    outbox_enabled: bool,
    bootstrap_schedule_coordinator: bool,
    owned_shards: Option<Vec<usize>>,
    workflow_sources: Vec<WorkflowPackageSource>,
    host_nifs: Vec<NifEntry>,
    recovery: Option<Arc<dyn ActiveWorkflowRecoverySeam>>,
    delegated: DelegatedSeams,
    signal_router_factory: Option<SignalRouterFactory>,
    activity_dispatcher: Option<Arc<dyn ActivityDispatcher>>,
    activity_serving: ActivityServing,
    active_registry: Option<Arc<Registry>>,
    visibility_reconciliation_interval: Option<Duration>,
    search_attribute_schema: SearchAttributeSchema,
    event_streaming_capacity: Option<NonZeroUsize>,
    query_timeout: Option<Duration>,
    seam_overrides: SeamOverrides,
    defer_startup_recovery: bool,
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineBuilder {
    /// Create a builder with no store, no scheduler-thread override, no loaded
    /// workflows, and no host NIFs.
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: None,
            visibility_store: None,
            scheduler_threads: None,
            signal_delivery: SignalDeliveryConfig::default(),
            completion_retry: crate::runtime::CompletionRetryConfig::default(),
            outbox_enabled: false,
            // The only field that defaults true: single-node engines seed the
            // schedule coordinator. A multi-node deployment disables it on nodes
            // that do not own the coordinator's shard (see the builder method).
            bootstrap_schedule_coordinator: true,
            // No shard restriction by default: the store owns ALL shards, which
            // is byte-identical to single-node behaviour. `build()` only ever
            // touches owned-shard scoping when a deployment sets this.
            owned_shards: None,
            workflow_sources: Vec::new(),
            host_nifs: Vec::new(),
            recovery: None,
            delegated: DelegatedSeams::default(),
            signal_router_factory: None,
            activity_dispatcher: None,
            activity_serving: ActivityServing::QueueRouted,
            active_registry: None,
            visibility_reconciliation_interval: None,
            search_attribute_schema: SearchAttributeSchema::new(),
            event_streaming_capacity: None,
            query_timeout: None,
            seam_overrides: SeamOverrides::default(),
            defer_startup_recovery: false,
        }
    }

    /// Record the caller-supplied workflow query reply timeout.
    ///
    /// Setting a timeout installs the concrete query-dispatch seam during
    /// `build()` (unless [`Self::query_service`] overrides it) and enables
    /// the in-engine `dispatch_query` NIF. There is no default: without this
    /// call the query seam stays deferred and `Engine::query` fails typed
    /// with its "not configured" error.
    #[must_use]
    pub const fn query_timeout(mut self, timeout: Duration) -> Self {
        self.query_timeout = Some(timeout);
        self
    }

    /// Inspect the configured workflow query reply timeout.
    #[must_use]
    pub const fn configured_query_timeout(&self) -> Option<Duration> {
        self.query_timeout
    }

    /// Opt in to live event streaming with a caller-provided broadcast capacity.
    ///
    /// `build()` wraps the configured store in a
    /// [`PublishingEventStore`](crate::publish::PublishingEventStore) before any
    /// recorder, recovery, or NIF bridge captures the store — so every
    /// successful append publishes — and installs the matching
    /// [`BroadcastEventPublisher`](crate::publish::BroadcastEventPublisher) as
    /// the event-publisher seam behind [`Engine::subscribe`]. Without this call
    /// the deferred publisher remains installed and subscriptions are empty.
    #[must_use]
    pub const fn event_streaming(mut self, capacity: NonZeroUsize) -> Self {
        self.event_streaming_capacity = Some(capacity);
        self
    }

    /// Supply the search attribute schema validating every recorded attribute.
    ///
    /// The default schema is empty, which rejects all search attributes: a
    /// deployment must declare each attribute name and type before workflows
    /// can record values for it.
    #[must_use]
    pub fn search_attribute_schema(mut self, schema: SearchAttributeSchema) -> Self {
        self.search_attribute_schema = schema;
        self
    }

    /// Supply the event store used by the engine.
    #[must_use]
    pub fn store<S>(mut self, store: S) -> Self
    where
        S: EventStore,
    {
        self.store = Some(Arc::new(store));
        self
    }

    /// Supply an already type-erased event store.
    #[must_use]
    pub fn store_arc(mut self, store: Arc<dyn EventStore>) -> Self {
        self.store = Some(store);
        self
    }

    /// Supply the visibility store used by the engine for workflow projections.
    #[must_use]
    pub fn visibility_store<S>(mut self, visibility_store: S) -> Self
    where
        S: VisibilityStore,
    {
        self.visibility_store = Some(Arc::new(visibility_store));
        self
    }

    /// Supply an already type-erased visibility store.
    #[must_use]
    pub fn visibility_store_arc(mut self, visibility_store: Arc<dyn VisibilityStore>) -> Self {
        self.visibility_store = Some(visibility_store);
        self
    }

    /// Explicitly opt in to an ephemeral in-memory visibility store.
    ///
    /// This is intended for tests and local scenarios that do not need durable
    /// visibility projections. Visibility data stored this way does not survive
    /// process restarts.
    #[must_use]
    pub fn in_memory_visibility(mut self) -> Self {
        self.visibility_store = Some(Arc::new(InMemoryStore::default()));
        self
    }

    /// Record the caller-supplied scheduler thread count.
    ///
    /// If this setter is never called, `None` is passed through to beamr.
    #[must_use]
    pub const fn scheduler_threads(mut self, threads: usize) -> Self {
        self.scheduler_threads = Some(threads);
        self
    }

    /// Record the caller-supplied periodic visibility reconciliation interval.
    ///
    /// If this setter is never called, no periodic background reconciliation task is spawned.
    #[must_use]
    pub const fn visibility_reconciliation_interval(mut self, interval: Duration) -> Self {
        self.visibility_reconciliation_interval = Some(interval);
        self
    }

    /// Record the caller-supplied signal delivery readiness and retry policy.
    #[must_use]
    pub const fn signal_delivery(mut self, signal_delivery: SignalDeliveryConfig) -> Self {
        self.signal_delivery = signal_delivery;
        self
    }

    /// Record the caller-supplied durable completion-retry backoff ladder.
    ///
    /// Separate from [`Self::signal_delivery`] because the completion retry is
    /// unbounded by attempts and sleeps between durable store round-trips; see
    /// [`crate::runtime::CompletionRetryConfig`] for why sharing one ladder
    /// between the two was a defect.
    #[must_use]
    pub const fn completion_retry(
        mut self,
        completion_retry: crate::runtime::CompletionRetryConfig,
    ) -> Self {
        self.completion_retry = completion_retry;
        self
    }

    /// Record whether the durable-outbox fan-out dispatch path is enabled.
    #[must_use]
    pub fn outbox_enabled(mut self, enabled: bool) -> Self {
        self.outbox_enabled = enabled;
        self
    }

    /// Control whether `build()` seeds the schedule-coordinator history.
    ///
    /// Default `true` (single-node). Under multi-shard active-active the
    /// coordinator stream is owned by exactly ONE shard; a deployment sets this
    /// `false` on every node that does NOT own that shard, so only the owner
    /// seeds (and serves) it — a non-owner would otherwise try to write the
    /// coordinator stream and race or fence the real owner.
    #[must_use]
    pub const fn bootstrap_schedule_coordinator(mut self, enabled: bool) -> Self {
        self.bootstrap_schedule_coordinator = enabled;
        self
    }

    /// Restrict this engine's store to the distribution shards this node owns.
    ///
    /// Under multi-shard active-active a node serves only a SUBSET of the
    /// cluster's shards. `build()` calls
    /// [`ReadableEventStore::set_owned_shards`](aion_store::ReadableEventStore::set_owned_shards)
    /// with this set BEFORE startup recovery, so the node recovers and
    /// enumerates only the workflows / timers / outbox rows that live on its
    /// shards. The set is deduplicated and ordered by the store.
    ///
    /// Not calling this leaves the store owning ALL shards — the single-node
    /// default, which is byte-identical to today's behaviour (`build()` never
    /// touches the scoping hook). Single-shard backends (in-memory, libSQL)
    /// ignore the call regardless, since they own everything unconditionally.
    #[must_use]
    pub fn owned_shards(mut self, shards: impl IntoIterator<Item = usize>) -> Self {
        self.owned_shards = Some(shards.into_iter().collect());
        self
    }

    /// Inspect the configured owned-shard set (`None` = own all shards).
    #[must_use]
    pub fn configured_owned_shards(&self) -> Option<&[usize]> {
        self.owned_shards.as_deref()
    }

    /// Add one workflow package source to load during `build()`.
    #[must_use]
    pub fn load_workflows(mut self, source: impl Into<WorkflowPackageSource>) -> Self {
        self.workflow_sources.push(source.into());
        self
    }

    /// Add many workflow package sources to load during `build()`.
    #[must_use]
    pub fn load_workflow_sources<I, S>(mut self, sources: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<WorkflowPackageSource>,
    {
        self.workflow_sources
            .extend(sources.into_iter().map(Into::into));
        self
    }

    /// Collect host-supplied NIF entries to install before workflow modules load.
    #[must_use]
    pub fn register_nifs(mut self, entries: impl IntoIterator<Item = NifEntry>) -> Self {
        self.host_nifs.extend(entries);
        self
    }

    /// Override the AD recovery seam used while repopulating active workflows.
    #[must_use]
    pub fn recovery_seam(mut self, recovery: Arc<dyn ActiveWorkflowRecoverySeam>) -> Self {
        self.recovery = Some(recovery);
        self
    }

    /// Use the production AD recovery seam created after runtime/package loading.
    #[must_use]
    pub fn production_recovery_seam(mut self) -> Self {
        self.recovery = None;
        self
    }

    /// Defer the four startup-recovery steps out of `build()` (#266).
    ///
    /// Recovery replay re-dispatches every in-flight activity through the
    /// configured activity dispatcher the moment it runs. A host that
    /// decorates that dispatcher with collaborators it can only install once
    /// the engine exists (the server's declared-body source) must therefore
    /// build first, install its seams, and only then run recovery — by
    /// calling [`Engine::run_startup_recovery`] exactly once. Without this
    /// call, `build()` runs recovery itself, byte-identically to before this
    /// seam existed, and `run_startup_recovery` refuses typed.
    #[must_use]
    pub fn defer_startup_recovery(mut self) -> Self {
        self.defer_startup_recovery = true;
        self
    }

    /// Override the AT signal-routing seam.
    #[must_use]
    pub fn signal_router(mut self, signal_router: Arc<dyn SignalRouter>) -> Self {
        self.signal_router_factory = None;
        self.delegated = DelegatedSeams::new(
            signal_router,
            self.delegated.query_service_arc(),
            self.delegated.event_publisher_arc(),
        );
        self
    }

    /// Override the AT signal-routing seam after the runtime is assembled.
    #[must_use]
    pub fn signal_router_factory<F>(mut self, factory: F) -> Self
    where
        F: Fn(Arc<RuntimeHandle>, Arc<SignalResumeHandoff>) -> Arc<dyn SignalRouter>
            + Send
            + Sync
            + 'static,
    {
        self.signal_router_factory = Some(Arc::new(factory));
        self
    }

    /// Override the AT query-dispatch seam.
    ///
    /// An explicit override wins over the concrete service that
    /// [`Self::query_timeout`] would otherwise install during `build()`.
    #[must_use]
    pub fn query_service(mut self, query_service: Arc<dyn QueryService>) -> Self {
        self.seam_overrides.query_service = true;
        self.delegated = DelegatedSeams::new(
            self.delegated.signal_router_arc(),
            query_service,
            self.delegated.event_publisher_arc(),
        );
        self
    }

    /// Override the AD/AT live event-publisher seam.
    ///
    /// Mutually exclusive with [`Self::event_streaming`], which installs the
    /// broadcast publisher itself; configuring both fails `build()`.
    #[must_use]
    pub fn event_publisher(mut self, event_publisher: Arc<dyn EventPublisher>) -> Self {
        self.seam_overrides.event_publisher = true;
        self.delegated = DelegatedSeams::new(
            self.delegated.signal_router_arc(),
            self.delegated.query_service_arc(),
            event_publisher,
        );
        self
    }

    /// Supply the activity dispatcher that backs activity dispatch NIFs.
    ///
    /// When set, the dispatcher is installed in the global bridge before
    /// workflow modules are loaded. Without a dispatcher, `dispatch_activity`
    /// returns an error to workflow code instead of crashing the process.
    #[must_use]
    pub fn activity_dispatcher(mut self, dispatcher: Arc<dyn ActivityDispatcher>) -> Self {
        self.activity_dispatcher = Some(dispatcher);
        self
    }

    /// Declare that the configured in-process [`ActivityDispatcher`] fulfils
    /// every activity this engine dispatches — the embedded posture.
    ///
    /// This gates ONLY the structural queue-service admission check: with
    /// in-process serving declared, a package whose contract retains
    /// unscoped (legacy-manifest) activities is admitted at start, because
    /// no queue exists to be unserved — dispatch runs on the configured
    /// dispatcher immediately, and a missing dispatcher fails loudly at
    /// dispatch. The `.v4` identity floor is unconditional either way. The
    /// server never declares this: its dispatcher routes to task queues, so
    /// it keeps the fail-closed queue-routed default.
    #[must_use]
    pub const fn in_process_activity_serving(mut self) -> Self {
        self.activity_serving = ActivityServing::InProcess;
        self
    }

    /// Supply the active workflow registry used by the built engine.
    ///
    /// Server-owned dispatchers that run behind raw NIFs use this to correlate a
    /// calling BEAM pid to the same workflow handle the engine registers.
    #[must_use]
    pub fn active_registry(mut self, registry: Arc<Registry>) -> Self {
        self.active_registry = Some(registry);
        self
    }

    /// Inspect the configured scheduler thread count.
    #[must_use]
    pub const fn scheduler_thread_count(&self) -> Option<usize> {
        self.scheduler_threads
    }

    /// Inspect the configured periodic visibility reconciliation interval.
    #[must_use]
    pub const fn configured_visibility_reconciliation_interval(&self) -> Option<Duration> {
        self.visibility_reconciliation_interval
    }

    /// Assemble the runtime configuration from the builder-supplied scheduler,
    /// signal delivery, completion-retry, and outbox knobs.
    fn runtime_config(&self) -> RuntimeConfig {
        RuntimeConfig::new(self.scheduler_threads)
            .with_signal_delivery(self.signal_delivery)
            .with_completion_retry(self.completion_retry)
            .with_outbox_enabled(self.outbox_enabled)
    }

    /// Start the runtime and install the engine plus host NIFs.
    fn start_runtime_with_nifs(
        runtime_config: RuntimeConfig,
        host_nifs: Vec<NifEntry>,
    ) -> Result<Arc<RuntimeHandle>, EngineError> {
        let runtime = Arc::new(RuntimeHandle::new(runtime_config)?);
        let mut nifs = NifRegistration::new();
        nifs.add_engine_nifs().add_host_nifs(host_nifs);
        runtime.install_nifs(nifs)?;
        Ok(runtime)
    }

    /// Construct the live engine.
    ///
    /// # Errors
    ///
    /// Returns typed [`EngineError`] variants for missing store, runtime startup,
    /// NIF registration, package loading, store reads, registry/supervision lock
    /// poison, or deferred AD recovery failures for active histories.
    pub async fn build(self) -> Result<Engine, EngineError> {
        let runtime_config = self.runtime_config();
        let (store, streaming_publisher) = wrap_event_streaming(
            self.store.ok_or(EngineError::MissingStore)?,
            self.event_streaming_capacity,
            self.seam_overrides.event_publisher,
        )?;
        let visibility_store = self
            .visibility_store
            .ok_or(EngineError::MissingVisibilityStore)?;
        claim_owned_shards(store.as_ref(), self.owned_shards.as_deref())?;

        let runtime = Self::start_runtime_with_nifs(runtime_config, self.host_nifs)?;

        // Persisted runtime deploys must be resident before startup recovery
        // below resolves any run's recorded pinned version — this is the
        // restart half of the deploy durability promise.
        let catalog = assemble_startup_catalog(
            runtime.as_ref(),
            store.as_ref(),
            self.workflow_sources,
            self.activity_serving,
        )
        .await?;

        let registry = self
            .active_registry
            .unwrap_or_else(|| Arc::new(Registry::default()));
        let nif_state = Arc::clone(runtime.nif_state());
        let query_mailbox_engine = install_engine_nif_seams(
            &nif_state,
            &registry,
            &store,
            &runtime,
            self.activity_dispatcher,
            self.query_timeout,
        );
        let supervision = Arc::new(SupervisionTree::new());
        let search_attribute_schema = Arc::new(self.search_attribute_schema);
        let signal_handoff = Arc::new(SignalResumeHandoff::new());

        let delegated = assemble_delegated_seams(SeamAssembly {
            configured: self.delegated,
            signal_router_factory: self.signal_router_factory,
            runtime: Arc::clone(&runtime),
            signal_handoff: Arc::clone(&signal_handoff),
            streaming_publisher,
            query_mailbox_engine,
            query_timeout: self.query_timeout,
            query_service_overridden: self.seam_overrides.query_service,
        });

        // Startup recovery re-spawns active workflow processes, and those
        // processes begin replaying on scheduler threads immediately. Replay
        // re-executes workflow code through the engine NIFs, so every NIF
        // bridge (signal, child) must be installed before the first recovered
        // process can run, or an early replayed spawn_child/receive_signal
        // call fails with a missing-bridge error.
        install_workflow_nif_bridges(
            &ChildBridgeAssembly {
                nif_state: &nif_state,
                store: &store,
                visibility_store: &visibility_store,
                runtime: &runtime,
                catalog: &catalog,
                registry: &registry,
                supervision: &supervision,
                signal_handoff: &signal_handoff,
                search_attribute_schema: &search_attribute_schema,
                watch_backoff: self.signal_delivery,
            },
            &delegated,
        )?;

        // A deferred build (#266) skips the recovery steps and stows what the
        // host owes [`Engine::run_startup_recovery`]; see
        // [`resolve_startup_recovery`].
        let deferred_startup_recovery = resolve_startup_recovery(
            self.defer_startup_recovery,
            &nif_state,
            StartupRecoveryContext {
                store: Arc::clone(&store),
                visibility_store: Arc::clone(&visibility_store),
                runtime: Arc::clone(&runtime),
                catalog: Arc::clone(&catalog),
                registry: Arc::clone(&registry),
                supervision: Arc::clone(&supervision),
                recovery: self.recovery,
                search_attribute_schema: Arc::clone(&search_attribute_schema),
                bootstrap_schedule_coordinator: self.bootstrap_schedule_coordinator,
            },
        )
        .await?;

        let visibility_reconciliation_task = maybe_spawn_visibility_reconciliation(
            self.visibility_reconciliation_interval,
            &store,
            &visibility_store,
        );

        let deferred = deferred_startup_recovery.is_some();
        let engine = Engine::new(EngineComponents {
            store,
            visibility_store,
            runtime,
            catalog,
            registry,
            supervision,
            delegated,
            signal_handoff,
            search_attribute_schema,
            visibility_reconciliation_task,
            deferred_startup_recovery,
        });
        if !deferred {
            engine.catchup_schedule_coordinator().await?;
            engine.recover_schedules_on_startup(Utc::now()).await?;
        }
        Ok(engine)
    }
}

#[cfg(test)]
mod tests {
    // 🔴 #125. This was a hand-rolled twelve-line gate announcing its skip with
    // `println!` — the exact pattern task #74 replaced, because the harness
    // captures `println!` on a PASSING test and a gated skip always passes, so
    // the announcement was invisible in precisely the case it existed for.
    //
    // #74's cure landed in `tests/test_support/gleam.rs` and is pinned across
    // four byte-identical copies. This file is `src/`, so a sweep over `tests/`
    // could never see it — and the correct copy was already sitting in THIS
    // crate's own `tests/` directory. A fix's search space is part of the fix.
    //
    // Reusing the canonical file by path rather than copying it, per the
    // precedent in `aion-package/src/structure/mod.rs`: a third copy would look
    // right in isolation and drift silently. It is DECLARED in `engine/mod.rs`
    // because `#[path]` resolves against the declaring module's directory and
    // `src/engine/builder/` does not exist.
    use super::super::gleam_test_support;

    use std::{num::NonZeroUsize, path::PathBuf, process::Command, sync::Arc, time::Duration};

    use aion_core::{Event, EventEnvelope, Payload, WorkflowId, WorkflowStatus};
    use aion_package::{
        BeamModule, BeamSet, CURRENT_FORMAT_VERSION, DeclaredActivity, ExtractionLimits, Manifest,
        ManifestVersion, Package, PackageBuilder,
    };
    use aion_store::visibility::{ListWorkflowsFilter, VisibilityStore};
    use aion_store::{InMemoryStore, ReadableEventStore, WritableEventStore, WriteToken};
    use chrono::Utc;
    use futures::StreamExt;
    use serde_json::json;

    use crate::engine::api_schedule::{
        schedule_coordinator_run_id, schedule_coordinator_workflow_id,
        schedule_coordinator_workflow_type,
    };
    use crate::runtime::{Determinism, Mfa, NifEntry};

    use super::EngineBuilder;
    use crate::EngineError;

    fn payload() -> Result<Payload, aion_core::PayloadError> {
        Payload::from_json(&json!({ "input": true }))
    }

    fn started(
        workflow_id: &WorkflowId,
        workflow_type: &str,
    ) -> Result<Event, aion_core::PayloadError> {
        Ok(Event::WorkflowStarted {
            envelope: EventEnvelope {
                seq: 1,
                recorded_at: Utc::now(),
                workflow_id: workflow_id.clone(),
            },
            workflow_type: workflow_type.to_owned(),
            input: payload()?,
            run_id: aion_core::RunId::new(uuid::Uuid::from_u128(1)),
            parent_run_id: None,
            package_version: aion_core::PackageVersion::new("a".repeat(64)),
        })
    }

    fn completed(workflow_id: &WorkflowId) -> Result<Event, aion_core::PayloadError> {
        Ok(Event::WorkflowCompleted {
            envelope: EventEnvelope {
                seq: 2,
                recorded_at: Utc::now(),
                workflow_id: workflow_id.clone(),
            },
            result: payload()?,
        })
    }

    fn package_manifest() -> Manifest {
        Manifest {
            entry_module: "counter".to_owned(),
            entry_function: "version".to_owned(),
            input_schema: json!({ "type": "object" }),
            output_schema: json!({ "type": "integer" }),
            timeout: Some(Duration::from_secs(30)),
            activities: vec![DeclaredActivity {
                activity_type: "activity/test".to_owned(),
            }],
            version: ManifestVersion::new("test"),
            format_version: CURRENT_FORMAT_VERSION,
            additional_workflows: Vec::new(),
        }
    }

    fn compile_counter_beam() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("aion-engine-builder-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&temp_dir)?;
        let source_path = temp_dir.join("counter.erl");
        let beam_path = temp_dir.join("counter.beam");
        std::fs::write(
            &source_path,
            "-module(counter).\n-export([version/0]).\nversion() -> 1.\n",
        )?;
        let status = Command::new("erlc")
            .arg("-o")
            .arg(&temp_dir)
            .arg(&source_path)
            .status()?;
        if !status.success() {
            let cleanup_result = std::fs::remove_dir_all(&temp_dir);
            drop(cleanup_result);
            return Err(format!("erlc failed with status {status}").into());
        }
        let bytes = std::fs::read(beam_path)?;
        std::fs::remove_dir_all(temp_dir)?;
        Ok(bytes)
    }

    fn fixture_package() -> Result<Package, Box<dyn std::error::Error>> {
        let beams = BeamSet::new(vec![BeamModule::new("counter", compile_counter_beam()?)])?;
        let archive = PackageBuilder::new(package_manifest(), beams).write_to_bytes()?;
        Ok(Package::load_from_bytes(
            archive,
            ExtractionLimits::unbounded(),
        )?)
    }

    fn write_fixture_package(package: &Package) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let path =
            std::env::temp_dir().join(format!("aion-engine-builder-{}.aion", uuid::Uuid::new_v4()));
        PackageBuilder::new(package.manifest().clone(), package.beams().clone())
            .write_to_path(&path)?;
        Ok(path)
    }

    #[tokio::test]
    async fn build_without_store_returns_missing_store() {
        let error = EngineBuilder::new().build().await.err();

        assert!(matches!(error, Some(EngineError::MissingStore)));
    }

    #[tokio::test]
    async fn build_without_visibility_store_returns_missing_visibility_store() {
        let error = EngineBuilder::new()
            .store(InMemoryStore::default())
            .build()
            .await
            .err();

        assert!(matches!(error, Some(EngineError::MissingVisibilityStore)));
    }

    #[tokio::test]
    async fn in_memory_visibility_allows_build_without_visibility_store() -> Result<(), EngineError>
    {
        let engine = EngineBuilder::new()
            .store(InMemoryStore::default())
            .in_memory_visibility()
            .build()
            .await?;

        engine.shutdown()?;
        Ok(())
    }

    /// 🔴 M-1. Releasing an engine WITHOUT shutting it down closes the epoch.
    ///
    /// `RuntimeHandle::shutdown` covers the explicit path. This covers the other
    /// one, and it is not hypothetical: a completion retry appends terminal
    /// events, so an engine dropped on an error path with a retry still armed
    /// would keep writing to histories nobody believes this process still owns.
    ///
    /// The refcount backstop cannot serve here, which is the whole reason this
    /// `Drop` exists. `EngineTaskRuntime::drop` fires only when the last strong
    /// reference goes, and an in-flight attempt holds one for the entire attempt
    /// — so at the instant the hazard is real, the backstop is pinned shut. This
    /// gate is set by a `Drop` that runs regardless of who else holds a handle.
    ///
    /// The first assertion is the control: an engine that never opened the epoch
    /// would satisfy the second trivially.
    ///
    /// 🔴 THE ENGINE IS BUILT WITH A RECONCILIATION INTERVAL, AND THAT IS NOT
    /// INCIDENTAL. Its first form used a bare fixture with no interval, so no
    /// reconciliation task was ever spawned — and a fixture that never starts
    /// the thing under test cannot see whether releasing the engine stops it.
    /// It could not: `Drop` did not abort `visibility_reconciliation_task`, and
    /// dropping a `JoinHandle` DETACHES rather than cancels, so a released
    /// engine left an unbounded loop holding both stores and calling
    /// `reconcile_visibility`, which WRITES. Two properties are pinned here
    /// because one `Drop` owns both: the epoch closes, and the writer stops.
    ///
    /// The writer half is measured by counting the loop's own ticks —
    /// `reconcile_visibility` reads `list_workflows` unconditionally on every
    /// pass — through a wrapper that delegates everything to a real store, so
    /// what is observed is the production loop and not a stand-in for it. The
    /// wait for two ticks before the drop is the positive control: it fails the
    /// test if the loop was never running, which is the exact way the first
    /// form of this test was vacuous.
    #[tokio::test]
    async fn dropping_an_engine_without_shutdown_closes_the_completion_retry_epoch()
    -> Result<(), EngineError> {
        let visibility = Arc::new(TickCountingVisibilityStore::default());
        let engine = EngineBuilder::new()
            .store(InMemoryStore::default())
            .visibility_store_arc(Arc::clone(&visibility) as Arc<dyn VisibilityStore>)
            .visibility_reconciliation_interval(Duration::from_millis(10))
            .build()
            .await?;
        // Taken before the drop and held across it: reading through the engine
        // afterwards is impossible, and reading a fresh handle would observe a
        // different runtime.
        let tasks = engine.runtime().engine_tasks();
        assert!(
            tasks.is_epoch_open(),
            "control: a live engine's epoch must be open, or the assertion below is trivially \
             satisfied"
        );

        // Positive control for the writer half: the loop must be observably
        // running BEFORE the drop, or "it stopped" is a statement about a task
        // that never started. Bounded so a stalled loop fails rather than hangs.
        let ticking = tokio::time::timeout(Duration::from_secs(10), async {
            while visibility.ticks() < 2 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await;
        assert!(
            ticking.is_ok(),
            "control: the periodic reconciliation loop must be running before the engine is \
             dropped, or this test measures nothing; it reached {} ticks",
            visibility.ticks()
        );

        drop(engine);

        assert!(
            !tasks.is_epoch_open(),
            "an engine released without an explicit shutdown must still close the epoch, or a \
             completion retry can append a terminal for a run this process no longer owns"
        );

        // `abort` is asynchronous: it schedules cancellation, so give the
        // runtime a window several intervals wide to deliver it before reading
        // the baseline. Anything after that baseline is a task still looping.
        tokio::time::sleep(Duration::from_millis(200)).await;
        let after_abort = visibility.ticks();
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(
            visibility.ticks(),
            after_abort,
            "an engine released without an explicit shutdown must also stop the visibility \
             reconciliation loop; it ticked again over twenty intervals after the drop, which \
             means a detached task is still WRITING to a visibility store this process no longer \
             owns"
        );
        Ok(())
    }

    /// How far ahead of "now" the shared wheel-timer fixture arms its deadline.
    ///
    /// 🔴 THIS IS A RACE MARGIN, NOT A TASTE. The release test has to reach
    /// `drop(engine)` while the timer is still pending; if the deadline passes
    /// first, the timer fires LEGITIMATELY and the test's own message blames
    /// `Drop` for a surviving task that never survived — a red that reassigns
    /// blame, which is worse than no test. The margin buys a window that the
    /// two atomic reads and one drop between arming and release cannot
    /// plausibly exhaust, and the release test asserts it was still inside that
    /// window rather than assuming it.
    const WHEEL_TIMER_ARMING_MARGIN: std::time::Duration = std::time::Duration::from_secs(2);

    /// How far past the deadline both wheel-timer tests observe before reading
    /// history — the fire path is asynchronous, so the deadline arriving is not
    /// the same as the append having landed.
    const WHEEL_TIMER_OBSERVATION_SLACK: std::time::Duration = std::time::Duration::from_secs(1);

    /// A resident run with one live wheel timer, armed through the production
    /// path, shared by the release test and its positive control. Returns the
    /// workflow and the deadline it armed, because the release test has to
    /// check it is still inside the arming window before it can attribute
    /// anything to the release.
    ///
    /// 🔴 ONE FIXTURE, DELIBERATELY. The control's whole job is to show that
    /// THIS arrangement's timer reaches the store; a second copy that drifted —
    /// a different residency, a different timer name, a different deadline —
    /// would control nothing while still reading as a control.
    ///
    /// The deadline has to arrive INSIDE the test. The hazard a released engine
    /// leaves behind is a parked task that later wakes and records, so a
    /// deadline beyond the test's own lifetime makes "nothing fired" true for a
    /// reason that has nothing to do with the release. That is what
    /// [`WHEEL_TIMER_ARMING_MARGIN`] trades against: near enough to observe,
    /// far enough not to race the drop.
    async fn arm_resident_run_with_wheel_timer(
        engine: &crate::Engine,
        store: &Arc<InMemoryStore>,
    ) -> Result<(WorkflowId, chrono::DateTime<Utc>), Box<dyn std::error::Error>> {
        use aion_store::EventStore;

        use crate::durability::{Recorder, WorkflowStartRecord};
        use crate::registry::{
            CompletionNotifier, HandleResidency, WorkflowHandle, WorkflowHandleParts,
        };

        // The wheel is armed only for a workflow the registry resolves as
        // `Resident`, and `TimerService::schedule` is the production path that
        // arms it.
        let workflow_id = WorkflowId::new_v4();
        let run_id = aion_core::RunId::new_v4();
        let timer_id = aion_core::TimerId::named("sleep")?;
        let mut recorder = Recorder::new(
            workflow_id.clone(),
            Arc::clone(store) as Arc<dyn EventStore>,
        );
        recorder
            .record_workflow_started(
                Utc::now(),
                WorkflowStartRecord {
                    workflow_type: "checkout".to_owned(),
                    input: payload()?,
                    run_id: run_id.clone(),
                    parent_run_id: None,
                    package_version: aion_core::PackageVersion::new("a".repeat(64)),
                },
            )
            .await?;
        recorder
            .record_timer_started(Utc::now(), timer_id.clone(), Utc::now())
            .await?;
        engine.registry().insert(
            (workflow_id.clone(), run_id.clone()),
            WorkflowHandle::new(WorkflowHandleParts {
                workflow_id: workflow_id.clone(),
                run_id,
                pid: 1,
                workflow_type: "checkout".to_owned(),
                namespace: String::from("default"),
                loaded_version: aion_package::ContentHash::from_bytes([9; 32]),
                cached_status: WorkflowStatus::Running,
                residency: HandleResidency::Resident,
                recorder,
                completion: CompletionNotifier::new(),
            }),
        )?;

        let fire_at = Utc::now() + chrono::Duration::from_std(WHEEL_TIMER_ARMING_MARGIN)?;
        crate::runtime::nif_timer_bridge::installed_timer_service(engine.runtime().nif_state())
            .map_err(|error| format!("the engine installed no timer service: {error}"))?
            .schedule(workflow_id.clone(), timer_id, fire_at)
            .await?;
        Ok((workflow_id, fire_at))
    }

    /// 🔴 M-1's THIRD WRITER, AND
    /// `dropping_an_engine_without_shutdown_closes_the_completion_retry_epoch`
    /// IS BLIND TO IT.
    ///
    /// `Drop for Engine` disarms the live timer wheel as well as closing the
    /// engine-task epoch and aborting the reconciliation loop. That sibling's
    /// fixture never arms a wheel timer, so deleting `shutdown_timer_wheel` from
    /// `Drop` leaves every one of its assertions green — a fixture that never
    /// starts the thing under test cannot see whether releasing the engine stops
    /// it, which is the same defect that test's own comment records having had.
    ///
    /// The wheel is a DURABLE writer: a fired wheel timer records `TimerFired`
    /// through the run's recorder. An engine released with one still armed
    /// therefore appends to a history this process no longer owns — the #119
    /// failover race reached through a different door, because
    /// `RuntimeHandle::shutdown` stops the beamr scheduler while the armed tasks
    /// live on the tokio runtime and are not reached by it. On the release path
    /// there is no shutdown at all, so `Drop` is the only thing that can.
    ///
    /// The count is read through an `Arc<EngineNifState>` held ACROSS the drop,
    /// so the release is the only variable between the two readings. The
    /// pre-drop assertion is the control and it is not decoration: it is the
    /// exact vacuity this test exists to close.
    #[tokio::test]
    async fn dropping_an_engine_without_shutdown_disarms_the_live_timer_wheel()
    -> Result<(), Box<dyn std::error::Error>> {
        use aion_store::EventStore;

        let store = Arc::new(InMemoryStore::default());
        let engine = EngineBuilder::new()
            .store_arc(Arc::clone(&store) as Arc<dyn EventStore>)
            .visibility_store(InMemoryStore::default())
            .build()
            .await?;
        // Held across the drop; see `EngineNifState::armed_wheel_timers`.
        let nif_state = Arc::clone(engine.runtime().nif_state());
        let (workflow_id, fire_at) = arm_resident_run_with_wheel_timer(&engine, &store).await?;
        assert_eq!(
            nif_state.armed_wheel_timers(),
            1,
            "control: the fixture must have actually armed a wheel timer, or the assertions below \
             are satisfied by an engine that never had one — the exact way the sibling test above \
             is blind to this half of `Drop`"
        );

        // 🔴 FIXTURE GUARD, NOT A CLAIM ABOUT `Drop`. Everything below reads a
        // timer that has NOT yet fired; if the deadline has already passed, a
        // `TimerFired` in history is the timer doing its job, and the decisive
        // assertion would report it as a task that survived the release. That is
        // a red which reassigns blame — the failure mode this guard exists to
        // make impossible to mistake. Both readings are taken before the drop,
        // so neither can be explained by it.
        let before_drop = Utc::now();
        assert!(
            before_drop < fire_at,
            "fixture: the arming window closed before the release was reached ({before_drop} is \
             not before {fire_at}). This is a FIXTURE timing failure — most likely a loaded box — \
             and NOT evidence about cancellation. Widen `WHEEL_TIMER_ARMING_MARGIN`; do not read \
             this as `Drop` leaking a task."
        );
        let armed = store.read_history(&workflow_id).await?;
        assert!(
            !armed
                .iter()
                .any(|event| matches!(event, Event::TimerFired { .. })),
            "fixture: the timer already fired before the engine was released, so this run cannot \
             say anything about cancellation. Same remedy as above — widen the margin: {armed:#?}"
        );

        drop(engine);

        assert_eq!(
            nif_state.armed_wheel_timers(),
            0,
            "an engine released without an explicit shutdown must empty its live timer wheel"
        );

        // 🔴 AND THE TASK ITSELF IS GONE, WHICH THE COUNT ABOVE CANNOT SEE.
        // `shutdown_timer_wheel` both removes the entry and aborts the handle;
        // the count observes only the removal, so deleting the abort leaves it
        // green while an armed task lives on. This waits past the deadline and
        // reads the durable record, which is where a surviving task would show
        // up — and the sibling test
        // `a_live_wheel_timer_fires_when_the_engine_is_not_released` is its
        // positive control: it proves this same fixture's timer DOES reach the
        // store when nothing disarms it, so an empty reading here is a
        // cancellation rather than a timer that was never going to arrive.
        tokio::time::sleep(WHEEL_TIMER_ARMING_MARGIN + WHEEL_TIMER_OBSERVATION_SLACK).await;
        let history = store.read_history(&workflow_id).await?;
        assert!(
            !history
                .iter()
                .any(|event| matches!(event, Event::TimerFired { .. })),
            "an engine released without an explicit shutdown must CANCEL its wheel tasks, not \
             merely forget them: a surviving task records `TimerFired` for a run this process no \
             longer owns, which is a second writer for one workflow: {history:#?}"
        );
        Ok(())
    }

    /// 🔴 AND A DRAIN IS NOT A GATE. The test above proves the wheel is EMPTIED
    /// by the release; this one proves it stays empty.
    ///
    /// `Drop for Engine` deliberately leaves the beamr scheduler running and the
    /// engine seams installed, so a workflow process still runnable can reach
    /// `sleep` a moment AFTER the drain and arm a fresh task — whose body is
    /// `fire_wheel_timer`, a durable `TimerFired` append against a run a
    /// successor engine may already own. That is the same second-writer breach
    /// the drain exists to prevent, reached one instant later, and until
    /// 2026-08-07 `arm_timer` had no check of any kind: it spawned
    /// unconditionally.
    ///
    /// The scheduling call is the PRODUCTION path, not a direct poke at the
    /// bridge, and it is made through an `Arc<EngineNifState>` held across the
    /// drop — the same instrument the sibling above uses, and available for the
    /// same reason (the drop does not clear the seams).
    ///
    /// Two controls, because refusal has two boring explanations. The first
    /// schedule succeeds BEFORE the drop, so the fixture demonstrably reaches
    /// the arming path at all; and the count is asserted `1` then `0` around the
    /// release, so the post-drop `Err` cannot be a fixture that was never able
    /// to arm anything.
    #[tokio::test]
    async fn arming_a_wheel_timer_after_the_engine_is_released_is_refused()
    -> Result<(), Box<dyn std::error::Error>> {
        use aion_store::EventStore;

        let store = Arc::new(InMemoryStore::default());
        let engine = EngineBuilder::new()
            .store_arc(Arc::clone(&store) as Arc<dyn EventStore>)
            .visibility_store(InMemoryStore::default())
            .build()
            .await?;
        let nif_state = Arc::clone(engine.runtime().nif_state());
        // CONTROL 1: this fixture can arm. The call below is the same
        // `TimerService::schedule` the post-drop attempt makes, and it succeeds.
        let (workflow_id, _fire_at) = arm_resident_run_with_wheel_timer(&engine, &store).await?;
        assert_eq!(
            nif_state.armed_wheel_timers(),
            1,
            "control: the fixture must have armed a wheel timer through the production path, or \
             the refusal below is satisfied by a fixture that could never arm one"
        );

        drop(engine);

        // CONTROL 2: the release emptied the wheel, so what follows is measured
        // against a torn-down wheel rather than a busy one.
        assert_eq!(nif_state.armed_wheel_timers(), 0);

        let refused = crate::runtime::nif_timer_bridge::installed_timer_service(&nif_state)
            .map_err(|error| format!("the released engine's timer seam is gone: {error}"))?
            .schedule(
                workflow_id,
                aion_core::TimerId::named("sleep-after-release")?,
                Utc::now() + chrono::Duration::from_std(WHEEL_TIMER_ARMING_MARGIN)?,
            )
            .await;
        let Err(error) = refused else {
            return Err(
                "arming a wheel timer through a released engine must be REFUSED: the task \
                        it spawns appends a durable `TimerFired` for a run this process no longer \
                        owns, which is a second writer for one workflow"
                    .into(),
            );
        };
        assert!(
            error.to_string().contains("torn down"),
            "the refusal must say WHY, so an operator reading it is not sent looking for a \
             missing workflow or a bad timer id: {error}"
        );
        assert_eq!(
            nif_state.armed_wheel_timers(),
            0,
            "the refused arm must leave nothing behind: a wheel entry inserted before the refusal \
             would be a durable writer with no owner to cancel it"
        );
        Ok(())
    }

    /// The positive control for the assertion above: this fixture's wheel timer
    /// really does reach the durable record when nothing disarms it.
    ///
    /// Without this, "no `TimerFired` after the drop" is equally well explained
    /// by a fixture whose timer could never fire at all — a registry entry the
    /// fire path rejects, a seam that was never installed, a deadline that never
    /// arrives. An absence is only evidence when its presence has been shown
    /// reachable by the same means.
    #[tokio::test]
    async fn a_live_wheel_timer_fires_when_the_engine_is_not_released()
    -> Result<(), Box<dyn std::error::Error>> {
        use aion_store::EventStore;

        let store = Arc::new(InMemoryStore::default());
        let engine = EngineBuilder::new()
            .store_arc(Arc::clone(&store) as Arc<dyn EventStore>)
            .visibility_store(InMemoryStore::default())
            .build()
            .await?;
        let (workflow_id, _) = arm_resident_run_with_wheel_timer(&engine, &store).await?;

        tokio::time::sleep(WHEEL_TIMER_ARMING_MARGIN + WHEEL_TIMER_OBSERVATION_SLACK).await;

        let history = store.read_history(&workflow_id).await?;
        assert!(
            history
                .iter()
                .any(|event| matches!(event, Event::TimerFired { .. })),
            "the fixture's wheel timer must reach the store when the engine is still held, or \
             the sibling test's absence proves nothing: {history:#?}"
        );
        drop(engine);
        Ok(())
    }

    /// A visibility store that counts the periodic reconciliation loop's passes.
    ///
    /// Delegates every method to a real [`InMemoryStore`] so the loop under
    /// observation does the same work it does in production; the counter is the
    /// only addition. `list_workflows` is the tick site because
    /// `reconcile_visibility` calls it unconditionally at the top of every pass,
    /// so the count cannot be lowered by the store happening to be consistent.
    #[derive(Default)]
    struct TickCountingVisibilityStore {
        inner: InMemoryStore,
        list_calls: std::sync::atomic::AtomicUsize,
    }

    impl TickCountingVisibilityStore {
        fn ticks(&self) -> usize {
            self.list_calls.load(std::sync::atomic::Ordering::Acquire)
        }
    }

    #[async_trait::async_trait]
    impl VisibilityStore for TickCountingVisibilityStore {
        async fn record_visibility(
            &self,
            record: aion_store::visibility::VisibilityRecord,
        ) -> Result<(), aion_store::StoreError> {
            self.inner.record_visibility(record).await
        }

        async fn list_workflows(
            &self,
            filter: ListWorkflowsFilter,
        ) -> Result<Vec<aion_store::visibility::WorkflowSummary>, aion_store::StoreError> {
            self.list_calls
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            self.inner.list_workflows(filter).await
        }

        async fn count_workflows(
            &self,
            filter: ListWorkflowsFilter,
        ) -> Result<u64, aion_store::StoreError> {
            self.inner.count_workflows(filter).await
        }
    }

    fn capacity(value: usize) -> Result<NonZeroUsize, Box<dyn std::error::Error>> {
        NonZeroUsize::new(value).ok_or_else(|| "capacity must be non-zero".into())
    }

    #[tokio::test]
    async fn event_streaming_delivers_recorder_appends_through_engine_subscribe()
    -> Result<(), Box<dyn std::error::Error>> {
        let engine = EngineBuilder::new()
            .store(InMemoryStore::default())
            .in_memory_visibility()
            .event_streaming(capacity(8)?)
            .build()
            .await?;
        let workflow_id = WorkflowId::new_v4();
        let mut subscription = engine.subscribe(crate::EventFilter {
            workflow_id: Some(workflow_id.clone()),
            run: None,
            family: None,
        });

        // The production append path: a Recorder over the engine's store,
        // which `event_streaming` wrapped before any recorder existed.
        let mut recorder = crate::durability::Recorder::new(workflow_id.clone(), engine.store());
        recorder
            .record_workflow_started(
                Utc::now(),
                crate::durability::WorkflowStartRecord {
                    workflow_type: "checkout".to_owned(),
                    input: payload()?,
                    run_id: aion_core::RunId::new(uuid::Uuid::from_u128(7)),
                    parent_run_id: None,
                    package_version: aion_core::PackageVersion::new("a".repeat(64)),
                },
            )
            .await?;

        let item = tokio::time::timeout(Duration::from_secs(2), subscription.next())
            .await?
            .ok_or("subscription ended without delivering the appended event")?;
        let event = item?;
        assert_eq!(event.workflow_id(), &workflow_id);
        assert_eq!(event.seq(), 1);
        assert!(matches!(event, Event::WorkflowStarted { .. }));
        engine.shutdown()?;
        Ok(())
    }

    #[tokio::test]
    async fn without_event_streaming_subscriptions_stay_on_deferred_empty_stream()
    -> Result<(), Box<dyn std::error::Error>> {
        let engine = EngineBuilder::new()
            .store(InMemoryStore::default())
            .in_memory_visibility()
            .build()
            .await?;

        let mut subscription = engine.subscribe(crate::EventFilter::default());
        let item = tokio::time::timeout(Duration::from_secs(2), subscription.next()).await?;

        assert!(item.is_none(), "deferred publisher streams must be empty");
        engine.shutdown()?;
        Ok(())
    }

    #[tokio::test]
    async fn event_streaming_conflicts_with_explicit_event_publisher()
    -> Result<(), Box<dyn std::error::Error>> {
        let error = EngineBuilder::new()
            .store(InMemoryStore::default())
            .in_memory_visibility()
            .event_publisher(Arc::new(crate::DeferredEventPublisher))
            .event_streaming(capacity(8)?)
            .build()
            .await
            .err();

        assert!(matches!(
            error,
            Some(EngineError::ConflictingEventPublisher)
        ));
        Ok(())
    }

    #[test]
    fn query_timeout_is_only_set_by_caller() {
        assert_eq!(EngineBuilder::new().configured_query_timeout(), None);
        assert_eq!(
            EngineBuilder::new()
                .query_timeout(Duration::from_secs(3))
                .configured_query_timeout(),
            Some(Duration::from_secs(3))
        );
    }

    async fn insert_running_workflow(
        engine: &crate::Engine,
    ) -> Result<(WorkflowId, aion_core::RunId), Box<dyn std::error::Error>> {
        let workflow_id = WorkflowId::new_v4();
        let run_id = aion_core::RunId::new_v4();
        let mut recorder = crate::durability::Recorder::new(workflow_id.clone(), engine.store());
        recorder
            .record_workflow_started(
                Utc::now(),
                crate::durability::WorkflowStartRecord {
                    workflow_type: "checkout".to_owned(),
                    input: payload()?,
                    run_id: run_id.clone(),
                    parent_run_id: None,
                    package_version: aion_core::PackageVersion::new("a".repeat(64)),
                },
            )
            .await?;
        let handle = crate::registry::WorkflowHandle::new(crate::registry::WorkflowHandleParts {
            workflow_id: workflow_id.clone(),
            run_id: run_id.clone(),
            pid: engine.runtime().spawn_test_process_with_trap_exit(true)?,
            workflow_type: "checkout".to_owned(),
            namespace: String::from("default"),
            loaded_version: aion_package::ContentHash::from_bytes([2; 32]),
            cached_status: WorkflowStatus::Running,
            residency: crate::registry::HandleResidency::Resident,
            recorder,
            completion: crate::registry::CompletionNotifier::new(),
        });
        engine
            .registry()
            .insert((workflow_id.clone(), run_id.clone()), handle)?;
        Ok((workflow_id, run_id))
    }

    #[tokio::test]
    async fn query_timeout_installs_the_concrete_query_seam()
    -> Result<(), Box<dyn std::error::Error>> {
        let engine = EngineBuilder::new()
            .store(InMemoryStore::default())
            .in_memory_visibility()
            .query_timeout(Duration::from_millis(250))
            .build()
            .await?;
        let (workflow_id, run_id) = insert_running_workflow(&engine).await?;

        // The concrete seam reached the query mailbox engine, which answers
        // an unregistered name with a typed UnknownQuery — the deferred seam
        // would have failed with its "not configured" runtime error instead.
        let result = engine
            .query(
                &workflow_id,
                &run_id,
                "state",
                aion_core::Payload::json_null(),
            )
            .await;

        assert!(matches!(
            result,
            Err(crate::EngineError::Query(crate::QueryError::UnknownQuery(name))) if name == "state"
        ));
        engine.shutdown()?;
        Ok(())
    }

    #[tokio::test]
    async fn without_query_timeout_the_query_seam_stays_deferred()
    -> Result<(), Box<dyn std::error::Error>> {
        let engine = EngineBuilder::new()
            .store(InMemoryStore::default())
            .in_memory_visibility()
            .build()
            .await?;
        let (workflow_id, run_id) = insert_running_workflow(&engine).await?;

        let result = engine
            .query(
                &workflow_id,
                &run_id,
                "state",
                aion_core::Payload::json_null(),
            )
            .await;

        assert!(matches!(
            result,
            Err(crate::EngineError::Runtime { reason }) if reason.contains("not configured")
        ));
        engine.shutdown()?;
        Ok(())
    }

    #[test]
    fn owned_shards_are_only_set_by_caller() {
        // The default builder configures NO shard restriction: `build()` never
        // touches the store's scoping hook, so single-node boot owns ALL shards
        // and is byte-identical to today.
        assert_eq!(EngineBuilder::new().configured_owned_shards(), None);
        assert_eq!(
            EngineBuilder::new()
                .owned_shards([2, 0, 2, 1])
                .configured_owned_shards(),
            Some([2, 0, 2, 1].as_slice())
        );
    }

    #[test]
    fn scheduler_threads_are_only_set_by_caller() {
        assert_eq!(EngineBuilder::new().scheduler_thread_count(), None);
        assert_eq!(
            EngineBuilder::new()
                .scheduler_threads(4)
                .scheduler_thread_count(),
            Some(4)
        );
    }

    /// The completion-retry ladder the caller set is the ladder the runtime is
    /// started with.
    ///
    /// Asserting on `runtime_config()` rather than on a builder accessor is the
    /// whole point: an accessor test would pass with the
    /// `.with_completion_retry(self.completion_retry)` line deleted, because the
    /// field would still hold what the setter put there while the runtime
    /// silently ran on the inherited default. This is the only place that link
    /// is held.
    ///
    /// The first assertion is the test's own control. It fixes that the chosen
    /// ladder differs from the default, so the equality below cannot be
    /// satisfied by a runtime configuration that ignored the builder entirely.
    #[test]
    fn the_completion_retry_ladder_reaches_the_runtime_configuration()
    -> Result<(), Box<dyn std::error::Error>> {
        let chosen = crate::runtime::CompletionRetryConfig::try_new(
            Duration::from_millis(250),
            Duration::from_secs(7),
        )?;
        assert_ne!(
            chosen,
            crate::runtime::CompletionRetryConfig::default(),
            "a ladder equal to the default could not distinguish wiring from inheritance"
        );

        assert_eq!(
            EngineBuilder::new()
                .completion_retry(chosen)
                .runtime_config()
                .completion_retry,
            chosen,
            "the runtime must be started with the operator's ladder, not the inherited default"
        );
        Ok(())
    }

    #[test]
    fn visibility_reconciliation_interval_is_only_set_by_caller() {
        let interval = Duration::from_millis(250);

        assert_eq!(
            EngineBuilder::new().configured_visibility_reconciliation_interval(),
            None
        );
        assert_eq!(
            EngineBuilder::new()
                .visibility_reconciliation_interval(interval)
                .configured_visibility_reconciliation_interval(),
            Some(interval)
        );
    }

    #[tokio::test]
    async fn duplicate_host_nif_mfa_returns_typed_error() {
        let mfa = Mfa::new("host", "zero", 0);
        let error = EngineBuilder::new()
            .store(InMemoryStore::default())
            .in_memory_visibility()
            .register_nifs([
                NifEntry::new(
                    mfa.clone(),
                    crate::runtime::nif::test_native_zero,
                    Determinism::Pure,
                ),
                NifEntry::dirty(
                    mfa,
                    crate::runtime::nif::test_native_zero,
                    Determinism::Pure,
                ),
            ])
            .build()
            .await
            .err();

        assert!(matches!(
            error,
            Some(EngineError::NifRegistration { reason }) if reason.contains("host:zero/0")
        ));
    }

    #[tokio::test]
    async fn empty_store_builds_coordinator_history_without_registry_or_supervision()
    -> Result<(), EngineError> {
        let store = Arc::new(InMemoryStore::default());
        let engine = EngineBuilder::new()
            .store_arc(store.clone())
            .in_memory_visibility()
            .build()
            .await?;

        assert!(engine.registry().list()?.is_empty());
        assert_eq!(engine.supervision().type_supervisor_count()?, 1);
        assert_eq!(engine.workflow_catalog().workflows()?.len(), 0);

        let coordinator_id = schedule_coordinator_workflow_id();
        let active = store.list_active().await?;
        assert_eq!(active, vec![coordinator_id.clone()]);
        let history = store.read_history(&coordinator_id).await?;
        let [started] = history.as_slice() else {
            return Err(EngineError::Load {
                reason: format!(
                    "expected exactly one coordinator event, found {}",
                    history.len()
                ),
            });
        };
        match started {
            Event::WorkflowStarted {
                workflow_type,
                input,
                run_id,
                parent_run_id,
                ..
            } => {
                assert_eq!(workflow_type, schedule_coordinator_workflow_type());
                assert_eq!(
                    input,
                    &Payload::from_json(&json!({})).map_err(|error| {
                        EngineError::Load {
                            reason: format!("failed to build expected payload: {error}"),
                        }
                    })?
                );
                assert_eq!(run_id, &schedule_coordinator_run_id());
                assert!(parent_run_id.is_none());
            }
            other => {
                return Err(EngineError::Load {
                    reason: format!("expected coordinator WorkflowStarted, found {other:?}"),
                });
            }
        }

        engine.shutdown()?;
        let rebuilt = EngineBuilder::new()
            .store_arc(store.clone())
            .in_memory_visibility()
            .build()
            .await?;
        let rebuilt_history = store.read_history(&coordinator_id).await?;
        assert_eq!(rebuilt_history.len(), 1);
        rebuilt.shutdown()?;

        Ok(())
    }

    #[tokio::test]
    async fn build_loads_already_loaded_package() -> Result<(), Box<dyn std::error::Error>> {
        if gleam_test_support::skip_if_unavailable() {
            return Ok(());
        }
        let package = fixture_package()?;
        let version = package.content_hash().clone();
        let deployed_entry_module = package.deployed_entry_module();

        let engine = EngineBuilder::new()
            .store(InMemoryStore::default())
            .in_memory_visibility()
            .load_workflows(package)
            .build()
            .await?;

        let loaded = engine
            .workflow_catalog()
            .get("counter", &version)?
            .ok_or("loaded package record missing")?;
        assert_eq!(loaded.deployed_entry_module(), deployed_entry_module);
        assert!(
            engine
                .runtime()
                .has_registered_module(&deployed_entry_module)
        );
        Ok(())
    }

    #[tokio::test]
    async fn startup_reconciliation_backfills_completed_visibility()
    -> Result<(), Box<dyn std::error::Error>> {
        let store = Arc::new(InMemoryStore::default());
        let completed_id = WorkflowId::new_v4();

        store
            .append(
                WriteToken::recorder(),
                &completed_id,
                &[
                    started(&completed_id, "billing")?,
                    completed(&completed_id)?,
                ],
                0,
            )
            .await?;

        let engine = EngineBuilder::new()
            .store_arc(store.clone())
            .visibility_store_arc(store.clone())
            .build()
            .await?;

        let summaries = store.list_workflows(ListWorkflowsFilter::default()).await?;
        let completed_summary = summaries
            .iter()
            .find(|summary| summary.workflow_id == completed_id)
            .ok_or("completed workflow missing from visibility")?;

        assert_eq!(completed_summary.status, WorkflowStatus::Completed);
        assert!(completed_summary.close_time.is_some());
        engine.shutdown()?;
        Ok(())
    }

    #[tokio::test]
    async fn periodic_visibility_reconciliation_repairs_gap_after_startup()
    -> Result<(), Box<dyn std::error::Error>> {
        let store = Arc::new(InMemoryStore::default());
        let engine = EngineBuilder::new()
            .store_arc(store.clone())
            .visibility_store_arc(store.clone())
            .visibility_reconciliation_interval(Duration::from_millis(25))
            .build()
            .await?;
        let workflow_id = WorkflowId::new_v4();

        store
            .append(
                WriteToken::recorder(),
                &workflow_id,
                &[started(&workflow_id, "checkout")?],
                0,
            )
            .await?;

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let summaries = store.list_workflows(ListWorkflowsFilter::default()).await?;
                if summaries.iter().any(|summary| {
                    summary.workflow_id == workflow_id && summary.status == WorkflowStatus::Running
                }) {
                    return Ok::<(), aion_store::StoreError>(());
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await??;

        engine.shutdown()?;
        Ok(())
    }

    #[tokio::test]
    async fn build_loads_package_from_path() -> Result<(), Box<dyn std::error::Error>> {
        if gleam_test_support::skip_if_unavailable() {
            return Ok(());
        }
        let package = fixture_package()?;
        let version = package.content_hash().clone();
        let path = write_fixture_package(&package)?;

        let engine = EngineBuilder::new()
            .store(InMemoryStore::default())
            .in_memory_visibility()
            .load_workflows(path.as_path())
            .build()
            .await?;
        std::fs::remove_file(path)?;

        assert!(
            engine
                .workflow_catalog()
                .get("counter", &version)?
                .is_some()
        );
        Ok(())
    }
}
