//! The pin behind [`HARNESS_FIELDS`]' "cannot drift" claim.
//!
//! The printer emits one `if let` per setting rather than walking the array — the twelve
//! values have twelve different types, and a table of extractors would trade a readable
//! sequence for a pile of closures. So the correspondence between the array and the
//! printer is not enforced by construction, and the comment used to claim it was. It is
//! enforced HERE instead, which is a real guarantee rather than a hopeful one:
//!
//! * every field word in [`HARNESS_FIELDS`] is printed, so a setting added to the
//!   grammar without a print position fails here instead of vanishing on `aion awl fmt`;
//! * the printed ORDER is exactly the array's, so the canonical shape and the refusal
//!   vocabulary read one table;
//! * nothing outside the array is printed, so a stray line cannot appear in canonical
//!   output.
//!
//! [`HARNESS_FIELDS`]: crate::ast::HARNESS_FIELDS

use crate::ast::HARNESS_FIELDS;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A document whose one `harness` section carries EVERY field, in a scrambled order.
///
/// Kind-scoping (`command` is ACP's, `binary` is Norn's) is a CHECKER rule, not a parser
/// one, so a document carrying all twelve parses cleanly and is exactly what this pin
/// needs — a section that only ever printed one kind's fields could not prove the other
/// kind's are printed at all. It is deliberately not checked: this is a test about the
/// printer, and running the checker would refuse the document for a reason that has
/// nothing to do with printing.
const EVERY_FIELD_SCRAMBLED: &str = "\
//! Every harness field, written in an order that is not the canonical one.
workflow every_harness_field
  input question: String
  outcome answered: type Answer, route success

type Answer { reply: String }

type Reply { text: String, stop_reason: String, session_id: String }

worker assistants
  harness
    binary \"/usr/local/bin/norn\"
    exit_grace 10s
    permission \"deny\"
    cwd \"/srv/agents\"
    args \"--acp\"
    command \"/usr/local/bin/claude-code-acp\"
    env_pass \"PATH\", \"HOME\"
    reconnect_max_attempts 100
    reconnect_max_backoff 2m
    reconnect_initial_backoff 5s
    concurrency 4
    kind acp
  action ask(prompt: String) -> Reply
    agent

step ask
  ask(prompt: question) -> reply
  route answered(reply: reply.text)
";

/// The field word each printed setting line opens with, in printed order.
fn printed_fields(source: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let document = crate::parse(source).map_err(|error| {
        format!(
            "{}:{}: {}",
            error.span.line, error.span.column, error.message
        )
    })?;
    let printed = crate::print(&document);
    Ok(printed
        .lines()
        .skip_while(|line| *line != "  harness")
        .skip(1)
        .take_while(|line| line.starts_with("    "))
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect())
}

#[test]
fn the_printer_emits_every_field_in_exactly_the_canonical_order() -> TestResult {
    let printed = printed_fields(EVERY_FIELD_SCRAMBLED)?;
    assert_eq!(
        printed,
        HARNESS_FIELDS.map(str::to_owned).to_vec(),
        "the printed shape and `HARNESS_FIELDS` are one table: every field is printed, \
         in the array's order, and nothing else is. A setting added to the grammar with \
         no print position would vanish on `aion awl fmt` — silently, from a deployed \
         document — and this is what stops that."
    );
    Ok(())
}

#[test]
fn the_scrambled_fixture_really_is_scrambled() {
    // The non-vacuity guard: if the fixture happened to be written in canonical order,
    // the test above would pass without the printer reordering anything at all.
    let authored: Vec<String> = EVERY_FIELD_SCRAMBLED
        .lines()
        .skip_while(|line| *line != "  harness")
        .skip(1)
        .take_while(|line| line.starts_with("    "))
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect();
    assert_eq!(
        authored.len(),
        HARNESS_FIELDS.len(),
        "the fixture must carry every field, or the pin only covers the ones it has"
    );
    assert_ne!(
        authored,
        HARNESS_FIELDS.map(str::to_owned).to_vec(),
        "the fixture must NOT already be in canonical order, or the printer is proved to \
         reorder nothing"
    );
}
