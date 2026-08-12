//! `aion assistant` — the operator's verb for the assistant that ships in this
//! binary (#139 leg C).
//!
//! # One document, read not restated
//!
//! Every name this module sends over the wire comes from
//! [`aion_server::assistant`]: the workflow type, the start inputs, the control
//! signal and its payload fields, the status query. That module verifies each
//! one against the embedded document before handing it over, so a document edit
//! that renames a surface refuses HERE, by name, instead of producing a request
//! the server rejects for reasons the operator has to reverse-engineer.
//!
//! # What is local and what needs a server
//!
//! `document` and `describe` read the binary and never dial. `start`, `say`,
//! `end`, and `status` are ordinary caller-SDK operations against the running
//! server named by `--endpoint`, carrying payloads validated against the
//! embedded contract BEFORE the connection is made.
//!
//! # Serving the agent side is still the operator's step
//!
//! The document declares one `agent` action on its worker's queue. A session
//! parks until something serves that queue (`aion worker agent`, whichever
//! harness), and the assistant's brain is that worker's business, not this
//! verb's.

use std::path::PathBuf;

use aion_client::{Client, StartOptions};
use aion_core::Payload;
use anyhow::{Context, Result, anyhow};
use clap::Subcommand;
use serde_json::{Value, json};

use aion_server::assistant::{
    AssistantSessionContract, CONTINUE_END_FIELD, CONTINUE_MESSAGE_FIELD, CONTINUE_SIGNAL,
    EmbeddedAssistant, OBJECTIVE_INPUT, REPO_PATH_INPUT, STATUS_QUERY, embedded_assistant,
};

use crate::output::{AcknowledgementOutput, QueryOutput, start_output, to_value};

/// Operate the assistant this binary carries.
#[derive(Debug, Subcommand)]
pub(crate) enum AssistantCommand {
    /// Print the assistant document compiled into this binary.
    ///
    /// This is how an operator with no repository checkout gets the document —
    /// to read it, or to feed it to `aion worker agent` so something serves the
    /// agent action a session dispatches.
    Document {
        /// Write the document to this path instead of standard output.
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Print the assistant's identity and contract: workflow type, content
    /// hash, start inputs, signals, queries, and the session surfaces the other
    /// subcommands drive.
    Describe,
    /// Start an assistant session and print its identifiers.
    ///
    /// The session parks on its control signal after each round; continue it
    /// with `aion assistant say` and finish it with `aion assistant end`.
    Start {
        /// The operator's opening ask. Blank is refused: there is nothing to
        /// open a session with.
        #[arg(long)]
        objective: String,
        /// The aion repository this session grounds its answers in — a local
        /// path or a clone URL.
        #[arg(long, required_unless_present = "no_repo", conflicts_with = "no_repo")]
        repo_path: Option<String>,
        /// Start with no repository (the document's scratch-workspace mode).
        /// Stated explicitly, because a session that silently has no ground
        /// truth answers from memory.
        #[arg(long)]
        no_repo: bool,
    },
    /// Send the operator's next message into a parked session.
    Say {
        /// The session's workflow identifier.
        workflow_id: String,
        /// The message, delivered to the agent verbatim.
        #[arg(long)]
        message: String,
        /// Specific run identifier. Defaults to the latest run when omitted.
        #[arg(long)]
        run_id: Option<String>,
    },
    /// End a session cleanly.
    End {
        /// The session's workflow identifier.
        workflow_id: String,
        /// Specific run identifier. Defaults to the latest run when omitted.
        #[arg(long)]
        run_id: Option<String>,
    },
    /// Read a session's live status: which phase it is in, and how many rounds
    /// it has run.
    Status {
        /// The session's workflow identifier.
        workflow_id: String,
        /// Specific run identifier. Defaults to the latest run when omitted.
        #[arg(long)]
        run_id: Option<String>,
    },
}

/// Runs one assistant subcommand.
///
/// `connect` is called at most ONCE, and only after the subcommand's own
/// arguments have been composed and validated — so a malformed invocation is
/// refused by name rather than by a connection error, and `document` and
/// `describe` never dial at all. It is a factory rather than a built client
/// precisely so that ordering is structural: there is no connection to misuse
/// before the composition step has run.
pub(crate) async fn run<Connect, Connecting>(
    command: &AssistantCommand,
    connect: Connect,
) -> Result<Value>
where
    Connect: FnOnce() -> Connecting,
    Connecting: std::future::Future<Output = Result<Client>>,
{
    let embedded = embedded_assistant()
        .map_err(|error| anyhow!("this binary carries no usable assistant: {error}"))?;
    match command {
        AssistantCommand::Document { output } => document(embedded, output.as_deref()),
        AssistantCommand::Describe => describe(embedded),
        AssistantCommand::Start {
            objective,
            repo_path,
            no_repo,
        } => {
            let input = compose_start(embedded, objective, repo_path.as_deref(), *no_repo)?;
            start(&connect().await?, embedded, input).await
        }
        AssistantCommand::Say {
            workflow_id,
            message,
            run_id,
        } => {
            let addressed =
                compose_continuation(embedded, workflow_id, message, false, run_id.as_deref())?;
            deliver(&connect().await?, addressed).await
        }
        AssistantCommand::End {
            workflow_id,
            run_id,
        } => {
            let addressed =
                compose_continuation(embedded, workflow_id, "", true, run_id.as_deref())?;
            deliver(&connect().await?, addressed).await
        }
        AssistantCommand::Status {
            workflow_id,
            run_id,
        } => {
            let (workflow_id, run_id) = compose_status(workflow_id, run_id.as_deref())?;
            status(&connect().await?, &workflow_id, run_id.as_ref()).await
        }
    }
}

/// `aion assistant document`.
fn document(embedded: &EmbeddedAssistant, output: Option<&std::path::Path>) -> Result<Value> {
    let source = embedded.source();
    let Some(path) = output else {
        // Printed to stdout as the document's own text, not wrapped in JSON:
        // the point of this verb is to produce a file `aion worker agent` and
        // `aion awl check` can read.
        print!("{source}");
        return Ok(Value::Null);
    };
    std::fs::write(path, source).with_context(|| {
        format!(
            "failed to write the assistant document to `{}`",
            path.display()
        )
    })?;
    Ok(json!({
        "workflow_type": embedded.workflow_type(),
        "content_hash": embedded.content_hash().to_string(),
        "written": path.display().to_string(),
        "bytes": source.len(),
    }))
}

/// `aion assistant describe`.
fn describe(embedded: &EmbeddedAssistant) -> Result<Value> {
    let signals: Vec<Value> = embedded
        .signals()
        .iter()
        .map(|signal| json!({"name": signal.name, "input_schema": signal.input_schema}))
        .collect();
    Ok(json!({
        "workflow_type": embedded.workflow_type(),
        // The queue something must serve for a session to run. Reported here
        // because `describe` is the surface an operator with no checkout reads,
        // and a worker on the wrong queue fails silently: it connects, it
        // registers, and it is never dispatched to.
        "task_queue": embedded.task_queue(),
        "content_hash": embedded.content_hash().to_string(),
        "document_bytes": embedded.source().len(),
        "input_schema": embedded.input_schema(),
        "signals": signals,
        "queries": embedded.queries(),
        "session": serde_json::to_value(AssistantSessionContract::current())
            .context("failed to encode the assistant session contract")?,
    }))
}

/// A request composed and validated locally, with nothing left to decide but
/// sending it.
///
/// Composition is deliberately separate from sending. Everything a bad
/// invocation can be caught by — a blank objective, an empty continuation, an
/// unparseable identifier, a payload the document's schema refuses — is decided
/// here, BEFORE anything dials. An operator who typed the command wrong should
/// be told what they typed wrong, not that a server could not be reached.
struct Addressed {
    workflow_id: aion_core::WorkflowId,
    run_id: Option<aion_core::RunId>,
    payload: Payload,
}

/// Composes and validates the start input.
fn compose_start(
    embedded: &EmbeddedAssistant,
    objective: &str,
    repo_path: Option<&str>,
    no_repo: bool,
) -> Result<Payload> {
    if objective.trim().is_empty() {
        return Err(anyhow!(
            "--objective must not be blank: it is the ask the session opens with"
        ));
    }
    // The document's scratch mode IS the empty string; `--no-repo` is the
    // operator saying so out loud rather than the CLI defaulting on their
    // behalf. `required_unless_present` already made one of the two mandatory.
    let repo = if no_repo { "" } else { repo_path.unwrap_or("") };
    let input = json!({ OBJECTIVE_INPUT: objective, REPO_PATH_INPUT: repo });
    validate(embedded.input_schema(), &input, "start input")?;
    payload(&input)
}

/// Starts the session from an already-validated input.
async fn start(client: &Client, embedded: &EmbeddedAssistant, input: Payload) -> Result<Value> {
    let outcome = client
        .start(
            embedded.workflow_type().to_owned(),
            input,
            StartOptions::default(),
        )
        .await
        .context("failed to start the assistant session")?;
    to_value(start_output(&outcome))
}

/// Composes and validates the one signal both `say` and `end` send.
///
/// `finish = false` is the operator's next message and must not be empty: the
/// document treats a blank continuation as a no-op nudge that spends no round,
/// so sending one costs a round trip and changes nothing.
fn compose_continuation(
    embedded: &EmbeddedAssistant,
    workflow_id: &str,
    message: &str,
    finish: bool,
    run_id: Option<&str>,
) -> Result<Addressed> {
    if !finish && message.is_empty() {
        return Err(anyhow!(
            "--message must not be empty: the document treats a blank continuation as a no-op \
             nudge that spends no round. To finish the session, use `aion assistant end`"
        ));
    }
    let workflow_id = crate::payload::parse_workflow_id(workflow_id)?;
    let run_id = run_id.map(crate::payload::parse_run_id).transpose()?;
    let value = json!({ CONTINUE_MESSAGE_FIELD: message, CONTINUE_END_FIELD: finish });
    validate(
        embedded
            .continuation_schema()
            .map_err(|error| anyhow!("{error}"))?,
        &value,
        CONTINUE_SIGNAL,
    )?;
    Ok(Addressed {
        workflow_id,
        run_id,
        payload: payload(&value)?,
    })
}

/// Delivers an already-validated continuation.
async fn deliver(client: &Client, addressed: Addressed) -> Result<Value> {
    client
        .signal(
            &addressed.workflow_id,
            addressed.run_id.as_ref(),
            CONTINUE_SIGNAL.to_owned(),
            addressed.payload,
        )
        .await
        .context("failed to signal the assistant session")?;
    to_value(AcknowledgementOutput {
        workflow_id: &addressed.workflow_id.to_string(),
        accepted: true,
    })
}

/// Parses the identifiers a status read addresses.
fn compose_status(
    workflow_id: &str,
    run_id: Option<&str>,
) -> Result<(aion_core::WorkflowId, Option<aion_core::RunId>)> {
    let workflow_id = crate::payload::parse_workflow_id(workflow_id)?;
    let run_id = run_id.map(crate::payload::parse_run_id).transpose()?;
    Ok((workflow_id, run_id))
}

/// Reads a session's live status.
async fn status(
    client: &Client,
    workflow_id: &aion_core::WorkflowId,
    run_id: Option<&aion_core::RunId>,
) -> Result<Value> {
    let answer = client
        .query(
            workflow_id,
            run_id,
            STATUS_QUERY.to_owned(),
            // The canonical "no arguments" document, exactly as `aion query`
            // sends when `--arguments` is omitted — the status query takes none.
            crate::payload::json_payload("null")?,
            // The CLI's one query deadline, shared with `aion query`: two
            // deadlines for one operation is two truths waiting to disagree.
            crate::QUERY_DEADLINE,
        )
        .await
        .context("failed to query the assistant session")?;
    to_value(QueryOutput {
        result: crate::payload::payload_to_json(&answer)?,
    })
}

/// Encodes a JSON value as a workflow payload.
fn payload(value: &Value) -> Result<Payload> {
    Payload::from_json(value).context("failed to encode the assistant payload")
}

/// Validates a value against a document-derived schema before it is sent, so a
/// contract mismatch is named locally rather than as a server refusal.
fn validate(schema: &Value, value: &Value, what: &str) -> Result<()> {
    let validator = jsonschema::validator_for(schema)
        .with_context(|| format!("the assistant's {what} schema is not a valid JSON Schema"))?;
    if validator.is_valid(value) {
        return Ok(());
    }
    let rendered = serde_json::to_string(schema)
        .with_context(|| format!("failed to encode the assistant's {what} schema"))?;
    Err(anyhow!(
        "the composed {what} does not match the assistant document's contract\nsent: {value}\nexpected schema: {rendered}"
    ))
}

#[cfg(test)]
#[path = "assistant_tests.rs"]
mod assistant_tests;
