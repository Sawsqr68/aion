//! Dependency-parallel layer lowering: one region layer of single-call steps
//! dispatched as ONE `workflow.all` — the MIR twin of the reference
//! `emitter/steps.rs::emit_parallel_layer`.
//!
//! Typed when every member calls the same action; raw wire-unified twins plus
//! a per-position decode when the members call different ones — the reference
//! delegates that half to `emitter/forks.rs::lower_hetero_parallel`, the very
//! function a heterogeneous NAMED fork uses, so both direct-path callers share
//! `fork_named::all_dispatch` for exactly the same reason.
//!
//! A layer that cannot dispatch concurrently never reaches here: `layers`
//! classifies it as degraded and its members lower as ordinary chain steps in
//! written order, carrying the S13 `degraded_parallel` marker on the first.

use crate::emitter::snake;

use super::super::func::{FlowFn, FnOrigin};
use super::super::ids::Span;
use super::super::ops::{Block, Tail, Value};
use super::ctx::Ctx;
use super::driver::LowerError;
use super::expr::Scope;
use super::flow::FlowEnv;
use super::layers;
use super::step::{Next, flow_end};

/// One parallelizable layer being lowered into its own chain function.
pub(super) struct ParallelUnit<'a> {
    /// The region entry step, for the function's origin qualifier.
    pub(super) entry_step: &'a crate::ast::Step,
    /// Every member step index of the layer, in written order.
    pub(super) members: &'a [usize],
    /// This unit's position in the region's chain.
    pub(super) position: usize,
    pub(super) param_names: &'a [String],
    /// The tail call into the next chain unit, when one follows.
    pub(super) next: Option<Next>,
    /// The region's highest member index — the fall-through anchor.
    pub(super) region_last: usize,
}

pub(super) fn lower_parallel_layer(
    ctx: &mut Ctx<'_>,
    env: FlowEnv<'_>,
    unit: ParallelUnit<'_>,
) -> Result<FlowFn, LowerError> {
    let flow = env.flow;
    let anchor_index = *unit.members.first().ok_or_else(|| LowerError::Planning {
        message: "a parallel chain unit holds no members".to_owned(),
    })?;
    let anchor = flow
        .steps
        .get(anchor_index)
        .ok_or_else(|| LowerError::Planning {
            message: format!("parallel layer names step {anchor_index}, which the flow lacks"),
        })?
        .clone();

    ctx.reset_vars();
    let mut scope: Scope = Scope::new();
    let mut param_vars = Vec::new();
    let mut param_tys = Vec::new();
    for name in unit.param_names {
        let ty = flow.bindings.get(name).cloned().ok_or_else(|| {
            LowerError::new(anchor.name_span, format!("binding `{name}` has no type"))
        })?;
        let var = ctx.fresh_var();
        param_vars.push(var);
        param_tys.push(ctx.tydesc(&ty));
        scope.insert(
            name.clone(),
            super::expr::Binding {
                var,
                ty: ty.clone(),
            },
        );
    }

    // Re-derive the classification rather than carry the borrowed calls across
    // the slot plan: `layers` is the single definition of what parallelizes,
    // and a unit that no longer classifies is a planning bug, never a document
    // error — the slot count was reserved from this same answer.
    let calls = layers::parallel_layer_calls(flow.steps, unit.members, ctx.emitter).ok_or_else(
        || LowerError::Planning {
            message: format!(
                "the layer headed by step `{}` was planned as parallel but no longer classifies",
                anchor.name
            ),
        },
    )?;
    let homogeneous = layers::homogeneous(&calls);
    let mut stmts = Vec::new();
    let binds = super::fork_named::all_dispatch(
        ctx,
        env.plan,
        &calls,
        !homogeneous,
        anchor.name_span,
        &scope,
        &mut stmts,
    )?;
    super::fork_named::bind_branches(
        ctx,
        env.plan,
        &calls,
        binds,
        homogeneous,
        &mut scope,
        &mut stmts,
    )?;

    // Every member of a multi-member layer falls through (a member that routes
    // away while parallel work is outstanding is refused during planning —
    // `emitter/graph.rs::build_regions`), so the layer's tail is always the
    // chain's own continuation and never a member's route.
    let body = match unit.next {
        Some(next) => {
            let mut args = Vec::new();
            for name in &next.param_names {
                let binding = scope.get(name).ok_or_else(|| {
                    LowerError::new(
                        anchor.name_span,
                        format!("fall-through needs `{name}` in scope"),
                    )
                })?;
                args.push(Value::Var(binding.var));
            }
            Block {
                stmts,
                tail: Tail::TailLocal {
                    callee: next.callee,
                    args,
                },
            }
        }
        None => flow_end(ctx, env, &anchor, &scope, unit.region_last, stmts)?,
    };

    let origin = if unit.position == 0 {
        FnOrigin::Region {
            entry_step: flow.origin_step(&unit.entry_step.name),
        }
    } else {
        FnOrigin::ChainStep {
            entry_step: flow.origin_step(&unit.entry_step.name),
            step: anchor.name.clone(),
        }
    };
    Ok(FlowFn {
        origin,
        name: format!("{}step_{}", flow.prefix, snake(&anchor.name)),
        params: param_vars,
        param_tys,
        ret_ty: flow.ret_ty.clone(),
        body,
        span: Span::from_source(anchor.name_span),
        degraded_parallel: false,
    })
}
