//! Workflow signal routing and resume handoff support.

/// Declared-signal admission at the engine's signal boundary.
pub(crate) mod admission;
/// Signal resume handoff values and errors.
pub mod resume;
/// Concrete router for delivering signals to workflow mailboxes.
pub mod router;

pub use resume::{SignalResumeError, SignalResumeHandoff};
pub use router::ConcreteSignalRouter;
