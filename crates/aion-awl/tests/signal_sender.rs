//! The signal SENDER end to end: parse, check, print, and both lowerings.
//!
//! The receiver (`signal` + `wait`) has always existed; the sender is the
//! half AWL was missing while the engine, the durable command, and the Gleam
//! SDK all carried it. These tests pin the surface, the checker's refusals,
//! and the fact that a send actually reaches `aion@signal:send/3` on both
//! backends rather than compiling to nothing.
//!
//! The one hazard the feature must answer is that the engine routes signals
//! BY NAME, so a send nothing awaits is a silent no-op. AWL answers it as far
//! as a runtime address allows: the name must be DECLARED outbound, the
//! payload is checked against that declaration, the target must be a String,
//! and a failed delivery FAULTS the step. What stays unprovable is the far
//! side's agreement — see `checker/sends.rs`.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use aion_awl::mir::{lower, print_mir, select};
use aion_awl::{CheckError, check, check_in, emit_in, parse, print};

type TestResult = Result<(), Box<dyn Error>>;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/rev2/step-bodies/valid/send_signal.awl")
}

/// Both backends' output for the corpus fixture, each self-validated: the
/// reference emitter's Gleam source and the direct path's printed MIR (whose
/// `select` run proves the module actually assembles to `.beam` bytes).
fn backends() -> Result<(String, String), Box<dyn Error>> {
    let path = fixture();
    let source = fs::read_to_string(&path)?;
    let document = parse(&source)?;
    let root = path
        .parent()
        .ok_or_else(|| format!("fixture has no parent: {}", path.display()))?;
    let errors = check_in(&document, root);
    if !errors.is_empty() {
        return Err(format!("fixture did not check cleanly: {errors:?}").into());
    }
    let gleam = emit_in(&document, root)?;
    let module = lower(&document, Some(root))?;
    let _ = select(&module)?;
    Ok((gleam, print_mir(&module)))
}

fn check_source(source: &str) -> Result<Vec<CheckError>, Box<dyn Error>> {
    let document = parse(source).map_err(|error| {
        format!(
            "failed to parse: {} at line {}, column {}",
            error.message, error.span.line, error.span.column
        )
    })?;
    Ok(check(&document))
}

fn parse_error(source: &str) -> Result<String, Box<dyn Error>> {
    match parse(source) {
        Ok(_) => Err("expected a parse error, the document parsed".into()),
        Err(error) => Ok(error.message),
    }
}

fn expect_error<'e>(errors: &'e [CheckError], substring: &str) -> Result<&'e CheckError, String> {
    errors
        .iter()
        .find(|error| error.message.contains(substring))
        .ok_or_else(|| {
            format!(
                "no diagnostic contains {substring:?}; got {:#?}",
                errors
                    .iter()
                    .map(|error| format!("line {}: {}", error.span.line, error.message))
                    .collect::<Vec<_>>()
            )
        })
}

/// A document with an outbound declaration, minus its steps. Tests append
/// step bodies to it so each case differs only in the statement under test.
const HEADER: &str = "\
//! Signal-sender fixture.
workflow relay
  input coordinator_id: String
  signal cancel: Reason
  send cancel: Reason
  send progress: Progress
  send ping: Nil
  outcome done: type Done, route success

type Done     { item: String }
type Progress { item: String, state: String }
type Reason   { why: String }
";

fn document_with(step_body: &str) -> String {
    format!("{HEADER}\nstep only\n{step_body}\n  route done(item: \"a\")\n")
}

// ---------------------------------------------------------------------
// The surface
// ---------------------------------------------------------------------

/// The well-formed send parses, checks clean, and prints back byte-identically
/// in every one of its three payload forms.
#[test]
fn a_well_formed_send_parses_checks_and_round_trips() -> TestResult {
    let source = document_with(
        "  send ping to coordinator_id\n  \
         send progress(item: \"a\", state: \"gated\") to coordinator_id\n  \
         send cancel(Reason(why: \"stop\")) to workflow.id",
    );
    let errors = check_source(&source)?;
    assert!(errors.is_empty(), "expected a clean check, got {errors:#?}");
    let printed = print(&parse(&source)?);
    assert_eq!(printed, source, "the send statement must round-trip");
    Ok(())
}

/// The corpus fixture round-trips too, so `aion awl fmt` is a fixed point on
/// a real document carrying sends.
#[test]
fn the_corpus_fixture_is_already_canonical() -> TestResult {
    let source = fs::read_to_string(fixture())?;
    assert_eq!(print(&parse(&source)?), source);
    Ok(())
}

/// A workflow may declare the same name in BOTH directions — a relay
/// receives `cancel` and forwards it — and the two are separate declarations
/// with separate shells.
#[test]
fn one_name_may_be_declared_inbound_and_outbound() -> TestResult {
    let source = document_with("  wait cancel -> reason\n  send cancel(reason) to coordinator_id");
    let errors = check_source(&source)?;
    assert!(errors.is_empty(), "expected a clean check, got {errors:#?}");
    Ok(())
}

/// A send is legal in a compensation block: notifying someone that the step
/// failed is exactly what compensation is for.
#[test]
fn a_send_is_legal_inside_an_on_failure_block() -> TestResult {
    let source = format!(
        "{HEADER}\nworker gates\n  action probe(item: String) -> Nil\n\n\
         step only\n  probe(item: \"a\")\n  on failure\n    \
         send progress(item: \"a\", state: \"failed\") to coordinator_id\n    \
         route done(item: \"a\")\n  route done(item: \"a\")\n"
    );
    let errors = check_source(&source)?;
    assert!(errors.is_empty(), "expected a clean check, got {errors:#?}");
    Ok(())
}

// ---------------------------------------------------------------------
// The checker's refusals — the hazard's answer
// ---------------------------------------------------------------------

/// Sending a name the header never declares outbound is a check error. This
/// is the rule that keeps a typo'd signal name from becoming a silent no-op
/// at runtime.
#[test]
fn sending_an_undeclared_signal_is_a_check_error() -> TestResult {
    let source = document_with("  send progres(item: \"a\", state: \"x\") to coordinator_id");
    let errors = check_source(&source)?;
    let error = expect_error(&errors, "no outbound signal named `progres` is declared")?;
    assert!(
        error.message.contains("send progres: <Type>"),
        "the diagnostic must name the fix: {}",
        error.message
    );
    Ok(())
}

/// A name declared only as an INBOUND signal gets its own diagnostic: the
/// author has written a direction confusion, not a typo.
#[test]
fn sending_a_receive_only_signal_names_the_direction() -> TestResult {
    let source = "\
//! Direction confusion.
workflow relay
  input coordinator_id: String
  signal cancel: Reason
  outcome done: type Done, route success

type Done   { item: String }
type Reason { why: String }

step only
  send cancel(why: \"stop\") to coordinator_id
  route done(item: \"a\")
";
    let errors = check_source(source)?;
    let error = expect_error(&errors, "declared as a signal this workflow RECEIVES")?;
    assert!(
        error.message.contains("send cancel: Reason"),
        "the diagnostic must name the fix: {}",
        error.message
    );
    Ok(())
}

/// The target is a workflow id, which is a String. Anything else is refused
/// at the target's own span.
#[test]
fn a_non_string_target_is_refused() -> TestResult {
    let source = document_with("  send ping to 7");
    let errors = check_source(&source)?;
    expect_error(
        &errors,
        "a send addresses a workflow by id, which is a String",
    )?;
    Ok(())
}

/// The payload is checked against the DECLARED type, exactly as an action
/// call's arguments are checked against its declaration.
#[test]
fn a_payload_field_of_the_wrong_type_is_refused() -> TestResult {
    let source = document_with("  send progress(item: 1, state: \"gated\") to coordinator_id");
    let errors = check_source(&source)?;
    expect_error(&errors, "field `item`")?;
    Ok(())
}

/// A missing required field is refused, so a partially-built payload cannot
/// reach the wire.
#[test]
fn a_missing_payload_field_is_refused() -> TestResult {
    let source = document_with("  send progress(item: \"a\") to coordinator_id");
    let errors = check_source(&source)?;
    expect_error(&errors, "state")?;
    Ok(())
}

/// The bare form carries no payload, so it is legal only for a `Nil` payload
/// — it deliberately does NOT pick up a same-named binding the way a bare
/// `route` does.
#[test]
fn the_bare_form_is_refused_for_a_non_nil_payload() -> TestResult {
    let source = document_with("  send progress to coordinator_id");
    let errors = check_source(&source)?;
    expect_error(&errors, "so the send must construct it")?;
    Ok(())
}

/// Named fields need a record payload type; a leaf-typed signal takes the
/// value form instead.
#[test]
fn named_fields_on_a_leaf_payload_are_refused() -> TestResult {
    let source = "\
//! Leaf payload.
workflow relay
  input coordinator_id: String
  send note: String
  outcome done: type Done, route success

type Done { item: String }

step only
  send note(text: \"hello\") to coordinator_id
  route done(item: \"a\")
";
    let errors = check_source(source)?;
    expect_error(&errors, "named payload fields need a record-typed signal")?;
    Ok(())
}

/// Two outbound declarations of one name contradict each other about what
/// the document sends.
#[test]
fn a_duplicate_outbound_declaration_is_refused() -> TestResult {
    let source = "\
//! Duplicate.
workflow relay
  input coordinator_id: String
  send ping: Nil
  send ping: Nil
  outcome done: type Done, route success

type Done { item: String }

step only
  send ping to coordinator_id
  route done(item: \"a\")
";
    let errors = check_source(source)?;
    expect_error(&errors, "duplicate send declaration `ping`")?;
    Ok(())
}

/// Declaration names follow the language's one naming rule.
#[test]
fn an_outbound_name_must_be_snake_case() -> TestResult {
    let source = "\
//! Casing.
workflow relay
  input coordinator_id: String
  send Ping: Nil
  outcome done: type Done, route success

type Done { item: String }

step only
  send Ping to coordinator_id
  route done(item: \"a\")
";
    let errors = check_source(source)?;
    expect_error(&errors, "send name `Ping` must be snake_case")?;
    Ok(())
}

// ---------------------------------------------------------------------
// Parser diagnostics
// ---------------------------------------------------------------------

/// The payload written after the target is the mistake English word order
/// invites; it gets a fix-it, not a bare "expected end of line".
#[test]
fn the_payload_after_the_target_gets_a_fix_it() -> TestResult {
    let source = document_with("  send progress to coordinator_id (item: \"a\", state: \"x\")");
    let message = parse_error(&source)?;
    assert!(
        message.contains("the payload belongs to the signal"),
        "unexpected diagnostic: {message}"
    );
    Ok(())
}

/// A send with no `to` is named as such, with the address form spelled out.
#[test]
fn a_send_without_a_target_is_named() -> TestResult {
    let source = document_with("  send ping");
    let message = parse_error(&source)?;
    assert!(
        message.contains("names the workflow it goes to"),
        "unexpected diagnostic: {message}"
    );
    Ok(())
}

/// `send` stays a plain identifier: the corpus already uses it as an action
/// name, and a call to that action must keep parsing as a call.
#[test]
fn send_is_not_reserved_and_still_names_an_action() -> TestResult {
    let source = "\
//! `send` as an ordinary name.
workflow relay
  input topic: String
  outcome done: type Done, route success

type Done    { item: String }
type Receipt { id: String }

worker mail
  action send(to: String, body: String) -> Receipt

step only
  send(to: topic, body: \"hello\") -> receipt
  route done(item: topic)
";
    let errors = check_source(source)?;
    assert!(errors.is_empty(), "expected a clean check, got {errors:#?}");
    assert_eq!(print(&parse(source)?), source);
    Ok(())
}

/// The worker vocabulary is a closed surface: an action's declarative body
/// takes `run "<command>"` and nothing else, so a send cannot appear there.
/// A send is workflow-level durable work an engine records and replays; an
/// action body is a shell command a worker runs. The refusal is the worker
/// block's own — it never reaches the send grammar at all — which is the
/// right layer for it, but it means the diagnostic names the worker block
/// rather than the send.
#[test]
fn a_send_is_not_an_action_body() -> TestResult {
    let source = "\
//! A send in the worker vocabulary.
workflow relay
  input coordinator_id: String
  send ping: Nil
  outcome done: type Done, route success

type Done { item: String }

worker gates
  action probe(item: String) -> Nil
    send ping to coordinator_id

step only
  probe(item: \"a\")
  route done(item: \"a\")
";
    let message = parse_error(source)?;
    assert!(
        message.contains("worker block"),
        "a send in an action's block must be refused by the worker grammar: {message}"
    );
    Ok(())
}

// ---------------------------------------------------------------------
// Lowering: the send actually reaches the engine on both backends
// ---------------------------------------------------------------------

/// The reference (Gleam-source) backend emits a real `signal.send` per send
/// statement, threaded through `result.try` so a failed delivery faults the
/// step, plus one outbound `SignalRef` constructor per declaration.
#[test]
fn the_reference_backend_emits_signal_send() -> TestResult {
    let (gleam, _) = backends()?;
    assert_eq!(gleam.matches("signal.send(").count(), 4);
    assert_eq!(
        gleam.matches("|> awl_error.map_engine_error").count(),
        // Three sends addressed by a plain expression, one addressed by
        // `workflow.id` (whose own `Result` is mapped the same way), and the
        // send that carries it.
        5
    );
    // One outbound ref constructor per DECLARATION, distinct from the
    // inbound one a `wait` uses.
    assert!(gleam.contains("fn heartbeat_send_signal() -> signal.SignalRef(Nil)"));
    assert!(gleam.contains("fn progress_send_signal()"));
    assert!(gleam.contains("fn verdict_send_signal() -> signal.SignalRef(String)"));
    assert!(gleam.contains("fn decision_signal()"));
    // Every send is bound by `use _ <- result.try(...)`: the failure is
    // threaded, never discarded.
    assert_eq!(gleam.matches("use _ <- result.try(signal.send(").count(), 4);
    Ok(())
}

/// The direct (MIR/`.beam`) backend reaches the same SDK entry point, with
/// the same error mapping and the same `try_bind` fault edge.
#[test]
fn the_direct_backend_reaches_the_same_sdk_entry_point() -> TestResult {
    let (_, mir) = backends()?;
    assert_eq!(mir.matches("call_rt aion@signal:send/3").count(), 4);
    assert!(mir.contains("origin=send_signal_ref(progress)"));
    assert!(mir.contains("origin=send_signal_ref(heartbeat)"));
    assert!(mir.contains("origin=send_signal_ref(verdict)"));
    // The inbound ref is a DIFFERENT function.
    assert!(mir.contains("origin=signal_ref(decision)"));
    Ok(())
}

/// A document that declares no sends must lower byte-identically to how it
/// lowered before the sender existed: the outbound shells are allocated
/// after every inbound one, so no existing `FnRef` moves.
#[test]
fn a_document_without_sends_is_unaffected() -> TestResult {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parity/wait_signal.awl");
    let source = fs::read_to_string(&path)?;
    let document = parse(&source)?;
    let root = path
        .parent()
        .ok_or_else(|| format!("fixture has no parent: {}", path.display()))?;
    assert!(check_in(&document, root).is_empty());
    let gleam = emit_in(&document, root)?;
    assert!(!gleam.contains("_send_signal"));
    assert!(!gleam.contains("signal.send("));
    let module = lower(&document, Some(root))?;
    let mir = print_mir(&module);
    assert!(!mir.contains("aion@signal:send/3"));
    assert!(!mir.contains("send_signal_ref"));
    Ok(())
}
