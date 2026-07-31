//! What the declaration under the cursor binds: its references, the occurrences
//! an editor should highlight, and its rename — or the refusal of one.
//!
//! Four features live together because all four are the same question first:
//! which spans belong to the declaration under the cursor? `target_at` answers
//! it once, and everything here is a different presentation of that answer.
//!
//! The checker already records, for every span it resolved, WHICH declaration
//! that span refers to. So the edit set is not guessed by matching text — a
//! text match would rename a string literal, a comment, and an unrelated
//! binding that happens to share a name. It is exactly the spans the checker
//! bound to this declaration, plus the declaration's own.
//!
//! REFUSAL IS PART OF THE FEATURE. A rename that produces a document which no
//! longer means what the author intended is worse than no rename at all,
//! because the author sees a successful edit and moves on. Three refusals are
//! enforced here, and each is checked against the language rather than against
//! a list written down beside it.

use std::collections::BTreeSet;
use std::path::Path;

use aion_awl::semantic::{DeclarationKind, SemanticAnalysis};
use aion_awl::{Document, Span, TokenKind};
use lsp_types::{DocumentHighlight, DocumentHighlightKind, Position, Range, TextEdit, Uri};

use crate::analysis::{byte_offset_at, range_for_span};

/// Why a rename was refused, in words an editor can show the author.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RenameRefusal {
    /// Nothing at the cursor resolves to a declaration.
    NotRenameable,
    /// The proposed name is not one token of the same lexical class.
    WrongShape {
        /// The name the author proposed.
        proposed: String,
        /// What the language expects there.
        expectation: String,
    },
    /// Another declaration of the same category already has the name.
    Collides {
        /// The proposed name.
        proposed: String,
        /// What that name already refers to.
        kind: &'static str,
    },
}

impl std::fmt::Display for RenameRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotRenameable => write!(
                formatter,
                "there is no declaration here to rename — put the cursor on a name"
            ),
            Self::WrongShape {
                proposed,
                expectation,
            } => write!(formatter, "`{proposed}` cannot stand here: {expectation}"),
            Self::Collides { proposed, kind } => write!(
                formatter,
                "`{proposed}` is already the name of another {kind} in this document, so the \
                 rename would merge two different things into one"
            ),
        }
    }
}

/// The declaration the cursor is on, together with every span bound to it.
struct Target {
    name: String,
    kind: DeclarationKind,
    declaration: Span,
    spans: BTreeSet<Span>,
}

/// The range an editor should pre-fill in its rename prompt, or `None` when
/// nothing at `position` can be renamed.
pub(crate) fn prepare(source: &str, position: Position, root: Option<&Path>) -> Option<Range> {
    let document = aion_awl::parse(source).ok()?;
    let analysis = analyze(&document, root);
    let target = target_at(source, &analysis, position)?;
    Some(range_for_span(source, target.declaration))
}

/// Every span that refers to the declaration at `position`, as locations.
///
/// `include_declaration` follows the client's `context.includeDeclaration`.
pub(crate) fn references(
    source: &str,
    uri: &Uri,
    position: Position,
    root: Option<&Path>,
    include_declaration: bool,
) -> Option<Vec<lsp_types::Location>> {
    let document = aion_awl::parse(source).ok()?;
    let analysis = analyze(&document, root);
    let target = target_at(source, &analysis, position)?;
    Some(
        target
            .spans
            .iter()
            .filter(|span| include_declaration || **span != target.declaration)
            .map(|span| lsp_types::Location::new(uri.clone(), range_for_span(source, *span)))
            .collect(),
    )
}

/// Every occurrence of the declaration at `position`, for the editor to tint.
///
/// The declaration itself is reported as `Write` and every use as `Read`, so
/// an author scanning a document sees where a thing is DEFINED distinctly from
/// where it is used, rather than one undifferentiated wash of colour.
///
/// This is the same resolved span set the rename edits, deliberately: if the
/// highlight and the rename could disagree, the highlight would be a promise
/// the rename does not keep.
pub(crate) fn highlights(
    source: &str,
    position: Position,
    root: Option<&Path>,
) -> Option<Vec<DocumentHighlight>> {
    let document = aion_awl::parse(source).ok()?;
    let analysis = analyze(&document, root);
    let target = target_at(source, &analysis, position)?;
    Some(
        target
            .spans
            .iter()
            .map(|span| DocumentHighlight {
                range: range_for_span(source, *span),
                kind: Some(if *span == target.declaration {
                    DocumentHighlightKind::WRITE
                } else {
                    DocumentHighlightKind::READ
                }),
            })
            .collect(),
    )
}

/// The edits that rename the declaration at `position` to `proposed`.
///
/// # Errors
///
/// Refuses when the cursor is not on a declaration, when `proposed` would not
/// lex as the same kind of name, or when it already names another declaration
/// of the same category.
pub(crate) fn rename(
    source: &str,
    position: Position,
    proposed: &str,
    root: Option<&Path>,
) -> Result<Vec<TextEdit>, RenameRefusal> {
    let document = aion_awl::parse(source).map_err(|_| RenameRefusal::NotRenameable)?;
    let analysis = analyze(&document, root);
    let target = target_at(source, &analysis, position).ok_or(RenameRefusal::NotRenameable)?;

    same_lexical_class(&target.name, proposed)?;
    no_collision(&analysis, &target, proposed)?;

    Ok(target
        .spans
        .iter()
        .map(|span| TextEdit::new(range_for_span(source, *span), proposed.to_owned()))
        .collect())
}

/// The proposed name must lex as ONE token of the same kind as the old name.
///
/// This is checked by running the real lexer rather than against a table of
/// which declaration categories are `TitleCase`. AWL's case classes are a
/// lexical fact — `PageAck` is a `TypeIdentifier` and `page_ack` is an
/// `Identifier` — so asking the lexer is both the accurate rule and one that
/// cannot drift from it. It rejects, in one check and for the right reason: a
/// reserved keyword (which lexes as `Keyword` and would change what the
/// document means), the wrong case class, punctuation, whitespace, an empty
/// name, and anything that lexes to more than one token.
fn same_lexical_class(current: &str, proposed: &str) -> Result<(), RenameRefusal> {
    let expectation = |what: &str| RenameRefusal::WrongShape {
        proposed: proposed.to_owned(),
        expectation: what.to_owned(),
    };
    let Some(wanted) = sole_token(current) else {
        // The name in the document does not lex on its own. Nothing can be
        // proven about a replacement, so nothing is changed.
        return Err(expectation(
            "the existing name does not lex as a single name, so no replacement can be checked",
        ));
    };
    let Some(actual) = sole_token(proposed) else {
        return Err(expectation(
            "a name must be exactly one identifier — not empty, not several words, and not a \
             reserved keyword",
        ));
    };
    match (&wanted, &actual) {
        (TokenKind::Identifier(_), TokenKind::Identifier(_))
        | (TokenKind::TypeIdentifier(_), TokenKind::TypeIdentifier(_)) => Ok(()),
        (TokenKind::TypeIdentifier(_), _) => Err(expectation(
            "this position holds a type name, which AWL writes in `TitleCase`",
        )),
        _ => Err(expectation(
            "this position holds an ordinary name, which AWL writes in `snake_case`",
        )),
    }
}

/// The single significant token `text` lexes to, if it is exactly one name.
fn sole_token(text: &str) -> Option<TokenKind> {
    let tokens = aion_awl::lex(text).ok()?;
    let mut significant = tokens.into_iter().map(|token| token.kind).filter(|kind| {
        !matches!(
            kind,
            TokenKind::Indent | TokenKind::Dedent | TokenKind::Newline
        )
    });
    let first = significant.next()?;
    significant.next().is_none().then_some(first)
}

/// No other declaration of the same category may already hold the name.
///
/// Deliberately scope-blind, and deliberately erring towards refusing. AWL's
/// scoping is real — two steps can each bind `result` harmlessly — so this
/// will occasionally refuse a rename that would have been safe. That is the
/// correct direction to be wrong in: a refusal costs the author one rejected
/// prompt, while a wrong accept silently merges two different things and the
/// author is told it worked.
fn no_collision(
    analysis: &SemanticAnalysis,
    target: &Target,
    proposed: &str,
) -> Result<(), RenameRefusal> {
    if proposed == target.name {
        return Ok(());
    }
    let clash = analysis
        .iter()
        .filter_map(|info| info.declaration.as_ref())
        .any(|declaration| {
            declaration.kind == target.kind
                && declaration.name == proposed
                && declaration.span != target.declaration
        });
    if clash {
        return Err(RenameRefusal::Collides {
            proposed: proposed.to_owned(),
            kind: target.kind.as_str(),
        });
    }
    Ok(())
}

/// The declaration under the cursor and every span the checker bound to it.
fn target_at(source: &str, analysis: &SemanticAnalysis, position: Position) -> Option<Target> {
    let byte = byte_offset_at(source, position);
    let declaration = analysis.info_at(byte)?.declaration.as_ref()?.clone();
    let mut spans: BTreeSet<Span> = analysis
        .iter()
        .filter(|info| {
            info.declaration
                .as_ref()
                .is_some_and(|found| found.span == declaration.span)
        })
        .map(|info| info.span)
        .collect();
    // The declaration's own name is always part of the set: a reference-only
    // rename would leave the document defining one name and using another.
    spans.insert(declaration.span);
    Some(Target {
        name: declaration.name,
        kind: declaration.kind,
        declaration: declaration.span,
        spans,
    })
}

fn analyze(document: &Document, root: Option<&Path>) -> SemanticAnalysis {
    root.map_or_else(
        || aion_awl::semantic::analyze(document),
        |root| aion_awl::semantic::analyze_in(document, root),
    )
}

#[cfg(test)]
#[path = "bindings_tests.rs"]
mod bindings_tests;
