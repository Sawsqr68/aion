use std::collections::BTreeSet;

use aion_awl::Keyword;
use aion_awl::guide::{GrammarPosition, words_offered_at};
use lsp_types::{CompletionItemKind, Position};

use super::{completions, keywords_for};

/// A document that PARSES, used where the point is context rather than damage.
const DOCUMENT: &str = "\
workflow triage
  input incident_id: String
  signal ack: PageAck
  outcome resolved: type Resolved, route success

type PageAck { responder: String }
type Resolved { incident_id: String }

worker ops
  action gather(incident_id: String) -> Diagnostics
  action page(responder: String) -> PageSent

step gather
  gather(incident_id: incident_id) -> diagnostics

step hold
  wait ack timeout 5m -> reply
";

/// Completions at the end of `source` once `insert` is appended — the shape an
/// editor actually asks about, with the cursor after a half-typed line.
fn at_end_of(source: &str, insert: &str) -> Vec<lsp_types::CompletionItem> {
    let text = format!("{source}{insert}");
    let line = u32::try_from(text.lines().count().saturating_sub(1)).unwrap_or_default();
    let column = u32::try_from(text.lines().next_back().unwrap_or_default().chars().count())
        .unwrap_or_default();
    completions(&text, Position::new(line, column))
}

fn labels(items: &[lsp_types::CompletionItem]) -> BTreeSet<String> {
    items.iter().map(|item| item.label.clone()).collect()
}

/// THE POINT OF THE WHOLE FEATURE. Completion is asked for while the line is
/// half-typed, so the document does NOT parse — and the declared names must
/// still come back.
///
/// `aion_awl::parse` genuinely fails on this input; the assertion below would
/// be empty for any provider written the way hover and go-to-definition are.
#[test]
fn declared_names_are_offered_while_the_document_does_not_parse() {
    let text = format!("{DOCUMENT}\nstep escalate\n  ga");
    assert!(
        aion_awl::parse(&text).is_err(),
        "the premise of this test is that the document is mid-edit and unparseable; if it parses, \
         the test proves nothing about the case completion exists for"
    );

    let items = at_end_of(DOCUMENT, "\nstep escalate\n  ga");
    let labels = labels(&items);
    assert!(
        labels.contains("gather") && labels.contains("page"),
        "a step body must offer the actions the document declares, even unparsed: {labels:?}"
    );
}

/// A document that does not even LEX is still served, because the cursor's own
/// half-typed line is the usual reason and blanking it recovers the rest.
#[test]
fn declared_names_survive_a_line_that_cannot_be_lexed() {
    let broken = "\n  \"unterminated";
    let text = format!("{DOCUMENT}{broken}");
    assert!(
        aion_awl::lex(&text).is_err(),
        "the premise is a source the lexer itself rejects"
    );

    let items = at_end_of(DOCUMENT, broken);
    assert!(
        !items.is_empty(),
        "an unlexable line must not empty the completion list — the vocabulary needs no document"
    );
}

/// Context beats the enclosing block: `route ` wants outcomes and steps, not
/// another statement opener.
#[test]
fn route_offers_outcomes_and_steps_not_statement_openers() {
    let labels = labels(&at_end_of(DOCUMENT, "\nstep decide\n  route "));
    assert!(
        labels.contains("resolved"),
        "route must offer the declared outcome: {labels:?}"
    );
    assert!(
        labels.contains("gather") && labels.contains("hold"),
        "route must offer step names: {labels:?}"
    );
    assert!(
        !labels.contains(Keyword::Spawn.as_word()),
        "route must NOT offer statement openers — that is the context being wrong: {labels:?}"
    );
}

/// `wait` and `send` want signals, and nothing else in the document.
#[test]
fn wait_offers_signals_only() {
    let labels = labels(&at_end_of(DOCUMENT, "\nstep listen\n  wait "));
    assert!(
        labels.contains("ack"),
        "wait must offer the declared signal: {labels:?}"
    );
    assert!(
        !labels.contains("gather"),
        "wait must not offer actions — a signal is the only thing that can stand here: {labels:?}"
    );
}

/// After a `:` the author is naming a type, so declared types and the built-ins
/// are offered and step names are not.
#[test]
fn a_type_position_offers_declared_and_builtin_types() {
    let labels = labels(&at_end_of(DOCUMENT, "\n  input reason: "));
    for expected in ["String", "Int", "Float", "Bool", "PageAck", "Resolved"] {
        assert!(
            labels.contains(expected),
            "a type position must offer `{expected}`: {labels:?}"
        );
    }
    assert!(
        !labels.contains("gather"),
        "a type position must not offer step or action names: {labels:?}"
    );
}

/// Column zero opens a declaration; an indented line under `worker` does not.
#[test]
fn the_enclosing_block_decides_when_nothing_else_does() {
    let top = labels(&at_end_of(DOCUMENT, "\n"));
    assert!(
        top.contains(Keyword::Worker.as_word()) && top.contains(Keyword::Step.as_word()),
        "column zero opens a top-level declaration: {top:?}"
    );
    assert!(
        !top.contains(Keyword::Action.as_word()),
        "`action` belongs inside a worker, not at column zero: {top:?}"
    );

    let inside = labels(&at_end_of(DOCUMENT, "\nworker more\n  "));
    assert!(
        inside.contains(Keyword::Action.as_word()),
        "a worker block offers `action`: {inside:?}"
    );
    assert!(
        !inside.contains(Keyword::Workflow.as_word()),
        "a worker block does not offer `workflow`: {inside:?}"
    );
}

/// A pipe stage offers the pipe verbs.
#[test]
fn a_pipe_offers_its_stages() {
    let labels = labels(&at_end_of(DOCUMENT, "\nstep sift\n  diagnostics |> "));
    for verb in [Keyword::Filter, Keyword::Sort, Keyword::Map, Keyword::Count] {
        assert!(
            labels.contains(verb.as_word()),
            "a pipe must offer `{}`: {labels:?}",
            verb.as_word()
        );
    }
}

/// The positional words are offered too. They are not in `Keyword` — reserving
/// them would break real documents — but an author still has to type them, and
/// a completion list that omits them teaches a language smaller than the one
/// the parser accepts.
#[test]
fn the_positional_words_are_offered_where_the_parser_accepts_them() {
    let body = labels(&at_end_of(DOCUMENT, "\nstep reply\n  "));
    assert!(
        body.contains(aion_awl::SEND_WORD) && body.contains(aion_awl::ANSWER_WORD),
        "a step body must offer `send` and `answer`: {body:?}"
    );
    let worker = labels(&at_end_of(
        DOCUMENT,
        "\nworker shell\n  action x() -> Y\n    ",
    ));
    assert!(
        worker.contains(aion_awl::RUN_WORD),
        "a worker block must offer the declarative body word `run`: {worker:?}"
    );
}

/// NO SILENT VOCABULARY LOSS. Every reserved word in the language is offered
/// in at least one context.
///
/// The per-context lists are curated on purpose — offering all 57 words
/// everywhere is a dictionary, not completion — but curation is exactly how a
/// word goes missing without anyone noticing. Adding a keyword and forgetting
/// to place it fails here, naming it.
#[test]
fn every_keyword_is_offered_somewhere() {
    // The context list is the table's own inventory, not a hand-kept copy —
    // a position added to the grammar is swept automatically instead of
    // being silently absent here.
    let offered: BTreeSet<&'static str> = GrammarPosition::ALL
        .into_iter()
        .flat_map(keywords_for)
        .collect();
    assert!(
        offered.len() > 20,
        "the control failed: only {} words were gathered across every context, so the comparison \
         below would be measuring an empty harness rather than the curation",
        offered.len()
    );

    let missing: Vec<&'static str> = Keyword::ALL
        .into_iter()
        .map(Keyword::as_word)
        .filter(|word| !offered.contains(word))
        .collect();
    assert!(
        missing.is_empty(),
        "these reserved words are offered in NO context, so an author can never be told they \
         exist: {missing:?}"
    );
}

/// The value words are offered where authors actually write them — the
/// cursor positions the checker accepts (`const c = schema of T`,
/// `const c = json { … }`, `type T = schema { … }`, `run json "…"`), pinned
/// against `completions()` itself so a cue that stops firing fails here even
/// while the table still lists the word.
#[test]
fn value_words_are_offered_where_authors_actually_write_them() {
    let at_const = labels(&at_end_of(DOCUMENT, "\nconst c = "));
    assert!(
        at_const.contains("schema") && at_const.contains("json"),
        "a const's value slot must offer `schema` and `json`: {at_const:?}"
    );
    let at_type_door = labels(&at_end_of(DOCUMENT, "\ntype Payload = "));
    assert!(
        at_type_door.contains("schema"),
        "a type door must offer `schema`: {at_type_door:?}"
    );
    let mid_word = labels(&at_end_of(DOCUMENT, "\ntype Payload = sch"));
    assert!(
        mid_word.contains("schema"),
        "a half-typed door keyword still gets the offer — the partial word is a filter, not a \
         context: {mid_word:?}"
    );
    let after_schema = labels(&at_end_of(DOCUMENT, "\nconst c = schema "));
    assert!(
        after_schema.contains("of"),
        "`schema ` must offer `of`: {after_schema:?}"
    );
    let run_body = labels(&at_end_of(
        DOCUMENT,
        "\nworker shell\n  action x() -> Y\n    run ",
    ));
    assert!(
        run_body.contains("json"),
        "a declared body's `run ` must offer the `json` capture selector: {run_body:?}"
    );
    // The guard: an author's own `run` binding in a step body is NOT a
    // declared body, so the capture selector is not offered there.
    let step_run = labels(&at_end_of(DOCUMENT, "\nstep more\n  run "));
    assert!(
        !step_run.contains("json"),
        "`run` outside a worker must not open the capture selector: {step_run:?}"
    );
}

/// EVERY OFFER IS REACHABLE. For each grammar position that offers any
/// words, a real cursor shape reaches it and `completions()` returns every
/// offered word there. `every_keyword_is_offered_somewhere` cannot see
/// reachability — it calls the table on a hand-written context list — so
/// this is the pin that catches a table home no cue can reach, which is
/// exactly how `schema` was once left offered only where the parser
/// refuses it.
#[test]
fn every_offering_position_is_reachable_through_completions() {
    let probes: Vec<(GrammarPosition, &str)> = vec![
        (GrammarPosition::TopLevel, "\n"),
        (GrammarPosition::WorkflowHeader, "\nworkflow more\n  "),
        (GrammarPosition::WorkerBlock, "\nworker more\n  "),
        (GrammarPosition::StepBody, "\nstep more\n  "),
        (GrammarPosition::RouteTarget, "\nstep more\n  route "),
        (GrammarPosition::StepReference, "\nstep more\n  after "),
        (GrammarPosition::SignalReference, "\nstep more\n  wait "),
        (GrammarPosition::PipeStage, "\nstep more\n  diagnostics |> "),
        (
            GrammarPosition::Condition,
            "\nstep more\n  outcome a: when ",
        ),
        (GrammarPosition::ConstValue, "\nconst c = "),
        (GrammarPosition::TypeDoor, "\ntype Payload = "),
        (GrammarPosition::SchemaSource, "\nconst c = schema "),
        (
            GrammarPosition::RunCapture,
            "\nworker shell\n  action x() -> Y\n    run ",
        ),
    ];
    let covered: Vec<GrammarPosition> = probes.iter().map(|(position, _)| *position).collect();
    for position in GrammarPosition::ALL {
        let offered = words_offered_at(position);
        if offered.is_empty() {
            continue;
        }
        assert!(
            covered.contains(&position),
            "{position:?} offers words but this pin has no cursor probe for it — add one, which \
             will force a reachable cue to exist"
        );
    }
    for (position, insert) in probes {
        let offered = words_offered_at(position);
        assert!(
            !offered.is_empty(),
            "the probe for {position:?} is stale: the position no longer offers any words"
        );
        let found = labels(&at_end_of(DOCUMENT, insert));
        let missing: Vec<&str> = offered
            .into_iter()
            .filter(|word| !found.contains(*word))
            .collect();
        assert!(
            missing.is_empty(),
            "at the {position:?} cursor, completions() omits offered words {missing:?}: {found:?}"
        );
    }
}

/// What the document declares sorts above the language's keywords.
#[test]
fn declared_names_sort_above_keywords() {
    let items = at_end_of(DOCUMENT, "\nstep more\n  ");
    let action = items
        .iter()
        .find(|item| item.label == "gather")
        .and_then(|item| item.sort_text.clone())
        .unwrap_or_default();
    let keyword = items
        .iter()
        .find(|item| item.label == Keyword::Spawn.as_word())
        .and_then(|item| item.sort_text.clone())
        .unwrap_or_default();
    assert!(
        !action.is_empty() && !keyword.is_empty(),
        "the control failed: both items must exist for the ordering to mean anything"
    );
    assert!(
        action < keyword,
        "an action the author wrote must rank above a keyword they did not: {action} vs {keyword}"
    );
}

/// Bindings a step body produces are in scope for the statements after them.
#[test]
fn bindings_produced_by_a_statement_are_offered() {
    let labels = labels(&at_end_of(DOCUMENT, "\nstep use\n  "));
    assert!(
        labels.contains("diagnostics") && labels.contains("reply"),
        "the `-> name` bindings the document creates must be offered: {labels:?}"
    );
}

/// Every item carries a kind, so an editor can render it as something other
/// than undifferentiated text.
#[test]
fn every_item_is_classified() {
    let items = at_end_of(DOCUMENT, "\nstep more\n  ");
    assert!(!items.is_empty());
    for item in &items {
        assert!(
            item.kind.is_some() && item.kind != Some(CompletionItemKind::TEXT),
            "`{}` came back unclassified",
            item.label
        );
        assert!(item.detail.is_some(), "`{}` came back bare", item.label);
    }
}
