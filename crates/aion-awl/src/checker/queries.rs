//! `answer` statement checking: the declared query, and the answer value
//! against that declaration.
//!
//! The read-only surface is the one place where a language-level omission
//! turns into an operator-visible LIE, so the refusals here are the point of
//! the feature.
//!
//! The engine dispatches a query by NAME against the handlers a run has
//! registered. A name nothing registered comes back as `UnknownQuery`, which
//! an operator reads as "this workflow does not have that query". When the
//! document DECLARED the query and simply never answered it, that reading is
//! false — the workflow does have it, the author forgot to set it. Neither the
//! engine nor the operator can tell those apart at runtime, so the language
//! refuses the shape at check time:
//!
//!  * every `answer` names a query the header DECLARES, so a typo cannot
//!    silently register a name no caller will ever ask for;
//!  * every declared query is answered somewhere in the document (see
//!    `checker::run`), so a declaration is a promise the document keeps;
//!  * the answer value is checked against the declared answer type, so what a
//!    caller receives has the stated shape.
//!
//! What this pass deliberately does NOT do is reason about WHERE, at run time,
//! the answer lands. The declared-somewhere rule is syntactic on purpose, and
//! the two cases it therefore admits are named here rather than left to be
//! discovered:
//!
//!  * an `answer` in a step the flow never routes to satisfies the rule while
//!    registering nothing;
//!  * an `answer` inside a per-item region satisfies the rule while
//!    registering on whichever process runs that region — the parent when the
//!    region lowers inline, a synthesized per-item CHILD when the region's
//!    shape requires one (`emitter::implicit_child_required`), in which case
//!    an operator querying the parent still gets `UnknownQuery`.
//!
//! Both are the same class: proving otherwise is the step-graph reachability
//! question, and binding query answers to it would make a
//! conditionally-answered query — the honest, common shape, a query answered
//! only after a stage completes — a compile error. One stated limit is worth
//! more than one stated limit plus an ad-hoc refusal for the corner of it that
//! happened to be noticed first. A document that wants a parent-visible answer
//! registers it in a parent step.

use std::collections::BTreeSet;

use crate::ast::{AnswerStmt, RoutePayload, Statement, Step};
use crate::semantic::DeclarationKind;
use crate::spanned::Spanned;

use super::args::check_args;
use super::context::Ctx;
use super::exprs::{View, type_of};
use super::types::{Ty, assignable, resolve};
use super::walk::{Scope, Walker};

/// Refuse every declared query the document never answers.
///
/// Deliberately a SYNTACTIC traversal of the whole document rather than a
/// by-product of the flow walk: the walk is skipped entirely when a flow's
/// `after` edges cycle, and a document that already has one diagnostic must
/// not also be told, falsely, that its queries are unanswered.
pub(super) fn check_declared_answers(ctx: &mut Ctx<'_>) {
    if ctx.doc.queries.is_empty() {
        return;
    }
    let mut answered = BTreeSet::new();
    for step in &ctx.doc.steps {
        collect_step(step, &mut answered);
    }
    for subflow in &ctx.doc.subflows {
        for step in &subflow.steps {
            collect_step(step, &mut answered);
        }
    }
    // Cloned so the borrow of `ctx.doc` ends before diagnostics are recorded.
    let unanswered: Vec<(crate::Span, String)> = ctx
        .doc
        .queries
        .iter()
        .filter(|query| !answered.contains(&query.name))
        .map(|query| (query.name_span, query.name.clone()))
        .collect();
    for (span, name) in unanswered {
        ctx.error(
            span,
            format!(
                "query `{name}` is declared but never answered — an operator asking for it \
                 would be told the workflow has no such query, which is not what this \
                 document says; register its value with `answer {name}(…)` where the state \
                 it reports is in scope"
            ),
        );
    }
}

fn collect_step(step: &Step, answered: &mut BTreeSet<String>) {
    collect_statements(&step.body, answered);
    if let Some(on_failure) = &step.on_failure {
        collect_statements(&on_failure.body, answered);
    }
}

fn collect_statements(statements: &[Statement], answered: &mut BTreeSet<String>) {
    for statement in statements {
        match statement {
            Statement::Answer(answer) => {
                answered.insert(answer.query.clone());
            }
            Statement::Fork(fork) => collect_statements(&fork.body, answered),
            Statement::Loop(looped) => collect_statements(&looped.body, answered),
            Statement::SubStep(sub) => collect_step(sub, answered),
            Statement::Call(_)
            | Statement::Spawn(_)
            | Statement::Pipe(_)
            | Statement::Wait(_)
            | Statement::Send(_)
            | Statement::Sleep(_)
            | Statement::Route(_)
            | Statement::Distribute(_)
            | Statement::Collect(_) => {}
        }
    }
}

pub(super) fn walk_answer(w: &mut Walker<'_, '_>, scope: &Scope, answer: &AnswerStmt) {
    let answer_ty = declared_answer(w, answer);
    check_value(w, scope, answer, &answer_ty);
}

/// Resolve the query's declared answer type, or report the undeclared name.
fn declared_answer(w: &mut Walker<'_, '_>, answer: &AnswerStmt) -> Ty {
    if let Some(ty) = w.ctx.queries.get(&answer.query).cloned() {
        if w.emit {
            w.ctx
                .semantic
                .reference(answer.query_span, DeclarationKind::Query, &answer.query);
            w.ctx.semantic.ty(answer.query_span, &ty.to_string());
        }
        return ty;
    }
    w.err(
        answer.query_span,
        format!(
            "no query named `{}` is declared in the workflow header — an answer names a \
             query the document declares with `query {}: <Type>`, so what an operator \
             receives has a stated shape",
            answer.query, answer.query
        ),
    );
    Ty::Unknown
}

/// Check the answer value against the declared type, in the same three forms a
/// route payload takes.
fn check_value(w: &mut Walker<'_, '_>, scope: &Scope, answer: &AnswerStmt, ty: &Ty) {
    let view = View::plain(scope);
    let value_span = answer.value_span.unwrap_or(answer.query_span);
    match &answer.value {
        Some(RoutePayload::Args(args)) => match resolve(ty, &w.ctx.types) {
            Ty::Record(record) => {
                let params: Vec<(String, Ty)> = record
                    .fields
                    .iter()
                    .map(|field| (field.name.clone(), field.ty.clone()))
                    .collect();
                check_args(
                    w,
                    view,
                    args,
                    &params,
                    &format!("query `{}` (type {ty})", answer.query),
                    "field",
                    answer.query_span,
                );
            }
            Ty::Unknown => {}
            other => {
                w.err(
                    value_span,
                    format!(
                        "query `{}` answers {other} — named fields need a record-typed \
                         query; answer with the value itself (`answer {}(<value>)`)",
                        answer.query, answer.query
                    ),
                );
            }
        },
        Some(RoutePayload::Value(value)) => {
            let value_ty = type_of(w, view, value);
            if !assignable(&value_ty, ty, &w.ctx.types) {
                w.err(
                    value.span(),
                    format!(
                        "the answer value is {value_ty}, but query `{}` answers {ty}",
                        answer.query
                    ),
                );
            }
        }
        // No parentheses is the value-less form. Like a bare `send`, it does
        // NOT pick up a same-named binding: registering whatever a coincidental
        // binding held is exactly the implicit wiring the read-only surface
        // must not have.
        None => {
            if !matches!(resolve(ty, &w.ctx.types), Ty::Nil | Ty::Unknown) {
                w.err(
                    answer.query_span,
                    format!(
                        "query `{}` answers {ty}, so the answer must construct it — \
                         `answer {}(…)`; only a `Nil` answer is written bare",
                        answer.query, answer.query
                    ),
                );
            }
        }
    }
}
