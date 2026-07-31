//! The indented block under an `action` declaration: its optional config
//! line and its optional declarative body.
//!
//! Split from `workers` so each file stays inside the 500-line law, and
//! because the body grammar is its own concern: a config line says how an
//! action is DELIVERED, a body says what it DOES.

use crate::ast::{ActionBody, ConfigLine, Lead, join_span};
use crate::{Keyword, RUN_WORD, TokenKind};

use super::ParseError;
use super::stream::{Stream, describe};
use super::workers::parse_config_line;

/// Parse the optional indented block under an action declaration: at most
/// one config line (`advisory, node …, timeout …, retry …`) and at most one
/// body (`run "…"`), in either written order. The canonical printer emits
/// the config line first; both orders parse, and the non-canonical one
/// normalizes.
pub(super) fn parse_action_block(
    stream: &mut Stream,
) -> Result<(Option<ConfigLine>, Option<ActionBody>), ParseError> {
    let leads = stream.take_leads()?;
    if !opens_action_block(stream) {
        stream.push_back_leads(leads);
        return Ok((None, None));
    }
    stream.eat(|kind| matches!(kind, TokenKind::Indent));
    stream.push_back_leads(leads);

    let mut config: Option<ConfigLine> = None;
    let mut body: Option<ActionBody> = None;
    loop {
        let line_lead = stream.take_leads()?;
        if stream.at_item_block_end() {
            stream.push_back_leads(line_lead);
            break;
        }
        let line_span = stream.peek_span();
        if stream.peek_is(is_run_word) {
            if body.is_some() {
                return Err(ParseError::new(
                    line_span,
                    "an action declares at most one `run` body",
                ));
            }
            body = Some(parse_run_body(stream, line_lead)?);
            continue;
        }
        if !stream.peek_is(is_config_key) {
            let found = stream
                .peek()
                .map_or_else(|| "end of input".to_owned(), |token| describe(&token.kind));
            return Err(ParseError::new(
                line_span,
                format!(
                    "expected a config key (`advisory`, `node`, `timeout`, `retry`) or a \
                     `run` command body, found {found}"
                ),
            ));
        }
        if config.is_some() {
            return Err(ParseError::new(
                line_span,
                "an action declares at most one config line; write every key on it, \
                 comma-separated",
            ));
        }
        config = Some(parse_config_line(stream, line_lead)?);
    }
    let stray = stream.take_leads()?;
    stream.consume_block_dedent();
    stream.push_back_leads(stray);
    Ok((config, body))
}

/// Whether an indented block opening here belongs to the action: its first
/// line must start with a config key or the `run` body word. Anything else
/// (the next `action`, a `step`) belongs to an outer construct.
fn opens_action_block(stream: &mut Stream) -> bool {
    stream.peek_is(|kind| matches!(kind, TokenKind::Indent))
        && stream
            .peek_second()
            .is_some_and(|token| is_config_key(&token.kind) || is_run_word(&token.kind))
}

/// Whether `kind` opens a config line.
fn is_config_key(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Keyword(
            Keyword::Node | Keyword::Timeout | Keyword::Retry | Keyword::Advisory | Keyword::Agent,
        )
    )
}

/// Whether `kind` is the body word `run`.
///
/// `run` is recognised POSITIONALLY, as an ordinary identifier in the one
/// grammar position where nothing else may stand, and is deliberately not a
/// reserved keyword: `step run`, `input run: String` and a binding named
/// `run` all predate this feature and must keep working, including where an
/// expression references them. Reserving the word would have broken them.
fn is_run_word(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::Identifier(name) if name == RUN_WORD)
}

/// Parse one `run "<command>"` body line; the `run` word is at the cursor.
fn parse_run_body(stream: &mut Stream, lead: Vec<Lead>) -> Result<ActionBody, ParseError> {
    let keyword = stream
        .eat(is_run_word)
        .ok_or_else(|| ParseError::new(stream.peek_span(), "expected a `run` command body"))?;
    let (command, command_span) = match stream.peek() {
        Some(token) => {
            let span = token.span;
            match &token.kind {
                TokenKind::String(value) => {
                    let value = value.clone();
                    stream.next();
                    (value, span)
                }
                TokenKind::Newline | TokenKind::Comment(_) => {
                    return Err(ParseError::new(
                        keyword.span,
                        "`run` takes a quoted command line: write `run \"<command>\"`",
                    ));
                }
                other => {
                    return Err(ParseError::new(
                        span,
                        format!(
                            "`run` takes a quoted command line, found {}",
                            describe(other)
                        ),
                    ));
                }
            }
        }
        None => {
            return Err(ParseError::new(
                keyword.span,
                "`run` takes a quoted command line, found end of input",
            ));
        }
    };
    let trailing = stream.end_line()?;
    Ok(ActionBody::Run {
        span: join_span(keyword.span, command_span),
        lead,
        trailing,
        command,
        command_span,
    })
}
