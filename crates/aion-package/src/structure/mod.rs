//! Workflow structure projection: a graph model derived from the typed source.
//!
//! A workflow is a typed function over a small, known vocabulary
//! (`run` / `all` / `race` / `map` / `spawn` / `receive` / `sleep` / timers), so
//! its primitive structure can be projected from the entry-module Gleam source
//! into an ordered node/edge graph automatically (C21, C23). Each node carries a
//! [`CorrelationKey`] so a consumer (the dashboard canvas, RM-007) can overlay a
//! run's recorded events onto the graph (C22). A bounded structural delta
//! regenerates Gleam that still type-checks (C24).
//!
//! The graph model is a projection, never the authoritative artifact: the typed
//! Gleam module remains the single source of truth (ADR-014, CN6). This module
//! delivers the data model and the regeneration only — the rendered canvas UI
//! and the live overlay are deferred to RM-007.

mod arms;
mod control_flow;
mod determinism;
mod error;
mod extract;
mod facts;
mod ident;
mod model;
mod reader;
mod regen;
mod scan;

#[cfg(test)]
mod tests;

/// The crate's ONE runtime gate for tests that shell out to the real `gleam`
/// CLI, shared with the integration tests rather than re-implemented here.
///
/// The unit test below is inside `src/`, so it cannot reach a `tests/` module
/// the ordinary way. Its own gate used `tracing::info!`, which with no
/// subscriber installed wrote the skip line nowhere at all (#74). Reaching the
/// shared file by path keeps one definition of "the skip must be visible"
/// instead of a third copy that looks right in isolation.
#[cfg(test)]
#[path = "../../tests/test_support/gleam.rs"]
mod gleam_test_support;

pub use determinism::{DeterminismError, Violation, ViolationKind, analyze_determinism};
pub use error::StructureError;
pub use extract::extract_structure;
pub use facts::{FactsError, WorkflowFacts, extract_workflow_facts};
pub use model::{
    ArmLabel, CorrelationKey, EdgeKind, GraphEdge, GraphNode, NodeId, NodePrimitive, WorkflowGraph,
};
pub use regen::{StructuralDelta, regenerate_gleam};
