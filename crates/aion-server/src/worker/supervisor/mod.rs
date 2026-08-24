//! Module declarations.

/// Typed managed-worker supervision failures, each carrying its remedy.
pub mod error;
/// Which executable a builtin deployment launches, and what it is at spawn.
pub mod executable;
/// The supervisor itself: desired state in, running processes out.
pub mod fleet;
/// One supervised instance: spawn, observe, restart, stop.
mod instance;
/// The operator-configured restart discipline. No invented numbers.
pub mod policy;
/// What the server says about a managed worker: desired joined with actual.
pub mod status;

#[cfg(test)]
#[path = "fleet_tests.rs"]
mod tests;

pub use error::SupervisionError;
pub use executable::ManagedExecutable;
pub use fleet::{FleetShutdownReport, WorkerSupervisor};
pub use policy::{SupervisionPolicy, UNCOMMISSIONED_REMEDY};
pub use status::{
    ManagedWorkerExit, ManagedWorkerReport, ManagedWorkerState, ManagedWorkerStatus, SpawnedBinary,
};
