//! The top-level declaration dispatcher.
//!
//! Its own module rather than another eighty lines of `document.rs`: this is
//! the one place the language says which words may open a top-level line,
//! and it grows every time a declaration family lands. A dispatcher folded
//! into the file that also parses a workflow header, a const, a type and a
//! worker is a file nobody reads whole — which is how a refusal diagnostic
//! comes to name a different set than the dispatch it describes.
//!
//! The refusal here names EXACTLY what the dispatcher admits, and
//! `tests/grammar_parser_gate.rs` holds the two together: a word added to
//! one and not the other fails that gate rather than a review.

use crate::ast::Document;
use crate::parser::{ParseError, Stream, describe};
use crate::{Keyword, TokenKind};

use super::document::parse_const_decl;
use super::hints::gone_keyword_hint;
use super::steps::parse_step;
use super::types::parse_type_decl;
use super::workers::{parse_child, parse_worker};

/// Parse the top-level declarations after the header: consts, types,
/// workers, children, and steps, in any interleaving (the printer
/// canonicalizes the grammar's order).
pub(super) fn parse_declarations(
    stream: &mut Stream,
    document: &mut Document,
) -> Result<(), ParseError> {
    loop {
        let lead = stream.take_leads()?;
        let docs = stream.take_docs();
        let Some(token) = stream.peek() else {
            if let Some(doc) = docs.first() {
                return Err(ParseError::new(
                    doc.span,
                    "`///` doc lines attach to the declaration that follows; nothing follows",
                ));
            }
            stream.push_back_leads(lead);
            return Ok(());
        };
        let span = token.span;
        match &token.kind {
            TokenKind::Keyword(Keyword::Const) => {
                stream.next();
                document
                    .consts
                    .push(parse_const_decl(stream, lead, docs, span)?);
            }
            TokenKind::Keyword(Keyword::Type) => {
                stream.next();
                document
                    .types
                    .push(parse_type_decl(stream, lead, docs, span)?);
            }
            TokenKind::Keyword(Keyword::Worker) => {
                stream.next();
                document
                    .workers
                    .push(parse_worker(stream, lead, docs, span)?);
            }
            TokenKind::Keyword(Keyword::Child) => {
                stream.next();
                document
                    .children
                    .push(parse_child(stream, lead, docs, span)?);
            }
            TokenKind::Keyword(Keyword::Subflow) => {
                stream.next();
                document
                    .subflows
                    .push(super::flow::parse_subflow(stream, lead, docs, span)?);
            }
            TokenKind::Keyword(Keyword::Step) => {
                stream.next();
                document.steps.push(parse_step(stream, lead, docs, span)?);
            }
            // `command` opens a top-level declaration POSITIONALLY, not as a
            // reserved word: `command` stands in this tree's own corpus today
            // as a harness-section field name, so reserving it would break
            // documents that exist. Recognised here, where nothing else may
            // stand, exactly as `run`, `send` and `query` are read elsewhere.
            TokenKind::Identifier(word) if word == crate::COMMAND_WORD => {
                document
                    .commands
                    .push(crate::command::parse_command_decl(stream, lead, docs)?);
            }
            // The assembly openers, positional for the same measured reason
            // `command` is: `runs`, `parameter`, `artifact` and their family
            // stand in this tree's own `.awl` corpus today, so reserving any
            // of them would break documents that exist.
            TokenKind::Identifier(word) if word == crate::RECIPE_WORD => {
                document
                    .recipes
                    .push(crate::command::parse_recipe_decl(stream, lead, docs)?);
            }
            TokenKind::Identifier(word) if word == crate::WALL_WORD => {
                document
                    .walls
                    .push(crate::command::parse_wall_decl(stream, lead, docs)?);
            }
            TokenKind::Identifier(word) if word == crate::ARTIFACT_WORD => {
                document
                    .artifacts
                    .push(crate::command::parse_artifact_decl(stream, lead, docs)?);
            }
            TokenKind::Identifier(word) => {
                let message = gone_keyword_hint(word).unwrap_or_else(|| {
                    format!(
                        "expected a `const`, `type`, `worker`, `child`, `subflow`, `step`, \
                         `command`, `recipe`, `wall`, or `artifact` declaration, found `{word}`"
                    )
                });
                return Err(ParseError::new(span, message));
            }
            other => {
                return Err(ParseError::new(
                    span,
                    format!(
                        "expected a `const`, `type`, `worker`, `child`, `subflow`, `step`, \
                         `command`, `recipe`, `wall`, or `artifact` declaration, found {}",
                        describe(other)
                    ),
                ));
            }
        }
    }
}
