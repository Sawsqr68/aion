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
///
/// 88 since the `agent` marker: `declarations/valid/agent_seam` and its control
/// arm `agent_seam_unmarked_control` — the same document with and without the
/// marker. `agent` classifies the DECLARATION (a worker routes the call to an
/// agent harness), so the two must run identically as well as lower
/// identically, and running both here is what proves the marker is inert at
/// run time rather than merely inert in the MIR.
///
/// 98 since value route payloads (`route out(<value>)`): seven fixtures joined
/// at once — `action_param_hygiene`, `counter_hygiene`, `region_close_bounded`,
/// `region_nested_local_reuse`, `region_sibling_local_reuse`,
/// `temporary_hygiene`, `value_route_payload`. A value payload is the one
/// route form whose payload is an arbitrary EXPRESSION evaluated at the route,
/// so it is exactly the form where the direct path could plausibly route a
/// different term than the oracle (a missed optional wrap, a dropped concat
/// prelude, a temporary captured from the wrong scope). Their buckets below
/// were read off the run, not predicted.
/// 101 since `subflow` declarations: `dev_flow`, `subflow_local_names`, and
/// `subflow_nested` joined at once. A subflow compiles INLINE — it is not a
/// child workflow, not an activity, and appends no event of its own — so the
/// only way to prove the direct path's inline function set behaves as the
/// reference's does is to run both and compare the durable trail the enclosing
/// workflow records. Their buckets below were read off the run, not predicted.
/// 105 since dependency-parallel layers: `after_multi_diamond`,
/// `release_pipeline_combined`, `dev_brief`, and `layer_hygiene` joined at
/// once. A parallel layer is the one construct whose two lowerings could agree
/// on every VALUE and still disagree on the durable trail — a layer dispatched
/// concurrently records both activities scheduled before either completes,
/// while the same layer run in written order records a completed round trip
/// between them. Nothing short of running both backends and comparing the
/// recorded event order decides that, which is why all four are here and not
/// only in aion-awl's ratchet. Their buckets below were read off the run, not
/// predicted.
///
/// 107 since substeps — the LAST refusal class the direct lowering path held.
/// `flow-shape/valid/substep_on_failure_route_tail` and
/// `loop-outcomes/valid/substeps_two_stage` joined together, and with them every
/// valid fixture in the corpus lowers. Both belong here rather than only in
/// aion-awl's ratchet because a substep group is a CONTROL-FLOW split: the
/// direct path lowers each substep into its own tail-called function where the
/// reference nests them inside the parent's, so the two could agree on every
/// value and still record a different sequence of activity round trips. The
/// `on failure` arm sharpens that further — a substep's compensation is lifted
/// out on the direct path and inlined in an `Error(_) ->` arm on the reference,
/// so only a run decides whether the routed failure outcome triggers
/// compensation on one backend and not the other. Their buckets below were read
/// off the run, not predicted.
///
/// 111 since the harness section (#204): the four `harness-section/valid/*`
/// fixtures — both harness kinds, the optional-settings-omitted shape, and the
/// harness-absent control. A `harness` section is serve-surface configuration
/// the lowering must CARRY WITHOUT CHANGING CONTROL FLOW, so the differential
/// question is whether its presence perturbs the lowered workflow at all; all
/// four ran byte-identical and to completion on both backends (0 divergences,
/// 0 infra), and their `EXPECTED_SUCCEEDED` entries below were read off that
/// run, not predicted.
///
/// 112 since the bound `cwd` (#228): `harness-section/valid/
/// norn_harness_with_a_bound_cwd`, the binding form whose agent action carries
/// the working directory beside the prompt. It joined the compiler's covered
/// corpus at `2066b3183` WITHOUT this ratchet moving with it, so from that
/// commit until this one both tests in this file died on the size assertion
/// ABOVE any differential work — which means the fixture had never been
/// differentially run at all, and the whole byte-identity question went
/// unmeasured rather than answered. It belongs in the differential on its own
/// merits: the directory is an extra parameter on the activity's input, so a
/// lowering that dropped it, or carried it anywhere but the input, would change
/// the trail. Its bucket below was read off the run, not predicted.
const COVERED_COUNT: usize = 112;

/// Out-of-oracle (r2 finding 3): fixtures whose durable park boundary is NOT
/// observable through any engine surface without racy sampling.
/// `SignalResumeHandoff::pending_count` reports queued signals, not a registered
/// wait, and visibility reports only `Running`; there is no positive
/// signal-park evidence. Per decision-16 (simplification over a racy proof) they
/// are EXCLUDED from the oracle rather than classified from 120ms of history
/// silence. They still lower, emit, and are covered by aion-awl's own ratchet;
/// only their differential RUN is out-of-oracle.
///
/// This list is DERIVED, not enumerated. It was originally populated by naming
/// the fixtures that had been SEEN to flake, and the prose above ("bare
/// signal-wait fixtures") was therefore true of the class while the list held
/// only two of its three members. `step-bodies/valid/send_signal` was the
/// missing one: it was classified `WorkflowFailed` from an uncontended run and
/// sat in the covered oracle as a latent load-sensitive flake, surfacing as a
/// `disposition=Parked` vs `disposition=Failed` divergence only when the box
/// was loaded enough to change which side of its 120ms window the run landed
/// on. It was never a lowering defect on either backend.
///
/// [`the_exclusion_list_is_exactly_the_unobservable_park_population`] now
/// COMPUTES the population from the property below and asserts this list equals
/// it, so a future fixture carrying the same hazard joins the list by failing
/// that test rather than by flaking in someone's differential months later.
///
/// The property: the document contains a signal `wait` with NO `timeout`
/// clause, and contains NO durable timer anywhere (`sleep`, or a `wait`
/// carrying a timeout). The second half is what keeps
/// `step-bodies/valid/wait_and_sleep` OUT of this list even though it does
/// carry a bare `wait go`: its `sleep 30s` parks first on a real durable timer,
/// which IS positive evidence, and it is pinned in [`EXPECTED_TIMER_PARKED`] on
/// that timer's exact identity. Per-attempt ACTIVITY timeouts (`node writer,
/// timeout 10m` in `header-types/valid/combined`) are deliberately not durable
/// timers here — which is precisely why the property is computed over the
/// parsed AST and never by grepping the source for `timeout`.
const EXCLUDED_OUT_OF_ORACLE: &[&str] = &[
    "header-types/valid/combined",
    "header-types/valid/signal_wait",
    "step-bodies/valid/send_signal",
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
    // Dependency-parallel layers, kind 1 — both run their real bodies and are
    // sent to a failure outcome by the canned data, and both recorded trails
    // were READ (`BC4_DUMP_FAILED`) rather than inferred from the bucket name:
    //
    // `after_multi_diamond` gathers, dispatches `extract_claims` (seq 6-7) and
    // `collect_citations` (seq 8-9) — both SCHEDULED AND STARTED before either
    // completes at seq 10-11, which is the recorded shape of a concurrent
    // layer — composes, and then takes its `otherwise` arm because the canned
    // `Report.complete` is the minimal Bool `false`:
    // `{"outcome":"incomplete","tag":"AwlOutcomeFailure"}`.
    //
    // `dev_brief` provisions, dispatches `scout` and `warm_build` with the same
    // interleave, runs one `fix_round`, and takes `route exhausted` because the
    // canned `Round.gates_green` is `false` and `config.max_fix_cycles`
    // generates as the minimal Int.
    //
    // Both fixtures therefore pin WHERE THEIR DATA SENDS THEM, after the
    // parallel layer has run; neither failure is in the dispatch. That the
    // direct backend records the same interleave byte-for-byte is the claim
    // this corpus exists to make.
    "dag-fork/valid/after_multi_diamond",
    "flagship/valid/dev_brief",
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
    // The same invariant one level down, inside a SUBSTEP: `publish` succeeds
    // on the canned data, the substep's terminal `route finish` fires the
    // PARENT's arm, and that arm's `route failed("substep finished")` is the
    // recorded end. The compensation must not run — which the trail decides,
    // because a `cleanup` activity round trip would appear in it.
    "flow-shape/valid/substep_on_failure_route_tail",
    // A value payload built by a binary CONCAT (`status + workflow.id`), and
    // the only one of the seven that takes a failure path. Kind 1, and the
    // fault is NOT in the payload: the fixture opens with `reviewers[0]` over
    // an `[String]` input, and the generator emits an empty array for an
    // unconstrained list (no `minItems`), so the index faults before the loop
    // or either route is reached — the same data-driven index fault that puts
    // `step-bodies/valid/index_and_concat` (identical shape) in this bucket.
    // Both backends record it byte-identically. Its payload lowering is proven
    // where it IS decidable: `mir::deferred_tests::value_route_payloads_now_lower`
    // lowers and verifies the concat, and this entry is pinned so a future
    // input generator that fills the list would move it loudly rather than
    // quietly changing what this fixture proves.
    "flow-shape/valid/temporary_hygiene",
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
    // The substep SIBLING CHAIN, kind 1: `fetch_batch` runs its activity and
    // the canned minimal `Batch.rows` sends it down the `nothing` arm, which
    // routes to the parent's `hollow` clause and out through `hollow_source`.
    // The chain is what this pins: the direct path reaches that arm through a
    // frame-resolved parent-clause fire inside a tail-called substep function,
    // the reference through a nested `case` in one function, and the recorded
    // trail must be identical either way.
    "loop-outcomes/valid/substeps_two_stage",
    "schema-doors/valid/import_constraints",
    "schema-doors/valid/import_nested_defs",
    "schema-doors/valid/import_ticket",
    "schema-doors/valid/inline_schema_round",
    "step-bodies/valid/collection_predicates",
    "step-bodies/valid/index_and_concat",
    "step-bodies/valid/literal_forms",
    // #97-B: guard-dependent optionality narrowing keys on an access PATH, so
    // an optional FIELD narrows the same way an optional binding always has.
    // Both are pinned because the two positions are different machinery: the
    // guard's short-circuit edge, and the arm's own proof.
    "step-bodies/valid/guard_chain_carries_every_proof",
    "step-bodies/valid/guard_narrows_a_deep_optional_field",
    "step-bodies/valid/guard_narrows_an_optional_field",
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
    // The DEGRADED dependency-parallel layer: `build_all` (a collection fork)
    // and `notes` (a bare call) share a layer that cannot ride a
    // `workflow.all`, so both backends run it in WRITTEN ORDER. That is the
    // arm where the two could most easily disagree — the direct path splits
    // the layer into two tail-called functions where the reference inlines
    // both into one — and a completed run with a byte-identical trail is what
    // proves the split changed the code shape and not the recorded order.
    "dag-fork/valid/release_pipeline_combined",
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
    // The `agent` marker and its control arm — the same document with and
    // without it. BOTH complete with byte-identical normalized trails, which is
    // the run-time half of the claim that `agent` classifies the DECLARATION
    // and never touches the flow. (The lowering half is
    // `mir::agent_control_tests` in aion-awl, which diffs their MIR goldens.)
    // Read off this run, not predicted.
    "declarations/valid/agent_seam",
    "declarations/valid/agent_seam_unmarked_control",
    "declarations/valid/timeout_inside_retry",
    "declarations/valid/worker_retry_backoff",
    "declarations/valid/worker_single_action",
    "declarations/valid/workers_multiple",
    "ergonomics/valid/flow_vocab_b1",
    "flagship/valid/awl_hello",
    // Value route payloads, name-hygiene arms: an author binding that collides
    // with a synthesized wrapper temporary (`awl_input_codec`) and one that
    // collides with the visit-counter candidate (`awl_visits_again_188`). Both
    // COMPLETE on both backends with byte-identical trails, which is what
    // proves the payload expression reads the author's binding and not the
    // synthesized one that shares its name.
    "flow-shape/valid/action_param_hygiene",
    "flow-shape/valid/counter_hygiene",
    // `subflow` declarations, flagship arm: `dev_flow` carries BOTH nested
    // flow kinds in one document — a subflow whose bounded `review` step
    // routes backward to `develop`, and a `distribute`/`collect ?` region
    // whose member step INVOKES that subflow per item. It completes on both
    // backends with byte-identical normalized trails, which is what proves
    // the inline function set runs the same activities in the same order as
    // the reference's — a subflow appends no event of its own, so the
    // enclosing workflow's trail is the only place the difference could show.
    "flow-shape/valid/dev_flow",
    "flow-shape/valid/distribute_activity_tolerant",
    "flow-shape/valid/distribute_child_collect",
    "flow-shape/valid/distribute_child_tolerant",
    // The dependency-parallel layer, CONCURRENT arm, completing its real path:
    // `left` and `right` both follow `root` and `merge` joins them. It is also
    // the name-hygiene arm — the document's own input is called `awl_layer`,
    // the generated name the reference's typed parallel layer allocates — so a
    // completed run with a byte-identical trail is what proves the reference's
    // allocator dodged its own collision while the direct path, which names
    // nothing, never had one.
    "flow-shape/valid/layer_hygiene",
    // The on-failure edge, arm EXECUTED, compensating to a SIBLING STEP:
    // `fail_charge` fails by name, the compensation routes to `unwind`, and
    // the refund settles the workflow — a completed run whose completion is
    // reached THROUGH the failure edge.
    "flow-shape/valid/on_failure_compensates_to_step",
    // Value payloads carried out of REGIONS: a literal on a bounded region's
    // close, and the piped `count` of the collected rows out of nested and
    // sibling regions whose locals are reused at unrelated types. All three
    // complete identically on both backends, so the payload expression is
    // resolved against the region's own scope on both paths.
    "flow-shape/valid/region_close_bounded",
    "flow-shape/valid/region_nested_local_reuse",
    "flow-shape/valid/region_pure_decision",
    "flow-shape/valid/region_sibling_local_reuse",
    "flow-shape/valid/sequence_activity_tolerant",
    "flow-shape/valid/sequence_region_loopback",
    // The subflow scoping arms. `subflow_local_names` reuses the same author
    // names (`value`, `out`) across two subflows at unrelated types and again
    // in the host; `subflow_nested` has a subflow invoke a subflow. Both
    // complete identically on both backends, so each flow's binding
    // environment is resolved independently on the direct path exactly as the
    // reference resolves it.
    "flow-shape/valid/subflow_local_names",
    "flow-shape/valid/subflow_nested",
    // The canonical value payload: `route done(note)` at the workflow
    // boundary, completing its real work -> route path on both backends.
    "flow-shape/valid/value_route_payload",
    "harness-section/valid/acp_harness",
    "harness-section/valid/harness_absent_control",
    "harness-section/valid/norn_harness",
    "harness-section/valid/norn_harness_with_a_bound_cwd",
    "harness-section/valid/norn_harness_without_optional_settings",
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

/// Every covered fixture must declare a DISTINCT workflow name.
///
/// The reference backend builds the whole corpus as ONE Gleam project, and a
/// fixture's emitted module is named for its workflow — so two covered
/// fixtures sharing a workflow name would write the same `src/<name>.gleam`
/// and only one of them could exist. The build refuses with
/// `ConfigInvalid { field: "workflow[N].entry_module" }`, which names an index
/// into a generated list and neither fixture path, so the corpus constraint is
/// asserted here where it can name both.
///
/// This is not hypothetical: `dag-fork/valid/release_pipeline_combined` and
/// `declarations/valid/declarations_combined` both declared `workflow
/// release_pipeline`, and the clash lay dormant for as long as only one of
/// them was covered. The rev-2 tree still holds several such duplicate names
/// among fixtures that are NOT both covered, so the next coverage landing can
/// walk into the same wall; this test is what turns that into a legible
/// failure naming the two fixtures and the name they share.
#[test]
fn every_covered_fixture_declares_a_distinct_workflow_name() -> TestResult {
    let names = fixtures::covered_paths()?;
    let mut seen: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    let mut clashes = Vec::new();
    for name in &names {
        let loaded = fixtures::load(name)?;
        let workflow = loaded.document.name.clone();
        if let Some(prior) = seen.insert(workflow.clone(), name.clone()) {
            clashes.push(format!("`{workflow}`: {prior} and {name}"));
        }
    }
    assert!(
        clashes.is_empty(),
        "covered fixtures share workflow names, so the reference project cannot \
         hold both: {clashes:?}"
    );
    Ok(())
}

/// Sorts a static name list into an owned, comparable vector.
fn sorted(names: &[&str]) -> Vec<String> {
    let mut owned: Vec<String> = names.iter().map(|name| (*name).to_owned()).collect();
    owned.sort();
    owned
}

/// Applies `visit` to every statement in `statements`, descending into every
/// nesting form a statement can carry.
///
/// Exhaustive by CONSTRUCTION: the match lists every `Statement` variant with
/// no wildcard arm, so a new nesting form added to the AST fails to compile
/// here instead of silently hiding a wait from the population below.
fn walk_statements(
    statements: &[aion_awl::Statement],
    visit: &mut impl FnMut(&aion_awl::Statement),
) {
    for statement in statements {
        visit(statement);
        match statement {
            aion_awl::Statement::Fork(fork) => walk_statements(&fork.body, visit),
            aion_awl::Statement::Loop(looped) => walk_statements(&looped.body, visit),
            aion_awl::Statement::SubStep(sub) => walk_step(sub, visit),
            aion_awl::Statement::Call(_)
            | aion_awl::Statement::Spawn(_)
            | aion_awl::Statement::Wait(_)
            | aion_awl::Statement::Send(_)
            | aion_awl::Statement::Answer(_)
            | aion_awl::Statement::Sleep(_)
            | aion_awl::Statement::Pipe(_)
            | aion_awl::Statement::Route(_)
            | aion_awl::Statement::Distribute(_)
            | aion_awl::Statement::Collect(_) => {}
        }
    }
}

/// Applies `visit` to every statement of a step, including its `on failure`
/// edge — a bare wait inside a compensation body parks exactly as one in the
/// guarded body does.
fn walk_step(step: &aion_awl::Step, visit: &mut impl FnMut(&aion_awl::Statement)) {
    walk_statements(&step.body, visit);
    if let Some(on_failure) = &step.on_failure {
        walk_statements(&on_failure.body, visit);
    }
}

/// Whether a document's only durable park boundary is a bare signal wait — the
/// property that makes a fixture unobservable to the oracle. See
/// [`EXCLUDED_OUT_OF_ORACLE`] for why each half is needed.
fn parks_only_on_a_bare_signal_wait(document: &aion_awl::Document) -> bool {
    let mut bare_wait = false;
    let mut durable_timer = false;
    let mut visit = |statement: &aion_awl::Statement| match statement {
        aion_awl::Statement::Wait(wait) => {
            if wait.timeout.is_some() {
                durable_timer = true;
            } else {
                bare_wait = true;
            }
        }
        aion_awl::Statement::Sleep(_) => durable_timer = true,
        _ => {}
    };
    for step in &document.steps {
        walk_step(step, &mut visit);
    }
    for subflow in &document.subflows {
        for step in &subflow.steps {
            walk_step(step, &mut visit);
        }
    }
    bare_wait && !durable_timer
}

/// The exclusion list is exactly the population that carries the hazard — it is
/// DERIVED here rather than trusted as a list.
///
/// This is the test that closes r2 finding 3's real defect. That finding
/// populated [`EXCLUDED_OUT_OF_ORACLE`] by naming the fixtures observed to
/// flake, which is an enumeration standing in for an invariant: it is correct
/// only until a fixture carrying the same hazard is added, or until one already
/// present has simply not flaked yet on an unloaded box. Exactly that happened
/// to `step-bodies/valid/send_signal`.
///
/// Asserting set EQUALITY (not mere containment) is what makes this a gate in
/// both directions: a hazardous fixture missing from the list fails, and so
/// does a name parked in the list that no longer carries the hazard — so the
/// exclusion can never quietly grow into a dumping ground for anything
/// inconvenient.
#[test]
fn the_exclusion_list_is_exactly_the_unobservable_park_population() -> TestResult {
    let names = fixtures::covered_paths()?;
    assert_eq!(
        names.len(),
        COVERED_COUNT,
        "covered ratchet size drifted (expected {COVERED_COUNT})"
    );
    let mut hazardous = Vec::new();
    for name in &names {
        let loaded = fixtures::load(name)?;
        if parks_only_on_a_bare_signal_wait(&loaded.document) {
            hazardous.push(name.clone());
        }
    }
    hazardous.sort();
    assert_eq!(
        hazardous,
        sorted(EXCLUDED_OUT_OF_ORACLE),
        "EXCLUDED_OUT_OF_ORACLE is not the computed bare-signal-wait population. \
         A fixture whose only durable park boundary is a bare signal wait has no \
         positive park evidence, so classifying it from history silence is \
         load-sensitive: it must be excluded, not pinned to whichever outcome an \
         unloaded box happened to produce."
    );
    Ok(())
}
