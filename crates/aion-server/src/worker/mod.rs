//! Module declarations.

/// Per-tenant keyed backpressure at the outbox claim (Control-Plane Phase 2,
/// P2-Q2).
pub mod backpressure;
/// Bridge from engine activity dispatch to connected workers.
pub mod bridge;
/// Worker contract admission shared by every registration transport.
pub mod contracts;
/// Server-side execution of declared action bodies (no worker required).
pub mod declared_body;
/// Activity completion handling and dispatch abstractions.
pub mod dispatch;
/// Task-envelope generation, idempotency keys, and completion fencing.
pub mod envelope;
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
/// Transport-domain classification and re-dispatch budget for activities whose
/// worker died before reporting a result.
pub mod transport_loss;
/// Server-side `{workspace_root}` expansion for declared action bodies (#139).
pub mod workspace_root;

pub use backpressure::{Backpressure, OwnedShardFraction};
pub use bridge::{OutboxDeliveryCallback, PendingActivities, WorkerActivityDispatcher};
pub use declared_body::{
    DeclaredBodies, DeclaredBodyLookup, DeclaredBodySource, DeclaredCommandDispatcher,
    EngineDeclaredBodies,
};
pub use dispatch::{
    ActivityCompletion, ActivityCompletionOutcome, ActivityCompletionSink, ActivityDispatcher,
    ScheduledActivity, handle_activity_result,
};
pub use envelope::{CompletionFences, CompletionToken, idempotency_key};
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
pub use transport_loss::{
    TRANSPORT_EXHAUSTED_REASON_PREFIX, TRANSPORT_LOSS_BUDGET_WINDOWS, TransportLossLedger,
    TransportLossVerdict, WORKER_LOST_REASON_PREFIX, is_transport_domain_reason,
    worker_lost_detail,
};
pub use workspace_root::{
    ExpandedCommand, WORKSPACE_ROOT_PLACEHOLDER, WorkspaceRoot, WorkspaceRootError,
};
