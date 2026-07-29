//! Declarative action bodies end to end: what parses, what the canonical
//! printer does with it, what the checker refuses, and what survives into
//! the compiled artifact.
//!
//! The corpus fixtures under `fixtures/rev2/declarations` carry the golden
//! accept/reject cases; these pin the behaviour the corpus cannot express —
//! AST shape, trivia, canonical re-ordering, and the compile seam.

use std::error::Error;
use std::path::Path;

use aion_awl::{ActionBody, CompiledActionBody, check, compile, parse, print};

type TestResult = Result<(), Box<dyn Error>>;

/// One action's name and the command it declares, if any.
type DeclaredAction = (String, Option<String>);

/// A document with one bodied action and one bodyless action.
const MIXED: &str = "\
//! One action the document implements, one it only requires.
workflow mixed_bodies
  input name: String
  outcome done: type Out, route success

type Out        { text: String }
type RunOutcome { exit_code: Int, stdout: String, stderr: String }

worker hands
  action greet(name: String) -> RunOutcome
    run \"echo hello $name\"
  action polish(text: String) -> Out

step greet
  greet(name: name) -> greeted

step polish after greet
  polish(text: greeted.stdout) -> polished
  route done(text: polished.text)
";

fn actions(source: &str) -> Result<Vec<DeclaredAction>, Box<dyn Error>> {
    let document = parse(source)?;
    let worker = document
        .workers
        .first()
        .ok_or("the document declares a worker")?;
    Ok(worker
        .actions
        .iter()
        .map(|action| {
            let command = action.body.as_ref().map(|body| match body {
                ActionBody::Run { command, .. } => command.clone(),
            });
            (action.name.clone(), command)
        })
        .collect())
}

#[test]
fn a_declared_body_parses_onto_its_action_and_a_bodyless_action_keeps_none() -> TestResult {
    assert_eq!(
        actions(MIXED)?,
        vec![
            ("greet".to_owned(), Some("echo hello $name".to_owned())),
            ("polish".to_owned(), None),
        ]
    );
    Ok(())
}

#[test]
fn a_bodyless_document_still_checks_clean() -> TestResult {
    // R1: an action with no body is a permanent, first-class case — the
    // out-of-band worker build. It must never acquire a diagnostic.
    let source = MIXED.replace("    run \"echo hello $name\"\n", "");
    let document = parse(&source)?;
    let errors = check(&document);
    assert!(errors.is_empty(), "{errors:#?}");
    assert_eq!(
        actions(&source)?,
        vec![("greet".to_owned(), None), ("polish".to_owned(), None)]
    );
    Ok(())
}

#[test]
fn a_body_round_trips_and_prints_idempotently() -> TestResult {
    let printed = print(&parse(MIXED)?);
    assert_eq!(printed, MIXED, "the canonical form must be the source");
    assert_eq!(printed, print(&parse(&printed)?), "printing is idempotent");
    Ok(())
}

#[test]
fn escapes_inside_a_command_survive_the_round_trip() -> TestResult {
    // The lexer resolves `\t`, `\n`, `\"` and `\\` inside the literal; the
    // printer must put every one of them back, or the deployed command
    // would drift from the authored one.
    let source = MIXED.replace(
        r#"run "echo hello $name""#,
        r#"run "printf 'a\tb\n\\\"' $name""#,
    );
    assert_ne!(source, MIXED, "the substitution must have happened");
    let document = parse(&source)?;
    let errors = check(&document);
    assert!(errors.is_empty(), "{errors:#?}");
    assert_eq!(print(&document), source, "escaped command must round-trip");
    Ok(())
}

#[test]
fn a_body_written_before_the_config_line_normalizes_to_canonical_order() -> TestResult {
    let source = MIXED.replace(
        "    run \"echo hello $name\"\n",
        "    run \"echo hello $name\"\n    node shell, timeout 30s\n",
    );
    assert_ne!(source, MIXED, "the substitution must have happened");
    let printed = print(&parse(&source)?);
    assert!(
        printed.contains("    node shell, timeout 30s\n    run \"echo hello $name\"\n"),
        "config must print before the body:\n{printed}"
    );
    assert_eq!(printed, print(&parse(&printed)?), "printing is idempotent");
    Ok(())
}

#[test]
fn comments_around_a_body_survive_the_round_trip() -> TestResult {
    let source = MIXED.replace(
        "    run \"echo hello $name\"\n",
        "    // the implementation itself\n    run \"echo hello $name\" // greets\n",
    );
    assert_ne!(source, MIXED, "the substitution must have happened");
    let printed = print(&parse(&source)?);
    assert_eq!(printed, source, "trivia around a body must be lossless");
    Ok(())
}

#[test]
fn two_bodies_on_one_action_are_a_parse_error() {
    let source = MIXED.replace(
        "    run \"echo hello $name\"\n",
        "    run \"echo hello $name\"\n    run \"echo again $name\"\n",
    );
    let Err(error) = parse(&source) else {
        unreachable!("two `run` bodies must not parse");
    };
    assert!(
        error.message.contains("at most one `run` body"),
        "{}",
        error.message
    );
}

#[test]
fn two_config_lines_on_one_action_are_a_parse_error() {
    // The action block gained a loop; the pre-existing one-config-line rule
    // must still hold inside it.
    let source = MIXED.replace(
        "    run \"echo hello $name\"\n",
        "    node shell\n    timeout 30s\n",
    );
    let Err(error) = parse(&source) else {
        unreachable!("two config lines must not parse");
    };
    assert!(
        error.message.contains("at most one config line"),
        "{}",
        error.message
    );
}

#[test]
fn run_is_still_usable_as_an_ordinary_name() -> TestResult {
    // `run` is NOT a reserved keyword — it is recognised positionally, in
    // the one place inside an action's block where nothing else may stand.
    // The corpus pins `step run`; this pins the harder cases a reserved
    // word would have broken: an input named `run`, a parameter named
    // `run`, and an EXPRESSION referencing that binding. Reserving the word
    // would have made the last one a parse error.
    let source = "\
//! `run` as an ordinary name, in every position that word can hold.
workflow run_as_a_name
  input run: String
  outcome done: type Out, route success

type Out { stdout: String }

worker hands
  action greet(run: String) -> Out
    run \"echo hello $run\"

step run
  greet(run: run) -> greeted
  route done(stdout: greeted.stdout)
";
    let document = parse(source)?;
    let errors = check(&document);
    assert!(errors.is_empty(), "{errors:#?}");
    assert_eq!(print(&document), source, "must round-trip byte-identically");
    Ok(())
}

#[test]
fn the_compiled_workflow_carries_the_command_and_its_binding() -> TestResult {
    let compiled = compile(MIXED, Path::new("."))?;
    assert_eq!(
        compiled.action_bodies.len(),
        1,
        "only the bodied action appears"
    );
    let implementation = compiled
        .action_bodies
        .first()
        .ok_or("the bodied action must be carried")?;
    assert_eq!(implementation.task_queue, "hands");
    assert_eq!(implementation.action, "greet");
    let CompiledActionBody::Run {
        command,
        parameters,
    } = &implementation.body;
    assert_eq!(command, "echo hello $name");
    assert_eq!(parameters, &["name".to_owned()]);
    Ok(())
}

#[test]
fn a_document_with_no_bodies_compiles_to_no_implementations() -> TestResult {
    let source = MIXED.replace("    run \"echo hello $name\"\n", "");
    let compiled = compile(&source, Path::new("."))?;
    assert!(
        compiled.action_bodies.is_empty(),
        "{:#?}",
        compiled.action_bodies
    );
    Ok(())
}
