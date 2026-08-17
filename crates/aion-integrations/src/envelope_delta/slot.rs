//! The identity a turn's base envelope is remembered under.
//!
//! Encoder and decoder must agree exactly on this, or the decoder will look for a base the encoder
//! filed somewhere else. It is defined once, here, and both sides derive it from the same event
//! fields.

use aion_core::{ActivityEvent, ActivityId, RunId, WorkflowId};
use serde_json::Value;
use uuid::Uuid;

use super::wire;

/// The transcript stream one turn belongs to.
///
/// A single activity attempt can run several agents, and each agent has its own sequence of turns,
/// so the producing agent is part of the identity. Without it, two agents' turns would displace
/// each other's bases.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct StreamSlot {
    workflow_id: WorkflowId,
    run_id: RunId,
    activity_id: ActivityId,
    attempt: u32,
    agent_id: Uuid,
}

impl StreamSlot {
    /// The slot the given event belongs to.
    pub(super) fn of(event: &ActivityEvent) -> Self {
        Self {
            workflow_id: event.workflow_id.clone(),
            run_id: event.run_id.clone(),
            activity_id: event.activity_id.clone(),
            attempt: event.attempt,
            agent_id: event.agent_id,
        }
    }
}

/// The base envelope a stream's current turn is being diffed against.
///
/// Exactly one is held per stream, not one per turn: a turn's frames are contiguous, so a frame
/// bearing a new response identifier means the previous turn is over and its base is dead weight.
/// This bounds what encoder and decoder retain to the number of live streams — no eviction policy,
/// no retention window, and nothing that grows with the length of a run.
#[derive(Clone, Debug)]
pub(super) struct RecordedBase {
    /// The provider response identifier this base belongs to.
    pub(super) response_id: String,
    /// The `worker_seq` of the event that carried it.
    pub(super) worker_seq: u64,
    /// The base envelope, verbatim.
    pub(super) value: Value,
    /// The digest of the base's serialized bytes, computed once when it is recorded.
    pub(super) digest: String,
}

impl RecordedBase {
    /// Records a frame as a turn's base, or `None` when its digest cannot be computed (in which
    /// case the caller must not treat it as a base, because no later delta could be verified
    /// against it).
    pub(super) fn record_parts(response_id: &str, worker_seq: u64, value: &Value) -> Option<Self> {
        Some(Self {
            response_id: response_id.to_owned(),
            worker_seq,
            value: value.clone(),
            digest: wire::value_digest(value)?,
        })
    }
}
