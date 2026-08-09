//! Module declarations.

/// The task model: identity, status, and the detailed status variants.
pub mod model;
/// Task execution: the background driver and cooperative cancellation.
pub mod runner;
/// The task store: creation, lookup, terminal transitions, and expiry.
pub mod store;
