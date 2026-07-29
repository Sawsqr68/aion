//! Declarative action bodies: command hygiene and parameter references.
//!
//! An action with no body is a requirement on an out-of-band worker and has
//! nothing to check here. An action WITH a body is the implementation, so
//! the command must name a program to execute, may only interpolate
//! parameters the action declares, and must never let a caller-supplied
//! value decide WHICH program runs.
//!
//! Every refusal here is one the worker SDK's executor would otherwise
//! raise at registration or at render time. Ruling on them at check time is
//! the point: a document that checks clean cannot fail on the way to the
//! process.

use crate::Span;
use crate::ast::{ActionBody, ActionDecl, TypeRef, scan_command};

use super::context::Ctx;
use super::types::{Ty, resolve};

/// The members of the outcome object a `run` body always produces. A bodied
/// action's declared result may name any subset of these, and nothing else:
/// the runtime hands back this object and only this object, so a member the
/// executor never produces would decode to nothing at all.
const OUTCOME_MEMBERS: &[(&str, Ty)] = &[
    ("exit_code", Ty::Int),
    ("stdout", Ty::Str),
    ("stderr", Ty::Str),
];

/// Check one action's declarative body, if it has one.
pub(super) fn check(ctx: &mut Ctx<'_>, action: &ActionDecl, returns: &Ty) {
    match &action.body {
        None => {}
        Some(ActionBody::Run {
            command,
            command_span,
            ..
        }) => {
            check_run(ctx, action, command, *command_span);
            check_run_result(ctx, action, returns);
        }
    }
}

/// A `run` body always produces `{ exit_code: Int, stdout: String, stderr:
/// String }`. The declared result must therefore be a record naming a subset
/// of those three, with their types — anything else would check clean and
/// then hand the workflow an object that does not fit it.
fn check_run_result(ctx: &mut Ctx<'_>, action: &ActionDecl, returns: &Ty) {
    let anchor = type_ref_span(&action.returns);
    let resolved = resolve(returns, &ctx.types);
    let Ty::Record(record) = resolved else {
        // `Unknown` means the type name was already reported as undeclared;
        // a second diagnostic for the same mistake is noise.
        if !matches!(resolved, Ty::Unknown) {
            ctx.error(
                anchor,
                format!(
                    "action `{}` declares a `run` body, so it returns the command's outcome \
                     object; its result type must be a record over {}, not `{resolved}`",
                    action.name,
                    available()
                ),
            );
        }
        return;
    };
    for field in &record.fields {
        let anchor = field.declaration.unwrap_or(anchor);
        let Some((_, expected)) = OUTCOME_MEMBERS.iter().find(|(name, _)| *name == field.name)
        else {
            ctx.error(
                anchor,
                format!(
                    "`{}` is not part of the outcome a `run` body produces, so action `{}` \
                     could never receive it; the available members are {}",
                    field.name,
                    action.name,
                    available()
                ),
            );
            continue;
        };
        if !super::types::same_ty(&field.ty, expected, &ctx.types) {
            ctx.error(
                anchor,
                format!(
                    "the outcome member `{}` a `run` body produces is `{expected}`, but \
                     action `{}` declares it as `{}`",
                    field.name, action.name, field.ty
                ),
            );
        }
    }
}

/// The outcome members, rendered for a diagnostic.
fn available() -> String {
    OUTCOME_MEMBERS
        .iter()
        .map(|(name, ty)| format!("`{name}: {ty}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The source span of a declared type reference.
const fn type_ref_span(ty: &TypeRef) -> Span {
    match ty {
        TypeRef::Named { span, .. }
        | TypeRef::List { span, .. }
        | TypeRef::Optional { span, .. } => *span,
    }
}

fn check_run(ctx: &mut Ctx<'_>, action: &ActionDecl, command: &str, literal: Span) {
    if command.trim().is_empty() {
        ctx.error(
            literal,
            format!(
                "action `{}` declares an empty `run` command; a declarative body must name \
                 a program to execute",
                action.name
            ),
        );
        return;
    }
    let scanned = match scan_command(command) {
        Ok(scanned) => scanned,
        Err(error) => {
            ctx.error(
                sub_span(literal, command, error.offset, error.len),
                error.message,
            );
            return;
        }
    };
    for reference in &scanned.references {
        let span = sub_span(literal, command, reference.offset, reference.len);
        if reference.offset < scanned.program_end {
            ctx.error(
                span,
                format!(
                    "the program a `run` command executes must be written out, not the \
                     parameter `{name}`: a caller-supplied value must never choose which \
                     program runs, so move `{name}` into an argument",
                    name = reference.name
                ),
            );
            continue;
        }
        if action
            .params
            .iter()
            .any(|param| param.name == reference.name)
        {
            continue;
        }
        ctx.error(
            span,
            format!(
                "`run` command references `${}`, which action `{}` does not declare; its \
                 parameters are {}",
                reference.name,
                action.name,
                declared(action)
            ),
        );
    }
}

/// The action's declared parameter names, rendered for a diagnostic.
fn declared(action: &ActionDecl) -> String {
    if action.params.is_empty() {
        return "none".to_owned();
    }
    action
        .params
        .iter()
        .map(|param| format!("`{}`", param.name))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Anchor a diagnostic inside the command literal.
///
/// When the literal carried no escape sequence its source bytes correspond
/// one-to-one with the command text after the opening quote, so the exact
/// sub-range is recoverable; when it did, the byte positions have shifted
/// and the whole literal is the honest anchor. A string literal never spans
/// a line (`\n` is an escape, not a raw newline), so the line is the
/// literal's own either way.
fn sub_span(literal: Span, command: &str, offset: usize, len: usize) -> Span {
    if literal.end.saturating_sub(literal.start) != command.len() + 2 {
        return literal;
    }
    let Some(prefix) = command.get(..offset) else {
        return literal;
    };
    let start = literal.start + 1 + offset;
    Span {
        start,
        end: start + len,
        line: literal.line,
        column: literal.column + 1 + prefix.chars().count(),
    }
}
