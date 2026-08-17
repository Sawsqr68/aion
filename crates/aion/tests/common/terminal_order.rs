//! Canonical order over CONCURRENT ACTIVITY TERMINALS — the one freedom the
//! durability design grants a recorded trail, and the only thing this module
//! is permitted to move.
//!
//! # Why this exists
//!
//! A dependency-parallel layer dispatches its members concurrently and each
//! member's completion task appends its own terminal event when it lands, so
//! the relative position of two concurrent terminals in the recorded history
//! is settle order. That is deliberate: `durability/cursor.rs` states that a
//! parallel activity's events recorded between another activity's anchor and
//! its terminal are "an interleaving artifact, not a command-stream
//! divergence", replay binds a recorded result to its call by ORDINAL and
//! never by position, and recording a higher ordinal's terminal before a lower
//! one has resolved is what lets `collect_all` fail fast and cancel a still
//! running sibling (pinned by
//! `concurrency_e2e.rs::collect_all_fail_fast_cancels_unresolved_and_replays_byte_identically`).
//!
//! Comparing that position across two productions of the same workflow is
//! therefore asserting the enumeration, not the invariant. This module removes
//! exactly that freedom and nothing else, so a differential can go on
//! asserting every property the design DOES promise: the same event set, the
//! same ordinal assignment, the same per-ordinal bindings, and the same
//! command-stream order.
//!
//! # Exactly what moves
//!
//! Only [`Event::ActivityCompleted`], [`Event::ActivityFailed`] and
//! [`Event::ActivityCancelled`] are terminals here, and one is reordered only
//! against another terminal it is **directly adjacent to**. A maximal run of
//! consecutive terminals is sorted by activity ordinal; the run's own recorded
//! sequence numbers are then re-stamped onto it in that order, because within
//! such a run "which member got which seq" is the same freedom as "which
//! member landed first". Ties (several attempts for one ordinal) keep their
//! recorded relative order — the sort is stable — since those are causally
//! ordered, not concurrent.
//!
//! Nothing else moves. Any event that is not one of those three kinds is a
//! **boundary**: it never moves, and it splits the run on both sides, so a
//! terminal can never cross a `sched` anchor, a signal, a child terminal, a
//! workflow lifecycle event, or even an `ActivityAdvisoryExhausted` (which
//! carries an activity id but is explicitly non-terminal). A terminal recorded
//! before its own anchor stays before its own anchor.
//!
//! The four literal defect shapes this is deliberately NOT blind to are pinned
//! as red controls in `awl_bc4_differential/terminal_order_controls.rs`.

use aion_core::{Event, EventEnvelope};

/// The activity ordinal of an event that is an activity TERMINAL, or `None`
/// for every other event.
///
/// The terminal set is exactly the three events that resolve an activity —
/// the same set `nif_collect.rs::recorded_terminal` treats as resolving an
/// ordinal. `ActivityAdvisoryExhausted` carries an activity id but is
/// deliberately excluded: it accompanies a terminal rather than being one, so
/// here it is an ordinary boundary.
fn terminal_ordinal(event: &Event) -> Option<u64> {
    match event {
        Event::ActivityCompleted { activity_id, .. }
        | Event::ActivityFailed { activity_id, .. }
        | Event::ActivityCancelled { activity_id, .. } => Some(activity_id.sequence_position()),
        _ => None,
    }
}

/// Rebuilds one terminal event carrying `seq` in place of its recorded
/// sequence number, leaving every other field byte-identical.
///
/// Only the three terminal variants are ever rebuilt; anything else is cloned
/// untouched, which keeps this total without granting the caller a way to
/// re-stamp a boundary event.
fn with_seq(event: &Event, seq: u64) -> Event {
    let envelope = EventEnvelope {
        seq,
        ..event.envelope().clone()
    };
    match event {
        Event::ActivityCompleted {
            activity_id,
            result,
            attempt,
            ..
        } => Event::ActivityCompleted {
            envelope,
            activity_id: activity_id.clone(),
            result: result.clone(),
            attempt: *attempt,
        },
        Event::ActivityFailed {
            activity_id,
            error,
            attempt,
            ..
        } => Event::ActivityFailed {
            envelope,
            activity_id: activity_id.clone(),
            error: error.clone(),
            attempt: *attempt,
        },
        Event::ActivityCancelled {
            activity_id,
            attempt,
            ..
        } => Event::ActivityCancelled {
            envelope,
            activity_id: activity_id.clone(),
            attempt: *attempt,
        },
        other => other.clone(),
    }
}

/// Imposes the canonical order on every maximal run of directly adjacent
/// activity terminals in `events`, returning the rewritten trail.
///
/// Runs of fewer than two terminals are already canonical and are copied
/// through untouched, so a trail with no concurrency is returned byte-identical
/// to its input.
#[must_use]
pub fn canonical_terminal_order(events: &[Event]) -> Vec<Event> {
    let mut canonical: Vec<Event> = Vec::with_capacity(events.len());
    let mut index = 0usize;
    while index < events.len() {
        let Some(_) = terminal_ordinal(&events[index]) else {
            // A boundary: never moved, and it ends any run on either side.
            canonical.push(events[index].clone());
            index += 1;
            continue;
        };
        let start = index;
        while index < events.len() && terminal_ordinal(&events[index]).is_some() {
            index += 1;
        }
        canonical.extend(canonical_run(&events[start..index]));
    }
    canonical
}

/// Canonicalizes ONE maximal run of directly adjacent activity terminals: sort
/// by ordinal (stable, so several attempts for one ordinal keep their causal
/// order), then re-stamp the run's own recorded sequence numbers in that order.
fn canonical_run(run: &[Event]) -> Vec<Event> {
    if run.len() < 2 {
        return run.to_vec();
    }
    let sequence_numbers: Vec<u64> = run.iter().map(Event::seq).collect();
    let mut ordered: Vec<&Event> = run.iter().collect();
    // Stable: equal ordinals keep their recorded relative order.
    ordered.sort_by_key(|event| terminal_ordinal(event));
    ordered
        .into_iter()
        .zip(sequence_numbers)
        .map(|(event, seq)| with_seq(event, seq))
        .collect()
}
