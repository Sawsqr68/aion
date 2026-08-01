//! Step-body and statement lowering (split from `flow` for the 500-line law):
//! one step's statements in written order, the step tail (outcome clauses,
//! chain fall-through, or the flow end), and the per-statement dispatch every
//! nested body (`loops`, `fork_named`, `failure`) reuses. A step's `on
//! failure` edge is layered over this body by `failure`, invoked from
//! `flow::lower_chain_step`.
//!
//! Still deferred (`LowerError::unsupported`, visible incompleteness — never
//! silent divergence from the reference):
//! - substeps: mechanical but large (per-substep fns over the shared plan's
//!   `sub_params`/`sub_node`, sibling/parent route frames, `on failure`
//!   interaction — reference `emitter/subs.rs:20-60`) — `FnOrigin::SubStep`
//!   already exists;
//! - `subflow` declarations: refused at the driver's staging gate, now the
//!   only construct that gate holds (value route payloads retired from it
//!   and lower in `route::route_payload`);
//! - dependency-parallel region layers (`parallel region`) — see the design
//!   sketch retained in the module history (flatten `region.layers` into the
//!   emitter's step walk, `workflow.all` single-call layers, degrade richer
//!   layers to written order with the S13 marker).
//!
//! Parity refusals (the reference emitter refuses these too — a language
//! boundary, not a direct-path gap): mid-chain routes, indexing inside
//! outcome guards and `max … visits` bounds, and the fork/fan-out boundaries
//! named in `forks`/`fanout`.

use crate::ast::{CallStmt, PipeEnd, Statement, Step};
use crate::emitter::type_ref_to_g;

use super::super::ids::{FnRef, Var};
use super::super::ops::{Block, Stmt, Tail, Value};
use super::activity::{activity_call, lower_sleep, record_new, zero_span};
use super::ctx::Ctx;
use super::driver::LowerError;
use super::expr::{Binding, Scope};
use super::flow::{FlowEnv, FlowExit};
use super::forks::lower_fork_stmt;
use super::loops::lower_loop_stmt;
use super::outcome::lower_outcomes;
use super::route::route_tail;
use super::slots::Slots;

/// The fall-through continuation of a non-terminal chain step: the successor's
/// function ref and parameter names.
pub(super) struct Next {
    pub(super) callee: FnRef,
    pub(super) param_names: Vec<String>,
}

pub(super) fn lower_step(
    ctx: &mut Ctx<'_>,
    env: FlowEnv<'_>,
    step: &Step,
    scope: &mut Scope,
    next: Option<Next>,
    region_last: usize,
    slots: &mut Slots,
) -> Result<Block, LowerError> {
    if step.body.iter().any(|s| matches!(s, Statement::SubStep(_))) {
        return Err(LowerError::unsupported("substeps", step.name_span));
    }
    let mut stmts = Vec::new();
    for statement in &step.body {
        if let Some(tail) = lower_statement(ctx, env, step, statement, scope, &mut stmts, slots)? {
            return Ok(Block { stmts, tail });
        }
    }
    if !step.outcomes.is_empty() {
        let outcome = lower_outcomes(ctx, env, &step.outcomes, scope)
            .map_err(|error| error.reanchor_unsupported(step.name_span))?;
        stmts.extend(outcome.stmts);
        return Ok(Block {
            stmts,
            tail: outcome.tail,
        });
    }
    // Fall-through: hand the chain-boundary live set to the successor as a
    // tail call (IR-14).
    if let Some(next) = next {
        let mut args = Vec::new();
        for name in &next.param_names {
            let binding = scope.get(name).ok_or_else(|| {
                LowerError::new(
                    step.name_span,
                    format!("fall-through needs `{name}` in scope"),
                )
            })?;
            args.push(Value::Var(binding.var));
        }
        return Ok(Block {
            stmts,
            tail: Tail::TailLocal {
                callee: next.callee,
                args,
            },
        });
    }
    flow_end(ctx, env, step, scope, region_last, stmts)
}

/// Where control goes when the flow's last chain step completes: an implicit
/// tail call into the next step's region, a member flow's exit return, or the
/// honest refusal (the reference `emitter/steps.rs::emit_flow_end`).
fn flow_end(
    ctx: &mut Ctx<'_>,
    env: FlowEnv<'_>,
    step: &Step,
    scope: &Scope,
    region_last: usize,
    mut stmts: Vec<Stmt>,
) -> Result<Block, LowerError> {
    let flow = env.flow;
    let next = region_last + 1;
    if next < flow.steps.len() {
        let target = &flow.steps[next];
        let region = flow.plan.region_of_entry(next).ok_or_else(|| {
            LowerError::new(
                target.name_span,
                format!(
                    "control falls into `{}`, which does not head a region — the document \
                     did not check cleanly",
                    target.name
                ),
            )
        })?;
        let mut args = Vec::new();
        for name in flow.plan.region_params(region) {
            let binding = scope.get(name).ok_or_else(|| {
                LowerError::new(
                    step.name_span,
                    format!("fall-through needs `{name}` in scope"),
                )
            })?;
            args.push(Value::Var(binding.var));
        }
        return Ok(Block {
            stmts,
            tail: Tail::TailLocal {
                callee: flow.fns.regions[region],
                args,
            },
        });
    }
    match &flow.exit {
        Some(FlowExit { binding, .. }) => {
            let bound = scope.get(binding).ok_or_else(|| {
                LowerError::new(
                    step.name_span,
                    format!("the collected binding `{binding}` is not in scope at the flow's end"),
                )
            })?;
            let ok = ctx.atom("ok");
            let value = record_new(ctx, ok, vec![Value::Var(bound.var)], &mut stmts);
            Ok(Block {
                stmts,
                tail: Tail::Return(Value::Var(value)),
            })
        }
        None => Err(LowerError::unsupported(
            "step falls through without a route",
            step.name_span,
        )),
    }
}

pub(super) fn lower_statement(
    ctx: &mut Ctx<'_>,
    env: FlowEnv<'_>,
    step: &Step,
    statement: &Statement,
    scope: &mut Scope,
    stmts: &mut Vec<Stmt>,
    slots: &mut Slots,
) -> Result<Option<Tail>, LowerError> {
    match statement {
        Statement::Call(call) => {
            lower_call(ctx, env, call, scope, stmts)?;
            Ok(None)
        }
        Statement::Sleep(sleep) => {
            lower_sleep(
                ctx,
                sleep.duration.magnitude,
                sleep.duration.unit,
                stmts,
                sleep.duration.span,
            )?;
            Ok(None)
        }
        Statement::Pipe(pipe) => {
            let (value, ty) = super::pipes::lower_pipe_value(
                ctx,
                env.plan,
                &pipe.head,
                &pipe.stages,
                scope,
                stmts,
            )?;
            match &pipe.end {
                PipeEnd::Bind(binding) => {
                    let var = as_var(ctx, value, stmts);
                    scope.insert(binding.name.clone(), Binding { var, ty });
                    Ok(None)
                }
                PipeEnd::Route(target) => {
                    let tail = route_tail(ctx, env, target, scope, Some((value, ty)), stmts)?;
                    Ok(Some(tail))
                }
            }
        }
        Statement::Route(route) => {
            let tail = route_tail(ctx, env, &route.target, scope, None, stmts)?;
            Ok(Some(tail))
        }
        Statement::Spawn(spawn) => {
            super::child_call::lower_spawn_stmt(ctx, env.plan, spawn, scope, stmts)?;
            Ok(None)
        }
        Statement::Wait(wait) => {
            super::wait::lower_wait_stmt(ctx, env.plan, step, wait, scope, stmts, slots)?;
            Ok(None)
        }
        Statement::Send(send) => {
            super::send::lower_send_stmt(ctx, env.plan, send, scope, stmts)?;
            Ok(None)
        }
        Statement::Answer(answer) => {
            super::query::lower_answer_stmt(ctx, env.plan, answer, scope, stmts)?;
            Ok(None)
        }
        Statement::Fork(fork) => {
            lower_fork_stmt(ctx, env, step, fork, scope, stmts, slots)?;
            Ok(None)
        }
        Statement::Loop(looped) => {
            lower_loop_stmt(ctx, env, step, looped, scope, stmts, slots)?;
            Ok(None)
        }
        Statement::SubStep(sub) => Err(LowerError::unsupported("substep", sub.name_span)),
        // The collapsed region step's fan-out pair: the header lowers the
        // whole delivery + collect; the collect marker is consumed.
        Statement::Distribute(distribute) => {
            super::fanout::lower_fanout(ctx, env, step, distribute, scope, stmts, slots)?;
            Ok(None)
        }
        Statement::Collect(_) => Ok(None),
    }
}

pub(super) fn lower_call(
    ctx: &mut Ctx<'_>,
    env: FlowEnv<'_>,
    call: &CallStmt,
    scope: &mut Scope,
    stmts: &mut Vec<Stmt>,
) -> Result<(), LowerError> {
    // An awaited child call dispatches through the string-name spawn form;
    // config on a child call refuses with the reference's class
    // (`child_call::lower_child_call_stmt`).
    if ctx.emitter.children.contains_key(call.call.name.as_str()) {
        return super::child_call::lower_child_call_stmt(ctx, env.plan, call, scope, stmts);
    }
    let bound = activity_call(
        ctx,
        env.plan,
        &call.call,
        call.config.as_ref(),
        None,
        scope,
        stmts,
    )?;
    if let Some(bind) = &call.bind {
        let ty = ctx
            .emitter
            .actions
            .get(call.call.name.as_str())
            .map(|&(_, decl)| type_ref_to_g(&decl.returns))
            .ok_or_else(|| {
                LowerError::new(
                    call.call.name_span,
                    format!(
                        "`{}` names neither a declared action nor a child workflow",
                        call.call.name
                    ),
                )
            })?;
        scope.insert(bind.name.clone(), Binding { var: bound, ty });
    }
    Ok(())
}

pub(super) fn as_var(ctx: &mut Ctx<'_>, value: Value, stmts: &mut Vec<Stmt>) -> Var {
    match value {
        Value::Var(var) => var,
        other => {
            let dst = ctx.fresh_var();
            stmts.push(Stmt::Bind {
                dst,
                value: other,
                span: zero_span(),
            });
            dst
        }
    }
}
