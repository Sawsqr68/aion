//! The compile seam for `worker` documents: from `.awl` source to the typed
//! action surface a worker advertises and the bodies it executes.
//!
//! A worker document produces no BEAM module and no workflow contract — it
//! declares no flow. What it produces is exactly what a worker process needs
//! to serve a queue: the queue's name, one typed contract per action, and the
//! command each action runs. The typed half comes from the SAME derivation
//! the workflow compiler uses ([`crate::compile::worker_contracts`]), so a
//! worker document and the workflow document that calls it cannot disagree
//! about an action's schemas through two code paths — and when they disagree
//! about the DECLARATION, the server's contract admission refuses the worker
//! by name, which is the mechanism that keeps the two files honest.

use std::path::Path;

use aion_package::WorkerContract;

use crate::ast::{ActionBody, DocumentFamily, RunCapture, scan_command};
use crate::compile::{CompileError, worker_contracts};

/// Everything a worker process needs to serve the queue a worker document
/// declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledWorkerDocument {
    /// The task queue this document serves — the name on its opening
    /// `worker` declaration.
    pub task_queue: String,
    /// The typed contract for every action on the queue, derived exactly as
    /// the deploy path derives the calling workflow's.
    pub contract: WorkerContract,
    /// The executable body of every action, in declaration order. A worker
    /// document has one per action: the checker refuses a bodyless action,
    /// because a worker document that could not serve one of its own
    /// declarations would be admitted and then dispatched work it cannot do.
    pub actions: Vec<CompiledWorkerAction>,
}

/// One action's executable body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledWorkerAction {
    /// Action name — the activity type dispatched on the queue.
    pub name: String,
    /// The declared command, exactly as authored. The executor parses it
    /// into a fixed argv itself and substitutes each parameter as one whole
    /// argument; it is never spliced back into a command string.
    pub command: String,
    /// The action parameters the command references, sorted and
    /// deduplicated. Checked against the declaration, so every name here is
    /// a declared parameter of the action.
    pub parameters: Vec<String>,
    /// What the action's result is taken from.
    pub capture: RunCapture,
}

/// Compile a `worker` document: parse → typecheck → contract derivation.
///
/// # Errors
///
/// Returns [`CompileError::Parse`] or [`CompileError::Check`] with the
/// document's own diagnostics, [`CompileError::Family`] when the source is a
/// `workflow` document (which this seam cannot serve, and which names the
/// remedy), and [`CompileError::Schema`] when a declared type derives a
/// schema no validator can compile.
pub fn compile_worker_document(
    source: &str,
    schema_root: &Path,
) -> Result<CompiledWorkerDocument, CompileError> {
    let document = crate::parse(source).map_err(CompileError::Parse)?;
    if document.family == DocumentFamily::Workflow {
        return Err(CompileError::Family {
            message: format!(
                "`{name}` is a workflow document, not a worker document: it declares a flow \
                 to run, not the actions of one task queue. Deploy it with `aion deploy`, \
                 and serve its queues with a worker document, a worker SDK build, or \
                 `aion worker shell`.",
                name = document.name
            ),
            span: document.name_span,
        });
    }
    let errors = crate::check_in(&document, schema_root);
    if !errors.is_empty() {
        return Err(CompileError::Check(errors));
    }
    let mut contracts = worker_contracts(&document, schema_root)?;
    // `ActionContract::body` means "the DEPLOYED PACKAGE says what this
    // action does, so the host executes it with no worker connected"
    // (`ActionContract::worker_owed`). A worker document's body means the
    // opposite: this worker executes it, and the queue is owed a connected
    // worker. Carrying the body here would flip that one reading, so the
    // executable half travels in `actions` and the contract states only the
    // typed surface a worker advertises.
    for contract in &mut contracts {
        for action in &mut contract.actions {
            action.body = None;
        }
    }
    // The checker has ruled that a worker document declares exactly one
    // `worker` block — its opening declaration — so a second contract cannot
    // reach here. Draining the first is the whole selection; there is no
    // choice to make and therefore no default to assume.
    let contract = if contracts.is_empty() {
        return Err(CompileError::Family {
            message: format!(
                "worker document `{name}` declares no actions, so there is no surface to \
                 serve; declare at least one `action` under `worker {name}`.",
                name = document.name
            ),
            span: document.name_span,
        });
    } else {
        contracts.remove(0)
    };
    let mut actions = Vec::new();
    for worker in document.workers.iter().take(1) {
        for action in &worker.actions {
            let Some(ActionBody::Run {
                capture,
                command,
                command_span,
                ..
            }) = &action.body
            else {
                // Unreachable through `check_in`, which refuses a bodyless
                // action in this family by name. Reported rather than
                // skipped: silently dropping the action would advertise a
                // contract this worker cannot serve, which is the one
                // failure the all-or-nothing surface rule exists to stop.
                return Err(CompileError::Lower {
                    message: format!(
                        "action `{}` reached the worker compiler with no body after checking \
                         clean; this is a checker defect",
                        action.name
                    ),
                    span: action.span,
                });
            };
            let scanned = scan_command(command).map_err(|error| CompileError::Lower {
                message: format!(
                    "action `{}` has a malformed `run` command that passed the checker: {error}",
                    action.name
                ),
                span: *command_span,
            })?;
            actions.push(CompiledWorkerAction {
                name: action.name.clone(),
                command: command.clone(),
                parameters: scanned.parameters(),
                capture: *capture,
            });
        }
    }
    Ok(CompiledWorkerDocument {
        task_queue: document.name.clone(),
        contract,
        actions,
    })
}
