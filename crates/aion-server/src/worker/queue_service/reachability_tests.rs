//! Tests for the in-flight-activity reachability projection.
//!
//! The controls matter as much as the exhibits here: a diagnostic that has only
//! ever been seen to FIRE is indistinguishable from one that always fires, so
//! every "it reports" test is paired with a "it stays silent" test built from
//! the same history.

use std::sync::Arc;

use aion_core::{
    ActivityError, ActivityErrorKind, ActivityId, ContentType, Event, EventEnvelope,
    PackageVersion, Payload, RunId, WorkflowId,
};
use chrono::{DateTime, TimeZone, Utc};

use super::super::declarations::{QueueDeclaration, QueueDeclarationSource, QueueDeclarations};
use super::super::state::QueueServiceState;
use super::super::taxonomy::{QueueServiceReason, ServiceAddress};
use super::{ActivityReachability, open_activities_in_active_segment};
use crate::error::ServerError;
use crate::worker::registry::ConnectedWorkerRegistry;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const NAMESPACE: &str = "default";
const QUEUE: &str = "default";
const ACTIVITY: &str = "assistant_provision";

/// A declarations source pinned to one answer, so a test states the structural
/// half of the verdict explicitly rather than inheriting it from an uninstalled
/// source.
struct FixedDeclarations(QueueDeclaration);

impl QueueDeclarations for FixedDeclarations {
    fn declaration_for(&self, _task_queue: &str) -> QueueDeclaration {
        self.0
    }
}

fn declarations(answer: QueueDeclaration) -> QueueDeclarationSource {
    let source = QueueDeclarationSource::default();
    source.install(Arc::new(FixedDeclarations(answer)));
    source
}

/// The events under test are identified by their address fields, never their
/// bodies, so every fixture payload is an empty JSON body.
fn empty_payload() -> Payload {
    Payload::new(ContentType::Json, Vec::new())
}

fn instant(offset_seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_700_000_000 + offset_seconds, 0)
        .single()
        .unwrap_or_default()
}

fn envelope(workflow_id: &WorkflowId, seq: u64) -> EventEnvelope {
    EventEnvelope {
        seq,
        recorded_at: instant(i64::try_from(seq).unwrap_or_default()),
        workflow_id: workflow_id.clone(),
    }
}

fn started(workflow_id: &WorkflowId, seq: u64) -> Event {
    Event::WorkflowStarted {
        envelope: envelope(workflow_id, seq),
        workflow_type: String::from("assistant"),
        input: empty_payload(),
        run_id: RunId::new_v4(),
        parent_run_id: None,
        parent_workflow_id: None,
        package_version: PackageVersion::new("sha256:test"),
    }
}

/// The engine's atomic dispatch pair: `ActivityScheduled` at `seq` immediately
/// followed by `ActivityStarted` at `seq + 1`, both stamped with the IDENTICAL
/// `recorded_at`. That is the shape the recorder really writes — one atomic
/// batch, consecutive sequence numbers, one timestamp — and it is the shape the
/// live 24-day exhibit has on disk. Building the fixture any other way would be
/// testing a history the engine never produces.
fn dispatch(workflow_id: &WorkflowId, seq: u64, ordinal: u64, attempt: u32) -> [Event; 2] {
    let recorded_at = instant(i64::try_from(seq).unwrap_or_default());
    let at = |seq: u64| EventEnvelope {
        seq,
        recorded_at,
        workflow_id: workflow_id.clone(),
    };
    [
        Event::ActivityScheduled {
            envelope: at(seq),
            activity_id: ActivityId::from_sequence_position(ordinal),
            activity_type: String::from(ACTIVITY),
            input: empty_payload(),
            task_queue: String::from(QUEUE),
            node: None,
        },
        Event::ActivityStarted {
            envelope: at(seq + 1),
            activity_id: ActivityId::from_sequence_position(ordinal),
            attempt,
        },
    ]
}

fn completed(workflow_id: &WorkflowId, seq: u64, ordinal: u64, attempt: u32) -> Event {
    Event::ActivityCompleted {
        envelope: envelope(workflow_id, seq),
        activity_id: ActivityId::from_sequence_position(ordinal),
        result: empty_payload(),
        attempt,
    }
}

/// The 24-day exhibit's shape: one ordinal dispatched, never terminated.
fn stuck_history(workflow_id: &WorkflowId) -> Vec<Event> {
    let mut history = vec![started(workflow_id, 1)];
    history.extend(dispatch(workflow_id, 2, 0, 1));
    history
}

fn reachability<'a>(
    registry: &'a ConnectedWorkerRegistry,
    declarations: &'a QueueDeclarationSource,
    state: &'a QueueServiceState,
) -> ActivityReachability<'a> {
    ActivityReachability {
        registry,
        declarations,
        state,
    }
}

/// Everything a live registration needs held for the duration of a test: the
/// registry guard and the delivery channel's receiver.
///
/// BOTH have to be held. The guard deregisters the worker when dropped, and
/// dropping the receiver closes the channel — either one released early silently
/// empties the very census the assertion is about, which is exactly how the
/// first draft of these tests reported a SERVED queue as unserved and would have
/// passed a broken negative control.
type Held = (
    crate::worker::WorkerRegistration,
    tokio::sync::mpsc::Receiver<crate::worker::registry::WorkerMessage>,
);

/// Register a worker serving `activity_types` on `(NAMESPACE, default queue)`.
fn serve(
    registry: &ConnectedWorkerRegistry,
    activity_types: &[String],
) -> Result<Held, ServerError> {
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    let registration = registry.register(NAMESPACE, activity_types.iter(), sender)?;
    Ok((registration, receiver))
}

#[test]
fn a_dispatch_to_an_empty_pool_is_reported_with_the_engine_stamped_address() -> TestResult {
    let workflow_id = WorkflowId::new_v4();
    let registry = ConnectedWorkerRegistry::default();
    let state = QueueServiceState::default();
    let declarations = declarations(QueueDeclaration::Declared);

    let unserved = reachability(&registry, &declarations, &state).unserved(
        NAMESPACE,
        &workflow_id,
        &stuck_history(&workflow_id),
    )?;

    assert_eq!(unserved.len(), 1, "{unserved:?}");
    let entry = &unserved[0];
    assert_eq!(entry.activity_id, ActivityId::from_sequence_position(0));
    assert_eq!(entry.activity_type, ACTIVITY);
    assert_eq!(entry.task_queue, QUEUE);
    assert_eq!(entry.node, None);
    assert_eq!(entry.attempt, 1);
    assert_eq!(entry.dispatched_at, instant(2));
    // The vocabulary is the taxonomy's own, computed here from the same source
    // the code reads rather than pinned as a literal.
    assert_eq!(entry.reason, QueueServiceReason::NoLivePollers.as_str());
    assert_eq!(
        entry.detail,
        QueueServiceReason::NoLivePollers.explain(&ServiceAddress {
            namespace: String::from(NAMESPACE),
            task_queue: String::from(QUEUE),
            activity_type: String::from(ACTIVITY),
            node: None,
        })
    );
    assert_eq!(entry.workers_in_pool, 0);
    assert_eq!(entry.workers_serving_activity, 0);
    assert_eq!(entry.compatible_workers, 0);
    // Nothing in THIS process is holding the dispatch in a selection wait — the
    // shape a run left behind by a restart takes.
    assert!(!entry.dispatch_parked);
    Ok(())
}

/// The negative control, and the one that decides whether the diagnostic means
/// anything: the SAME history, the SAME activity in flight, with a worker that
/// serves it. Silence is the required answer.
#[test]
fn an_activity_a_live_worker_can_take_is_never_reported() -> TestResult {
    let workflow_id = WorkflowId::new_v4();
    let registry = ConnectedWorkerRegistry::default();
    let held = serve(&registry, &[String::from(ACTIVITY)])?;
    let state = QueueServiceState::default();
    let declarations = declarations(QueueDeclaration::Declared);

    let unserved = reachability(&registry, &declarations, &state).unserved(
        NAMESPACE,
        &workflow_id,
        &stuck_history(&workflow_id),
    )?;

    assert!(
        unserved.is_empty(),
        "a served in-flight activity must produce no diagnostic, got {unserved:?}"
    );
    // Non-vacuity: the fixture really does hold an in-flight activity, so the
    // silence above is the verdict and not an empty projection.
    assert_eq!(
        open_activities_in_active_segment(&stuck_history(&workflow_id)).len(),
        1
    );
    drop(held);
    Ok(())
}

/// The second control: a run whose activity genuinely finished has nothing in
/// flight, so an empty fleet cannot make it look stuck.
#[test]
fn a_completed_activity_is_never_reported_even_with_no_workers() -> TestResult {
    let workflow_id = WorkflowId::new_v4();
    let registry = ConnectedWorkerRegistry::default();
    let state = QueueServiceState::default();
    let declarations = declarations(QueueDeclaration::Declared);
    let mut history = stuck_history(&workflow_id);
    history.push(completed(&workflow_id, 4, 0, 1));

    let unserved = reachability(&registry, &declarations, &state).unserved(
        NAMESPACE,
        &workflow_id,
        &history,
    )?;

    assert!(unserved.is_empty(), "{unserved:?}");
    Ok(())
}

/// Pollers that do not cover the activity type are a different fault from no
/// pollers at all, and the operator is told which.
#[test]
fn workers_that_serve_something_else_are_reported_as_incompatible() -> TestResult {
    let workflow_id = WorkflowId::new_v4();
    let registry = ConnectedWorkerRegistry::default();
    let held = serve(&registry, &[String::from("something_else")])?;
    let state = QueueServiceState::default();
    let declarations = declarations(QueueDeclaration::Declared);

    let unserved = reachability(&registry, &declarations, &state).unserved(
        NAMESPACE,
        &workflow_id,
        &stuck_history(&workflow_id),
    )?;

    assert_eq!(unserved.len(), 1, "{unserved:?}");
    assert_eq!(
        unserved[0].reason,
        QueueServiceReason::PollersIncompatible.as_str()
    );
    assert_eq!(unserved[0].workers_in_pool, 1);
    assert_eq!(unserved[0].workers_serving_activity, 0);
    drop(held);
    Ok(())
}

/// A queue no deployed contract declares can never be served, and that is a
/// different sentence from "no worker has connected yet".
#[test]
fn an_undeclared_queue_is_reported_as_structural() -> TestResult {
    let workflow_id = WorkflowId::new_v4();
    let registry = ConnectedWorkerRegistry::default();
    let state = QueueServiceState::default();
    let declarations = declarations(QueueDeclaration::NotDeclared);

    let unserved = reachability(&registry, &declarations, &state).unserved(
        NAMESPACE,
        &workflow_id,
        &stuck_history(&workflow_id),
    )?;

    assert_eq!(unserved.len(), 1, "{unserved:?}");
    assert_eq!(
        unserved[0].reason,
        QueueServiceReason::NoQueueDeclaration.as_str()
    );
    Ok(())
}

/// A dispatch this process is holding in its selection wait is flagged as such,
/// which is the difference between "the engine is waiting for a worker" and
/// "nothing is even waiting for it".
#[test]
fn a_dispatch_parked_in_this_process_says_so() -> TestResult {
    let workflow_id = WorkflowId::new_v4();
    let activity_id = ActivityId::from_sequence_position(0);
    let registry = ConnectedWorkerRegistry::default();
    let declarations = declarations(QueueDeclaration::Declared);
    let state = QueueServiceState::default();
    let address = ServiceAddress {
        namespace: String::from(NAMESPACE),
        task_queue: String::from(QUEUE),
        activity_type: String::from(ACTIVITY),
        node: None,
    };
    state.mark(super::super::state::Parked {
        address: &address,
        reason: QueueServiceReason::NoLivePollers,
        policy: super::super::policy::QueueServicePolicy::Strict,
        census: super::super::census::PoolCensus::default(),
        workflow_id: &workflow_id,
        activity_id: &activity_id,
    })?;

    let unserved = reachability(&registry, &declarations, &state).unserved(
        NAMESPACE,
        &workflow_id,
        &stuck_history(&workflow_id),
    )?;

    assert_eq!(unserved.len(), 1, "{unserved:?}");
    assert!(unserved[0].dispatch_parked);
    Ok(())
}

/// The exhibit's real shape: one ordinal re-armed on every server start. It is
/// ONE stuck activity, reported once, at its most recent dispatch — not one
/// entry per re-arm.
#[test]
fn a_re_armed_ordinal_is_reported_once_at_its_latest_dispatch() -> TestResult {
    let workflow_id = WorkflowId::new_v4();
    let mut history = vec![started(&workflow_id, 1)];
    history.extend(dispatch(&workflow_id, 2, 0, 1));
    history.extend(dispatch(&workflow_id, 4, 0, 1));
    history.extend(dispatch(&workflow_id, 6, 0, 1));
    let registry = ConnectedWorkerRegistry::default();
    let state = QueueServiceState::default();
    let declarations = declarations(QueueDeclaration::Declared);

    let unserved = reachability(&registry, &declarations, &state).unserved(
        NAMESPACE,
        &workflow_id,
        &history,
    )?;

    assert_eq!(unserved.len(), 1, "{unserved:?}");
    assert_eq!(unserved[0].dispatched_at, instant(6));
    Ok(())
}

#[test]
fn the_projection_scopes_to_the_active_run_segment() {
    let workflow_id = WorkflowId::new_v4();
    let mut history = vec![started(&workflow_id, 1)];
    history.extend(dispatch(&workflow_id, 2, 0, 1));
    // A continue-as-new (or reopen) begins a new segment; the previous
    // segment's unterminated dispatch belongs to a run that no longer exists.
    history.push(started(&workflow_id, 4));
    history.extend(dispatch(&workflow_id, 5, 1, 1));

    let open = open_activities_in_active_segment(&history);

    assert_eq!(open.len(), 1, "{open:?}");
    assert_eq!(open[0].activity_id, ActivityId::from_sequence_position(1));
}

#[test]
fn a_failed_and_a_cancelled_activity_are_both_retired() {
    let workflow_id = WorkflowId::new_v4();
    let mut history = vec![started(&workflow_id, 1)];
    history.extend(dispatch(&workflow_id, 2, 0, 1));
    history.extend(dispatch(&workflow_id, 4, 1, 1));
    history.push(Event::ActivityFailed {
        envelope: envelope(&workflow_id, 6),
        activity_id: ActivityId::from_sequence_position(0),
        error: ActivityError {
            kind: ActivityErrorKind::Terminal,
            message: String::from("boom"),
            details: None,
        },
        attempt: 1,
    });
    history.push(Event::ActivityCancelled {
        envelope: envelope(&workflow_id, 7),
        activity_id: ActivityId::from_sequence_position(1),
        attempt: 1,
    });

    assert!(open_activities_in_active_segment(&history).is_empty());
}

/// An `ActivityStarted` whose `ActivityScheduled` is not in this segment has no
/// recorded address, so there is nothing to classify and nothing is invented.
#[test]
fn a_start_without_a_schedule_in_segment_is_not_projected() {
    let workflow_id = WorkflowId::new_v4();
    let history = vec![
        started(&workflow_id, 1),
        Event::ActivityStarted {
            envelope: envelope(&workflow_id, 2),
            activity_id: ActivityId::from_sequence_position(0),
            attempt: 1,
        },
    ];

    assert!(open_activities_in_active_segment(&history).is_empty());
}

#[test]
fn an_empty_history_projects_nothing() {
    assert!(open_activities_in_active_segment(&[]).is_empty());
}
