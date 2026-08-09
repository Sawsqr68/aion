//! Tests for the AWL worker's derived surface, its execution, and the
//! refusals that stand between a wrong document and a connected worker.
//!
//! Every fixture is written to a path carrying this process's id, so
//! concurrent tests in this binary never read each other's document.

use std::error::Error;
use std::path::PathBuf;
use std::time::Duration;

use aion_awl::RunCapture;
use aion_core::ActivityId;
use aion_worker::ActivityContext;
use serde_json::{Value, json};

use super::args::{self, AwlWorkerArgs};
use super::serve;
use crate::settings::Environment;

type TestResult = Result<(), Box<dyn Error>>;

/// A worker document exercising all three captures.
const DOCUMENT: &str = r#"//! greeter: the queue this worker document serves.
worker greeter
  action greet(name: String) -> String
    run text "printf %s $name"
  action record(name: String) -> Recorded
    run json "printf '{\"name\":\"%s\"}' $name"
  action touch(path: String) -> Touched
    run "printf %s $path"

type Recorded { name: String }

type Touched { exit_code: Int, stdout: String }
"#;

/// The workflow that CALLS the queue above.
const WORKFLOW: &str = r"//! A workflow document, which this verb must refuse.
workflow greet_flow
  input name: String
  outcome greeted: type Greeting, route success

type Greeting { message: String }

worker greeter
  action greet(name: String) -> Greeting

step do_greet
  name |> greet |> route greeted
";

fn write_document(label: &str, source: &str) -> Result<PathBuf, Box<dyn Error>> {
    let path = std::env::temp_dir().join(format!(
        "aion-worker-awl-{label}-{}.awl",
        std::process::id()
    ));
    std::fs::write(&path, source)?;
    Ok(path)
}

fn arguments() -> AwlWorkerArgs {
    AwlWorkerArgs {
        document: PathBuf::from("unused.awl"),
        identity: None,
        concurrency: None,
        reconnect_initial_backoff: None,
        reconnect_max_backoff: None,
        reconnect_max_attempts: None,
        check: false,
    }
}

#[test]
fn the_surface_comes_from_the_document() -> TestResult {
    let path = write_document("surface", DOCUMENT)?;
    let compiled = serve::compile(&path)?;
    assert_eq!(compiled.task_queue, "greeter");
    let names = compiled
        .contract
        .actions
        .iter()
        .map(|action| action.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["greet", "record", "touch"]);
    let captures = compiled
        .actions
        .iter()
        .map(|action| action.capture)
        .collect::<Vec<_>>();
    assert_eq!(
        captures,
        vec![RunCapture::Text, RunCapture::Json, RunCapture::Outcome]
    );
    Ok(())
}

#[test]
fn a_workflow_document_is_refused_by_name() -> TestResult {
    let path = write_document("workflow", WORKFLOW)?;
    let Err(error) = serve::compile(&path) else {
        return Err("a workflow document cannot be served by this verb".into());
    };
    let error = error.to_string();
    assert!(error.contains("is a workflow document"), "{error}");
    assert!(error.contains("aion deploy"), "{error}");
    Ok(())
}

#[test]
fn a_document_that_does_not_check_is_refused_with_its_diagnostics() -> TestResult {
    let path = write_document(
        "unchecked",
        "//! A bodyless action is a requirement, not an implementation.\nworker greeter\n  action greet(name: String) -> String\n",
    )?;
    let Err(error) = serve::compile(&path) else {
        return Err("a document that does not check cannot be served".into());
    };
    let error = error.to_string();
    assert!(error.contains("declares no body"), "{error}");
    Ok(())
}

/// `--check` reports the descriptors an admission decision is made on, and the
/// command each action would run — the two halves an operator needs before
/// dialling.
#[test]
fn the_check_report_carries_the_descriptors_and_the_commands() -> TestResult {
    let path = write_document("report", DOCUMENT)?;
    let mut written = Vec::new();
    serve::report(&mut written, &serve::compile(&path)?)?;
    let report: Value = serde_json::from_slice(&written)?;
    assert_eq!(report["task_queue"], json!("greeter"));
    let actions = report["actions"]
        .as_array()
        .ok_or("the report lists its actions")?;
    assert_eq!(actions.len(), 3);
    assert_eq!(actions[0]["name"], json!("greet"));
    assert_eq!(actions[0]["capture"], json!("text"));
    assert_eq!(actions[0]["command"], json!("printf %s $name"));
    assert_eq!(
        actions[0]["input_schema"]["properties"]["name"]["type"],
        json!("string")
    );
    assert_eq!(actions[0]["output_schema"]["type"], json!("string"));
    assert_eq!(actions[1]["capture"], json!("json"));
    // A declared type's schema is derived as a `$ref` into `$defs`, exactly as
    // the deploy path derives the caller's — asserted in that shape rather
    // than in an inlined one, because it is the shape an operator diffs.
    assert_eq!(
        actions[1]["output_schema"]["$ref"],
        json!("#/$defs/Recorded")
    );
    assert_eq!(
        actions[1]["output_schema"]["$defs"]["Recorded"]["properties"]["name"]["type"],
        json!("string")
    );
    assert_eq!(actions[2]["capture"], json!("outcome"));
    Ok(())
}

/// The identity is DERIVED from the queue the document declares, and every
/// other connection setting must be supplied — the refusal lists them all at
/// once rather than one per correction.
#[test]
fn missing_connection_settings_are_all_named_at_once() -> TestResult {
    let environment = Environment::from_pairs([]);
    let Err(error) = args::resolve(&arguments(), "greeter", &environment) else {
        return Err("a worker with no settings must be refused".into());
    };
    let error = error.to_string();
    for flag in [
        "--concurrency",
        "--reconnect-initial-backoff",
        "--reconnect-max-backoff",
        "--reconnect-max-attempts",
    ] {
        assert!(error.contains(flag), "{flag} must be named: {error}");
    }
    Ok(())
}

#[test]
fn the_identity_is_derived_from_the_queue_and_overridable() -> TestResult {
    let environment = Environment::from_pairs([
        ("AION_WORKER_CONCURRENCY", "4"),
        ("AION_RECONNECT_INITIAL_BACKOFF_SECONDS", "0.1"),
        ("AION_RECONNECT_MAX_BACKOFF_SECONDS", "5"),
        ("AION_RECONNECT_MAX_ATTEMPTS", "100"),
    ]);
    let settings = args::resolve(&arguments(), "greeter", &environment)?;
    assert_eq!(settings.identity, "greeter-awl-worker");
    assert_eq!(settings.concurrency, 4);
    assert_eq!(settings.initial_backoff, Duration::from_millis(100));
    assert_eq!(settings.max_backoff, Duration::from_secs(5));
    assert_eq!(settings.max_attempts, 100);

    let mut named = arguments();
    named.identity = Some("greeters-of-ada".to_owned());
    let settings = args::resolve(&named, "greeter", &environment)?;
    assert_eq!(settings.identity, "greeters-of-ada");
    Ok(())
}

#[test]
fn the_built_worker_serves_exactly_the_documents_actions() -> TestResult {
    let path = write_document("built", DOCUMENT)?;
    let compiled = serve::compile(&path)?;
    let environment = Environment::from_pairs([
        ("AION_WORKER_CONCURRENCY", "4"),
        ("AION_RECONNECT_INITIAL_BACKOFF_SECONDS", "0.1"),
        ("AION_RECONNECT_MAX_BACKOFF_SECONDS", "5"),
        ("AION_RECONNECT_MAX_ATTEMPTS", "100"),
    ]);
    let settings = args::resolve(&arguments(), &compiled.task_queue, &environment)?;
    let worker = serve::build_worker(&compiled, &settings, "127.0.0.1:50051")?;
    assert_eq!(worker.activity_types(), &["greet", "record", "touch"]);
    Ok(())
}

/// The process boundary, for real: one command per capture, run through the
/// same executor the server uses for a workflow document's declared bodies.
#[tokio::test]
async fn every_capture_shapes_a_real_commands_result() -> TestResult {
    let path = write_document("execute", DOCUMENT)?;
    let compiled = serve::compile(&path)?;
    let (context, _cancellation) = ActivityContext::new(ActivityId::from_sequence_position(1), 1);

    let text = serve::served_action(&compiled.actions[0])?;
    let value = serve::execute(&text, &json!({"name": "Ada"}), &context).await?;
    assert_eq!(value, json!("Ada"));

    let structured = serve::served_action(&compiled.actions[1])?;
    let value = serve::execute(&structured, &json!({"name": "Ada"}), &context).await?;
    assert_eq!(value, json!({"name": "Ada"}));

    let outcome = serve::served_action(&compiled.actions[2])?;
    let value = serve::execute(&outcome, &json!({"path": "/tmp/x"}), &context).await?;
    assert_eq!(
        value,
        json!({"exit_code": 0, "stdout": "/tmp/x", "stderr": ""})
    );
    Ok(())
}

/// A parameter value is one whole argv element and never re-enters a parser:
/// the executor's guarantee, held here because this verb is a second caller
/// of it and the property is the reason a body is safe to declare at all.
#[tokio::test]
async fn a_parameter_value_cannot_become_a_command() -> TestResult {
    let path = write_document("injection", DOCUMENT)?;
    let compiled = serve::compile(&path)?;
    let (context, _cancellation) = ActivityContext::new(ActivityId::from_sequence_position(2), 1);
    let text = serve::served_action(&compiled.actions[0])?;
    let value = serve::execute(&text, &json!({"name": "; rm -rf /"}), &context).await?;
    assert_eq!(value, json!("; rm -rf /"));
    Ok(())
}

#[tokio::test]
async fn a_non_object_dispatch_input_is_refused_terminally() -> TestResult {
    let path = write_document("input", DOCUMENT)?;
    let compiled = serve::compile(&path)?;
    let (context, _cancellation) = ActivityContext::new(ActivityId::from_sequence_position(3), 1);
    let text = serve::served_action(&compiled.actions[0])?;
    let Err(failure) = serve::execute(&text, &Value::String("Ada".to_owned()), &context).await
    else {
        return Err("a non-object input cannot bind the action's parameters".into());
    };
    assert_eq!(
        *failure.classification(),
        aion_worker::Classification::Terminal
    );
    assert!(
        failure.message().contains("is not an object"),
        "{failure:?}"
    );
    Ok(())
}

/// A `run json` body whose command prints something that is not JSON fails
/// TERMINALLY: re-running the same command cannot make the same output parse.
#[tokio::test]
async fn invalid_json_output_is_terminal() -> TestResult {
    let path = write_document(
        "badjson",
        "//! A command that lies about printing JSON.\nworker greeter\n  action record(name: String) -> Recorded\n    run json \"printf %s $name\"\n\ntype Recorded { name: String }\n",
    )?;
    let compiled = serve::compile(&path)?;
    let (context, _cancellation) = ActivityContext::new(ActivityId::from_sequence_position(4), 1);
    let structured = serve::served_action(&compiled.actions[0])?;
    let Err(failure) = serve::execute(&structured, &json!({"name": "Ada"}), &context).await else {
        return Err("output that is not JSON cannot become a decoded result".into());
    };
    assert_eq!(
        *failure.classification(),
        aion_worker::Classification::Terminal
    );
    assert!(failure.message().contains("not valid JSON"), "{failure:?}");
    Ok(())
}
