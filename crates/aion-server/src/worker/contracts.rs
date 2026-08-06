//! Shared worker registration admission against durable package contracts.
//!
//! Admission holds a registering worker against the deployed `.v4` contracts
//! its queue can still dispatch under — the REACHABLE ones, chosen by
//! [`aion::Engine::worker_contracts_for_admission`]. Retained-but-unreachable
//! versions are named in a refusal but never held against the worker: demanding
//! them made one stale deploy enough to kill a whole task queue.

use std::collections::BTreeSet;

use aion::AdmissionReason;
use aion_package::ActivityDescriptor;

mod refusal;

use super::admission_audit::AdmissionAudit;
use refusal::render_mismatch;

/// What a registering worker announced about itself, in the TWO independent
/// forms it announces it in.
///
/// These are different sets and they are used for different things, which is
/// precisely why they must travel together. `activity_types` is the NAME set the
/// dispatcher selects on; `contracts` is the typed-descriptor set admission
/// compares against the deployed `.v4` contracts. A worker can advertise a name
/// with no contract behind it — it is then selectable but not admissible.
///
/// Before this pair existed, admission received only `contracts` while the
/// refusal log printed only `activity_types`, and on 2026-07-30 that produced a
/// refusal record which listed action `assistant` as advertised in the same line
/// that reported it `<missing>`. Both facts were true of different sets and the
/// record said which of neither. Carrying both through the gate makes the
/// refusal state the gap instead of contradicting itself.
#[derive(Clone, Copy, Debug)]
pub struct WorkerAdvertisement<'a> {
    /// Activity-type NAMES the worker advertised — the dispatcher's selection
    /// set.
    pub activity_types: &'a BTreeSet<String>,
    /// Typed contracts the worker advertised — the set admission compares.
    pub contracts: &'a [ActivityDescriptor],
}

impl WorkerAdvertisement<'_> {
    /// Advertised names carrying no typed contract, sorted.
    ///
    /// Non-empty means the worker is selectable for actions it cannot be
    /// admitted for, which is the exact shape of the 2026-07-30 refusal loop.
    #[must_use]
    pub fn names_without_contracts(&self) -> Vec<String> {
        let described = self
            .contracts
            .iter()
            .map(|contract| contract.name.as_str())
            .collect::<BTreeSet<_>>();
        self.activity_types
            .iter()
            .filter(|name| !described.contains(name.as_str()))
            .cloned()
            .collect()
    }
}

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
/// **Every refusal this gate returns, it also SAYS** — at WARN, on the server,
/// naming the queue, the node, the worker build and the reason, through
/// [`name_refusal`]. A caller may add transport detail of its own but must not
/// restate the refusal: the naming is the gate's, so no transport can be added
/// that forgets to do it.
///
/// # Errors
///
/// Returns [`ContractAdmissionError::Catalog`] when the deployed contracts or
/// the liveness they are judged against cannot be read — an unreadable answer
/// refuses rather than admits — and [`ContractAdmissionError::Mismatch`] with
/// the field-level diagnosis before the worker becomes dispatch-visible.
pub fn validate_worker_contracts(
    engine: &aion::Engine,
    audit: &AdmissionAudit,
    task_queue: &str,
    node: Option<&str>,
    identity: &str,
    advertised: WorkerAdvertisement<'_>,
) -> Result<(), ContractAdmissionError> {
    let admission = match engine.worker_contracts_for_admission(task_queue) {
        Ok(admission) => admission,
        Err(source) => {
            // An unreadable catalog refuses too, so it is named too. Leaving one
            // refusal branch voiceless is how this defect survived a fix: the
            // gRPC path logged the SKIPPED case and swallowed the verdict.
            let refusal = ContractAdmissionError::Catalog { source };
            name_refusal(audit, task_queue, node, identity, &refusal.to_string());
            return Err(refusal);
        }
    };
    let mut disagreements = Vec::new();
    for required in &admission.required {
        for diff in aion_package::contract_diffs(
            &required.contract.package_version.to_string(),
            &required.contract.contract,
            node,
            advertised.contracts,
        ) {
            disagreements.push(Disagreement {
                reason: required.reason,
                workflow_types: required.contract.workflow_types.clone(),
                diff,
            });
        }
    }
    if disagreements.is_empty() {
        // This site is being SERVED. Forget its last refusal so a worker that is
        // fixed, admitted, and then breaks again is heard immediately instead of
        // silenced by its own history.
        audit.clear_admitted(task_queue, node);
        return Ok(());
    }
    // Named from the ERROR's own rendering, never from `details` alone: the
    // `WORKER_CONTRACT_MISMATCH:` prefix is what makes a refusal greppable and
    // it lives in the Display. A log that paraphrases the error it is reporting
    // is the 2026-07-30 self-contradicting record in a new costume.
    let refusal = ContractAdmissionError::Mismatch {
        identity: identity.to_owned(),
        task_queue: task_queue.to_owned(),
        details: render_mismatch(&admission, node, advertised, &disagreements),
    };
    name_refusal(audit, task_queue, node, identity, &refusal.to_string());
    Err(refusal)
}

/// The ONE place a contract refusal is spoken, for every transport.
///
/// It lives at the gate rather than in a caller because that is the whole
/// finding behind #147: this exact defect was found, fixed and pinned on
/// 2026-07-29 — but in the liminal CALLER — and the gRPC caller re-manifested it
/// verbatim within a week, refusing every dial in silence while a `Rejected` ack
/// carried the reason nobody printed. One rule maintained in two places had
/// already drifted. A second copy of the log would have been the same mistake a
/// third time, so the naming moved INTO the gate and the liminal caller stopped
/// restating it.
///
/// Repeats are silent, and that is a transition rule rather than a rate limit:
/// see [`AdmissionAudit`] for why an identical redial carries no information and
/// why nothing here is a configurable cap.
fn name_refusal(
    audit: &AdmissionAudit,
    task_queue: &str,
    node: Option<&str>,
    identity: &str,
    reason: &str,
) {
    if !audit.should_name(task_queue, node, identity, reason) {
        return;
    }
    tracing::warn!(
        task_queue = %task_queue,
        node = ?node,
        identity = %identity,
        reason = %reason,
        "REFUSED worker registration"
    );
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
