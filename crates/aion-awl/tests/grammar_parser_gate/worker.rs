use super::*;

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
        word: "fallback",
        source: "//! Witness: an authored `fallback` chain inside a worker block.\nworkflow \
                 witness\n  input name: String\n  outcome done: type String, route \
                 success\n\nworker crew\n  action ping(name: String) -> String\n    fallback \
                 norn, claude\n\nstep go\n  ping(name: name) -> answer\n  route done(answer)\n",
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
                 success\n\ntype Reply { text: String, final_message: String, stop_reason: String, session_id: String }\n\nworker crew\n  \
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
