//! The `on failure` edge of a top-level step: the guarded body's fallible
//! results are tested in place and a failure tail-calls a LIFTED compensation
//! `FlowFn`.
//!
//! Shape (ruled design B, AWL-BC-IR.md `Attempt` row): every fallible
//! statement in a guarded step already lowers to a `Stmt::TryBind` over its
//! `Result`. Under a guard each `TryBind` becomes
//! `Tail::If { test: Test::IsTagged { ok, 2 } }`: the then-arm unwraps the
//! payload (`FieldGet` at index 1) and carries the rest of the step, the
//! else-arm is a bare `Tail::TailLocal` into the compensation function. Every
//! primitive is the one tolerant collect already uses (`lower/fanout.rs`), so
//! this construct needs NO new instruction selection — the reserved
//! `Stmt::Attempt` op stays unconstructed, and building it would need a new
//! ruling.
//!
//! Two invariants ride the shape rather than a check:
//!
//! - A body-terminal route lowers to a `Tail`, never to a `TryBind`, so it can
//!   never pass through the `IsTagged` test. A deliberate
//!   `route <failure outcome>` therefore CANNOT trigger compensation — the
//!   reference's `emitter/failure.rs` invariant, here made unrepresentable
//!   instead of merely avoided. Outcome clauses and the chain fall-through sit
//!   in the innermost then-arm for the same reason, exactly as the reference
//!   renders them inside its `Ok(...) ->` arm.
//! - The edge binds NOTHING (AWL-COMPLETION-COLD-READ.md §2.3). The reference
//!   discards the reason (`Error(_)`), the AST's `OnFailure` has no binding
//!   slot, and the checker's `avail_in` law gives the compensation block only
//!   what was live at the step's entry — the failure point inside the body is
//!   unknowable. So the compensation function's parameters ARE the step's
//!   `avail_in`: no captures, no closure, no defs tuple. A later
//!   reason-binding would arrive as one additional parameter, not a captures
//!   protocol.

use crate::ast::{OnFailure, Statement, Step};
use crate::emitter::snake;

use super::super::func::{FlowFn, FnOrigin, MirFn};
use super::super::ids::{AtomRef, FnRef, Span, Var};
use super::super::ops::{Block, Stmt, Tail, Test, Value};
use super::ctx::Ctx;
use super::driver::LowerError;
use super::expr::{Binding, Scope};
use super::flow::FlowEnv;
use super::slots::Slots;
use super::step::lower_statement;

/// A guarded step's failure edge: where control goes when any fallible
/// statement in the body hands back an `Error`.
pub(super) struct FailureEdge {
    /// The reserved compensation-function slot's ordinal.
    ordinal: usize,
    /// The compensation function's reference.
    callee: FnRef,
    /// The step's `avail_in` values, in the compensation's parameter order.
    args: Vec<Value>,
    /// The `ok` atom every guard tests against.
    ok: AtomRef,
}

/// Reserve the compensation function and resolve the step's `avail_in` into
/// the edge's call arguments.
///
/// Called with the scope as it stands at the START of the step body — after a
/// `max … visits` prologue (which rebinds the counter) and before any
/// statement lowers, exactly the scope the reference clones into its
/// compensation arm (`emitter/failure.rs:107`).
pub(super) fn plan_edge(
    ctx: &mut Ctx<'_>,
    step: &Step,
    on_failure: &OnFailure,
    param_names: &[String],
    scope: &Scope,
    slots: &mut Slots,
) -> Result<FailureEdge, LowerError> {
    refuse_uninventoried(ctx, on_failure)?;
    let (ordinal, callee) = slots.compensations.take()?;
    let mut args = Vec::with_capacity(param_names.len());
    for name in param_names {
        let binding = scope.get(name).ok_or_else(|| {
            LowerError::new(
                step.name_span,
                format!("the `on failure` edge needs `{name}` in scope"),
            )
        })?;
        args.push(Value::Var(binding.var));
    }
    let ok = ctx.atom("ok");
    Ok(FailureEdge {
        ordinal,
        callee,
        args,
        ok,
    })
}

/// The compensation body's statements are lowered OUTSIDE the shared
/// slot-inventory traversal (`plan_slots::for_each_step` walks `step.body`
/// only), so any construct that consumes a reserved slot would take one that
/// was never counted and misalign every later `FnRef`. Refuse those honestly
/// here rather than lower them wrong; extending the inventories to descend
/// into `on failure` bodies is the increment that retires this.
fn refuse_uninventoried(ctx: &Ctx<'_>, on_failure: &OnFailure) -> Result<(), LowerError> {
    let body = on_failure.body.as_slice();
    let span = on_failure.span;
    for statement in body {
        let shape = match statement {
            Statement::SubStep(_) => "a substep inside an `on failure` block",
            Statement::Distribute(_) | Statement::Collect(_) => {
                "a per-item region inside an `on failure` block"
            }
            Statement::Call(_)
            | Statement::Sleep(_)
            | Statement::Pipe(_)
            | Statement::Route(_)
            | Statement::Spawn(_)
            | Statement::Wait(_)
            | Statement::Send(_)
            // An `answer` draws its two lifted functions from the DYNAMIC
            // pool at the tail of the module (like a collection predicate),
            // never from a reserved inventory, so it cannot misalign a
            // `FnRef` by lowering outside the slot traversal.
            | Statement::Answer(_)
            | Statement::Fork(_)
            | Statement::Loop(_) => continue,
        };
        return Err(LowerError::unsupported(shape, span));
    }
    if super::loops::count_loops(body) > 0 {
        return Err(LowerError::unsupported(
            "a bounded loop inside an `on failure` block",
            span,
        ));
    }
    if super::forks::count_fork_fns(body, ctx.emitter) > 0 {
        return Err(LowerError::unsupported(
            "a fork inside an `on failure` block",
            span,
        ));
    }
    if super::wait::count_wait_fns(body) > 0 {
        return Err(LowerError::unsupported(
            "a timeout wait inside an `on failure` block",
            span,
        ));
    }
    if super::forks::needs_child_witness(body, ctx.emitter) {
        return Err(LowerError::unsupported(
            "a child spawn inside an `on failure` block",
            span,
        ));
    }
    Ok(())
}

/// Rewrite a guarded step body so every `TryBind` tests its result in place
/// and routes a failure to the compensation function, then PROVE none was
/// missed.
///
/// The rewrite reaches the statements of the body's own straight-line burst
/// and everything the recursion carries into the success arms. A `TryBind`
/// left anywhere else — inside a tail's arms, or inside a `ShortCircuit`'s
/// conditionally-evaluated right-hand burst — would return its `Error`
/// straight out of the step function while the reference, whose whole body
/// sits inside the attempt closure, would have caught it and compensated.
/// That is a SILENT divergence, so an unguarded residual refuses instead.
/// The check is over the built block, not over a list of shapes, so a future
/// lowering that starts producing fallible results in a new position is
/// caught by construction rather than by remembering to extend a list.
pub(super) fn guard(step: &Step, body: Block, edge: &FailureEdge) -> Result<Block, LowerError> {
    let guarded = guard_block(body, edge);
    if residual_fallible_block(&guarded) {
        return Err(LowerError::unsupported(
            "a fallible result the `on failure` edge cannot guard in place (a conditional \
             prelude or an outcome-guard result)",
            step.name_span,
        ));
    }
    Ok(guarded)
}

fn guard_block(body: Block, edge: &FailureEdge) -> Block {
    let Block { mut stmts, tail } = body;
    let Some(position) = stmts.iter().position(is_try_bind) else {
        return Block { stmts, tail };
    };
    let mut rest = stmts.split_off(position).into_iter();
    let Some(Stmt::TryBind {
        dst, result, span, ..
    }) = rest.next()
    else {
        // `position` located a `TryBind`; the iterator cannot yield anything
        // else. Leaving the block untouched is the honest no-op.
        return Block { stmts, tail };
    };
    let inner = guard_block(
        Block {
            stmts: rest.collect(),
            tail,
        },
        edge,
    );
    let mut then_stmts = Vec::with_capacity(inner.stmts.len() + 1);
    then_stmts.push(Stmt::FieldGet {
        dst,
        base: Value::Var(result),
        index: 1,
        span,
    });
    then_stmts.extend(inner.stmts);
    Block {
        stmts,
        tail: Tail::If {
            test: Test::IsTagged {
                value: Value::Var(result),
                tag: edge.ok,
                arity: 2,
            },
            then_block: Box::new(Block {
                stmts: then_stmts,
                tail: inner.tail,
            }),
            else_block: Box::new(Block {
                stmts: Vec::new(),
                tail: Tail::TailLocal {
                    callee: edge.callee,
                    args: edge.args.clone(),
                },
            }),
            span,
        },
    }
}

fn is_try_bind(stmt: &Stmt) -> bool {
    matches!(stmt, Stmt::TryBind { .. })
}

/// Whether any `TryBind` survives anywhere in a guarded block — its own
/// statements, a `ShortCircuit`'s nested right-hand burst, or either arm of
/// any tail.
fn residual_fallible_block(block: &Block) -> bool {
    block.stmts.iter().any(residual_fallible_stmt) || residual_fallible_tail(&block.tail)
}

fn residual_fallible_stmt(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::TryBind { .. } => true,
        Stmt::ShortCircuit { rhs_stmts, .. } => rhs_stmts.iter().any(residual_fallible_stmt),
        Stmt::Attempt { on_ok, on_err, .. } => {
            residual_fallible_block(on_ok) || residual_fallible_block(on_err)
        }
        _ => false,
    }
}

fn residual_fallible_tail(tail: &Tail) -> bool {
    match tail {
        Tail::If {
            then_block,
            else_block,
            ..
        } => residual_fallible_block(then_block) || residual_fallible_block(else_block),
        Tail::SelectEnum { arms, .. } => {
            arms.iter().any(|(_, block)| residual_fallible_block(block))
        }
        Tail::Return(_) | Tail::TailLocal { .. } | Tail::TailRt { .. } => false,
    }
}

/// Build the lifted compensation `FlowFn` and record it against its reserved
/// slot. Its parameters are the guarded step's `avail_in`, in the step
/// function's own parameter order, so the edge's tail call is a plain
/// forwarding of the step's entry values.
pub(super) fn lower_compensation(
    ctx: &mut Ctx<'_>,
    env: FlowEnv<'_>,
    step: &Step,
    on_failure: &OnFailure,
    param_names: &[String],
    edge: &FailureEdge,
    slots: &mut Slots,
) -> Result<(), LowerError> {
    let flow = env.flow;
    let mut scope: Scope = Scope::new();
    let mut params: Vec<Var> = Vec::with_capacity(param_names.len());
    let mut param_tys = Vec::with_capacity(param_names.len());
    for name in param_names {
        let ty = flow.bindings.get(name).cloned().ok_or_else(|| {
            LowerError::new(step.name_span, format!("binding `{name}` has no type"))
        })?;
        let var = ctx.fresh_var();
        params.push(var);
        param_tys.push(ctx.tydesc(&ty));
        scope.insert(name.clone(), Binding { var, ty });
    }

    let mut stmts = Vec::new();
    let mut tail = None;
    for statement in &on_failure.body {
        if let Some(found) =
            lower_statement(ctx, env, step, statement, &mut scope, &mut stmts, slots)?
        {
            tail = Some(found);
            break;
        }
    }
    // The checker requires an `on failure` block to end in a route, and the
    // reference emitter refuses one that does not; a block that reached here
    // without producing a tail would leave the compensation with nowhere to
    // go, so refuse rather than invent a continuation.
    let tail = tail.ok_or_else(|| {
        LowerError::new(
            on_failure.span,
            format!(
                "the `on failure` block of step `{}` does not end in a route — the document \
                 did not check cleanly",
                step.name
            ),
        )
    })?;

    let function = MirFn::Flow(FlowFn {
        origin: FnOrigin::Compensation {
            step: flow.origin_step(&step.name),
        },
        name: format!("{}{}_on_failure", flow.prefix, snake(&step.name)),
        params,
        param_tys,
        ret_ty: flow.ret_ty.clone(),
        body: Block { stmts, tail },
        span: Span::from_source(on_failure.span),
        degraded_parallel: false,
    });
    slots.compensations.finish(edge.ordinal, function);
    Ok(())
}
