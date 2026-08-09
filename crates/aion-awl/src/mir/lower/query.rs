//! `answer` statement lowering — the MIR twin of the reference
//! `emitter/queries.rs`.
//!
//! Shape at the statement site: build the answer VALUE from the bindings in
//! scope, `MakeClosure` a nullary reply thunk capturing it, `CallLocal` the
//! per-answer registrar, then `TryBind` — which is what makes a refused
//! registration fault the step and reach the step's `on failure` edge instead
//! of leaving a declared query silently unanswerable. The bound value is `Nil`
//! and nothing names it: an answer produces no value.
//!
//! Two lifted functions per statement, both drawn from the DYNAMIC pool at the
//! tail of the module (the pool collection predicates already use), so an
//! `answer` never disturbs the reserved-`FnRef` layout the loop/fork/wait/
//! compensation pools depend on. Each is named by its OWN position in that
//! pool, so a pair reads as consecutive ordinals rather than a shared one:
//!
//!   * `awl_answer_reply_<n>(arguments, value) -> value` — the body of
//!     `fn(_) { value }`. Its LAST parameter is the capture, appended by
//!     `MakeClosure` at creation (S9); the leading one is the caller's decoded
//!     query arguments, so its callable arity is one, which is the arity
//!     `aion@query:handler/4` invokes a reply at. An AWL `answer` declares no
//!     arguments, so the parameter is bound and ignored.
//!   * `awl_answer_<n>(reply) -> Result(Nil, AwlError)` — calls
//!     `aion@query:handler/4` with the query name, the unit arguments codec,
//!     the answer type's codec, and the thunk, then discriminates its `Result`
//!     in place. The registrar exists because a `case` is a TAIL in this IR
//!     and a statement site has no tail to spend; lifting it needs no new
//!     instruction selection.
//!
//! The error arm collapses to `AwlFailed`, matching the reference backend
//! exactly. `register_query` (`aion/src/runtime/nif_query.rs`) fails only when
//! the caller is not a workflow process or the handler registry lock is
//! poisoned — the workflow-context failure class the SDK's own
//! `map_engine_error` already collapses to `AwlFailed`.

use crate::ast::{AnswerStmt, RoutePayload};
use crate::emitter::{GType, snake, type_ref_to_g};

use super::super::func::{CodecRef, FlowFn, FnOrigin, MirFn};
use super::super::ids::Span;
use super::super::ops::{Block, LiveAfter, Stmt, Tail, Test, Value};
use super::super::runtime::RuntimeFn;
use super::super::tydesc::TyDesc;
use super::activity::record_new;
use super::build::{FnPlan, codec_ref_for};
use super::child_call::codec_value;
use super::ctx::Ctx;
use super::driver::LowerError;
use super::expr::{Scope, lower_arg_for};

pub(super) fn lower_answer_stmt(
    ctx: &mut Ctx<'_>,
    plan: &FnPlan,
    answer: &AnswerStmt,
    scope: &Scope,
    stmts: &mut Vec<Stmt>,
) -> Result<(), LowerError> {
    let decl = ctx
        .emitter
        .queries
        .get(answer.query.as_str())
        .ok_or_else(|| {
            LowerError::new(
                answer.query_span,
                format!("`{}` names no declared query", answer.query),
            )
        })?;
    let answer_ty = type_ref_to_g(&decl.ty);
    let value = lower_value(ctx, answer, &answer_ty, scope, stmts)?;

    let (reply_ordinal, reply_ref) = ctx.take_dynamic()?;
    let (registrar_ordinal, registrar_ref) = ctx.take_dynamic()?;
    // Each lifted function is its own SSA namespace, built mid-way through the
    // host body: save the host counter, build EACH from `v0` (so every
    // function's parameters are its lowest vars, which is what register
    // allocation reads), then restore.
    let saved = ctx.swap_var_counter(0);
    let reply = build_reply_fn(ctx, answer, &answer_ty, reply_ordinal);
    ctx.swap_var_counter(0);
    let registrar = build_registrar_fn(ctx, plan, answer, &answer_ty, registrar_ordinal);
    ctx.swap_var_counter(saved);
    ctx.finish_dynamic(reply_ordinal, MirFn::Flow(reply));
    ctx.finish_dynamic(registrar_ordinal, MirFn::Flow(registrar?));

    let span = Span::from_source(answer.span);
    let thunk = ctx.fresh_var();
    stmts.push(Stmt::MakeClosure {
        dst: thunk,
        lifted: reply_ref,
        captures: vec![value],
        span,
    });
    let registered = ctx.fresh_var();
    stmts.push(Stmt::CallLocal {
        dst: Some(registered),
        callee: registrar_ref,
        args: vec![Value::Var(thunk)],
        live_after: LiveAfter::default(),
        span,
    });
    let discarded = ctx.fresh_var();
    stmts.push(Stmt::TryBind {
        dst: discarded,
        result: registered,
        live_after: LiveAfter::default(),
        span,
    });
    Ok(())
}

/// `fn(_) { value }`: the caller's arguments in and ignored, one capture in,
/// that capture out. The arguments parameter comes first because
/// `MakeClosure` appends captures after a lifted function's callable
/// parameters.
fn build_reply_fn(
    ctx: &mut Ctx<'_>,
    answer: &AnswerStmt,
    answer_ty: &GType,
    ordinal: usize,
) -> FlowFn {
    let arguments = ctx.fresh_var();
    let captured = ctx.fresh_var();
    let ty = ctx.tydesc(answer_ty);
    FlowFn {
        origin: FnOrigin::QueryAnswer {
            query: answer.query.clone(),
            index: u32::try_from(ordinal).unwrap_or(u32::MAX),
            reply: true,
        },
        name: format!("awl_answer_reply_{ordinal}"),
        params: vec![arguments, captured],
        param_tys: vec![TyDesc::Nil, ty.clone()],
        ret_ty: ty,
        body: Block {
            stmts: Vec::new(),
            tail: Tail::Return(Value::Var(captured)),
        },
        span: Span::from_source(answer.span),
        degraded_parallel: false,
    }
}

/// `fn(reply) { case query.handler(name, nil_codec(), codec, reply) {
/// Ok(_) -> Ok(Nil) Error(_) -> Error(AwlFailed) } }`.
fn build_registrar_fn(
    ctx: &mut Ctx<'_>,
    plan: &FnPlan,
    answer: &AnswerStmt,
    answer_ty: &GType,
    ordinal: usize,
) -> Result<FlowFn, LowerError> {
    let span = Span::from_source(answer.span);
    let reply = ctx.fresh_var();
    let mut stmts = Vec::new();
    let name_lit = ctx.binary(&answer.query);
    // An AWL `answer` takes no arguments, so the handler's arguments codec is
    // the SDK unit codec; the reply thunk binds and ignores what it decodes.
    let arguments_codec = codec_value(ctx, &CodecRef::SdkNil, &mut stmts, answer.span);
    let codec_ref = codec_ref_for(ctx, plan, answer_ty)?;
    let codec = codec_value(ctx, &codec_ref, &mut stmts, answer.span);
    let registered = ctx.fresh_var();
    stmts.push(Stmt::CallRt {
        dst: Some(registered),
        callee: RuntimeFn::QueryHandler,
        args: vec![
            Value::Lit(name_lit),
            Value::Var(arguments_codec),
            Value::Var(codec),
            Value::Var(reply),
        ],
        live_after: LiveAfter::default(),
        span,
    });
    let ok = ctx.atom("ok");
    let registered_block = ok_nil_block(ctx, span);
    let refused_block = failed_block(ctx, span);
    // The reply is called with the decoded arguments, so its callable type is
    // arity one — matching the leading parameter `build_reply_fn` adds.
    let reply_ty = TyDesc::Fn(vec![TyDesc::Nil], Box::new(ctx.tydesc(answer_ty)));
    Ok(FlowFn {
        origin: FnOrigin::QueryAnswer {
            query: answer.query.clone(),
            index: u32::try_from(ordinal).unwrap_or(u32::MAX),
            reply: false,
        },
        name: format!("awl_answer_{ordinal}"),
        params: vec![reply],
        param_tys: vec![reply_ty],
        ret_ty: TyDesc::Result(Box::new(TyDesc::Nil), Box::new(TyDesc::AwlError)),
        body: Block {
            stmts,
            tail: Tail::If {
                test: Test::IsTagged {
                    value: Value::Var(registered),
                    tag: ok,
                    arity: 2,
                },
                then_block: Box::new(registered_block),
                else_block: Box::new(refused_block),
                span,
            },
        },
        span,
        degraded_parallel: false,
    })
}

/// `Ok(Nil)` — registration succeeded and the statement produces no value.
fn ok_nil_block(ctx: &mut Ctx<'_>, span: Span) -> Block {
    let mut stmts = Vec::new();
    let ok = ctx.atom("ok");
    let dst = ctx.fresh_var();
    stmts.push(Stmt::RecordNew {
        dst,
        tag: ok,
        args: vec![Value::Nil],
        span,
    });
    Block {
        stmts,
        tail: Tail::Return(Value::Var(dst)),
    }
}

/// `Error(awl_error.AwlFailed)` — the step faults, and the failure reaches an
/// `on failure` block exactly like a refused send.
fn failed_block(ctx: &mut Ctx<'_>, span: Span) -> Block {
    let mut stmts = Vec::new();
    let failed = ctx.atom("awl_failed");
    let error = ctx.atom("error");
    let dst = ctx.fresh_var();
    stmts.push(Stmt::RecordNew {
        dst,
        tag: error,
        args: vec![Value::Atom(failed)],
        span,
    });
    Block {
        stmts,
        tail: Tail::Return(Value::Var(dst)),
    }
}

/// Build the answer value in whichever of the three forms the author wrote —
/// the same three the reference backend renders.
fn lower_value(
    ctx: &mut Ctx<'_>,
    answer: &AnswerStmt,
    answer_ty: &GType,
    scope: &Scope,
    stmts: &mut Vec<Stmt>,
) -> Result<Value, LowerError> {
    match &answer.value {
        Some(RoutePayload::Value(value)) => lower_arg_for(ctx, value, answer_ty, scope, stmts),
        Some(RoutePayload::Args(args)) => {
            let Some((gleam_name, record)) = ctx.emitter.env.record_of(answer_ty) else {
                return Err(LowerError::new(
                    answer.query_span,
                    format!(
                        "query `{}` answers a type that cannot take named fields",
                        answer.query
                    ),
                ));
            };
            let fields = record.fields.clone();
            let tag = ctx.atom(&snake(&gleam_name));
            if fields.is_empty() {
                return Ok(Value::Atom(tag));
            }
            let mut values = Vec::new();
            for field in &fields {
                let value = match args.iter().find(|arg| arg.name == field.awl_name) {
                    Some(arg) => lower_arg_for(ctx, &arg.value, &field.ty, scope, stmts)?,
                    // An absent optional field is `None`, exactly as a route
                    // payload and a send payload treat one.
                    None if matches!(ctx.emitter.env.resolve(&field.ty), GType::Option(_)) => {
                        Value::Atom(ctx.atom("none"))
                    }
                    None => {
                        return Err(LowerError::new(
                            answer.span,
                            format!(
                                "the answer of query `{}` misses field `{}`",
                                answer.query, field.awl_name
                            ),
                        ));
                    }
                };
                values.push(value);
            }
            Ok(Value::Var(record_new(ctx, tag, values, stmts)))
        }
        None => {
            if matches!(ctx.emitter.env.resolve(answer_ty), GType::Nil) {
                return Ok(Value::Nil);
            }
            Err(LowerError::new(
                answer.query_span,
                format!("query `{}` needs a constructed answer", answer.query),
            ))
        }
    }
}
