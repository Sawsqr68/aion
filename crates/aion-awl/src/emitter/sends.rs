//! `send` lowering for the reference (Gleam-source) backend.
//!
//! One line: `signal.send(<target>, <name>_send_signal(), <payload>)`, its
//! `Result(Nil, EngineError)` mapped into the AWL error type and threaded
//! through `result.try` like every other fallible statement. That threading
//! IS the failure contract — a failed delivery faults the step and reaches
//! the step's `on failure` block, rather than passing quietly.
//!
//! Delivery itself is the SDK's `signal.send`, which the engine records as a
//! correlated `SendSignal` command; replay resolves the recorded occurrence
//! instead of sending again. Nothing about delivery is reimplemented here.

use crate::ast::{RoutePayload, SendStmt};

use super::context::Emitter;
use super::error::EmitError;
use super::exprs::{Scope, render_arg_for};
use super::names::snake;
use super::stmts::flush_prelude;
use super::types::{GType, type_ref_to_g};

pub(super) fn lower_send(
    emitter: &mut Emitter<'_>,
    send: &SendStmt,
    scope: &Scope,
) -> Result<(), EmitError> {
    let Some(&decl) = emitter.sends.get(send.signal.as_str()) else {
        return Err(EmitError::new(
            send.signal_span,
            format!("`{}` names no declared outbound signal", send.signal),
        ));
    };
    let payload_ty = type_ref_to_g(&decl.ty);
    let mut prelude = Vec::new();
    let target = render_arg_for(emitter, &send.target, &GType::Str, scope, &mut prelude)?;
    let payload = render_payload(emitter, send, &payload_ty, scope, &mut prelude)?;
    flush_prelude(emitter, prelude);
    emitter.line(&format!(
        "use _ <- result.try(signal.send({target}, {}_send_signal(), {payload}) \
         |> awl_error.map_engine_error)",
        snake(&send.signal)
    ));
    Ok(())
}

/// Render the payload value in whichever of the three forms the author
/// wrote. The checker has already ruled on shape, so a mismatch here is a
/// checker defect and is reported as one rather than silently coerced.
fn render_payload(
    emitter: &mut Emitter<'_>,
    send: &SendStmt,
    payload_ty: &GType,
    scope: &Scope,
    prelude: &mut Vec<String>,
) -> Result<String, EmitError> {
    match &send.payload {
        Some(RoutePayload::Value(value)) => {
            render_arg_for(emitter, value, payload_ty, scope, prelude)
        }
        Some(RoutePayload::Args(args)) => {
            let Some((gleam_name, record)) = emitter.env.record_of(payload_ty) else {
                return Err(EmitError::new(
                    send.signal_span,
                    format!(
                        "signal `{}` carries {}, which cannot take named payload fields",
                        send.signal,
                        emitter.env.gleam_type(payload_ty)
                    ),
                ));
            };
            let fields = record.fields.clone();
            if fields.is_empty() {
                return Ok(gleam_name);
            }
            let mut rendered = format!("{gleam_name}(");
            for (position, field) in fields.iter().enumerate() {
                if position > 0 {
                    rendered.push_str(", ");
                }
                let value = match args.iter().find(|arg| arg.name == field.awl_name) {
                    Some(arg) => render_arg_for(emitter, &arg.value, &field.ty, scope, prelude)?,
                    // An absent optional field is `None`, exactly as a route
                    // payload treats one.
                    None if matches!(emitter.env.resolve(&field.ty), GType::Option(_)) => {
                        "None".to_owned()
                    }
                    None => {
                        return Err(EmitError::new(
                            send.span,
                            format!(
                                "the payload of signal `{}` misses field `{}`",
                                send.signal, field.awl_name
                            ),
                        ));
                    }
                };
                rendered.push_str(&value);
            }
            rendered.push(')');
            Ok(rendered)
        }
        None => {
            if matches!(emitter.env.resolve(payload_ty), GType::Nil) {
                return Ok("Nil".to_owned());
            }
            Err(EmitError::new(
                send.signal_span,
                format!(
                    "signal `{}` carries {} and needs a constructed payload",
                    send.signal,
                    emitter.env.gleam_type(payload_ty)
                ),
            ))
        }
    }
}
