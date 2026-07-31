//! Engine error taxonomy.

use crate::schedule::{ScheduleError, ScheduleEvaluatorError};
use aion_core::{RunId, ScheduleId, WorkflowId};
use aion_package::{ContentHash, ContractIdentityError, PackageError};
use aion_store::StoreError;

use crate::durability::DurabilityError;

/// Errors returned by the embedded workflow engine.
#[derive(thiserror::Error, Debug)]
pub enum EngineError {
    /// The builder was asked to construct an engine without an event store.
    #[error("engine store is required")]
    MissingStore,

    /// The builder was asked to construct an engine without a visibility store.
    #[error(
        "engine visibility store is required; call EngineBuilder::visibility_store() or EngineBuilder::in_memory_visibility()"
    )]
    MissingVisibilityStore,

    /// A workflow package failed to load or validate for engine registration.
    #[error("workflow package load failed: {reason}")]
    Load {
        /// Human-readable load failure reason.
        reason: String,
    },

    /// A package offered for deployment declares a contract the engine cannot
    /// enforce: at least one declared schema does not compile into a validator.
    ///
    /// Refused at the door rather than admitted, because the alternative is
    /// silent: every admission boundary answers an uncompilable schema by
    /// letting the value through unchecked, so a package that reaches the
    /// catalog with one runs with that part of its declared contract switched
    /// off and only a log line to say so.
    #[error(
        "workflow package `{workflow_type}` declares {count} type(s) the engine cannot compile into a validator, so nothing it declares there could ever be enforced: {detail}"
    )]
    UnenforceableContract {
        /// Logical workflow type of the refused package.
        workflow_type: String,
        /// How many declarations could not be compiled.
        count: usize,
        /// Each unenforceable declaration, named, with the compiler's reason.
        detail: String,
    },

    /// A route or unload targeted a `(workflow type, version)` that is not loaded.
    #[error(
        "workflow `{workflow_type}` version `{version}` is not loaded (loaded versions: {loaded})"
    )]
    UnknownVersion {
        /// Logical workflow type requested by the caller.
        workflow_type: String,
        /// Content-hash version requested by the caller.
        version: ContentHash,
        /// Comma-separated loaded versions of the type, or `none`.
        loaded: String,
    },

    /// An unload was refused because something still pins the version.
    #[error("cannot unload workflow `{workflow_type}` version `{version}`: {pinned_by}")]
    VersionPinned {
        /// Logical workflow type targeted by the unload.
        workflow_type: String,
        /// Content-hash version targeted by the unload.
        version: ContentHash,
        /// What pins the version, naming the concrete holder.
        pinned_by: PinHolder,
    },

    /// An unload was refused because the version is route-active for its type.
    #[error(
        "cannot unload workflow `{workflow_type}` version `{version}`: it is the route-active version; route another version first"
    )]
    RouteActive {
        /// Logical workflow type targeted by the unload.
        workflow_type: String,
        /// Content-hash version targeted by the unload.
        version: ContentHash,
    },

    /// An idempotent re-load presented the resident package identity with a
    /// different manifest. V4 binds beams and the durable execution contract,
    /// but not every packaging/admin field, so this remains the wrong-deploy
    /// tripwire: the resident version is retained and the archive is refused.
    #[error(
        "workflow `{workflow_type}` version `{version}` is already loaded with a different manifest (resident digest {resident_digest}, incoming digest {incoming_digest}); rebuild the archive so its complete manifest matches the resident version, or change its contract-bound content"
    )]
    ManifestMismatch {
        /// Logical workflow type targeted by the load.
        workflow_type: String,
        /// Content-hash version shared by both archives.
        version: ContentHash,
        /// Canonical digest of the resident manifest.
        resident_digest: String,
        /// Canonical digest of the incoming manifest.
        incoming_digest: String,
    },

    /// The builder was given both `event_streaming` and an explicit event-publisher seam.
    #[error(
        "conflicting event publisher configuration: EngineBuilder::event_streaming installs the broadcast publisher and cannot be combined with EngineBuilder::event_publisher"
    )]
    ConflictingEventPublisher,

    /// Live event streaming setup failed.
    #[error("event streaming setup failed: {0}")]
    EventStreaming(#[from] crate::publish::PublishError),

    /// The configured event store returned an error.
    #[error("store error: {0}")]
    Store(#[from] StoreError),

    /// The durability recorder or replay path returned an error.
    #[error("durability error: {0}")]
    Durability(#[from] DurabilityError),

    /// A `.aion` package operation returned an error.
    #[error("package error: {0}")]
    Package(#[from] PackageError),

    /// The selected package identity predates the `.v4` contract commitment.
    #[error("workflow `{workflow_type}` cannot start: {source}")]
    ContractIdentity {
        /// Workflow type selected for the start.
        workflow_type: String,
        /// Typed migration refusal from the package identity boundary.
        #[source]
        source: ContractIdentityError,
    },

    /// The package names activities without a durable queue-scoped contract.
    #[error(
        "NO_QUEUE_DECLARATION: workflow `{workflow_type}` version `{version}` has unscoped activities {activities}; re-deploy from a checked AWL contract"
    )]
    NoQueueDeclaration {
        /// Workflow type selected for the start.
        workflow_type: String,
        /// Exact `.v4` package identity selected for the run.
        version: ContentHash,
        /// Stable comma-separated unscoped activity names.
        activities: String,
    },

    /// A start's input did not satisfy the declared input schema of the exact
    /// package identity the start resolved to.
    ///
    /// Returned at the start boundary BEFORE any history is appended and
    /// before any process is spawned, so a refused start leaves no trace: the
    /// caller sees their own mistake at the moment they made it, with nothing
    /// to clean up.
    #[error(
        "start input for workflow `{workflow_type}` does not satisfy the input type declared by package version `{version}`: {reason}"
    )]
    StartInputRefused {
        /// Workflow type selected for the start.
        workflow_type: String,
        /// Exact `.v4` package identity the start resolved to.
        version: ContentHash,
        /// What did not match, naming every field that failed.
        reason: String,
    },

    /// A signal was refused at the boundary: its name is not declared by the
    /// target run's package, or its payload did not satisfy the declared
    /// payload type.
    ///
    /// Returned BEFORE anything is recorded and before the arrival can be
    /// consumed, so the target run's history is unchanged and it stays parked
    /// on exactly the wait it was parked on. That ordering is the whole point:
    /// a signal decoded after being consumed destroys a durable run that a
    /// refusal merely inconveniences.
    #[error(
        "signal `{signal_name}` was refused for workflow `{workflow_id}` run `{run_id}` against the contract declared by package version `{version}`: {reason}"
    )]
    SignalRefused {
        /// Workflow execution the signal targeted.
        workflow_id: WorkflowId,
        /// Concrete run the signal targeted.
        run_id: RunId,
        /// Signal name the caller sent.
        signal_name: String,
        /// Exact `.v4` package identity the target run is pinned to.
        version: ContentHash,
        /// Why the signal was refused — an undeclared name, or the fields of
        /// the payload that did not match the declared type.
        reason: String,
    },

    /// The embedded runtime returned an error.
    #[error("runtime error: {reason}")]
    Runtime {
        /// Human-readable runtime failure reason.
        reason: String,
    },

    /// A Gate-3 BIF required for tracked local fun spawns was not registered.
    #[error("required Gate-3 BIF `{module}:{function}/{arity}` was missing during runtime startup")]
    Gate3BifReplacementMissing {
        /// Native module containing the required function.
        module: String,
        /// Required native function.
        function: String,
        /// Required native function arity.
        arity: u8,
    },

    /// The runtime-owned cleanup executor's ownership state was poisoned.
    #[error("process cleanup executor state was poisoned")]
    CleanupExecutorPoisoned,

    /// The runtime cleanup worker did not stop within the configured bound.
    #[error("process cleanup executor did not stop within {timeout_millis}ms")]
    CleanupExecutorShutdownTimedOut {
        /// Configured shutdown observation bound in milliseconds.
        timeout_millis: u128,
    },

    /// The process-exit registry lifecycle lock was poisoned.
    #[error("process exit registry lifecycle state was poisoned")]
    ProcessExitRegistryPoisoned,

    /// A process exit record's installation/abort ownership gate was poisoned.
    #[error("process exit ownership gate for process {process_id} was poisoned")]
    ProcessExitOwnershipPoisoned {
        /// Process whose monitor/abort ownership could not be serialized.
        process_id: u64,
    },

    /// A process exit record's fan-out state was poisoned.
    #[error("process exit outcome state for process {process_id} was poisoned")]
    ProcessExitStatePoisoned {
        /// Process whose cached exit state could not be accessed.
        process_id: u64,
    },

    /// The scheduler's one exit-event subscription was already claimed.
    #[error("beamr process exit-event subscription is already owned")]
    ProcessExitSubscriptionUnavailable,

    /// The singleton process-exit drainer could not be spawned.
    #[error("process exit drainer could not start: {reason}")]
    ProcessExitDrainerSpawn {
        /// Operating-system thread creation failure.
        reason: String,
    },

    /// The singleton process-exit drainer's ownership lock was poisoned.
    #[error("process exit drainer state was poisoned")]
    ProcessExitDrainerPoisoned,

    /// beamr published an exit event without the promised durable outcome.
    #[error("process {process_id} exit event had no takeable outcome")]
    ProcessExitOutcomeMissingAfterEvent {
        /// Process named by the contract-breaking event.
        process_id: u64,
    },

    /// beamr disconnected its event publisher while the runtime still owned it.
    #[error("beamr process exit-event publisher disconnected")]
    ProcessExitEventStreamDisconnected,

    /// The process-exit drainer did not stop within the configured bound.
    #[error("process exit drainer did not stop within {timeout_millis}ms")]
    ProcessExitDrainerShutdownTimedOut {
        /// Configured shutdown observation bound in milliseconds.
        timeout_millis: u128,
    },

    /// The process-exit drainer thread panicked.
    #[error("process exit drainer terminated unexpectedly")]
    ProcessExitDrainerPanicked,

    /// The process-exit callback dispatcher's ownership state was poisoned.
    #[error("process exit callback dispatcher state was poisoned")]
    ProcessExitCallbackDispatcherPoisoned,

    /// The process-exit callback dispatcher had already stopped.
    #[error("process exit callback dispatcher is unavailable")]
    ProcessExitCallbackDispatcherUnavailable,

    /// The process-exit callback dispatcher did not stop within its configured bound.
    #[error("process exit callback dispatcher did not stop within {timeout_millis}ms")]
    ProcessExitCallbackDispatcherShutdownTimedOut {
        /// Configured shutdown observation bound in milliseconds.
        timeout_millis: u128,
    },

    /// A retired process generation cannot accept another outcome consumer.
    #[error("process {process_id} already reached its terminal runtime outcome")]
    ProcessExitAlreadyTerminal {
        /// Process generation whose heavyweight exit record was retired.
        process_id: u64,
    },

    /// A workflow's activity-delivery synchronization lock was poisoned.
    #[error("activity delivery lock for process {process_id} was poisoned")]
    ActivityDeliveryPoisoned {
        /// Workflow process whose scoped delivery lock was poisoned.
        process_id: u64,
    },

    /// The active workflow registry lock was poisoned.
    #[error("active workflow registry lock was poisoned")]
    RegistryPoisoned,

    /// The workflow catalog lock was poisoned.
    #[error("workflow catalog lock was poisoned")]
    CatalogPoisoned,

    /// A precondition on the target workflow's current state was not met.
    ///
    /// Raised by the reopen operation when the target run is not in a reopenable
    /// state: not terminal, terminal for a non-reopenable reason
    /// (Completed/`TimedOut`), or already Running. The `reason` names the actual
    /// status so callers and operators can see why the reopen was rejected. Maps
    /// to the `INVALID_STATE` wire code (gRPC `FailedPrecondition` / HTTP 409).
    #[error("invalid workflow state: {reason}")]
    InvalidState {
        /// Human-readable precondition-failure reason naming the actual status.
        reason: String,
    },

    /// The engine is already shutting down and no new workflow starts are accepted.
    #[error("engine is shutting down")]
    ShuttingDown,

    /// No live, durable, or loaded workflow was found for the request.
    #[error("workflow `{workflow_type}` was not found")]
    WorkflowNotFound {
        /// Logical workflow type requested by the caller.
        workflow_type: String,
    },

    /// A terminal-writer reservation could not be taken because the workflow
    /// already has a writer (#117(c)).
    ///
    /// The extraordinary cancellation path exists only for a run that can never
    /// obtain a handle. A workflow that has one — or that another reservation is
    /// already writing — is not that case, and taking a second writer would
    /// break the single-writer invariant this refusal protects.
    #[error("workflow `{workflow_id}` run `{run_id}` cannot take the terminal writer: {holder}")]
    TerminalWriterUnavailable {
        /// Workflow whose writer slot is occupied.
        workflow_id: String,
        /// Run the refused reservation named.
        run_id: String,
        /// What holds the slot, in the operator's terms.
        holder: String,
    },

    /// A handle could not be registered because a terminal-writer reservation
    /// holds this workflow's writer slot (#117(c)).
    ///
    /// The mirror of [`Self::TerminalWriterUnavailable`], and transient by
    /// construction: a reservation lives only across one terminal transition.
    #[error(
        "workflow `{workflow_id}` cannot register a handle: run `{run_id}` holds the terminal writer"
    )]
    TerminalWriterHeld {
        /// Workflow whose writer slot is reserved.
        workflow_id: String,
        /// Run holding the reservation.
        run_id: String,
    },

    /// The extraordinary cancellation path was asked for a run whose pinned
    /// package resolves right now, so the run is recoverable (#117(c)).
    ///
    /// Measured at the moment of the request, never cited from an earlier boot's
    /// verdict: a redeploy between then and now is exactly the remedy that makes
    /// the ordinary path work again, and the ordinary path must be used when it
    /// does.
    #[error(
        "workflow `{workflow_id}` run `{run_id}` is recoverable: its pinned package version `{version}` resolves, so it must be recovered and cancelled through the ordinary path"
    )]
    RunIsRecoverable {
        /// Workflow the request named.
        workflow_id: String,
        /// Run the request named.
        run_id: String,
        /// The pinned package version that resolved.
        version: String,
    },

    /// A run holds no handle, cannot obtain one, and this engine has no recorded
    /// reason why (#117(c)).
    ///
    /// Distinct from [`Self::WorkflowNotFound`] on purpose: the run EXISTS and
    /// its history is readable. What is absent is a verdict from this process's
    /// startup recovery, so the extraordinary cancellation path — which must
    /// cite that verdict — has nothing to cite.
    #[error(
        "workflow `{workflow_id}` run `{run_id}` is not resident and this engine recorded no reason it could not be made resident; it exists but cannot be cancelled here"
    )]
    NoResidencyVerdict {
        /// Workflow the request named.
        workflow_id: String,
        /// Run the request named.
        run_id: String,
    },

    /// No durable schedule was found for the request.
    #[error("schedule `{schedule_id}` was not found")]
    ScheduleNotFound {
        /// Schedule identifier requested by the caller.
        schedule_id: ScheduleId,
    },

    /// Schedule trigger, projection, or evaluator side effect failed.
    #[error("schedule error: {reason}")]
    Schedule {
        /// Human-readable schedule failure reason.
        reason: String,
    },

    /// Native implemented function registration failed.
    #[error("NIF registration failed: {reason}")]
    NifRegistration {
        /// Human-readable native implemented function registration failure reason.
        reason: String,
    },

    /// Signal routing failed after the target was resolved.
    #[error("signal router error: {0}")]
    SignalRouter(#[from] SignalRouterError),

    /// Live workflow query dispatch failed after the target was resolved.
    #[error("query error: {0}")]
    Query(#[from] crate::query::QueryError),
}

/// What pins a workflow version against unload, naming the concrete holder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinHolder {
    /// A start resolved this version but has not yet registered a handle.
    InFlightStart,
    /// A live, non-terminal run executes on this version.
    LiveRun {
        /// Pinning workflow id.
        workflow_id: WorkflowId,
        /// Pinning run id.
        run_id: RunId,
    },
    /// A recoverable instance in the store is pinned to this version.
    RecoverableRun {
        /// Pinning workflow id.
        workflow_id: WorkflowId,
    },
    /// A recorded-but-never-started child is pinned to this version.
    RecordedChild {
        /// Child workflow id pinned to the version.
        child_workflow_id: WorkflowId,
        /// Parent workflow whose history records the child.
        recorded_by: WorkflowId,
    },
}

impl std::fmt::Display for PinHolder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InFlightStart => formatter.write_str("an in-flight start is pinned to it"),
            Self::LiveRun {
                workflow_id,
                run_id,
            } => write!(
                formatter,
                "live run `{workflow_id}/{run_id}` is pinned to it"
            ),
            Self::RecoverableRun { workflow_id } => {
                write!(formatter, "recoverable run `{workflow_id}` is pinned to it")
            }
            Self::RecordedChild {
                child_workflow_id,
                recorded_by,
            } => write!(
                formatter,
                "child `{child_workflow_id}` recorded by `{recorded_by}` is pinned to it and has not started"
            ),
        }
    }
}

/// Errors surfaced by the signal routing boundary.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum SignalRouterError {
    /// The target workflow is terminal and cannot receive new signals.
    #[error("workflow {workflow_id}/{run_id} is terminal")]
    Terminal {
        /// Target workflow id.
        workflow_id: WorkflowId,
        /// Target run id.
        run_id: RunId,
    },

    /// The router could not defer a recorded non-resident signal.
    #[error("signal resume handoff failed: {reason}")]
    Handoff {
        /// Human-readable handoff failure reason.
        reason: String,
    },

    /// The signal was durably recorded but could not be delivered to the live mailbox.
    #[error(
        "signal `{signal_name}` for workflow {workflow_id}/{run_id} could not be delivered to process {process_id}: {reason}"
    )]
    DeliveryFailed {
        /// Target workflow id.
        workflow_id: WorkflowId,
        /// Target run id.
        run_id: RunId,
        /// Embedded runtime process identifier selected for delivery.
        process_id: u64,
        /// Signal name that was recorded and attempted.
        signal_name: String,
        /// Human-readable delivery failure reason.
        reason: String,
    },
}

impl From<ScheduleError> for EngineError {
    fn from(error: ScheduleError) -> Self {
        Self::Schedule {
            reason: error.to_string(),
        }
    }
}

impl From<ScheduleEvaluatorError> for EngineError {
    fn from(error: ScheduleEvaluatorError) -> Self {
        match error {
            ScheduleEvaluatorError::ScheduleNotFound { schedule_id } => {
                Self::ScheduleNotFound { schedule_id }
            }
            other => Self::Schedule {
                reason: other.to_string(),
            },
        }
    }
}
