//! Hover and completion documentation for the words of the language.
//!
//! Every sentence rendered here comes from `aion_awl::guide`. This module
//! writes none of its own, and that is the point: an editor tooltip that
//! paraphrased the reference would be a second, unreviewed account of the
//! language, and it would go on reading plausibly long after the first one
//! changed.
//!
//! ## Why hover answers for reserved words only
//!
//! A reserved word can be nothing else — the lexer produces a keyword token
//! for it wherever it stands, so naming its meaning is always true. The six
//! POSITIONAL words are the opposite case: they lex as ordinary identifiers
//! and mean what the glossary says ONLY in the one grammar position where the
//! parser recognises them. `step send` names a step and `action send(to:
//! String, …)` names a parameter; a tooltip claiming either "sends a declared
//! signal to another running workflow" would be plainly false.
//!
//! Completion is the opposite situation and gets the opposite answer: it
//! offers a positional word precisely because the cursor is in the position
//! that gives the word its meaning, so the documentation attached there is
//! true by construction. That asymmetry is deliberate, not an omission.

use aion_awl::guide::{self, GuideEntry, WordKind};
use aion_awl::{TokenKind, lex};
use lsp_types::{Documentation, Hover, HoverContents, MarkupContent, MarkupKind, Position};

use crate::analysis::{byte_offset_at, range_for_span};

/// The glossary entry for the reserved word under `position`, if one is
/// there.
///
/// Reads the TOKEN stream rather than the parse tree, so an author can hover
/// a keyword in a document that is mid-edit and does not yet parse — which is
/// most of the time an author needs to ask what a word means.
pub(crate) fn hover_at(source: &str, position: Position) -> Option<Hover> {
    let byte = byte_offset_at(source, position);
    let tokens = lex(source).ok()?;
    let token = tokens
        .iter()
        .find(|token| token.span.start <= byte && byte < token.span.end)?;
    let TokenKind::Keyword(keyword) = token.kind else {
        return None;
    };
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: render_hover(guide::for_keyword(keyword)),
        }),
        range: Some(range_for_span(source, token.span)),
    })
}

/// The documentation a completion item carries for `word`.
///
/// `None` for anything the language does not name, which is how a declared
/// identifier that happens to spell a word of the language keeps its own
/// meaning. Completion shows the FIRST example only: a completion popup is a
/// glance while typing, and stacking every form of a word there would bury
/// the list it sits beside. Hover is where an author reads at length, so
/// hover carries them all.
pub(crate) fn documentation_for(word: &str) -> Option<Documentation> {
    guide::for_word(word).map(|entry| {
        let examples = entry.examples();
        Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: render(entry, &examples[..examples.len().min(1)]),
        })
    })
}

/// The full rendering for hover: every checked example the word has.
fn render_hover(entry: GuideEntry) -> String {
    render(entry, entry.examples())
}

/// One entry as markdown: the word as AWL, its meaning, the given checked
/// examples as fenced blocks, and its citation.
///
/// The citation names the reference by section rather than by repository
/// path — the author behind this hover may be on a crates.io install with no
/// checkout, and `aion awl guide --reference` prints the full text wherever
/// the binary is.
fn render(entry: GuideEntry, examples: &[&str]) -> String {
    let noun = match entry.kind() {
        WordKind::Reserved => "reserved word",
        WordKind::Positional => "positional word",
    };
    let blocks = examples
        .iter()
        .map(|example| format!("```awl\n{example}\n```"))
        .collect::<Vec<_>>()
        .join("\n\n");
    format!(
        "```awl\n{}\n```\n\n{} — {}\n\n{blocks}\n\n{} · `{}`",
        entry.word(),
        noun,
        entry.summary(),
        entry.citation(),
        guide::REFERENCE_COMMAND
    )
}

#[cfg(test)]
#[path = "glossary_tests.rs"]
mod glossary_tests;
