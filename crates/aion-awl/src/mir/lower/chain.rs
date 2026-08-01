//! Chain-boundary name liveness for multi-step sequential regions: each
//! non-entry chain step lowers to its own `FlowFn`, reached by the previous
//! step's tail call (IR-14), so its parameter list is the set of names live at
//! that boundary. This is the shared plan's fixed point restricted to a
//! straight-line chain — refs/defs collection follows the reference
//! (`emitter/liveness.rs` `collect_into`/`collect_route`) except for the
//! loop-body-bind asymmetry documented at the loop arm below. A route to
//! another region contributes that region's `region_params` exactly as the
//! fixed point's `params(callee) − defs` term does; a route to a nested
//! flow's exit contributes the collected binding (a region member flow) or
//! the bare-route pickup (a subflow), mirroring the shared collector's
//! `target_refs_form`. A chain whose last step falls through seeds the
//! backward walk with the demand of the implicit hand-off (the next step's
//! region params, or a member flow's collected binding — a subflow has no
//! fall-through exit, so it seeds nothing). The
//! shared collector cannot be consumed directly here: it intentionally
//! aggregates an entire region into one graph node, while chain splitting
//! needs a live-in set at every interior step boundary. A step carrying a
//! trailing SUBSTEP block folds its leading statements plus the shared plan's
//! `sub_params` for the first substep — the reference's own call edge from the
//! region node into `sub_node[(member, 0)]` — and not its own outcome clauses,
//! which the LAST substep evaluates.

use std::collections::BTreeSet;

use crate::ast::{Expr, Guard, PipeEnd, PipeStage, RoutePayload, RouteTarget, Statement, Step};
use crate::emitter::{expr_refs, falls_through};

use super::super::ids::Var;
use super::super::tydesc::TyDesc;
use super::ctx::Ctx;
use super::driver::LowerError;
use super::expr::{Binding, Scope};
use super::flow::{ExitKind, FlowCtx, FlowExit};
use super::layers::ChainUnit;
use super::visits::counter_name;

/// Per-position parameter names for a region's chain, in chain-unit order. The
/// entry position is computed too (it equals the region's params for a chain
/// the shared fixed point saw), but callers keep `region_params` for the entry
/// so the parity anchor stays the shared plan.
///
/// A parallel chain unit folds every member of its layer. Layer membership is
/// mutual dependency-independence (`emitter/graph.rs::layer`), so no member
/// reads a name another member defines and the fold is order-insensitive
/// across them; they are folded in reverse written order all the same, so the
/// walk stays one uniform backward pass.
///
/// `region_last` is the region's HIGHEST member index — the reference's own
/// fall-through anchor (`emitter/liveness.rs`'s `layers.iter().flatten().max()`
/// and `emitter/steps.rs::emit_region`) — not the last unit's step, which can
/// differ when an `after` names a step written later.
pub(super) fn chain_params(
    ctx: &Ctx<'_>,
    flow: &FlowCtx<'_>,
    units: &[ChainUnit],
    region_last: usize,
) -> Result<Vec<Vec<String>>, LowerError> {
    let steps = flow.steps;
    let mut live: BTreeSet<String> = BTreeSet::new();
    // Seed with the implicit end-of-chain demand: falling through into the
    // next step's region, or a member flow's exit return of the collected
    // binding (the reference `emit_flow_end` / the shared collector's
    // fall-through edge).
    if steps.get(region_last).is_some_and(falls_through) {
        let next = region_last + 1;
        if next < steps.len() {
            if let Some(region) = flow.plan.region_of_entry(next) {
                live.extend(flow.plan.region_params(region).iter().cloned());
            }
        } else if let Some(FlowExit {
            kind: ExitKind::Region { binding },
            ..
        }) = &flow.exit
        {
            // Only a REGION member flow has a fall-through exit; a subflow's
            // last step must route to its outcome, so nothing is demanded here
            // (the reference liveness matches only `ExitLive::Region`).
            live.insert(binding.clone());
        }
    }
    let mut params = vec![Vec::new(); units.len()];
    for position in (0..units.len()).rev() {
        for &member in units[position].members.iter().rev() {
            let step = steps.get(member).ok_or_else(|| LowerError::Planning {
                message: format!("chain unit names step {member}, which the flow does not hold"),
            })?;
            step_live_in(ctx, flow, member, step, &mut live)?;
        }
        params[position] = live.iter().cloned().collect();
    }
    Ok(params)
}

/// Bind a function's parameter names into a fresh scope: one SSA var per name,
/// with the type the flow's binding environment established for it.
///
/// Every generated flow function opens this way — a chain step, a substep, and
/// a lifted compensation alike — so the three share it rather than each
/// re-deriving the same walk. A name the environment cannot type is a document
/// that did not check cleanly, never a `Nil` guess.
pub(super) fn bind_params(
    ctx: &mut Ctx<'_>,
    flow: &FlowCtx<'_>,
    names: &[String],
    anchor: crate::Span,
    scope: &mut Scope,
) -> Result<(Vec<Var>, Vec<TyDesc>), LowerError> {
    let mut vars = Vec::with_capacity(names.len());
    let mut tys = Vec::with_capacity(names.len());
    for name in names {
        let ty = flow
            .bindings
            .get(name)
            .cloned()
            .ok_or_else(|| LowerError::new(anchor, format!("binding `{name}` has no type")))?;
        let var = ctx.fresh_var();
        vars.push(var);
        tys.push(ctx.tydesc(&ty));
        scope.insert(name.clone(), Binding { var, ty });
    }
    Ok((vars, tys))
}

/// Fold one step: `live = refs(step) ∪ (live − defs(step))`, with refs
/// collected use-before-def in written order.
fn step_live_in(
    ctx: &Ctx<'_>,
    flow: &FlowCtx<'_>,
    index: usize,
    step: &Step,
    live: &mut BTreeSet<String>,
) -> Result<(), LowerError> {
    let mut refs: BTreeSet<String> = BTreeSet::new();
    let mut defs: BTreeSet<String> = BTreeSet::new();
    // A bounded step reads its language-owned visit counter and its bound
    // expression at entry, before any of its defs (the shared collector's
    // `collect_step`).
    if let Some(max_visits) = &step.max_visits {
        refs.insert(counter_name(ctx, step)?);
        add_expr(&max_visits.bound, &defs, &mut refs);
    }
    // A collapsed per-item region step reads every free name its member
    // flow's wrapper threads in (beyond the per-item variable).
    if matches!(step.body.first(), Some(Statement::Distribute(_)))
        && let Some(region) = flow.regions.get(&step.name)
        && let Some(nested) = ctx.plans.regions.get(&region.id)
    {
        for name in &nested.wrapper_params {
            if name != &region.var {
                refs.insert(name.clone());
            }
        }
    }
    let split = super::subs::substep_split(step)?;
    let leading = step.body.get(..split).unwrap_or(&step.body);
    collect_statements(ctx, flow, leading, &mut defs, &mut refs);
    if split < step.body.len() {
        // A substep parent's demand is its leading statements plus whatever the
        // FIRST substep's function needs — the shared plan's fixed point for
        // that node, which already folds the whole substep chain and the
        // parent's own outcome clauses (the reference's call edge from the
        // region node to `sub_node[(member, 0)]`, and its
        // `collect_substep` chain-end fold). The parent's arms are therefore
        // deliberately NOT collected here: they are evaluated at the chain's
        // end, inside the last substep, not at the parent.
        let params = flow
            .plan
            .sub_params_of(index, 0)
            .ok_or_else(|| LowerError::Planning {
                message: format!("step `{}` has substeps but no planned nodes", step.name),
            })?;
        for name in params {
            if !defs.contains(name) {
                refs.insert(name.clone());
            }
        }
    } else {
        for clause in &step.outcomes {
            if let Guard::When { expr, .. } = &clause.guard {
                add_expr(expr, &defs, &mut refs);
            }
            route_refs(ctx, flow, &clause.route, &defs, &mut refs, true);
        }
    }
    for def in &defs {
        live.remove(def);
    }
    live.extend(refs);
    Ok(())
}

fn collect_statements(
    ctx: &Ctx<'_>,
    flow: &FlowCtx<'_>,
    statements: &[Statement],
    defs: &mut BTreeSet<String>,
    refs: &mut BTreeSet<String>,
) {
    for statement in statements {
        match statement {
            Statement::Call(call) => {
                for arg in &call.call.args {
                    add_expr(&arg.value, defs, refs);
                }
                if let Some(bind) = &call.bind {
                    defs.insert(bind.name.clone());
                }
            }
            Statement::Spawn(spawn) => {
                for arg in &spawn.call.args {
                    add_expr(&arg.value, defs, refs);
                }
            }
            Statement::Pipe(pipe) => {
                add_expr(&pipe.head, defs, refs);
                for stage in &pipe.stages {
                    if let PipeStage::Combinator(combinator) = stage
                        && let Some(arg) = &combinator.arg
                    {
                        add_expr(arg, defs, refs);
                    }
                }
                match &pipe.end {
                    PipeEnd::Bind(binding) => {
                        defs.insert(binding.name.clone());
                    }
                    PipeEnd::Route(target) => {
                        // A piped route carries the piped value as its
                        // payload: no bare-outcome pickup (`collect_pipe`).
                        route_refs(ctx, flow, target, defs, refs, false);
                    }
                }
            }
            Statement::Route(route) => {
                route_refs(ctx, flow, &route.target, defs, refs, true);
            }
            Statement::Wait(wait) => {
                defs.insert(wait.bind.name.clone());
            }
            // A send defines nothing and reads its target plus every payload
            // expression.
            Statement::Send(send) => {
                add_expr(&send.target, defs, refs);
                for arg in send.payload_args() {
                    add_expr(&arg.value, defs, refs);
                }
                if let Some(RoutePayload::Value(value)) = &send.payload {
                    add_expr(value, defs, refs);
                }
            }
            // An answer defines nothing and reads every value expression it
            // registers.
            Statement::Answer(answer) => {
                for arg in answer.value_args() {
                    add_expr(&arg.value, defs, refs);
                }
                if let Some(RoutePayload::Value(value)) = &answer.value {
                    add_expr(value, defs, refs);
                }
            }
            // Boundary-shaped like the shared collector's `collect_loop`:
            // seed and `max` read the pre-loop scope; the threaded var and
            // counter are local while collecting the body/`until`. Deliberate
            // asymmetry: shared `collect_into` registers every `loop_defs`
            // entry (including body binds) as a step def; this chain boundary
            // keeps body binds local because the checker forbids them after
            // the loop. Only the threaded value and counter escape here.
            Statement::Loop(looped) => {
                add_expr(&looped.seed, defs, refs);
                if let Some(max) = &looped.max {
                    add_expr(&max.expr, defs, refs);
                }
                let mut loop_defs = defs.clone();
                loop_defs.insert(looped.var.clone());
                if let Some(counter) = &looped.counter {
                    loop_defs.insert(counter.name.clone());
                }
                collect_statements(ctx, flow, &looped.body, &mut loop_defs, refs);
                if let Some(until) = &looped.until {
                    add_expr(&until.expr, &loop_defs, refs);
                }
                defs.insert(looped.var.clone());
                if let Some(counter) = &looped.counter {
                    defs.insert(counter.name.clone());
                }
            }
            Statement::Fork(fork) => {
                if let crate::ast::ForkHeader::Collection {
                    var, collection, ..
                } = &fork.header
                {
                    add_expr(collection, defs, refs);
                    let mut branch_defs = defs.clone();
                    branch_defs.insert(var.clone());
                    collect_statements(ctx, flow, &fork.body, &mut branch_defs, refs);
                } else {
                    // Named branches merge their bindings at the join.
                    collect_statements(ctx, flow, &fork.body, defs, refs);
                }
                if let Some(bind) = &fork.join.bind {
                    defs.insert(bind.name.clone());
                }
            }
            // The fan-out pair of a collapsed region step: the header reads
            // the collection, the collect defines the gathered binding
            // (member free names fold in `step_live_in`).
            Statement::Distribute(distribute) => {
                add_expr(&distribute.collection, defs, refs);
            }
            Statement::Collect(collect) => {
                defs.insert(collect.bind.name.clone());
            }
            // Sleeps reference no bindings. A substep never reaches here: a
            // trailing block is split off in `step_live_in` and folded through
            // its planned node, and a substep in any other position refuses
            // during body lowering.
            Statement::Sleep(_) | Statement::SubStep(_) => {}
        }
    }
}

/// A route's name demand: payload argument (or value expression) refs, the
/// exit contract of a member flow, the bare-route outcome pickup
/// (`Statement::Route` only — a piped route carries its payload), or — for a
/// route to another region's entry — that region's parameter list (the fixed
/// point's cross-function term).
fn route_refs(
    ctx: &Ctx<'_>,
    flow: &FlowCtx<'_>,
    target: &RouteTarget,
    defs: &BTreeSet<String>,
    refs: &mut BTreeSet<String>,
    bare_pickup: bool,
) {
    for arg in target.payload_args() {
        add_expr(&arg.value, defs, refs);
    }
    if let Some(RoutePayload::Value(value)) = &target.payload {
        add_expr(value, defs, refs);
    }
    if let Some(exit) = &flow.exit
        && exit.name == target.name
    {
        match &exit.kind {
            ExitKind::Region { binding } => {
                if !defs.contains(binding) {
                    refs.insert(binding.clone());
                }
            }
            // A subflow's bare `route out` picks up the binding named after
            // the outcome, exactly as a bare workflow-outcome route does (the
            // reference `target_refs_form`'s `ExitLive::Subflow` arm).
            ExitKind::Subflow { .. } => {
                if target.payload.is_none() && bare_pickup && !defs.contains(target.name.as_str()) {
                    refs.insert(target.name.clone());
                }
            }
        }
        return;
    }
    if flow.exit.is_none() && ctx.emitter.outcomes.contains_key(target.name.as_str()) {
        if target.payload.is_none() && bare_pickup && !defs.contains(target.name.as_str()) {
            refs.insert(target.name.clone());
        }
        return;
    }
    if target.payload.is_some() {
        return;
    }
    let Some(step_index) = flow.steps.iter().position(|step| step.name == target.name) else {
        return;
    };
    let Some(region) = flow.plan.region_of_entry(step_index) else {
        return;
    };
    for name in flow.plan.region_params(region) {
        if !defs.contains(name) {
            refs.insert(name.clone());
        }
    }
}

fn add_expr(expr: &Expr, defs: &BTreeSet<String>, refs: &mut BTreeSet<String>) {
    let mut found = BTreeSet::new();
    expr_refs(expr, &mut found);
    for name in found {
        if !defs.contains(&name) {
            refs.insert(name);
        }
    }
}
