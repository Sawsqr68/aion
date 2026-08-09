//! Module declarations.

/// Run-to-completion contained command with output capture and cancellation.
pub mod cancellable;
/// The process-group containment core: spawn, observe, terminate, verify.
pub mod contained;

pub use cancellable::{
    CancellableCommandOutput, CommandOutputObserver, CommandStream, run_cancellable_command,
};
pub use contained::{ContainedChild, PROCESS_GROUP_TERMINATION_GRACE, ProcessGroupError};
