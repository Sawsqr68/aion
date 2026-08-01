//! Dependency-parallel layer classification: the MIR twin of the reference
//! `emitter/steps.rs::layer_calls` and the walk `emit_layers` performs over a
//! region's topological layers.
//!
//! A region is a dependency-connected group of steps laid out in topological
//! layers (`emitter/graph.rs::layer`). A layer with more than one member is a
//! DEPENDENCY-PARALLEL layer: its members have no dependency on one another
//! and the reference dispatches them concurrently — as one `workflow.all` —
//! when every member is exactly one call of a declared, non-advisory action
//! with no outcomes, no `on failure` edge, and no `max … visits` bound.
//! Anything richer degrades to written order, which the reference records in
//! the generated module as a comment and the MIR records as the S13
//! `degraded_parallel` marker.
//!
//! The direct path splits a region into one function per CHAIN UNIT rather
//! than inlining the whole region into one function the way the Gleam
//! reference does (IR-14). A [`ChainUnit`] is therefore either one step or a
//! whole parallelizable layer — a parallelizable layer must be ONE function
//! because its members share a single `workflow.all` dispatch, while a
//! degraded layer's members are ordinary chain steps that tail-call one
//! another in written order.
//!
//! Slot arithmetic follows from that: `plan_slots::chain_slots` reserves one
//! function slot per unit. Every layer of a region without a
//! dependency-parallel layer holds exactly one member, so the unit count
//! equals the layer count and every pre-existing module keeps byte-identical
//! function references.

use crate::ast::{CallStmt, Statement, Step};
use crate::emitter::Emitter;

use super::activity::declared_advisory;

/// One chain unit of a region: the steps that lower into a single function.
pub(super) struct ChainUnit {
    /// The steps this unit covers, in written order — one step, or every
    /// member of a parallelizable layer. Never empty.
    pub(super) members: Vec<usize>,
    /// Whether the members dispatch concurrently through one `workflow.all`.
    pub(super) parallel: bool,
    /// S13: this unit is the FIRST member of a dependency-parallel layer that
    /// could NOT dispatch concurrently, so the layer runs in written order.
    /// The reference names the same degradation in the generated Gleam
    /// (`emit_layers`'s `awl stopgap` comment).
    pub(super) degraded: bool,
}

impl ChainUnit {
    /// The step whose name, span, and origin anchor this unit's function: the
    /// unit's first member (a parallel layer's function is named for the first
    /// step it dispatches).
    pub(super) fn anchor(&self) -> usize {
        // `members` is never empty by construction; the fallback keeps this
        // total rather than panicking on a future construction bug.
        self.members.first().copied().unwrap_or_default()
    }
}

/// Split one region's topological layers into chain units, in execution order.
pub(super) fn chain_units(
    steps: &[Step],
    layers: &[Vec<usize>],
    emitter: &Emitter<'_>,
) -> Vec<ChainUnit> {
    let mut units = Vec::new();
    for layer in layers {
        if layer.len() > 1 && parallel_layer_calls(steps, layer, emitter).is_some() {
            units.push(ChainUnit {
                members: layer.clone(),
                parallel: true,
                degraded: false,
            });
            continue;
        }
        // Written order, one unit per member. A multi-member layer that
        // reaches here is the reference's recorded degradation, marked on its
        // first member exactly where the reference emits its comment.
        let degrades = layer.len() > 1;
        for (position, &member) in layer.iter().enumerate() {
            units.push(ChainUnit {
                members: vec![member],
                parallel: false,
                degraded: degrades && position == 0,
            });
        }
    }
    units
}

/// The single bare action call of every member of a multi-step layer, when the
/// layer is parallelizable — the reference `emitter/steps.rs::layer_calls`,
/// gate for gate:
///
/// - a member carrying outcome clauses, an `on failure` edge, or a
///   `max … visits` bound is more than a dispatch and cannot join the
///   `workflow.all`;
/// - a member whose body is anything but exactly one call statement likewise
///   (the SDK parallelizes activities, not statement sequences);
/// - a call that names a child workflow rather than a declared action is not
///   an activity dispatch;
/// - R5: an advisory call is NOT a layer-completion dependency. Its result is
///   never consumed and its failure never faults the step, so it must not join
///   a `workflow.all` whose whole purpose is to gate the layer on every
///   member's outcome — the layer falls back to written order and the advisory
///   call keeps its around-the-fault-arm form.
pub(super) fn parallel_layer_calls<'s>(
    steps: &'s [Step],
    members: &[usize],
    emitter: &Emitter<'_>,
) -> Option<Vec<&'s CallStmt>> {
    let mut calls = Vec::with_capacity(members.len());
    for &member in members {
        let step = steps.get(member)?;
        if !step.outcomes.is_empty() || step.on_failure.is_some() || step.max_visits.is_some() {
            return None;
        }
        let [Statement::Call(call)] = step.body.as_slice() else {
            return None;
        };
        if !emitter.actions.contains_key(call.call.name.as_str()) {
            return None;
        }
        if declared_advisory(emitter, call.call.name.as_str()) {
            return None;
        }
        calls.push(call);
    }
    Some(calls)
}

/// Whether every call of a parallelizable layer names the SAME action — the
/// reference's homogeneity test (`emit_parallel_layer`). A homogeneous layer
/// rides the typed `workflow.all`; a heterogeneous one rides the raw
/// wire-unified twins and decodes each bound position with that action's own
/// return codec.
pub(super) fn homogeneous(calls: &[&CallStmt]) -> bool {
    let Some(first) = calls.first() else {
        return true;
    };
    calls.iter().all(|call| call.call.name == first.call.name)
}
