//! Which step a run is on, folded from its own history.
//!
//! There is no stored "current step" field anywhere and there deliberately is
//! not one: exactly like [`crate::WorkflowStatus`], the answer is a PROJECTION
//! of the authoritative event history, computed on read. A maintained
//! projection would be a second copy of a fact history already holds, and the
//! two would drift the first time an append path forgot to update it.
//!
//! # What "open" means here, precisely
//!
//! An activity ordinal is OPEN when the run's active segment records it as
//! scheduled (and possibly dispatched) with no terminal event for it. The last
//! event naming the ordinal decides:
//!
//! - [`Event::ActivityScheduled`] (re)opens it at the address the engine
//!   stamped — the task queue and node the dispatch really went to.
//! - [`Event::ActivityStarted`] records the delivery attempt and the instant
//!   the ENGINE dispatched. It is NOT proof a worker took the work: the event
//!   is written at dispatch, atomically with its `ActivityScheduled`, before
//!   any worker is selected. That is why [`StepState::Dispatched`] is named for
//!   what happened rather than for what a reader might hope happened, and why
//!   liveness is answered by joining the live fleet, never by this fold.
//! - [`Event::ActivityAdoptionOffered`] advances NOTHING. It records that
//!   recovery handed the ordinal's still-dangling attempt back to the worker's
//!   adoption path (#36) — the same execution, the same attempt, already
//!   dispatched. Retiring the ordinal on it would tell a reader that a builder
//!   still at work had stopped; re-opening it as scheduled would erase the
//!   dispatch that is still in flight. Both would be false, so the arm is
//!   written out rather than left to the catch-all.
//! - `ActivityCompleted` / `ActivityFailed` / `ActivityCancelled` retire it.
//! - [`Event::ActivityAdvisoryExhausted`] is deliberately NOT a terminal: it
//!   ACCOMPANIES an `ActivityFailed` and never replaces it, so treating it as
//!   one would retire an ordinal that its own failure already retired.
//! - [`Event::WorkflowReopened`] SUPERSEDES the recorded terminal of every
//!   activity it names, which is the whole point of the event — those
//!   activities resolve to live re-dispatch on replay. The fold re-opens them
//!   as [`StepState::Reopened`]: they are open again, and nothing has been
//!   dispatched for them in this lease yet, so calling them `Dispatched` would
//!   claim a delivery that has not happened.
//!
//! # Fan-out means "the current step" is not always one step
//!
//! A fan-out has several ordinals open at once. [`current_step`] answers with
//! the most recently advanced one because a reader asking "what is it doing"
//! needs a single answer; [`open_steps`] returns all of them so a reader that
//! must not lose the siblings never has to guess that they existed.

use chrono::{DateTime, Utc};

use crate::{ActivityId, Event, describe_live::OpenStep, describe_live::StepState};

/// Working state for one activity ordinal while the segment is scanned.
#[derive(Clone, Debug)]
struct Open {
    step: OpenStep,
    /// Position of the last event that advanced this ordinal, so the most
    /// recently advanced open ordinal can be identified without a second scan.
    last_advanced_at: usize,
}

/// Every step the run's active segment records as open, in the order the
/// ordinals were first opened.
///
/// The active segment is everything from the latest [`Event::WorkflowStarted`]
/// — a continue-as-new starts a fresh segment, and the prior segment's
/// activities belong to a run that no longer executes.
///
/// An `ActivityStarted` with no `ActivityScheduled` in the segment opens
/// nothing: its address was never recorded in this segment, so there is no
/// address to report and inventing one would be a guess.
#[must_use]
pub fn open_steps(history: &[Event]) -> Vec<OpenStep> {
    fold(history).into_iter().map(|open| open.step).collect()
}

/// The single step a reader is told the run is on: the open step whose last
/// history event is the most recent.
///
/// `None` for a run with nothing open — a completed run, a failed run, a run
/// blocked on a timer or a signal. That absence is an answer, not a gap: the
/// run is genuinely not inside an activity.
#[must_use]
pub fn current_step(history: &[Event]) -> Option<OpenStep> {
    fold(history)
        .into_iter()
        .max_by_key(|open| open.last_advanced_at)
        .map(|open| open.step)
}

/// The shared scan behind both readers.
fn fold(history: &[Event]) -> Vec<Open> {
    let segment_start = history
        .iter()
        .rposition(|event| matches!(event, Event::WorkflowStarted { .. }))
        .unwrap_or(0);
    let mut open: Vec<Open> = Vec::new();
    for (position, event) in history[segment_start..].iter().enumerate() {
        match event {
            Event::ActivityScheduled {
                envelope,
                activity_id,
                activity_type,
                task_queue,
                node,
                ..
            } => {
                let entry = Open {
                    step: OpenStep {
                        activity_id: activity_id.clone(),
                        activity_type: activity_type.clone(),
                        task_queue: task_queue.clone(),
                        node: node.clone(),
                        scheduled_at: envelope.recorded_at,
                        state: StepState::Scheduled,
                    },
                    last_advanced_at: position,
                };
                replace_or_push(&mut open, entry);
            }
            Event::ActivityStarted {
                envelope,
                activity_id,
                attempt,
            } => {
                if let Some(held) = find_mut(&mut open, activity_id) {
                    held.step.state = StepState::Dispatched {
                        attempt: *attempt,
                        dispatched_at: envelope.recorded_at,
                    };
                    held.last_advanced_at = position;
                }
            }
            // The adopted attempt is the one already folded above: same
            // ordinal, same attempt, same dispatch instant. Stated, not left to
            // the catch-all, so a future edit has to argue with the module docs
            // before it retires or re-opens a live execution.
            Event::ActivityAdoptionOffered { .. } => {}
            Event::ActivityCompleted { activity_id, .. }
            | Event::ActivityFailed { activity_id, .. }
            | Event::ActivityCancelled { activity_id, .. } => {
                open.retain(|held| &held.step.activity_id != activity_id);
            }
            Event::WorkflowReopened {
                envelope, reopened, ..
            } => reopen(
                &mut open,
                history,
                segment_start,
                reopened,
                envelope.recorded_at,
                position,
            ),
            _ => {}
        }
    }
    open
}

/// Re-open every activity a [`Event::WorkflowReopened`] names.
///
/// The reopened ordinal's terminal event has already retired it from `open`, so
/// its recorded address has to be recovered from the segment's last
/// `ActivityScheduled` for that ordinal. An ordinal with no scheduling in this
/// segment is skipped for the same reason a bare `ActivityStarted` is: nothing
/// recorded its address here, and inventing one would be a guess.
fn reopen(
    open: &mut Vec<Open>,
    history: &[Event],
    segment_start: usize,
    reopened: &[ActivityId],
    recorded_at: DateTime<Utc>,
    position: usize,
) {
    for activity_id in reopened {
        let Some(scheduled) = last_scheduling(&history[segment_start..], activity_id) else {
            continue;
        };
        let entry = Open {
            step: OpenStep {
                state: StepState::Reopened {
                    reopened_at: recorded_at,
                },
                ..scheduled
            },
            last_advanced_at: position,
        };
        replace_or_push(open, entry);
    }
}

/// The address the segment's most recent `ActivityScheduled` for `activity_id`
/// stamped, projected as a freshly opened step.
fn last_scheduling(segment: &[Event], activity_id: &ActivityId) -> Option<OpenStep> {
    segment.iter().rev().find_map(|event| match event {
        Event::ActivityScheduled {
            envelope,
            activity_id: scheduled_id,
            activity_type,
            task_queue,
            node,
            ..
        } if scheduled_id == activity_id => Some(OpenStep {
            activity_id: scheduled_id.clone(),
            activity_type: activity_type.clone(),
            task_queue: task_queue.clone(),
            node: node.clone(),
            scheduled_at: envelope.recorded_at,
            state: StepState::Scheduled,
        }),
        _ => None,
    })
}

/// Replace the held entry for `entry`'s ordinal, or append it when the ordinal
/// is not held. Ordinals stay in first-opened order either way.
fn replace_or_push(open: &mut Vec<Open>, entry: Open) {
    match find_mut(open, &entry.step.activity_id) {
        Some(held) => *held = entry,
        None => open.push(entry),
    }
}

/// The held entry for `activity_id`, if the ordinal is open.
fn find_mut<'a>(open: &'a mut [Open], activity_id: &ActivityId) -> Option<&'a mut Open> {
    open.iter_mut()
        .find(|held| &held.step.activity_id == activity_id)
}

#[cfg(test)]
#[path = "current_step_tests.rs"]
mod tests;
