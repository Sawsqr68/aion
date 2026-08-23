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

/// R7's LSP row, held against THE TABLE rather than against a copy.
///
/// Every word the command-declaration surface adds must hover to the guide's
/// own sentence and citation. The expected text is read from
/// `aion_awl::guide` on each run, so this cannot pass against a tooltip that
/// paraphrased the reference — the only way it goes green is if hover renders
/// what the one table says.
#[test]
fn every_command_word_hovers_to_the_guide_sentence_from_the_table() {
    let source = "\
//! Census the files a flight changed.
workflow gate_census
  outcome censused: type String, route success

command diff_census(base_pin: String, cargo_bin: String)
  program \"git\" \"diff\"
  refuses \"--\"
  specimen \"-x\"
  option \"--name-only\"
  operand range: String = \"{{base_pin}}..HEAD\"
  env GIT_PAGER = \"cat\"
  cwd \"{workspace_root}\"
  hardening
    path \"{{cargo_bin}}\"
    open_files 65536
  timeout 10m owner \"waffles\"

step census
  route censused(\"ok\")
";
    let words = [
        "command",
        "program",
        "refuses",
        "specimen",
        "option",
        "operand",
        "env",
        "cwd",
        "hardening",
        "path",
        "open_files",
    ];
    for word in words {
        let Some(entry) = aion_awl::guide::for_word(word) else {
            unreachable!("`{word}` must have a glossary entry");
        };
        let at = source
            .find(&format!("\n  {word} "))
            .map(|index| index + 3)
            .or_else(|| source.find(&format!("\n  {word}\n")).map(|index| index + 3))
            .or_else(|| {
                source
                    .find(&format!("\n    {word} "))
                    .map(|index| index + 5)
            })
            .or_else(|| source.find(&format!("\n{word} ")).map(|index| index + 1));
        let Some(offset) = at else {
            unreachable!("the fixture must carry `{word}` opening a line");
        };
        let text = hover_text(hover_at(source, position_at(source, offset)));
        assert!(
            text.contains(entry.summary()),
            "hovering `{word}` must render the guide's own sentence; got:\n{text}"
        );
        assert!(
            text.contains(&entry.citation()),
            "hovering `{word}` must carry the guide's own citation; got:\n{text}"
        );
        assert!(
            text.contains("AWL reference §17"),
            "`{word}` must cite the command-declaration section; got:\n{text}"
        );
    }
}

/// The exception is narrow: the same word standing as an author's own name
/// hovers to nothing, so the rule the module doc states is still the rule.
#[test]
fn a_command_word_used_as_a_name_still_hovers_to_nothing() {
    let source = "\
//! A harness section, where `command` is a field name and not a declaration.
workflow launcher
  outcome done: type String, route success

worker crew
  action ask(prompt: String) -> String
  harness
    kind acp
    command \"claude\"

step go
  route done(\"ok\")
";
    let Some(offset) = source.find("    command \"claude\"").map(|index| index + 4) else {
        unreachable!("the fixture must carry the field name");
    };
    assert_eq!(
        hover_text(hover_at(source, position_at(source, offset))),
        "",
        "`command` inside a harness section is a field name; hover must claim nothing"
    );

    // The control: hover DOES answer in this fixture, so a green above cannot
    // mean hover is simply broken here.
    let Some(control) = source.find("workflow launcher") else {
        unreachable!("the fixture must carry the control word");
    };
    assert!(!hover_text(hover_at(source, position_at(source, control))).is_empty());
}

/// COMPLETION: a cursor inside a command body is offered the body's own
/// vocabulary, delegated from the one table and never re-typed here.
#[test]
fn completion_inside_a_command_body_offers_the_bodys_vocabulary() {
    let source = "\
//! Census.
workflow gate_census
  outcome censused: type String, route success

command diff_census(base_pin: String)
  program \"git\" \"diff\"
  
step census
  route censused(\"ok\")
";
    let Some(offset) = source.find("\n  \nstep").map(|index| index + 3) else {
        unreachable!("the fixture must carry an empty indented line in the command body");
    };
    let position = position_at(source, offset);
    let offered: Vec<String> = completions(source, position)
        .into_iter()
        .map(|item| item.label)
        .collect();
    let expected = aion_awl::guide::words_offered_at(aion_awl::guide::GrammarPosition::CommandBody);
    for word in expected {
        assert!(
            offered.iter().any(|label| label == word),
            "`{word}` is offered in a command body by the table but not by completion: {offered:?}"
        );
    }
}

/// BRIEF-R8's LSP row for the RECIPE cluster, held against THE TABLE rather
/// than against a copy.
///
/// Every one of the fifteen words the assembly surface adds must hover to
/// the guide's own sentence and citation. The expected text is read from
/// `aion_awl::guide` on each run, so this cannot pass against a tooltip that
/// paraphrased the reference — the only way it goes green is if hover
/// renders what the one table says.
///
/// The word list is DERIVED from the guide, not written here: every entry
/// citing §18 must be hovered by this test, so a sixteenth word added to the
/// cluster fails the build until the fixture carries it.
#[test]
fn every_recipe_word_hovers_to_the_guide_sentence_from_the_table() {
    let source = "\
//! Stage a flight: declare the wall, the artifacts, and the assembly.
workflow staging
  outcome staged: type String, route success

command push_branch(branch: String)
  program \"git\" \"push\"
  admits \"--\"
  operand target: String = \"{{branch}}\"

wall changed_paths
  allows \"crates/aion-awl\"
  excludes \"mutation vocabulary\"

artifact gate_entry \".fleet/gate-entry.sh\"
  digest \"1e1e3ad2ae66bb0bef5b82a8e154491032ee3d7598ac361a731a5f0917b47a6b\"
  restores \".fleet/gate-entry.sh\"

recipe stage_the_flight
  parameter branch: String per_item
  parameter control_expect: String re_derive derives \"origin/main:f\" extract \"line 22\"
  step read_it
    captures ground: String of artifact gate_entry
  step self_test
    after read_it
    requires \"workers serving the queue\"
    fires artifact gate_entry expecting \"deliverable_absent\"
  step push
    after self_test
    runs command push_branch

step announce
  route staged(\"ok\")
";
    // DERIVED, not enumerated: every glossary entry citing §18.
    let cluster: Vec<&'static str> = aion_awl::guide::all()
        .into_iter()
        .filter(|entry| entry.section().number() == 18)
        .map(aion_awl::guide::GuideEntry::word)
        .collect();
    assert_eq!(
        cluster.len(),
        15,
        "the recipe cluster carries fifteen words; the fixture below covers them: {cluster:?}"
    );

    // The three words that stand MID-LINE — `derives` and `extract` on a
    // `parameter` clause, `expecting` on a `fires` clause — are surfaced by
    // COMPLETION and not by hover, exactly as cut-1's `owner` is. Hover
    // answers for a word OPENING a line, because an identifier standing
    // mid-line is overwhelmingly a name, and claiming otherwise is the
    // false-hover the scoping exists to prevent. They are not skipped: the
    // assertion below is that each carries the guide's own documentation
    // where completion offers it.
    let midline = ["derives", "extract", "expecting"];
    let mut hovered = 0usize;

    for word in cluster {
        let Some(entry) = aion_awl::guide::for_word(word) else {
            unreachable!("`{word}` must have a glossary entry");
        };
        if midline.contains(&word) {
            let documentation = documentation_for(word)
                .unwrap_or_else(|| unreachable!("`{word}` must carry completion documentation"));
            let Documentation::MarkupContent(markup) = documentation else {
                unreachable!("`{word}`'s documentation must be markup");
            };
            assert!(
                markup.value.contains(entry.summary()) && markup.value.contains(&entry.citation()),
                "`{word}` stands mid-line and must be surfaced by completion with the guide's own \
                 sentence and citation; got:\n{}",
                markup.value
            );
            continue;
        }
        // The word standing where the parser recognises it: at column zero
        // for an opener, indented for a clause, one level deeper for a step
        // clause.
        let at = source
            .find(&format!("\n{word} "))
            .map(|index| index + 1)
            .or_else(|| source.find(&format!("\n  {word} ")).map(|index| index + 3))
            .or_else(|| {
                source
                    .find(&format!("\n    {word} "))
                    .map(|index| index + 5)
            });
        let Some(offset) = at else {
            unreachable!("the fixture must carry `{word}` opening a line");
        };
        let text = hover_text(hover_at(source, position_at(source, offset)));
        assert!(
            text.contains(entry.summary()),
            "hovering `{word}` must render the guide's own sentence; got:\n{text}"
        );
        assert!(
            text.contains(&entry.citation()),
            "hovering `{word}` must carry the guide's own citation; got:\n{text}"
        );
        assert!(
            text.contains("AWL reference §18"),
            "`{word}` must cite the recipe-declaration section; got:\n{text}"
        );
        hovered += 1;
    }
    assert_eq!(
        hovered,
        15 - midline.len(),
        "every word of the cluster that opens a line must have been hovered"
    );
}

/// The clause scoping holds ACROSS clusters: a `digest` typed inside a
/// command body hovers to nothing, and a `program` typed inside an artifact
/// body does too. Without this, widening hover past one section would make
/// every clause word answer everywhere.
#[test]
fn a_clause_word_of_the_wrong_cluster_hovers_to_nothing() {
    let in_a_command = "\
//! A command body, where `digest` is not a clause.
workflow probe
  outcome done: type String, route success

command diff_census
  program \"git\" \"diff\"
  digest \"deadbeef\"

step go
  route done(\"ok\")
";
    let Some(offset) = in_a_command.find("\n  digest ").map(|index| index + 3) else {
        unreachable!("the fixture must carry the word");
    };
    assert_eq!(
        hover_text(hover_at(in_a_command, position_at(in_a_command, offset))),
        "",
        "`digest` is no clause of a command body; hover must claim nothing"
    );

    let in_an_artifact = "\
//! An artifact body, where `program` is not a clause.
workflow probe
  outcome done: type String, route success

artifact gate \".fleet/gate-entry.sh\"
  program \"git\"

step go
  route done(\"ok\")
";
    let Some(offset) = in_an_artifact.find("\n  program ").map(|index| index + 3) else {
        unreachable!("the fixture must carry the word");
    };
    assert_eq!(
        hover_text(hover_at(
            in_an_artifact,
            position_at(in_an_artifact, offset)
        )),
        "",
        "`program` is no clause of an artifact body; hover must claim nothing"
    );

    // The control: `digest` DOES hover inside an artifact body, so the two
    // greens above cannot mean hover is simply broken for the word.
    let where_it_belongs = "\
//! An artifact body, where `digest` is exactly a clause.
workflow probe
  outcome done: type String, route success

artifact gate \".fleet/gate-entry.sh\"
  digest \"deadbeef\"

step go
  route done(\"ok\")
";
    let Some(offset) = where_it_belongs.find("\n  digest ").map(|index| index + 3) else {
        unreachable!("the fixture must carry the word");
    };
    assert!(
        hover_text(hover_at(
            where_it_belongs,
            position_at(where_it_belongs, offset)
        ))
        .contains("content digest"),
        "the control failed: `digest` must hover inside an artifact body"
    );
}
