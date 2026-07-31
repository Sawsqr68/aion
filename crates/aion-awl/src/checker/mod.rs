//! Typechecker for the rev-2 surface plus the rev-3 flow shape: call
//! contracts, binding flow along the graph, graph shape (targets,
//! reachability, `max … visits`-bounded cycles), per-item regions
//! (`distribute`/`sequence` … `collect`), subflows, outcome
//! exhaustiveness, guard-dependent optionality, and schema projection.

mod agents;
mod anchor;
mod args;
mod avail;
mod blocks;
mod bodies;
mod calls;
mod collections;
mod consts;
mod context;
mod cycles;
mod decls;
mod durations;
mod entry;
mod error;
mod exhaustive;
mod exprs;
mod graph;
mod operators;
mod outcomes;
mod project;
mod queries;
mod regions;
mod sends;
mod stages;
mod statement_info;
mod subflows;
mod types;
mod walk;

pub(crate) use entry::analyze;
pub use entry::{check, check_in};
pub use error::CheckError;
