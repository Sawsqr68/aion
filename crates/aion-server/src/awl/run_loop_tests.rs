//! What an operator is told when their queue has no connected worker.
//!
//! The two worlds this must separate are indistinguishable from the
//! connected-worker count — it is 0 in both — and telling the wrong one apart
//! from the right one is the whole point of the hint. So they are asserted
//! against each other here, never one alone.

use super::unserved_hint;
use crate::worker::admission_audit::RememberedRefusal;

const QUEUE: &str = "desk2";

fn refusal(node: Option<&str>, identity: &str, reason: &str) -> RememberedRefusal {
    RememberedRefusal {
        node: node.map(ToOwned::to_owned),
        identity: identity.to_owned(),
        reason: reason.to_owned(),
    }
}

#[test]
fn with_no_refusal_on_record_the_operator_is_told_to_start_a_worker() {
    let hint = unserved_hint(QUEUE, &[]);
    assert!(
        hint.contains("Scaffold and run this worker"),
        "nothing is dialling this queue, so starting one IS the remedy: {hint}"
    );
    assert!(
        !hint.contains("REFUSING"),
        "and it must not invent a refusal that never happened: {hint}"
    );
}

#[test]
fn with_a_refusal_on_record_the_operator_is_told_the_worker_is_being_refused() {
    let hint = unserved_hint(
        QUEUE,
        &[refusal(
            None,
            "desk2-build",
            "WORKER_CONTRACT_MISMATCH: action `charge` field `input_schema.type`",
        )],
    );
    assert!(
        !hint.contains("Scaffold and run this worker"),
        "the operator's worker IS running; this instruction cannot help them \
         and was the only signal during the #146 self-run: {hint}"
    );
    assert!(
        hint.contains("REFUSING"),
        "the hint must say what is actually happening: {hint}"
    );
    assert!(
        hint.contains("desk2-build"),
        "and WHICH build, so the operator knows which process to fix: {hint}"
    );
    assert!(
        hint.contains("input_schema.type"),
        "and WHY, carried from the gate's own diagnosis rather than paraphrased \
         into something unactionable: {hint}"
    );
}

#[test]
fn a_node_pinned_refusal_names_its_node() {
    let hint = unserved_hint(QUEUE, &[refusal(Some("netbox"), "desk2-build", "mismatch")]);
    assert!(
        hint.contains("node `netbox`"),
        "a worker serving one node is a different worker from the one serving \
         another, and an operator with several will fix the wrong one: {hint}"
    );
}

#[test]
fn further_refusals_are_counted_rather_than_dropped() {
    let one = unserved_hint(QUEUE, &[refusal(None, "build-a", "mismatch")]);
    assert!(
        !one.contains("other connection"),
        "a single refusal must not claim company it does not have: {one}"
    );

    let two = unserved_hint(
        QUEUE,
        &[
            refusal(Some("netbox"), "build-a", "mismatch"),
            refusal(Some("shell"), "build-b", "mismatch"),
        ],
    );
    assert!(
        two.contains("One other connection was refused"),
        "fixing the named one would leave the queue unserved, and an operator \
         told about one refusal will believe they are done: {two}"
    );

    let four = unserved_hint(
        QUEUE,
        &[
            refusal(Some("a"), "build-a", "mismatch"),
            refusal(Some("b"), "build-b", "mismatch"),
            refusal(Some("c"), "build-c", "mismatch"),
            refusal(Some("d"), "build-d", "mismatch"),
        ],
    );
    assert!(
        four.contains("3 other connections were refused"),
        "the count is the count, not a fixed word: {four}"
    );
}
