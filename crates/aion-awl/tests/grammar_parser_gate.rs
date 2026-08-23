//! The gate that ties the grammar table to the parser itself.
//!
//! `aion_awl::guide`'s grammar table says which words may stand where, and
//! three surfaces print it as prose — the ops console's language panel, the
//! CLI's guide, and the editor's completion. The table's own unit tests can
//! only check the table against itself; nothing there can see a word offered
//! in a position the parser refuses. This gate closes that hole by asking
//! the parser: every probe below builds a real document with a word standing
//! in a claimed position and reads what `aion_awl::parse` (and for the
//! witness documents, the full checker) actually does.
//!
//! Two claim shapes are gated, per position:
//!
//! * **Opener positions** — where a line's first word is dispatched by an
//!   exhaustive parser `match` with a distinctive refusal diagnostic — are
//!   gated by EQUALITY: the probe-admitted set must be exactly the offered
//!   set (top level), or the offered set must be exactly the admitted
//!   openers plus explicitly witnessed mid-line words (workflow header,
//!   worker block). The refusal diagnostics are additionally cross-checked
//!   against the probe results, so the sentence the parser prints and the
//!   dispatch it performs cannot drift apart unnoticed.
//! * **Witnessed words** — words legitimately offered in a region without
//!   opening its lines (`type` inside `outcome done: type String, …`) —
//!   each carry a witness document through the real `parse` + `check_in`
//!   pipeline, exactly as `guide_tests.rs` runs the glossary's examples.
//!
//! COVERAGE, stated exactly: this gate covers `top-level` in both
//! directions; `workflow-header`, `worker-block` and `command-body` in both
//! directions (admitted openers plus a witness per offered mid-line word);
//! and the
//! four value positions (`const-value`, `type-door`, `schema-source`,
//! `run-capture`) in the offered direction — every offered word carries a
//! checker-passing witness whose token stream shows the word in the claimed
//! slot; there is no dispatcher diagnostic to probe for an admitted
//! direction there. It does NOT cover `step-body` or the remaining
//! cursor-cue positions (`route-target`, `step-reference`,
//! `signal-reference`, `pipe-stage`, `condition`): a step body admits
//! arbitrary expression heads (any identifier, `true`, `workflow.id`,
//! `not …`), so "admitted opener ⇒ offered" is not an invariant there —
//! curation may honestly under-offer — and the offered-⇒-admitted
//! direction for those ~40 (word, position) pairs needs a hand-authored
//! witness corpus this lane does not carry. `type-body` and
//! `type-reference` are gated in the EMPTINESS direction: the parser
//! admits no vocabulary word in either slot, probed word by word, so their
//! offered lists must stay empty. The glossary gate meanwhile runs every
//! word's complete examples through the same checker, so every word is
//! exercised somewhere real; what is not machine-checked is only the
//! per-position filing outside the positions above.

use std::collections::BTreeSet;

use aion_awl::guide::{GrammarPosition, words_offered_at};
use aion_awl::{Keyword, POSITIONAL_WORDS, TokenKind};

/// Every word of the language: reserved then positional.
fn vocabulary() -> Vec<&'static str> {
    Keyword::ALL
        .into_iter()
        .map(Keyword::as_word)
        .chain(POSITIONAL_WORDS)
        .collect()
}

/// The narration every probe document opens with (the parser requires it).
const NARRATION: &str = "//! Probe document for the grammar gate.\n";

/// The dispatcher refusals, by their stable openings. These are matched as
/// prefixes of real parse errors; if the parser rewords one, every probe
/// reads as "admitted" and the equality assertions below go loudly red —
/// the gate fails safe rather than quietly matching nothing.
const OPENER_REFUSAL: &str = "expected the `workflow` or `worker` header";
// Moved with the parser when the command-declaration surface landed: the
// dispatcher now admits `command` and its diagnostic names it, so the prefix
// this gate matches on had to follow. The ASSERTIONS below are unchanged —
// this constant is the gate's grip on the parser's own sentence, not a claim
// about what the parser should admit, and the equality checks are what would
// have caught a wording change that did not correspond to a dispatch change.
const DECLARATION_REFUSAL: &str =
    "expected a `const`, `type`, `worker`, `child`, `subflow`, `step`, or `command` declaration";
const HEADER_REFUSAL: &str =
    "expected `timeout`, `input`, `signal`, `send`, `query`, or `outcome` in the workflow header";

/// Whether parsing `source` fails with the given dispatcher refusal.
fn refused_with(source: &str, refusal: &str) -> bool {
    match aion_awl::parse(source) {
        Ok(_) => false,
        Err(error) => error.message.starts_with(refusal),
    }
}

/// Whether the parser admits `word` opening a document.
fn admitted_as_document_opener(word: &str) -> bool {
    let source = format!("{NARRATION}{word} probe\n");
    !refused_with(&source, OPENER_REFUSAL)
}

/// Whether the parser admits `word` opening a top-level declaration after a
/// complete, valid workflow.
fn admitted_at_declaration_slot(word: &str) -> bool {
    let source = format!(
        "{NARRATION}workflow probe\n  outcome done: type String, route success\n\nstep go\n  \
         route done(\"ok\")\n\n{word} probe\n"
    );
    !refused_with(&source, DECLARATION_REFUSAL)
}

/// Whether the parser admits `word` opening a workflow-header line.
fn admitted_as_header_opener(word: &str) -> bool {
    let source = format!(
        "{NARRATION}workflow probe\n  {word} probe_tail\n  outcome done: type String, route \
         success\n\nstep go\n  route done(\"ok\")\n"
    );
    !refused_with(&source, HEADER_REFUSAL)
}

/// Whether the parser admits `word` opening a worker-block item line.
///
/// Refusal here is read from the error's SPAN, not a message prefix: a
/// non-action keyword may either be refused inside the block ("expected an
/// `action` declaration inside the worker block") or end the block and be
/// refused by the top-level dispatcher instead — both land their error
/// exactly on the probe word, while an admitted opener fails later, on its
/// own missing continuation.
fn admitted_as_worker_item(word: &str) -> bool {
    let probe = format!("{word} probe_tail");
    let source = format!(
        "{NARRATION}workflow probe\n  outcome done: type String, route success\n\nworker crew\n  \
         {probe}\n\nstep go\n  route done(\"ok\")\n"
    );
    let Some(word_offset) = source.find(&probe) else {
        return false;
    };
    match aion_awl::parse(&source) {
        Ok(_) => true,
        Err(error) => error.span.start != word_offset,
    }
}

/// The words a refusal diagnostic names in backticks, up to the clause that
/// describes what was found instead.
fn backticked_words(message: &str) -> BTreeSet<String> {
    let claim = message.split(", found").next().unwrap_or(message);
    let mut words = BTreeSet::new();
    let mut rest = claim;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else { break };
        words.insert(after[..close].to_owned());
        rest = &after[close + 1..];
    }
    words
}

/// Whether `text` uses `word` as a real token (never inside a string or a
/// comment) — the same reading `guide_tests.rs` applies to examples.
fn uses_word(text: &str, word: &str) -> bool {
    let Ok(tokens) = aion_awl::lex(text) else {
        return false;
    };
    tokens.iter().any(|token| match &token.kind {
        TokenKind::Keyword(keyword) => keyword.as_word() == word,
        TokenKind::Identifier(name) => name == word,
        _ => false,
    })
}

/// Parse and fully check `source`; returns every complaint, so a witness
/// that fails says why.
fn check_document(source: &str) -> Vec<String> {
    match aion_awl::parse(source) {
        Err(error) => vec![format!(
            "{}:{}: {}",
            error.span.line, error.span.column, error.message
        )],
        Ok(document) => aion_awl::check_in(&document, std::env::temp_dir().as_path())
            .into_iter()
            .map(|error| {
                format!(
                    "{}:{}: {}",
                    error.span.line, error.span.column, error.message
                )
            })
            .collect(),
    }
}

/// TOP LEVEL, both directions: the offered set must be EXACTLY the words the
/// parser admits opening a document or a top-level declaration. This is the
/// gate class that catches a word filed "Declaration" that the parser
/// refuses at column zero — the falsehood the panel would otherwise print.
#[test]
fn the_top_level_offer_is_exactly_what_the_parser_admits() {
    // Controls: the probe machinery must be able to say yes and no.
    assert!(
        admitted_as_document_opener("workflow") && admitted_at_declaration_slot("const"),
        "the control failed: the probes cannot recognise an admitted word"
    );
    assert!(
        !admitted_as_document_opener("sleep") && !admitted_at_declaration_slot("sleep"),
        "the control failed: the probes cannot recognise a refused word"
    );

    let offered: BTreeSet<&str> = words_offered_at(GrammarPosition::TopLevel)
        .into_iter()
        .collect();
    let mut wrong = Vec::new();
    for word in vocabulary() {
        let admitted = admitted_as_document_opener(word) || admitted_at_declaration_slot(word);
        if admitted && !offered.contains(word) {
            wrong.push(format!(
                "`{word}` opens a top-level line but the table does not offer it there"
            ));
        }
        if !admitted && offered.contains(word) {
            wrong.push(format!(
                "`{word}` is offered at the top level but the parser refuses it there — the \
                 panel would print a false \"stands at the start of a top-level line\""
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The declaration refusal names exactly the set the dispatcher admits, so
/// the diagnostic and the dispatch cannot drift apart. (`workflow` is
/// admitted only as the document opener, which the diagnostic's own probe
/// position cannot reach, so it is checked against the opener refusal.)
#[test]
fn the_refusal_diagnostics_name_what_the_parser_actually_admits() {
    let source = format!(
        "{NARRATION}workflow probe\n  outcome done: type String, route success\n\nstep go\n  \
         route done(\"ok\")\n\nsleep probe\n"
    );
    let Err(error) = aion_awl::parse(&source) else {
        unreachable!("`sleep` must be refused at the top level");
    };
    let named = backticked_words(&error.message);
    let admitted: BTreeSet<String> = vocabulary()
        .into_iter()
        .filter(|word| admitted_at_declaration_slot(word))
        .map(str::to_owned)
        .collect();
    assert_eq!(
        named, admitted,
        "the top-level refusal names a different set than the dispatcher admits"
    );

    let header_probe = format!(
        "{NARRATION}workflow probe\n  sleep probe_tail\n  outcome done: type String, route \
         success\n\nstep go\n  route done(\"ok\")\n"
    );
    let Err(error) = aion_awl::parse(&header_probe) else {
        unreachable!("`sleep` must be refused in the header");
    };
    let named = backticked_words(&error.message);
    let admitted: BTreeSet<String> = vocabulary()
        .into_iter()
        .filter(|word| admitted_as_header_opener(word))
        .map(str::to_owned)
        .collect();
    assert_eq!(
        named, admitted,
        "the header refusal names a different set than the header dispatcher admits"
    );
}

/// A mid-line witness: one header line carrying the word, plus any extra
/// top-level declarations the line needs to check cleanly.
struct HeaderWitness {
    word: &'static str,
    header_line: &'static str,
    epilogue: &'static str,
}

/// The words offered in the workflow header WITHOUT opening a header line,
/// each witnessed standing mid-line in a header line the checker accepts.
const HEADER_WITNESSES: &[HeaderWitness] = &[
    HeaderWitness {
        word: "type",
        header_line: "outcome typed: type String, route success",
        epilogue: "",
    },
    HeaderWitness {
        word: "route",
        header_line: "outcome routed: type String, route success",
        epilogue: "",
    },
    HeaderWitness {
        word: "success",
        header_line: "outcome won: type String, route success",
        epilogue: "",
    },
    HeaderWitness {
        word: "failure",
        header_line: "outcome lost: type String, route failure",
        epilogue: "",
    },
    // `of` deliberately carries NO witness: `schema of` is an expression
    // form (`parser/exprs.rs`), header lines have no expression position,
    // and the probe run for this gate showed every attempted header shape
    // refused ("expected a type, found keyword `schema`"). Offering `of` in
    // the workflow header was therefore a false position, and the table no
    // longer claims it — this comment is the record of why.
];

/// WORKFLOW HEADER, both directions: every word the parser admits opening a
/// header line must be offered there, and every offered word must either be
/// such an opener or carry a checker-passing mid-line witness. A word with
/// neither is a false "stands in a workflow header"; a witness for a word no
/// longer offered is stale and refused too.
#[test]
fn the_header_offer_is_openers_plus_witnessed_midline_words() {
    assert!(
        admitted_as_header_opener("input") && !admitted_as_header_opener("sleep"),
        "the control failed: the header probe cannot tell an opener from a refusal"
    );

    let offered: BTreeSet<&str> = words_offered_at(GrammarPosition::WorkflowHeader)
        .into_iter()
        .collect();
    let openers: BTreeSet<&str> = vocabulary()
        .into_iter()
        .filter(|word| admitted_as_header_opener(word))
        .collect();

    let mut wrong = Vec::new();
    for opener in &openers {
        if !offered.contains(opener) {
            wrong.push(format!(
                "`{opener}` opens a workflow-header line but the table does not offer it there"
            ));
        }
    }

    let witnessed: BTreeSet<&str> = HEADER_WITNESSES
        .iter()
        .map(|witness| witness.word)
        .collect();
    for word in &offered {
        if !openers.contains(word) && !witnessed.contains(word) {
            wrong.push(format!(
                "`{word}` is offered in the workflow header, is refused as a header-line opener, \
                 and has no mid-line witness — the panel would print a false \"stands in a \
                 workflow header\""
            ));
        }
    }
    for word in &witnessed {
        if !offered.contains(word) || openers.contains(word) {
            wrong.push(format!(
                "the witness for `{word}` is stale: the word is not an offered mid-line header \
                 word any more"
            ));
        }
    }

    for witness in HEADER_WITNESSES {
        if !uses_word(witness.header_line, witness.word) {
            wrong.push(format!(
                "the witness line for `{}` never uses the word as a token",
                witness.word
            ));
            continue;
        }
        let source = format!(
            "//! Witness: `{}` standing inside a workflow header line.\nworkflow witness\n  {}\n  \
             outcome done: type String, route success\n\n{}step go\n  route done(\"ok\")\n",
            witness.word, witness.header_line, witness.epilogue
        );
        let complaints = check_document(&source);
        if !complaints.is_empty() {
            wrong.push(format!(
                "the witness for `{}` does not pass the real checker: {}",
                witness.word,
                complaints.join("; ")
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[path = "grammar_parser_gate/worker.rs"]
mod worker;

/// Whether the parser admits `word` opening a line inside a command body.
///
/// Refusal is read from the error's SPAN rather than a message prefix, for the
/// same reason the worker-item probe does: a word the command body does not
/// admit may be refused inside the block or may end the block and be refused
/// by the top-level dispatcher, and both land the error exactly on the probe
/// word, while an admitted clause fails later on its own missing continuation.
fn admitted_as_command_clause(word: &str) -> bool {
    let probe = format!("{word} probe_tail");
    let source = format!(
        "{NARRATION}workflow probe\n  outcome done: type String, route success\n\ncommand \
         probe_command\n  {probe}\n\nstep go\n  route done(\"ok\")\n"
    );
    let Some(word_offset) = source.find(&probe) else {
        return false;
    };
    match aion_awl::parse(&source) {
        Ok(_) => true,
        Err(error) => error.span.start != word_offset,
    }
}

/// A mid-line or nested witness: a command declaration the checker accepts,
/// in which the word stands where it actually stands.
struct CommandWitness {
    word: &'static str,
    declaration: &'static str,
}

/// The words offered in a command body WITHOUT opening a body line, each
/// witnessed standing where the parser really takes it.
///
/// `path` and `open_files` stand one level deeper, inside the `hardening`
/// block; `owner` stands mid-line on the `timeout` clause. They are offered in
/// the body because that is where an author's cursor is when they need them —
/// the same reason `type`, `route`, `success` and `failure` are offered in a
/// workflow header without opening a header line.
const COMMAND_WITNESSES: &[CommandWitness] = &[
    CommandWitness {
        word: "path",
        declaration: "command probe_command(bin: String)\n  program \"git\"\n  admits \"--\"\n  \
                      hardening\n    path \"{{bin}}\"",
    },
    CommandWitness {
        word: "open_files",
        declaration: "command probe_command\n  program \"git\"\n  admits \"--\"\n  hardening\n    \
                      open_files 65536",
    },
    CommandWitness {
        word: "owner",
        declaration: "command probe_command\n  program \"git\"\n  admits \"--\"\n  timeout 10m \
                      owner \"the gate's author\"",
    },
];

/// COMMAND BODY, both directions: every word the parser admits opening a
/// command-body line must be offered there, and every offered word must
/// either be such an opener or carry a checker-passing witness showing where
/// it really stands.
///
/// The same class the workflow header gets, applied to the position the
/// command-declaration surface added — so the panel and the completion list
/// cannot claim a clause the parser refuses, and cannot omit one it takes.
#[test]
fn the_command_body_offer_is_openers_plus_witnessed_words() {
    assert!(
        admitted_as_command_clause("program") && admitted_as_command_clause("operand"),
        "the control failed: the probe cannot recognise an admitted clause"
    );
    assert!(
        !admitted_as_command_clause("sleep") && !admitted_as_command_clause("recipe"),
        "the control failed: the probe cannot recognise a refused word"
    );

    let offered: BTreeSet<&str> = words_offered_at(GrammarPosition::CommandBody)
        .into_iter()
        .collect();
    let openers: BTreeSet<&str> = vocabulary()
        .into_iter()
        .filter(|word| admitted_as_command_clause(word))
        .collect();
    let witnessed: BTreeSet<&str> = COMMAND_WITNESSES
        .iter()
        .map(|witness| witness.word)
        .collect();

    let mut wrong = Vec::new();
    for opener in &openers {
        if !offered.contains(opener) {
            wrong.push(format!(
                "`{opener}` opens a command-body line but the table does not offer it there"
            ));
        }
    }
    for word in &offered {
        if !openers.contains(word) && !witnessed.contains(word) {
            wrong.push(format!(
                "`{word}` is offered in a command body, opens no body line, and carries no \
                 witness — the panel would print a false \"stands in a command declaration\""
            ));
        }
    }
    for witness in COMMAND_WITNESSES {
        if !offered.contains(witness.word) {
            wrong.push(format!(
                "`{}` carries a witness but is no longer offered in a command body; the witness \
                 is stale",
                witness.word
            ));
            continue;
        }
        let source = format!(
            "{NARRATION}workflow probe\n  outcome done: type String, route success\n\n{}\n\nstep \
             go\n  route done(\"ok\")\n",
            witness.declaration
        );
        let complaints = check_document(&source);
        if !complaints.is_empty() {
            wrong.push(format!(
                "`{}`'s witness does not check clean: {complaints:?}",
                witness.word
            ));
            continue;
        }
        if !uses_word(&source, witness.word) {
            wrong.push(format!(
                "`{}`'s witness never carries the word as a token",
                witness.word
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// AMENDMENT 1, STRUCTURAL. No recipe, dependency, or assembly form is
/// REACHABLE from a gate-leg command declaration: each named form is refused
/// by name, with its own cause, and the reason rides in the diagnostic text.
///
/// One test per named form, because a single test over a list would go green
/// on the day one form quietly started parsing while the others still failed.
#[test]
fn every_assembly_form_is_refused_by_name_with_its_reason() {
    for word in ["recipe", "depends", "needs", "after", "steps", "then"] {
        let source = format!(
            "{NARRATION}workflow probe\n  outcome done: type String, route success\n\ncommand \
             probe_command\n  program \"git\"\n  admits \"--\"\n  {word} \"x\"\n\nstep go\n  \
             route done(\"ok\")\n"
        );
        let Err(error) = aion_awl::parse(&source) else {
            unreachable!("`{word}` must be refused inside a command body");
        };
        assert!(
            error.message.contains("named cause: recipe_form_refused"),
            "`{word}` must be refused BY NAME as an assembly form, not by falling through to an \
             unknown-clause diagnostic; got: {}",
            error.message
        );
        assert!(
            error.message.contains("a reader"),
            "the refusal must carry the review property as its reason; got: {}",
            error.message
        );
    }
}
