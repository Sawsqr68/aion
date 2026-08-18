//! Red controls for the #42 canonical terminal order: the normalizer must be
//! blind to settle order and to NOTHING ELSE.
//!
//! A normalizer that made the differential green by sorting more than it is
//! allowed to would be indistinguishable, from the outside, from one that
//! works — right up until it swallowed a real defect. So each control here is
//! built from a LITERAL defect shape and asserted red **through the normalized
//! comparison**, exactly as the differential performs it
//! (`canonical_terminal_order` then `trail_norm::normalized_trail`, the same
//! order and the same functions `driver.rs` uses).
//!
//! The four shapes are the steward's disposition, condition 2:
//!
//! a. a result's bytes bound to the wrong ordinal;
//! b. a missing or extra event;
//! c. a terminal recorded before its own `sched` anchor;
//! d. an event moved across a non-terminal boundary.
//!
//! The positive control (settle order alone) is asserted green in the same
//! file, because a set of red controls with no green arm proves only that the
//! comparison can fail, never that it can pass.

use aion_core::{
    ActivityId, DEFAULT_TASK_QUEUE, Event, EventEnvelope, PackageVersion, Payload, RunId,
    WorkflowId,
};
use chrono::DateTime;

use crate::terminal_order::canonical_terminal_order;
use crate::trail_norm;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A deterministic envelope. The workflow id and the timestamp are identity
/// fields the normalizer rewrites to placeholders anyway, so what they are
/// cannot affect a control's verdict.
fn envelope(seq: u64) -> EventEnvelope {
    EventEnvelope {
        seq,
        recorded_at: DateTime::from_timestamp(1_700_000_000, 0).unwrap_or_default(),
        workflow_id: WorkflowId::new_v4(),
    }
}

fn scheduled(seq: u64, ordinal: u64) -> Result<Event, Box<dyn std::error::Error>> {
    Ok(Event::ActivityScheduled {
        envelope: envelope(seq),
        activity_id: ActivityId::from_sequence_position(ordinal),
        activity_type: format!("action_{ordinal}"),
        input: Payload::from_json(&serde_json::json!({ "in": ordinal }))?,
        task_queue: DEFAULT_TASK_QUEUE.to_owned(),
        node: None,
    })
}

fn completed(seq: u64, ordinal: u64, result: &str) -> Result<Event, Box<dyn std::error::Error>> {
    Ok(Event::ActivityCompleted {
        envelope: envelope(seq),
        activity_id: ActivityId::from_sequence_position(ordinal),
        result: Payload::from_json(&serde_json::json!(result))?,
        attempt: 1,
    })
}

fn signal(seq: u64) -> Result<Event, Box<dyn std::error::Error>> {
    Ok(Event::SignalReceived {
        envelope: envelope(seq),
        name: "release".to_owned(),
        payload: Payload::from_json(&serde_json::json!({ "label": "release" }))?,
    })
}

fn started(seq: u64) -> Result<Event, Box<dyn std::error::Error>> {
    Ok(Event::WorkflowStarted {
        envelope: envelope(seq),
        workflow_type: "control".to_owned(),
        input: Payload::from_json(&serde_json::json!({}))?,
        run_id: RunId::new_v4(),
        parent_run_id: None,
        parent_workflow_id: None,
        package_version: PackageVersion::new("a".repeat(64)),
    })
}

/// The differential's comparison, verbatim: canonical terminal order, then the
/// identity normalizer, then positional equality.
fn compares_equal(left: &[Event], right: &[Event]) -> Result<bool, serde_json::Error> {
    let left = trail_norm::normalized_trail(&canonical_terminal_order(left))?;
    let right = trail_norm::normalized_trail(&canonical_terminal_order(right))?;
    Ok(left == right)
}

/// POSITIVE control: settle order alone is the freedom, so the two trails that
/// differ ONLY by which concurrent terminal landed first must compare equal.
///
/// Without this arm the four red controls below would be satisfied by a
/// normalizer that does nothing at all.
#[test]
fn settle_order_alone_compares_equal() -> TestResult {
    let a_first = vec![
        started(1)?,
        scheduled(2, 0)?,
        scheduled(3, 1)?,
        completed(4, 0, "done-a")?,
        completed(5, 1, "done-b")?,
    ];
    let b_first = vec![
        started(1)?,
        scheduled(2, 0)?,
        scheduled(3, 1)?,
        completed(4, 1, "done-b")?,
        completed(5, 0, "done-a")?,
    ];
    assert!(
        compares_equal(&a_first, &b_first)?,
        "settle order is the permitted freedom and must normalize away"
    );
    Ok(())
}

/// RED CONTROL (a): a result's bytes bound to the WRONG ordinal.
///
/// Both trails carry the same event kinds in the same positions and the same
/// two payloads; only the ordinal each payload is bound to is swapped. This is
/// the defect the whole differential exists to catch — a backend that wired a
/// fork's results to the wrong branches — and it is exactly one bit away from
/// the permitted freedom, so it is the sharpest control of the four.
#[test]
fn a_result_bound_to_the_wrong_ordinal_stays_red() -> TestResult {
    let correct = vec![
        started(1)?,
        scheduled(2, 0)?,
        scheduled(3, 1)?,
        completed(4, 0, "done-a")?,
        completed(5, 1, "done-b")?,
    ];
    let miswired = vec![
        started(1)?,
        scheduled(2, 0)?,
        scheduled(3, 1)?,
        completed(4, 0, "done-b")?,
        completed(5, 1, "done-a")?,
    ];
    assert!(
        !compares_equal(&correct, &miswired)?,
        "a result bound to the wrong ordinal must survive normalization"
    );
    Ok(())
}

/// RED CONTROL (b): a missing event, and an extra one.
///
/// The canonical order rewrites positions within a run; it must never make two
/// trails of different content compare equal by absorbing a member. Asserted in
/// both directions so a length-tolerant comparison cannot pass one arm.
#[test]
fn a_missing_or_extra_event_stays_red() -> TestResult {
    let complete = vec![
        started(1)?,
        scheduled(2, 0)?,
        scheduled(3, 1)?,
        completed(4, 0, "done-a")?,
        completed(5, 1, "done-b")?,
    ];
    let missing = vec![
        started(1)?,
        scheduled(2, 0)?,
        scheduled(3, 1)?,
        completed(4, 0, "done-a")?,
    ];
    assert!(
        !compares_equal(&complete, &missing)?,
        "a missing terminal must survive normalization"
    );
    assert!(
        !compares_equal(&missing, &complete)?,
        "an extra terminal must survive normalization"
    );
    Ok(())
}

/// RED CONTROL (c): a terminal recorded BEFORE its own `sched` anchor.
///
/// A history where an activity completes before it was scheduled is a
/// command-stream defect, not an interleaving artifact. The canonical order
/// never moves a terminal across a non-terminal event, so the anchor cannot
/// close over it: each terminal here sits in a run of length one and is left
/// exactly where it was recorded.
#[test]
fn a_terminal_before_its_own_anchor_stays_red() -> TestResult {
    let anchored = vec![started(1)?, scheduled(2, 0)?, completed(3, 0, "done-a")?];
    let inverted = vec![started(1)?, completed(2, 0, "done-a")?, scheduled(3, 0)?];
    assert!(
        !compares_equal(&anchored, &inverted)?,
        "a terminal recorded before its own anchor must survive normalization"
    );
    Ok(())
}

/// RED CONTROL (d): an event moved ACROSS a non-terminal boundary.
///
/// Two terminals separated by a signal are not adjacent and are not
/// concurrent-in-the-permitted-sense: a signal between them is an observable
/// command-stream position. Swapping them across that boundary must stay red,
/// which is what proves the run-splitting rule is real rather than decorative.
#[test]
fn a_terminal_moved_across_a_non_terminal_boundary_stays_red() -> TestResult {
    let recorded = vec![
        started(1)?,
        scheduled(2, 0)?,
        scheduled(3, 1)?,
        completed(4, 0, "done-a")?,
        signal(5)?,
        completed(6, 1, "done-b")?,
    ];
    let crossed = vec![
        started(1)?,
        scheduled(2, 0)?,
        scheduled(3, 1)?,
        completed(4, 1, "done-b")?,
        signal(5)?,
        completed(6, 0, "done-a")?,
    ];
    assert!(
        !compares_equal(&recorded, &crossed)?,
        "a terminal moved across a signal must survive normalization"
    );
    Ok(())
}

/// RED CONTROL (e): the `sched` ANCHORS' recorded order.
///
/// Not one of the disposition's four, and added because mutation-testing the
/// four found this gap: widening the terminal set to include
/// `ActivityScheduled` left every one of (a)-(d) green, yet it would normalize
/// away the command-stream order — the exact property condition 1 names first
/// ("not `sched` anchors"). A layer that dispatched its members in a different
/// order is a real backend divergence, so two trails differing only in which
/// anchor was recorded first must stay red.
#[test]
fn reordered_sched_anchors_stay_red() -> TestResult {
    let written_order = vec![
        started(1)?,
        scheduled(2, 0)?,
        scheduled(3, 1)?,
        completed(4, 0, "done-a")?,
        completed(5, 1, "done-b")?,
    ];
    let anchors_swapped = vec![
        started(1)?,
        scheduled(2, 1)?,
        scheduled(3, 0)?,
        completed(4, 0, "done-a")?,
        completed(5, 1, "done-b")?,
    ];
    assert!(
        !compares_equal(&written_order, &anchors_swapped)?,
        "the order the anchors were recorded in is command-stream order and \
         must survive normalization"
    );
    Ok(())
}

/// The scope statement, asserted rather than documented: a trail carrying NO
/// adjacent terminal pair is returned byte-identical, so the normalizer is
/// provably inert everywhere except on the freedom it was granted.
#[test]
fn a_trail_without_adjacent_terminals_is_returned_untouched() -> TestResult {
    let trail = vec![
        started(1)?,
        scheduled(2, 0)?,
        completed(3, 0, "done-a")?,
        signal(4)?,
        scheduled(5, 1)?,
        completed(6, 1, "done-b")?,
    ];
    assert_eq!(
        canonical_terminal_order(&trail),
        trail,
        "with no two terminals adjacent there is no freedom to remove"
    );
    Ok(())
}
