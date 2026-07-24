//! Graph projection of a parsed AWL document — the canvas's 1:1 picture.
//!
//! Workflow control uses one node per step. Each step also carries a nested,
//! positional statement graph for calls, waits, forks, loops, and pipes. All
//! construct kinds come from the checker's semantic index (never re-derived
//! here); this module only adds wire labels and edge geometry.

mod build;
mod edges;
mod statement_build;
mod statement_types;
mod types;

#[cfg(test)]
mod tests;

pub use build::build;
pub use types::{GraphProjection, ProjectionEdgeKind};
