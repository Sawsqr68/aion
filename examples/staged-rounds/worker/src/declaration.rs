//! The AWL DOCUMENT as this worker's one source of advertisement truth.
//!
//! WHAT THIS EXISTS FOR. A worker that registers a handler and no wire
//! DESCRIPTOR advertises nothing, and a queue carrying any deployed contract
//! refuses it: the server's admission gate (`aion_package::contract_diffs`)
//! demands, for every bodyless action a dispatch could reach the connection
//! with, an advertised descriptor whose input schema accepts everything the
//! workflow may send and whose output schema is decodable by the workflow. An
//! unadvertised action is reported as `<missing>` and the whole connection is
//! rejected with `WORKER_CONTRACT_MISMATCH`. That happened live on 2026-07-30:
//! all five of this worker's connections were refused on every dial.
//!
//! WHY THE DOCUMENT AND NOT THE RUST TYPES. The schemas the server admits
//! against are the ones `aion_awl::compile` derived from
//! `../../awl/staged_rounds.awl` and committed into package identity. Deriving
//! the advertisement from the handlers' Rust types instead (with `schemars`)
//! would produce a SECOND, independent rendering of the same shapes — and the
//! contract gate exists precisely to catch the drift between two such
//! renderings. Sourcing the advertisement from the compiled contract makes the
//! drift structurally impossible: the bytes advertised are the bytes deployed.
//!
//! The document is embedded with `include_str!`, so the binary carries the
//! declaration it was built from and cannot be run against a document it never
//! saw.

use std::collections::BTreeSet;
use std::path::Path;

use aion_package::{ActivityDescriptor, PackageContract, WorkerContract};

#[path = "declaration/tests.rs"]
#[cfg(test)]
mod tests;

/// The staged-rounds document, embedded at compile time. This binary
/// advertises what THIS text declares — nothing else.
pub const DOCUMENT: &str = include_str!("../../awl/staged_rounds.awl");

/// The document's own directory, as it stood at build time. `aion_awl::compile`
/// resolves a document's schema imports relative to it; the staged-rounds
/// document declares every type inline, so nothing is read from here today, and
/// a document that grew an import would fail compilation with a message naming
/// the path rather than silently deriving a different schema.
const DOCUMENT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../awl");

/// The one task queue every staged-rounds activity is dispatched on — the
/// document's `worker` block name, which the deploy path turns into the task
/// queue.
pub const TASK_QUEUE: &str = "staged_rounds";

/// The compiled `worker staged_rounds` contract: every action the document
/// declares, with the schemas and the node pin the server admits against.
#[derive(Clone, Debug)]
pub struct Declaration {
    contract: WorkerContract,
}

impl Declaration {
    /// Compile the embedded document and take its `staged_rounds` worker
    /// contract.
    ///
    /// This is the production path, run ONCE at startup: everything the worker
    /// advertises is derived from the value it returns.
    ///
    /// # Errors
    ///
    /// A message carrying the compiler's own diagnostics when the embedded
    /// document does not compile, or naming the workers the document does
    /// declare when it declares no `staged_rounds` worker.
    pub fn compile() -> Result<Self, String> {
        let compiled = aion_awl::compile(DOCUMENT, Path::new(DOCUMENT_DIR)).map_err(|error| {
            format!("the embedded staged-rounds document does not compile: {error}")
        })?;
        Ok(Self {
            contract: select_worker(compiled.contract)?,
        })
    }

    /// Build a declaration from an already-compiled worker contract.
    ///
    /// [`Self::compile`] funnels through this seam, and it is what lets the
    /// guards be driven against a contract of a deliberately wrong shape.
    #[must_use]
    pub const fn from_worker_contract(contract: WorkerContract) -> Self {
        Self { contract }
    }

    /// The compiled contract, exactly as the server admits against it.
    #[must_use]
    pub const fn contract(&self) -> &WorkerContract {
        &self.contract
    }

    /// The DECLARED wire descriptor for one action a connection on `node`
    /// registers a handler for.
    ///
    /// # Errors
    ///
    /// A message naming the action when the document's `staged_rounds` worker
    /// declares no action of that name, or when it declares one pinned to a
    /// DIFFERENT node — a handler registered on a node the action is not pinned
    /// to can never be dispatched to, because the server routes by (namespace,
    /// `task_queue`, node) alone.
    pub fn descriptor(&self, action: &str, node: &str) -> Result<ActivityDescriptor, String> {
        let declared = self
            .contract
            .actions
            .iter()
            .find(|declared| declared.name == action)
            .ok_or_else(|| {
                format!(
                    "this binary registers a handler for activity `{action}`, which the \
                     document's `worker {TASK_QUEUE}` block does not declare; it declares: \
                     [{}]",
                    self.declared_action_names().join(", ")
                )
            })?;
        if let Some(pin) = declared.node.as_deref()
            && pin != node
        {
            return Err(format!(
                "this binary registers a handler for activity `{action}` on node `{node}`, \
                 but the document pins that action to node `{pin}`; the server routes by \
                 (namespace, task_queue, node) alone, so no dispatch could ever reach the \
                 handler"
            ));
        }
        Ok(ActivityDescriptor {
            name: declared.name.clone(),
            input_schema: declared.input_schema.clone(),
            output_schema: declared.output_schema.clone(),
        })
    }

    /// Require every declared action that NEEDS an out-of-band worker to be
    /// served by this binary.
    ///
    /// An action whose declaration carries a `body` is executed by the SERVER
    /// itself, so no worker serves it and none is demanded here — the same
    /// exclusion `aion_package::contract_diffs` applies when it filters
    /// `body.is_none()` before deciding what a connection owes.
    ///
    /// # Errors
    ///
    /// A message naming every unserved bodyless action AND its declared node,
    /// because the node is what tells the operator which connection owed it.
    pub fn require_every_bodyless_action_served(
        &self,
        served: &BTreeSet<String>,
    ) -> Result<(), String> {
        let missing = self
            .contract
            .actions
            .iter()
            .filter(|action| action.body.is_none())
            .filter(|action| !served.contains(&action.name))
            .map(|action| {
                format!(
                    "`{}` (declared on node {})",
                    action.name,
                    action.node.as_deref().map_or_else(
                        || "any — the action is unpinned".to_owned(),
                        |node| format!("`{node}`"),
                    )
                )
            })
            .collect::<Vec<_>>();
        if missing.is_empty() {
            return Ok(());
        }
        Err(format!(
            "the document declares {} action(s) with no body — each REQUIRES an out-of-band \
             worker — that this binary registers no handler for: {}. The queue's contract \
             cannot be served by this build, so it refuses to start rather than leave those \
             dispatches parked forever.",
            missing.len(),
            missing.join(", ")
        ))
    }

    /// The declared action names, quoted, in declaration order.
    fn declared_action_names(&self) -> Vec<String> {
        self.contract
            .actions
            .iter()
            .map(|action| format!("`{}`", action.name))
            .collect()
    }
}

/// Take the `staged_rounds` worker contract out of a compiled package contract.
///
/// # Errors
///
/// A message naming every worker the contract DOES declare when it declares no
/// `staged_rounds` worker — a worker with nothing to advertise must refuse to
/// start, not start advertising nothing.
fn select_worker(contract: PackageContract) -> Result<WorkerContract, String> {
    let declared = contract
        .workers
        .iter()
        .map(|worker| format!("`{}`", worker.task_queue))
        .collect::<Vec<_>>()
        .join(", ");
    contract
        .workers
        .into_iter()
        .find(|worker| worker.task_queue == TASK_QUEUE)
        .ok_or_else(|| {
            format!(
                "the compiled document declares no `worker {TASK_QUEUE}` block, so this \
                 binary has no declared action to advertise; it declares: [{declared}]"
            )
        })
}
