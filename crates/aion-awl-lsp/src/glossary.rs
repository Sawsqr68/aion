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
//!
//! ## Why the command-declaration words are the exception
//!
//! The asymmetry above is about not KNOWING the position. For the thirteen
//! words of a `command` declaration, hover CAN know it: each one is recognised
//! only as the first word of a line, and whether that line opens a top-level
//! declaration or stands inside a `command` block is readable from the text
//! before the cursor — the same reading completion already performs. So hover
//! answers for them under exactly that condition and stays silent otherwise,
//! which keeps the rule intact rather than carving an exception out of it:
//! `command` as a harness-section field name (which it is in 54 documents of
//! this tree) still hovers to nothing.

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
    let entry = match &token.kind {
        TokenKind::Keyword(keyword) => guide::for_keyword(*keyword),
        TokenKind::Identifier(word) => command_word_entry(source, word, token.span.start)?,
        _ => return None,
    };
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: render_hover(entry),
        }),
        range: Some(range_for_span(source, token.span)),
    })
}

/// The glossary entry for a declared-surface word standing where the parser
/// recognises it, or `None` when the identifier is just a name.
///
/// The two recognised positions, and nothing else: a DECLARATION OPENER as
/// the first word of a line at column zero, and a clause word as the first
/// word of an indented line whose enclosing declaration is one this cluster
/// opens. An author's own `cwd:` field inside a `harness` section fails both
/// and hovers to nothing, which is the whole point.
///
/// The clause word is additionally scoped to the SECTION its own opener
/// cites, so a `digest` typed inside a `command` body hovers to nothing and
/// a `program` typed inside an `artifact` body does too. The scoping is read
/// from the entries themselves rather than from a list here.
fn command_word_entry(source: &str, word: &str, start: usize) -> Option<GuideEntry> {
    let (line_start, _) = crate::completion::line_bounds(source, start);
    if source.get(line_start..start)?.trim_start() != "" {
        // Something else stands before the word on this line, so the word is
        // not opening anything.
        return None;
    }
    let indented = start > line_start;
    if let Some(opener) = DECLARED_SURFACE_OPENERS
        .into_iter()
        .find(|opener| *opener == word)
    {
        return (!indented)
            .then(|| guide::for_positional_word(opener))
            .flatten();
    }
    if !indented {
        return None;
    }
    let enclosing = crate::completion::enclosing_declaration(source, line_start)?;
    let section = section_of(enclosing)?;
    guide::for_positional_word(word).filter(|entry| entry.section().number() == section)
}

/// The top-level words this cluster opens a declaration with.
///
/// `command` is cut-1's; `recipe`, `wall` and `artifact` are the assembly
/// surface's. Each is a positional word, so each is recognised only at column
/// zero — which is exactly the condition above.
const DECLARED_SURFACE_OPENERS: [&str; 4] = [
    aion_awl::COMMAND_WORD,
    aion_awl::RECIPE_WORD,
    aion_awl::WALL_WORD,
    aion_awl::ARTIFACT_WORD,
];

/// The reference section a declaration opener's own cluster cites.
///
/// Read from the opener's glossary entry rather than written here, so a
/// cluster that moves section takes its clause scoping with it and no
/// constant falls behind.
fn section_of(opener: &str) -> Option<u8> {
    DECLARED_SURFACE_OPENERS
        .into_iter()
        .find(|candidate| *candidate == opener)
        .and_then(guide::for_positional_word)
        .map(|entry| entry.section().number())
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
