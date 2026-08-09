//! Compiling the worker document, advertising its surface, and serving it.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use aion_awl::{CompiledWorkerAction, CompiledWorkerDocument, RunCapture};
use aion_package::ActivityDescriptor;
use aion_worker::shell::ShellAction;
use aion_worker::{ActivityContext, ActivityFailure, Worker, WorkerConfig};
use anyhow::{Context, Result};
use serde_json::Value;

use super::args::{self, AwlWorkerArgs, ConnectionSettings};
use crate::settings::Environment;

/// One action, ready to serve: its parsed command and what its result is.
pub(super) struct ServedAction {
    /// Action name — the activity type dispatched on the queue.
    name: String,
    /// The parsed command. Parsed ONCE, at startup: a command the executor
    /// cannot parse is a refusal at the desk rather than a terminal failure
    /// on the first dispatch that reaches it.
    command: ShellAction,
    /// What a successful command's result is.
    capture: RunCapture,
}

/// Serve the worker document `args.document` names on the queue it declares.
///
/// # Errors
///
/// Returns an error when the document cannot be read, is a workflow document,
/// does not check cleanly, when a connection setting has no value, or when the
/// worker cannot be built or served.
pub(crate) async fn run(args: &AwlWorkerArgs, endpoint: &str) -> Result<()> {
    let compiled = compile(&args.document)?;
    if args.check {
        print!("{}", report(&compiled));
        return Ok(());
    }
    let environment = Environment::from_process();
    let settings = args::resolve(args, &compiled.task_queue, &environment)?;
    build_worker(&compiled, &settings, endpoint)?.run().await?;
    Ok(())
}

/// Read and compile the document, naming it in every refusal.
pub(super) fn compile(document: &Path) -> Result<CompiledWorkerDocument> {
    let source = std::fs::read_to_string(document)
        .with_context(|| format!("failed to read AWL document {}", document.display()))?;
    aion_awl::compile_worker_document(&source, crate::awl::document_root(document)).map_err(
        |error| {
            anyhow::anyhow!(
                "{} cannot be served:\n{}",
                document.display(),
                crate::awl::compile_diagnostics(document, &error).join("\n")
            )
        },
    )
}

/// The typed descriptor for every action, keyed by action name.
fn descriptors(compiled: &CompiledWorkerDocument) -> BTreeMap<String, ActivityDescriptor> {
    compiled
        .contract
        .actions
        .iter()
        .map(|action| {
            (
                action.name.clone(),
                ActivityDescriptor {
                    name: action.name.clone(),
                    input_schema: action.input_schema.clone(),
                    output_schema: action.output_schema.clone(),
                },
            )
        })
        .collect()
}

/// What `--check` prints: the queue, and one line per action naming what it
/// runs and what its result is.
///
/// This is the whole admission decision plus the whole execution decision, on
/// one screen, before anything connects or runs.
pub(super) fn report(compiled: &CompiledWorkerDocument) -> String {
    let mut out = format!(
        "task queue: {}\nactions: {}\n",
        compiled.task_queue,
        compiled.actions.len()
    );
    for action in &compiled.actions {
        out.push_str(&format!(
            "  {} [{}] {}\n",
            action.name,
            capture_word(action.capture),
            action.command
        ));
    }
    out
}

/// How a capture reads in the `--check` report.
const fn capture_word(capture: RunCapture) -> &'static str {
    match capture {
        RunCapture::Outcome => "outcome record",
        RunCapture::Text => "stdout as text",
        RunCapture::Json => "stdout as json",
    }
}

/// Build the worker, registering every action the document declares.
///
/// Serving is all-or-nothing by construction here: the document IS the served
/// set, so there is no way to advertise part of a queue — the failure the
/// manifest worker has to check for cannot be expressed.
pub(super) fn build_worker(
    compiled: &CompiledWorkerDocument,
    settings: &ConnectionSettings,
    endpoint: &str,
) -> Result<Worker> {
    let mut descriptors = descriptors(compiled);
    let config = WorkerConfig::builder()
        .endpoint(crate::normalize_endpoint(endpoint))
        .task_queue(&compiled.task_queue)
        .identity(&settings.identity)
        .max_concurrency(settings.concurrency)
        .reconnect_initial_backoff(settings.initial_backoff)
        .reconnect_max_backoff(settings.max_backoff)
        .reconnect_max_attempts(settings.max_attempts)
        .build()?;
    let mut builder = Worker::builder(config);
    for action in &compiled.actions {
        let name = action.name.clone();
        let descriptor = descriptors.remove(&name).with_context(|| {
            format!("no derived descriptor for action `{name}`; the action surface is incomplete")
        })?;
        let served = Arc::new(served_action(action)?);
        builder = builder.register_activity_with_descriptor(
            name,
            descriptor,
            move |input: Value, context| {
                let served = Arc::clone(&served);
                Box::pin(async move { execute(&served, &input, context).await })
            },
        )?;
    }
    builder.build().map_err(Into::into)
}

/// Parse one action's command at startup.
pub(super) fn served_action(action: &CompiledWorkerAction) -> Result<ServedAction> {
    let command = ShellAction::new(&action.command).with_context(|| {
        // The checker rules on command hygiene, so reaching this means the
        // document and the executor disagree — named as the defect it is
        // rather than surfacing later as a dispatch failure.
        format!(
            "action `{}` declares a command the executor cannot parse",
            action.name
        )
    })?;
    Ok(ServedAction {
        name: action.name.clone(),
        command,
        capture: action.capture,
    })
}

/// Run one dispatch and shape its result per the action's capture.
///
/// Failure classification is the executor's for every capture: a non-zero
/// exit is retryable and carries the command's own stderr, a spawn failure or
/// a cancellation is terminal. The capture decides only what a SUCCESSFUL
/// command's result is, so the three forms cannot drift into three failure
/// vocabularies.
pub(super) async fn execute(
    served: &ServedAction,
    input: &Value,
    context: &ActivityContext,
) -> Result<Value, ActivityFailure> {
    let arguments = decode_arguments(&served.name, input)?;
    let outcome = served.command.run(&arguments, context).await?;
    match served.capture {
        RunCapture::Outcome => serde_json::to_value(&outcome).map_err(|error| {
            ActivityFailure::terminal(format!(
                "action `{}` could not encode its command outcome: {error}",
                served.name
            ))
        }),
        RunCapture::Text => Ok(Value::String(outcome.stdout)),
        RunCapture::Json => serde_json::from_str(&outcome.stdout).map_err(|error| {
            ActivityFailure::terminal(format!(
                "action `{}` declares a `run json` body and its command printed output that \
                 is not valid JSON: {error}",
                served.name
            ))
        }),
    }
}

/// Decode the dispatch input into the action's named arguments.
///
/// A declared action's parameters are named in the document, so the input
/// must be a JSON object; anything else cannot bind to `$name` references.
/// Retrying cannot change the input, so the refusal is terminal.
pub(super) fn decode_arguments(
    action: &str,
    input: &Value,
) -> Result<BTreeMap<String, Value>, ActivityFailure> {
    match input {
        Value::Object(members) => Ok(members.clone().into_iter().collect()),
        other => Err(ActivityFailure::terminal(format!(
            "action `{action}` was dispatched input that is not an object binding its \
             parameters by name; got {}",
            json_kind(other)
        ))),
    }
}

/// A JSON value's kind, named for a refusal message.
const fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}
