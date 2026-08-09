//! `POST /workflows/describe-live` — the mid-step join.
//!
//! One call, four sources, one answer: the run's summary and projected status
//! from authoritative history, the CURRENT step folded from that same history,
//! the live fleet's verdict on whether the dispatch can be served, the live
//! attempt's owner and its volatile progress note, and a bounded tail of the
//! attempt's durable transcript.
//!
//! # Why a separate route rather than a flag on `/workflows/describe`
//!
//! `describe` answers "what does history record", is consumed by the ops console
//! against a frozen TS-exported response shape, and its cost is one history
//! read. This answers "what is happening right now", takes its own request knob
//! (the transcript tail bound), and reads live registries and the observability
//! keyspace as well. Bolting it on as an opt-in flag would make one response
//! type mean two different things depending on a boolean, and every reader would
//! have to know which one it was holding. Two routes, two shapes, no ambiguity.
//!
//! # Namespace scoping
//!
//! Identical to `describe`: the caller is scoped through
//! [`crate::NamespaceGuard`] on the describe operation before anything is read,
//! so a caller probing a foreign or unknown workflow gets the guard's anti-leak
//! answer and never a step, a note, or a transcript.

use aion_core::{
    ActivityId, AttemptLiveness, CurrentStep, DescribeLiveResponse, Event, HeartbeatNote,
    LiveAttempt, OpenStep, StepState, TranscriptStreamHead, TranscriptTail, WorkflowId,
};
use aion_proto::ProtoDescribeWorkflowRequest;
use aion_store::{ActivityRecord, ActivityStreamKey};
use axum::{Json, extract::State};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::auth::HttpCaller;
use super::clean_dtos::{optional_proto_run_id, proto_workflow_id};
use super::error::HttpWireError;
use super::payload::describe_response_to_ops_console;
use super::transcripts::transcript_head;
use crate::worker::{ActivityReachability, AttemptProgress, attempt_progress};
use crate::{ServerError, ServerState, api::handlers};

/// The live-describe request: the run to read, plus how much transcript tail to
/// include.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct DescribeLiveRequest {
    /// The namespace the target workflow runs under (the auth scope).
    pub namespace: String,
    /// The target workflow, as a plain UUID string.
    pub workflow_id: String,
    /// Optional run selector; omitted resolves the current run.
    #[serde(default)]
    pub run_id: Option<String>,
    /// How many of the current attempt's most recent retained transcript
    /// records to include.
    ///
    /// Omitted returns [`TranscriptTail::NotRequested`] — no tail is read and
    /// nothing is claimed about the stream. There is deliberately no default
    /// window: how much transcript a caller wants is the caller's decision, and
    /// a number invented here would be a cost every caller paid without asking.
    #[serde(default)]
    pub tail: Option<u32>,
}

/// `POST /workflows/describe-live`.
///
/// Returns `200` with the composed live view. Only an authorization failure, an
/// unknown workflow, or a store/registry fault is an HTTP error: every honest
/// "nothing to report" (no current step, no live owner, no note, no transcript)
/// is a typed part of the body.
pub(crate) async fn describe_live(
    State(state): State<ServerState>,
    HttpCaller(caller): HttpCaller,
    Json(request): Json<DescribeLiveRequest>,
) -> Result<Json<DescribeLiveResponse>, HttpWireError> {
    let tail = request.tail;
    let described = ProtoDescribeWorkflowRequest {
        namespace: request.namespace,
        workflow_id: Some(proto_workflow_id(&request.workflow_id).map_err(HttpWireError)?),
        run_id: optional_proto_run_id(request.run_id.as_deref()).map_err(HttpWireError)?,
        // The decoded history comes back on the outcome regardless; asking for
        // the ENCODED history would pay to serialize a payload this response
        // does not carry.
        include_history: false,
    };
    let outcome = handlers::describe(state.namespace_guard(), &caller, described)
        .await
        .map_err(HttpWireError)?;
    let summary = describe_response_to_ops_console(&outcome.response)?.summary;

    let unserved = ActivityReachability {
        registry: state.worker_registry(),
        declarations: state.queue_declarations(),
        state: state.queue_service_state(),
    }
    .unserved(&outcome.namespace, &outcome.workflow_id, &outcome.history)
    .map_err(|error| HttpWireError(error.to_wire_error()))?;

    let open_steps = aion_core::open_steps(&outcome.history);
    let current = aion_core::current_step(&outcome.history);
    let current_attempt = current.as_ref().and_then(dispatched_attempt);
    let current_step = match current {
        Some(step) => Some(CurrentStep {
            attempt: live_attempt(&state, &outcome.workflow_id, &step, tail).await?,
            step,
        }),
        None => None,
    };
    let transcript_streams = transcript_streams(
        &state,
        &outcome.workflow_id,
        &outcome.history,
        current_attempt,
    )
    .await?;

    Ok(Json(DescribeLiveResponse {
        summary,
        current_step,
        open_steps,
        transcript_streams,
        unserved,
    }))
}

/// The `(activity, attempt)` of a step the engine has dispatched, or `None` for
/// a step that has not been dispatched and therefore has no attempt.
fn dispatched_attempt(step: &OpenStep) -> Option<(ActivityId, u32)> {
    match step.state {
        StepState::Dispatched { attempt, .. } => Some((step.activity_id.clone(), attempt)),
        StepState::Scheduled | StepState::Reopened { .. } => None,
    }
}

/// The live view of a dispatched step's attempt: who owns it, what it last said,
/// and its transcript tail. `None` for a step with no attempt at all.
async fn live_attempt(
    state: &ServerState,
    workflow_id: &WorkflowId,
    step: &OpenStep,
    tail: Option<u32>,
) -> Result<Option<LiveAttempt>, HttpWireError> {
    let StepState::Dispatched {
        attempt,
        dispatched_at,
    } = step.state
    else {
        return Ok(None);
    };
    let key = ActivityStreamKey::new(workflow_id.clone(), step.activity_id.clone(), attempt);
    Ok(Some(LiveAttempt {
        attempt,
        dispatched_at,
        liveness: liveness(state, workflow_id, &step.activity_id, attempt)?,
        note: note(
            state,
            workflow_id,
            &step.activity_id,
            attempt,
            dispatched_at,
        )?,
        transcript: transcript(state, &key, tail).await?,
    }))
}

/// Whether a connected worker owns the attempt, read from the SAME attempt→owner
/// index `POST /workflows/attempts` enumerates and the intervention router
/// resolves against.
fn liveness(
    state: &ServerState,
    workflow_id: &WorkflowId,
    activity_id: &ActivityId,
    attempt: u32,
) -> Result<AttemptLiveness, HttpWireError> {
    let attempts = state
        .intervention_router()
        .intervenable_attempts(workflow_id)
        .map_err(|error| HttpWireError(error.to_wire_error()))?;
    let owned = attempts
        .into_iter()
        .find(|(key, _capabilities)| &key.activity_id == activity_id && key.attempt == attempt);
    Ok(match owned {
        Some((_key, capabilities)) => AttemptLiveness::Live { capabilities },
        None => AttemptLiveness::NoLiveOwner,
    })
}

/// The attempt's volatile progress note, projected onto the wire's typed
/// absence.
///
/// The one rule this function exists to keep: an unmeasurable note NEVER
/// becomes a silent one. [`AttemptProgress::Untracked`] carries no information
/// about what the worker reported, so it is reported as
/// [`HeartbeatNote::Unavailable`] with the evidence — when this process began
/// holding notes, and whether the attempt predates that.
fn note(
    state: &ServerState,
    workflow_id: &WorkflowId,
    activity_id: &ActivityId,
    attempt: u32,
    dispatched_at: DateTime<Utc>,
) -> Result<HeartbeatNote, HttpWireError> {
    let tracker = state.heartbeat_tracker();
    let progress = attempt_progress(tracker, workflow_id, activity_id, attempt)
        .map_err(|error| HttpWireError(error.to_wire_error()))?;
    Ok(match progress {
        AttemptProgress::Reported {
            payload,
            reported_at,
        } => HeartbeatNote::Reported {
            payload,
            reported_at,
        },
        AttemptProgress::NoneSent => HeartbeatNote::NoneSent,
        AttemptProgress::Untracked => {
            let notes_held_since = tracker.notes_held_since();
            HeartbeatNote::Unavailable {
                notes_held_since,
                restarted_since_attempt_began: dispatched_at < notes_held_since,
            }
        }
    })
}

/// The bounded tail of one attempt's retained transcript.
///
/// An unrequested tail reads nothing at all — [`TranscriptTail::NotRequested`]
/// claims nothing about the stream, which is different from claiming it is
/// empty.
async fn transcript(
    state: &ServerState,
    key: &ActivityStreamKey,
    tail: Option<u32>,
) -> Result<TranscriptTail, HttpWireError> {
    let Some(tail) = tail else {
        return Ok(TranscriptTail::NotRequested);
    };
    let mut records = state
        .transcript_publisher()
        .replay_from(key, 0)
        .await
        .map_err(|error| HttpWireError(ServerError::from(error).to_wire_error()))?;
    let head_seq = transcript_head(&records)?;
    let tail = usize::try_from(tail).unwrap_or(usize::MAX);
    let omitted = records.len().saturating_sub(tail);
    drop(records.drain(..omitted));
    Ok(TranscriptTail::Window {
        events: records
            .into_iter()
            .map(|record: ActivityRecord| record.event)
            .collect(),
        head_seq,
        omitted_before: u64::try_from(omitted).unwrap_or(u64::MAX),
        retention_truncated: retention_truncated(head_seq, retention_cap(state)),
    })
}

/// Every retained transcript stream of the workflow, annotated with the activity
/// type it belongs to and whether it is the current attempt's — the two facts a
/// bare `(activity, attempt, head)` enumeration leaves a reader to guess.
async fn transcript_streams(
    state: &ServerState,
    workflow_id: &WorkflowId,
    history: &[Event],
    current: Option<(ActivityId, u32)>,
) -> Result<Vec<TranscriptStreamHead>, HttpWireError> {
    let summaries = state
        .transcript_publisher()
        .list_streams(workflow_id)
        .await
        .map_err(|error| HttpWireError(ServerError::from(error).to_wire_error()))?;
    let cap = retention_cap(state);
    Ok(summaries
        .into_iter()
        .map(|summary| TranscriptStreamHead {
            current: current.as_ref().is_some_and(|(activity_id, attempt)| {
                activity_id == &summary.key.activity_id && *attempt == summary.key.attempt
            }),
            activity_type: activity_type_of(history, &summary.key.activity_id),
            retention_truncated: retention_truncated(summary.head, cap),
            activity_id: summary.key.activity_id,
            attempt: summary.key.attempt,
            head_seq: summary.head,
        })
        .collect())
}

/// The operator-configured per-stream retention cap (`[observability]`).
fn retention_cap(state: &ServerState) -> u64 {
    state.runtime_config().observability.max_stream_events
}

/// Whether the retention cap has closed a stream.
///
/// The publisher persists a marker record AT the cap sequence and nothing after
/// it, so a head past the cap is the durable evidence that persistence stopped
/// while the agent kept talking. Computed in ONE place and used for both the
/// stream heads and the tail, so the two can never disagree about a stream.
const fn retention_truncated(head_seq: u64, cap: u64) -> bool {
    head_seq > cap
}

/// The activity type of an ordinal, from the run's own history.
///
/// Scans the WHOLE history rather than the active segment: a retained stream can
/// belong to an activity of an earlier segment, and answering `None` for it
/// because the current segment does not mention it would hide a type this
/// history plainly records.
fn activity_type_of(history: &[Event], activity_id: &ActivityId) -> Option<String> {
    history.iter().rev().find_map(|event| match event {
        Event::ActivityScheduled {
            activity_id: scheduled,
            activity_type,
            ..
        } if scheduled == activity_id => Some(activity_type.clone()),
        _ => None,
    })
}

#[cfg(test)]
#[path = "describe_live_tests.rs"]
mod tests;
