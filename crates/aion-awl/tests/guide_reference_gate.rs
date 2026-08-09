//! The gate that makes the glossary's citations true.
//!
//! Every entry in `aion_awl::guide` ends with a pointer: "read §N — Title of
//! the reference". A pointer is a claim about a file this crate does not
//! contain, so nothing in the crate can keep it honest. This test reads the
//! reference itself.
//!
//! What it establishes, precisely: that for every section an entry cites, the
//! reference carries a heading with THAT number and THAT exact title, and
//! that the entry's word is written somewhere inside that section's text. The
//! second half is what makes the citation load-bearing rather than
//! decorative — a word could otherwise be filed under a section that never
//! mentions it, and the reader following the pointer would find nothing.
//!
//! What it does NOT establish: that the one-sentence summary is a correct
//! account of the construct. No mechanical check can read that. It is
//! reviewed as prose, in one place, which is the whole reason the glossary
//! exists as a single source instead of three copies.
//!
//! The section inventory is derived from the reference on every run rather
//! than transcribed here. A baseline transcribed into the test would agree
//! with itself after the document was re-organised, and report green over a
//! reader who can no longer find §8.

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use aion_awl::guide::{self, GuideEntry};

type TestResult = Result<(), Box<dyn Error>>;

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

/// The reference's numbered sections, in document order, as
/// `number -> (title, body)`.
fn sections(source: &str) -> BTreeMap<u8, (String, String)> {
    let mut sections = BTreeMap::new();
    let mut current: Option<(u8, String, String)> = None;
    for line in source.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            if let Some((number, title, body)) = current.take() {
                sections.insert(number, (title, body));
            }
            if let Some((number, title)) = heading.split_once(". ") {
                if let Ok(number) = number.parse::<u8>() {
                    current = Some((number, title.to_owned(), String::new()));
                }
            }
        } else if let Some((_, _, body)) = current.as_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    if let Some((number, title, body)) = current {
        sections.insert(number, (title, body));
    }
    sections
}

/// Every path the glossary hands an author must be a file that exists.
///
/// These are the pointers the CLI prints and the console renders. A dead one
/// is the exact defect this lane was opened to close, so it is gated rather
/// than proof-read.
#[test]
fn every_documented_path_the_glossary_names_exists() {
    let named = [
        guide::REFERENCE_PATH,
        guide::GUIDE_PATH,
        guide::AUTHORING_PATH,
        guide::WORKERS_PATH,
        guide::COMMANDS_PATH,
        guide::DECLARED_COMMANDS_PATH,
    ];
    let mut missing = Vec::new();
    for relative in named {
        if !repo_path(relative).is_file() {
            missing.push(relative);
        }
    }
    assert!(
        missing.is_empty(),
        "the glossary names paths that do not exist: {missing:?}"
    );
}

#[test]
fn every_cited_section_exists_with_the_title_the_glossary_writes() -> TestResult {
    let source = fs::read_to_string(repo_path(guide::REFERENCE_PATH))?;
    let sections = sections(&source);
    assert!(
        !sections.is_empty(),
        "no numbered sections were read from {}; the gate would pass vacuously",
        guide::REFERENCE_PATH
    );

    let mut wrong = Vec::new();
    for entry in guide::all() {
        let cited = entry.section();
        match sections.get(&cited.number()) {
            None => wrong.push(format!(
                "`{}` cites §{}, which the reference does not carry",
                entry.word(),
                cited.number()
            )),
            Some((title, _)) if title != cited.title() => wrong.push(format!(
                "`{}` cites §{} as \"{}\"; the reference titles it \"{title}\"",
                entry.word(),
                cited.number(),
                cited.title()
            )),
            Some(_) => {}
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    Ok(())
}

#[test]
fn every_cited_section_actually_discusses_the_word() -> TestResult {
    let source = fs::read_to_string(repo_path(guide::REFERENCE_PATH))?;
    let sections = sections(&source);
    let mut silent = Vec::new();
    for entry in guide::all() {
        let Some((_, body)) = sections.get(&entry.section().number()) else {
            continue;
        };
        if !mentions(body, entry.word()) {
            silent.push(format!(
                "`{}` cites §{}, whose text never uses the word",
                entry.word(),
                entry.section().number()
            ));
        }
    }
    assert!(silent.is_empty(), "{}", silent.join("\n"));
    Ok(())
}

/// Whether `body` uses `word` as a whole word.
///
/// Substring matching would report `in` present in "input" and `of` present
/// in "before", which would make the check above unable to fail for the
/// shortest words — exactly the ones most likely to be mis-filed.
fn mentions(body: &str, word: &str) -> bool {
    body.match_indices(word).any(|(index, _)| {
        let before = body[..index].chars().next_back();
        let after = body[index + word.len()..].chars().next();
        let boundary = |character: Option<char>| {
            character.is_none_or(|character| !character.is_alphanumeric() && character != '_')
        };
        boundary(before) && boundary(after)
    })
}

/// The gate must be able to fail. A citation to a section that cannot exist
/// has to be rejected by the same code path the real entries run through —
/// otherwise a green run says only that the check ran, not that it read
/// anything.
#[test]
fn the_gate_rejects_a_citation_the_reference_cannot_answer() -> TestResult {
    let source = fs::read_to_string(repo_path(guide::REFERENCE_PATH))?;
    let sections = sections(&source);
    assert!(
        !sections.contains_key(&u8::MAX),
        "the reference must not carry a §255 for this control to mean anything"
    );
    assert!(
        !mentions(
            sections
                .get(&1)
                .map(|(_, body)| body.as_str())
                .unwrap_or_default(),
            "definitely_not_in_the_reference"
        ),
        "the whole-word matcher must not find a word that is not there"
    );
    Ok(())
}

/// `mentions` is the load-bearing half of the citation check, so it is tested
/// against known answers rather than trusted. A matcher that quietly said
/// "yes" to everything would make the section check unable to fail.
#[test]
fn the_whole_word_matcher_reads_boundaries() {
    assert!(mentions("an `in` binding", "in"));
    assert!(mentions("`fork item in items`", "in"));
    assert!(!mentions("input and inference", "in"));
    assert!(!mentions("before the route", "of"));
    assert!(mentions("the shape of a step", "of"));
}

/// The console renders the same entries the CLI prints, so the count is a
/// fact both surfaces depend on. Deriving it from the language's own
/// inventories (rather than writing 63 here) is what makes a new word appear
/// everywhere without a second edit.
#[test]
fn the_inventory_covers_the_whole_vocabulary() {
    let entries: Vec<GuideEntry> = guide::all();
    assert_eq!(
        entries.len(),
        aion_awl::Keyword::ALL.len() + aion_awl::POSITIONAL_WORDS.len()
    );
}
