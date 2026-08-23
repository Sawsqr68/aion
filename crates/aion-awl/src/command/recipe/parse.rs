//! The recipe surface's grammar: `recipe`, `wall`, and `artifact`.
//!
//! # Amendment 1's OTHER side is structural too
//!
//! `super::super::parse` makes recipes unrepresentable inside a gate-leg
//! command declaration. This module makes the converse unrepresentable: a
//! `command` declaration cannot be written INSIDE a recipe body. There is no
//! production for one, and the word is recognised only so the refusal can
//! say why — a command is declared at the top level and REFERENCED by a
//! step, so the command a reader of the document sees is the command that
//! ran. Nesting one would put a declaration where no reader looks for it and
//! would make the recipe layer reachable from a gate leg by a second route.
//!
//! # A step body is never a string
//!
//! `runs "cargo test"` is refused at the PARSE layer, by name, with its own
//! cause. A shell string in a step body is the whole defect family this
//! surface exists to remove, one level up from argv: a recipe that assembled
//! strings would produce commands no reader saw.

use crate::ast::{DocLine, Lead, join_span};
use crate::parser::{ParseError, Stream, describe, parse_type_ref};
use crate::{Keyword, Span, TokenKind};

use super::super::cause::CommandCause;
use super::super::clauses::refusal_to_parse_error;
use super::super::error::CommandRefusal;
use super::decl::{
    ArtifactDecl, Derivation, ParamLiteral, ParameterKind, RecipeDecl, RecipeParam, RecipeStep,
    StepBody, StepReference, WallDecl,
};
use super::words::{
    ALLOWS_WORD, ARTIFACT_CLAUSE_WORDS, ARTIFACT_WORD, CAPTURES_WORD, COMMAND_WORD, DERIVES_WORD,
    DIGEST_WORD, EXCLUDES_WORD, EXPECTING_WORD, EXTRACT_WORD, FIRES_WORD, PARAMETER_WORD,
    RECIPE_CLAUSE_WORDS, RECIPE_WORD, REQUIRES_WORD, RESTORES_WORD, RUNS_WORD, STEP_CLAUSE_WORDS,
    WALL_CLAUSE_WORDS, WALL_WORD, admitted,
};

/// Parse one `recipe` declaration. The `recipe` word has NOT been consumed.
///
/// # Errors
///
/// Returns a [`ParseError`] carrying the rendered [`CommandRefusal`], so a
/// recipe refusal reaches an author through the document's own diagnostic
/// channel with its cause code and observed bytes intact.
pub(crate) fn parse_recipe_decl(
    stream: &mut Stream,
    lead: Vec<Lead>,
    docs: Vec<DocLine>,
) -> Result<RecipeDecl, ParseError> {
    let (span, name, name_span, trailing) = header(stream, RECIPE_WORD, "a recipe name")?;
    let mut parameters = Vec::new();
    let mut steps = Vec::new();
    let mut clauses = 0usize;
    read_block(stream, |stream| {
        let (word, word_span, is_keyword) = clause_word(stream, RECIPE_WORD, &name)?;
        if is_keyword {
            if word == Keyword::Step.as_word() {
                stream.next();
                steps.push(parse_step(stream, word_span, &name)?);
                clauses += 1;
                return Ok(());
            }
            return Err(recipe_clause_unknown(&word, word_span, RECIPE_WORD, &name));
        }
        if word == COMMAND_WORD {
            return Err(nested_command(word_span, &name));
        }
        if word != PARAMETER_WORD {
            return Err(recipe_clause_unknown(&word, word_span, RECIPE_WORD, &name));
        }
        stream.next();
        parameters.push(parse_parameter(stream, word_span, &name)?);
        clauses += 1;
        Ok(())
    })?;
    if clauses == 0 {
        return Err(body_empty(RECIPE_WORD, &name, span));
    }
    Ok(RecipeDecl {
        span,
        lead,
        docs,
        trailing,
        name,
        name_span,
        parameters,
        steps,
    })
}

/// Parse one `wall` declaration. The `wall` word has NOT been consumed.
///
/// # Errors
///
/// Returns a [`ParseError`] for a clause the wall body does not admit, or a
/// body that states nothing.
pub(crate) fn parse_wall_decl(
    stream: &mut Stream,
    lead: Vec<Lead>,
    docs: Vec<DocLine>,
) -> Result<WallDecl, ParseError> {
    let (span, name, name_span, trailing) = header(stream, WALL_WORD, "a wall name")?;
    let mut allows = Vec::new();
    let mut excludes = Vec::new();
    let mut clauses = 0usize;
    read_block(stream, |stream| {
        let (word, word_span, is_keyword) = clause_word(stream, WALL_WORD, &name)?;
        if is_keyword {
            return Err(wall_clause_unknown(&word, word_span, &name));
        }
        stream.next();
        match word.as_str() {
            ALLOWS_WORD => allows.push(literal(
                stream,
                "a path pattern",
                word_span,
                &name,
                ALLOWS_WORD,
            )?),
            EXCLUDES_WORD => {
                excludes.push(literal(
                    stream,
                    "an excluded term",
                    word_span,
                    &name,
                    EXCLUDES_WORD,
                )?);
            }
            _ => return Err(wall_clause_unknown(&word, word_span, &name)),
        }
        clauses += 1;
        Ok(())
    })?;
    if clauses == 0 {
        return Err(body_empty(WALL_WORD, &name, span));
    }
    Ok(WallDecl {
        span,
        lead,
        docs,
        trailing,
        name,
        name_span,
        allows,
        excludes,
    })
}

/// Parse one `artifact` declaration. The `artifact` word has NOT been
/// consumed.
///
/// # Errors
///
/// Returns a [`ParseError`] for a missing address, a clause the artifact body
/// does not admit, or a body that states nothing.
pub(crate) fn parse_artifact_decl(
    stream: &mut Stream,
    lead: Vec<Lead>,
    docs: Vec<DocLine>,
) -> Result<ArtifactDecl, ParseError> {
    let Some(keyword) = stream.next() else {
        return Err(ParseError::new(
            stream.peek_span(),
            "expected an `artifact` declaration, found end of input",
        ));
    };
    let (name, name_span) = stream.expect_name("an artifact name")?;
    let span = join_span(keyword.span, name_span);
    let (address, address_span) = expect_string(stream, "the artifact's address")?;
    let trailing = stream.end_line()?;

    let mut digest = None;
    let mut restores = None;
    let mut excludes = Vec::new();
    read_block(stream, |stream| {
        let (word, word_span, is_keyword) = clause_word(stream, ARTIFACT_WORD, &name)?;
        if is_keyword {
            return Err(artifact_clause_unknown(&word, word_span, &name));
        }
        stream.next();
        match word.as_str() {
            DIGEST_WORD => {
                digest = Some(literal(
                    stream,
                    "the content digest",
                    word_span,
                    &name,
                    DIGEST_WORD,
                )?);
            }
            RESTORES_WORD => {
                restores = Some(literal(
                    stream,
                    "the path this artifact restores",
                    word_span,
                    &name,
                    RESTORES_WORD,
                )?);
            }
            EXCLUDES_WORD => {
                excludes.push(literal(
                    stream,
                    "an unfilled-slot sentinel",
                    word_span,
                    &name,
                    EXCLUDES_WORD,
                )?);
            }
            _ => return Err(artifact_clause_unknown(&word, word_span, &name)),
        }
        Ok(())
    })?;

    Ok(ArtifactDecl {
        span,
        lead,
        docs,
        trailing,
        name,
        name_span,
        address,
        address_span,
        digest,
        restores,
        excludes,
    })
}

/// `parameter control_expect: String re_derive derives "…" extract "…"`.
fn parse_parameter(
    stream: &mut Stream,
    span: Span,
    recipe: &str,
) -> Result<RecipeParam, ParseError> {
    let (name, name_span) = stream.expect_name("a parameter name")?;
    if stream
        .eat(|kind| matches!(kind, TokenKind::Colon))
        .is_none()
    {
        return Err(ParseError::new(
            name_span,
            format!(
                "every recipe `{recipe}` parameter needs a type: write `{PARAMETER_WORD} {name}: \
                 <Type> <kind>`"
            ),
        ));
    }
    let ty = parse_type_ref(stream)?;
    let default = if stream
        .eat(|kind| matches!(kind, TokenKind::Equal))
        .is_some()
    {
        let (text, literal_span) = expect_string(stream, "the parameter's default")?;
        Some(ParamLiteral {
            text,
            span: literal_span,
        })
    } else {
        None
    };

    let (kind_word, kind_span) = expect_word(stream, "the parameter's kind")?;
    let Some(kind) = ParameterKind::from_word(&kind_word) else {
        return Err(refusal_to_parse_error(&CommandRefusal::refuse(
            CommandCause::ParameterKindUnknown,
            kind_span,
            &kind_word,
            format!(
                "recipe `{recipe}`'s parameter `{name}` names the kind `{kind_word}`, which is \
                 not one of the declared taxonomy — {}. The kinds are read off the live composer \
                 corpus, not invented, and a kind outside the four says nothing a composition \
                 could act on",
                ParameterKind::admitted()
            ),
        )));
    };

    let derivation = if stream
        .peek_is(|kind| matches!(kind, TokenKind::Identifier(word) if word == DERIVES_WORD))
    {
        Some(parse_derivation(stream, recipe, &name)?)
    } else {
        None
    };
    end_clause_line(stream, span, recipe, PARAMETER_WORD)?;
    Ok(RecipeParam {
        span: join_span(span, kind_span),
        name,
        name_span,
        ty,
        kind,
        kind_span,
        default,
        derivation,
    })
}

/// `derives "<source address>" extract "<extraction>"`.
fn parse_derivation(
    stream: &mut Stream,
    recipe: &str,
    name: &str,
) -> Result<Derivation, ParseError> {
    let Some(derives) = stream.next() else {
        return Err(ParseError::new(
            stream.peek_span(),
            "expected `derives`, found end of input",
        ));
    };
    let (source, source_span) = expect_string(stream, "the derivation's source address")?;
    let Some(_) =
        stream.eat(|kind| matches!(kind, TokenKind::Identifier(word) if word == EXTRACT_WORD))
    else {
        return Err(ParseError::new(
            source_span,
            format!(
                "recipe `{recipe}`'s parameter `{name}` names a derivation source and no \
                 extraction: write `{DERIVES_WORD} \"<source>\" {EXTRACT_WORD} \"<extraction>\"`. \
                 The address and what is taken out of it are two declared things, because an \
                 address that encoded its own extraction would carry two truths in one string"
            ),
        ));
    };
    let (extraction, extraction_span) = expect_string(stream, "the derivation's extraction")?;
    Ok(Derivation {
        span: join_span(derives.span, extraction_span),
        source,
        source_span,
        extraction,
        extraction_span,
    })
}

/// `step compose` and its own indented clause block.
fn parse_step(stream: &mut Stream, span: Span, recipe: &str) -> Result<RecipeStep, ParseError> {
    let (name, name_span) = stream.expect_name("a step name")?;
    let trailing = stream.end_line()?;
    if trailing.is_some() {
        return Err(ParseError::new(
            span,
            format!(
                "recipe `{recipe}`'s step `{name}` carries a trailing comment on its header; put \
                 the comment on its own line above the step so the canonical printer can keep it"
            ),
        ));
    }
    let mut after = Vec::new();
    let mut requires = Vec::new();
    let mut body: Option<StepBody> = None;
    read_block(stream, |stream| {
        let (word, word_span, is_keyword) = clause_word(stream, "step", &name)?;
        if is_keyword {
            if word == Keyword::After.as_word() {
                stream.next();
                let (target, target_span) = stream.expect_name("a step name")?;
                end_clause_line(stream, word_span, recipe, Keyword::After.as_word())?;
                after.push(StepReference {
                    span: word_span,
                    name: target,
                    name_span: target_span,
                });
                return Ok(());
            }
            return Err(step_clause_unknown(&word, word_span, recipe, &name));
        }
        stream.next();
        match word.as_str() {
            REQUIRES_WORD => {
                requires.push(literal(
                    stream,
                    "an at-fire precondition",
                    word_span,
                    recipe,
                    REQUIRES_WORD,
                )?);
            }
            RUNS_WORD => body = Some(parse_runs(stream, word_span, recipe, &name)?),
            FIRES_WORD => body = Some(parse_fires(stream, word_span, recipe, &name)?),
            CAPTURES_WORD => body = Some(parse_captures(stream, word_span, recipe, &name)?),
            _ => return Err(step_clause_unknown(&word, word_span, recipe, &name)),
        }
        Ok(())
    })?;
    Ok(RecipeStep {
        span: join_span(span, name_span),
        name,
        name_span,
        after,
        requires,
        body,
    })
}

/// `runs command <name>` — and the refusal a shell string earns instead.
fn parse_runs(
    stream: &mut Stream,
    span: Span,
    recipe: &str,
    step: &str,
) -> Result<StepBody, ParseError> {
    if let Some((text, literal_span)) = eat_string(stream) {
        return Err(refusal_to_parse_error(&CommandRefusal::refuse(
            CommandCause::ShellStringBody,
            literal_span,
            &text,
            format!(
                "recipe `{recipe}`'s step `{step}` writes its body as a shell string. A step runs \
                 a DECLARED command — write `{RUNS_WORD} {COMMAND_WORD} <name>` and declare the \
                 command at the top level — because a string some shell re-splits is a command no \
                 reader saw, which is the whole property a declared surface exists to keep"
            ),
        )));
    }
    let Some(_) =
        stream.eat(|kind| matches!(kind, TokenKind::Identifier(word) if word == COMMAND_WORD))
    else {
        return Err(ParseError::new(
            span,
            format!(
                "recipe `{recipe}`'s step `{step}` says `{RUNS_WORD}` and does not say what: \
                 write `{RUNS_WORD} {COMMAND_WORD} <name>`, naming a command the document declares"
            ),
        ));
    };
    let (command, command_span) = stream.expect_name("a declared command's name")?;
    end_clause_line(stream, span, recipe, RUNS_WORD)?;
    Ok(StepBody::RunsCommand {
        span: join_span(span, command_span),
        command,
        command_span,
    })
}

/// `fires artifact <name> expecting "<known answer>"`.
fn parse_fires(
    stream: &mut Stream,
    span: Span,
    recipe: &str,
    step: &str,
) -> Result<StepBody, ParseError> {
    let Some(_) =
        stream.eat(|kind| matches!(kind, TokenKind::Identifier(word) if word == ARTIFACT_WORD))
    else {
        return Err(ParseError::new(
            span,
            format!(
                "recipe `{recipe}`'s step `{step}` says `{FIRES_WORD}` and does not say what: \
                 write `{FIRES_WORD} {ARTIFACT_WORD} <name> {EXPECTING_WORD} \"<known answer>\"`"
            ),
        ));
    };
    let (artifact, artifact_span) = stream.expect_name("a declared artifact's name")?;
    let Some(_) =
        stream.eat(|kind| matches!(kind, TokenKind::Identifier(word) if word == EXPECTING_WORD))
    else {
        return Err(ParseError::new(
            artifact_span,
            format!(
                "recipe `{recipe}`'s step `{step}` fires `{artifact}` and names no known answer: \
                 write `{EXPECTING_WORD} \"<bytes>\"`. A fire with nothing to compare against \
                 proves the artifact ran, never that it said what it must"
            ),
        ));
    };
    let (expecting, expecting_span) = expect_string(stream, "the known answer")?;
    end_clause_line(stream, span, recipe, FIRES_WORD)?;
    Ok(StepBody::FiresArtifact {
        span: join_span(span, expecting_span),
        artifact,
        artifact_span,
        expecting,
        expecting_span,
    })
}

/// `captures <name>: <Type> of artifact <name>`.
fn parse_captures(
    stream: &mut Stream,
    span: Span,
    recipe: &str,
    step: &str,
) -> Result<StepBody, ParseError> {
    let (capture, capture_span) = stream.expect_name("the capture's name")?;
    if stream
        .eat(|kind| matches!(kind, TokenKind::Colon))
        .is_none()
    {
        return Err(ParseError::new(
            capture_span,
            format!(
                "recipe `{recipe}`'s step `{step}` captures `{capture}` with no type: write \
                 `{CAPTURES_WORD} {capture}: <Type> of {ARTIFACT_WORD} <name>`. A capture into an \
                 untyped result is a value nothing downstream can check"
            ),
        ));
    }
    let ty = parse_type_ref(stream)?;
    let Some(_) = stream.eat(|kind| matches!(kind, TokenKind::Keyword(Keyword::Of))) else {
        return Err(ParseError::new(
            capture_span,
            format!(
                "recipe `{recipe}`'s step `{step}` captures `{capture}` from nothing: write `of \
                 {ARTIFACT_WORD} <name>`, naming an artifact the document declares"
            ),
        ));
    };
    let Some(_) =
        stream.eat(|kind| matches!(kind, TokenKind::Identifier(word) if word == ARTIFACT_WORD))
    else {
        return Err(ParseError::new(
            capture_span,
            format!(
                "recipe `{recipe}`'s step `{step}` captures from something that is not a declared \
                 artifact: write `of {ARTIFACT_WORD} <name>`"
            ),
        ));
    };
    let (artifact, artifact_span) = stream.expect_name("a declared artifact's name")?;
    end_clause_line(stream, span, recipe, CAPTURES_WORD)?;
    Ok(StepBody::CapturesArtifact {
        span: join_span(span, artifact_span),
        capture,
        capture_span,
        ty,
        artifact,
        artifact_span,
    })
}

/// The declaration's header line: `<word> <name>`, with the opener consumed.
fn header(
    stream: &mut Stream,
    word: &str,
    what: &str,
) -> Result<(Span, String, Span, Option<crate::ast::Comment>), ParseError> {
    let Some(keyword) = stream.next() else {
        return Err(ParseError::new(
            stream.peek_span(),
            format!("expected a `{word}` declaration, found end of input"),
        ));
    };
    let (name, name_span) = stream.expect_name(what)?;
    let span = join_span(keyword.span, name_span);
    let trailing = stream.end_line()?;
    Ok((span, name, name_span, trailing))
}

/// Run `clause` over each line of an indented block, if one opens.
///
/// Every body of this surface is a flat list of clause lines, so the loop
/// ends exactly where the block does: there is no clause that closes a body
/// early, and inventing a way to would give a body two endings.
fn read_block(
    stream: &mut Stream,
    mut clause: impl FnMut(&mut Stream) -> Result<(), ParseError>,
) -> Result<(), ParseError> {
    let outer_leads = stream.take_leads()?;
    if !stream.open_block() {
        stream.push_back_leads(outer_leads);
        return Ok(());
    }
    stream.push_back_leads(outer_leads);
    loop {
        let clause_lead = stream.take_leads()?;
        if stream.at_item_block_end() {
            stream.push_back_leads(clause_lead);
            break;
        }
        let _ = stream.take_docs();
        clause(stream)?;
    }
    let stray = stream.take_leads()?;
    stream.consume_block_dedent();
    stream.push_back_leads(stray);
    Ok(())
}

/// The word opening a clause line, read WITHOUT consuming it, whether the
/// lexer made it a keyword or an identifier.
fn clause_word(
    stream: &mut Stream,
    construct: &str,
    name: &str,
) -> Result<(String, Span, bool), ParseError> {
    let Some(token) = stream.peek() else {
        return Err(ParseError::new(
            stream.peek_span(),
            format!("expected a clause inside {construct} `{name}`, found end of input"),
        ));
    };
    let span = token.span;
    match &token.kind {
        TokenKind::Identifier(word) => Ok((word.clone(), span, false)),
        TokenKind::Keyword(keyword) => Ok((keyword.as_word().to_owned(), span, true)),
        other => Err(refusal_to_parse_error(&CommandRefusal::refuse(
            CommandCause::AssemblyClauseUnknown,
            span,
            describe(other),
            format!(
                "expected a clause inside {construct} `{name}`, found {}",
                describe(other)
            ),
        ))),
    }
}

/// Read a clause's single string literal and close the line.
fn literal(
    stream: &mut Stream,
    what: &str,
    span: Span,
    owner: &str,
    clause: &str,
) -> Result<ParamLiteral, ParseError> {
    let (text, literal_span) = expect_string(stream, what)?;
    end_clause_line(stream, span, owner, clause)?;
    Ok(ParamLiteral {
        text,
        span: literal_span,
    })
}

fn recipe_clause_unknown(word: &str, span: Span, construct: &str, name: &str) -> ParseError {
    refusal_to_parse_error(&CommandRefusal::refuse(
        CommandCause::AssemblyClauseUnknown,
        span,
        word,
        format!(
            "expected one of {} inside {construct} `{name}`, found `{word}`",
            admitted(&RECIPE_CLAUSE_WORDS, &[Keyword::Step.as_word()])
        ),
    ))
}

fn step_clause_unknown(word: &str, span: Span, recipe: &str, step: &str) -> ParseError {
    refusal_to_parse_error(&CommandRefusal::refuse(
        CommandCause::AssemblyClauseUnknown,
        span,
        word,
        format!(
            "expected one of {} inside recipe `{recipe}`'s step `{step}`, found `{word}`",
            admitted(&STEP_CLAUSE_WORDS, &[Keyword::After.as_word()])
        ),
    ))
}

fn wall_clause_unknown(word: &str, span: Span, name: &str) -> ParseError {
    refusal_to_parse_error(&CommandRefusal::refuse(
        CommandCause::AssemblyClauseUnknown,
        span,
        word,
        format!(
            "expected one of {} inside wall `{name}`, found `{word}`",
            admitted(&WALL_CLAUSE_WORDS, &[])
        ),
    ))
}

fn artifact_clause_unknown(word: &str, span: Span, name: &str) -> ParseError {
    refusal_to_parse_error(&CommandRefusal::refuse(
        CommandCause::AssemblyClauseUnknown,
        span,
        word,
        format!(
            "expected one of {} inside artifact `{name}`, found `{word}`",
            admitted(&ARTIFACT_CLAUSE_WORDS, &[])
        ),
    ))
}

fn nested_command(span: Span, recipe: &str) -> ParseError {
    refusal_to_parse_error(&CommandRefusal::refuse(
        CommandCause::CommandDeclarationNested,
        span,
        COMMAND_WORD,
        format!(
            "a `{COMMAND_WORD}` declaration cannot stand inside recipe `{recipe}`. Declare the \
             command at the TOP LEVEL and have a step name it — `{RUNS_WORD} {COMMAND_WORD} \
             <name>`. This is amendment 1's other side: a gate leg's review property is that a \
             reader sees the exact command that ran, and a command hidden inside an assembly is a \
             command no reader of the top level meets"
        ),
    ))
}

fn body_empty(construct: &str, name: &str, span: Span) -> ParseError {
    refusal_to_parse_error(&CommandRefusal::refuse(
        CommandCause::RecipeBodyEmpty,
        span,
        name,
        format!(
            "{construct} `{name}` states nothing; a declaration that says nothing declares nothing"
        ),
    ))
}

/// Close a clause line, refusing a trailing comment the printer could not
/// keep.
fn end_clause_line(
    stream: &mut Stream,
    span: Span,
    owner: &str,
    clause: &str,
) -> Result<(), ParseError> {
    if stream.end_line()?.is_some() {
        return Err(ParseError::new(
            span,
            format!(
                "`{owner}`'s `{clause}` clause carries a trailing comment; put the comment on its \
                 own line above the clause so the canonical printer can keep it"
            ),
        ));
    }
    Ok(())
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

/// Read a bare word — identifier or keyword — or say what stood there.
fn expect_word(stream: &mut Stream, what: &str) -> Result<(String, Span), ParseError> {
    let Some(token) = stream.next() else {
        return Err(ParseError::new(
            stream.peek_span(),
            format!("expected {what}, found end of input"),
        ));
    };
    match token.kind {
        TokenKind::Identifier(word) => Ok((word, token.span)),
        TokenKind::Keyword(keyword) => Ok((keyword.as_word().to_owned(), token.span)),
        other => Err(ParseError::new(
            token.span,
            format!("expected {what}, found {}", describe(&other)),
        )),
    }
}
