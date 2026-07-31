//! The fleet-wide degraded-residency read (#117).
//!
//! `GET /workflows/unrecoverable` answers "which runs did this engine fail to
//! bring back, and why" from the live per-process set startup recovery records
//! into ([`aion::registry::UnrecoverableRuns`]).
//!
//! # Why this endpoint had to exist
//!
//! The per-run half of the same question was already answerable: `POST
//! /workflows/describe` reports a run's degraded residency once you can name the
//! run. That is exactly the thing an operator cannot do here. A run that startup
//! recovery skipped is skipped SILENTLY as far as every read surface is
//! concerned — it still projects `Running`, it still lists, it still describes —
//! and the only place its id was ever named as degraded was one ERROR line at
//! boot, in a stream that has since scrolled away. Cancelling it is now
//! possible (#117(c)), but a lever you cannot aim is not a remedy. This is the
//! read that hands over the id.
//!
//! # It reports a live condition, not a scar list
//!
//! Entries clear the moment the engine observes the run resident, so an EMPTY
//! response is the healthy answer and a non-empty one is always about right now.
//! A degraded flag that outlived its degradation would send an operator to
//! redeploy a run that is running fine, which is worse than no flag at all.

use axum::{Json, extract::State};
use serde::Serialize;

use super::auth::HttpCaller;
use super::error::HttpWireError;
use crate::{CallerIdentity, ServerState};

/// One run this engine process could not make resident.
#[derive(Debug, Serialize)]
pub(crate) struct UnrecoverableRunBody {
    /// The degraded run's workflow id — the value every remedy needs and the
    /// reason this endpoint exists.
    workflow_id: String,
    /// The owning namespace recorded in the run's own history, or `null` for a
    /// run that recorded no namespace attribute at all. `null` is a fact about
    /// the run, not a denial: a caller who cannot access a namespace never sees
    /// its runs in this list in the first place.
    namespace: Option<String>,
    /// The run's workflow type, as recovery read it from `WorkflowStarted`.
    workflow_type: String,
    /// The typed recovery failure, rendered — for the identity-domain case, the
    /// pinned package version that could not be loaded.
    reason: String,
    /// When THIS engine process observed the failure, RFC 3339. Not a workflow
    /// timestamp: engine-operational observations are outside the determinism
    /// boundary, so wall-clock is the correct clock here.
    observed_at: String,
    /// What an operator can do about it, in one sentence.
    remedy: &'static str,
}

/// The single remedy sentence, stated once.
///
/// Both remedies are real and they are ordered: redeploying the pinned version
/// makes the run recoverable on the next boot and loses nothing, so it comes
/// first; cancelling is the terminal answer for a run whose code is gone for
/// good. Naming only the second would push operators toward destroying runs
/// that a redeploy would have saved.
const REMEDY: &str = "redeploy the pinned package version to make this run recoverable, or cancel \
                      the run to terminate it — a run in this list is not running and will not \
                      resume under this build";

/// `GET /workflows/unrecoverable`.
///
/// Namespace-filtered by the caller's grant, on the same existence-leak boundary
/// `GET /namespaces` and `GET /queues/unserved` enforce: a caller must never
/// learn that a namespace it cannot access exists, so a run it cannot access is
/// dropped rather than reported. An EMPTY list is the healthy answer, not an
/// error.
///
/// Rows are returned oldest observation first. The backing set is an unordered
/// map, so SOME order has to be chosen; observation order is the one an operator
/// reading a degraded list is actually looking for, and choosing it here keeps
/// the arbitrariness out of the storage layer.
pub(crate) async fn list_unrecoverable_runs(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
) -> Result<Json<Vec<UnrecoverableRunBody>>, HttpWireError> {
    collect(&state, &caller)
        .await
        .map(Json)
        .map_err(|error| HttpWireError(error.to_wire_error()))
}

/// The read and the grant filter, apart from the transport.
///
/// Extracted so the namespace boundary can be tested against a caller with a
/// specific grant. Going through the HTTP surface for that would mean minting a
/// scoped credential per case, which tests the token path rather than the filter
/// this function is responsible for.
pub(crate) async fn collect(
    state: &ServerState,
    caller: &CallerIdentity,
) -> Result<Vec<UnrecoverableRunBody>, crate::ServerError> {
    let resolver = state.namespace_guard().resolver();
    let mut rows = Vec::new();
    for (workflow_id, entry) in state.unrecoverable_runs()? {
        // The sweep starts from ids and does not know their namespaces, so the
        // attribution is read unscoped and the grant filter is applied here.
        let namespace = resolver
            .recorded_workflow_attribution(&workflow_id)
            .await?
            .map(|attribution| attribution.namespace);
        if !visible_to(caller, namespace.as_deref()) {
            continue;
        }
        rows.push(UnrecoverableRunBody {
            workflow_id: workflow_id.to_string(),
            namespace,
            workflow_type: entry.workflow_type,
            reason: entry.reason,
            observed_at: entry.observed_at.to_rfc3339(),
            remedy: REMEDY,
        });
    }
    rows.sort_by(|left, right| {
        left.observed_at
            .cmp(&right.observed_at)
            .then_with(|| left.workflow_id.cmp(&right.workflow_id))
    });
    Ok(rows)
}

/// Whether this caller may be told the run exists.
///
/// An attributed run is visible on the ordinary grant check. An UNATTRIBUTED
/// run — one whose history carries no owning namespace — is visible only to a
/// caller holding every namespace.
///
/// Both halves matter and they pull in opposite directions. Hiding unattributed
/// runs from everyone would recreate the defect for exactly the runs most likely
/// to hit it: an unattributed run belongs to no tenant, so no enumerated grant
/// would ever match it and it would be permanently invisible to the only person
/// who can fix it. Showing them to enumerated callers would leak the existence
/// of runs outside their tenancy. The all-namespaces operator is the one caller
/// for whom neither is true.
fn visible_to(caller: &CallerIdentity, namespace: Option<&str>) -> bool {
    namespace.map_or_else(|| caller.all_namespaces(), |name| caller.can_access(name))
}

#[cfg(test)]
#[path = "unrecoverable_tests.rs"]
mod tests;
