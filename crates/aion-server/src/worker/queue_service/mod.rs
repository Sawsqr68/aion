//! Module declarations.

/// Live poller census for a pool address and the taxonomy classification it feeds.
pub mod census;
/// Whether a task queue is declared by any deployed `.v4` contract.
pub mod declarations;
/// Schedule-to-start hand-off to a live worker (where `SATURATED` is emitted).
pub mod delivery;
/// The four-way taxonomy and the typed `WorkerUnavailable` dispatch failure.
pub mod taxonomy;

pub use census::{PoolCensus, classify};
pub use declarations::{
    EngineQueueDeclarations, QueueDeclaration, QueueDeclarationSource, QueueDeclarations,
};
pub use delivery::{DeliveryRefusal, PARK_POLL_INTERVAL, deliver_within_schedule_to_start};
pub use taxonomy::{
    ExpiredClock, QueueServiceReason, ServiceAddress, UnavailableSummary, WorkerUnavailable,
};
