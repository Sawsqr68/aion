== 0. environment ==
  HOME absent from the caller's environment (aion clears a declared body's env to PATH); supplied from the passwd entry: /home/rocketfish
== 1. toolchain ==
  pinned: 1.97.1
  active: rustc 1.97.1 (8bab26f4f 2026-07-14)
  source: 1.97.1-x86_64-unknown-linux-gnu (overridden by '/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/rust-toolchain.toml')
  pin HONORED: active toolchain is sourced from the pin file
== 2. tree receipt ==
  HEAD: 31f616a864f58d890734f831c97a3b2b43735281
  TREE: CLEAN
== 3. paths census ==
  census OK (97 files within the wall, 59 in the deliverable crate family)
== 4a. clippy ==
   Compiling aion-server v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-server)
    Checking aion-cli v0.21.0 (/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-cli)
    Finished `dev` profile [unoptimized] target(s) in 31.07s
  clippy clean
== 4b. new-test control ==
  new test annotations in diff: 71
== 4c. suite ==
  suite names ~5472 test(s)
    Finished `test` profile [unoptimized] target(s) in 0.31s
────────────
 Nextest run ID 0237c5f3-6ab7-492e-aa97-e309b53cf101 with nextest profile: default
    Starting 5472 tests across 282 binaries
        PASS [   0.006s] (   1/5472) aion-awl ast::command_tests::a_doubled_dollar_is_a_literal_dollar_and_never_a_reference
        PASS [   0.007s] (   2/5472) aion-awl ast::command_tests::a_dollar_before_a_hyphen_is_refused
        PASS [   0.007s] (   3/5472) aion-awl ast::command_tests::a_brace_reference_holding_a_non_name_is_refused
        PASS [   0.007s] (   4/5472) aion-awl ast::command_tests::a_dollar_inside_single_quotes_is_literal
        PASS [   0.006s] (   5/5472) aion-awl ast::command_tests::a_reference_ending_the_command_is_measured_to_the_end
        PASS [   0.006s] (   6/5472) aion-awl ast::command_tests::a_name_may_carry_digits_and_underscores
        PASS [   0.007s] (   7/5472) aion-awl ast::command_tests::a_doubled_dollar_before_a_brace_stays_literal
        PASS [   0.007s] (   8/5472) aion-awl ast::command_tests::a_unicode_name_is_refused_rather_than_split
        PASS [   0.007s] (   9/5472) aion-awl ast::command_tests::a_brace_reference_holding_spaces_is_refused
        PASS [   0.006s] (  10/5472) aion-awl ast::command_tests::a_dollar_inside_double_quotes_is_a_reference
        PASS [   0.007s] (  11/5472) aion-awl ast::command_tests::a_dollar_before_a_space_is_refused
        PASS [   0.007s] (  12/5472) aion-awl ast::command_tests::a_bare_reference_reports_its_name_offset_and_length
        PASS [   0.006s] (  13/5472) aion-awl ast::command_tests::adjacent_references_are_both_found
        PASS [   0.007s] (  14/5472) aion-awl ast::command_tests::a_command_without_a_dollar_has_no_references
        PASS [   0.006s] (  15/5472) aion-awl ast::command_tests::a_dangling_dollar_at_the_end_is_refused
        PASS [   0.007s] (  16/5472) aion-awl ast::command_tests::a_reference_in_program_position_is_reported_inside_the_program_word
        PASS [   0.006s] (  17/5472) aion-awl ast::command_tests::an_empty_brace_reference_is_refused
        PASS [   0.006s] (  18/5472) aion-awl ast::command_tests::an_unterminated_double_quote_is_refused
        PASS [   0.006s] (  19/5472) aion-awl ast::command_tests::parameters_are_sorted_and_deduplicated
        PASS [   0.006s] (  20/5472) aion-awl ast::command_tests::the_program_word_ends_at_the_first_unquoted_whitespace
        PASS [   0.006s] (  21/5472) aion-awl ast::command_tests::the_empty_command_scans_to_nothing
        PASS [   0.006s] (  22/5472) aion-awl ast::command_tests::an_unterminated_brace_reference_is_refused
        PASS [   0.006s] (  23/5472) aion-awl ast::command_tests::multibyte_text_before_a_reference_keeps_byte_offsets_honest
        PASS [   0.006s] (  24/5472) aion-awl ast::command_tests::an_uppercase_shell_variable_is_refused_with_the_escape_hint
        PASS [   0.006s] (  25/5472) aion-awl ast::command_tests::braces_delimit_a_reference_that_abuts_more_name_characters
        PASS [   0.006s] (  26/5472) aion-awl ast::command_tests::an_unterminated_single_quote_is_refused
        PASS [   0.006s] (  27/5472) aion-awl checker::harness::tests::a_duplicate_setting_is_refused_at_the_parse_because_neither_reading_is_honest
        PASS [   0.007s] (  28/5472) aion-awl checker::harness::tests::an_env_pass_entry_carrying_a_value_is_refused_rather_than_split
        PASS [   0.007s] (  29/5472) aion-awl checker::harness::tests::a_norn_section_is_never_asked_for_an_acp_setting
        PASS [   0.007s] (  30/5472) aion-awl checker::harness::tests::a_setting_for_the_other_kind_is_refused_by_name_in_both_directions
        PASS [   0.006s] (  31/5472) aion-awl command::cause::cause_tests::layer_wire_strings_are_distinct
        PASS [   0.006s] (  32/5472) aion-awl command::cause::cause_tests::a_cause_round_trips_through_its_wire_string
        PASS [   0.006s] (  33/5472) aion-awl command::cause::cause_tests::every_layer_files_at_least_one_cause_and_every_cause_has_a_layer
        PASS [   0.006s] (  34/5472) aion-awl command::cause::cause_tests::the_vocabulary_carries_no_catch_all_code
        PASS [   0.005s] (  35/5472) aion-awl command::derived::derived_tests::the_three_derived_shapes_are_caught
        PASS [   0.006s] (  36/5472) aion-awl command::cause::cause_tests::the_inventory_carries_every_variant_exactly_once
        PASS [   0.006s] (  37/5472) aion-awl checker::harness::tests::an_unknown_setting_is_refused_naming_what_the_section_carries
        PASS [   0.006s] (  38/5472) aion-awl command::derived::derived_tests::ordinary_text_is_left_alone
        PASS [   0.007s] (  39/5472) aion-awl command::cause::cause_tests::every_wire_string_is_distinct_and_shaped_like_a_cause_code
        PASS [   0.007s] (  40/5472) aion-awl checker::harness::tests::an_unknown_kind_suppresses_the_per_kind_demands_rather_than_guessing_one
        PASS [   0.006s] (  41/5472) aion-awl command::error::error_tests::a_constructor_fed_a_cause_from_another_layer_returns_a_typed_error
        PASS [   0.009s] (  42/5472) aion-awl command::derived::derived_tests::every_shape_names_its_derivation
        PASS [   0.011s] (  43/5472) aion-awl checker::harness::tests::every_required_norn_field_is_named_when_it_is_the_one_removed
        PASS [   0.005s] (  44/5472) aion-awl command::error::error_tests::the_internal_door_files_every_cause_at_its_own_layer
        PASS [   0.005s] (  45/5472) aion-awl command::fill::fill_tests::a_backtick_is_refused_at_the_parse_layer_with_its_own_cause
        PASS [   0.006s] (  46/5472) aion-awl command::error::error_tests::a_rendered_refusal_carries_its_observed_bytes_and_its_cause
        PASS [   0.005s] (  47/5472) aion-awl command::fill::fill_tests::a_single_brace_is_literal_text
        PASS [   0.006s] (  48/5472) aion-awl command::error::error_tests::the_internal_door_never_produces_a_silent_refusal
        PASS [   0.006s] (  49/5472) aion-awl command::fill::fill_tests::a_hole_span_narrows_into_the_literal_when_it_can
        PASS [   0.006s] (  50/5472) aion-awl command::error::error_tests::every_cause_constructs_at_its_own_layer_and_nowhere_else
        PASS [   0.005s] (  51/5472) aion-awl command::fill::fill_tests::a_mixed_fill_keeps_its_segments_in_source_order
        PASS [   0.005s] (  52/5472) aion-awl command::fill::fill_tests::a_plain_literal_is_one_segment
        PASS [   0.006s] (  53/5472) aion-awl command::error::error_tests::a_constructor_fed_an_empty_diagnostic_returns_a_typed_error
        PASS [   0.012s] (  54/5472) aion-awl checker::harness::tests::every_required_acp_field_is_named_when_it_is_the_one_removed
        PASS [   0.005s] (  55/5472) aion-awl command::fill::fill_tests::a_sole_hole_is_recognised_as_such
        PASS [   0.006s] (  56/5472) aion-awl command::fill::fill_tests::an_empty_interpolation_is_refused
        PASS [   0.005s] (  57/5472) aion-awl guide::grammar::grammar_tests::positions_are_read_from_both_halves_of_the_table_in_order
        PASS [   0.005s] (  58/5472) aion-awl guide::grammar::grammar_tests::the_canonical_words_land_in_their_forms
        PASS [   0.005s] (  59/5472) aion-awl guide::lookup::guide_tests::a_citation_names_the_reference_and_the_section
        PASS [   0.006s] (  60/5472) aion-awl command::fill::fill_tests::an_interpolation_that_is_not_a_bare_parameter_name_is_refused
        PASS [   0.005s] (  61/5472) aion-awl guide::grammar::grammar_tests::the_table_offers_no_word_the_lexer_does_not_name
        PASS [   0.005s] (  62/5472) aion-awl guide::grammar::grammar_tests::the_all_inventories_carry_every_variant
        PASS [   0.006s] (  63/5472) aion-awl command::fill::fill_tests::an_unterminated_interpolation_is_refused
        PASS [   0.006s] (  64/5472) aion-awl guide::grammar::grammar_tests::offered_words_compose_keywords_then_positional_words
        PASS [   0.006s] (  65/5472) aion-awl command::fill::fill_tests::the_same_fill_parses_identically_every_time
        PASS [   0.006s] (  66/5472) aion-awl guide::grammar::grammar_tests::wire_identifiers_are_distinct_and_prose_is_one_finished_line
        PASS [   0.007s] (  67/5472) aion-awl guide::grammar::grammar_tests::every_derived_form_is_honest_against_the_table
        PASS [   0.008s] (  68/5472) aion-awl guide::grammar::grammar_tests::every_word_of_the_vocabulary_stands_somewhere
        PASS [   0.009s] (  69/5472) aion-awl guide::grammar::grammar_tests::every_form_files_at_least_one_word
        PASS [   0.005s] (  70/5472) aion-awl guide::lookup::guide_tests::every_keyword_entry_names_the_word_the_lexer_produces
        PASS [   0.005s] (  71/5472) aion-awl guide::lookup::guide_tests::the_example_word_matcher_reads_tokens_not_text
        PASS [   0.005s] (  72/5472) aion-awl guide::lookup::guide_tests::the_two_inventories_do_not_overlap
        PASS [   0.005s] (  73/5472) aion-awl guide::lookup::guide_tests::every_word_of_the_language_is_reachable_by_name
        PASS [   0.006s] (  74/5472) aion-awl guide::lookup::guide_tests::every_positional_word_has_an_entry_recorded_as_positional
        PASS [   0.006s] (  75/5472) aion-awl guide::lookup::guide_tests::every_summary_is_one_rendered_line
        PASS [   0.006s] (  76/5472) aion-awl guide::lookup::guide_tests::the_inventory_is_complete_and_carries_no_duplicate
        PASS [   0.006s] (  77/5472) aion-awl harness_config::tests::a_misplaced_workspace_root_in_cwd_is_refused_at_the_seam
        PASS [   0.034s] (  78/5472) aion-awl checker::harness::tests::a_norn_section_is_complete_without_command_permission_or_exit_grace
        PASS [   0.007s] (  79/5472) aion-awl harness_config::tests::a_relative_norn_binary_is_named_at_the_launch_seam_not_carried_through
        PASS [   0.006s] (  80/5472) aion-awl lexer::grammar_vocabulary_tests::the_bindable_positional_words_are_deliberately_left_plain
        PASS [   0.008s] (  81/5472) aion-awl harness_config::tests::a_missing_norn_cwd_is_named_at_the_launch_seam
        PASS [   0.006s] (  82/5472) aion-awl lexer::grammar_vocabulary_tests::every_reserved_word_is_known_to_the_editor_grammar
        PASS [   0.005s] (  83/5472) aion-awl lexer::grammar_vocabulary_tests::the_grammar_colours_nothing_the_lexer_cannot_account_for
        PASS [   0.005s] (  84/5472) aion-awl lexer::grammar_vocabulary_tests::the_coloured_exception_is_a_genuine_positional_word
        PASS [   0.006s] (  85/5472) aion-awl harness_config::tests::a_workspace_rooted_cwd_reaches_the_launch_seam_unexpanded
        PASS [   0.035s] (  86/5472) aion-awl checker::harness::tests::an_acp_section_is_complete_without_any_norn_setting
        PASS [   0.006s] (  87/5472) aion-awl harness_config::tests::an_absolute_norn_binary_passes_the_same_net
        PASS [   0.006s] (  88/5472) aion-awl harness_config::tests::an_omitted_norn_binary_is_absence_and_not_a_refusal
        PASS [   0.009s] (  89/5472) aion-awl harness_config::tests::a_relative_acp_command_is_named_by_its_own_field
        PASS [   0.006s] (  90/5472) aion-awl lexer::grammar_vocabulary_tests::the_specifically_typed_words_are_still_covered
        PASS [   0.014s] (  91/5472) aion-awl guide::lookup::guide_tests::the_embedded_reference_carries_the_cited_sections
        PASS [   0.006s] (  92/5472) aion-awl lexer::tokens_tests::every_keyword_in_the_inventory_round_trips_and_is_distinct
        PASS [   0.006s] (  93/5472) aion-awl mir::agent_control_tests::agent_marked_and_unmarked_lower_identically
        PASS [   0.006s] (  94/5472) aion-awl mir::agent_control_tests::the_control_pair_differs_only_by_the_marker
        PASS [   0.008s] (  95/5472) aion-awl lexer::tokens_tests::the_positional_words_are_named_without_being_reserved
        PASS [   0.009s] (  96/5472) aion-awl lexer::tokens_tests::keyword_inventory_covers_every_variant
        PASS [   0.021s] (  97/5472) aion-awl guide::lookup::guide_tests::every_entry_example_is_a_complete_document_using_its_word
        PASS [   0.024s] (  98/5472) aion-awl mir::deferred_tests::a_substeps_failure_edge_keeps_its_routed_outcome_out_of_the_guard
        PASS [   0.030s] (  99/5472) aion-awl mir::codec_tests::required_recursive_default_is_refused_on_both_sides
        PASS [   0.028s] ( 100/5472) aion-awl mir::deferred_tests::a_subflow_outcome_payload_is_wrapped_for_an_optional_outcome
        PASS [   0.030s] ( 101/5472) aion-awl mir::codec_tests::colliding_codec_stems_are_refused_not_cross_wired
        PASS [   0.031s] ( 102/5472) aion-awl mir::child_fork_tests::sequential_child_fork_matches_reference_fold_order
        PASS [   0.028s] ( 103/5472) aion-awl mir::deferred_tests::a_substep_visit_bound_refuses_rather_than_going_unenforced
        PASS [   0.033s] ( 104/5472) aion-awl mir::child_fork_tests::child_spawn_keeps_identity_and_outcome_envelope_codec
        PASS [   0.027s] ( 105/5472) aion-awl mir::deferred_tests::a_value_payload_is_wrapped_for_an_optional_outcome
        PASS [   0.030s] ( 106/5472) aion-awl mir::deferred_tests::a_substep_chain_hands_off_and_closes_on_the_parents_arms
        PASS [   0.026s] ( 107/5472) aion-awl mir::deferred_tests::bc4_zero_step_workflow_refuses_no_start_region
        PASS [   0.034s] ( 108/5472) aion-awl mir::child_fork_tests::parallel_child_fork_is_spawn_all_then_ordered_await_with_emitter_parity
        PASS [   0.029s] ( 109/5472) aion-awl mir::deferred_tests::each_subflow_keeps_its_own_binding_environment
        PASS [   0.041s] ( 110/5472) aion-awl mir::deferred_tests::a_parallel_layer_dispatches_through_one_workflow_all
        PASS [   0.006s] ( 111/5472) aion-awl mir::fork_tests::collection_child_fork_config_refuses_with_the_child_class
        PASS [   0.045s] ( 112/5472) aion-awl mir::deferred_tests::a_degraded_parallel_layer_is_marked_on_its_first_member_only
        PASS [   0.040s] ( 113/5472) aion-awl mir::deferred_tests::an_advisory_call_keeps_its_layer_in_written_order
        PASS [   0.028s] ( 114/5472) aion-awl mir::deferred_tests::ordinary_call_index_lowers_with_the_anchored_runtime_message
        PASS [   0.029s] ( 115/5472) aion-awl mir::deferred_tests::guard_optional_wait_now_lowers
        PASS [   0.029s] ( 116/5472) aion-awl mir::deferred_tests::post_join_combinator_now_lowers
        PASS [   0.029s] ( 117/5472) aion-awl mir::deferred_tests::sort_non_comparable_key_refuses_with_the_reference_class
        PASS [   0.030s] ( 118/5472) aion-awl mir::deferred_tests::value_route_payloads_now_lower
        PASS [   0.031s] ( 119/5472) aion-awl mir::deferred_tests::substep_fixtures_now_lower
        PASS [   0.030s] ( 120/5472) aion-awl mir::fork_tests::action_and_child_collection_forks_route_distinctly
        PASS [   0.033s] ( 121/5472) aion-awl mir::deferred_tests::statement_level_child_forms_plan_the_witness
        PASS [   0.036s] ( 122/5472) aion-awl mir::deferred_tests::nested_handlers_on_failure_now_lowers
        PASS [   0.006s] ( 123/5472) aion-awl mir::harness_control_tests::a_harness_section_does_not_change_the_lowering
        PASS [   0.006s] ( 124/5472) aion-awl mir::harness_control_tests::the_control_pair_differs_only_by_the_section
        PASS [   0.039s] ( 125/5472) aion-awl mir::deferred_tests::subflow_declarations_now_lower
        PASS [   0.005s] ( 126/5472) aion-awl mir::runtime::tests::enumeration_covers_every_variant_exactly_once
        PASS [   0.029s] ( 127/5472) aion-awl mir::fork_tests::homogeneous_named_fork_pins_one_typed_all
        PASS [   0.005s] ( 128/5472) aion-awl mir::runtime::tests::import_surface_projects_the_full_enumeration
        PASS [   0.035s] ( 129/5472) aion-awl mir::fork_tests::fork_branch_call_site_config_lowers_with_per_key_merge
        PASS [   0.034s] ( 130/5472) aion-awl mir::fork_tests::heterogeneous_named_fork_pins_raw_twins_and_positional_decode
        PASS [   0.005s] ( 131/5472) aion-awl mir::runtime::tests::query_handler_is_not_a_durable_effect
        PASS [   0.006s] ( 132/5472) aion-awl mir::select::control_tests::assert_list_badmatch_traps_the_subject_list
        PASS [   0.037s] ( 133/5472) aion-awl mir::fork_tests::named_child_branch_refuses_cleanly
        PASS [   0.059s] ( 134/5472) aion-awl mir::deferred_tests::parallel_layer_fixtures_now_lower
        PASS [   0.032s] ( 135/5472) aion-awl mir::fork_tests::parallel_action_fork_pins_map_fanout_semantics
        PASS [   0.031s] ( 136/5472) aion-awl mir::fork_tests::sequential_fork_pins_fold_initial_state_and_result_order
        PASS [   0.006s] ( 137/5472) aion-awl mir::select::inspect_tests::framed_return_reloads_the_return_value_into_x0
        PASS [   0.026s] ( 138/5472) aion-awl mir::pipe_tests::post_stage_any_all_lower_through_predicate_closures
        PASS [   0.030s] ( 139/5472) aion-awl mir::pipe_tests::pipe_child_stage_lowers_to_spawn_and_wait
        PASS [   0.029s] ( 140/5472) aion-awl mir::pipe_tests::sort_string_key_selects_string_compare
        PASS [   0.035s] ( 141/5472) aion-awl mir::fork_tests::stopgap_refusals_match_the_reference_classes
        PASS [   0.005s] ( 142/5472) aion-awl mir::select::inspect_tests::return_keeps_x0_live_across_a_preceding_heap_op
        PASS [   0.032s] ( 143/5472) aion-awl mir::pipe_tests::combinator_stages_lower_to_list_runtime_calls
        PASS [   0.007s] ( 144/5472) aion-awl mir::select::inspect_tests::marshaling_rejects_swapped_args_dropped_store_and_wrong_arity
        PASS [   0.006s] ( 145/5472) aion-awl mir::select::inspect_tests::targeted_framed_function_brackets_and_single_exits
        PASS [   0.008s] ( 146/5472) aion-awl mir::select::inspect_tests::targeted_frameless_body_uses_no_frame_or_y
        PASS [   0.009s] ( 147/5472) aion-awl mir::select::inspect_tests::targeted_heap_ops_declare_accurate_live
        PASS [   0.030s] ( 148/5472) aion-awl mir::select::control_tests::short_circuit_optional_selects_checked_assert_some
        PASS [   0.033s] ( 149/5472) aion-awl mir::select::control_tests::enum_total_tail_selects_with_explicit_case_end_trap
        PASS [   0.034s] ( 150/5472) aion-awl mir::select::control_tests::loop_counter_selects_a_gc_bif2_against_erlang_plus
        PASS [   0.034s] ( 151/5472) aion-awl mir::select::inspect_tests::awl_hello_outcome_route_returns_not_tail_calls
        PASS [   0.006s] ( 152/5472) aion-awl mir::select::tests::shape_call_import
        PASS [   0.037s] ( 153/5472) aion-awl mir::select::control_tests::outcome_guard_decision_tree_uses_real_failure_labels
        PASS [   0.007s] ( 154/5472) aion-awl mir::select::tests::shape_call_local
        PASS [   0.007s] ( 155/5472) aion-awl mir::select::tests::shape_field_get
        PASS [   0.006s] ( 156/5472) aion-awl mir::select::tests::shape_frameless_tail_import
        PASS [   0.006s] ( 157/5472) aion-awl mir::select::tests::shape_json_obj_two_pairs
        PASS [   0.006s] ( 158/5472) aion-awl mir::select::tests::shape_make_closure
        PASS [   0.006s] ( 159/5472) aion-awl mir::select::tests::shape_record_tuple
        PASS [   0.006s] ( 160/5472) aion-awl mir::select::tests::shape_record_zero_field
        PASS [   0.006s] ( 161/5472) aion-awl mir::select::tests::shape_record_with_literal_int
        PASS [   0.007s] ( 162/5472) aion-awl mir::select::tests::shape_try_bind
        PASS [   0.029s] ( 163/5472) aion-awl mir::select::tests::minimal_module_has_the_expected_abi_structure
        PASS [   0.030s] ( 164/5472) aion-awl mir::select::tests::run_shell_passes_raw_payload_not_the_closure
        PASS [   0.035s] ( 165/5472) aion-awl mir::select::tests::dead_body_is_a_real_sidecar_visible_function
        PASS [   0.040s] ( 166/5472) aion-awl mir::select::inspect_tests::targeted_loop_recursion_is_a_self_tail_call
        PASS [   0.041s] ( 167/5472) aion-awl mir::select::inspect_tests::targeted_route_to_step_is_a_local_tail_call
        PASS [   0.032s] ( 168/5472) aion-awl mir::select::tests::selection_is_deterministic
        PASS [   0.005s] ( 169/5472) aion-awl positions::tests::a_multibyte_span_covers_its_full_extent_in_utf16_units
        PASS [   0.005s] ( 170/5472) aion-awl positions::tests::an_inverted_span_collapses_to_its_start
        PASS [   0.005s] ( 171/5472) aion-awl positions::tests::offsets_past_the_document_clamp_to_its_end
        PASS [   0.005s] ( 172/5472) aion-awl positions::tests::point_spans_stay_points
        PASS [   0.030s] ( 173/5472) aion-awl mir::tests::backward_route_reentry_now_lowers
        PASS [   0.032s] ( 174/5472) aion-awl mir::tests::a_worker_document_is_refused_by_the_lowering
        PASS [   0.005s] ( 175/5472) aion-awl positions::tests::positions_are_utf16_not_bytes_or_scalar_columns
        PASS [   0.005s] ( 176/5472) aion-awl printer::harness::tests::the_printer_emits_every_field_in_exactly_the_canonical_order
        PASS [   0.031s] ( 177/5472) aion-awl mir::tests::counterless_loop_pins_the_scalar_result_path
        PASS [   0.166s] ( 178/5472) aion-awl guide::lookup::guide_tests::every_entry_example_passes_the_real_checker
        PASS [   0.005s] ( 179/5472) aion-awl workspace_root::tests::a_relative_path_is_refused_as_it_always_was
        PASS [   0.005s] ( 180/5472) aion-awl workspace_root::tests::a_leading_placeholder_is_the_second_accepted_form
        PASS [   0.030s] ( 181/5472) aion-awl mir::tests::loop_lowering_pins_the_reference_semantics
        PASS [   0.005s] ( 182/5472) aion-awl workspace_root::tests::an_absolute_path_is_the_first_accepted_form
        PASS [   0.005s] ( 183/5472) aion-awl workspace_root::tests::an_empty_value_is_refused_before_either_form_is_considered
        PASS [   0.005s] ( 184/5472) aion-awl workspace_root::tests::the_new_form_did_not_widen_the_old_refusal
        PASS [   0.008s] ( 185/5472) aion-awl printer::harness::tests::the_scrambled_fixture_really_is_scrambled
        PASS [   0.029s] ( 186/5472) aion-awl mir::tests::optional_rhs_is_nested_below_short_circuit_branch
        PASS [   0.031s] ( 187/5472) aion-awl mir::tests::minimal_exercises_core_op_shapes
        PASS [   0.007s] ( 188/5472) aion-awl::action_bodies a_body_round_trips_and_prints_idempotently
        PASS [   0.007s] ( 189/5472) aion-awl::action_bodies a_body_written_before_the_config_line_normalizes_to_canonical_order
        PASS [   0.041s] ( 190/5472) aion-awl mir::tests::compound_until_clones_control_only_into_unresolved_leaves
        PASS [   0.008s] ( 191/5472) aion-awl::action_bodies a_declared_body_parses_onto_its_action_and_a_bodyless_action_keeps_none
        PASS [   0.006s] ( 192/5472) aion-awl::action_bodies comments_around_a_body_survive_the_round_trip
        PASS [   0.006s] ( 193/5472) aion-awl::action_body_executor_parity a_parameter_in_program_position_is_refused_by_both
        PASS [   0.006s] ( 194/5472) aion-awl::action_bodies two_bodies_on_one_action_are_a_parse_error
        PASS [   0.005s] ( 195/5472) aion-awl::action_body_executor_parity every_command_the_checker_accepts_the_executor_also_accepts
        PASS [   0.006s] ( 196/5472) aion-awl::action_bodies two_config_lines_on_one_action_are_a_parse_error
        PASS [   0.006s] ( 197/5472) aion-awl::action_body_executor_parity the_accepted_table_is_actually_accepted
        PASS [   0.036s] ( 198/5472) aion-awl mir::tests::typed_codec_bodies_pin_the_wire_shapes
        PASS [   0.006s] ( 199/5472) aion-awl::action_body_executor_parity the_refused_table_is_actually_refused
        PASS [   0.008s] ( 200/5472) aion-awl::assistant_example nudge_cap_exhaustion_always_ends_the_session
        PASS [   0.007s] ( 201/5472) aion-awl::assistant_example the_round_loop_dispatches_exactly_once_per_iteration
        PASS [   0.031s] ( 202/5472) aion-awl::action_bodies a_bodyless_document_still_checks_clean
        PASS [   0.007s] ( 203/5472) aion-awl::assistant_example the_wait_loop_dispatches_nothing
        PASS [   0.031s] ( 204/5472) aion-awl::action_bodies run_is_still_usable_as_an_ordinary_name
        PASS [   0.034s] ( 205/5472) aion-awl::action_bodies escapes_inside_a_command_survive_the_round_trip
        PASS [   0.035s] ( 206/5472) aion-awl::action_bodies a_document_with_no_bodies_compiles_to_no_implementations
        PASS [   0.006s] ( 207/5472) aion-awl::awl2_defects action_config_duration_span_matches_flagship_true_position
        PASS [   0.034s] ( 208/5472) aion-awl::action_bodies the_compiled_workflow_carries_the_command_and_its_binding
        PASS [   0.005s] ( 209/5472) aion-awl::awl2_defects dead_header_keywords_get_targeted_fix_its
        PASS [   0.005s] ( 210/5472) aion-awl::awl2_defects comment_torture_document_round_trips_byte_identically
        PASS [   0.004s] ( 211/5472) aion-awl::awl2_defects error_column_after_multibyte_prose_is_character_correct
        PASS [   0.006s] ( 212/5472) aion-awl::awl2_defects dead_statement_keywords_get_targeted_fix_its
        PASS [   0.004s] ( 213/5472) aion-awl::awl2_defects guard_error_deep_in_document_reports_true_line_and_offset
        PASS [   0.008s] ( 214/5472) aion-awl::awl2_defects fork_collection_span_matches_flagship_true_position
        PASS [   0.005s] ( 215/5472) aion-awl::awl2_defects guard_error_at_exactly_line_forty_is_source_correct
        PASS [   0.005s] ( 216/5472) aion-awl::awl2_defects inline_schema_error_column_after_multibyte_content_is_character_correct
        PASS [   0.005s] ( 217/5472) aion-awl::awl2_defects own_line_comments_survive_at_every_nesting_level
        PASS [   0.036s] ( 218/5472) aion-awl::assistant_example the_assistant_example_checks_clean_and_is_canonically_formatted
        PASS [   0.006s] ( 219/5472) aion-awl::b2_review_regressions a_failure_route_does_not_inherit_success_bindings
        PASS [   0.006s] ( 220/5472) aion-awl::b2_review_regressions a_failure_route_into_a_collect_breaks_definite_assignment
        PASS [   0.006s] ( 221/5472) aion-awl::b2_review_regressions a_self_routing_substep_without_a_bound_is_rejected
        PASS [   0.039s] ( 222/5472) aion-awl::action_body_executor_parity the_compiled_binding_matches_what_the_executor_would_parse
        PASS [   0.007s] ( 223/5472) aion-awl::b2_review_regressions a_sub_cycle_avoiding_the_bounded_vertex_is_rejected
        PASS [   0.006s] ( 224/5472) aion-awl::b2_review_regressions a_subflow_parameter_cannot_shadow_a_const
        PASS [   0.036s] ( 225/5472) aion-awl::assistant_example the_emitted_round_loop_exits_on_the_stalled_field
        PASS [   0.007s] ( 226/5472) aion-awl::b2_review_regressions a_substep_visits_bound_is_type_checked
        PASS [   0.007s] ( 227/5472) aion-awl::b2_review_regressions incompatible_per_path_collected_types_are_rejected
        PASS [   0.008s] ( 228/5472) aion-awl::b2_review_regressions an_unbounded_sibling_substep_cycle_is_rejected
        PASS [   0.007s] ( 229/5472) aion-awl::b2_review_regressions the_failure_binding_document_is_refused_before_emission
        PASS [   0.007s] ( 230/5472) aion-awl::b2_review_round2 a_pure_sibling_after_cycle_is_rejected
        PASS [   0.007s] ( 231/5472) aion-awl::b2_review_round2 a_sibling_substep_after_cycle_is_rejected
        PASS [   0.006s] ( 232/5472) aion-awl::b2_review_round2 an_unknown_sibling_after_target_is_rejected
        PASS [   0.007s] ( 233/5472) aion-awl::b2_review_round2 a_structurally_equal_join_rejects_incompatible_consumption
        PASS [   0.056s] ( 234/5472) aion-awl::assistant_example the_assistant_example_compiles
        PASS [   0.008s] ( 235/5472) aion-awl::b2_review_round2 incompatible_types_meeting_at_a_join_are_rejected_there
        PASS [   0.034s] ( 236/5472) aion-awl::b2_review_regressions a_bounded_sibling_substep_cycle_is_accepted
        PASS [   0.029s] ( 237/5472) aion-awl::b2_review_regressions bounds_covering_every_overlapping_cycle_check_clean
        PASS [   0.009s] ( 238/5472) aion-awl::builtin_reservation every_reserved_name_is_refused_by_both_surfaces_for_the_same_reason
        PASS [   0.031s] ( 239/5472) aion-awl::b2_review_regressions compensation_bindings_before_the_failure_route_do_flow
        PASS [   0.030s] ( 240/5472) aion-awl::b2_review_regressions same_named_subflow_outcomes_resolve_to_their_own_declarations
        PASS [   0.072s] ( 241/5472) aion-awl::assistant_example the_assistant_example_answers_the_status_query
        PASS [   0.032s] ( 242/5472) aion-awl::b2_review_round2 a_structurally_equal_join_keeps_a_concrete_type
        PASS [   0.031s] ( 243/5472) aion-awl::b2_review_round2 disjoint_branches_may_reuse_a_name_at_different_types
        PASS [   0.030s] ( 244/5472) aion-awl::b2_review_round2 substep_after_is_refused_at_emission
        PASS [   0.006s] ( 245/5472) aion-awl::checker check_without_a_root_reports_unresolvable_imports
        PASS [   0.035s] ( 246/5472) aion-awl::builtin_reservation checker_and_emitter_acceptance_round_trip_for_reserved_names_and_control
        PASS [   0.006s] ( 247/5472) aion-awl::checker diagnostics_arrive_sorted_by_source_position
        PASS [   0.006s] ( 248/5472) aion-awl::checker duplicate_step_names_are_refused
        PASS [   0.034s] ( 249/5472) aion-awl::builtin_reservation ordinary_name_passes_both_surfaces
        PASS [   0.028s] ( 250/5472) aion-awl::canonical_emission derived_schemas_serialize_key_sorted
        PASS [   0.008s] ( 251/5472) aion-awl::checker imported_optional_field_is_optional_in_the_projection_direction
        PASS [   0.031s] ( 252/5472) aion-awl::canonical_emission folded_schema_text_is_key_sorted_in_the_emitted_source
        PASS [   0.006s] ( 253/5472) aion-awl::checker unknown_binding_error_is_span_exact
        PASS [   0.037s] ( 254/5472) aion-awl::builtin_reservation schema_of_keeps_referenceable_and_reserved_roles_distinct
        PASS [   0.037s] ( 255/5472) aion-awl::canonical_emission authored_key_order_cannot_change_the_emitted_bytes
        PASS [   0.034s] ( 256/5472) aion-awl::canonical_emission projected_record_fields_are_emitted_in_key_order
        PASS [   0.008s] ( 257/5472) aion-awl::checker_hardening deep_record_mismatch_is_refused_past_the_old_depth_cap
        PASS [   0.009s] ( 258/5472) aion-awl::checker_hardening call_site_config_on_a_child_call_is_refused
        PASS [   0.031s] ( 259/5472) aion-awl::canonical_emission the_sidecar_matches_a_key_sorted_expectation
        PASS [   0.006s] ( 260/5472) aion-awl::checker_hardening inline_schema_error_anchors_at_the_offending_nested_token
        PASS [   0.006s] ( 261/5472) aion-awl::checker_hardening inline_schema_refusal_skips_an_earlier_property_named_like_the_keyword
        PASS [   0.007s] ( 262/5472) aion-awl::checker_hardening outcome_clauses_behind_a_body_terminal_route_are_refused
        PASS [   0.009s] ( 263/5472) aion-awl::checker_hardening statements_after_an_unconditional_route_are_refused
        PASS [   0.007s] ( 264/5472) aion-awl::checker_regressions dangling_middle_step_is_refused
        PASS [   0.006s] ( 265/5472) aion-awl::checker_regressions loop_max_may_not_read_loop_locals
        PASS [   0.008s] ( 266/5472) aion-awl::checker_regressions backward_route_re_armed_by_an_after_edge_needs_a_bound
        PASS [   0.066s] ( 267/5472) aion-awl::builtin_reservation emitter_preclaims_every_name_the_checker_reserves
        PASS [   0.010s] ( 268/5472) aion-awl::checker_regressions loop_max_may_not_read_the_counter
        PASS [   0.007s] ( 269/5472) aion-awl::checker_regressions named_fork_branch_cannot_read_a_sibling_bindings
        PASS [   0.007s] ( 270/5472) aion-awl::checker_regressions loop_rebind_inside_a_collection_fork_branch_does_not_count
        PASS [   0.028s] ( 271/5472) aion-awl::checker route_targeted_earlier_step_reads_a_later_steps_binding
        PASS [   0.010s] ( 272/5472) aion-awl::checker_regressions named_fork_join_binding_is_refused
        PASS [   0.006s] ( 273/5472) aion-awl::checker_regressions step_sharing_a_workflow_outcome_name_is_refused
        PASS [   0.007s] ( 274/5472) aion-awl::checker_regressions piped_route_to_a_step_is_refused
        PASS [   0.006s] ( 275/5472) aion-awl::checker_rulings list_element_optional_is_refused_in_input_position
        PASS [   0.006s] ( 276/5472) aion-awl::checker_rulings list_element_optional_is_refused_in_outcome_and_child_positions
        PASS [   0.006s] ( 277/5472) aion-awl::checker_rulings loop_inside_a_fork_branch_still_requires_outcome_clauses
        PASS [   0.007s] ( 278/5472) aion-awl::checker_rulings loop_inside_a_substep_answers_to_the_substep
        PASS [   0.006s] ( 279/5472) aion-awl::checker_rulings nested_list_element_optional_is_refused
        PASS [   0.007s] ( 280/5472) aion-awl::checker_rulings route_nested_in_a_fork_inside_a_loop_is_refused_by_check
        PASS [   0.029s] ( 281/5472) aion-awl::checker_hardening structurally_equal_recursive_types_are_compatible_and_terminate
        PASS [   0.030s] ( 282/5472) aion-awl::checker_regressions falling_step_consumed_by_an_after_dependent_is_legal
        PASS [   0.030s] ( 283/5472) aion-awl::checker_regressions loop_max_over_inputs_stays_clean
        PASS [   0.026s] ( 284/5472) aion-awl::command_emit a_declaration_with_no_timeout_emits_no_timeout
        PASS [   0.029s] ( 285/5472) aion-awl::checker_rulings optional_list_stays_legal
        PASS [   0.028s] ( 286/5472) aion-awl::checker_rulings routes_outside_loop_bodies_stay_legal
        PASS [   0.028s] ( 287/5472) aion-awl::command_emit a_list_operand_emits_one_element_per_item
        PASS [   0.030s] ( 288/5472) aion-awl::command_emit a_declared_default_fills_an_omitted_parameter
        PASS [   0.030s] ( 289/5472) aion-awl::command_emit a_declared_timeout_emits_its_own_number_and_owner
        PASS [   0.027s] ( 290/5472) aion-awl::command_emit a_refusing_invocation_never_carries_the_marker
        PASS [   0.078s] ( 291/5472) aion-awl::checker every_check_staged_invalid_fixture_is_rejected_as_expected
        PASS [   0.027s] ( 292/5472) aion-awl::command_emit a_refusing_invocation_refuses_a_leading_dash_operand
        PASS [   0.008s] ( 293/5472) aion-awl::command_gate_census every_fixture_cites_a_specimen_that_is_still_at_its_pinned_digest
        PASS [   0.029s] ( 294/5472) aion-awl::command_emit a_value_containing_a_space_stays_one_argv_element
        PASS [   0.005s] ( 295/5472) aion-awl::command_gate_census the_normalizer_rejects_a_command_line_that_is_not_the_census
        PASS [   0.006s] ( 296/5472) aion-awl::command_just_subset every_absorbed_production_and_every_refused_form_has_a_fixture
        PASS [   0.026s] ( 297/5472) aion-awl::command_emit an_admitting_invocation_carries_the_marker_before_its_operands
        PASS [   0.006s] ( 298/5472) aion-awl::command_just_subset every_fixture_records_its_provenance
        PASS [   0.024s] ( 299/5472) aion-awl::command_emit the_marker_is_placed_whatever_the_operands_turn_out_to_be
        PASS [   0.030s] ( 300/5472) aion-awl::command_emit an_admitting_invocation_with_no_operands_emits_no_marker
        PASS [   0.006s] ( 301/5472) aion-awl::command_just_subset the_refused_table_and_the_refusal_fixtures_name_the_same_causes
        PASS [   0.008s] ( 302/5472) aion-awl::command_just_subset the_fixture_corpus_is_discovered_and_not_empty
        PASS [   0.032s] ( 303/5472) aion-awl::command_emit env_cwd_and_hardening_resolve_structurally
        PASS [   0.029s] ( 304/5472) aion-awl::command_emit the_parameter_set_refusals_each_fire_with_their_own_cause
        PASS [   0.006s] ( 305/5472) aion-awl::command_refusals every_cause_has_a_refusing_specimen
        PASS [   0.005s] ( 306/5472) aion-awl::command_specimen a_specimen_swallowed_by_its_fill_fails_at_the_specimen_layer
        PASS [   0.029s] ( 307/5472) aion-awl::command_gate_census one_shape_and_four_parameter_sets_emit_the_four_rigs_census_command_lines
        PASS [   0.006s] ( 308/5472) aion-awl::command_specimen a_specimen_that_is_accepted_fails_at_the_specimen_layer
        PASS [   0.008s] ( 309/5472) aion-awl::command_refusals every_specimen_refuses_at_the_layer_its_cause_names
        PASS [   0.011s] ( 310/5472) aion-awl::command_refusals every_document_specimen_refuses_with_its_own_cause_and_bytes
        PASS [   0.005s] ( 311/5472) aion-awl::command_specimen the_canonical_refusing_fixtures_specimen_fires_and_refuses
        PASS [   0.037s] ( 312/5472) aion-awl::command_emit firing_a_declared_specimen_proves_the_refusal_is_load_bearing
        PASS [   0.007s] ( 313/5472) aion-awl::command_specimen an_admitting_invocation_has_no_specimen_to_fire
        PASS [   0.006s] ( 314/5472) aion-awl::command_specimen the_wrong_cause_code_is_filed_at_the_specimen_layer_though_its_red_arm_is_shadowed
        PASS [   0.007s] ( 315/5472) aion-awl::compile language_errors_pass_through_verbatim
        PASS [   0.032s] ( 316/5472) aion-awl::command_just_subset every_fixture_meets_its_declared_expectation
        PASS [   0.047s] ( 317/5472) aion-awl::command_emit the_same_declaration_and_parameters_emit_byte_identical_argv
        PASS [   0.005s] ( 318/5472) aion-awl::compile lowering_display_parity_for_message_and_planning
        PASS [   0.006s] ( 319/5472) aion-awl::compile multi_outcome_envelope_derives_one_arm_per_outcome
        PASS [   0.028s] ( 320/5472) aion-awl::command_refusals the_two_canonical_shapes_are_accepted
        PASS [   0.298s] ( 321/5472) aion-awl mir::select::inspect_tests::wrong_live_expectation_goes_red
        PASS [   0.006s] ( 322/5472) aion-awl::compile_visits_scope visits_route_payload_is_refused_by_the_shared_check_before_lowering
        PASS [   0.029s] ( 323/5472) aion-awl::compile declared_node_and_override_both_surface
        PASS [   0.035s] ( 324/5472) aion-awl::compile awl_hello_contracts_match_the_reference_shapes
        PASS [   0.036s] ( 325/5472) aion-awl::compile awl_hello_actions_are_the_two_unpinned_requirements
        PASS [   0.036s] ( 326/5472) aion-awl::compile call_site_node_override_yields_the_override
        PASS [   0.009s] ( 327/5472) aion-awl::concat_lowering checked_compile_rejects_mixed_concat_before_lowering
        PASS [   0.029s] ( 328/5472) aion-awl::compile mutually_recursive_payload_falls_back_via_trial_defs
        PASS [   0.027s] ( 329/5472) aion-awl::compile recursive_payload_derives_through_defs_not_inline
        PASS [   0.007s] ( 330/5472) aion-awl::concat_lowering int_alias_plus_string_still_refuses_at_direct_lowering
        PASS [   0.008s] ( 331/5472) aion-awl::concat_lowering direct_lower_refuses_concat_unless_both_operands_are_strings
        PASS [   0.005s] ( 332/5472) aion-awl::corpus_roundtrip the_walk_excludes_exactly_the_three_named_classes
        PASS [   0.007s] ( 333/5472) aion-awl::corpus_roundtrip the_comparison_rejects_an_outcome_the_tree_does_not_produce
        PASS [   0.032s] ( 334/5472) aion-awl::compile refusal_passthrough_is_lossless
        PASS [   0.049s] ( 335/5472) aion-awl::compile awl_hello_compiles_deterministically_to_the_select_bytes
        PASS [   0.007s] ( 336/5472) aion-awl::editor_assets every_editor_covers_every_node_the_authoritative_query_colours
        PASS [   0.005s] ( 337/5472) aion-awl::editor_assets grammar_builtin_types_match_the_awl_authority
        PASS [   0.051s] ( 338/5472) aion-awl::compile fork_action_fanout_compiles_deterministically_to_the_select_bytes
        PASS [   0.006s] ( 339/5472) aion-awl::editor_assets the_recorded_pin_is_present_on_the_public_mirror
        PASS [   0.012s] ( 340/5472) aion-awl::editor_assets no_query_names_a_node_the_grammar_does_not_produce
        PASS [   0.033s] ( 341/5472) aion-awl::concat_lowering checker_approved_string_alias_concat_lowers_and_selects
        PASS [   0.055s] ( 342/5472) aion-awl::compile_child_fork child_collection_fork_compiles_deterministically_to_the_select_bytes
        PASS [   0.032s] ( 343/5472) aion-awl::declared_signal_contract every_declared_signal_reaches_the_contract
        PASS [   0.033s] ( 344/5472) aion-awl::declared_signal_contract each_signal_carries_its_own_declared_schema
        PASS [   0.029s] ( 345/5472) aion-awl::emitter child_tests::child_call_lowers_to_string_name_spawn
        PASS [   0.041s] ( 346/5472) aion-awl::declared_signal_contract the_declared_schema_admits_conforming_and_refuses_malformed
        PASS [   0.029s] ( 347/5472) aion-awl::emitter child_tests::named_fork_child_refusal_is_honest
        PASS [   0.034s] ( 348/5472) aion-awl::editor_assets the_zed_pin_names_a_mirror_commit_carrying_this_grammar
        PASS [   0.367s] ( 349/5472) aion-awl mir::select::inspect_tests::make_fun_capture_reads_are_modeled_and_a_removed_reload_is_caught
        PASS [   0.036s] ( 350/5472) aion-awl::emitter child_tests::child_collection_fork_spawns_all_then_awaits_all
        PASS [   0.006s] ( 351/5472) aion-awl::emitter compile_tests::the_compile_proof_builds_where_cargo_clean_can_reach_it
        PASS [   0.029s] ( 352/5472) aion-awl::emitter child_tests::spawn_lowers_detached
        PASS [   0.031s] ( 353/5472) aion-awl::emitter codec_bodies_pin_the_same_wire_fragments
        PASS [   0.029s] ( 354/5472) aion-awl::emitter enum_total_outcomes_lower_to_case
        PASS [   0.031s] ( 355/5472) aion-awl::emitter conflicting_binding_types_across_branches_are_refused
        PASS [   0.029s] ( 356/5472) aion-awl::emitter float_orderings_render_float_operators
        PASS [   0.031s] ( 357/5472) aion-awl::emitter fork_forms_map_and_fold
        PASS [   0.037s] ( 358/5472) aion-awl::emitter flagship_scout_and_warm_build_share_one_parallel_all
        PASS [   0.005s] ( 359/5472) aion-awl::emitter gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.051s] ( 360/5472) aion-awl::emitter emitted_sources_avoid_nondeterministic_apis
        PASS [   0.021s] ( 361/5472) aion-awl::emitter fork_tests::parallel_action_fork_is_one_map_with_error_collapse
        PASS [   0.005s] ( 362/5472) aion-awl::emitter gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.004s] ( 363/5472) aion-awl::emitter gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.005s] ( 364/5472) aion-awl::emitter gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.005s] ( 365/5472) aion-awl::emitter gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.027s] ( 366/5472) aion-awl::emitter fork_tests::homogeneous_named_fork_is_one_typed_all
        PASS [   0.006s] ( 367/5472) aion-awl::emitter gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.029s] ( 368/5472) aion-awl::emitter fork_tests::sequential_fork_folds_from_empty_and_reverses
        PASS [   0.262s] ( 369/5472) aion-awl::checker every_valid_fixture_checks_clean
        PASS [   0.006s] ( 370/5472) aion-awl::emitter loop_tests::counting_name_collision_is_checker_refused
        PASS [   0.006s] ( 371/5472) aion-awl::emitter loop_tests::loop_max_must_be_loop_invariant
        PASS [   0.034s] ( 372/5472) aion-awl::emitter gleam_keywords_are_sanitized_everywhere
        PASS [   0.006s] ( 373/5472) aion-awl::emitter loop_tests::unchecked_counter_is_not_in_until_scope
        PASS [   0.014s] ( 374/5472) aion-awl::emitter loop_tests::route_inside_a_loop_body_is_refused
        PASS [   0.032s] ( 375/5472) aion-awl::emitter heterogeneous_named_fork_dispatches_one_parallel_all
        PASS [   0.030s] ( 376/5472) aion-awl::emitter loop_tests::counterless_loop_uses_the_scalar_result_path
        PASS [   0.035s] ( 377/5472) aion-awl::emitter loop_counter_is_language_owned
        PASS [   0.010s] ( 378/5472) aion-awl::emitter piped_route_with_payload_is_refused_at_emit
        PASS [   0.037s] ( 379/5472) aion-awl::emitter loop_tests::compound_until_is_short_circuited_in_nested_loops
        PASS [   0.008s] ( 380/5472) aion-awl::fallback_config duplicate_fallback_queue_is_anchored_on_the_second_occurrence
        PASS [   0.013s] ( 381/5472) aion-awl::emitter schema_import_without_root_is_a_spanned_emit_error
        PASS [   0.009s] ( 382/5472) aion-awl::fallback_config empty_and_soft_keyword_entries_are_parse_errors
        PASS [   0.006s] ( 383/5472) aion-awl::fallback_config fallback_is_declaration_only
        PASS [   0.007s] ( 384/5472) aion-awl::fallback_config fallback_then_retry_is_two_config_keys
        PASS [   0.035s] ( 385/5472) aion-awl::emitter on_failure_compensates_then_routes
        PASS [   0.043s] ( 386/5472) aion-awl::emitter nested_boolean_expressions_use_gleam_block_grouping
        PASS [   0.037s] ( 387/5472) aion-awl::emitter wait_timeout_binds_optionally
        PASS [   0.030s] ( 388/5472) aion-awl::fallback_config fallback_round_trips_and_lowers_after_retry
        PASS [   0.042s] ( 389/5472) aion-awl::emitter sequential_layer_fallback_is_announced
        PASS [   0.006s] ( 390/5472) aion-awl::flow_vocab_b4_compile_proof gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.010s] ( 391/5472) aion-awl::flow_vocab_b4_compile_proof gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.007s] ( 392/5472) aion-awl::flow_vocab_b4_compile_proof gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.030s] ( 393/5472) aion-awl::feature_any_all_workflow_id workflow_id_has_reference_direct_structural_parity
        PASS [   0.035s] ( 394/5472) aion-awl::feature_any_all_workflow_id fallible_predicates_have_result_callbacks_and_outer_binds
        PASS [   0.005s] ( 395/5472) aion-awl::flow_vocab_b4_compile_proof gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.006s] ( 396/5472) aion-awl::flow_vocab_b4_compile_proof gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.006s] ( 397/5472) aion-awl::flow_vocab_b4_compile_proof gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.007s] ( 398/5472) aion-awl::flow_vocab_compile_proof gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.038s] ( 399/5472) aion-awl::feature_any_all_workflow_id nested_any_all_has_reference_direct_structural_parity
        PASS [   0.006s] ( 400/5472) aion-awl::flow_vocab_compile_proof gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.009s] ( 401/5472) aion-awl::flow_vocab_compile_proof gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.010s] ( 402/5472) aion-awl::flow_vocab_compile_proof gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.006s] ( 403/5472) aion-awl::flow_vocab_compile_proof gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.006s] ( 404/5472) aion-awl::flow_vocab_ergonomics an_expression_headed_statement_still_needs_a_terminator
        PASS [   0.011s] ( 405/5472) aion-awl::flow_vocab_compile_proof gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.009s] ( 406/5472) aion-awl::flow_vocab_ergonomics bindings_cannot_shadow_consts
        PASS [   0.010s] ( 407/5472) aion-awl::flow_vocab_ergonomics const_values_reject_runtime_reads
        PASS [   0.014s] ( 408/5472) aion-awl::flow_vocab_ergonomics const_cycles_are_rejected_at_the_backward_reference
        PASS [   0.007s] ( 409/5472) aion-awl::flow_vocab_ergonomics duplicate_consts_are_rejected_at_the_second_declaration
        PASS [   0.006s] ( 410/5472) aion-awl::flow_vocab_ergonomics invalid_json_error_span_stays_on_char_boundaries_for_non_ascii
        PASS [   0.010s] ( 411/5472) aion-awl::flow_vocab_ergonomics invalid_json_body_points_into_the_body
        PASS [   0.029s] ( 412/5472) aion-awl::flow_vocab_ergonomics a_statement_may_start_with_a_string_literal
        PASS [   0.007s] ( 413/5472) aion-awl::flow_vocab_ergonomics json_is_a_reserved_word_at_declaration_sites
        PASS [   0.017s] ( 414/5472) aion-awl::flow_vocab_ergonomics emit_refuses_documents_that_do_not_check_cleanly
        PASS [   0.238s] ( 415/5472) aion-awl::corpus_roundtrip every_pre_existing_document_reads_exactly_as_it_did_at_the_baseline
        PASS [   0.010s] ( 416/5472) aion-awl::flow_vocab_ergonomics malformed_multiline_json_string_is_reported_inside_the_body
        PASS [   0.032s] ( 417/5472) aion-awl::flow_vocab_ergonomics consts_fold_through_emission_as_plain_strings
        PASS [   0.008s] ( 418/5472) aion-awl::flow_vocab_ergonomics mir_lower_refuses_documents_that_do_not_check_cleanly
        PASS [   0.006s] ( 419/5472) aion-awl::flow_vocab_ergonomics raw_string_value_is_verbatim_with_no_escape_processing
        PASS [   0.011s] ( 420/5472) aion-awl::flow_vocab_ergonomics of_is_a_reserved_word_at_declaration_sites
        PASS [   0.007s] ( 421/5472) aion-awl::flow_vocab_ergonomics schema_of_an_undeclared_type_is_rejected
        PASS [   0.007s] ( 422/5472) aion-awl::flow_vocab_ergonomics undefined_const_references_are_rejected
        PASS [   0.007s] ( 423/5472) aion-awl::flow_vocab_ergonomics unterminated_raw_string_is_refused_at_the_opener
        PASS [   0.008s] ( 424/5472) aion-awl::flow_vocab_ergonomics unterminated_json_body_is_refused_at_the_open_brace
        PASS [   0.032s] ( 425/5472) aion-awl::flow_vocab_ergonomics json_body_brace_counting_respects_braces_inside_strings
        PASS [   0.008s] ( 426/5472) aion-awl::flow_vocab_regions a_collect_with_no_open_region_is_rejected
        PASS [   0.545s] ( 427/5472) aion-awl mir::tests::lowers_cover_and_golden
        PASS [   0.012s] ( 428/5472) aion-awl::flow_vocab_regions a_collect_closes_the_nearest_open_region
        PASS [   0.007s] ( 429/5472) aion-awl::flow_vocab_regions a_distribute_step_may_not_carry_outcomes
        PASS [   0.009s] ( 430/5472) aion-awl::flow_vocab_regions a_route_to_a_flow_outcome_from_inside_is_rejected
        PASS [   0.031s] ( 431/5472) aion-awl::flow_vocab_ergonomics the_vocabulary_document_round_trips_byte_identically
        PASS [   0.010s] ( 432/5472) aion-awl::flow_vocab_regions a_route_out_of_the_region_is_rejected
        PASS [   0.009s] ( 433/5472) aion-awl::flow_vocab_regions a_route_to_the_collect_from_outside_is_rejected
        PASS [   0.013s] ( 434/5472) aion-awl::flow_vocab_regions a_route_entering_the_region_mid_track_is_rejected
        PASS [   0.035s] ( 435/5472) aion-awl::flow_vocab_ergonomics the_semantic_index_declares_consts_with_their_folded_type
        PASS [   0.007s] ( 436/5472) aion-awl::flow_vocab_regions an_after_edge_may_not_cross_the_region_boundary
        PASS [   0.009s] ( 437/5472) aion-awl::flow_vocab_regions an_unclosed_region_is_rejected_at_its_opener
        PASS [   0.008s] ( 438/5472) aion-awl::flow_vocab_regions distribute_must_be_its_steps_only_line
        PASS [   0.011s] ( 439/5472) aion-awl::flow_vocab_regions collect_must_open_its_step
        PASS [   0.013s] ( 440/5472) aion-awl::flow_vocab_regions distribute_needs_a_list_collection
        PASS [   0.011s] ( 441/5472) aion-awl::flow_vocab_regions region_statements_are_step_level_only
        PASS [   0.048s] ( 442/5472) aion-awl::flow_vocab_refusals every_new_construct_emits_with_its_generated_shape
        PASS [   0.014s] ( 443/5472) aion-awl::flow_vocab_regions region_locals_fall_out_of_scope_at_the_collect
        PASS [   0.013s] ( 444/5472) aion-awl::flow_vocab_regions the_collected_binding_must_be_assigned_on_every_success_path
        PASS [   0.622s] ( 445/5472) aion-awl mir::select::tests::every_lowered_fixture_emits_and_validates
        PASS [   0.014s] ( 446/5472) aion-awl::flow_vocab_regions the_collected_binding_must_be_produced_inside_the_region
        PASS [   0.044s] ( 447/5472) aion-awl::flow_vocab_regions an_empty_collection_is_legal
        PASS [   0.055s] ( 448/5472) aion-awl::flow_vocab_regions an_in_region_loopback_is_legal_and_stays_inside
        PASS [   0.048s] ( 449/5472) aion-awl::flow_vocab_regions nested_regions_with_inner_loopback_check_clean
        PASS [   0.036s] ( 450/5472) aion-awl::flow_vocab_shape a_sequence_region_with_a_strict_collect_round_trips
        PASS [   0.033s] ( 451/5472) aion-awl::flow_vocab_shape collect_results_type_strict_and_tolerant_forms
        PASS [   0.034s] ( 452/5472) aion-awl::flow_vocab_shape every_step_carries_its_kind_in_the_semantic_index
        PASS [   0.006s] ( 453/5472) aion-awl::flow_vocab_subflows a_step_level_max_must_end_in_visits
        PASS [   0.032s] ( 454/5472) aion-awl::flow_vocab_shape every_step_carries_its_region_membership_in_the_semantic_index
        PASS [   0.006s] ( 455/5472) aion-awl::flow_vocab_subflows a_subflow_call_may_not_nest_inside_a_block
        PASS [   0.011s] ( 456/5472) aion-awl::flow_vocab_subflows a_subflow_call_must_be_its_steps_only_statement
        PASS [   0.032s] ( 457/5472) aion-awl::flow_vocab_shape sequence_steps_classify_as_sequence
        PASS [   0.009s] ( 458/5472) aion-awl::flow_vocab_subflows a_subflow_cannot_be_spawned
        PASS [   0.006s] ( 459/5472) aion-awl::flow_vocab_subflows a_subflow_is_not_a_pipe_stage
        PASS [   0.010s] ( 460/5472) aion-awl::flow_vocab_subflows a_subflow_captures_nothing_from_the_enclosing_flow
        PASS [   0.032s] ( 461/5472) aion-awl::flow_vocab_shape the_subflow_declaration_reaches_the_semantic_index
        PASS [   0.014s] ( 462/5472) aion-awl::flow_vocab_subflows a_subflow_declares_exactly_one_outcome
        PASS [   0.006s] ( 463/5472) aion-awl::flow_vocab_subflows a_subflow_without_an_outcome_is_refused
        PASS [   0.032s] ( 464/5472) aion-awl::flow_vocab_subflows a_subflow_call_binds_the_outcome_type
        PASS [   0.010s] ( 465/5472) aion-awl::flow_vocab_subflows a_subflow_outcome_carries_no_route
        PASS [   0.051s] ( 466/5472) aion-awl::flow_vocab_shape the_shape_document_round_trips_byte_identically
        PASS [   0.011s] ( 467/5472) aion-awl::flow_vocab_subflows collect_requires_its_result_binding
        PASS [   0.009s] ( 468/5472) aion-awl::flow_vocab_visits a_cycle_bounded_only_by_an_inner_loop_is_now_rejected
        PASS [   0.015s] ( 469/5472) aion-awl::flow_vocab_subflows a_value_route_payload_must_match_the_outcome_type
        PASS [   0.009s] ( 470/5472) aion-awl::flow_vocab_visits a_cycle_rebinding_that_changes_the_type_is_rejected
        PASS [   0.014s] ( 471/5472) aion-awl::flow_vocab_subflows subflow_recursion_is_rejected
        PASS [   0.009s] ( 472/5472) aion-awl::flow_vocab_visits a_non_cycle_step_still_may_not_rebind
        PASS [   0.015s] ( 473/5472) aion-awl::flow_vocab_visits the_visits_bound_must_be_input_derived
        PASS [   0.017s] ( 474/5472) aion-awl::flow_vocab_visits the_visits_bound_must_be_an_int
        PASS [   0.038s] ( 475/5472) aion-awl::flow_vocab_subflows document_consts_stay_visible_inside_a_subflow
        PASS [   0.010s] ( 476/5472) aion-awl::flow_vocab_visits visits_is_unreadable_outside_the_bounded_steps_guards
        PASS [   0.010s] ( 477/5472) aion-awl::grammar_parser_gate every_assembly_form_is_refused_by_name_with_its_reason
        PASS [   0.032s] ( 478/5472) aion-awl::flow_vocab_visits a_visits_bound_on_a_cycle_member_makes_the_cycle_legal
        PASS [   0.043s] ( 479/5472) aion-awl::flow_vocab_visits a_cycle_step_may_rebind_a_cycle_threaded_name_keeping_its_type
        PASS [   0.009s] ( 480/5472) aion-awl::grammar_parser_gate the_top_level_offer_is_exactly_what_the_parser_admits
        PASS [   0.020s] ( 481/5472) aion-awl::grammar_parser_gate the_refusal_diagnostics_name_what_the_parser_actually_admits
        PASS [   0.014s] ( 482/5472) aion-awl::grammar_parser_gate worker::the_type_positions_offer_nothing_because_the_parser_admits_nothing
        PASS [   0.051s] ( 483/5472) aion-awl::flow_vocab_visits visits_in_a_bounded_guard_remains_clean_when_the_payload_uses_ordinary_data
        PASS [   0.005s] ( 484/5472) aion-awl::guide_reference_gate embedded_reference_matches_the_authored_document
        PASS [   0.007s] ( 485/5472) aion-awl::guide_reference_gate every_cited_section_actually_discusses_the_word
        PASS [   0.006s] ( 486/5472) aion-awl::guide_reference_gate every_cited_section_exists_with_the_title_the_glossary_writes
        PASS [   0.042s] ( 487/5472) aion-awl::grammar_parser_gate the_header_offer_is_openers_plus_witnessed_midline_words
        PASS [   0.034s] ( 488/5472) aion-awl::grammar_parser_gate worker::every_value_position_offer_is_witnessed_in_its_slot
        PASS [   0.008s] ( 489/5472) aion-awl::guide_reference_gate every_documented_path_the_glossary_names_exists
        PASS [   0.010s] ( 490/5472) aion-awl::guide_reference_gate the_gate_rejects_a_citation_the_reference_cannot_answer
        PASS [   0.005s] ( 491/5472) aion-awl::guide_reference_gate the_whole_word_matcher_reads_boundaries
        PASS [   0.012s] ( 492/5472) aion-awl::guide_reference_gate the_inventory_covers_the_whole_vocabulary
        PASS [   0.060s] ( 493/5472) aion-awl::grammar_parser_gate the_command_body_offer_is_openers_plus_witnessed_words
        PASS [   0.010s] ( 494/5472) aion-awl::lexer awl_hello_flagship_lexes_to_the_exact_token_sequence
        PASS [   0.006s] ( 495/5472) aion-awl::lexer dev_brief_loop_region_lexes_loop_counting_until_max
        PASS [   0.011s] ( 496/5472) aion-awl::lexer dead_awl0_words_lex_as_plain_identifiers
        PASS [   0.046s] ( 497/5472) aion-awl::grammar_parser_gate worker::the_worker_block_offer_is_openers_plus_witnessed_words
        PASS [   0.011s] ( 498/5472) aion-awl::lexer doc_header_lines_lex_as_data_with_verbatim_text
        PASS [   0.011s] ( 499/5472) aion-awl::lexer doc_line_spans_carry_true_position_and_marker
        PASS [   0.005s] ( 500/5472) aion-awl::lexer four_slashes_and_plain_comments_stay_trivia
        PASS [   0.007s] ( 501/5472) aion-awl::lexer every_rev2_keyword_lexes_as_a_keyword
        PASS [   0.016s] ( 502/5472) aion-awl::lexer doc_lines_lex_as_data_and_do_not_disturb_indentation
        PASS [   0.004s] ( 503/5472) aion-awl::lexer trailing_doc_header_marker_after_code_is_trivia_not_doc_data
        PASS [   0.005s] ( 504/5472) aion-awl::lexer trailing_comment_after_code_stays_trivia_before_newline
        PASS [   0.037s] ( 505/5472) aion-awl::implicit_child_identity same_shape_parents_receive_distinct_reserved_child_types
        PASS [   0.015s] ( 506/5472) aion-awl::lexer keywords_embedded_in_longer_identifiers_stay_identifiers
        PASS [   0.009s] ( 507/5472) aion-awl::lexer_spans columns_after_multibyte_content_on_the_same_line_are_char_based
        PASS [   0.009s] ( 508/5472) aion-awl::lexer_spans bad_escape_reports_escape_span
        PASS [   0.005s] ( 509/5472) aion-awl::lexer_spans crlf_newline_spans_cover_the_full_terminator
        PASS [   0.013s] ( 510/5472) aion-awl::lexer trailing_triple_slash_after_code_is_trivia_not_doc_data
        PASS [   0.005s] ( 511/5472) aion-awl::lexer_spans lf_newline_span_is_the_single_terminator_byte
        PASS [   0.006s] ( 512/5472) aion-awl::lexer_spans odd_indentation_reports_span
        PASS [   0.010s] ( 513/5472) aion-awl::lexer_spans lone_dot_without_field_name_reports_span
        PASS [   0.006s] ( 514/5472) aion-awl::lexer_spans spans_stay_byte_correct_after_multibyte_doc_lines
        PASS [   0.036s] ( 515/5472) aion-awl::lexer entire_rev2_corpus_lexes_without_error
        PASS [   0.010s] ( 516/5472) aion-awl::lexer_spans over_indentation_jump_reports_a_lexical_error
        PASS [   0.016s] ( 517/5472) aion-awl::lexer_spans lex_error_deep_in_document_reports_true_line_and_offset
        PASS [   0.006s] ( 518/5472) aion-awl::lexer_spans tab_in_indentation_reports_span
        PASS [   0.006s] ( 519/5472) aion-awl::lexer_spans unterminated_string_reports_span
        PASS [   0.009s] ( 520/5472) aion-awl::lexer_spans tokens_expose_byte_line_and_column_spans
        PASS [   0.005s] ( 521/5472) aion-awl::operators_and_literals combinators_take_accessor_arguments
        PASS [   0.017s] ( 522/5472) aion-awl::lexer_spans stray_character_reports_span
        PASS [   0.005s] ( 523/5472) aion-awl::operators_and_literals durations_lex_with_all_units_and_backoff_ranges
        PASS [   0.005s] ( 524/5472) aion-awl::operators_and_literals float_lexeme_is_preserved_verbatim
        PASS [   0.018s] ( 525/5472) aion-awl::operators_and_literals arrow_binds_call_results_and_accessors_chain
        PASS [   0.023s] ( 526/5472) aion-awl::lexer_spans token_deep_in_document_reports_true_line_and_offset
        PASS [   0.013s] ( 527/5472) aion-awl::operators_and_literals fork_line_lexes_with_in_as_identifier
        PASS [   0.010s] ( 528/5472) aion-awl::operators_and_literals inline_schema_json_tokens_lex
        PASS [   0.017s] ( 529/5472) aion-awl::operators_and_literals enum_bar_is_distinct_from_pipe
        PASS [   0.022s] ( 530/5472) aion-awl::operators_and_literals comparisons_booleans_and_string_concat_lex
        PASS [   0.009s] ( 531/5472) aion-awl::operators_and_literals list_literals_and_literal_indexing_lex
        PASS [   0.007s] ( 532/5472) aion-awl::operators_and_literals loop_seed_equal_and_schema_door_equal_lex_as_equal
        PASS [   0.005s] ( 533/5472) aion-awl::operators_and_literals predicates_lex_as_keyword_pairs
        PASS [   0.007s] ( 534/5472) aion-awl::operators_and_literals pipe_chain_with_accessor_stage_and_route_terminator
        PASS [   0.006s] ( 535/5472) aion-awl::operators_and_literals question_marks_optional_types_including_list_types
        PASS [   0.009s] ( 536/5472) aion-awl::operators_and_literals separated_unit_letter_is_not_a_duration
        PASS [   0.007s] ( 537/5472) aion-awl::operators_and_literals string_escapes_booleans_and_floats_lex
        PASS [   0.007s] ( 538/5472) aion-awl::operators_and_literals unit_prefixed_identifier_is_not_a_duration
        PASS [   0.011s] ( 539/5472) aion-awl::operators_and_literals spawn_join_wait_timeout_line_shapes_lex
        PASS [   0.007s] ( 540/5472) aion-awl::parser_corpus corpus_has_not_shrunk
        PASS [   0.263s] ( 541/5472) aion-awl::flow_vocab_roundtrip_property generated_vocabulary_documents_round_trip_losslessly
        PASS [   0.007s] ( 542/5472) aion-awl::parser_diagnostics call_headed_pipe_chain_gets_a_targeted_fix_it
        PASS [   0.009s] ( 543/5472) aion-awl::parser_diagnostics duplicate_loop_until_is_rejected_at_the_second_keyword
        PASS [   0.010s] ( 544/5472) aion-awl::parser_diagnostics duplicate_loop_max_is_rejected_at_the_second_keyword
        PASS [   0.009s] ( 545/5472) aion-awl::parser_diagnostics statement_after_an_outcome_clause_is_rejected
        PASS [   0.019s] ( 546/5472) aion-awl::parser_corpus every_parse_staged_invalid_fixture_fails_as_expected
        PASS [   0.014s] ( 547/5472) aion-awl::printer_roundtrip comma_tolerance_normalizes_to_canonical_form
        PASS [   0.012s] ( 548/5472) aion-awl::printer_roundtrip long_pipe_chains_break_before_each_stage
        PASS [   0.006s] ( 549/5472) aion-awl::printer_roundtrip stageless_bind_over_the_width_limit_stays_on_one_line_and_reparses
        PASS [   0.009s] ( 550/5472) aion-awl::printer_roundtrip wrapped_bind_chain_keeps_its_trailing_comment
        PASS [   0.012s] ( 551/5472) aion-awl::printer_roundtrip trailing_comments_survive_and_normalize
        PASS [   0.051s] ( 552/5472) aion-awl::parser_corpus every_check_staged_invalid_fixture_parses_cleanly
        PASS [   0.053s] ( 553/5472) aion-awl::parser_corpus every_valid_fixture_parses
        PASS [   0.052s] ( 554/5472) aion-awl::printer_roundtrip parse_print_is_identity_over_the_canonical_corpus
        PASS [   0.064s] ( 555/5472) aion-awl::printer_roundtrip print_parse_print_is_idempotent_over_the_whole_corpus
        PASS [   0.048s] ( 556/5472) aion-awl::query_surface both_words_stay_usable_as_ordinary_names
        PASS [   0.036s] ( 557/5472) aion-awl::query_surface queries_parse_only_when_declared
        PASS [   0.067s] ( 558/5472) aion-awl::query_surface both_backends_register_against_the_arguments_bearing_handler
        PASS [   0.038s] ( 559/5472) aion-awl::query_surface the_checker_refuses_each_broken_shape_and_accepts_its_repair
        PASS [   0.067s] ( 560/5472) aion-awl::query_surface both_backends_register_the_same_number_of_answers
        PASS [   0.360s] ( 561/5472) aion-awl::flow_vocab_roundtrip_property generated_flow_shape_documents_round_trip_losslessly
        PASS [   0.005s] ( 562/5472) aion-awl::recipe_compare_genericity a_record_that_names_nothing_is_unrepresentable
        PASS [   0.611s] ( 563/5472) aion-awl::emitter every_valid_fixture_emits
        PASS [   0.009s] ( 564/5472) aion-awl::recipe_compare_genericity the_compare_surface_matches_on_no_cause_identity
        PASS [   0.008s] ( 565/5472) aion-awl::recipe_compare_genericity the_ratchet_shaped_prior_art_is_cited_at_its_bytes
        PASS [   0.020s] ( 566/5472) aion-awl::recipe_compare_genericity the_compare_consumes_every_member_of_the_vocabulary_uniformly
        PASS [   0.032s] ( 567/5472) aion-awl::recipe_compare_genericity a_ratchet_shaped_record_flows_through_the_compare_as_data
        PASS [   0.039s] ( 568/5472) aion-awl::query_surface the_reference_backend_emits_one_handler_per_answer
        PASS [   0.073s] ( 569/5472) aion-awl::query_surface the_compile_seam_carries_the_surface_into_the_beam
        PASS [   0.028s] ( 570/5472) aion-awl::recipe_compose a_bound_value_still_carrying_its_sentinel_refuses
        PASS [   0.028s] ( 571/5472) aion-awl::recipe_compose a_correction_propagates_to_every_consumer_with_a_sweep_record_listing_them
        PASS [   0.660s] ( 572/5472) aion-awl::emitter compile_tests::query_surface_fixture_compiles_under_gleam
        PASS [   0.008s] ( 573/5472) aion-awl::recipe_compose the_sentinel_rule_reads_a_real_value_as_a_real_value
        PASS [   0.030s] ( 574/5472) aion-awl::recipe_compose a_declared_default_fills_a_slot_the_composition_omits
        PASS [   0.007s] ( 575/5472) aion-awl::recipe_corpus the_fixture_set_carries_its_own_completeness_claim
        PASS [   0.684s] ( 576/5472) aion-awl::emitter compile_tests::flagship_fixtures_compile_under_gleam
        PASS [   0.024s] ( 577/5472) aion-awl::recipe_corpus every_exhibit_matches_the_digest_the_rig_pinned
        PASS [   0.089s] ( 578/5472) aion-awl::query_surface the_printer_round_trips_the_surface
        PASS [   0.097s] ( 579/5472) aion-awl::query_surface the_derived_schema_carries_the_query_surface
        PASS [   0.044s] ( 580/5472) aion-awl::recipe_compose a_source_no_recipe_names_is_swept_to_an_empty_list
        PASS [   0.039s] ( 581/5472) aion-awl::recipe_compose an_unfilled_slot_refuses_with_the_slot_name_in_observed_bytes
        PASS [   0.050s] ( 582/5472) aion-awl::recipe_compose a_filled_composition_resolves_and_defers_only_the_re_derive_slots
        PASS [   0.093s] ( 583/5472) aion-awl::query_surface the_direct_backend_lowers_one_handler_call_per_answer
        PASS [   0.005s] ( 584/5472) aion-awl::recipe_declarations a_shell_string_step_body_is_unparseable
        PASS [   0.008s] ( 585/5472) aion-awl::recipe_declarations a_command_declaration_cannot_be_nested_inside_a_recipe
        PASS [   0.702s] ( 586/5472) aion-awl::emitter compile_tests::loop_and_fork_fixtures_compile_under_gleam
        PASS [   0.006s] ( 587/5472) aion-awl::recipe_declarations every_assembly_form_still_refuses_inside_a_command_declaration
        PASS [   0.029s] ( 588/5472) aion-awl::recipe_corpus the_ground_sentinel_guard_is_expressed_as_a_declared_artifact
        PASS [   0.043s] ( 589/5472) aion-awl::recipe_corpus the_field_kind_rows_become_parameter_kind_declarations
        PASS [   0.018s] ( 590/5472) aion-awl::recipe_declarations a_cyclic_dependency_is_a_check_error_with_a_span
        PASS [   0.051s] ( 591/5472) aion-awl::recipe_compose every_unfilled_slot_is_reported_at_once
        PASS [   0.006s] ( 592/5472) aion-awl::recipe_declarations the_parameter_kind_taxonomy_is_closed
        PASS [   0.030s] ( 593/5472) aion-awl::recipe_corpus the_kit_sed_fill_slots_become_declared_parameters
        PASS [   0.006s] ( 594/5472) aion-awl::recipe_fire a_receipt_that_cannot_say_what_it_measured_is_unrepresentable
        PASS [   0.037s] ( 595/5472) aion-awl::recipe_corpus the_haem_wall_allowlist_is_one_declaration_and_its_projections_are_measured
        PASS [   0.032s] ( 596/5472) aion-awl::recipe_corpus the_two_build_input_copies_differ_and_both_are_expressed
        PASS [   0.034s] ( 597/5472) aion-awl::recipe_corpus the_staging_checklist_becomes_step_kinds_with_its_exclusions_named
        PASS [   0.029s] ( 598/5472) aion-awl::recipe_declarations the_edges_and_not_the_source_order_decide_what_runs_first
        PASS [   0.034s] ( 599/5472) aion-awl::recipe_declarations the_assembly_surface_round_trips_through_the_canonical_printer
        PASS [   0.028s] ( 600/5472) aion-awl::recipe_fire a_declared_command_that_fails_stops_the_stage
        PASS [   0.033s] ( 601/5472) aion-awl::recipe_declarations the_three_step_bodies_parse_into_their_typed_forms
        PASS [   0.027s] ( 602/5472) aion-awl::recipe_fire a_recipe_may_consume_a_dispatch_value_and_may_not_restate_it
        PASS [   0.033s] ( 603/5472) aion-awl::recipe_fire a_clean_census_proceeds_and_the_receipt_lists_what_was_measured
        PASS [   0.035s] ( 604/5472) aion-awl::recipe_fire a_census_mismatch_refuses_the_fire_with_both_hashes
        PASS [   0.008s] ( 605/5472) aion-awl::recipe_fire an_object_that_moved_between_the_write_and_the_claim_refuses
        PASS [   0.030s] ( 606/5472) aion-awl::recipe_fire a_specimen_fire_against_the_known_answer_red_passes_and_its_receipt_carries_the_comparison
        PASS [   0.009s] ( 607/5472) aion-awl::recipe_fire an_object_that_stayed_put_is_claimed
        PASS [   0.027s] ( 608/5472) aion-awl::recipe_fire a_stage_compare_mismatch_refuses_with_both_sides_bytes
        PASS [   0.010s] ( 609/5472) aion-awl::recipe_parameters a_declaration_carrying_the_same_truth_twice_is_a_check_error
        PASS [   0.036s] ( 610/5472) aion-awl::recipe_fire a_step_naming_an_undeclared_command_refuses_at_the_fire_layer
        PASS [   0.032s] ( 611/5472) aion-awl::recipe_fire an_artifact_still_carrying_its_sentinel_refuses_at_the_fire_minute
        PASS [   0.046s] ( 612/5472) aion-awl::recipe_fire a_runs_command_step_hands_the_emitted_argv_to_the_runner
        PASS [   0.759s] ( 613/5472) aion-awl::emitter compile_tests::signal_sender_fixture_compiles_under_gleam
        PASS [   0.017s] ( 614/5472) aion-awl::recipe_fire the_recipe_family_opens_no_file_and_spawns_no_process
        PASS [   0.008s] ( 615/5472) aion-awl::recipe_parameters a_derivation_on_a_non_re_derive_kind_is_a_check_error
        PASS [   0.010s] ( 616/5472) aion-awl::recipe_parameters a_re_derive_parameter_without_a_derivation_is_a_check_error
        PASS [   0.039s] ( 617/5472) aion-awl::recipe_fire an_artifact_with_no_declared_digest_cannot_pass_the_census
        PASS [   0.010s] ( 618/5472) aion-awl::recipe_parameters an_extraction_outside_the_closed_set_is_a_check_error
        PASS [   0.030s] ( 619/5472) aion-awl::recipe_fire an_unanswered_precondition_stops_the_fire
        PASS [   0.008s] ( 620/5472) aion-awl::recipe_parameters the_extraction_grammar_reads_exactly_two_forms
        PASS [   0.034s] ( 621/5472) aion-awl::recipe_fire the_census_is_the_set_of_referenced_artifacts_named_once
        PASS [   0.034s] ( 622/5472) aion-awl::recipe_fire this_cut_adds_no_venue_binding_hole_beyond_the_one_cut_one_admitted
        PASS [   0.039s] ( 623/5472) aion-awl::recipe_fire an_unmeasurable_census_row_refuses_the_fire
        PASS [   0.026s] ( 624/5472) aion-awl::recipe_parameters a_derivation_resolves_at_fire_time_against_the_named_source
        PASS [   0.007s] ( 625/5472) aion-awl::recipe_wall a_wall_that_allows_nothing_is_a_check_error
        PASS [   0.646s] ( 626/5472) aion-awl::flow_vocab_compile_proof flow_vocab_b1_fixture_compiles_under_gleam
        PASS [   0.059s] ( 627/5472) aion-awl::recipe_fire an_inexact_restoration_refuses
        PASS [   0.027s] ( 628/5472) aion-awl::recipe_parameters literal_where_derived_fires_against_a_re_derive_slot_fed_a_literal
        PASS [   0.036s] ( 629/5472) aion-awl::recipe_parameters an_unreadable_derivation_source_refuses_at_the_fire_layer
        PASS [   0.029s] ( 630/5472) aion-awl::recipe_wall a_diff_against_an_unprojectable_state_is_the_projection_refusal
        PASS [   0.032s] ( 631/5472) aion-awl::recipe_parameters each_declared_kind_parses_and_checks
        PASS [   0.026s] ( 632/5472) aion-awl::recipe_wall a_projection_that_would_emit_an_excluded_term_refuses
        PASS [   0.027s] ( 633/5472) aion-awl::recipe_wall a_wall_whose_exclusions_are_absent_from_it_projects
        PASS [   0.029s] ( 634/5472) aion-awl::recipe_wall an_excluded_term_in_the_walls_own_name_refuses
        PASS [   0.028s] ( 635/5472) aion-awl::recipe_wall one_wall_declaration_projects_into_both_renderings
        PASS [   0.027s] ( 636/5472) aion-awl::recipe_wall the_enforcement_rendering_escapes_accidents_and_keeps_intent
        PASS [   0.049s] ( 637/5472) aion-awl::recipe_parameters composing_a_re_derive_slot_refuses_with_the_consumed_cause
        PASS [   0.058s] ( 638/5472) aion-awl::recipe_parameters an_extraction_past_the_end_of_its_source_refuses
        PASS [   0.029s] ( 639/5472) aion-awl::recipe_wall the_structured_diff_names_every_changed_projection
        PASS [   0.032s] ( 640/5472) aion-awl::recipe_wall two_identical_declaration_states_diff_to_nothing
        PASS [   0.028s] ( 641/5472) aion-awl::retry_counting reference_emitter_maps_three_further_attempts_to_four_total
        PASS [   0.034s] ( 642/5472) aion-awl::retry_counting direct_lowering_maps_three_further_attempts_to_four_total
        PASS [   0.036s] ( 643/5472) aion-awl::retry_counting declared_count_is_the_identity_and_does_not_move
        PASS [   0.039s] ( 644/5472) aion-awl::retry_counting zero_further_attempts_is_one_total_execution
        PASS [   0.039s] ( 645/5472) aion-awl::runtime_codecs assert_list_badmatch_reports_the_subject_at_runtime
        PASS [   0.155s] ( 646/5472) aion-awl::recipe_declarations a_deep_chain_orders_rather_than_overflowing
        PASS [   0.028s] ( 647/5472) aion-awl::runtime_codecs fix_cycle::empty_list_literal_materializes_nil_at_runtime
        PASS [   0.007s] ( 648/5472) aion-awl::runtime_codecs gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.005s] ( 649/5472) aion-awl::runtime_codecs gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.006s] ( 650/5472) aion-awl::runtime_codecs gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.005s] ( 651/5472) aion-awl::runtime_codecs gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.006s] ( 652/5472) aion-awl::runtime_codecs gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.005s] ( 653/5472) aion-awl::runtime_codecs gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.952s] ( 654/5472) aion-awl::flow_vocab_b4_compile_proof b4_flow_shape_fixtures_compile_under_gleam
        PASS [   0.841s] ( 655/5472) aion-awl::runtime_codecs concat::general_concat_executes_with_emitter_byte_parity
        PASS [   0.886s] ( 656/5472) aion-awl::runtime_codecs nested_collection_predicates_execute_with_reference_parity
        PASS [   0.063s] ( 657/5472) aion-awl::runtime_codecs semantic_pins::value_boolean_operators_skip_faulting_rhs_at_runtime
        PASS [   1.039s] ( 658/5472) aion-awl::runtime_codecs direct_codecs_execute_with_reference_parity
        PASS [   0.856s] ( 659/5472) aion-awl::runtime_codecs pipes::pipe_combinators_execute_with_reference_parity
        PASS [   1.216s] ( 660/5472) aion-awl::runtime_codecs fix_cycle::dev_brief_fix_cycle_encode_reaches_dispatch_with_reference_parity
        PASS [   1.507s] ( 661/5472) aion-awl::runtime_codecs fork_generality::plain_call_site_config_merges_per_key_with_reference_parity
        PASS [   0.033s] ( 662/5472) aion-awl::schema a_door_the_validator_cannot_compile_is_refused_at_check
        PASS [   1.562s] ( 663/5472) aion-awl::runtime_codecs fork_generality::fork_inside_loop_completes_two_passes_with_reference_parity
        PASS [   1.574s] ( 664/5472) aion-awl::runtime_codecs fork_generality::named_fork_heterogeneous_site_config_parity
        PASS [   1.578s] ( 665/5472) aion-awl::runtime_codecs fork_generality::named_fork_homogeneous_site_config_parity
        PASS [   0.028s] ( 666/5472) aion-awl::schema builtins_lists_and_optionals_map_to_json_types
        PASS [   1.558s] ( 667/5472) aion-awl::runtime_codecs pipes::pipe_child_stage_executes_with_reference_parity
        PASS [   0.006s] ( 668/5472) aion-awl::schema import_derivation_without_a_root_refuses
        PASS [   0.062s] ( 669/5472) aion-awl::schema an_imported_schemas_internal_refs_survive_composition
        PASS [   0.055s] ( 670/5472) aion-awl::schema check_proves_the_signal_child_and_activity_schemas_too
        PASS [   0.027s] ( 671/5472) aion-awl::schema imported_schemas_reemit_verbatim_with_constraints_preserved
        PASS [   0.045s] ( 672/5472) aion-awl::schema enums_derive_string_enum_schemas
        PASS [   0.052s] ( 673/5472) aion-awl::schema doc_comments_flow_to_descriptions_at_type_and_field_level
        PASS [   0.005s] ( 674/5472) aion-awl::schema unknown_types_are_refused_by_name
        PASS [   0.029s] ( 675/5472) aion-awl::schema inline_schemas_reemit_their_pasted_json_with_constraints
        PASS [   0.008s] ( 676/5472) aion-awl::schema_door brace_on_the_line_after_schema_is_not_a_raw_door
        PASS [   0.006s] ( 677/5472) aion-awl::schema_door braces_and_escaped_quotes_inside_json_strings_do_not_close_the_body
        PASS [   0.026s] ( 678/5472) aion-awl::schema recursive_records_self_reference_the_schema_root
        PASS [   0.030s] ( 679/5472) aion-awl::schema optional_fields_are_omitted_from_required_and_never_nullable
        PASS [   0.008s] ( 680/5472) aion-awl::schema_door inline_schema_body_is_one_verbatim_token
        PASS [   0.006s] ( 681/5472) aion-awl::schema_door negative_numbers_exponents_and_json_escapes_pass_verbatim
        PASS [   0.006s] ( 682/5472) aion-awl::schema_door no_structural_tokens_are_emitted_inside_a_multiline_body
        PASS [   0.028s] ( 683/5472) aion-awl::schema workflow_contract_covers_inputs_with_optionality_and_narration
        PASS [   0.005s] ( 684/5472) aion-awl::schema_door pasted_schema_with_non_two_space_indentation_lexes
        PASS [   0.005s] ( 685/5472) aion-awl::schema_door schema_body_span_is_document_true
        PASS [   0.041s] ( 686/5472) aion-awl::schema named_type_references_become_local_defs
        PASS [   0.005s] ( 687/5472) aion-awl::schema_door schema_file_import_door_is_not_raw_captured
        PASS [   0.011s] ( 688/5472) aion-awl::schema_door single_line_schema_body_keeps_lexing_the_rest_of_the_line
        PASS [   0.010s] ( 689/5472) aion-awl::schema_door tokens_after_a_multiline_body_report_the_closing_line
        PASS [   0.012s] ( 690/5472) aion-awl::schema_door unterminated_schema_body_reports_the_opening_brace
        PASS [   1.677s] ( 691/5472) aion-awl::runtime_codecs fork_generality::parallel_action_fork_site_config_dispatches_with_reference_parity
        PASS [   0.036s] ( 692/5472) aion-awl::semantic semantic_spans_cover_declarations_and_checker_owned_scopes
        PASS [   0.029s] ( 693/5472) aion-awl::shell_run_example the_shell_run_example_checks_clean_and_is_canonically_formatted
        PASS [   0.006s] ( 694/5472) aion-awl::signal_sender a_duplicate_outbound_declaration_is_refused
        PASS [   0.008s] ( 695/5472) aion-awl::signal_sender a_missing_payload_field_is_refused
        PASS [   0.006s] ( 696/5472) aion-awl::signal_sender a_non_string_target_is_refused
        PASS [   0.053s] ( 697/5472) aion-awl::shell_run_example the_example_command_is_identity_bound
        PASS [   0.010s] ( 698/5472) aion-awl::signal_sender a_payload_field_of_the_wrong_type_is_refused
        PASS [   0.006s] ( 699/5472) aion-awl::signal_sender a_send_without_a_target_is_named
        PASS [   0.709s] ( 700/5472) aion-awl::runtime_codecs stmt_parity::index_and_lists_execute_with_reference_parity
        PASS [   0.016s] ( 701/5472) aion-awl::signal_sender a_send_is_not_an_action_body
        PASS [   0.009s] ( 702/5472) aion-awl::signal_sender an_outbound_name_must_be_snake_case
        PASS [   0.069s] ( 703/5472) aion-awl::signal_sender a_document_without_sends_is_unaffected
        PASS [   0.032s] ( 704/5472) aion-awl::signal_sender a_send_is_legal_inside_an_on_failure_block
        PASS [   0.011s] ( 705/5472) aion-awl::signal_sender named_fields_on_a_leaf_payload_are_refused
        PASS [   0.070s] ( 706/5472) aion-awl::short_circuit gleam_emitter_keeps_optional_rhs_inside_some_branch
        PASS [   0.075s] ( 707/5472) aion-awl::shell_run_example the_shell_run_example_compiles_with_its_command_in_the_package_contract
        PASS [   0.013s] ( 708/5472) aion-awl::signal_sender sending_a_receive_only_signal_names_the_direction
        PASS [   0.013s] ( 709/5472) aion-awl::signal_sender the_bare_form_is_refused_for_a_non_nil_payload
        PASS [   0.013s] ( 710/5472) aion-awl::signal_sender the_corpus_fixture_is_already_canonical
        PASS [   0.017s] ( 711/5472) aion-awl::signal_sender sending_an_undeclared_signal_is_a_check_error
        PASS [   0.008s] ( 712/5472) aion-awl::signal_sender the_payload_after_the_target_gets_a_fix_it
        PASS [   0.042s] ( 713/5472) aion-awl::signal_sender a_well_formed_send_parses_checks_and_round_trips
        PASS [   0.034s] ( 714/5472) aion-awl::signal_sender one_name_may_be_declared_inbound_and_outbound
        PASS [   0.006s] ( 715/5472) aion-awl::worker_documents a_worker_document_publishes_no_start_contract
        PASS [   0.006s] ( 716/5472) aion-awl::worker_documents every_action_body_carries_its_capture
        PASS [   0.008s] ( 717/5472) aion-awl::worker_documents command_hygiene_holds_in_the_worker_family
        PASS [   0.007s] ( 718/5472) aion-awl::worker_documents printing_normalizes_and_reparses
        PASS [   0.006s] ( 719/5472) aion-awl::worker_documents the_opening_declaration_chooses_the_family
        PASS [   0.045s] ( 720/5472) aion-awl::signal_sender send_is_not_reserved_and_still_names_an_action
        PASS [   0.006s] ( 721/5472) aion-awl::worker_documents the_worker_compiler_refuses_a_workflow_document_by_name
        PASS [   0.005s] ( 722/5472) aion-awl::worker_documents the_workflow_compiler_refuses_a_worker_document_by_name
        PASS [   0.007s] ( 723/5472) aion-awl::workflow_timeout overflowing_workflow_timeout_is_checked_at_the_declaration
        PASS [   0.042s] ( 724/5472) aion-awl::signal_sender the_reference_backend_emits_signal_send
        PASS [   0.041s] ( 725/5472) aion-awl::worker_documents a_worker_document_checks_clean_and_prints_canonically
        PASS [   0.049s] ( 726/5472) aion-awl::signal_sender the_direct_backend_reaches_the_same_sdk_entry_point
        PASS [   0.007s] ( 727/5472) aion-awl-lsp analysis::tests::point_spans_stay_points
        PASS [   0.016s] ( 728/5472) aion-awl::workflow_timeout workflow_timeout_parses_and_prints_with_header_trivia
        PASS [   0.009s] ( 729/5472) aion-awl-lsp analysis::tests::positions_are_utf16_not_bytes_or_scalar_columns
        PASS [   0.040s] ( 730/5472) aion-awl::worker_documents the_advertised_contract_stays_worker_owed
        PASS [   0.017s] ( 731/5472) aion-awl::workflow_timeout zero_and_negative_workflow_timeouts_are_checked_at_the_declaration
        PASS [   0.044s] ( 732/5472) aion-awl::worker_documents the_advertised_surface_matches_the_callers_contract
        PASS [   3.041s] ( 733/5472) aion-awl mir::select::inspect_tests::covered_ratchet_upholds_ir14_and_section_11
        PASS [   1.833s] ( 734/5472) aion-awl::runtime_codecs fork_generality::sequential_fork_site_config_and_index_prelude_dispatch_with_reference_parity
        PASS [   0.029s] ( 735/5472) aion-awl-lsp bindings::bindings_tests::a_different_declaration_sharing_the_name_is_untouched
        PASS [   0.029s] ( 736/5472) aion-awl-lsp bindings::bindings_tests::a_name_of_the_wrong_case_class_is_refused
        PASS [   0.032s] ( 737/5472) aion-awl-lsp bindings::bindings_tests::a_collision_with_the_same_kind_is_refused
        PASS [   0.031s] ( 738/5472) aion-awl-lsp bindings::bindings_tests::a_keyword_binds_nothing_and_highlights_nothing
        PASS [   0.030s] ( 739/5472) aion-awl-lsp bindings::bindings_tests::a_position_with_no_declaration_is_refused
        PASS [   0.034s] ( 740/5472) aion-awl-lsp bindings::bindings_tests::a_reserved_keyword_is_refused
        PASS [   0.060s] ( 741/5472) aion-awl::workflow_timeout timeout_is_compile_metadata_for_both_backends_not_workflow_code
        PASS [   0.005s] ( 742/5472) aion-awl-lsp completion::completion_tests::a_pipe_offers_its_stages
        PASS [   0.006s] ( 743/5472) aion-awl-lsp completion::completion_tests::a_type_position_offers_declared_and_builtin_types
        PASS [   0.010s] ( 744/5472) aion-awl-lsp completion::completion_tests::bindings_produced_by_a_statement_are_offered
        PASS [   0.030s] ( 745/5472) aion-awl-lsp bindings::bindings_tests::renaming_a_type_rewrites_its_uses
        PASS [   0.033s] ( 746/5472) aion-awl-lsp bindings::bindings_tests::prepare_selects_the_declaration_name
        PASS [   0.033s] ( 747/5472) aion-awl-lsp bindings::bindings_tests::references_find_every_use_and_honour_the_declaration_flag
        PASS [   0.031s] ( 748/5472) aion-awl-lsp bindings::bindings_tests::the_declaration_highlights_as_a_write_and_its_uses_as_reads
        PASS [   0.029s] ( 749/5472) aion-awl-lsp bindings::bindings_tests::the_highlight_set_is_exactly_what_a_rename_would_edit
        PASS [   0.035s] ( 750/5472) aion-awl-lsp bindings::bindings_tests::renaming_an_input_rewrites_every_use_and_leaves_a_checking_document
        PASS [   0.045s] ( 751/5472) aion-awl-lsp bindings::bindings_tests::highlighting_does_not_light_a_different_declaration_of_the_same_name
        PASS [   0.028s] ( 752/5472) aion-awl-lsp bindings::bindings_tests::the_refusals_are_not_a_provider_that_refuses_everything
        PASS [   0.010s] ( 753/5472) aion-awl-lsp completion::completion_tests::declared_names_survive_a_line_that_cannot_be_lexed
        PASS [   0.009s] ( 754/5472) aion-awl-lsp completion::completion_tests::every_item_is_classified
        PASS [   0.009s] ( 755/5472) aion-awl-lsp completion::completion_tests::declared_names_are_offered_while_the_document_does_not_parse
        PASS [   0.010s] ( 756/5472) aion-awl-lsp completion::completion_tests::declared_names_sort_above_keywords
        PASS [   0.008s] ( 757/5472) aion-awl-lsp completion::completion_tests::every_keyword_is_offered_somewhere
        PASS [   0.008s] ( 758/5472) aion-awl-lsp completion::completion_tests::every_offering_position_is_reachable_through_completions
        PASS [   0.009s] ( 759/5472) aion-awl-lsp completion::completion_tests::route_offers_outcomes_and_steps_not_statement_openers
        PASS [   0.009s] ( 760/5472) aion-awl-lsp completion::completion_tests::the_enclosing_block_decides_when_nothing_else_does
        PASS [   0.006s] ( 761/5472) aion-awl-lsp completion::completion_tests::wait_offers_signals_only
        PASS [   0.008s] ( 762/5472) aion-awl-lsp glossary::glossary_tests::a_clause_word_of_the_wrong_cluster_hovers_to_nothing
        PASS [   0.011s] ( 763/5472) aion-awl-lsp completion::completion_tests::the_positional_words_are_offered_where_the_parser_accepts_them
        PASS [   0.006s] ( 764/5472) aion-awl-lsp glossary::glossary_tests::an_offered_word_carries_the_language_s_sentence
        PASS [   0.013s] ( 765/5472) aion-awl-lsp glossary::glossary_tests::a_command_word_used_as_a_name_still_hovers_to_nothing
        PASS [   0.011s] ( 766/5472) aion-awl-lsp glossary::glossary_tests::a_keyword_answers_in_a_document_that_does_not_parse
        PASS [   0.015s] ( 767/5472) aion-awl-lsp completion::completion_tests::value_words_are_offered_where_authors_actually_write_them
        PASS [   0.009s] ( 768/5472) aion-awl-lsp glossary::glossary_tests::every_command_word_hovers_to_the_guide_sentence_from_the_table
        PASS [   0.012s] ( 769/5472) aion-awl-lsp glossary::glossary_tests::completion_inside_a_command_body_offers_the_bodys_vocabulary
        PASS [   0.006s] ( 770/5472) aion-awl-lsp glossary::glossary_tests::hover_makes_no_claim_about_a_word_that_might_be_a_name
        PASS [   0.011s] ( 771/5472) aion-awl-lsp glossary::glossary_tests::every_offered_word_of_the_language_is_documented
        PASS [   0.010s] ( 772/5472) aion-awl-lsp glossary::glossary_tests::hover_and_completion_agree_and_completion_shows_one_example
        PASS [   0.017s] ( 773/5472) aion-awl-lsp glossary::glossary_tests::every_recipe_word_hovers_to_the_guide_sentence_from_the_table
        PASS [   0.032s] ( 774/5472) aion-awl-lsp glossary::glossary_tests::a_declared_name_still_wins_over_the_glossary
        PASS [   0.024s] ( 775/5472) aion-awl-lsp::protocol live_completion_answers_a_half_typed_step_body
        PASS [   0.023s] ( 776/5472) aion-awl-lsp::protocol live_initialize_open_diagnose_format_shutdown_exchange
        PASS [   0.007s] ( 777/5472) aion-awl-package bundle::tests::closure_is_nonempty_and_versioned
        PASS [   0.010s] ( 778/5472) aion-awl-package bundle::tests::generated_table_is_sorted_and_duplicate_free
        PASS [   0.005s] ( 779/5472) aion-awl-package source::tests::a_current_directory_prefix_normalises_to_the_plain_path
        PASS [   0.035s] ( 780/5472) aion-awl-lsp::protocol rev2_corpus_hover_and_definition_use_checker_semantics
        PASS [   0.006s] ( 781/5472) aion-awl-package source::tests::a_document_filename_with_a_directory_part_is_refused
        PASS [   0.006s] ( 782/5472) aion-awl-package source::tests::a_document_without_imports_collects_no_schemas
        PASS [   0.007s] ( 783/5472) aion-awl-package source::tests::an_import_above_the_document_root_is_refused
        PASS [   0.007s] ( 784/5472) aion-awl-package source::tests::nested_schema_imports_keep_their_document_relative_paths
        PASS [   0.050s] ( 785/5472) aion-awl-lsp::protocol live_document_highlight_distinguishes_the_declaration_from_its_uses
        PASS [   0.010s] ( 786/5472) aion-awl-package source::tests::an_unreadable_import_is_reported
        PASS [   0.050s] ( 787/5472) aion-awl-lsp::protocol live_references_and_rename_including_a_refusal
        PASS [   0.007s] ( 788/5472) aion-awl-package source::tests::sibling_schema_import_is_archived_beside_the_document
        PASS [   0.057s] ( 789/5472) aion-awl-package assemble::tests::repeated_action_requirements_dedupe_into_one_activity
        PASS [   0.039s] ( 790/5472) aion-awl-package::awl_assemble bundle_is_closed_under_imports_modulo_engine_and_erts_modules
        PASS [   0.092s] ( 791/5472) aion-awl-lsp::corpus lsp_g2_all_valid_fixtures_use_the_canonical_idempotent_printer
        PASS [   0.037s] ( 792/5472) aion-awl-package::awl_assemble every_workflow_beam_import_is_shipped_in_the_closure
        PASS [   0.009s] ( 793/5472) aion-awl-package::awl_assemble unsafe_workflow_name_is_refused_typed
        PASS [   0.012s] ( 794/5472) aion-awl-package::awl_assemble regeneration_script_is_documented_and_outside_the_product_path
        PASS [   0.007s] ( 795/5472) aion-awl-package::awl_assemble workflow_name_colliding_with_a_bundle_module_is_refused_typed
        PASS [   0.064s] ( 796/5472) aion-awl-package::awl_assemble bundle_is_versioned_pinned_and_carries_the_imported_sdk_modules
        PASS [   0.066s] ( 797/5472) aion-awl-package::awl_assemble fixed_input_archive_bytes_match_the_cross_process_pin
        PASS [   0.126s] ( 798/5472) aion-awl-package assemble::tests::unauthored_timeout_is_absent_and_authored_timeout_is_declared
        PASS [   0.110s] ( 799/5472) aion-awl-package::awl_assemble assembling_twice_yields_byte_identical_archives
        PASS [   0.076s] ( 800/5472) aion-awl-package::awl_source_round_trip a_document_without_imports_still_carries_its_own_source
        PASS [   0.096s] ( 801/5472) aion-awl-package::awl_assemble format_v1_loader_admits_the_archive_with_derived_manifest
        PASS [   0.088s] ( 802/5472) aion-awl-package::awl_source_round_trip a_sibling_schema_import_is_archived_beside_the_document
        PASS [   2.156s] ( 803/5472) aion-awl::runtime_codecs child_envelope::real_child_outcome_envelope_completes_parent_fold_on_both_backends
        PASS [   0.155s] ( 804/5472) aion-awl-package::awl_source_round_trip a_deployed_document_and_its_nested_schemas_survive_the_persistence_rewrite
        PASS [   0.215s] ( 805/5472) aion-awl-package::awl_assemble document_timeout_reaches_the_manifest_and_absence_arms_nothing
        PASS [   0.101s] ( 806/5472) aion-awl-package::declared_command_sealing an_assembled_archive_carries_the_declared_command_and_loads
        PASS [   0.006s] ( 807/5472) aion-awl-package::distribute_parity duplicate_workflow_types_are_refused_typed
        PASS [   0.161s] ( 808/5472) aion-awl-package::awl_source_round_trip assembly_with_archived_source_is_byte_deterministic
        PASS [   0.009s] ( 809/5472) aion-awl-package::distribute_parity unsafe_synthesized_workflow_type_is_refused_typed
        PASS [   1.134s] ( 810/5472) aion-awl::runtime_codecs stmt_parity::wait_signal_binds_payload_with_reference_parity
        PASS [   0.336s] ( 811/5472) aion-awl-lsp::corpus lsp_g1_rev2_corpus_diagnostics_conform
        PASS [   0.006s] ( 812/5472) aion-awl-package::sdk_closure_freshness a_changed_body_is_invisible_to_exports_and_caught_by_bytes
        PASS [   0.023s] ( 813/5472) aion-awl-package::runtime_surface_guard every_runtime_fn_signature_is_exported_by_the_closure
        PASS [   0.004s] ( 814/5472) aion-awl-package::sdk_closure_freshness a_changed_metadata_chunk_does_not_diverge
        PASS [   0.005s] ( 815/5472) aion-awl-package::sdk_closure_freshness byte_identical_beams_do_not_diverge
        PASS [   0.005s] ( 816/5472) aion-awl-package::sdk_closure_freshness gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.216s] ( 817/5472) aion-awl-package::awl_source_round_trip archiving_the_document_does_not_change_the_package_version
        PASS [   0.010s] ( 818/5472) aion-awl-package::sdk_closure_freshness gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.006s] ( 819/5472) aion-awl-package::sdk_closure_freshness gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.003s] ( 820/5472) aion-awl-package::sdk_closure_freshness gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.004s] ( 821/5472) aion-awl-package::sdk_closure_freshness gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.004s] ( 822/5472) aion-awl-package::sdk_closure_freshness gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.005s] ( 823/5472) aion-awl-package::sdk_closure_freshness gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.100s] ( 824/5472) aion-awl-package::distribute_parity direct_archive_carries_the_implicit_child_with_gleam_path_identity
        PASS [   0.004s] ( 825/5472) aion-awl-package::sdk_closure_freshness gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   0.097s] ( 826/5472) aion-awl-package::distribute_parity direct_distribute_archive_is_deterministic
        PASS [   0.020s] ( 827/5472) aion-awl-package::sdk_closure_freshness gleam_test_support::every_gleam_support_file_is_classified
        PASS [   1.462s] ( 828/5472) aion-awl::runtime_codecs semantic_pins::duration_boundary_executes_exactly_and_next_second_is_refused
        PASS [   0.094s] ( 829/5472) aion-awl-package::distribute_parity single_activity_distribute_synthesizes_no_child
        PASS [   0.033s] ( 830/5472) aion-cli::assistant_verb_e2e start_requires_the_operator_to_state_the_repository_question
        PASS [   0.294s] ( 831/5472) aion-awl-package::declared_command_sealing a_one_character_command_edit_is_enough_to_refuse
        PASS [   0.057s] ( 832/5472) aion-cli::authoring_quality_e2e determinism_gate_passes_clean_and_fails_on_injected_wall_clock
        PASS [   0.008s] ( 833/5472) aion-cli::authoring_quality_e2e gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.004s] ( 834/5472) aion-cli::authoring_quality_e2e gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.004s] ( 835/5472) aion-cli::authoring_quality_e2e gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.003s] ( 836/5472) aion-cli::authoring_quality_e2e gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.004s] ( 837/5472) aion-cli::authoring_quality_e2e gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.004s] ( 838/5472) aion-cli::authoring_quality_e2e gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.286s] ( 839/5472) aion-awl-package::declared_command_sealing repacking_without_editing_anything_still_loads
        PASS [   0.005s] ( 840/5472) aion-cli::authoring_quality_e2e gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.142s] ( 841/5472) aion-cli::assistant_verb_e2e an_empty_message_is_refused_before_any_connection
        PASS [   1.423s] ( 842/5472) aion-awl::runtime_codecs stmt_parity::child_call_statement_and_spawn_execute_with_reference_parity
        PASS [   0.142s] ( 843/5472) aion-cli::assistant_verb_e2e a_valid_message_against_a_dead_endpoint_reports_a_connection_failure
        PASS [   0.005s] ( 844/5472) aion-cli::authoring_quality_e2e gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   0.139s] ( 845/5472) aion-cli::assistant_verb_e2e document_prints_the_embedded_document_verbatim
        PASS [   0.022s] ( 846/5472) aion-cli::authoring_quality_e2e gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.327s] ( 847/5472) aion-awl-package::declared_command_sealing deleting_the_body_outright_makes_the_archive_refuse_to_load
        PASS [   0.015s] ( 848/5472) aion-cli::awl_corpus_gate the_gate_refuses_the_retired_three_field_agent_shape
        PASS [   0.026s] ( 849/5472) aion-cli::awl_corpus_gate the_derived_corpus_is_non_vacuous_and_covers_every_area
        PASS [   0.018s] ( 850/5472) aion-cli::awl_emit_beam emit_beam_binary_refuses_stdout
        PASS [   0.299s] ( 851/5472) aion-awl-package::declared_command_sealing rewriting_the_declared_command_makes_the_archive_refuse_to_load
        PASS [   0.037s] ( 852/5472) aion-cli::awl_recipe_verb_e2e a_fill_that_is_not_a_binding_is_refused
        PASS [   0.040s] ( 853/5472) aion-cli::awl_recipe_verb_e2e an_undeclared_recipe_is_named_with_what_the_document_does_declare
        PASS [   0.041s] ( 854/5472) aion-cli::awl_recipe_verb_e2e compose_refuses_an_unfilled_slot_by_name
        PASS [   0.045s] ( 855/5472) aion-cli::awl_recipe_verb_e2e fire_measures_derives_fires_runs_and_writes_a_receipt
        PASS [   0.014s] ( 856/5472) aion-cli::console_guide_asset the_console_asset_is_readable_json_with_the_shape_the_panel_uses
        PASS [   0.028s] ( 857/5472) aion-cli::console_guide_asset the_console_asset_is_what_the_cli_renders_today
        PASS [   0.054s] ( 858/5472) aion-cli::awl_recipe_verb_e2e fire_refuses_a_census_mismatch_measured_against_the_real_tree
        PASS [   0.044s] ( 859/5472) aion-cli::awl_recipe_verb_e2e fire_refuses_an_unanswered_precondition_and_still_prints_its_receipt
        PASS [   0.015s] ( 860/5472) aion-cli::deploy_commands gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.008s] ( 861/5472) aion-cli::deploy_commands gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.052s] ( 862/5472) aion-cli::declared_environment_report_e2e a_declaration_this_machine_can_satisfy_produces_no_warning_at_all
        PASS [   0.127s] ( 863/5472) aion-cli::awl_emit_beam emit_beam_resolves_schema_imports_from_the_document_directory
        PASS [   0.008s] ( 864/5472) aion-cli::deploy_commands gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.142s] ( 865/5472) aion-cli::awl_emit_beam emit_beam_binary_output_equals_the_archive_entry_module
        PASS [   0.012s] ( 866/5472) aion-cli::deploy_commands gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.009s] ( 867/5472) aion-cli::deploy_commands gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.007s] ( 868/5472) aion-cli::deploy_commands gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.008s] ( 869/5472) aion-cli::deploy_commands gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.054s] ( 870/5472) aion-cli::declared_environment_report_e2e an_absent_declared_variable_is_reported_on_stderr_where_an_operator_reads_it
        PASS [   0.010s] ( 871/5472) aion-cli::deploy_commands gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   0.281s] ( 872/5472) aion-cli::assistant_verb_e2e the_local_subcommands_never_need_a_server
        PASS [   0.009s] ( 873/5472) aion-cli::dev_hot_reload_e2e gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.011s] ( 874/5472) aion-cli::dev_hot_reload_e2e gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.005s] ( 875/5472) aion-cli::dev_hot_reload_e2e gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.007s] ( 876/5472) aion-cli::dev_hot_reload_e2e gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.006s] ( 877/5472) aion-cli::dev_hot_reload_e2e gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.007s] ( 878/5472) aion-cli::dev_hot_reload_e2e gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.005s] ( 879/5472) aion-cli::dev_hot_reload_e2e gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.041s] ( 880/5472) aion-cli::deploy_commands gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.005s] ( 881/5472) aion-cli::dev_hot_reload_e2e gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   0.336s] ( 882/5472) aion-cli::assistant_verb_e2e describe_answers_from_the_binary_with_no_server
        PASS [   0.004s] ( 883/5472) aion-cli::doc_pointer_gate the_gate_rejects_a_pointer_at_a_path_that_is_not_there
        PASS [   0.012s] ( 884/5472) aion-cli::doc_pointer_gate every_documentation_pointer_names_a_file_that_exists
        PASS [   0.008s] ( 885/5472) aion-cli::doc_pointer_gate the_extractor_reads_both_pointer_forms_and_skips_what_is_not_a_path
        PASS [   0.026s] ( 886/5472) aion-cli::dev_hot_reload_e2e gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.014s] ( 887/5472) aion-cli::doc_pointer_gate the_project_templates_still_point_somewhere
        PASS [   0.005s] ( 888/5472) aion-cli::embedded_assistant_launchable_e2e the_spike_carries_the_embedded_prompt_constants_verbatim
        PASS [   0.071s] ( 889/5472) aion-cli::describe_history_bound_e2e short_history_describe_is_unchanged
        PASS [   0.022s] ( 890/5472) aion-cli::error_rendering connect_failure_renders_unavailable_with_detail_and_hint
        PASS [   0.019s] ( 891/5472) aion-cli::error_rendering not_owner_renders_a_routing_hint_not_a_connectivity_one
        PASS [   0.024s] ( 892/5472) aion-cli::error_rendering signal_against_a_terminal_run_renders_workflow_terminal
        PASS [   0.084s] ( 893/5472) aion-cli::embedded_assistant_launchable_e2e the_embedded_document_is_launchable_by_the_recommended_verb
        PASS [   2.713s] ( 894/5472) aion-awl::runtime_codecs child_envelope::child_output_codec_is_strict_and_symmetric_on_both_backends
        PASS [   0.025s] ( 895/5472) aion-cli::error_rendering signal_not_found_renders_class_detail_error_type_and_hint
        PASS [   0.025s] ( 896/5472) aion-cli::error_rendering typed_error_type_from_query_outcome_reaches_stderr
        PASS [   0.110s] ( 897/5472) aion-cli::embedded_assistant_launchable_e2e the_recommended_serve_invocation_reaches_the_dial_not_a_refusal
        PASS [   0.025s] ( 898/5472) aion-cli::error_rendering typed_error_type_from_status_details_reaches_stderr
        PASS [   0.088s] ( 899/5472) aion-cli::error_rendering each_query_wire_code_renders_distinctly_end_to_end
        PASS [   0.005s] ( 900/5472) aion-cli::generate_e2e gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.011s] ( 901/5472) aion-cli::generate_e2e gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.005s] ( 902/5472) aion-cli::generate_e2e gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.009s] ( 903/5472) aion-cli::generate_e2e gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.005s] ( 904/5472) aion-cli::generate_e2e gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.008s] ( 905/5472) aion-cli::generate_e2e gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   2.754s] ( 906/5472) aion-awl::runtime_codecs fallible_predicates_execute_through_production_hosts_with_parity
        PASS [   0.008s] ( 907/5472) aion-cli::generate_e2e gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.006s] ( 908/5472) aion-cli::generate_e2e gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   0.005s] ( 909/5472) aion-cli::generate_types_first_e2e gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.006s] ( 910/5472) aion-cli::generate_types_first_e2e gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.014s] ( 911/5472) aion-cli::generate_types_first_e2e gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.026s] ( 912/5472) aion-cli::generate_e2e gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.006s] ( 913/5472) aion-cli::generate_types_first_e2e gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.146s] ( 914/5472) aion-cli::embedded_assistant_launchable_e2e the_same_document_without_its_harness_section_is_refused
        PASS [   0.007s] ( 915/5472) aion-cli::generate_types_first_e2e gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.005s] ( 916/5472) aion-cli::generate_types_first_e2e gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.004s] ( 917/5472) aion-cli::generate_types_first_e2e gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.005s] ( 918/5472) aion-cli::generate_types_first_e2e gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   0.005s] ( 919/5472) aion-cli::issue36_pause_restart_resume_e2e a_paused_run_resumed_across_a_server_death_adopts_its_live_builder
        PASS [   0.222s] ( 920/5472) aion-cli::describe_history_bound_e2e describe_answers_for_histories_of_arbitrary_size
        PASS [   0.006s] ( 921/5472) aion-cli::issue36_pause_restart_resume_e2e a_running_run_recovered_across_a_server_death_adopts_its_live_builder
        PASS [   0.020s] ( 922/5472) aion-cli::generate_types_first_e2e gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.107s] ( 923/5472) aion-cli::launcher_command a_foreign_listener_on_the_resolved_port_is_refused_by_name
        PASS [   0.619s] ( 924/5472) aion-cli::assistant_verb_e2e the_status_verb_round_trips_against_a_live_server
        PASS [   0.009s] ( 925/5472) aion-cli::lsub5b_osproc_failover_e2e osproc::harness_diagnostics::a_failure_preserves_the_work_dir_beyond_the_tempdir
        PASS [   0.005s] ( 926/5472) aion-cli::lsub5b_osproc_failover_e2e osproc::harness_diagnostics::an_unreadable_or_empty_log_says_so_instead_of_quoting_nothing
        PASS [   0.006s] ( 927/5472) aion-cli::lsub5b_osproc_failover_e2e osproc_liminal_listener_accepts_real_worker_connection
        PASS [   0.107s] ( 928/5472) aion-cli::lsub5b_osproc_failover_e2e osproc::harness_diagnostics::a_boot_death_reports_the_exit_status_the_bind_phase_and_the_log
        PASS [   0.008s] ( 929/5472) aion-cli::lsub5b_osproc_kill9_failover_e2e osproc::harness_diagnostics::a_failure_preserves_the_work_dir_beyond_the_tempdir
        PASS [   0.005s] ( 930/5472) aion-cli::lsub5b_osproc_kill9_failover_e2e osproc::harness_diagnostics::an_unreadable_or_empty_log_says_so_instead_of_quoting_nothing
        PASS [   0.006s] ( 931/5472) aion-cli::lsub5b_osproc_kill9_failover_e2e osproc_kill9_liminal_failover_completes_exactly_once
        PASS [   0.008s] ( 932/5472) aion-cli::new_agent_e2e gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.008s] ( 933/5472) aion-cli::new_agent_e2e gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.004s] ( 934/5472) aion-cli::new_agent_e2e gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.007s] ( 935/5472) aion-cli::new_agent_e2e gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.112s] ( 936/5472) aion-cli::lsub5b_osproc_kill9_failover_e2e osproc::harness_diagnostics::a_boot_death_reports_the_exit_status_the_bind_phase_and_the_log
        PASS [   0.004s] ( 937/5472) aion-cli::new_agent_e2e gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.009s] ( 938/5472) aion-cli::new_agent_e2e gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.010s] ( 939/5472) aion-cli::new_agent_e2e gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.005s] ( 940/5472) aion-cli::new_agent_e2e gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   0.024s] ( 941/5472) aion-cli::new_agent_e2e gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.658s] ( 942/5472) aion-cli::deploy_commands deploy_subcommands_drive_a_live_server
        PASS [   0.007s] ( 943/5472) aion-cli::new_hello_world_e2e gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.004s] ( 944/5472) aion-cli::new_hello_world_e2e gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.004s] ( 945/5472) aion-cli::new_hello_world_e2e gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.006s] ( 946/5472) aion-cli::new_hello_world_e2e gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.004s] ( 947/5472) aion-cli::new_hello_world_e2e gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.005s] ( 948/5472) aion-cli::new_hello_world_e2e gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.025s] ( 949/5472) aion-cli::new_hello_world_e2e gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.007s] ( 950/5472) aion-cli::new_hello_world_e2e gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.006s] ( 951/5472) aion-cli::new_hello_world_e2e gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   2.567s] ( 952/5472) aion-awl::runtime_codecs stmt_parity::wait_timeout_arms_execute_with_reference_parity
        PASS [   1.055s] ( 953/5472) aion-cli::generate_types_first_e2e stray_authored_schema_fails_with_the_migration_hint
        PASS [   1.057s] ( 954/5472) aion-cli::generate_types_first_e2e type_outside_subset_fails_loudly
        PASS [   0.010s] ( 955/5472) aion-cli::new_scaffold_build dev_pipeline_template_builds_packages_and_passes_its_tests
        PASS [   1.324s] ( 956/5472) aion-cli::launcher_command foreground_launch_serves_in_process_and_drains_on_sigterm
        PASS [   0.010s] ( 957/5472) aion-cli::new_scaffold_build gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.012s] ( 958/5472) aion-cli::new_scaffold_build gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.009s] ( 959/5472) aion-cli::new_scaffold_build gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.011s] ( 960/5472) aion-cli::new_scaffold_build gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.010s] ( 961/5472) aion-cli::new_scaffold_build gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.005s] ( 962/5472) aion-cli::new_scaffold_build gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.023s] ( 963/5472) aion-cli::new_scaffold_build gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.009s] ( 964/5472) aion-cli::new_scaffold_build gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.008s] ( 965/5472) aion-cli::new_scaffold_build gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   1.423s] ( 966/5472) aion-cli::launcher_command bare_launch_starts_a_background_server_and_repeats_idempotently
        PASS [   1.991s] ( 967/5472) aion-awl-package::sdk_closure_freshness committed_closure_matches_a_fresh_oracle_harvest
        PASS [   0.023s] ( 968/5472) aion-cli::new_scaffold_build new_refuses_dev_pipeline_without_a_worker
        PASS [   0.019s] ( 969/5472) aion-cli::new_scaffold_build new_refuses_non_empty_directory
        PASS [   0.018s] ( 970/5472) aion-cli::new_scaffold_build new_refuses_worker_for_templates_without_activities
        PASS [   0.014s] ( 971/5472) aion-cli::new_scaffold_build new_rejects_invalid_names_with_the_rule
        PASS [   2.671s] ( 972/5472) aion-cli::awl_corpus_gate every_living_awl_document_checks_clean
        PASS [   2.870s] ( 973/5472) aion-cli::new_awl_e2e default_awl_scaffold_deploys_and_runs_to_completion
        PASS [   0.020s] ( 974/5472) aion-cli::one_motion_awl invalid_awl_deploy_exits_nonzero_with_the_compiler_span_diagnostic
        PASS [   0.098s] ( 975/5472) aion-cli::one_motion_awl mismatching_run_input_prints_schema_without_attempting_deploy
        PASS [   0.115s] ( 976/5472) aion-cli::one_motion_live_e2e a_boot_failure_reports_the_servers_own_account_of_it
        PASS [   2.493s] ( 977/5472) aion-cli::new_scaffold_build approval_flow_template_builds_and_packages
        PASS [   4.111s] ( 978/5472) aion-cli::dev_hot_reload_e2e dev_hot_loads_a_new_content_hash_version_on_edit_without_restart
        PASS [   2.396s] ( 979/5472) aion-cli::new_scaffold_build saga_template_builds_and_packages
        PASS [   2.844s] ( 980/5472) aion-cli::new_scaffold_build hello_world_template_builds_and_packages
        PASS [   0.023s] ( 981/5472) aion-cli::one_motion_live_e2e the_run_timeout_flag_has_no_default
        PASS [   0.022s] ( 982/5472) aion-cli::package_command package_build_failure_is_surfaced_before_packaging
        PASS [   0.092s] ( 983/5472) aion-cli::package_command package_hello_world_succeeds_offline_with_result_document
        PASS [   0.135s] ( 984/5472) aion-cli::package_command package_out_override_with_dotdot_is_exempt_from_confinement
        PASS [   0.021s] ( 985/5472) aion-cli::package_command package_without_descriptor_fails_with_error_chain_and_no_dial
        PASS [   0.014s] ( 986/5472) aion-cli::parked_timer_osproc_kill9_failover_e2e gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.006s] ( 987/5472) aion-cli::parked_timer_osproc_kill9_failover_e2e gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.005s] ( 988/5472) aion-cli::parked_timer_osproc_kill9_failover_e2e gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.004s] ( 989/5472) aion-cli::parked_timer_osproc_kill9_failover_e2e gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.007s] ( 990/5472) aion-cli::parked_timer_osproc_kill9_failover_e2e gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.005s] ( 991/5472) aion-cli::parked_timer_osproc_kill9_failover_e2e gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   5.025s] ( 992/5472) aion-cli::authoring_quality_e2e generated_test_scaffold_compiles_under_gleam_build
        PASS [   0.013s] ( 993/5472) aion-cli::parked_timer_osproc_kill9_failover_e2e gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.023s] ( 994/5472) aion-cli::parked_timer_osproc_kill9_failover_e2e gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.007s] ( 995/5472) aion-cli::parked_timer_osproc_kill9_failover_e2e gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   0.009s] ( 996/5472) aion-cli::parked_timer_osproc_kill9_failover_e2e osproc_kill9_parked_timer_resumes_and_completes_exactly_once
        PASS [   0.785s] ( 997/5472) aion-cli::one_motion_live_e2e one_motion_run_compiles_deploys_starts_awaits_and_reports_the_result
        PASS [   0.006s] ( 998/5472) aion-cli::retired_doctrine_gate the_matcher_finds_each_banned_phrase_through_markdown_disguises
        PASS [   0.058s] ( 999/5472) aion-cli::retired_doctrine_gate the_derived_scope_is_non_vacuous_and_covers_the_documents_that_slipped
        PASS [   0.020s] (1000/5472) aion-cli::server_command an_explicit_missing_config_refuses_and_scaffolds_nothing
        PASS [   0.110s] (1001/5472) aion-cli::retired_doctrine_gate no_living_markdown_teaches_the_retired_doctrine
        PASS [   0.005s] (1002/5472) aion-cli::stacked_dev_live_e2e stacked_dev_lands_through_the_real_worker_and_review_signal
        PASS [   0.144s] (1003/5472) aion-cli::tail_follow_e2e tail_follow_attributes_replay_only_after_the_selected_generation_starts
        PASS [   1.024s] (1004/5472) aion-cli::one_motion_live_e2e one_motion_run_reports_a_failure_outcome_with_its_details_and_exits_nonzero
        PASS [   0.149s] (1005/5472) aion-cli::tail_follow_e2e tail_no_follow_does_not_append_json_null_to_its_transcript_output
        PASS [   0.135s] (1006/5472) aion-cli::bin/aion assistant::assistant_tests::a_blank_objective_is_refused
        PASS [   5.504s] (1007/5472) aion-cli::authoring_quality_e2e input_skeleton_round_trips_through_the_workflow_input_codec
        PASS [   0.120s] (1008/5472) aion-cli::bin/aion assistant::assistant_tests::a_continuation_missing_the_end_field_is_refused
        PASS [   4.723s] (1009/5472) aion-cli::new_hello_world_e2e scaffolded_hello_world_completes_against_the_generated_config
        PASS [   0.136s] (1010/5472) aion-cli::bin/aion assistant::assistant_tests::a_malformed_workflow_id_is_refused_at_composition
        PASS [   0.514s] (1011/5472) aion-cli::server_command server_subcommand_boots_serves_and_drains_cleanly
        PASS [   0.124s] (1012/5472) aion-cli::bin/aion assistant::assistant_tests::a_start_input_missing_the_objective_is_refused_by_the_schema
        PASS [   0.138s] (1013/5472) aion-cli::bin/aion assistant::assistant_tests::an_empty_message_is_refused_at_composition
        PASS [   4.316s] (1014/5472) aion-cli::new_scaffold_build agent_scaffold_generate_round_trips_byte_identically
        PASS [   0.166s] (1015/5472) aion-cli::bin/aion assistant::assistant_tests::both_continuation_payloads_satisfy_the_signal_contract
        PASS [   0.132s] (1016/5472) aion-cli::bin/aion assistant::assistant_tests::describe_reports_the_embedded_identity_and_contract
        PASS [   0.142s] (1017/5472) aion-cli::bin/aion assistant::assistant_tests::document_writes_the_embedded_bytes
        PASS [   0.032s] (1018/5472) aion-cli::bin/aion awl::tests::check_source_counts_actions_on_a_clean_worker_document
        PASS [   0.033s] (1019/5472) aion-cli::bin/aion awl::tests::check_source_accepts_the_flow_vocabulary
        PASS [   0.133s] (1020/5472) aion-cli::bin/aion assistant::assistant_tests::no_repo_composes_the_documents_scratch_mode
        PASS [   0.010s] (1021/5472) aion-cli::bin/aion awl::tests::check_source_renders_typecheck_errors_as_diagnostics
        PASS [   0.011s] (1022/5472) aion-cli::bin/aion awl::tests::check_source_renders_a_parse_error_as_a_diagnostic
        PASS [   0.010s] (1023/5472) aion-cli::bin/aion awl::tests::diagnostic_renders_the_compiler_style_line
        PASS [   0.031s] (1024/5472) aion-cli::bin/aion awl::tests::check_source_counts_steps_on_a_clean_document
        PASS [   0.011s] (1025/5472) aion-cli::bin/aion awl::tests::emit_beam_refuses_without_output
        PASS [   5.141s] (1026/5472) aion-cli::new_agent_e2e scaffolded_agent_deploys_but_start_is_refused_unscoped
        PASS [   0.131s] (1027/5472) aion-cli::bin/aion assistant::assistant_tests::the_status_verb_dispatches_a_query_the_document_declares
        PASS [   0.045s] (1028/5472) aion-cli::bin/aion awl::tests::emit_beam_writes_a_module_and_a_beam_shaped_sidecar
        PASS [   0.039s] (1029/5472) aion-cli::bin/aion awl::tests::emit_gleam_target_reproduces_the_committed_golden
        PASS [   0.158s] (1030/5472) aion-cli::bin/aion assistant::assistant_tests::the_composed_start_input_satisfies_the_document_contract
        PASS [   0.007s] (1031/5472) aion-cli::bin/aion awl::tests::emit_source_is_gated_on_a_clean_typecheck
        PASS [   0.052s] (1032/5472) aion-cli::bin/aion awl::tests::emit_gleam_synthesized_children_reproduce_the_committed_goldens
        PASS [   0.007s] (1033/5472) aion-cli::bin/aion awl::tests::emit_source_renders_a_parse_error_as_a_diagnostic
        PASS [   0.007s] (1034/5472) aion-cli::bin/aion awl::tests::format_source_is_the_canonical_printer
        PASS [   0.006s] (1035/5472) aion-cli::bin/aion awl::tests::format_source_reports_a_parse_error_without_output
        PASS [   0.022s] (1036/5472) aion-cli::bin/aion awl::tests::format_source_is_idempotent_on_the_flow_vocabulary
        PASS [   0.035s] (1037/5472) aion-cli::bin/aion awl::tests::emit_source_folds_the_flow_vocabulary
        PASS [   0.020s] (1038/5472) aion-cli::bin/aion awl::tests::schema_source_is_gated_on_a_clean_typecheck
        PASS [   0.059s] (1039/5472) aion-cli::bin/aion awl::tests::emit_output_writes_packaging_sidecar_for_implicit_children
        PASS [   0.093s] (1040/5472) aion-cli::bin/aion awl::tests::emit_beam_bytes_equal_the_archive_entry_module
        PASS [   0.014s] (1041/5472) aion-cli::bin/aion awl_guide::awl_guide_tests::a_positional_word_is_labelled_as_positional
        PASS [   0.034s] (1042/5472) aion-cli::bin/aion awl::tests::schema_source_derives_a_declared_type
        PASS [   0.013s] (1043/5472) aion-cli::bin/aion awl_guide::awl_guide_tests::each_reference_section_is_printed_once_in_order
        PASS [   0.011s] (1044/5472) aion-cli::bin/aion awl_guide::awl_guide_tests::one_word_prints_its_sentence_examples_and_citation
        PASS [   0.022s] (1045/5472) aion-cli::bin/aion awl_guide::awl_guide_tests::a_word_the_language_does_not_have_resolves_to_nothing
        PASS [   0.006s] (1046/5472) aion-cli::bin/aion awl_guide::awl_guide_tests::the_embedded_reference_is_the_whole_document
        PASS [   0.025s] (1047/5472) aion-cli::bin/aion awl_guide::awl_guide_tests::a_single_form_word_prints_one_unnumbered_example
        PASS [   0.021s] (1048/5472) aion-cli::bin/aion awl_guide::awl_guide_tests::one_word_as_json_carries_only_that_word
        PASS [   0.066s] (1049/5472) aion-cli::bin/aion awl::tests::emit_source_generates_gleam_for_a_clean_document
        PASS [   0.012s] (1050/5472) aion-cli::bin/aion awl_guide::awl_guide_tests::the_text_form_names_the_documents_an_author_needs
        PASS [   0.014s] (1051/5472) aion-cli::bin/aion awl_recipe::firer::tests::a_named_cause_is_read_out_of_the_shapes_this_estate_prints
        PASS [   0.020s] (1052/5472) aion-cli::bin/aion awl_guide::awl_guide_tests::the_text_form_prints_every_word_of_the_language
        PASS [   0.028s] (1053/5472) aion-cli::bin/aion awl_guide::awl_guide_tests::the_json_form_carries_the_grammar_axis_and_the_reference
        PASS [   0.014s] (1054/5472) aion-cli::bin/aion awl_recipe::firer::tests::an_output_naming_no_cause_is_read_as_nothing
        PASS [   0.010s] (1055/5472) aion-cli::bin/aion awl_recipe::firer::tests::the_line_the_cause_stood_on_is_the_bytes_observed
        PASS [   0.066s] (1056/5472) aion-cli::bin/aion awl::tests::schema_source_without_type_emits_the_start_contract
        PASS [   0.011s] (1057/5472) aion-cli::bin/aion awl_scaffold::tests::a_link_within_one_tree_is_relative
        PASS [   0.015s] (1058/5472) aion-cli::bin/aion awl_scaffold::tests::a_link_across_unrelated_trees_is_absolute
        PASS [   0.018s] (1059/5472) aion-cli::bin/aion awl_scaffold::tests::a_document_that_does_not_compile_writes_nothing
        PASS [   0.050s] (1060/5472) aion-cli::bin/aion awl_guide::awl_guide_tests::the_json_form_carries_the_whole_inventory_and_the_paths
        PASS [   0.049s] (1061/5472) aion-cli::bin/aion awl_guide::awl_guide_tests::the_json_form_is_byte_stable
        PASS [   0.056s] (1062/5472) aion-cli::bin/aion awl_scaffold::tests::a_bogus_sdk_path_is_refused
        PASS [   0.048s] (1063/5472) aion-cli::bin/aion awl_scaffold::tests::a_multi_node_document_yields_one_connection_per_node
        PASS [   0.043s] (1064/5472) aion-cli::bin/aion awl_scaffold::tests::an_unknown_worker_name_is_refused
        PASS [   0.008s] (1065/5472) aion-cli::bin/aion check_deterministic::tests::negative_fixture_clean_workflow_passes
        PASS [   0.008s] (1066/5472) aion-cli::bin/aion check_deterministic::tests::positive_fixture_tainted_workflow_fails_non_zero
        PASS [   0.045s] (1067/5472) aion-cli::bin/aion awl_scaffold::tests::every_registration_carries_a_descriptor
        PASS [   0.043s] (1068/5472) aion-cli::bin/aion awl_scaffold::tests::the_server_executed_action_reaches_no_generated_file
        PASS [   0.006s] (1069/5472) aion-cli::bin/aion console::tests::probe_classification_reads_the_status_line
        PASS [   0.008s] (1070/5472) aion-cli::bin/aion check_deterministic::tests::without_the_flag_the_command_explains_itself
        PASS [   0.048s] (1071/5472) aion-cli::bin/aion awl_scaffold::tests::the_generated_crate_reaches_the_document_it_was_generated_from
        PASS [   0.006s] (1072/5472) aion-cli::bin/aion deploy::tests::token_resolution_prefers_the_flag
        PASS [   0.008s] (1073/5472) aion-cli::bin/aion console::tests::served_url_rewrites_wildcard_binds_to_loopback
        PASS [   0.007s] (1074/5472) aion-cli::bin/aion dev::args::tests::debounce_parses_positive_milliseconds
        PASS [   0.007s] (1075/5472) aion-cli::bin/aion dev::args::tests::debounce_rejects_zero_and_non_numeric
        PASS [   0.009s] (1076/5472) aion-cli::bin/aion dev::pipeline::tests::reload_outcome_reads_the_deploy_response_document
        PASS [   0.007s] (1077/5472) aion-cli::bin/aion dev::pipeline::tests::reload_outcome_rejects_a_response_missing_a_field
        PASS [   0.007s] (1078/5472) aion-cli::bin/aion dev::watch::tests::relevant_only_for_source_content_changes
        PASS [   0.007s] (1079/5472) aion-cli::bin/aion dev::watch::tests::gleam_sources_are_recognised_case_insensitively
        PASS [   0.067s] (1080/5472) aion-cli::bin/aion awl_scaffold::tests::a_second_run_rewrites_the_generated_files_and_keeps_the_handlers
        PASS [   0.008s] (1081/5472) aion-cli::bin/aion dev::watch::tests::source_root_prefers_src_then_falls_back_to_root
        PASS [   0.008s] (1082/5472) aion-cli::bin/aion dev::watch::tests::watcher_errors_force_a_rebuild
        PASS [   0.007s] (1083/5472) aion-cli::bin/aion generate::tests::comment_lines_are_never_treated_as_imports
        PASS [   0.009s] (1084/5472) aion-cli::bin/aion generate::tests::aliased_and_unqualified_imports_yield_bare_name
        PASS [   0.007s] (1085/5472) aion-cli::bin/aion generate::tests::cyclic_imports_do_not_loop
        PASS [   0.008s] (1086/5472) aion-cli::bin/aion generate::tests::crash_leftovers_are_recovered
        PASS [   0.008s] (1087/5472) aion-cli::bin/aion generate::tests::extraction_lock_is_exclusive_then_reacquirable
        PASS [   0.007s] (1088/5472) aion-cli::bin/aion generate::tests::imports_are_parsed_to_bare_module_names
        PASS [   0.008s] (1089/5472) aion-cli::bin/aion generate::tests::marked_json_is_extracted_from_noisy_stdout
        PASS [   0.007s] (1090/5472) aion-cli::bin/aion generate::tests::missing_marker_is_an_error
        PASS [   0.011s] (1091/5472) aion-cli::bin/aion generate::tests::indented_real_import_is_found
        PASS [   0.008s] (1092/5472) aion-cli::bin/aion generate::tests::modules_reaching_sets_aside_targets_and_their_importers
        PASS [   0.008s] (1093/5472) aion-cli::bin/aion generate::tests::sync_clean_match_check_is_checked
        PASS [   0.009s] (1094/5472) aion-cli::bin/aion generate::tests::module_names_use_forward_slashes_under_src
        PASS [   0.007s] (1095/5472) aion-cli::bin/aion generate::tests::sync_clean_match_write_is_unchanged_and_preserves_bytes
        PASS [   0.007s] (1096/5472) aion-cli::bin/aion generate::tests::sync_no_workflow_table_bails
        PASS [   0.007s] (1097/5472) aion-cli::bin/aion generate::tests::sync_stale_list_check_bails_with_path_and_reason
        PASS [   0.009s] (1098/5472) aion-cli::bin/aion generate::tests::sync_multiple_workflow_tables_bails_naming_the_count
        PASS [   0.009s] (1099/5472) aion-cli::bin/aion generate::tests::sync_missing_activities_key_is_created_on_write
        PASS [   0.007s] (1100/5472) aion-cli::bin/aion generate::tests::transitive_importers_of_the_target_are_found
        PASS [   0.008s] (1101/5472) aion-cli::bin/aion generate::tests::sync_stale_list_write_rewrites_in_declared_order
        PASS [   0.007s] (1102/5472) aion-cli::bin/aion generate::tests::trailing_comment_after_import_is_ignored_but_module_kept
        PASS [   0.007s] (1103/5472) aion-cli::bin/aion harness::tests::a_declared_kind_resolves_against_the_adapters_this_build_links
        PASS [   0.008s] (1104/5472) aion-cli::bin/aion harness::tests::a_build_with_no_harness_advertises_the_empty_set
        PASS [   0.009s] (1105/5472) aion-cli::bin/aion harness::tests::a_declared_acp_launch_reaches_the_adapter_verbatim
        PASS [   0.011s] (1106/5472) aion-cli::bin/aion harness::tests::a_bound_cwd_resolves_to_the_parameter_the_job_will_carry
        PASS [   0.011s] (1107/5472) aion-cli::bin/aion harness::tests::a_cwd_that_cannot_be_absolute_is_refused_at_the_composition_root
        PASS [   0.012s] (1108/5472) aion-cli::bin/aion harness::tests::a_workspace_rooted_cwd_is_expanded_against_this_boxs_own_root
        PASS [   0.008s] (1109/5472) aion-cli::bin/aion harness::tests::an_env_pass_entry_carrying_a_value_is_refused_at_the_launch_boundary
        PASS [   0.007s] (1110/5472) aion-cli::bin/aion harness::tests::each_harness_advertises_its_adapters_published_capabilities
        PASS [   0.017s] (1111/5472) aion-cli::bin/aion harness::tests::a_workspace_rooted_cwd_on_a_box_with_no_root_refuses_before_anything_spawns
        PASS [   0.008s] (1112/5472) aion-cli::bin/aion harness::tests::each_permission_spelling_resolves_to_the_adapters_policy
        PASS [   0.008s] (1113/5472) aion-cli::bin/aion harness::tests::every_compiled_harness_composes
        PASS [   0.007s] (1114/5472) aion-cli::bin/aion harness::tests::the_declared_argv_reaches_the_child_expanded_and_in_order
        PASS [   0.015s] (1115/5472) aion-cli::bin/aion harness::tests::an_uncompiled_harness_name_is_refused_naming_what_is_compiled_in
        PASS [   0.009s] (1116/5472) aion-cli::bin/aion harness::tests::the_declared_env_pass_list_is_the_childs_whole_environment
        PASS [   0.009s] (1117/5472) aion-cli::bin/aion harness::tests::the_norn_binary_is_optional_and_the_absence_takes_the_adapters_own_default
        PASS [   0.012s] (1118/5472) aion-cli::bin/aion harness::tests::the_document_vocabulary_and_the_catalogue_vocabulary_are_one_table
        PASS [   0.008s] (1119/5472) aion-cli::bin/aion input::tests::unknown_workflow_type_fails_naming_the_available_types
        PASS [   0.020s] (1120/5472) aion-cli::bin/aion harness::tests::the_reported_launch_directory_is_the_one_the_child_will_stand_in
        PASS [   0.011s] (1121/5472) aion-cli::bin/aion inspect::tests::parse_mock_rejects_a_missing_or_unknown_kind
        PASS [   0.011s] (1122/5472) aion-cli::bin/aion inspect::tests::parse_mock_requires_explicit_failure_message
        PASS [   0.019s] (1123/5472) aion-cli::bin/aion input::tests::emits_type_derived_skeleton_for_the_named_workflow
        PASS [   0.011s] (1124/5472) aion-cli::bin/aion inspect::tests::parse_mock_requires_the_result_or_payload_field
        PASS [   0.018s] (1125/5472) aion-cli::bin/aion inspect::tests::inspection_json_renders_random_as_a_draw_ordinal_projection_not_per_step
        PASS [   0.020s] (1126/5472) aion-cli::bin/aion inspect::tests::parse_mock_reads_every_explicit_kind
        PASS [   0.007s] (1127/5472) aion-cli::bin/aion launch::tests::provisioning_tightens_a_permissive_home_and_refuses_a_symlink
        PASS [   0.007s] (1128/5472) aion-cli::bin/aion launch::tests::the_config_line_claims_created_only_for_the_template_bytes
        PASS [   0.010s] (1129/5472) aion-cli::bin/aion inspect::tests::what_if_to_json_terminal_and_diverged_carry_from_seq_once
        PASS [   4.266s] (1130/5472) aion-cli::new_scaffold_build hello_world_scaffold_generate_round_trips_byte_identically
        PASS [   0.014s] (1131/5472) aion-cli::bin/aion launch::tests::the_log_tail_is_bounded_and_reports_unreadable_logs
        PASS [   0.014s] (1132/5472) aion-cli::bin/aion launch::tests::the_not_aion_refusal_names_port_and_answer
        PASS [   0.010s] (1133/5472) aion-cli::bin/aion launch::tests::the_ready_report_names_every_location_per_backend
        PASS [   0.011s] (1134/5472) aion-cli::bin/aion new::scaffold::tests::agent_scaffold_warns_it_will_be_refused_at_start
        PASS [   0.011s] (1135/5472) aion-cli::bin/aion new::scaffold::tests::accepts_an_existing_empty_directory
        PASS [   0.018s] (1136/5472) aion-cli::bin/aion inspect::tests::what_if_to_json_emits_from_seq_exactly_once
        PASS [   0.013s] (1137/5472) aion-cli::bin/aion new::scaffold::tests::awl_name_validation_rejects_reserved_words_with_the_rule
        PASS [   0.008s] (1138/5472) aion-cli::bin/aion new::scaffold::tests::name_validation_accepts_snake_case
        PASS [   0.007s] (1139/5472) aion-cli::bin/aion new::scaffold::tests::readme_instructions_match_real_subcommands
        PASS [   0.009s] (1140/5472) aion-cli::bin/aion new::scaffold::tests::name_validation_rejects_invalid_names_with_the_rule
        PASS [   0.015s] (1141/5472) aion-cli::bin/aion new::scaffold::tests::bare_new_parses_to_the_awl_template
        PASS [   0.010s] (1142/5472) aion-cli::bin/aion new::scaffold::tests::refuses_non_empty_target_directory
        PASS [   0.014s] (1143/5472) aion-cli::bin/aion new::scaffold::tests::every_template_renders_without_leftover_placeholders
        PASS [   0.007s] (1144/5472) aion-cli::bin/aion new::scaffold::tests::refuses_worker_for_templates_without_activities
        PASS [   0.008s] (1145/5472) aion-cli::bin/aion new::scaffold::tests::render_substitutes_name_and_rejects_leftover_placeholders
        PASS [   0.014s] (1146/5472) aion-cli::bin/aion new::scaffold::tests::refuses_dev_pipeline_without_a_worker_and_writes_nothing
        PASS [   0.007s] (1147/5472) aion-cli::bin/aion new::template::tests::agent_template_declares_its_three_steps_and_a_review_deadline_input
        PASS [   0.008s] (1148/5472) aion-cli::bin/aion new::template::tests::dev_pipeline_declares_three_workflow_entries_and_its_gates
        PASS [   0.009s] (1149/5472) aion-cli::bin/aion new::template::tests::aion_toml_carries_every_required_server_key
        PASS [   0.012s] (1150/5472) aion-cli::bin/aion new::scaffold::tests::scaffold_writes_every_manifest_file
        PASS [   0.010s] (1151/5472) aion-cli::bin/aion new::template::tests::agent_template_requires_a_worker
        PASS [   0.014s] (1152/5472) aion-cli::bin/aion new::scaffold::tests::scaffold_dev_pipeline_ships_the_types_module_and_generated_artifacts
        PASS [   0.011s] (1153/5472) aion-cli::bin/aion new::template::tests::every_template_manifest_is_complete
        PASS [   0.007s] (1154/5472) aion-cli::bin/aion new::template::tests::types_first_templates_ship_marked_generated_artifacts
        PASS [   0.006s] (1155/5472) aion-cli::bin/aion new::template::tests::worker_free_templates_do_not_require_a_worker
        PASS [   0.007s] (1156/5472) aion-cli::bin/aion new::template::tests::worker_manifests_exist_exactly_for_templates_with_activities
        PASS [   0.007s] (1157/5472) aion-cli::bin/aion new::template::tests::workflow_descriptors_declare_the_template_activities
        PASS [   0.009s] (1158/5472) aion-cli::bin/aion one_motion::tests::await_timeout_refuses_anything_but_one_whole_duration_literal
        PASS [   0.011s] (1159/5472) aion-cli::bin/aion one_motion::tests::await_timeout_accepts_every_awl_duration_unit
        PASS [   0.009s] (1160/5472) aion-cli::bin/aion output::tests::describe_output_decodes_terminal_payload_and_carries_identity
        PASS [   0.009s] (1161/5472) aion-cli::bin/aion output::tests::describe_output_does_not_decode_summary_payload_shapes
        PASS [   0.010s] (1162/5472) aion-cli::bin/aion output::tests::empty_payload_uses_clear_empty_indicator
        PASS [   0.009s] (1163/5472) aion-cli::bin/aion output::tests::invalid_payload_shape_is_left_unchanged
        PASS [   0.010s] (1164/5472) aion-cli::bin/aion output::tests::invalid_utf8_json_payload_falls_back_to_hex
        PASS [   0.006s] (1165/5472) aion-cli::bin/aion output::tests::non_json_payload_falls_back_to_hex_with_content_type
        PASS [   0.007s] (1166/5472) aion-cli::bin/aion output::tests::raw_describe_output_preserves_terminal_payload_bytes
        PASS [   0.007s] (1167/5472) aion-cli::bin/aion package::tests::a_scoped_contract_is_silent
        PASS [   0.016s] (1168/5472) aion-cli::bin/aion output::tests::malformed_json_payload_falls_back_to_hex
        PASS [   0.010s] (1169/5472) aion-cli::bin/aion package::tests::an_unscoped_contract_names_every_activity_in_sorted_order
        PASS [   0.043s] (1170/5472) aion-cli::bin/aion new::template::tests::embedded_awl_is_clean_and_canonical_after_name_substitution
        PASS [   0.010s] (1171/5472) aion-cli::bin/aion payload::tests::every_accepted_status_parses_and_is_named_in_the_refusal
        PASS [   0.006s] (1172/5472) aion-cli::bin/aion payload::tests::parse_status_accepts_documented_and_serde_spellings
        PASS [   0.013s] (1173/5472) aion-cli::bin/aion payload::tests::parse_run_id_rejects_invalid_uuid
        PASS [   0.009s] (1174/5472) aion-cli::bin/aion payload::tests::parse_workflow_id_rejects_invalid_uuid
        PASS [   0.013s] (1175/5472) aion-cli::bin/aion payload::tests::json_payload_round_trips_to_json_value
        PASS [   0.010s] (1176/5472) aion-cli::bin/aion render::tests::a_shutting_down_not_running_is_not_given_the_generic_hint
        PASS [   0.007s] (1177/5472) aion-cli::bin/aion render::tests::each_query_wire_code_renders_a_distinct_class_detail_and_hint
        PASS [   0.007s] (1178/5472) aion-cli::bin/aion render::tests::invalid_input_and_backend_render_without_a_hint
        PASS [   0.009s] (1179/5472) aion-cli::bin/aion render::tests::namespace_denied_and_not_found_render_their_classes
        PASS [   0.005s] (1180/5472) aion-cli::bin/aion render::tests::structured_error_type_gets_its_own_line
        PASS [   0.014s] (1181/5472) aion-cli::bin/aion render::tests::nested_context_layers_all_appear_before_the_detail
        PASS [   0.019s] (1182/5472) aion-cli::bin/aion render::tests::empty_server_detail_is_stated_not_hidden
        PASS [   0.018s] (1183/5472) aion-cli::bin/aion render::tests::non_client_errors_render_the_full_anyhow_chain
        PASS [   0.015s] (1184/5472) aion-cli::bin/aion render::tests::unavailable_renders_transport_chain_and_endpoint_hint
        PASS [   0.012s] (1185/5472) aion-cli::bin/aion tail::live::tests::current_attempts_uses_the_highest_attempt_per_activity
        PASS [   0.070s] (1186/5472) aion-cli::bin/aion one_motion::tests::mismatching_input_is_refused_before_an_archive_can_be_deployed
        PASS [   0.007s] (1187/5472) aion-cli::bin/aion tail::tests::backfill_notice_does_not_treat_head_sequence_as_an_event_count
        PASS [   0.010s] (1188/5472) aion-cli::bin/aion tail::tests::a_pure_delta_stream_renders_byte_for_byte_as_the_old_format_did
        PASS [   0.017s] (1189/5472) aion-cli::bin/aion tail::tests::a_mixed_old_then_delta_stream_renders_byte_for_byte_as_the_old_format_did
        PASS [   0.012s] (1190/5472) aion-cli::bin/aion tail::tests::child_label_falls_back_for_absent_or_empty_display_names
        PASS [   0.015s] (1191/5472) aion-cli::bin/aion tail::tests::a_window_opening_after_a_base_reports_it_and_invents_nothing
        PASS [   0.006s] (1192/5472) aion-cli::bin/aion tail::tests::old_norn_response_turn_render_is_pinned_before_format_change
        PASS [   0.007s] (1193/5472) aion-cli::bin/aion tail::tests::empty_retained_backfill_reports_its_nonzero_durable_head
        PASS [   0.007s] (1194/5472) aion-cli::bin/aion tail::tests::retained_events_interleave_by_emitted_timestamp
        PASS [   1.118s] (1195/5472) aion-cli::server_command a_bare_boot_scaffolds_the_home_config_and_serves_deploy_and_outbox
        PASS [   0.011s] (1196/5472) aion-cli::bin/aion tests::a_bare_invocation_refuses_loudly_naming_the_new_shape
        PASS [   0.011s] (1197/5472) aion-cli::bin/aion tests::a_deleted_flag_on_an_unrelated_verb_earns_no_advice
        PASS [   0.011s] (1198/5472) aion-cli::bin/aion tail::tests::the_old_format_two_turn_stream_is_the_control_rendering
        PASS [   0.016s] (1199/5472) aion-cli::bin/aion tail::tests::prefix_keeps_attempt_and_supersession_visible
        PASS [   0.012s] (1200/5472) aion-cli::bin/aion tail::tests::the_instructions_block_stops_repeating_and_the_turn_costs_strictly_less
        PASS [   0.090s] (1201/5472) aion-cli::bin/aion one_motion::tests::document_timeout_reaches_the_one_motion_archive_manifest
        PASS [   0.009s] (1202/5472) aion-cli::bin/aion tests::an_argv_with_no_deleted_flag_earns_no_migration_advice
        PASS [   0.014s] (1203/5472) aion-cli::bin/aion tests::a_server_run_without_endpoint_is_not_refused_and_clients_keep_the_default
        PASS [   0.009s] (1204/5472) aion-cli::bin/aion tests::awl_emit_parses_the_file_path_and_optional_output
        PASS [   0.008s] (1205/5472) aion-cli::bin/aion tests::awl_emit_requires_a_file
        PASS [   0.011s] (1206/5472) aion-cli::bin/aion tests::awl_check_and_fmt_parse_the_file_path
        PASS [   0.012s] (1207/5472) aion-cli::bin/aion tests::a_worker_awl_invocation_earns_no_migration_advice
        PASS [   0.012s] (1208/5472) aion-cli::bin/aion tests::awl_commands_require_a_file_and_fmt_has_no_check_mode
        PASS [   0.015s] (1209/5472) aion-cli::bin/aion tests::bare_invocation_parses_as_the_launcher
        PASS [   0.007s] (1210/5472) aion-cli::bin/aion tests::deploy_help_documents_the_operator_surface
        PASS [   0.008s] (1211/5472) aion-cli::bin/aion tests::check_help_documents_the_determinism_gate
        PASS [   0.008s] (1212/5472) aion-cli::bin/aion tests::describe_accepts_optional_run_id
        PASS [   0.011s] (1213/5472) aion-cli::bin/aion tests::check_parses_path_and_requires_the_deterministic_flag
        PASS [   0.010s] (1214/5472) aion-cli::bin/aion tests::describe_accepts_raw_payload_flag
        PASS [   0.012s] (1215/5472) aion-cli::bin/aion tests::deploy_subcommands_parse_with_global_token
        PASS [   0.010s] (1216/5472) aion-cli::bin/aion tests::describe_help_documents_raw_payload_flag
        PASS [   0.009s] (1217/5472) aion-cli::bin/aion tests::dev_requires_gleam_path_and_rejects_zero_debounce
        PASS [   0.010s] (1218/5472) aion-cli::bin/aion tests::dev_parses_path_gleam_path_and_optional_debounce
        PASS [   0.012s] (1219/5472) aion-cli::bin/aion tests::dev_help_documents_the_instant_loop_and_pinning
        PASS [   0.007s] (1220/5472) aion-cli::bin/aion tests::inspect_help_documents_oplog_read_and_what_if
        PASS [   0.012s] (1221/5472) aion-cli::bin/aion tests::input_parses_workflow_type_and_path
        PASS [   0.015s] (1222/5472) aion-cli::bin/aion tests::every_dial_address_the_cli_hands_out_carries_a_scheme
        PASS [   0.009s] (1223/5472) aion-cli::bin/aion tests::new_parses_name_template_and_worker
        PASS [   0.010s] (1224/5472) aion-cli::bin/aion tests::new_help_documents_templates_and_refusal
        PASS [   0.016s] (1225/5472) aion-cli::bin/aion tests::inspect_parses_workflow_run_from_and_mock
        PASS [   0.010s] (1226/5472) aion-cli::bin/aion tests::normalize_endpoint_defaults_to_http_scheme
        PASS [   0.128s] (1227/5472) aion-cli::bin/aion one_motion::tests::known_good_corpus_fixture_compiles_and_assembles_without_a_server
        PASS [   0.013s] (1228/5472) aion-cli::bin/aion tests::launcher_flags_with_a_subcommand_are_refused_by_name
        PASS [   0.008s] (1229/5472) aion-cli::bin/aion tests::normalize_endpoint_preserves_explicit_scheme
        PASS [   0.026s] (1230/5472) aion-cli::bin/aion tests::every_migrated_launch_flag_is_refused_by_the_parser
        PASS [   0.009s] (1231/5472) aion-cli::bin/aion tests::package_defaults_to_current_directory_without_build_or_out
        PASS [   0.010s] (1232/5472) aion-cli::bin/aion tests::package_help_documents_local_only_behaviour
        PASS [   0.014s] (1233/5472) aion-cli::bin/aion tests::package_accepts_path_out_and_build
        PASS [   0.009s] (1234/5472) aion-cli::bin/aion tests::reopen_help_documents_the_subcommand
        PASS [   0.008s] (1235/5472) aion-cli::bin/aion tests::server_help_documents_the_server_surface
        PASS [   0.012s] (1236/5472) aion-cli::bin/aion tests::query_arguments_default_to_the_null_document_and_accept_a_json_flag
        PASS [   0.008s] (1237/5472) aion-cli::bin/aion tests::server_subcommand_converts_all_overrides
        PASS [   0.011s] (1238/5472) aion-cli::bin/aion tests::reopen_parses_workflow_id_and_optional_run_id
        PASS [   0.007s] (1239/5472) aion-cli::bin/aion tests::start_malformed_inline_input_json_error_names_the_flag
        PASS [   0.006s] (1240/5472) aion-cli::bin/aion tests::start_missing_input_file_error_names_the_path
        PASS [   0.009s] (1241/5472) aion-cli::bin/aion tests::server_subcommand_maps_authoring_gleam_path
        PASS [   0.009s] (1242/5472) aion-cli::bin/aion tests::server_workflow_package_flag_is_repeatable
        PASS [   0.007s] (1243/5472) aion-cli::bin/aion tests::start_malformed_input_file_json_error_names_the_flag_and_path
        PASS [   0.008s] (1244/5472) aion-cli::bin/aion tests::start_reads_input_payload_from_file
        PASS [   0.012s] (1245/5472) aion-cli::bin/aion tests::signal_cancel_and_query_accept_optional_run_id
        PASS [   0.009s] (1246/5472) aion-cli::bin/aion tests::start_rejects_input_and_input_file_together
        PASS [   0.012s] (1247/5472) aion-cli::bin/aion tests::start_reads_the_display_name_flag
        PASS [   0.008s] (1248/5472) aion-cli::bin/aion tests::tail_accepts_parent_id_without_any_child_id
        PASS [   0.009s] (1249/5472) aion-cli::bin/aion tests::start_without_input_file_keeps_inline_input
        PASS [   0.008s] (1250/5472) aion-cli::bin/aion tests::the_same_flags_on_worker_agent_still_earn_the_advice
        PASS [   0.009s] (1251/5472) aion-cli::bin/aion tests::the_argv_a_pre_change_managed_worker_record_replays_earns_a_named_refusal
        PASS [   0.009s] (1252/5472) aion-cli::bin/aion tests::the_server_refuses_a_client_endpoint_instead_of_binding_the_default
        PASS [   0.011s] (1253/5472) aion-cli::bin/aion tests::the_agent_verb_survives_the_flag_deletion_and_takes_a_document
        PASS [   0.009s] (1254/5472) aion-cli::bin/aion tests::top_level_help_lists_the_server_subcommand
        PASS [   0.007s] (1255/5472) aion-cli::bin/aion worker_agent::environment_report::tests::a_declaration_whose_every_name_is_absent_says_so_in_words
        PASS [   0.007s] (1256/5472) aion-cli::bin/aion worker_agent::environment_report::tests::a_closed_writer_is_reported_rather_than_swallowed
        PASS [   0.010s] (1257/5472) aion-cli::bin/aion tests::worker_agent_help_documents_the_model_it_serves
        PASS [   0.008s] (1258/5472) aion-cli::bin/aion worker_agent::environment_report::tests::a_complete_environment_reports_nothing_at_all
        PASS [   0.006s] (1259/5472) aion-cli::bin/aion worker_agent::environment_report::tests::every_absent_variable_is_named_beside_the_queue_and_the_document
        PASS [   0.011s] (1260/5472) aion-cli::bin/aion worker_agent::tests::a_blank_environment_value_counts_as_missing
        PASS [   0.014s] (1261/5472) aion-cli::bin/aion tests::workflow_operations_allow_omitted_run_id
        PASS [   0.016s] (1262/5472) aion-cli::bin/aion tests::worker_agent_parses_the_document_and_repeatable_liminal_addresses
        PASS [   0.007s] (1263/5472) aion-cli::bin/aion worker_agent::tests::a_document_without_a_harness_section_declares_no_launch
        PASS [   0.008s] (1264/5472) aion-cli::bin/aion worker_agent::tests::a_document_that_does_not_compile_is_refused_with_its_diagnostics
        PASS [   0.010s] (1265/5472) aion-cli::bin/aion worker_agent::tests::missing_settings_are_reported_by_name_and_all_at_once
        PASS [   0.038s] (1266/5472) aion-cli::bin/aion worker_agent::tests::a_partially_served_queue_is_refused_by_name
        PASS [   0.038s] (1267/5472) aion-cli::bin/aion worker_agent::tests::a_queue_of_only_bodied_actions_is_refused_by_name
        PASS [   0.041s] (1268/5472) aion-cli::bin/aion worker_agent::tests::a_body_carrying_action_is_excluded_from_the_surface
        PASS [   0.036s] (1269/5472) aion-cli::bin/aion worker_agent::tests::a_queue_the_document_does_not_declare_is_refused
        PASS [   0.036s] (1270/5472) aion-cli::bin/aion worker_agent::tests::check_prints_the_surface_without_connecting
        PASS [   0.034s] (1271/5472) aion-cli::bin/aion worker_agent::tests::serving_a_document_with_no_harness_section_refuses_naming_the_section
        PASS [   0.042s] (1272/5472) aion-cli::bin/aion worker_agent::tests::a_single_worker_block_derives_the_queue_without_a_flag
        PASS [   0.032s] (1273/5472) aion-cli::bin/aion worker_agent::tests::the_acp_check_report_advertises_exactly_cancel
        PASS [   0.006s] (1274/5472) aion-cli::bin/aion worker_awl::tests::a_document_that_does_not_check_is_refused_with_its_diagnostics
        PASS [   0.011s] (1275/5472) aion-cli::bin/aion worker_agent::tests::the_environment_supplies_settings_and_the_flag_wins
        PASS [   0.010s] (1276/5472) aion-cli::bin/aion worker_agent::tests::the_launch_settings_come_from_the_documents_harness_section
        PASS [   0.057s] (1277/5472) aion-cli::bin/aion worker_agent::tests::a_multi_worker_document_refuses_and_names_every_choice
        PASS [   0.013s] (1278/5472) aion-cli::bin/aion worker_awl::tests::a_workflow_document_is_refused_by_name
        PASS [   0.007s] (1279/5472) aion-cli::bin/aion worker_awl::tests::missing_connection_settings_are_all_named_at_once
        PASS [   0.034s] (1280/5472) aion-cli::bin/aion worker_agent::tests::the_refusal_does_not_point_at_a_check_that_answers_ok
        PASS [   0.039s] (1281/5472) aion-cli::bin/aion worker_agent::tests::the_advertised_surface_is_the_documents_and_is_not_empty
        PASS [   0.043s] (1282/5472) aion-cli::bin/aion worker_agent::tests::the_composed_config_advertises_exactly_the_documents_surface
        PASS [   0.034s] (1283/5472) aion-cli::bin/aion worker_awl::tests::a_parameter_value_cannot_become_a_command
        PASS [   0.011s] (1284/5472) aion-cli::bin/aion worker_awl::tests::the_identity_is_derived_from_the_queue_and_overridable
        PASS [   0.009s] (1285/5472) aion-cli::bin/aion worker_managed::tests::a_response_without_a_worker_says_so_by_name
        PASS [   0.039s] (1286/5472) aion-cli::bin/aion worker_awl::tests::every_capture_shapes_a_real_commands_result
        PASS [   0.039s] (1287/5472) aion-cli::bin/aion worker_awl::tests::invalid_json_output_is_terminal
        PASS [   0.042s] (1288/5472) aion-cli::bin/aion worker_awl::tests::a_non_object_dispatch_input_is_refused_terminally
        PASS [   0.008s] (1289/5472) aion-cli::bin/aion worker_managed::tests::desired_and_actual_state_are_both_rendered
        PASS [   0.035s] (1290/5472) aion-cli::bin/aion worker_awl::tests::the_built_worker_serves_exactly_the_documents_actions
        PASS [   0.010s] (1291/5472) aion-cli::bin/aion worker_managed::tests::the_upgrade_flag_distinguishes_unknown_from_not_upgraded
        PASS [   0.019s] (1292/5472) aion-cli::bin/aion worker_managed::tests::an_exit_renders_every_recorded_field
        PASS [   0.045s] (1293/5472) aion-cli::bin/aion worker_awl::tests::the_check_report_carries_the_descriptors_and_the_commands
        PASS [   0.021s] (1294/5472) aion-cli::bin/aion worker_shell::tests::a_wired_action_streams_its_output_onto_the_transcript
        PASS [   0.018s] (1295/5472) aion-cli::bin/aion worker_shell::tests::composite_field_projection_is_typed_refusal
        PASS [   0.051s] (1296/5472) aion-cli::bin/aion worker_awl::tests::the_surface_comes_from_the_document
        PASS [   0.044s] (1297/5472) aion-cli::bin/aion worker_shell::tests::a_queue_the_document_does_not_declare_is_refused
        PASS [   0.042s] (1298/5472) aion-cli::bin/aion worker_shell::tests::an_action_with_a_declared_body_is_refused
        PASS [   0.054s] (1299/5472) aion-cli::bin/aion worker_shell::tests::a_partially_wired_queue_is_refused
        PASS [   0.018s] (1300/5472) aion-cli::bin/aion worker_shell::tests::strict_manifest_rejects_unknown_keys_and_bad_placeholders
        PASS [   0.013s] (1301/5472) aion-client client::tests::auth_debug_redacts_token
        PASS [   0.018s] (1302/5472) aion-client client::tests::builder_captures_connection_options
        PASS [   0.019s] (1303/5472) aion-client client::tests::client_is_clone_send_sync
        PASS [   0.021s] (1304/5472) aion-client client::tests::stream_endpoint_has_no_default
        PASS [   0.078s] (1305/5472) aion-cli::bin/aion worker_shell::tests::an_undeclared_action_is_refused_by_name
        PASS [   0.031s] (1306/5472) aion-client error::tests::client_error_is_send_sync_static
        PASS [   0.062s] (1307/5472) aion-cli::bin/aion worker_shell::tests::the_worker_advertises_the_documents_action_surface
        PASS [   0.079s] (1308/5472) aion-cli::bin/aion worker_shell::tests::process_boundary_round_trips_text_and_json
        PASS [   0.100s] (1309/5472) aion-cli::bin/aion worker_shell::tests::emitted_manifest_passes_the_real_whole_queue_startup_check
        PASS [   0.048s] (1310/5472) aion-client error::tests::detail_display_appends_the_typed_discriminator
        PASS [   0.045s] (1311/5472) aion-client error::tests::not_owner_is_its_own_class_and_never_collapses_into_unavailable
        PASS [   0.030s] (1312/5472) aion-client ops::tests::a_blank_display_name_is_refused
        PASS [   0.030s] (1313/5472) aion-client ops::tests::backend_outcome_error_is_a_server_fault_not_query_failed
        PASS [   0.086s] (1314/5472) aion-client error::tests::display_is_class_colon_detail_for_every_variant
        PASS [   0.062s] (1315/5472) aion-client ops::tests::a_replay_with_a_different_name_dedupes_and_reports_the_unapplied_name
        PASS [   0.050s] (1316/5472) aion-client ops::tests::query_forwards_its_arguments_onto_the_wire
        PASS [   0.062s] (1317/5472) aion-client ops::tests::cancel_list_and_describe_map_requests
        PASS [   0.086s] (1318/5472) aion-client ops::tests::a_no_argument_query_sends_the_canonical_null_document
        PASS [   0.193s] (1319/5472) aion-cli::bin/aion worker_shell::tests::an_existing_but_empty_pid_file_is_not_readiness
        PASS [   0.017s] (1320/5472) aion-client ops::tests::signal_maps_latest_run_and_error
        PASS [   0.079s] (1321/5472) aion-client ops::tests::query_failed_outcome_error_maps_to_query_failed
        PASS [   0.030s] (1322/5472) aion-client ops::tests::reopen_returns_running_run_and_maps_request
        PASS [   0.050s] (1323/5472) aion-client ops::tests::query_typed_decodes_no_arg_query_result
        PASS [   0.022s] (1324/5472) aion-client ops::tests::start_idempotency_replays_identical_and_rejects_conflicts
        PASS [   0.076s] (1325/5472) aion-client ops::tests::query_maps_result_error_and_deadline
        PASS [   0.025s] (1326/5472) aion-client ops::tests::the_unapplied_name_report_fires_only_for_a_replay_that_asked_for_a_name
        PASS [   0.058s] (1327/5472) aion-client ops::tests::reopen_maps_invalid_state_to_distinct_typed_error
        PASS [   0.015s] (1328/5472) aion-client stream::tests::live_only_disconnect_after_delivery_is_honest_unavailable
        PASS [   0.067s] (1329/5472) aion-client ops::tests::query_typed_forwards_non_empty_args_without_silent_drop
        PASS [   0.034s] (1330/5472) aion-client payload::tests::typed_payload_round_trips_through_json_payload
        PASS [   0.029s] (1331/5472) aion-client stream::tests::from_sequence_passes_the_cursor_on_the_initial_attach
        PASS [   0.045s] (1332/5472) aion-client ops::tests::start_idempotency_treats_a_changed_route_as_a_different_request
        PASS [   0.041s] (1333/5472) aion-client ops::tests::start_maps_request_and_returns_handle
        PASS [   0.017s] (1334/5472) aion-client stream::tests::live_only_unavailable_attach_failure_is_retried_before_any_delivery
        PASS [   0.023s] (1335/5472) aion-client stream::tests::live_only_streams_do_not_dedupe_sequence_numbers_across_workflows
        PASS [   0.024s] (1336/5472) aion-client stream::tests::namespace_denied_is_terminal_and_never_retried
        PASS [   0.026s] (1337/5472) aion-client stream::tests::live_only_streams_reconnect_only_before_any_delivery
        PASS [   0.025s] (1338/5472) aion-client stream::tests::non_retryable_attach_failure_is_terminal
        PASS [   0.026s] (1339/5472) aion-client stream::tests::not_found_is_terminal_and_never_retried
        PASS [   0.023s] (1340/5472) aion-client stream::tests::terminal_failure_is_yielded_before_end
        PASS [   0.021s] (1341/5472) aion-client stream::tests::unavailable_reconnect_failure_retries_with_the_same_cursor
        PASS [   0.030s] (1342/5472) aion-client stream::tests::resumes_after_transient_disconnect_without_gaps_or_duplicates
        PASS [   0.016s] (1343/5472) aion-client transport::embedded::tests::cursor_zero_is_invalid_argument
        PASS [   0.026s] (1344/5472) aion-client transport::embedded::tests::cursor_ahead_of_history_is_invalid_argument
        PASS [   0.036s] (1345/5472) aion-client stream::tests::unavailable_attach_failure_is_retried_until_attach_succeeds
        PASS [   0.023s] (1346/5472) aion-client transport::embedded::tests::cursor_at_head_plus_one_yields_empty_replay_and_live_tail_only
        PASS [   0.017s] (1347/5472) aion-client transport::embedded::tests::every_named_engine_error_reaches_its_own_class
        PASS [   0.017s] (1348/5472) aion-client transport::embedded::tests::live_lag_maps_to_unavailable
        PASS [   0.021s] (1349/5472) aion-client transport::embedded::tests::mid_history_cursor_replays_suffix_only
        PASS [   0.032s] (1350/5472) aion-client transport::embedded::tests::lag_mid_splice_surfaces_unavailable_after_the_replay
        PASS [   0.020s] (1351/5472) aion-client transport::embedded::tests::overlap_between_snapshot_and_live_is_deduplicated_contiguous_unique
        PASS [   0.028s] (1352/5472) aion-client transport::embedded::tests::shutting_down_carries_its_discriminator_not_only_its_class
        PASS [   0.022s] (1353/5472) aion-client transport::transcript_ws::tests::decodes_frame_from_canonical_server_serializer
        PASS [   0.022s] (1354/5472) aion-client transport::transcript_ws::tests::lag_is_a_recoverable_typed_item
        PASS [   0.043s] (1355/5472) aion-client transport::embedded::tests::per_workflow_stream_closes_after_terminal_event
        PASS [   0.027s] (1356/5472) aion-client transport::transcript_ws::tests::rejects_unknown_kind_and_fields_loudly
        PASS [   0.037s] (1357/5472) aion-client transport::ws::socket_tests::abrupt_socket_drop_surfaces_one_unavailable_item
        PASS [   0.042s] (1358/5472) aion-client transport::ws::socket_tests::terminal_error_frame_ends_the_attempt_with_the_mapped_error
        PASS [   0.042s] (1359/5472) aion-client transport::ws::unit_tests::initial_attach_sends_no_resume_cursor
        PASS [   0.052s] (1360/5472) aion-client transport::ws::unit_tests::custom_ca_certificate_is_added_to_the_websocket_trust_roots
        PASS [   0.068s] (1361/5472) aion-client transport::ws::socket_tests::open_subscription_streams_events_and_forwards_identity_headers
        PASS [   0.018s] (1362/5472) aion-client transport::ws::unit_tests::live_only_subscriptions_reject_resume_cursors
        PASS [   0.018s] (1363/5472) aion-client transport::ws::unit_tests::malformed_custom_ca_pem_is_invalid_argument
        PASS [   0.049s] (1364/5472) aion-client transport::ws::unit_tests::lagged_error_frames_map_to_unavailable_so_the_resume_loop_reconnects
        PASS [   0.073s] (1365/5472) aion-client transport::ws::socket_tests::resume_loop_reconnects_with_the_cursor_over_a_real_socket
        PASS [   0.017s] (1366/5472) aion-client transport::ws::unit_tests::missing_stream_endpoint_is_invalid_argument_with_precise_message
        PASS [   0.019s] (1367/5472) aion-client transport::ws::unit_tests::stream_url_maps_http_schemes_and_passes_ws_through
        PASS [   0.025s] (1368/5472) aion-client transport::ws::unit_tests::no_tls_options_means_no_custom_connector
        PASS [   0.028s] (1369/5472) aion-client transport::ws::unit_tests::per_workflow_frame_carries_the_resume_cursor
        PASS [   0.029s] (1370/5472) aion-client transport::ws::unit_tests::stream_url_rejects_non_websocket_schemes
        PASS [   0.020s] (1371/5472) aion-client::conformance shared_client_contract_conformance
        PASS [   0.008s] (1372/5472) aion-client::error_mapping namespace_denied_proto_details_override_the_status_message
        PASS [   0.044s] (1373/5472) aion-client transport::ws::unit_tests::terminal_error_frames_map_through_the_shared_taxonomy
        PASS [   0.044s] (1374/5472) aion-client transport::ws::unit_tests::tls_options_without_custom_ca_build_a_webpki_connector
        PASS [   0.040s] (1375/5472) aion-client transport::ws::unit_tests::unrecognizable_frames_are_terminal_server_errors
        PASS [   0.062s] (1376/5472) aion-client transport::ws::unit_tests::streamed_event_frames_decode_to_core_events
        PASS [   0.012s] (1377/5472) aion-client::error_mapping sequence_conflict_is_a_server_bug_not_already_exists
        PASS [   0.073s] (1378/5472) aion-client transport::ws::unit_tests::resume_cursor_zero_never_reaches_the_wire
        PASS [   0.015s] (1379/5472) aion-client::error_mapping status_fallback_without_details_keeps_the_status_message
        PASS [   0.016s] (1380/5472) aion-client::error_mapping status_details_carry_the_authoritative_typed_wire_error
        PASS [   0.015s] (1381/5472) aion-client::error_mapping wire_error_type_discriminator_is_preserved
        PASS [   0.017s] (1382/5472) aion-client::error_mapping wire_errors_map_to_distinct_variants_preserving_detail
        PASS [   0.015s] (1383/5472) aion-core activity_event::tests::envelope_round_trips_through_json
        PASS [   0.018s] (1384/5472) aion-core activity_event::tests::ephemeral_delta_round_trips_without_store_seq
        PASS [   0.014s] (1385/5472) aion-core activity_event::tests::every_event_kind_round_trips
        PASS [   0.016s] (1386/5472) aion-core activity_event::tests::the_run_axis_survives_the_wire_and_separates_two_generations
        PASS [   0.016s] (1387/5472) aion-core agent_outcome::tests::a_three_field_payload_no_longer_decodes
        PASS [   0.018s] (1388/5472) aion-core agent_outcome::tests::an_empty_session_id_is_refused_by_name
        PASS [   0.020s] (1389/5472) aion-core agent_outcome::tests::the_record_round_trips_through_its_payload
        PASS [   0.016s] (1390/5472) aion-core current_step::tests::a_dispatch_without_a_scheduling_opens_nothing
        PASS [   0.023s] (1391/5472) aion-core agent_outcome::tests::every_stop_kind_has_its_canonical_snake_case_spelling
        PASS [   0.023s] (1392/5472) aion-core agent_outcome::tests::the_record_serializes_as_the_four_snake_case_fields
        PASS [   0.010s] (1393/5472) aion-core current_step::tests::a_reopen_reopens_only_the_activities_it_names
        PASS [   0.031s] (1394/5472) aion-core current_step::tests::a_fan_out_keeps_every_sibling_and_current_is_the_most_recently_advanced
        PASS [   0.016s] (1395/5472) aion-core current_step::tests::a_reopen_of_an_unscheduled_ordinal_reports_nothing
        PASS [   0.021s] (1396/5472) aion-core current_step::tests::a_retried_ordinal_reports_the_new_attempt_once
        PASS [   0.022s] (1397/5472) aion-core current_step::tests::a_run_with_no_activity_has_no_current_step
        PASS [   0.033s] (1398/5472) aion-core current_step::tests::a_new_run_segment_drops_the_prior_segments_open_step
        PASS [   0.020s] (1399/5472) aion-core current_step::tests::a_scheduled_step_is_current_and_carries_its_stamped_address
        PASS [   0.021s] (1400/5472) aion-core current_step::tests::an_adoption_offer_leaves_the_ordinal_exactly_as_dispatched
        PASS [   0.025s] (1401/5472) aion-core current_step::tests::advisory_exhaustion_is_not_a_terminal
        PASS [   0.024s] (1402/5472) aion-core current_step::tests::re_dispatch_after_a_reopen_advances_the_ordinal
        PASS [   0.033s] (1403/5472) aion-core current_step::tests::an_empty_history_folds_to_nothing
        PASS [   0.030s] (1404/5472) aion-core current_step::tests::each_terminal_retires_only_its_own_ordinal
        PASS [   0.020s] (1405/5472) aion-core current_step::tests::unrelated_events_neither_open_nor_retire_a_step
        PASS [   0.032s] (1406/5472) aion-core current_step::tests::dispatch_advances_the_same_ordinal
        PASS [   0.310s] (1407/5472) aion-client transport::embedded::tests::a_directly_read_store_refusal_keeps_its_routing_class
        PASS [   0.012s] (1408/5472) aion-core error::tests::activity_error_reports_terminal_classification
        PASS [   0.014s] (1409/5472) aion-core error::tests::policy_refused_is_neither_retryable_nor_settled
        PASS [   0.009s] (1410/5472) aion-core event::tests::activity_scheduled_decodes_old_history_without_node_as_none
        PASS [   0.314s] (1411/5472) aion-client transport::embedded::tests::embedded_resume_splices_recorded_history_with_live_appends
        PASS [   0.022s] (1412/5472) aion-core error::tests::workflow_error_from_activity_error_preserves_message_and_details
        PASS [   0.021s] (1413/5472) aion-core event::tests::activity_lifecycle_decodes_old_history_without_attempt_as_legacy_sentinel
        PASS [   0.026s] (1414/5472) aion-core error::tests::errors_round_trip_through_json
        PASS [   0.017s] (1415/5472) aion-core event::tests::activity_lifecycle_records_and_reads_back_its_attempt
        PASS [   0.039s] (1416/5472) aion-core error::tests::activity_error_reports_retryable_classification
        PASS [   0.020s] (1417/5472) aion-core event::tests::activity_scheduled_decodes_old_history_without_task_queue_as_default
        PASS [   0.018s] (1418/5472) aion-core event::tests::activity_scheduled_records_and_reads_back_its_task_queue
        PASS [   0.016s] (1419/5472) aion-core event::tests::display_name_is_none_without_the_attribute
        PASS [   0.016s] (1420/5472) aion-core event::tests::display_name_later_update_overrides_earlier
        PASS [   0.028s] (1421/5472) aion-core event::tests::activity_scheduled_records_and_reads_back_its_node
        PASS [   0.010s] (1422/5472) aion-core event::tests::old_history_without_pause_resume_decodes_unchanged
        PASS [   0.027s] (1423/5472) aion-core event::tests::display_name_projects_from_recorded_attribute
        PASS [   0.028s] (1424/5472) aion-core event::tests::child_events_round_trip_through_json
        PASS [   0.017s] (1425/5472) aion-core event::tests::events_round_trip_through_json
        PASS [   0.032s] (1426/5472) aion-core event::tests::event_accessors_return_envelope_fields
        PASS [   0.013s] (1427/5472) aion-core event::tests::start_time_task_queue_is_none_without_the_attribute
        PASS [   0.025s] (1428/5472) aion-core event::tests::extended_events_round_trip_through_json
        PASS [   0.013s] (1429/5472) aion-core filter::tests::a_continue_as_new_successor_inherits_its_predecessors_display_name
        PASS [   0.018s] (1430/5472) aion-core event::tests::start_time_task_queue_projects_from_recorded_attribute
        PASS [   0.032s] (1431/5472) aion-core event::tests::pause_resume_events_round_trip_through_json
        PASS [   0.021s] (1432/5472) aion-core event::tests::timer_cancelled_decodes_old_history_without_cause_as_workflow_intent
        PASS [   0.021s] (1433/5472) aion-core filter::tests::combined_filter_requires_every_field_to_match
        PASS [   0.021s] (1434/5472) aion-core filter::tests::default_filter_matches_all_summaries
        PASS [   0.007s] (1435/5472) aion-core filter::tests::failure_projection_is_reset_by_reopen
        PASS [   0.020s] (1436/5472) aion-core filter::tests::failure_projection_attributes_failed_step_and_reason
        PASS [   0.019s] (1437/5472) aion-core filter::tests::failure_projection_has_reason_but_no_step_when_no_activity_failed
        PASS [   0.021s] (1438/5472) aion-core filter::tests::failure_projection_ignores_a_failed_step_that_later_succeeded
        PASS [   0.019s] (1439/5472) aion-core filter::tests::failure_projection_is_none_for_non_failed
        PASS [   0.020s] (1440/5472) aion-core filter::tests::parent_filter_matches_parent_workflow_id
        PASS [   0.021s] (1441/5472) aion-core filter::tests::schedule_events_do_not_set_summary_end_time
        PASS [   0.017s] (1442/5472) aion-core filter::tests::started_before_filter_matches_start_time_inclusively
        PASS [   0.030s] (1443/5472) aion-core filter::tests::started_after_filter_matches_start_time_inclusively
        PASS [   0.013s] (1444/5472) aion-core filter::tests::summary_projects_continued_as_new
        PASS [   0.030s] (1445/5472) aion-core filter::tests::status_filter_matches_projected_status
        PASS [   0.031s] (1446/5472) aion-core filter::tests::summary_end_time_uses_last_terminal_event
        PASS [   0.026s] (1447/5472) aion-core filter::tests::summary_from_history_projects_required_fields
        PASS [   0.022s] (1448/5472) aion-core filter::tests::summary_from_history_without_start_returns_none
        PASS [   0.030s] (1449/5472) aion-core filter::tests::summary_from_history_projects_latest_display_name
        PASS [   0.021s] (1450/5472) aion-core filter::tests::summary_projects_current_run_after_continue_as_new
        PASS [   0.014s] (1451/5472) aion-core generated_types::the_display_name_key_needs_no_escaping
        PASS [   0.013s] (1452/5472) aion-core ids::tests::display_formats_are_stable
        PASS [   0.009s] (1453/5472) aion-core ids::tests::sequence_identifiers_expose_positions
        PASS [   0.023s] (1454/5472) aion-core filter::tests::workflow_type_filter_matches_exact_type
        PASS [   0.006s] (1455/5472) aion-core intervention::tests::command_without_auth_subject_round_trips
        PASS [   0.024s] (1456/5472) aion-core generated_types::export_ops_console_wire_types
        PASS [   0.008s] (1457/5472) aion-core intervention::tests::capabilities_gate_on_advertised_primitives
        PASS [   0.025s] (1458/5472) aion-core ids::tests::identifiers_round_trip_through_json
        PASS [   0.025s] (1459/5472) aion-core ids::tests::named_timer_rejects_empty_name
        PASS [   0.010s] (1460/5472) aion-core intervention::tests::every_intervention_variant_round_trips
        PASS [   0.023s] (1461/5472) aion-core ids::tests::uuid_identifiers_are_hash_map_keys
        PASS [   0.008s] (1462/5472) aion-core payload::tests::json_values_round_trip_losslessly
        PASS [   0.020s] (1463/5472) aion-core intervention::tests::empty_capability_set_is_first_class
        PASS [   0.019s] (1464/5472) aion-core intervention::tests::every_outcome_round_trips
        PASS [   0.011s] (1465/5472) aion-core schedule::tests::schedule_config_round_trips_through_json
        PASS [   0.020s] (1466/5472) aion-core payload::tests::json_null_is_the_serialized_null_document
        PASS [   0.013s] (1467/5472) aion-core schedule::tests::overlap_policies_round_trip_through_json
        PASS [   0.012s] (1468/5472) aion-core schedule::tests::schedule_id_display_and_from_str_use_uuid_format
        PASS [   0.015s] (1469/5472) aion-core schedule::tests::catch_up_policies_round_trip_through_json
        PASS [   0.014s] (1470/5472) aion-core schedule::tests::schedule_config_defaults_missing_policy_fields
        PASS [   0.013s] (1471/5472) aion-core schedule::tests::schedule_id_is_a_hash_map_key
        PASS [   0.015s] (1472/5472) aion-core search::tests::search_attribute_values_round_trip_through_json
        PASS [   0.018s] (1473/5472) aion-core schedule::tests::trigger_specs_round_trip_through_json
        PASS [   0.018s] (1474/5472) aion-core search::tests::schema_registers_and_validates_matching_types
        PASS [   0.016s] (1475/5472) aion-core search::tests::validating_mismatched_type_errors
        PASS [   0.020s] (1476/5472) aion-core search::tests::registering_same_name_with_different_type_errors
        PASS [   0.022s] (1477/5472) aion-core schedule::tests::schedule_id_round_trips_through_json
        PASS [   0.016s] (1478/5472) aion-core search::tests::validating_unregistered_attribute_errors
        PASS [   0.023s] (1479/5472) aion-core search::tests::search_attribute_types_round_trip_through_json
        PASS [   0.019s] (1480/5472) aion-core search::tests::values_report_matching_attribute_types
        PASS [   0.019s] (1481/5472) aion-core status::tests::complete_after_resume_projects_completed
        PASS [   0.020s] (1482/5472) aion-core status::tests::completed_terminal_event_projects_to_completed
        PASS [   0.025s] (1483/5472) aion-core status::tests::complete_while_paused_still_projects_completed
        PASS [   0.026s] (1484/5472) aion-core status::tests::cancelled_terminal_event_projects_to_cancelled
        PASS [   0.021s] (1485/5472) aion-core status::tests::current_lease_terminal_is_none_for_running_and_reopened
        PASS [   0.024s] (1486/5472) aion-core status::tests::continued_as_new_projects_status
        PASS [   0.020s] (1487/5472) aion-core status::tests::current_lease_terminal_returns_terminal_after_reopen_and_retermination
        PASS [   0.012s] (1488/5472) aion-core status::tests::empty_history_projects_to_running
        PASS [   0.011s] (1489/5472) aion-core status::tests::failed_terminal_event_projects_to_failed
        PASS [   0.013s] (1490/5472) aion-core status::tests::last_terminal_lifecycle_event_determines_status
        PASS [   0.010s] (1491/5472) aion-core status::tests::pause_projects_paused_and_is_non_terminal
        PASS [   0.017s] (1492/5472) aion-core status::tests::projection_is_deterministic
        PASS [   0.019s] (1493/5472) aion-core status::tests::non_terminal_history_projects_to_running
        PASS [   0.023s] (1494/5472) aion-core status::tests::reopen_after_failure_projects_running
        PASS [   0.016s] (1495/5472) aion-core status::tests::schedule_events_do_not_change_workflow_status
        PASS [   0.028s] (1496/5472) aion-core status::tests::reopen_then_new_terminal_projects_that_terminal
        PASS [   0.025s] (1497/5472) aion-core status::tests::replacement_start_projects_continue_as_new_chain_running
        PASS [   0.025s] (1498/5472) aion-core status::tests::resume_after_pause_projects_running
        PASS [   0.023s] (1499/5472) aion-core status::tests::started_then_continued_as_new_projects_status
        PASS [   0.012s] (1500/5472) aion-fleet-plan grammar::admit::tests::canonical_round_trips
        PASS [   0.017s] (1501/5472) aion-core status::tests::workflow_status_terminality_classifies_running_and_terminal_statuses
        PASS [   0.032s] (1502/5472) aion-core status::tests::run_segment_scopes_to_the_named_run
        PASS [   0.025s] (1503/5472) aion-core status::tests::timed_out_terminal_event_projects_to_timed_out
        PASS [   0.014s] (1504/5472) aion-fleet-plan grammar::admit::tests::prose_is_refused
        PASS [   0.024s] (1505/5472) aion-fleet-plan grammar::admit::tests::always_parses
        PASS [   0.012s] (1506/5472) aion-fleet-plan grammar::admit::tests::the_exemplar_condition_parses
        PASS [   0.016s] (1507/5472) aion-fleet-plan ids::tests::conclusion_path_is_the_designs_canonical_shape
        PASS [   0.031s] (1508/5472) aion-fleet-plan grammar::admit::tests::equals_parses
        PASS [   0.016s] (1509/5472) aion-fleet-plan ids::tests::no_refused_vector_can_escape_the_conclusion_prefix
        PASS [   0.018s] (1510/5472) aion-fleet-plan ids::tests::the_design_exemplar_ids_are_accepted
        PASS [   0.023s] (1511/5472) aion-fleet-plan ids::tests::digits_may_lead
        PASS [   0.024s] (1512/5472) aion-fleet-plan ids::tests::all_nine_charset_vectors_are_refused
        PASS [   0.024s] (1513/5472) aion-fleet-plan ids::tests::underscore_is_accepted_in_the_tail_only
        PASS [   0.019s] (1514/5472) aion-fleet-plan verify::tests::only_an_exact_object_id_is_accepted
        PASS [   0.025s] (1515/5472) aion-fleet-plan verify::tests::observed_bytes_show_every_byte
        PASS [   0.022s] (1516/5472) aion-fleet-plan verify::tests::the_token_contract_is_bytes_not_prose
        PASS [   0.022s] (1517/5472) aion-fleet-plan::r1_plan_refusals a_plan_at_the_ceiling_is_accepted
        PASS [   0.017s] (1518/5472) aion-fleet-plan::r1_plan_refusals advance_hold_is_valid
        PASS [   0.016s] (1519/5472) aion-fleet-plan::r1_plan_refusals canonical_form_refusals_are_each_named
        PASS [   0.029s] (1520/5472) aion-fleet-plan::r1_plan_refusals a_plan_over_the_dispatch_authored_ceiling_refuses_before_any_leg_is_dispatched
        PASS [   0.011s] (1521/5472) aion-fleet-plan::r1_plan_refusals charset_no_accepted_id_can_escape_the_conclusion_prefix
        PASS [   0.015s] (1522/5472) aion-fleet-plan::r1_plan_refusals cause_code_wire_renderings_are_pinned
        PASS [   0.037s] (1523/5472) aion-fleet-plan::r1_plan_refusals a_plan_without_a_block_is_the_legacy_path_not_a_refusal
        PASS [   0.022s] (1524/5472) aion-fleet-plan::r1_plan_refusals charset_all_nine_stage_id_vectors_refuse
        PASS [   0.025s] (1525/5472) aion-fleet-plan::r1_plan_refusals charset_all_nine_leg_id_vectors_refuse
        PASS [   0.010s] (1526/5472) aion-fleet-plan::r1_plan_refusals clause_02_duplicate_stage_id_refuses
        PASS [   0.019s] (1527/5472) aion-fleet-plan::r1_plan_refusals clause_01_an_empty_block_refuses
        PASS [   0.010s] (1528/5472) aion-fleet-plan::r1_plan_refusals clause_03_a_leg_in_two_stages_refuses
        PASS [   0.018s] (1529/5472) aion-fleet-plan::r1_plan_refusals clause_01_two_blocks_refuse
        PASS [   0.023s] (1530/5472) aion-fleet-plan::r1_plan_refusals clause_01_unsupported_version_refuses_by_name
        PASS [   0.025s] (1531/5472) aion-fleet-plan::r1_plan_refusals clause_08_an_arbitrary_multi_parent_graph_refuses
        PASS [   0.025s] (1532/5472) aion-fleet-plan::r1_plan_refusals clause_05_a_dispatch_declared_leg_in_no_stage_refuses
        PASS [   0.026s] (1533/5472) aion-fleet-plan::r1_plan_refusals clause_04_a_leg_dispatch_never_declared_refuses
        PASS [   0.027s] (1534/5472) aion-fleet-plan::r1_plan_refusals clause_07_unknown_predecessor_refuses
        PASS [   0.027s] (1535/5472) aion-fleet-plan::r1_plan_refusals clause_06_a_stage_with_no_leg_refuses
        PASS [   0.027s] (1536/5472) aion-fleet-plan::r1_plan_refusals clause_07_a_stage_following_itself_refuses
        PASS [   0.027s] (1537/5472) aion-fleet-plan::r1_plan_refusals clause_08_multiple_roots_refuse
        PASS [   0.028s] (1538/5472) aion-fleet-plan::r1_plan_refusals clause_08_no_root_refuses
        PASS [   0.019s] (1539/5472) aion-fleet-plan::r1_plan_refusals clause_10_a_conclusion_owner_outside_its_stage_refuses
        PASS [   0.020s] (1540/5472) aion-fleet-plan::r1_plan_refusals clause_13_an_admit_on_a_later_stage_refuses
        PASS [   0.019s] (1541/5472) aion-fleet-plan::r1_plan_refusals clause_13_an_admit_naming_no_stage_refuses
        PASS [   0.023s] (1542/5472) aion-fleet-plan::r1_plan_refusals clause_11_a_multi_leg_stage_with_multiple_conclusions_refuses
        PASS [   0.016s] (1543/5472) aion-fleet-plan::r1_plan_refusals clause_16_an_advance_outside_auto_or_hold_refuses
        PASS [   0.010s] (1544/5472) aion-fleet-plan::r1_plan_refusals clause_17_loose_fleet_state_refuses_at_exit_one
        PASS [   0.024s] (1545/5472) aion-fleet-plan::r1_plan_refusals clause_14_an_undeclared_compared_conclusion_refuses
        PASS [   0.007s] (1546/5472) aion-fleet-plan::r1_plan_refusals subsumed_clauses::clause_08_no_root_refuses_a_rootless_set
        PASS [   0.032s] (1547/5472) aion-fleet-plan::r1_plan_refusals clause_09_a_cycle_refuses_by_name_not_as_a_missing_root
        PASS [   0.014s] (1548/5472) aion-fleet-plan::r1_plan_refusals subsumed_clauses::clause_02_nonempty_refuses_an_empty_stage_id
        PASS [   0.017s] (1549/5472) aion-fleet-plan::r1_plan_refusals subsumed_clauses::clause_06_a_legless_stage_refuses_at_the_clause
        PASS [   0.024s] (1550/5472) aion-fleet-plan::r1_plan_refusals every_graph_refusal_is_exit_four_and_never_widens_plan_mismatch
        PASS [   0.014s] (1551/5472) aion-fleet-plan::r1_plan_refusals the_140_exemplar_block_is_valid
        PASS [   0.013s] (1552/5472) aion-fleet-plan::r1_plan_refusals the_design_mixed_graph_example_is_valid
        PASS [   0.016s] (1553/5472) aion-fleet-plan::r1_plan_refusals subsumed_clauses::clause_15_colliding_conclusion_paths_refuse
        PASS [   0.026s] (1554/5472) aion-fleet-plan::r1_plan_refusals subsumed_clauses::clause_08_multiple_terminals_refuse
        PASS [   0.030s] (1555/5472) aion-fleet-plan::r1_plan_refusals subsumed_clauses::clause_12_multi_value_multi_leg_refuses
        PASS [   0.018s] (1556/5472) aion-fleet-plan::r1_plan_refusals the_normalized_graph_is_the_ordering_authority
        PASS [   0.057s] (1557/5472) aion-fleet-plan::r3_conclusion_verifier conclusion_malformed_specimen_1_token_plus_trailing_garbage
        PASS [   0.064s] (1558/5472) aion-fleet-plan::r3_conclusion_verifier a_declared_token_verifies_from_the_exact_commit
        PASS [   0.061s] (1559/5472) aion-fleet-plan::r3_conclusion_verifier conclusion_missing_when_the_path_is_a_tree_not_a_blob
        PASS [   0.062s] (1560/5472) aion-fleet-plan::r3_conclusion_verifier conclusion_missing_when_the_path_is_absent_at_the_exact_commit
        PASS [   0.082s] (1561/5472) aion-fleet-plan::r3_conclusion_verifier a_refusal_carries_every_field_the_design_names
        PASS [   0.078s] (1562/5472) aion-fleet-plan::r3_conclusion_verifier conclusion_malformed_specimen_2_no_lf
        PASS [   0.015s] (1563/5472) aion-fleet-plan::r3_conclusion_verifier the_three_cause_codes_are_exactly_the_designs_three
        PASS [   0.094s] (1564/5472) aion-fleet-plan::r3_conclusion_verifier conclusion_malformed_specimen_3_two_lines
        PASS [   0.018s] (1565/5472) aion-fleet-plan::r3_conclusion_verifier the_verifier_refuses_a_ref_name_before_any_git_runs
        PASS [   0.058s] (1566/5472) aion-fleet-plan::r3_conclusion_verifier measured_red_verifies_as_a_success_conclusion
        PASS [   0.042s] (1567/5472) aion-fleet-plan::r3_conclusion_verifier the_verifier_refuses_an_unsafe_stage_id_before_deriving_a_path
        PASS [   0.077s] (1568/5472) aion-fleet-plan::r3_conclusion_verifier conclusion_undeclared_when_the_token_is_outside_the_closed_vocabulary
        PASS [   0.061s] (1569/5472) aion-fleet-plan::r6_replay_determinism a_successor_uses_the_recorded_accepted_sha_never_a_ref
        PASS [   0.094s] (1570/5472) aion-fleet-plan::r3_conclusion_verifier the_verifier_reads_the_commit_it_was_given_not_the_current_branch
        PASS [   0.025s] (1571/5472) aion-fleet-plan::r8_exemplar r4_a_failing_stage_dispatches_nothing_downstream
        PASS [   0.073s] (1572/5472) aion-fleet-plan::r6_replay_determinism no_pattern_kill_anywhere
        PASS [   0.104s] (1573/5472) aion-fleet-plan::r6_replay_determinism a_skipped_stage_creates_no_base
        PASS [   0.020s] (1574/5472) aion-fleet-plan::r8_exemplar r4_no_hold_approves_a_stage_whose_conclusion_has_not_verified
        PASS [   0.040s] (1575/5472) aion-fleet-plan::r8_exemplar r4_a_refused_conclusion_accepts_no_sha_and_runs_nothing_downstream
        PASS [   0.013s] (1576/5472) aion-fleet-plan::r8_exemplar r4_the_cause_is_never_unplanned_and_is_never_defaulted
        PASS [   0.015s] (1577/5472) aion-fleet-plan::r8_exemplar r5_the_base_law_all_five_clauses
        PASS [   0.023s] (1578/5472) aion-fleet-plan::r8_exemplar r5_the_condition_is_the_normalized_graphs_canonical_text_not_plan_prose
        PASS [   0.022s] (1579/5472) aion-fleet-plan::r8_exemplar r8a_a_plan_without_a_block_takes_the_legacy_path
        PASS [   0.106s] (1580/5472) aion-fleet-plan::r6_replay_determinism the_decision_function_is_pure_over_the_recorded_ledger
        PASS [   0.016s] (1581/5472) aion-integration-acp harness::tests::a_job_that_names_no_tree_is_refused_before_any_spawn
        PASS [   0.020s] (1582/5472) aion-integration-acp harness::tests::a_relative_additional_directory_is_refused
        PASS [   0.021s] (1583/5472) aion-integration-acp harness::tests::a_relative_cwd_is_refused_as_a_terminal_configuration_error
        PASS [   0.219s] (1584/5472) aion-fleet-plan::r3_conclusion_verifier the_vocabulary_comparison_is_byte_exact_not_prose
        PASS [   0.098s] (1585/5472) aion-fleet-plan::r8_exemplar r8a_check_plan_exit_by_exit
        PASS [   0.017s] (1586/5472) aion-integration-acp harness::tests::a_relative_directory_from_the_job_is_refused_the_same_way
        PASS [   0.090s] (1587/5472) aion-fleet-plan::r8_exemplar r8c_l3_is_skipped_with_no_worktree_cut_and_no_seat_spent
        PASS [   0.021s] (1588/5472) aion-integration-acp harness::tests::absolute_paths_pass_validation
        PASS [   0.098s] (1589/5472) aion-fleet-plan::r8_exemplar r8c_measured_red_verifies_as_success_and_l2_runs_from_l1s_accepted_sha
        PASS [   0.022s] (1590/5472) aion-integration-acp harness::tests::each_job_roots_its_session_in_the_tree_that_job_named
        PASS [   0.017s] (1591/5472) aion-integration-acp harness::tests::removed_env_vars_are_expressed_as_child_removals
        PASS [   0.017s] (1592/5472) aion-integration-acp harness::tests::the_child_runs_in_the_session_cwd
        PASS [   0.016s] (1593/5472) aion-integration-acp harness::tests::the_prompt_is_read_through_the_same_workspace_that_places_the_agent
        PASS [   0.012s] (1594/5472) aion-integration-acp messages::tests::a_changed_message_id_closes_the_previous_message_durably
        PASS [   0.131s] (1595/5472) aion-fleet-plan::r8_exemplar r8b_happy_path_three_stages_each_on_the_predecessors_accepted_sha
        PASS [   0.015s] (1596/5472) aion-integration-acp harness::tests::the_registration_set_is_the_negotiated_set
        PASS [   0.024s] (1597/5472) aion-integration-acp harness::tests::the_configured_command_and_args_are_passed_through_verbatim
        PASS [   0.018s] (1598/5472) aion-integration-acp harness::tests::the_published_set_is_exactly_what_the_adapter_can_translate
        PASS [   0.014s] (1599/5472) aion-integration-acp messages::tests::a_dropped_message_id_does_not_fabricate_a_boundary
        PASS [   0.362s] (1600/5472) aion-fleet-plan::r3_conclusion_verifier conclusion_malformed_further_byte_shapes
        PASS [   0.017s] (1601/5472) aion-integration-acp messages::tests::a_first_untagged_chunk_does_not_split_a_later_tagged_one
        PASS [   0.017s] (1602/5472) aion-integration-acp messages::tests::a_new_user_message_id_closes_the_previous_user_turn
        PASS [   0.018s] (1603/5472) aion-integration-acp messages::tests::a_non_text_chunk_survives_verbatim_and_is_not_appended
        PASS [   0.008s] (1604/5472) aion-integration-acp messages::tests::each_agent_chunk_yields_one_ephemeral_delta
        PASS [   0.027s] (1605/5472) aion-integration-acp messages::tests::a_non_text_thought_or_user_block_is_recorded_under_its_own_stream
        PASS [   0.012s] (1606/5472) aion-integration-acp messages::tests::finish_closes_the_open_message_and_is_idempotent
        PASS [   0.028s] (1607/5472) aion-integration-acp messages::tests::a_streamed_user_turn_becomes_one_durable_message
        PASS [   0.021s] (1608/5472) aion-integration-acp messages::tests::a_tool_only_turn_has_no_text_or_final_message
        PASS [   0.008s] (1609/5472) aion-integration-acp messages::tests::three_answer_messages_keep_the_last_message_distinct
        PASS [   0.022s] (1610/5472) aion-integration-acp messages::tests::reasoning_streams_live_and_survives_durably
        PASS [   0.011s] (1611/5472) aion-integration-acp messages::tests::two_completed_messages_are_joined_by_a_newline
        PASS [   0.024s] (1612/5472) aion-integration-acp messages::tests::the_three_streams_do_not_interleave_into_each_other
        PASS [   0.030s] (1613/5472) aion-integration-acp messages::tests::reasoning_never_enters_the_runs_result
        PASS [   0.021s] (1614/5472) aion-integration-acp messages::tests::untagged_chunks_accumulate_as_one_message
        PASS [   0.016s] (1615/5472) aion-integration-acp outcome::tests::a_response_with_neither_result_nor_error_is_a_protocol_error
        PASS [   0.024s] (1616/5472) aion-integration-acp outcome::tests::a_request_cancelled_error_is_reported_as_a_cancellation
        PASS [   0.013s] (1617/5472) aion-integration-acp outcome::tests::an_ordinary_error_response_surfaces_its_code_and_message
        PASS [   0.023s] (1618/5472) aion-integration-acp outcome::tests::a_result_that_is_not_a_prompt_response_is_a_protocol_error
        PASS [   0.018s] (1619/5472) aion-integration-acp outcome::tests::cancelled_is_distinguishable_from_any_other_failure
        PASS [   0.030s] (1620/5472) aion-integration-acp outcome::tests::an_auth_required_error_names_the_authentication_it_needs
        PASS [   0.020s] (1621/5472) aion-integration-acp outcome::tests::end_turn_yields_the_canonical_agent_outcome_record
        PASS [   0.010s] (1622/5472) aion-integration-acp permission::tests::allow_once_never_widens_to_allow_always
        PASS [   0.013s] (1623/5472) aion-integration-acp outcome::tests::refusal_is_never_reported_as_success
        PASS [   0.030s] (1624/5472) aion-integration-acp outcome::tests::an_enormous_partial_answer_is_bounded_with_an_explicit_marker
        PASS [   0.011s] (1625/5472) aion-integration-acp permission::tests::allow_once_selects_the_allow_once_option
        PASS [   0.028s] (1626/5472) aion-integration-acp outcome::tests::each_non_completing_stop_reason_is_a_failure_naming_itself
        PASS [   0.012s] (1627/5472) aion-integration-acp permission::tests::an_empty_option_list_still_produces_an_answer
        PASS [   0.012s] (1628/5472) aion-integration-acp permission::tests::deny_selects_the_reject_once_option
        PASS [   0.008s] (1629/5472) aion-integration-acp protocol::tests::method_names_match_the_published_wire_strings
        PASS [   0.020s] (1630/5472) aion-integration-acp protocol::tests::advertised_client_capabilities_are_all_false
        PASS [   0.017s] (1631/5472) aion-integration-acp protocol::tests::error_codes_match_the_specified_numbers
        PASS [   0.026s] (1632/5472) aion-integration-acp permission::tests::deny_falls_back_to_reject_always_when_no_reject_once_is_offered
        PASS [   0.027s] (1633/5472) aion-integration-acp permission::tests::each_policy_has_a_distinct_audit_label
        PASS [   0.029s] (1634/5472) aion-integration-acp permission::tests::deny_cancels_when_no_reject_option_is_offered
        PASS [   0.026s] (1635/5472) aion-integration-acp protocol::tests::client_info_names_this_crate_and_its_real_version
        PASS [   0.014s] (1636/5472) aion-integration-acp protocol::tests::protocol_version_is_the_integer_one
        PASS [   0.010s] (1637/5472) aion-integration-acp translate::tests::a_failed_tool_call_update_marks_the_result_as_an_error
        PASS [   0.016s] (1638/5472) aion-integration-acp translate::tests::a_cancelled_decision_is_recorded_as_cancelled
        PASS [   0.011s] (1639/5472) aion-integration-acp translate::tests::a_plan_reaches_the_transcript_with_all_its_entries
        PASS [   0.020s] (1640/5472) aion-integration-acp translate::tests::a_completed_tool_call_update_yields_a_tool_result
        PASS [   0.017s] (1641/5472) aion-integration-acp translate::tests::a_refused_peer_request_is_recorded_with_its_code
        PASS [   0.024s] (1642/5472) aion-integration-acp translate::tests::a_non_terminal_tool_call_update_is_the_verbatim_frame_alone
        PASS [   0.021s] (1643/5472) aion-integration-acp translate::tests::a_permission_request_and_its_decision_are_both_recorded
        PASS [   0.026s] (1644/5472) aion-integration-acp translate::tests::a_mismatched_protocol_version_is_refused_naming_both_versions
        PASS [   0.016s] (1645/5472) aion-integration-acp translate::tests::an_agent_message_chunk_streams_an_ephemeral_delta
        PASS [   0.019s] (1646/5472) aion-integration-acp translate::tests::a_tool_call_yields_the_classified_event_and_the_verbatim_frame
        PASS [   0.025s] (1647/5472) aion-integration-acp translate::tests::a_user_message_chunk_accumulates_into_a_durable_user_message
        PASS [   0.016s] (1648/5472) aion-integration-acp translate::tests::an_agent_thought_chunk_streams_a_note_and_survives_durably
        PASS [   0.016s] (1649/5472) aion-integration-acp translate::tests::every_stop_reason_maps_to_a_kind_and_a_wire_label
        PASS [   0.009s] (1650/5472) aion-integration-acp translate::tests::the_matching_protocol_version_passes_the_gate
        PASS [   0.015s] (1651/5472) aion-integration-acp translate::tests::stamping_fills_the_run_identity_and_the_acp_attribution
        PASS [   0.027s] (1652/5472) aion-integration-acp translate::tests::only_end_turn_is_a_completed_run
        PASS [   0.027s] (1653/5472) aion-integration-acp translate::tests::only_cancel_is_advertised
        PASS [   0.012s] (1654/5472) aion-integration-acp translate::tests::the_remaining_variants_each_reach_the_transcript_verbatim
        PASS [   0.014s] (1655/5472) aion-integration-acp wire_tests::handshake_and_streams::a_multi_chunk_user_turn_is_still_one_durable_message
        PASS [   0.016s] (1656/5472) aion-integration-acp translate::tests::the_terminal_response_is_recorded_verbatim_with_its_meta
        PASS [   0.018s] (1657/5472) aion-integration-acp wire_tests::handshake_and_streams::a_failed_tool_call_update_is_an_error_result
        PASS [   0.010s] (1658/5472) aion-integration-acp wire_tests::handshake_and_streams::an_agent_message_chunk_streams_a_delta_and_closes_a_durable_message
        PASS [   0.009s] (1659/5472) aion-integration-acp wire_tests::handshake_and_streams::an_agent_thought_chunk_streams_live_and_survives_durably
        PASS [   0.017s] (1660/5472) aion-integration-acp wire_tests::handshake_and_streams::a_user_message_chunk_becomes_one_durable_user_message
        PASS [   0.021s] (1661/5472) aion-integration-acp wire_tests::handshake_and_streams::a_plan_reaches_the_transcript_with_every_entry
        PASS [   0.023s] (1662/5472) aion-integration-acp wire_tests::handshake_and_streams::a_tool_call_and_its_completion_become_a_call_and_a_result
        PASS [   0.410s] (1663/5472) aion-fleet-plan::r6_replay_determinism crash_and_resume_at_every_one_of_the_seven_boundaries
        PASS [   0.023s] (1664/5472) aion-integration-acp wire_tests::handshake_and_streams::a_version_the_adapter_cannot_speak_is_refused_before_a_session_exists
        PASS [   0.019s] (1665/5472) aion-integration-acp wire_tests::handshake_and_streams::the_handshake_negotiates_version_one_and_opens_a_session
        PASS [   0.021s] (1666/5472) aion-integration-acp wire_tests::handshake_and_streams::the_six_remaining_update_variants_each_reach_the_transcript
        PASS [   0.025s] (1667/5472) aion-integration-acp wire_tests::handshake_and_streams::reasoning_never_becomes_the_activitys_result
        PASS [   0.024s] (1668/5472) aion-integration-acp wire_tests::handshake_and_streams::the_initialize_request_sends_the_integer_version_and_all_false_capabilities
        PASS [   0.042s] (1669/5472) aion-integration-acp wire_tests::handshake_and_streams::an_unrecognised_update_variant_still_reaches_the_transcript
        PASS [   0.029s] (1670/5472) aion-integration-acp wire_tests::outcomes_and_permissions::a_cancelled_stop_reason_is_distinguishable_from_a_failure
        PASS [   0.014s] (1671/5472) aion-integration-acp wire_tests::outcomes_and_permissions::an_unadvertised_primitive_is_gated_before_anything_is_written
        PASS [   0.032s] (1672/5472) aion-integration-acp wire_tests::outcomes_and_permissions::an_inbound_fs_read_gets_an_error_response_rather_than_silence
        PASS [   0.035s] (1673/5472) aion-integration-acp wire_tests::outcomes_and_permissions::a_permission_request_arriving_after_a_cancel_is_answered_cancelled
        PASS [   0.027s] (1674/5472) aion-integration-acp wire_tests::outcomes_and_permissions::an_inbound_terminal_create_is_refused_the_same_way
        PASS [   0.029s] (1675/5472) aion-integration-acp wire_tests::outcomes_and_permissions::end_turn_is_the_completed_run_and_carries_the_agents_text
        PASS [   0.031s] (1676/5472) aion-integration-acp wire_tests::outcomes_and_permissions::cancelling_sends_session_cancel_and_ends_the_turn_as_cancelled
        PASS [   0.017s] (1677/5472) aion-integration-acp wire_tests::raw_and_resume::a_line_that_is_not_a_jsonrpc_frame_does_not_destroy_the_turn
        PASS [   0.023s] (1678/5472) aion-integration-acp wire_tests::outcomes_and_permissions::the_allow_once_policy_selects_allow_once_and_records_the_decision
        PASS [   0.027s] (1679/5472) aion-integration-acp wire_tests::outcomes_and_permissions::the_deny_policy_selects_a_reject_option_and_records_the_decision
        PASS [   0.022s] (1680/5472) aion-integration-acp wire_tests::raw_and_resume::a_prior_session_and_a_load_capable_agent_resume_via_session_load
        PASS [   0.033s] (1681/5472) aion-integration-acp wire_tests::outcomes_and_permissions::refusal_and_the_two_limits_are_failures_naming_themselves
        PASS [   0.022s] (1682/5472) aion-integration-acp wire_tests::raw_and_resume::asking_to_resume_an_agent_that_cannot_load_refuses_and_mints_nothing
        PASS [   0.008s] (1683/5472) aion-integration-acp wire_tests::terminal_and_pump::the_driver_contract_completes_against_an_agent_that_stays_alive
        PASS [   0.010s] (1684/5472) aion-integration-acp wire_tests::terminal_and_pump::the_agent_closing_the_channel_is_a_transport_error_not_a_hang
        PASS [   0.019s] (1685/5472) aion-integration-acp wire_tests::terminal_and_pump::an_undecodable_terminal_result_still_emits_a_terminal_boundary
        PASS [   0.022s] (1686/5472) aion-integration-acp wire_tests::terminal_and_pump::a_notification_read_during_the_handshake_reaches_the_transcript
        PASS [   0.007s] (1687/5472) aion-integration-acp::declared_environment_e2e a_bare_command_with_no_path_declared_is_refused_naming_path
        PASS [   0.026s] (1688/5472) aion-integration-acp wire_tests::terminal_and_pump::a_failed_terminal_frame_still_emits_a_terminal_boundary
        PASS [   0.018s] (1689/5472) aion-integration-acp::acp_live_agent a_real_acp_agent_completes_a_turn
        PASS [   0.029s] (1690/5472) aion-integration-acp wire_tests::terminal_and_pump::answering_a_flood_of_peer_requests_does_not_stall_the_read_loop
        PASS [   0.013s] (1691/5472) aion-integration-cli demux::tests::a_plain_log_line_falls_through_to_raw_verbatim
        PASS [   0.014s] (1692/5472) aion-integration-cli demux::tests::a_json_object_with_unknown_type_is_raw_but_keeps_structure
        PASS [   0.028s] (1693/5472) aion-integration-acp wire_tests::terminal_and_pump::the_terminal_response_is_recorded_verbatim_on_the_transcript
        PASS [   0.021s] (1694/5472) aion-integration-cli demux::tests::a_bare_json_value_is_raw_not_mapped
        PASS [   0.025s] (1695/5472) aion-integration-acp::declared_environment_e2e a_declared_environment_keeps_claudecode_out_of_the_agent_child
        PASS [   0.025s] (1696/5472) aion-integration-acp::declared_environment_e2e without_a_declaration_the_child_still_inherits_which_is_what_makes_the_pin_meaningful
        PASS [   0.008s] (1697/5472) aion-integration-cli demux::tests::tool_result_line_maps_to_tool_result_event
        PASS [   0.020s] (1698/5472) aion-integration-cli demux::tests::message_line_maps_to_message_event
        PASS [   0.013s] (1699/5472) aion-integration-cli demux::tests::tool_call_line_maps_to_tool_call_event
        PASS [   0.013s] (1700/5472) aion-integration-cli demux::tests::unknown_stop_reason_falls_through_to_other
        PASS [   0.023s] (1701/5472) aion-integration-cli demux::tests::stop_line_maps_stop_reasons
        PASS [   0.015s] (1702/5472) aion-integration-cli mock::tests::accepts_an_advertised_command_and_records_it
        PASS [   0.012s] (1703/5472) aion-integration-cli mock::tests::rejects_respond_to_approval_as_capability_not_supported
        PASS [   0.013s] (1704/5472) aion-integration-cli mock::tests::advertises_inject_message_and_cancel_only
        PASS [   0.011s] (1705/5472) aion-integration-cli session::tests::a_run_that_announced_no_stop_falls_back_to_end_turn
        PASS [   0.021s] (1706/5472) aion-integration-cli demux::tests::worker_seq_is_carried_onto_the_event
        PASS [   0.015s] (1707/5472) aion-integration-cli mock::tests::streams_a_transcript_and_returns_a_result
        PASS [   0.008s] (1708/5472) aion-integration-cli session::tests::intervene_rejects_every_command_as_capability_not_supported
        PASS [   0.011s] (1709/5472) aion-integration-cli session::tests::capabilities_are_always_empty
        PASS [   0.009s] (1710/5472) aion-integration-cli session::tests::stdout_lines_demux_into_a_live_event_stream
        PASS [   0.017s] (1711/5472) aion-integration-cli session::tests::an_event_is_never_the_result_the_two_are_separate_channels
        PASS [   0.016s] (1712/5472) aion-integration-cli session::tests::the_announced_stop_reason_reaches_the_outcome_record
        PASS [   0.010s] (1713/5472) aion-integration-norn advice::tests::a_hand_written_workers_spelling_is_carried_verbatim_too
        PASS [   0.016s] (1714/5472) aion-integration-norn advice::tests::a_capability_rejection_is_never_rewritten
        PASS [   0.019s] (1715/5472) aion-integration-cli::cli_observability_e2e cli_harness_streams_a_live_transcript_with_no_controls
        PASS [   0.019s] (1716/5472) aion-integration-cli::mock_intervention_e2e drives_inject_and_cancel_end_to_end_and_rejects_respond_to_approval
        PASS [   0.022s] (1717/5472) aion-integration-cli::cli_observability_e2e cli_harness_reports_a_non_zero_exit_as_a_harness_failure
        PASS [   0.013s] (1718/5472) aion-integration-norn advice::tests::a_refusal_at_session_open_is_advised
        PASS [   0.025s] (1719/5472) aion-integration-cli session::tests::the_last_announced_stop_wins
        PASS [   0.029s] (1720/5472) aion-integration-cli::cli_observability_e2e a_signal_killed_agent_is_not_reported_as_choosing_to_stop
        PASS [   0.014s] (1721/5472) aion-integration-norn advice::tests::a_refusal_at_the_terminal_result_is_advised
        PASS [   0.012s] (1722/5472) aion-integration-norn advice::tests::the_advice_is_withheld_once_the_opt_in_was_given
        PASS [   0.013s] (1723/5472) aion-integration-norn advice::tests::the_composed_argv_is_what_decides_the_opt_in
        PASS [   0.016s] (1724/5472) aion-integration-norn advice::tests::the_advice_names_the_deployments_own_opt_in_and_states_the_tradeoff
        PASS [   0.021s] (1725/5472) aion-integration-norn advice::tests::a_successful_run_forwards_its_payload
        PASS [   0.008s] (1726/5472) aion-integration-norn advice::tests::unrelated_failures_are_passed_through_unadvised
        PASS [   0.020s] (1727/5472) aion-integration-norn advice::tests::once_the_opt_in_is_given_the_advice_goes_quiet
        PASS [   0.017s] (1728/5472) aion-integration-norn advice::tests::the_recorded_refusal_is_recognised
        PASS [   0.016s] (1729/5472) aion-integration-norn advice::tests::unrelated_failures_are_not_mistaken_for_it
        PASS [   0.025s] (1730/5472) aion-integration-norn advice::tests::a_transport_refusal_is_advised_too
        PASS [   0.009s] (1731/5472) aion-integration-norn claim_area::tests::home_resolution_is_loud_and_matches_the_server_chain
        PASS [   0.015s] (1732/5472) aion-integration-norn advice::tests::unrelated_failures_pass_through_untouched
        PASS [   0.018s] (1733/5472) aion-integration-norn claim_area::tests::claim_path_probe
        PASS [   0.019s] (1734/5472) aion-integration-norn claim_area::tests::key_is_sha256_hex_of_only_the_canonical_path
        PASS [   0.022s] (1735/5472) aion-integration-norn claim_area::tests::aliases_and_restarts_derive_one_stable_claim_directory
        PASS [   0.028s] (1736/5472) aion-integration-norn harness::tests::a_cwd_that_is_not_there_is_refused_before_the_spawn_can_blame_the_binary
        PASS [   0.021s] (1737/5472) aion-integration-norn harness::tests::a_job_that_names_no_tree_is_refused_before_any_spawn
        PASS [   0.028s] (1738/5472) aion-integration-norn harness::tests::a_declared_environment_rides_the_previewable_invocation
        PASS [   0.017s] (1739/5472) aion-integration-norn harness::tests::a_relative_cwd_is_refused_as_a_terminal_configuration_error
        PASS [   0.018s] (1740/5472) aion-integration-norn harness::tests::a_relative_tree_from_the_job_is_refused_the_same_way
        PASS [   0.007s] (1741/5472) aion-integration-norn harness::tests::expands_both_placeholders_amid_literal_text
        PASS [   0.020s] (1742/5472) aion-integration-norn harness::tests::an_absolute_cwd_that_exists_is_accepted
        PASS [   0.010s] (1743/5472) aion-integration-norn harness::tests::argument_without_placeholders_passes_through_unchanged
        PASS [   0.014s] (1744/5472) aion-integration-norn harness::tests::each_job_runs_in_the_tree_that_job_named
        PASS [   0.018s] (1745/5472) aion-integration-norn harness::tests::template_args_expand_and_literal_args_preserve_reserved_substrings
        PASS [   0.017s] (1746/5472) aion-integration-norn harness::tests::the_child_runs_in_the_declared_cwd
        PASS [   0.010s] (1747/5472) aion-integration-norn harness::tests::the_preview_carries_the_declared_cwd
        PASS [   0.016s] (1748/5472) aion-integration-norn harness::tests::the_invocation_preview_is_the_spawned_reality
        PASS [   0.025s] (1749/5472) aion-integration-norn harness::tests::removed_env_vars_are_expressed_as_child_removals
        PASS [   0.016s] (1750/5472) aion-integration-norn harness::tests::the_published_set_is_exactly_what_the_adapter_can_translate
        PASS [   0.015s] (1751/5472) aion-integration-norn session::tests::a_notification_is_never_captured_as_the_result
        PASS [   0.018s] (1752/5472) aion-integration-norn harness::tests::unrecognised_braced_text_is_literal
        PASS [   0.028s] (1753/5472) aion-integration-norn harness::tests::the_prompt_is_read_through_the_same_workspace_that_places_the_agent
        PASS [   0.034s] (1754/5472) aion-integration-norn session::tests::capabilities_are_the_negotiated_set
        PASS [   0.026s] (1755/5472) aion-integration-norn session::tests::envelope_compaction::one_turn_costs_about_one_envelope_not_three
        PASS [   0.031s] (1756/5472) aion-integration-norn session::tests::envelope_compaction::one_turn_persists_its_instructions_block_exactly_once
        PASS [   0.044s] (1757/5472) aion-integration-norn session::tests::envelope_compaction::compaction_leaves_the_worker_sequence_and_identity_untouched
        PASS [   0.020s] (1758/5472) aion-integration-norn session::tests::intervene_rejects_an_unsupported_primitive_without_sending
        PASS [   0.018s] (1759/5472) aion-integration-norn session::tests::intervene_sends_request_and_awaits_the_peer_ack
        PASS [   0.008s] (1760/5472) aion-integration-norn session::tests::live_events_stream_from_peer_notifications
        PASS [   0.033s] (1761/5472) aion-integration-norn session::tests::intervene_surfaces_a_peer_error_ack
        PASS [   0.022s] (1762/5472) aion-integration-norn session::tests::outcomes::a_completed_structured_output_is_refused_naming_the_agent_outcome_contract
        PASS [   0.058s] (1763/5472) aion-integration-norn session::tests::envelope_compaction::the_frames_after_a_turns_first_are_persisted_as_deltas
        PASS [   0.019s] (1764/5472) aion-integration-norn session::tests::outcomes::completed_envelope_missing_the_output_key_is_a_protocol_error
        PASS [   0.027s] (1765/5472) aion-integration-norn session::tests::outcomes::a_response_with_neither_result_nor_error_is_a_protocol_error
        PASS [   0.015s] (1766/5472) aion-integration-norn session::tests::outcomes::completed_string_output_is_the_canonical_agent_outcome_record
        PASS [   0.019s] (1767/5472) aion-integration-norn session::tests::outcomes::each_non_completed_stop_reason_surfaces_with_its_detail_and_partial_verbatim
        PASS [   0.017s] (1768/5472) aion-integration-norn session::tests::outcomes::measured_norn_policy_phrase_is_the_narrow_textual_fallback
        PASS [   0.029s] (1769/5472) aion-integration-norn session::tests::outcomes::completed_envelope_with_present_null_output_is_refused_naming_null
        PASS [   0.024s] (1770/5472) aion-integration-norn session::tests::outcomes::non_envelope_result_is_a_protocol_error_never_a_silent_passthrough
        PASS [   0.019s] (1771/5472) aion-integration-norn session::tests::outcomes::null_result_is_the_slash_command_harness_error
        PASS [   0.013s] (1772/5472) aion-integration-norn session::tests::outcomes::structured_provider_policy_refusal_has_its_own_class
        PASS [   0.026s] (1773/5472) aion-integration-norn session::tests::outcomes::ordinary_generic_json_rpc_failure_remains_harness_retryable
        PASS [   0.033s] (1774/5472) aion-integration-norn session::tests::wait_result_returns_the_id_matched_run_execute_response_payload
        PASS [   0.021s] (1775/5472) aion-integration-norn translate::tests::cancel_maps_to_cancel_request
        PASS [   0.058s] (1776/5472) aion-integration-norn spawn_guard::tests::matching_live_incarnation_refuses_and_tree_mismatch_is_loud_and_preserved
        PASS [   0.050s] (1777/5472) aion-integration-norn spawn_guard::tests::same_execution_reattaches_without_deleting_the_live_marker
        PASS [   0.016s] (1778/5472) aion-integration-norn translate::tests::different_protocol_version_is_a_protocol_error_naming_expected_and_received
        PASS [   0.016s] (1779/5472) aion-integration-norn translate::tests::inject_interrupt_maps_to_inject_message_request
        PASS [   0.010s] (1780/5472) aion-integration-norn translate::tests::inject_normal_maps_to_normal_priority
        PASS [   0.014s] (1781/5472) aion-integration-norn translate::tests::message_notification_maps_to_message_event
        PASS [   0.082s] (1782/5472) aion-integration-norn spawn_guard::tests::live_exclusion_drop_release_and_per_tree_scope_are_preserved
        PASS [   0.011s] (1783/5472) aion-integration-norn translate::tests::missing_protocol_field_is_a_protocol_error_naming_expected_and_received
        PASS [   0.007s] (1784/5472) aion-integration-norn translate::tests::outcomes::a_completed_envelope_maps_to_the_canonical_end_turn_stop_reason
        PASS [   0.096s] (1785/5472) aion-integration-norn spawn_guard::tests::dead_and_pid_reused_incarnations_are_cleaned_at_the_new_location
        PASS [   0.011s] (1786/5472) aion-integration-norn translate::tests::non_string_protocol_field_is_reported_as_the_value_received
        PASS [   0.019s] (1787/5472) aion-integration-norn translate::tests::missing_agent_id_falls_back_to_nil_never_panics
        PASS [   0.011s] (1788/5472) aion-integration-norn translate::tests::outcomes::a_completed_non_string_output_is_a_contract_refusal_naming_what_was_found
        PASS [   0.015s] (1789/5472) aion-integration-norn translate::tests::outcomes::a_completed_envelope_without_a_session_id_is_a_contract_refusal_naming_no_session
        LEAK [   0.271s] (1790/5472) aion-integration-norn harness::tests::a_foreign_live_sibling_collision_is_retryable_not_terminal
        PASS [   0.012s] (1791/5472) aion-integration-norn translate::tests::outcomes::a_non_string_session_id_is_a_protocol_refusal
        PASS [   0.025s] (1792/5472) aion-integration-norn translate::tests::outcomes::a_completed_envelope_without_the_output_key_is_a_protocol_error
        PASS [   0.019s] (1793/5472) aion-integration-norn translate::tests::outcomes::a_present_null_output_is_distinct_from_a_missing_key_and_refused_as_null
        PASS [   0.012s] (1794/5472) aion-integration-norn translate::tests::outcomes::a_stop_without_a_string_reason_is_a_protocol_error
        PASS [   0.013s] (1795/5472) aion-integration-norn translate::tests::outcomes::a_result_without_envelope_version_is_a_protocol_error_naming_it
        PASS [   0.015s] (1796/5472) aion-integration-norn translate::tests::outcomes::an_empty_string_session_id_is_a_protocol_refusal
        PASS [   0.018s] (1797/5472) aion-integration-norn translate::tests::outcomes::a_result_without_stop_is_a_protocol_error_naming_it
        PASS [   0.015s] (1798/5472) aion-integration-norn translate::tests::outcomes::an_enormous_partial_output_is_bounded_with_an_explicit_marker
        PASS [   0.018s] (1799/5472) aion-integration-norn translate::tests::outcomes::non_completed_stop_with_null_output_carries_no_partial_label
        PASS [   0.018s] (1800/5472) aion-integration-norn translate::tests::outcomes::completed_string_output_becomes_the_agent_outcome_record
        PASS [   0.015s] (1801/5472) aion-integration-norn translate::tests::parse_capabilities_ignores_unknown_labels
        PASS [   0.022s] (1802/5472) aion-integration-norn translate::tests::outcomes::non_completed_stop_is_a_harness_error_carrying_the_stop_object_and_partial
        PASS [   0.021s] (1803/5472) aion-integration-norn translate::tests::parse_capabilities_missing_array_is_empty_set
        PASS [   0.013s] (1804/5472) aion-integration-norn translate::tests::parse_capabilities_reads_norns_advertised_set
        LEAK [   0.307s] (1805/5472) aion-integration-norn harness::tests::a_live_self_occupancy_is_adopted
        PASS [   0.011s] (1806/5472) aion-integration-norn translate::tests::stop_notification_maps_stop_reasons
        PASS [   0.017s] (1807/5472) aion-integration-norn translate::tests::raw_non_delta_notification_stays_durable
        PASS [   0.010s] (1808/5472) aion-integration-norn translate::tests::unclassifiable_notification_falls_through_to_raw
        PASS [   0.022s] (1809/5472) aion-integration-norn translate::tests::raw_per_token_delta_notification_is_ephemeral_and_drops_its_payload
        PASS [   0.014s] (1810/5472) aion-integration-norn translate::tests::tool_call_delta_notification_is_ephemeral
        PASS [   0.013s] (1811/5472) aion-integration-norn translate::tests::tool_call_notification_maps_to_tool_call_event
        PASS [   0.020s] (1812/5472) aion-integration-norn translate::tests::text_delta_notification_maps_to_ephemeral_delta
        PASS [   0.011s] (1813/5472) aion-integration-norn translate::tests::unknown_stop_reason_falls_through_to_other
        PASS [   0.019s] (1814/5472) aion-integration-norn translate::tests::tool_result_notification_maps_to_tool_result_event
        PASS [   0.009s] (1815/5472) aion-integration-norn::declared_environment_e2e a_binary_on_path_with_no_path_declared_is_refused_naming_path
        PASS [   0.013s] (1816/5472) aion-integration-norn translate::tests::unsupported_primitives_are_capability_not_supported_and_never_build_a_request
        LEAK [   0.308s] (1817/5472) aion-integration-norn harness::tests::stale_dead_sibling_marker_is_cleaned_before_spawn
        PASS [   0.018s] (1818/5472) aion-integration-norn translate::tests::usage_estimate_notification_maps_to_progress
        PASS [   0.012s] (1819/5472) aion-integrations envelope_delta::tests::a_base_that_was_altered_after_the_delta_was_built_is_refused_not_patched
        PASS [   0.021s] (1820/5472) aion-integration-norn::cross_process_reattach_e2e replacement_worker_reattaches_the_surviving_norn_session
        PASS [   0.009s] (1821/5472) aion-integrations envelope_delta::tests::a_delta_from_a_future_format_version_is_refused_by_name
        PASS [   0.016s] (1822/5472) aion-integration-norn::norn_e2e norn_harness_drives_a_real_run_end_to_end
        PASS [   0.008s] (1823/5472) aion-integrations envelope_delta::tests::a_frame_whose_delta_would_not_be_smaller_is_persisted_in_full
        PASS [   0.018s] (1824/5472) aion-integrations envelope_delta::tests::a_delta_resolves_against_a_base_that_was_written_in_the_old_format
        PASS [   0.017s] (1825/5472) aion-integrations envelope_delta::tests::a_field_set_to_null_round_trips_as_null_rather_than_being_removed
        PASS [   0.019s] (1826/5472) aion-integrations envelope_delta::tests::a_field_that_changed_type_is_replaced_wholesale_not_merged
        PASS [   0.017s] (1827/5472) aion-integrations envelope_delta::tests::a_malformed_delta_document_is_refused_and_still_names_its_turn
        PASS [   0.025s] (1828/5472) aion-integrations envelope_delta::tests::a_delta_whose_base_was_never_seen_is_reported_and_never_guessed
        PASS [   0.017s] (1829/5472) aion-integrations envelope_delta::tests::a_removed_field_is_carried_as_a_pointer_and_restored_by_removal
        PASS [   0.012s] (1830/5472) aion-integrations envelope_delta::tests::an_ephemeral_frame_is_never_compacted_because_it_is_never_durable
        PASS [   0.057s] (1831/5472) aion-integration-norn::declared_environment_e2e a_declared_environment_keeps_claudecode_out_of_the_norn_child
        PASS [   0.030s] (1832/5472) aion-integrations envelope_delta::tests::a_new_turn_displaces_the_previous_turns_base_rather_than_accumulating
        PASS [   0.028s] (1833/5472) aion-integrations envelope_delta::tests::a_non_envelope_raw_value_and_a_non_raw_kind_are_both_left_alone
        PASS [   0.009s] (1834/5472) aion-integrations envelope_delta::tests::keys_containing_pointer_metacharacters_survive_the_round_trip
        PASS [   0.013s] (1835/5472) aion-integrations envelope_delta::tests::applying_a_patch_to_the_wrong_base_errors_instead_of_half_patching
        PASS [   0.038s] (1836/5472) aion-integrations envelope_delta::tests::a_turns_later_frames_compact_and_restore_to_the_exact_original_bytes
        PASS [   0.037s] (1837/5472) aion-integrations envelope_delta::tests::an_ephemeral_envelope_frame_cannot_displace_a_durable_turns_base
        PASS [   0.076s] (1838/5472) aion-integration-norn::declared_environment_e2e without_a_declaration_the_norn_child_still_inherits
        PASS [   0.015s] (1839/5472) aion-integrations environment::tests::a_blank_entry_is_refused_because_no_environment_can_carry_one
        PASS [   0.020s] (1840/5472) aion-integrations environment::tests::a_bare_program_without_path_in_the_declaration_is_refused_naming_path
        PASS [   0.021s] (1841/5472) aion-integrations environment::tests::a_bare_program_with_path_declared_is_permitted
        PASS [   2.032s] (1842/5472) aion-cli::bin/aion worker_shell::tests::cancellation_reaps_the_entire_spawned_process_group
        PASS [   0.035s] (1843/5472) aion-integrations envelope_delta::tests::the_repeated_instructions_block_is_persisted_once_per_turn
        PASS [   0.035s] (1844/5472) aion-integrations envelope_delta::tests::two_agents_in_one_attempt_do_not_displace_each_others_bases
        PASS [   0.022s] (1845/5472) aion-integrations environment::tests::a_variable_the_declaration_does_not_name_is_absent_in_the_child
        PASS [   0.027s] (1846/5472) aion-integrations environment::tests::a_declared_name_the_context_does_not_carry_is_reported_absent_not_invented
        PASS [   0.028s] (1847/5472) aion-integrations environment::tests::a_key_value_pair_is_refused_rather_than_split
        PASS [   0.025s] (1848/5472) aion-integrations environment::tests::a_relative_multi_component_program_is_not_gated_on_path
        PASS [   0.019s] (1849/5472) aion-integrations environment::tests::declaration_order_is_the_operators_and_duplicates_collapse_once
        PASS [   0.014s] (1850/5472) aion-integrations environment::tests::the_process_snapshot_really_reads_this_process
        PASS [   0.028s] (1851/5472) aion-integrations environment::tests::an_absolute_program_needs_no_path_so_a_hermetic_declaration_stays_legitimate
        PASS [   0.028s] (1852/5472) aion-integrations environment::tests::an_empty_declaration_renders_as_words_in_a_refusal
        PASS [   0.023s] (1853/5472) aion-integrations environment::tests::every_environment_fault_reaches_the_seam_as_a_deterministic_refusal
        PASS [   0.013s] (1854/5472) aion-integrations jsonrpc::envelope::tests::a_null_id_is_treated_as_a_notification_not_a_request
        PASS [   0.020s] (1855/5472) aion-integrations environment::tests::the_resolved_pairs_are_the_childs_entire_environment_not_an_overlay
        PASS [   0.021s] (1856/5472) aion-integrations error::tests::capability_not_supported_names_the_primitive
        PASS [   0.014s] (1857/5472) aion-integrations jsonrpc::envelope::tests::an_absent_result_decodes_as_none
        PASS [   0.010s] (1858/5472) aion-integrations jsonrpc::envelope::tests::error_response_carries_result_none
        PASS [   0.010s] (1859/5472) aion-integrations jsonrpc::envelope::tests::request_serialises_with_id_and_omits_absent_params
        PASS [   0.024s] (1860/5472) aion-integrations jsonrpc::envelope::tests::a_present_null_result_decodes_as_some_null
        PASS [   0.032s] (1861/5472) aion-integrations error::tests::each_constructor_renders_its_class
        PASS [   0.033s] (1862/5472) aion-integrations error::tests::deterministic_refusals_are_exactly_contract_and_configuration
        PASS [   0.032s] (1863/5472) aion-integrations error::tests::harness_error_is_send_sync_static
        PASS [   0.024s] (1864/5472) aion-integrations jsonrpc::envelope::tests::classifies_request_notification_and_response
        PASS [   0.011s] (1865/5472) aion-integrations jsonrpc::transport::tests::ids_are_monotonic
        PASS [   0.018s] (1866/5472) aion-integrations jsonrpc::envelope::tests::string_ids_round_trip
        PASS [   0.010s] (1867/5472) aion-integrations jsonrpc::transport::tests::recv_returns_none_at_end_of_stream
        PASS [   0.007s] (1868/5472) aion-integrations workspace::tests::a_bound_directory_is_subtracted_and_what_remains_is_the_prompt
        PASS [   0.010s] (1869/5472) aion-integrations workspace::tests::a_blank_directory_is_refused_rather_than_read_as_here
        PASS [   0.026s] (1870/5472) aion-integrations jsonrpc::envelope::tests::notification_never_carries_an_id
        PASS [   0.014s] (1871/5472) aion-integrations spec::tests::spec_carries_run_identity_and_input
        PASS [   0.017s] (1872/5472) aion-integrations spec::tests::two_generations_of_one_chain_are_distinct_specs
        PASS [   0.014s] (1873/5472) aion-integrations workspace::tests::a_json_string_input_is_the_prompt_verbatim
        PASS [   0.022s] (1874/5472) aion-integrations workspace::tests::a_fixed_tree_ignores_what_the_job_carries
        PASS [   0.019s] (1875/5472) aion-integrations workspace::tests::a_job_that_names_its_tree_resolves_to_that_tree
        PASS [   0.023s] (1876/5472) aion-integrations workspace::tests::a_job_missing_the_parameter_is_refused_and_never_defaulted
        PASS [   0.017s] (1877/5472) aion-integrations workspace::tests::a_prompt_field_that_is_not_text_is_refused_naming_what_arrived
        PASS [   0.006s] (1878/5472) aion-integrations workspace::tests::an_absent_reserved_parameter_changes_nothing
        PASS [   0.021s] (1879/5472) aion-integrations workspace::tests::a_one_field_object_asks_the_agent_the_field_and_not_the_json
        PASS [   0.020s] (1880/5472) aion-integrations workspace::tests::a_parameter_that_is_not_a_string_says_what_it_was
        PASS [   0.034s] (1881/5472) aion-integrations jsonrpc::transport::tests::request_and_notification_round_trip_over_a_duplex
        PASS [   0.019s] (1882/5472) aion-integrations workspace::tests::a_reserved_parameter_is_subtracted_and_what_remains_is_the_prompt
        PASS [   0.009s] (1883/5472) aion-integrations workspace::tests::an_object_carrying_two_possible_prompts_is_refused_by_name
        PASS [   0.016s] (1884/5472) aion-integrations workspace::tests::an_input_that_is_not_an_object_says_it_carries_no_parameters
        PASS [   0.006s] (1885/5472) aion-integrations workspace::tests::the_prompt_is_found_wherever_the_directory_sits_in_the_object
        PASS [   0.013s] (1886/5472) aion-integrations workspace::tests::json_tagged_text_that_is_not_json_passes_through_as_the_prompt
        PASS [   0.021s] (1887/5472) aion-integrations workspace::tests::an_object_carrying_only_the_directory_is_refused
        PASS [   0.007s] (1888/5472) aion-integrations::mock_harness unadvertised_command_is_rejected_even_when_some_are_supported
        PASS [   0.020s] (1889/5472) aion-integrations workspace::tests::each_form_answers_only_for_itself
        PASS [   0.017s] (1890/5472) aion-integrations::mock_harness observability_only_harness_is_first_class
        PASS [   0.021s] (1891/5472) aion-integrations workspace::tests::non_utf8_input_is_refused_as_a_protocol_fault
        PASS [   0.025s] (1892/5472) aion-integrations workspace::tests::input_that_is_not_json_is_refused_terminally
        PASS [   0.008s] (1893/5472) aion-mcp protocol::error::tests::an_unimplemented_method_is_a_404_carrying_the_json_rpc_code
        PASS [   0.019s] (1894/5472) aion-mcp http::tests::method_not_allowed_names_post
        PASS [   0.026s] (1895/5472) aion-integrations::mock_harness rich_harness_streams_events_accepts_supported_command_and_returns_result
        PASS [   0.020s] (1896/5472) aion-mcp http::tests::the_origin_header_is_read_case_insensitively
        PASS [   0.013s] (1897/5472) aion-mcp protocol::headers::tests::a_param_header_is_sentinel_decoded_before_the_numeric_comparison
        PASS [   0.015s] (1898/5472) aion-mcp protocol::headers::tests::a_missing_required_header_is_a_mismatch_and_an_unexpected_one_is_too
        PASS [   0.030s] (1899/5472) aion-mcp http::tests::a_json_response_sets_its_content_type
        PASS [   0.022s] (1900/5472) aion-mcp protocol::error::tests::missing_capability_emits_the_core_code_not_the_tasks_draft_code
        PASS [   0.021s] (1901/5472) aion-mcp protocol::error::tests::unsupported_version_carries_supported_and_requested
        PASS [   0.009s] (1902/5472) aion-mcp protocol::headers::tests::header_names_are_case_insensitive_and_values_are_not
        PASS [   0.023s] (1903/5472) aion-mcp protocol::headers::tests::a_broken_sentinel_is_a_header_mismatch
        PASS [   0.018s] (1904/5472) aion-mcp protocol::headers::tests::an_integer_beyond_the_safe_range_never_matches
        PASS [   0.016s] (1905/5472) aion-mcp protocol::headers::tests::control_characters_are_refused
        PASS [   0.022s] (1906/5472) aion-mcp protocol::headers::tests::an_integer_param_compares_numerically_not_lexically
        PASS [   0.016s] (1907/5472) aion-mcp protocol::headers::tests::param_headers_are_enumerated_by_suffix
        PASS [   0.016s] (1908/5472) aion-mcp protocol::headers::tests::the_base64_sentinel_is_decoded_before_comparison
        PASS [   0.023s] (1909/5472) aion-mcp protocol::headers::tests::string_and_boolean_params_compare_exactly
        PASS [   0.016s] (1910/5472) aion-mcp protocol::jsonrpc::tests::a_non_json_body_is_a_parse_error
        PASS [   0.016s] (1911/5472) aion-mcp protocol::jsonrpc::tests::a_request_and_a_notification_are_distinguished_by_the_id
        PASS [   0.014s] (1912/5472) aion-mcp protocol::meta::tests::a_complete_meta_parses
        PASS [   0.027s] (1913/5472) aion-mcp protocol::jsonrpc::tests::a_client_sent_response_is_refused
        PASS [   0.017s] (1914/5472) aion-mcp protocol::meta::tests::an_unimplemented_version_is_refused_before_the_capability_check
        PASS [   0.018s] (1915/5472) aion-mcp protocol::meta::tests::missing_capabilities_is_invalid_params_not_an_empty_default
        PASS [   0.019s] (1916/5472) aion-mcp protocol::meta::tests::a_non_object_extension_value_is_not_a_declaration
        PASS [   0.031s] (1917/5472) aion-mcp protocol::jsonrpc::tests::a_null_id_is_refused
        PASS [   0.017s] (1918/5472) aion-mcp protocol::result::tests::cache_hints_write_both_fields
        PASS [   0.015s] (1919/5472) aion-mcp protocol::result::tests::the_envelope_places_server_info_inside_meta
        PASS [   0.018s] (1920/5472) aion-mcp protocol::version::tests::the_implemented_revision_is_the_only_supported_one
        PASS [   0.023s] (1921/5472) aion-mcp server::config::tests::an_empty_origin_list_allows_nothing
        PASS [   0.025s] (1922/5472) aion-mcp server::call_result::tests::a_tool_failure_is_an_is_error_result_without_structured_content
        PASS [   0.034s] (1923/5472) aion-mcp protocol::meta::tests::missing_meta_entirely_is_invalid_params
        PASS [   0.021s] (1924/5472) aion-mcp server::discover::tests::the_result_carries_every_required_discover_field
        PASS [   0.018s] (1925/5472) aion-mcp server::discover::tests::tools_capability_does_not_claim_list_changed
        PASS [   0.024s] (1926/5472) aion-mcp server::config::tests::both_cacheable_results_are_private
        PASS [   0.024s] (1927/5472) aion-mcp server::discover::tests::the_tasks_extension_is_advertised_only_to_a_declaring_client
        PASS [   0.014s] (1928/5472) aion-mcp tasks::model::tests::terminal_statuses_are_exactly_the_three
        PASS [   0.020s] (1929/5472) aion-mcp tasks::model::tests::a_completed_task_embeds_its_result_and_a_failed_one_embeds_its_error
        PASS [   0.021s] (1930/5472) aion-mcp server::dispatch::tests::subscriptions_listen_is_not_implemented
        PASS [   0.017s] (1931/5472) aion-mcp tasks::model::tests::an_unlimited_ttl_is_written_as_an_explicit_null
        PASS [   0.033s] (1932/5472) aion-mcp server::dispatch::tests::mcp_name_is_sourced_from_the_right_body_field_per_method
        PASS [   0.024s] (1933/5472) aion-mcp tasks::resolve::tests::an_unreadable_handle_is_invalid_params_and_a_backend_failure_is_internal
        PASS [   0.023s] (1934/5472) aion-mcp tools::annotations::tests::a_case_insensitive_duplicate_is_refused
        PASS [   0.025s] (1935/5472) aion-mcp tools::annotations::tests::a_name_that_is_not_a_field_token_is_refused
        PASS [   0.077s] (1936/5472) aion-mcp server::call_result::tests::a_success_carries_structured_content_and_its_serialization
        PASS [   0.025s] (1937/5472) aion-mcp tools::annotations::tests::a_number_typed_annotation_is_refused
        PASS [   0.018s] (1938/5472) aion-mcp tools::annotations::tests::a_reachable_annotation_is_collected_with_its_property_path
        PASS [   0.086s] (1939/5472) aion-mcp server::call_result::tests::structured_content_violating_the_declared_schema_is_refused
        PASS [   0.019s] (1940/5472) aion-mcp tools::annotations::tests::an_annotation_behind_any_conditional_keyword_is_refused
        PASS [   0.022s] (1941/5472) aion-mcp tools::annotations::tests::a_schema_with_no_annotations_yields_none
        PASS [   0.032s] (1942/5472) aion-mcp tools::descriptor::tests::an_object_input_schema_is_recognised
        PASS [   0.026s] (1943/5472) aion-mcp tools::descriptor::tests::annotation_constructors_state_every_hint
        PASS [   0.077s] (1944/5472) aion-mcp tasks::resolve::tests::a_result_violating_the_published_schema_fails_the_task
        PASS [   0.084s] (1945/5472) aion-mcp tasks::resolve::tests::a_projection_never_advertises_a_lifetime
        PASS [   0.031s] (1946/5472) aion-mcp tools::descriptor::tests::annotations_serialize_under_their_wire_names
        PASS [   0.028s] (1947/5472) aion-mcp tools::service::tests::a_non_object_input_schema_is_refused
        PASS [   0.036s] (1948/5472) aion-mcp tools::descriptor::tests::tool_names_are_checked_against_the_permitted_alphabet
        PASS [   0.084s] (1949/5472) aion-mcp tasks::resolve::tests::finished_work_completes_the_task_and_carries_the_inline_result
        PASS [   0.070s] (1950/5472) aion-mcp tools::service::tests::a_duplicate_name_is_refused
        PASS [   0.065s] (1951/5472) aion-mcp tools::service::tests::an_uncompilable_schema_is_refused_at_construction
        PASS [   0.061s] (1952/5472) aion-mcp::conformance a_base64_sentinel_header_matches_the_plain_body_value
        PASS [   0.062s] (1953/5472) aion-mcp::conformance a_header_disagreeing_with_the_body_is_refused
        PASS [   0.060s] (1954/5472) aion-mcp::conformance a_malformed_body_is_a_parse_error
        PASS [   0.086s] (1955/5472) aion-mcp tools::service::tests::validation_reports_the_violating_pointer
        PASS [   0.091s] (1956/5472) aion-mcp tools::service::tests::publication_order_is_preserved
        PASS [   0.088s] (1957/5472) aion-mcp::conformance a_disallowed_origin_is_403_before_anything_is_dispatched
        PASS [   0.049s] (1958/5472) aion-mcp::conformance a_non_declaring_client_is_not_offered_the_tasks_extension
        PASS [   0.082s] (1959/5472) aion-mcp::conformance a_missing_required_header_is_refused
        PASS [   0.048s] (1960/5472) aion-mcp::conformance a_request_without_client_capabilities_is_invalid_params
        PASS [   0.046s] (1961/5472) aion-mcp::conformance a_tool_call_returns_content_and_structured_content_without_cache_hints
        PASS [   0.044s] (1962/5472) aion-mcp::conformance a_tool_level_failure_is_a_result_not_a_protocol_error
        PASS [   0.071s] (1963/5472) aion-mcp::conformance a_param_header_obeys_the_full_three_armed_matrix
        PASS [   0.048s] (1964/5472) aion-mcp::conformance an_integer_param_header_is_compared_numerically_not_as_a_string
        PASS [   0.049s] (1965/5472) aion-mcp::conformance an_unimplemented_method_is_a_404_carrying_minus_32601
        PASS [   0.042s] (1966/5472) aion-mcp::conformance an_unrecognised_param_header_is_forwarded_and_ignored
        PASS [   0.044s] (1967/5472) aion-mcp::conformance an_unknown_tool_is_invalid_params
        PASS [   0.076s] (1968/5472) aion-mcp::conformance an_unimplemented_version_lists_the_supported_ones
        PASS [   0.050s] (1969/5472) aion-mcp::conformance arguments_violating_the_input_schema_are_refused_before_the_tool_runs
        PASS [   0.117s] (1970/5472) aion-mcp::conformance a_notification_is_acknowledged_with_202_and_no_body
        PASS [   0.086s] (1971/5472) aion-mcp::conformance an_mcp_name_header_on_a_method_that_has_no_name_is_refused
        PASS [   0.045s] (1972/5472) aion-mcp::conformance only_the_deliberately_annotated_tool_carries_x_mcp_header
        PASS [   0.032s] (1973/5472) aion-mcp::tasks_lifecycle a_created_task_advertises_no_lifetime_and_resolves_at_once
        PASS [   0.065s] (1974/5472) aion-mcp::conformance get_and_delete_are_405_and_no_session_header_is_ever_minted
        PASS [   0.042s] (1975/5472) aion-mcp::tasks_lifecycle an_unknown_task_id_is_invalid_params_on_every_tasks_method
        PASS [   0.051s] (1976/5472) aion-mcp::tasks_lifecycle a_non_declaring_client_is_refused_with_the_core_capability_code
        PASS [   0.074s] (1977/5472) aion-mcp::conformance tools_list_is_deterministic_cacheable_and_fully_annotated
        PASS [   0.055s] (1978/5472) aion-mcp::tasks_lifecycle cancellation_is_acknowledged_empty_and_changes_nothing
        PASS [   0.065s] (1979/5472) aion-mcp::tasks_lifecycle a_failed_task_shaped_call_is_answered_inline_and_mints_no_task
        PASS [   0.048s] (1980/5472) aion-mcp::tasks_lifecycle tasks_methods_are_gated_on_the_declared_extension
        PASS [   0.009s] (1981/5472) aion-package admission::tests::a_declared_signal_resolves_to_its_own_schema
        PASS [   0.020s] (1982/5472) aion-package admission::tests::a_contract_declaring_nothing_has_nothing_unenforceable
        PASS [   0.072s] (1983/5472) aion-mcp::tasks_lifecycle creation_and_reading_agree_because_one_projection_answers_both
        PASS [   0.119s] (1984/5472) aion-mcp::conformance server_discover_returns_the_full_cacheable_shape
        PASS [   0.049s] (1985/5472) aion-mcp::tasks_lifecycle tasks_update_acknowledges_and_requires_input_responses
        PASS [   0.102s] (1986/5472) aion-mcp::tasks_lifecycle a_valid_handle_does_not_authorize_a_caller_who_may_not_read_the_work
        PASS [   0.061s] (1987/5472) aion-mcp::tasks_lifecycle the_mcp_name_header_must_equal_the_task_id
        PASS [   0.021s] (1988/5472) aion-package admission::tests::an_additional_entry_resolves_to_its_own_input_schema
        PASS [   0.039s] (1989/5472) aion-package admission::tests::a_missing_required_field_is_refused_naming_it
        PASS [   0.013s] (1990/5472) aion-package admission::tests::declared_signal_names_are_sorted_for_a_stable_refusal_message
        PASS [   0.041s] (1991/5472) aion-package admission::tests::a_real_schema_declares_something
        PASS [   0.071s] (1992/5472) aion-package admission::tests::a_conforming_value_is_admitted
        PASS [   0.080s] (1993/5472) aion-mcp::tasks_lifecycle there_is_no_tasks_list_method
        PASS [   0.055s] (1994/5472) aion-package admission::tests::a_wrong_typed_field_is_refused_naming_its_location
        PASS [   0.010s] (1995/5472) aion-package beam::tests::beam_set_rejects_duplicate_logical_names
        PASS [   0.065s] (1996/5472) aion-package admission::tests::a_non_object_payload_is_refused_at_the_root
        PASS [   0.021s] (1997/5472) aion-package beam::tests::beam_set_order_is_independent_of_insertion_order
        PASS [   0.053s] (1998/5472) aion-package admission::tests::an_uncompilable_declaration_is_reported_and_a_compilable_one_is_not
        PASS [   0.028s] (1999/5472) aion-package admission::tests::the_undeclared_forms_declare_nothing
        PASS [   0.010s] (2000/5472) aion-package build_coverage::tests::insertion_order_is_observable_exactly_when_the_map_preserves_it
        PASS [   0.021s] (2001/5472) aion-package beam::tests::beam_set_rejects_engine_reserved_namespaces
        PASS [   0.019s] (2002/5472) aion-package beam::tests::lookup_returns_exact_bytes_by_logical_name
        PASS [   0.020s] (2003/5472) aion-package build_coverage::tests::serde_json_map_ordering_is_announced
        PASS [   0.013s] (2004/5472) aion-package builder::tests::caller_supplied_manifest_version_is_overwritten
        PASS [   0.022s] (2005/5472) aion-package builder::tests::awl_entries_are_written_under_their_own_prefixes
        PASS [   0.021s] (2006/5472) aion-package builder::tests::awl_source_inclusion_does_not_change_manifest_version
        PASS [   0.019s] (2007/5472) aion-package builder::tests::finalised_manifest_version_equals_v4_contract_hash
        PASS [   0.059s] (2008/5472) aion-package admission::tests::surplus_fields_are_admitted
        PASS [   0.017s] (2009/5472) aion-package builder::tests::rejects_logical_names_with_deployed_name_separator
        PASS [   0.020s] (2010/5472) aion-package builder::tests::missing_entry_module_returns_typed_error
        PASS [   0.037s] (2011/5472) aion-package builder::tests::identical_inputs_produce_identical_archive_bytes
        PASS [   0.014s] (2012/5472) aion-package canonical::tests::as_value_exposes_the_wrapped_document
        PASS [   0.016s] (2013/5472) aion-package builder::tests::write_to_bytes_succeeds_without_source_entries
        PASS [   0.092s] (2014/5472) aion-package admission::tests::every_violation_is_reported
        PASS [   0.100s] (2015/5472) aion-package admission::tests::an_uncompilable_schema_is_a_package_defect_not_a_caller_error
        PASS [   0.034s] (2016/5472) aion-package builder::tests::rejects_unsafe_source_names
        PASS [   0.010s] (2017/5472) aion-package canonical::tests::scalars_arrays_and_nulls_round_trip_unchanged
        PASS [   0.038s] (2018/5472) aion-package builder::tests::rejects_unsafe_awl_schema_paths
        PASS [   0.032s] (2019/5472) aion-package builder::tests::source_inclusion_does_not_change_manifest_version
        PASS [   0.028s] (2020/5472) aion-package canonical::tests::canonical_bytes_are_independent_of_insertion_order
        PASS [   0.011s] (2021/5472) aion-package canonical::tests::sorted_entries_is_key_order_whatever_the_insertion_order
        PASS [   0.020s] (2022/5472) aion-package canonical::tests::canonical_bytes_are_key_sorted_at_every_depth
        PASS [   0.025s] (2023/5472) aion-package canonical::tests::canonical_encoding_matches_a_key_sorted_expectation
        PASS [   0.018s] (2024/5472) aion-package canonical::tests::serialize_value_canonicalises_a_field_of_a_derived_type
        PASS [   0.010s] (2025/5472) aion-package codegen::activity_project::tests::write_then_check_round_trips_and_detects_drift
        PASS [   0.017s] (2026/5472) aion-package codegen::activity_project::tests::declared_type_without_a_boundary_type_errors
        PASS [   0.022s] (2027/5472) aion-package codegen::activity_golden::tests::optionals_lists_nested_records_and_enums_are_derived
        PASS [   0.016s] (2028/5472) aion-package codegen::activity_project::tests::in_vm_tier_emits_neither_worker_nor_golden
        PASS [   0.024s] (2029/5472) aion-package codegen::activity_golden::tests::all_required_scalars_pin_to_a_zero_value_literal
        PASS [   0.026s] (2030/5472) aion-package codegen::activity_golden::tests::unresolved_named_type_is_an_internal_error
        PASS [   0.015s] (2031/5472) aion-package codegen::activity_worker_python::tests::pascal_case_joins_snake_segments
        PASS [   0.015s] (2032/5472) aion-package codegen::activity_worker_python::tests::the_advertised_surface_satisfies_a_contract_over_the_same_declared_types
        PASS [   0.017s] (2033/5472) aion-package codegen::activity_worker_python::tests::serve_call_passes_the_advertised_descriptors
        PASS [   0.017s] (2034/5472) aion-package codegen::activity_worker_rust::tests::enums_and_keyword_fields_render_with_serde_attrs
        PASS [   0.019s] (2035/5472) aion-package codegen::activity_worker_python::tests::routes_each_activity_to_its_handler_in_order
        PASS [   0.018s] (2036/5472) aion-package codegen::activity_worker_rust::tests::keyword_field_names_are_escaped_or_mangled
        PASS [   0.030s] (2037/5472) aion-package codegen::activity_worker_python::tests::every_activity_advertises_real_input_and_output_schemas
        PASS [   0.016s] (2038/5472) aion-package codegen::activity_worker_rust::tests::worker_emits_structs_registration_and_config_in_order
        PASS [   0.012s] (2039/5472) aion-package codegen::awl_worker::tests::a_server_executed_action_is_absent_everywhere
        PASS [   0.023s] (2040/5472) aion-package codegen::activity_worker_rust::tests::type_mapping_covers_every_scalar_list_and_named
        PASS [   0.013s] (2041/5472) aion-package codegen::awl_worker::tests::an_unpinned_queue_dials_one_connection_with_no_node
        PASS [   0.017s] (2042/5472) aion-package codegen::activity_wrappers::tests::wrappers_module_emits_one_constructor_per_declaration_in_order
        PASS [   0.019s] (2043/5472) aion-package codegen::awl_worker::tests::a_queue_of_only_bodied_actions_is_refused
        PASS [   0.016s] (2044/5472) aion-package codegen::awl_worker::tests::every_registration_carries_a_descriptor_from_the_document
        PASS [   0.015s] (2045/5472) aion-package codegen::awl_worker::tests::one_connection_per_node_serves_exactly_the_reachable_actions
        PASS [   0.022s] (2046/5472) aion-package codegen::awl_worker::tests::every_stub_fails_loudly_and_names_its_action
        PASS [   0.018s] (2047/5472) aion-package codegen::awl_worker::tests::keyword_action_names_are_escaped_or_refused
        PASS [   0.016s] (2048/5472) aion-package codegen::awl_worker::tests::shell_manifest_refuses_any_node_pinned_queue
        PASS [   0.019s] (2049/5472) aion-package codegen::awl_worker::tests::shell_manifest_wires_the_whole_queue_with_declared_result_encodings
        PASS [   0.027s] (2050/5472) aion-package codegen::awl_worker::tests::generation_is_deterministic
        PASS [   0.028s] (2051/5472) aion-package codegen::awl_worker::tests::only_the_handlers_file_is_author_owned
        PASS [   0.037s] (2052/5472) aion-package codegen::awl_worker::tests::every_stub_documents_the_declared_schemas
        PASS [   0.015s] (2053/5472) aion-package codegen::codec_module::tests::all_required_records_use_plain_object_lists_without_option_imports
        PASS [   0.024s] (2054/5472) aion-package codegen::awl_worker::tests::the_declaration_embeds_the_document_and_takes_its_schemas
        PASS [   0.026s] (2055/5472) aion-package codegen::awl_worker::tests::the_emitted_connection_table_is_the_plan
        PASS [   0.015s] (2056/5472) aion-package codegen::codec_module::tests::empty_records_emit_bare_constructors
        PASS [   0.017s] (2057/5472) aion-package codegen::codec_module::tests::emission_is_deterministic
        PASS [   0.023s] (2058/5472) aion-package codegen::awl_worker::tests::unusable_names_and_paths_are_refused
        PASS [   0.029s] (2059/5472) aion-package codegen::awl_worker::tests::the_manifest_is_standalone_and_strict
        PASS [   0.034s] (2060/5472) aion-package codegen::awl_worker::tests::shell_readme_carries_the_production_invocation
        PASS [   0.019s] (2061/5472) aion-package codegen::declaration::tests::empty_manifest_parses_to_no_declarations
        PASS [   0.024s] (2062/5472) aion-package codegen::codec_module::tests::full_module_golden_with_enum_list_and_optional
        PASS [   0.019s] (2063/5472) aion-package codegen::declaration::tests::malformed_json_is_rejected
        PASS [   0.024s] (2064/5472) aion-package codegen::codec_module::tests::nested_lists_encode_with_depth_named_lambdas
        PASS [   0.022s] (2065/5472) aion-package codegen::declaration::tests::names_that_cannot_derive_an_identifier_are_rejected
        PASS [   0.029s] (2066/5472) aion-package codegen::declaration::tests::duplicate_activity_is_rejected
        PASS [   0.024s] (2067/5472) aion-package codegen::declaration::tests::unknown_tier_is_rejected
        PASS [   0.028s] (2068/5472) aion-package codegen::declaration::tests::parses_in_declaration_order
        PASS [   0.014s] (2069/5472) aion-package codegen::error::tests::codegen_error_is_send_sync_and_static
        PASS [   0.018s] (2070/5472) aion-package codegen::declaration::tests::well_formed_snake_case_names_are_accepted
        PASS [   0.012s] (2071/5472) aion-package codegen::input_skeleton::tests::required_enum_uses_the_first_wire_value
        PASS [   0.016s] (2072/5472) aion-package codegen::input_skeleton::tests::required_arrays_are_empty
        PASS [   0.026s] (2073/5472) aion-package codegen::error::tests::display_messages_name_module_type_and_field
        PASS [   0.029s] (2074/5472) aion-package codegen::input_skeleton::tests::empty_enum_fails
        PASS [   0.030s] (2075/5472) aion-package codegen::error::tests::packaging_errors_convert_transparently
        PASS [   0.019s] (2076/5472) aion-package codegen::input_skeleton::tests::unsupported_construct_fails_with_pointer
        PASS [   0.020s] (2077/5472) aion-package codegen::input_skeleton::tests::required_scalars_get_type_shaped_placeholders
        PASS [   0.030s] (2078/5472) aion-package codegen::input_skeleton::tests::nested_required_objects_recurse
        PASS [   0.015s] (2079/5472) aion-package codegen::interface::tests::functions_constants_and_aliases_in_the_types_module_fail_loudly
        PASS [   0.022s] (2080/5472) aion-package codegen::interface::tests::duplicate_wire_strings_and_colliding_prefixes_fail_loudly
        PASS [   0.022s] (2081/5472) aion-package codegen::interface::tests::generic_opaque_unlabelled_mixed_and_cross_module_types_fail_loudly
        PASS [   0.017s] (2082/5472) aion-package codegen::interface::tests::nested_option_tuples_and_mismatched_constructor_fail_loudly
        PASS [   0.020s] (2083/5472) aion-package codegen::interface::tests::missing_and_empty_types_module_fail_loudly
        PASS [   0.020s] (2084/5472) aion-package codegen::interface::tests::records_enums_scalars_lists_and_options_map_into_the_model
        PASS [   0.024s] (2085/5472) aion-package codegen::interface::tests::mapping_is_deterministic_across_runs
        PASS [   0.022s] (2086/5472) aion-package codegen::interface::tests::recursive_types_fail_loudly
        PASS [   0.021s] (2087/5472) aion-package codegen::interface::tests::unknown_type_kind_and_invalid_json_fail_loudly
        PASS [   0.016s] (2088/5472) aion-package codegen::names::tests::reserved_words_are_recognised
        PASS [   0.023s] (2089/5472) aion-package codegen::names::tests::pascal_to_snake_is_deterministic_and_shape_true
        PASS [   0.017s] (2090/5472) aion-package codegen::names::tests::snake_identifiers_are_classified
        PASS [   0.016s] (2091/5472) aion-package codegen::project::tests::package_name_reads_and_validates
        PASS [   0.033s] (2092/5472) aion-package codegen::project::tests::check_on_disk_passes_clean_and_types_missing_and_drift
        PASS [   0.023s] (2093/5472) aion-package codegen::schema_emit::tests::stale_marked_schema_is_removed_on_write_and_drift_under_check
        PASS [   0.032s] (2094/5472) aion-package codegen::schema_emit::tests::enum_schema_and_empty_record_render
        PASS [   0.025s] (2095/5472) aion-package codegen::test_scaffold::tests::scaffold_mocks_each_activity_and_advances_per_timer
        PASS [   0.032s] (2096/5472) aion-package codegen::schema_emit::tests::stray_unmarked_json_is_a_loud_error_with_the_migration_hint
        PASS [   0.051s] (2097/5472) aion-package codegen::schema_emit::tests::emitted_schemas_parse_as_json_and_are_deterministic
        PASS [   0.047s] (2098/5472) aion-package codegen::schema_emit::tests::record_schema_renders_required_optional_list_enum_and_nested
        PASS [   0.016s] (2099/5472) aion-package compatibility::tests::a_comment_in_a_declared_schema_does_not_have_to_be_reproduced
        PASS [   0.034s] (2100/5472) aion-package codegen::test_scaffold::tests::scaffold_with_no_timers_does_not_advance_the_clock
        PASS [   0.053s] (2101/5472) aion-package codegen::schema_emit::tests::write_then_check_round_trips_and_check_catches_a_hand_edit
        PASS [   0.038s] (2102/5472) aion-package codegen::test_scaffold::tests::scaffold_with_no_activities_mocks_nothing_but_still_asserts_replay
        PASS [   0.032s] (2103/5472) aion-package compatibility::tests::a_connection_on_an_unknown_node_owes_only_the_unpinned_actions
        PASS [   0.021s] (2104/5472) aion-package compatibility::tests::an_action_pinned_to_this_node_is_still_demanded
        PASS [   0.010s] (2105/5472) aion-package compatibility::tests::input_widening_and_optional_output_addition_are_compatible
        PASS [   0.029s] (2106/5472) aion-package compatibility::tests::a_node_partitioned_connection_is_not_demanded_another_nodes_actions
        PASS [   0.038s] (2107/5472) aion-package compatibility::tests::a_declared_body_is_not_required_of_a_worker
        PASS [   0.018s] (2108/5472) aion-package compatibility::tests::input_narrowing_and_output_widening_are_refused
        PASS [   0.038s] (2109/5472) aion-package compatibility::tests::a_node_less_connection_owes_only_the_unpinned_actions
        PASS [   0.016s] (2110/5472) aion-package contract::tests::a_declared_body_is_identity_bearing
        PASS [   0.025s] (2111/5472) aion-package compatibility::tests::local_defs_and_inline_schemas_compare_semantically
        PASS [   0.027s] (2112/5472) aion-package compatibility::tests::mismatch_reports_the_exact_schema_field
        PASS [   0.034s] (2113/5472) aion-package compatibility::tests::an_unpinned_action_is_demanded_of_every_node
        PASS [   0.023s] (2114/5472) aion-package contract::tests::advisory_is_identity_bearing
        PASS [   0.023s] (2115/5472) aion-package contract::tests::editing_a_declared_command_changes_identity
        PASS [   0.030s] (2116/5472) aion-package contract::tests::advisory_identity_is_declaration_order_independent
        PASS [   0.031s] (2117/5472) aion-package contract::tests::advisory_and_body_cannot_be_confused_in_the_record
        PASS [   0.017s] (2118/5472) aion-package contract::tests::v4_identity_ignores_json_object_and_set_array_order
        PASS [   0.009s] (2119/5472) aion-package hash::tests::content_hash_changes_when_a_module_byte_changes
        PASS [   0.024s] (2120/5472) aion-package error::tests::display_messages_name_the_failed_condition
        PASS [   0.029s] (2121/5472) aion-package contract::tests::the_v5_domain_supersedes_every_v4_identity
        PASS [   0.013s] (2122/5472) aion-package extraction::tests::single_entry_past_budget_is_refused_reporting_the_limit
        PASS [   0.017s] (2123/5472) aion-package error::tests::package_error_is_send_and_sync
        PASS [   0.025s] (2124/5472) aion-package contract::tests::v4_identity_is_declaration_order_independent
        PASS [   0.019s] (2125/5472) aion-package extraction::tests::unbounded_budget_reads_everything
        PASS [   0.008s] (2126/5472) aion-package hash::tests::content_hash_framing_prevents_name_bytes_boundary_ambiguity
        PASS [   0.012s] (2127/5472) aion-package hash::tests::content_hash_rejects_uppercase_text
        PASS [   0.304s] (2128/5472) aion-package codegen::activity_worker_python::tests::emitted_module_is_syntactically_valid_python
        PASS [   0.028s] (2129/5472) aion-package extraction::tests::bounded_budget_charges_across_entries
        PASS [   0.010s] (2130/5472) aion-package hash::tests::legacy_identity_remains_exactly_the_beams_only_hash
        PASS [   0.026s] (2131/5472) aion-package hash::tests::content_hash_changes_when_a_module_name_changes
        PASS [   0.023s] (2132/5472) aion-package hash::tests::content_hash_text_round_trips
        PASS [   0.026s] (2133/5472) aion-package hash::tests::explicit_timeout_identity_is_deterministic_and_value_sensitive
        PASS [   0.030s] (2134/5472) aion-package hash::tests::content_hash_is_independent_of_insertion_order
        PASS [   0.019s] (2135/5472) aion-package hash::tests::non_v1_non_v3_timeout_identity_is_rejected
        PASS [   0.024s] (2136/5472) aion-package hash::tests::mixed_archive_with_injected_additional_timeout_reads_as_undeclared
        PASS [   0.018s] (2137/5472) aion-package hash::tests::per_entry_identity_binds_every_additional_entry_timeout
        PASS [   0.020s] (2138/5472) aion-package hash::tests::v3_identity_binds_the_primary_routing_identity
        PASS [   0.026s] (2139/5472) aion-package hash::tests::v1_single_value_archive_loads_but_reads_undeclared
        PASS [   0.026s] (2140/5472) aion-package manifest::tests::additional_workflow_entry_round_trips_with_internal_flag
        PASS [   0.023s] (2141/5472) aion-package manifest::tests::supported_format_version_passes
        PASS [   0.029s] (2142/5472) aion-package manifest::tests::manifest_json_keys_are_stable
        PASS [   0.037s] (2143/5472) aion-package manifest::tests::absent_additional_workflow_list_loads_as_legacy_empty
        PASS [   0.021s] (2144/5472) aion-package manifest::tests::unsupported_format_version_returns_typed_error
        PASS [   0.015s] (2145/5472) aion-package namespace::tests::deployed_name_sets_for_different_hashes_are_disjoint
        PASS [   0.037s] (2146/5472) aion-package manifest::tests::canonical_digest_renders_as_lowercase_hex
        PASS [   0.034s] (2147/5472) aion-package manifest::tests::manifest_with_schemas_and_declared_activities_round_trips
        PASS [   0.041s] (2148/5472) aion-package manifest::tests::canonical_digest_detects_manifest_divergence
        PASS [   0.036s] (2149/5472) aion-package manifest::tests::manifest_round_trips_losslessly_through_json
        PASS [   0.015s] (2150/5472) aion-package namespace::tests::forward_then_inverse_round_trips_many_pairs
        PASS [   0.018s] (2151/5472) aion-package namespace::tests::deployed_names_follow_beam_set_canonical_order
        PASS [   0.015s] (2152/5472) aion-package namespace::tests::same_logical_module_under_same_hash_is_idempotent
        PASS [   0.016s] (2153/5472) aion-package namespace::tests::malformed_deployed_names_return_typed_errors
        PASS [   0.015s] (2154/5472) aion-package package::tests::a_nested_document_entry_is_refused
        PASS [   0.021s] (2155/5472) aion-package namespace::tests::forward_transform_uses_mandated_separator_and_hash_text
        PASS [   0.026s] (2156/5472) aion-package namespace::tests::inverse_then_forward_recovers_deployed_name
        PASS [   0.013s] (2157/5472) aion-package package::tests::an_archive_without_awl_entries_loads_with_no_provenance
        PASS [   0.020s] (2158/5472) aion-package package::tests::a_non_utf8_document_entry_is_refused
        PASS [   0.025s] (2159/5472) aion-package namespace::tests::parse_preserves_separator_neighbouring_chars
        PASS [   0.015s] (2160/5472) aion-package package::tests::builder_produced_package_loads_successfully
        PASS [   0.016s] (2161/5472) aion-package package::tests::awl_provenance_survives_the_persistence_rewrite
        PASS [   0.018s] (2162/5472) aion-package package::tests::invalid_source_entry_returns_malformed_entry
        PASS [   0.018s] (2163/5472) aion-package package::tests::load_from_path_loads_successfully
        PASS [   0.034s] (2164/5472) aion-package package::tests::an_unrecognised_awl_family_is_refused
        PASS [   0.029s] (2165/5472) aion-package package::tests::beam_entry_with_deployed_name_separator_returns_malformed_entry
        PASS [   0.020s] (2166/5472) aion-package package::tests::missing_entry_module_returns_exact_variant_when_hash_matches
        PASS [   0.017s] (2167/5472) aion-package package::tests::missing_manifest_returns_missing_manifest
        PASS [   0.030s] (2168/5472) aion-package package::tests::malformed_beam_entry_returns_exact_variant
        PASS [   0.020s] (2169/5472) aion-package package::tests::re_routing_the_primary_entry_module_invalidates_the_stored_identity
        PASS [   0.023s] (2170/5472) aion-package package::tests::package_on_exact_inflate_budget_loads_and_one_byte_under_refuses
        PASS [   0.031s] (2171/5472) aion-package package::tests::non_zip_input_returns_archive_read
        PASS [   0.019s] (2172/5472) aion-package package::tests::to_archive_bytes_preserves_legacy_identity
        PASS [   0.023s] (2173/5472) aion-package package::tests::schema_entries_without_a_document_are_refused
        PASS [   0.029s] (2174/5472) aion-package package::tests::truncated_zip_input_returns_archive_read
        PASS [   0.017s] (2175/5472) aion-package package::tests::unparseable_manifest_returns_manifest_parse
        PASS [   0.034s] (2176/5472) aion-package package::tests::to_archive_bytes_preserves_v4_contract_identity
        PASS [   0.029s] (2177/5472) aion-package package::tests::unknown_format_version_returns_exact_variant
        PASS [   0.021s] (2178/5472) aion-package project::assemble::tests::exclusions_and_sources_are_reported_and_shipped
        PASS [   0.019s] (2179/5472) aion-package project::assemble::tests::missing_entry_module_returns_entry_module_not_found
        PASS [   0.031s] (2180/5472) aion-package project::assemble::tests::explicit_output_field_is_respected
        PASS [   0.032s] (2181/5472) aion-package project::assemble::tests::multi_workflow_project_binds_each_primary_route_into_identity
        PASS [   0.026s] (2182/5472) aion-package project::assemble::tests::output_override_with_multiple_workflows_is_ambiguous
        PASS [   0.033s] (2183/5472) aion-package project::assemble::tests::output_override_may_point_outside_root_via_dotdot
        PASS [   0.034s] (2184/5472) aion-package project::assemble::tests::output_override_applies_to_single_workflow_project
        PASS [   0.032s] (2185/5472) aion-package project::assemble::tests::packaged_workflow_round_trips_manifest_and_hash
        PASS [   0.041s] (2186/5472) aion-package project::assemble::tests::repackaging_produces_identical_archive_bytes
        PASS [   0.025s] (2187/5472) aion-package project::assemble::tests::user_module_with_reserved_name_fails_typed
        PASS [   0.045s] (2188/5472) aion-package project::assemble::tests::output_write_failure_names_the_output_path
        PASS [   0.019s] (2189/5472) aion-package project::config::tests::dotdot_output_escaping_root_is_rejected
        PASS [   0.035s] (2190/5472) aion-package project::assemble::tests::source_inclusion_changes_bytes_but_never_the_version
        PASS [   0.030s] (2191/5472) aion-package project::config::tests::absent_timeout_seconds_declares_no_timeout
        PASS [   0.026s] (2192/5472) aion-package project::config::tests::absolute_schema_path_outside_root_is_rejected_unread
        PASS [   0.015s] (2193/5472) aion-package project::config::tests::duplicate_entry_modules_return_config_invalid
        PASS [   0.018s] (2194/5472) aion-package project::config::tests::dotdot_paths_staying_inside_root_are_accepted_and_normalized
        PASS [   0.019s] (2195/5472) aion-package project::config::tests::empty_entry_function_returns_config_invalid
        PASS [   0.023s] (2196/5472) aion-package project::config::tests::duplicate_activities_return_config_invalid
        PASS [   0.013s] (2197/5472) aion-package project::config::tests::explicit_output_conflicts_are_rejected_with_both_workflows
        PASS [   0.040s] (2198/5472) aion-package project::config::tests::dotdot_output_schema_escaping_root_is_rejected
        PASS [   0.020s] (2199/5472) aion-package project::config::tests::full_descriptor_round_trips_with_derived_and_explicit_outputs
        PASS [   0.011s] (2200/5472) aion-package project::config::tests::invalid_schema_json_returns_schema_parse_with_path
        PASS [   0.020s] (2201/5472) aion-package project::config::tests::generated_awl_sidecar_supplies_additional_workflow_entries
        PASS [   0.032s] (2202/5472) aion-package project::config::tests::empty_activity_strings_return_config_invalid
        PASS [   0.014s] (2203/5472) aion-package project::config::tests::missing_descriptor_returns_config_missing
        PASS [   0.014s] (2204/5472) aion-package project::config::tests::omitting_each_required_field_returns_config_parse_naming_it
        PASS [   0.020s] (2205/5472) aion-package project::config::tests::missing_schema_file_returns_actionable_schema_missing
        PASS [   0.010s] (2206/5472) aion-package project::config::tests::unknown_keys_in_any_table_return_config_parse_naming_them
        PASS [   0.013s] (2207/5472) aion-package project::config::tests::textually_distinct_outputs_naming_the_same_file_conflict
        PASS [   0.027s] (2208/5472) aion-package project::config::tests::include_source_defaults_to_true_without_package_table
        PASS [   0.017s] (2209/5472) aion-package project::config::tests::unsafe_entry_modules_return_config_invalid
        PASS [   0.018s] (2210/5472) aion-package project::config::tests::zero_timeout_returns_config_invalid
        PASS [   0.166s] (2211/5472) aion-package package::tests::inflate_bomb_past_bounded_budget_is_refused_reporting_the_limit
        PASS [   0.013s] (2212/5472) aion-package project::confine::tests::dot_and_inside_root_dotdot_components_fold_away
        PASS [   0.017s] (2213/5472) aion-package project::confine::tests::absolute_paths_are_rejected_with_field_and_declared_path
        PASS [   0.025s] (2214/5472) aion-package project::config::tests::workflow_types_must_be_unique_across_every_project_entry
        PASS [   0.017s] (2215/5472) aion-package project::confine::tests::relative_paths_resolve_under_root
        PASS [   0.018s] (2216/5472) aion-package project::config::tests::zero_workflow_tables_return_config_invalid
        PASS [   0.023s] (2217/5472) aion-package project::confine::tests::dotdot_climbing_above_root_is_rejected
        PASS [   0.015s] (2218/5472) aion-package project::discover::tests::gleam_toml_dependency_missing_from_lockfile_is_unresolved
        PASS [   0.011s] (2219/5472) aion-package project::discover::tests::missing_build_directory_returns_project_not_built
        PASS [   0.021s] (2220/5472) aion-package project::discover::tests::cross_package_duplicate_module_carries_both_provenances
        PASS [   0.018s] (2221/5472) aion-package project::discover::tests::missing_gleam_toml_returns_gleam_toml_missing
        PASS [   0.021s] (2222/5472) aion-package project::discover::tests::missing_closure_package_ebin_returns_project_not_built
        PASS [   0.033s] (2223/5472) aion-package project::discover::tests::dev_dependency_modules_are_excluded_and_reported
        PASS [   0.032s] (2224/5472) aion-package project::discover::tests::discovers_production_closure_modules_with_exact_bytes
        PASS [   0.018s] (2225/5472) aion-package project::discover::tests::transitive_requirement_missing_from_lockfile_is_unresolved
        PASS [   0.025s] (2226/5472) aion-package project::discover::tests::missing_src_directory_returns_source_read
        PASS [   0.030s] (2227/5472) aion-package project::discover::tests::missing_lockfile_returns_gleam_metadata_read
        PASS [   0.021s] (2228/5472) aion-package project::discover::tests::sources_are_collected_recursively_with_logical_names
        PASS [   0.025s] (2229/5472) aion-package project::discover::tests::unparseable_gleam_metadata_returns_gleam_metadata_parse
        PASS [   0.031s] (2230/5472) aion-package project::discover::tests::sdk_test_modules_are_filtered_only_from_aion_flow_and_reported
        PASS [   0.018s] (2231/5472) aion-package project::error::tests::package_error_converts_transparently
        PASS [   0.025s] (2232/5472) aion-package project::discover::tests::user_module_with_reserved_name_is_discovered_not_filtered
        PASS [   0.023s] (2233/5472) aion-package project::error::tests::packaging_error_is_send_sync_and_static
        PASS [   0.026s] (2234/5472) aion-package project::error::tests::display_messages_name_the_failed_condition
        PASS [   0.016s] (2235/5472) aion-package structure::arms::tests::two_expression_arms_split_cleanly
        PASS [   0.017s] (2236/5472) aion-package structure::arms::tests::wildcard_arm_is_labelled
        PASS [   0.019s] (2237/5472) aion-package structure::arms::tests::block_body_arm_is_bounded_by_braces
        PASS [   5.754s] (2238/5472) aion-cli::one_motion_live_e2e an_operator_timeout_stops_the_wait_and_leaves_the_run_running
        PASS [   0.017s] (2239/5472) aion-package structure::control_flow::unit::recognise_covers_the_vocabulary
        PASS [   0.011s] (2240/5472) aion-package structure::determinism::tests::entropy_in_a_helper_passed_as_a_value_is_flagged
        PASS [   0.022s] (2241/5472) aion-package structure::determinism::tests::direct_wall_clock_call_is_flagged
        PASS [   0.023s] (2242/5472) aion-package structure::determinism::tests::clean_workflow_has_no_violations
        PASS [   0.027s] (2243/5472) aion-package structure::control_flow::unit::map_functions_finds_bodies
        PASS [   0.019s] (2244/5472) aion-package structure::determinism::tests::forbidden_word_inside_a_string_literal_is_not_flagged
        PASS [   0.019s] (2245/5472) aion-package structure::determinism::tests::multiple_distinct_calls_are_all_reported
        PASS [   0.023s] (2246/5472) aion-package structure::determinism::tests::entropy_in_a_reachable_helper_is_flagged
        PASS [   0.021s] (2247/5472) aion-package structure::determinism::tests::missing_entry_function_is_a_loud_error
        PASS [   0.017s] (2248/5472) aion-package structure::facts::tests::aliased_workflow_import_is_honoured
        PASS [   0.021s] (2249/5472) aion-package structure::determinism::tests::mutually_recursive_helpers_terminate
        PASS [   0.016s] (2250/5472) aion-package structure::facts::tests::missing_workflow_import_is_an_error
        PASS [   0.009s] (2251/5472) aion-package structure::ident::tests::snake_identifiers_are_classified
        PASS [   0.026s] (2252/5472) aion-package structure::determinism::tests::unreachable_helper_violation_is_not_flagged
        PASS [   0.022s] (2253/5472) aion-package structure::facts::tests::reads_typed_entry_and_counts_reachable_timers
        PASS [   0.017s] (2254/5472) aion-package structure::ident::tests::reserved_words_are_recognised
        PASS [   0.018s] (2255/5472) aion-package structure::facts::tests::unreachable_timer_is_not_counted
        PASS [   0.027s] (2256/5472) aion-package structure::facts::tests::missing_define_call_is_an_error
        PASS [   0.010s] (2257/5472) aion-package structure::reader::tests::last_identifier_argument_reads_the_define_entry
        PASS [   0.022s] (2258/5472) aion-package structure::facts::tests::timer_in_a_helper_passed_as_a_value_is_counted
        PASS [   0.026s] (2259/5472) aion-package structure::reader::tests::end_of_call_skips_balanced_args
        PASS [   0.024s] (2260/5472) aion-package structure::reader::tests::leading_local_call_detects_helper
        PASS [   0.016s] (2261/5472) aion-package structure::reader::tests::run_activity_name_strips_suffix
        PASS [   0.023s] (2262/5472) aion-package structure::reader::tests::last_identifier_argument_rejects_a_qualified_last_argument
        PASS [   0.029s] (2263/5472) aion-package structure::reader::tests::last_identifier_argument_ignores_nested_call_arguments
        PASS [   0.019s] (2264/5472) aion-package structure::reader::tests::leading_local_call_rejects_qualified
        PASS [   0.020s] (2265/5472) aion-package structure::reader::tests::match_brace_honours_nesting
        PASS [   0.007s] (2266/5472) aion-package structure::scan::tests::qualified_calls_are_recognised
        PASS [   0.026s] (2267/5472) aion-package structure::scan::tests::arrow_does_not_swallow_a_lone_minus
        PASS [   0.028s] (2268/5472) aion-package structure::scan::tests::braces_arrow_and_comma_are_emitted
        PASS [   0.028s] (2269/5472) aion-package structure::scan::tests::escapes_in_strings_are_unescaped
        PASS [   0.018s] (2270/5472) aion-package structure::tests::aliased_import_shim_style_is_followed
        PASS [   0.020s] (2271/5472) aion-package structure::scan::tests::string_contents_do_not_match_but_are_recoverable
        PASS [   0.018s] (2272/5472) aion-package structure::tests::branched_graph_is_refused_by_the_bounded_round_trip
        PASS [   0.025s] (2273/5472) aion-package structure::tests::aliased_workflow_import_is_supported
        PASS [   0.022s] (2274/5472) aion-package structure::tests::append_run_unknown_activity_is_refused
        PASS [   0.033s] (2275/5472) aion-package structure::scan::tests::line_comments_are_excluded
        PASS [   0.024s] (2276/5472) aion-package structure::tests::c23_oracle_matches_real_branches_and_rejects_a_flat_sequence
        PASS [   0.011s] (2277/5472) aion-package structure::tests::entry_without_workflow_import_is_a_loud_error
        PASS [   0.027s] (2278/5472) aion-package structure::tests::data_case_with_two_primitive_arms_is_a_branch
        PASS [   0.020s] (2279/5472) aion-package structure::tests::entry_reaching_execute_directly_and_through_define_walks_it_twice
        PASS [   0.026s] (2280/5472) aion-package structure::tests::delta_targeting_absent_node_is_refused
        PASS [   0.028s] (2281/5472) aion-package structure::tests::define_with_non_bare_last_argument_is_a_no_op_walk
        PASS [   0.035s] (2282/5472) aion-package structure::tests::define_naming_the_entry_function_is_recursion_guarded
        PASS [   0.027s] (2283/5472) aion-package structure::tests::each_primitive_is_extracted_with_its_correlation_key
        PASS [   5.994s] (2284/5472) aion-cli::one_motion_live_e2e a_declared_document_timeout_terminates_the_one_motion_await
        PASS [   0.027s] (2285/5472) aion-package structure::tests::extraction_is_deterministic
        PASS [   0.021s] (2286/5472) aion-package structure::tests::fan_out_members_are_individual_run_nodes
        PASS [   0.020s] (2287/5472) aion-package structure::tests::missing_entry_function_is_a_loud_error
        PASS [   0.030s] (2288/5472) aion-package structure::tests::fan_out_member_undeclared_activity_is_rejected
        PASS [   0.015s] (2289/5472) aion-package structure::tests::shim_style_entry_yields_the_same_graph_as_direct_style
        PASS [   0.018s] (2290/5472) aion-package structure::tests::run_node_with_undeclared_activity_is_rejected
        PASS [   0.019s] (2291/5472) aion-package structure::tests::regeneration_does_not_mutate_the_package_or_graph
        PASS [   0.013s] (2292/5472) aion-package structure::tests::spawn_carries_child_name_and_ordinal
        PASS [   0.025s] (2293/5472) aion-package structure::tests::primitive_case_mints_a_labelled_branch
        PASS [   0.029s] (2294/5472) aion-package structure::tests::non_utf8_entry_source_is_a_loud_error
        PASS [   0.023s] (2295/5472) aion-package structure::tests::primitive_free_helper_is_pruned
        PASS [   0.031s] (2296/5472) aion-package structure::tests::missing_entry_source_is_a_loud_error
        PASS [   0.030s] (2297/5472) aion-package structure::tests::order_saga_extracts_faithful_branched_control_flow
        PASS [   0.014s] (2298/5472) aion-package structure::tests::unbounded_case_surfaces_as_an_opaque_node
        PASS [   0.014s] (2299/5472) aion-package version::tests::loaded_package_produces_matching_version_record
        PASS [   0.020s] (2300/5472) aion-package::builder source_inclusion_does_not_change_manifest_version
        PASS [   0.007s] (2301/5472) aion-package::package loaded_hash_matches_canonical_contract_bound_hash
        PASS [   0.017s] (2302/5472) aion-package::package corrupted_beam_contents_return_integrity_mismatch
        PASS [   0.019s] (2303/5472) aion-package::package deployed_modules_match_namespace_transform_for_every_module
        PASS [   0.027s] (2304/5472) aion-package::builder write_to_bytes_produces_fixed_zip_layout
        PASS [   0.029s] (2305/5472) aion-package::builder identical_inputs_produce_byte_identical_archives
        PASS [   0.030s] (2306/5472) aion-package::package builder_produced_package_loads_successfully
        PASS [   0.037s] (2307/5472) aion-package::golden_gleam_exec the_skip_is_visible_in_a_passing_run
        PASS [   0.027s] (2308/5472) aion-package::project_determinism library_archive_is_byte_identical_to_direct_builder_construction
        PASS [   0.016s] (2309/5472) aion-package::roundtrip source_inclusion_does_not_change_loaded_content_hash_or_manifest_version
        PASS [   0.027s] (2310/5472) aion-package::roundtrip identical_inputs_produce_byte_identical_packages
        PASS [   0.033s] (2311/5472) aion-package::roundtrip altering_packed_beam_contents_is_rejected_as_integrity_mismatch
        PASS [   0.031s] (2312/5472) aion-package::roundtrip package_round_trips_without_source
        PASS [   0.032s] (2313/5472) aion-package::roundtrip package_round_trips_manifest_beams_source_and_deployed_names
        PASS [   0.015s] (2314/5472) aion-proto build_coverage::generated_stubs_compiled_into_this_build_so_contracts_can_be_checked
        PASS [   0.008s] (2315/5472) aion-proto convert::tests::payload_round_trips_all_json_kinds_and_raw_bytes
        PASS [   0.020s] (2316/5472) aion-proto convert::tests::activity_id_round_trips_both_directions
        PASS [   0.015s] (2317/5472) aion-proto convert::tests::envelope_rejects_missing_payload
        PASS [   0.022s] (2318/5472) aion-proto convert::tests::core_event_round_trips_through_wire_envelope
        PASS [   0.017s] (2319/5472) aion-proto convert::tests::timer_id_rejects_missing_and_empty_name
        PASS [   0.022s] (2320/5472) aion-proto convert::tests::run_id_round_trips_both_directions
        PASS [   0.015s] (2321/5472) aion-proto convert::tests::workflow_id_round_trips_both_directions
        PASS [   0.014s] (2322/5472) aion-proto convert::tests::workflow_status_round_trips_both_directions
        PASS [   0.029s] (2323/5472) aion-proto convert::tests::timer_id_round_trips_both_directions
        PASS [   0.020s] (2324/5472) aion-proto deploy::tests::deploy_shapes_round_trip_through_json
        PASS [   0.017s] (2325/5472) aion-proto error::tests::proto_numeric_values_are_pinned
        PASS [   0.021s] (2326/5472) aion-proto error::tests::proto_round_trips_every_code
        PASS [   0.025s] (2327/5472) aion-proto error::tests::json_codes_match_as_str_and_round_trip
        PASS [   0.015s] (2328/5472) aion-proto error::tests::rejects_unspecified_proto_code
        PASS [   0.010s] (2329/5472) aion-proto error::tests::representative_documented_mappings_use_stable_codes
        PASS [   0.012s] (2330/5472) aion-proto events::tests::per_workflow_resume_cursor_json_shape_is_pinned
        PASS [   0.021s] (2331/5472) aion-proto error::tests::string_codes_are_pinned
        PASS [   0.013s] (2332/5472) aion-proto events::tests::streamed_event_rejects_missing_envelope
        PASS [   0.018s] (2333/5472) aion-proto error::tests::wire_error_is_send_sync
        PASS [   0.012s] (2334/5472) aion-proto events::tests::streamed_event_rejects_namespace_mismatch
        PASS [   0.023s] (2335/5472) aion-proto events::tests::per_workflow_resume_cursor_round_trips_prost
        PASS [   0.015s] (2336/5472) aion-proto events::tests::subscription_request_round_trips_all_variants
        PASS [   0.018s] (2337/5472) aion-proto events::tests::subscription_request_without_resume_field_decodes_to_none
        PASS [   0.021s] (2338/5472) aion-proto events::tests::streamed_event_round_trips_core_event
        PASS [   0.018s] (2339/5472) aion-proto worker::tests::activity_error_round_trips_preserving_classification
        PASS [   0.021s] (2340/5472) aion-proto worker::tests::activity_error_result_round_trips_through_serde_and_proto
        PASS [   0.010s] (2341/5472) aion-proto worker::tests::activity_task_attempt_uses_wire_tag_five
        PASS [   0.014s] (2342/5472) aion-proto worker::tests::activity_success_result_round_trips_through_serde_and_proto
        PASS [   0.012s] (2343/5472) aion-proto worker::tests::liveness_answer_round_trips_and_echoes_the_ping_sequence_field
        PASS [   0.012s] (2344/5472) aion-proto worker::tests::liveness_frames_are_disjoint_from_the_other_stream_frames
        PASS [   0.013s] (2345/5472) aion-proto worker::tests::drain_request_round_trips_through_serde_and_proto
        PASS [   0.015s] (2346/5472) aion-proto worker::tests::heartbeat_round_trips_through_serde_and_proto
        PASS [   0.025s] (2347/5472) aion-proto worker::tests::activity_task_round_trips_through_serde_and_proto
        PASS [   0.013s] (2348/5472) aion-proto worker::tests::liveness_ping_round_trips_and_pins_its_wire_tags
        PASS [   0.012s] (2349/5472) aion-proto worker::tests::register_ack_round_trips_through_serde_and_proto
        PASS [   0.010s] (2350/5472) aion-proto worker::tests::server_to_worker_ack_arms_pin_oneof_tags_three_and_four
        PASS [   0.012s] (2351/5472) aion-proto worker::tests::result_ack_round_trips_through_serde_and_proto
        PASS [   0.009s] (2352/5472) aion-proto worker::tests::worker_instance_identity_uses_wire_tag_seven
        PASS [   0.011s] (2353/5472) aion-proto worker::tests::worker_registration_round_trips_through_serde_and_proto
        PASS [   0.013s] (2354/5472) aion-proto workflow::tests::query_round_trips_json_and_proto
        PASS [   0.015s] (2355/5472) aion-proto worker::tests::worker_registration_task_queue_and_node_use_wire_tags_three_and_four
        PASS [   0.018s] (2356/5472) aion-proto worker::tests::worker_registration_repeated_namespaces_round_trip
        PASS [   0.024s] (2357/5472) aion-proto workflow::tests::list_workflows_round_trips_json_and_proto
        PASS [   0.013s] (2358/5472) aion-proto workflow::tests::reopen_round_trips_json_and_proto
        PASS [   0.018s] (2359/5472) aion-proto workflow::tests::rename_messages_agree_with_the_generated_stubs
        PASS [   0.015s] (2360/5472) aion-proto workflow::tests::start_workflow_display_name_absent_round_trips
        PASS [   0.020s] (2361/5472) aion-proto workflow::tests::rename_round_trips_json_and_proto
        PASS [   0.012s] (2362/5472) aion-proto workflow::tests::start_workflow_display_name_is_the_same_wire_field_as_the_generated_stub
        PASS [   0.016s] (2363/5472) aion-proto workflow::tests::start_workflow_round_trips_json_and_proto
        PASS [   0.014s] (2364/5472) aion-rs child::spawn::tests::spawn_failure_after_record_keeps_the_durable_start
        PASS [   0.033s] (2365/5472) aion-rs activity::bridge::tests::dispatcher_is_accessible_after_install_on_engine_state
        PASS [   0.230s] (2366/5472) aion-package::package_project_hello_world packages_hello_world_byte_identically_to_direct_construction
        PASS [   0.012s] (2367/5472) aion-rs child::spawn::tests::spawn_records_started_before_requesting_the_child_start
        PASS [   0.010s] (2368/5472) aion-rs child::spawn::tests::spawn_rejects_engine_echoing_a_different_child_identity
        PASS [   0.014s] (2369/5472) aion-rs durability::command::tests::command_keys_round_trip_for_cursor_matched_variants
        PASS [   0.011s] (2370/5472) aion-rs durability::command::tests::resolutions_round_trip_by_equality
        PASS [   0.052s] (2371/5472) aion-rs activity::dispatch::tests::failing_activity_surfaces_typed_error_with_trapped_exit
        PASS [   0.026s] (2372/5472) aion-rs durability::correlation::tests::derives_positional_child_ordinals_independent_of_sequence_numbers
        PASS [   0.020s] (2373/5472) aion-rs durability::correlation::tests::derives_signal_occurrence_indices_by_name
        PASS [   0.022s] (2374/5472) aion-rs durability::correlation::tests::interleaved_async_arrivals_do_not_shift_child_ordinals
        PASS [   0.022s] (2375/5472) aion-rs durability::cursor::tests::a_with_timeout_outcome_is_not_stranded_for_the_next_command
        PASS [   0.014s] (2376/5472) aion-rs durability::cursor::tests::adjacent_timer_start_and_fire_resolve_unchanged
        PASS [   0.014s] (2377/5472) aion-rs durability::cursor::tests::fast_forward_to_child_terminal_exhausts_when_no_terminal_recorded
        PASS [   0.087s] (2378/5472) aion-rs activity::dispatch::tests::successful_activity_result_is_delivered_to_workflow
        PASS [   0.017s] (2379/5472) aion-rs durability::cursor::tests::fast_forward_to_child_terminal_skips_consumed_commands
        PASS [   0.026s] (2380/5472) aion-rs durability::cursor::tests::new_accepts_in_order_history_and_exposes_starting_sequence
        PASS [   0.023s] (2381/5472) aion-rs durability::cursor::tests::new_rejects_decreasing_sequence_order
        PASS [   0.024s] (2382/5472) aion-rs durability::cursor::tests::pause_resume_markers_are_invisible_to_the_cursor
        PASS [   0.016s] (2383/5472) aion-rs durability::cursor::tests::reopen_supersedes_terminal_failure_and_exhausts
        PASS [   0.020s] (2384/5472) aion-rs durability::cursor::tests::reopen_then_new_attempt_resolves_to_the_new_outcome
        PASS [   0.036s] (2385/5472) aion-rs durability::cursor::tests::reopen_of_a_different_activity_does_not_supersede_this_failure
        PASS [   0.022s] (2386/5472) aion-rs durability::cursor::tests::reopen_with_new_attempt_leaves_no_straggler_for_later_commands
        PASS [   0.074s] (2387/5472) aion-rs durability::cursor::tests::fast_forward_and_resolution_smoke_over_large_history
        PASS [   0.016s] (2388/5472) aion-rs durability::cursor::tests::resolve_activity_skips_interleaved_child_terminal
        PASS [   0.018s] (2389/5472) aion-rs durability::cursor::tests::resolve_activity_resolves_interleaved_parallel_activity_ranges
        PASS [   0.016s] (2390/5472) aion-rs durability::cursor::tests::resolve_activity_skips_interleaved_signal_and_leaves_it_matchable
        PASS [   0.038s] (2391/5472) aion-rs durability::cursor::tests::reports_mismatch_for_different_next_family
        PASS [   0.027s] (2392/5472) aion-rs durability::cursor::tests::resolve_child_terminal_reports_mismatch_without_skipping_in_strict_replay
        PASS [   0.030s] (2393/5472) aion-rs durability::cursor::tests::resolves_activity_match_then_reports_exhaustion
        PASS [   0.034s] (2394/5472) aion-rs durability::cursor::tests::resolve_activity_still_mismatches_on_wrong_anchor_key
        PASS [   0.028s] (2395/5472) aion-rs durability::cursor::tests::skips_non_matchable_lifecycle_events_before_resolving
        PASS [   0.019s] (2396/5472) aion-rs durability::cursor::tests::terminal_failure_without_reopen_still_matches_unchanged
        PASS [   0.039s] (2397/5472) aion-rs durability::cursor::tests::returns_terminal_activity_failure_as_recorded_outcome
        PASS [   0.022s] (2398/5472) aion-rs durability::cursor::tests::timer_resolves_across_an_interleaved_parallel_timer
        PASS [   0.026s] (2399/5472) aion-rs durability::cursor::tests::timer_resolves_across_an_interleaved_activity_and_child_terminal
        PASS [   0.020s] (2400/5472) aion-rs durability::cursor::tests::timer_resolves_across_an_interleaved_rename
        PASS [   0.026s] (2401/5472) aion-rs durability::cursor::tests::timer_resolves_across_interleaved_pause_and_resume
        PASS [   0.036s] (2402/5472) aion-rs durability::cursor::tests::timer_resolves_across_an_interleaved_signal
        PASS [   0.019s] (2403/5472) aion-rs durability::determinism::tests::identical_recorded_sequences_have_identical_now_values
        PASS [   0.032s] (2404/5472) aion-rs durability::cursor::tests::walks_retry_failures_to_eventual_activity_success
        PASS [   0.016s] (2405/5472) aion-rs durability::determinism::tests::now_starts_at_workflow_started_and_advances_with_recorded_events
        PASS [   0.033s] (2406/5472) aion-rs durability::cursor::tests::timer_without_a_recorded_terminal_still_resolves_started
        PASS [   0.029s] (2407/5472) aion-rs durability::error::tests::durability_error_display_mentions_underlying_cause
        PASS [   0.036s] (2408/5472) aion-rs durability::error::tests::non_determinism_display_includes_context
        PASS [   0.039s] (2409/5472) aion-rs durability::executor::executor_tests::recorded_history_returns_resolutions_without_live_calls
        PASS [   0.039s] (2410/5472) aion-rs durability::executor::executor_tests::resume_live_activity_records_result_and_replay_uses_cache
        PASS [   0.046s] (2411/5472) aion-rs durability::executor::executor_tests::live_executor_is_object_safe
        PASS [   0.034s] (2412/5472) aion-rs durability::executor::executor_tests::resume_live_activity_rejects_retryable_failure_without_recording_terminal_outcome
        PASS [   0.295s] (2413/5472) aion-rs activity::dispatch::tests::dispatch_spawns_linked_child_and_uses_dirty_registration
        PASS [   0.029s] (2414/5472) aion-rs durability::recorder::fan_out_tests::fan_out_completion_drops_after_a_terminal_failure
        PASS [   0.031s] (2415/5472) aion-rs durability::recorder::fan_out_tests::fan_out_completion_drops_duplicate_for_resolved_ordinal
        PASS [   0.035s] (2416/5472) aion-rs durability::recorder::fan_out_tests::fan_out_completion_drops_for_cancelled_ordinal
        PASS [   0.024s] (2417/5472) aion-rs durability::recorder::fan_out_tests::fan_out_completion_records_for_scheduled_but_not_terminal_ordinal
        PASS [   0.015s] (2418/5472) aion-rs durability::recorder::fan_out_tests::fan_out_completion_sequence_conflict_surfaces_without_advancing
        PASS [   0.029s] (2419/5472) aion-rs durability::recorder::fan_out_tests::fan_out_completion_records_first_terminal_for_unresolved_ordinal
        PASS [   0.045s] (2420/5472) aion-rs durability::recorder::fan_out_tests::fan_out_completion_records_failed_terminal
        PASS [   0.030s] (2421/5472) aion-rs durability::recorder::fan_out_tests::fan_out_completion_records_over_a_retryable_attempt_record
        PASS [   0.014s] (2422/5472) aion-rs durability::recorder::fan_out_tests::fan_out_dispatch_empty_items_is_a_no_op
        PASS [   0.031s] (2423/5472) aion-rs durability::recorder::fan_out_tests::fan_out_dispatch_appends_2n_events_in_scheduled_started_order
        PASS [   0.026s] (2424/5472) aion-rs durability::recorder::fan_out_tests::fan_out_dispatch_sequence_conflict_surfaces_without_advancing
        PASS [   0.023s] (2425/5472) aion-rs durability::recorder::fan_out_tests::fan_out_dispatch_stages_n_outbox_rows_with_dispatch_keys
        PASS [   0.021s] (2426/5472) aion-rs durability::recorder::tests::invalid_search_attributes_return_error_without_appending
        PASS [   0.045s] (2427/5472) aion-rs durability::recorder::fan_out_tests::fan_out_dispatch_is_atomic_on_store_failure
        PASS [   0.023s] (2428/5472) aion-rs durability::recorder::tests::recorder_visibility_updates_after_search_attributes
        PASS [   0.048s] (2429/5472) aion-rs durability::recorder::tests::continue_as_new_does_not_settle_outbox_rows
        PASS [   0.026s] (2430/5472) aion-rs durability::recorder::tests::recorder_advances_expected_sequence_between_appends
        PASS [  10.040s] (2431/5472) aion-cli::generate_types_first_e2e generate_compiles_round_trips_and_checks
        PASS [   0.025s] (2432/5472) aion-rs durability::recorder::tests::records_activity_events_in_sequence_order
        PASS [   0.046s] (2433/5472) aion-rs durability::recorder::tests::reconcile_head_forward_is_forward_only_and_restores_the_append_path
        PASS [   0.023s] (2434/5472) aion-rs durability::recorder::tests::records_workflow_continued_as_new_terminal_event
        PASS [   0.031s] (2435/5472) aion-rs durability::recorder::tests::records_workflow_timed_out_terminal_and_projects_timed_out
        PASS [   0.033s] (2436/5472) aion-rs durability::recorder::tests::records_workflow_reopened_and_projects_running
        PASS [   0.023s] (2437/5472) aion-rs durability::recorder::tests::sequence_overflow_returns_error_without_appending
        PASS [   0.047s] (2438/5472) aion-rs durability::recorder::tests::records_validated_search_attributes_updated_event
        PASS [   0.038s] (2439/5472) aion-rs durability::recorder::tests::resume_at_continues_from_existing_history_head
        PASS [   0.034s] (2440/5472) aion-rs durability::recorder::tests::sequence_conflict_surfaces_without_advancing_or_retrying
        PASS [   0.035s] (2441/5472) aion-rs durability::recorder::tests::started_with_attributes_appends_both_events_in_one_atomic_batch
        PASS [   0.028s] (2442/5472) aion-rs durability::recorder::tests::started_with_empty_attributes_appends_only_the_start_event
        PASS [   0.028s] (2443/5472) aion-rs durability::recorder::tests::started_with_invalid_attributes_appends_nothing
        PASS [   0.026s] (2444/5472) aion-rs durability::recorder::tests::workflow_terminals_settle_outbox_rows_and_settle_errors_are_nonfatal
        PASS [   0.016s] (2445/5472) aion-rs durability::replay::tests::ignores_terminal_events_for_other_workflows
        PASS [   0.037s] (2446/5472) aion-rs durability::recorder::tests::visibility_upsert_failure_after_append_logs_warning_and_succeeds
        PASS [   0.039s] (2447/5472) aion-rs durability::recovery::tests::recover_lists_active_and_reads_each_history
        PASS [   0.026s] (2448/5472) aion-rs durability::replay::tests::reopened_activity_resolves_live_not_terminal
        PASS [   0.046s] (2449/5472) aion-rs durability::replay::tests::drive_reports_terminal_history_without_resume_live
        PASS [   0.023s] (2450/5472) aion-rs durability::replay::tests::terminal_history_accepts_matching_complete_workflow_command
        PASS [   0.030s] (2451/5472) aion-rs durability::replay::tests::step_returns_recorded_resolutions_and_advances_now
        PASS [   0.031s] (2452/5472) aion-rs durability::replay_inspect::replay_inspect_tests::divergent_command_is_built_from_the_resolver_error
        PASS [   0.040s] (2453/5472) aion-rs durability::replay::tests::terminal_history_rejects_extra_non_terminal_command
        PASS [   0.034s] (2454/5472) aion-rs durability::replay_inspect::replay_inspect_tests::inspect_run_errors_when_run_segment_is_absent
        PASS [   0.029s] (2455/5472) aion-rs durability::replay_inspect::replay_inspect_tests::inspect_run_random_projection_is_deterministic_and_run_distinct
        PASS [   0.037s] (2456/5472) aion-rs durability::replay_inspect::replay_inspect_tests::inspect_run_now_equals_each_event_recorded_at
        PASS [   0.041s] (2457/5472) aion-rs durability::replay_inspect::replay_inspect_tests::inspect_run_projects_one_step_per_event_with_resolutions
        PASS [   0.040s] (2458/5472) aion-rs durability::replay_inspect::replay_inspect_tests::inspect_run_random_projection_equals_the_production_formula
        PASS [   0.030s] (2459/5472) aion-rs durability::replay_inspect::replay_inspect_tests::inspect_run_surfaces_the_divergent_command
        PASS [   0.039s] (2460/5472) aion-rs durability::replay_inspect::replay_inspect_tests::inspect_run_scopes_to_the_current_run_segment
        PASS [   0.024s] (2461/5472) aion-rs durability::replay_inspect::replay_inspect_tests::what_if_activity_completion_reproduces_the_recorded_path
        PASS [   0.040s] (2462/5472) aion-rs durability::replay_inspect::replay_inspect_tests::random_int_at_rejects_an_inverted_range
        PASS [   0.039s] (2463/5472) aion-rs durability::replay_inspect::replay_inspect_tests::recorded_divergence_round_trips_through_the_real_failure_format
        PASS [   0.033s] (2464/5472) aion-rs durability::replay_inspect::replay_inspect_tests::what_if_child_completion_resolves_through_the_real_replay
        PASS [   0.035s] (2465/5472) aion-rs durability::replay_inspect::replay_inspect_tests::what_if_activity_failure_diverges_from_the_recorded_completion
        PASS [   0.020s] (2466/5472) aion-rs durability::replay_inspect::replay_inspect_tests::what_if_child_failure_resolves_through_the_real_replay
        PASS [   0.022s] (2467/5472) aion-rs durability::replay_inspect::replay_inspect_tests::what_if_rejects_an_absent_fork_sequence
        PASS [   0.025s] (2468/5472) aion-rs durability::replay_inspect::replay_inspect_tests::what_if_rejects_a_mismatched_mock_family
        PASS [   0.033s] (2469/5472) aion-rs durability::resolver::tests::dangling_retryable_failure_resumes_live_for_redispatch
        PASS [  10.261s] (2470/5472) aion-cli::generate_e2e generate_check_gates_workflow_toml_drift
        PASS [   0.039s] (2471/5472) aion-rs durability::resolver::tests::genuinely_reordered_activity_anchors_fail_typed
        PASS [   0.043s] (2472/5472) aion-rs durability::resolver::tests::interleaved_async_arrival_inside_activity_range_replays_clean
        PASS [   0.048s] (2473/5472) aion-rs durability::replay_inspect::replay_inspect_tests::what_if_timer_firing_resolves_through_the_real_replay
        PASS [   0.058s] (2474/5472) aion-rs durability::replay_inspect::replay_inspect_tests::what_if_signal_delivery_resolves_through_the_real_replay
        PASS [   0.045s] (2475/5472) aion-rs durability::resolver::tests::maps_terminal_failures_to_recorded_resolutions
        PASS [   0.051s] (2476/5472) aion-rs durability::resolver::tests::resolves_all_recorded_families_through_single_entry_point
        PASS [   0.019s] (2477/5472) aion-rs durability::seq::tests::starts_at_zero_by_default
        PASS [   0.028s] (2478/5472) aion-rs durability::seq::tests::advances_only_after_success_is_marked
        PASS [   0.025s] (2479/5472) aion-rs durability::seq::tests::next_sequence_overflow_is_detectable_by_advance
        PASS [   0.035s] (2480/5472) aion-rs durability::resolver::tests::resolves_recorded_activity_then_resumes_live_at_history_end
        PASS [   0.032s] (2481/5472) aion-rs durability::seq::tests::starts_from_explicit_head
        PASS [   0.042s] (2482/5472) aion-rs engine::api::api_tests::lifecycle::live_timers_dedups_repeated_start
        PASS [   0.015s] (2483/5472) aion-rs engine::api::api_tests::lifecycle::live_timers_empty_history_is_empty
        PASS [   0.029s] (2484/5472) aion-rs engine::api::api_tests::lifecycle::live_timers_excludes_fired_and_cancelled
        PASS [   0.027s] (2485/5472) aion-rs engine::api::api_tests::lifecycle::live_timers_lists_started_and_unterminated
        PASS [   0.012s] (2486/5472) aion-rs engine::api::api_tests::lifecycle::live_timers_scopes_to_active_run_segment
        PASS [   0.286s] (2487/5472) aion-rs engine::admission::admission_tests::an_undeclared_queue_demands_nothing
        PASS [   0.316s] (2488/5472) aion-rs engine::api::api_tests::lifecycle::cancelled_workflow_leaves_no_orphan_for_recovery
        PASS [   0.325s] (2489/5472) aion-rs engine::api::api_tests::lifecycle::cancel_cancels_multiple_live_timers
        PASS [   0.331s] (2490/5472) aion-rs engine::api::api_tests::lifecycle::cancel_records_timer_cancelled_before_workflow_cancelled
        PASS [   0.047s] (2491/5472) aion-rs engine::api::api_tests::shutdown::a_second_failing_engine_shutdown_step_is_reported_not_swallowed
        PASS [   0.403s] (2492/5472) aion-rs engine::admission::admission_tests::a_superseded_version_with_no_live_run_binds_no_worker
        PASS [   0.076s] (2493/5472) aion-rs engine::api::api_tests::shutdown::a_failing_teardown_step_does_not_skip_the_ones_after_it
        PASS [   0.026s] (2494/5472) aion-rs engine::api::api_tests::shutdown::engine_drop_releases_the_descriptor_owning_task_runtime
        PASS [   0.451s] (2495/5472) aion-rs engine::admission::admission_tests::a_rollback_moves_the_demanded_version_with_the_route
        PASS [   0.468s] (2496/5472) aion-rs engine::admission::admission_tests::a_superseded_version_with_a_live_run_still_binds_every_worker
        PASS [   0.333s] (2497/5472) aion-rs engine::api::api_tests::lifecycle::result_returns_completed_payload
        PASS [   0.088s] (2498/5472) aion-rs engine::api::api_tests::shutdown::shutdown_is_idempotent
        PASS [   0.054s] (2499/5472) aion-rs engine::api::api_tests::shutdown::shutdown_rejects_subsequent_starts
        PASS [   0.036s] (2500/5472) aion-rs engine::builder::tests::arming_a_wheel_timer_after_the_engine_is_released_is_refused
        PASS [   0.313s] (2501/5472) aion-rs engine::api::api_tests::lifecycle::result_returns_failed_workflow_error
        PASS [   0.308s] (2502/5472) aion-rs engine::api::api_tests::lifecycle::start_then_cancel_records_started_then_cancelled
        PASS [   0.024s] (2503/5472) aion-rs engine::builder::tests::build_without_store_returns_missing_store
        PASS [   0.017s] (2504/5472) aion-rs engine::builder::tests::build_without_visibility_store_returns_missing_visibility_store
        PASS [   0.310s] (2505/5472) aion-rs engine::api::api_tests::shutdown::continue_as_new_unknown_workflow_returns_not_found
        PASS [   0.293s] (2506/5472) aion-rs engine::api::api_tests::shutdown::result_unknown_workflow_returns_not_found
        PASS [   0.332s] (2507/5472) aion-rs engine::api::api_tests::shutdown::list_workflows_merges_live_and_terminal_without_duplicates
        PASS [   0.300s] (2508/5472) aion-rs engine::api::api_tests::shutdown::shutdown_rejects_schedule_creation
        PASS [   0.049s] (2509/5472) aion-rs engine::builder::tests::event_streaming_conflicts_with_explicit_event_publisher
        PASS [   0.098s] (2510/5472) aion-rs engine::builder::tests::duplicate_host_nif_mfa_returns_typed_error
        PASS [   1.791s] (2511/5472) aion-package::schema_equivalence emitted_order_saga_schemas_are_semantically_equivalent_to_the_authored_originals
        PASS [   0.026s] (2512/5472) aion-rs engine::builder::tests::owned_shards_are_only_set_by_caller
        PASS [   0.317s] (2513/5472) aion-rs engine::builder::tests::event_streaming_delivers_recorder_appends_through_engine_subscribe
        PASS [   0.476s] (2514/5472) aion-rs engine::builder::tests::dropping_an_engine_without_shutdown_closes_the_completion_retry_epoch
        PASS [   0.346s] (2515/5472) aion-rs engine::builder::tests::in_memory_visibility_allows_build_without_visibility_store
        PASS [   0.023s] (2516/5472) aion-rs engine::builder::tests::query_timeout_is_only_set_by_caller
        PASS [   0.028s] (2517/5472) aion-rs engine::builder::tests::scheduler_threads_are_only_set_by_caller
        PASS [   0.019s] (2518/5472) aion-rs engine::builder::tests::the_completion_retry_ladder_reaches_the_runtime_configuration
        PASS [   0.022s] (2519/5472) aion-rs engine::builder::tests::the_jit_threshold_reaches_the_runtime_configuration
        PASS [   0.672s] (2520/5472) aion-rs engine::builder::tests::build_loads_already_loaded_package
        PASS [   0.015s] (2521/5472) aion-rs engine::builder::tests::visibility_reconciliation_interval_is_only_set_by_caller
        PASS [   0.565s] (2522/5472) aion-rs engine::builder::tests::empty_store_builds_coordinator_history_without_registry_or_supervision
        PASS [   0.772s] (2523/5472) aion-rs engine::builder::tests::build_loads_package_from_path
        PASS [   0.324s] (2524/5472) aion-rs engine::builder::tests::periodic_visibility_reconciliation_repairs_gap_after_startup
        PASS [   0.303s] (2525/5472) aion-rs engine::builder::tests::query_timeout_installs_the_concrete_query_seam
        PASS [   0.357s] (2526/5472) aion-rs engine::builder::tests::startup_reconciliation_backfills_completed_visibility
        PASS [   0.352s] (2527/5472) aion-rs engine::builder::tests::without_event_streaming_subscriptions_stay_on_deferred_empty_stream
        PASS [   0.312s] (2528/5472) aion-rs engine::delegated::tests::query_delegates_to_service_and_returns_payload
        PASS [   0.378s] (2529/5472) aion-rs engine::builder::tests::without_query_timeout_the_query_seam_stays_deferred
        PASS [   0.026s] (2530/5472) aion-rs engine::fence::tests::deposed_at_publish_leaves_scope_unchanged_and_recovers_nothing
        PASS [   0.048s] (2531/5472) aion-rs engine::fence::tests::deposed_at_acquire_recovers_nothing
        PASS [   0.038s] (2532/5472) aion-rs engine::fence::tests::falsifiability_external_execution_is_exactly_once_under_fix
        PASS [   0.320s] (2533/5472) aion-rs engine::delegated::tests::signal_delegates_to_router_and_unknown_returns_not_found
        PASS [   0.047s] (2534/5472) aion-rs engine::fence::tests::partial_win_a_fenced_b
        PASS [   0.337s] (2535/5472) aion-rs engine::delegated::tests::query_terminal_run_is_not_running_and_unknown_is_not_found
        PASS [   0.023s] (2536/5472) aion-rs engine::fence::tests::publish_fence_precedes_extend_for_every_survivor
        PASS [   0.013s] (2537/5472) aion-rs engine::gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.030s] (2538/5472) aion-rs engine::fence::tests::residual_window_depose_excludes_shard
        PASS [   0.020s] (2539/5472) aion-rs engine::gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.048s] (2540/5472) aion-rs engine::gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.041s] (2541/5472) aion-rs engine::gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.057s] (2542/5472) aion-rs engine::gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.038s] (2543/5472) aion-rs engine::gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.071s] (2544/5472) aion-rs engine::gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.039s] (2545/5472) aion-rs engine::gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.057s] (2546/5472) aion-rs engine::gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   0.018s] (2547/5472) aion-rs engine::shutdown_gate::tests::operations_are_refused_once_shutdown_begins
        PASS [   0.354s] (2548/5472) aion-rs engine::delegated::tests::signal_inside_the_registration_birth_window_waits_for_the_handle
        PASS [   0.087s] (2549/5472) aion-rs engine::shutdown_gate::tests::close_waits_for_admitted_operations_to_drain
        PASS [   0.368s] (2550/5472) aion-rs engine::delegated::tests::subscribe_delegates_to_publisher_stream_with_filter
        PASS [   0.121s] (2551/5472) aion-rs engine::startup::tests::a_cancelled_run_is_terminal_at_boot_and_never_has_a_writer_minted
        PASS [   0.112s] (2552/5472) aion-rs engine::startup::tests::a_paused_run_forced_into_the_active_list_is_never_made_resident
        PASS [   0.225s] (2553/5472) aion-rs engine::startup::tests::a_cancelled_run_forced_into_the_active_list_never_has_a_writer_minted
        PASS [   0.080s] (2554/5472) aion-rs engine::startup::tests::a_terminal_run_forced_into_the_active_list_is_never_made_resident
        PASS [   0.132s] (2555/5472) aion-rs engine::startup::tests::a_run_whose_pinned_version_cannot_load_is_retained_as_unrecoverable
        PASS [   0.805s] (2556/5472) aion-rs engine::delegated::tests::signal_for_a_started_run_with_no_handle_fails_typed_after_the_budget
        PASS [   0.019s] (2557/5472) aion-rs engine::startup::tests::paused_history_with_a_dangling_activity_requires_adoption
        PASS [   0.378s] (2558/5472) aion-rs engine::startup::tests::adoption_repairs_orphaned_future_dated_terminal_deadline
        PASS [   0.373s] (2559/5472) aion-rs engine::startup::tests::cold_boot_sweep_errors_on_a_live_handle_ordering_breach
        PASS [   0.418s] (2560/5472) aion-rs engine::startup::tests::cold_boot_repairs_predecessor_deadline_before_starting_the_successor
        PASS [   0.451s] (2561/5472) aion-rs engine::startup::tests::adoption_sweep_repairs_acquired_but_skips_owned_resident
        PASS [   0.356s] (2562/5472) aion-rs engine::startup::tests::recovered_monitor_installation_failure_drains_retained_completion
        PASS [   0.361s] (2563/5472) aion-rs engine::startup::tests::recovered_reconcile_failure_after_publication_runs_observed_abort
        PASS [   0.281s] (2564/5472) aion-rs engine::startup::tests::startup_sweep_retires_non_timeout_terminal_deadlines_but_skips_timed_out
        PASS [   0.304s] (2565/5472) aion-rs engine::startup::tests::sweep_start_race_lost_to_the_exit_monitor_is_benign
        PASS [   0.036s] (2566/5472) aion-rs engine_seam::tests::fake_can_inject_delivery_failure
        PASS [   0.030s] (2567/5472) aion-rs engine_seam::tests::fake_captures_delivered_message_for_resident_workflow
        PASS [   0.344s] (2568/5472) aion-rs engine::startup::tests::sweep_start_failure_without_a_successor_still_fails
        PASS [   1.105s] (2569/5472) aion-rs engine::reload::reload_tests::unload_after_result_before_terminal_registry_reconcile_succeeds
        PASS [   0.306s] (2570/5472) aion-rs engine::startup_sweeps::tests::adoption_sweep_repairs_the_candidate_without_a_concurrent_writer
        PASS [   0.063s] (2571/5472) aion-rs lifecycle::cancel_never_alive::tests::a_workflow_holding_a_handle_never_reaches_the_never_alive_branch
        PASS [   1.227s] (2572/5472) aion-rs engine::reload::reload_tests::unload_still_refuses_while_a_run_is_genuinely_live
        PASS [   0.064s] (2573/5472) aion-rs lifecycle::cancel_never_alive::tests::a_workflow_that_never_existed_is_still_not_found
        PASS [   0.341s] (2574/5472) aion-rs engine::startup_sweeps::tests::adoption_sweep_yields_to_a_concurrent_reopen_without_aborting
        PASS [   0.051s] (2575/5472) aion-rs lifecycle::cancel_never_alive::tests::the_same_run_is_cancellable_when_the_package_is_absent
        PASS [   0.016s] (2576/5472) aion-rs lifecycle::completion::tests::a_changed_fault_is_stated_at_once_and_does_not_defer_the_escalation
        PASS [   0.057s] (2577/5472) aion-rs lifecycle::completion::tests::a_failure_after_the_terminal_landed_reports_it_as_recorded
        PASS [   0.049s] (2578/5472) aion-rs lifecycle::completion::tests::a_lost_ownership_refusal_is_reported_not_retried
        PASS [   0.312s] (2579/5472) aion-rs lifecycle::cancel_never_alive::tests::a_run_that_was_never_alive_can_be_cancelled
        PASS [   0.314s] (2580/5472) aion-rs lifecycle::cancel_never_alive::tests::a_run_with_no_recorded_verdict_is_refused_without_calling_it_missing
        PASS [   0.319s] (2581/5472) aion-rs lifecycle::cancel_never_alive::tests::a_run_whose_pinned_package_now_loads_is_refused
        PASS [   0.053s] (2582/5472) aion-rs lifecycle::completion::tests::a_reopened_run_that_finishes_again_records_its_terminal
        PASS [   0.059s] (2583/5472) aion-rs lifecycle::completion::tests::a_recorded_monitor_stands_down_inside_the_reopen_window
        PASS [   0.323s] (2584/5472) aion-rs lifecycle::cancel_never_alive::tests::a_second_cancellation_of_the_same_run_is_refused
        PASS [   0.068s] (2585/5472) aion-rs lifecycle::completion::tests::a_reopened_run_is_not_appended_into_by_a_monitor_that_never_recorded
        PASS [   0.048s] (2586/5472) aion-rs lifecycle::completion::tests::a_resumed_completion_appends_nothing_and_still_refreshes_visibility
        PASS [   0.053s] (2587/5472) aion-rs lifecycle::completion::tests::a_sequence_conflict_is_reported_not_retried
        PASS [   0.040s] (2588/5472) aion-rs lifecycle::completion::tests::abnormal_exit_records_failed_reconciles_and_notifies
        PASS [   0.076s] (2589/5472) aion-rs lifecycle::completion::tests::a_retrying_completion_returns_with_the_retry_still_armed
        PASS [   0.069s] (2590/5472) aion-rs lifecycle::completion::tests::a_transient_append_failure_still_lands_the_terminal
        PASS [   0.076s] (2591/5472) aion-rs lifecycle::completion::tests::a_transient_history_read_failure_still_lands_the_terminal
        PASS [   0.040s] (2592/5472) aion-rs lifecycle::completion::tests::an_unchanging_fault_is_restated_on_attempt_count_doubling
        PASS [   0.079s] (2593/5472) aion-rs lifecycle::completion::tests::an_attempt_refuses_to_append_once_the_epoch_is_closed
        PASS [   0.065s] (2594/5472) aion-rs lifecycle::completion::tests::normal_exit_records_completed_reconciles_and_notifies
        PASS [   0.054s] (2595/5472) aion-rs lifecycle::completion::tests::one_monitor_lease_can_never_hold_more_than_one_completion_retry
        PASS [   0.312s] (2596/5472) aion-rs lifecycle::cancel_never_alive::tests::cancelling_a_run_the_history_does_not_carry_is_refused
        PASS [   0.044s] (2597/5472) aion-rs lifecycle::completion::tests::process_exit_does_not_retire_a_timed_out_deadline
        PASS [   0.018s] (2598/5472) aion-rs lifecycle::completion::tests::terminal_outcome_is_scoped_to_requested_run_segment
        PASS [   0.063s] (2599/5472) aion-rs lifecycle::completion::tests::process_exit_leaves_a_timed_out_runs_visibility_to_its_teardown
        PASS [   0.052s] (2600/5472) aion-rs lifecycle::completion::tests::process_exit_resumes_interrupted_deadline_cancellation
        PASS [   0.020s] (2601/5472) aion-rs lifecycle::completion::tests::the_fault_key_is_the_typed_variant_not_the_rendered_error
        PASS [   0.063s] (2602/5472) aion-rs lifecycle::completion::tests::the_continuation_start_refuses_once_the_epoch_is_closed
        PASS [   0.067s] (2603/5472) aion-rs lifecycle::completion::tests::the_resume_arm_refuses_to_append_once_the_epoch_is_closed
        PASS [   0.261s] (2604/5472) aion-rs lifecycle::completion::tests::at_the_ceiling_every_failing_attempt_states_itself
        PASS [   3.055s] (2605/5472) aion-rs engine::builder::tests::a_live_wheel_timer_fires_when_the_engine_is_not_released
        PASS [   0.306s] (2606/5472) aion-rs lifecycle::completion::tests::runtime_shutdown_closes_the_completion_retry_epoch
        PASS [   0.316s] (2607/5472) aion-rs lifecycle::continue_as_new::tests::different_replacement_type_rejects_before_terminal_mutation
        PASS [   0.312s] (2608/5472) aion-rs lifecycle::continue_as_new::tests::pending_activity_rejects_without_terminal_event
        PASS [   0.307s] (2609/5472) aion-rs lifecycle::continue_as_new::tests::pending_child_rejects_without_terminal_event
        PASS [   0.325s] (2610/5472) aion-rs lifecycle::continue_as_new::tests::recorded_terminal_rejects_continue_without_second_terminal_event
        PASS [   3.070s] (2611/5472) aion-rs engine::builder::tests::dropping_an_engine_without_shutdown_disarms_the_live_timer_wheel
        PASS [   0.292s] (2612/5472) aion-rs lifecycle::continue_as_new::tests::success_records_notifies_deregisters_and_starts_new_run
        PASS [   0.087s] (2613/5472) aion-rs lifecycle::deadline::tests::deadline_records_timed_out_and_tears_down
        PASS [   0.091s] (2614/5472) aion-rs lifecycle::deadline::tests::re_fire_after_recorded_timeout_resumes_teardown_without_a_second_terminal
        PASS [   0.316s] (2615/5472) aion-rs lifecycle::deadline::tests::cold_start_finalizes_timed_out_teardown_without_a_registered_handle
        PASS [   0.021s] (2616/5472) aion-rs lifecycle::reopen::tests::a_reopened_run_that_reterminated_reopens_from_the_new_failure
        PASS [   0.582s] (2617/5472) aion-rs lifecycle::completion::tests::the_configured_backoff_ladder_governs_when_a_retry_lands
        PASS [   0.011s] (2618/5472) aion-rs lifecycle::reopen::tests::cancelled_run_reopens_with_an_empty_set
        PASS [   0.014s] (2619/5472) aion-rs lifecycle::reopen::tests::completed_run_is_rejected_as_invalid_state
        PASS [   0.032s] (2620/5472) aion-rs lifecycle::reopen::tests::concurrent_fan_out_reopens_every_failed_key
        PASS [   0.027s] (2621/5472) aion-rs lifecycle::reopen::tests::failed_run_computes_the_terminally_failed_step
        PASS [   0.281s] (2622/5472) aion-rs lifecycle::deadline::tests::deadline_for_deregistered_run_is_a_noop
        PASS [   0.017s] (2623/5472) aion-rs lifecycle::reopen::tests::failed_then_succeeded_step_is_not_reopened
        PASS [   0.019s] (2624/5472) aion-rs lifecycle::reopen::tests::fired_timer_is_not_rearmed
        PASS [   0.032s] (2625/5472) aion-rs lifecycle::reopen::tests::in_flight_activity_is_never_reopened
        PASS [   0.016s] (2626/5472) aion-rs lifecycle::reopen::tests::rearm_keeps_the_original_fire_at_and_covers_multiple_timers
        PASS [   0.319s] (2627/5472) aion-rs lifecycle::deadline::tests::deadline_fire_retires_the_deadline_behind_a_competing_terminal
        PASS [   0.015s] (2628/5472) aion-rs lifecycle::reopen::tests::running_run_is_rejected_as_invalid_state
        PASS [   0.034s] (2629/5472) aion-rs lifecycle::reopen::tests::outstanding_timer_rearms_without_touching_history
        PASS [   0.314s] (2630/5472) aion-rs lifecycle::deadline::tests::deadline_loses_to_an_already_recorded_terminal
        PASS [   0.032s] (2631/5472) aion-rs lifecycle::reopen::tests::teardown_cancelled_timer_is_rearmed_with_a_restart_marker
        PASS [   0.021s] (2632/5472) aion-rs lifecycle::reopen::tests::timed_out_run_is_rejected_as_invalid_state
        PASS [   0.035s] (2633/5472) aion-rs lifecycle::reopen::tests::workflow_intent_cancellation_is_never_resurrected
        PASS [   0.320s] (2634/5472) aion-rs lifecycle::deadline::tests::retired_deadline_loses_before_any_workflow_terminal
        PASS [   0.284s] (2635/5472) aion-rs lifecycle::deadline::tests::timeout_teardown_retires_the_runs_active_timers
        PASS [   0.294s] (2636/5472) aion-rs lifecycle::deadline::tests::teardown_failure_preserves_retry_anchors_then_resumes
        PASS [   0.316s] (2637/5472) aion-rs lifecycle::reopen::tests::reopen_after_result_before_terminal_registry_reconcile_succeeds
        PASS [   0.291s] (2638/5472) aion-rs lifecycle::start::contract_tests::in_process_serving_still_refuses_pre_v4_identity
        PASS [   0.307s] (2639/5472) aion-rs lifecycle::start::contract_tests::in_process_serving_admits_unscoped_activities_past_admission
        PASS [   0.332s] (2640/5472) aion-rs lifecycle::reopen::tests::two_reopens_from_one_terminal_head_admit_exactly_one_writer
        PASS [   0.292s] (2641/5472) aion-rs lifecycle::start::contract_tests::pre_v4_exact_version_is_refused_before_history_or_registration
        PASS [   0.319s] (2642/5472) aion-rs lifecycle::start::start_tests::normal_start_monitor_failure_retracts_publication_and_observes_abort
        PASS [   0.323s] (2643/5472) aion-rs lifecycle::start::start_tests::failed_start_with_unabortable_process_retains_ownership_not_orphan
        PASS [   0.328s] (2644/5472) aion-rs lifecycle::start::contract_tests::unscoped_activity_contract_is_refused_before_history_or_registration
        PASS [   0.087s] (2645/5472) aion-rs lifecycle::terminate::tests::cancel_kills_linked_children_records_notifies_and_deregisters
        PASS [   0.308s] (2646/5472) aion-rs lifecycle::start::start_tests::recorder_append_happens_before_spawn_failure
        PASS [   0.317s] (2647/5472) aion-rs lifecycle::start::start_tests::start_with_existing_workflow_id_resumes_history_sequence
        PASS [   0.309s] (2648/5472) aion-rs lifecycle::start::start_tests::successful_start_appends_spawns_places_registers_and_returns_handle
        PASS [   0.351s] (2649/5472) aion-rs lifecycle::start::start_tests::start_with_attributes_records_started_then_attributes_before_spawn
        PASS [   0.027s] (2650/5472) aion-rs lifecycle::transition::tests::reconcile_after_suspend_preserves_suspended_residency
        PASS [   0.352s] (2651/5472) aion-rs lifecycle::start::start_tests::start_with_unregistered_attribute_fails_without_append_or_spawn
        PASS [   0.035s] (2652/5472) aion-rs lifecycle::transition::tests::resume_is_idempotent_for_already_resident_handle
        PASS [   0.040s] (2653/5472) aion-rs lifecycle::transition::tests::resume_missing_handle_returns_workflow_not_found
        PASS [   0.047s] (2654/5472) aion-rs lifecycle::transition::tests::resume_returns_suspended_handle_to_resident_without_status_change
        PASS [   0.052s] (2655/5472) aion-rs lifecycle::transition::tests::suspend_keeps_running_handle_present_with_suspended_residency
        PASS [   0.301s] (2656/5472) aion-rs lifecycle::terminate::tests::cancel_unknown_workflow_returns_not_found
        PASS [   0.046s] (2657/5472) aion-rs lifecycle::transition::tests::suspend_missing_handle_returns_workflow_not_found
        PASS [   0.037s] (2658/5472) aion-rs lifecycle::visibility::tests::search_attributes_collects_from_multiple_updates
        PASS [   0.019s] (2659/5472) aion-rs lifecycle::visibility::tests::started_projection_returns_load_error_for_empty_history
        PASS [   0.048s] (2660/5472) aion-rs lifecycle::visibility::tests::search_attributes_returns_empty_when_no_updates
        PASS [   0.057s] (2661/5472) aion-rs lifecycle::visibility::tests::started_projection_extracts_fields_from_workflow_started
        PASS [   0.049s] (2662/5472) aion-rs lifecycle::visibility::tests::started_projection_returns_load_error_when_no_started_event
        PASS [   0.024s] (2663/5472) aion-rs lifecycle::visibility::tests::terminal_recorded_at_returns_cancelled_timestamp
        PASS [   0.040s] (2664/5472) aion-rs lifecycle::visibility::tests::terminal_recorded_at_finds_terminal_after_interleaved_non_terminal_events
        PASS [   0.384s] (2665/5472) aion-rs lifecycle::start::start_tests::unknown_workflow_type_returns_not_found_and_appends_nothing
        PASS [   0.019s] (2666/5472) aion-rs lifecycle::visibility::tests::terminal_recorded_at_returns_continued_as_new_timestamp
        PASS [   0.033s] (2667/5472) aion-rs lifecycle::visibility::tests::terminal_recorded_at_returns_completed_timestamp
        PASS [   0.026s] (2668/5472) aion-rs lifecycle::visibility::tests::terminal_recorded_at_returns_timed_out_timestamp
        PASS [   0.035s] (2669/5472) aion-rs lifecycle::visibility::tests::terminal_recorded_at_returns_none_for_non_terminal_history
        PASS [   0.038s] (2670/5472) aion-rs lifecycle::visibility::tests::terminal_recorded_at_returns_failed_timestamp
        PASS [   0.337s] (2671/5472) aion-rs lifecycle::terminate::tests::complete_cancels_an_armed_deadline
        PASS [   0.029s] (2672/5472) aion-rs lifecycle::visibility::tests::visibility_record_from_history_projects_running_workflow
        PASS [   0.043s] (2673/5472) aion-rs lifecycle::visibility::tests::visibility_record_from_history_projects_completed_workflow_with_attributes
        PASS [   0.015s] (2674/5472) aion-rs loader::catalog::catalog_tests::content_hash_fixture_changes_when_bytes_change
        PASS [   0.050s] (2675/5472) aion-rs loader::catalog::catalog_tests::collision_from_different_hash_fails_before_registration
        PASS [   0.040s] (2676/5472) aion-rs loader::catalog::catalog_tests::entry_verification_failure_rolls_back_every_registration
        PASS [   0.081s] (2677/5472) aion-rs loader::catalog::catalog_multi_entry_tests::multi_entry_package_registers_and_routes_atomically
        PASS [   0.337s] (2678/5472) aion-rs lifecycle::terminate::tests::complete_records_notifies_and_deregisters
        PASS [   0.100s] (2679/5472) aion-rs loader::catalog::catalog_multi_entry_tests::swapping_out_one_member_removes_and_restores_the_whole_archive_group
        PASS [   0.068s] (2680/5472) aion-rs loader::catalog::catalog_tests::identical_reload_is_idempotent_and_reload_re_routes
        PASS [   0.055s] (2681/5472) aion-rs loader::catalog::catalog_tests::missing_entry_module_returns_load_error
        PASS [   0.140s] (2682/5472) aion-rs loader::catalog::catalog_multi_entry_tests::secondary_entry_verification_failure_commits_no_routes
        PASS [   0.362s] (2683/5472) aion-rs lifecycle::terminate::tests::fail_records_notifies_and_deregisters
        PASS [   0.150s] (2684/5472) aion-rs loader::catalog::catalog_multi_entry_tests::staged_group_is_not_resolvable_until_durable_publication
        PASS [   0.081s] (2685/5472) aion-rs loader::catalog::catalog_tests::records_deployed_entry_function_and_routes_to_it
        PASS [   0.080s] (2686/5472) aion-rs loader::catalog::catalog_tests::start_pins_are_held_until_dropped
        PASS [   0.136s] (2687/5472) aion-rs loader::catalog::catalog_tests::registers_every_package_derived_deployed_module
        PASS [   0.133s] (2688/5472) aion-rs loader::catalog::catalog_tests::runtime_failure_does_not_commit_catalog_state
        PASS [   0.149s] (2689/5472) aion-rs loader::catalog::catalog_tests::route_version_re_points_and_rejects_unknown_hashes
        PASS [   0.153s] (2690/5472) aion-rs loader::catalog::catalog_tests::retains_two_versions_and_routes_to_the_last_loaded
        PASS [   0.126s] (2691/5472) aion-rs loader::catalog::catalog_tests::swap_out_refuses_routed_package_and_restore_round_trips
        PASS [   0.039s] (2692/5472) aion-rs loader::catalog::catalog_timeout_tests::per_entry_identity_authenticates_primary_and_additional
        PASS [   0.035s] (2693/5472) aion-rs loader::declared_queues::declared_queues_tests::a_mixed_catalog_names_the_entry_it_could_not_decode
        PASS [   0.059s] (2694/5472) aion-rs loader::catalog::catalog_timeout_tests::package_builder_authenticates_every_entry_timeout
        PASS [   0.198s] (2695/5472) aion-rs loader::catalog::catalog_tests::same_hash_different_manifest_reload_is_refused_typed
        PASS [   0.102s] (2696/5472) aion-rs loader::catalog::catalog_tests::versions_listing_reports_route_flags_sorted
        PASS [   0.025s] (2697/5472) aion-rs loader::declared_queues::declared_queues_tests::a_read_reports_exactly_what_it_was_given
        PASS [   0.037s] (2698/5472) aion-rs loader::declared_queues::declared_queues_tests::a_read_of_only_v4_entries_covers_every_entry
        PASS [   0.037s] (2699/5472) aion-rs publish::publisher::tests::overflow_yields_lagged_error_then_resumes_with_subsequent_events
        PASS [   0.105s] (2700/5472) aion-rs loader::catalog::catalog_timeout_tests::timeout_only_redeploy_loads_as_a_new_version
        PASS [   0.044s] (2701/5472) aion-rs publish::publisher::tests::subscription_applies_workflow_and_family_filters
        PASS [   0.050s] (2702/5472) aion-rs publish::store::tests::append_publishes_committed_events_in_seq_order
        PASS [   0.021s] (2703/5472) aion-rs publish::store::tests::failed_append_publishes_nothing
        PASS [   0.046s] (2704/5472) aion-rs publish::store::tests::capacity_above_broadcast_maximum_is_rejected
        PASS [   0.078s] (2705/5472) aion-rs loader::declared_queues::declared_queues_tests::an_undecodable_entry_is_named_at_warn
        PASS [   0.034s] (2706/5472) aion-rs publish::store::tests::forwards_outbox_cancel_settle_to_inner
        PASS [   0.041s] (2707/5472) aion-rs publish::store::tests::reads_delegate_to_inner_store
        PASS [   0.041s] (2708/5472) aion-rs query::concrete::tests::suspended_residency_is_not_running_and_never_delivers
        PASS [   0.056s] (2709/5472) aion-rs query::concrete::tests::happy_path_dispatches_run_exact_and_returns_handler_reply
        PASS [   0.039s] (2710/5472) aion-rs query::concrete::tests::unknown_query_propagates_typed_through_engine_error
        PASS [   0.074s] (2711/5472) aion-rs publish::store::tests::forwards_per_shard_failover_seam_to_inner
        PASS [   0.030s] (2712/5472) aion-rs query::service::tests::handler_failure_propagates_as_typed_handler_failed
        PASS [   0.074s] (2713/5472) aion-rs query::concrete::tests::terminal_history_is_not_running_and_never_delivers
        PASS [   0.027s] (2714/5472) aion-rs query::service::tests::query_returns_registered_handler_reply
        PASS [   0.056s] (2715/5472) aion-rs query::service::tests::non_replying_workflow_times_out
        PASS [   0.033s] (2716/5472) aion-rs query::service::tests::terminal_and_non_resident_workflows_are_not_running
        PASS [   0.045s] (2717/5472) aion-rs query::service::tests::query_process_dispatches_to_the_resolved_process_without_resolving
        PASS [   0.054s] (2718/5472) aion-rs query::service::tests::query_does_not_record_events
        PASS [   0.034s] (2719/5472) aion-rs registry::table::tests::a_handle_on_another_run_of_the_workflow_blocks_the_reservation
        PASS [   0.060s] (2720/5472) aion-rs query::service::tests::unknown_query_returns_typed_error_and_workflow_remains_live
        PASS [   0.052s] (2721/5472) aion-rs query::service::tests::unknown_workflow_returns_typed_unknown_error
        PASS [   0.043s] (2722/5472) aion-rs registry::table::tests::a_reservation_is_invisible_to_every_live_process_lookup
        PASS [   0.058s] (2723/5472) aion-rs registry::handle::tests::completion_notifier_stores_outcome_without_active_receiver
        PASS [   0.060s] (2724/5472) aion-rs registry::table::tests::a_reservation_and_a_handle_are_mutually_exclusive
        PASS [   0.038s] (2725/5472) aion-rs registry::table::tests::dropping_a_reservation_releases_the_slot
        PASS [   0.035s] (2726/5472) aion-rs registry::table::tests::insert_if_absent_wins_the_slot_and_rejects_the_racer
        PASS [   0.046s] (2727/5472) aion-rs registry::table::tests::a_second_reservation_for_the_same_workflow_is_refused
        PASS [   0.041s] (2728/5472) aion-rs registry::table::tests::list_returns_snapshot_handles
        PASS [   0.051s] (2729/5472) aion-rs registry::table::tests::live_pid_tracks_newest_run_across_continue_as_new
        PASS [   0.049s] (2730/5472) aion-rs registry::table::tests::reconcile_refuses_a_run_absent_from_the_history
        PASS [   0.052s] (2731/5472) aion-rs registry::table::tests::reconcile_missing_handle_is_noop
        PASS [   0.559s] (2732/5472) aion-rs loader::catalog::catalog_tests::package_loaded_under_content_hash_namespace_spawns_entrypoint
        PASS [   0.053s] (2733/5472) aion-rs registry::table::tests::reconcile_projection_wins_over_disagreeing_cache
        PASS [   0.074s] (2734/5472) aion-rs registry::table::tests::poisoned_lock_returns_typed_registry_error
        PASS [   0.037s] (2735/5472) aion-rs registry::table::tests::reconcile_still_sees_the_successor_as_running
        PASS [   0.062s] (2736/5472) aion-rs registry::table::tests::reconcile_scopes_the_projection_to_the_named_run
        PASS [   0.028s] (2737/5472) aion-rs registry::table::tests::reconcile_updates_completed_projection
        PASS [   0.034s] (2738/5472) aion-rs registry::table::tests::reconcile_updates_cancelled_projection
        PASS [   0.024s] (2739/5472) aion-rs registry::table::tests::remove_if_pid_reports_absent_for_an_empty_slot
        PASS [   0.450s] (2740/5472) aion-rs loader::catalog::catalog_tests::unexported_entry_function_fails_the_runtime_load
        PASS [   0.047s] (2741/5472) aion-rs registry::table::tests::remove_deletes_only_the_requested_run
        PASS [   0.059s] (2742/5472) aion-rs registry::table::tests::remove_if_pid_removes_matching_handle_and_clears_index
        PASS [   0.043s] (2743/5472) aion-rs registry::table::tests::remove_if_pid_returns_replacement_without_disturbing_it_or_the_index
        PASS [   0.071s] (2744/5472) aion-rs registry::table::tests::registry_is_send_sync
        PASS [   0.045s] (2745/5472) aion-rs registry::unrecoverable::tests::a_recorded_run_can_be_asked_about_by_id
        PASS [   0.039s] (2746/5472) aion-rs registry::unrecoverable::tests::list_reports_every_degraded_run_and_only_those
        PASS [   0.058s] (2747/5472) aion-rs registry::table::tests::stores_two_runs_for_the_same_workflow_without_shadowing
        PASS [   0.047s] (2748/5472) aion-rs registry::unrecoverable::tests::observing_the_run_resident_clears_the_degraded_fact
        PASS [   0.064s] (2749/5472) aion-rs registry::unrecoverable::tests::an_unrecorded_run_is_not_degraded
        PASS [   0.045s] (2750/5472) aion-rs registry::unrecoverable::tests::the_newest_boots_verdict_replaces_the_previous_one
        PASS [   0.041s] (2751/5472) aion-rs runtime::config::completion_retry_ladder_tests::a_ceiling_below_the_floor_cannot_be_named
        PASS [   0.055s] (2752/5472) aion-rs runtime::cleanup_executor::tests::shutdown_is_bounded_and_can_be_retried_after_worker_release
        PASS [   0.036s] (2753/5472) aion-rs runtime::engine_nifs::tests::registers_all_engine_nifs_as_unique_entries_with_correct_scheduling
        PASS [   0.061s] (2754/5472) aion-rs runtime::config::completion_retry_ladder_tests::a_zero_initial_backoff_cannot_be_named
        PASS [   0.059s] (2755/5472) aion-rs runtime::engine_nifs::tests::engine_and_test_double_export_identical_mfa_sets
        PASS [   0.034s] (2756/5472) aion-rs runtime::engine_tasks::tests::a_release_from_a_foreign_task_cannot_evict_a_live_completion_retry
        PASS [   0.060s] (2757/5472) aion-rs runtime::config::completion_retry_ladder_tests::the_default_ladder_is_one_the_constructor_would_accept
        PASS [   0.062s] (2758/5472) aion-rs runtime::engine_nifs::tests::returns_error_on_wrong_arity
        PASS [   0.037s] (2759/5472) aion-rs runtime::engine_tasks::tests::abort_for_parent_leaves_other_parents_armed
        PASS [   0.058s] (2760/5472) aion-rs runtime::engine_nifs::tests::unimplemented_stub_returns_standard_error_tuple
        PASS [   0.048s] (2761/5472) aion-rs runtime::engine_tasks::tests::a_reopened_runs_successor_lease_can_arm_its_own_completion_retry
        PASS [   0.041s] (2762/5472) aion-rs runtime::engine_tasks::tests::abort_watch_disarms_a_single_key
        PASS [   0.057s] (2763/5472) aion-rs runtime::engine_tasks::tests::arming_is_idempotent_per_key
        PASS [   0.047s] (2764/5472) aion-rs runtime::engine_tasks::tests::shutdown_gates_new_arms_and_awaits_aborted_tasks
        PASS [   0.067s] (2765/5472) aion-rs runtime::engine_tasks::tests::drop_backstop_aborts_without_blocking
        PASS [   0.055s] (2766/5472) aion-rs runtime::engine_tasks::tests::shutdown_is_safe_from_inside_a_host_async_context
        PASS [   0.082s] (2767/5472) aion-rs runtime::engine_tasks::tests::dropping_the_registry_aborts_completion_retries_too
        PASS [   0.157s] (2768/5472) aion-rs runtime::handle::delivery::tests::interleavings::withheld_removal_does_not_block_reregistered_workflow_delivery
        PASS [   0.368s] (2769/5472) aion-rs runtime::handle::delivery::tests::interleavings::late_delivery_after_monitor_cleanup_does_not_recreate_a_gate
        PASS [   0.343s] (2770/5472) aion-rs runtime::handle::delivery::tests::outbox_completion_lands_where_take_reads_it
        PASS [   0.357s] (2771/5472) aion-rs runtime::handle::delivery::tests::outbox_completion_is_run_scoped_across_continue_as_new
        PASS [   0.385s] (2772/5472) aion-rs runtime::handle::delivery::tests::interleavings::monitor_drains_outcome_when_death_follows_successful_enqueue
        PASS [   0.384s] (2773/5472) aion-rs runtime::handle::delivery::tests::interleavings::poisoned_monitor_drain_removes_state_and_returns_typed_error
        PASS [   0.448s] (2774/5472) aion-rs runtime::handle::delivery::tests::interleavings::monitor_drain_overlaps_failure_before_enqueue_and_rolls_it_back
        PASS [   0.459s] (2775/5472) aion-rs runtime::handle::delivery::tests::interleavings::monitor_drain_overlaps_completion_before_enqueue_and_rolls_it_back
        PASS [   0.040s] (2776/5472) aion-rs runtime::handle::spawn_bifs::tests::missing_fun_spawn_mfas_are_typed_and_leave_vacant_keys_unchanged
        PASS [   0.096s] (2777/5472) aion-rs runtime::handle::delivery::tests::rollback::suspended_workflow_marker_retries_after_live_refusal
        PASS [   0.117s] (2778/5472) aion-rs runtime::handle::tests::a_second_failing_shutdown_step_is_reported_even_though_only_one_is_returned
        PASS [   0.439s] (2779/5472) aion-rs runtime::handle::delivery::tests::outbox_failure_is_run_scoped_across_continue_as_new
        PASS [   0.359s] (2780/5472) aion-rs runtime::handle::delivery::tests::rollback::failed_legacy_result_redelivery_restores_published_completion
        PASS [   0.442s] (2781/5472) aion-rs runtime::handle::delivery::tests::rollback::failed_completion_wrapper_redelivery_restores_published_completion
        PASS [   0.437s] (2782/5472) aion-rs runtime::handle::delivery::tests::rollback::failed_failure_wrapper_redelivery_restores_published_failure
        PASS [   0.441s] (2783/5472) aion-rs runtime::handle::delivery::tests::rollback::first_live_marker_refusal_preserves_attempted_completion
        PASS [   0.044s] (2784/5472) aion-rs runtime::handle::tests::runtime_handle_is_send_sync
        PASS [   0.110s] (2785/5472) aion-rs runtime::handle::tests::shutdown_closes_the_epoch_even_when_a_drain_fails
        PASS [   0.474s] (2786/5472) aion-rs runtime::handle::tests::distinct_nifs_are_registered_and_callable
        PASS [   0.401s] (2787/5472) aion-rs runtime::handle::tests::duplicate_nif_mfa_returns_typed_error
        PASS [   0.420s] (2788/5472) aion-rs runtime::handle::tests::payload_binary_remains_valid_through_spawn_and_is_released
        PASS [   0.211s] (2789/5472) aion-rs runtime::handle::tests::signal_delivery_to_dead_process_returns_typed_error
        PASS [   0.389s] (2790/5472) aion-rs runtime::handle::tests::registers_spawns_and_shuts_down
        PASS [   0.154s] (2791/5472) aion-rs runtime::monitor::tests::attach_abort_finalizer_reports_no_job_so_the_caller_retracts_now
        PASS [   0.820s] (2792/5472) aion-rs runtime::handle::readiness::tests::a_live_recovered_process_may_reach_its_await_after_the_old_deadline
        PASS [   0.341s] (2793/5472) aion-rs runtime::monitor::tests::abort_job_finalizer_runs_on_completion_and_when_already_complete
        PASS [   0.401s] (2794/5472) aion-rs runtime::handle::tests::workflow_outcome_releases_payload_heaps
        PASS [   0.371s] (2795/5472) aion-rs runtime::monitor::tests::abort_refuses_a_process_owned_by_a_committed_monitor
        PASS [   0.276s] (2796/5472) aion-rs runtime::monitor::tests::completed_process_lifecycle_state_returns_to_baseline_under_churn
        PASS [   0.492s] (2797/5472) aion-rs runtime::monitor::tests::bounded_cleanup_queue_exhaustion_is_typed
        PASS [   0.339s] (2798/5472) aion-rs runtime::monitor::tests::duplicate_installation_after_retirement_is_typed_terminal
        PASS [   0.417s] (2799/5472) aion-rs runtime::monitor::tests::delayed_exit_observation_survives_heavy_churn_and_runs_full_cleanup
        PASS [   0.381s] (2800/5472) aion-rs runtime::monitor::tests::duplicate_installation_cannot_consume_outcome_or_abort_owned_process
        PASS [   0.358s] (2801/5472) aion-rs runtime::monitor::tests::event_driven_exit_runs_delivery_cleanup_to_true_terminal_state
        PASS [   0.111s] (2802/5472) aion-rs runtime::monitor::tests::unavailable_cleanup_executor_is_typed_without_sticky_abort_identity
        PASS [   0.371s] (2803/5472) aion-rs runtime::monitor::tests::exact_cleanup_predicate_rejects_a_live_native_entry
        PASS [   0.038s] (2804/5472) aion-rs runtime::nif::tests::collects_host_and_engine_nifs
        PASS [   0.028s] (2805/5472) aion-rs runtime::nif_activity::tests::absent_fields_resolve_to_the_named_default
        PASS [   0.044s] (2806/5472) aion-rs runtime::nif_activity::tests::absent_node_resolves_to_no_affinity
        PASS [   0.397s] (2807/5472) aion-rs runtime::monitor::tests::monitor_installation_failure_drains_retained_completion_transaction
        PASS [   0.046s] (2808/5472) aion-rs runtime::nif_activity::tests::absent_null_or_malformed_tier_resolves_to_no_selection
        PASS [   0.027s] (2809/5472) aion-rs runtime::nif_activity::tests::malformed_config_falls_through_to_start_time_queue
        PASS [   0.054s] (2810/5472) aion-rs runtime::nif_activity::tests::activity_override_wins_over_start_time_queue
        PASS [   0.060s] (2811/5472) aion-rs runtime::nif_activity::tests::activity_override_wins_over_workflow_default
        PASS [   0.036s] (2812/5472) aion-rs runtime::nif_activity::tests::malformed_config_resolves_to_no_node_affinity
        PASS [   0.048s] (2813/5472) aion-rs runtime::nif_activity::tests::malformed_config_resolves_to_the_named_default
        PASS [   0.344s] (2814/5472) aion-rs runtime::monitor::tests::monitor_rejects_pid_never_spawned_by_this_runtime
        PASS [   0.038s] (2815/5472) aion-rs runtime::nif_activity::tests::no_selection_resolves_to_the_named_default
        PASS [   0.050s] (2816/5472) aion-rs runtime::nif_activity::tests::named_default_is_the_last_resort_below_start_time_queue
        PASS [   0.030s] (2817/5472) aion-rs runtime::nif_activity::tests::node_pin_resolves_to_the_selected_node
        PASS [   0.246s] (2818/5472) aion-rs runtime::monitor::tests::unmonitored_abort_bound_exhaustion_is_typed
        PASS [   0.057s] (2819/5472) aion-rs runtime::nif_activity::tests::null_node_resolves_to_no_affinity
        PASS [   0.058s] (2820/5472) aion-rs runtime::nif_activity::tests::start_time_queue_applies_when_no_config_selection
        PASS [   0.378s] (2821/5472) aion-rs runtime::monitor::tests::shutdown_drains_in_flight_exits_before_bounded_drainer_join
        PASS [   0.057s] (2822/5472) aion-rs runtime::nif_activity::tests::tier_selection_is_read_from_config
        PASS [   0.038s] (2823/5472) aion-rs runtime::nif_activity::tests::workflow_default_wins_over_start_time_queue
        PASS [   0.047s] (2824/5472) aion-rs runtime::nif_activity::tests::workflow_default_applies_when_activity_selects_none
        PASS [   0.369s] (2825/5472) aion-rs runtime::monitor::tests::unregistered_absent_process_aborts_without_waiting_for_outcome
        PASS [   0.163s] (2826/5472) aion-rs runtime::nif_activity_delivery_tests::in_vm_spawn_failure_fallback_resolves_failed_never_suspend
        PASS [   0.118s] (2827/5472) aion-rs runtime::nif_activity_dispatch::tests::blocking_support::blocking_dispatcher_completes_on_single_threaded_runtime
        PASS [   0.047s] (2828/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_fallback_recovery::replay_after_last_refusal_finishes_without_inventing_an_execution
        PASS [   0.358s] (2829/5472) aion-rs runtime::nif_activity_dispatch::tests::await_paths::poisoned_take_fails_typed_and_monitor_drains_retained_state
        PASS [   0.047s] (2830/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_fallback_recovery::replay_before_refusal_record_chooses_the_same_authored_hop
        PASS [   0.386s] (2831/5472) aion-rs runtime::nif_activity_delivery_tests::real_await_nif_poison_fails_process_and_monitor_drains_state
        PASS [   0.392s] (2832/5472) aion-rs runtime::nif_activity_dispatch::tests::await_paths::awaited_terminal_records_the_noted_final_attempt
        PASS [   0.040s] (2833/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_fallback_recovery::replay_between_refusal_and_hop_records_the_same_hop_before_dispatch
        PASS [   0.055s] (2834/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_fallback_recovery::replay_between_hop_and_start_reuses_the_event_queue_not_fresh_derivation
        PASS [   0.043s] (2835/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_outcomes::absent_policy_keeps_run_exactly_once_for_retryable_failures
        PASS [   0.063s] (2836/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_outcomes::a_closed_engine_task_epoch_stops_the_retry_loop_writing
        PASS [   0.079s] (2837/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_fallback_recovery::replay_between_start_and_delivery_uses_the_recorded_hop_queue
        PASS [   0.069s] (2838/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_outcomes::exhausted_retry_budget_names_exhaustion_and_last_refusal
        PASS [   0.516s] (2839/5472) aion-rs runtime::nif_activity_delivery_tests::fast_completion_during_dispatch_nif_resolves_on_next_real_await
        PASS [   0.054s] (2840/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_outcomes::non_retryable_failure_fails_immediately_without_a_retry_trail
        PASS [   0.075s] (2841/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_outcomes::policy_refusal_hops_before_retry_and_records_the_durable_route
        PASS [   0.073s] (2842/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_outcomes::policy_refusal_without_chain_uses_configured_retry_and_keeps_its_prefix
        PASS [   0.059s] (2843/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_outcomes::retryable_failure_redispatches_with_incremented_recorded_attempt
        PASS [   0.043s] (2844/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_parking::retry_loop_aborts_without_recording_once_the_activity_settled
        PASS [   0.082s] (2845/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_parking::parked_dispatch_stands_down_without_recording_even_with_retry_budget
        PASS [   0.062s] (2846/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_timeout::an_expired_attempt_is_retried_while_budget_remains
        PASS [   0.501s] (2847/5472) aion-rs runtime::nif_activity_dispatch::tests::await_paths::stale_snapshot_expiry_suspends_then_converges_with_replay
        PASS [   0.037s] (2848/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_worker_loss::a_settled_ordinal_stops_the_worker_loss_redispatch
        PASS [   0.089s] (2849/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_timeout::an_expired_attempt_without_a_policy_fails_naming_the_bound
        PASS [   0.064s] (2850/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_worker_loss::a_flapping_link_terminates_naming_the_transport_not_the_action
        PASS [   0.071s] (2851/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_worker_loss::a_worker_loss_consumes_no_authored_budget_and_records_nothing
        PASS [   0.670s] (2852/5472) aion-rs runtime::nif_activity_dispatch::tests::await_paths::delivered_completion_settles_durably_ahead_of_snapshot_expiry
        PASS [   0.025s] (2853/5472) aion-rs runtime::nif_activity_fallback::tests::fallback_config_accepts_only_string_arrays
        PASS [   0.047s] (2854/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_worker_loss::an_action_failure_still_consumes_its_authored_budget
        PASS [   0.060s] (2855/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_worker_loss::a_worker_loss_redispatches_even_with_no_authored_retry_policy
        PASS [   0.030s] (2856/5472) aion-rs runtime::nif_activity_retry::tests::absent_null_and_malformed_policies_decode_to_no_retries
        PASS [   0.043s] (2857/5472) aion-rs runtime::nif_activity_fallback::tests::next_hop_skips_current_queue_but_keeps_authored_index
        PASS [   0.042s] (2858/5472) aion-rs runtime::nif_activity_retry::tests::a_positive_millisecond_value_is_the_authored_bound
        PASS [   0.030s] (2859/5472) aion-rs runtime::nif_activity_retry::tests::absent_null_and_unparseable_configs_decode_to_no_bound
        PASS [   0.033s] (2860/5472) aion-rs runtime::nif_activity_retry::tests::backoff_delays_grow_and_cap
        PASS [   0.030s] (2861/5472) aion-rs runtime::nif_activity_retry::tests::parked_classification_is_prefix_scoped_and_disjoint_from_retryable
        PASS [   0.034s] (2862/5472) aion-rs runtime::nif_activity_retry::tests::reason_classification_follows_the_wire_prefix_only
        PASS [   0.040s] (2863/5472) aion-rs runtime::nif_activity_retry::tests::next_attempt_continues_the_recorded_trail
        PASS [   0.050s] (2864/5472) aion-rs runtime::nif_activity_retry::tests::degenerate_and_malformed_bounds_decode_to_no_bound
        PASS [   0.030s] (2865/5472) aion-rs runtime::nif_activity_retry::tests::sdk_shaped_policies_decode_exactly
        PASS [   0.033s] (2866/5472) aion-rs runtime::nif_activity_retry::tests::supersession_names_only_a_started_and_unsettled_attempt
        PASS [   0.041s] (2867/5472) aion-rs runtime::nif_activity_retry::tests::settlement_tracks_terminal_outcomes_not_retryable_attempts
        PASS [   0.035s] (2868/5472) aion-rs runtime::nif_activity_retry::tests::the_expiry_reason_types_a_timeout_and_is_not_retryable_class
        PASS [   0.244s] (2869/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_timeout::an_unbounded_dispatch_is_never_ended_by_the_engine
        PASS [   0.065s] (2870/5472) aion-rs runtime::nif_child::tests::child_keys_are_run_scoped_ordinals_independent_of_recorder_head
        PASS [   0.457s] (2871/5472) aion-rs runtime::nif_activity_dispatch::tests::retry_parking::parked_dispatch_delivers_nothing_to_the_workflow_process
        PASS [   0.338s] (2872/5472) aion-rs runtime::nif_child::tests::deadline_recorded_before_child_terminal_takes_the_timeout_branch
        PASS [   0.369s] (2873/5472) aion-rs runtime::nif_child::tests::awaiting_a_child_the_engine_has_no_trace_of_is_a_typed_error
        PASS [   0.357s] (2874/5472) aion-rs runtime::nif_child::tests::child_terminal_recorded_before_deadline_resolves_the_child
        PASS [   0.348s] (2875/5472) aion-rs runtime::nif_child::tests::live_await_pins_arms_watcher_and_suspends_idempotently
        PASS [   0.354s] (2876/5472) aion-rs runtime::nif_child::tests::expired_scope_releases_pin_without_touching_the_child
        PASS [   0.399s] (2877/5472) aion-rs runtime::nif_child::tests::expired_scope_disarms_the_armed_watcher
        PASS [   9.843s] (2878/5472) aion-package structure::tests::append_run_regenerates_type_checking_gleam
        PASS [   0.366s] (2879/5472) aion-rs runtime::nif_child::tests::mismatched_pin_is_a_typed_error
        PASS [   0.040s] (2880/5472) aion-rs runtime::nif_child_watch::tests::cancelled_and_timed_out_map_to_failed_with_prefixes
        PASS [   0.362s] (2881/5472) aion-rs runtime::nif_child::tests::recorded_failure_resolves_as_error_prefixed_data_and_clears_pin
        PASS [   0.330s] (2882/5472) aion-rs runtime::nif_child_spawn_retry::tests::epoch_close_drops_the_in_flight_start_attempt
        PASS [   0.382s] (2883/5472) aion-rs runtime::nif_child::tests::query_sentinel_precedes_recorded_resolution_and_preserves_pin
        PASS [   0.336s] (2884/5472) aion-rs runtime::nif_child_spawn_retry::tests::permanent_start_failure_stays_internal_and_armed
        PASS [   0.361s] (2885/5472) aion-rs runtime::nif_child_spawn_retry::tests::existing_child_history_ends_the_retry_without_a_second_start
        PASS [   0.057s] (2886/5472) aion-rs runtime::nif_child_watch::tests::continue_as_new_outcome_is_refused_as_a_recordable_terminal
        PASS [   0.047s] (2887/5472) aion-rs runtime::nif_child_watch::tests::current_run_handle_selects_the_latest_run_not_an_arbitrary_one
        PASS [   0.408s] (2888/5472) aion-rs runtime::nif_child::tests::stale_snapshot_live_timeout_converges_with_replay_child_branch
        PASS [   0.046s] (2889/5472) aion-rs runtime::nif_child_watch::tests::latest_run_id_reads_the_newest_started_run
        PASS [   0.051s] (2890/5472) aion-rs runtime::nif_child_watch::tests::record_is_terminal_guarded_and_duplicate_suppressed
        PASS [   0.347s] (2891/5472) aion-rs runtime::nif_child_watch::tests::arming_is_idempotent_per_parent_and_child
        PASS [   0.415s] (2892/5472) aion-rs runtime::nif_child_watch::tests::cleanup_aborts_only_the_exited_parents_watchers
        PASS [   0.031s] (2893/5472) aion-rs runtime::nif_collect::tests::harness::only_an_explicit_in_vm_selection_is_refused_from_fan_out
        PASS [   0.343s] (2894/5472) aion-rs runtime::nif_child_watch::tests::marker_failure_after_durable_record_is_non_fatal
        PASS [   0.079s] (2895/5472) aion-rs runtime::nif_collect::tests::retryable_settlement::a_cancellation_over_an_open_retry_trail_resolves_the_member_as_cancelled
        PASS [   0.369s] (2896/5472) aion-rs runtime::nif_child_watch::tests::transient_record_failure_is_retried_until_the_terminal_lands
        PASS [   0.369s] (2897/5472) aion-rs runtime::nif_child_watch::tests::registry_miss_backs_off_until_the_child_appears
        PASS [   0.372s] (2898/5472) aion-rs runtime::nif_child_watch::tests::watcher_follows_continue_as_new_chain_to_final_terminal
        PASS [   0.404s] (2899/5472) aion-rs runtime::nif_child_watch::tests::missed_doorbell_degrades_to_store_polling_not_a_permanent_park
        PASS [   0.036s] (2900/5472) aion-rs runtime::nif_collect::tests::retryable_settlement::a_retryable_attempt_record_leaves_the_member_unresolved
        PASS [   0.072s] (2901/5472) aion-rs runtime::nif_collect::tests::retryable_settlement::a_terminal_ending_a_retry_trail_resolves_the_member_over_the_retry_record
        PASS [   0.035s] (2902/5472) aion-rs runtime::nif_collect::tests::routing_fresh::fresh_fan_out_item_carries_the_selected_node
        PASS [   0.029s] (2903/5472) aion-rs runtime::nif_collect::tests::routing_fresh::fresh_fan_out_item_carries_the_selected_task_queue
        PASS [   0.023s] (2904/5472) aion-rs runtime::nif_collect::tests::routing_fresh::fresh_fan_out_items_fall_back_to_the_start_time_queue
        PASS [   0.150s] (2905/5472) aion-rs runtime::nif_collect::tests::retryable_settlement::a_terminal_ending_a_retry_trail_wins_a_race_with_the_terminals_message
        PASS [   0.032s] (2906/5472) aion-rs runtime::nif_collect::tests::routing_fresh::fresh_fan_out_items_resolve_precedence_per_member
        PASS [   0.628s] (2907/5472) aion-rs runtime::nif_collect::tests::expiry::all_stale_snapshot_expiry_suspends_then_converges_with_replay
        PASS [   0.171s] (2908/5472) aion-rs runtime::nif_collect::tests::routing_fresh::no_selection_records_on_the_workflow_start_time_queue
        PASS [   0.385s] (2909/5472) aion-rs runtime::nif_collect::tests::retryable_settlement::a_retryable_attempt_record_does_not_fail_fast_a_collect_all
        PASS [   0.364s] (2910/5472) aion-rs runtime::nif_collect::tests::retryable_settlement::a_terminal_ending_a_retry_trail_fails_fast_with_the_terminals_message
        PASS [   0.672s] (2911/5472) aion-rs runtime::nif_collect::tests::expiry::race_stale_snapshot_expiry_suspends_then_settles_the_recorded_winner
        PASS [   0.396s] (2912/5472) aion-rs runtime::nif_collect::tests::retryable_settlement::a_retryable_attempt_record_does_not_win_a_race
        PASS [   0.454s] (2913/5472) aion-rs runtime::nif_collect::tests::retryable_settlement::a_retry_record_landing_mid_sweep_leaves_the_replayed_settlement_unchanged
        PASS [   0.041s] (2914/5472) aion-rs runtime::nif_collect::tests::routing_recovery::recovery_from_pre_field_history_defaults_task_queue
        PASS [   0.046s] (2915/5472) aion-rs runtime::nif_collect::tests::routing_recovery::recovery_from_pre_field_history_has_no_node
        PASS [   0.047s] (2916/5472) aion-rs runtime::nif_collect::tests::routing_recovery::recovery_re_targets_the_recorded_node_not_none
        PASS [   0.036s] (2917/5472) aion-rs runtime::nif_collect::tests::routing_recovery::recovery_re_targets_the_recorded_task_queue_not_default
        PASS [   0.479s] (2918/5472) aion-rs runtime::nif_collect::tests::routing_fresh::recovery_re_resolves_the_start_time_queue_not_default
        PASS [   0.399s] (2919/5472) aion-rs runtime::nif_collect::tests::routing_fresh::scheduled_events_record_each_members_resolved_node
        PASS [   0.362s] (2920/5472) aion-rs runtime::nif_collect::tests::settlement::empty_list_resolves_immediately_without_pinning
        PASS [   0.417s] (2921/5472) aion-rs runtime::nif_collect::tests::routing_fresh::scheduled_events_record_each_members_resolved_task_queue
        PASS [   0.032s] (2922/5472) aion-rs runtime::nif_concurrency::allocation_failure_control::allocation_failure_refuses_with_beamrs_exact_reason
        PASS [   0.377s] (2923/5472) aion-rs runtime::nif_collect::tests::settlement::pinned_base_is_reused_across_reentries_and_counter_advances_once
        PASS [   0.409s] (2924/5472) aion-rs runtime::nif_collect::tests::settlement::cancellation_set_covers_exactly_the_unresolved_ordinals
        PASS [   0.035s] (2925/5472) aion-rs runtime::nif_concurrency::allocation_failure_control::control_empty_results_encode_successfully
        PASS [   0.419s] (2926/5472) aion-rs runtime::nif_collect::tests::settlement::fail_fast_returns_lowest_ordinal_failure_and_cancels_unresolved
        PASS [   0.048s] (2927/5472) aion-rs runtime::nif_context::tests::await_child_skips_consumed_commands_to_recorded_terminal
        PASS [   0.047s] (2928/5472) aion-rs runtime::nif_context::tests::await_child_without_recorded_terminal_resumes_live
        PASS [   0.055s] (2929/5472) aion-rs runtime::nif_context::tests::block_on_recorder_reads_current_head_without_deadlock
        PASS [   0.057s] (2930/5472) aion-rs runtime::nif_context::tests::context_reads_the_recorded_start_time_task_queue
        PASS [   0.089s] (2931/5472) aion-rs runtime::nif_context::tests::birth_window_registration_resolves_instead_of_failing
        PASS [   0.089s] (2932/5472) aion-rs runtime::nif_context::tests::context_without_start_time_attribute_projects_none
        PASS [   0.041s] (2933/5472) aion-rs runtime::nif_context::tests::resolves_registered_pid_to_context
        PASS [   0.059s] (2934/5472) aion-rs runtime::nif_context::tests::resolve_command_returns_recorded_activity_resolution
        PASS [   0.050s] (2935/5472) aion-rs runtime::nif_continue_as_new::tests::a_closed_epoch_refuses_the_continue_as_new_terminal
        PASS [   0.057s] (2936/5472) aion-rs runtime::nif_continue_as_new::tests::an_already_terminal_run_reports_its_own_cause_not_the_epoch
        PASS [   0.067s] (2937/5472) aion-rs runtime::nif_continue_as_new::tests::a_deadline_retirement_that_fails_after_the_terminal_reports_terminal_only
        PASS [   0.349s] (2938/5472) aion-rs runtime::nif_collect::tests::settlement::race_batch_tie_breaks_to_lowest_ordinal_and_drains_loser_entries
        PASS [   0.348s] (2939/5472) aion-rs runtime::nif_collect::tests::settlement::race_settlement_advances_now_over_the_recorded_winner
        PASS [   0.646s] (2940/5472) aion-rs runtime::nif_collect::tests::settlement::expired_scope_cancels_unresolved_and_replay_derives_the_same_abort
        PASS [   0.042s] (2941/5472) aion-rs runtime::nif_determinism::tests::an_unconsumed_async_arrival_never_moves_now
        PASS [   0.060s] (2942/5472) aion-rs runtime::nif_continue_as_new::tests::the_process_ends_whenever_the_terminal_landed_and_for_one_refusal_besides
        PASS [   0.065s] (2943/5472) aion-rs runtime::nif_determinism::tests::each_engine_state_resolves_its_own_context_source
        PASS [   0.054s] (2944/5472) aion-rs runtime::nif_determinism::tests::live_and_replayed_activity_serve_identical_now_sequences
        PASS [   0.087s] (2945/5472) aion-rs runtime::nif_determinism::tests::consuming_a_recorded_command_advances_now_to_that_event_only
        PASS [   0.061s] (2946/5472) aion-rs runtime::nif_determinism::tests::now_before_any_consumption_is_the_run_start_not_the_history_tail
        PASS [   0.028s] (2947/5472) aion-rs runtime::nif_determinism::tests::random_int_is_stable_uniform_and_validates_ranges
        PASS [   0.046s] (2948/5472) aion-rs runtime::nif_determinism::tests::random_int_context_returns_error_for_invalid_range
        PASS [   0.049s] (2949/5472) aion-rs runtime::nif_determinism::tests::random_is_stable_for_same_ids_and_sequence
        PASS [   0.250s] (2950/5472) aion-rs runtime::nif_context::tests::unknown_pid_returns_unknown_process
        PASS [   0.126s] (2951/5472) aion-rs runtime::nif_determinism::tests::elapsed_across_a_replayed_sleep_is_the_recorded_span_not_zero
        PASS [   0.054s] (2952/5472) aion-rs runtime::nif_determinism::tests::replay_sequence_matches_recorded_history_and_seeded_positions
        PASS [   0.048s] (2953/5472) aion-rs runtime::nif_determinism::tests::resumed_run_answers_the_run_start_before_the_sleep_is_consumed
        PASS [   0.452s] (2954/5472) aion-rs runtime::nif_collect::tests::settlement::race_winner_is_first_recorded_terminal_not_lowest_ordinal
        PASS [   0.038s] (2955/5472) aion-rs runtime::nif_query::nif_query_tests::dispatch_query_round_trips_through_query_service
        PASS [   0.074s] (2956/5472) aion-rs runtime::nif_query::nif_query_tests::cleanup_process_drains_queries_and_drops_reply_senders
        PASS [   0.060s] (2957/5472) aion-rs runtime::nif_query::nif_query_tests::dispatch_query_is_refused_during_replay
        PASS [   0.061s] (2958/5472) aion-rs runtime::nif_query::nif_query_tests::dispatch_query_is_refused_while_servicing_a_query
        PASS [   0.064s] (2959/5472) aion-rs runtime::nif_query::nif_query_tests::dispatch_query_without_configured_timeout_fails_typed
        PASS [   0.054s] (2960/5472) aion-rs runtime::nif_query::nif_query_tests::query_nifs_do_not_change_event_history
        PASS [   0.074s] (2961/5472) aion-rs runtime::nif_query::nif_query_tests::query_nifs_accept_a_pid_the_registry_has_not_inserted_yet
        PASS [   0.058s] (2962/5472) aion-rs runtime::nif_query::nif_query_tests::register_query_stores_names_only_and_is_idempotent
        PASS [   0.061s] (2963/5472) aion-rs runtime::nif_query::nif_query_tests::replies_clear_the_servicing_guard_even_for_unknown_ids
        PASS [   0.073s] (2964/5472) aion-rs runtime::nif_query::nif_query_tests::reply_query_delivers_pending_response_and_errors_for_missing_id
        PASS [   0.063s] (2965/5472) aion-rs runtime::nif_query::nif_query_tests::reply_query_error_delivers_typed_handler_failed
        PASS [   0.040s] (2966/5472) aion-rs runtime::nif_query_diagnostic::tests::diagnostic_bound_preserves_utf8_boundaries
        PASS [   0.036s] (2967/5472) aion-rs runtime::nif_query_mailbox::tests::a_continued_workflow_resolves_to_its_current_run
        PASS [   0.159s] (2968/5472) aion-rs runtime::nif_query::nif_query_tests::delivery_queues_pending_query_and_wakes_the_workflow_process
        PASS [   0.043s] (2969/5472) aion-rs runtime::nif_query_mailbox::tests::a_stale_predecessor_handle_without_an_index_entry_is_unknown
        PASS [   0.045s] (2970/5472) aion-rs runtime::nif_query_mailbox::tests::the_live_run_index_follows_registration_order
        PASS [   0.039s] (2971/5472) aion-rs runtime::nif_query_pump::tests::empty_queue_returns_none
        PASS [   0.046s] (2972/5472) aion-rs runtime::nif_query_pump::tests::sentinel_carries_arguments_as_an_escaped_json_string
        PASS [   0.044s] (2973/5472) aion-rs runtime::nif_query_pump::tests::sentinel_pops_one_query_sets_servicing_and_preserves_await_pin
        PASS [   0.039s] (2974/5472) aion-rs runtime::nif_signal::tests::derives_signal_occurrences_from_matching_signal_names
        PASS [   0.052s] (2975/5472) aion-rs runtime::nif_query_pump::tests::servicing_guard_refuses_then_clears
        PASS [   0.061s] (2976/5472) aion-rs runtime::nif_query_pump::tests::sentinel_skips_queries_whose_caller_stopped_waiting
        PASS [   0.066s] (2977/5472) aion-rs runtime::nif_timer::tests::author_timer_id_accepts_ordinary_name
        PASS [   0.062s] (2978/5472) aion-rs runtime::nif_timer::tests::author_timer_id_refuses_reserved_deadline_prefix
        PASS [   0.045s] (2979/5472) aion-rs runtime::nif_timer::tests::author_timer_id_refuses_reserved_schedule_prefix
        PASS [   0.321s] (2980/5472) aion-rs runtime::nif_signal::tests::signal_recorded_after_deadline_takes_the_timeout_branch
        PASS [   0.344s] (2981/5472) aion-rs runtime::nif_signal::tests::recovered_interleaved_sends_and_receives_replay_without_appends
        PASS [   0.398s] (2982/5472) aion-rs runtime::nif_signal::tests::signal_recorded_before_deadline_resolves_the_payload
        PASS [   0.408s] (2983/5472) aion-rs runtime::nif_signal::tests::recovered_send_replays_recorded_sent_despite_later_same_name_arrival
        PASS [   0.449s] (2984/5472) aion-rs runtime::nif_signal::tests::live_expiry_is_decided_from_the_resolution_snapshot_only
        PASS [   0.366s] (2985/5472) aion-rs runtime::nif_timer::tests::expired_scope_cancel_losing_to_a_recorded_fire_returns_fired
        PASS [   0.441s] (2986/5472) aion-rs runtime::nif_timer::tests::expired_scope_cancel_winning_returns_cancelled
        PASS [   0.342s] (2987/5472) aion-rs runtime::nif_timer_bridge::tests::a_refused_teardown_cancel_does_not_promise_a_reissue
        PASS [   0.353s] (2988/5472) aion-rs runtime::nif_timer_bridge::tests::a_refused_cancel_is_not_reported_as_a_refused_fire
        PASS [   0.324s] (2989/5472) aion-rs runtime::nif_timer_bridge::tests::a_torn_down_wheel_does_not_finalize_an_unregistered_timeout
        PASS [   0.346s] (2990/5472) aion-rs runtime::nif_timer_bridge::tests::a_rogue_head_event_does_not_reconcile_and_the_conflict_surfaces
        PASS [   0.383s] (2991/5472) aion-rs runtime::nif_timer_bridge::tests::a_torn_down_wheel_does_not_time_a_run_out
        PASS [   0.374s] (2992/5472) aion-rs runtime::nif_timer_bridge::tests::a_torn_down_wheel_refuses_to_append
        PASS [   0.350s] (2993/5472) aion-rs runtime::nif_timer_bridge::tests::a_wheel_teardown_during_arming_leaves_no_armed_timer
        PASS [   0.323s] (2994/5472) aion-rs runtime::nif_timer_bridge::tests::deadline_fire_retries_through_a_store_outage_spanning_both_reads
        PASS [   0.280s] (2995/5472) aion-rs runtime::process_exit::round12_tests::blocking_callback_cannot_backpressure_drainer_and_shutdown_is_retryable
        PASS [   0.381s] (2996/5472) aion-rs runtime::nif_timer_bridge::tests::stand_down_is_not_a_fault
        PASS [   0.435s] (2997/5472) aion-rs runtime::process_exit::round12_tests::beam_spawn_children_release_outcomes_to_baseline
        PASS [   0.349s] (2998/5472) aion-rs runtime::process_exit::round12_tests::panicking_callback_cannot_terminate_singleton_drainer
        PASS [   0.363s] (2999/5472) aion-rs runtime::process_exit::round13_tests::committed_callback_admission_hands_off_before_dispatcher_shutdown
        PASS [   0.796s] (3000/5472) aion-rs runtime::process_exit::round12_tests::lag_recovery_releases_beam_spawn_children_to_baseline
        PASS [   0.375s] (3001/5472) aion-rs runtime::process_exit::round13_tests::shutdown_drains_parked_release_only_children_to_baseline
        PASS [   0.793s] (3002/5472) aion-rs runtime::process_exit::round12_tests::lag_recovery_waits_for_spawn_reservation_before_snapshot
        PASS [   0.450s] (3003/5472) aion-rs runtime::process_exit::round14_tests::fun_spawn_variants_are_release_only_and_return_maps_to_baseline
        PASS [   6.448s] (3004/5472) aion-rs runtime::handle::tests::repeated_completed_payload_spawns_do_not_accumulate_retained_heaps
        PASS [   0.108s] (3005/5472) aion-rs runtime::wake_confirm::tests::already_observed_orders_never_wake
        PASS [   0.424s] (3006/5472) aion-rs runtime::process_exit::round14_tests::shutdown_union_drains_parked_fun_spawn_children_to_baseline
        PASS [   0.211s] (3007/5472) aion-rs runtime::wake_confirm::tests::concurrent_orders_stop_independently
        PASS [   0.049s] (3008/5472) aion-rs runtime::wake_confirm::tests::shutdown_is_idempotent_and_gates_new_orders
        PASS [   0.026s] (3009/5472) aion-rs schedule::evaluator::tests::allow_all_overlap_always_starts
        PASS [   1.121s] (3010/5472) aion-rs runtime::process_exit::round13_tests::full_callback_queue_retains_bounded_automatic_retries
        PASS [   0.426s] (3011/5472) aion-rs runtime::process_exit::tests::runtime_claims_the_only_exit_event_subscription_with_typed_duplicate_failure
        PASS [   0.025s] (3012/5472) aion-rs schedule::evaluator::tests::buffer_one_overlap_queues_at_most_one
        PASS [   0.047s] (3013/5472) aion-rs schedule::evaluator::tests::buffered_tick_does_not_start_after_schedule_becomes_inactive
        PASS [   0.041s] (3014/5472) aion-rs schedule::evaluator::tests::cancel_previous_overlap_cancels_then_starts
        PASS [   0.038s] (3015/5472) aion-rs schedule::evaluator::tests::paused_and_deleted_schedules_do_not_arm
        PASS [   0.032s] (3016/5472) aion-rs schedule::evaluator::tests::recovery_applies_skip_catch_up_without_recording_trigger
        PASS [   0.049s] (3017/5472) aion-rs schedule::evaluator::tests::recovery_applies_one_catch_up_and_records_trigger
        PASS [   0.049s] (3018/5472) aion-rs schedule::evaluator::tests::recovery_applies_all_catch_up_and_records_triggers
        PASS [   0.634s] (3019/5472) aion-rs runtime::process_exit::tests::lagged_stream_resynchronizes_every_registered_outcome
        PASS [   0.027s] (3020/5472) aion-rs schedule::evaluator::tests::recovery_reconstructs_state_and_rearms_only_active_schedules
        PASS [   0.665s] (3021/5472) aion-rs runtime::process_exit::round14_tests::overflow_recovery_releases_both_fun_spawn_variants_to_baseline
        PASS [   0.027s] (3022/5472) aion-rs schedule::policy::tests::allow_all_policy_starts_even_when_execution_is_running
        PASS [   0.047s] (3023/5472) aion-rs schedule::evaluator::tests::skip_overlap_skips_and_rearms
        PASS [   0.048s] (3024/5472) aion-rs schedule::evaluator::tests::timer_fire_starts_records_and_rearms_next_fire
        PASS [   0.036s] (3025/5472) aion-rs schedule::policy::tests::buffer_one_policy_queues_at_most_one_pending_tick
        PASS [   0.036s] (3026/5472) aion-rs schedule::policy::tests::cancel_previous_policy_cancels_then_starts_when_execution_is_running
        PASS [   0.027s] (3027/5472) aion-rs schedule::policy::tests::catch_up_all_fires_every_missed_trigger
        PASS [   0.022s] (3028/5472) aion-rs schedule::state::tests::created_state_is_unpaused_with_initial_next_trigger
        PASS [   0.025s] (3029/5472) aion-rs schedule::trigger::tests::interval_returns_reference_plus_period
        PASS [   0.038s] (3030/5472) aion-rs schedule::state::tests::schedule_events_project_state_fields
        PASS [   0.062s] (3031/5472) aion-rs schedule::policy::tests::catch_up_skip_only_advances_to_future
        PASS [   0.066s] (3032/5472) aion-rs schedule::policy::tests::catch_up_one_fires_once_and_advances_to_future
        PASS [   0.068s] (3033/5472) aion-rs schedule::policy::tests::skip_policy_skips_when_execution_is_running
        PASS [   0.036s] (3034/5472) aion-rs schedule::trigger::tests::invalid_cron_trigger_returns_typed_parse_error
        PASS [   0.044s] (3035/5472) aion-rs schedule::trigger::tests::interval_timestamp_overflow_returns_typed_range_error
        PASS [   0.039s] (3036/5472) aion-rs schedule::trigger::tests::midnight_cron_returns_next_midnight_strictly_after_reference
        PASS [   0.042s] (3037/5472) aion-rs schedule::trigger::tests::invalid_cron_expressions_return_typed_errors
        PASS [   0.375s] (3038/5472) aion-rs runtime::wake_confirm::tests::ladder_persists_until_the_target_is_observed
        PASS [   0.045s] (3039/5472) aion-rs schedule::trigger::tests::oversized_interval_returns_typed_range_error
        PASS [   0.034s] (3040/5472) aion-rs schedule::trigger::tests::successive_calls_are_strictly_increasing
        PASS [   0.048s] (3041/5472) aion-rs schedule::trigger::tests::zero_interval_returns_typed_error
        PASS [   0.057s] (3042/5472) aion-rs schedule::trigger::tests::parses_standard_five_field_cron_expressions
        PASS [   0.060s] (3043/5472) aion-rs signal::resume::tests::deferred_signals_deliver_in_order_exactly_once
        PASS [   0.050s] (3044/5472) aion-rs signal::resume::tests::failed_delivery_is_logged_dropped_and_subsequent_signals_continue
        PASS [   0.078s] (3045/5472) aion-rs signal::router::tests::dead_resident_pid_records_signal_and_resolves_as_accepted
        PASS [   0.087s] (3046/5472) aion-rs signal::router::tests::terminal_run_rejects_signal_without_appending
        PASS [   0.061s] (3047/5472) aion-rs supervision::tree::tests::activity_child_cannot_alias_workflow_process
        PASS [   0.156s] (3048/5472) aion-rs supervision::policy::tests::activity_process_policy_keeps_children_non_trapping
        PASS [   0.030s] (3049/5472) aion-rs supervision::tree::tests::activity_children_are_one_level_below_workflow_process
        PASS [   0.026s] (3050/5472) aion-rs supervision::tree::tests::missing_workflow_activity_parent_is_typed_error
        PASS [   0.035s] (3051/5472) aion-rs supervision::tree::tests::one_engine_root_has_one_supervisor_per_workflow_type
        PASS [   0.031s] (3052/5472) aion-rs supervision::tree::tests::placing_existing_workflow_is_idempotent_and_preserves_children
        PASS [   0.040s] (3053/5472) aion-rs supervision::tree::tests::poisoned_tree_lock_returns_typed_registry_error
        PASS [   0.030s] (3054/5472) aion-rs supervision::tree::tests::workflow_pid_cannot_move_between_type_supervisors
        PASS [   0.026s] (3055/5472) aion-rs time::deadline::tests::anonymous_timer_is_not_a_deadline
        PASS [   0.037s] (3056/5472) aion-rs supervision::tree::tests::workflows_sit_under_type_supervisors_not_new_supervisors
        PASS [   0.020s] (3057/5472) aion-rs time::deadline::tests::author_named_timer_is_not_a_deadline
        PASS [   0.026s] (3058/5472) aion-rs time::deadline::tests::deadline_prefixed_but_malformed_suffix_has_no_run
        PASS [   0.034s] (3059/5472) aion-rs time::deadline::tests::deadline_timer_id_round_trips_to_its_run
        PASS [   0.030s] (3060/5472) aion-rs time::deadline::tests::outstanding_deadline_timer_is_none_for_a_run_without_a_deadline
        PASS [   0.025s] (3061/5472) aion-rs time::deadline::tests::outstanding_deadline_timer_tracks_liveness_and_scopes_to_the_run
        PASS [   0.350s] (3062/5472) aion-rs supervision::policy::tests::policy_spawns_workflow_through_runtime_trapping_api
        PASS [   0.040s] (3063/5472) aion-rs time::named::tests::cancel_timer_after_cancel_is_idempotent_noop
        PASS [   0.378s] (3064/5472) aion-rs supervision::policy::tests::abnormal_activity_exit_is_trapped_by_workflow
        PASS [   0.371s] (3065/5472) aion-rs supervision::policy::tests::cancelling_workflow_terminates_linked_activity_children
        PASS [  10.375s] (3066/5472) aion-rs lifecycle::deadline::tests::deadline_and_cancel_race_records_exactly_one_terminal
        PASS [   0.046s] (3067/5472) aion-rs time::named::tests::cancel_timer_after_fire_is_noop
        PASS [   0.042s] (3068/5472) aion-rs time::named::tests::cancel_timer_disarms_resident_wheel_and_records_cancelled
        PASS [   0.367s] (3069/5472) aion-rs supervision::policy::tests::workflow_spawn_policy_requests_trap_exit
        PASS [   0.052s] (3070/5472) aion-rs time::named::tests::cancel_timer_settles_anonymous_scope_deadline
        PASS [   0.050s] (3071/5472) aion-rs time::named::tests::sleep_derives_anonymous_id_and_fire_at_from_recorded_inputs
        PASS [   0.059s] (3072/5472) aion-rs time::named::tests::start_timer_arms_named_timer_without_rewriting_id
        PASS [   0.066s] (3073/5472) aion-rs time::named::tests::start_timer_preserves_named_id_in_history_and_timer_row
        PASS [   0.052s] (3074/5472) aion-rs time::recovery::tests::cancelled_predecessor_deadline_is_not_rearmed_after_continue_as_new
        PASS [   0.038s] (3075/5472) aion-rs time::recovery::tests::orphaned_timer_for_unknown_workflow_is_skipped_not_fatal
        PASS [   0.048s] (3076/5472) aion-rs time::recovery::tests::cancelled_timer_is_never_fired_by_recovery
        PASS [   0.047s] (3077/5472) aion-rs time::recovery::tests::running_startup_sweep_twice_records_due_timer_once_total
        PASS [   0.048s] (3078/5472) aion-rs time::recovery::tests::startup_sweep_does_not_fire_future_timer
        PASS [   0.034s] (3079/5472) aion-rs time::timer_service::tests::already_recorded_fire_after_run_terminal_delivers_no_wake
        PASS [   0.035s] (3080/5472) aion-rs time::timer_service::tests::already_recorded_fire_delivers_owed_wake_without_second_append
        PASS [   0.054s] (3081/5472) aion-rs time::recovery::tests::tick_uses_injected_clock_records_once_and_redelivers_the_wake
        PASS [   0.062s] (3082/5472) aion-rs time::recovery::tests::startup_sweep_fires_past_timer_and_delivers
        PASS [   0.051s] (3083/5472) aion-rs time::timer_service::tests::already_recorded_fire_for_nonresident_workflow_wakes_nothing
        PASS [   0.039s] (3084/5472) aion-rs time::timer_service::tests::deadline_fire_without_handler_is_typed_error
        PASS [   0.043s] (3085/5472) aion-rs time::timer_service::tests::deadline_fire_routes_to_handler_and_records_no_timer_fired
        PASS [   0.031s] (3086/5472) aion-rs time::timer_service::tests::deadline_handler_failure_surfaces_as_typed_error
        PASS [   0.037s] (3087/5472) aion-rs time::timer_service::tests::disposition_tracks_the_last_event_for_the_id_and_agrees_with_liveness
        PASS [   0.024s] (3088/5472) aion-rs time::timer_service::tests::fire_records_without_delivery_when_workflow_becomes_nonresident
        PASS [   0.025s] (3089/5472) aion-rs time::timer_service::tests::fire_records_timer_fired_then_delivers_mailbox_message
        PASS [   0.022s] (3090/5472) aion-rs time::timer_service::tests::firing_prior_run_timer_after_continue_as_new_is_noop
        PASS [   0.036s] (3091/5472) aion-rs time::timer_service::tests::fire_resolves_residency_at_fire_time
        PASS [   0.030s] (3092/5472) aion-rs time::timer_service::tests::firing_cancelled_timer_is_noop
        PASS [   0.058s] (3093/5472) aion-rs time::timer_service::tests::firing_same_timer_twice_records_once_and_redelivers_the_wake
        PASS [   0.039s] (3094/5472) aion-rs time::timer_service::tests::refused_terminal_fire_records_nothing_and_delivers_no_wake
        PASS [   0.048s] (3095/5472) aion-rs time::timer_service::tests::firing_unstarted_timer_records_nothing
        PASS [   0.052s] (3096/5472) aion-rs time::timer_service::tests::prior_run_segment_timer_is_not_live
        PASS [   0.042s] (3097/5472) aion-rs time::timer_service::tests::restarted_named_timer_after_cancel_is_live
        PASS [   0.065s] (3098/5472) aion-rs time::timer_service::tests::re_armed_named_timer_fires_again
        PASS [   0.052s] (3099/5472) aion-rs time::timer_service::tests::restarted_named_timer_after_fire_is_live
        PASS [   0.050s] (3100/5472) aion-rs time::timer_service::tests::schedule_records_timer_row_without_timer_started_event
        PASS [   0.042s] (3101/5472) aion-rs time::timer_service::tests::started_then_cancelled_timer_is_dead
        PASS [   0.055s] (3102/5472) aion-rs time::timer_service::tests::schedule_for_nonresident_records_without_arming
        PASS [   0.058s] (3103/5472) aion-rs time::timer_service::tests::schedule_arms_wheel_for_resident_workflow
        PASS [   0.052s] (3104/5472) aion-rs time::timer_service::tests::shared_coordinator_serializes_cancel_and_fire_across_services
        PASS [   0.044s] (3105/5472) aion-rs time::timer_service::tests::started_then_fired_timer_is_dead
        PASS [   0.046s] (3106/5472) aion-rs time::timer_service::tests::started_timer_is_live
        PASS [   0.027s] (3107/5472) aion-rs::activity_retry_e2e gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.040s] (3108/5472) aion-rs::activity_retry_e2e gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.054s] (3109/5472) aion-rs::activity_retry_e2e gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.058s] (3110/5472) aion-rs::activity_retry_e2e gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.067s] (3111/5472) aion-rs::activity_retry_e2e gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.063s] (3112/5472) aion-rs::activity_retry_e2e gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.037s] (3113/5472) aion-rs::activity_retry_e2e gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   0.053s] (3114/5472) aion-rs::activity_retry_e2e gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.032s] (3115/5472) aion-rs::adoption_parked_timer_e2e gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.095s] (3116/5472) aion-rs::activity_retry_e2e gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.074s] (3117/5472) aion-rs::adoption_parked_timer_e2e gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.042s] (3118/5472) aion-rs::adoption_parked_timer_e2e gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.053s] (3119/5472) aion-rs::adoption_parked_timer_e2e gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.043s] (3120/5472) aion-rs::adoption_parked_timer_e2e gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.054s] (3121/5472) aion-rs::adoption_parked_timer_e2e gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.024s] (3122/5472) aion-rs::adoption_parked_timer_e2e gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.040s] (3123/5472) aion-rs::adoption_parked_timer_e2e gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   0.062s] (3124/5472) aion-rs::adoption_parked_timer_xnode_e2e gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.047s] (3125/5472) aion-rs::adoption_parked_timer_xnode_e2e gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.140s] (3126/5472) aion-rs::adoption_parked_timer_e2e gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.042s] (3127/5472) aion-rs::adoption_parked_timer_xnode_e2e gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.040s] (3128/5472) aion-rs::adoption_parked_timer_xnode_e2e gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.038s] (3129/5472) aion-rs::adoption_parked_timer_xnode_e2e gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.022s] (3130/5472) aion-rs::adoption_parked_timer_xnode_e2e gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.014s] (3131/5472) aion-rs::adoption_parked_timer_xnode_e2e gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.037s] (3132/5472) aion-rs::adoption_parked_timer_xnode_e2e gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   0.091s] (3133/5472) aion-rs::adoption_parked_timer_xnode_e2e gleam_test_support::every_gleam_support_file_is_classified
        PASS [  18.042s] (3134/5472) aion-package::golden_gleam_exec generated_golden_passes_real_gleam_test_for_float_optional_enum_and_nested
        PASS [   2.828s] (3135/5472) aion-rs::advisory_class_e2e advisory_exhaustion_warns_and_the_run_completes
        PASS [   2.872s] (3136/5472) aion-rs::advisory_class_e2e advisory_exhaustion_replays_from_history_without_divergence
        PASS [   0.026s] (3137/5472) aion-rs::aion_active_active_showcase gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.067s] (3138/5472) aion-rs::aion_active_active_showcase gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.055s] (3139/5472) aion-rs::aion_active_active_showcase gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.033s] (3140/5472) aion-rs::aion_active_active_showcase gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.028s] (3141/5472) aion-rs::aion_active_active_showcase gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.023s] (3142/5472) aion-rs::aion_active_active_showcase gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.026s] (3143/5472) aion-rs::aion_active_active_showcase gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.048s] (3144/5472) aion-rs::aion_active_active_showcase gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.049s] (3145/5472) aion-rs::aion_active_active_showcase gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   6.328s] (3146/5472) aion-rs runtime::nif_timer_bridge::tests::an_ack_lost_ordinary_fire_is_retried_reconciled_and_the_run_keeps_appending
        PASS [   0.058s] (3147/5472) aion-rs::aion_cluster_failover_showcase gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.034s] (3148/5472) aion-rs::aion_cluster_failover_showcase gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.032s] (3149/5472) aion-rs::aion_cluster_failover_showcase gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   6.604s] (3150/5472) aion-rs runtime::nif_timer_bridge::tests::a_transiently_refused_ordinary_fire_is_retried_to_success
        PASS [   0.045s] (3151/5472) aion-rs::aion_cluster_failover_showcase gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.022s] (3152/5472) aion-rs::aion_cluster_failover_showcase gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.053s] (3153/5472) aion-rs::aion_cluster_failover_showcase gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.037s] (3154/5472) aion-rs::aion_cluster_failover_showcase gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.095s] (3155/5472) aion-rs::aion_cluster_failover_showcase gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.035s] (3156/5472) aion-rs::aion_cluster_failover_showcase gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   4.370s] (3157/5472) aion-rs::activity_retry_e2e exhausted_retries_fail_the_run_and_reopen_continues_the_trail
        PASS [   0.058s] (3158/5472) aion-rs::aion_on_haematite_showcase gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.030s] (3159/5472) aion-rs::aion_on_haematite_showcase gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.038s] (3160/5472) aion-rs::aion_on_haematite_showcase gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.045s] (3161/5472) aion-rs::aion_on_haematite_showcase gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.043s] (3162/5472) aion-rs::aion_on_haematite_showcase gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.039s] (3163/5472) aion-rs::aion_on_haematite_showcase gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.098s] (3164/5472) aion-rs::aion_on_haematite_showcase gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.041s] (3165/5472) aion-rs::aion_on_haematite_showcase gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.063s] (3166/5472) aion-rs::aion_on_haematite_showcase gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [  30.186s] (3167/5472) aion-cli::generate_e2e generate_round_trips_check_gates_and_invents_no_defaults
        PASS [   2.863s] (3168/5472) aion-rs::advisory_class_e2e non_advisory_exhaustion_still_fails_the_run
        PASS [  10.264s] (3169/5472) aion-rs runtime::nif_continue_as_new::tests::the_nif_gate_ends_a_live_process_for_each_outcome_that_must_end_it_and_spares_the_rest
        PASS [   7.898s] (3170/5472) aion-rs::activity_retry_e2e in_process_dispatch_miss_is_typed_recorded_and_fails_the_run
        PASS [   0.197s] (3171/5472) aion-rs::awl_bc4_differential adversarial::adversarial_fixtures_classify_as_expected
        PASS [   4.225s] (3172/5472) aion-rs::awl_bc4_differential abi::abi_static_rows_over_spliced_entry
        PASS [   4.207s] (3173/5472) aion-rs::awl_bc4_differential abi::int_code_end_terminator_is_rejected
        PASS [   6.345s] (3174/5472) aion-rs::awl_bc4_differential abi::abi_entry_result_recording_through_the_trail
        PASS [  12.035s] (3175/5472) aion-rs::activity_retry_e2e retryable_flakes_cost_retries_not_the_run
        PASS [   9.707s] (3176/5472) aion-rs::aion_active_active_showcase multi_shard_active_active_failover_demo
        PASS [   0.200s] (3177/5472) aion-rs::awl_bc4_differential covered::every_covered_fixture_declares_a_distinct_workflow_name
        PASS [  10.075s] (3178/5472) aion-rs::aion_cluster_failover_showcase aion_workflow_survives_owner_node_death_on_haematite_cluster
        PASS [   0.190s] (3179/5472) aion-rs::awl_bc4_differential covered::the_exclusion_list_is_exactly_the_unobservable_park_population
        PASS [   0.050s] (3180/5472) aion-rs::awl_bc4_differential driver::classification_tests::a_one_sided_refusal_is_a_divergence
        PASS [   0.044s] (3181/5472) aion-rs::awl_bc4_differential driver::classification_tests::aliasing_erases_identities_and_nothing_else
        PASS [   0.031s] (3182/5472) aion-rs::awl_bc4_differential driver::classification_tests::identical_refusals_are_agreement_not_infrastructure_noise
        PASS [   0.032s] (3183/5472) aion-rs::awl_bc4_differential driver::classification_tests::refusals_for_different_reasons_are_a_divergence
        PASS [  13.679s] (3184/5472) aion-rs::adoption_parked_timer_e2e parked_timer_workflow_resumes_on_shard_adoption
        PASS [   0.034s] (3185/5472) aion-rs::awl_bc4_differential gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.068s] (3186/5472) aion-rs::awl_bc4_differential gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.047s] (3187/5472) aion-rs::awl_bc4_differential gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.040s] (3188/5472) aion-rs::awl_bc4_differential gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.020s] (3189/5472) aion-rs::awl_bc4_differential gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.026s] (3190/5472) aion-rs::awl_bc4_differential gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.014s] (3191/5472) aion-rs::awl_bc4_differential gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.056s] (3192/5472) aion-rs::awl_bc4_differential gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.024s] (3193/5472) aion-rs::awl_bc4_differential gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   0.010s] (3194/5472) aion-rs::awl_bc4_differential serde_pin::event_serde_field_names_match_the_normalizer_literals
        PASS [   0.010s] (3195/5472) aion-rs::awl_bc4_differential serde_pin::normalizer_leaves_adversarially_named_or_valued_fields_untouched
        PASS [   0.010s] (3196/5472) aion-rs::awl_bc4_differential serde_pin::normalizer_replaces_every_identity_field
        PASS [   0.013s] (3197/5472) aion-rs::awl_bc4_differential terminal_order_controls::a_missing_or_extra_event_stays_red
        PASS [   0.027s] (3198/5472) aion-rs::awl_bc4_differential terminal_order_controls::a_result_bound_to_the_wrong_ordinal_stays_red
        PASS [   0.027s] (3199/5472) aion-rs::awl_bc4_differential terminal_order_controls::a_terminal_before_its_own_anchor_stays_red
        PASS [   0.037s] (3200/5472) aion-rs::awl_bc4_differential terminal_order_controls::a_terminal_moved_across_a_non_terminal_boundary_stays_red
        PASS [   0.058s] (3201/5472) aion-rs::awl_bc4_differential terminal_order_controls::a_trail_without_adjacent_terminals_is_returned_untouched
        PASS [   6.084s] (3202/5472) aion-rs::awl_bc4_differential child_boundary::child_spawning_fixtures_fail_at_the_child_boundary
        PASS [   0.022s] (3203/5472) aion-rs::awl_bc4_differential terminal_order_controls::settle_order_alone_compares_equal
        PASS [   0.058s] (3204/5472) aion-rs::awl_bc4_differential terminal_order_controls::reordered_sched_anchors_stay_red
        PASS [   0.032s] (3205/5472) aion-rs::awl_retry_counting_e2e gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.056s] (3206/5472) aion-rs::awl_retry_counting_e2e gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.024s] (3207/5472) aion-rs::awl_retry_counting_e2e gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.017s] (3208/5472) aion-rs::awl_retry_counting_e2e gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.052s] (3209/5472) aion-rs::awl_retry_counting_e2e gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.026s] (3210/5472) aion-rs::awl_retry_counting_e2e gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [  14.034s] (3211/5472) aion-rs::adoption_parked_timer_xnode_e2e parked_timer_workflow_resumes_on_xnode_shard_adoption
        PASS [   0.032s] (3212/5472) aion-rs::awl_retry_counting_e2e gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.039s] (3213/5472) aion-rs::awl_retry_counting_e2e gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   0.080s] (3214/5472) aion-rs::awl_retry_counting_e2e gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.672s] (3215/5472) aion-rs::child_await_e2e await_child_follows_continue_as_new_chain_and_survives_restart
        PASS [   5.268s] (3216/5472) aion-rs::awl_bc4_differential child_boundary::declarations_combined_fails_via_data_driven_route_failure
        PASS [   0.933s] (3217/5472) aion-rs::child_await_e2e can_replacement_sweep_starts_the_successor_run_at_startup
        PASS [   0.755s] (3218/5472) aion-rs::child_await_e2e pre_d14_parent_beam_runs_unchanged_while_sixteen_awaits_resolve
        PASS [   6.277s] (3219/5472) aion-rs::awl_bc4_differential compensation::a_routed_failure_outcome_never_compensates
        PASS [   1.323s] (3220/5472) aion-rs::child_await_e2e crash_mid_await_child_recovers_and_matches_uncrashed_control
        PASS [   1.047s] (3221/5472) aion-rs::child_await_e2e queried_crash_recovery_matches_unqueried_uncrashed_control
        PASS [   0.935s] (3222/5472) aion-rs::child_await_e2e query_answers_while_parent_is_parked_in_await_child
        PASS [   0.411s] (3223/5472) aion-rs::child_await_e2e watcher_resolves_terminal_child_with_no_handle_from_the_store
        PASS [   0.054s] (3224/5472) aion-rs::child_await_query_e2e gleam_test_support::a_project_gleam_refuses_fails_as_a_code_defect
        PASS [   0.042s] (3225/5472) aion-rs::child_await_query_e2e gleam_test_support::a_rejected_module_is_a_defect
        PASS [   0.029s] (3226/5472) aion-rs::child_await_query_e2e gleam_test_support::an_unreachable_registry_is_infrastructure
        PASS [   0.511s] (3227/5472) aion-rs::child_await_e2e unloaded_child_type_fails_before_recording
        PASS [   0.046s] (3228/5472) aion-rs::child_await_query_e2e gleam_test_support::an_unsatisfiable_requirement_is_a_defect
        PASS [   0.034s] (3229/5472) aion-rs::child_await_query_e2e gleam_test_support::dependency_failure_titles_are_classified_by_layer
        PASS [   0.036s] (3230/5472) aion-rs::child_await_query_e2e gleam_test_support::download_progress_alone_is_not_a_registry_failure
        PASS [   0.024s] (3231/5472) aion-rs::child_await_query_e2e gleam_test_support::the_ci_tar_archive_failure_is_infrastructure
        PASS [   0.033s] (3232/5472) aion-rs::child_await_query_e2e gleam_test_support::the_shared_gleam_support_copies_have_not_diverged
        PASS [   0.126s] (3233/5472) aion-rs::child_await_query_e2e gleam_test_support::every_gleam_support_file_is_classified
        PASS [   0.745s] (3234/5472) aion-rs::child_await_e2e recovery_sweep_starts_recorded_but_never_spawned_child_exactly_once
        PASS [   0.536s] (3235/5472) aion-rs::child_workflows_e2e child_workflow_runs_end_to_end_through_nif_path
        PASS [  13.963s] (3236/5472) aion-rs::aion_on_haematite_showcase aion_workflow_survives_restart_recovered_from_haematite
        PASS [   0.618s] (3237/5472) aion-rs::child_workflows_e2e restart_after_child_completed_replays_recorded_child_without_respawn
        PASS [   1.326s] (3238/5472) aion-rs::child_workflows_e2e continue_as_new_scopes_child_correlation_to_each_run
        PASS [   0.503s] (3239/5472) aion-rs::child_workflows_e2e restart_mid_child_resumes_awaiting_same_child
        PASS [   0.546s] (3240/5472) aion-rs::child_workflows_e2e spawned_child_records_its_parent_and_the_summary_names_it
        PASS [   0.643s] (3241/5472) aion-rs::concurrency_e2e a_layer_settled_in_inverted_order_replays_each_result_onto_its_own_member
        PASS [   0.973s] (3242/5472) aion-rs::child_workflows_e2e signal_between_spawns_preserves_child_identities_across_restart
        PASS [   8.198s] (3243/5472) aion-rs::awl_bc4_differential compensation::compensation_arms_execute_on_both_backends
        FAIL [   6.669s] (3244/5472) aion-rs::awl_bc4_differential covered::flagship_awl_hello_matches
  stdout ───

    running 1 test
    test covered::flagship_awl_hello_matches ... FAILED

    failures:

    failures:
        covered::flagship_awl_hello_matches

    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 36 filtered out; finished in 6.63s

  stderr ───
    COMPILED 1 module(s) under real `gleam build` in 1 attempt(s): awl_hello

    thread 'covered::flagship_awl_hello_matches' (3344632) panicked at crates/aion/tests/awl_bc4_differential/covered.rs:431:5:
    flagship not clean:
    differential report: 0 succeeded, 0 failed-path, 0 cancelled, 0 timer-parked, 0 excluded, 0 refused, 0 AGREED-REFUSAL, 1 DIVERGENCES, 0 INFRA
      DIVERGENCE flagship/valid/awl_hello event[0] : reference="disposition=Parked" direct="disposition=Completed"

    note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

  Cancelling due to test failure: 15 tests still running
        PASS [   1.236s] (3245/5472) aion-rs::concurrency_e2e a_parallel_layer_records_one_trail_whichever_member_settles_first
        PASS [   1.061s] (3246/5472) aion-rs::concurrency_e2e collect_all_fail_fast_cancels_unresolved_and_replays_byte_identically
        PASS [   1.012s] (3247/5472) aion-rs::concurrency_e2e collect_all_success_returns_input_order_and_replays_byte_identically
        PASS [   0.868s] (3248/5472) aion-rs::concurrency_e2e collect_race_settles_first_failure_and_replays_byte_identically
        PASS [  14.263s] (3249/5472) aion-rs::awl_bc4_differential adversarial::adversarial_differential_is_byte_identical
        PASS [  17.262s] (3250/5472) aion-rs::aion_cluster_failover_showcase inflight_workflow_completes_on_survivor_after_owner_death
        PASS [   8.232s] (3251/5472) aion-rs::awl_bc4_differential oracle_self_test::reference_package_under_direct_expectation_is_refused
        PASS [  11.329s] (3252/5472) aion-rs::awl_retry_counting_e2e retry_three_runs_the_action_four_times_on_both_backends
        PASS [  12.068s] (3253/5472) aion-rs::child_await_query_e2e parent_awaits_child_and_completes
        PASS [  17.332s] (3254/5472) aion-rs::child_await_query_e2e query_answered_while_parked_in_child_await
        PASS [  58.124s] (3255/5472) aion-cli::new_scaffold_build saga_worker_crate_passes_cargo_check
        PASS [  59.987s] (3256/5472) aion-cli::new_scaffold_build dev_pipeline_worker_crate_is_formatted_and_passes_cargo_check
        SLOW [> 60.000s] (─────────) aion-package::awl_worker_build the_generated_worker_compiles_lints_and_proves_its_advertisement
        SLOW [> 60.000s] (─────────) aion-package::rust_worker_build generated_rust_worker_compiles_against_real_aion_worker
        PASS [  63.186s] (3257/5472) aion-package::rust_worker_build generated_rust_worker_compiles_against_real_aion_worker
        SLOW [> 60.000s] (─────────) aion-rs::awl_bc4_differential covered::covered_set_is_byte_identical
        PASS [ 101.528s] (3258/5472) aion-package::awl_worker_build the_generated_worker_compiles_lints_and_proves_its_advertisement
        PASS [ 102.722s] (3259/5472) aion-rs::awl_bc4_differential covered::covered_set_is_byte_identical
────────────
     Summary [ 143.404s] 3259/5472 tests run: 3258 passed (3 slow, 3 leaky), 1 failed, 0 skipped
        FAIL [   6.669s] (3244/5472) aion-rs::awl_bc4_differential covered::flagship_awl_hello_matches
warning: 2213/5472 tests were not run due to test failure (run with --no-fail-fast to run all tests, or run with --max-fail)
error: test run failed
GATE-RED: cargo nextest run --workspace rc=100 (named cause: suite_red)
