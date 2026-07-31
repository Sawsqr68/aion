//! Outcome-clause lowering. S17 makes both conditional cascades and enum-total
//! dispatch block tails: every arm ends in a route and no continuation can
//! follow the control construct.

use crate::ast::{BinaryOp, Expr, Guard, OutcomeClause, PredicateKind};
use crate::emitter::{GType, snake};
use crate::spanned::Spanned;

use super::super::ids::Span;
use super::super::ops::{Block, Stmt, Tail, Test, Value};
use super::ctx::Ctx;
use super::driver::LowerError;
use super::expr::{Binding, Scope, lower_expr};
use super::flow::FlowEnv;
use super::route::route_tail;

pub(super) fn lower_outcomes(
    ctx: &mut Ctx<'_>,
    env: FlowEnv<'_>,
    clauses: &[OutcomeClause],
    scope: &Scope,
) -> Result<Block, LowerError> {
    // Emitter parity: indexing inside an outcome guard is refused by the
    // reference (`emitter/outcomes.rs` — a non-empty guard prelude errors),
    // so direct must not silently out-support it. The scan covers both the
    // cascade guards and the enum-total subject (the subject is the left
    // side of a `when` guard).
    for clause in clauses {
        if let Guard::When { expr, span } = &clause.guard
            && super::expr::expr_contains_index(expr)
        {
            return Err(LowerError::unsupported(
                "indexing inside an outcome guard",
                *span,
            ));
        }
    }
    if let Some((subject, arms)) = enum_total_form(clauses) {
        let mut stmts = Vec::new();
        let (subject, _) = lower_expr(ctx, &subject, scope, &mut stmts)?;
        let mut lowered = Vec::with_capacity(arms.len());
        for (variant, clause) in arms {
            lowered.push((
                ctx.atom(&snake(&variant)),
                lower_outcome_arm(ctx, env, clause, scope, None)?,
            ));
        }
        return Ok(Block {
            stmts,
            tail: Tail::SelectEnum {
                subject,
                arms: lowered,
                span: Span::from_source(clauses[0].span),
            },
        });
    }
    lower_outcome_cascade(ctx, env, clauses, scope)
}

fn enum_total_form(clauses: &[OutcomeClause]) -> Option<(Expr, Vec<(String, &OutcomeClause)>)> {
    let mut subject: Option<Expr> = None;
    let mut arms = Vec::with_capacity(clauses.len());
    for clause in clauses {
        let Guard::When { expr, .. } = &clause.guard else {
            return None;
        };
        let Expr::Binary {
            left, op, right, ..
        } = expr
        else {
            return None;
        };
        if !matches!(op, BinaryOp::Eq) {
            return None;
        }
        let Expr::Variant { name, .. } = right.as_ref() else {
            return None;
        };
        if subject
            .as_ref()
            .is_some_and(|existing| !same_enum_subject(existing, left))
        {
            return None;
        }
        subject.get_or_insert_with(|| left.as_ref().clone());
        arms.push((name.clone(), clause));
    }
    Some((subject?, arms))
}

fn same_enum_subject(left: &Expr, right: &Expr) -> bool {
    match (left, right) {
        (Expr::Ref { name: a, .. }, Expr::Ref { name: b, .. }) => a == b,
        (
            Expr::Field {
                base: a_base,
                name: a_name,
                ..
            },
            Expr::Field {
                base: b_base,
                name: b_name,
                ..
            },
        ) => a_name == b_name && same_enum_subject(a_base, b_base),
        _ => false,
    }
}

#[derive(Clone)]
enum Decision<'a> {
    Test {
        expr: &'a Expr,
        when_true: Box<Decision<'a>>,
        when_false: Box<Decision<'a>>,
    },
    /// The false side of `x is absent` and the true side of `x is present`
    /// prove the SUBJECT is `Some` — a binding (`x`) or a field path over one
    /// (`x.y`). Keep that proof on the control edge so a short-circuited RHS
    /// can safely read the unwrapped value.
    NarrowPresent {
        subject: &'a Expr,
        span: crate::Span,
        next: Box<Decision<'a>>,
    },
    Arm {
        clause: &'a OutcomeClause,
        guard: Option<&'a Expr>,
    },
    Cascade(&'a [OutcomeClause]),
    /// A pre-built continuation (loop `until` exit/recurse blocks). Cloning
    /// the same block into sibling arms is single-def-legal: the verifier
    /// checks each arm from its own def-set clone.
    Ready(&'a Block),
}

fn lower_outcome_cascade(
    ctx: &mut Ctx<'_>,
    env: FlowEnv<'_>,
    clauses: &[OutcomeClause],
    scope: &Scope,
) -> Result<Block, LowerError> {
    let Some((clause, rest)) = clauses.split_first() else {
        return Err(LowerError::new(
            ctx.emitter.document.span,
            "outcome clauses ended without an `otherwise` arm",
        ));
    };
    match &clause.guard {
        Guard::Otherwise { span } => {
            if !rest.is_empty() {
                return Err(LowerError::new(*span, "`otherwise` must be the last arm"));
            }
            lower_outcome_arm(ctx, env, clause, scope, None)
        }
        Guard::When { expr, span } => {
            let decision = guard_decision(
                expr,
                Decision::Arm {
                    clause,
                    guard: Some(expr),
                },
                Decision::Cascade(rest),
            );
            // Keep the source clause span on the outer decision. Nested tests
            // carry their own expression spans.
            lower_decision(ctx, env, &decision, scope, Some(*span))
        }
    }
}

/// Compile boolean source syntax into a control tree rather than an eager
/// value expression. In particular, the right side of `and`/`or` (including
/// every `FieldGet` or call prelude it creates) lives only in the branch where
/// Gleam's `&&`/`||` semantics evaluate it.
fn guard_decision<'a>(
    expr: &'a Expr,
    when_true: Decision<'a>,
    when_false: Decision<'a>,
) -> Decision<'a> {
    match expr {
        Expr::Not { expr, .. } => guard_decision(expr, when_false, when_true),
        Expr::Binary {
            left,
            op: BinaryOp::And,
            right,
            ..
        } => {
            let rhs = guard_decision(right, when_true, when_false.clone());
            let rhs = narrow_short_circuit_rhs(left, BinaryOp::And, rhs);
            guard_decision(left, rhs, when_false)
        }
        Expr::Binary {
            left,
            op: BinaryOp::Or,
            right,
            ..
        } => {
            let rhs = guard_decision(right, when_true.clone(), when_false);
            let rhs = narrow_short_circuit_rhs(left, BinaryOp::Or, rhs);
            guard_decision(left, when_true, rhs)
        }
        _ => Decision::Test {
            expr,
            when_true: Box::new(when_true),
            when_false: Box::new(when_false),
        },
    }
}

/// Carry the proof the LAST operand of `left`'s chain establishes into the
/// decision reached once `left` held.
///
/// The last operand, and only the last: `a and b and c` parses as
/// `(a and b) and c`, and `guard_decision` recurses into `left`, so by the time
/// this runs on `(a and b)` the recursion has already wrapped `a`'s proof
/// around everything downstream of `a`'s test. What no recursion reaches is the
/// proof of the operand tested LAST — its true edge is this `next`. Before the
/// chain was flattened here, `b`'s proof was simply dropped and `c` read a
/// still-wrapped optional.
fn narrow_short_circuit_rhs<'a>(left: &'a Expr, op: BinaryOp, next: Decision<'a>) -> Decision<'a> {
    let kind = match op {
        BinaryOp::And => PredicateKind::Present,
        BinaryOp::Or => PredicateKind::Absent,
        _ => return next,
    };
    let Some(Expr::Predicate {
        subject,
        kind: actual,
        span,
    }) = crate::ast::chain_operands(left, op).last().copied()
    else {
        return next;
    };
    if *actual == kind {
        Decision::NarrowPresent {
            subject,
            span: *span,
            next: Box::new(next),
        }
    } else {
        next
    }
}

fn lower_decision(
    ctx: &mut Ctx<'_>,
    env: FlowEnv<'_>,
    decision: &Decision<'_>,
    scope: &Scope,
    outer_span: Option<crate::Span>,
) -> Result<Block, LowerError> {
    match decision {
        Decision::Test {
            expr,
            when_true,
            when_false,
        } => {
            let mut stmts = Vec::new();
            let (test, _) = lower_expr(ctx, expr, scope, &mut stmts)?;
            Ok(Block {
                stmts,
                tail: Tail::If {
                    test: Test::IsTrue(test),
                    then_block: Box::new(lower_decision(ctx, env, when_true, scope, None)?),
                    else_block: Box::new(lower_decision(ctx, env, when_false, scope, None)?),
                    span: Span::from_source(outer_span.unwrap_or_else(|| expr.span())),
                },
            })
        }
        Decision::NarrowPresent {
            subject,
            span,
            next,
        } => {
            let mut narrowed = scope.clone();
            let mut stmts = Vec::new();
            narrow_subject(ctx, subject, *span, &mut narrowed, &mut stmts)?;
            let mut block = lower_decision(ctx, env, next, &narrowed, None)?;
            stmts.append(&mut block.stmts);
            block.stmts = stmts;
            Ok(block)
        }
        Decision::Arm { clause, guard } => lower_outcome_arm(ctx, env, clause, scope, *guard),
        Decision::Cascade(clauses) => lower_outcome_cascade(ctx, env, clauses, scope),
        Decision::Ready(block) => Ok((*block).clone()),
    }
}

/// Lower a boolean condition through the same short-circuit decision builder
/// outcome guards use (the BC-2b-2 correctness fix), continuing into
/// pre-built true/false blocks. The loop `until` path: Gleam's value-position
/// `&&`/`||` short-circuit, so an eager `BoolOp` lowering would diverge on an
/// effectful or narrowing right-hand side.
pub(super) fn lower_condition(
    ctx: &mut Ctx<'_>,
    env: FlowEnv<'_>,
    expr: &Expr,
    scope: &Scope,
    when_true: &Block,
    when_false: &Block,
) -> Result<Block, LowerError> {
    let decision = guard_decision(
        expr,
        Decision::Ready(when_true),
        Decision::Ready(when_false),
    );
    lower_decision(ctx, env, &decision, scope, None)
}

fn lower_outcome_arm(
    ctx: &mut Ctx<'_>,
    env: FlowEnv<'_>,
    clause: &OutcomeClause,
    scope: &Scope,
    guard: Option<&Expr>,
) -> Result<Block, LowerError> {
    let mut arm_scope = scope.clone();
    let mut stmts = Vec::new();
    narrow_present(ctx, guard, &mut arm_scope, &mut stmts)?;
    let tail = route_tail(ctx, env, &clause.route, &arm_scope, None, &mut stmts)?;
    Ok(Block { stmts, tail })
}

/// Unwrap what the arm's guard proved but the decision tree did not already
/// materialize: the LAST conjunct.
///
/// An arm is the true edge of its whole guard, so every conjunct held there.
/// `guard_decision` wraps each conjunct's proof around the decision that
/// follows its test, which covers all of them EXCEPT the last — that one's true
/// edge is the arm itself, and no `NarrowPresent` sits between. So a bare
/// `x is present` guard is narrowed here (its only conjunct is its last), and
/// under `a is present and b is present` the tree supplies `a` and this
/// supplies `b`. Neither is narrowed twice.
fn narrow_present(
    ctx: &mut Ctx<'_>,
    guard: Option<&Expr>,
    scope: &mut Scope,
    stmts: &mut Vec<Stmt>,
) -> Result<(), LowerError> {
    let Some(guard) = guard else {
        return Ok(());
    };
    let Some(Expr::Predicate {
        subject,
        kind: PredicateKind::Present,
        span,
    }) = crate::ast::chain_operands(guard, BinaryOp::And)
        .last()
        .copied()
    else {
        return Ok(());
    };
    narrow_subject(ctx, subject, *span, scope, stmts)
}

/// Unwrap the optional a guard proved present, and record the unwrapped value
/// against the path it was proved for.
///
/// A bare binding rebinds the NAME, so every later read of it sees the
/// unwrapped value. A field path (`nudge.end`) has no name to rebind — nothing
/// in the document ever named it — so the value is recorded against the path
/// itself, and `lower_expr` resolves a matching field read to it.
///
/// A subject that is not an optional path narrows nothing and emits nothing.
/// The checker has already refused `is present` on a non-optional, so this is
/// the belt on those braces, not a silent fallback: it leaves the value exactly
/// as it was rather than inventing an unwrap the source did not prove.
fn narrow_subject(
    ctx: &mut Ctx<'_>,
    subject: &Expr,
    span: crate::Span,
    scope: &mut Scope,
    stmts: &mut Vec<Stmt>,
) -> Result<(), LowerError> {
    let Some(path) = crate::ast::access_path(subject) else {
        return Ok(());
    };
    // Lower into a scratch buffer: if the subject turns out not to be an
    // optional, nothing is spliced in and no dead field read is emitted.
    let mut prelude = Vec::new();
    let (option, ty) = lower_expr(ctx, subject, scope, &mut prelude)?;
    let GType::Option(inner) = ctx.emitter.env.resolve(&ty) else {
        return Ok(());
    };
    let Value::Var(option) = option else {
        return Ok(());
    };
    stmts.append(&mut prelude);
    let dst = ctx.fresh_var();
    stmts.push(Stmt::AssertSome {
        dst,
        option,
        span: Span::from_source(span),
    });
    let binding = Binding {
        var: dst,
        ty: *inner,
    };
    if let [name] = path.as_slice() {
        scope.insert(name.clone(), binding);
    } else {
        scope.narrow_path(path, binding);
    }
    Ok(())
}
