//! `send` statement lowering — the MIR twin of the reference
//! `emitter/sends.rs`.
//!
//! Shape: `CallLocal {sig}_send_signal()` for the typed reference, then
//! `aion@signal:send/3` on `(target, ref, payload)`, then
//! `awl_error.map_engine_error` on the `Result(Nil, EngineError)` it
//! returns, then `TryBind` — which is what makes a failed delivery fault the
//! step and reach the step's `on failure` edge instead of passing quietly.
//! The bound value is `Nil` and nothing names it: a send produces no value.
//!
//! No slots are reserved and no lifted functions are built, so a `send` never
//! disturbs the reserved-`FnRef` layout the loop/fork/wait pools depend on.

use crate::ast::{RoutePayload, SendStmt};
use crate::emitter::{GType, snake};

use super::super::ops::{LiveAfter, Stmt, Value};
use super::super::runtime::RuntimeFn;
use super::activity::{call_rt, record_new};
use super::build::FnPlan;
use super::ctx::Ctx;
use super::driver::LowerError;
use super::expr::{Scope, lower_arg_for};

pub(super) fn lower_send_stmt(
    ctx: &mut Ctx<'_>,
    plan: &FnPlan,
    send: &SendStmt,
    scope: &Scope,
    stmts: &mut Vec<Stmt>,
) -> Result<(), LowerError> {
    let decl = ctx.emitter.sends.get(send.signal.as_str()).ok_or_else(|| {
        LowerError::new(
            send.signal_span,
            format!("`{}` names no declared outbound signal", send.signal),
        )
    })?;
    let payload_ty = crate::emitter::type_ref_to_g(&decl.ty);
    let signal_ref =
        *plan
            .send_signals
            .get(send.signal.as_str())
            .ok_or_else(|| LowerError::Planning {
                message: format!("outbound signal `{}` has no planned shell", send.signal),
            })?;

    let target = lower_arg_for(ctx, &send.target, &GType::Str, scope, stmts)?;
    let payload = lower_payload(ctx, send, &payload_ty, scope, stmts)?;

    let sig = ctx.fresh_var();
    stmts.push(Stmt::CallLocal {
        dst: Some(sig),
        callee: signal_ref,
        args: Vec::new(),
        live_after: LiveAfter::default(),
        span: super::super::ids::Span::from_source(send.signal_span),
    });
    // SDK arg order: workflow id, signal reference, payload
    // (`aion/signal.gleam`).
    let delivered = call_rt(
        ctx,
        RuntimeFn::SignalSend,
        vec![target, Value::Var(sig), payload],
        stmts,
        send.span,
    );
    let mapped = call_rt(
        ctx,
        RuntimeFn::MapEngineError,
        vec![Value::Var(delivered)],
        stmts,
        send.span,
    );
    let discarded = ctx.fresh_var();
    stmts.push(Stmt::TryBind {
        dst: discarded,
        result: mapped,
        live_after: LiveAfter::default(),
        span: super::super::ids::Span::from_source(send.span),
    });
    Ok(())
}

/// Build the payload value in whichever of the three forms the author wrote —
/// the same three the reference backend renders.
fn lower_payload(
    ctx: &mut Ctx<'_>,
    send: &SendStmt,
    payload_ty: &GType,
    scope: &Scope,
    stmts: &mut Vec<Stmt>,
) -> Result<Value, LowerError> {
    match &send.payload {
        Some(RoutePayload::Value(value)) => lower_arg_for(ctx, value, payload_ty, scope, stmts),
        Some(RoutePayload::Args(args)) => {
            let Some((gleam_name, record)) = ctx.emitter.env.record_of(payload_ty) else {
                return Err(LowerError::new(
                    send.signal_span,
                    format!(
                        "signal `{}` carries a payload that cannot take named fields",
                        send.signal
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
                    // payload treats one.
                    None if matches!(ctx.emitter.env.resolve(&field.ty), GType::Option(_)) => {
                        Value::Atom(ctx.atom("none"))
                    }
                    None => {
                        return Err(LowerError::new(
                            send.span,
                            format!(
                                "the payload of signal `{}` misses field `{}`",
                                send.signal, field.awl_name
                            ),
                        ));
                    }
                };
                values.push(value);
            }
            Ok(Value::Var(record_new(ctx, tag, values, stmts)))
        }
        None => {
            if matches!(ctx.emitter.env.resolve(payload_ty), GType::Nil) {
                return Ok(Value::Nil);
            }
            Err(LowerError::new(
                send.signal_span,
                format!("signal `{}` needs a constructed payload", send.signal),
            ))
        }
    }
}
