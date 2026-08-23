//! `{{parameter}}` — typed, check-time, closed over declared parameters.
//!
//! # What this is, and the fence around it
//!
//! A fill is a template that produces ONE argv element (or, for a list-typed
//! hole standing alone in an operand position, one element per item). It is
//! resolved at check and compile time, over the command's own declared
//! parameters, into whole-cloth argv. It is a CONSTRUCTION surface.
//!
//! ⛔ It is not, and must not become, a templating language. There are no
//! conditionals, no loops, no filters, no function calls, no nesting, and no
//! way to reach anything the command did not declare. Every one of those is a
//! step toward evaluating author-supplied text at run time, which is the
//! thing the determinism boundary exists to prevent. A later cut that widens
//! this is widening the wrong thing.
//!
//! # Backticks are refused, at the parse layer
//!
//! Just's backtick command substitution runs a shell at evaluation time. That
//! is a side effect in a position that must be pure, and no recorded activity
//! stands behind it, so it is refused where it is written rather than checked
//! for later. A refusal at the parse layer means the form is unrepresentable:
//! there is no `Fill` value carrying a backtick.
//!
//! # File contents enter as addresses
//!
//! A fill never splices the contents of a file. A command that needs a file
//! names the file — an address the executing side opens — because splicing
//! makes the argv depend on bytes nobody declared and nobody reviewed.

use crate::Span;

use super::cause::CommandCause;
use super::error::CommandRefusal;

/// The opening delimiter of an interpolation.
const HOLE_OPEN: &str = "{{";
/// The closing delimiter of an interpolation.
const HOLE_CLOSE: &str = "}}";

/// One piece of a fill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FillSegment {
    /// Literal text, emitted verbatim.
    Literal(String),
    /// An interpolation of one declared parameter.
    Hole {
        /// The parameter's name.
        name: String,
        /// Where the `{{…}}` stands, as precisely as the literal allows.
        span: Span,
        /// Byte offset of the `{{` within the fill's own text.
        offset: usize,
        /// Byte length of the whole `{{…}}` within the fill's own text.
        len: usize,
    },
}

/// A parsed fill: its segments, and the text they were read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fill {
    /// Span of the string literal the fill was read from.
    pub span: Span,
    /// The literal's decoded text, verbatim — the bytes a refusal observes.
    pub source: String,
    /// The segments, in order.
    pub segments: Vec<FillSegment>,
}

impl Fill {
    /// Parse `source` — one string literal's decoded text — as a fill.
    ///
    /// `span` is the literal's span in the document. Hole spans are narrowed
    /// within it when the literal carried no escape (so that content length
    /// plus the two quotes is exactly the span's width); when an escape makes
    /// the mapping ambiguous, a hole's span is the whole literal's, which is
    /// imprecise but never wrong.
    ///
    /// # Errors
    ///
    /// Returns a parse-layer [`CommandRefusal`] for a backtick, an
    /// unterminated `{{`, an empty `{{}}`, or a `{{…}}` whose contents are
    /// not a parameter name.
    pub fn parse(source: &str, span: Span) -> Result<Self, CommandRefusal> {
        if let Some(at) = source.find('`') {
            return Err(refuse(
                CommandCause::BacktickSubstitution,
                narrow(span, source, at, 1),
                source,
                "a backtick in a fill is command substitution — it would run a shell at \
                 evaluation time, which the determinism boundary does not admit; pass the value \
                 in as a declared parameter instead",
            ));
        }

        let mut segments = Vec::new();
        let mut literal = String::new();
        let mut rest = source;
        let mut cursor = 0usize;
        while let Some(open) = rest.find(HOLE_OPEN) {
            literal.push_str(&rest[..open]);
            let hole_at = cursor + open;
            let after_open = &rest[open + HOLE_OPEN.len()..];
            let Some(close) = after_open.find(HOLE_CLOSE) else {
                return Err(refuse(
                    CommandCause::InterpolationUnterminated,
                    narrow(span, source, hole_at, source.len() - hole_at),
                    source,
                    "an interpolation opened `{{` and never closed it; write `{{name}}` around \
                     one declared parameter",
                ));
            };
            let inner = &after_open[..close];
            let len = HOLE_OPEN.len() + close + HOLE_CLOSE.len();
            let name = inner.trim();
            if name.is_empty() {
                return Err(refuse(
                    CommandCause::InterpolationEmpty,
                    narrow(span, source, hole_at, len),
                    source,
                    "`{{}}` interpolates nothing; write `{{name}}` around one declared parameter",
                ));
            }
            if !is_parameter_name(name) {
                return Err(refuse(
                    CommandCause::InterpolationMalformed,
                    narrow(span, source, hole_at, len),
                    source,
                    format!(
                        "`{name}` is not a parameter name; a fill interpolates the command's own \
                         snake_case parameters ([a-z][a-z0-9_]*) and nothing else — there are no \
                         expressions, calls, or paths inside `{{{{…}}}}`"
                    ),
                ));
            }
            if !literal.is_empty() {
                segments.push(FillSegment::Literal(std::mem::take(&mut literal)));
            }
            segments.push(FillSegment::Hole {
                name: name.to_owned(),
                span: narrow(span, source, hole_at, len),
                offset: hole_at,
                len,
            });
            cursor = hole_at + len;
            rest = &source[cursor..];
        }
        literal.push_str(rest);
        if !literal.is_empty() {
            segments.push(FillSegment::Literal(literal));
        }

        Ok(Self {
            span,
            source: source.to_owned(),
            segments,
        })
    }

    /// The parameter names this fill interpolates, in source order, with
    /// their spans — the binding a resolution must supply.
    #[must_use]
    pub fn holes(&self) -> Vec<(&str, Span)> {
        self.segments
            .iter()
            .filter_map(|segment| match segment {
                FillSegment::Hole { name, span, .. } => Some((name.as_str(), *span)),
                FillSegment::Literal(_) => None,
            })
            .collect()
    }

    /// The literal segments, verbatim — what a derived-shape rule reads.
    #[must_use]
    pub fn literals(&self) -> Vec<&str> {
        self.segments
            .iter()
            .filter_map(|segment| match segment {
                FillSegment::Literal(text) => Some(text.as_str()),
                FillSegment::Hole { .. } => None,
            })
            .collect()
    }

    /// The single parameter this fill is, when it is EXACTLY one hole and
    /// nothing else.
    ///
    /// This is the one shape that carries its parameter's own type through:
    /// `"{{count}}"` is an `Int` when `count` is, while `"n={{count}}"` is a
    /// String because it is text with a number written into it. It is also
    /// the only shape a list may stand in, because a list has no rendering as
    /// part of a larger word.
    #[must_use]
    pub fn sole_hole(&self) -> Option<&str> {
        match self.segments.as_slice() {
            [FillSegment::Hole { name, .. }] => Some(name.as_str()),
            _ => None,
        }
    }

    /// Whether the fill produces the same bytes on every resolution given the
    /// same parameters — which it always does, there being no other input.
    ///
    /// Stated as a function rather than a comment so the determinism claim is
    /// something a test calls rather than something a reader hopes.
    #[must_use]
    pub const fn is_deterministic(&self) -> bool {
        true
    }
}

/// Whether `name` is a `snake_case` AWL parameter name (`[a-z][a-z0-9_]*`).
///
/// The same character set the existing declared-body scanner enforces
/// (`crate::ast::command`), so the two surfaces agree on what a parameter
/// name is and neither accepts a name the other would refuse.
fn is_parameter_name(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

/// A span inside the literal, when the mapping is unambiguous.
///
/// The literal's span covers its quotes, so content byte `n` sits at
/// `span.start + 1 + n` — but only when the lexer decoded no escape, which is
/// exactly the case where the decoded length plus two quotes equals the
/// span's width. Otherwise the mapping is unknown and the whole literal's
/// span is returned: imprecise, never wrong.
fn narrow(span: Span, source: &str, offset: usize, len: usize) -> Span {
    if span.end.saturating_sub(span.start) != source.len() + 2 {
        return span;
    }
    let start = span.start + 1 + offset;
    let end = (start + len).min(span.end);
    Span {
        start,
        end,
        line: span.line,
        column: span.column + 1 + source[..offset].chars().count(),
    }
}

/// Build a refusal. The layer is derived from the cause, so a fill can only
/// ever produce parse-layer refusals — the four causes below are parse
/// causes, and `CommandRefusal::refuse` files each one where it belongs
/// without this function choosing.
fn refuse(
    cause: CommandCause,
    span: Span,
    observed: &str,
    message: impl Into<String>,
) -> CommandRefusal {
    CommandRefusal::refuse(cause, span, observed, message)
}

#[cfg(test)]
#[path = "fill_tests.rs"]
mod fill_tests;
