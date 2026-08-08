//! Shell-worker wiring generated from the same compiled queue contract and
//! demand plan as the Rust worker scaffold.
//!
//! The shell runtime validates its manifest against the queue's complete
//! worker-owed action set before dialing. It also exposes no node flag while its
//! worker configuration registers with a hostname-derived node, so it cannot
//! express the per-node topology a pinned queue requires. This emitter therefore
//! wires every planned action for an unpinned queue and refuses any queue with a
//! worker-owed node pin.

use crate::contract::WorkerContract;

use super::error::AwlScaffoldError;
use super::plan::{ConnectionPlan, ServableAction, plan};
use super::scaffold::{FileOwnership, ScaffoldedFile};
use super::text::string_literal;

/// Renders the strict shell-worker manifest and its operating instructions for
/// one compiled queue contract.
///
/// The manifest carries commands and result encodings only. Action schemas,
/// timeout, and retry remain owned by the AWL document supplied to the worker at
/// startup. Every generated command fails loudly and retries under the
/// document's policy until the author replaces it, so an untouched scaffold can
/// never report a result nobody computed.
///
/// # Errors
///
/// Returns an [`AwlScaffoldError`] when the queue has no worker-owed action,
/// when an action cannot be named safely by the shared connection plan, or when
/// any worker-owed action is node-pinned and the shell runtime therefore cannot
/// represent the queue's required topology.
pub fn emit_shell_manifest(
    contract: &WorkerContract,
    document_name: &str,
) -> Result<Vec<ScaffoldedFile>, AwlScaffoldError> {
    let plan = plan(contract)?;
    if plan.servable.iter().any(|action| action.node.is_some()) {
        return Err(AwlScaffoldError::NodePinnedQueue {
            task_queue: plan.task_queue,
        });
    }

    let worker_manifest = emit_worker_manifest(&plan.task_queue, document_name, &plan.servable);
    let readme = emit_readme(&plan, document_name);
    Ok(vec![
        ScaffoldedFile {
            relative: "worker.toml".to_owned(),
            contents: worker_manifest,
            ownership: FileOwnership::Generated,
        },
        ScaffoldedFile {
            relative: "README.md".to_owned(),
            contents: readme,
            ownership: FileOwnership::Generated,
        },
    ])
}

/// Emits only fields accepted by the shell worker's deny-unknown-fields
/// deserializer.
fn emit_worker_manifest(
    task_queue: &str,
    document_name: &str,
    actions: &[ServableAction],
) -> String {
    let mut output = String::new();
    output.push_str("# Generated from ");
    output.push_str(&string_literal(document_name));
    output.push_str(" by `aion awl scaffold`.\n");
    output.push_str("# Wiring only: types, timeout, and retry live only in the .awl.\n\n");
    output.push_str("[worker]\nname = ");
    output.push_str(&string_literal(task_queue));
    output.push_str("\ntask_queue = ");
    output.push_str(&string_literal(task_queue));
    output.push('\n');

    for action in actions {
        let command = format!(
            "echo 'aion scaffold stub: action {} has no command wired - edit worker.toml' >&2; exit 78",
            action.name
        );
        let result = if action
            .output_schema
            .get("type")
            .and_then(serde_json::Value::as_str)
            == Some("string")
        {
            "text"
        } else {
            "json"
        };
        output.push_str("\n[[action]]\nname = ");
        output.push_str(&string_literal(&action.name));
        output.push_str("\ncommand = [\"sh\", \"-ec\", ");
        output.push_str(&string_literal(&command));
        output.push_str("]\nresult = \"");
        output.push_str(result);
        output.push_str("\"\n");
    }
    output
}

/// Explains the production invocation, schema ownership, and whole-queue
/// startup contract.
fn emit_readme(plan: &ConnectionPlan, document_name: &str) -> String {
    let mut output = String::new();
    output.push_str("# Shell worker for `");
    output.push_str(&plan.task_queue);
    output.push_str("`\n\nGenerated from `");
    output.push_str(document_name);
    output.push_str("`.\n\n## Run\n\n```console\naion worker shell --manifest worker.toml --awl ");
    output.push_str(document_name);
    output.push_str(" --endpoint <server>\n```\n\n");
    output.push_str(
        "The manifest carries wiring only. The AWL document is the sole source of action \
         schemas, types, timeout, and retry. Pass the same document this scaffold was \
         generated from.\n\nServing is all-or-nothing per queue. This manifest wires the complete \
         worker-owed action set, and `aion worker shell` validates that whole set against the \
         document before it dials the server. Server-executed actions never belong in the \
         manifest.\n",
    );
    output
}
