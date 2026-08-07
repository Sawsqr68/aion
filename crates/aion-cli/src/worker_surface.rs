//! The typed action surface a worker advertises, derived from an `.awl` document.
//!
//! Every worker the `aion` binary can serve — the manifest-wired shell worker and
//! the compiled-in agent harness alike — faces the same admission problem, so the
//! derivation lives here once rather than in each verb.
//!
//! # Why the document, and not the server
//!
//! The schemas come from the document through the SAME `aion_awl::compile` seam the
//! deploy path uses, so the worker's advertisement and the server's stored contract
//! are two INDEPENDENT derivations of one source. That is what lets contract
//! admission catch a worker still running an older document. A worker that instead
//! read the contract back off the server would only be comparing the server against
//! itself, which proves nothing.
//!
//! # Why the whole queue, or none of it
//!
//! A worker advertising no descriptors is refused by every queue carrying a deployed
//! contract, and AWL emits a contract for every `worker` block — so an empty
//! advertisement is refused everywhere a worker could be wanted. A PARTIAL one is
//! worse: the worker is admitted on the actions it did declare and then dispatched
//! one it never did. Both are refused here, by name, before the worker dials.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use aion_package::{ActivityDescriptor, PackageContract, WorkerContract};
use anyhow::{Context, Result, bail};

/// How a refusal names whatever decided which of a queue's actions get served.
///
/// The all-or-nothing rule is one rule, but the thing that broke it differs by
/// verb: a shell worker's manifest WIRES actions, an agent worker SERVES the ones
/// its document declares. Carrying the wording here keeps one implementation of the
/// rule without either verb's diagnostics reading as though it were the other.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ServingSource {
    /// The noun for whatever chose the served set ("manifest").
    pub(crate) subject: &'static str,
    /// The verb for "declares that it serves" ("wires").
    pub(crate) serves: &'static str,
    /// The negative verb phrase for "does not serve" ("does not wire").
    pub(crate) omits: &'static str,
}

/// Compiles `document` and returns the contract surface committed into package
/// identity — the same contract the deploy path derives.
///
/// # Errors
///
/// Returns an error naming the document when it cannot be read, and one carrying
/// the compiler's own diagnostics when it does not compile: a document that does
/// not compile has no action surface to derive, and saying so beats advertising
/// nothing and being refused remotely.
pub(crate) fn compile_contract(document: &Path) -> Result<PackageContract> {
    let source = std::fs::read_to_string(document)
        .with_context(|| format!("failed to read AWL document {}", document.display()))?;
    let compiled =
        aion_awl::compile(&source, crate::awl::document_root(document)).map_err(|error| {
            anyhow::anyhow!(
                "{} does not compile, so no action surface can be derived from it:\n{}",
                document.display(),
                crate::awl::compile_diagnostics(document, &error).join("\n")
            )
        })?;
    Ok(compiled.contract)
}

/// Resolves the ONE `worker` block whose queue this worker serves.
///
/// `requested` is an explicit queue selection (`requested_by` names where it came
/// from, for the refusal text). With no selection the queue is DERIVED: a document
/// declaring exactly one worker block leaves nothing to choose, which is
/// unambiguous derivation rather than an assumed default. A document declaring
/// several is genuinely ambiguous, so it is refused with every choice named.
///
/// # Errors
///
/// Returns an error when `requested` names a queue the document does not declare
/// (listing what it does declare), when the document declares no worker block at
/// all, or when it declares several and none was selected.
pub(crate) fn select_worker<'contract>(
    document: &Path,
    contract: &'contract PackageContract,
    requested: Option<&str>,
    requested_by: &str,
) -> Result<&'contract WorkerContract> {
    let declared = contract
        .workers
        .iter()
        .map(|candidate| candidate.task_queue.as_str())
        .collect::<Vec<_>>();
    match requested {
        Some(queue) => contract
            .workers
            .iter()
            .find(|candidate| candidate.task_queue == queue)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "{requested_by} serves queue `{queue}` but {} declares no such worker \
                     (it declares: {})",
                    document.display(),
                    if declared.is_empty() {
                        "none".to_owned()
                    } else {
                        declared.join(", ")
                    }
                )
            }),
        None => match contract.workers.as_slice() {
            [only] => Ok(only),
            [] => bail!(
                "{} declares no worker block, so there is no task queue to serve; \
                 a queue is the name of a `worker` block in the document",
                document.display()
            ),
            _ => bail!(
                "{} declares {} worker blocks, so which queue to serve is ambiguous; \
                 select one with {requested_by}: {}",
                document.display(),
                declared.len(),
                declared.join(", ")
            ),
        },
    }
}

/// The names of `worker`'s actions a WORKER may serve: every action whose body the
/// document does not carry.
///
/// An action WITH a body is executed by the server in-process from the deployed
/// contract, so it is neither this worker's to run nor its to advertise.
pub(crate) fn serviceable_action_names(worker: &WorkerContract) -> BTreeSet<String> {
    worker
        .actions
        .iter()
        .filter(|action| action.worker_owed())
        .map(|action| action.name.clone())
        .collect()
}

/// Reconciles the actions a worker intends to serve against the queue the document
/// declares, returning the typed descriptors to advertise, keyed by action name.
///
/// Three ways this refuses, each by name:
///
/// * `served` carries an action the queue does not declare — the worker would be
///   admitted for it and then dispatched a call the document never described.
/// * `served` carries an action whose body the document declares — the server runs
///   a declared body itself, and a worker claiming it would shadow the declared
///   implementation with a different one.
/// * the queue declares a serviceable action `served` omits — a partial
///   advertisement is admitted and then dispatched an action it never declared.
///
/// # Errors
///
/// Returns an error naming the offending action(s) and the rule they broke.
pub(crate) fn reconcile(
    document: &Path,
    worker: &WorkerContract,
    served: &BTreeSet<String>,
    source: ServingSource,
) -> Result<BTreeMap<String, ActivityDescriptor>> {
    let queue = worker.task_queue.as_str();
    let ServingSource {
        subject,
        serves,
        omits,
    } = source;
    let mut declared = worker
        .actions
        .iter()
        .filter(|action| action.worker_owed())
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
        .collect::<BTreeMap<_, _>>();

    let mut descriptors = BTreeMap::new();
    for action in served {
        let Some(descriptor) = declared.remove(action) else {
            let carries_body = worker
                .actions
                .iter()
                .any(|candidate| &candidate.name == action && !candidate.worker_owed());
            if carries_body {
                bail!(
                    "{subject} {serves} action `{action}`, but `{}` declares a body for it: \
                     the server runs a declared body itself and no worker serves it",
                    document.display()
                );
            }
            bail!(
                "{subject} {serves} action `{action}`, which queue `{queue}` in {} does not declare",
                document.display()
            );
        };
        descriptors.insert(action.clone(), descriptor);
    }
    if !declared.is_empty() {
        let unserved = declared.keys().cloned().collect::<Vec<_>>().join(", ");
        bail!(
            "queue `{queue}` in {} declares {} that the {subject} {omits}; \
             a worker must serve its whole queue or none of it, because a partial \
             advertisement is admitted and then dispatched an action it never declared",
            document.display(),
            if unserved.contains(", ") {
                format!("actions {unserved}")
            } else {
                format!("action `{unserved}`")
            }
        );
    }
    Ok(descriptors)
}
