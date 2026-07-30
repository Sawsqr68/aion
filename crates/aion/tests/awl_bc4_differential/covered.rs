//! Deliverable 2 + 5: the differential over the covered ratchet. Every fixture
//! the oracle covers runs through a real engine; the normalized durable trails
//! must be byte-identical, and the EXACT outcome inventory — every name in every
//! bucket — is pinned so any drift (a name swap, a completed->failed flip, a
//! failed->cancelled semantic regression, an intersection erosion, an infra
//! defect) fails loudly instead of quiescing.

use std::collections::BTreeSet;

use crate::driver::run_differential;
use crate::fixtures;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The exact covered corpus size (`aion-awl/src/mir/covered.rs`).
///
/// 84 since declarative action bodies: `declarations/valid/action_run_bodies`
/// joined the corpus because a `run` body is a property of the ACTION
/// DECLARATION, not of the flow — the call site lowers exactly as a bodyless
/// action's does and the body rides the package contract, so both backends
/// must still produce byte-identical trails. Verified by running the
/// differential, not asserted from the reasoning.
///
/// 85 since outbound signals: `step-bodies/valid/send_signal` joined the corpus
/// with the `send` statement. A send is a durable outward effect that both
/// backends must lower identically, so it belongs in the differential and not
/// merely in aion-awl's own ratchet. Its bucket below was read off the run, not
/// predicted.
const COVERED_COUNT: usize = 86;

/// Out-of-oracle (r2 finding 3): bare signal-wait fixtures whose durable park
/// boundary is NOT observable through any engine surface without racy sampling.
/// `SignalResumeHandoff::pending_count` reports queued signals, not a registered
/// wait, and visibility reports only `Running`; there is no positive
/// signal-park evidence. Per decision-16 (simplification over a racy proof) they
/// are EXCLUDED from the oracle and pinned here, rather than classified from
/// 120ms of history silence. They still lower, emit, and are covered by
/// aion-awl's own ratchet; only their differential RUN is out-of-oracle.
const EXCLUDED_OUT_OF_ORACLE: &[&str] = &[
    "header-types/valid/combined",
    "header-types/valid/signal_wait",
];

/// The fixtures that park at a durable TIMER, proven by visibility `Running`
/// plus this exact pending-timer identity (the first anonymous timer).
// Each parked fixture's authored workflow timeout arms a run-scoped deadline
// alongside its awaited timer; the deadline's run-uuid is aliased to `<run>`
// by the driver before the evidence lands here.
const EXPECTED_TIMER_PARKED: &[(&str, &[&str])] = &[
    (
        "loop-outcomes/valid/guard_optional_wait",
        &["timer:anonymous:0", "timer:named:deadline:<run>"],
    ),
    (
        "step-bodies/valid/wait_and_sleep",
        &["timer:anonymous:0", "timer:named:deadline:<run>"],
    ),
    (
        "step-bodies/valid/wait_timeout_optional",
        &["timer:anonymous:0", "timer:named:deadline:<run>"],
    ),
];

/// This corpus records no cancellation; the bucket is asserted empty so a
/// `WorkflowFailed` -> `WorkflowCancelled` regression cannot hide.
const EXPECTED_CANCELLED: &[&str] = &[];

/// The covered fixtures that terminate in `WorkflowFailed` — BYTE-IDENTICALLY on
/// both backends — under the minimal schema-valid inputs and activity results
/// the harness feeds. Two honest kinds, pinned so any flip is loud:
/// 1. Data-driven error outcomes (`AwlOutcomeFailure`): the workflow runs its
///    real body (activities decode and complete) and a `route failure` / else
///    branch is taken because the minimal data goes there (a `score 0.0 < 0.5`
///    guard, an empty collection, a zero-count loop).
/// 2. Child-spawning fixtures (`AwlChildFailed`): the synthesized child workflow
///    type is not registered for spawning in the differential's package, so the
///    parent fails at the child boundary — `child_call_awaited`,
///    `child_spawn_combo`, `spawn_detached`, proven in
///    `child_spawning_fixtures_fail_at_the_child_boundary`.
///    (`declarations_combined` also declares a child but takes a data-driven
///    route-failure first, so it belongs to kind 1.)
const EXPECTED_FAILED: &[&str] = &[
    "dag-fork/valid/fork_named_branches",
    "declarations/valid/call_site_override",
    "declarations/valid/child_call_awaited",
    "declarations/valid/child_spawn_combo",
    "declarations/valid/declarations_combined",
    "declarations/valid/spawn_detached",
    "declarations/valid/worker_action_config_lines",
    // The on-failure edge, arm EXECUTED. `fail_publish` fails by name (the
    // `fail_*` convention at `fixtures::action_results`), so the guarded body
    // takes its error edge, the compensation call RUNS, and the run ends on
    // the failure outcome the compensation routed to — the only fixtures in
    // this corpus whose compensation is executed rather than merely lowered.
    "flow-shape/valid/on_failure_compensates_to_outcome",
    // A deliberate `route <failure outcome>` as the guarded body's TAIL. The
    // publish SUCCEEDS here; the route fires and the compensation must NOT —
    // `AwlOutcomeFailure` is never mistaken for a step failure.
    "flow-shape/valid/on_failure_route_tail",
    // `on failure` on `publish`; the publish SUCCEEDS on the canned data, so
    // the compensation is lowered but not taken and the run takes the
    // data-driven `nothing was published` route.
    "loop-outcomes/valid/on_failure_compensation",
    // `on failure` on `publish`; the run never reaches it — the zero-round
    // build loop takes the data-driven `not_ready` route first.
    "loop-outcomes/valid/ship_release_combined",
    "header-types/valid/builtins",
    "header-types/valid/zero_inputs",
    "loop-outcomes/valid/backward_route_bounded_cycle",
    "loop-outcomes/valid/float_threshold_guard",
    "loop-outcomes/valid/fork_in_loop_live_ins",
    "loop-outcomes/valid/loop_after_fall_through",
    "loop-outcomes/valid/loop_compound_until_nested",
    "loop-outcomes/valid/loop_counting_until_max",
    "loop-outcomes/valid/loop_without_counting",
    "loop-outcomes/valid/route_outcome_by_name",
    "schema-doors/valid/import_constraints",
    "schema-doors/valid/import_nested_defs",
    "schema-doors/valid/import_ticket",
    "schema-doors/valid/inline_schema_round",
    "step-bodies/valid/collection_predicates",
    "step-bodies/valid/index_and_concat",
    "step-bodies/valid/literal_forms",
    "step-bodies/valid/predicates_and_operators",
    // The read-only surface. It runs its real body — five answers in
    // `announce`, one re-registered per loop pass — and then takes the
    // data-driven `rejected` route, because the harness's canned `evaluate`
    // returns `approved: false` and the loop exhausts its bound. What this pin
    // asserts is what the differential measures: both backends REACH that
    // outcome and record byte-identical normalized trails with the answers in
    // the body, so neither lowering of `answer` faults, reorders, or diverges.
    // That an answer appends no EVENT is a separate property, proven where it
    // is decidable — `RuntimeFn::QueryHandler` belongs to no durable family
    // (`aion-awl`'s `query_handler_is_not_a_durable_effect`).
    "step-bodies/valid/query_answer",
    // Fails on its FIRST statement, `send heartbeat to coordinator_id`, because
    // the harness input supplies `coordinator_id: "x"` — not a live run. Read
    // off the trail, not inferred: the run reaches `WorkflowFailed` at seq 3
    // with NO `ActivityScheduled` at all, so the fault is the send's
    // unresolvable target and not the `evaluate` action below it. Both backends
    // record it byte-identically, which is what pins an outbound signal's
    // failure as a real durable effect rather than a silent no-op.
    //
    // Its recorded detail is a bare `{"tag":"AwlFailed"}` — no signal name, no
    // target, no cause — where an authored failure route records
    // `AwlOutcomeFailure` with its outcome and payload. That asymmetry is a
    // diagnosis gap tracked separately; it is NOT what this pin asserts, and
    // this comment exists so nobody reads the bucket as a blessing of it.
    "step-bodies/valid/send_signal",
];

/// The covered fixtures that COMPLETE their real path (`WorkflowCompleted`).
const EXPECTED_SUCCEEDED: &[&str] = &[
    "dag-fork/valid/after_single",
    "dag-fork/valid/child_collection_fork",
    "dag-fork/valid/child_collection_fork_sequential",
    "dag-fork/valid/empty_fork_collection",
    "dag-fork/valid/fall_through_chain",
    "dag-fork/valid/fork_action_fanout",
    "dag-fork/valid/fork_collection_join",
    "dag-fork/valid/fork_named_homogeneous",
    "dag-fork/valid/fork_sequential",
    "dag-fork/valid/fork_sequential_route",
    "dag-fork/valid/runtime_sized_fork",
    "dag-fork/valid/sit_one",
    // A declared `run` body rides the package CONTRACT, not the flow, so the
    // call site must lower exactly as a bodyless action's does. This entry is
    // the proof rather than the claim: the fixture completes on both backends
    // with byte-identical normalized trails and zero divergences.
    "declarations/valid/action_run_bodies",
    // R5: an advisory side channel that always fails. It SUCCEEDS on both
    // backends — that is the whole point of the class, and the differential
    // proves the direct and Gleam lowerings route around the fault arm
    // identically.
    "declarations/valid/advisory_side_channel",
    "declarations/valid/timeout_inside_retry",
    "declarations/valid/worker_retry_backoff",
    "declarations/valid/worker_single_action",
    "declarations/valid/workers_multiple",
    "ergonomics/valid/flow_vocab_b1",
    "flagship/valid/awl_hello",
    "flow-shape/valid/distribute_activity_tolerant",
    "flow-shape/valid/distribute_child_collect",
    "flow-shape/valid/distribute_child_tolerant",
    // The on-failure edge, arm EXECUTED, compensating to a SIBLING STEP:
    // `fail_charge` fails by name, the compensation routes to `unwind`, and
    // the refund settles the workflow — a completed run whose completion is
    // reached THROUGH the failure edge.
    "flow-shape/valid/on_failure_compensates_to_step",
    "flow-shape/valid/region_pure_decision",
    "flow-shape/valid/sequence_activity_tolerant",
    "flow-shape/valid/sequence_region_loopback",
    "header-types/valid/doc_comments",
    "header-types/valid/enum",
    "header-types/valid/line_width",
    "header-types/valid/max_arity_record",
    "header-types/valid/minimal",
    "header-types/valid/noncanonical_commas",
    "header-types/valid/workflow_timeout",
    "loop-outcomes/valid/enum_when_totality",
    // `on failure` on `fulfil`; the ship SUCCEEDS on the canned data, so the
    // compensation is lowered but not taken and the run completes.
    "loop-outcomes/valid/nested_handlers",
    "schema-doors/valid/inline_verbatim_constraints",
    "schema-doors/valid/mixed_doors",
    "schema-doors/valid/optional_shorthand",
    "schema-doors/valid/short_circuit_optional",
    "step-bodies/valid/calls_and_side_effects",
    "step-bodies/valid/combinators",
    "step-bodies/valid/fallible_all_short_circuit",
    "step-bodies/valid/fallible_any_short_circuit",
    "step-bodies/valid/fallible_collection_predicates",
    "step-bodies/valid/general_concat",
    "step-bodies/valid/pipe_chain_stages",
    "step-bodies/valid/step_bodies_combined",
    "step-bodies/valid/unicode_payloads",
    "step-bodies/valid/workflow_id",
];

/// Flagship de-risk: the first AWL workflow completes its real greet -> shout
/// path with a byte-identical trail across both backends.
#[tokio::test(flavor = "multi_thread")]
async fn flagship_awl_hello_matches() -> TestResult {
    if crate::gleam_test_support::skip_if_unavailable() {
        return Ok(());
    }
    let names = vec![String::from("flagship/valid/awl_hello")];
    let report = run_differential(&names, "flagship", &[]).await?;
    assert!(
        report.infra.is_empty() && report.divergences.is_empty() && report.refusals.is_empty(),
        "flagship not clean:\n{}",
        report.render()
    );
    assert_eq!(
        report.succeeded,
        vec![String::from("flagship/valid/awl_hello")],
        "flagship did not complete its real path:\n{}",
        report.render()
    );
    Ok(())
}

/// The full covered differential: build both backends for every covered fixture
/// (ONE `gleam build`), run both, assert byte-identical normalized trails, and
/// pin the EXACT name inventory of every outcome bucket.
#[tokio::test(flavor = "multi_thread")]
async fn covered_set_is_byte_identical() -> TestResult {
    if crate::gleam_test_support::skip_if_unavailable() {
        return Ok(());
    }
    let names = fixtures::covered_paths()?;
    assert_eq!(
        names.len(),
        COVERED_COUNT,
        "covered ratchet size drifted (expected {COVERED_COUNT})"
    );
    let report = run_differential(&names, "covered", EXCLUDED_OUT_OF_ORACLE).await?;
    println!("{}", report.render());

    assert!(
        report.infra.is_empty(),
        "covered differential hit infrastructure failures:\n{}",
        report.render()
    );
    assert!(
        report.divergences.is_empty(),
        "covered differential found divergences:\n{}",
        report.render()
    );
    assert!(
        report.refusals.is_empty(),
        "covered fixtures unexpectedly refused (intersection shrinkage):\n{}",
        report.render()
    );
    // Both backends agreeing on a refusal IS a comparison result, and the
    // report says so — but it is pinned EMPTY here, because a fixture neither
    // backend would run proves nothing about lowering. This bucket exists so
    // that outcome is named correctly when it happens; it is not a pass.
    assert!(
        report.agreed_refusal_fixtures().is_empty(),
        "covered fixtures were refused by BOTH backends — agreement, but the \
         fixture bodies never executed:\n{}",
        report.render()
    );

    // Exact per-bucket NAME pins (r2 finding 2).
    assert_eq!(
        report.succeeded_fixtures(),
        sorted(EXPECTED_SUCCEEDED),
        "succeeded set drifted:\n{}",
        report.render()
    );
    assert_eq!(
        report.failed_fixtures(),
        sorted(EXPECTED_FAILED),
        "failed set drifted:\n{}",
        report.render()
    );
    assert_eq!(
        report.cancelled_fixtures(),
        sorted(EXPECTED_CANCELLED),
        "cancellation must be empty for this corpus:\n{}",
        report.render()
    );
    assert_eq!(
        report.excluded_fixtures(),
        sorted(EXCLUDED_OUT_OF_ORACLE),
        "excluded set drifted:\n{}",
        report.render()
    );

    // Timer parks pinned to their exact pending-timer identity (r2 finding 3).
    let expected_timer: Vec<(String, Vec<String>)> = EXPECTED_TIMER_PARKED
        .iter()
        .map(|(name, timers)| {
            (
                (*name).to_owned(),
                timers.iter().map(|timer| (*timer).to_owned()).collect(),
            )
        })
        .collect();
    assert_eq!(
        report.timer_parked_evidence(),
        expected_timer,
        "timer-park set or pending-timer identity drifted:\n{}",
        report.render()
    );

    // The union of every bucket is EXACTLY the distinct covered names.
    let mut union: BTreeSet<String> = BTreeSet::new();
    let parked_names = report.parked_timer.iter().map(|(name, _)| name);
    for name in report
        .succeeded
        .iter()
        .chain(&report.failed)
        .chain(&report.cancelled)
        .chain(parked_names)
        .chain(&report.excluded)
    {
        assert!(union.insert(name.clone()), "duplicate outcome for {name}");
    }
    let covered: BTreeSet<String> = names.into_iter().collect();
    assert_eq!(
        union, covered,
        "the outcome union is not exactly the covered corpus"
    );
    Ok(())
}

/// Sorts a static name list into an owned, comparable vector.
fn sorted(names: &[&str]) -> Vec<String> {
    let mut owned: Vec<String> = names.iter().map(|name| (*name).to_owned()).collect();
    owned.sort();
    owned
}
