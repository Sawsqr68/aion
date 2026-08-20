//! `ServerError` taxonomy for server library modules.

use std::borrow::Cow;
use std::net::SocketAddr;
use std::path::PathBuf;

use aion::EngineError;
use aion_core::{ActivityId, WorkflowId};
use aion_proto::WireError;
use aion_store::StoreError;
use thiserror::Error;

#[path = "error_engine.rs"]
mod engine;
#[path = "error_process_exit.rs"]
mod process_exit;

/// Server-library error taxonomy.
#[derive(Debug, Error)]
pub enum ServerError {
    /// Operator configuration could not be loaded or validated.
    #[error("configuration error: {message}")]
    Config {
        /// Redacted, operator-facing failure message.
        message: String,
    },

    /// A path-ambient store backend was configured beneath a renameable directory.
    #[error(
        "unsafe store.data_dir `{}`: ancestor `{}` is not owner-controlled: {reason}; \
         leave store.data_dir unset so it defaults beneath the private Aion home \
         (`$AION_HOME`, default `$HOME/.aion`), or set a path whose ancestor chain is \
         owner-only (a leading `~` expands against $HOME; a relative path resolves \
         against the server's working directory)",
        .data_root.display(),
        .component.display()
    )]
    UnsafeDataRootAncestor {
        /// Descriptor-resolved data root that the backend would use by pathname.
        data_root: PathBuf,
        /// First unsafe component in the resolved root's ancestor chain.
        component: PathBuf,
        /// Ownership, mode, or inspection failure that made the component unsafe.
        reason: String,
    },

    /// A transport listener could not bind or start.
    #[error("{transport} transport failed at {address}: {message}")]
    TransportBind {
        /// Transport name.
        transport: &'static str,
        /// Configured listener address.
        address: SocketAddr,
        /// Redacted, operator-facing failure message.
        message: String,
    },

    /// A running transport task aborted: it panicked or was cancelled.
    #[error("{transport} transport task failed: {message}")]
    Transport {
        /// Transport name.
        transport: &'static str,
        /// Redacted, operator-facing failure message.
        message: String,
    },

    /// A termination-signal listener could not be installed or failed.
    #[error("{listener} listener failed: {message}")]
    SignalListener {
        /// Listener name (`SIGTERM`, `SIGINT`, or the portable fallback).
        listener: &'static str,
        /// Redacted, operator-facing failure message.
        message: String,
    },

    /// The death note — the durable record of what killed the process — could
    /// not be armed at boot.
    #[error("death note error: {message}")]
    DeathNote {
        /// Redacted, operator-facing failure message.
        message: String,
    },

    /// Namespace validation or authorization failed.
    #[error("namespace error: {message}")]
    Namespace {
        /// Redacted namespace failure message.
        message: String,
    },

    /// Engine call failed.
    #[error("engine call failed: {source}")]
    EngineCall {
        /// Typed engine error returned by the embedded engine.
        #[from]
        source: EngineError,
    },

    /// Store backend call failed before an engine handle was available.
    #[error("store backend failed: {source}")]
    StoreBackend {
        /// Typed store error returned by the configured backend.
        #[from]
        source: StoreError,
    },

    /// Streaming failure.
    #[error("stream failure: {failure}")]
    Stream {
        /// Stream failure class.
        failure: StreamFailure,
    },

    /// A scheduled activity could not be pushed to a worker.
    #[error(
        "worker dispatch failed for namespace {namespace}, activity type {activity_type}: {reason}"
    )]
    WorkerDispatch {
        /// Namespace scoped before dispatch.
        namespace: String,
        /// Activity type requested by the engine.
        activity_type: String,
        /// Redacted dispatch failure reason.
        reason: String,
    },

    /// The worker connection chosen for a dispatch was lost mid-flight: the
    /// connection was already gone at push time, or it closed before the worker
    /// sent its correlated push reply.
    ///
    /// This is DISTINCT from [`Self::WorkerDispatch`]: a `WorkerDispatch` covers a
    /// genuine reply timeout (the worker is alive but slow), a no-worker-available
    /// selection failure, or any other dispatch fault, all of which keep the
    /// outbox's normal exponential backoff. A `WorkerConnectionLost` instead means
    /// the chosen worker is gone (and has already been deregistered by liminal's
    /// `on_worker_unregistered`), so the row can be re-armed for IMMEDIATE re-claim
    /// to fail over to a live worker without waiting out the backoff. The outbox
    /// dispatcher keys its fast-failover decision on this variant.
    #[error("worker connection lost during dispatch on {channel}: {detail}")]
    WorkerConnectionLost {
        /// Row-derived dispatch channel for operator diagnostics.
        channel: String,
        /// Redacted, operator-facing description of how the connection was lost.
        detail: String,
    },

    /// The worker connection chosen for a dispatch refused admission because it
    /// already holds its pending-push cap — the worker is ALIVE and WORKING,
    /// its connection is simply full of held dispatches.
    ///
    /// DISTINCT from both [`Self::WorkerDispatch`] (which consumes a retry
    /// attempt on backoff) and [`Self::WorkerConnectionLost`] (immediate
    /// attempt-consuming failover): a busy worker is neither slow to reply nor
    /// gone, so the outbox re-arms the row ATTEMPT-NEUTRALLY after a short
    /// backoff — capacity pressure must never spend the retry budget that
    /// prices genuine delivery failures, and must never dead-letter work.
    #[error("worker connection busy during dispatch on {channel}: {detail}")]
    WorkerBusy {
        /// Row-derived dispatch channel for operator diagnostics.
        channel: String,
        /// Redacted, operator-facing description of the admission refusal.
        detail: String,
    },

    /// A second dispatcher tried to install a responder for an execution site
    /// whose prior attempt is still held live.
    #[error(
        "pending activity collision for workflow {workflow_id}, activity {activity_id}: \
         a live responder already owns this execution site"
    )]
    PendingActivityCollision {
        /// Workflow whose responder slot is already occupied.
        workflow_id: WorkflowId,
        /// Activity site whose responder slot is already occupied.
        activity_id: ActivityId,
    },

    /// A worker result did not prove ownership of the current execution
    /// generation and was refused before reaching workflow state.
    #[error(
        "activity completion rejected for workflow {workflow_id}, activity {activity_id}: {reason}"
    )]
    ActivityCompletionRejected {
        /// Workflow named by the submitted completion.
        workflow_id: WorkflowId,
        /// Activity site named by the submitted completion.
        activity_id: ActivityId,
        /// Typed reason the completion token did not authorize this write.
        reason: CompletionRejectionReason,
    },

    /// A second server-executed declared body was started for an execution site
    /// whose prior attempt is still running here.
    ///
    /// Distinct from [`Self::PendingActivityCollision`], which is about the
    /// responder slot a dispatch installs: this is about the PROCESS. The
    /// registry that makes a running declared command cancellable holds one
    /// cancellation handle per attempt, so admitting a second execution would
    /// replace the first command's handle — and a replaced handle is a command
    /// running on the operator's machine that nothing can stop. Refusing the
    /// second is the only outcome that leaves both attempts accounted for.
    #[error(
        "declared command for workflow {workflow_id}, activity {activity_id} attempt {attempt} \
         is already executing at this server"
    )]
    DeclaredAttemptCollision {
        /// Workflow whose declared body is already executing.
        workflow_id: WorkflowId,
        /// Activity site whose declared body is already executing.
        activity_id: ActivityId,
        /// Attempt whose declared body is already executing.
        attempt: u32,
    },

    /// A lock was poisoned and the protected state cannot be trusted.
    #[error("{resource} lock was poisoned")]
    LockPoisoned {
        /// Protected resource name.
        resource: &'static str,
    },

    /// A failure already translated into the public wire taxonomy.
    #[error("wire error: {wire}")]
    Wire {
        /// Stable wire error.
        wire: WireError,
    },
}

/// Typed completion-fence refusal classes.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CompletionRejectionReason {
    /// A worker from before completion-token support omitted the required token.
    #[error("completion token is missing (worker registration era is incompatible)")]
    MissingCompletionToken,
    /// No generation is currently authorized for this activity execution.
    #[error("no execution generation is currently accepting completion")]
    NoCurrentGeneration,
    /// A later dispatch superseded the submitted generation.
    #[error("completion token belongs to a stale execution generation")]
    StaleGeneration,
}

/// Bounded-stream and connection failure classes.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum StreamFailure {
    /// Bounded per-connection buffer overflowed because the consumer lagged.
    #[error("consumer lagged behind bounded buffer")]
    Lagged,
    /// Subscriber closed the connection.
    #[error("subscriber connection closed")]
    Closed,
    /// Upstream engine event stream ended unexpectedly.
    #[error("engine event stream closed")]
    UpstreamClosed,
}

impl From<WireError> for ServerError {
    fn from(wire: WireError) -> Self {
        Self::Wire { wire }
    }
}

impl ServerError {
    /// Convert a server error that crosses a transport boundary into the stable
    /// public wire taxonomy.
    #[must_use]
    pub fn to_wire_error(&self) -> WireError {
        match self {
            Self::Config { .. }
            | Self::UnsafeDataRootAncestor { .. }
            | Self::TransportBind { .. }
            | Self::Transport { .. }
            | Self::SignalListener { .. }
            | Self::DeathNote { .. }
            | Self::LockPoisoned { .. } => WireError::backend("server backend failure"),
            Self::ActivityCompletionRejected { .. } => {
                WireError::backend("stale activity completion rejected")
            }
            Self::PendingActivityCollision { .. } => {
                WireError::backend("pending activity collision")
            }
            Self::DeclaredAttemptCollision { .. } => {
                WireError::backend("declared command attempt collision")
            }
            Self::WorkerDispatch { .. } => WireError::backend("worker dispatch failed"),
            Self::WorkerConnectionLost { .. } => {
                WireError::backend("worker connection lost during dispatch")
            }
            Self::WorkerBusy { .. } => WireError::backend("worker connection busy during dispatch"),
            Self::Namespace { message } => WireError::namespace_denied(message.clone()),
            Self::EngineCall { source } => wire_from_engine(source),
            Self::StoreBackend { source } => wire_from_store(source),
            Self::Stream { failure } => match failure {
                StreamFailure::Lagged => WireError::lagged("subscriber lagged behind"),
                StreamFailure::Closed | StreamFailure::UpstreamClosed => {
                    WireError::backend("event stream closed")
                }
            },
            Self::Wire { wire } => wire.clone(),
        }
    }

    /// Return true when this is an operator configuration failure.
    #[must_use]
    pub const fn is_config(&self) -> bool {
        matches!(
            self,
            Self::Config { .. } | Self::UnsafeDataRootAncestor { .. }
        )
    }

    /// Construct a namespace-denied error without embedding authorization logic.
    #[must_use]
    pub fn namespace_denied(message: impl Into<String>) -> Self {
        Self::Namespace {
            message: message.into(),
        }
    }

    /// Construct the loud, whole-registration rejection when a worker's advertised
    /// `node` violates a `Pinned{L}` namespace's placement (Control-Plane Phase 2,
    /// P2-I1). Names the offending namespace, the worker's advertised node (or
    /// "none"), and the required label set, so the operator sees exactly why the
    /// registration was refused. Carried on the namespace-denied wire code — a
    /// registration refused on isolation grounds is a namespace-authorization
    /// failure, not a transient dispatch error.
    #[must_use]
    pub fn placement_admission_denied(
        namespace: &str,
        worker_node: Option<&str>,
        required: &std::collections::BTreeSet<String>,
    ) -> Self {
        let node = worker_node.unwrap_or("none");
        let required = required
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        Self::namespace_denied(format!(
            "worker registration rejected: namespace {namespace} is Pinned to node label(s) \
             [{required}] but the worker advertises node {node}, which is not in the required set"
        ))
    }

    /// Construct a deploy-authorization denial carried on the dedicated
    /// `deploy_denied` wire code (deploy is not a namespace operation).
    #[must_use]
    pub fn deploy_denied(message: impl Into<String>) -> Self {
        Self::Wire {
            wire: WireError::deploy_denied(message),
        }
    }

    /// Construct a lagged-stream error.
    #[must_use]
    pub const fn lagged_stream() -> Self {
        Self::Stream {
            failure: StreamFailure::Lagged,
        }
    }

    /// Construct a worker-dispatch error.
    #[must_use]
    pub fn worker_dispatch(
        namespace: impl Into<String>,
        activity_type: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::WorkerDispatch {
            namespace: namespace.into(),
            activity_type: activity_type.into(),
            reason: reason.into(),
        }
    }

    /// Construct a worker-connection-lost error for a dispatch whose chosen
    /// worker connection was gone at push time or closed before replying.
    #[must_use]
    pub fn worker_connection_lost(channel: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::WorkerConnectionLost {
            channel: channel.into(),
            detail: detail.into(),
        }
    }

    /// Return true when this is a lost-worker-connection dispatch failure.
    ///
    /// The outbox dispatcher keys its fast cross-node failover on this: a lost
    /// connection means the worker is gone (already deregistered), so the row is
    /// re-armed for immediate re-claim instead of waiting out the retry backoff.
    #[must_use]
    pub const fn is_worker_connection_lost(&self) -> bool {
        matches!(self, Self::WorkerConnectionLost { .. })
    }

    /// Construct a worker-busy error for a dispatch refused at push admission
    /// because the worker's connection already holds its pending-push cap.
    #[must_use]
    pub fn worker_busy(channel: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::WorkerBusy {
            channel: channel.into(),
            detail: detail.into(),
        }
    }

    /// Return true when this is a busy-worker admission refusal.
    ///
    /// The outbox dispatcher keys its ATTEMPT-NEUTRAL re-arm on this: a full
    /// connection means the worker is alive and holding earlier dispatches, so
    /// the row waits for capacity without spending the retry budget that prices
    /// genuine delivery failures.
    #[must_use]
    pub const fn is_worker_busy(&self) -> bool {
        matches!(self, Self::WorkerBusy { .. })
    }

    /// Construct a lock-poison error at the lock boundary.
    #[must_use]
    pub const fn lock_poisoned(resource: &'static str) -> Self {
        Self::LockPoisoned { resource }
    }
}

/// Stable structured error metadata for tracing events.
#[derive(Clone)]
pub struct ErrorTraceFields<'a> {
    /// Outer error type recorded in the `error_type` tracing field.
    pub error_type: Cow<'a, str>,
    /// Optional inner store error type for `StoreError` records.
    pub store_error_type: Option<&'static str>,
    /// Human-readable reason safe for operator logs.
    pub reason: &'a dyn std::fmt::Display,
}

impl ServerError {
    /// Return stable typed fields for structured error logging.
    #[must_use]
    pub fn trace_fields(&self) -> ErrorTraceFields<'_> {
        match self {
            Self::Config { message } => ErrorTraceFields {
                error_type: Cow::Borrowed("Config"),
                store_error_type: None,
                reason: message,
            },
            Self::UnsafeDataRootAncestor { reason, .. } => ErrorTraceFields {
                error_type: Cow::Borrowed("UnsafeDataRootAncestor"),
                store_error_type: None,
                reason,
            },
            Self::TransportBind { message, .. } => ErrorTraceFields {
                error_type: Cow::Borrowed("TransportBind"),
                store_error_type: None,
                reason: message,
            },
            Self::Transport { message, .. } => ErrorTraceFields {
                error_type: Cow::Borrowed("Transport"),
                store_error_type: None,
                reason: message,
            },
            Self::SignalListener { message, .. } => ErrorTraceFields {
                error_type: Cow::Borrowed("SignalListener"),
                store_error_type: None,
                reason: message,
            },
            Self::DeathNote { message } => ErrorTraceFields {
                error_type: Cow::Borrowed("DeathNote"),
                store_error_type: None,
                reason: message,
            },
            Self::Namespace { message } => ErrorTraceFields {
                error_type: Cow::Borrowed("Namespace"),
                store_error_type: None,
                reason: message,
            },
            Self::EngineCall { source } => engine_trace_fields(source),
            Self::StoreBackend { source } => store_trace_fields(source),
            Self::Stream { failure } => ErrorTraceFields {
                error_type: Cow::Borrowed("Stream"),
                store_error_type: None,
                reason: failure,
            },
            Self::WorkerDispatch { reason, .. } => ErrorTraceFields {
                error_type: Cow::Borrowed("WorkerDispatch"),
                store_error_type: None,
                reason,
            },
            Self::WorkerConnectionLost { detail, .. } => ErrorTraceFields {
                error_type: Cow::Borrowed("WorkerConnectionLost"),
                store_error_type: None,
                reason: detail,
            },
            Self::WorkerBusy { detail, .. } => ErrorTraceFields {
                error_type: Cow::Borrowed("WorkerBusy"),
                store_error_type: None,
                reason: detail,
            },
            Self::PendingActivityCollision { activity_id, .. } => ErrorTraceFields {
                error_type: Cow::Borrowed("PendingActivityCollision"),
                store_error_type: None,
                reason: activity_id,
            },
            Self::DeclaredAttemptCollision { activity_id, .. } => ErrorTraceFields {
                error_type: Cow::Borrowed("DeclaredAttemptCollision"),
                store_error_type: None,
                reason: activity_id,
            },
            Self::ActivityCompletionRejected { reason, .. } => ErrorTraceFields {
                error_type: Cow::Borrowed("ActivityCompletionRejected"),
                store_error_type: None,
                reason,
            },
            Self::LockPoisoned { resource } => ErrorTraceFields {
                error_type: Cow::Borrowed("LockPoisoned"),
                store_error_type: None,
                reason: resource,
            },
            Self::Wire { wire } => ErrorTraceFields {
                error_type: wire
                    .error_type
                    .as_deref()
                    .map_or_else(|| Cow::Borrowed(wire.code.as_str()), Cow::Borrowed),
                store_error_type: None,
                reason: wire,
            },
        }
    }
}

/// The wire type name for each #117(c) never-alive cancellation refusal.
///
/// Each keeps its OWN name rather than collapsing into one: an operator told
/// "not found" for a run whose history they are reading learns nothing, and
/// telling them "this workflow already has a writer" versus "your package now
/// loads again" is the difference between a dead end and the next action.
///
/// Total without a panic. A variant that should never reach here gets the
/// family label — an admittedly vague answer, which is the right failure mode
/// for a function whose only job is to name an error someone is already
/// receiving.
fn never_alive_error_type(source: &EngineError) -> &'static str {
    match source {
        EngineError::TerminalWriterUnavailable { .. } => "TerminalWriterUnavailable",
        EngineError::TerminalWriterHeld { .. } => "TerminalWriterHeld",
        EngineError::RunIsRecoverable { .. } => "RunIsRecoverable",
        EngineError::NoResidencyVerdict { .. } => "NoResidencyVerdict",
        _ => "EngineError",
    }
}

/// A durability failure that is really a STORE failure keeps the store's own
/// trace fields; the rest are the engine's.
///
/// Extracted from [`engine_trace_fields`] because a nested match is a different
/// question from the flat dispatch around it, not to satisfy a line count.
fn durability_trace_fields<'a>(
    durability: &'a aion::durability::DurabilityError,
    source: &'a EngineError,
) -> ErrorTraceFields<'a> {
    match durability {
        aion::durability::DurabilityError::Store(store) => store_trace_fields(store),
        aion::durability::DurabilityError::NonDeterminism(_)
        | aion::durability::DurabilityError::HistoryShape { .. }
        | aion::durability::DurabilityError::SearchAttribute(_) => {
            simple_engine_fields("Durability", source)
        }
        // Its own label rather than the generic `Durability`, and the SAME
        // label the engine-level variant gets below, so an operator searching
        // traces for "did this engine begin closing" finds every seam that
        // refused for that reason under one name instead of one name and a
        // generic one.
        aion::durability::DurabilityError::EngineTaskEpochClosed { .. } => {
            simple_engine_fields("EngineTaskEpochClosed", source)
        }
    }
}

fn engine_trace_fields(source: &EngineError) -> ErrorTraceFields<'_> {
    match source {
        EngineError::WorkflowNotFound { .. } => simple_engine_fields("WorkflowNotFound", source),
        // #117(c): the never-alive cancellation path's four refusals.
        EngineError::TerminalWriterUnavailable { .. }
        | EngineError::TerminalWriterHeld { .. }
        | EngineError::RunIsRecoverable { .. }
        | EngineError::NoResidencyVerdict { .. } => {
            simple_engine_fields(never_alive_error_type(source), source)
        }
        EngineError::InvalidState { .. } => simple_engine_fields("InvalidState", source),
        EngineError::ScheduleNotFound { .. } => simple_engine_fields("ScheduleNotFound", source),
        EngineError::ShuttingDown => simple_engine_fields("ShuttingDown", source),
        EngineError::EngineTaskEpochClosed { .. } => {
            simple_engine_fields("EngineTaskEpochClosed", source)
        }
        EngineError::Store(store) => store_trace_fields(store),
        EngineError::Durability(durability) => durability_trace_fields(durability, source),
        EngineError::MissingStore => simple_engine_fields("MissingStore", source),
        EngineError::MissingVisibilityStore => {
            simple_engine_fields("MissingVisibilityStore", source)
        }
        EngineError::ConflictingEventPublisher => {
            simple_engine_fields("ConflictingEventPublisher", source)
        }
        EngineError::EventStreaming(_) => simple_engine_fields("EventStreaming", source),
        EngineError::Load { .. } => simple_engine_fields("Load", source),
        EngineError::UnenforceableContract { .. } => {
            simple_engine_fields("UnenforceableContract", source)
        }
        EngineError::UnknownVersion { .. } => simple_engine_fields("UnknownVersion", source),
        EngineError::VersionPinned { .. } => simple_engine_fields("VersionPinned", source),
        EngineError::RouteActive { .. } => simple_engine_fields("RouteActive", source),
        EngineError::ManifestMismatch { .. } => simple_engine_fields("ManifestMismatch", source),
        EngineError::Package(_) => simple_engine_fields("Package", source),
        EngineError::ContractIdentity { .. } => simple_engine_fields("ContractIdentity", source),
        EngineError::NoQueueDeclaration { .. } => {
            simple_engine_fields("NoQueueDeclaration", source)
        }
        EngineError::StartInputRefused { .. } => simple_engine_fields("StartInputRefused", source),
        EngineError::SignalRefused { .. } => simple_engine_fields("SignalRefused", source),
        EngineError::Schedule { .. } => simple_engine_fields("Schedule", source),
        EngineError::Runtime { .. } => simple_engine_fields("Runtime", source),
        EngineError::Gate3BifReplacementMissing { .. } => {
            simple_engine_fields("Gate3BifReplacementMissing", source)
        }
        EngineError::StartupRecoveryNotDeferred => {
            simple_engine_fields("StartupRecoveryNotDeferred", source)
        }
        EngineError::StartupRecoveryAlreadyRan => {
            simple_engine_fields("StartupRecoveryAlreadyRan", source)
        }
        EngineError::StartupRecoverySlotPoisoned => {
            simple_engine_fields("StartupRecoverySlotPoisoned", source)
        }
        EngineError::CleanupExecutorPoisoned => {
            simple_engine_fields("CleanupExecutorPoisoned", source)
        }
        EngineError::CleanupExecutorShutdownTimedOut { .. } => {
            simple_engine_fields("CleanupExecutorShutdownTimedOut", source)
        }
        EngineError::RunNotInHistory { .. } => simple_engine_fields("RunNotInHistory", source),
        EngineError::ProcessExitRegistryPoisoned => {
            simple_engine_fields("ProcessExitRegistryPoisoned", source)
        }
        EngineError::ProcessExitOwnershipPoisoned { .. } => {
            simple_engine_fields("ProcessExitOwnershipPoisoned", source)
        }
        EngineError::ProcessExitStatePoisoned { .. }
        | EngineError::ProcessExitSubscriptionUnavailable
        | EngineError::ProcessExitDrainerSpawn { .. }
        | EngineError::ProcessExitDrainerPoisoned
        | EngineError::ProcessExitOutcomeMissingAfterEvent { .. }
        | EngineError::ProcessExitEventStreamDisconnected
        | EngineError::ProcessExitDrainerShutdownTimedOut { .. }
        | EngineError::ProcessExitDrainerPanicked => process_exit::drainer_trace(source),
        EngineError::ProcessExitCallbackDispatcherPoisoned
        | EngineError::ProcessExitCallbackDispatcherUnavailable
        | EngineError::ProcessExitCallbackDispatcherShutdownTimedOut { .. } => {
            process_exit::callback_trace(source)
        }
        EngineError::ProcessExitAlreadyTerminal { .. } => {
            simple_engine_fields("ProcessExitAlreadyTerminal", source)
        }
        EngineError::ActivityDeliveryPoisoned { .. } => {
            simple_engine_fields("ActivityDeliveryPoisoned", source)
        }
        EngineError::RegistryPoisoned => simple_engine_fields("RegistryPoisoned", source),
        EngineError::CatalogPoisoned => simple_engine_fields("CatalogPoisoned", source),
        EngineError::NifRegistration { .. } => simple_engine_fields("NifRegistration", source),
        EngineError::SignalRouter(_) => simple_engine_fields("SignalRouter", source),
        EngineError::Query(query) => simple_engine_fields(engine::query_error_type(query), source),
    }
}

fn simple_engine_fields<'a>(
    error_type: &'static str,
    source: &'a EngineError,
) -> ErrorTraceFields<'a> {
    ErrorTraceFields {
        error_type: Cow::Borrowed(error_type),
        store_error_type: None,
        reason: source,
    }
}

fn store_trace_fields(source: &StoreError) -> ErrorTraceFields<'_> {
    ErrorTraceFields {
        error_type: Cow::Borrowed("StoreError"),
        store_error_type: Some(engine::store_error_type(source)),
        reason: source,
    }
}

fn wire_from_engine(source: &EngineError) -> WireError {
    use EngineError as E;
    use engine::backend_wire as backend;

    match source {
        EngineError::WorkflowNotFound { .. } => {
            WireError::not_found_with_type("WorkflowNotFound", source.to_string())
        }
        // Reopen preconditions use failed-precondition/HTTP 409, not NotFound.
        EngineError::InvalidState { reason } => engine::invalid_state_wire(reason),
        // #117(c). All four are PRECONDITION failures, never NotFound: in every
        // one of them the run exists and its history is readable, and what has
        // failed is a condition on cancelling it here and now. Mapping any of
        // them to 404 would reproduce, one layer out, the exact lie this path
        // exists to stop telling.
        E::TerminalWriterUnavailable { .. }
        | E::TerminalWriterHeld { .. }
        | E::RunIsRecoverable { .. }
        | E::NoResidencyVerdict { .. } => engine::invalid_state_wire(&source.to_string())
            .with_error_type(never_alive_error_type(source)),
        EngineError::ScheduleNotFound { .. } => {
            WireError::not_found_with_type("ScheduleNotFound", source.to_string())
        }
        EngineError::ShuttingDown => {
            WireError::not_running_with_type("ShuttingDown", source.to_string())
        }
        // The engine-task epoch closed before this run's terminal could be
        // appended, so this process is no longer the run's single writer. The
        // work was refused because the engine is going away, not because the
        // request was wrong. Reachable only from an internal completion path,
        // never from a request — but a total match is what keeps a new variant
        // a COMPILE failure here rather than a silent default.
        //
        // 🔴 `backend`, NOT `not_running`, and an earlier revision of this arm
        // had it wrong. The variant's own doc (`crates/aion/src/error.rs:389`)
        // states the opposite of what `not_running` claims: "in both cases THE
        // RUN STAYS `Running`", and a startup sweep re-installs a monitor. A
        // caller told `not_running` is then sent to the CLI hint for that class
        // (`aion-cli/src/render.rs:157-160`) — "the target run is no longer
        // running; `aion list --status running` shows runs that can still serve
        // queries" — which is false about this run twice over, and would have
        // the operator hunting for a terminal that never landed. `backend`
        // makes no claim about the run's state at all and carries the variant's
        // own `Display`, which says exactly what happened; it is also the class
        // this table already gives every other engine-internal refusal
        // (`MissingStore`, `ConflictingEventPublisher`, `EventStreaming`).
        E::EngineTaskEpochClosed { .. } => backend("EngineTaskEpochClosed", source),
        EngineError::Store(store) => wire_from_store(store),
        EngineError::Durability(durability) => engine::durability_wire(durability, source),
        E::MissingStore => backend("MissingStore", source),
        E::MissingVisibilityStore => backend("MissingVisibilityStore", source),
        E::ConflictingEventPublisher => backend("ConflictingEventPublisher", source),
        E::EventStreaming(_) => backend("EventStreaming", source),
        E::Load { .. } => backend("Load", source),
        // The archive is well-formed and the defect is in what it DECLARES, so
        // this is the operator's own input to correct — not a backend fault.
        EngineError::UnenforceableContract { .. } => {
            WireError::invalid_input(source.to_string()).with_error_type("UnenforceableContract")
        }
        // Deploy refusals preserve not-found, state-conflict, and input classes.
        EngineError::UnknownVersion { .. } => {
            WireError::not_found_with_type("UnknownVersion", source.to_string())
        }
        EngineError::VersionPinned { .. } => {
            WireError::version_pinned(source.to_string()).with_error_type("VersionPinned")
        }
        EngineError::RouteActive { .. } => {
            WireError::version_pinned(source.to_string()).with_error_type("RouteActive")
        }
        EngineError::ManifestMismatch { .. } => {
            WireError::invalid_input(source.to_string()).with_error_type("ManifestMismatch")
        }
        E::Package(_) => backend("Package", source),
        E::ContractIdentity { .. } => engine::contract_refusal_wire("ContractIdentity", source),
        E::NoQueueDeclaration { .. } => engine::contract_refusal_wire("NoQueueDeclaration", source),
        E::StartInputRefused { .. } => engine::declared_contract_wire("StartInputRefused", source),
        E::SignalRefused { .. } => engine::declared_contract_wire("SignalRefused", source),
        EngineError::Schedule { .. } => backend("Schedule", source),
        E::Runtime { .. } => backend("Runtime", source),
        E::Gate3BifReplacementMissing { .. } => backend("Gate3BifReplacementMissing", source),
        // Boot-sequencing invariants (#266): misuse of the one-shot deferred
        // recovery slot is a server construction bug, never a caller's fault.
        E::StartupRecoveryNotDeferred => backend("StartupRecoveryNotDeferred", source),
        E::StartupRecoveryAlreadyRan => backend("StartupRecoveryAlreadyRan", source),
        E::StartupRecoverySlotPoisoned => backend("StartupRecoverySlotPoisoned", source),
        E::CleanupExecutorPoisoned => backend("CleanupExecutorPoisoned", source),
        E::CleanupExecutorShutdownTimedOut { .. } => {
            backend("CleanupExecutorShutdownTimedOut", source)
        }
        // aion#94: the registry holds a run that the history it was reconciled
        // against does not contain. Nothing about the caller's request is wrong
        // — it is an engine-internal disagreement between two of the engine's
        // own stores — so `backend`, which makes no claim about the run's state
        // and carries the variant's own Display.
        E::RunNotInHistory { .. } => backend("RunNotInHistory", source),
        E::ProcessExitRegistryPoisoned => backend("ProcessExitRegistryPoisoned", source),
        E::ProcessExitOwnershipPoisoned { .. } => backend("ProcessExitOwnershipPoisoned", source),
        EngineError::ProcessExitStatePoisoned { .. }
        | EngineError::ProcessExitSubscriptionUnavailable
        | EngineError::ProcessExitDrainerSpawn { .. }
        | EngineError::ProcessExitDrainerPoisoned
        | EngineError::ProcessExitOutcomeMissingAfterEvent { .. }
        | EngineError::ProcessExitEventStreamDisconnected
        | EngineError::ProcessExitDrainerShutdownTimedOut { .. }
        | EngineError::ProcessExitDrainerPanicked => process_exit::drainer_wire(source),
        EngineError::ProcessExitCallbackDispatcherPoisoned
        | EngineError::ProcessExitCallbackDispatcherUnavailable
        | EngineError::ProcessExitCallbackDispatcherShutdownTimedOut { .. } => {
            process_exit::callback_wire(source)
        }
        E::ProcessExitAlreadyTerminal { .. } => backend("ProcessExitAlreadyTerminal", source),
        E::ActivityDeliveryPoisoned { .. } => backend("ActivityDeliveryPoisoned", source),
        E::CatalogPoisoned => backend("CatalogPoisoned", source),
        E::RegistryPoisoned => backend("RegistryPoisoned", source),
        E::NifRegistration { .. } => backend("NifRegistration", source),
        E::SignalRouter(_) => backend("SignalRouter", source),
        EngineError::Query(query) => engine::query_wire(query, source),
    }
}

fn wire_from_store(source: &StoreError) -> WireError {
    match source {
        StoreError::SequenceConflict { .. } => WireError::new_with_type(
            aion_proto::WireErrorCode::SequenceConflict,
            "SequenceConflict",
            source.to_string(),
        ),
        StoreError::NotFound { .. } => {
            WireError::not_found_with_type("NotFound", source.to_string())
        }
        StoreError::NotOwner { .. } => {
            WireError::not_owner(source.to_string()).with_error_type("NotOwner")
        }
        StoreError::Backend(_) => WireError::backend_with_type("Backend", source.to_string()),
        StoreError::Serialization(_) => {
            WireError::backend_with_type("Serialization", source.to_string())
        }
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
