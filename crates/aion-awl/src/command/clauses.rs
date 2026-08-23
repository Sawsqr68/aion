//! One function per clause of a command body, and the token helpers they
//! share.
//!
//! Each clause reads its own line whole and refuses anything it cannot keep.
//! A trailing comment on a clause line is refused rather than dropped: the
//! canonical printer must reproduce a document byte for byte, and trivia it
//! has nowhere to put is trivia it would silently lose.

use crate::ast::{DurationLiteral, join_span};
use crate::parser::{ParseError, Stream, describe, parse_type_ref, type_ref_span};
use crate::{Span, TokenKind};

use super::cause::CommandCause;
use super::decl::{
    CommandParam, CommandTimeout, CwdDecl, END_OF_OPTIONS_MARKER, EnvBinding, HardeningDecl,
    HardeningLimit, HardeningPath, LeadingDashSpecimen, OperandDecl, OptionDecl, ProgramDecl,
    ProgramWord, TypedFill,
};
use super::error::CommandRefusal;
use super::fill::Fill;
use super::words::{
    CWD_WORD, ENV_WORD, HARDENING_WORD, OPEN_FILES_WORD, OPERAND_WORD, OPTION_WORD, OWNER_WORD,
    PATH_WORD, PROGRAM_WORD, SPECIMEN_WORD,
};

/// `command name(p: Type, q: Type = "default")`.
pub(super) fn parse_parameters(
    stream: &mut Stream,
    name: &str,
) -> Result<Vec<CommandParam>, ParseError> {
    if !stream.peek_is(|kind| matches!(kind, TokenKind::LeftParen)) {
        return Ok(Vec::new());
    }
    stream.expect(
        &TokenKind::LeftParen,
        "expected `(` to open the parameter list",
    )?;
    let mut parameters = Vec::new();
    loop {
        if stream
            .eat(|kind| matches!(kind, TokenKind::RightParen))
            .is_some()
        {
            return Ok(parameters);
        }
        let (parameter, parameter_span) = stream.expect_name("a parameter name")?;
        if stream
            .eat(|kind| matches!(kind, TokenKind::Colon))
            .is_none()
        {
            return Err(ParseError::new(
                parameter_span,
                format!(
                    "every command `{name}` parameter needs a type: write `{parameter}: <Type>`"
                ),
            ));
        }
        let ty = parse_type_ref(stream)?;
        let mut end = type_ref_span(&ty);
        let default = if stream
            .eat(|kind| matches!(kind, TokenKind::Equal))
            .is_some()
        {
            let (text, literal_span) = expect_string(stream, "a default value")?;
            end = literal_span;
            Some(fill(&text, literal_span)?)
        } else {
            None
        };
        parameters.push(CommandParam {
            span: join_span(parameter_span, end),
            name: parameter,
            name_span: parameter_span,
            ty,
            default,
        });
        if stream
            .eat(|kind| matches!(kind, TokenKind::Comma))
            .is_some()
        {
            continue;
        }
        stream.expect(
            &TokenKind::RightParen,
            "expected `)` to close the parameter list",
        )?;
        return Ok(parameters);
    }
}

/// `program "git" "diff"`.
pub(super) fn parse_program(
    stream: &mut Stream,
    span: Span,
    name: &str,
) -> Result<ProgramDecl, ParseError> {
    let mut words = Vec::new();
    while let Some((text, word_span)) = eat_string(stream) {
        words.push(ProgramWord {
            span: word_span,
            text,
        });
    }
    let end = stream.end_line()?;
    if end.is_some() {
        // A trailing comment on a program line is trivia the printer keeps on
        // the declaration, not on the clause; dropping it here would lose it.
        return Err(ParseError::new(
            span,
            format!(
                "command `{name}`'s `{PROGRAM_WORD}` clause carries a trailing comment; put the \
                 comment on its own line above the clause so the canonical printer can keep it"
            ),
        ));
    }
    if words.is_empty() {
        return Err(refusal_to_parse_error(&CommandRefusal::refuse(
            CommandCause::ProgramWordsEmpty,
            span,
            PROGRAM_WORD,
            format!(
                "command `{name}`'s `{PROGRAM_WORD}` clause names no words; write `{PROGRAM_WORD} \
                 \"<program>\"` followed by the subcommand words that qualify it — the declared \
                 unit is the INVOCATION, because whether `--` separates options from operands is \
                 a fact about `git tag`, not about `git`"
            ),
        )));
    }
    Ok(ProgramDecl { span, words })
}

/// `admits "--"` / `refuses "--"` — the marker's bytes are the contract.
pub(super) fn parse_end_of_options_marker(
    stream: &mut Stream,
    span: Span,
    name: &str,
    word: &str,
) -> Result<(), ParseError> {
    let (marker, marker_span) = expect_string(stream, "the end-of-options marker `\"--\"`")?;
    end_clause_line(stream, span, name, word)?;
    if marker != END_OF_OPTIONS_MARKER {
        return Err(refusal_to_parse_error(&CommandRefusal::refuse(
            CommandCause::EndOfOptionsMarkerWrong,
            marker_span,
            &marker,
            format!(
                "`{word}` names the end-of-options marker, which is exactly \
                 `\"{END_OF_OPTIONS_MARKER}\"`; there is no other marker for this language to \
                 place or withhold"
            ),
        )));
    }
    Ok(())
}

/// `specimen "-x"`.
pub(super) fn parse_specimen(
    stream: &mut Stream,
    span: Span,
    name: &str,
) -> Result<LeadingDashSpecimen, ParseError> {
    let (bytes, bytes_span) = expect_string(stream, "the specimen bytes")?;
    end_clause_line(stream, span, name, SPECIMEN_WORD)?;
    Ok(LeadingDashSpecimen {
        span,
        bytes,
        bytes_span,
    })
}

/// `option "--name-only"` or `option "--max-count": Int = "{{count}}"`.
pub(super) fn parse_option(
    stream: &mut Stream,
    span: Span,
    name: &str,
) -> Result<OptionDecl, ParseError> {
    let (flag, flag_span) = expect_string(stream, "the option's flag")?;
    let value = if stream
        .eat(|kind| matches!(kind, TokenKind::Colon))
        .is_some()
    {
        Some(parse_typed_fill(stream, &flag)?)
    } else {
        None
    };
    end_clause_line(stream, span, name, OPTION_WORD)?;
    Ok(OptionDecl {
        span,
        flag,
        flag_span,
        value,
    })
}

/// `operand range: String = "{{base_pin}}..HEAD"`.
pub(super) fn parse_operand(
    stream: &mut Stream,
    span: Span,
    name: &str,
) -> Result<OperandDecl, ParseError> {
    let (operand, operand_span) = stream.expect_name("an operand name")?;
    if stream
        .eat(|kind| matches!(kind, TokenKind::Colon))
        .is_none()
    {
        return Err(ParseError::new(
            operand_span,
            format!("every operand needs a type: write `{OPERAND_WORD} {operand}: <Type> = \"…\"`"),
        ));
    }
    let value = parse_typed_fill(stream, &operand)?;
    end_clause_line(stream, span, name, OPERAND_WORD)?;
    Ok(OperandDecl {
        span,
        name: operand,
        name_span: operand_span,
        value,
    })
}

/// `: <Type> = "<fill>"`, the colon already consumed.
fn parse_typed_fill(stream: &mut Stream, what: &str) -> Result<TypedFill, ParseError> {
    let ty = parse_type_ref(stream)?;
    if stream
        .eat(|kind| matches!(kind, TokenKind::Equal))
        .is_none()
    {
        return Err(ParseError::new(
            type_ref_span(&ty),
            format!("`{what}` declares a type and no value: write `= \"…\"` after the type"),
        ));
    }
    let (text, literal_span) = expect_string(stream, "the value")?;
    Ok(TypedFill {
        ty,
        fill: fill(&text, literal_span)?,
    })
}

/// `env NAME = "<fill>"` — structural, never a `K=V` string prefix.
pub(super) fn parse_env(
    stream: &mut Stream,
    span: Span,
    name: &str,
) -> Result<EnvBinding, ParseError> {
    if let Some((text, literal_span)) = eat_string(stream) {
        return Err(refusal_to_parse_error(&CommandRefusal::refuse(
            CommandCause::EnvStringPrefix,
            literal_span,
            &text,
            format!(
                "an environment binding is structural: write `{ENV_WORD} NAME = \"<value>\"`. A \
                 `\"K=V\"` string is a prefix some shell would have to split, and splitting is \
                 exactly what this surface exists to remove — the grammar has no production for it"
            ),
        )));
    }
    let (variable, variable_span) = expect_env_name(stream)?;
    if stream
        .eat(|kind| matches!(kind, TokenKind::Equal))
        .is_none()
    {
        return Err(ParseError::new(
            variable_span,
            format!("`{ENV_WORD} {variable}` declares no value: write `= \"<value>\"`"),
        ));
    }
    let (text, literal_span) = expect_string(stream, "the environment value")?;
    end_clause_line(stream, span, name, ENV_WORD)?;
    Ok(EnvBinding {
        span,
        name: variable,
        name_span: variable_span,
        fill: fill(&text, literal_span)?,
    })
}

/// `cwd "{workspace_root}"`.
pub(super) fn parse_cwd(
    stream: &mut Stream,
    span: Span,
    name: &str,
) -> Result<CwdDecl, ParseError> {
    let (value, value_span) = expect_string(stream, "the working directory")?;
    end_clause_line(stream, span, name, CWD_WORD)?;
    Ok(CwdDecl {
        span,
        value,
        value_span,
    })
}

/// `timeout <duration> owner "<who>"`, the `timeout` keyword consumed.
pub(super) fn parse_timeout(
    stream: &mut Stream,
    span: Span,
    name: &str,
) -> Result<CommandTimeout, ParseError> {
    let Some(token) = stream.next() else {
        return Err(ParseError::new(
            span,
            format!("command `{name}`'s `timeout` names no duration"),
        ));
    };
    let TokenKind::Duration { magnitude, unit } = token.kind else {
        return Err(ParseError::new(
            token.span,
            format!(
                "command `{name}`'s `timeout` takes a duration literal, found {}",
                describe(&token.kind)
            ),
        ));
    };
    let duration = DurationLiteral {
        span: token.span,
        magnitude,
        unit,
    };
    let Some(owner_token) =
        stream.eat(|kind| matches!(kind, TokenKind::Identifier(word) if word == OWNER_WORD))
    else {
        return Err(ParseError::new(
            token.span,
            format!(
                "command `{name}`'s `timeout` names no owner: write `timeout <duration> \
                 {OWNER_WORD} \"<who>\"`. A ceiling nobody owns is a number nobody chose"
            ),
        ));
    };
    let (owner, owner_span) = expect_string(stream, "the timeout's owner")?;
    end_clause_line(stream, span, name, "timeout")?;
    Ok(CommandTimeout {
        span: join_span(span, owner_token.span),
        duration,
        owner,
        owner_span,
    })
}

/// The `hardening` block: `path` and `open_files`, at least one of them.
pub(super) fn parse_hardening(
    stream: &mut Stream,
    span: Span,
    name: &str,
) -> Result<HardeningDecl, ParseError> {
    let trailing = stream.end_line()?;
    if trailing.is_some() {
        return Err(ParseError::new(
            span,
            format!(
                "command `{name}`'s `{HARDENING_WORD}` block carries a trailing comment; put it \
                 on its own line above the block"
            ),
        ));
    }
    let mut path = None;
    let mut open_files = None;
    let outer_leads = stream.take_leads()?;
    if stream.open_block() {
        stream.push_back_leads(outer_leads);
        loop {
            let member_lead = stream.take_leads()?;
            if stream.at_item_block_end() {
                stream.push_back_leads(member_lead);
                break;
            }
            let _ = stream.take_docs();
            let Some(token) = stream.peek() else {
                break;
            };
            let member_span = token.span;
            let TokenKind::Identifier(word) = &token.kind else {
                return Err(ParseError::new(
                    member_span,
                    format!(
                        "expected `{PATH_WORD}` or `{OPEN_FILES_WORD}` inside the \
                         `{HARDENING_WORD}` block, found {}",
                        describe(&token.kind)
                    ),
                ));
            };
            let word = word.clone();
            stream.next();
            match word.as_str() {
                PATH_WORD => {
                    let (text, literal_span) = expect_string(stream, "the PATH")?;
                    end_clause_line(stream, member_span, name, PATH_WORD)?;
                    path = Some(HardeningPath {
                        span: member_span,
                        fill: fill(&text, literal_span)?,
                    });
                }
                OPEN_FILES_WORD => {
                    let (value, value_span) = expect_integer(stream)?;
                    end_clause_line(stream, member_span, name, OPEN_FILES_WORD)?;
                    open_files = Some(HardeningLimit {
                        span: member_span,
                        value,
                        value_span,
                    });
                }
                other => {
                    return Err(ParseError::new(
                        member_span,
                        format!(
                            "expected `{PATH_WORD}` or `{OPEN_FILES_WORD}` inside the \
                             `{HARDENING_WORD}` block, found `{other}`"
                        ),
                    ));
                }
            }
        }
        let stray = stream.take_leads()?;
        stream.consume_block_dedent();
        stream.push_back_leads(stray);
    } else {
        stream.push_back_leads(outer_leads);
    }
    // An empty block is NOT refused here: `hardening_empty` is a check-layer
    // cause, and a parser that raised it would be filing a check refusal at
    // the parse layer — the exact conflation the carrier-contract law forbids.
    Ok(HardeningDecl {
        span,
        path,
        open_files,
    })
}

/// Close a clause line, refusing a trailing comment the printer could not keep.
fn end_clause_line(
    stream: &mut Stream,
    span: Span,
    name: &str,
    clause: &str,
) -> Result<(), ParseError> {
    if stream.end_line()?.is_some() {
        return Err(ParseError::new(
            span,
            format!(
                "command `{name}`'s `{clause}` clause carries a trailing comment; put the comment \
                 on its own line above the clause so the canonical printer can keep it"
            ),
        ));
    }
    Ok(())
}

/// Parse a fill, converting its refusal into the document's diagnostic
/// channel so cause code and observed bytes reach the author.
pub(super) fn fill(text: &str, span: Span) -> Result<Fill, ParseError> {
    Fill::parse(text, span).map_err(|refusal| refusal_to_parse_error(&refusal))
}

/// A command refusal as a document diagnostic. The rendered form carries the
/// observed bytes and the named cause, so nothing is lost in the crossing.
pub(super) fn refusal_to_parse_error(refusal: &CommandRefusal) -> ParseError {
    ParseError::new(refusal.span(), refusal.rendered())
}

/// Read a string literal, or say what stood there instead.
fn expect_string(stream: &mut Stream, what: &str) -> Result<(String, Span), ParseError> {
    let Some(token) = stream.next() else {
        return Err(ParseError::new(
            stream.peek_span(),
            format!("expected {what}, found end of input"),
        ));
    };
    match token.kind {
        TokenKind::String(text) => Ok((text, token.span)),
        other => Err(ParseError::new(
            token.span,
            format!("expected {what}, found {}", describe(&other)),
        )),
    }
}

/// Read a string literal if one stands here.
fn eat_string(stream: &mut Stream) -> Option<(String, Span)> {
    if !stream.peek_is(|kind| matches!(kind, TokenKind::String(_))) {
        return None;
    }
    let token = stream.next()?;
    match token.kind {
        TokenKind::String(text) => Some((text, token.span)),
        _ => None,
    }
}

/// Read an environment variable name. `SCREAMING_SNAKE` lexes as a type
/// identifier, `snake_case` as an identifier; both are legal names of an
/// environment variable, and neither is a string.
fn expect_env_name(stream: &mut Stream) -> Result<(String, Span), ParseError> {
    let Some(token) = stream.next() else {
        return Err(ParseError::new(
            stream.peek_span(),
            "expected an environment variable name, found end of input",
        ));
    };
    match token.kind {
        TokenKind::Identifier(name) | TokenKind::TypeIdentifier(name) => Ok((name, token.span)),
        other => Err(ParseError::new(
            token.span,
            format!(
                "expected an environment variable name, found {}",
                describe(&other)
            ),
        )),
    }
}

/// Read an integer literal.
fn expect_integer(stream: &mut Stream) -> Result<(u64, Span), ParseError> {
    let Some(token) = stream.next() else {
        return Err(ParseError::new(
            stream.peek_span(),
            "expected a whole number, found end of input",
        ));
    };
    match token.kind {
        TokenKind::Integer(value) => Ok((value, token.span)),
        other => Err(ParseError::new(
            token.span,
            format!("expected a whole number, found {}", describe(&other)),
        )),
    }
}
