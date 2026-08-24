//! Module declarations.

/// Asking the workers holding a cancelled run's activities to stop (#233).
pub mod activity_cancel;
/// Whether a contract refusal is new information, and therefore worth saying.
pub mod admission_audit;
/// Reading one attempt's volatile worker progress note, with typed absence.
pub mod attempt_progress;
/// Per-tenant keyed backpressure at the outbox claim (Control-Plane Phase 2,
/// P2-Q2).
pub mod backpressure;
/// Bridge from engine activity dispatch to connected workers.
pub mod bridge;
/// Worker contract admission shared by every registration transport.
pub mod contracts;
/// Server-side execution of declared action bodies (no worker required).
pub mod declared_body;
/// The refusal, and the followable remedy, for retained versions that declare
/// different bodies for one action.
pub mod declared_body_ambiguity;
/// Stopping the declared bodies this server is executing: the registry that
/// makes a running server-executed attempt visible, and the signal that reaches
/// its process group.
pub mod declared_body_cancel;
/// Which retained version's body a declared-command dispatch runs — the
/// dispatching run's own, not the queue's.
pub mod declared_body_selection;
/// A server-run declared command's output, published onto the same transcript
/// an agent step's is.
pub(crate) mod declared_body_transcript;
/// Activity completion handling and dispatch abstractions.
pub mod dispatch;
/// Task-envelope generation, idempotency keys, and completion fencing.
pub mod envelope;
/// gRPC half of the worker dead-man switch (#197): the transport liveness ping
/// pushed down a worker's task stream, and the registry its answers correlate
/// through.
pub mod grpc_liveness;
/// Worker heartbeat and liveness tracking.
pub mod heartbeat;
/// Server-side mid-run intervention routing (NOI-6): capability gate + attempt
/// owner resolution + push to the owning worker over a pluggable transport.
pub mod intervention;
/// Server half of the liminal connection dead-man switch: the liveness ping/pong
/// wire pair and the probe that keeps a healthy idle connection's lease alive
/// while making a dead one loud on both sides.
#[cfg(feature = "liminal-transport")]
pub mod liminal_liveness;
/// Cross-node outbox dispatch over the liminal bus (#13-0 spike, feature-gated).
#[cfg(feature = "liminal-transport")]
pub mod liminal_transport;
/// The transport-neutral vocabulary the dead-man switch's transport halves
/// share: why a ping failed, and which wire carried it.
pub mod liveness;
/// The outbox dead-letter path: retire the row, tell the workflow, record whether it was told.
pub(crate) mod outbox_dead_letter;
/// Server-side outbox completion delivery into live workflows.
pub mod outbox_delivery;
/// Non-replayed durable-outbox fan-out dispatcher (dormant unless commissioned).
pub mod outbox_dispatcher;
/// Live stale-claim outbox reconciler (dormant unless commissioned).
pub mod outbox_reconciler;
/// Operator-driven redrive of dead-lettered outbox rows, gated on workflow liveness and on
/// whether the dead letter's failure was already judged.
pub mod outbox_redrive;
/// Terminal-workflow outbox settlement sweep for boot and shard adoption (#253).
pub mod outbox_settle;
/// Short-TTL per-namespace placement cache for the dispatcher (Control-Plane
/// Phase 2, P2-P3).
pub mod placement_cache;
/// Unserved-queue honesty at the bridge seam (R1): the four-way taxonomy,
/// service policies, the two service clocks, and the queryable queue state.
pub mod queue_service;
/// Throttled per-namespace quota-state broadcaster for the ops-console live badge
/// (Control-Plane Phase 2, P2-Q3).
pub mod quota_broadcast;
/// Short-TTL per-namespace concurrency-quota cache for the dispatcher's keyed
/// backpressure (Control-Plane Phase 2, P2-Q2).
pub mod quota_cache;
/// Connected-worker registry and handles.
pub mod registry;
/// Managed-worker supervision (W-1..W-4): desired state in, contained OS
/// processes out, with a status surface that is a join of the two.
pub mod supervisor;
/// Transport-domain classification and re-dispatch budget for activities whose
/// worker died before reporting a result.
pub mod transport_loss;
/// Server-side `{workspace_root}` expansion for declared action bodies (#139).
pub mod workspace_root;

pub use activity_cancel::{
    CancelDelivery, CancelRequest, InFlightCancellation, cancel_in_flight_activities,
};
pub use admission_audit::AdmissionAudit;
pub use attempt_progress::{AttemptProgress, attempt_progress};
pub use backpressure::{Backpressure, OwnedShardFraction};
pub use bridge::{OutboxDeliveryCallback, PendingActivities, WorkerActivityDispatcher};
pub use declared_body::{
    DeclaredBodies, DeclaredBodyLookup, DeclaredBodySource, DeclaredCommandDispatcher,
    DispatchingRun, EngineDeclaredBodies,
};
pub use declared_body_ambiguity::{DeclaringVersion, ambiguous_body_refusal};
pub use declared_body_cancel::{DeclaredAttemptRegistration, DeclaredCommandAttempts};
pub use declared_body_selection::select_declared_body;
pub use dispatch::{
    ActivityCompletion, ActivityCompletionOutcome, ActivityCompletionSink, ActivityDispatcher,
    ScheduledActivity, handle_activity_result,
};
pub use envelope::{CompletionFences, CompletionToken, idempotency_key};
pub use grpc_liveness::{GrpcLivenessTarget, GrpcLivenessWaiters};
pub use heartbeat::{
    HeartbeatSweeper, HeartbeatTracker, HeartbeatUpdate, InFlightActivity, LostWorkerReport,
    TaskLiveness, sweep_interval,
};
pub use intervention::{AttemptKey, AttemptOwnerIndex, InterventionRouter, InterventionTransport};
#[cfg(feature = "liminal-transport")]
pub use liminal_liveness::{LivenessPing, LivenessPong, LivenessProbe, LivenessTarget};
#[cfg(feature = "liminal-transport")]
pub use liminal_transport::{
    DispatchRequest, DispatchResponse, InterventionReply, InterventionRequest,
    LiminalCompletionSource, LiminalConnectionNotifier, LiminalInterventionTransport,
    LiminalWorkerDelivery, RegistryLiminalDispatch, channel_for_row, dispatch_channel_name,
};
pub use outbox_delivery::ServerOutboxDeliveryCallback;
pub use outbox_dispatcher::{
    DeliveryGate, OutboxDispatcher, OutboxDispatcherConfig, OutboxRowDispatch, WorkerOutboxDispatch,
};
pub use outbox_reconciler::{OutboxReconciler, OutboxReconcilerConfig};
pub use outbox_redrive::{RedriveRefused, list_dead_letters, redrive_dead_lettered_row};
pub use outbox_settle::settle_terminal_outbox_rows;
pub use placement_cache::{
    PlacementCache, WorkerSelection, preferred_node_order, worker_selection_for,
};
pub use queue_service::{
    ActivityReachability, DeliveryRefusal, EngineQueueDeclarations, ExpiredClock, OpenActivity,
    PoolCensus, QueueDeclaration, QueueDeclarationSource, QueueDeclarations, QueueServiceConfig,
    QueueServiceOverride, QueueServicePolicy, QueueServiceReason, QueueServiceState,
    ServiceAddress, UnavailableSummary, UnservedDispatch, UnservedKey, UnservedQueue,
    WorkerUnavailable, open_activities_in_active_segment,
};
pub use quota_broadcast::QuotaBroadcaster;
pub use quota_cache::QuotaCache;
pub use registry::{
    ConnectedWorkerRegistry, WorkerDelivery, WorkerHandle, WorkerId, WorkerRegistration,
};
pub use supervisor::{
    FleetShutdownReport, ManagedExecutable, ManagedWorkerExit, ManagedWorkerReport,
    ManagedWorkerState, ManagedWorkerStatus, SpawnedBinary, SupervisionError, SupervisionPolicy,
    WorkerSupervisor,
};
pub use transport_loss::{
    TRANSPORT_EXHAUSTED_REASON_PREFIX, TRANSPORT_LOSS_BUDGET_WINDOWS, TransportLossLedger,
    TransportLossVerdict, WORKER_LOST_REASON_PREFIX, is_transport_domain_reason,
    worker_lost_detail,
};
pub use workspace_root::{
    ExpandedCommand, WORKSPACE_ROOT_PLACEHOLDER, WorkspaceRoot, WorkspaceRootError,
};
