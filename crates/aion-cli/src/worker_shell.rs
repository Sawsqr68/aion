//! Manifest-driven, zero-source shell worker composition root.
//!
//! The manifest contains wiring only: commands, scalar argument/environment
//! projections, and a text-vs-JSON encoding hint derived from AWL. Types,
//! timeout, and retry remain owned by the `.awl`; output shape is enforced by
//! the workflow decoder exactly as it is for every other worker.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use aion_package::ActivityDescriptor;

use aion_worker::{
    ActivityContext, ActivityFailure, CancellableCommandOutput, CommandTranscript,
    ProcessGroupError, Worker, WorkerConfig, run_cancellable_command,
};
use anyhow::{Context, Result, bail};
use clap::Args;
use serde::Deserialize;
use serde_json::Value;
use tokio::process::Command;

use crate::worker_surface::{self, ServingSource};

#[derive(Debug, Args)]
pub struct ShellArgs {
    /// Strict TOML shell-worker manifest generated from checked AWL.
    #[arg(long)]
    manifest: PathBuf,
    /// The `.awl` document the manifest was generated from.
    ///
    /// Required, not defaulted. The manifest deliberately carries wiring only —
    /// types live in the document — so the document is the only place the
    /// worker's advertised schemas can come from. Without it the worker
    /// advertises nothing and every queue carrying a deployed contract refuses
    /// it, which is the failure this flag exists to make impossible.
    #[arg(long)]
    awl: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ShellManifest {
    worker: WorkerSection,
    action: Vec<ActionWiring>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerSection {
    name: String,
    task_queue: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ResultEncoding {
    Text,
    Json,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionWiring {
    name: String,
    command: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    result: ResultEncoding,
}

pub async fn run(args: &ShellArgs, endpoint: &str) -> Result<()> {
    // Install the product's own tracing subscriber BEFORE anything can log, for
    // the same reason `aion worker agent` does (see `worker_agent::serve`): the
    // worker SDK's redial driver reports a refused dial — a contract mismatch, a
    // wrong address, a closed port — through tracing, and with no subscriber
    // installed that is dropped on the floor. A shell worker that cannot connect
    // then looks exactly like one that is idle: the process is up, and the
    // process being up is precisely the reading a failed dial makes worthless.
    //
    // The agent verb knew this and the shell verb did not, which is the same
    // shape as the endpoint defect above it: a rule two sibling paths both need,
    // written down in one of them.
    aion_server::observability::tracing::init()?;

    let source = std::fs::read_to_string(&args.manifest)
        .with_context(|| format!("failed to read shell manifest {}", args.manifest.display()))?;
    let manifest = parse_manifest(&source)?;
    let descriptors = declared_descriptors(&args.awl, &manifest)?;
    build_worker(manifest, descriptors, endpoint)?.run().await?;
    Ok(())
}

/// How a refusal names a shell worker's manifest as the thing that chose which of
/// the queue's actions get served.
const MANIFEST_SOURCE: ServingSource = ServingSource {
    subject: "manifest",
    serves: "wires",
    omits: "does not wire",
};

/// Derives the typed action surface this worker must advertise from the `.awl`
/// document, keyed by action name.
///
/// The manifest names the queue and the actions it wires; the document owns the
/// schemas and the all-or-nothing rule. Both derivation and reconciliation live in
/// [`crate::worker_surface`], shared with the agent worker — see that module for why
/// the surface must come from the document rather than from the server, and why a
/// queue is served whole or not at all.
fn declared_descriptors(
    document: &Path,
    manifest: &ShellManifest,
) -> Result<BTreeMap<String, ActivityDescriptor>> {
    let contract = worker_surface::compile_contract(document)?;
    let worker = worker_surface::select_worker(
        document,
        &contract,
        Some(manifest.worker.task_queue.as_str()),
        "manifest",
    )?;
    let wired = manifest
        .action
        .iter()
        .map(|action| action.name.clone())
        .collect::<BTreeSet<_>>();
    worker_surface::reconcile(document, worker, &wired, MANIFEST_SOURCE)
}

fn build_worker(
    manifest: ShellManifest,
    mut descriptors: BTreeMap<String, ActivityDescriptor>,
    endpoint: &str,
) -> Result<Worker> {
    let config = WorkerConfig::builder()
        .endpoint(endpoint)
        .task_queue(&manifest.worker.task_queue)
        .identity(format!("{}-shell-worker", manifest.worker.name))
        .max_concurrency(4)
        .reconnect_initial_backoff(Duration::from_millis(100))
        .reconnect_max_backoff(Duration::from_secs(5))
        .reconnect_max_attempts(usize::MAX)
        .build()?;
    let mut builder = Worker::builder(config);
    for action in manifest.action {
        let name = action.name.clone();
        let descriptor = descriptors.remove(&name).with_context(|| {
            format!("no derived descriptor for action `{name}`; the action surface is incomplete")
        })?;
        builder = builder.register_activity_with_descriptor(
            name,
            descriptor,
            move |input: Value, context| {
                let action = action.clone();
                Box::pin(async move { execute(&action, &input, context).await })
            },
        )?;
    }
    builder.build().map_err(Into::into)
}

fn parse_manifest(source: &str) -> Result<ShellManifest> {
    let manifest: ShellManifest = toml_edit::de::from_str(source)
        .context("shell worker manifest is not valid strict TOML")?;
    if manifest.worker.name.trim().is_empty() {
        bail!("shell worker manifest worker.name must not be empty");
    }
    if manifest.worker.task_queue.trim().is_empty() {
        bail!("shell worker manifest worker.task_queue must not be empty");
    }
    if manifest.action.is_empty() {
        bail!("shell worker manifest must declare at least one action");
    }
    let mut names = BTreeSet::new();
    for action in &manifest.action {
        if action.name.trim().is_empty() {
            bail!("shell worker manifest action.name must not be empty");
        }
        if !names.insert(action.name.as_str()) {
            bail!("shell worker manifest repeats action `{}`", action.name);
        }
        if action.command.is_empty() || action.command[0].trim().is_empty() {
            bail!("shell worker action `{}` has an empty command", action.name);
        }
        for value in action.command.iter().chain(action.env.values()) {
            validate_placeholders(value)
                .with_context(|| format!("invalid projection for action `{}`", action.name))?;
        }
    }
    Ok(manifest)
}

fn validate_placeholders(value: &str) -> Result<()> {
    let mut rest = value;
    while let Some(start) = rest.find('{') {
        let after = &rest[start..];
        let Some(end) = after.find('}') else {
            bail!("unterminated placeholder in `{value}`");
        };
        let placeholder = &after[..=end];
        if placeholder != "{input}"
            && !(placeholder.starts_with("{input.")
                && placeholder.len() > "{input.}".len()
                && placeholder[7..placeholder.len() - 1]
                    .chars()
                    .all(|character| character == '_' || character.is_ascii_alphanumeric()))
        {
            bail!("unsupported placeholder `{placeholder}`");
        }
        rest = &after[end + 1..];
    }
    if rest.contains('}') {
        bail!("unmatched closing brace in `{value}`");
    }
    Ok(())
}

async fn execute(
    action: &ActionWiring,
    input: &Value,
    context: &ActivityContext,
) -> Result<Value, ActivityFailure> {
    let program = expand(&action.command[0], input)?;
    let mut command = Command::new(program);
    for argument in &action.command[1..] {
        command.arg(expand(argument, input)?);
    }
    for (name, value) in &action.env {
        command.env(name, expand(value, input)?);
    }
    // The wired command's output streams onto the activity's transcript seam
    // line by line as it is written, so a manifest-driven action is as readable
    // mid-run as any other step. The action's own result is unchanged.
    let transcript = CommandTranscript::new(context);
    let output = match run_cancellable_command(command, context.cancelled(), &transcript).await {
        Ok(CancellableCommandOutput::Completed(output)) => output,
        Ok(CancellableCommandOutput::Cancelled) => {
            return Err(ActivityFailure::terminal(format!(
                "shell action `{}` was cancelled after its process group stopped",
                action.name
            )));
        }
        Err(ProcessGroupError::Spawn { source }) => {
            return Err(ActivityFailure::retryable(format!(
                "shell action `{}` could not spawn: {source}",
                action.name
            )));
        }
        Err(error) => {
            return Err(ActivityFailure::terminal(format!(
                "shell action `{}` process containment failed: {error}",
                action.name
            )));
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if !output.status.success() {
        let exit = output
            .status
            .code()
            .map_or_else(|| "signal".to_owned(), |code| code.to_string());
        return Err(ActivityFailure::retryable(format!(
            "shell action `{}` exited {exit}: {stderr}",
            action.name
        )));
    }
    match action.result {
        ResultEncoding::Text => Ok(Value::String(stdout)),
        ResultEncoding::Json => serde_json::from_str(&stdout).map_err(|error| {
            ActivityFailure::terminal(format!(
                "shell action `{}` emitted invalid JSON: {error}",
                action.name
            ))
        }),
    }
}

fn expand(template: &str, input: &Value) -> Result<String, ActivityFailure> {
    let whole = serde_json::to_string(input).map_err(|error| {
        ActivityFailure::terminal(format!("input could not be projected as JSON: {error}"))
    })?;
    let mut output = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        output.push_str(&rest[..start]);
        let after = &rest[start..];
        let end = after.find('}').ok_or_else(|| {
            ActivityFailure::terminal(format!("unterminated input placeholder in `{template}`"))
        })?;
        let placeholder = &after[..=end];
        if placeholder == "{input}" {
            output.push_str(&whole);
        } else if let Some(field) = placeholder
            .strip_prefix("{input.")
            .and_then(|value| value.strip_suffix('}'))
        {
            let value = input.get(field).ok_or_else(|| {
                ActivityFailure::terminal(format!("input has no top-level field `{field}`"))
            })?;
            output.push_str(&scalar(field, value)?);
        } else {
            return Err(ActivityFailure::terminal(format!(
                "unsupported input placeholder `{placeholder}`"
            )));
        }
        rest = &after[end + 1..];
    }
    output.push_str(rest);
    Ok(output)
}

fn scalar(field: &str, value: &Value) -> Result<String, ActivityFailure> {
    match value {
        Value::String(value) => Ok(value.clone()),
        Value::Number(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => {
            Err(ActivityFailure::terminal(format!(
                "input field `{field}` is composite or optional and cannot be projected into an argument or environment value"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io;
    use std::path::Path;
    use std::process::Command as StdCommand;
    use std::time::Instant;

    use aion_core::ActivityId;
    use aion_package::emit_shell_manifest;
    use serde_json::json;

    use super::*;

    type TestResult = Result<(), Box<dyn Error>>;

    const MANIFEST: &str = r#"
[worker]
name = "greeter"
task_queue = "greeter"

[[action]]
name = "greet"
command = ["printf", "%s", "{input.name}"]
result = "text"
"#;

    #[test]
    fn strict_manifest_rejects_unknown_keys_and_bad_placeholders() {
        assert!(
            parse_manifest(
                &MANIFEST.replace("name = \"greeter\"", "name = \"greeter\"\nextra = true")
            )
            .is_err()
        );
        assert!(parse_manifest(&MANIFEST.replace("{input.name}", "{nested.name}")).is_err());
    }

    #[test]
    fn composite_field_projection_is_typed_refusal() {
        let failure = expand("{input.items}", &json!({"items": [1, 2]}));
        assert!(failure.is_err());
    }

    /// The document the manifest above is wired from: one queue `greeter`, one
    /// bodyless action `greet` whose types the document owns.
    const DOCUMENT: &str = r"//! greeter: the document the shell worker derives its advertised surface from.
workflow greeter_flow
  input name: String

  outcome greeted: type Greeting, route success

/// What the greeting carries back.
type Greeting {
  message: String,
}

/// The queue a shell worker serves.
worker greeter
  action greet(name: String) -> Greeting

step do_greet
  name |> greet |> route greeted
";

    /// Writes `source` to a uniquely named document so concurrent tests in this
    /// binary never read each other's fixture.
    fn write_document(label: &str, source: &str) -> Result<PathBuf, Box<dyn Error>> {
        let dir =
            std::env::temp_dir().join(format!("aion-shell-worker-{}-{label}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("document.awl");
        std::fs::write(&path, source)?;
        Ok(path)
    }

    /// THE REGRESSION THAT MATTERS. A worker advertising an empty action surface
    /// is refused by contract admission on every queue carrying a deployed
    /// contract — and AWL emits a contract for every worker block, so that is
    /// every queue a scaffolded shell worker exists to serve. The advertised
    /// surface must carry the document's own schemas.
    #[test]
    fn the_worker_advertises_the_documents_action_surface() -> TestResult {
        let document = write_document("advertises", DOCUMENT)?;
        let manifest = parse_manifest(MANIFEST)?;
        let descriptors = declared_descriptors(&document, &manifest)?;
        let worker = build_worker(manifest, descriptors, "http://127.0.0.1:50051")?;

        let advertised = worker.activity_descriptors();
        assert_eq!(
            advertised.len(),
            1,
            "expected the one declared action to be advertised, got {advertised:?}"
        );
        assert_eq!(advertised[0].name, "greet");
        // The schemas are the document's, not a permissive stand-in: `greet`
        // takes a named `name` parameter and returns a record with `message`.
        assert_eq!(
            advertised[0].input_schema["properties"]["name"]["type"],
            json!("string")
        );
        // A declared record return is advertised in its `$ref` + `$defs` form,
        // which is what the server's subset check resolves.
        assert_eq!(
            advertised[0].output_schema["$ref"],
            json!("#/$defs/Greeting")
        );
        assert_eq!(
            advertised[0].output_schema["$defs"]["Greeting"]["properties"]["message"]["type"],
            json!("string")
        );
        Ok(())
    }

    /// A manifest wiring an action the document does not declare would be
    /// admitted for it and then dispatched a call the document never described.
    #[test]
    fn an_undeclared_action_is_refused_by_name() -> TestResult {
        let document = write_document("undeclared", DOCUMENT)?;
        let manifest = parse_manifest(&MANIFEST.replace("name = \"greet\"", "name = \"shout\""))?;
        let Err(error) = declared_descriptors(&document, &manifest) else {
            return Err("an action absent from the document must be refused".into());
        };
        let error = error.to_string();
        assert!(
            error.contains("shout") && error.contains("does not declare"),
            "refusal must name the action and the reason, got: {error}"
        );
        Ok(())
    }

    /// An action whose body the document carries is run by the server. A worker
    /// claiming it would shadow the declared body with a different one.
    #[test]
    fn an_action_with_a_declared_body_is_refused() -> TestResult {
        // A `run` body's outcome members are fixed by the language, so this needs
        // its own document rather than a patch of the shared one: the action's
        // return type, and the outcome it routes to, both follow the body.
        let bodied = r#"//! bodied: the queue whose action the document implements itself.
workflow bodied_flow
  input name: String

  outcome ran: type RunOutcome, route success

/// What a declared command reported.
type RunOutcome { exit_code: Int, stdout: String, stderr: String }

/// The server runs this action; no worker serves it.
worker greeter
  action greet(name: String) -> RunOutcome
    run "printf %s $name"

step do_greet
  name |> greet |> route ran
"#;
        let document = write_document("bodied", bodied)?;
        let manifest = parse_manifest(MANIFEST)?;
        let Err(error) = declared_descriptors(&document, &manifest) else {
            return Err("a declared body is the server's to run, not a worker's".into());
        };
        let error = error.to_string();
        assert!(
            error.contains("declares a body"),
            "refusal must explain that the server runs a declared body, got: {error}"
        );
        Ok(())
    }

    /// Serving is all-or-nothing: a manifest that wires only some of a queue's
    /// actions gets the worker admitted and then dispatched one it cannot serve.
    #[test]
    fn a_partially_wired_queue_is_refused() -> TestResult {
        let two_actions = DOCUMENT.replace(
            "  action greet(name: String) -> Greeting",
            "  action greet(name: String) -> Greeting\n  action shout(name: String) -> Greeting",
        );
        let document = write_document("partial", &two_actions)?;
        let manifest = parse_manifest(MANIFEST)?;
        let Err(error) = declared_descriptors(&document, &manifest) else {
            return Err("a queue must be served whole or not at all".into());
        };
        let error = error.to_string();
        assert!(
            error.contains("shout") && error.contains("whole queue or none"),
            "refusal must name the unserved action and the rule, got: {error}"
        );
        Ok(())
    }

    /// The production emitter must satisfy the shell runtime's real strict
    /// deserializer and whole-queue startup reconciliation without a second
    /// interpretation of which actions are worker-owed.
    #[test]
    fn emitted_manifest_passes_the_real_whole_queue_startup_check() -> TestResult {
        let two_actions = r"//! Two bodyless actions the emitted shell manifest must wire together.
workflow complete_shell_flow
  input name: String
  outcome greeted: type Greeting, route success

type Greeting { message: String }

worker greeter
  action greet(name: String) -> Greeting
  action shout(name: String) -> Greeting

step greet_first
  greet(name: name) -> greeting

step shout_second after greet_first
  shout(name: greeting.message) -> shouted
  route greeted(message: shouted.message)
";
        let document = write_document("emitted-complete", two_actions)?;
        let document_root = document
            .parent()
            .ok_or("generated fixture document has no parent")?;
        let compiled = aion_awl::compile(two_actions, document_root)?;
        let contract = compiled
            .contract
            .workers
            .iter()
            .find(|worker| worker.task_queue == "greeter")
            .ok_or("compiled fixture omitted worker greeter")?;
        let files = emit_shell_manifest(contract, "complete_shell_flow.awl")?;
        let source = files
            .iter()
            .find(|file| file.relative == "worker.toml")
            .map(|file| file.contents.as_str())
            .ok_or("shell emitter omitted worker.toml")?;
        let manifest = parse_manifest(source)?;
        let descriptors = declared_descriptors(&document, &manifest)?;

        assert_eq!(descriptors.len(), 2);
        assert!(descriptors.contains_key("greet"));
        assert!(descriptors.contains_key("shout"));
        Ok(())
    }

    /// A manifest pointed at the wrong document cannot derive anything, and
    /// saying so beats advertising nothing and being refused remotely.
    #[test]
    fn a_queue_the_document_does_not_declare_is_refused() -> TestResult {
        let document = write_document("wrongqueue", DOCUMENT)?;
        let manifest = parse_manifest(
            &MANIFEST.replace("task_queue = \"greeter\"", "task_queue = \"other\""),
        )?;
        let Err(error) = declared_descriptors(&document, &manifest) else {
            return Err("a queue absent from the document must be refused".into());
        };
        let error = error.to_string();
        assert!(
            error.contains("other") && error.contains("greeter"),
            "refusal must name both the wired queue and what the document declares, got: {error}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn process_boundary_round_trips_text_and_json() -> Result<()> {
        let manifest = parse_manifest(MANIFEST)?;
        let document = write_document("roundtrip", DOCUMENT)
            .map_err(|error| anyhow::anyhow!("fixture: {error}"))?;
        let descriptors = declared_descriptors(&document, &manifest)?;
        let worker = build_worker(manifest.clone(), descriptors, "http://127.0.0.1:50051")?;
        assert_eq!(worker.activity_types(), &["greet"]);
        let (context, _cancellation) = ActivityContext::new(
            aion_core::WorkflowId::new_v4(),
            aion_core::RunId::new_v4(),
            ActivityId::from_sequence_position(1),
            1,
        );
        let text = execute(&manifest.action[0], &json!({"name": "Ada"}), &context).await?;
        assert_eq!(text, json!("Ada"));

        let json_action = ActionWiring {
            name: "record".to_owned(),
            command: vec!["printf".to_owned(), "%s".to_owned(), "{input}".to_owned()],
            env: BTreeMap::new(),
            result: ResultEncoding::Json,
        };
        let value = execute(&json_action, &json!({"ok": true}), &context).await?;
        assert_eq!(value, json!({"ok": true}));
        Ok(())
    }

    /// A manifest-wired action's output reaches the activity's transcript seam,
    /// one event per line, without changing the value the action returns.
    ///
    /// The mid-run timing of that delivery is proven where the streaming lives
    /// (`aion_worker::process`); what this pins is the WIRING — that this
    /// composition root hands the running command a transcript at all.
    #[tokio::test]
    async fn a_wired_action_streams_its_output_onto_the_transcript() -> Result<()> {
        let (events, mut transcript) = tokio::sync::mpsc::unbounded_channel();
        let (context, _cancellation) = ActivityContext::with_transcript(
            aion_core::WorkflowId::new_v4(),
            aion_core::RunId::new_v4(),
            ActivityId::from_sequence_position(3),
            1,
            events,
        );
        let action = ActionWiring {
            name: "noisy".to_owned(),
            command: vec![
                "sh".to_owned(),
                "-c".to_owned(),
                "echo one; echo two".to_owned(),
            ],
            env: BTreeMap::new(),
            result: ResultEncoding::Text,
        };

        let value = execute(&action, &Value::Null, &context).await?;
        assert_eq!(value, json!("one\ntwo"), "the action's own result stands");

        // Closing the seam ends the read below; the command has already finished.
        drop(context);
        let mut lines = Vec::new();
        while let Some(event) = transcript.recv().await {
            if let aion_core::ActivityEventKind::Message { text, .. } = event.kind {
                lines.push((event.agent_role, text));
            }
        }
        assert_eq!(
            lines,
            vec![
                ("command stdout".to_owned(), "one".to_owned()),
                ("command stdout".to_owned(), "two".to_owned()),
            ]
        );
        Ok(())
    }

    #[tokio::test]
    async fn cancellation_reaps_the_entire_spawned_process_group() -> TestResult {
        let directory = tempfile::tempdir()?;
        let parent_file = directory.path().join("parent.pid");
        let grandchild_file = directory.path().join("grandchild.pid");
        let script = "echo $$ > \"$1\"; sh -c 'echo $$ > \"$1\"; sleep 300' sh \"$2\" & wait";
        let action = ActionWiring {
            name: "orphan-scan".to_owned(),
            command: vec![
                "sh".to_owned(),
                "-c".to_owned(),
                script.to_owned(),
                "sh".to_owned(),
                parent_file.to_string_lossy().into_owned(),
                grandchild_file.to_string_lossy().into_owned(),
            ],
            env: BTreeMap::new(),
            result: ResultEncoding::Text,
        };
        let (context, cancellation) = ActivityContext::new(
            aion_core::WorkflowId::new_v4(),
            aion_core::RunId::new_v4(),
            ActivityId::from_sequence_position(2),
            1,
        );
        let execution = tokio::spawn(async move { execute(&action, &Value::Null, &context).await });

        wait_for_pid_file(&parent_file).await?;
        wait_for_pid_file(&grandchild_file).await?;
        let parent = read_pid(&parent_file)?;
        let grandchild = read_pid(&grandchild_file)?;
        cancellation.cancel();

        let result = tokio::time::timeout(Duration::from_secs(5), execution)
            .await
            .map_err(|_| io::Error::other("cancelled shell action did not complete"))??;
        if result.is_ok() {
            return Err(io::Error::other("cancelled shell action reported success").into());
        }

        let gone = wait_for_processes_gone(&[parent, grandchild], Duration::from_secs(2)).await?;
        if gone.iter().all(|(_, is_gone)| *is_gone) {
            return Ok(());
        }
        for (pid, is_gone) in &gone {
            if !is_gone {
                kill_process(*pid)?;
            }
        }
        Err(io::Error::other(format!(
            "orphan scan found live processes after cancellation: {gone:?}"
        ))
        .into())
    }

    async fn wait_for_pid_file(path: &Path) -> TestResult {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !path.is_file() {
            if Instant::now() >= deadline {
                return Err(io::Error::other(format!(
                    "timed out waiting for pid file {}",
                    path.display()
                ))
                .into());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Ok(())
    }

    fn read_pid(path: &Path) -> Result<i32, Box<dyn Error>> {
        Ok(std::fs::read_to_string(path)?.trim().parse()?)
    }

    async fn wait_for_processes_gone(
        pids: &[i32],
        timeout: Duration,
    ) -> Result<Vec<(i32, bool)>, io::Error> {
        let deadline = Instant::now() + timeout;
        loop {
            let states = pids
                .iter()
                .map(|pid| process_is_gone(*pid).map(|gone| (*pid, gone)))
                .collect::<Result<Vec<_>, _>>()?;
            if states.iter().all(|(_, gone)| *gone) || Instant::now() >= deadline {
                return Ok(states);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    fn process_is_gone(pid: i32) -> Result<bool, io::Error> {
        let status = StdCommand::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?;
        Ok(!status.success())
    }

    fn kill_process(pid: i32) -> Result<(), io::Error> {
        let status = StdCommand::new("kill")
            .args(["-KILL", &pid.to_string()])
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "cleanup could not kill orphan pid {pid}"
            )))
        }
    }
}
