//! What the editor is allowed to claim about a word.

use lsp_types::{Documentation, Hover, HoverContents, MarkupContent, Position};

use crate::analysis::position_at;
use crate::completion::completions;
use crate::navigation;

use super::{documentation_for, hover_at};

/// A document that PARSES, for the cases where the point is precedence
/// rather than damage.
const DOCUMENT: &str = "\
//! Review a repository.
workflow review
  input repo: String
  outcome done: type Report, route success

type Report { summary: String }

worker inspectors
  action inspect(repo: String) -> Report

step look
  inspect(repo: repo) -> report
  report |> route done
";

/// The position of `needle` in `source`.
fn position_of(source: &str, needle: &str) -> Option<Position> {
    source.find(needle).map(|byte| position_at(source, byte))
}

fn hover_text(hover: Option<Hover>) -> String {
    match hover.map(|hover| hover.contents) {
        Some(HoverContents::Markup(MarkupContent { value, .. })) => value,
        Some(HoverContents::Scalar(_) | HoverContents::Array(_)) | None => String::new(),
    }
}

fn documentation_text(documentation: Option<Documentation>) -> String {
    match documentation {
        Some(
            Documentation::MarkupContent(MarkupContent { value, .. })
            | Documentation::String(value),
        ) => value,
        None => String::new(),
    }
}

/// THE CASE THE FEATURE EXISTS FOR. An author asks what a word means while
/// the line is half-written, so the document does not parse and the
/// declaration-based hover has nothing to say.
#[test]
fn a_keyword_answers_in_a_document_that_does_not_parse() {
    let source = format!("{DOCUMENT}\nstep escalate\n  distribute item in\n");
    assert!(
        aion_awl::parse(&source).is_err(),
        "the premise of this test is a mid-edit document; if it parses, the test proves nothing \
         about the case the fallback exists for"
    );
    let position = position_of(&source, "distribute");
    assert!(
        position.is_some(),
        "the fixture must carry the hovered word"
    );
    let text = hover_text(position.and_then(|position| hover_at(&source, position)));
    assert!(
        text.contains("one item track per element in parallel"),
        "the tooltip must carry the language's own sentence: {text}"
    );
    assert!(
        text.contains("AWL reference §8 — Parallel and sequential work"),
        "the tooltip must cite the reference section: {text}"
    );
    assert!(
        !text.contains("AWL-REFERENCE.md"),
        "the tooltip must not cite a repository path an installed binary cannot resolve: {text}"
    );
    assert!(
        text.contains("distribute host in hosts"),
        "the tooltip must carry the word's complete example document: {text}"
    );
    let fences = text.matches("```awl\n").count();
    assert!(
        fences >= 3,
        "a word with several forms must show one fenced block per example \
         (the word fence plus at least two examples): {text}"
    );
}

/// A positional word lexes as an identifier, so hover cannot know whether it
/// carries its language meaning or is simply a name. It must therefore claim
/// nothing — with a positive control beside it, so a test that passed because
/// hover was broken everywhere would fail instead.
#[test]
fn hover_makes_no_claim_about_a_word_that_might_be_a_name() {
    let source = "\
//! Relay a report onward.
workflow relay
  input send: String
  outcome done: type Report, route success

type Report { summary: String }

step forward
  route done(summary: send)
";
    let name = position_of(source, "send: String");
    assert!(name.is_some(), "the fixture must carry the identifier");
    assert!(
        name.and_then(|position| hover_at(source, position))
            .is_none(),
        "`send` here is a declared input name; hover must not claim it signals another workflow"
    );

    let control = position_of(source, "workflow");
    assert!(control.is_some(), "the fixture must carry the control word");
    assert!(
        control
            .and_then(|position| hover_at(source, position))
            .is_some(),
        "the control proves hover answers at all in this fixture"
    );
}

/// A declaration the author wrote wins over the language's glossary: hovering
/// your own `Report` must show `Report`.
#[test]
fn a_declared_name_still_wins_over_the_glossary() {
    let position = position_of(DOCUMENT, "Report { summary");
    assert!(position.is_some(), "the fixture must carry the declaration");
    let text =
        hover_text(position.and_then(|position| navigation::hover(DOCUMENT, position, None)));
    assert!(
        text.contains("Report"),
        "the declaration answer must be the one shown: {text}"
    );
    assert!(
        !text.contains("AWL reference §"),
        "a declared name is no word of the language and must carry no citation: {text}"
    );
}

/// The keyword hover and the completion documentation are the same claim in
/// two places — the same sentence, the same citation, rendered from one
/// entry. They differ in exactly one deliberate way: hover carries every
/// example, completion only the first, because a completion popup is a
/// glance while typing. This pins both the agreement and the difference.
#[test]
fn hover_and_completion_agree_and_completion_shows_one_example() {
    let position = position_of(DOCUMENT, "worker inspectors");
    assert!(position.is_some(), "the fixture must carry the word");
    let hovered = hover_text(position.and_then(|position| hover_at(DOCUMENT, position)));
    assert!(!hovered.is_empty(), "`worker` must hover");
    let completion = documentation_text(documentation_for("worker"));

    let sentence = "Declares a typed worker contract";
    assert!(hovered.contains(sentence), "{hovered}");
    assert!(completion.contains(sentence), "{completion}");
    assert!(hovered.contains("AWL reference §4"), "{hovered}");
    assert!(completion.contains("AWL reference §4"), "{completion}");

    // `worker` has a workflow-document example and a worker-document one.
    // Hover shows both; completion stops after the first.
    assert!(hovered.contains("A worker DOCUMENT"), "{hovered}");
    assert!(!completion.contains("A worker DOCUMENT"), "{completion}");
    let hover_fences = hovered.matches("```awl\n").count();
    let completion_fences = completion.matches("```awl\n").count();
    assert_eq!(
        completion_fences, 2,
        "completion carries the word fence and exactly one example: {completion}"
    );
    assert!(
        hover_fences > completion_fences,
        "hover must carry every example: {hovered}"
    );
}

/// Completion offers a positional word only where the parser recognises it,
/// so the documentation attached there IS true. The asymmetry with hover is
/// deliberate, and pinned here.
#[test]
fn an_offered_word_carries_the_language_s_sentence() {
    let source = format!("{DOCUMENT}\nworker shells\n  action greet(name: String) -> Out\n    r");
    let line = u32::try_from(source.lines().count().saturating_sub(1)).unwrap_or_default();
    let column = u32::try_from(
        source
            .lines()
            .next_back()
            .unwrap_or_default()
            .chars()
            .count(),
    )
    .unwrap_or_default();
    let items = completions(&source, Position::new(line, column));
    let run = items.iter().find(|item| item.label == "run");
    assert!(run.is_some(), "an action block must offer `run`");
    let text = documentation_text(run.and_then(|item| item.documentation.clone()));
    assert!(
        text.contains("no sandbox"),
        "the offered `run` must carry the language's own warning: {text}"
    );
    assert!(
        text.contains("AWL reference §4 — Workers, actions, and calls"),
        "the offered `run` must cite the reference: {text}"
    );
}

/// Every word the completion provider can offer has an entry. Without this, a
/// word added to the offered set would arrive silently undocumented in the
/// editor — the drift the single source exists to prevent.
#[test]
fn every_offered_word_of_the_language_is_documented() {
    for keyword in aion_awl::Keyword::ALL {
        assert!(
            documentation_for(keyword.as_word()).is_some(),
            "`{}` is offered by completion and carries no documentation",
            keyword.as_word()
        );
    }
    for word in aion_awl::POSITIONAL_WORDS {
        assert!(
            documentation_for(word).is_some(),
            "`{word}` is offered by completion and carries no documentation"
        );
    }
    assert!(
        documentation_for("inspect").is_none(),
        "a declared name is no word of the language and must carry no documentation"
    );
}
