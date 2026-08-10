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
//! directions; `workflow-header` and `worker-block` in both directions
//! (admitted openers plus a witness per offered mid-line word); and the
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
const DECLARATION_REFUSAL: &str =
    "expected a `const`, `type`, `worker`, `child`, `subflow`, or `step` declaration";
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

/// A worker-region witness: a complete document in which the word stands
/// inside the worker block (and, to keep the region claim honest, nowhere
/// before it).
struct WorkerWitness {
    word: &'static str,
    source: &'static str,
}

/// The words offered in the worker block WITHOUT opening an item line, each
/// witnessed inside a worker block the checker accepts. Every witness uses
/// its word only after the `worker` line, which the test verifies against
/// the token stream.
const WORKER_WITNESSES: &[WorkerWitness] = &[
    WorkerWitness {
        word: "node",
        source: "//! Witness: `node` pinning an action inside a worker block.\nworkflow \
                 witness\n  input name: String\n  outcome done: type String, route \
                 success\n\nworker crew\n  action ping(name: String) -> Nil\n    node \
                 primary\n\nstep go\n  ping(name: name)\n  route done(name)\n",
    },
    WorkerWitness {
        word: "timeout",
        source: "//! Witness: `timeout` bounding an action inside a worker block.\nworkflow \
                 witness\n  input name: String\n  outcome done: type String, route \
                 success\n\nworker crew\n  action ping(name: String) -> Nil\n    advisory, \
                 timeout 30s\n\nstep go\n  ping(name: name)\n  route done(name)\n",
    },
    WorkerWitness {
        word: "retry",
        source: "//! Witness: `retry` on an action inside a worker block.\nworkflow witness\n  \
                 input name: String\n  outcome done: type String, route success\n\nworker crew\n  \
                 action ping(name: String) -> Nil\n    retry 5 every 10s\n\nstep go\n  ping(name: \
                 name)\n  route done(name)\n",
    },
    WorkerWitness {
        word: "every",
        source: "//! Witness: `every` fixing a retry interval inside a worker block.\nworkflow \
                 witness\n  input name: String\n  outcome done: type String, route \
                 success\n\nworker crew\n  action ping(name: String) -> Nil\n    retry 5 every \
                 10s\n\nstep go\n  ping(name: name)\n  route done(name)\n",
    },
    WorkerWitness {
        word: "backoff",
        source: "//! Witness: `backoff` growing a retry interval inside a worker \
                 block.\nworkflow witness\n  input name: String\n  outcome done: type String, \
                 route success\n\nworker crew\n  action ping(name: String) -> Nil\n    retry 4 \
                 backoff 10s..2m\n\nstep go\n  ping(name: name)\n  route done(name)\n",
    },
    WorkerWitness {
        word: "advisory",
        source: "//! Witness: `advisory` marking an action inside a worker block.\nworkflow \
                 witness\n  input name: String\n  outcome done: type String, route \
                 success\n\nworker crew\n  action ping(name: String) -> Nil\n    advisory, \
                 timeout 30s\n\nstep go\n  ping(name: name)\n  route done(name)\n",
    },
    WorkerWitness {
        word: "agent",
        source: "//! Witness: `agent` marking an action inside a worker block.\nworkflow \
                 witness\n  input question: String\n  outcome done: type String, route \
                 success\n\ntype Reply { text: String, stop_reason: String }\n\nworker crew\n  \
                 action ask(prompt: String) -> Reply\n    agent, timeout 10m\n\nstep go\n  \
                 ask(prompt: question) -> reply\n  route done(reply.text)\n",
    },
    WorkerWitness {
        word: "run",
        source: "//! Witness: `run` as a declared action body inside a worker block.\nworkflow \
                 witness\n  input name: String\n  outcome done: type Stdout, route \
                 success\n\ntype Stdout { stdout: String }\n\nworker shell\n  action greet(name: \
                 String) -> Stdout\n    run \"echo hello\"\n\nstep go\n  greet(name: name) -> \
                 out\n  route done(out)\n",
    },
];

/// WORKER BLOCK, both directions: item-line openers must be offered, and
/// every offered non-opener word must be witnessed standing after the
/// `worker` line of a checker-passing document, using the word nowhere
/// before that line.
#[test]
fn the_worker_block_offer_is_openers_plus_witnessed_words() {
    assert!(
        admitted_as_worker_item("action") && !admitted_as_worker_item("sleep"),
        "the control failed: the worker-item probe cannot tell an opener from a refusal"
    );

    let offered: BTreeSet<&str> = words_offered_at(GrammarPosition::WorkerBlock)
        .into_iter()
        .collect();
    let openers: BTreeSet<&str> = vocabulary()
        .into_iter()
        .filter(|word| admitted_as_worker_item(word))
        .collect();

    let mut wrong = Vec::new();
    for opener in &openers {
        if !offered.contains(opener) {
            wrong.push(format!(
                "`{opener}` opens a worker-block line but the table does not offer it there"
            ));
        }
    }

    let witnessed: BTreeSet<&str> = WORKER_WITNESSES
        .iter()
        .map(|witness| witness.word)
        .collect();
    for word in &offered {
        if !openers.contains(word) && !witnessed.contains(word) {
            wrong.push(format!(
                "`{word}` is offered in the worker block, is refused as an item opener, and has \
                 no witness — the panel would print a false \"stands in a worker block\""
            ));
        }
    }
    for word in &witnessed {
        if !offered.contains(word) || openers.contains(word) {
            wrong.push(format!(
                "the witness for `{word}` is stale: the word is not an offered non-opener worker \
                 word any more"
            ));
        }
    }

    for witness in WORKER_WITNESSES {
        let complaints = check_document(witness.source);
        if !complaints.is_empty() {
            wrong.push(format!(
                "the witness for `{}` does not pass the real checker: {}",
                witness.word,
                complaints.join("; ")
            ));
            continue;
        }
        if !word_stands_only_after_worker(witness.source, witness.word) {
            wrong.push(format!(
                "the witness for `{}` does not confine the word to the worker region",
                witness.word
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Whether every token spelling `word` in `source` sits after the `worker`
/// keyword token — the mechanical form of "the witness shows the word in
/// the worker region, not somewhere else".
fn word_stands_only_after_worker(source: &str, word: &str) -> bool {
    let Ok(tokens) = aion_awl::lex(source) else {
        return false;
    };
    let Some(worker_at) = tokens.iter().find_map(|token| {
        matches!(token.kind, TokenKind::Keyword(Keyword::Worker)).then_some(token.span.start)
    }) else {
        return false;
    };
    let mut seen = false;
    for token in &tokens {
        let spells_word = match &token.kind {
            TokenKind::Keyword(keyword) => keyword.as_word() == word,
            TokenKind::Identifier(name) => name == word,
            _ => false,
        };
        if spells_word {
            seen = true;
            if token.span.start < worker_at {
                return false;
            }
        }
    }
    seen
}

/// A value-position witness: a complete checker-passing document in which
/// the word stands in the claimed slot. The slot is verified against the
/// token stream, not the prose: the word's token must immediately follow
/// the predecessor the position names, on a line opened by the declaration
/// the position belongs to.
struct ValueWitness {
    position: GrammarPosition,
    word: &'static str,
    source: &'static str,
}

/// One witness per (value position, offered word) pair. The documents are
/// the glossary's own gated examples, verbatim — already proven through the
/// checker by `guide_tests.rs` — so the gate and the taught examples cannot
/// disagree.
const VALUE_WITNESSES: &[ValueWitness] = &[
    ValueWitness {
        position: GrammarPosition::ConstValue,
        word: "schema",
        source: "//! `schema of Type` in constant position.\nworkflow structured_reply\n  input \
                 question: String\n  outcome done: type Answer, route success\n\nconst \
                 answer_schema = schema of Answer\n\ntype Answer { reply: String }\n\nworker \
                 assistants\n  action ask(prompt: String, output_schema: String) -> Answer\n\nstep \
                 ask\n  ask(prompt: question, output_schema: answer_schema) -> result\n  route \
                 done(result)",
    },
    ValueWitness {
        position: GrammarPosition::ConstValue,
        word: "json",
        source: "//! `json { … }` opens a literal JSON body in constant position.\nworkflow \
                 request_review\n  input diff: String\n  outcome done: type String, route \
                 success\n\nconst output_schema = json { \"type\": \"string\" }\n\nworker \
                 reviewers\n  action review(prompt: String, contract: String) -> String\n\nstep \
                 review\n  review(prompt: diff, contract: output_schema) -> verdict\n  route \
                 done(verdict)",
    },
    ValueWitness {
        position: GrammarPosition::TypeDoor,
        word: "schema",
        source: "//! An inline schema door names a JSON shape as an AWL type.\nworkflow \
                 ingest_payload\n  input raw: Payload\n  outcome done: type Payload, route \
                 success\n\ntype Payload = schema {\n  \"type\": \"object\",\n  \"required\": \
                 [\"value\"],\n  \"properties\": { \"value\": { \"type\": \"string\" } }\n}\n\nstep \
                 accept\n  route done(raw)",
    },
    ValueWitness {
        position: GrammarPosition::SchemaSource,
        word: "of",
        source: "//! `schema of Type` derives a schema constant from a declared type.\nworkflow \
                 structured_reply\n  input question: String\n  outcome done: type Answer, route \
                 success\n\nconst answer_schema = schema of Answer\n\ntype Answer { reply: String \
                 }\n\nworker assistants\n  action ask(prompt: String, output_schema: String) -> \
                 Answer\n\nstep ask\n  ask(prompt: question, output_schema: answer_schema) -> \
                 result\n  route done(result)",
    },
    ValueWitness {
        position: GrammarPosition::RunCapture,
        word: "json",
        source: "//! `run json` names the capture selector: stdout parses into the type.\nworker \
                 probes\n  action host_report(host: String) -> Report\n    run json \"host-tool \
                 --json --host $host\"\n\ntype Report { cpu_count: Int, host: String }",
    },
];

/// Whether some token spelling `word` in `source` immediately follows a
/// token `precedes` accepts, on a line whose first token `opens` accepts —
/// the mechanical reading of "the word stands in the claimed slot".
fn word_stands_in_slot(source: &str, word: &str, precedes: TokenTest, opens: TokenTest) -> bool {
    let Ok(tokens) = aion_awl::lex(source) else {
        return false;
    };
    // Newlines, indentation, comments, and doc lines are structure, not
    // words; the slot reading skips them so "immediately follows" means the
    // previous WORD or sign, and "opens the line" means the line's first
    // real token.
    let structural = |kind: &TokenKind| {
        matches!(
            kind,
            TokenKind::Newline
                | TokenKind::Indent
                | TokenKind::Dedent
                | TokenKind::Comment(_)
                | TokenKind::DocHeader(_)
                | TokenKind::DocLine(_)
        )
    };
    tokens.iter().enumerate().any(|(index, token)| {
        let spells_word = match &token.kind {
            TokenKind::Keyword(keyword) => keyword.as_word() == word,
            TokenKind::Identifier(name) => name == word,
            _ => false,
        };
        if !spells_word || index == 0 {
            return false;
        }
        let previous = tokens[..index]
            .iter()
            .rev()
            .find(|candidate| !structural(&candidate.kind));
        if !previous.is_some_and(|previous| precedes(&previous.kind)) {
            return false;
        }
        tokens
            .iter()
            .find(|candidate| {
                candidate.span.line == token.span.line && !structural(&candidate.kind)
            })
            .is_some_and(|first| opens(&first.kind))
    })
}

/// A token predicate, as the slot contracts name their parts.
type TokenTest = fn(&TokenKind) -> bool;

/// The slot contract for each value position: what must precede the word,
/// and what must open its line.
fn slot_contract(position: GrammarPosition) -> Option<(TokenTest, TokenTest)> {
    match position {
        GrammarPosition::ConstValue => Some((
            |kind| matches!(kind, TokenKind::Equal),
            |kind| matches!(kind, TokenKind::Keyword(Keyword::Const)),
        )),
        GrammarPosition::TypeDoor => Some((
            |kind| matches!(kind, TokenKind::Equal),
            |kind| matches!(kind, TokenKind::Keyword(Keyword::Type)),
        )),
        GrammarPosition::SchemaSource => Some((
            |kind| matches!(kind, TokenKind::Keyword(Keyword::Schema)),
            |kind| matches!(kind, TokenKind::Keyword(Keyword::Const)),
        )),
        GrammarPosition::RunCapture => Some((
            |kind| matches!(kind, TokenKind::Identifier(name) if name == "run"),
            |kind| matches!(kind, TokenKind::Identifier(name) if name == "run"),
        )),
        _ => None,
    }
}

/// THE VALUE POSITIONS, offered direction: every word offered at
/// `const-value`, `type-door`, `schema-source`, or `run-capture` carries a
/// checker-passing witness showing it in exactly that slot, and every
/// witness is for a word still offered there. There is no dispatcher to
/// probe for the admitted direction — the module doc declares that.
#[test]
fn every_value_position_offer_is_witnessed_in_its_slot() {
    let value_positions = [
        GrammarPosition::ConstValue,
        GrammarPosition::TypeDoor,
        GrammarPosition::SchemaSource,
        GrammarPosition::RunCapture,
    ];
    let mut wrong = Vec::new();
    for position in value_positions {
        let offered: BTreeSet<&str> = words_offered_at(position).into_iter().collect();
        let witnessed: BTreeSet<&str> = VALUE_WITNESSES
            .iter()
            .filter(|witness| witness.position == position)
            .map(|witness| witness.word)
            .collect();
        for word in &offered {
            if !witnessed.contains(word) {
                wrong.push(format!(
                    "`{word}` is offered at {position:?} and has no witness — the panel would \
                     print an unproven \"{}\"",
                    position.label()
                ));
            }
        }
        for word in &witnessed {
            if !offered.contains(word) {
                wrong.push(format!(
                    "the witness for `{word}` at {position:?} is stale: the word is no longer \
                     offered there"
                ));
            }
        }
    }
    for witness in VALUE_WITNESSES {
        let complaints = check_document(witness.source);
        if !complaints.is_empty() {
            wrong.push(format!(
                "the witness for `{}` at {:?} does not pass the real checker: {}",
                witness.word,
                witness.position,
                complaints.join("; ")
            ));
            continue;
        }
        let Some((precedes, opens)) = slot_contract(witness.position) else {
            wrong.push(format!(
                "{:?} has a witness but no slot contract",
                witness.position
            ));
            continue;
        };
        if !word_stands_in_slot(witness.source, witness.word, precedes, opens) {
            wrong.push(format!(
                "the witness for `{}` at {:?} never shows the word in the claimed slot",
                witness.word, witness.position
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// THE TYPE POSITIONS OFFER NOTHING, and the parser is why: the slot after
/// a colon and a type body's field-type slot accept only a type identifier,
/// a list, or `?` (`parser/types.rs`) — every keyword of the language is
/// refused in both, probed here one by one. The offered lists must be empty
/// for as long as that holds; re-adding a word to either turns this red,
/// which is the gate that keeps the panel from ever again printing "stands
/// in a type body; after a colon, naming a type" over a word the parser
/// refuses there. (The field-NAME slot admits any identifier — but as the
/// author's own name, not as a recognised word, so it grounds no offer.)
#[test]
fn the_type_positions_offer_nothing_because_the_parser_admits_nothing() {
    assert!(
        words_offered_at(GrammarPosition::TypeReference).is_empty(),
        "the table offers words after a colon, where the parser admits no keyword"
    );
    assert!(
        words_offered_at(GrammarPosition::TypeBody).is_empty(),
        "the table offers words in a type body, where the parser admits no keyword"
    );

    // Control: a real type is admitted in both slots.
    let good_reference = format!(
        "{NARRATION}workflow probe\n  input x: String\n  outcome done: type String, route \
         success\n\nstep go\n  route done(\"ok\")\n"
    );
    assert!(
        aion_awl::parse(&good_reference).is_ok(),
        "the control failed: a plain type reference must parse"
    );

    let mut admitted = Vec::new();
    for word in vocabulary() {
        let reference_probe = format!(
            "{NARRATION}workflow probe\n  input x: {word}\n  outcome done: type String, route \
             success\n\nstep go\n  route done(\"ok\")\n"
        );
        if aion_awl::parse(&reference_probe).is_ok() {
            admitted.push(format!("`{word}` after a colon"));
        }
        let field_probe = format!(
            "{NARRATION}workflow probe\n  input x: T\n  outcome done: type String, route \
             success\n\ntype T {{ a: {word} }}\n\nstep go\n  route done(\"ok\")\n"
        );
        if aion_awl::parse(&field_probe).is_ok() {
            admitted.push(format!("`{word}` as a field type"));
        }
    }
    assert!(
        admitted.is_empty(),
        "the parser now admits vocabulary words in a type slot — decide their offer and form \
         deliberately: {admitted:?}"
    );
}
