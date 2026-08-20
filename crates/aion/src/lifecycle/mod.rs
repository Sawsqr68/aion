//! Workflow lifecycle operations: start, terminate, suspend, and resume.

/// Cancelling a run that was never made resident and never can be (#117(c)).
pub mod cancel_never_alive;
/// Workflow completion notification helpers.
pub mod completion;
/// Retrying one run's completion until its terminal lands durably.
mod completion_retry;
/// Continue-as-new lifecycle transition support.
pub mod continue_as_new;
/// Engine-side workflow-deadline timeout handler.
pub mod deadline;
/// Durable operator pause/resume (#204): hold new activity dispatch while paused.
pub mod pause;
/// Durable operator rename (#211): record a display name.
pub mod rename;
/// Reopen a terminal-Failed or terminal-Cancelled workflow and re-drive it.
pub mod reopen;
/// Workflow start request and initialization helpers.
pub mod start;
/// A runtime-armed hold between the completion doorbell and terminal registry
/// reconciliation, used to pin issue #51 deterministically.
///
/// Crate-visible, not module-private: the window has more than one consumer, and
/// the unload pin check (`engine::reload`) needs the same deterministic hold that
/// `reopen` does — see aion#94, which is that window reached through the unload
/// caller. Widening stops at the crate, and `arm` remains `#[cfg(test)]`, so the
/// property this module's doc rests on is intact: no production caller can arm a
/// gate even by mistake.
pub(crate) mod terminal_reconcile_gate;
/// Terminal workflow transitions: complete, fail, and cancel.
pub mod terminate;
/// Runtime suspend and resume transition helpers.
pub mod transition;
/// Visibility projection helpers for lifecycle events.
pub mod visibility;

pub use continue_as_new::{ContinueAsNewContext, ContinueAsNewRequest, continue_as_new};
pub use pause::{PauseWorkflowContext, PausedRuns};
pub use rename::RenameWorkflowContext;
pub use reopen::{ReopenWorkflowContext, reopen};
pub use terminate::{TerminateWorkflowContext, cancel, complete, fail};
pub use transition::{resume, suspend};
