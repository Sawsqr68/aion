//! Engine API, builder, and delegated seam surfaces.

/// Which deployed package versions bind a registering worker.
pub mod admission;
/// High-level workflow engine API.
pub mod api;
/// Engine durable-schedule API surface and coordinator assembly.
mod api_schedule;
/// Engine construction and package source configuration.
pub mod builder;
/// Delegated signal, query, and event-publishing seams.
pub mod delegated;
/// Double-adoption fence-ordering planner (ADR-021 clean-partial).
mod fence;
/// The shared `gleam` test gate, reused from this crate's own `tests/` copy
/// rather than re-implemented (#125).
///
/// It is declared HERE, not beside its only consumer in `builder.rs`, for a
/// mechanical reason: `#[path]` resolves against the declaring module's
/// directory, and `builder.rs` is not a `mod.rs`, so its children resolve under
/// a `src/engine/builder/` that does not exist — the `..` traversal fails at the
/// OS before rustc ever reads the target. `src/engine/` is a real directory, so
/// the path resolves from here. Same technique as
/// `aion-package/src/structure/mod.rs`.
#[cfg(test)]
#[path = "../../tests/test_support/gleam.rs"]
mod gleam_test_support;
/// Runtime package-load seam: live load, routing, listing, unload.
mod reload;
/// `EngineHandle` seam implementation for the engine.
mod seam_handle;
/// Seam-assembly helpers used by `EngineBuilder::build()`.
mod seams;
/// Shutdown gating for in-flight lifecycle operations.
mod shutdown_gate;
/// Startup recovery wiring used by `EngineBuilder::build()`.
pub(crate) mod startup;
/// Crash-window repair sweeps run by startup recovery.
mod startup_sweeps;

pub use admission::{
    AdmissionReason, QueueAdmission, ReasonCensus, RequiredContract, UnreachableContract,
};
pub use api::Engine;
pub use api_schedule::schedule_coordinator_workflow_id;
pub use builder::EngineBuilder;
pub use delegated::{
    DeferredEventPublisher, DeferredQueryService, DeferredSignalRouter, DelegatedSeams,
    EventFamily, EventFilter, EventPublisher, EventStreamLagged, QueryService, SignalRouter,
};
