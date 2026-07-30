//! Shared worker registration admission against durable package contracts.
//!
//! Admission holds a registering worker against the deployed `.v4` contracts
//! its queue can still dispatch under — the REACHABLE ones, chosen by
//! [`aion::Engine::worker_contracts_for_admission`]. Retained-but-unreachable
//! versions are named in a refusal but never held against the worker: demanding
//! them made one stale deploy enough to kill a whole task queue.

use aion::AdmissionReason;
use aion_package::ActivityDescriptor;

mod refusal;

use refusal::render_mismatch;

/// Typed refusal from worker contract admission.
#[derive(Debug, thiserror::Error)]
pub enum ContractAdmissionError {
    /// The durable package catalog could not be read.
    #[error("contract catalog lookup failed: {source}")]
    Catalog {
        /// Engine catalog failure.
        #[source]
        source: aion::EngineError,
    },
    /// One or more reachable `.v4` contracts differ from the worker surface.
    ///
    /// The whole diagnosis rides in `details`: which package version, why that
    /// version was still held against the worker, which action, which field,
    /// what was expected, what was advertised, and what the operator can do
    /// about it. A refused worker retries silently forever otherwise, and the
    /// message is the only thing standing between a two-minute fix and an hour
    /// of guessing.
    #[error(
        "WORKER_CONTRACT_MISMATCH: worker build `{identity}` queue `{task_queue}` refused. {details}"
    )]
    Mismatch {
        /// Stable worker-build identity.
        identity: String,
        /// Queue being admitted.
        task_queue: String,
        /// Full field-level diagnosis and operator remedy.
        details: String,
    },
}

/// Validates a worker against every REACHABLE `.v4` contract for its queue.
///
/// `node` is the connection's advertised locality, `None` when it carries none.
/// Callers holding a raw proto3 string normalize with
/// [`super::registry::optional_node`] — the SAME normalization the registry then
/// routes by, never a restatement of it, because admission decides which actions
/// a connection owes from exactly the locality the dispatcher will filter on.
///
/// Two independent narrowings apply, and both exist because a worker that is
/// doing its whole job must not be refused:
///
/// 1. Only REACHABLE package versions are held against the connection. Under
///    content-hash namespacing a queue retains every deployed version; one
///    that no longer routes and carries no live run can never produce a
///    dispatch, so demanding it protects nothing and costs the queue.
/// 2. Within a reachable version, only the actions whose dispatch can reach
///    this connection's node are demanded of it, because the server routes by
///    (namespace × `task_queue` × node) and a worker serving several nodes
///    therefore opens one connection PER NODE, each advertising only that
///    node's actions.
///
/// # Errors
///
/// Returns [`ContractAdmissionError::Catalog`] when the deployed contracts or
/// the liveness they are judged against cannot be read — an unreadable answer
/// refuses rather than admits — and [`ContractAdmissionError::Mismatch`] with
/// the field-level diagnosis before the worker becomes dispatch-visible.
pub fn validate_worker_contracts(
    engine: &aion::Engine,
    task_queue: &str,
    node: Option<&str>,
    identity: &str,
    advertised: &[ActivityDescriptor],
) -> Result<(), ContractAdmissionError> {
    let admission = engine
        .worker_contracts_for_admission(task_queue)
        .map_err(|source| ContractAdmissionError::Catalog { source })?;
    let mut disagreements = Vec::new();
    for required in &admission.required {
        for diff in aion_package::contract_diffs(
            &required.contract.package_version.to_string(),
            &required.contract.contract,
            node,
            advertised,
        ) {
            disagreements.push(Disagreement {
                reason: required.reason,
                workflow_types: required.contract.workflow_types.clone(),
                diff,
            });
        }
    }
    if disagreements.is_empty() {
        return Ok(());
    }
    Err(ContractAdmissionError::Mismatch {
        identity: identity.to_owned(),
        task_queue: task_queue.to_owned(),
        details: render_mismatch(&admission, node, &disagreements),
    })
}

/// One field-level disagreement, carrying why its version was demanded.
#[derive(Clone, Debug)]
struct Disagreement {
    /// Why the version this diff came from still binds the worker.
    reason: AdmissionReason,
    /// Workflow types the version implements, sorted.
    workflow_types: Vec<String>,
    /// The field-level difference itself.
    diff: aion_package::ContractDiff,
}

#[cfg(test)]
#[path = "contracts_tests.rs"]
mod contracts_tests;
