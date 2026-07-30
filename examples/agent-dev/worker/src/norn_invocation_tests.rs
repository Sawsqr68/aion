//! Tests for the norn argv and the degraded-session advice.
//!
//! The negative controls carry the weight here. A flag only ever observed to
//! fire is untested in the case that matters, and advice that fires on every
//! failure teaches nothing.

use super::{
    ALLOW_DEGRADED_SESSION_FLAG, degraded_session_advice, is_degraded_session_refusal, norn_args,
};

/// The exact refusal recorded in durable history for the 28-day-parked
/// assistant run — the evidence this whole path was built from, verbatim.
const RECORDED_REFUSAL: &str = "run/execute failed (code -32603): agent error: session error: \
     storage error: open_session failed: session \
     'f9b062c2-d260-43f9-8374-d3210330f567-assistant' is a fresh-epoch \
     projection; explicit resume approval is required";

const RUN_CLONE: &str = "/Users/tom/.aion/clones/{workflow_id}/repo";

#[test]
fn the_opt_in_puts_the_flag_in_the_argv() {
    let args = norn_args(RUN_CLONE, "default", true);
    assert!(
        args.iter().any(|arg| arg == ALLOW_DEGRADED_SESSION_FLAG),
        "the opt-in must reach norn's argv; got {args:?}"
    );
}

#[test]
fn without_the_opt_in_the_flag_is_absent() {
    // The negative control: an opt-in that is always passed is not an opt-in.
    let args = norn_args(RUN_CLONE, "default", false);
    assert!(
        !args.iter().any(|arg| arg == ALLOW_DEGRADED_SESSION_FLAG),
        "the flag must NOT be passed unless the operator asked for it; got {args:?}"
    );
}

#[test]
fn the_argv_is_otherwise_identical_with_and_without_the_opt_in() {
    // The flag is purely additive: it must not perturb the workspace
    // confinement, the pinned session id, or the account.
    let without = norn_args(RUN_CLONE, "default", false);
    let with = norn_args(RUN_CLONE, "default", true);
    assert_eq!(
        with.len(),
        without.len() + 1,
        "the opt-in must add exactly one argument"
    );
    assert_eq!(
        &with[..without.len()],
        without.as_slice(),
        "the opt-in must not reorder or replace any existing argument"
    );
}

#[test]
fn the_argv_carries_the_confinement_the_session_pin_and_the_account() {
    let args = norn_args(RUN_CLONE, "work-alias", false);
    for (flag, value) in [
        ("--workspace-root", RUN_CLONE),
        ("-C", RUN_CLONE),
        ("--session-id", "{workflow_id}-{activity_type}"),
        ("--account", "work-alias"),
    ] {
        let at = args.iter().position(|arg| arg == flag);
        assert!(at.is_some(), "{flag} is missing from {args:?}");
        let index = at.unwrap_or_default();
        assert_eq!(
            args.get(index + 1).map(String::as_str),
            Some(value),
            "{flag} must be followed by {value}"
        );
    }
    assert!(
        args.iter().any(|arg| arg == "--resume-if-exists"),
        "the session pin is what makes a multi-round conversation continuous"
    );
}

#[test]
fn the_recorded_refusal_is_recognised() {
    assert!(
        is_degraded_session_refusal(RECORDED_REFUSAL),
        "the refusal recorded on the real run must be recognised"
    );
}

#[test]
fn unrelated_failures_are_not_mistaken_for_it() {
    for detail in [
        "harness reported failure: exit code 1",
        "storage error: open_session failed: no such session",
        // Names the condition but makes no approval demand.
        "session is a fresh-epoch projection",
        // Makes the demand but for a different condition.
        "explicit resume approval is required for forked sessions",
        "",
    ] {
        assert!(
            !is_degraded_session_refusal(detail),
            "must not claim the degraded-session condition for: {detail}"
        );
    }
}

#[test]
fn the_advice_names_the_flag_and_states_the_tradeoff() {
    let advice = degraded_session_advice(RECORDED_REFUSAL, false);
    assert!(advice.is_some(), "the recorded refusal must produce advice");
    let advice = advice.unwrap_or_default();
    assert!(
        advice.contains("--norn-allow-degraded-session"),
        "the advice must name the worker flag, not norn's: {advice}"
    );
    assert!(
        advice.contains("fidelity"),
        "the advice must state the tradeoff, not just the escape hatch: {advice}"
    );
    assert!(
        advice.contains(RECORDED_REFUSAL),
        "the advice must PRECEDE, not replace, the underlying refusal: {advice}"
    );
}

#[test]
fn the_advice_is_withheld_once_the_opt_in_was_given() {
    // Pointing at a flag that is already in the argv sends the operator in a
    // circle; at that point norn refused for some other reason.
    assert_eq!(
        degraded_session_advice(RECORDED_REFUSAL, true),
        None,
        "advice must not repeat a flag the operator already passed"
    );
}

#[test]
fn unrelated_failures_are_passed_through_unadvised() {
    assert_eq!(
        degraded_session_advice("harness reported failure: exit code 1", false),
        None,
        "only the degraded-session refusal gets this advice"
    );
}
