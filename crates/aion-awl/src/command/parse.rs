//! The command declaration's grammar.
//!
//! # Amendment 1 is STRUCTURAL, not advisory
//!
//! A gate leg consumes the command half of the absorbed vocabulary and
//! nothing else. Recipes, dependency edges, and multi-step assembly are not
//! discouraged here — they are UNREPRESENTABLE. There is no production for a
//! recipe body, none for a dependency list, none for a second step. The words
//! that would open one ([`ASSEMBLY_WORDS`]) are recognised only so that the
//! refusal can say WHY, and the reason rides in the diagnostic: a gate leg's
//! review property is that a reader sees the exact command that ran, and a
//! command assembled from parts at run time is a command no reader saw.
//!
//! # The clause set is closed
//!
//! A command body admits exactly the clauses in [`CLAUSE_WORDS`]. Anything
//! else is refused by name, with the admitted set printed — the same
//! discipline the top-level dispatcher already applies, for the same reason:
//! an author who mistypes a clause must be told what the language actually
//! has, not left to guess.

use crate::ast::{DocLine, Lead, join_span};
use crate::parser::{ParseError, Stream, describe};
use crate::{Span, TokenKind};

use super::cause::CommandCause;
use super::clauses::{
    parse_cwd, parse_end_of_options_marker, parse_env, parse_hardening, parse_operand,
    parse_option, parse_parameters, parse_program, parse_specimen, parse_timeout,
    refusal_to_parse_error,
};
use super::decl::{
    CommandDecl, CommandParam, CommandTimeout, CwdDecl, EndOfOptionsClause, EnvBinding,
    HardeningDecl, LeadingDashSpecimen, OperandDecl, OptionDecl, ProgramDecl,
};
use super::error::CommandRefusal;
use super::words::{
    ADMITS_WORD, ASSEMBLY_WORDS, CLAUSE_WORDS, CWD_WORD, ENV_WORD, HARDENING_WORD, OPERAND_WORD,
    OPTION_WORD, OWNER_WORD, PROGRAM_WORD, REFUSES_WORD, SPECIMEN_WORD,
};

/// Parse one `command` declaration. The `command` word has NOT been consumed.
///
/// # Errors
///
/// Returns a [`ParseError`] carrying the rendered [`CommandRefusal`] — the
/// document's own diagnostic channel, so a command refusal reaches an author
/// through exactly the path every other parse refusal does, cause code and
/// observed bytes included.
pub(crate) fn parse_command_decl(
    stream: &mut Stream,
    lead: Vec<Lead>,
    docs: Vec<DocLine>,
) -> Result<CommandDecl, ParseError> {
    let Some(keyword) = stream.next() else {
        return Err(ParseError::new(
            stream.peek_span(),
            "expected a `command` declaration, found end of input",
        ));
    };
    let keyword_span = keyword.span;
    let (name, name_span) = stream.expect_name("a command name")?;
    let span = join_span(keyword_span, name_span);
    let parameters = parse_parameters(stream, &name)?;
    let trailing = stream.end_line()?;

    let mut body = Body::default();
    let outer_leads = stream.take_leads()?;
    if stream.open_block() {
        stream.push_back_leads(outer_leads);
        loop {
            let clause_lead = stream.take_leads()?;
            if stream.at_item_block_end() {
                stream.push_back_leads(clause_lead);
                break;
            }
            let _ = stream.take_docs();
            parse_clause(stream, &mut body, &name)?;
        }
        let stray = stream.take_leads()?;
        stream.consume_block_dedent();
        stream.push_back_leads(stray);
    } else {
        stream.push_back_leads(outer_leads);
    }

    body.finish(Header {
        span,
        lead,
        docs,
        trailing,
        name,
        name_span,
        parameters,
    })
}

/// The clauses read so far, gathered before they become a declaration.
///
/// Gathering rather than resolving is deliberate. Everything a later ruling
/// must be able to SEE — a second `admits`, a second `hardening`, a missing
/// `program` — is kept, because a parser that collapsed them would leave the
/// checker unable to name what it refuses.
#[derive(Default)]
struct Body {
    program: Option<ProgramDecl>,
    end_of_options: Vec<EndOfOptionsClause>,
    specimen: Option<LeadingDashSpecimen>,
    options: Vec<OptionDecl>,
    operands: Vec<OperandDecl>,
    env: Vec<EnvBinding>,
    cwd: Option<CwdDecl>,
    hardening: Vec<HardeningDecl>,
    timeout: Option<CommandTimeout>,
    clauses_seen: usize,
}

impl Body {
    fn finish(self, header: Header) -> Result<CommandDecl, ParseError> {
        if self.clauses_seen == 0 {
            return Err(refusal_to_parse_error(&CommandRefusal::refuse(
                CommandCause::BodyEmpty,
                header.span,
                &header.name,
                format!(
                    "command `{}` states nothing; a command declaration names at least a \
                     `{PROGRAM_WORD}` and its end-of-options property",
                    header.name
                ),
            )));
        }
        Ok(CommandDecl {
            span: header.span,
            lead: header.lead,
            docs: header.docs,
            trailing: header.trailing,
            name: header.name,
            name_span: header.name_span,
            parameters: header.parameters,
            program: self.program,
            end_of_options: self.end_of_options,
            specimen: self.specimen,
            options: self.options,
            operands: self.operands,
            env: self.env,
            cwd: self.cwd,
            hardening: self.hardening,
            timeout: self.timeout,
        })
    }
}

/// Everything the declaration's header line carried.
struct Header {
    span: Span,
    lead: Vec<Lead>,
    docs: Vec<DocLine>,
    trailing: Option<crate::ast::Comment>,
    name: String,
    name_span: Span,
    parameters: Vec<CommandParam>,
}

/// Read one clause line of a command body.
fn parse_clause(stream: &mut Stream, body: &mut Body, name: &str) -> Result<(), ParseError> {
    let Some(token) = stream.peek() else {
        return Err(ParseError::new(
            stream.peek_span(),
            format!("expected a clause inside command `{name}`, found end of input"),
        ));
    };
    let span = token.span;
    // The word, whether the lexer made it a keyword or an identifier. Read
    // before dispatching, because a refused ASSEMBLY form must be refused by
    // name whichever the lexer made it — `after` is a reserved word of this
    // language AND the spelling of a dependency edge, and it would otherwise
    // fall through to a generic "unknown clause" that says nothing about why
    // a gate leg has no dependencies.
    let word = match &token.kind {
        TokenKind::Identifier(word) => word.clone(),
        TokenKind::Keyword(keyword) => keyword.as_word().to_owned(),
        other => {
            let described = describe(other);
            return Err(refusal_to_parse_error(&CommandRefusal::refuse(
                CommandCause::ClauseUnknown,
                span,
                &described,
                format!(
                    "expected one of {} inside command `{name}`, found {described}",
                    admitted_clause_list()
                ),
            )));
        }
    };

    if let Some(refusal) = assembly_refusal(&word, span) {
        return Err(refusal);
    }

    if matches!(token.kind, TokenKind::Keyword(crate::Keyword::Timeout)) {
        stream.next();
        body.timeout = Some(parse_timeout(stream, span, name)?);
        body.clauses_seen += 1;
        return Ok(());
    }
    if !matches!(token.kind, TokenKind::Identifier(_)) {
        return Err(refusal_to_parse_error(&CommandRefusal::refuse(
            CommandCause::ClauseUnknown,
            span,
            &word,
            format!(
                "expected one of {} inside command `{name}`, found the reserved word `{word}`",
                admitted_clause_list()
            ),
        )));
    }

    stream.next();
    dispatch_clause(stream, body, name, &word, span)?;
    body.clauses_seen += 1;
    Ok(())
}

/// The refusal a recipe, dependency-edge or assembly word earns, or `None`
/// when the word is not one of them.
///
/// Its own function so that the reason — the review property a gate leg exists
/// to have — is written once and cannot drift from the list it applies to.
fn assembly_refusal(word: &str, span: Span) -> Option<ParseError> {
    let (_, form) = ASSEMBLY_WORDS
        .into_iter()
        .find(|(assembly, _)| *assembly == word)?;
    Some(refusal_to_parse_error(&CommandRefusal::refuse(
        CommandCause::RecipeFormRefused,
        span,
        word,
        format!(
            "`{word}` would open {form}, and a gate-leg command declaration has no production for \
             one: a gate leg consumes the command half only. The reason is the review property — \
             a reader of a gate leg sees the exact command that ran, and a command assembled from \
             parts at run time is a command no reader saw. Declare each command whole, on its own"
        ),
    )))
}

/// Read the rest of a clause line, once its opening word is known to be one
/// the body admits.
fn dispatch_clause(
    stream: &mut Stream,
    body: &mut Body,
    name: &str,
    word: &str,
    span: Span,
) -> Result<(), ParseError> {
    match word {
        PROGRAM_WORD => {
            body.program = Some(parse_program(stream, span, name)?);
        }
        ADMITS_WORD | REFUSES_WORD => {
            let admits = word == ADMITS_WORD;
            parse_end_of_options_marker(stream, span, name, word)?;
            body.end_of_options
                .push(EndOfOptionsClause { span, admits });
        }
        SPECIMEN_WORD => {
            body.specimen = Some(parse_specimen(stream, span, name)?);
        }
        OPTION_WORD => body.options.push(parse_option(stream, span, name)?),
        OPERAND_WORD => body.operands.push(parse_operand(stream, span, name)?),
        ENV_WORD => body.env.push(parse_env(stream, span, name)?),
        CWD_WORD => body.cwd = Some(parse_cwd(stream, span, name)?),
        HARDENING_WORD => body.hardening.push(parse_hardening(stream, span, name)?),
        OWNER_WORD => {
            return Err(refusal_to_parse_error(&CommandRefusal::refuse(
                CommandCause::ClauseUnknown,
                span,
                word,
                format!(
                    "`{OWNER_WORD}` names who owns a declared timeout and stands on the `timeout` \
                     line itself: write `timeout <duration> {OWNER_WORD} \"<who>\"`"
                ),
            )));
        }
        other => {
            return Err(refusal_to_parse_error(&CommandRefusal::refuse(
                CommandCause::ClauseUnknown,
                span,
                other,
                format!(
                    "expected one of {} inside command `{name}`, found `{other}`",
                    admitted_clause_list()
                ),
            )));
        }
    }
    Ok(())
}

/// The admitted clause set, rendered for a diagnostic.
fn admitted_clause_list() -> String {
    let mut rendered: Vec<String> = CLAUSE_WORDS
        .into_iter()
        .map(|word| format!("`{word}`"))
        .collect();
    rendered.push("`timeout`".to_owned());
    rendered.join(", ")
}
