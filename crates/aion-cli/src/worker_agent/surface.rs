//! The action surface an agent worker advertises, derived from its `.awl` document.
//!
//! An agent activity's output is HARNESS-owned — the agent decides what it produces
//! — so there is no Rust type to derive a schema from, which is exactly why
//! `AgentHarnessConfig::with_activity_descriptors` exists. The document is the only
//! place the schemas can come from, and it is the same place the deployed contract
//! came from: see [`crate::worker_surface`] for why that independence is the point.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

use aion_package::{ActivityDescriptor, CanonicalJson};
use anyhow::{Context, Result, bail};
use serde_json::json;

use crate::worker_surface::{self, ServingSource};

/// How a refusal names an agent worker as the thing serving a queue's actions.
const AGENT_SOURCE: ServingSource = ServingSource {
    subject: "the agent worker",
    serves: "serves",
    omits: "does not serve",
};

/// The queue and the typed actions a worker built from one document would announce.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AdvertisedSurface {
    /// The queue the document declares and this worker serves.
    pub(crate) task_queue: String,
    /// The typed actions, ordered by name so two runs advertise identically.
    pub(crate) descriptors: Vec<ActivityDescriptor>,
}

impl AdvertisedSurface {
    /// The action-type names this worker announces — the set the server selects it
    /// for, and therefore the set the harness must be prepared to be dispatched.
    pub(crate) fn names(&self) -> BTreeSet<String> {
        self.descriptors
            .iter()
            .map(|descriptor| descriptor.name.clone())
            .collect()
    }
}

/// Derives the surface an agent worker would advertise from `document`.
///
/// `requested_queue` is an explicit selection; without one the queue is derived
/// from the document, which is unambiguous only when it declares exactly one
/// `worker` block.
///
/// # Errors
///
/// Returns an error when the document cannot be read or compiled, when the queue
/// cannot be resolved unambiguously, when the resolved queue leaves NOTHING for a
/// worker to serve, or when the served set and the queue's declaration disagree.
pub(crate) fn derive(document: &Path, requested_queue: Option<&str>) -> Result<AdvertisedSurface> {
    let contract = worker_surface::compile_contract(document)?;
    let worker =
        worker_surface::select_worker(document, &contract, requested_queue, "--task-queue")?;
    let served = worker_surface::serviceable_action_names(worker);
    // An empty advertisement is refused by every queue carrying a deployed
    // contract, and AWL emits a contract for every worker block — so a worker that
    // got this far and advertised nothing would be refused remotely, with the
    // reason living on the far side of a socket. Refuse here instead, and say
    // which of the two ways it happened.
    if served.is_empty() {
        let queue = &worker.task_queue;
        if worker.actions.is_empty() {
            bail!(
                "queue `{queue}` in {} declares no actions, so an agent worker on it would \
                 advertise nothing and every queue carrying a deployed contract would refuse it",
                document.display()
            );
        }
        let bodied = worker
            .actions
            .iter()
            .map(|action| action.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        bail!(
            "every action on queue `{queue}` in {} carries a declared body ({bodied}), which the \
             server runs itself; there is nothing on this queue for an agent worker to serve",
            document.display()
        );
    }
    let descriptors = worker_surface::reconcile(document, worker, &served, AGENT_SOURCE)?;
    Ok(AdvertisedSurface {
        task_queue: worker.task_queue.clone(),
        descriptors: descriptors.into_values().collect(),
    })
}

/// Writes the exact surface this worker would advertise, plus the harness that
/// would drive it, as JSON.
///
/// The descriptors ARE the admission decision on a contract-bearing queue, and the
/// compiled-in harness decides whether the worker can serve at all — so both belong
/// in a report an operator reads instead of a remote refusal.
///
/// # Errors
///
/// Returns an error when the surface cannot be encoded or the writer cannot be
/// written to (a closed pipe).
pub(crate) fn print(writer: &mut impl Write, surface: &AdvertisedSurface) -> Result<()> {
    let report = json!({
        "task_queue": surface.task_queue,
        "harness": crate::harness::compiled_harness_name(),
        "intervention_capabilities": crate::harness::compiled_harness_capabilities(),
        "actions": surface
            .descriptors
            .iter()
            .map(|descriptor| {
                json!({
                    "name": descriptor.name,
                    "input_schema": descriptor.input_schema,
                    "output_schema": descriptor.output_schema,
                })
            })
            .collect::<Vec<_>>(),
    });
    // Canonically encoded: the advertised schemas are documents an operator
    // diffs against a package contract, so their key order is the schemas' own
    // content and never the build's `serde_json` map representation.
    serde_json::to_writer_pretty(&mut *writer, &CanonicalJson::new(report))
        .context("failed to encode the advertised action surface")?;
    writeln!(writer).context("failed to write the advertised action surface")?;
    Ok(())
}
