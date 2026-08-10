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
fn one_word_prints_its_sentence_examples_and_citation() {
    let entry = aion_awl::guide::for_word("collect");
    assert!(entry.is_some(), "`collect` is a word of the language");
    let rendered = entry.map(render_one).unwrap_or_default();
    assert!(rendered.starts_with("collect  (reserved word)\n"));
    assert!(rendered.contains("Closes the nearest region"));
    assert!(
        rendered.contains("AWL reference §8 — Parallel and sequential work"),
        "{rendered}"
    );
    assert!(
        !rendered.contains("AWL-REFERENCE.md"),
        "the citation must not name a repository path an installed binary cannot resolve: \
         {rendered}"
    );
    assert!(
        rendered.contains("\n      collect health -> results\n"),
        "the example document must be printed, indented as a block: {rendered}"
    );
    assert!(
        rendered.contains("\n    workflow check_services\n"),
        "the example must be a complete document, not a fragment: {rendered}"
    );
    assert!(
        rendered.contains("  Example 1:\n") && rendered.contains("  Example 2:\n"),
        "a word with several forms must number its examples: {rendered}"
    );
    assert!(
        rendered.contains("collect report? -> reports"),
        "the tolerant form must be among the examples: {rendered}"
    );
    assert!(
        rendered.contains("Full reference: aion awl guide --reference"),
        "the entry must say how to open the full reference from this binary: {rendered}"
    );
}

/// A word with exactly one honest form prints its single example without a
/// number — a lone "Example 1:" would promise a second that never comes.
#[test]
fn a_single_form_word_prints_one_unnumbered_example() {
    let entry = aion_awl::guide::for_word("spawn");
    assert!(entry.is_some(), "`spawn` is a word of the language");
    let rendered = entry.map(render_one).unwrap_or_default();
    assert!(
        !rendered.contains("Example 1:"),
        "a single example must not be numbered: {rendered}"
    );
    assert!(
        rendered.contains("\n      spawn notify_archived(report_id: report_id)\n"),
        "the single example must still be printed: {rendered}"
    );
}

/// The `--reference` output is the embedded reference itself — the same file
/// the repository's `guide_reference_gate` reads, so an installed binary
/// prints exactly what the gate verified.
#[test]
fn the_embedded_reference_is_the_whole_document() {
    let text = aion_awl::guide::reference_text();
    assert!(
        text.contains("## 1. Document structure"),
        "the embedded reference must open with its first numbered section"
    );
    assert!(
        text.contains("## 16. Worker documents"),
        "the embedded reference must reach its final numbered section"
    );
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
    for entry in parsed["entries"].as_array().cloned().unwrap_or_default() {
        let examples = entry["examples"].as_array().cloned().unwrap_or_default();
        assert!(
            !examples.is_empty(),
            "every entry must carry at least one example document: {entry}"
        );
        for example in &examples {
            assert!(
                example
                    .as_str()
                    .is_some_and(|example| example.starts_with("//!")),
                "every example must be a complete narrated document: {entry}"
            );
        }
    }
}

/// The grammar axis on the wire: the form taxonomy and position inventory
/// ride the document, every entry names a form the taxonomy carries and
/// positions the inventory carries, and the full reference text ships so the
/// console can render the sections the citations point into.
#[test]
fn the_json_form_carries_the_grammar_axis_and_the_reference() {
    let text = json(None).unwrap_or_default();
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);

    let forms = parsed["forms"].as_array().cloned().unwrap_or_default();
    let form_ids: Vec<&str> = forms
        .iter()
        .filter_map(|form| form["id"].as_str())
        .collect();
    assert_eq!(
        form_ids.len(),
        aion_awl::guide::GrammarForm::ALL.len(),
        "every form of the taxonomy must ride the wire: {forms:?}"
    );
    for form in &forms {
        for field in ["id", "label", "description"] {
            assert!(
                form[field].as_str().is_some_and(|value| !value.is_empty()),
                "a form is missing `{field}`: {form}"
            );
        }
    }

    let positions = parsed["positions"].as_array().cloned().unwrap_or_default();
    let position_ids: Vec<&str> = positions
        .iter()
        .filter_map(|position| position["id"].as_str())
        .collect();
    assert_eq!(
        position_ids.len(),
        aion_awl::guide::GrammarPosition::ALL.len(),
        "every grammar position must ride the wire: {positions:?}"
    );

    for entry in parsed["entries"].as_array().cloned().unwrap_or_default() {
        let form = entry["form"].as_str().unwrap_or_default();
        assert!(
            form_ids.contains(&form),
            "an entry names a form the taxonomy does not carry: {entry}"
        );
        let stands = entry["positions"].as_array().cloned().unwrap_or_default();
        assert!(
            !stands.is_empty(),
            "an entry stands in no position: {entry}"
        );
        for position in &stands {
            assert!(
                position
                    .as_str()
                    .is_some_and(|position| position_ids.contains(&position)),
                "an entry stands in a position the inventory does not carry: {entry}"
            );
        }
    }

    let reference = parsed["reference_text"].as_str().unwrap_or_default();
    assert!(
        reference.contains("## 1. Document structure")
            && reference.contains("## 16. Worker documents"),
        "the wire must carry the whole reference, first section to last"
    );
    assert_eq!(
        reference,
        aion_awl::guide::reference_text(),
        "the wire's reference must be the embedded reference, verbatim"
    );
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
    assert_eq!(parsed["entries"][0]["form"].as_str(), Some("statement"));
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
