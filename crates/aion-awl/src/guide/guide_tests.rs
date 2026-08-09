//! What the glossary must satisfy without reading any file.
//!
//! The claim these tests establish is narrow and worth stating plainly: that
//! every word of the language has exactly one entry, that the entry's word is
//! the word the lexer actually produces, and that no two entries claim the
//! same word. Whether a sentence is TRUE of the language is not something a
//! test can read; what it CAN read is whether the sentence points at a
//! section that exists, and `tests/guide_reference_gate.rs` does exactly that
//! against the reference document itself.

use std::collections::BTreeSet;

use crate::lexer::{Keyword, POSITIONAL_WORDS, TokenKind, lex};

use crate::guide::{GuideEntry, WordKind, all, for_keyword, for_positional_word, for_word};

#[test]
fn every_keyword_entry_names_the_word_the_lexer_produces() {
    for keyword in Keyword::ALL {
        let entry = for_keyword(keyword);
        assert_eq!(
            entry.word(),
            keyword.as_word(),
            "the entry for {keyword:?} names a different word than the lexer does"
        );
        assert_eq!(
            entry.kind(),
            WordKind::Reserved,
            "{} is a reserved word and must be recorded as one",
            keyword.as_word()
        );
    }
}

#[test]
fn every_positional_word_has_an_entry_recorded_as_positional() {
    for word in POSITIONAL_WORDS {
        let entry = for_positional_word(word);
        assert!(
            entry.is_some(),
            "the lexer names `{word}` a positional word and the glossary has no entry"
        );
        if let Some(entry) = entry {
            assert_eq!(entry.word(), word);
            assert_eq!(
                entry.kind(),
                WordKind::Positional,
                "`{word}` lexes as an identifier and must be recorded as positional"
            );
        }
    }
}

/// A positional word must NOT be reported as reserved, and a reserved word
/// must not be reachable through the positional arm. Without this the two
/// arms could silently overlap, and a surface would print "keyword" over a
/// word an author is still free to use as a name.
#[test]
fn the_two_inventories_do_not_overlap() {
    for word in POSITIONAL_WORDS {
        assert!(
            Keyword::from_word(word).is_none(),
            "`{word}` is listed positional but the lexer reserves it"
        );
        let lexed = lex(word);
        assert!(lexed.is_ok(), "a bare word must lex: `{word}`");
        let tokens = lexed.unwrap_or_default();
        assert!(
            tokens
                .iter()
                .any(|token| matches!(&token.kind, TokenKind::Identifier(text) if text == word)),
            "`{word}` must lex as an identifier"
        );
    }
    for keyword in Keyword::ALL {
        assert!(
            for_positional_word(keyword.as_word()).is_none(),
            "`{}` is reserved and must not answer through the positional arm",
            keyword.as_word()
        );
    }
}

#[test]
fn every_word_of_the_language_is_reachable_by_name() {
    for keyword in Keyword::ALL {
        assert_eq!(
            for_word(keyword.as_word()).map(GuideEntry::word),
            Some(keyword.as_word()),
            "a reserved word must resolve by name"
        );
    }
    for word in POSITIONAL_WORDS {
        assert_eq!(
            for_word(word).map(GuideEntry::word),
            Some(word),
            "a positional word must resolve by name"
        );
    }
    assert!(
        for_word("definitely_not_a_word_of_awl").is_none(),
        "an identifier that is not a word of the language must resolve to nothing"
    );
}

#[test]
fn the_inventory_is_complete_and_carries_no_duplicate() {
    let entries = all();
    assert_eq!(
        entries.len(),
        Keyword::ALL.len() + POSITIONAL_WORDS.len(),
        "the inventory must carry every reserved word and every positional word"
    );
    let mut seen = BTreeSet::new();
    for entry in &entries {
        assert!(
            seen.insert(entry.word()),
            "`{}` appears twice in the inventory",
            entry.word()
        );
    }
}

/// A surface renders the summary as one line of a tooltip or one line of
/// `--help`. An entry carrying a newline, surrounding whitespace, or no full
/// stop would render differently in each surface — which is the drift this
/// module exists to prevent, arriving through the back door.
#[test]
fn every_summary_is_one_rendered_line() {
    for entry in all() {
        let summary = entry.summary();
        assert!(
            !summary.is_empty(),
            "`{}` carries an empty summary",
            entry.word()
        );
        assert!(
            !summary.contains('\n'),
            "`{}` carries a multi-line summary",
            entry.word()
        );
        assert_eq!(
            summary.trim(),
            summary,
            "`{}` carries a summary with surrounding whitespace",
            entry.word()
        );
        assert!(
            summary.ends_with('.'),
            "`{}` carries a summary that is not a finished sentence",
            entry.word()
        );
    }
}

#[test]
fn a_citation_names_the_reference_and_the_section() {
    assert_eq!(
        for_word("worker").map(GuideEntry::citation).as_deref(),
        Some("examples/assistant/resources/AWL-REFERENCE.md §4 — Workers, actions, and calls")
    );
}
