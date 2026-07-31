//! The signal SENDER: the header's `send <name>: <Type>` declaration and the
//! body's `send <name>[(<payload>)] to <expr>` statement.
//!
//! `send` is recognised POSITIONALLY, as a plain identifier, exactly like the
//! declarative action body's `run` (see `lexer/tokens.rs`). Reserving the word
//! would break every document that already uses it as an ordinary name — the
//! corpus has both `action send(to: String, …)` and `step send` — and the
//! grammar never needs it reserved: in the header a line opens with its
//! declaration word, and in a body `send` heads a statement only when a name
//! follows it, which no other statement shape can produce. `to` is positional
//! for the same reason: it is an AWL-0/1 word that lexes as an identifier and
//! is used as a parameter name in the corpus today.

use crate::ast::{Lead, RoutePayload, SendDecl, SendStmt, Statement, join_span};
use crate::{SEND_WORD, Span, TO_WORD, TokenKind};

use super::ParseError;
use super::exprs::{expr_span, parse_expr};
use super::statements::{ParsedPayload, parse_route_payload};
use super::stream::Stream;
use super::types::{parse_type_ref, type_ref_span};

/// Whether an identifier at the head of a header line opens a `send`
/// declaration.
pub(super) fn heads_declaration(word: &str) -> bool {
    word == SEND_WORD
}

/// Whether an identifier at the head of a body line opens a `send`
/// statement: the word `send` IMMEDIATELY followed by a name.
///
/// The lookahead is what keeps a document that CALLS an action named `send`
/// working — `send(to: x, body: y) -> receipt` puts a `(` after the word, and
/// a pipe head named `send` puts a `|>` or `->` there. It deliberately does
/// NOT look past a newline (unlike `peek_second_is`): a lone `send` at the
/// end of a body would otherwise swallow the next line's first word and
/// report a send's diagnostic for a statement nobody wrote.
pub(super) fn heads_statement(stream: &mut Stream, word: &str) -> bool {
    word == SEND_WORD
        && matches!(
            stream.peek_second().map(|token| &token.kind),
            Some(TokenKind::Identifier(_) | TokenKind::TypeIdentifier(_))
        )
}

/// Parse one `send <name>: <Type>` header declaration; the `send` word (at
/// `send_span`) is still at the cursor.
pub(super) fn parse_declaration(
    stream: &mut Stream,
    lead: Vec<Lead>,
    send_span: Span,
) -> Result<SendDecl, ParseError> {
    stream.next();
    let (name, name_span) = stream.expect_name("a send name")?;
    if stream
        .eat(|kind| matches!(kind, TokenKind::Colon))
        .is_none()
    {
        return Err(ParseError::new(
            name_span,
            "a `send` declares the signal it sends as `send <name>: <Type>`",
        ));
    }
    let ty = parse_type_ref(stream)?;
    let type_span = type_ref_span(&ty);
    let trailing = stream.end_line()?;
    Ok(SendDecl {
        span: join_span(send_span, type_span),
        lead,
        trailing,
        name,
        name_span,
        ty,
    })
}

/// Parse one `send <name>[(<payload>)] to <expr>` statement; the `send` word
/// (at `send_span`) is still at the cursor.
pub(super) fn parse_statement(
    stream: &mut Stream,
    lead: Vec<Lead>,
    send_span: Span,
) -> Result<Statement, ParseError> {
    stream.next();
    let (signal, signal_span) = stream.expect_name("a signal name after `send`")?;
    let (payload, payload_span) = parse_payload(stream)?;
    expect_to(stream, &signal, signal_span)?;
    let target = parse_expr(stream)?;
    let target_span = expr_span(&target);
    reject_trailing_payload(stream, &signal)?;
    let trailing = stream.end_line()?;
    Ok(Statement::Send(SendStmt {
        span: join_span(send_span, target_span),
        lead,
        trailing,
        signal,
        signal_span,
        payload,
        payload_span,
        target,
    }))
}

/// Parse the optional parenthesised payload. Absent parentheses is the
/// payload-less form (`send ping to id`), which the checker admits only for a
/// `Nil` payload type.
fn parse_payload(stream: &mut Stream) -> Result<(Option<RoutePayload>, Option<Span>), ParseError> {
    if !stream.peek_is(|kind| matches!(kind, TokenKind::LeftParen)) {
        return Ok((None, None));
    }
    let open = stream.peek_span();
    stream.next();
    let parsed = parse_route_payload(stream)?;
    let (payload, close) = match parsed {
        ParsedPayload::Args(args, close) => (RoutePayload::Args(args), close),
        ParsedPayload::Value(value, close) => (RoutePayload::Value(value), close),
    };
    Ok((Some(payload), Some(join_span(open, close))))
}

fn expect_to(stream: &mut Stream, signal: &str, signal_span: Span) -> Result<(), ParseError> {
    let is_to =
        stream.peek_is(|kind| matches!(kind, TokenKind::Identifier(word) if word == TO_WORD));
    if is_to {
        stream.next();
        return Ok(());
    }
    Err(ParseError::new(
        signal_span,
        format!(
            "a `send` names the workflow it goes to: `send {signal}(…) to <workflow id>` \
             (an id-valued expression — `workflow.id`, an input, or a binding)"
        ),
    ))
}

/// Catch the payload written AFTER the target. It is the natural word order
/// in English and therefore the mistake authors will make; without this the
/// statement dies on a bare "expected end of line, found `(`".
fn reject_trailing_payload(stream: &mut Stream, signal: &str) -> Result<(), ParseError> {
    if !stream.peek_is(|kind| matches!(kind, TokenKind::LeftParen)) {
        return Ok(());
    }
    Err(ParseError::new(
        stream.peek_span(),
        format!(
            "the payload belongs to the signal, not the target: write \
             `send {signal}(…) to <workflow id>`"
        ),
    ))
}
