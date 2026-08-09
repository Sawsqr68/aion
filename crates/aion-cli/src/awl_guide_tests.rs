//! What `aion awl guide` must print, and what it must refuse.

use aion_awl::{Keyword, POSITIONAL_WORDS};

use super::{json, render_all, render_one};

#[test]
fn the_text_form_prints_every_word_of_the_language() {
    let rendered = render_all();
    for keyword in Keyword::ALL {
        assert!(
            rendered.contains(&format!("  {}  ", keyword.as_word()))
                || rendered.contains(&format!("  {} ", keyword.as_word())),
            "`{}` is a word of the language and must be listed",
            keyword.as_word()
        );
    }
    for word in POSITIONAL_WORDS {
        assert!(
            rendered.contains(word),
            "`{word}` is a word of the language and must be listed"
        );
    }
}

/// The command is the CLI's answer to "where do I read more". Every path it
/// names is gated for existence by `tests/doc_pointer_gate.rs`; this pins
/// that it names them at all.
#[test]
fn the_text_form_names_the_documents_an_author_needs() {
    let rendered = render_all();
    for path in [
        "docs/authoring/AWL.md",
        "examples/assistant/resources/AWL-REFERENCE.md",
        "examples/assistant/resources/AWL-AUTHORING.md",
        "examples/assistant/resources/WORKERS.md",
        "examples/assistant/resources/COMMANDS.md",
        "docs/workers/DECLARED-COMMANDS.md",
    ] {
        assert!(rendered.contains(path), "the guide must name {path}");
    }
}

/// Sections are printed once each, in reference order. A grouping that
/// emitted a heading every time the section changed would print §4 six times
/// over, and the reader would take each as a new section.
#[test]
fn each_reference_section_is_printed_once_in_order() {
    let rendered = render_all();
    let headings: Vec<u8> = rendered
        .lines()
        .filter_map(|line| line.strip_prefix('§'))
        .filter_map(|line| line.split_once(' ').map(|(number, _)| number.to_owned()))
        .filter_map(|number| number.parse::<u8>().ok())
        .collect();
    assert!(
        !headings.is_empty(),
        "the guide must print section headings"
    );
    let mut sorted = headings.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        headings, sorted,
        "sections must appear once each, in reference order"
    );
}

#[test]
fn one_word_prints_its_sentence_and_its_citation() {
    let entry = aion_awl::guide::for_word("collect");
    assert!(entry.is_some(), "`collect` is a word of the language");
    let rendered = entry.map(render_one).unwrap_or_default();
    assert!(rendered.starts_with("collect  (reserved word)\n"));
    assert!(rendered.contains("Closes the nearest region"));
    assert!(rendered.contains("AWL-REFERENCE.md §8 — Parallel and sequential work"));
}

/// A positional word is labelled as one. An author who reads "reserved word"
/// over `run` would conclude they cannot name a step `run`, which is false.
#[test]
fn a_positional_word_is_labelled_as_positional() {
    let entry = aion_awl::guide::for_word("run");
    assert!(entry.is_some(), "`run` is a word of the language");
    let rendered = entry.map(render_one).unwrap_or_default();
    assert!(
        rendered.starts_with("run  (positional word)\n"),
        "{rendered}"
    );
}

#[test]
fn the_json_form_carries_the_whole_inventory_and_the_paths() {
    let rendered = json(None);
    assert!(rendered.is_ok(), "the glossary must render as JSON");
    let text = rendered.unwrap_or_default();
    assert!(text.ends_with('\n'), "the asset must end in a newline");
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    assert_eq!(
        parsed["entries"].as_array().map(Vec::len),
        Some(Keyword::ALL.len() + POSITIONAL_WORDS.len())
    );
    assert_eq!(parsed["guide"].as_str(), Some("docs/authoring/AWL.md"));
    assert_eq!(
        parsed["reference"].as_str(),
        Some("examples/assistant/resources/AWL-REFERENCE.md")
    );
    assert_eq!(parsed["entries"][0]["word"].as_str(), Some("workflow"));
}

/// The console asset is gated by byte comparison, which is only a gate if the
/// rendering is deterministic.
#[test]
fn the_json_form_is_byte_stable() {
    assert_eq!(
        json(None).unwrap_or_default(),
        json(None).unwrap_or_default()
    );
}

#[test]
fn one_word_as_json_carries_only_that_word() {
    let text = json(Some("distribute")).unwrap_or_default();
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    assert_eq!(parsed["entries"].as_array().map(Vec::len), Some(1));
    assert_eq!(parsed["entries"][0]["word"].as_str(), Some("distribute"));
    assert_eq!(parsed["entries"][0]["kind"].as_str(), Some("keyword"));
    assert_eq!(parsed["entries"][0]["section"]["number"].as_u64(), Some(8));
}

/// A word the language does not have must produce no entry, not an empty
/// document that reads like an answer.
#[test]
fn a_word_the_language_does_not_have_resolves_to_nothing() {
    assert!(aion_awl::guide::for_word("select").is_none());
    let text = json(Some("select")).unwrap_or_default();
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    assert_eq!(parsed["entries"].as_array().map(Vec::len), Some(0));
}
