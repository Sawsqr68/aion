//! The read-only surface: the header's `query <name>: <Type>` declaration and
//! the body's `answer <name>[(<value>)]` statement.
//!
//! Both words are recognised POSITIONALLY, as plain identifiers, exactly like
//! the declarative action body's `run` and the signal sender's `send`/`to`
//! (see `lexer/tokens.rs` and `parser/send.rs`). Reserving either would break
//! documents that already use the word as an ordinary name, and the corpus
//! carries both today:
//!
//!   * `query` is an action parameter in
//!     `examples/general-worker/awl/general_worker_example.awl:35` —
//!     `action parse_output(text: String, mode: String, query: String)`, called
//!     with `query: "topic"` on line 40;
//!   * `answer` is a binding in
//!     `crates/aion-awl/tests/fixtures/rev2/step-bodies/valid/send_signal.awl:26`
//!     — `wait decision -> answer`.
//!
//! The grammar never needs either reserved: in the header a line opens with
//! its declaration word, and in a body `answer` heads a statement only when a
//! NAME follows it, which no other statement shape can produce.

use crate::ast::{AnswerStmt, Lead, QueryDecl, RoutePayload, Statement, join_span};
use crate::{Span, TokenKind};

use super::ParseError;
use super::statements::{ParsedPayload, parse_route_payload};
use super::stream::Stream;
use super::types::{parse_type_ref, type_ref_span};

/// Whether an identifier at the head of a header line opens a `query`
/// declaration.
pub(super) fn heads_declaration(word: &str) -> bool {
    word == "query"
}

/// Whether an identifier at the head of a body line opens an `answer`
/// statement: the word `answer` IMMEDIATELY followed by a name.
///
/// The lookahead is what keeps a document that CALLS an action named `answer`
/// working — `answer(text: x) -> reply` puts a `(` after the word, and a pipe
/// head named `answer` puts a `|>` or `->` there. Like `send`, it deliberately
/// does NOT look past a newline: a lone `answer` at the end of a body would
/// otherwise swallow the next line's first word and report an answer's
/// diagnostic for a statement nobody wrote.
pub(super) fn heads_statement(stream: &mut Stream, word: &str) -> bool {
    word == "answer"
        && matches!(
            stream.peek_second().map(|token| &token.kind),
            Some(TokenKind::Identifier(_) | TokenKind::TypeIdentifier(_))
        )
}

/// Parse one `query <name>: <Type>` header declaration; the `query` word (at
/// `query_span`) is still at the cursor.
pub(super) fn parse_declaration(
    stream: &mut Stream,
    lead: Vec<Lead>,
    query_span: Span,
) -> Result<QueryDecl, ParseError> {
    stream.next();
    let (name, name_span) = stream.expect_name("a query name")?;
    if stream
        .eat(|kind| matches!(kind, TokenKind::Colon))
        .is_none()
    {
        return Err(ParseError::new(
            name_span,
            "a `query` declares the type of the answer it reports as \
             `query <name>: <Type>`",
        ));
    }
    let ty = parse_type_ref(stream)?;
    let type_span = type_ref_span(&ty);
    let trailing = stream.end_line()?;
    Ok(QueryDecl {
        span: join_span(query_span, type_span),
        lead,
        trailing,
        name,
        name_span,
        ty,
    })
}

/// Parse one `answer <name>[(<value>)]` statement; the `answer` word (at
/// `answer_span`) is still at the cursor.
pub(super) fn parse_statement(
    stream: &mut Stream,
    lead: Vec<Lead>,
    answer_span: Span,
) -> Result<Statement, ParseError> {
    stream.next();
    let (query, query_span) = stream.expect_name("a query name after `answer`")?;
    let (value, value_span) = parse_value(stream)?;
    let trailing = stream.end_line()?;
    Ok(Statement::Answer(AnswerStmt {
        span: join_span(answer_span, value_span.unwrap_or(query_span)),
        lead,
        trailing,
        query,
        query_span,
        value,
        value_span,
    }))
}

/// Parse the optional parenthesised answer value. Absent parentheses is the
/// value-less form (`answer ready`), which the checker admits only for a `Nil`
/// answer type.
fn parse_value(stream: &mut Stream) -> Result<(Option<RoutePayload>, Option<Span>), ParseError> {
    if !stream.peek_is(|kind| matches!(kind, TokenKind::LeftParen)) {
        return Ok((None, None));
    }
    let open = stream.peek_span();
    stream.next();
    let parsed = parse_route_payload(stream)?;
    let (value, close) = match parsed {
        ParsedPayload::Args(args, close) => (RoutePayload::Args(args), close),
        ParsedPayload::Value(expr, close) => (RoutePayload::Value(expr), close),
    };
    Ok((Some(value), Some(join_span(open, close))))
}
