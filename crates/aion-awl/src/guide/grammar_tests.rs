//! What the promoted grammar table must satisfy.
//!
//! Two families of claim. First, the table itself: it speaks only the
//! language's own vocabulary, and it drops none of it — a word offered in no
//! position is a word no completion list and no panel can ever teach.
//! Second, the derived axis: every form [`form_of`] assigns must be honest
//! against the table it was derived from, so a surface printing "step
//! statement" over a word is printing a fact about where completion offers
//! it, not an opinion.

use std::collections::BTreeSet;

use crate::lexer::{Keyword, POSITIONAL_WORDS};

use super::{
    GrammarForm, GrammarPosition, form_of, positional_offered_at, positions_of, words_offered_at,
};

/// NO SILENT VOCABULARY LOSS — the promoted home of the LSP's own pin.
/// Every reserved word and every positional word stands in at least one
/// position, so every word has a form and every word can be offered.
#[test]
fn every_word_of_the_vocabulary_stands_somewhere() {
    let offered: BTreeSet<&'static str> = GrammarPosition::ALL
        .into_iter()
        .flat_map(words_offered_at)
        .collect();
    assert!(
        offered.len() > 20,
        "the control failed: only {} words were gathered across every position, so the \
         comparison below would measure an empty harness rather than the curation",
        offered.len()
    );
    let mut missing: Vec<&'static str> = Keyword::ALL
        .into_iter()
        .map(Keyword::as_word)
        .filter(|word| !offered.contains(word))
        .collect();
    missing.extend(
        POSITIONAL_WORDS
            .into_iter()
            .filter(|word| !offered.contains(word)),
    );
    assert!(
        missing.is_empty(),
        "these words stand in NO position, so no surface can ever teach them: {missing:?}"
    );
}

/// The table speaks only the vocabulary: every positional word it offers is
/// one the lexer names positional. (The keyword lists are typed, so they
/// cannot stray; the string lists can.)
#[test]
fn the_table_offers_no_word_the_lexer_does_not_name() {
    for position in GrammarPosition::ALL {
        for word in positional_offered_at(position) {
            assert!(
                POSITIONAL_WORDS.contains(word),
                "`{word}` is offered {} but the lexer does not name it a positional word",
                position.label()
            );
        }
    }
}

/// `words_offered_at` is the LSP's delegation target, so its composition is
/// pinned against a position whose whole list is small enough to write out:
/// reserved words first in curated order, then the positional words.
#[test]
fn offered_words_compose_keywords_then_positional_words() {
    assert_eq!(
        words_offered_at(GrammarPosition::SignalReference),
        vec!["timeout", "to"]
    );
    assert_eq!(
        words_offered_at(GrammarPosition::WorkerBlock),
        vec![
            "action", "node", "timeout", "retry", "fallback", "every", "backoff", "advisory",
            "agent", "run"
        ]
    );
}

/// `positions_of` answers in the table's own order and reads both halves of
/// the table — a reserved word and a positional word each resolve.
#[test]
fn positions_are_read_from_both_halves_of_the_table_in_order() {
    // `timeout` is REUSED by the command-declaration surface rather than
    // respelled, so it now stands in a fifth position. The vector is pinned
    // whole on purpose: a word gaining or losing a position is a change to
    // what every completion list offers, and it must be stated here rather
    // than discovered later.
    assert_eq!(
        positions_of("timeout"),
        vec![
            GrammarPosition::WorkflowHeader,
            GrammarPosition::WorkerBlock,
            GrammarPosition::StepBody,
            GrammarPosition::SignalReference,
            GrammarPosition::CommandBody,
        ]
    );
    assert_eq!(positions_of("to"), vec![GrammarPosition::SignalReference]);
    assert_eq!(
        positions_of("definitely_not_a_word_of_awl"),
        Vec::<GrammarPosition>::new()
    );
}

/// THE DERIVED-AXIS GATE. Every word's form must be honest against the
/// table: the form claims a position family, and the word must actually be
/// offered there — with every family the derivation ranks higher absent.
/// This is what makes the panel's grouping a reading of the completion
/// table rather than a second, driftable account of the grammar.
#[test]
fn every_derived_form_is_honest_against_the_table() {
    let vocabulary: Vec<&'static str> = Keyword::ALL
        .into_iter()
        .map(Keyword::as_word)
        .chain(POSITIONAL_WORDS)
        .collect();
    for word in vocabulary {
        let positions = positions_of(word);
        let form = form_of(word);
        assert!(
            form.is_some(),
            "`{word}` is a word of the language and must have a form"
        );
        let Some(form) = form else {
            continue;
        };
        assert_form_is_honest(word, form, &positions);
    }
}

/// One word's form, held against the positions the table offers it in.
///
/// Its own function so the loop above reads as the sweep it is: this is the
/// per-form claim, and it is exhaustive with no wildcard, so a form added to
/// the taxonomy fails to compile here until someone says what standing in it
/// means.
fn assert_form_is_honest(word: &str, form: GrammarForm, positions: &[GrammarPosition]) {
    let stands_in = |position: GrammarPosition| positions.contains(&position);
    {
        match form {
            GrammarForm::Declaration => assert!(
                stands_in(GrammarPosition::TopLevel),
                "`{word}` is filed a declaration but may not stand at the top level"
            ),
            GrammarForm::Statement => assert!(
                stands_in(GrammarPosition::StepBody) && !stands_in(GrammarPosition::TopLevel),
                "`{word}` is filed a statement but its positions say otherwise: {positions:?}"
            ),
            GrammarForm::ValueWord => assert!(
                (stands_in(GrammarPosition::ConstValue)
                    || stands_in(GrammarPosition::TypeDoor)
                    || stands_in(GrammarPosition::SchemaSource)
                    || stands_in(GrammarPosition::RunCapture))
                    && !stands_in(GrammarPosition::TopLevel)
                    && !stands_in(GrammarPosition::StepBody),
                "`{word}` is filed a value word but its positions say otherwise: {positions:?}"
            ),
            GrammarForm::HeaderClause => assert!(
                (stands_in(GrammarPosition::WorkflowHeader)
                    || stands_in(GrammarPosition::WorkerBlock))
                    && !stands_in(GrammarPosition::TopLevel)
                    && !stands_in(GrammarPosition::StepBody)
                    && !stands_in(GrammarPosition::ConstValue)
                    && !stands_in(GrammarPosition::TypeDoor)
                    && !stands_in(GrammarPosition::SchemaSource)
                    && !stands_in(GrammarPosition::RunCapture),
                "`{word}` is filed a header clause but its positions say otherwise: {positions:?}"
            ),
            GrammarForm::PipelineStage => assert!(
                stands_in(GrammarPosition::PipeStage)
                    && positions.iter().all(|position| {
                        matches!(
                            position,
                            GrammarPosition::PipeStage | GrammarPosition::Condition
                        )
                    }),
                "`{word}` is filed a pipeline stage but its positions say otherwise: {positions:?}"
            ),
            GrammarForm::ConditionWord => assert_eq!(
                positions,
                vec![GrammarPosition::Condition],
                "`{word}` is filed a condition word but stands elsewhere too"
            ),
            GrammarForm::CommandClause => assert!(
                stands_in(GrammarPosition::CommandBody)
                    && !stands_in(GrammarPosition::TopLevel)
                    && !stands_in(GrammarPosition::StepBody)
                    && !stands_in(GrammarPosition::WorkflowHeader)
                    && !stands_in(GrammarPosition::WorkerBlock)
                    && !stands_in(GrammarPosition::ConstValue)
                    && !stands_in(GrammarPosition::TypeDoor)
                    && !stands_in(GrammarPosition::SchemaSource)
                    && !stands_in(GrammarPosition::RunCapture),
                "`{word}` is filed a command clause but its positions say otherwise: {positions:?}"
            ),
            GrammarForm::AssemblyClause => assert!(
                (stands_in(GrammarPosition::RecipeBody)
                    || stands_in(GrammarPosition::RecipeStep)
                    || stands_in(GrammarPosition::WallBody)
                    || stands_in(GrammarPosition::ArtifactBody))
                    && !stands_in(GrammarPosition::TopLevel)
                    && !stands_in(GrammarPosition::StepBody)
                    && !stands_in(GrammarPosition::WorkflowHeader)
                    && !stands_in(GrammarPosition::WorkerBlock)
                    && !stands_in(GrammarPosition::ConstValue)
                    && !stands_in(GrammarPosition::TypeDoor)
                    && !stands_in(GrammarPosition::SchemaSource)
                    && !stands_in(GrammarPosition::RunCapture)
                    && !stands_in(GrammarPosition::CommandBody),
                "`{word}` is filed an assembly clause but its positions say otherwise: \
                 {positions:?}"
            ),
            GrammarForm::TargetWord => assert!(
                positions.iter().all(|position| {
                    matches!(
                        position,
                        GrammarPosition::RouteTarget
                            | GrammarPosition::StepReference
                            | GrammarPosition::SignalReference
                    )
                }) && !positions.is_empty(),
                "`{word}` is filed a target word but its positions say otherwise: {positions:?}"
            ),
        }
    }
}

/// The precedence's visible outcomes, pinned by word so a table edit that
/// silently refiles a word fails here naming it.
#[test]
fn the_canonical_words_land_in_their_forms() {
    let filed = [
        ("workflow", GrammarForm::Declaration),
        ("step", GrammarForm::Declaration),
        // `send` declares in the header and sends in a step body; the
        // parser refuses it at the top level (`tests/grammar_parser_gate`),
        // so the precedence files it by its step-body stand.
        ("send", GrammarForm::Statement),
        // `query` stands only in the workflow header (`query name: Type`).
        ("query", GrammarForm::HeaderClause),
        // `schema` opens no top-level line: it stands after `=` on const
        // and type lines, so it files as a value word.
        ("schema", GrammarForm::ValueWord),
        ("wait", GrammarForm::Statement),
        ("route", GrammarForm::Statement),
        ("outcome", GrammarForm::Statement),
        ("in", GrammarForm::Statement),
        ("of", GrammarForm::ValueWord),
        ("json", GrammarForm::ValueWord),
        ("input", GrammarForm::HeaderClause),
        ("action", GrammarForm::HeaderClause),
        ("run", GrammarForm::HeaderClause),
        ("success", GrammarForm::HeaderClause),
        ("filter", GrammarForm::PipelineStage),
        ("any", GrammarForm::PipelineStage),
        ("is", GrammarForm::ConditionWord),
        ("otherwise", GrammarForm::ConditionWord),
        ("to", GrammarForm::TargetWord),
        // `command` opens a top-level line, so the precedence files it a
        // declaration exactly as it files `workflow` and `step`.
        ("command", GrammarForm::Declaration),
        // Its clauses stand only inside a command body.
        ("program", GrammarForm::CommandClause),
        ("admits", GrammarForm::CommandClause),
        ("refuses", GrammarForm::CommandClause),
        ("specimen", GrammarForm::CommandClause),
        ("operand", GrammarForm::CommandClause),
        ("open_files", GrammarForm::CommandClause),
        // `timeout` stands in a command body too, and gaining that position
        // does not refile it: the derivation files a word under the outermost
        // thing it can open, and `timeout` opens a step-body statement, which
        // outranks both the header and the command body.
        ("timeout", GrammarForm::Statement),
        // The assembly openers stand at the top level, so the precedence
        // files them declarations exactly as it files `command`.
        ("recipe", GrammarForm::Declaration),
        ("wall", GrammarForm::Declaration),
        ("artifact", GrammarForm::Declaration),
        // Their clauses stand only inside an assembly body.
        ("parameter", GrammarForm::AssemblyClause),
        ("derives", GrammarForm::AssemblyClause),
        ("extract", GrammarForm::AssemblyClause),
        ("allows", GrammarForm::AssemblyClause),
        ("excludes", GrammarForm::AssemblyClause),
        ("digest", GrammarForm::AssemblyClause),
        ("restores", GrammarForm::AssemblyClause),
        ("runs", GrammarForm::AssemblyClause),
        ("captures", GrammarForm::AssemblyClause),
        ("fires", GrammarForm::AssemblyClause),
        ("expecting", GrammarForm::AssemblyClause),
        ("requires", GrammarForm::AssemblyClause),
        // `step` and `after` gain a recipe position and are NOT refiled:
        // `step` opens a top-level declaration and `after` opens a step-body
        // statement, and both outrank an assembly body. One word, one
        // meaning, read where it stands.
        ("after", GrammarForm::Statement),
        // `of` gains the recipe-step position and stays a value word: it
        // opens a declaration's value after `=`, which outranks it.
        ("of", GrammarForm::ValueWord),
    ];
    for (word, expected) in filed {
        assert_eq!(
            form_of(word),
            Some(expected),
            "`{word}` is filed under the wrong form"
        );
    }
    assert_eq!(
        form_of("definitely_not_a_word_of_awl"),
        None,
        "a word the language does not have must have no form"
    );
}

/// No form is a dead taxonomy entry: every form files at least one word, so
/// a panel grouping by form never renders an empty heading and a form whose
/// last word moved away fails loudly here.
#[test]
fn every_form_files_at_least_one_word() {
    let vocabulary: Vec<&'static str> = Keyword::ALL
        .into_iter()
        .map(Keyword::as_word)
        .chain(POSITIONAL_WORDS)
        .collect();
    for form in GrammarForm::ALL {
        assert!(
            vocabulary.iter().any(|word| form_of(word) == Some(form)),
            "no word of the language is filed under {:?} ({})",
            form,
            form.label()
        );
    }
}

/// `ALL` is how every consumer iterates the taxonomies, so each must carry
/// every variant. The ordinal functions are exhaustive matches with no
/// wildcard: a new variant fails to COMPILE here until it is given an
/// ordinal, and the assertions then refuse an `ALL` that does not carry it
/// (`positional_offered_at` is already an exhaustive match itself, so the
/// compiler forces its arm the same way).
#[test]
fn the_all_inventories_carry_every_variant() {
    fn position_ordinal(position: GrammarPosition) -> usize {
        match position {
            GrammarPosition::TopLevel => 0,
            GrammarPosition::WorkflowHeader => 1,
            GrammarPosition::WorkerBlock => 2,
            GrammarPosition::StepBody => 3,
            GrammarPosition::TypeBody => 4,
            GrammarPosition::RouteTarget => 5,
            GrammarPosition::StepReference => 6,
            GrammarPosition::SignalReference => 7,
            GrammarPosition::PipeStage => 8,
            GrammarPosition::TypeReference => 9,
            GrammarPosition::Condition => 10,
            GrammarPosition::ConstValue => 11,
            GrammarPosition::TypeDoor => 12,
            GrammarPosition::SchemaSource => 13,
            GrammarPosition::RunCapture => 14,
            GrammarPosition::CommandBody => 15,
            GrammarPosition::RecipeBody => 16,
            GrammarPosition::RecipeStep => 17,
            GrammarPosition::WallBody => 18,
            GrammarPosition::ArtifactBody => 19,
        }
    }
    fn form_ordinal(form: GrammarForm) -> usize {
        match form {
            GrammarForm::Declaration => 0,
            GrammarForm::HeaderClause => 1,
            GrammarForm::Statement => 2,
            GrammarForm::PipelineStage => 3,
            GrammarForm::ConditionWord => 4,
            GrammarForm::ValueWord => 5,
            GrammarForm::TargetWord => 6,
            GrammarForm::CommandClause => 7,
            GrammarForm::AssemblyClause => 8,
        }
    }
    const POSITION_VARIANTS: usize = 20;
    const FORM_VARIANTS: usize = 9;

    let mut seen = [false; POSITION_VARIANTS];
    for position in GrammarPosition::ALL {
        seen[position_ordinal(position)] = true;
    }
    assert_eq!(GrammarPosition::ALL.len(), POSITION_VARIANTS);
    assert!(
        seen.iter().all(|hit| *hit),
        "GrammarPosition::ALL does not carry every variant"
    );

    let mut seen = [false; FORM_VARIANTS];
    for form in GrammarForm::ALL {
        seen[form_ordinal(form)] = true;
    }
    assert_eq!(GrammarForm::ALL.len(), FORM_VARIANTS);
    assert!(
        seen.iter().all(|hit| *hit),
        "GrammarForm::ALL does not carry every variant"
    );
}

/// The wire identifiers are what the console keys groups by, so they must
/// be distinct and shaped like identifiers; the labels and descriptions are
/// rendered prose, so they must be one finished line each.
#[test]
fn wire_identifiers_are_distinct_and_prose_is_one_finished_line() {
    let mut form_ids = BTreeSet::new();
    for form in GrammarForm::ALL {
        assert!(
            form_ids.insert(form.id()),
            "form id `{}` is claimed twice",
            form.id()
        );
        assert!(!form.label().is_empty() && !form.label().contains('\n'));
        assert!(
            form.description().ends_with('.') && !form.description().contains('\n'),
            "{form:?} needs a one-line, finished-sentence description"
        );
    }
    let mut position_ids = BTreeSet::new();
    for position in GrammarPosition::ALL {
        assert!(
            position_ids.insert(position.id()),
            "position id `{}` is claimed twice",
            position.id()
        );
        assert!(!position.label().is_empty() && !position.label().contains('\n'));
    }
}
