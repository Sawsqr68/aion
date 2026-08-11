//! The BC-2 coverage ratchet: the exact set of `valid/` fixtures the direct
//! path lowers (split from `tests` for the 500-line law).

/// The exact set of `valid/` fixtures this BC-2 increment lowers, pinned so a
/// regression from covered → refused (or a newly-covered fixture) fails the
/// ratchet instead of being silently absorbed by the deferred bucket. Paths
/// are relative to `tests/fixtures/rev2`, without the `.awl` extension.
pub(super) const COVERED: &[&str] = &[
    // Dependency-parallel layers (`step c after a, b`): a layer of single
    // bare action calls dispatches as ONE `workflow.all`, richer layers
    // degrade to written order with the S13 marker. `after_multi_diamond` is
    // the canonical diamond — two heterogeneous calls in one layer, joined by
    // a third step.
    "dag-fork/valid/after_multi_diamond",
    "dag-fork/valid/after_single",
    "dag-fork/valid/child_collection_fork",
    "dag-fork/valid/child_collection_fork_sequential",
    // BC-4 adversarial fixtures (rev-2): a fork over a possibly-empty input
    // collection (zero-item boundary) and a fork whose fan-out width is only
    // known at run time — both lower, so both join the ratchet and the
    // differential corpus.
    "dag-fork/valid/empty_fork_collection",
    "dag-fork/valid/fall_through_chain",
    "dag-fork/valid/fork_action_fanout",
    "dag-fork/valid/fork_collection_join",
    "dag-fork/valid/fork_named_branches",
    "dag-fork/valid/fork_named_homogeneous",
    "dag-fork/valid/fork_sequential",
    "dag-fork/valid/fork_sequential_route",
    // The DEGRADED half of the parallel-layer landing: `build_all` and `notes`
    // share a dependency layer, but `build_all`'s body is a collection fork
    // rather than one bare action call, so the layer cannot ride a
    // `workflow.all` and runs in written order — exactly as the reference
    // emitter degrades it, with the S13 `degraded_parallel` marker in the
    // golden where the reference emits its `awl stopgap` comment.
    "dag-fork/valid/release_pipeline_combined",
    // BC-4 adversarial: a runtime-sized fork over a plain input list.
    "dag-fork/valid/runtime_sized_fork",
    "dag-fork/valid/sit_one",
    // A declared `run` body is a property of the ACTION DECLARATION, not of
    // the flow: the calls lower exactly as a bodyless action's do, and the
    // body rides the package contract instead.
    "declarations/valid/action_run_bodies",
    // R5: an advisory side channel lowers past the fault arm.
    "declarations/valid/advisory_side_channel",
    // The `agent` marker is a DECLARATION classification, like `advisory` and
    // like a `run` body: it rides the package contract so a worker knows to
    // hand the call to a harness, and the call site lowers exactly as any
    // other action call does. Pinning it here is what proves that — a marker
    // that changed the lowering would be a marker that changed the flow's
    // meaning, which it must not.
    "declarations/valid/agent_seam",
    // The control arm for the line above: the same document WITHOUT the
    // marker. Both are pinned so `agent_marked_and_unmarked_lower_identically`
    // has two committed goldens to compare.
    "declarations/valid/agent_seam_unmarked_control",
    "declarations/valid/call_site_override",
    "declarations/valid/child_call_awaited",
    // BC-4 adversarial: await a child, then fire-and-forget a detached child.
    "declarations/valid/child_spawn_combo",
    "declarations/valid/declarations_combined",
    "declarations/valid/spawn_detached",
    // BC-4 adversarial: a per-attempt timeout nested inside a retry schedule.
    "declarations/valid/timeout_inside_retry",
    "declarations/valid/worker_action_config_lines",
    "declarations/valid/worker_retry_backoff",
    "declarations/valid/worker_single_action",
    "declarations/valid/workers_multiple",
    "ergonomics/valid/flow_vocab_b1",
    "flagship/valid/awl_hello",
    // The flagship arm of the parallel-layer landing: `scout` and `warm_build`
    // both declare `after provision`, forming a heterogeneous layer whose
    // second member binds nothing — so the join destructures a discarded
    // position and decodes only the bound one.
    "flagship/valid/dev_brief",
    // `subflow` declarations: an inline flow compiled once as a local
    // function set (wrapper + chains) and invoked by a plain local call.
    // `dev_flow` is the flagship arm — two nested flows in one document (a
    // subflow AND a `distribute`/`collect` region whose member step invokes
    // it), a bounded backward route inside the subflow, and the subflow's
    // outcome carried out as a value payload.
    "flow-shape/valid/dev_flow",
    // Value route payloads (`route out(<value>)`): the expression itself is
    // the payload, coerced toward the outcome type exactly as the reference
    // `render_arg_for` coerces it. These two are the NAME-HYGIENE arms —
    // an author binding that collides with a synthesized wrapper temporary
    // (`awl_input_codec`) or with the visit-counter candidate
    // (`awl_visits_again_188`) — so they prove the payload lowers without
    // the allocator capturing the author's own name.
    "flow-shape/valid/action_param_hygiene",
    "flow-shape/valid/counter_hygiene",
    "flow-shape/valid/distribute_activity_tolerant",
    "flow-shape/valid/distribute_child_collect",
    "flow-shape/valid/distribute_child_tolerant",
    // The NAME-HYGIENE arm of the parallel-layer landing: an author input
    // literally named `awl_layer`, the generated name the reference's typed
    // parallel layer allocates. The direct path names nothing — a layer's join
    // is an SSA var — so this fixture is the proof that the collision has no
    // surface to land on rather than a proof that the allocator dodged it.
    "flow-shape/valid/layer_hygiene",
    // The on-failure edge, arm EXECUTED: `fail_*` actions fail by name in the
    // BC-4 differential, so these two RUN their compensation — one routing to
    // a workflow failure outcome, one to a sibling step that completes.
    "flow-shape/valid/on_failure_compensates_to_outcome",
    "flow-shape/valid/on_failure_compensates_to_step",
    // The on-failure edge (top-level steps): a guarded body whose terminal
    // route stays OUTSIDE the guard, so a deliberate `route <failure outcome>`
    // can never trigger compensation.
    "flow-shape/valid/on_failure_route_tail",
    // A value payload as a REGION's terminal route: a literal payload on the
    // close of a `distribute`/`collect` region carrying a `max … visits`
    // bound, so the payload lowers after region synthesis rather than before.
    "flow-shape/valid/region_close_bounded",
    // Value payloads carried out of nested and sibling regions whose locals
    // are reused at unrelated types — the payload is the piped `count` of the
    // collected rows, so the expression lowering rides the region's own
    // binding, not the host's.
    "flow-shape/valid/region_nested_local_reuse",
    "flow-shape/valid/region_pure_decision",
    "flow-shape/valid/region_sibling_local_reuse",
    "flow-shape/valid/sequence_activity_tolerant",
    "flow-shape/valid/sequence_region_loopback",
    // The subflow NAME-SCOPING arm: two subflows whose parameters and
    // outcomes reuse the same author names at unrelated types, and a host
    // that reuses them again. Each flow carries its own binding environment,
    // so a lowering that leaked one scope into another would mistype here.
    "flow-shape/valid/subflow_local_names",
    // Substeps, the `on failure` arm: a substep's guarded body whose TERMINAL
    // route fires a PARENT outcome arm carrying a value payload. The route is a
    // tail, never a fallible result, so it stays outside the guard — a
    // deliberate `route <failure outcome>` through the parent's arm cannot
    // trigger the substep's compensation.
    "flow-shape/valid/substep_on_failure_route_tail",
    // The subflow NESTING arm: a subflow invoking a subflow, each compiled
    // once, the inner invocation binding the outer's local.
    "flow-shape/valid/subflow_nested",
    // A value payload built by a binary CONCAT (`status + workflow.id`) in
    // BOTH arms of a decision, inside a document whose inputs deliberately
    // collide with every synthesized temporary. The payload is an expression
    // with its own prelude statements, not a bare binding.
    "flow-shape/valid/temporary_hygiene",
    // The canonical arm: `route done(note)` at the workflow boundary.
    "flow-shape/valid/value_route_payload",
    // The `harness` section is LAUNCH configuration — how a worker process
    // starts an agent — and not flow meaning, so a document carrying one must
    // lower exactly as the same document without it. Pinning all three shapes
    // here is what proves it: a section that changed the lowering would be a
    // launch setting that changed what the workflow does.
    "harness-section/valid/acp_harness",
    // The ABSENCE control: the norn fixture with its section removed. Its
    // golden must equal the norn fixture's, which is what turns "the section
    // does not change the lowering" from a claim about three documents that
    // all carry one into a comparison against a document that does not.
    "harness-section/valid/harness_absent_control",
    "harness-section/valid/norn_harness",
    "harness-section/valid/norn_harness_without_optional_settings",
    "header-types/valid/builtins",
    "header-types/valid/combined",
    "header-types/valid/doc_comments",
    "header-types/valid/enum",
    "header-types/valid/line_width",
    // BC-4 adversarial: a high-arity record — a wide tuple through the codec.
    "header-types/valid/max_arity_record",
    "header-types/valid/minimal",
    "header-types/valid/noncanonical_commas",
    "header-types/valid/signal_wait",
    "header-types/valid/workflow_timeout",
    "header-types/valid/zero_inputs",
    // `backward_route_bounded_cycle` regressed covered → refused at rev 3
    // (the cycle rule demanded a `max … visits` bound); the fan-out parity
    // landing lowers the bound, restoring coverage.
    "loop-outcomes/valid/backward_route_bounded_cycle",
    "loop-outcomes/valid/enum_when_totality",
    "loop-outcomes/valid/float_threshold_guard",
    "loop-outcomes/valid/fork_in_loop_live_ins",
    "loop-outcomes/valid/guard_optional_wait",
    "loop-outcomes/valid/loop_after_fall_through",
    "loop-outcomes/valid/loop_compound_until_nested",
    "loop-outcomes/valid/loop_counting_until_max",
    "loop-outcomes/valid/loop_without_counting",
    // The on-failure edge: compensation to a workflow outcome.
    "loop-outcomes/valid/nested_handlers",
    "loop-outcomes/valid/on_failure_compensation",
    "loop-outcomes/valid/route_outcome_by_name",
    // Substeps, the SIBLING-CHAIN arm: two substeps where the first routes to
    // the second by name and the second falls through into the parent's own
    // outcome clauses — the substep group's boundary.
    "loop-outcomes/valid/substeps_two_stage",
    // The on-failure edge: compensation to a SIBLING STEP, inside a workflow
    // that also carries a counting loop and a `max … visits` bound.
    "loop-outcomes/valid/ship_release_combined",
    "schema-doors/valid/import_constraints",
    "schema-doors/valid/import_nested_defs",
    "schema-doors/valid/import_ticket",
    "schema-doors/valid/inline_schema_round",
    "schema-doors/valid/inline_verbatim_constraints",
    "schema-doors/valid/mixed_doors",
    "schema-doors/valid/optional_shorthand",
    "schema-doors/valid/short_circuit_optional",
    "step-bodies/valid/calls_and_side_effects",
    "step-bodies/valid/collection_predicates",
    "step-bodies/valid/combinators",
    "step-bodies/valid/fallible_all_short_circuit",
    "step-bodies/valid/fallible_any_short_circuit",
    "step-bodies/valid/fallible_collection_predicates",
    "step-bodies/valid/general_concat",
    "step-bodies/valid/index_and_concat",
    "step-bodies/valid/literal_forms",
    "step-bodies/valid/pipe_chain_stages",
    // #97-B: guard-dependent optionality narrowing keys on an access PATH, so
    // an optional FIELD narrows the same way an optional binding always has.
    // Both are pinned because the two positions are different machinery: the
    // guard's short-circuit edge, and the arm's own proof.
    "step-bodies/valid/guard_chain_carries_every_proof",
    "step-bodies/valid/guard_narrows_a_deep_optional_field",
    "step-bodies/valid/guard_narrows_an_optional_field",
    "step-bodies/valid/predicates_and_operators",
    // The read-only surface: header `query` declarations, all three answer
    // forms, an answer re-registered inside a loop, and one inside an
    // `on failure` block.
    "step-bodies/valid/query_answer",
    // The signal SENDER: header `send` declarations, all three payload
    // forms, and both addressing shapes an id-valued expression allows.
    "step-bodies/valid/send_signal",
    "step-bodies/valid/step_bodies_combined",
    // BC-4 adversarial: UTF-8 string literals and concat through the wire.
    "step-bodies/valid/unicode_payloads",
    "step-bodies/valid/wait_and_sleep",
    "step-bodies/valid/wait_timeout_optional",
    "step-bodies/valid/workflow_id",
];
