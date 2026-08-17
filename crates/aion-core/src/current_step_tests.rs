//! Fold correctness across the whole activity lifecycle.
//!
//! Each test drives a history through one lifecycle transition and asserts what
//! the fold says AND what it stops saying — an assertion that only checks the
//! new fact would pass on a fold that never retires anything.

use chrono::{DateTime, Utc};

use super::{current_step, open_steps};
use crate::{
    ActivityError, ActivityErrorKind, ActivityId, Event, EventEnvelope, Payload, RunId, StepState,
    WorkflowId,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn workflow_id() -> WorkflowId {
    WorkflowId::new(uuid::Uuid::from_u128(1))
}

fn recorded_at(offset: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(1_700_000_000 + offset, 0).unwrap_or_default()
}

fn envelope(seq: u64) -> EventEnvelope {
    EventEnvelope {
        seq,
        recorded_at: recorded_at(i64::try_from(seq).unwrap_or(0)),
        workflow_id: workflow_id(),
    }
}

fn payload() -> Result<Payload, crate::PayloadError> {
    Payload::from_json(&serde_json::json!({ "input": true }))
}

fn started(seq: u64) -> Result<Event, crate::PayloadError> {
    Ok(Event::WorkflowStarted {
        envelope: envelope(seq),
        workflow_type: "fixture".to_owned(),
        input: payload()?,
        run_id: RunId::new(uuid::Uuid::from_u128(10)),
        parent_run_id: None,
        package_version: crate::PackageVersion::new("a".repeat(64)),
    })
}

fn scheduled(seq: u64, ordinal: u64, activity_type: &str) -> Result<Event, crate::PayloadError> {
    Ok(Event::ActivityScheduled {
        envelope: envelope(seq),
        activity_id: ActivityId::from_sequence_position(ordinal),
        activity_type: activity_type.to_owned(),
        input: payload()?,
        task_queue: "agents".to_owned(),
        node: Some("node-a".to_owned()),
    })
}

fn dispatched(seq: u64, ordinal: u64, attempt: u32) -> Event {
    Event::ActivityStarted {
        envelope: envelope(seq),
        activity_id: ActivityId::from_sequence_position(ordinal),
        attempt,
    }
}

fn completed(seq: u64, ordinal: u64, attempt: u32) -> Result<Event, crate::PayloadError> {
    Ok(Event::ActivityCompleted {
        envelope: envelope(seq),
        activity_id: ActivityId::from_sequence_position(ordinal),
        result: payload()?,
        attempt,
    })
}

fn failed(seq: u64, ordinal: u64, attempt: u32) -> Event {
    Event::ActivityFailed {
        envelope: envelope(seq),
        activity_id: ActivityId::from_sequence_position(ordinal),
        error: ActivityError {
            kind: ActivityErrorKind::Retryable,
            message: "boom".to_owned(),
            details: None,
        },
        attempt,
    }
}

fn cancelled(seq: u64, ordinal: u64, attempt: u32) -> Event {
    Event::ActivityCancelled {
        envelope: envelope(seq),
        activity_id: ActivityId::from_sequence_position(ordinal),
        attempt,
    }
}

/// A run with no activity at all has no current step — and the absence is the
/// answer, so `open_steps` is empty too.
#[test]
fn a_run_with_no_activity_has_no_current_step() -> TestResult {
    let history = vec![started(1)?];
    assert_eq!(current_step(&history), None);
    assert!(open_steps(&history).is_empty());
    Ok(())
}

/// An empty history folds to nothing rather than panicking on the missing
/// segment start.
#[test]
fn an_empty_history_folds_to_nothing() {
    assert_eq!(current_step(&[]), None);
    assert!(open_steps(&[]).is_empty());
}

/// Scheduled but not dispatched: the step is current, and it reports
/// `Scheduled` — claiming `Dispatched` would claim a delivery that has not
/// happened.
#[test]
fn a_scheduled_step_is_current_and_carries_its_stamped_address() -> TestResult {
    let history = vec![started(1)?, scheduled(2, 4, "review")?];
    let step = current_step(&history).ok_or("scheduled step must be current")?;
    assert_eq!(step.activity_id, ActivityId::from_sequence_position(4));
    assert_eq!(step.activity_type, "review");
    assert_eq!(step.task_queue, "agents");
    assert_eq!(step.node.as_deref(), Some("node-a"));
    assert_eq!(step.scheduled_at, recorded_at(2));
    assert_eq!(step.state, StepState::Scheduled);
    Ok(())
}

/// Dispatch advances the SAME ordinal rather than opening a second one, and
/// carries the attempt and the dispatch instant.
#[test]
fn dispatch_advances_the_same_ordinal() -> TestResult {
    let history = vec![started(1)?, scheduled(2, 4, "review")?, dispatched(3, 4, 1)];
    assert_eq!(open_steps(&history).len(), 1, "one ordinal, not two");
    let step = current_step(&history).ok_or("dispatched step must be current")?;
    assert_eq!(
        step.state,
        StepState::Dispatched {
            attempt: 1,
            dispatched_at: recorded_at(3),
        }
    );
    Ok(())
}

/// Every terminal retires the ordinal. Absence is paired with survival: a
/// second, still-open ordinal proves the fold retired one thing and not
/// everything.
#[test]
fn each_terminal_retires_only_its_own_ordinal() -> TestResult {
    let terminals: Vec<Event> = vec![completed(5, 4, 1)?, failed(5, 4, 1), cancelled(5, 4, 1)];
    for terminal in terminals {
        let history = vec![
            started(1)?,
            scheduled(2, 4, "review")?,
            dispatched(3, 4, 1),
            scheduled(4, 6, "survivor")?,
            terminal.clone(),
        ];
        let open = open_steps(&history);
        assert_eq!(
            open.len(),
            1,
            "the terminal {terminal:?} must retire exactly its own ordinal"
        );
        assert_eq!(open[0].activity_type, "survivor");
        let step = current_step(&history).ok_or("the survivor is current")?;
        assert_eq!(step.activity_type, "survivor");
    }
    Ok(())
}

/// A retryable failure followed by a fresh dispatch reports the NEW attempt on
/// one ordinal — not a retired step and not two open ones.
#[test]
fn a_retried_ordinal_reports_the_new_attempt_once() -> TestResult {
    let history = vec![
        started(1)?,
        scheduled(2, 4, "review")?,
        dispatched(3, 4, 1),
        failed(4, 4, 1),
        scheduled(5, 4, "review")?,
        dispatched(6, 4, 2),
    ];
    let open = open_steps(&history);
    assert_eq!(open.len(), 1);
    assert_eq!(
        open[0].state,
        StepState::Dispatched {
            attempt: 2,
            dispatched_at: recorded_at(6),
        }
    );
    Ok(())
}

/// An advisory exhaustion ACCOMPANIES a failure and never replaces it, so it
/// must not retire anything on its own.
#[test]
fn advisory_exhaustion_is_not_a_terminal() -> TestResult {
    let history = vec![
        started(1)?,
        scheduled(2, 4, "notify")?,
        dispatched(3, 4, 1),
        Event::ActivityAdvisoryExhausted {
            envelope: envelope(4),
            activity_id: ActivityId::from_sequence_position(4),
            activity_type: "notify".to_owned(),
            reason: "boom".to_owned(),
            attempt: 1,
        },
    ];
    let open = open_steps(&history);
    assert_eq!(open.len(), 1, "the advisory marker retires nothing");
    assert_eq!(
        open[0].state,
        StepState::Dispatched {
            attempt: 1,
            dispatched_at: recorded_at(3),
        }
    );

    // The accompanying terminal failure is what retires it.
    let mut with_failure = history;
    with_failure.push(failed(5, 4, 1));
    assert!(open_steps(&with_failure).is_empty());
    Ok(())
}

/// A reopen supersedes the recorded terminal of the activities it names: the
/// ordinal is open again, reported as `Reopened` (nothing has been dispatched
/// for it in this lease), and an ordinal the reopen does NOT name stays retired.
#[test]
fn a_reopen_reopens_only_the_activities_it_names() -> TestResult {
    let history = vec![
        started(1)?,
        scheduled(2, 4, "review")?,
        dispatched(3, 4, 1),
        failed(4, 4, 1),
        scheduled(5, 6, "publish")?,
        dispatched(6, 6, 1),
        completed(7, 6, 1)?,
        Event::WorkflowReopened {
            envelope: envelope(8),
            run_id: RunId::new(uuid::Uuid::from_u128(10)),
            reopened: vec![ActivityId::from_sequence_position(4)],
        },
    ];
    let open = open_steps(&history);
    assert_eq!(open.len(), 1, "only the named ordinal comes back");
    assert_eq!(open[0].activity_type, "review");
    assert_eq!(
        open[0].state,
        StepState::Reopened {
            reopened_at: recorded_at(8),
        },
        "a reopened ordinal has not been dispatched in this lease"
    );
    // The address is recovered from the ordinal's own recorded scheduling.
    assert_eq!(open[0].task_queue, "agents");
    assert_eq!(open[0].scheduled_at, recorded_at(2));
    Ok(())
}

/// A reopen naming an ordinal this segment never scheduled records no address,
/// so the fold reports nothing for it rather than inventing one.
#[test]
fn a_reopen_of_an_unscheduled_ordinal_reports_nothing() -> TestResult {
    let history = vec![
        started(1)?,
        Event::WorkflowReopened {
            envelope: envelope(2),
            run_id: RunId::new(uuid::Uuid::from_u128(10)),
            reopened: vec![ActivityId::from_sequence_position(4)],
        },
    ];
    assert!(open_steps(&history).is_empty());
    assert_eq!(current_step(&history), None);
    Ok(())
}

/// Re-dispatch after a reopen supersedes the `Reopened` state on the same
/// ordinal.
#[test]
fn re_dispatch_after_a_reopen_advances_the_ordinal() -> TestResult {
    let history = vec![
        started(1)?,
        scheduled(2, 4, "review")?,
        dispatched(3, 4, 1),
        failed(4, 4, 1),
        Event::WorkflowReopened {
            envelope: envelope(5),
            run_id: RunId::new(uuid::Uuid::from_u128(10)),
            reopened: vec![ActivityId::from_sequence_position(4)],
        },
        scheduled(6, 4, "review")?,
        dispatched(7, 4, 2),
    ];
    let open = open_steps(&history);
    assert_eq!(open.len(), 1);
    assert_eq!(
        open[0].state,
        StepState::Dispatched {
            attempt: 2,
            dispatched_at: recorded_at(7),
        }
    );
    Ok(())
}

/// A continue-as-new starts a fresh segment: the prior segment's open activity
/// belongs to a run that no longer executes and is not reported.
#[test]
fn a_new_run_segment_drops_the_prior_segments_open_step() -> TestResult {
    let history = vec![
        started(1)?,
        scheduled(2, 4, "old")?,
        dispatched(3, 4, 1),
        started(4)?,
        scheduled(5, 1, "new")?,
    ];
    let open = open_steps(&history);
    assert_eq!(open.len(), 1, "only the active segment is folded");
    assert_eq!(open[0].activity_type, "new");
    Ok(())
}

/// An `ActivityStarted` with no scheduling in this segment records no address,
/// so it opens nothing.
#[test]
fn a_dispatch_without_a_scheduling_opens_nothing() -> TestResult {
    let history = vec![started(1)?, dispatched(2, 4, 1)];
    assert!(open_steps(&history).is_empty());
    assert_eq!(current_step(&history), None);
    Ok(())
}

/// A fan-out keeps every sibling open, and `current_step` picks the most
/// recently advanced one — not the first, and not the last opened.
#[test]
fn a_fan_out_keeps_every_sibling_and_current_is_the_most_recently_advanced() -> TestResult {
    let history = vec![
        started(1)?,
        scheduled(2, 4, "left")?,
        scheduled(3, 5, "middle")?,
        scheduled(4, 6, "right")?,
        // The MIDDLE ordinal is dispatched last, so it is the most recently
        // advanced despite being neither first nor last opened.
        dispatched(5, 5, 1),
    ];
    let open = open_steps(&history);
    assert_eq!(open.len(), 3, "no sibling is lost");
    assert_eq!(
        open.iter()
            .map(|step| step.activity_type.as_str())
            .collect::<Vec<_>>(),
        vec!["left", "middle", "right"],
        "siblings stay in first-opened order"
    );
    let step = current_step(&history).ok_or("a fan-out still has a current step")?;
    assert_eq!(step.activity_type, "middle");
    Ok(())
}

/// Timers and signals interleaved with activities change nothing about the
/// fold: a run blocked on a timer with no open activity has no current step.
#[test]
fn unrelated_events_neither_open_nor_retire_a_step() -> TestResult {
    let history = vec![
        started(1)?,
        scheduled(2, 4, "review")?,
        dispatched(3, 4, 1),
        completed(4, 4, 1)?,
        Event::TimerStarted {
            envelope: envelope(5),
            timer_id: crate::TimerId::anonymous(1),
            fire_at: recorded_at(60),
        },
    ];
    assert_eq!(current_step(&history), None);
    assert!(open_steps(&history).is_empty());
    Ok(())
}

/// A recovery adoption offer (#36) advances nothing and retires nothing: it
/// says the SAME execution is being handed back to the worker's adoption path,
/// so the ordinal stays open at the attempt and dispatch instant it already
/// had. A fold that retired it would tell an operator their still-working
/// builder had stopped; a fold that re-opened it as Scheduled would erase the
/// dispatch that is still in flight.
#[test]
fn an_adoption_offer_leaves_the_ordinal_exactly_as_dispatched() -> TestResult {
    let history = vec![
        started(1)?,
        scheduled(2, 0, "delegate")?,
        dispatched(3, 0, 1),
        Event::ActivityAdoptionOffered {
            envelope: envelope(4),
            activity_id: ActivityId::from_sequence_position(0),
            attempt: 1,
        },
    ];
    let step = current_step(&history).ok_or("the adopted ordinal is still the current step")?;
    assert_eq!(step.activity_type, "delegate");
    assert_eq!(
        step.state,
        StepState::Dispatched {
            attempt: 1,
            dispatched_at: recorded_at(3),
        },
        "the offer must not restate the attempt or move the dispatch instant"
    );
    assert_eq!(open_steps(&history).len(), 1, "no sibling is invented");
    Ok(())
}
