//! The three note states, each reached by the situation that produces it.
//!
//! Every test names the situation rather than the expected variant, because the
//! whole point of the type is that a reader can tell the situations apart — a
//! test that only asserted the variant would pass on an implementation that
//! answered the same way for all three.

use std::time::{Duration, Instant};

use aion_core::{ActivityId, Payload, WorkflowId};
use aion_proto::{ProtoActivityId, ProtoHeartbeat, ProtoPayload, ProtoWorkflowId};
use serde_json::json;
use uuid::Uuid;

use super::{AttemptProgress, attempt_progress};
use crate::error::ServerError;
use crate::worker::heartbeat::{HeartbeatTracker, InFlightActivity};
use crate::worker::registry::{ConnectedWorkerRegistry, WorkerId};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const ATTEMPT: u32 = 1;

fn workflow_id() -> WorkflowId {
    WorkflowId::new(Uuid::from_u128(7))
}

fn activity_id() -> ActivityId {
    ActivityId::from_sequence_position(4)
}

/// Register one worker in `registry` and return its id with its registration
/// guard (dropping the guard deregisters the worker).
///
/// Worker ids are allocated by the registry, so two workers that must be
/// DISTINCT have to come from the SAME registry — two registries each hand out
/// their own first id, and a test using two registries would silently be
/// testing one worker talking to itself.
fn register(
    registry: &ConnectedWorkerRegistry,
) -> Result<(crate::worker::WorkerRegistration, WorkerId), ServerError> {
    let (sender, _receiver) = tokio::sync::mpsc::channel(1);
    let activity_types = [String::from("review")];
    let registration = registry.register("tenant-a", activity_types.iter(), sender)?;
    let worker_id = registration
        .worker_id()
        .ok_or_else(|| ServerError::lock_poisoned("test worker registration"))?;
    Ok((registration, worker_id))
}

/// A registered worker id in a fresh registry, for the single-worker cases.
fn worker() -> Result<
    (
        ConnectedWorkerRegistry,
        crate::worker::WorkerRegistration,
        WorkerId,
    ),
    ServerError,
> {
    let registry = ConnectedWorkerRegistry::default();
    let (registration, worker_id) = register(&registry)?;
    Ok((registry, registration, worker_id))
}

fn track(
    tracker: &HeartbeatTracker,
    worker_id: WorkerId,
    attempt: u32,
    now: Instant,
) -> Result<(), ServerError> {
    tracker.track_task(
        worker_id,
        InFlightActivity {
            workflow_id: workflow_id(),
            activity_id: activity_id(),
            attempt,
            completion_token: crate::worker::CompletionToken::for_test(),
        },
        now,
    )
}

fn note(value: &serde_json::Value) -> Result<Payload, aion_core::PayloadError> {
    Payload::from_json(value)
}

fn heartbeat(progress: Option<Payload>) -> ProtoHeartbeat {
    ProtoHeartbeat {
        workflow_id: Some(ProtoWorkflowId::from(workflow_id())),
        activity_id: Some(ProtoActivityId::from(activity_id())),
        progress: progress.map(ProtoPayload::from),
    }
}

/// A worker that has reported a note: the note comes back, with the instant this
/// process received it.
#[test]
fn a_reported_note_comes_back_with_its_receive_instant() -> TestResult {
    let tracker = HeartbeatTracker::new(Duration::from_secs(5));
    let (_registry, _registration, worker_id) = worker()?;
    let start = Instant::now();
    track(&tracker, worker_id, ATTEMPT, start)?;

    let before = chrono::Utc::now();
    let reported = note(&json!({ "doing": "reading the brief" }))?;
    tracker.record_heartbeat(worker_id, heartbeat(Some(reported.clone())), start)?;
    let after = chrono::Utc::now();

    match attempt_progress(&tracker, &workflow_id(), &activity_id(), ATTEMPT)? {
        AttemptProgress::Reported {
            payload,
            reported_at,
        } => {
            assert_eq!(payload, reported);
            assert!(
                reported_at >= before && reported_at <= after,
                "the stamp is the instant this process received the heartbeat"
            );
        }
        other => return Err(format!("a reported note must come back: {other:?}").into()),
    }
    Ok(())
}

/// A tracked attempt whose worker has reported nothing: `NoneSent` — a real
/// measurement, distinguishable from the store being unable to answer.
#[test]
fn a_tracked_attempt_with_no_note_is_silence_not_ignorance() -> TestResult {
    let tracker = HeartbeatTracker::new(Duration::from_secs(5));
    let (_registry, _registration, worker_id) = worker()?;
    track(&tracker, worker_id, ATTEMPT, Instant::now())?;

    assert_eq!(
        attempt_progress(&tracker, &workflow_id(), &activity_id(), ATTEMPT)?,
        AttemptProgress::NoneSent
    );
    Ok(())
}

/// A liveness-only heartbeat (no payload) refreshes the lease and must NOT be
/// read as a note: the attempt is still `NoneSent`.
#[test]
fn a_payload_free_heartbeat_is_not_a_note() -> TestResult {
    let tracker = HeartbeatTracker::new(Duration::from_secs(5));
    let (_registry, _registration, worker_id) = worker()?;
    let start = Instant::now();
    track(&tracker, worker_id, ATTEMPT, start)?;
    tracker.record_heartbeat(worker_id, heartbeat(None), start)?;

    assert_eq!(
        attempt_progress(&tracker, &workflow_id(), &activity_id(), ATTEMPT)?,
        AttemptProgress::NoneSent
    );
    Ok(())
}

/// A tracker that never held the attempt — the shape a RESTARTED process is in
/// for every attempt dispatched before it started — cannot measure anything, and
/// says so rather than reporting silence.
///
/// A fresh tracker is the same object a restarted process has: the note store is
/// process-local and starts empty. Paired with the tracked case above (same
/// query, opposite answer) this shows the read distinguishes the two rather than
/// answering `Untracked` unconditionally.
#[test]
fn an_attempt_this_process_never_held_is_unmeasurable() -> TestResult {
    let tracker = HeartbeatTracker::new(Duration::from_secs(5));
    assert_eq!(
        attempt_progress(&tracker, &workflow_id(), &activity_id(), ATTEMPT)?,
        AttemptProgress::Untracked
    );
    Ok(())
}

/// A note reported on a DIFFERENT attempt of the same activity is never
/// attributed to this one. The tracker's key is worker + workflow + activity,
/// so without the attempt discriminator a superseded attempt's note would be
/// served as the current attempt's.
#[test]
fn a_prior_attempts_note_is_never_served_as_this_attempts() -> TestResult {
    let tracker = HeartbeatTracker::new(Duration::from_secs(5));
    let registry = ConnectedWorkerRegistry::default();
    let (_first, worker_id) = register(&registry)?;
    let (_second, retry_worker) = register(&registry)?;
    assert_ne!(
        worker_id, retry_worker,
        "the two attempts must be held by genuinely different workers"
    );
    let start = Instant::now();
    // Attempt 1 reports a note, then attempt 2 is dispatched to another worker
    // while attempt 1's entry has not yet been swept.
    track(&tracker, worker_id, 1, start)?;
    tracker.record_heartbeat(
        worker_id,
        heartbeat(Some(note(&json!({ "doing": "attempt one" }))?)),
        start,
    )?;
    track(&tracker, retry_worker, 2, start)?;

    assert_eq!(
        attempt_progress(&tracker, &workflow_id(), &activity_id(), 2)?,
        AttemptProgress::NoneSent,
        "attempt 2 has reported nothing; attempt 1's note is not its note"
    );
    match attempt_progress(&tracker, &workflow_id(), &activity_id(), 1)? {
        AttemptProgress::Reported { payload, .. } => {
            assert_eq!(payload, note(&json!({ "doing": "attempt one" }))?);
        }
        other => return Err(format!("attempt 1 still holds its own note: {other:?}").into()),
    }
    Ok(())
}

/// With two owners of one attempt tracked at once (the failover window), the
/// entry with the most recent heartbeat wins — the adopter's note, not the
/// dying owner's superseded one.
#[test]
fn the_freshest_owner_wins_during_a_failover() -> TestResult {
    let tracker = HeartbeatTracker::new(Duration::from_secs(5));
    let registry = ConnectedWorkerRegistry::default();
    let (_dying_registration, dying) = register(&registry)?;
    let (_adopter_registration, adopter) = register(&registry)?;
    assert_ne!(
        dying, adopter,
        "a failover has two DIFFERENT workers holding one attempt"
    );
    let start = Instant::now();
    track(&tracker, dying, ATTEMPT, start)?;
    tracker.record_heartbeat(
        dying,
        heartbeat(Some(note(&json!({ "doing": "the dying owner" }))?)),
        start,
    )?;
    track(&tracker, adopter, ATTEMPT, start + Duration::from_secs(1))?;
    tracker.record_heartbeat(
        adopter,
        heartbeat(Some(note(&json!({ "doing": "the adopter" }))?)),
        start + Duration::from_secs(2),
    )?;

    match attempt_progress(&tracker, &workflow_id(), &activity_id(), ATTEMPT)? {
        AttemptProgress::Reported { payload, .. } => {
            assert_eq!(payload, note(&json!({ "doing": "the adopter" }))?);
        }
        other => return Err(format!("the freshest owner's note wins: {other:?}").into()),
    }
    Ok(())
}

/// Completing the task retires the entry, so the note becomes unmeasurable
/// rather than turning into "the worker said nothing".
#[test]
fn a_completed_attempt_is_unmeasurable_not_silent() -> TestResult {
    let tracker = HeartbeatTracker::new(Duration::from_secs(5));
    let (_registry, _registration, worker_id) = worker()?;
    let start = Instant::now();
    track(&tracker, worker_id, ATTEMPT, start)?;
    tracker.record_heartbeat(
        worker_id,
        heartbeat(Some(note(&json!({ "doing": "work" }))?)),
        start,
    )?;
    assert!(tracker.complete_task(worker_id, &workflow_id(), &activity_id())?);

    assert_eq!(
        attempt_progress(&tracker, &workflow_id(), &activity_id(), ATTEMPT)?,
        AttemptProgress::Untracked
    );
    Ok(())
}

/// `notes_held_since` is the instant the tracker began holding notes, which is
/// what a reader compares an attempt's dispatch instant against.
#[test]
fn notes_held_since_is_the_trackers_own_construction_instant() {
    let before = chrono::Utc::now();
    let tracker = HeartbeatTracker::new(Duration::from_secs(5));
    let after = chrono::Utc::now();
    let held_since = tracker.notes_held_since();
    assert!(
        held_since >= before && held_since <= after,
        "notes_held_since brackets construction: {held_since}"
    );
}
