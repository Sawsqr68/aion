//! The stable compile seam: one call from `.awl` source to everything
//! downstream packaging needs — deterministic `.beam` bytes, the derived
//! start/outcome contracts, action requirements, and the `.gleam_types`
//! sidecar. Wraps the `#[doc(hidden)]` MIR pipeline (lower → verify →
//! select → sidecar) without exposing any MIR type.

use std::fmt;
use std::path::Path;
use std::time::Duration;

use aion_package::{
    ActionBodyContract, ActionContract, AdditionalWorkflowContract, ChildContract, PackageContract,
    RetryContract, SignalContract, WorkerContract,
};
use serde_json::Value;

use crate::ast::{ActionBody, Document, PipeStage, RetrySpec, Statement, Step, scan_command};
use crate::mir::{LowerError, lower, project_sidecar, select, verify};
use crate::{CheckError, ParseError, SchemaError, Span};

/// Everything the compile pipeline produces for one workflow document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledWorkflow {
    /// Workflow name declared in the AWL document.
    pub workflow_name: String,
    /// First declared worker, which is the document-derived task queue for
    /// deploy surfaces; `None` means the caller applies its default queue.
    pub first_worker: Option<String>,
    /// Document-declared workflow timeout; `None` leaves packaging policy to
    /// the assembler default.
    pub timeout: Option<Duration>,
    /// Deterministic `.beam` bytes (the `select` output, self-validated).
    pub beam_bytes: Vec<u8>,
    /// Derived start contract (the existing workflow-schema derivation).
    pub input_schema: Value,
    /// Derived outcome envelope (`{ "outcome": name, "payload": … }`).
    pub output_schema: Value,
    /// Complete checked contract surface committed into package identity.
    pub contract: PackageContract,
    /// Effective action requirements, one row per distinct node requirement.
    pub actions: Vec<ActionRequirement>,
    /// The declarative action implementations the document carries, one row
    /// per action that declares a body, in declaration order. Actions
    /// absent from this list are requirements on an out-of-band worker.
    pub action_bodies: Vec<ActionImplementation>,
    /// `.gleam_types` sidecar bytes (deterministic).
    pub sidecar_bytes: Vec<u8>,
    /// Synthesized same-package workflow entries (one per implicit per-item
    /// child a parallel multi-step `distribute` region requires), in the
    /// shared emission order — the packaging seam maps these onto the
    /// manifest's `additional_workflows`.
    pub synthesized_workflows: Vec<crate::SynthesizedWorkflowEntry>,
}

/// One effective action requirement: the task queue (worker name), the
/// action, and one effective node requirement — the call-site `node`
/// override where a call pins one, the action declaration's `node`
/// otherwise. An action whose call sites pin distinct nodes yields one row
/// per distinct requirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRequirement {
    /// Worker (task queue) name the action is declared on.
    pub task_queue: String,
    /// Action name.
    pub action: String,
    /// Effective node requirement, `None` when unpinned.
    pub node: Option<String>,
}

/// One action the document implements itself: the queue it is declared on,
/// the action name, and the declarative body a runtime must execute in
/// place of an out-of-band worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionImplementation {
    /// Worker (task queue) name the action is declared on.
    pub task_queue: String,
    /// Action name.
    pub action: String,
    /// The compiled body.
    pub body: CompiledActionBody,
}

/// A compiled declarative action body: what a runtime needs to execute the
/// action with no source document in hand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompiledActionBody {
    /// One declared command.
    Run {
        /// The declared command, exactly as authored. This is the
        /// authoritative form: the executor parses it into a fixed argv
        /// itself and substitutes each parameter as one whole argument. It
        /// is never spliced back into a command string.
        command: String,
        /// The action parameters the command references, sorted and
        /// deduplicated — the binding a runtime must supply. Checked
        /// against the declaration, so every name here is a declared
        /// parameter of the action.
        parameters: Vec<String>,
    },
}

/// A compile failure at any pipeline stage, carrying the existing
/// diagnostics losslessly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    /// The document does not parse.
    Parse(ParseError),
    /// The document does not typecheck (every diagnostic, source order).
    Check(Vec<CheckError>),
    /// A contract schema could not be derived.
    Schema(SchemaError),
    /// A shape the direct compiler does not yet lower (the BC-2 scope
    /// marker, NOT a reference refusal).
    Unsupported {
        /// The refused shape, verbatim from the lowering.
        shape: String,
        /// Source span of the refused construct.
        span: Span,
    },
    /// A span-anchored lowering failure.
    Lower {
        /// Diagnostic text, verbatim from the lowering.
        message: String,
        /// Source span of the offending construct.
        span: Span,
    },
    /// A planning refusal from the shared emitter passes (no span).
    Planning {
        /// Diagnostic text, verbatim from the planning pass.
        message: String,
    },
    /// An internal backend failure (verify/select), a bug upstream — the
    /// wrapped diagnostic rendered verbatim.
    Backend {
        /// Diagnostic text, verbatim from the backend stage.
        message: String,
    },
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "{error}"),
            // `CheckError` has no Display of its own; the structured
            // diagnostics (pinned verbatim by test) are the contract, this
            // aggregate rendering is only a fallback for `.to_string()`.
            Self::Check(errors) => {
                let mut first = true;
                for error in errors {
                    if !first {
                        writeln!(f)?;
                    }
                    first = false;
                    write!(
                        f,
                        "{} at line {}, column {}",
                        error.message, error.span.line, error.span.column
                    )?;
                }
                Ok(())
            }
            Self::Schema(error) => write!(f, "{error}"),
            // The two span-anchored lowering forms render exactly as the
            // MIR path's `LowerError` does today (pinned by test).
            Self::Unsupported { shape, span } => {
                write!(f, "BC-2 does not yet lower {shape} (line {})", span.line)
            }
            Self::Lower { message, span } => write!(f, "{message} (line {})", span.line),
            Self::Planning { message } | Self::Backend { message } => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for CompileError {}

/// Compile `.awl` source to a [`CompiledWorkflow`], resolving schema imports
/// relative to `schema_root` (the document's directory) exactly as the CLI's
/// emit path does.
///
/// Pipeline: parse → typecheck → contract derivation → MIR lower → verify →
/// select → sidecar. Deterministic: the same source yields the same bytes.
///
/// # Errors
///
/// Returns [`CompileError`] wrapping the failing stage's existing
/// diagnostics: parse/check/schema errors verbatim, lowering refusals with
/// the unsupported/planning distinction preserved, and backend invariant
/// failures rendered verbatim.
pub fn compile(source: &str, schema_root: &Path) -> Result<CompiledWorkflow, CompileError> {
    let document = crate::parse(source).map_err(CompileError::Parse)?;
    let errors = crate::check_in(&document, schema_root);
    if !errors.is_empty() {
        return Err(CompileError::Check(errors));
    }
    let input_schema =
        crate::schema_for_workflow_in(&document, schema_root).map_err(CompileError::Schema)?;
    let output_schema =
        crate::schema_for_outcomes_in(&document, schema_root).map_err(CompileError::Schema)?;
    let actions = action_requirements(&document);
    let action_bodies = action_implementations(&document)?;
    let synthesized_workflows = synthesized_workflow_entries(&document, schema_root)?;
    let contract = package_contract(
        &document,
        schema_root,
        input_schema.clone(),
        output_schema.clone(),
        &synthesized_workflows,
    )?;
    let module = lower(&document, Some(schema_root)).map_err(lower_diagnostic)?;
    verify(&module).map_err(|error| CompileError::Backend {
        message: error.to_string(),
    })?;
    let beam_bytes = select(&module).map_err(|error| CompileError::Backend {
        message: error.to_string(),
    })?;
    let sidecar_bytes = project_sidecar(&module);
    let timeout = document
        .timeout
        .as_ref()
        .map(|timeout| {
            timeout
                .duration
                .checked_duration()
                .ok_or_else(|| CompileError::Backend {
                    message: "checked workflow timeout overflowed during compilation".to_owned(),
                })
        })
        .transpose()?;
    Ok(CompiledWorkflow {
        workflow_name: document.name.clone(),
        first_worker: document.workers.first().map(|worker| worker.name.clone()),
        timeout,
        beam_bytes,
        input_schema,
        output_schema,
        contract,
        actions,
        action_bodies,
        sidecar_bytes,
        synthesized_workflows,
    })
}

/// Compile every declarative action body in the document.
///
/// The checker has already ruled on interpolation, so a malformed command
/// cannot reach here; if one ever did it is a checker defect, and it is
/// reported as a span-anchored lowering failure rather than swallowed.
fn action_implementations(document: &Document) -> Result<Vec<ActionImplementation>, CompileError> {
    let mut implementations = Vec::new();
    for worker in &document.workers {
        for action in &worker.actions {
            let Some(ActionBody::Run {
                command,
                command_span,
                ..
            }) = &action.body
            else {
                continue;
            };
            let scanned = scan_command(command).map_err(|error| CompileError::Lower {
                message: format!(
                    "action `{}` has a malformed `run` command that passed the checker: {error}",
                    action.name
                ),
                span: *command_span,
            })?;
            implementations.push(ActionImplementation {
                task_queue: worker.name.clone(),
                action: action.name.clone(),
                body: CompiledActionBody::Run {
                    command: command.clone(),
                    parameters: scanned.parameters(),
                },
            });
        }
    }
    Ok(implementations)
}

fn package_contract(
    document: &Document,
    schema_root: &Path,
    input_schema: Value,
    output_schema: Value,
    synthesized: &[crate::SynthesizedWorkflowEntry],
) -> Result<PackageContract, CompileError> {
    let mut workers = Vec::new();
    for worker in &document.workers {
        let mut actions = Vec::new();
        for action in &worker.actions {
            let input_schema = crate::schema_for_params_in(document, schema_root, &action.params)
                .map_err(CompileError::Schema)?;
            let output_schema =
                crate::schema_for_type_ref_in(document, schema_root, &action.returns)
                    .map_err(CompileError::Schema)?;
            let node = action
                .config
                .as_ref()
                .and_then(|config| config.node.as_ref())
                .map(|node| node.name.clone());
            let timeout = action
                .config
                .as_ref()
                .and_then(|config| config.timeout.as_ref())
                .map(checked_contract_duration)
                .transpose()?;
            let retry = action
                .config
                .as_ref()
                .and_then(|config| config.retry.as_ref())
                .map(retry_contract)
                .transpose()?;
            let advisory = action
                .config
                .as_ref()
                .is_some_and(|config| config.advisory.is_some());
            // The declarative body rides into package identity: a document
            // that says what an action DOES is a different promise from one
            // that only says an action exists.
            let body = action.body.as_ref().map(|body| match body {
                ActionBody::Run { command, .. } => ActionBodyContract::Run {
                    command: command.clone(),
                },
            });
            actions.push(ActionContract {
                name: action.name.clone(),
                input_schema,
                output_schema,
                node,
                timeout,
                retry,
                advisory,
                body,
            });
        }
        workers.push(WorkerContract {
            task_queue: worker.name.clone(),
            actions,
        });
    }
    let children = document
        .children
        .iter()
        .map(|child| {
            Ok(ChildContract {
                name: child.name.clone(),
                input_schema: crate::schema_for_params_in(document, schema_root, &child.params)
                    .map_err(CompileError::Schema)?,
                output_schema: crate::schema_for_type_ref_in(document, schema_root, &child.returns)
                    .map_err(CompileError::Schema)?,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let signals = document
        .signals
        .iter()
        .map(|signal| {
            Ok(SignalContract {
                name: signal.name.clone(),
                input_schema: crate::schema_for_type_ref_in(document, schema_root, &signal.ty)
                    .map_err(CompileError::Schema)?,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let additional_workflows = synthesized
        .iter()
        .map(|entry| AdditionalWorkflowContract {
            workflow_type: entry.workflow_type.clone(),
            input_schema: entry.input_schema.clone(),
            output_schema: entry.output_schema.clone(),
        })
        .collect();
    Ok(PackageContract {
        input_schema,
        output_schema,
        workers,
        children,
        signals,
        additional_workflows,
        unscoped_activities: Vec::new(),
    })
}

fn retry_contract(retry: &RetrySpec) -> Result<RetryContract, CompileError> {
    match retry {
        RetrySpec::Every { count, every, .. } => Ok(RetryContract::Every {
            count: *count,
            every: checked_contract_duration(every)?,
        }),
        RetrySpec::Backoff {
            count, min, max, ..
        } => Ok(RetryContract::Backoff {
            count: *count,
            min: checked_contract_duration(min)?,
            max: checked_contract_duration(max)?,
        }),
    }
}

fn checked_contract_duration(duration: &crate::DurationLiteral) -> Result<Duration, CompileError> {
    duration
        .checked_duration()
        .ok_or_else(|| CompileError::Backend {
            message: "checked contract duration overflowed during compilation".to_owned(),
        })
}

/// The synthesized same-package workflow entries of a checked document, from
/// the shared shaping + planning passes (D-BC1: byte-identical child names
/// and contracts on both backends).
fn synthesized_workflow_entries(
    document: &Document,
    schema_root: &Path,
) -> Result<Vec<crate::SynthesizedWorkflowEntry>, CompileError> {
    let shaped = crate::emitter::shape_document(document, Some(schema_root)).map_err(|error| {
        CompileError::Planning {
            message: error.to_string(),
        }
    })?;
    let (emitter, plans) =
        crate::emitter::prepare(&shaped, Some(schema_root)).map_err(|error| {
            CompileError::Planning {
                message: error.to_string(),
            }
        })?;
    crate::emitter::synthesized_entries(&emitter, &plans).map_err(|error| CompileError::Planning {
        message: error.to_string(),
    })
}

/// Derive the effective action requirements of a parsed document: for each
/// declared action, the distinct effective node requirements across its call
/// sites (a call-site `node` override wins over the action declaration's),
/// in declaration order then call-site source order. An action with no call
/// sites carries its declared requirement.
#[must_use]
pub fn action_requirements(document: &Document) -> Vec<ActionRequirement> {
    let mut requirements = Vec::new();
    for worker in &document.workers {
        for action in &worker.actions {
            let declared = action
                .config
                .as_ref()
                .and_then(|config| config.node.as_ref())
                .map(|value| value.name.as_str());
            let mut nodes = Vec::new();
            for step in &document.steps {
                step_nodes(step, &action.name, declared, &mut nodes);
            }
            for subflow in &document.subflows {
                for step in &subflow.steps {
                    step_nodes(step, &action.name, declared, &mut nodes);
                }
            }
            if nodes.is_empty() {
                nodes.push(declared.map(str::to_owned));
            }
            for node in nodes {
                requirements.push(ActionRequirement {
                    task_queue: worker.name.clone(),
                    action: action.name.clone(),
                    node,
                });
            }
        }
    }
    requirements
}

fn step_nodes(step: &Step, action: &str, declared: Option<&str>, nodes: &mut Vec<Option<String>>) {
    statement_nodes(&step.body, action, declared, nodes);
    if let Some(on_failure) = &step.on_failure {
        statement_nodes(&on_failure.body, action, declared, nodes);
    }
}

fn statement_nodes(
    statements: &[Statement],
    action: &str,
    declared: Option<&str>,
    nodes: &mut Vec<Option<String>>,
) {
    for statement in statements {
        match statement {
            Statement::Call(call) => {
                if call.call.name == action {
                    let node = call
                        .config
                        .as_ref()
                        .and_then(|config| config.node.as_ref())
                        .map_or_else(
                            || declared.map(str::to_owned),
                            |value| Some(value.name.clone()),
                        );
                    push_unique(nodes, node);
                }
            }
            Statement::Pipe(pipe) => {
                for stage in &pipe.stages {
                    if let PipeStage::Action { name, .. } = stage
                        && name == action
                    {
                        push_unique(nodes, declared.map(str::to_owned));
                    }
                }
            }
            Statement::Fork(fork) => statement_nodes(&fork.body, action, declared, nodes),
            Statement::Loop(looped) => statement_nodes(&looped.body, action, declared, nodes),
            Statement::SubStep(sub) => step_nodes(sub, action, declared, nodes),
            // `spawn` starts a child workflow, never a worker action; the
            // region statements carry no calls.
            Statement::Spawn(_)
            | Statement::Wait(_)
            | Statement::Send(_)
            | Statement::Answer(_)
            | Statement::Sleep(_)
            | Statement::Route(_)
            | Statement::Distribute(_)
            | Statement::Collect(_) => {}
        }
    }
}

fn push_unique(nodes: &mut Vec<Option<String>>, node: Option<String>) {
    if !nodes.contains(&node) {
        nodes.push(node);
    }
}

/// Map a lowering failure into the public error, field-for-field.
fn lower_diagnostic(error: LowerError) -> CompileError {
    match error {
        LowerError::Unsupported { shape, span } => CompileError::Unsupported { shape, span },
        LowerError::Message { message, span } => CompileError::Lower { message, span },
        LowerError::Planning { message } => CompileError::Planning { message },
    }
}
