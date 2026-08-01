//! The flow-function slot inventory (split from `build` for the 500-line
//! law), extended by the B4/B5 parity landing to every nested flow.
//!
//! Canonical slot order (byte-stability contract: a module without rev-3
//! constructs has no nested flows, no `distribute` statements, and no child
//! adapters, so every pre-B4 `FnRef` is preserved exactly):
//!
//! 1. host region chains (one slot per CHAIN UNIT — `layers::chain_units`).
//!    A region without a dependency-parallel layer holds exactly one member
//!    per layer, so the unit count equals the layer count and every
//!    pre-parallel module keeps byte-identical function references. Then one
//!    slot per SUBSTEP of the same flow (steps in written order, substeps in
//!    block order — the reference `emitter/steps.rs::emit_flow_fns` emits
//!    every region function before any substep chain). A flow with no
//!    substeps reserves none;
//! 2. nested flow function sets — subflows in DECLARATION order first (the
//!    reference `emitter/flows.rs::emit_nested` order), then per-item regions
//!    by ascending region id; each set is the run-once entry wrapper slot
//!    followed by that flow's region-chain slots. A subflow's own collapsed
//!    regions are per-item regions like any other and take their slots in the
//!    region pass, not inside the subflow's set;
//! 3. loop slots — host steps first (plan-region order, layers flattened,
//!    `loops::count_loops` per body), then each nested flow's steps in the
//!    same nested-flow order;
//! 4. fork slots — the same extended traversal, now counting BOTH `fork`
//!    statements (`forks::count_fork_fns`) and the fan-out lifted closures
//!    of a collapsed region step (`fanout::count_fanout_fns` — the
//!    `distribute`/`sequence` marker heads the synthetic body, so its
//!    closures precede any later fork's in the same step);
//! 5. wait slots — the same extended traversal;
//! 6. compensation slots — one per step carrying an `on failure` block, the
//!    same extended traversal (a module with no `on failure` reserves none,
//!    so every pre-edge `FnRef` is preserved exactly);
//! 7. implicit-child adapter shells (`_execute` then `_run`), ascending
//!    region id, one pair per region that runs as a synthesized child;
//! 8. the fixed helpers (T-DEAD, T-WIT) and dynamic predicates, as always.

use std::collections::BTreeMap;

use crate::ast::Step;
use crate::emitter::{Emitter, Plan, RegionShape, implicit_child_required};

use super::super::ids::FnRef;
use super::build::{AdapterFns, FlowFns, NestedFns};
use super::ctx::Ctx;
use super::driver::LowerError;

/// The reserved flow-function slots: per-flow chain tables, the shared
/// loop/fork/wait pools, and the implicit-child adapter pairs.
pub(super) struct FlowSlots {
    pub(super) host: FlowFns,
    pub(super) subflow_fns: BTreeMap<String, NestedFns>,
    pub(super) region_fns: BTreeMap<usize, NestedFns>,
    pub(super) loops: Vec<FnRef>,
    pub(super) forks: Vec<FnRef>,
    pub(super) waits: Vec<FnRef>,
    pub(super) compensations: Vec<FnRef>,
    pub(super) adapters: BTreeMap<usize, AdapterFns>,
    pub(super) child_witness_needed: bool,
}

/// One flow's traversal surface, in the canonical order (host, then regions
/// by ascending id).
pub(super) struct FlowWalk<'a> {
    pub(super) steps: &'a [Step],
    regions: &'a BTreeMap<String, RegionShape>,
    pub(super) plan: &'a Plan,
}

/// Every flow in the canonical nested-flow order.
pub(super) fn flows_in_order<'c>(ctx: &Ctx<'c>) -> Result<Vec<FlowWalk<'c>>, LowerError> {
    let emitter = ctx.emitter;
    let plans = ctx.plans;
    let mut flows = vec![FlowWalk {
        steps: &emitter.document.steps,
        regions: emitter.host_regions,
        plan: &plans.host,
    }];
    for shape in emitter.subflow_shapes {
        let nested = plans
            .subflows
            .get(&shape.name)
            .ok_or_else(|| unplanned_subflow(&shape.name))?;
        flows.push(FlowWalk {
            steps: &shape.flow.steps,
            regions: &shape.flow.regions,
            plan: &nested.plan,
        });
    }
    for (&id, nested) in &plans.regions {
        let shape = plans
            .region_shapes
            .get(&id)
            .ok_or_else(|| LowerError::Planning {
                message: format!("region {id} lost its shape"),
            })?;
        flows.push(FlowWalk {
            steps: &shape.members.steps,
            regions: &shape.members.regions,
            plan: &nested.plan,
        });
    }
    Ok(flows)
}

/// One flow's region-chain slots (one per chain unit, min one), then one slot
/// per substep of every step's trailing block — steps in written order,
/// substeps in block order, which is exactly the order `subs::lower_sub_chains`
/// appends their functions in and the order the reference emits them
/// (`emitter/steps.rs::emit_flow_fns`).
fn chain_slots(
    walk: &FlowWalk<'_>,
    emitter: &Emitter<'_>,
    next: &mut u32,
) -> Result<FlowFns, LowerError> {
    let mut regions = Vec::new();
    let mut chains = Vec::new();
    for region in &walk.plan.regions {
        regions.push(FnRef(*next));
        let slots = super::layers::chain_units(walk.steps, &region.layers, emitter)
            .len()
            .max(1);
        let mut chain = Vec::with_capacity(slots);
        for _ in 0..slots {
            chain.push(FnRef(*next));
            *next += 1;
        }
        chains.push(chain);
    }
    let mut subs = BTreeMap::new();
    for (index, step) in walk.steps.iter().enumerate() {
        for position in 0..super::subs::substeps_of(step)?.len() {
            subs.insert((index, position), FnRef(*next));
            *next += 1;
        }
    }
    Ok(FlowFns {
        regions,
        chains,
        subs,
    })
}

/// Every action a HETEROGENEOUS dependency-parallel layer dispatches, over
/// every flow — these need the raw wrapper twin planned exactly as a
/// heterogeneous named fork's actions do (`build::raw_activity_shell`).
///
/// The named-fork inventory (`forks::raw_action_inventory`) cannot see these:
/// a parallel layer is a property of the region PLAN, not of any statement, so
/// it is derived here where the per-flow plans are already in hand.
pub(super) fn parallel_raw_actions(ctx: &Ctx<'_>) -> Result<Vec<String>, LowerError> {
    let mut out = std::collections::BTreeSet::new();
    for walk in flows_in_order(ctx)? {
        for region in &walk.plan.regions {
            for layer in &region.layers {
                if layer.len() < 2 {
                    continue;
                }
                let Some(calls) =
                    super::layers::parallel_layer_calls(walk.steps, layer, ctx.emitter)
                else {
                    continue;
                };
                if super::layers::homogeneous(&calls) {
                    continue;
                }
                out.extend(calls.iter().map(|call| call.call.name.clone()));
            }
        }
    }
    Ok(out.into_iter().collect())
}

/// Reserve every flow-function slot in the canonical order documented in the
/// module header.
pub(super) fn plan_flow_slots(ctx: &Ctx<'_>, next: &mut u32) -> Result<FlowSlots, LowerError> {
    let emitter = ctx.emitter;
    let plans = ctx.plans;
    // The chain-slot walks in the SAME canonical order the traversal pools
    // below use, so a unit count is never read against another flow's steps.
    let flows = flows_in_order(ctx)?;
    let mut walks = flows.iter();
    let host = chain_slots(next_walk(&mut walks, "the host flow")?, emitter, next)?;
    let mut subflow_fns = BTreeMap::new();
    for shape in emitter.subflow_shapes {
        if !plans.subflows.contains_key(&shape.name) {
            return Err(unplanned_subflow(&shape.name));
        }
        let wrapper = FnRef(*next);
        *next += 1;
        let walk = next_walk(&mut walks, &format!("subflow `{}`", shape.name))?;
        let fns = chain_slots(walk, emitter, next)?;
        subflow_fns.insert(shape.name.clone(), NestedFns { wrapper, fns });
    }
    let mut region_fns = BTreeMap::new();
    for &id in plans.regions.keys() {
        let wrapper = FnRef(*next);
        *next += 1;
        let walk = next_walk(&mut walks, &format!("region {id}"))?;
        let fns = chain_slots(walk, emitter, next)?;
        region_fns.insert(id, NestedFns { wrapper, fns });
    }
    if walks.next().is_some() {
        return Err(LowerError::Planning {
            message: "the flow walk outran the chain-slot plan".to_owned(),
        });
    }

    let mut loops = Vec::new();
    for flow in &flows {
        for_each_body(flow, |step| {
            for _ in 0..super::loops::count_loops(&step.body) {
                loops.push(FnRef(*next));
                *next += 1;
            }
        })?;
    }
    let mut forks = Vec::new();
    let mut child_witness_needed = false;
    for flow in &flows {
        for_each_body(flow, |step| {
            child_witness_needed |= super::forks::needs_child_witness(&step.body, emitter)
                || super::fanout::needs_child_witness(step, flow.regions, emitter);
            let count = super::fanout::count_fanout_fns(step, flow.regions, emitter)
                + super::forks::count_fork_fns(&step.body, emitter);
            for _ in 0..count {
                forks.push(FnRef(*next));
                *next += 1;
            }
        })?;
    }
    let mut waits = Vec::new();
    for flow in &flows {
        for_each_body(flow, |step| {
            for _ in 0..super::wait::count_wait_fns(&step.body) {
                waits.push(FnRef(*next));
                *next += 1;
            }
        })?;
    }
    let mut compensations = Vec::new();
    for flow in &flows {
        for_each_body(flow, |step| {
            if step.on_failure.is_some() {
                compensations.push(FnRef(*next));
                *next += 1;
            }
        })?;
    }
    let mut adapters = BTreeMap::new();
    for &id in plans.regions.keys() {
        let Some(&shape) = plans.region_shapes.get(&id) else {
            continue;
        };
        if !implicit_child_required(emitter, shape) {
            continue;
        }
        let execute = FnRef(*next);
        let run = FnRef(*next + 1);
        *next += 2;
        adapters.insert(id, AdapterFns { execute, run });
    }
    Ok(FlowSlots {
        host,
        subflow_fns,
        region_fns,
        loops,
        forks,
        waits,
        compensations,
        adapters,
        child_witness_needed,
    })
}

/// The next flow in the canonical walk, or a planning error — the chain-slot
/// pass and the traversal pools must consume the SAME sequence, so a
/// mismatched length is a bug in one of them, never a document error.
fn next_walk<'w, 'c>(
    walks: &mut std::slice::Iter<'w, FlowWalk<'c>>,
    what: &str,
) -> Result<&'w FlowWalk<'c>, LowerError> {
    walks.next().ok_or_else(|| LowerError::Planning {
        message: format!("{what} has no planned flow walk"),
    })
}

/// A subflow the shared planning pass never produced a plan for — a planning
/// bug, never a document error (`plan_all` plans every shaped subflow).
fn unplanned_subflow(name: &str) -> LowerError {
    LowerError::Planning {
        message: format!("subflow `{name}` was never planned"),
    }
}

/// Walk every body of one flow in LOWERING order — the order the reserved-slot
/// pools must be consumed in: first the chain steps (plan regions in order,
/// layers flattened, written order within a layer), then every substep of every
/// trailing block (steps in written order, substeps in block order), which is
/// when `flow::lower_flow_fns` runs `subs::lower_sub_chains`.
///
/// A parent step's own count covers only its LEADING statements: the counting
/// walks (`loops::count_loops` and its siblings) do not descend into a
/// `SubStep`, so a substep's own loops, forks, waits, and `on failure` edge are
/// counted here, once, at the position their functions are built.
fn for_each_body(flow: &FlowWalk<'_>, mut f: impl FnMut(&Step)) -> Result<(), LowerError> {
    for region in &flow.plan.regions {
        for &step_index in region.layers.iter().flatten() {
            if let Some(step) = flow.steps.get(step_index) {
                f(step);
            }
        }
    }
    for step in flow.steps {
        for sub in super::subs::substeps_of(step)? {
            f(sub);
        }
    }
    Ok(())
}

/// T-DEAD precedes T-WIT; the dynamic lifted-function pool (predicate
/// closures, query-answer functions) follows both. This preserves every
/// existing function reference when a module uses none of them.
pub(super) fn fixed_helper_refs(
    next: u32,
    has_activity: bool,
    child_witness_needed: bool,
) -> (Option<FnRef>, FnRef) {
    let dead_offset = u32::from(has_activity);
    let child_witness = child_witness_needed.then_some(FnRef(next + dead_offset));
    let fixed_count = dead_offset + u32::from(child_witness_needed);
    (child_witness, FnRef(next + fixed_count))
}
