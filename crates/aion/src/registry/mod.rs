//! Workflow registry handles and tables.

/// Workflow handle residency, completion, and mailbox state.
pub mod handle;
/// In-memory workflow registry table.
pub mod table;
/// Runs startup recovery could not make resident, retained so the fact can be
/// queried rather than only witnessed in a boot log (#117).
pub mod unrecoverable;

pub use handle::{
    CompletionNotifier, HandleResidency, Residency, TerminalOutcome, WorkflowHandle,
    WorkflowHandleParts,
};
pub use table::Registry;
pub use unrecoverable::{UnrecoverableRun, UnrecoverableRuns};
