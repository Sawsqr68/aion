//! Which deployed package versions a registering worker must satisfy.
//!
//! # The contradiction this resolves
//!
//! Content-hash module namespacing (load-bearing invariant 5) exists so that
//! long-lived workflows can coexist with new deploys: every `.aion` package
//! version is a distinct immutable module, and the catalog retains them side by
//! side. Worker admission used to take that retained SET and demand that ONE
//! connection be contract-compatible with ALL of it simultaneously. But a
//! worker advertises exactly one shape per action, so the moment any historical
//! version on the queue declared a different shape for an action, no worker
//! could ever satisfy the whole set and the queue was permanently unservable.
//! Coexistence by design became mutual exclusion at the worker boundary.
//!
//! It is not hypothetical: on 2026-07-30 a stale, non-routed package whose
//! `verify_integration` action lacked a `base_branch` field made the
//! `staged_rounds` queue unservable. Every worker connection was refused with
//! `WORKER_CONTRACT_MISMATCH` on every dial, and the only way out was manually
//! unloading the stale version.
//!
//! # The rule
//!
//! A retained version binds a registering worker when a dispatch could actually
//! be produced under it — that is, when it is REACHABLE:
//!
//! - it is route-active, so a caller can start a workflow on it at any moment
//!   ([`AdmissionReason::RouteActive`]);
//! - a non-terminal run is registered on it ([`AdmissionReason::LiveWorkflow`]);
//! - an in-flight start has resolved it and not yet registered its run
//!   ([`AdmissionReason::StartInFlight`]).
//!
//! A version that is none of these cannot be entered by anything: new starts
//! resolve the routed version, and no existing run is pinned to it. Demanding a
//! worker satisfy it protects nothing and costs the whole queue.
//!
//! # What this read is and is not
//!
//! It is a synchronous, in-memory read. That is a hard requirement, not a
//! convenience: admission runs on the liminal connection process's frame-apply
//! callback (a beamr scheduler thread), where a durable scan would block the
//! scheduler that other connections' dispatches and completions are applied on.
//!
//! It is therefore a PRE-FLIGHT check over what could dispatch right now, never
//! a standing guarantee — and it never was one. A package deployed AFTER a
//! worker registered is never checked against that already-registered worker at
//! all, so "every registered worker satisfies every retained contract" has
//! never held. The gate's job is to catch an operator's mismatch at the moment
//! of connection, with a message precise enough to fix it.
//!
//! The one live run this read cannot see is a durably-`Paused` run that this
//! process has not resurrected — startup recovery deliberately does not respawn
//! a paused run, so it holds no registry handle until an operator resumes it.
//! Such a run's version is not demanded of a worker registering before the
//! resume. Closing that would require the engine to carry the paused runs'
//! pinned versions in memory across the `list_paused` rebuild.

use std::collections::HashSet;

use aion_package::ContentHash;

use crate::EngineError;
use crate::loader::DeployedWorkerContract;

use super::api::Engine;

/// Why one retained contract binds a registering worker.
///
/// The three variants are exactly the three conditions on which
/// [`Engine::unload_workflow_version`] refuses a version, so a version admission
/// demands is always a version unload will not remove.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AdmissionReason {
    /// The version routes new starts, so a workflow can begin on it at any
    /// moment — including between this check and the worker's first dispatch.
    RouteActive,
    /// At least one registered run with a non-terminal projected status is
    /// pinned to the version.
    LiveWorkflow,
    /// A start has resolved the version and has not yet registered its run.
    StartInFlight,
}

impl AdmissionReason {
    /// Operator-facing clause naming why the version was demanded.
    #[must_use]
    pub const fn explanation(self) -> &'static str {
        match self {
            Self::RouteActive => "it currently routes new starts",
            Self::LiveWorkflow => "a live workflow run is pinned to it",
            Self::StartInFlight => "a workflow start is in flight on it",
        }
    }
}

/// One retained contract a registering worker must satisfy, with the reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequiredContract {
    /// The retained queue contract under its exact package identity.
    pub contract: DeployedWorkerContract,
    /// Why this version is still reachable.
    pub reason: AdmissionReason,
}

/// One retained contract that binds nobody because nothing can reach it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnreachableContract {
    /// Exact package identity that was skipped.
    pub package_version: ContentHash,
    /// Workflow types this version implements, sorted.
    pub workflow_types: Vec<String>,
}

/// The admission decision inputs for one task queue.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QueueAdmission {
    /// Contracts a registering worker must satisfy, in stable identity order.
    pub required: Vec<RequiredContract>,
    /// Retained contracts nothing can reach, in stable identity order. Reported
    /// rather than dropped silently: an operator reading a refusal needs to
    /// know which retained versions were NOT held against the worker.
    pub unreachable: Vec<UnreachableContract>,
}

impl QueueAdmission {
    /// How many required contracts carry each reason, for refusal summaries.
    #[must_use]
    pub fn reason_census(&self) -> ReasonCensus {
        let mut census = ReasonCensus::default();
        for required in &self.required {
            match required.reason {
                AdmissionReason::RouteActive => census.route_active += 1,
                AdmissionReason::LiveWorkflow => census.live_workflow += 1,
                AdmissionReason::StartInFlight => census.start_in_flight += 1,
            }
        }
        census
    }
}

/// Counts of required contracts by reason.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReasonCensus {
    /// Versions demanded because they route new starts.
    pub route_active: usize,
    /// Versions demanded because a live run is pinned to them.
    pub live_workflow: usize,
    /// Versions demanded because a start is in flight on them.
    pub start_in_flight: usize,
}

impl Engine {
    /// Splits `task_queue`'s retained contracts into the set a registering
    /// worker must satisfy and the set nothing can reach.
    ///
    /// See the [module documentation](self) for the rule and its limits.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::CatalogPoisoned`] when the catalog snapshot or
    /// start-pin lock is poisoned, and [`EngineError::RegistryPoisoned`] when
    /// the active-execution registry lock is poisoned. Admission never guesses
    /// on a poisoned lock: an unreadable liveness answer refuses the worker
    /// rather than admitting one whose obligations are unknown.
    pub fn worker_contracts_for_admission(
        &self,
        task_queue: &str,
    ) -> Result<QueueAdmission, EngineError> {
        let catalog = self.workflow_catalog();
        let contracts = catalog.worker_contracts_for_queue(task_queue)?;
        if contracts.is_empty() {
            return Ok(QueueAdmission::default());
        }

        // Versions carrying a non-terminal registered run. A content hash is
        // matched on its own rather than paired with a workflow type: a hash
        // shared by an archive group's members is the SAME immutable package,
        // and matching wider can only ever demand more of a worker.
        let mut live = HashSet::new();
        for handle in self.registry().list()? {
            if !handle.cached_status().is_terminal() {
                live.insert(handle.loaded_version().clone());
            }
        }
        let starting = catalog
            .pinned_start_versions()?
            .into_iter()
            .map(|(_, version)| version)
            .collect::<HashSet<_>>();

        let mut admission = QueueAdmission::default();
        for contract in contracts {
            // Route-active is checked first: it is the only reason that holds
            // for a version with no run at all, and it is the reason an
            // operator can act on directly.
            let reason = if contract.route_active {
                Some(AdmissionReason::RouteActive)
            } else if live.contains(&contract.package_version) {
                Some(AdmissionReason::LiveWorkflow)
            } else if starting.contains(&contract.package_version) {
                Some(AdmissionReason::StartInFlight)
            } else {
                None
            };
            match reason {
                Some(reason) => admission
                    .required
                    .push(RequiredContract { contract, reason }),
                None => admission.unreachable.push(UnreachableContract {
                    package_version: contract.package_version,
                    workflow_types: contract.workflow_types,
                }),
            }
        }
        Ok(admission)
    }
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod admission_tests;
