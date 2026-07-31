//! Guard-dependent optionality narrowing: which ACCESS PATHS a guard proves
//! present, and what type each carries inside the arm it guards.
//!
//! Narrowing keys on a path (`nudge.end`), not on a binding name. Keying on a
//! name is what stopped `when nudge.end is present and nudge.end` from
//! type-checking: the representation could not express "this FIELD is proved
//! present", only "this BINDING is". That cost a live wire contract — the
//! assistant port had to make both signal fields required because a document
//! could not narrow one.
//!
//! Proofs are held as a SET rather than a single slot, because one guard can
//! prove more than one path and a later proof may depend on an earlier one
//! (`a is present and a.b is present` narrows `a.b` only because the first
//! proof made `a` a record). How far a proof reaches is deliberately pinned to
//! what the two code generators do — see `collect`.

use crate::ast::{Expr, PredicateKind, access_path};

use super::exprs::View;
use super::types::{Ty, resolve};
use super::walk::{Scope, Walker};

/// One guard-proved narrowing: an access path and the type it carries where
/// the proof holds (the optional's inner type).
#[derive(Clone)]
pub(super) struct Narrowing {
    /// The access path the guard proved present (`nudge.end` → two segments).
    pub(super) path: Vec<String>,
    /// The type that path carries inside the arm.
    pub(super) ty: Ty,
}

/// The narrowed type of exactly `path`, innermost proof first. A later proof
/// about the same path supersedes an earlier one.
pub(super) fn lookup(narrowings: &[Narrowing], path: &[String]) -> Option<Ty> {
    narrowings
        .iter()
        .rev()
        .find(|narrowing| narrowing.path == path)
        .map(|narrowing| narrowing.ty.clone())
}

/// The narrowed type of a bare binding name — a one-segment path.
pub(super) fn lookup_name(narrowings: &[Narrowing], name: &str) -> Option<Ty> {
    narrowings
        .iter()
        .rev()
        .find(|narrowing| narrowing.path.len() == 1 && narrowing.path[0] == name)
        .map(|narrowing| narrowing.ty.clone())
}

/// The type `path` carries in `view`, consulting the narrowings at EVERY
/// prefix. Reading `a.b.c` where an outer arm narrowed `a.b` must see the
/// narrowed `a.b`, or the walk would stop at an optional it has already been
/// given the proof for.
///
/// Returns `None` when the path does not resolve — the caller's job is to
/// decline to narrow, never to report. The ordinary typing walk reports the
/// defect at its own site with its own span.
fn path_type(w: &Walker<'_, '_>, view: View<'_>, path: &[String]) -> Option<Ty> {
    let mut ty: Option<Ty> = None;
    for length in 1..=path.len() {
        if let Some(narrowed) = view.narrowed(&path[..length]) {
            ty = Some(narrowed);
            continue;
        }
        ty = Some(match ty {
            None => view.vars.get(&path[0]).map(|value| value.ty.clone())?,
            Some(base) => match resolve(&base, &w.ctx.types) {
                Ty::Record(record) => record.field(&path[length - 1])?.ty.clone(),
                _ => return None,
            },
        });
    }
    ty
}

/// The narrowing an `is present` (or the false edge of an `is absent`) proves
/// about `subject`, if the subject is a path whose type is optional.
fn narrowing_of(w: &Walker<'_, '_>, view: View<'_>, subject: &Expr) -> Option<Narrowing> {
    let path = access_path(subject)?;
    let ty = path_type(w, view, &path)?;
    let Ty::Optional(inner) = resolve(&ty, &w.ctx.types) else {
        return None;
    };
    Some(Narrowing {
        path,
        ty: inner.as_ref().clone(),
    })
}

/// Collect the narrowing `expr` proves on the edge where it holds, appending
/// to the proofs already in force.
///
/// `kind` is what the edge proves: `is present` on the true edge of an `and`,
/// `is absent` on the false edge of an `or`.
///
/// This deliberately does NOT walk into a nested conjunction. `a and b and c`
/// parses as `(a and b) and c`, so a proof `b` establishes does not reach `c`
/// here — and it does not reach it in either backend either: both the direct
/// lowering (`mir/lower/outcome.rs`) and the Gleam reference emitter
/// (`emitter/exprs.rs`) narrow from a binary's DIRECT left operand only. The
/// checker stays exactly as permissive as the code generators are correct. A
/// checker that outran them would accept a document that compiles to a test
/// against an unwrapped optional — silently false, never reported. Widening
/// this is a three-layer change, not a checker change.
pub(super) fn collect(
    w: &Walker<'_, '_>,
    vars: &Scope,
    accessor: Option<&Ty>,
    expr: &Expr,
    kind: PredicateKind,
    proofs: &mut Vec<Narrowing>,
) {
    let Expr::Predicate {
        subject,
        kind: actual,
        ..
    } = expr
    else {
        return;
    };
    if *actual != kind {
        return;
    }
    let found = {
        let view = View {
            vars,
            narrow: proofs,
            accessor,
        };
        narrowing_of(w, view, subject)
    };
    if let Some(narrowing) = found {
        proofs.push(narrowing);
    }
}

/// The narrowings a step's outcome guard proves inside its arm. The guard is
/// typed with nothing in force, so this starts the set rather than extending
/// one.
pub(super) fn from_guard(w: &Walker<'_, '_>, scope: &Scope, guard: &Expr) -> Vec<Narrowing> {
    let mut proofs = Vec::new();
    collect(w, scope, None, guard, PredicateKind::Present, &mut proofs);
    proofs
}
