//! Tests for the norn argv.
//!
//! The negative controls carry the weight here: a flag only ever observed to
//! fire is untested in the case that matters. The degraded-session advice
//! tests moved with the machinery to `aion_integration_norn::advice`; what
//! stays here is the seam between THIS worker's argv and that machinery —
//! that the opt-in state the advice keys on is derivable from the argv this
//! worker actually composes, and that this worker's own flag spelling
//! survives into the advice verbatim.

use aion_integration_norn::{
    ALLOW_DEGRADED_SESSION_FLAG, argv_allows_degraded_session, degraded_session_advice,
};

use super::{DEGRADED_SESSION_OPT_IN, norn_args};

/// The exact refusal recorded in durable history for the 28-day-parked
/// assistant run — the evidence the advice path was built from, verbatim.
const RECORDED_REFUSAL: &str = "run/execute failed (code -32603): agent error: session error: \
     storage error: open_session failed: session \
     'f9b062c2-d260-43f9-8374-d3210330f567-assistant' is a fresh-epoch \
     projection; explicit resume approval is required";

const RUN_CLONE: &str = "/Users/tom/.aion/clones/{workflow_id}/repo";

#[test]
fn the_opt_in_puts_the_flag_in_the_argv() {
    let args = norn_args(RUN_CLONE, "default", None, true);
    assert!(
        args.iter().any(|arg| arg == ALLOW_DEGRADED_SESSION_FLAG),
        "the opt-in must reach norn's argv; got {args:?}"
    );
}

#[test]
fn without_the_opt_in_the_flag_is_absent() {
    // The negative control: an opt-in that is always passed is not an opt-in.
    let args = norn_args(RUN_CLONE, "default", None, false);
    assert!(
        !args.iter().any(|arg| arg == ALLOW_DEGRADED_SESSION_FLAG),
        "the flag must NOT be passed unless the operator asked for it; got {args:?}"
    );
}

#[test]
fn the_argv_is_otherwise_identical_with_and_without_the_opt_in() {
    // The flag is purely additive: it must not perturb the workspace
    // confinement, the pinned session id, or the account.
    let without = norn_args(RUN_CLONE, "default", None, false);
    let with = norn_args(RUN_CLONE, "default", None, true);
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
fn the_promoted_detection_reads_the_opt_in_from_this_workers_argv() {
    // The advice keys its quiet/fire state on the COMPOSED argv, so the argv
    // this worker builds must be legible to the promoted detection — in both
    // directions.
    assert!(argv_allows_degraded_session(norn_args(
        RUN_CLONE, "default", None, true
    )));
    assert!(!argv_allows_degraded_session(norn_args(
        RUN_CLONE, "default", None, false
    )));
}

#[test]
fn the_advice_names_this_workers_own_flag_spelling() {
    // The promoted advice carries the caller's opt-in instruction verbatim;
    // this worker's is its own `--norn-allow-degraded-session`, not norn's
    // flag and not the built-in verb's `--norn-arg` spelling.
    let advice = degraded_session_advice(RECORDED_REFUSAL, false, DEGRADED_SESSION_OPT_IN)
        .unwrap_or_default();
    assert!(
        advice.contains("--norn-allow-degraded-session"),
        "the advice must name the worker flag, not norn's: {advice}"
    );
}

#[test]
fn a_supplied_model_reaches_the_argv_as_norns_model_flag() {
    let args = norn_args(RUN_CLONE, "default", Some("gpt-5.6-sol"), false);
    let at = args.iter().position(|arg| arg == "--model");
    assert!(at.is_some(), "--model is missing from {args:?}");
    let index = at.unwrap_or_default();
    assert_eq!(
        args.get(index + 1).map(String::as_str),
        Some("gpt-5.6-sol"),
        "--model must be followed by the operator's value"
    );
}

#[test]
fn without_a_model_the_flag_is_absent_so_norn_chooses() {
    // The negative control: a pass-through with a smuggled default is not a
    // pass-through — omitting the flag must leave model selection to norn.
    let args = norn_args(RUN_CLONE, "default", None, false);
    assert!(
        !args.iter().any(|arg| arg == "--model"),
        "--model must NOT appear unless the operator supplied one; got {args:?}"
    );
}

#[test]
fn the_model_is_purely_additive_to_the_argv() {
    let without = norn_args(RUN_CLONE, "default", None, false);
    let with = norn_args(RUN_CLONE, "default", Some("gpt-5.6-sol"), false);
    assert_eq!(
        with.len(),
        without.len() + 2,
        "the model must add exactly the flag and its value"
    );
    assert_eq!(
        &with[..without.len()],
        without.as_slice(),
        "the model must not reorder or replace any existing argument"
    );
}

#[test]
fn the_argv_carries_the_confinement_the_session_pin_and_the_account() {
    let args = norn_args(RUN_CLONE, "work-alias", None, false);
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
