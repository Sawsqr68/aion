//! The `worker` document family: what it parses to, what it refuses, and the
//! one property that makes it safe to split an action's declaration across
//! two files — the typed surface a worker document derives is the SAME
//! derivation the workflow document that calls it derives.

use std::error::Error;
use std::path::{Path, PathBuf};

use aion_awl::{
    ActionBody, CompileError, DocumentFamily, RunCapture, SchemaError, check_in,
    compile_worker_document, parse, print, schema_for_workflow,
};

type TestResult = Result<(), Box<dyn Error>>;

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rev2/worker-documents")
}

const GREETER: &str = "\
//! Serve the `greeter` queue.
worker greeter
  action greet(name: String) -> Greeting
    run json \"greeter-tool --json --name $name\"
  action shout(text: String) -> String
    run text \"tr a-z A-Z\"
  action archive(path: String) -> Archived
    run \"tar -cf archive.tar $path\"

type Greeting { message: String }

type Archived { exit_code: Int, stderr: String }
";

/// The workflow that CALLS the queue `GREETER` implements. Its `worker`
/// block is the requirement; the worker document is the implementation.
const CALLER: &str = "\
//! Greet a name through the `greeter` queue.
workflow greet_flow
  input name: String
  outcome greeted: type Greeting, route success

type Greeting { message: String }

worker greeter
  action greet(name: String) -> Greeting

step do_greet
  name |> greet |> route greeted
";

#[test]
fn the_opening_declaration_chooses_the_family() -> TestResult {
    assert_eq!(parse(GREETER)?.family, DocumentFamily::Worker);
    assert_eq!(parse(CALLER)?.family, DocumentFamily::Workflow);
    Ok(())
}

#[test]
fn a_worker_document_checks_clean_and_prints_canonically() -> TestResult {
    let document = parse(GREETER)?;
    let errors = check_in(&document, &corpus_root());
    assert!(errors.is_empty(), "{errors:#?}");
    assert_eq!(print(&document), GREETER);
    Ok(())
}

#[test]
fn every_action_body_carries_its_capture() -> TestResult {
    let document = parse(GREETER)?;
    let worker = document.workers.first().ok_or("the header worker block")?;
    let captures = worker
        .actions
        .iter()
        .map(|action| match &action.body {
            Some(ActionBody::Run { capture, .. }) => Ok(*capture),
            None => Err(format!("action `{}` parsed with no body", action.name)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        captures,
        vec![RunCapture::Json, RunCapture::Text, RunCapture::Outcome]
    );
    Ok(())
}

/// The anti-drift property: the action surface a worker document advertises
/// and the one the calling workflow's package commits to are two runs of ONE
/// derivation. If they were derived twice, a worker could advertise a shape
/// the queue's contract never promised and admission would compare two
/// independent opinions instead of one contract.
#[test]
fn the_advertised_surface_matches_the_callers_contract() -> TestResult {
    let served = compile_worker_document(GREETER, &corpus_root())?;
    let called = aion_awl::compile(CALLER, &corpus_root())?;
    let caller_queue = called
        .contract
        .workers
        .iter()
        .find(|worker| worker.task_queue == "greeter")
        .ok_or("the caller declares the `greeter` queue")?;
    let caller_greet = caller_queue
        .actions
        .iter()
        .find(|action| action.name == "greet")
        .ok_or("the caller declares action `greet`")?;
    let served_greet = served
        .contract
        .actions
        .iter()
        .find(|action| action.name == "greet")
        .ok_or("the worker document serves action `greet`")?;
    assert_eq!(served.task_queue, "greeter");
    assert_eq!(served_greet.input_schema, caller_greet.input_schema);
    assert_eq!(served_greet.output_schema, caller_greet.output_schema);
    Ok(())
}

/// A worker document's contract states the typed surface only. The body is
/// carried separately, because `ActionContract::worker_owed` reads a body as
/// "the deployed package runs this itself, no worker needed" — the opposite
/// of what a worker document's body means.
#[test]
fn the_advertised_contract_stays_worker_owed() -> TestResult {
    let served = compile_worker_document(GREETER, &corpus_root())?;
    for action in &served.contract.actions {
        assert!(
            action.worker_owed(),
            "action `{}` must remain owed to this worker",
            action.name
        );
    }
    let commands = served
        .actions
        .iter()
        .map(|action| {
            (
                action.name.as_str(),
                action.capture,
                action.parameters.clone(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        commands,
        vec![
            ("greet", RunCapture::Json, vec!["name".to_owned()]),
            ("shout", RunCapture::Text, Vec::new()),
            ("archive", RunCapture::Outcome, vec!["path".to_owned()]),
        ]
    );
    Ok(())
}

#[test]
fn the_workflow_compiler_refuses_a_worker_document_by_name() -> TestResult {
    let Err(CompileError::Family { message, span }) = aion_awl::compile(GREETER, &corpus_root())
    else {
        return Err("the workflow compiler must refuse a worker document".into());
    };
    assert!(message.contains("is a worker document"), "{message}");
    assert!(message.contains("aion worker awl"), "{message}");
    assert_eq!(span.line, 2);
    Ok(())
}

#[test]
fn the_worker_compiler_refuses_a_workflow_document_by_name() -> TestResult {
    let Err(CompileError::Family { message, span }) =
        compile_worker_document(CALLER, &corpus_root())
    else {
        return Err("the worker compiler must refuse a workflow document".into());
    };
    assert!(message.contains("is a workflow document"), "{message}");
    assert!(message.contains("aion deploy"), "{message}");
    assert_eq!(span.line, 2);
    Ok(())
}

#[test]
fn a_worker_document_publishes_no_start_contract() -> TestResult {
    let document = parse(GREETER)?;
    let Err(SchemaError::NotAWorkflow { name, contract, .. }) = schema_for_workflow(&document)
    else {
        return Err("a worker document has no start contract to derive".into());
    };
    assert_eq!(name, "greeter");
    assert_eq!(contract, "start contract");
    Ok(())
}

/// A body may reference only the action's own parameters, and never in the
/// program position — the rules the workflow family already enforces, held
/// over the new family by the SAME checker pass rather than a second one.
#[test]
fn command_hygiene_holds_in_the_worker_family() -> TestResult {
    let source = "\
//! Hygiene.
worker greeter
  action greet(name: String) -> String
    run text \"$name --loud\"
  action shout(text: String) -> String
    run text \"tr $missing\"
";
    let document = parse(source)?;
    let errors = check_in(&document, &corpus_root());
    let messages = errors
        .iter()
        .map(|error| error.message.clone())
        .collect::<Vec<_>>();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("must be written out, not the parameter `name`")),
        "{messages:#?}"
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("which action `shout` does not declare")),
        "{messages:#?}"
    );
    Ok(())
}

/// `fmt` normalizes a worker document, and the normalized form re-parses to
/// the same document — the printer property the workflow family already
/// holds, extended to the file header that tells the families apart.
#[test]
fn printing_normalizes_and_reparses() -> TestResult {
    let source = "\
//! Non-canonical spacing.
worker greeter
  action greet(name: String,) -> String
    run    text     \"printf %s $name\"
";
    let printed = print(&parse(source)?);
    assert_eq!(
        printed,
        "\
//! Non-canonical spacing.
worker greeter
  action greet(name: String) -> String
    run text \"printf %s $name\"
"
    );
    assert_eq!(print(&parse(&printed)?), printed);
    Ok(())
}
