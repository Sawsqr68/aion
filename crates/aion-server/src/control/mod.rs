//! Server control from the operator's desk: the pid file the server writes,
//! the incarnation checks that make it trustworthy, the shutdown outcome
//! record that crosses the process boundary, and the stop flow built on all
//! three. `aion server stop` / `aion server status` render these; the logic
//! and its types live here so the server crate owns one truth about its own
//! lifecycle records.

pub mod incarnation;
pub mod outcome;
pub mod pid_file;
pub mod stop;

pub use incarnation::{IncarnationProbe, SelfIdentity};
pub use outcome::{NoteFate, OutcomeRecord, ParkedWorkerRecord};
pub use pid_file::{PidFileGuard, PidRecord, StaleReconciliation};
pub use stop::{StopOutcome, StopRefusal, StopVerdict};
