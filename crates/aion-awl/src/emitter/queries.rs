//! `answer` lowering for the reference (Gleam-source) backend.
//!
//! One `query.handler(<name>, awlc.nil_codec(), <answer codec>, fn(_) { … })`,
//! its `Result(Nil, QueryError)` discriminated in place and threaded through
//! `result.try` like every other fallible statement. That threading IS the
//! failure contract — a registration the engine refuses faults the step and
//! reaches the step's `on failure` block, rather than leaving a declared query
//! silently unanswerable for the rest of the run.
//!
//! The closure captures a VALUE, not state: the answer expression is evaluated
//! here, at the statement, over the bindings in scope. Registering a value is
//! what keeps the surface deterministic — nothing is evaluated at an arbitrary
//! yield point, so replay re-runs the same statement and re-registers the same
//! answer. An `answer` inside a loop re-registers each pass, which is exactly
//! how a run reports live progress.
//!
//! The error side collapses to `AwlFailed` rather than a query-specific
//! variant, and that is a deliberate reading of what can actually fail here.
//! `register_query` (`aion/src/runtime/nif_query.rs`) fails only when the
//! caller is not a workflow process, or when the handler registry lock is
//! poisoned — the workflow-context failure class the SDK's own
//! `map_engine_error` already collapses to `AwlFailed`. Inventing a new
//! taxonomy variant for the same class would be a second spelling of one
//! failure, not more information.

use crate::ast::{AnswerStmt, RoutePayload};

use super::context::Emitter;
use super::error::EmitError;
use super::exprs::{Scope, render_arg_for};
use super::names::string_lit;
use super::stmts::flush_prelude;
use super::types::{GType, type_ref_to_g};

pub(super) fn lower_answer(
    emitter: &mut Emitter<'_>,
    answer: &AnswerStmt,
    scope: &Scope,
) -> Result<(), EmitError> {
    let Some(&decl) = emitter.queries.get(answer.query.as_str()) else {
        return Err(EmitError::new(
            answer.query_span,
            format!("`{}` names no declared query", answer.query),
        ));
    };
    let answer_ty = type_ref_to_g(&decl.ty);
    let codec = emitter.codec_fn(&answer_ty);
    let mut prelude = Vec::new();
    let value = render_value(emitter, answer, &answer_ty, scope, &mut prelude)?;
    flush_prelude(emitter, prelude);
    emitter.flags.uses_query_module = true;
    // An AWL `answer` declares no arguments, so the handler's arguments codec
    // is the SDK's unit codec and the caller's document is ignored. Passing a
    // codec rather than omitting the parameter is what keeps a declared query
    // answerable at all: the handler contract carries arguments in every case.
    emitter.line(&format!(
        "use _ <- result.try(case query.handler({}, awlc.nil_codec(), {codec}(), \
         fn(_) {{ {value} }}) {{ \
         Ok(_) -> Ok(Nil) Error(_) -> Error(awl_error.AwlFailed) }})",
        string_lit(&answer.query)
    ));
    Ok(())
}

/// Render the answer value in whichever of the three forms the author wrote.
/// The checker has already ruled on shape, so a mismatch here is a checker
/// defect and is reported as one rather than silently coerced.
fn render_value(
    emitter: &mut Emitter<'_>,
    answer: &AnswerStmt,
    answer_ty: &GType,
    scope: &Scope,
    prelude: &mut Vec<String>,
) -> Result<String, EmitError> {
    match &answer.value {
        Some(RoutePayload::Value(value)) => {
            render_arg_for(emitter, value, answer_ty, scope, prelude)
        }
        Some(RoutePayload::Args(args)) => {
            let Some((gleam_name, record)) = emitter.env.record_of(answer_ty) else {
                return Err(EmitError::new(
                    answer.query_span,
                    format!(
                        "query `{}` answers {}, which cannot take named fields",
                        answer.query,
                        emitter.env.gleam_type(answer_ty)
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
                    // payload and a send payload treat one.
                    None if matches!(emitter.env.resolve(&field.ty), GType::Option(_)) => {
                        "None".to_owned()
                    }
                    None => {
                        return Err(EmitError::new(
                            answer.span,
                            format!(
                                "the answer of query `{}` misses field `{}`",
                                answer.query, field.awl_name
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
            if matches!(emitter.env.resolve(answer_ty), GType::Nil) {
                return Ok("Nil".to_owned());
            }
            Err(EmitError::new(
                answer.query_span,
                format!(
                    "query `{}` answers {} and needs a constructed value",
                    answer.query,
                    emitter.env.gleam_type(answer_ty)
                ),
            ))
        }
    }
}
