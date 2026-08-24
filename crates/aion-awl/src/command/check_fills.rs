//! The fill rules: hole resolution, the type rule, and the derived-literal
//! refusal.
//!
//! # The type rule, stated once
//!
//! A fill that is EXACTLY one `{{parameter}}` carries that parameter's own
//! type through. A fill that mixes literal text with interpolations is a
//! `String`, because text with a value written into it is text. That is the
//! whole rule, and it is what makes `"{{count}}"` an `Int` while
//! `"n={{count}}"` is not — no inference, no coercion, no widening.
//!
//! A list is the one type that does not produce a single argv element, so it
//! stands in exactly one position: alone in an operand, where it emits one
//! element per item. That position is the absorbed variadic tail, typed.

use crate::ast::TypeRef;

use super::cause::CommandCause;
use super::decl::{CommandDecl, TypedFill};
use super::derived::derived_shape_in;
use super::error::CommandRefusal;
use super::fill::Fill;

/// Every fill: its holes name declared parameters, its type agrees with the
/// position it fills, and no literal stands where a derivation belongs.
pub(super) fn check_fills(decl: &CommandDecl, refusals: &mut Vec<CommandRefusal>) {
    for option in &decl.options {
        let Some(value) = &option.value else {
            continue;
        };
        check_typed_fill(
            decl,
            value,
            &format!("option `{}`", option.flag),
            false,
            refusals,
        );
    }
    for operand in &decl.operands {
        check_typed_fill(
            decl,
            &operand.value,
            &format!("operand `{}`", operand.name),
            true,
            refusals,
        );
    }
    for binding in &decl.env {
        check_string_fill(
            decl,
            &binding.fill,
            &format!("environment binding `{}`", binding.name),
            refusals,
        );
    }
    if let Some(block) = decl.hardening.first()
        && let Some(path) = &block.path
    {
        check_string_fill(decl, &path.fill, "the hardened PATH", refusals);
    }
}

/// A typed fill: hole resolution, then the type rule.
fn check_typed_fill(
    decl: &CommandDecl,
    value: &TypedFill,
    what: &str,
    list_position: bool,
    refusals: &mut Vec<CommandRefusal>,
) {
    if !check_holes_are_declared(decl, &value.fill, what, refusals) {
        return;
    }
    check_literal_derivations(&value.fill, refusals, what);

    if let Some(name) = value.fill.sole_hole() {
        check_sole_hole(decl, value, name, what, list_position, refusals);
    } else {
        check_mixed_fill(decl, value, what, refusals);
    }
}

/// A fill that is EXACTLY one hole: it carries its parameter's own type
/// through, and it is the one position a list may stand in.
fn check_sole_hole(
    decl: &CommandDecl,
    value: &TypedFill,
    name: &str,
    what: &str,
    list_position: bool,
    refusals: &mut Vec<CommandRefusal>,
) {
    let declared = type_name(&value.ty);

    let Some(parameter) = decl.parameters.iter().find(|item| item.name == name) else {
        return;
    };
    let supplied = type_name(&parameter.ty);
    if supplied != declared {
        refusals.push(CommandRefusal::refuse(
            CommandCause::ParameterTypeMismatch,
            value.fill.span,
            &value.fill.source,
            format!(
                "{what} is declared `{declared}` and is filled by parameter `{name}`, \
                 which is declared `{supplied}`"
            ),
        ));
    }
    if is_list(&parameter.ty) && !list_position {
        refusals.push(CommandRefusal::refuse(
            CommandCause::ListInScalarPosition,
            value.fill.span,
            &value.fill.source,
            format!(
                "{what} is filled by the list parameter `{name}`, and a list has no \
                 single argv element to be there; a list stands alone in an OPERAND, \
                 where it emits one element per item — the variadic tail"
            ),
        ));
    }
}

/// A fill mixing literal text with interpolations: it is a `String`, because
/// text with a value written into it is text.
fn check_mixed_fill(
    decl: &CommandDecl,
    value: &TypedFill,
    what: &str,
    refusals: &mut Vec<CommandRefusal>,
) {
    let declared = type_name(&value.ty);

    if declared != "String" {
        refusals.push(CommandRefusal::refuse(
            CommandCause::ParameterTypeMismatch,
            value.fill.span,
            &value.fill.source,
            format!(
                "{what} is declared `{declared}`, but its fill mixes literal text with \
                 its interpolations, and text with a value written into it is a `String`. \
                 Write the fill as exactly one `{{{{parameter}}}}` to carry that \
                 parameter's own type through"
            ),
        ));
    }
    for (name, span) in value.fill.holes() {
        if decl
            .parameters
            .iter()
            .any(|item| item.name == name && is_list(&item.ty))
        {
            refusals.push(CommandRefusal::refuse(
                CommandCause::ListInScalarPosition,
                span,
                &value.fill.source,
                format!(
                    "{what} writes the list parameter `{name}` into a larger word, and a \
                     list has no rendering as part of one"
                ),
            ));
        }
    }
}

/// An untyped fill that must produce one string: env values and the hardened
/// PATH, both of which the operating system reads as text whatever AWL thinks.
fn check_string_fill(
    decl: &CommandDecl,
    fill: &Fill,
    what: &str,
    refusals: &mut Vec<CommandRefusal>,
) {
    if !check_holes_are_declared(decl, fill, what, refusals) {
        return;
    }
    check_literal_derivations(fill, refusals, what);
    for (name, span) in fill.holes() {
        if decl
            .parameters
            .iter()
            .any(|item| item.name == name && is_list(&item.ty))
        {
            refusals.push(CommandRefusal::refuse(
                CommandCause::ListInScalarPosition,
                span,
                &fill.source,
                format!(
                    "{what} is filled by the list parameter `{name}`; an environment value and a \
                     PATH are single strings, and a list has no single rendering as one"
                ),
            ));
        }
    }
}

/// Every hole names a declared parameter. Returns whether all of them did.
fn check_holes_are_declared(
    decl: &CommandDecl,
    fill: &Fill,
    what: &str,
    refusals: &mut Vec<CommandRefusal>,
) -> bool {
    let mut all_known = true;
    for (name, span) in fill.holes() {
        if !decl.parameters.iter().any(|item| item.name == name) {
            all_known = false;
            let declared: Vec<&str> = decl
                .parameters
                .iter()
                .map(|item| item.name.as_str())
                .collect();
            let known = if declared.is_empty() {
                "command `".to_owned() + &decl.name + "` declares no parameters"
            } else {
                format!("command `{}` declares {declared:?}", decl.name)
            };
            refusals.push(CommandRefusal::refuse(
                CommandCause::ParameterUndeclared,
                span,
                &fill.source,
                format!(
                    "{what} interpolates `{{{{{name}}}}}`, which names no declared parameter; \
                     interpolation is CLOSED over the command's own declared parameters — {known}"
                ),
            ));
        }
    }
    all_known
}

/// No hand literal where a derivation exists.
pub(super) fn check_literal_derivations(
    fill: &Fill,
    refusals: &mut Vec<CommandRefusal>,
    what: &str,
) {
    for literal in fill.literals() {
        let Some((shape, observed)) = derived_shape_in(literal) else {
            continue;
        };
        refusals.push(CommandRefusal::refuse(
            CommandCause::LiteralWhereDerived,
            fill.span,
            observed,
            format!(
                "{what} carries the literal `{observed}`, for which a derivation exists: {}. \
                 Declare it as a parameter and let the thing that derives it supply it — a \
                 hand-written derived value records what was true once, on a day, and nothing \
                 re-checks it",
                shape.derivation()
            ),
        ));
    }
}

/// A type's name, as an author wrote it.
fn type_name(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Named { name, .. } => name.clone(),
        TypeRef::List { inner, .. } => format!("[{}]", type_name(inner)),
        TypeRef::Optional { inner, .. } => format!("{}?", type_name(inner)),
    }
}

/// Whether a type is a list — the one shape that emits several argv elements.
fn is_list(ty: &TypeRef) -> bool {
    matches!(ty, TypeRef::List { .. })
}
