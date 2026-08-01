//! Substep-chain lowering — the MIR twin of the reference
//! `emitter/subs.rs::emit_sub_chain`/`emit_sub_tail`.
//!
//! A step body's TRAILING substep block lowers to one `FlowFn` per substep
//! (`{prefix}sub_<parent>_<sub>`, the reference `FlowCtx::sub_fn` name), each
//! entered by a tail call: the parent's own function hands off to the first
//! substep after its leading statements, a substep that falls through tail-calls
//! the next sibling, and the LAST substep evaluates the parent's outcome clauses
//! inline — the substep group's boundary, which is why the reference refuses a
//! substep parent that carries none.
//!
//! Route resolution inside a substep runs through a frame
//! ([`super::flow::SubFrame`], the twin of `emitter/steps.rs::Frame`): sibling
//! substeps resolve FIRST, then the parent's outcome-clause names, whose routes
//! are then evaluated in the PARENT frame — so a parent arm can reach a step or
//! a workflow outcome but never a sibling substep, exactly as the reference
//! resolves it.
//!
//! `on failure` on a substep rides the same shape a top-level step's edge does
//! (`super::failure`): the body's fallible results are tested in place and a
//! failure tail-calls the lifted compensation, while the substep's tail — its
//! own outcomes, the sibling hand-off, or the parent's arms — is a `Tail` and
//! can therefore never pass through the guard. That is the reference's
//! "a routed failure outcome stays outside the compensation attempt" invariant,
//! here made unrepresentable rather than merely avoided.

use crate::ast::{Statement, Step};
use crate::emitter::{snake, substep_split as reference_substep_split};

use super::super::func::{FlowFn, FnOrigin, MirFn};
use super::super::ids::{FnRef, Span};
use super::super::ops::{Block, Tail, Value};
use super::ctx::Ctx;
use super::driver::LowerError;
use super::expr::Scope;
use super::failure::Guarded;
use super::flow::{FlowCtx, FlowEnv, SubFrame};
use super::outcome::lower_outcomes;
use super::slots::Slots;
use super::step::lower_body_stmts;

/// Where a step body's trailing substep block starts, with the reference's own
/// shape gates: statements after the block, a substep nesting further substeps,
/// and a substep parent carrying no outcome clauses all refuse. The reference
/// function is called directly rather than mirrored, so the two backends cannot
/// drift about what a substep block IS.
pub(super) fn substep_split(step: &Step) -> Result<usize, LowerError> {
    reference_substep_split(step).map_err(|error| LowerError::new(error.span, error.message))
}

/// The substeps of one step's trailing block, in written order.
pub(super) fn substeps_of(step: &Step) -> Result<Vec<&Step>, LowerError> {
    let split = substep_split(step)?;
    let mut subs = Vec::new();
    for statement in step.body.get(split..).unwrap_or_default() {
        let Statement::SubStep(sub) = statement else {
            return Err(LowerError::new(
                step.name_span,
                format!("step `{}`'s substep block is mis-shaped", step.name),
            ));
        };
        subs.push(sub.as_ref());
    }
    Ok(subs)
}

/// The generated-name stem of one substep's function (`sub_<parent>_<sub>` —
/// the reference `FlowCtx::sub_fn`, without the flow prefix).
pub(super) fn sub_stem(parent: &str, sub: &str) -> String {
    format!("sub_{}_{}", snake(parent), snake(sub))
}

/// The reserved function reference of one substep.
fn sub_ref(flow: &FlowCtx<'_>, parent: usize, position: usize) -> Result<FnRef, LowerError> {
    flow.fns
        .subs
        .get(&(parent, position))
        .copied()
        .ok_or_else(|| LowerError::Planning {
            message: format!("substep {position} of step {parent} reserved no function slot"),
        })
}

/// One substep call's arguments: the shared plan's `sub_params` read out of the
/// caller's scope, exactly as the reference renders them by name.
fn sub_args(
    env: FlowEnv<'_>,
    parent: usize,
    position: usize,
    scope: &Scope,
    span: crate::Span,
) -> Result<Vec<Value>, LowerError> {
    let names = env
        .flow
        .plan
        .sub_params_of(parent, position)
        .ok_or_else(|| LowerError::Planning {
            message: format!("substep {position} of step {parent} was never planned"),
        })?
        .to_vec();
    let mut args = Vec::with_capacity(names.len());
    for name in &names {
        let binding = scope.get(name).ok_or_else(|| {
            LowerError::new(span, format!("the substep chain needs `{name}` in scope"))
        })?;
        args.push(Value::Var(binding.var));
    }
    Ok(args)
}

/// A tail call into a sibling substep (or, from the parent, into the first one)
/// — the shape every substep edge takes.
pub(super) fn call_tail(
    env: FlowEnv<'_>,
    parent: usize,
    position: usize,
    scope: &Scope,
    span: crate::Span,
) -> Result<Tail, LowerError> {
    Ok(Tail::TailLocal {
        callee: sub_ref(env.flow, parent, position)?,
        args: sub_args(env, parent, position, scope, span)?,
    })
}

/// Identity of one substep within its parent's chain.
struct SubChain<'a> {
    parent: &'a Step,
    /// The parent's index in the flow's step list.
    index: usize,
    /// Where the parent's trailing substep block starts.
    split: usize,
    /// How many substeps the block holds.
    count: usize,
    /// This substep's position in the block.
    position: usize,
}

/// Lower every substep chain of one flow, in the reference's emission order:
/// steps in written order, substeps in block order, AFTER every region function
/// of the flow (`emitter/steps.rs::emit_flow_fns`). That is the order
/// `plan_slots` reserves the slots in.
pub(super) fn lower_sub_chains(
    ctx: &mut Ctx<'_>,
    env: FlowEnv<'_>,
    functions: &mut Vec<MirFn>,
    slots: &mut Slots,
) -> Result<(), LowerError> {
    let flow = env.flow;
    for (index, parent) in flow.steps.iter().enumerate() {
        let split = substep_split(parent)?;
        let subs = substeps_of(parent)?;
        let count = subs.len();
        for (position, sub) in subs.into_iter().enumerate() {
            let chain = SubChain {
                parent,
                index,
                split,
                count,
                position,
            };
            let function = lower_sub(ctx, env, &chain, sub, slots)?;
            functions.push(MirFn::Flow(function));
        }
    }
    Ok(())
}

fn lower_sub(
    ctx: &mut Ctx<'_>,
    env: FlowEnv<'_>,
    chain: &SubChain<'_>,
    sub: &Step,
    slots: &mut Slots,
) -> Result<FlowFn, LowerError> {
    // The reference emits NO visit prologue for a substep
    // (`emitter/subs.rs::emit_sub_chain` has no `emit_visits_prologue` call),
    // so a bound declared on one is rewritten into its guards and then never
    // incremented or tested — silently unenforced. Refuse rather than mirror a
    // silent drop; a landing that lowers the prologue here retires this.
    if let Some(max_visits) = &sub.max_visits {
        return Err(LowerError::unsupported(
            "a `max … visits` bound on a substep",
            max_visits.span,
        ));
    }
    let flow = env.flow;
    ctx.reset_vars();
    let param_names = flow
        .plan
        .sub_params_of(chain.index, chain.position)
        .ok_or_else(|| LowerError::Planning {
            message: format!(
                "substep `{}` of step `{}` was never planned",
                sub.name, chain.parent.name
            ),
        })?
        .to_vec();
    let mut scope: Scope = Scope::new();
    let (params, param_tys) =
        super::chain::bind_params(ctx, flow, &param_names, sub.name_span, &mut scope)?;
    // Every route inside this substep resolves against its own frame.
    let framed = FlowEnv {
        frame: Some(SubFrame {
            parent: chain.index,
            split: chain.split,
        }),
        ..env
    };
    // The `on failure` edge is planned before the body lowers, over the scope
    // as it stands at the substep's entry — the same discipline
    // `chain::lower_chain_step` follows for a top-level step.
    let edge = match &sub.on_failure {
        Some(on_failure) => Some((
            on_failure,
            super::failure::plan_edge(ctx, sub, on_failure, &param_names, &scope, slots)?,
        )),
        None => None,
    };
    let body = lower_sub_body(ctx, framed, chain, sub, &mut scope, slots)?;
    let body = match edge {
        Some((on_failure, edge)) => {
            let body = super::failure::guard(sub, body, &edge)?;
            let guarded = Guarded {
                step: sub,
                on_failure,
                param_names: &param_names,
                stem: sub_stem(&chain.parent.name, &sub.name),
                origin_step: flow.origin_step(&format!("{}.{}", chain.parent.name, sub.name)),
            };
            super::failure::lower_compensation(ctx, framed, &guarded, &edge, slots)?;
            body
        }
        None => body,
    };
    Ok(FlowFn {
        origin: FnOrigin::SubStep {
            parent: flow.origin_step(&chain.parent.name),
            sub: sub.name.clone(),
        },
        name: format!("{}{}", flow.prefix, sub_stem(&chain.parent.name, &sub.name)),
        params,
        param_tys,
        ret_ty: flow.ret_ty.clone(),
        body,
        span: Span::from_source(sub.name_span),
        degraded_parallel: false,
    })
}

/// One substep's statements and its tail (the reference `emit_sub_tail`): its
/// own outcome clauses, an already-terminal route, the next sibling, or — at the
/// chain's end — the parent's outcome clauses evaluated in the parent frame.
fn lower_sub_body(
    ctx: &mut Ctx<'_>,
    env: FlowEnv<'_>,
    chain: &SubChain<'_>,
    sub: &Step,
    scope: &mut Scope,
    slots: &mut Slots,
) -> Result<Block, LowerError> {
    let mut stmts = Vec::new();
    if let Some(tail) = lower_body_stmts(ctx, env, sub, &sub.body, scope, &mut stmts, slots)? {
        return Ok(Block { stmts, tail });
    }
    if !sub.outcomes.is_empty() {
        let outcome = lower_outcomes(ctx, env, &sub.outcomes, scope)
            .map_err(|error| error.reanchor_unsupported(sub.name_span))?;
        stmts.extend(outcome.stmts);
        return Ok(Block {
            stmts,
            tail: outcome.tail,
        });
    }
    let next = chain.position + 1;
    if next < chain.count {
        let tail = call_tail(env, chain.index, next, scope, sub.name_span)?;
        return Ok(Block { stmts, tail });
    }
    // The chain's end: the parent's outcome clauses ARE the boundary, and they
    // resolve at the parent's level. `substep_split` already refuses a parent
    // without them, so an empty list here is a planning bug, not a document
    // error.
    if chain.parent.outcomes.is_empty() {
        return Err(LowerError::Planning {
            message: format!(
                "step `{}` carries substeps but no outcome clauses to close the chain",
                chain.parent.name
            ),
        });
    }
    let parent_env = FlowEnv { frame: None, ..env };
    let outcome = lower_outcomes(ctx, parent_env, &chain.parent.outcomes, scope)
        .map_err(|error| error.reanchor_unsupported(chain.parent.name_span))?;
    stmts.extend(outcome.stmts);
    Ok(Block {
        stmts,
        tail: outcome.tail,
    })
}
