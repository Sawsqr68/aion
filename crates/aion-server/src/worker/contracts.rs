//! Shared worker registration admission against durable package contracts.

use aion_package::ActivityDescriptor;

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
    /// One or more deployed `.v4` contracts differ from the worker surface.
    #[error(
        "WORKER_CONTRACT_MISMATCH: worker build `{identity}` queue `{task_queue}` refused: {details}"
    )]
    Mismatch {
        /// Stable worker-build identity.
        identity: String,
        /// Queue being admitted.
        task_queue: String,
        /// Stable field-level differences across exact package identities.
        details: String,
    },
}

/// Validates a worker against every retained `.v4` contract for its queue.
///
/// `node` is the connection's advertised locality, `None` when it carries none.
/// Callers holding a raw proto3 string normalize with
/// [`super::registry::optional_node`] — the SAME normalization the registry then
/// routes by, never a restatement of it, because admission decides which actions
/// a connection owes from exactly the locality the dispatcher will filter on.
///
/// Only the actions whose dispatch can reach this connection's node are demanded
/// of it, because the server routes by (namespace × `task_queue` × node) and a
/// worker serving several nodes therefore opens one connection PER NODE, each
/// advertising only that node's actions. Demanding the whole queue of every
/// connection refused each one for omitting the others' actions, so a correctly
/// partitioned worker could never register at all.
///
/// # Errors
///
/// Returns [`ContractAdmissionError::Catalog`] when durable contract records
/// cannot be read, or [`ContractAdmissionError::Mismatch`] with field-level
/// differences before the worker becomes dispatch-visible.
pub fn validate_worker_contracts(
    engine: &aion::Engine,
    task_queue: &str,
    node: Option<&str>,
    identity: &str,
    advertised: &[ActivityDescriptor],
) -> Result<(), ContractAdmissionError> {
    let contracts = engine
        .worker_contracts_for_queue(task_queue)
        .map_err(|source| ContractAdmissionError::Catalog { source })?;
    let mut diffs = Vec::new();
    for contract in contracts {
        diffs.extend(aion_package::contract_diffs(
            &contract.package_version.to_string(),
            &contract.contract,
            node,
            advertised,
        ));
    }
    if diffs.is_empty() {
        return Ok(());
    }
    let details = diffs
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ");
    Err(ContractAdmissionError::Mismatch {
        identity: identity.to_owned(),
        task_queue: task_queue.to_owned(),
        details,
    })
}
