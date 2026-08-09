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

/// The citation names the reference by SECTION, never by repository path. An
/// author who installed `aion` from crates.io has no repository to resolve a
/// path against; the full text is embedded and `aion awl guide --reference`
/// prints it, which is what [`crate::guide::REFERENCE_COMMAND`] says.
#[test]
fn a_citation_names_the_reference_and_the_section() {
    assert_eq!(
        for_word("worker").map(GuideEntry::citation).as_deref(),
        Some("AWL reference §4 — Workers, actions, and calls")
    );
    for entry in all() {
        assert!(
            !entry.citation().contains('/'),
            "`{}` cites a path; citations name sections, not files",
            entry.word()
        );
    }
}

/// The embedded reference is the document the citations point into. Empty or
/// heading-free text would make every citation a pointer to nowhere.
#[test]
fn the_embedded_reference_carries_the_cited_sections() {
    let text = crate::guide::reference_text();
    assert!(
        !text.trim().is_empty(),
        "the embedded reference must not be empty"
    );
    for entry in all() {
        let heading = format!(
            "## {}. {}",
            entry.section().number(),
            entry.section().title()
        );
        assert!(
            text.contains(&heading),
            "`{}` cites §{} — {}, and the embedded reference has no such heading",
            entry.word(),
            entry.section().number(),
            entry.section().title()
        );
    }
}

/// Whether `example` uses `word` as an actual TOKEN of the document.
///
/// Reads the lexer's own token stream rather than the raw text, so a word
/// that appears only inside a string literal or a comment cannot satisfy the
/// demonstration check — the lexer produces no token for either. A reserved
/// word arrives as its keyword token; a positional word lexes as an ordinary
/// identifier, which is exactly how the parser sees it too.
fn uses_word(example: &str, word: &str) -> bool {
    let Ok(tokens) = lex(example) else {
        return false;
    };
    tokens.iter().any(|token| match &token.kind {
        TokenKind::Keyword(keyword) => keyword.as_word() == word,
        TokenKind::Identifier(text) => text == word,
        _ => false,
    })
}

/// The token matcher is load-bearing for the example gate, so it is tested
/// against known answers rather than trusted: a word inside an identifier, a
/// string literal, or a comment must not count.
#[test]
fn the_example_word_matcher_reads_tokens_not_text() {
    assert!(uses_word("fork item in items", "in"));
    assert!(!uses_word("input name: String", "in"));
    assert!(uses_word("schema of Plan", "of"));
    assert!(!uses_word("workflow poll_job", "flow"));
    assert!(
        !uses_word("step go\n  route done(\"the fork road\")", "fork"),
        "a word inside a string literal is not a demonstration"
    );
    assert!(
        !uses_word("// a fork in a comment\nstep go", "fork"),
        "a word inside a comment is not a demonstration"
    );
}

/// Every entry teaches by example, and every example must actually
/// demonstrate ITS word: at least one complete document per entry, each
/// opening with the mandatory narration and carrying the word as a real
/// token. Two examples that are byte-identical would be padding, not
/// teaching, so duplicates are refused too.
#[test]
fn every_entry_example_is_a_complete_document_using_its_word() {
    for entry in all() {
        assert!(
            !entry.examples().is_empty(),
            "`{}` carries no examples",
            entry.word()
        );
        for (index, example) in entry.examples().iter().enumerate() {
            assert!(
                !example.trim().is_empty(),
                "`{}` example {} is empty",
                entry.word(),
                index + 1
            );
            assert!(
                example.starts_with("//!"),
                "`{}` example {} must open with the mandatory `//!` narration",
                entry.word(),
                index + 1
            );
            assert_eq!(
                example.trim_end(),
                &example[..],
                "`{}` example {} carries trailing whitespace",
                entry.word(),
                index + 1
            );
            for (line_number, line) in example.lines().enumerate() {
                assert_eq!(
                    line.trim_end(),
                    line,
                    "`{}` example {} line {} ends in whitespace",
                    entry.word(),
                    index + 1,
                    line_number + 1
                );
            }
            assert!(
                uses_word(example, entry.word()),
                "`{}` example {} never uses its own word as a token",
                entry.word(),
                index + 1
            );
        }
        for (index, example) in entry.examples().iter().enumerate() {
            assert!(
                !entry.examples()[..index].contains(example),
                "`{}` example {} repeats an earlier example verbatim",
                entry.word(),
                index + 1
            );
        }
    }
}

/// THE GATE. Every example runs through the crate's real pipeline the way
/// `aion awl check` runs it — `parse`, then `check_in` against the document's
/// root directory — and must come out with zero diagnostics. The root this
/// gate supplies carries the one schema file the corpus imports
/// (`schemas/existing.schema.json`), standing in for the directory beside a
/// real document. Every clean document is then pinned CANONICAL: `print` (the
/// one renderer `aion awl fmt` writes) must reproduce the example byte for
/// byte, so no example can teach a layout the formatter would immediately
/// rewrite. A failure names the word and the example's position.
#[test]
fn every_entry_example_passes_the_real_checker() -> Result<(), std::io::Error> {
    let root = std::env::temp_dir().join(format!("aion-awl-guide-examples-{}", std::process::id()));
    std::fs::create_dir_all(root.join("schemas"))?;
    std::fs::write(
        root.join("schemas/existing.schema.json"),
        br#"{ "type": "object", "required": ["value"], "properties": { "value": { "type": "string" } } }"#,
    )?;

    let mut failures = Vec::new();
    for entry in all() {
        for (index, example) in entry.examples().iter().enumerate() {
            match crate::parse(example) {
                Err(error) => failures.push(format!(
                    "`{}` example {} does not parse — {}:{}: {}",
                    entry.word(),
                    index + 1,
                    error.span.line,
                    error.span.column,
                    error.message
                )),
                Ok(document) => {
                    for error in crate::check_in(&document, &root) {
                        failures.push(format!(
                            "`{}` example {} fails the checker — {}:{}: {}",
                            entry.word(),
                            index + 1,
                            error.span.line,
                            error.span.column,
                            error.message
                        ));
                    }
                    let canonical = crate::print(&document);
                    if canonical.trim_end() != *example {
                        failures.push(format!(
                            "`{}` example {} is not canonical under `aion awl fmt`; replace it \
                             with the printer's own output:\n{canonical}",
                            entry.word(),
                            index + 1
                        ));
                    }
                }
            }
        }
    }

    std::fs::remove_dir_all(&root)?;
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    Ok(())
}
