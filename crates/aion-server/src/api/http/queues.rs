//! The fleet-wide unserved-queue read.
//!
//! `GET /queues/unserved` answers "which addresses is nobody serving, why, and
//! which runs are stuck on them" from the live queue-service state the
//! dispatcher's selection wait publishes into
//! ([`crate::worker::QueueServiceState`]).
//!
//! The per-run half of the same question is on `POST /workflows/describe`; this
//! is the half an operator asks when they do not yet know WHICH run is stuck.

use axum::{Json, extract::State};
use serde::Serialize;

use super::auth::HttpCaller;
use super::error::HttpWireError;
use crate::ServerState;

/// One address nobody is serving, with everything parked on it.
#[derive(Debug, Serialize)]
pub(crate) struct UnservedQueueBody {
    /// Correctness/isolation boundary.
    namespace: String,
    /// Pool selector within the namespace.
    task_queue: String,
    /// Activity type that is unserved on it.
    activity_type: String,
    /// Canonical queue-service reason, in the vocabulary the dispatch refusals
    /// and the server logs use.
    reason: &'static str,
    /// One sentence naming what an operator has to fix.
    detail: String,
    /// Policy the parked dispatches are held under (`strict` / `durable_pending`).
    policy: &'static str,
    /// Workers connected for the pool, whatever they serve.
    workers_in_pool: u64,
    /// Of those, workers advertising this activity type.
    workers_serving_activity: u64,
    /// Of those, workers that also satisfy the parked dispatches' node pins.
    compatible_workers: u64,
    /// How long the address has been continuously unserved, in milliseconds.
    unserved_for_ms: u64,
    /// Every dispatch parked on it, longest-waiting first.
    waiting: Vec<UnservedDispatchBody>,
}

/// One run's activity parked on an unserved address.
#[derive(Debug, Serialize)]
pub(crate) struct UnservedDispatchBody {
    /// Owning workflow.
    workflow_id: String,
    /// Activity ordinal recorded in history.
    activity_id: String,
    /// Node this dispatch is pinned to, if any.
    node: Option<String>,
    /// How long this dispatch has been parked, in milliseconds.
    waiting_for_ms: u64,
}

/// `GET /queues/unserved`.
///
/// Namespace-filtered by the caller's grant, on the same existence-leak
/// boundary `GET /namespaces` enforces: a caller must never learn that a
/// namespace it cannot access exists, so an address it cannot access is dropped
/// rather than reported. An EMPTY list is the healthy answer, not an error.
pub(crate) async fn list_unserved_queues(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
) -> Result<Json<Vec<UnservedQueueBody>>, HttpWireError> {
    let unserved = state
        .unserved_queues()
        .map_err(|error| HttpWireError(error.to_wire_error()))?;
    let body = unserved
        .into_iter()
        .filter(|queue| caller.can_access(&queue.key.namespace))
        .map(project)
        .collect();
    Ok(Json(body))
}

/// Project one live entry onto the wire shape.
fn project(queue: crate::worker::UnservedQueue) -> UnservedQueueBody {
    let address = crate::worker::ServiceAddress {
        namespace: queue.key.namespace.clone(),
        task_queue: queue.key.task_queue.clone(),
        activity_type: queue.key.activity_type.clone(),
        // The node pin is per-dispatch, not per-address, so the address-level
        // sentence is the unpinned one and each pin is reported on its own
        // waiting entry.
        node: None,
    };
    UnservedQueueBody {
        reason: queue.reason.as_str(),
        detail: queue.reason.explain(&address),
        policy: queue.policy.as_str(),
        namespace: queue.key.namespace,
        task_queue: queue.key.task_queue,
        activity_type: queue.key.activity_type,
        workers_in_pool: count(queue.census.workers_in_pool),
        workers_serving_activity: count(queue.census.workers_serving_activity),
        compatible_workers: count(queue.census.compatible_workers),
        unserved_for_ms: millis(queue.unserved_for),
        waiting: queue
            .waiting
            .into_iter()
            .map(|dispatch| UnservedDispatchBody {
                workflow_id: dispatch.workflow_id.to_string(),
                activity_id: dispatch.activity_id.to_string(),
                node: dispatch.node,
                waiting_for_ms: millis(dispatch.waiting_for),
            })
            .collect(),
    }
}

/// Widen a census count to the wire width, saturating rather than wrapping.
fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

/// Milliseconds of a duration, saturating rather than wrapping.
fn millis(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "queues_tests.rs"]
mod tests;
