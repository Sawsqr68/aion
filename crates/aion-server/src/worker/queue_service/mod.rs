//! Module declarations.

/// Live poller census for a pool address and the taxonomy classification it feeds.
pub mod census;
/// Whether a task queue is declared by any deployed `.v4` contract.
pub mod declarations;
/// Schedule-to-start hand-off to a live worker (where `SATURATED` is emitted).
pub mod delivery;
/// Service policies (`strict` / `durable_pending`) and the two service clocks.
pub mod policy;
/// Whether an in-flight activity can still reach a worker.
pub mod reachability;
/// The live, queryable unserved-queue state.
pub mod state;
/// The four-way taxonomy and the typed `WorkerUnavailable` dispatch failure.
pub mod taxonomy;
/// The classification-driven selection wait.
pub mod wait;

pub use census::{PoolCensus, classify};
pub use declarations::{
    EngineQueueDeclarations, QueueDeclaration, QueueDeclarationSource, QueueDeclarations,
};
pub use delivery::{DeliveryRefusal, PARK_POLL_INTERVAL, deliver_within_schedule_to_start};
pub use policy::{QueueServiceConfig, QueueServiceOverride, QueueServicePolicy};
pub use reachability::{ActivityReachability, OpenActivity, open_activities_in_active_segment};
pub use state::{Parked, QueueServiceState, UnservedDispatch, UnservedKey, UnservedQueue};
pub use taxonomy::{
    ExpiredClock, QueueServiceReason, ServiceAddress, UnavailableSummary, WorkerUnavailable,
};
pub use wait::{SelectionRefusal, ServiceWait, select_worker_or_refuse};
