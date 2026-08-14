//! Connected-worker registry keyed by worker-pool address and activity type.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use aion_core::{
    ClusterEvent, DeploymentAssociation, InterventionCapabilities, WorkerDeathReason,
    WorkerTransport,
};
use aion_proto::{ProtoActivityTask, ProtoCancelActivity, ProtoLivenessPing, ProtoRegisterWorker};
use aion_store::{NamespaceOrigin, NamespacePlacement, NamespaceStore, WorkerDeploymentStore};
use tokio::sync::{Notify, mpsc};

use crate::cluster_publisher::ClusterEventPublisher;
use crate::config::AutoCreate;
use crate::error::ServerError;
use crate::namespace::{CallerIdentity, NamespaceGuard, NamespaceMinter, NamespaceOperation};
use crate::observability::Metrics;
use crate::worker::admission_audit::AdmissionAudit;

/// The literal task queue an empty/absent selector normalizes to.
///
/// A worker-pool address has two disjoint dimensions; the second one
/// (`task_queue`) is a liveness selector, not a correctness boundary. An empty
/// `task_queue` is normalized to this one named default pool so a producer that
/// names no queue and a worker that advertises none both land on the same pool.
///
/// Re-exported from [`aion_core::DEFAULT_TASK_QUEUE`] so the server cannot drift
/// from the canonical domain default; the name is kept stable here for existing
/// call sites.
pub use aion_core::DEFAULT_TASK_QUEUE;

/// Server-side handle used to push activity tasks to a connected worker stream.
pub type WorkerTaskSender = mpsc::Sender<WorkerMessage>;

/// Transport through which the server delivers a dispatch to a registered worker.
///
/// A worker is selected the SAME way regardless of transport (`select_worker`
/// over the `(namespace, task_queue, node)` pool key); only the delivery leg
/// differs. The default gRPC path pushes a [`WorkerMessage`] onto the worker's
/// stream `mpsc` ([`WorkerDelivery::Grpc`]); a liminal-connected worker is
/// delivered to by pushing the dispatch out on its existing liminal connection
/// ([`WorkerDelivery::Liminal`], feature-gated). This enum is the minimal
/// transport-agnostic seam: the registry holds it on each [`WorkerHandle`], and
/// the dispatch path reads the variant it needs. The gRPC variant carries exactly
/// the `mpsc::Sender` it always did, so the gRPC dispatch path is unchanged.
#[derive(Clone, Debug)]
pub enum WorkerDelivery {
    /// gRPC stream delivery: the dispatch path pushes a [`WorkerMessage`] onto
    /// this `mpsc` sender, exactly as before this enum existed.
    Grpc(WorkerTaskSender),
    /// Liminal server-push delivery: the dispatch path pushes the serialized
    /// dispatch out on the worker's existing liminal connection and awaits the
    /// correlated reply. Carries the connection identity needed to address that
    /// push.
    #[cfg(feature = "liminal-transport")]
    Liminal(crate::worker::liminal_transport::LiminalWorkerDelivery),
}

/// Message queued from server-side dispatch/shutdown into a worker stream writer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkerMessage {
    /// Activity invocation pushed to a worker.
    ActivityTask(Box<ProtoActivityTask>),
    /// Graceful-shutdown notification; no new work will be dispatched.
    DrainRequest,
    /// Transport liveness ping pushed by the liveness probe (#197).
    ///
    /// It travels this channel — the SAME one dispatches travel — deliberately.
    /// The fact the probe needs is whether the server can reach this worker's
    /// DISPATCH path, and a ping on a parallel channel could be answered by a
    /// process whose dispatch path is dead. Answered by the worker SDK RUNTIME,
    /// never by action code.
    LivenessPing(ProtoLivenessPing),
    /// Ask the worker to stop ONE in-flight activity (#233).
    ///
    /// A request, not a guarantee: pushing this proves only that the server
    /// asked. Whether the work stops depends on the worker still holding the
    /// activity and on the action being interruptible, and neither is
    /// observable from here.
    ///
    /// It travels this channel — the SAME one the dispatch travelled — so the
    /// cancel cannot overtake or bypass the task it interrupts, and so a worker
    /// whose dispatch path is dead cannot appear to have been told.
    CancelActivity(ProtoCancelActivity),
}

/// Address of a worker pool: the two disjoint routing dimensions that select a
/// pool, before an `activity_type` is matched within it.
///
/// `namespace` is the correctness/isolation boundary — a workflow's activities
/// only ever reach workers in the workflow's namespace, so crossing it is a bug.
/// `task_queue` is the pool/flavour selector within that namespace (norn /
/// claude / cpu / gpu) — a miss is a liveness issue, never a correctness one.
///
/// This is a named type rather than a `(String, String)` tuple so a `node`
/// dimension (Tier 3 affinity) can be added later without re-threading every
/// call site.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PoolAddress {
    namespace: String,
    task_queue: String,
}

impl PoolAddress {
    /// Build a pool address, normalizing an empty `task_queue` to the named
    /// [`DEFAULT_TASK_QUEUE`] pool. The `namespace` is the authorization
    /// boundary and is never normalized.
    #[must_use]
    pub fn new(namespace: impl Into<String>, task_queue: impl Into<String>) -> Self {
        let task_queue = task_queue.into();
        let task_queue = if task_queue.is_empty() {
            String::from(DEFAULT_TASK_QUEUE)
        } else {
            task_queue
        };
        Self {
            namespace: namespace.into(),
            task_queue,
        }
    }

    /// The correctness/isolation boundary of this pool.
    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// The pool/flavour selector within the namespace.
    #[must_use]
    pub fn task_queue(&self) -> &str {
        &self.task_queue
    }
}

/// Registry match key: a worker-pool address plus the activity type matched
/// within that pool. A named type (not an anonymous tuple) so the routing
/// identity stays self-describing and extensible.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct ActivityKey {
    pool: PoolAddress,
    activity_type: String,
}

impl ActivityKey {
    fn new(pool: PoolAddress, activity_type: impl Into<String>) -> Self {
        Self {
            pool,
            activity_type: activity_type.into(),
        }
    }
}

type WorkerMap = HashMap<WorkerId, WorkerHandle>;
type RegistryMap = HashMap<ActivityKey, WorkerMap>;

/// Stable identifier assigned to a connected worker stream.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerId(u64);

impl WorkerId {
    /// Build a worker id from the numeric value exposed by administrative and
    /// wire surfaces.
    #[must_use]
    pub const fn from_value(value: u64) -> Self {
        Self(value)
    }

    /// Raw numeric value, as carried by the wire `RegisterAck.worker_id` so
    /// workers can correlate their logs with the server's.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Worker-supplied identity for one instance of a durable deployment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerInstanceIdentity {
    /// Durable deployment name supplied on the wire.
    pub deployment: String,
    /// Operator/launcher-assigned instance identifier.
    pub instance_id: String,
    /// Deployment-store lookup result captured when registration was accepted.
    pub association: DeploymentAssociation,
}

struct RegistrationOptions {
    instance: Option<WorkerInstanceIdentity>,
    intervention_capabilities: InterventionCapabilities,
}

/// Cloneable handle for a registered worker stream.
///
/// A worker serves a SET of namespaces under a single `task_queue`, so it is
/// indexed under one `(namespace, task_queue, activity_type)` key per namespace
/// in its set. `node` is an OPTIONAL locality affinity (a locality, not a
/// process — many handles may share a node id) used as a within-pool filter at
/// selection time; `None` means the worker advertised no locality.
#[derive(Clone, Debug)]
pub struct WorkerHandle {
    id: WorkerId,
    namespaces: BTreeSet<String>,
    task_queue: String,
    node: Option<String>,
    activity_types: BTreeSet<String>,
    instance: Option<WorkerInstanceIdentity>,
    delivery: WorkerDelivery,
    /// The neutral mid-run intervention primitives this worker's harness advertises
    /// support for (NOI-6). The server gates every intervention command on THIS set
    /// and NEVER routes an unadvertised primitive. Empty = observability-only (the
    /// default for every non-agent worker), so a normal activity worker advertises
    /// no controls and the intervention router refuses every command for it.
    intervention_capabilities: InterventionCapabilities,
}

impl WorkerHandle {
    /// Worker identifier assigned by this server process.
    #[must_use]
    pub const fn id(&self) -> WorkerId {
        self.id
    }

    /// Namespaces authorized for this worker stream. The worker is reachable for
    /// a dispatch only when its set includes the workflow's namespace.
    #[must_use]
    pub const fn namespaces(&self) -> &BTreeSet<String> {
        &self.namespaces
    }

    /// Task queue (pool/flavour) this worker serves within each namespace.
    #[must_use]
    pub fn task_queue(&self) -> &str {
        &self.task_queue
    }

    /// Optional locality affinity this worker advertised. `None` means the
    /// worker carries no node and is reachable only for unpinned dispatches.
    #[must_use]
    pub fn node(&self) -> Option<&str> {
        self.node.as_deref()
    }

    /// Activity types advertised by this worker.
    #[must_use]
    pub fn activity_types(&self) -> &BTreeSet<String> {
        &self.activity_types
    }

    /// Optional durable deployment/instance association supplied at registration.
    #[must_use]
    pub const fn instance(&self) -> Option<&WorkerInstanceIdentity> {
        self.instance.as_ref()
    }

    /// The transport this worker is delivered to through.
    #[must_use]
    pub const fn delivery(&self) -> &WorkerDelivery {
        &self.delivery
    }

    /// The neutral intervention primitives this worker's harness advertises (NOI-6).
    ///
    /// The intervention router gates on this set and never routes an unadvertised
    /// primitive. Empty (the default for a plain activity worker) means the worker
    /// is observability-only: the router refuses every intervention command for it.
    #[must_use]
    pub const fn intervention_capabilities(&self) -> &InterventionCapabilities {
        &self.intervention_capabilities
    }

    /// gRPC stream sender used by the gRPC dispatch path to push work, or `None`
    /// when this worker is delivered to over a non-gRPC transport (liminal).
    ///
    /// The gRPC dispatch path registers every worker with a [`WorkerDelivery::Grpc`]
    /// delivery, so this is always `Some` for a gRPC-registered worker — the
    /// behaviour the path relied on before delivery became transport-agnostic.
    #[must_use]
    pub fn sender(&self) -> Option<&WorkerTaskSender> {
        match &self.delivery {
            WorkerDelivery::Grpc(sender) => Some(sender),
            #[cfg(feature = "liminal-transport")]
            WorkerDelivery::Liminal(_) => None,
        }
    }
}

#[derive(Debug)]
struct RegistryState {
    next_worker_id: u64,
    workers: BTreeMap<WorkerId, WorkerHandle>,
    by_activity: RegistryMap,
    /// Round-robin cursor per `(namespace, task_queue, activity_type)` triple, so
    /// each pool rotates independently of every other pool.
    rotation: HashMap<ActivityKey, usize>,
    /// When a worker advertising a given `(pool, activity_type)` and node last
    /// LEFT service, per advertised node. Read by [`ConnectedWorkerRegistry::pool_census`]
    /// to answer "how old is the last compatible poller" for an address that has
    /// none right now (R1). Node-keyed because a node-pinned dispatch's notion
    /// of "compatible" is node-specific: a worker still serving the activity on
    /// another node must never make a pinned address look freshly served.
    last_departure: HashMap<ActivityKey, BTreeMap<Option<String>, Instant>>,
    /// Workers the server currently cannot reach on the server-to-worker push
    /// leg, and which are therefore excluded from selection while they remain
    /// so.
    ///
    /// Held here rather than derived at selection time because selection must
    /// not take the heartbeat tracker's lock: the liveness probe owns the
    /// evidence and publishes the verdict, and selection only reads it.
    ///
    /// This is exclusion from DISPATCH, not deregistration. An unreachable
    /// worker stays registered, keeps its in-flight work, and becomes eligible
    /// again the moment a ping is answered — nothing about it is torn down on
    /// the strength of a push failure.
    dispatch_ineligible: BTreeSet<WorkerId>,
}

impl Default for RegistryState {
    fn default() -> Self {
        Self {
            next_worker_id: 1,
            workers: BTreeMap::new(),
            by_activity: HashMap::new(),
            rotation: HashMap::new(),
            last_departure: HashMap::new(),
            dispatch_ineligible: BTreeSet::new(),
        }
    }
}

/// Cloneable registry of currently connected worker streams.
#[derive(Clone)]
pub struct ConnectedWorkerRegistry {
    inner: Arc<Mutex<RegistryState>>,
    metrics: Option<Metrics>,
    /// WS3 cluster-event publisher: emits `WorkerConnected`/`WorkerDisconnected`
    /// topology deltas on register/deregister. `None` keeps existing
    /// constructions (and every test) silent, exactly like `metrics`.
    cluster_publisher: Option<ClusterEventPublisher>,
    /// Minted-on-use hook (Control-Plane Phase 1). `None` disables minting, so
    /// registration is byte-identical to before the registry existed; `Some`
    /// durably records (open) or gates (closed) each authorized namespace.
    minter: Option<NamespaceMinter>,
    /// Durable deployment store used only to classify instance associations.
    deployment_store: Option<Arc<dyn WorkerDeploymentStore>>,
    worker_arrived: Arc<Notify>,
    /// The refusal side of this registry's ledger, shared by every transport.
    ///
    /// The registry records who was ADMITTED; this records who was turned away,
    /// so a repeated identical refusal can be met with silence. It lives here
    /// because both callers of the admission gate already hold the registry, so
    /// one home serves both — and one shared record is what stops the two
    /// transports drifting apart the way #147 found them.
    audit: Arc<AdmissionAudit>,
}

impl std::fmt::Debug for ConnectedWorkerRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConnectedWorkerRegistry")
            .field(
                "deployment_store_attached",
                &self.deployment_store.is_some(),
            )
            .finish_non_exhaustive()
    }
}

impl Default for ConnectedWorkerRegistry {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(RegistryState::default())),
            metrics: None,
            cluster_publisher: None,
            minter: None,
            deployment_store: None,
            worker_arrived: Arc::new(Notify::new()),
            audit: Arc::new(AdmissionAudit::new()),
        }
    }
}

impl ConnectedWorkerRegistry {
    /// Build a registry that records connected-worker gauge updates.
    #[must_use]
    pub fn with_metrics(metrics: Metrics) -> Self {
        Self {
            inner: Arc::new(Mutex::new(RegistryState::default())),
            metrics: Some(metrics),
            cluster_publisher: None,
            minter: None,
            deployment_store: None,
            worker_arrived: Arc::new(Notify::new()),
            audit: Arc::new(AdmissionAudit::new()),
        }
    }

    /// The admission audit every registration transport names its refusals
    /// through. Clones of a registry share one, so a worker refused over gRPC
    /// and then over liminal is one site, not two.
    #[must_use]
    pub fn admission_audit(&self) -> &AdmissionAudit {
        &self.audit
    }

    /// Attach the WS3 cluster-event publisher so worker topology changes are
    /// pushed to the dashboard. Pure builder addition.
    #[must_use]
    pub fn with_cluster_publisher(mut self, publisher: ClusterEventPublisher) -> Self {
        self.cluster_publisher = Some(publisher);
        self
    }

    /// Attach the durable worker-deployment store used for association lookup.
    #[must_use]
    pub fn with_worker_deployment_store(mut self, store: Arc<dyn WorkerDeploymentStore>) -> Self {
        self.deployment_store = Some(store);
        self
    }

    /// Install the minted-on-use namespace hook (Control-Plane Phase 1).
    ///
    /// After a registration is authorized and its namespace set scoped, each
    /// authorized namespace is durably recorded ([`AutoCreate::Open`]) or gated
    /// ([`AutoCreate::Closed`]) through `store`. Without this builder the
    /// registry never touches the namespace registry, so registration stays
    /// byte-identical to before the registry existed. Pure builder addition,
    /// mirroring [`Self::with_cluster_publisher`].
    ///
    /// When a cluster publisher has already been attached
    /// ([`Self::with_cluster_publisher`], called first on the boot path), it is
    /// threaded into the minter so a first worker-mint emits the live
    /// `namespace created` delta to the ops console (S8). Order-independence is
    /// not assumed: callers wire the publisher before minting on the boot path.
    #[must_use]
    pub fn with_namespace_minting(
        mut self,
        store: Arc<dyn NamespaceStore>,
        policy: AutoCreate,
    ) -> Self {
        let minter = NamespaceMinter::new(store, policy);
        let minter = match &self.cluster_publisher {
            Some(publisher) => minter.with_cluster_publisher(publisher.clone()),
            None => minter,
        };
        self.minter = Some(minter);
        self
    }

    /// Thread the boot's namespace-mint routing context into the registry's
    /// minter, so a worker registering for a namespace whose registry shard this
    /// node does not own mints through the shard's owner instead of being
    /// refused `NotOwner` forever.
    ///
    /// The SECOND of the two minter construction sites (the first is
    /// [`ServerState::namespace_minter`](crate::ServerState::namespace_minter),
    /// which serves the gRPC and HTTP start seams). Called after
    /// [`Self::with_namespace_minting`] on the boot path; a no-op when no minter
    /// is installed, and never called at all off-cluster, so default/test
    /// registries stay byte-identical.
    #[must_use]
    pub fn with_namespace_routing(mut self, routing: crate::namespace::NamespaceRouting) -> Self {
        self.minter = self.minter.map(|minter| minter.with_routing(routing));
        self
    }

    /// Authorize a worker registration and insert it into the connected-worker registry.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] if namespace authorization fails or the registry lock is poisoned.
    pub async fn accept_registration(
        &self,
        guard: &NamespaceGuard,
        caller: &CallerIdentity,
        registration: &ProtoRegisterWorker,
        sender: WorkerTaskSender,
    ) -> Result<WorkerRegistration, ServerError> {
        // Verify the operation against the guard's worker-registration policy,
        // then authorize EACH namespace in the worker's set: a worker serves a
        // SET of correctness boundaries, so the registration is denied unless
        // the caller is granted every one. The wire's empty `node` carries no
        // locality affinity; a non-empty value is the worker's advertised node.
        guard
            .scope(caller, &NamespaceOperation::register_worker(registration))
            .await?;
        let namespaces = guard.scope_worker_namespaces(caller, &registration.namespaces)?;
        // MINT HOOK (Control-Plane Phase 1). This runs strictly AFTER the
        // per-namespace authorization above (`scope` + `scope_worker_namespaces`),
        // so it can only ever mint a namespace the caller is already authorized
        // for — the mint is auth-scoped by construction (CVE-2025-14986: open
        // minting and namespace isolation only coexist when minting is
        // auth-gated). It runs BEFORE the worker is inserted, so a `closed`
        // rejection never leaves a half-registered worker behind.
        self.mint_or_gate_namespaces(&namespaces).await?;
        let node = optional_node(&registration.node);
        // PLACEMENT-ADMISSION GATE (Control-Plane Phase 2, P2-I1). Runs strictly
        // AFTER the mint hook (so every authorized namespace has a durable record to
        // read a placement from) and with BOTH the worker's advertised `node` and
        // the full authorized namespace set in scope. It rejects the WHOLE
        // registration (Open Decision 6) when the worker's node violates any
        // `Pinned{L}` namespace it would serve, so only L-node workers ever enter a
        // hard-pinned namespace's pool. Auth-scoped by construction (it only ever
        // gates a namespace already authorized above); a no-op with no minter
        // installed, so default/test registries stay byte-identical.
        self.enforce_pinned_placement(&namespaces, node.as_deref())
            .await?;
        let instance = self
            .resolve_instance_identity(registration.instance.as_ref())
            .await?;
        self.register_delivery(
            namespaces,
            registration.task_queue.clone(),
            node,
            instance,
            registration.activity_types.iter(),
            WorkerDelivery::Grpc(sender),
        )
    }

    /// Resolve a wire instance identity without refusing unknown deployments.
    ///
    /// Workers connect inward, forever: the server never holds an exhaustive
    /// roster of launchers, and a worker may outlive the deployment record
    /// that spawned it (record deleted, store swapped, or the worker predates
    /// the record entirely). Refusing an unknown deployment name here would
    /// orphan real, servable workers on exactly the recovery paths where they
    /// matter most. The association is therefore observational — the record's
    /// existence is captured as a Known/Absent/Unchecked state for supervision
    /// to read (Unchecked means no store was attached) —
    /// and never an admission gate. Admission is decided solely by the
    /// contract checks that precede this call.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] when durable deployment lookup fails.
    pub async fn resolve_instance_identity(
        &self,
        instance: Option<&aion_proto::ProtoWorkerInstanceIdentity>,
    ) -> Result<Option<WorkerInstanceIdentity>, ServerError> {
        let Some(instance) = instance else {
            return Ok(None);
        };
        let association = match &self.deployment_store {
            Some(store) => {
                if store
                    .get_worker_deployment(&instance.deployment)
                    .await
                    .map_err(ServerError::from)?
                    .is_some()
                {
                    DeploymentAssociation::Known
                } else {
                    DeploymentAssociation::Absent
                }
            }
            None => DeploymentAssociation::Unchecked,
        };
        Ok(Some(WorkerInstanceIdentity {
            deployment: instance.deployment.clone(),
            instance_id: instance.instance_id.clone(),
            association,
        }))
    }

    /// Apply the minted-on-use policy to an already-authorized namespace set.
    ///
    /// A no-op when no minter is installed (every default/test registry), so
    /// registration stays byte-identical. With a minter, the work is delegated
    /// to the shared [`NamespaceMinter::mint_or_gate`] — the single
    /// transport-agnostic implementation reused by the workflow-start safety net
    /// — with [`NamespaceOrigin::WorkerMint`] so a first mint is attributed to
    /// worker registration. See that method for the open/closed policy, the
    /// idempotent "namespace created" event, and the retryable `NotOwner`
    /// surface.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::StoreBackend`] if a durable upsert/lookup fails
    /// (including a retryable `NotOwner` fence), or [`ServerError::Namespace`]
    /// when `closed` rejects an unknown namespace.
    async fn mint_or_gate_namespaces(&self, namespaces: &[String]) -> Result<(), ServerError> {
        let Some(minter) = &self.minter else {
            return Ok(());
        };
        minter
            .mint_or_gate(namespaces, NamespaceOrigin::WorkerMint)
            .await
    }

    /// Reject the whole registration when the worker's advertised `node` violates
    /// any `Pinned{L}` namespace it would serve (Control-Plane Phase 2, P2-I1).
    ///
    /// For each authorized namespace whose placement is [`NamespacePlacement::Pinned`],
    /// the worker's advertised `node` must be `Some(n)` with `n ∈ L`; a `None` node
    /// or an `n ∉ L` is a loud, whole-registration rejection naming the namespace,
    /// the node, and the required set. This guarantees only L-node workers ever
    /// serve a hard-pinned namespace's pool, which is exactly what lets the
    /// `Some(N ∉ L)` composition case (§2.2) resolve to the correct isolation stall
    /// at dispatch rather than needing a start-time enumeration of future nodes.
    ///
    /// Non-`Pinned` placements ([`NamespacePlacement::Unplaced`]/[`NamespacePlacement::Prefer`])
    /// are UNAFFECTED — byte-identical registration. A no-op when no minter is
    /// installed (every default/test registry), so those stay behaviour-identical:
    /// the gate reads placement from the SAME registry record the minter/placement
    /// endpoint writes, never a second source of truth.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::Namespace`] (placement-admission denial) when the
    /// worker's node violates a `Pinned` namespace, or [`ServerError::StoreBackend`]
    /// if a placement read fails at the backend.
    async fn enforce_pinned_placement(
        &self,
        namespaces: &[String],
        node: Option<&str>,
    ) -> Result<(), ServerError> {
        let Some(minter) = &self.minter else {
            return Ok(());
        };
        for namespace in namespaces {
            let NamespacePlacement::Pinned { nodes } = minter.placement_of(namespace).await? else {
                continue;
            };
            let admitted = node.is_some_and(|n| nodes.contains(n));
            if !admitted {
                return Err(ServerError::placement_admission_denied(
                    namespace, node, &nodes,
                ));
            }
        }
        Ok(())
    }

    /// Insert an already-authorized worker stream into the default task queue of
    /// a single `namespace`, with no node affinity.
    ///
    /// Convenience over [`Self::register_namespaces`] for callers that serve one
    /// namespace and do not select a task queue (notably tests of the default
    /// pool).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn register<'a>(
        &self,
        namespace: impl Into<String>,
        activity_types: impl IntoIterator<Item = &'a String>,
        sender: WorkerTaskSender,
    ) -> Result<WorkerRegistration, ServerError> {
        self.register_namespaces(
            [namespace.into()],
            String::from(DEFAULT_TASK_QUEUE),
            None,
            activity_types,
            sender,
        )
    }

    /// Insert an already-authorized worker stream into one explicit worker pool
    /// (single namespace + task queue), with no node affinity.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn register_pool<'a>(
        &self,
        pool: PoolAddress,
        activity_types: impl IntoIterator<Item = &'a String>,
        sender: WorkerTaskSender,
    ) -> Result<WorkerRegistration, ServerError> {
        let PoolAddress {
            namespace,
            task_queue,
        } = pool;
        self.register_namespaces([namespace], task_queue, None, activity_types, sender)
    }

    /// Insert an already-authorized worker stream serving a SET of namespaces
    /// under one `task_queue`, with an optional `node` locality affinity.
    ///
    /// The worker is indexed under one `(namespace, task_queue, activity_type)`
    /// key per namespace in its set, so a dispatch in any of those namespaces
    /// can reach it. `node` is recorded on the handle and used only as a
    /// within-pool filter at selection time — it is NOT part of [`PoolAddress`].
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn register_namespaces<'a>(
        &self,
        namespaces: impl IntoIterator<Item = String>,
        task_queue: impl Into<String>,
        node: Option<String>,
        activity_types: impl IntoIterator<Item = &'a String>,
        sender: WorkerTaskSender,
    ) -> Result<WorkerRegistration, ServerError> {
        self.register_delivery(
            namespaces,
            task_queue,
            node,
            None,
            activity_types,
            WorkerDelivery::Grpc(sender),
        )
    }

    /// Insert an already-authorized worker serving a SET of namespaces under one
    /// `task_queue` and optional `node`, delivered to through an explicit
    /// [`WorkerDelivery`] transport.
    ///
    /// This is the transport-agnostic registration core: [`Self::register_namespaces`]
    /// is the gRPC façade over it (it wraps the stream sender in
    /// [`WorkerDelivery::Grpc`]). Selection (`select_worker`/`workers_for`) is
    /// identical across transports; only the held delivery differs.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn register_delivery<'a>(
        &self,
        namespaces: impl IntoIterator<Item = String>,
        task_queue: impl Into<String>,
        node: Option<String>,
        instance: Option<WorkerInstanceIdentity>,
        activity_types: impl IntoIterator<Item = &'a String>,
        delivery: WorkerDelivery,
    ) -> Result<WorkerRegistration, ServerError> {
        self.register_delivery_inner(
            namespaces,
            task_queue,
            node,
            activity_types,
            delivery,
            RegistrationOptions {
                instance,
                intervention_capabilities: InterventionCapabilities::none(),
            },
        )
    }

    /// Insert an already-authorized worker exactly like [`Self::register_delivery`],
    /// additionally recording the neutral [`InterventionCapabilities`] its harness
    /// advertises (NOI-6).
    ///
    /// This is the capability-carrying registration core: [`Self::register_delivery`]
    /// is the façade over it that advertises the empty set (observability-only), so
    /// every existing caller stays byte-identical. Selection is unchanged — the
    /// capability set is metadata the intervention router gates on, never a routing
    /// dimension.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn register_delivery_with_capabilities<'a>(
        &self,
        namespaces: impl IntoIterator<Item = String>,
        task_queue: impl Into<String>,
        node: Option<String>,
        activity_types: impl IntoIterator<Item = &'a String>,
        delivery: WorkerDelivery,
        intervention_capabilities: InterventionCapabilities,
    ) -> Result<WorkerRegistration, ServerError> {
        self.register_delivery_inner(
            namespaces,
            task_queue,
            node,
            activity_types,
            delivery,
            RegistrationOptions {
                instance: None,
                intervention_capabilities,
            },
        )
    }

    fn register_delivery_inner<'a>(
        &self,
        namespaces: impl IntoIterator<Item = String>,
        task_queue: impl Into<String>,
        node: Option<String>,
        activity_types: impl IntoIterator<Item = &'a String>,
        delivery: WorkerDelivery,
        options: RegistrationOptions,
    ) -> Result<WorkerRegistration, ServerError> {
        let namespaces = namespaces.into_iter().collect::<BTreeSet<_>>();
        let task_queue = task_queue.into();
        let activity_types = activity_types.into_iter().cloned().collect::<BTreeSet<_>>();
        let mut state = self.state()?;
        let worker_id = WorkerId(state.next_worker_id);
        state.next_worker_id = state.next_worker_id.saturating_add(1);

        // Capture the node affinity for the WS3 WorkerConnected delta before the
        // handle moves it.
        let node_for_event = node.clone();
        let instance_for_event = options.instance.clone();
        let handle = WorkerHandle {
            id: worker_id,
            namespaces: namespaces.clone(),
            task_queue: task_queue.clone(),
            node,
            activity_types: activity_types.clone(),
            instance: options.instance,
            delivery,
            intervention_capabilities: options.intervention_capabilities,
        };

        for namespace in &namespaces {
            let pool = PoolAddress::new(namespace.clone(), task_queue.clone());
            for activity_type in &activity_types {
                let key = ActivityKey::new(pool.clone(), activity_type.clone());
                // This address is served again, so its departure record has no
                // remaining meaning — drop it in lockstep with the insert. The
                // departure map is therefore bounded by the addresses real
                // workers have served and left, never by caller-supplied
                // dispatch strings (which never create an entry at all).
                if let Some(by_node) = state.last_departure.get_mut(&key) {
                    by_node.remove(&handle.node);
                    if by_node.is_empty() {
                        state.last_departure.remove(&key);
                    }
                }
                state
                    .by_activity
                    .entry(key)
                    .or_default()
                    .insert(worker_id, handle.clone());
            }
        }
        let transport = transport_of(&handle.delivery);
        state.workers.insert(worker_id, handle);
        drop(state);

        if let Some(metrics) = &self.metrics {
            for namespace in &namespaces {
                metrics.worker_connected(namespace);
            }
        }

        // WS3: one WorkerConnected delta carrying the full namespace set (the
        // event is namespace-list-valued; the deploy-scoped cluster channel sees
        // it whole). Edge-triggered by the real insert, never a poll.
        if let Some(publisher) = &self.cluster_publisher {
            let namespaces_vec: Vec<String> = namespaces.iter().cloned().collect();
            let task_queue_owned = task_queue.clone();
            drop(publisher.emit(|meta| {
                ClusterEvent::WorkerConnected {
                    meta,
                    worker_id: worker_id.value().to_string(),
                    namespaces: namespaces_vec,
                    task_queue: task_queue_owned,
                    transport,
                    node: node_for_event,
                    deployment: instance_for_event
                        .as_ref()
                        .map(|identity| identity.deployment.clone()),
                    deployment_association: instance_for_event.map(|identity| identity.association),
                }
            }));
        }

        self.worker_arrived.notify_waiters();

        Ok(WorkerRegistration {
            registry: self.clone(),
            parts: Some(WorkerRegistrationParts {
                worker_id,
                namespaces,
                task_queue,
                activity_types,
            }),
        })
    }

    /// Wait until at least one new worker registers.
    ///
    /// Returns immediately if a registration occurred since the last call.
    /// Callers should re-check the registry after waking — the newly arrived
    /// worker may not serve the namespace or activity type the caller needs.
    pub async fn wait_for_worker(&self) {
        self.worker_arrived.notified().await;
    }

    /// Return a snapshot of workers registered for the
    /// `(namespace, task_queue, activity_type)` pool, ordered by worker id and
    /// then rotated so each call starts from the next worker in the pool. The
    /// rotation cursor is per triple, so each pool round-robins independently.
    ///
    /// When `node` is `Some`, the result is filtered to workers whose advertised
    /// node equals it — a dispatch pinned to a node reaches only workers on that
    /// node (NODE affinity = require). When `node` is `None`, the behaviour is
    /// exactly the unpinned pool: every worker in the `(namespace, task_queue)`
    /// pool is a candidate regardless of locality. node is a within-pool filter,
    /// NOT part of the pool key, so the per-triple rotation cursor is shared
    /// across pinned and unpinned lookups of the same pool.
    ///
    /// The id sort matters: `by_activity` holds workers in a `HashMap`, whose
    /// iteration order is unspecified. Sorting first makes the rotation below
    /// the sole, deterministic source of ordering — true round-robin across
    /// calls with the same membership, not a wobble layered on hash order.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn workers_for(
        &self,
        namespace: &str,
        task_queue: &str,
        activity_type: &str,
        node: Option<&str>,
    ) -> Result<Vec<WorkerHandle>, ServerError> {
        let mut state = self.state()?;
        let key = ActivityKey::new(PoolAddress::new(namespace, task_queue), activity_type);
        let mut workers: Vec<WorkerHandle> = state
            .by_activity
            .get(&key)
            .map(|workers| {
                workers
                    .values()
                    .filter(|worker| worker_matches_node(worker, node))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        if workers.is_empty() {
            return Ok(workers);
        }
        workers.sort_by_key(WorkerHandle::id);
        let idx = state.rotation.entry(key).or_insert(0);
        let start = *idx % workers.len();
        *idx = idx.wrapping_add(1);
        let mut rotated = Vec::with_capacity(workers.len());
        rotated.extend_from_slice(&workers[start..]);
        rotated.extend_from_slice(&workers[..start]);
        Ok(rotated)
    }

    /// Return a snapshot of every connected worker stream.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn all_workers(&self) -> Result<Vec<WorkerHandle>, ServerError> {
        let state = self.state()?;
        Ok(state.workers.values().cloned().collect())
    }

    /// Return the handle for a worker by id, or `None` when it is not registered.
    ///
    /// The intervention router resolves the owning worker of a target attempt by id
    /// (NOI-6): the attempt-owner back-index stores a [`WorkerId`], and the router
    /// reads back the live handle to gate on its advertised capabilities and select
    /// its delivery. A `None` result means the owner disconnected — the router
    /// treats that as the attempt-scoped no-op.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn worker_by_id(&self, worker_id: WorkerId) -> Result<Option<WorkerHandle>, ServerError> {
        Ok(self.state()?.workers.get(&worker_id).cloned())
    }

    /// Replace the advertised intervention capabilities of a registered worker
    /// (NOI-6). The liminal registration frame cannot carry capabilities, so a
    /// liminal agent worker announces them on the reserved capabilities channel
    /// right after registering, and this applies the announcement to the live
    /// handle the intervention router gates on. Returns `false` when the worker
    /// is no longer registered (a disconnect racing the announcement — benign).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn set_intervention_capabilities(
        &self,
        worker_id: WorkerId,
        capabilities: &InterventionCapabilities,
    ) -> Result<bool, ServerError> {
        let mut state = self.state()?;
        if !state.workers.contains_key(&worker_id) {
            return Ok(false);
        }
        if let Some(handle) = state.workers.get_mut(&worker_id) {
            handle.intervention_capabilities = capabilities.clone();
        }
        // The selection index holds handle clones; keep them capability-consistent
        // even though capabilities are never a routing dimension.
        for workers in state.by_activity.values_mut() {
            if let Some(handle) = workers.get_mut(&worker_id) {
                handle.intervention_capabilities = capabilities.clone();
            }
        }
        Ok(true)
    }

    /// Broadcast a graceful drain request to every connected worker stream.
    /// Workers are removed from routing before any transport signal is attempted.
    /// A liminal worker has no drain control frame, so it is force-fenced and
    /// deregistered with an error-level identity-bearing log instead of being
    /// silently skipped.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn broadcast_drain(&self) -> Result<usize, ServerError> {
        let workers = self.all_workers()?;
        let mut delivered = 0usize;
        for worker in workers {
            if self.drain_worker(worker.id())? {
                delivered = delivered.saturating_add(1);
            }
        }
        Ok(delivered)
    }

    /// Stop assigning work to one worker and request a graceful transport drain.
    ///
    /// Returns `false` if the worker is not registered or if its transport has no
    /// drain channel. In the latter case the worker is deregistered immediately,
    /// after an error-level log names the worker and its transport limitation.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn drain_worker(&self, worker_id: WorkerId) -> Result<bool, ServerError> {
        let worker = {
            let mut state = self.state()?;
            let Some(worker) = state.workers.get(&worker_id).cloned() else {
                return Ok(false);
            };
            Self::remove_worker_from_service(&mut state, &worker);
            worker
        };
        match worker.delivery() {
            WorkerDelivery::Grpc(sender) => {
                if sender.try_send(WorkerMessage::DrainRequest).is_ok() {
                    tracing::info!(worker_id = worker_id.value(), "worker drain requested");
                    Ok(true)
                } else {
                    tracing::error!(
                        worker_id = worker_id.value(),
                        "worker drain signal failed; force-deregistering closed transport"
                    );
                    self.deregister(worker_id)?;
                    Ok(false)
                }
            }
            #[cfg(feature = "liminal-transport")]
            WorkerDelivery::Liminal(delivery) => {
                tracing::error!(
                    worker_id = worker_id.value(),
                    connection_pid = delivery.pid(),
                    "liminal transport has no drain control channel; worker fenced and \
                     deregistered at drain start"
                );
                self.deregister(worker_id)?;
                Ok(false)
            }
        }
    }

    /// Select one worker for the `(namespace, task_queue, activity_type)` pool.
    ///
    /// When `node` is `Some`, only workers whose advertised node equals it are
    /// considered (NODE affinity = require); `None` considers every worker in
    /// the pool. node is a within-pool filter, NOT part of the pool key.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn select_worker(
        &self,
        namespace: &str,
        task_queue: &str,
        activity_type: &str,
        node: Option<&str>,
    ) -> Result<Option<WorkerHandle>, ServerError> {
        let state = self.state()?;
        let key = ActivityKey::new(PoolAddress::new(namespace, task_queue), activity_type);
        Ok(state.by_activity.get(&key).and_then(|workers| {
            workers
                .values()
                .filter(|worker| worker_matches_node(worker, node))
                // Reachability is a dispatch PRECONDITION, so it is enforced
                // here rather than discovered at push time. Selecting a worker
                // the server cannot push to produces a dispatch that can only
                // fail, and on the liminal transport it fails by consuming
                // connection capacity — so an unreachable worker chosen anyway
                // makes its own unreachability worse.
                .filter(|worker| !state.dispatch_ineligible.contains(&worker.id))
                .min_by_key(|worker| worker.id)
                .cloned()
        }))
    }

    /// Publish the liveness probe's reachability verdict: the set of workers the
    /// server cannot currently reach on the push leg, which
    /// [`Self::select_worker`] then skips.
    ///
    /// Replaces the whole set rather than toggling one worker, so the published
    /// verdict is always exactly one round's evidence and a worker can never be
    /// left excluded by a stale entry nobody cleared.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn set_dispatch_ineligible(
        &self,
        unreachable: BTreeSet<WorkerId>,
    ) -> Result<(), ServerError> {
        self.state()?.dispatch_ineligible = unreachable;
        Ok(())
    }

    /// Whether a worker is currently excluded from dispatch selection for
    /// unreachability.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn is_dispatch_ineligible(&self, worker_id: WorkerId) -> Result<bool, ServerError> {
        Ok(self.state()?.dispatch_ineligible.contains(&worker_id))
    }

    /// Every gRPC-delivered worker the liveness probe must ping this round,
    /// paired with the stream sender the ping rides (#197).
    ///
    /// The liminal half of the same census is
    /// [`LiminalConnectionNotifier::liveness_targets`](crate::worker::LiminalConnectionNotifier::liveness_targets),
    /// which enumerates CONNECTIONS. This one enumerates REGISTRATIONS, because
    /// a gRPC worker's only server-side identity is its registry handle: the
    /// stream is owned by a tonic task and reachable solely through the sender
    /// the registration carries.
    ///
    /// Deliberately not filtered by current eligibility. A worker excluded from
    /// dispatch is precisely the worker whose next answered ping restores it,
    /// so skipping the excluded set would make exclusion permanent — the exact
    /// defect this lane exists to remove.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn grpc_liveness_targets(
        &self,
    ) -> Result<Vec<super::grpc_liveness::GrpcLivenessTarget>, ServerError> {
        Ok(self
            .state()?
            .workers
            .values()
            .filter_map(|worker| match &worker.delivery {
                WorkerDelivery::Grpc(sender) => Some(super::grpc_liveness::GrpcLivenessTarget {
                    worker_id: worker.id,
                    sender: sender.clone(),
                }),
                #[cfg(feature = "liminal-transport")]
                WorkerDelivery::Liminal(_) => None,
            })
            .collect())
    }

    /// How many workers that selection COULD have chosen are currently excluded
    /// from it by the liveness verdict (#197 R3).
    ///
    /// `tiers` is the ordered sequence of node filters the selection actually
    /// walked, and taking it as an argument — rather than re-deriving one here
    /// — is the whole point of the method. Selection over a `Pinned{L}`
    /// namespace walks the required labels and NEVER spills to a `None`
    /// any-node tier; a census taken over the row's own node instead would pass
    /// `None`, match every worker in the pool, and report an unlabelled worker
    /// that was never a candidate as the reason the row found nobody. The
    /// refusal built on that count then tells an operator not to start the
    /// labelled worker that is the only remedy.
    ///
    /// Counted as a UNION over the tiers and over DISTINCT workers: the pool is
    /// walked once and a worker admissible to more than one tier is counted
    /// once, so the number is a headcount rather than a sum of overlapping
    /// matches.
    ///
    /// Read under the SAME lock and with the SAME `worker_matches_node` filter
    /// [`Self::select_worker`] applies, so a refusal quoting this count
    /// describes the fleet selection actually saw.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn ineligible_workers_over_tiers(
        &self,
        namespace: &str,
        task_queue: &str,
        activity_type: &str,
        tiers: &[Option<String>],
    ) -> Result<usize, ServerError> {
        let state = self.state()?;
        let key = ActivityKey::new(PoolAddress::new(namespace, task_queue), activity_type);
        Ok(state.by_activity.get(&key).map_or(0, |workers| {
            workers
                .values()
                .filter(|worker| state.dispatch_ineligible.contains(&worker.id))
                .filter(|worker| {
                    tiers
                        .iter()
                        .any(|tier| worker_matches_node(worker, tier.as_deref()))
                })
                .count()
        }))
    }

    /// The currently published exclusion set, read as a whole.
    ///
    /// The liveness probe reads this BEFORE publishing a round's verdict, so it
    /// can announce the workers that just LEFT the set. Without the whole
    /// previous set there is no way to name a recovery: per-worker queries can
    /// only be asked about workers the new verdict already mentions, and a
    /// recovered worker is precisely the one it does not.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn dispatch_ineligible(&self) -> Result<BTreeSet<WorkerId>, ServerError> {
        Ok(self.state()?.dispatch_ineligible.clone())
    }

    /// Census the live fleet for one dispatch address (R1).
    ///
    /// Taken under the SAME lock discipline as [`Self::select_worker`] and read
    /// immediately after a selection miss, so the taxonomy verdict describes
    /// the fleet selection actually saw. Three nested counts — the pool, the
    /// activity coverage within it, the node coverage within that — are what
    /// separate `NO_LIVE_POLLERS` from `POLLERS_INCOMPATIBLE`.
    ///
    /// `last_compatible_poller_age` is zero while a compatible worker is
    /// connected, the elapsed time since the most recent compatible departure
    /// when one has left, and `None` when this server has never had one.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn pool_census(
        &self,
        namespace: &str,
        task_queue: &str,
        activity_type: &str,
        node: Option<&str>,
    ) -> Result<super::queue_service::PoolCensus, ServerError> {
        let state = self.state()?;
        let key = ActivityKey::new(PoolAddress::new(namespace, task_queue), activity_type);
        let workers_in_pool = state
            .workers
            .values()
            .filter(|worker| {
                worker.task_queue == task_queue && worker.namespaces.contains(namespace)
            })
            .count();
        let serving = state.by_activity.get(&key);
        let workers_serving_activity = serving.map_or(0, HashMap::len);
        let compatible_workers = serving.map_or(0, |workers| {
            workers
                .values()
                .filter(|worker| worker_matches_node(worker, node))
                .count()
        });
        let last_compatible_poller_age = if compatible_workers > 0 {
            Some(Duration::ZERO)
        } else {
            state
                .last_departure
                .get(&key)
                .and_then(|by_node| {
                    by_node
                        .iter()
                        .filter(|(departed_node, _)| match node {
                            None => true,
                            Some(node) => departed_node.as_deref() == Some(node),
                        })
                        .map(|(_, departed_at)| *departed_at)
                        .max()
                })
                .map(|departed_at| departed_at.elapsed())
        };
        Ok(super::queue_service::PoolCensus {
            workers_in_pool,
            workers_serving_activity,
            compatible_workers,
            last_compatible_poller_age,
        })
    }

    /// Return whether a worker stream is currently registered.
    ///
    /// The activity dispatch path uses this after queuing a task to detect a
    /// worker whose stream tore down concurrently: a sweep that ran before
    /// the dispatch tracked its task can never complete it, so the dispatch
    /// must fail the activity itself instead of waiting forever.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn is_registered(&self, worker_id: WorkerId) -> Result<bool, ServerError> {
        Ok(self.state()?.workers.contains_key(&worker_id))
    }

    /// Remove a worker by id from every namespace/activity index it advertised.
    ///
    /// Emits a WS3 [`WorkerDeathReason::Disconnect`] delta — the truthful default
    /// for a removed worker whose stream/registration went away. Callers that can
    /// PROVE a finer reason (a liveness-timeout sweep) call
    /// [`Self::deregister_with_reason`] instead, so the dashboard never sees a
    /// fabricated distinction.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn deregister(&self, worker_id: WorkerId) -> Result<(), ServerError> {
        self.deregister_with_reason(worker_id, WorkerDeathReason::Disconnect)
    }

    /// Remove a worker by id, attributing the departure to an explicit
    /// [`WorkerDeathReason`] the caller can prove at its call site (for example a
    /// heartbeat sweep passes [`WorkerDeathReason::Timeout`]).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn deregister_with_reason(
        &self,
        worker_id: WorkerId,
        reason: WorkerDeathReason,
    ) -> Result<(), ServerError> {
        let mut state = self.state()?;
        let removed_namespaces = Self::remove_worker(&mut state, worker_id);
        drop(state);

        let Some(namespaces) = removed_namespaces else {
            // Already gone: no metrics double-count, no duplicate delta.
            return Ok(());
        };

        if let Some(metrics) = &self.metrics {
            for namespace in &namespaces {
                metrics.worker_disconnected(namespace);
            }
        }
        self.emit_worker_disconnected(worker_id, &namespaces, reason);

        Ok(())
    }

    /// Emit a WS3 `WorkerDisconnected` delta if a publisher is attached.
    fn emit_worker_disconnected(
        &self,
        worker_id: WorkerId,
        namespaces: &BTreeSet<String>,
        reason: WorkerDeathReason,
    ) {
        if let Some(publisher) = &self.cluster_publisher {
            let namespaces_vec: Vec<String> = namespaces.iter().cloned().collect();
            drop(publisher.emit(|meta| ClusterEvent::WorkerDisconnected {
                meta,
                worker_id: worker_id.value().to_string(),
                namespaces: namespaces_vec,
                reason,
            }));
        }
    }

    /// Remove a worker from every `(namespace, task_queue, activity_type)` index
    /// it advertised. Returns the namespace set it served (for metrics), or
    /// `None` if the worker was already gone.
    fn remove_worker(state: &mut RegistryState, worker_id: WorkerId) -> Option<BTreeSet<String>> {
        let handle = state.workers.remove(&worker_id)?;
        Self::remove_worker_from_service(state, &handle);
        Some(handle.namespaces)
    }

    /// Fence a worker from every routing index while retaining its connection
    /// handle so an in-progress graceful drain can finish and tear down normally.
    fn remove_worker_from_service(state: &mut RegistryState, handle: &WorkerHandle) {
        let departed_at = Instant::now();
        for namespace in &handle.namespaces {
            let pool = PoolAddress::new(namespace.clone(), handle.task_queue.clone());
            for activity_type in &handle.activity_types {
                let key = ActivityKey::new(pool.clone(), activity_type.clone());
                // Departure time is recorded for EVERY address this worker
                // served, so a later census can age the last compatible poller
                // instead of reporting a bare "nobody" (R1).
                state
                    .last_departure
                    .entry(key.clone())
                    .or_default()
                    .insert(handle.node.clone(), departed_at);
                if let Some(workers) = state.by_activity.get_mut(&key) {
                    workers.remove(&handle.id);
                    if workers.is_empty() {
                        state.by_activity.remove(&key);
                        // Prune the round-robin cursor in lockstep: the cursor
                        // map is keyed on arbitrary caller-supplied strings and
                        // is lazily created by `workers_for`, so leaving stale
                        // entries behind leaks memory unboundedly on a
                        // never-dying server. When the last worker for a triple
                        // leaves, its cursor has no remaining meaning.
                        state.rotation.remove(&key);
                    }
                }
            }
        }
    }

    fn state(&self) -> Result<MutexGuard<'_, RegistryState>, ServerError> {
        self.inner
            .lock()
            .map_err(|_| ServerError::lock_poisoned("connected worker registry"))
    }
}

/// Map a held [`WorkerDelivery`] to the wire [`WorkerTransport`] discriminant for
/// the WS3 `WorkerConnected` delta.
const fn transport_of(delivery: &WorkerDelivery) -> WorkerTransport {
    match delivery {
        WorkerDelivery::Grpc(_) => WorkerTransport::Grpc,
        #[cfg(feature = "liminal-transport")]
        WorkerDelivery::Liminal(_) => WorkerTransport::Liminal,
    }
}

/// Normalize a wire `node` string into an optional locality affinity: an empty
/// value (the proto3 default) carries no node, anything else is the worker's
/// advertised node id.
///
/// Shared with contract admission ([`super::contracts::validate_worker_contracts`])
/// rather than restated there: admission decides which of a package's actions a
/// connection owes from the same locality this registry then routes by, and two
/// independent normalizations of the wire default would be a place for that
/// agreement to drift silently.
pub(crate) fn optional_node(node: &str) -> Option<String> {
    if node.is_empty() {
        None
    } else {
        Some(node.to_owned())
    }
}

/// Whether a worker satisfies an optional node filter. `None` (unpinned) matches
/// every worker; `Some(node)` matches only a worker advertising that exact node
/// (NODE affinity = require). A worker with no advertised node never matches a
/// pinned dispatch.
fn worker_matches_node(worker: &WorkerHandle, node: Option<&str>) -> bool {
    match node {
        None => true,
        Some(node) => worker.node() == Some(node),
    }
}

#[derive(Clone, Debug)]
struct WorkerRegistrationParts {
    worker_id: WorkerId,
    namespaces: BTreeSet<String>,
    task_queue: String,
    activity_types: BTreeSet<String>,
}

/// Registration token owned by the worker stream task.
///
/// Dropping the token performs best-effort cleanup for disconnect paths. Call
/// [`WorkerRegistration::deregister`] when the caller needs a typed poison error.
#[derive(Debug)]
pub struct WorkerRegistration {
    registry: ConnectedWorkerRegistry,
    parts: Option<WorkerRegistrationParts>,
}

impl WorkerRegistration {
    /// Worker id assigned to this registration.
    #[must_use]
    pub fn worker_id(&self) -> Option<WorkerId> {
        self.parts.as_ref().map(|parts| parts.worker_id)
    }

    /// Authorized namespace set for this registration.
    #[must_use]
    pub fn namespaces(&self) -> Option<&BTreeSet<String>> {
        self.parts.as_ref().map(|parts| &parts.namespaces)
    }

    /// Task queue (pool/flavour) this registration serves within each namespace.
    #[must_use]
    pub fn task_queue(&self) -> Option<&str> {
        self.parts.as_ref().map(|parts| parts.task_queue.as_str())
    }

    /// Activity types advertised by this registration.
    #[must_use]
    pub fn activity_types(&self) -> Option<&BTreeSet<String>> {
        self.parts.as_ref().map(|parts| &parts.activity_types)
    }

    /// Explicitly remove this worker from the registry.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::LockPoisoned`] if the registry lock is poisoned.
    pub fn deregister(mut self) -> Result<(), ServerError> {
        let Some(parts) = self.parts.take() else {
            return Ok(());
        };
        self.registry.deregister(parts.worker_id)
    }
}

impl Drop for WorkerRegistration {
    fn drop(&mut self) {
        let Some(parts) = self.parts.take() else {
            return;
        };
        let removed_namespaces = self.registry.inner.lock().ok().and_then(|mut state| {
            ConnectedWorkerRegistry::remove_worker(&mut state, parts.worker_id)
        });
        if let Some(namespaces) = removed_namespaces {
            if let Some(metrics) = &self.registry.metrics {
                for namespace in &namespaces {
                    metrics.worker_disconnected(namespace);
                }
            }
            // A dropped registration token means the worker's stream/connection
            // went away — the truthful reason is Disconnect, not a fabricated
            // timeout/deregister distinction this path cannot prove.
            self.registry.emit_worker_disconnected(
                parts.worker_id,
                &namespaces,
                WorkerDeathReason::Disconnect,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::NamespaceMode;
    use crate::namespace::{NamespaceResolver, StaticScheduleNamespaces, StaticWorkflowNamespaces};
    use crate::worker::heartbeat::{DISPATCH_PROBATION_PINGS, HeartbeatTracker};

    use super::*;

    fn guard() -> NamespaceGuard {
        NamespaceGuard::new(NamespaceResolver::authorization_only(
            NamespaceMode::SharedEngine,
            StaticWorkflowNamespaces::default(),
            StaticScheduleNamespaces::default(),
        ))
    }

    fn caller(namespace: &str) -> CallerIdentity {
        CallerIdentity::new("worker", [namespace.to_owned()])
    }

    /// A test-expectation failure as a `ServerError`, so `Result`-returning
    /// tests can fail on an unexpected `None` without `panic!`/`expect`.
    fn test_failure(message: &str) -> ServerError {
        ServerError::worker_dispatch("default".to_owned(), "test".to_owned(), message.to_owned())
    }

    fn registration(namespace: &str, activity_types: &[&str]) -> ProtoRegisterWorker {
        registration_with_queue(namespace, "", activity_types)
    }

    fn registration_with_queue(
        namespace: &str,
        task_queue: &str,
        activity_types: &[&str],
    ) -> ProtoRegisterWorker {
        registration_full(&[namespace], task_queue, "", activity_types)
    }

    fn registration_full(
        namespaces: &[&str],
        task_queue: &str,
        node: &str,
        activity_types: &[&str],
    ) -> ProtoRegisterWorker {
        ProtoRegisterWorker {
            namespaces: namespaces.iter().map(|value| (*value).to_owned()).collect(),
            activity_types: activity_types
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            task_queue: task_queue.to_owned(),
            node: node.to_owned(),
            activities: Vec::new(),
            identity: String::new(),
            instance: None,
        }
    }

    fn multi_caller(namespaces: &[&str]) -> CallerIdentity {
        CallerIdentity::new("worker", namespaces.iter().map(|value| (*value).to_owned()))
    }

    /// `set_intervention_capabilities` replaces a live worker's advertised set —
    /// the announcement path a liminal agent worker takes after its in-band
    /// registration (the registration frame cannot carry capabilities) — and
    /// reports an unknown worker as `false` (an announcement racing a
    /// disconnect is benign, never an error).
    #[tokio::test]
    async fn set_intervention_capabilities_updates_live_worker() -> Result<(), ServerError> {
        let registry = ConnectedWorkerRegistry::default();
        let (sender, _receiver) = mpsc::channel(1);
        let types = ["scout".to_owned()];
        let guard = registry.register_delivery_with_capabilities(
            ["default".to_owned()],
            "default",
            None,
            types.iter(),
            WorkerDelivery::Grpc(sender),
            InterventionCapabilities::none(),
        )?;
        let Some(worker_id) = guard.worker_id() else {
            return Err(test_failure("registration carries an id"));
        };

        let announced = InterventionCapabilities {
            supported: vec![aion_core::InterventionPrimitive::InjectMessage],
        };
        assert!(
            registry.set_intervention_capabilities(worker_id, &announced)?,
            "a live worker's capabilities must be updatable"
        );
        let Some(handle) = registry.worker_by_id(worker_id)? else {
            return Err(test_failure("worker stays registered"));
        };
        assert_eq!(handle.intervention_capabilities(), &announced);

        assert!(
            !registry.set_intervention_capabilities(WorkerId(u64::MAX), &announced)?,
            "an unknown worker reports false, never an error"
        );
        Ok(())
    }

    #[tokio::test]
    async fn register_and_deregister_are_namespace_isolated() -> Result<(), ServerError> {
        let registry = ConnectedWorkerRegistry::default();
        let (tenant_a_tx, _tenant_a_rx) = mpsc::channel(1);
        let (tenant_b_tx, _tenant_b_rx) = mpsc::channel(1);

        let tenant_a = registry
            .accept_registration(
                &guard(),
                &caller("tenant-a"),
                &registration("tenant-a", &["charge", "charge"]),
                tenant_a_tx,
            )
            .await?;
        let tenant_b = registry
            .accept_registration(
                &guard(),
                &caller("tenant-b"),
                &registration("tenant-b", &["charge"]),
                tenant_b_tx,
            )
            .await?;

        let tq = DEFAULT_TASK_QUEUE;
        assert_eq!(
            registry.workers_for("tenant-a", tq, "charge", None)?.len(),
            1
        );
        assert_eq!(
            registry.workers_for("tenant-b", tq, "charge", None)?.len(),
            1
        );
        assert!(
            registry
                .workers_for("tenant-a", tq, "missing", None)?
                .is_empty()
        );

        let tenant_a_id = tenant_a.worker_id();
        tenant_a.deregister()?;

        assert!(
            registry
                .workers_for("tenant-a", tq, "charge", None)?
                .is_empty()
        );
        assert_eq!(
            registry.workers_for("tenant-b", tq, "charge", None)?.len(),
            1
        );
        assert_ne!(tenant_a_id, tenant_b.worker_id());

        tenant_b.deregister()?;
        assert!(
            registry
                .workers_for("tenant-b", tq, "charge", None)?
                .is_empty()
        );
        Ok(())
    }

    #[tokio::test]
    async fn denied_namespace_is_not_registered() -> Result<(), ServerError> {
        let registry = ConnectedWorkerRegistry::default();
        let (tx, _rx) = mpsc::channel(1);
        let denied = registry
            .accept_registration(
                &guard(),
                &caller("tenant-a"),
                &registration("tenant-b", &["charge"]),
                tx,
            )
            .await;

        assert!(denied.is_err());
        assert!(
            registry
                .workers_for("tenant-b", DEFAULT_TASK_QUEUE, "charge", None)?
                .is_empty()
        );
        Ok(())
    }

    #[tokio::test]
    async fn task_queues_partition_disjoint_pools_within_one_namespace() -> Result<(), ServerError>
    {
        // Same namespace + same activity_type, two DIFFERENT task queues: the
        // pools are disjoint, a lookup for one queue never returns the other's
        // worker, and round-robin holds independently per (ns, tq, type) triple.
        let registry = ConnectedWorkerRegistry::default();
        let (norn_tx, _norn_rx) = mpsc::channel(1);
        let (claude_a_tx, _claude_a_rx) = mpsc::channel(1);
        let (claude_b_tx, _claude_b_rx) = mpsc::channel(1);

        let norn = registry
            .accept_registration(
                &guard(),
                &caller("local"),
                &registration_with_queue("local", "norn", &["dev"]),
                norn_tx,
            )
            .await?;
        // Two workers on the SAME (local, claude) pool to exercise round-robin.
        let claude_a = registry
            .accept_registration(
                &guard(),
                &caller("local"),
                &registration_with_queue("local", "claude", &["dev"]),
                claude_a_tx,
            )
            .await?;
        let claude_b = registry
            .accept_registration(
                &guard(),
                &caller("local"),
                &registration_with_queue("local", "claude", &["dev"]),
                claude_b_tx,
            )
            .await?;

        let norn_pool = registry.workers_for("local", "norn", "dev", None)?;
        assert_eq!(norn_pool.len(), 1, "norn pool has exactly its one worker");
        let norn_id = norn.worker_id().ok_or_else(missing_id)?;
        assert_eq!(norn_pool[0].id(), norn_id);

        let claude_pool = registry.workers_for("local", "claude", "dev", None)?;
        assert_eq!(
            claude_pool.len(),
            2,
            "claude pool sees only its two workers"
        );
        let claude_ids: BTreeSet<WorkerId> = claude_pool.iter().map(WorkerHandle::id).collect();
        assert!(
            !claude_ids.contains(&norn_id),
            "the norn worker must never appear in the claude pool"
        );

        // A dispatch targeting `norn` never reaches a `claude` worker, and vice
        // versa: the disjoint key is the boundary.
        assert!(
            !registry
                .workers_for("local", "norn", "dev", None)?
                .iter()
                .any(|worker| claude_ids.contains(&worker.id()))
        );

        // Round-robin per triple: the (local, claude, dev) cursor advances
        // independently and cycles through both claude workers, while the
        // (local, norn, dev) cursor keeps returning its single worker.
        let first = registry.workers_for("local", "claude", "dev", None)?[0].id();
        let second = registry.workers_for("local", "claude", "dev", None)?[0].id();
        assert_ne!(
            first, second,
            "claude pool round-robins across both workers"
        );
        assert_eq!(
            registry.workers_for("local", "norn", "dev", None)?[0].id(),
            norn_id,
            "the norn pool rotation is unaffected by claude traffic"
        );

        norn.deregister()?;
        claude_a.deregister()?;
        claude_b.deregister()?;
        Ok(())
    }

    #[tokio::test]
    async fn same_task_queue_in_different_namespaces_is_isolated() -> Result<(), ServerError> {
        // Same task_queue string, two DIFFERENT namespaces: namespace is the
        // correctness boundary, so the pools are isolated.
        let registry = ConnectedWorkerRegistry::default();
        let (local_tx, _local_rx) = mpsc::channel(1);
        let (remote_tx, _remote_rx) = mpsc::channel(1);

        let local = registry
            .accept_registration(
                &guard(),
                &caller("local"),
                &registration_with_queue("local", "gpu", &["render"]),
                local_tx,
            )
            .await?;
        let remote = registry
            .accept_registration(
                &guard(),
                &caller("remote"),
                &registration_with_queue("remote", "gpu", &["render"]),
                remote_tx,
            )
            .await?;

        let local_pool = registry.workers_for("local", "gpu", "render", None)?;
        let remote_pool = registry.workers_for("remote", "gpu", "render", None)?;
        assert_eq!(local_pool.len(), 1);
        assert_eq!(remote_pool.len(), 1);
        assert_ne!(
            local_pool[0].id(),
            remote_pool[0].id(),
            "a shared task_queue string does not merge two namespaces"
        );

        local.deregister()?;
        assert!(
            registry
                .workers_for("local", "gpu", "render", None)?
                .is_empty(),
            "deregistering the local worker leaves the remote namespace untouched"
        );
        assert_eq!(
            registry.workers_for("remote", "gpu", "render", None)?.len(),
            1
        );

        remote.deregister()?;
        Ok(())
    }

    #[tokio::test]
    async fn worker_serving_a_namespace_set_is_reachable_in_each() -> Result<(), ServerError> {
        // A worker advertising {a, b} is reachable for dispatch in BOTH a and b;
        // a worker in {a} is NOT reachable in b.
        let registry = ConnectedWorkerRegistry::default();
        let (ab_tx, _ab_rx) = mpsc::channel(1);
        let (a_tx, _a_rx) = mpsc::channel(1);

        let worker_ab = registry
            .accept_registration(
                &guard(),
                &multi_caller(&["a", "b"]),
                &registration_full(&["a", "b"], "default", "", &["dev"]),
                ab_tx,
            )
            .await?;
        let worker_a = registry
            .accept_registration(
                &guard(),
                &caller("a"),
                &registration_full(&["a"], "default", "", &["dev"]),
                a_tx,
            )
            .await?;

        let in_a = registry.workers_for("a", "default", "dev", None)?;
        let in_b = registry.workers_for("b", "default", "dev", None)?;
        let both_id = worker_ab.worker_id().ok_or_else(missing_id)?;
        let only_a_id = worker_a.worker_id().ok_or_else(missing_id)?;

        // Namespace a sees BOTH workers; namespace b sees ONLY the {a, b} worker.
        let a_ids: BTreeSet<WorkerId> = in_a.iter().map(WorkerHandle::id).collect();
        assert_eq!(a_ids, BTreeSet::from([both_id, only_a_id]));
        assert_eq!(in_b.len(), 1, "only the {{a, b}} worker is reachable in b");
        assert_eq!(in_b[0].id(), both_id);
        assert!(
            !in_b.iter().any(|worker| worker.id() == only_a_id),
            "the {{a}}-only worker must not be reachable in b"
        );

        // Deregistering the {a, b} worker removes it from BOTH buckets.
        worker_ab.deregister()?;
        assert!(
            registry
                .workers_for("b", "default", "dev", None)?
                .is_empty()
        );
        assert_eq!(registry.workers_for("a", "default", "dev", None)?.len(), 1);

        worker_a.deregister()?;
        Ok(())
    }

    #[tokio::test]
    async fn node_pin_filters_within_pool() -> Result<(), ServerError> {
        // Two workers in the same (namespace, task_queue) pool on different
        // nodes: unpinned round-robins across both; pinned to node N reaches
        // ONLY the worker(s) on N; pinned to a node with no worker finds none.
        let registry = ConnectedWorkerRegistry::default();
        let (n1_tx, _n1_rx) = mpsc::channel(1);
        let (n2_tx, _n2_rx) = mpsc::channel(1);

        let on_n1 = registry
            .accept_registration(
                &guard(),
                &caller("ns"),
                &registration_full(&["ns"], "tq", "n1", &["dev"]),
                n1_tx,
            )
            .await?;
        let on_n2 = registry
            .accept_registration(
                &guard(),
                &caller("ns"),
                &registration_full(&["ns"], "tq", "n2", &["dev"]),
                n2_tx,
            )
            .await?;
        let n1_id = on_n1.worker_id().ok_or_else(missing_id)?;
        let n2_id = on_n2.worker_id().ok_or_else(missing_id)?;

        // Unpinned: both workers are candidates and round-robin advances.
        let unpinned = registry.workers_for("ns", "tq", "dev", None)?;
        assert_eq!(unpinned.len(), 2, "unpinned reaches the whole pool");
        let first = registry.workers_for("ns", "tq", "dev", None)?[0].id();
        let second = registry.workers_for("ns", "tq", "dev", None)?[0].id();
        assert_ne!(first, second, "unpinned round-robins across both nodes");

        // Pinned to n1: only the n1 worker; pinned to n2: only the n2 worker.
        let pinned_n1 = registry.workers_for("ns", "tq", "dev", Some("n1"))?;
        assert_eq!(pinned_n1.len(), 1);
        assert_eq!(pinned_n1[0].id(), n1_id);
        let pinned_n2 = registry.workers_for("ns", "tq", "dev", Some("n2"))?;
        assert_eq!(pinned_n2.len(), 1);
        assert_eq!(pinned_n2[0].id(), n2_id);

        // select_worker honours the same filter.
        assert_eq!(
            registry
                .select_worker("ns", "tq", "dev", Some("n1"))?
                .map(|worker| worker.id()),
            Some(n1_id)
        );

        // Pinned to a node with no worker finds no candidate (the dispatcher
        // then waits via the same no-worker path the existing test exercises).
        assert!(
            registry
                .workers_for("ns", "tq", "dev", Some("absent"))?
                .is_empty(),
            "a pin to a node with no worker yields no candidate"
        );
        assert!(
            registry
                .select_worker("ns", "tq", "dev", Some("absent"))?
                .is_none()
        );

        on_n1.deregister()?;
        on_n2.deregister()?;
        Ok(())
    }

    #[tokio::test]
    async fn shared_node_id_round_robins_across_workers() -> Result<(), ServerError> {
        // Two workers SHARING a node id in the same pool: a dispatch pinned to
        // that node round-robins across BOTH (node is locality, not process).
        let registry = ConnectedWorkerRegistry::default();
        let (a_tx, _a_rx) = mpsc::channel(1);
        let (b_tx, _b_rx) = mpsc::channel(1);

        let worker_a = registry
            .accept_registration(
                &guard(),
                &caller("ns"),
                &registration_full(&["ns"], "tq", "shared", &["dev"]),
                a_tx,
            )
            .await?;
        let worker_b = registry
            .accept_registration(
                &guard(),
                &caller("ns"),
                &registration_full(&["ns"], "tq", "shared", &["dev"]),
                b_tx,
            )
            .await?;
        let a_id = worker_a.worker_id().ok_or_else(missing_id)?;
        let b_id = worker_b.worker_id().ok_or_else(missing_id)?;

        let pinned = registry.workers_for("ns", "tq", "dev", Some("shared"))?;
        assert_eq!(
            pinned.len(),
            2,
            "both workers on the shared node are candidates"
        );
        let pinned_ids: BTreeSet<WorkerId> = pinned.iter().map(WorkerHandle::id).collect();
        assert_eq!(pinned_ids, BTreeSet::from([a_id, b_id]));

        let first = registry.workers_for("ns", "tq", "dev", Some("shared"))?[0].id();
        let second = registry.workers_for("ns", "tq", "dev", Some("shared"))?[0].id();
        assert_ne!(
            first, second,
            "a pin to a shared node round-robins across both workers on it"
        );

        worker_a.deregister()?;
        worker_b.deregister()?;
        Ok(())
    }

    #[tokio::test]
    async fn rotation_cursor_is_pruned_when_last_worker_leaves() -> Result<(), ServerError> {
        // The round-robin cursor is keyed on arbitrary caller-supplied strings;
        // it must not outlive the pool it rotates, or a never-dying server leaks
        // memory. After the last worker for a triple deregisters, no cursor for
        // that triple may remain.
        let registry = ConnectedWorkerRegistry::default();
        let (tx, _rx) = mpsc::channel(1);
        let worker = registry
            .accept_registration(
                &guard(),
                &caller("ns"),
                &registration_full(&["ns"], "tq", "", &["dev"]),
                tx,
            )
            .await?;

        // Drive the lazy cursor insert.
        let _ = registry.workers_for("ns", "tq", "dev", None)?;
        let key = ActivityKey::new(PoolAddress::new("ns", "tq"), "dev");
        assert!(
            registry.state()?.rotation.contains_key(&key),
            "a lookup must have created the rotation cursor"
        );

        worker.deregister()?;
        let state = registry.state()?;
        assert!(
            !state.rotation.contains_key(&key),
            "the rotation cursor must be pruned once the last worker leaves"
        );
        assert!(
            !state.by_activity.contains_key(&key),
            "the activity bucket must also be gone"
        );
        Ok(())
    }

    fn missing_id() -> ServerError {
        ServerError::lock_poisoned("registration unexpectedly missing a worker id")
    }

    // ---- Minted-on-use (Control-Plane Phase 1) -----------------------------

    fn namespace_store() -> Arc<dyn NamespaceStore> {
        Arc::new(aion_store::InMemoryStore::default())
    }

    fn minting_registry(
        store: &Arc<dyn NamespaceStore>,
        policy: AutoCreate,
    ) -> ConnectedWorkerRegistry {
        ConnectedWorkerRegistry::default().with_namespace_minting(Arc::clone(store), policy)
    }

    #[tokio::test]
    async fn open_register_mints_durable_record_and_is_idempotent() -> Result<(), ServerError> {
        let store = namespace_store();
        let registry = minting_registry(&store, AutoCreate::Open);

        // First registration mints the namespace.
        let (tx_one, _rx_one) = mpsc::channel(1);
        let first = registry
            .accept_registration(
                &guard(),
                &caller("orders"),
                &registration("orders", &["charge"]),
                tx_one,
            )
            .await?;
        let record = store
            .get_namespace("orders")
            .await?
            .ok_or_else(|| ServerError::namespace_denied("expected a minted record"))?;
        assert_eq!(record.name, "orders");
        assert_eq!(record.origin, NamespaceOrigin::WorkerMint);

        // Re-registering the same namespace is idempotent: no duplicate row,
        // and the prior worker is unaffected.
        let (tx_two, _rx_two) = mpsc::channel(1);
        registry
            .accept_registration(
                &guard(),
                &caller("orders"),
                &registration("orders", &["refund"]),
                tx_two,
            )
            .await?;
        let all = store.list_namespaces().await?;
        assert_eq!(
            all.iter().filter(|r| r.name == "orders").count(),
            1,
            "re-register must not create a duplicate namespace row"
        );
        drop(first);
        Ok(())
    }

    #[tokio::test]
    async fn open_register_mints_each_namespace_in_a_multi_namespace_worker()
    -> Result<(), ServerError> {
        let store = namespace_store();
        let registry = minting_registry(&store, AutoCreate::Open);
        let (tx, _rx) = mpsc::channel(1);

        registry
            .accept_registration(
                &guard(),
                &multi_caller(&["alpha", "beta"]),
                &registration_full(&["alpha", "beta"], "", "", &["charge"]),
                tx,
            )
            .await?;

        assert!(store.get_namespace("alpha").await?.is_some());
        assert!(store.get_namespace("beta").await?.is_some());
        Ok(())
    }

    // ---- Placement admission (Control-Plane Phase 2, P2-I1) -----------------

    /// Pre-mint `namespace` and set its placement to `Pinned{nodes}`, returning a
    /// minting registry over the same store so `accept_registration` reads the
    /// placement from the SAME durable record.
    async fn pinned_registry(
        store: &Arc<dyn NamespaceStore>,
        namespace: &str,
        nodes: &[&str],
    ) -> Result<ConnectedWorkerRegistry, ServerError> {
        store
            .register_namespace(namespace, NamespaceOrigin::Explicit)
            .await?;
        store
            .set_namespace_placement(
                namespace,
                NamespacePlacement::Pinned {
                    nodes: nodes.iter().map(|n| (*n).to_owned()).collect(),
                },
            )
            .await?;
        Ok(minting_registry(store, AutoCreate::Open))
    }

    /// A worker on a node IN the required set registers successfully into a
    /// `Pinned{n1}` namespace, and is reachable in the pool.
    #[tokio::test]
    async fn pinned_admits_a_worker_on_a_required_node() -> Result<(), ServerError> {
        let store = namespace_store();
        let registry = pinned_registry(&store, "iso", &["n1"]).await?;
        let (tx, _rx) = mpsc::channel(1);

        let _registration = registry
            .accept_registration(
                &guard(),
                &caller("iso"),
                &registration_full(&["iso"], "", "n1", &["charge"]),
                tx,
            )
            .await?;

        assert_eq!(
            registry
                .workers_for("iso", DEFAULT_TASK_QUEUE, "charge", Some("n1"))?
                .len(),
            1,
            "an n1 worker must be admitted into the Pinned{{n1}} namespace's pool"
        );
        Ok(())
    }

    /// A worker on a node NOT in the required set is rejected — the WHOLE
    /// registration fails (loud) and no worker is inserted. This would FAIL under
    /// no admission gate (the worker would join and steal Pinned dispatches).
    #[tokio::test]
    async fn pinned_rejects_a_wrong_node_worker() -> Result<(), ServerError> {
        let store = namespace_store();
        let registry = pinned_registry(&store, "iso", &["n1"]).await?;
        let (tx, _rx) = mpsc::channel(1);

        let denied = registry
            .accept_registration(
                &guard(),
                &caller("iso"),
                &registration_full(&["iso"], "", "n2", &["charge"]),
                tx,
            )
            .await;
        assert!(
            matches!(denied, Err(ServerError::Namespace { .. })),
            "a wrong-node (n2) worker must be rejected from a Pinned{{n1}} namespace"
        );
        assert!(
            registry
                .workers_for("iso", DEFAULT_TASK_QUEUE, "charge", None)?
                .is_empty(),
            "a rejected registration must not insert a worker on any node"
        );
        Ok(())
    }

    /// A worker advertising NO node (`node == ""` → `None`) is rejected from a
    /// `Pinned{n1}` namespace: an unlabelled worker can never satisfy a hard pin.
    #[tokio::test]
    async fn pinned_rejects_a_node_less_worker() -> Result<(), ServerError> {
        let store = namespace_store();
        let registry = pinned_registry(&store, "iso", &["n1"]).await?;
        let (tx, _rx) = mpsc::channel(1);

        let denied = registry
            .accept_registration(
                &guard(),
                &caller("iso"),
                &registration_full(&["iso"], "", "", &["charge"]),
                tx,
            )
            .await;
        assert!(
            matches!(denied, Err(ServerError::Namespace { .. })),
            "a node-less worker must be rejected from a Pinned{{n1}} namespace"
        );
        assert!(
            registry
                .workers_for("iso", DEFAULT_TASK_QUEUE, "charge", None)?
                .is_empty(),
            "a rejected node-less registration must not insert a worker"
        );
        Ok(())
    }

    /// Reject-WHOLE-registration (Open Decision 6): a worker serving BOTH a
    /// non-isolated namespace and a `Pinned{n1}` namespace on a wrong node is
    /// rejected entirely — the compliant namespace does NOT get a partial admit.
    #[tokio::test]
    async fn pinned_violation_rejects_the_whole_multi_namespace_registration()
    -> Result<(), ServerError> {
        let store = namespace_store();
        let registry = pinned_registry(&store, "iso", &["n1"]).await?;
        let (tx, _rx) = mpsc::channel(1);

        let denied = registry
            .accept_registration(
                &guard(),
                &multi_caller(&["free", "iso"]),
                &registration_full(&["free", "iso"], "", "n2", &["charge"]),
                tx,
            )
            .await;
        assert!(
            matches!(denied, Err(ServerError::Namespace { .. })),
            "a wrong-node worker serving a Pinned namespace fails the WHOLE registration"
        );
        assert!(
            registry
                .workers_for("free", DEFAULT_TASK_QUEUE, "charge", None)?
                .is_empty(),
            "the compliant namespace must NOT be partially admitted"
        );
        Ok(())
    }

    /// Unplaced and Prefer namespaces are UNAFFECTED: a node-less worker registers
    /// normally (byte-identical to the pre-P2-I1 behaviour). Only Pinned gates.
    #[tokio::test]
    async fn unplaced_and_prefer_admission_is_unaffected_by_the_pinned_gate()
    -> Result<(), ServerError> {
        let store = namespace_store();
        // `unpl` is left Unplaced (default); `pref` is Prefer{n1}. A node-less
        // worker must be admitted into BOTH.
        store
            .register_namespace("pref", NamespaceOrigin::Explicit)
            .await?;
        store
            .set_namespace_placement(
                "pref",
                NamespacePlacement::Prefer {
                    nodes: ["n1".to_owned()].into_iter().collect(),
                },
            )
            .await?;
        let registry = minting_registry(&store, AutoCreate::Open);

        let (tx_a, _rx_a) = mpsc::channel(1);
        let _reg_a = registry
            .accept_registration(
                &guard(),
                &caller("unpl"),
                &registration_full(&["unpl"], "", "", &["charge"]),
                tx_a,
            )
            .await?;
        let (tx_b, _rx_b) = mpsc::channel(1);
        let _reg_b = registry
            .accept_registration(
                &guard(),
                &caller("pref"),
                &registration_full(&["pref"], "", "", &["charge"]),
                tx_b,
            )
            .await?;

        assert_eq!(
            registry
                .workers_for("unpl", DEFAULT_TASK_QUEUE, "charge", None)?
                .len(),
            1,
            "an Unplaced namespace admits a node-less worker unchanged"
        );
        assert_eq!(
            registry
                .workers_for("pref", DEFAULT_TASK_QUEUE, "charge", None)?
                .len(),
            1,
            "a Prefer namespace admits a node-less worker unchanged (only Pinned gates)"
        );
        Ok(())
    }

    /// A default (no-minter) registry is byte-identical: the placement gate is a
    /// no-op with no minter installed, so a node-less worker registers freely even
    /// though there is no way to have set a placement in the first place.
    #[tokio::test]
    async fn no_minter_registry_skips_the_placement_gate() -> Result<(), ServerError> {
        let registry = ConnectedWorkerRegistry::default();
        let (tx, _rx) = mpsc::channel(1);
        let _registration = registry
            .accept_registration(
                &guard(),
                &caller("plain"),
                &registration_full(&["plain"], "", "", &["charge"]),
                tx,
            )
            .await?;
        assert_eq!(
            registry
                .workers_for("plain", DEFAULT_TASK_QUEUE, "charge", None)?
                .len(),
            1,
            "with no minter the placement gate is a no-op — registration is unchanged"
        );
        Ok(())
    }

    #[tokio::test]
    async fn concurrent_registrations_for_a_new_namespace_create_exactly_one_record()
    -> Result<(), ServerError> {
        let store = namespace_store();
        let registry = minting_registry(&store, AutoCreate::Open);

        let mut handles = Vec::new();
        for _ in 0..8 {
            let registry = registry.clone();
            handles.push(tokio::spawn(async move {
                let (tx, rx) = mpsc::channel(1);
                let outcome = registry
                    .accept_registration(
                        &guard(),
                        &caller("rush"),
                        &registration("rush", &["charge"]),
                        tx,
                    )
                    .await;
                // Keep the receiver alive for the duration of the registration.
                drop(rx);
                outcome.map(|registration| registration.worker_id())
            }));
        }
        for handle in handles {
            handle
                .await
                .map_err(|_| ServerError::lock_poisoned("registration task panicked"))??;
        }

        let all = store.list_namespaces().await?;
        assert_eq!(
            all.iter().filter(|r| r.name == "rush").count(),
            1,
            "racing registrations must converge on exactly one durable record"
        );
        Ok(())
    }

    #[tokio::test]
    async fn closed_rejects_unknown_namespace_and_does_not_create_it() -> Result<(), ServerError> {
        let store = namespace_store();
        let registry = minting_registry(&store, AutoCreate::Closed);
        let (tx, _rx) = mpsc::channel(1);

        let denied = registry
            .accept_registration(
                &guard(),
                &caller("ghost"),
                &registration("ghost", &["charge"]),
                tx,
            )
            .await;
        assert!(
            matches!(denied, Err(ServerError::Namespace { .. })),
            "closed policy must reject an unknown namespace"
        );
        assert!(
            store.get_namespace("ghost").await?.is_none(),
            "closed policy must NOT create the namespace it rejected"
        );
        let tq = DEFAULT_TASK_QUEUE;
        assert!(
            registry
                .workers_for("ghost", tq, "charge", None)?
                .is_empty(),
            "a rejected registration must not insert a worker"
        );
        Ok(())
    }

    #[tokio::test]
    async fn closed_admits_a_known_namespace() -> Result<(), ServerError> {
        let store = namespace_store();
        // Pre-mint the namespace (the POST /namespaces escape hatch's effect).
        store
            .register_namespace("known", NamespaceOrigin::Explicit)
            .await?;
        let registry = minting_registry(&store, AutoCreate::Closed);
        let (tx, _rx) = mpsc::channel(1);

        // Bind the registration token: dropping it deregisters the worker.
        let _registration = registry
            .accept_registration(
                &guard(),
                &caller("known"),
                &registration("known", &["charge"]),
                tx,
            )
            .await?;
        let tq = DEFAULT_TASK_QUEUE;
        assert_eq!(
            registry.workers_for("known", tq, "charge", None)?.len(),
            1,
            "a known namespace must register under closed policy"
        );
        Ok(())
    }

    #[tokio::test]
    async fn no_minter_leaves_registration_untouched() -> Result<(), ServerError> {
        // The default registry installs no minter: registration succeeds and
        // never touches any namespace registry (byte-identical legacy path).
        let registry = ConnectedWorkerRegistry::default();
        let (tx, _rx) = mpsc::channel(1);
        let _registration = registry
            .accept_registration(
                &guard(),
                &caller("orders"),
                &registration("orders", &["charge"]),
                tx,
            )
            .await?;
        let tq = DEFAULT_TASK_QUEUE;
        assert_eq!(registry.workers_for("orders", tq, "charge", None)?.len(), 1);
        Ok(())
    }

    /// R1 census: an address no worker has ever served reports an empty fleet
    /// and no poller age at all — "never seen" is not "seen long ago".
    #[test]
    fn a_never_served_address_censuses_empty_with_no_poller_age() -> Result<(), ServerError> {
        let registry = ConnectedWorkerRegistry::default();
        let census = registry.pool_census("default", "general", "greet", None)?;
        assert_eq!(census.workers_in_pool, 0);
        assert_eq!(census.workers_serving_activity, 0);
        assert_eq!(census.compatible_workers, 0);
        assert_eq!(census.last_compatible_poller_age, None);
        assert!(!census.is_served());
        Ok(())
    }

    /// R1 census: pool membership, activity coverage, and node coverage are
    /// three separate counts — that separation is what tells `NO_LIVE_POLLERS`
    /// apart from `POLLERS_INCOMPATIBLE`.
    #[test]
    fn the_census_separates_pool_activity_and_node_coverage() -> Result<(), ServerError> {
        let registry = ConnectedWorkerRegistry::default();
        let (tx, _rx) = mpsc::channel(1);
        let _worker = registry.register_namespaces(
            [String::from("default")],
            "general",
            Some(String::from("n1")),
            [String::from("greet")].iter(),
            tx,
        )?;

        let unpinned = registry.pool_census("default", "general", "greet", None)?;
        assert_eq!(unpinned.workers_in_pool, 1);
        assert_eq!(unpinned.workers_serving_activity, 1);
        assert_eq!(unpinned.compatible_workers, 1);
        assert_eq!(unpinned.last_compatible_poller_age, Some(Duration::ZERO));

        // Same pool, activity nobody advertises: pollers are live but
        // incompatible.
        let other_activity = registry.pool_census("default", "general", "settle", None)?;
        assert_eq!(other_activity.workers_in_pool, 1);
        assert_eq!(other_activity.workers_serving_activity, 0);
        assert_eq!(other_activity.compatible_workers, 0);

        // Same activity, wrong node: coverage exists in the pool but not for
        // this dispatch.
        let wrong_node = registry.pool_census("default", "general", "greet", Some("n2"))?;
        assert_eq!(wrong_node.workers_serving_activity, 1);
        assert_eq!(wrong_node.compatible_workers, 0);
        assert_eq!(wrong_node.last_compatible_poller_age, None);
        Ok(())
    }

    /// ACCEPTANCE (b) FOR #58, wiring half: a worker whose PROCESS looks alive
    /// but which the server cannot REACH on the push leg must lose dispatch
    /// eligibility — and losing it must actually remove it from selection.
    ///
    /// This is the composition the lease split exists for. The tracker half is
    /// pinned in `heartbeat::reachability_tests`; this pins the half that turns
    /// that verdict into a dispatch decision, using the SAME two types the
    /// liveness probe wires together — a real [`HeartbeatTracker`] census
    /// feeding a real [`ConnectedWorkerRegistry`].
    ///
    /// The staging is Finding B exactly: the worker's own liveness pump keeps
    /// beating (`record_connection_activity`) well past the window, so every
    /// signal that says "this process is alive" says so — and NO ping is ever
    /// answered, so the one signal that says "the server can reach it" is
    /// absent. Before the split those were one lease and the pump's weaker
    /// positive evidence buried the ping's negative evidence.
    ///
    /// The control is the second half of the test: the same worker, reachable,
    /// must still be selected. Without it a registry that selected NOBODY would
    /// satisfy the first assertion and prove nothing.
    #[test]
    fn a_pump_alive_worker_the_server_cannot_reach_is_not_selected() -> Result<(), ServerError> {
        const WINDOW: Duration = Duration::from_secs(30);

        let registry = ConnectedWorkerRegistry::default();
        let tracker = HeartbeatTracker::new(WINDOW);
        let (tx, _rx) = mpsc::channel(1);
        let worker = registry.register_namespaces(
            [String::from("default")],
            "general",
            None,
            [String::from("greet")].iter(),
            tx,
        )?;
        let Some(worker_id) = worker.worker_id() else {
            return Err(test_failure("registration carries an id"));
        };
        let start = Instant::now();
        tracker.register_connection(worker_id, start)?;

        // Publishing the tracker's verdict is one motion the liveness probe
        // performs at the end of every round, and this test performs it four
        // times. Written once so the four publications cannot drift apart and
        // quietly stop testing the same thing.
        let publish = |now: Instant| -> Result<(), ServerError> {
            registry.set_dispatch_ineligible(
                tracker
                    .unreachable_workers(now)?
                    .into_iter()
                    .map(|excluded| excluded.worker_id)
                    .collect(),
            )
        };

        // Baseline: the worker SERVES ITS PROBATION and is therefore selectable.
        // Registration alone grants nothing — the handshake ack is sent, never
        // confirmed received — so eligibility here is earned by answered pings.
        // If this did not hold, no worker could ever be dispatched to.
        for _ in 0..DISPATCH_PROBATION_PINGS {
            assert!(
                tracker.record_dispatch_reachability(worker_id, start)?,
                "the worker is tracked while it serves its probation"
            );
        }
        publish(start)?;
        assert!(
            registry
                .select_worker("default", "general", "greet", None)?
                .is_some(),
            "a worker that has answered a full run of pings must be selectable: this is the \
             control, and without it a registry selecting NOBODY would satisfy every assertion \
             below"
        );

        // Now the poisoned-connection shape: the pump beats past the window and
        // not one ping is answered. Each unanswered probe is recorded exactly as
        // the liveness probe records it, because that is the trajectory a real
        // poisoned connection takes — staleness alone would be a shape only a
        // stopped probe could produce.
        let much_later = start + WINDOW * 4;
        assert!(
            tracker.record_connection_activity(worker_id, much_later)?,
            "the worker is still tracked; its process is plainly alive"
        );
        assert!(
            tracker.record_dispatch_unreachable(worker_id)?,
            "the probe fired and went unanswered"
        );
        publish(much_later)?;

        assert!(
            registry.is_dispatch_ineligible(worker_id)?,
            "a worker the server cannot reach must be marked ineligible however alive its \
             process looks"
        );
        assert!(
            registry
                .select_worker("default", "general", "greet", None)?
                .is_none(),
            "an ineligible worker must not be SELECTED: selecting one produces a dispatch that \
             can only fail, and on the liminal transport it fails by consuming connection \
             capacity — making the unreachability worse"
        );

        // The recovery path: exclusion must be WITHDRAWABLE, or a single bad
        // round would strand a healthy worker forever. It is withdrawn by a
        // served probation, not by one answer — and the intermediate assertion
        // below pins that distinction rather than assuming it.
        assert!(
            tracker.record_dispatch_reachability(worker_id, much_later)?,
            "the worker is still tracked"
        );
        publish(much_later)?;
        assert!(
            registry.is_dispatch_ineligible(worker_id)?,
            "one answer part-way through a fresh probation must NOT restore eligibility: a link \
             answering one probe in three would otherwise flap in and out of selection"
        );

        for _ in 1..DISPATCH_PROBATION_PINGS {
            assert!(
                tracker.record_dispatch_reachability(worker_id, much_later)?,
                "the worker is still tracked"
            );
        }
        publish(much_later)?;
        assert!(
            !registry.is_dispatch_ineligible(worker_id)?,
            "an answered ping must clear the exclusion"
        );
        assert!(
            registry
                .select_worker("default", "general", "greet", None)?
                .is_some(),
            "and the worker must be selectable again"
        );
        Ok(())
    }

    /// R1 census: after the last compatible worker leaves, the address reports
    /// how long ago it was served — the age an operator sees on the parked
    /// dispatch's WARN.
    #[test]
    fn a_departed_worker_leaves_a_last_compatible_poller_age() -> Result<(), ServerError> {
        let registry = ConnectedWorkerRegistry::default();
        let (tx, _rx) = mpsc::channel(1);
        let worker = registry.register_namespaces(
            [String::from("default")],
            "general",
            None,
            [String::from("greet")].iter(),
            tx,
        )?;
        worker.deregister()?;

        let census = registry.pool_census("default", "general", "greet", None)?;
        assert_eq!(census.workers_in_pool, 0);
        assert_eq!(census.compatible_workers, 0);
        let age = census
            .last_compatible_poller_age
            .ok_or_else(|| test_failure("a departed worker must leave an age behind"))?;
        assert!(
            age < Duration::from_secs(60),
            "the recorded departure is implausibly old: {age:?}"
        );

        // A worker returning to the address clears the departure record: the
        // map that answers "how long ago" never accumulates live addresses.
        let (tx, _rx) = mpsc::channel(1);
        let _back = registry.register_namespaces(
            [String::from("default")],
            "general",
            None,
            [String::from("greet")].iter(),
            tx,
        )?;
        let served = registry.pool_census("default", "general", "greet", None)?;
        assert_eq!(served.last_compatible_poller_age, Some(Duration::ZERO));
        assert!(served.is_served());
        Ok(())
    }
}
