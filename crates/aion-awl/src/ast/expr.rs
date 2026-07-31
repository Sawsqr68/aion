use std::time::Duration;

use crate::{DurationUnit, Span};

/// Duration literal made from an integer magnitude and a unit suffix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurationLiteral {
    /// Source span covering the complete duration literal.
    pub span: Span,
    /// Numeric amount written before the duration unit.
    pub magnitude: u64,
    /// Unit suffix used by the duration literal.
    pub unit: DurationUnit,
}

impl DurationLiteral {
    /// Converts this literal to a standard duration when its whole-seconds
    /// value fits in a `u64`.
    #[must_use]
    pub fn checked_duration(&self) -> Option<Duration> {
        let seconds_per_unit = match self.unit {
            DurationUnit::Seconds => 1,
            DurationUnit::Minutes => 60,
            DurationUnit::Hours => 60 * 60,
            DurationUnit::Days => 24 * 60 * 60,
        };
        self.magnitude
            .checked_mul(seconds_per_unit)
            .map(Duration::from_secs)
    }

    /// Converts this literal to the signed millisecond representation used by
    /// native BEAM duration calls. Returns `None` instead of changing the
    /// declared duration when multiplication or the `i64` conversion would
    /// overflow.
    #[must_use]
    pub fn checked_milliseconds(&self) -> Option<i64> {
        let milliseconds_per_unit = match self.unit {
            DurationUnit::Seconds => 1_000,
            DurationUnit::Minutes => 60_000,
            DurationUnit::Hours => 3_600_000,
            DurationUnit::Days => 86_400_000,
        };
        self.magnitude
            .checked_mul(milliseconds_per_unit)
            .and_then(|milliseconds| i64::try_from(milliseconds).ok())
    }
}

/// Named argument inside a call, record construction, or route payload.
///
/// AWL rev-2 has no positional arguments anywhere: every argument is
/// `name: expr`, checked against the declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arg {
    /// Source span from the argument name through its value expression.
    pub span: Span,
    /// Argument name.
    pub name: String,
    /// Source span of the argument name.
    pub name_span: Span,
    /// Value expression assigned to the argument.
    pub value: Expr,
}

/// Binary operator supported by expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    /// Logical disjunction, `or`.
    Or,
    /// Logical conjunction, `and`.
    And,
    /// Equality comparison, `==`.
    Eq,
    /// Inequality comparison, `!=`.
    Ne,
    /// Less-than comparison, `<`.
    Lt,
    /// Less-than-or-equal comparison, `<=`.
    Le,
    /// Greater-than comparison, `>`.
    Gt,
    /// Greater-than-or-equal comparison, `>=`.
    Ge,
    /// String concatenation, `+` (string-only; AWL has no arithmetic).
    Concat,
}

/// Postfix `is …` predicate kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredicateKind {
    /// `is empty` — a list has no items.
    Empty,
    /// `is present` — an optional value is there.
    Present,
    /// `is absent` — an optional value is missing.
    Absent,
}

/// Boolean collection predicate selected by a collection pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantifier {
    /// `collection |> any(predicate)` — at least one item satisfies the predicate.
    Any,
    /// `collection |> all(predicate)` — every item satisfies the predicate.
    All,
}

/// Expression tree used in guards, arguments, seeds, bounds, and pipes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// String literal, holding the decoded value (escapes processed).
    String {
        /// Source span covering the full literal including quotes.
        span: Span,
        /// Decoded string value without source quotes.
        value: String,
    },
    /// Triple-quoted `"""…"""` raw string literal: the value is the text
    /// between the delimiters, byte-for-byte — newlines literal, zero escape
    /// processing — so the printer re-emits the literal verbatim.
    RawString {
        /// Source span from the opening `"""` through the closing `"""`.
        span: Span,
        /// Raw string value, exactly as written between the delimiters.
        value: String,
    },
    /// A `json { … }` literal. The value is the verbatim brace-balanced body
    /// text (braces included); its type is `String`. The checker validates
    /// the body as JSON; the printer re-emits it byte-for-byte.
    Json {
        /// Source span from the `json` keyword through the closing `}`.
        span: Span,
        /// Verbatim body text, including the enclosing braces.
        body: String,
        /// Source span of the braced body alone, for diagnostics that point
        /// into it.
        body_span: Span,
    },
    /// A `schema of Type` expression: the named type's derived JSON Schema
    /// as a compile-time `String`.
    SchemaOf {
        /// Source span from `schema` through the type name.
        span: Span,
        /// The named type whose schema is derived.
        name: String,
        /// Source span of the type name.
        name_span: Span,
    },
    /// Integer literal.
    Int {
        /// Source span covering the literal.
        span: Span,
        /// Parsed unsigned integer value.
        value: u64,
    },
    /// Float literal, preserving the source lexeme for lossless printing.
    Float {
        /// Source span covering the literal.
        span: Span,
        /// Original float text exactly as written.
        value: String,
    },
    /// Boolean literal.
    Bool {
        /// Source span covering the literal.
        span: Span,
        /// Parsed boolean value.
        value: bool,
    },
    /// Duration literal.
    Duration(DurationLiteral),
    /// List literal with zero or more items.
    List {
        /// Source span from `[` through `]`.
        span: Span,
        /// Item expressions in source order.
        items: Vec<Expr>,
    },
    /// Reference to an input, binding, or counter by name.
    Ref {
        /// Source span covering the reference identifier.
        span: Span,
        /// Referenced name.
        name: String,
    },
    /// The reserved `workflow` builtin namespace. It is not a value by itself.
    Workflow {
        /// Source span covering `workflow`.
        span: Span,
    },
    /// Bare `TitleCase` enum-variant reference (`result.category == Urgent`).
    Variant {
        /// Source span covering the variant name.
        span: Span,
        /// Variant name.
        name: String,
    },
    /// Record construction with required named fields: `TypeName(field: expr, …)`.
    Record {
        /// Source span from the type name through the closing parenthesis.
        span: Span,
        /// Record type name being constructed.
        name: String,
        /// Source span of the type name.
        name_span: Span,
        /// Named field initializers in source order.
        args: Vec<Arg>,
    },
    /// Field access on a base expression: `base.field`.
    Field {
        /// Source span from the base expression through the field name.
        span: Span,
        /// Expression producing the value being accessed.
        base: Box<Expr>,
        /// Field name selected from the base.
        name: String,
        /// Source span of the `.field` accessor.
        name_span: Span,
    },
    /// Literal-only indexing: `items[0]`. Computed indices are unwritable.
    Index {
        /// Source span from the base expression through `]`.
        span: Span,
        /// Expression producing the list being indexed.
        base: Box<Expr>,
        /// Literal index value.
        index: u64,
        /// Source span of the index literal.
        index_span: Span,
    },
    /// Bare `.field` accessor shorthand, legal as a combinator argument
    /// (`filter(.blocking)` keeps items whose `blocking` is true).
    Accessor {
        /// Source span covering the accessor.
        span: Span,
        /// Accessed field name (without the dot).
        name: String,
    },
    /// Logical negation: `not expr`.
    Not {
        /// Source span from `not` through the operand.
        span: Span,
        /// Operand whose truth value is inverted.
        expr: Box<Expr>,
    },
    /// Binary expression with an infix operator.
    Binary {
        /// Source span from the left operand through the right operand.
        span: Span,
        /// Left operand.
        left: Box<Expr>,
        /// Operator applied between the operands.
        op: BinaryOp,
        /// Right operand.
        right: Box<Expr>,
    },
    /// Postfix predicate: `subject is empty|present|absent`.
    Predicate {
        /// Source span from the subject through the predicate word.
        span: Span,
        /// Subject expression the predicate examines.
        subject: Box<Expr>,
        /// Which predicate is applied.
        kind: PredicateKind,
    },
    /// Boolean collection pipeline: `collection |> any|all(predicate)`.
    CollectionPredicate {
        /// Source span from the collection through the closing parenthesis.
        span: Span,
        /// Expression producing the list to inspect.
        collection: Box<Expr>,
        /// Whether existential or universal quantification is requested.
        quantifier: Quantifier,
        /// Predicate evaluated with `.field` accessors bound to each item.
        predicate: Box<Expr>,
    },
}

pub(crate) fn join_span(start: Span, end: Span) -> Span {
    Span {
        start: start.start,
        end: end.end,
        line: start.line,
        column: start.column,
    }
}

/// The dotted access path an expression names, if it is a bare reference or a
/// chain of field accesses over one (`nudge.end` → `["nudge", "end"]`).
///
/// This is the identity guard narrowing keys on: a guard proves a PATH is
/// present, and every layer — checker, direct lowering, reference emitter —
/// has to agree on which reads that proof covers. `workflow.id` is
/// deliberately not a path: it is a builtin, not a binding, and can never be
/// optional.
pub(crate) fn access_path(expr: &Expr) -> Option<Vec<String>> {
    match expr {
        Expr::Ref { name, .. } => Some(vec![name.clone()]),
        Expr::Field { base, name, .. } => {
            let mut path = access_path(base)?;
            path.push(name.clone());
            Some(path)
        }
        _ => None,
    }
}

/// The path a field access names, given its parts (`base`, `.name`).
pub(crate) fn field_path(base: &Expr, name: &str) -> Option<Vec<String>> {
    let mut path = access_path(base)?;
    path.push(name.to_owned());
    Some(path)
}

/// Every operand of a chain of ONE connective, in source order.
///
/// `a and b and c` parses as `(a and b) and c`, so an expression that reads as
/// three conjuncts is a two-operand binary whose left is another binary. Every
/// layer that reasons about what a guard proves has to see all three: a proof
/// established by the middle conjunct is in force where the last one is
/// evaluated, and a layer that only looked at the direct operand would miss it.
/// Like [`access_path`], this lives in the AST because the checker, the direct
/// lowering, and the reference emitter must agree on it exactly.
///
/// Nesting on the right (`a and (b and c)`, which only source parentheses can
/// produce) flattens the same way — the operands are the same three, and the
/// proofs run in the same order.
pub(crate) fn chain_operands(expr: &Expr, op: BinaryOp) -> Vec<&Expr> {
    let mut operands = Vec::new();
    push_chain_operands(expr, op, &mut operands);
    operands
}

fn push_chain_operands<'e>(expr: &'e Expr, op: BinaryOp, operands: &mut Vec<&'e Expr>) {
    if let Expr::Binary {
        left,
        op: connective,
        right,
        ..
    } = expr
        && *connective == op
    {
        push_chain_operands(left, op, operands);
        push_chain_operands(right, op, operands);
    } else {
        operands.push(expr);
    }
}
