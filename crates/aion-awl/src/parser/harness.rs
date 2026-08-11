//! The `harness` section inside a `worker` block.
//!
//! # Why `harness` is not a reserved word
//!
//! The section opens on the IDENTIFIER `harness`, matched positionally where a worker
//! block's items stand. Reserving the word would retire it from every other position in
//! the language at once — a parameter, a field, a step could no longer be called
//! `harness` — and nothing about this section needs that. The word is unambiguous where
//! it stands: a worker block's items are `action` declarations and this, and neither can
//! be mistaken for the other.
//!
//! # Why every setting is one line
//!
//! An action's config keys share a line because they are five short words. A harness
//! carries absolute paths, argument lists and variable names; packed onto one line they
//! would wrap on every screen and a diff of a changed `cwd` would rewrite the whole
//! setting. One line per setting also means a refusal anchors on the field itself,
//! which is what lets the checker name a missing one.

use crate::ast::{
    DocLine, DurationLiteral, HARNESS_ARGS_FIELD, HARNESS_BINARY_FIELD, HARNESS_COMMAND_FIELD,
    HARNESS_CONCURRENCY_FIELD, HARNESS_CWD_FIELD, HARNESS_ENV_PASS_FIELD, HARNESS_EXIT_GRACE_FIELD,
    HARNESS_FIELDS, HARNESS_KIND_FIELD, HARNESS_PERMISSION_FIELD,
    HARNESS_RECONNECT_INITIAL_BACKOFF_FIELD, HARNESS_RECONNECT_MAX_ATTEMPTS_FIELD,
    HARNESS_RECONNECT_MAX_BACKOFF_FIELD, HARNESS_WORD, HarnessDecl, HarnessInteger, HarnessSetting,
    HarnessText, HarnessWord, Lead,
};
use crate::{Span, TokenKind};

use super::ParseError;
use super::stream::{Stream, describe};

/// Whether the stream stands on the opening word of a `harness` section.
pub(super) fn at_harness(stream: &mut Stream) -> bool {
    stream.peek_is(|kind| matches!(kind, TokenKind::Identifier(word) if word == HARNESS_WORD))
}

/// Parse a `harness` section; the stream stands on the `harness` word.
pub(super) fn parse_harness(
    stream: &mut Stream,
    lead: Vec<Lead>,
    docs: Vec<DocLine>,
) -> Result<HarnessDecl, ParseError> {
    let Some(opener) = stream.next() else {
        return Err(ParseError::new(
            stream.eof_span(),
            "expected a `harness` section, found end of input",
        ));
    };
    let span = opener.span;
    let trailing = stream.end_line()?;

    let mut section = HarnessDecl {
        span,
        lead,
        docs,
        trailing,
        kind: None,
        concurrency: None,
        reconnect_initial_backoff: None,
        reconnect_max_backoff: None,
        reconnect_max_attempts: None,
        env_pass: None,
        command: None,
        args: None,
        cwd: None,
        permission: None,
        exit_grace: None,
        binary: None,
    };

    let outer_leads = stream.take_leads()?;
    if !stream.open_block() {
        stream.push_back_leads(outer_leads);
        return Err(ParseError::new(
            span,
            format!(
                "the `{HARNESS_WORD}` section declares its settings in an indented block, and \
                 this one is empty; every setting is the operator's and none of them has a \
                 default, so an empty section would say nothing about how to launch anything"
            ),
        ));
    }
    stream.push_back_leads(outer_leads);
    loop {
        let setting_lead = stream.take_leads()?;
        if stream.at_item_block_end() {
            stream.push_back_leads(setting_lead);
            break;
        }
        let setting_docs = stream.take_docs();
        parse_setting(stream, &mut section, setting_lead, setting_docs)?;
    }
    let stray = stream.take_leads()?;
    stream.consume_block_dedent();
    stream.push_back_leads(stray);
    Ok(section)
}

/// Parse one `<field> <value>` line into its slot on `section`.
fn parse_setting(
    stream: &mut Stream,
    section: &mut HarnessDecl,
    lead: Vec<Lead>,
    docs: Vec<DocLine>,
) -> Result<(), ParseError> {
    let (field, field_span) = harness_field(stream)?;
    match field.as_str() {
        HARNESS_KIND_FIELD => {
            claim(section.kind.as_ref(), &field, field_span)?;
            let value = expect_word(stream, &field, field_span)?;
            section.kind = Some(finish(stream, field_span, lead, docs, value)?);
        }
        HARNESS_CONCURRENCY_FIELD => {
            claim(section.concurrency.as_ref(), &field, field_span)?;
            let value = expect_integer(stream, &field, field_span)?;
            section.concurrency = Some(finish(stream, field_span, lead, docs, value)?);
        }
        HARNESS_RECONNECT_INITIAL_BACKOFF_FIELD => {
            claim(
                section.reconnect_initial_backoff.as_ref(),
                &field,
                field_span,
            )?;
            let value = expect_duration(stream, &field, field_span)?;
            section.reconnect_initial_backoff =
                Some(finish(stream, field_span, lead, docs, value)?);
        }
        HARNESS_RECONNECT_MAX_BACKOFF_FIELD => {
            claim(section.reconnect_max_backoff.as_ref(), &field, field_span)?;
            let value = expect_duration(stream, &field, field_span)?;
            section.reconnect_max_backoff = Some(finish(stream, field_span, lead, docs, value)?);
        }
        HARNESS_RECONNECT_MAX_ATTEMPTS_FIELD => {
            claim(section.reconnect_max_attempts.as_ref(), &field, field_span)?;
            let value = expect_integer(stream, &field, field_span)?;
            section.reconnect_max_attempts = Some(finish(stream, field_span, lead, docs, value)?);
        }
        HARNESS_ENV_PASS_FIELD => {
            claim(section.env_pass.as_ref(), &field, field_span)?;
            let value = expect_text_list(stream, &field, field_span)?;
            section.env_pass = Some(finish(stream, field_span, lead, docs, value)?);
        }
        HARNESS_COMMAND_FIELD => {
            claim(section.command.as_ref(), &field, field_span)?;
            let value = expect_text(stream, &field, field_span)?;
            section.command = Some(finish(stream, field_span, lead, docs, value)?);
        }
        HARNESS_ARGS_FIELD => {
            claim(section.args.as_ref(), &field, field_span)?;
            let value = expect_text_list(stream, &field, field_span)?;
            section.args = Some(finish(stream, field_span, lead, docs, value)?);
        }
        HARNESS_CWD_FIELD => {
            claim(section.cwd.as_ref(), &field, field_span)?;
            let value = expect_text(stream, &field, field_span)?;
            section.cwd = Some(finish(stream, field_span, lead, docs, value)?);
        }
        HARNESS_PERMISSION_FIELD => {
            claim(section.permission.as_ref(), &field, field_span)?;
            let value = expect_text(stream, &field, field_span)?;
            section.permission = Some(finish(stream, field_span, lead, docs, value)?);
        }
        HARNESS_EXIT_GRACE_FIELD => {
            claim(section.exit_grace.as_ref(), &field, field_span)?;
            let value = expect_duration(stream, &field, field_span)?;
            section.exit_grace = Some(finish(stream, field_span, lead, docs, value)?);
        }
        HARNESS_BINARY_FIELD => {
            claim(section.binary.as_ref(), &field, field_span)?;
            let value = expect_text(stream, &field, field_span)?;
            section.binary = Some(finish(stream, field_span, lead, docs, value)?);
        }
        unknown => {
            return Err(ParseError::new(
                field_span,
                format!(
                    "`{unknown}` is not a `{HARNESS_WORD}` setting; the section carries {}",
                    HARNESS_FIELDS.map(|name| format!("`{name}`")).join(", ")
                ),
            ));
        }
    }
    Ok(())
}

/// The field word opening a setting line.
///
/// Read as an IDENTIFIER: none of the field words is reserved, so `command` and `args`
/// stay usable everywhere else in the language. A reserved word standing here is named
/// rather than silently treated as a field, because the mistake it represents (a config
/// key written in the wrong block) reads nothing like a typo.
fn harness_field(stream: &mut Stream) -> Result<(String, Span), ParseError> {
    let Some(token) = stream.peek() else {
        return Err(ParseError::new(
            stream.eof_span(),
            format!("expected a `{HARNESS_WORD}` setting, found end of input"),
        ));
    };
    let span = token.span;
    match &token.kind {
        TokenKind::Identifier(word) => {
            let word = word.clone();
            stream.next();
            Ok((word, span))
        }
        other => {
            let found = describe(other);
            Err(ParseError::new(
                span,
                format!(
                    "expected a `{HARNESS_WORD}` setting, found {found}; the section carries {}",
                    HARNESS_FIELDS.map(|name| format!("`{name}`")).join(", ")
                ),
            ))
        }
    }
}

/// Refuses a second declaration of a setting already given.
///
/// A repeated setting has no honest reading: taking the first silently ignores what the
/// operator wrote second, and taking the second silently ignores the first. Both would
/// launch an agent with a value nobody could point at in the file.
fn claim<V>(slot: Option<&HarnessSetting<V>>, field: &str, span: Span) -> Result<(), ParseError> {
    if slot.is_some() {
        return Err(ParseError::new(
            span,
            format!("duplicate `{field}` setting in the `{HARNESS_WORD}` section"),
        ));
    }
    Ok(())
}

/// Closes a setting line, attaching its trailing comment.
fn finish<V>(
    stream: &mut Stream,
    span: Span,
    lead: Vec<Lead>,
    docs: Vec<DocLine>,
    value: V,
) -> Result<HarnessSetting<V>, ParseError> {
    let trailing = stream.end_line()?;
    Ok(HarnessSetting {
        span,
        lead,
        docs,
        trailing,
        value,
    })
}

/// A bare word value.
fn expect_word(stream: &mut Stream, field: &str, anchor: Span) -> Result<HarnessWord, ParseError> {
    let Some(token) = stream.peek() else {
        return Err(missing_value(anchor, field, "a word"));
    };
    let span = token.span;
    match &token.kind {
        TokenKind::Identifier(word) => {
            let word = word.clone();
            stream.next();
            Ok(HarnessWord { span, word })
        }
        TokenKind::Newline | TokenKind::Comment(_) => Err(missing_value(anchor, field, "a word")),
        other => {
            let found = describe(other);
            Err(ParseError::new(
                span,
                format!("`{field}` takes a word, found {found}"),
            ))
        }
    }
}

/// A whole-number value.
fn expect_integer(
    stream: &mut Stream,
    field: &str,
    anchor: Span,
) -> Result<HarnessInteger, ParseError> {
    let Some(token) = stream.peek() else {
        return Err(missing_value(anchor, field, "a whole number"));
    };
    let span = token.span;
    match token.kind {
        TokenKind::Integer(value) => {
            stream.next();
            Ok(HarnessInteger { span, value })
        }
        TokenKind::Newline | TokenKind::Comment(_) => {
            Err(missing_value(anchor, field, "a whole number"))
        }
        ref other => {
            let found = describe(other);
            Err(ParseError::new(
                span,
                format!("`{field}` takes a whole number, found {found}"),
            ))
        }
    }
}

/// A duration literal value.
fn expect_duration(
    stream: &mut Stream,
    field: &str,
    anchor: Span,
) -> Result<DurationLiteral, ParseError> {
    super::workers::expect_duration(
        stream,
        anchor,
        &format!("`{field}` takes a duration (`5s`, `2m`, `1h`, `1d`)"),
    )
}

/// A quoted string value.
fn expect_text(stream: &mut Stream, field: &str, anchor: Span) -> Result<HarnessText, ParseError> {
    let Some(token) = stream.peek() else {
        return Err(missing_value(anchor, field, "a quoted string"));
    };
    let span = token.span;
    match &token.kind {
        TokenKind::String(value) => {
            let value = value.clone();
            stream.next();
            Ok(HarnessText { span, value })
        }
        TokenKind::Newline | TokenKind::Comment(_) => {
            Err(missing_value(anchor, field, "a quoted string"))
        }
        other => {
            let found = describe(other);
            Err(ParseError::new(
                span,
                format!("`{field}` takes a quoted string, found {found}"),
            ))
        }
    }
}

/// One or more quoted strings, comma-separated.
///
/// At least one is required: a list field written with nothing after it is an unfinished
/// line, not a declaration of emptiness. A section that wants no arguments omits `args`
/// entirely, which reads as what it is.
fn expect_text_list(
    stream: &mut Stream,
    field: &str,
    anchor: Span,
) -> Result<Vec<HarnessText>, ParseError> {
    let mut values = vec![expect_text(stream, field, anchor)?];
    while stream
        .eat(|kind| matches!(kind, TokenKind::Comma))
        .is_some()
    {
        values.push(expect_text(stream, field, anchor)?);
    }
    Ok(values)
}

/// The refusal for a setting line that names a field and then ends.
fn missing_value(anchor: Span, field: &str, shape: &str) -> ParseError {
    ParseError::new(
        anchor,
        format!("`{field}` needs {shape}; nothing here supplies a value it does not carry"),
    )
}
