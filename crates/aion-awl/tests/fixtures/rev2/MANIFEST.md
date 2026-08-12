# rev-2 fixture corpus — coverage ledger

One line per fixture. Later phases (lexer, parser/printer, checker, emitter) gate against this
ledger: every valid fixture must parse, print canonically (except where noted), and check clean;
every invalid fixture must fail at the sidecar's stage with a diagnostic containing the sidecar's
substring, primary span on the sidecar's 1-based line.

Sidecar format (`<name>.expected`, 3 lines): `PARSE|CHECK` / required message substring /
1-based line of the primary span. Spec of record:
`docs/design/aion-authoring/awl/AWL-2-SPEC.md`; decisions D1-D9 in `AWL-2-BUILD-PLAN.md`.

## flagship — the two spec examples, verbatim

- `flagship/valid/awl_hello.awl` — spec worked example 1, byte-identical; pipe chain with
  `.field` stage and `|> route` terminator, single worker, bare actions.
- `flagship/valid/dev_brief.awl` — spec worked example 2, byte-identical; file-import schema
  door, `String?`, doc'd type + field, `after` DAG diamond, unbound side-effect call,
  `loop = seed counting … until … max`, fork-over-collection + `join ->`, `filter(.field)`,
  `is empty`, route-targeted step, two-line canonical outcome break, `== 1` comparison in payload.
- `flagship/valid/schemas/brief.schema.json` — import target for `Brief` (object, optional
  `acceptance_criteria` array, descriptions).
- `flagship/valid/schemas/run_config.schema.json` — import target for `RunConfig` (nested
  `$defs`-local `$ref` for lenses; required string/array/integer fields matching every
  `config.*` use in dev_brief).

## header-types — narration, header decls, shorthand types, enums

Valid:
- `header-types/valid/minimal.awl` — smallest complete workflow: one input, one outcome, one type,
  one worker/action, pipe-route step.
- `header-types/valid/builtins.awl` — every builtin: `Dir`, `Float`, `Int`, `Bool`, `String`,
  `[String]`, `String?`, `[String]?`, `Nil` action result, composed optional-list field.
- `header-types/valid/zero_inputs.awl` — zero-input workflow (grammar requires only >=1 outcome).
- `header-types/valid/workflow_timeout.awl` — document-level `timeout 6h` in the workflow header;
  timeout remains compile metadata and does not change emitted workflow code.
- `header-types/valid/signal_wait.awl` — `signal` decl + `wait <signal> -> name`, two outcomes
  (success + failure), payload construction in route.
- `header-types/valid/enum.awl` — payload-less enum declared and used as a field type.
- `header-types/valid/doc_comments.awl` — multi-line `///` on types, `///` on fields
  (field docs force multi-line layout per golden precedent).
- `header-types/valid/line_width.awl` — 100-col rule: single-line type vs multi-line type with
  trailing commas.
- `header-types/valid/noncanonical_commas.awl` — comma tolerance (multi-line without trailing
  commas; single-line WITH trailing comma). Parse-accepts, fmt normalizes: EXCLUDE from
  byte-identity print goldens.
- `header-types/valid/combined.awl` — kitchen sink: multi-line `//!`, 4 inputs, signal, enum,
  doc'd type, durable wait, canonical two-line outcome break; plain `//` trivia comments at
  declaration level and statement level (the corpus's lossless-comment witnesses).

Invalid (stage / substring / span line):
- `missing_doc_header.awl` — no `//!` narration → PARSE "//!" @1 (judgment: grammar says one or
  more `//!` lines; re-stage to CHECK if the checker owns it).
- `doc_line_after_header.awl` — `//!` after header → PARSE "//!" @7.
- `no_outcomes.awl` — zero outcomes → PARSE "outcome" @2 (same judgment note as above).
- `input_missing_type.awl` — `input host` → PARSE "input" @3.
- `signal_missing_type.awl` — `signal cancel` → PARSE "signal" @4.
- `outcome_missing_type.awl` — outcome without `type T` → PARSE "type" @4.
- `outcome_route_invalid.awl` — `route sideways` (not success|failure) → PARSE "sideways" @4.
- `keyword_as_input_name.awl` — `input step:` (keyword reserved everywhere) → PARSE "step" @4.
- `unclosed_list_type.awl` — `[String` → PARSE "]" @4.
- `about_gone.awl` — gone keyword `about` → PARSE "about" @3 (pointed migration diagnostic).
- `option_gone.awl` — gone `Option(String)` → PARSE "Option" @4.
- `list_gone.awl` — gone `List(String)` → PARSE "List" @6.
- `finish_gone.awl` — gone `finish` statement (finishing IS routing) → PARSE "finish" @13.
- `enum_payload_variant.awl` — `Circle(Float)` payload variant deferred → PARSE "payload" @7.
- `duplicate_input.awl` — CHECK "duplicate input" @4 (span at second occurrence).
- `duplicate_signal.awl` — CHECK "duplicate signal" @5.
- `duplicate_outcome.awl` — CHECK "duplicate outcome" @5.
- `duplicate_type.awl` — CHECK "duplicate type" @7.
- `duplicate_field.awl` — CHECK "duplicate field" @6.
- `builtin_type_redeclared.awl` — `type Bool { … }` → CHECK "Bool" @7.
- `unknown_input_type.awl` — CHECK "unknown type" @4.
- `unknown_outcome_type.awl` — CHECK "unknown type" @4.
- `workflow_name_not_snake_case.awl` — CHECK "snake_case" @2.
- `type_name_not_title_case.awl` — CHECK "TitleCase" @7.
- `field_name_not_snake_case.awl` — CHECK "snake_case" @6.
- `enum_variant_not_title_case.awl` — CHECK "TitleCase" @7.
- `list_element_optional.awl` — `[String?]` field type in a shorthand type declaration (ruled
  2026-07-11: `?` illegal in list-element position) → CHECK "element" @6 (span on the element
  type).

## schema-doors — inline `schema {…}`, file import, `?` optionality

Valid (schema files live beside the .awl that imports them):
- `schema-doors/valid/inline_schema_round.awl` — inline raw-schema door (spec-verbatim Round
  shape) projected into guards and payloads.
- `schema-doors/valid/import_ticket.awl` (+ `ticket.schema.json`) — file import; absent-from-
  required property types as `T?`; `is present` flow-typing on a plain binding.
- `schema-doors/valid/import_nested_defs.awl` (+ `order.schema.json`) — nested objects, arrays,
  string enum compared with a string literal, `$defs`-local `$ref`, multi-level field access.
- `schema-doors/valid/import_constraints.awl` (+ `profile.schema.json`) — constraint keywords
  (minLength/maxLength/pattern/minimum/maximum/format) preserved but ignored for typing;
  string `+`.
- `schema-doors/valid/optional_shorthand.awl` — `?` shorthand with field `///` docs; payload
  construction both providing and omitting an optional field.
- `schema-doors/valid/mixed_doors.awl` (+ `intake.schema.json`) — all three doors in one
  document, inline schema with constraints, action config line, `count`, route-targeted step.
- `schema-doors/valid/short_circuit_optional.awl` — short-circuit flow-typing: `when detail
  is absent or detail.approved` — the left guard already decides, so the unsafe optional
  field read on the right is skipped.
- `schema-doors/valid/inline_verbatim_constraints.awl` — the paste-verbatim promise: inline
  schema with a negative bound, `1e-3`/`1E5` exponent literals, `\uXXXX` and `\/` string
  escapes, braces inside a `pattern` string, and three-space indentation — all raw-captured
  by the lexer, preserved byte-for-byte on re-emit (constraints ignored for typing).

Invalid:
- `import_oneof.awl` (+ `oneof.schema.json`) — unsupported keyword → CHECK "oneOf" @6 (span at
  the import decl; keyword + JSON path belong in the message).
- `import_anyof.awl` (+ `anyof.schema.json`) — CHECK "anyOf" @6.
- `import_patternprops.awl` (+ `patternprops.schema.json`) — CHECK "patternProperties" @6.
- `import_external_ref.awl` (+ `extref.schema.json`) — non-`$defs`-local `$ref` → CHECK "$ref"
  @6 (inferred: spec supports only $defs-local refs).
- `import_missing_file.awl` — import target absent on disk → CHECK "nowhere.schema.json" @6.
- `import_unparseable.awl` (+ `broken.schema.json`, deliberately truncated JSON) → CHECK
  "broken.schema.json" @6.
- `import_null_type.awl` (+ `nullable.schema.json`) — `"type": "null"` property → CHECK "null"
  @6 (inferred: no null in the language).
- `inline_null_type.awl` — same rule through the inline door → CHECK "null" @11 (span at the
  offending JSON line inside the block).
- `inline_oneof.awl` — unsupported-keyword rule applied to the inline door → CHECK "oneOf" @12
  (judgment: three doors, one type system).
- `inline_bad_json.awl` — malformed inline JSON → PARSE "wibble" @8 (diagnostic quotes the
  offending lexeme).
- `construct_null_payload.awl` — `body: null` in payload construction (D4: absence by omission)
  → CHECK "null" @17 (judgment: build plan assigns null-rejection to phase 4; flip to PARSE with
  a one-line sidecar edit if implementation rejects earlier).

## declarations — worker/action config, child, spawn, call contracts

Valid:
- `declarations/valid/worker_single_action.awl` — bare action, no config line.
- `declarations/valid/worker_action_config_lines.awl` — `node+timeout` and
  `node+timeout+retry N every D` config lines.
- `declarations/valid/worker_retry_backoff.awl` — `retry N backoff D..D` range form.
- `declarations/valid/workers_multiple.awl` — two worker blocks in one document.
- `declarations/valid/child_call_awaited.awl` — `child` decl + awaited call with binding.
- `declarations/valid/spawn_detached.awl` — `spawn` fire-and-forget of a Nil-returning child.
- `declarations/valid/call_site_override.awl` — call-site `node`/`timeout` override (SYNTAX
  INFERRED: config line indented under the call statement — spec grants the override but shows
  no concrete form; rewrite mechanically if the real grammar differs).
- `declarations/valid/declarations_combined.awl` — 4 actions covering all config shapes incl.
  node-only and retry-only lines, 2 children, awaited call, spawn, unbound side-effect calls,
  call-site override, route-to-step + route-to-outcome.
- `declarations/valid/action_run_bodies.awl` — two actions on one queue: one the document
  implements itself with a declared `run` command body, one it only requires of an out-of-band
  worker.
- `declarations/valid/advisory_side_channel.awl` — an `advisory` action: announce progress on
  a side channel, then do the work the run exists for (an advisory failure never faults the
  step).
- `declarations/valid/agent_seam.awl` — the `agent` marker: one `String` prompt in, the
  canonical agent-outcome record out (a declared record of `text: String` and
  `stop_reason: String`) — the whole of what an adapter needs to hand the call to a harness
  instead of running it as an ordinary activity.
- `declarations/valid/agent_seam_unmarked_control.awl` — the control arm: the same workflow,
  action shape, and call with the marker absent. `agent` is a declaration classification that
  rides the package contract, so the two must lower identically
  (`agent_marked_and_unmarked_lower_identically` compares their committed goldens).

Invalid:
- `positional_call_args.awl` — positional argument → PARSE "named" @13 (judgment: "unwritable"
  read as grammar-level; canonical fixture for the class, step-bodies duplicate removed).
- `missing_required_arg.awl` — required arg omitted → CHECK "channel" @13 (canonical; dup removed).
- `unknown_arg_name.awl` — undeclared arg name → CHECK "recipient" @13 (canonical; dup removed;
  may legitimately cascade a missing-arg secondary — substring pins the primary only).
- `duplicate_arg_name.awl` — same named arg twice → CHECK "duplicate" @13.
- `arg_type_mismatch.awl` — Int passed for String → CHECK "String" @13 (canonical; dup removed).
- `spawn_with_binding.awl` — `spawn … -> handle` → CHECK "spawn" @15 (the one stage the spec
  states explicitly).
- `spawn_of_action.awl` — spawn targets a worker action → CHECK "child" @14.
- `spawn_unknown_child.awl` — spawn of undeclared child → CHECK "audit_ghost" @13.
- `call_unknown_action.awl` — pipe into undeclared action → CHECK "frobnicate" @12.
- `child_inside_worker.awl` — child declared inside a worker block → PARSE "child" @11.
- `worker_without_actions.awl` — worker block with zero actions → PARSE "action" @8 (judgment:
  "one or more per worker" read as grammar).
- `retry_missing_schedule.awl` — `retry 2` without every/backoff → PARSE "every" @10.
- `retry_backoff_missing_range.awl` — backoff without `..max` → PARSE ".." @10.
- `timeout_missing_duration.awl` — bare `timeout` key → PARSE "duration" @10.
- `action_missing_return_type.awl` — no `-> Type` → PARSE "->" @9 (inferred mandatory: every
  spec example carries it and Nil exists for effect-only actions).
- `action_param_missing_type.awl` — untyped parameter → PARSE "parameter" @9.
- `call_site_retry_override.awl` — `retry` at a call site (only node/timeout may pin) → CHECK
  "retry" @15.
- `duplicate_action_name.awl` — same action twice on one worker → CHECK "duplicate" @10
  (inferred: "checked against the declaration" is ill-defined under duplicates).
- `action_unknown_return_type.awl` — return type never declared → CHECK "Ack" @10 (downstream
  untyped-binding cascades possible; substring pins the primary).
- `action_list_element_optional.awl` — `[String?]` action parameter (ruled 2026-07-11: `?`
  illegal in list-element position, signature surface) → CHECK "element" @9.
- `run_declared_twice.awl` — one action, two declared commands → PARSE
  "at most one `run` body" @11.
- `run_without_command.awl` — the `run` body word without a command line → PARSE
  "quoted command line" @10.
- `run_empty_command.awl` — a whitespace-only command names no program → CHECK
  "empty `run` command" @10.
- `run_parameterized_program.awl` — the program itself is a parameter, so a caller-supplied
  value would choose which program runs → CHECK "must be written out" @11.
- `run_unknown_parameter.awl` — the command interpolates a parameter the action never
  declares → CHECK "does not declare" @10.
- `run_shell_variable_unescaped.awl` — bare `$HOME` (inside `run` the dollar belongs to AWL;
  `$$HOME` reaches the command) → CHECK "is not a parameter name" @11.
- `run_result_not_a_record.awl` — a bodied action hands back the outcome OBJECT; a scalar
  result type can never be it → CHECK "must be a record over" @10.
- `run_result_unknown_member.awl` — the result type names a member the outcome never
  carries → CHECK "is not part of the outcome" @9.
- `run_result_member_wrong_type.awl` — `exit_code` declared String where the outcome carries
  Int → CHECK "is `Int`, but action" @9.
- `advisory_declared_twice.awl` — one config key, one spelling → PARSE
  "duplicate `advisory` key" @10.
- `advisory_binds_result.awl` — an advisory failure never faults the step, so it can promise
  no value to bind → CHECK "advisory" @13.
- `advisory_pipe_stage.awl` — a pipe stage hands its value on; an advisory stage would have
  none → CHECK "advisory" @13.
- `advisory_call_site_override.awl` — `advisory` classifies the action itself; a call site
  cannot declare it → CHECK "advisory" @14.
- `agent_takes_two_parameters.awl` — `agent` names a SHAPE: one `String` prompt in, the
  agent-outcome record out → CHECK "takes 2 parameters" @13.
- `agent_parameter_is_not_a_string.awl` — a structured input has no place to go on the wire
  to an agent → CHECK "`String` prompt handed to the harness" @13.
- `agent_returns_bare_string.awl` — an agent action returns the canonical agent-outcome
  record, never a bare `String` ("parse it downstream" flattened the stop reason) →
  CHECK "returns `String` — an agent action returns the canonical agent-outcome record" @12.
- `agent_returns_the_wrong_record.awl` — a record of the wrong fields is not the contract;
  the wire will never carry it →
  CHECK "returns `Answer` — an agent action returns the canonical agent-outcome record" @12.
- `agent_returns_wrong_field_types.awl` — right field names, wrong field types
  (`stop_reason: Int`); the structure is checked type by type →
  CHECK "returns `Reply` — an agent action returns the canonical agent-outcome record" @13.
- `agent_action_declares_a_body.awl` — agent + declared body is two implementations of one
  action → CHECK "also declares a body" @14. NOT an isolated refusal, and cannot be: no
  return type satisfies both the agent-record rule (`text` + `stop_reason`) and the bodied
  action's rule (a record over `exit_code`/`stdout`/`stderr`) at once, so this fixture
  necessarily emits the two run-body member diagnostics alongside the body refusal it pins
  (3 diagnostics total); the sidecar pins the body refusal specifically.
- `agent_call_site_override.awl` — `agent` classifies the action itself; a call site cannot
  declare it → CHECK "`agent` stays on the" @14.

## step-bodies — calls, bindings, pipes, combinators, wait/sleep, literals

Valid:
- `step-bodies/valid/calls_and_side_effects.awl` — call with `->` binding, unbound side-effect
  call, zero-stage `binding |> route outcome` terminator.
- `step-bodies/valid/pipe_chain_stages.awl` — `value |> action |> .field -> name` chain split
  across steps.
- `step-bodies/valid/combinators.awl` — `filter`/`sort`/`map`/`count` with `.field` accessors,
  `is empty` guard.
- `step-bodies/valid/predicates_and_operators.awl` — all six comparisons, `not`/`and`/`or`,
  `is present`/`is absent` on `String?`.
- `step-bodies/valid/general_concat.awl` — direct-route payload with left-associated nested
  string `+`: an input field, the dev-brief worktree literal, and `workflow.id` (the flagship
  provision-path shape).
- `step-bodies/valid/index_and_concat.awl` — literal-only indexing `xs[0]`, string `+`.
- `step-bodies/valid/wait_and_sleep.awl` — `wait signal -> name` (no timeout), `sleep 30s`.
- `step-bodies/valid/wait_timeout_optional.awl` — `wait … timeout 2d -> name` binding `T?`,
  `is present` guarded arm using the narrowed value (only fixture with the `d` duration unit).
- `step-bodies/valid/literal_forms.awl` — list literal `["a", "b"]`, float literal `0.5`,
  `true`, escaped string (`\"` `\n` `\t` `\\`), `///` doc on an action and on a step.
- `step-bodies/valid/send_signal.awl` — the signal SENDER: header `send <name>: <Type>`
  declarations (record, leaf, and `Nil` payloads) and all three `send … to <expr>` payload
  forms, addressed by an input-carried coordinator id and by `workflow.id`; sits alongside a
  `signal`/`wait` receiver in the same document.
- `step-bodies/valid/step_bodies_combined.awl` — realistic multi-construct: pipe chains,
  combinators, indexing, `+`, sleep, wait+timeout, fall-through + route-targeted steps.
- `step-bodies/valid/collection_predicates.awl` — `any`/`all` collection predicates evaluated
  entirely in the workflow VM, including over the empty list literal `[]`.
- `step-bodies/valid/fallible_collection_predicates.awl` — `workflow.id` resolved inside
  existential and universal predicates.
- `step-bodies/valid/fallible_any_short_circuit.awl` — a fallible existential predicate stops
  after its first matching item.
- `step-bodies/valid/fallible_all_short_circuit.awl` — a fallible universal predicate stops
  after its first non-matching item.
- `step-bodies/valid/workflow_id.awl` — the replay-stable `workflow.id` attached to the
  terminal record.
- `step-bodies/valid/guard_narrows_an_optional_field.awl` — #97-B: `is present` narrows an
  optional FIELD by its access path, not just a whole binding (the record itself is not
  optional).
- `step-bodies/valid/guard_narrows_a_deep_optional_field.awl` — the same rule two records
  deep (`outer.inner.end`).
- `step-bodies/valid/guard_chain_carries_every_proof.awl` — a guard chain carries EVERY proof
  it makes, not just the first (`a is present and b is present and b` narrows through both
  layers).
- `step-bodies/valid/query_answer.awl` — the read-only surface: header `query` declarations,
  all three `answer` forms, an answer re-registered inside a loop and one inside an
  `on failure` block.

Invalid:
- `wait_missing_binding.awl` — `wait go` without `-> name` → PARSE "->" @15 (span on the wait
  statement line per source-correct-span discipline).
- `sleep_non_duration.awl` — `sleep 5` → PARSE "duration" @13 (judgment: grammar shows
  `sleep <duration>`).
- `computed_index.awl` — `responders[lead]` → PARSE "index" @14 (spec: literal-only, read as
  grammar).
- `equals_statement_binder.awl` — gone `=` statement binder → PARSE "=" @15.
- `pipe_missing_stage.awl` — `x |> -> name` → PARSE "|>" @15.
- `binding_missing_name.awl` — `->` with no name → PARSE "->" @15.
- `bare_field_statement.awl` — `.field` as a statement → PARSE ".greeting" @16.
- `pipe_chain_unterminated.awl` — chain without `-> name`/route → PARSE "pipe" @17 (judgment:
  "terminate with" read as grammar).
- `unknown_action.awl` — direct call of undeclared action → CHECK "polish" @16 (kept alongside
  declarations' pipe-call variant: different call surface).
- `unknown_binding.awl` — read of a never-bound name → CHECK "greeting" @18.
- `rebound_binding.awl` — single-assignment violation → CHECK "greeting" @16.
- `pipe_multi_arg_action.awl` — pipe stage action taking two args → CHECK "provision" @13.
- `unknown_field_access.awl` — `.volume` on a type without it → CHECK "volume" @19.
- `filter_non_bool.awl` — filter on a String accessor → CHECK "title" @17.
- `map_unknown_field.awl` — map projecting a missing field → CHECK "nope" @17.
- `count_with_argument.awl` — `count(.title)` → CHECK "count" @17 (judgment: arity is semantic).
- `is_present_non_optional.awl` — `is present` on plain String → CHECK "is present" @18.
- `is_empty_non_list.awl` — `is empty` on Int → CHECK "is empty" @19 (one fixture pins the
  rule; `is absent` shares it).
- `comparison_type_mismatch.awl` — Int < String → CHECK "<" @18.
- `arithmetic_plus.awl` — Int + Int (no arithmetic; `+` is string-only) → CHECK "+" @18.
- `and_non_bool.awl` — CHECK "and" @18.
- `not_non_bool.awl` — CHECK "not" @18.
- `wait_unknown_signal.awl` — wait on undeclared signal → CHECK "approval" @13.
- `pipe_route_unknown_outcome.awl` — pipe-route to unknown target → CHECK "finished" @19 (kept
  alongside loop-outcomes' outcome-clause variant: different surface).
- `pipe_route_payload_mismatch.awl` — piped Greeting into an outcome carrying Shouted → CHECK
  "Greeting" @15.
- `pipe_stage_type_mismatch.awl` — Int piped into a String-taking action → CHECK "Int" @18.
- `index_non_list.awl` — `topic[0]` on String → CHECK "index" @13.
- `any_non_list.awl` — a collection predicate over a non-list value → CHECK
  "needs a List collection" @9.
- `all_non_bool.awl` — a predicate item expression that is not Bool → CHECK
  "predicate must be Bool" @10.
- `bare_workflow.awl` — the `workflow` namespace used without a field → CHECK
  "builtin namespace, not a value" @8.
- `unknown_workflow_field.awl` — a `workflow.*` field the language does not define → CHECK
  "unknown workflow builtin" @8.
- `narrowing_does_not_escape_its_arm.awl` — a field-path proof belongs to the arm that
  established it, and to nothing else → CHECK "found Bool?" @27.
- `proof_does_not_flow_backwards.awl` — a guard chain carries its proofs FORWARD only →
  CHECK "found Bool?" @29.
- `query_duplicate_declaration.awl` — two declarations of one query name → CHECK
  "duplicate query declaration `review_status`" @6.
- `query_declared_never_answered.awl` — a query declared and never answered → CHECK
  "is declared but never answered" @6.
- `query_answer_undeclared.awl` — an answer whose query the header never declares → CHECK
  "no query named `review_status` is declared in the workflow header" @11.
- `query_answer_field_mismatch.awl` — an answer whose field set is not the declared answer
  type's → CHECK "has no field `stage`" @12.
- `query_answer_unbound.awl` — an answer reading a binding not in scope at the statement →
  CHECK "assessment" @16.

## dag-fork — `after` graphs, fork/join

Valid:
- `dag-fork/valid/after_single.awl` — one explicit `after` edge.
- `dag-fork/valid/after_multi_diamond.awl` — diamond: two parallel steps sharing `after gather`,
  `after claims, citations` join.
- `dag-fork/valid/fall_through_chain.awl` — pure fall-through, no after/route until final pipe.
- `dag-fork/valid/fork_action_fanout.awl` — parallel action collection fork
  (the doc-certification shape): one unbound action call with a captured free
  name, `join -> name` feeding the route payload directly (no combinators —
  the BC-2b-4 direct-lowering flagship, BEAM-golden'd).
- `dag-fork/valid/fork_collection_join.awl` — `fork x in xs … join -> name`,
  count/filter/is-empty discrimination.
- `dag-fork/valid/fork_named_homogeneous.awl` — bare `fork`, two branches of
  the SAME action → one typed `workflow.all`, source-order destructure.
- `dag-fork/valid/fork_sequential_route.awl` — `sequential` collection fork
  whose joined list routes directly (no combinators — direct-lowerable).
- `dag-fork/valid/fork_named_branches.awl` — bare `fork` heterogeneous branches with per-branch
  bindings, bare `join` (no `->`), branch bindings consumed downstream.
- `dag-fork/valid/fork_sequential.awl` — `fork x in xs sequential … join -> name`.
- `dag-fork/valid/child_collection_fork.awl` — child-per-item collection fan-out regression:
  `fork spec in sittings` calling the `sit_one` child per item, `join -> rows` routed out.
- `dag-fork/valid/child_collection_fork_sequential.awl` — the sequential arm of the same
  regression: `fork value in values sequential` over a child call.
- `dag-fork/valid/sit_one.awl` — the real child half of the cross-module collection-fork
  contract proof: the workflow `child_collection_fork` names as its `sit_one` child.
- `dag-fork/valid/release_pipeline_combined.awl` — fall-through root, diamond, collection fork,
  sequential fork, route-targeted step, per-action node/timeout/retry config.

Invalid (all CHECK — the family's required errors are semantic):
- `unknown_after_target.awl` — CHECK "missing_step" @16 (span on the step header).
- `unknown_after_second_target.awl` — unknown name in SECOND `after` position → CHECK
  "phantom" @16 (span-discipline case).
- `after_self_cycle.awl` — `step settle after settle` → CHECK "cycle" @11.
- `after_cycle_pair.awl` — mutual after edges → CHECK "cycle" @11 (first-written participant;
  fixture asserts the cycle diagnosis wins over any unreachable-step diagnosis).
- `route_cycle_self_unbounded.awl` — otherwise arm routes to its own step, no bound → CHECK
  "bound" @17 (span on the routing outcome line).
- `route_cycle_two_steps_unbounded.awl` — pure route-route cycle, no bound → CHECK "bound" @24
  (backward-edge route line).
- `unreachable_step.awl` — step no route targets, below a step that always pipe-routes →
  CHECK "orphan" @15.
- `fork_non_list_collection.awl` — `fork item in doc` over a bare record →
  CHECK "needs a list" @13.

Note: dag-fork route-cycle sidecars pin "bound", loop-outcomes' mixed-cycle pins "cycle" — the
diagnostic wording must carry both (e.g. "route cycle without a bound").

## loop-outcomes — loop/counting, outcome arms, routing, on failure, substeps

Valid:
- REV-3 EDITS: `backward_route_bounded_cycle.awl` carries `max max_polish_rounds visits` on
  `check` (the input-derived-bound witness) and `ship_release_combined.awl` carries
  `max 3 visits` on `canary` — both required by the rev-3 cycle rule. The bounds regressed
  both fixtures covered → refused when rev 3 landed; the bound lowers now, and both are back
  in the covered ratchet (`src/mir/covered.rs`) and emit today.
- `loop-outcomes/valid/loop_counting_until_max.awl` — `loop x = seed counting n … until … max`.
- `loop-outcomes/valid/loop_without_counting.awl` — bounded `loop x = seed … until … max`
  without a public counter (pins the scalar loop result and no-destructure call site).
- `loop-outcomes/valid/loop_compound_until_nested.awl` — nested bounded loops with `and` and
  optional-narrowing `or` compound `until` conditions (pins short-circuit ordering and nested
  loop-slot planning).
- `loop-outcomes/valid/fork_in_loop_live_ins.awl` — two-step fall-through into a loop containing
  a collection fork; its `join -> verdicts` binding feeds a later body action and the `until`
  condition while remaining loop-local (pins direct chain-boundary live-in collection without
  threading the join binding out of the loop).
- `loop-outcomes/valid/backward_route_bounded_cycle.awl` — backward route legal because a
  max-bounded loop sits on a step in the cycle; counter used in a later step's payload.
- `loop-outcomes/valid/loop_after_fall_through.awl` — a fall-through chain whose terminal
  step loops on the first step's binding (pins the interior chain boundary carrying the
  loop's seed and ceiling names).
- `loop-outcomes/valid/route_outcome_by_name.awl` — bare `route <outcome>` picks up the
  in-scope binding NAMED like the outcome (interpretation: by-name, not by-type; the negative
  is `bare_route_no_binding`).
- `loop-outcomes/valid/float_threshold_guard.awl` — grade an essay and gate the verdict on a
  Float threshold comparison in a `when` guard.
- `loop-outcomes/valid/enum_when_totality.awl` — enum-subject `when x.category == Variant`
  arms, total without `otherwise` (pins bare-TitleCase variant comparison syntax for declared
  enums; imported string enums compare against string literals, see import_nested_defs).
- `loop-outcomes/valid/guard_optional_wait.awl` — wait+timeout `T?` used as `T` inside the
  `is present` arm (kept alongside wait_timeout_optional: Nil-returning config'd action, 30m).
- `loop-outcomes/valid/on_failure_compensation.awl` — `on failure` calls then
  route-to-workflow-outcome.
- `loop-outcomes/valid/substeps_two_stage.awl` — substep grammar: inner route to a sibling
  substep and to a parent outcome clause (interpretation: routing to the parent's outcome
  NAME fires that outcome's route; its payload uses literals only).
- `loop-outcomes/valid/ship_release_combined.awl` — loop+counting, wait guard, backward route,
  `on failure` ending route-to-step, counter reused downstream.

Invalid:
- `loop_missing_seed.awl` — loop without `= seed` → PARSE "=" @18.
- `counting_missing_name.awl` — `counting` without a name → PARSE "counting" @18.
- `counting_shadows_loop_binding.awl` — public counter reuses the threaded binding name →
  CHECK "counting" @13 (the values have distinct types and Gleam forbids a duplicate tuple binder).
- `outcome_missing_route.awl` — `when` arm with no route → PARSE "route" @23.
- `loop_no_max.awl` — unbounded loop → CHECK "unbounded" @15 (judgment: legality rule, not
  grammar; reclassify PARSE if `max` becomes grammar-mandatory).
- `loop_no_rebind.awl` — body never rebinds the threaded value → CHECK "rebind" @18 (anchored
  on the loop line).
- `loop_exhaustion_uncovered.awl` — ceiling case uncovered → CHECK "exhaust" @15 (anchored on
  the step header, match-expression convention).
- `when_without_otherwise.awl` — lone when, non-exhaustive → CHECK "exhaust" @13.
- `enum_variant_uncovered.awl` — Spam lane missing → CHECK "Spam" @15 (diagnostic must NAME
  the missing variant).
- `otherwise_not_last.awl` — CHECK "otherwise" @23 (anchored on the misplaced otherwise).
- `route_unknown_target.awl` — outcome-clause route to unknown target → CHECK "report" @23.
- `route_cycle_unbounded.awl` — mixed fall-through+route cycle, no bound → CHECK "cycle" @24.
- `on_failure_no_route.awl` — compensation without terminal route → CHECK "route" @26 (span on
  the `on failure` line).
- `optional_used_unguarded.awl` — `T?` read in an unguarded otherwise arm → CHECK "reply" @22
  (canonical for the flow-typing rule; step-bodies duplicate removed).
- `payload_missing_field.awl` — payload omits a required field → CHECK "polls" @23 (canonical;
  schema-doors duplicate removed).
- `bare_route_no_binding.awl` — bare `route summary` with no binding named `summary` in scope
  → CHECK "summary" @18 (flips to valid if the pickup rule is by-type).
- `final_step_falls_off.awl` — final step never routes → CHECK "route" @13 (span on the step
  header).
- `substep_route_escape.awl` — inner route targets a step outside the parent → CHECK
  "parent" @24.
- `binding_not_on_all_paths.awl` — step reachable by two routes reads a binding made on only
  one of them (spec: "the checker rejects reads of bindings not guaranteed on every path into
  a step") → CHECK "trimmed" @31.
- `loop_step_no_outcomes.awl` — loop-carrying step with ZERO outcome clauses (ruled 2026-07-11:
  exhaustion must be explicitly named; the strong reading) → CHECK "exhaust" @16 (anchored on
  the loop line — the sibling of `loop_exhaustion_uncovered`'s step-header anchor).
- `route_in_loop_body.awl` — `route` statement inside a loop body (ruled 2026-07-11: loops exit
  via `until`/`max`; routing is the step's outcomes' job) → CHECK "`loop` body" @17 (span on
  the route).
- `pipe_route_in_loop_body.awl` — pipe-chain `|> route` terminator inside a loop body (same
  ruling, pipe surface) → CHECK "`loop` body" @17 (span on the route target).

## Corpus-wide conventions and recorded judgments

- Every valid fixture is a complete workflow: `//!` narration, header with >=1 outcome, worker
  actions for every call, reachable terminal routes; <=100 columns; two-space indents;
  newline-terminated; no tabs.
- Every invalid fixture carries exactly one defect. Substrings are offender identifiers,
  keywords, or operators where possible (wording-agnostic); a few force phrasing ("named",
  "duration", "pipe", "unbounded", "exhaust", "parent") and are contract-setting.
- Duplicate-declaration spans anchor at the second occurrence. Imported-schema errors anchor at
  the `type X = schema("…")` declaration line; inline-schema errors at the offending JSON line.
- Stage assignments where the spec names the error but not the stage are recorded per-fixture
  above; sidecar line-1 values ARE the family ruling to revisit, not settled spec text.
- Unused declarations (types/signals/inputs/actions/bindings) are treated as legal — the spec's
  checker duty list has no unused-decl rule. Unreachable workflow OUTCOMES are not an error
  (the checker list covers unreachable steps only).
- Deliberately NOT fixtured (spec-silent; no grammar invented): enum totality via anything but
  `== Variant`; literal arguments to `filter`/`sort`/`map`; `sort` key comparability;
  parenthesized boolean grouping; (two shapes once on this list are now fixtured: compound
  guards — `when x is present and …` — by the #97-B narrowing family, and the empty list
  literal `[]` by `collection_predicates`);
  `T??`; trailing `|` in enums; zero-arg actions; declaration-order violations beyond the two
  covered; `join -> name` after named branches / bare `join` after collection forks;
  `required` naming an absent property; top-level non-object imported schema; explicit null in
  input documents (start-time, not statically expressible); out-of-range literal index
  (a runtime step failure per spec); config-key ordering/duplication on config lines;
  unconditional bare `outcome name:` clauses (grammar shows only when/otherwise);
  gone-list keywords beyond the four representatives (`about`, `Option`, `List`, `=`-binder,
  plus `finish`) — one class, five witnesses.
- Type-brace column alignment — RULED at the parser/printer phase (2026-07-10): group
  alignment IS canonical. Within a maximal run of adjacent single-line type declarations of
  the same form (`{ … }` bodies, or `= …` doors/enums; runs break on blank lines, comments,
  doc lines, or a multi-line declaration), names pad to a common column; workflow-header
  `outcome` runs align their `type` and `route` columns the same way. Alignment padding is
  exempt from the 100-column rule (the single-line decision is made unpadded). All valid
  fixtures were mechanically re-normalized through the printer to match; the flagship pair
  needed (and received) no edits — `print(parse(dev_brief.awl))` is byte-identical.
  (noncanonical_commas remains excluded from byte-identity goldens.)
- Checker-phase rulings (2026-07-11), recorded where the sidecars left the stage or rule open:
  every CHECK-staged sidecar passed at its recorded stage — no re-staging was needed (the
  `construct_null_payload` judgment note resolves to CHECK: `null` lexes as an identifier and
  the checker refuses the reference everywhere with a targeted absence-is-omission diagnostic).
  Bare `route <outcome>` pickup is BY NAME, as `bare_route_no_binding` anticipated. Optional
  (`?`) parameters may be omitted at call sites, mirroring record construction (spec-silent;
  one rule for both argument surfaces). Type compatibility is structural for record shapes so
  a schema-projected record satisfies a declared record with the same fields (dev_brief's
  `config.lenses` items vs the declared `Lens`); display names stay nominal in diagnostics.
  Route-cycle boundedness — SUPERSEDED at flow-vocabulary rev 3 (B2): a cycle is legal iff
  a member step carries a `max … visits` re-entry bound; a `max`-bounded `loop` inside a
  member NO LONGER satisfies the rule (the loop bounds its own iteration, not the step's
  re-entry — the decoy-loop soundness gap, closed by design). `backward_route_bounded_cycle`
  and `ship_release_combined` gained visits bounds to stay valid; the decoy shape is pinned
  invalid at `flow-shape/invalid/decoy_loop_cycle.awl`. The pre-rev-3 rule, for the record,
  accepted a `max`-bounded loop on any step of the control-flow SCC
  (route edges + fall-through edges + `after` edges — CORRECTED at the checker fix round,
  2026-07-11: a dependency's completion re-arms its dependents, so a backward route plus a
  forward `after` edge is as unbounded a cycle as two routes; the earlier narrower SCC
  contradicted the spec's "unbounded cycles are unwritable"); the diagnostic anchors on the
  first backward or self route edge (falling back to any in-cycle route edge, then the
  earliest member's header) and its wording carries both pinned substrings ("cycle",
  "bound"). The route-cycle/exhaustiveness/`unbounded` trio all anchor exactly where the
  sidecars pinned: backward-edge route line / step header / loop header line respectively.
- Outcome-clause layout — RULED at the same phase: a payload-constructing route
  (`route out(field: …)`) ALWAYS breaks after the guard comma onto its own line one level
  deeper; a bare route stays on the guard line when the clause fits 100 columns. The spec's
  printer-contract prose ("payload construction breaks … when over 100 columns") and its
  worked examples (which break 90- and 99-column payload clauses) disagree; byte-identity
  with the flagship pins the examples' reading. Valid fixtures were re-normalized to match
  (splits/joins of outcome clauses only — audited via whitespace-insensitive diff).
- Checker fix-round rulings (2026-07-11 adversarial panel; regression suites
  `checker_regressions.rs` + `checker_hardening.rs` pin each one): named-branch fork branches
  walk in isolated clones of the pre-fork scope and merge bindings at `join` (a sibling's
  binding is unreadable mid-fork); `join -> name` on the named form is refused (spec shows
  bindless `join`). Every non-terminal falling-through step must have its completion consumed
  (the next step's fall-through edge or an `after` dependent) — the successor duty is no
  longer final-step-only. A piped route must target a workflow outcome; step, sibling, and
  parent-arm targets are refused (silent value loss). Steps and workflow outcomes share one
  route-target namespace (collision is a declaration-time error anchored at the step). A bind
  inside a collection-fork branch is NOT a loop rebind (branch bindings never escape).
  Inline schema-door diagnostics anchor by walking the raw JSON to the failing path, never by
  first token occurrence. Structural compatibility carries no acceptance depth cap
  (coinductive in-progress pair set; recursive types still terminate). Dead control flow is
  refused: statements behind an unconditional body route and outcome clauses behind a
  body-terminal route. Call-site config on CHILD calls is refused (the engine routes
  children, not a queue).
- Ratified rulings (Tom, 2026-07-11) — the two items previously OPEN here are RESOLVED, plus a
  third moved up from the emitter stopgap:
  - R1, loop exhaustion must be explicitly named: a step whose body contains a `loop` (fork
    branches included; a substep answers for its own clauses) MUST declare conditional outcome
    clauses covering the exhausted case — a loop-carrying step with zero outcome clauses is a
    CHECK error anchored on the loop (`loop_step_no_outcomes`). The permissive fall-through
    reading is dead.
  - R2, `?` is illegal in list-element position: `[T?]` is refused at CHECK in every type
    position (shorthand fields, inputs/signals/outcomes, action/child signatures — one rule at
    type-reference resolution), `[T]?` stays legal, and the imported-schema projection cannot
    manufacture the shape (only object properties wrap in `?`; a null-admitting `items` type
    is already refused as a null union). Fixtures: `list_element_optional`,
    `action_list_element_optional`; the remaining positions are pinned by
    `tests/checker_rulings.rs`.
  - R3, `route` is illegal inside a `loop` body: statement and pipe-chain `|> route`
    terminator alike, refused at CHECK with the span on the route (`route_in_loop_body`,
    `pipe_route_in_loop_body`). The emitter's emit-time refusal survives only as a defensive
    backstop for unchecked documents. No sidecar re-staging was needed: every sidecar stage
    was already PARSE/CHECK — the old route-in-loop refusal was pinned by an emitter unit
    test, now reframed as the backstop pin.

## ergonomics — flow-vocabulary B1: consts, raw strings, json literals, schema of

Valid:
- `ergonomics/valid/flow_vocab_b1.awl` — the B1 compile proof: multi-line raw string const,
  `json { … }` const (brace-in-string description), `schema of Verdict`, const-over-const `+`
  concatenation, consts consumed as call arguments, and an expression-headed statement
  (`"run: " + result + …` as a pipe head). Byte-canonical; emits and `gleam build`s
  (`tests/flow_vocab_compile_proof.rs`); MIR-covered with goldens.

Invalid (all CHECK; the unterminated-raw-string lex refusal is pinned by
`tests/flow_vocab_ergonomics.rs` because no corpus fixture may die in the lexer):
- `const_duplicate.awl` — second `const prompt` → CHECK "duplicate const declaration" @7.
- `const_cycle.awl` — mutually recursive consts → CHECK "defined in terms of itself" @7.
- `const_unknown_ref.awl` — const value names no const → CHECK "unknown const" @6.
- `const_not_compile_time.awl` — const value reads a workflow input → CHECK @6.
- `const_shadowed_binding.awl` — step binding reuses a const name → CHECK @12.
- `json_invalid_body.awl` — `json { … }` body is not JSON → CHECK "not valid JSON" @8, span
  INSIDE the body.
- `schema_of_unknown_type.awl` — `schema of Missing` → CHECK "unknown type" @6.

## worker-documents — the `worker` file family (#30)

A `.awl` file whose opening declaration is `worker <queue>` rather than
`workflow <name>` is an IMPLEMENTATION of one task queue: its actions each
carry the body that runs them, and it declares no flow. Both families parse
through one grammar, so a workflow-shaped declaration in a worker document is
a well-formed parse the CHECKER refuses by name — which is why almost every
fixture below is CHECK-staged.

Valid (parse, check clean, print byte-canonically; `emit` refuses these by
family and `tests/emitter.rs` asserts that refusal positively):
- `worker-documents/valid/greeter.awl` — the three capture forms in one file:
  `run json` (stdout decoded into a declared type), `run text` (trimmed stdout
  as one string), and the bare `run` (the command's outcome record, subset
  return type).
- `worker-documents/valid/reporter.awl` — the trivia a formatter must
  preserve across the new file header: a comment between the narration and the
  `worker` line, a same-line comment ON that line (which lives on the header
  block, not on the document), a `///` doc line on an action, a
  comment-and-blank lead inside the block, and a zero-parameter action.

Invalid (stage / substring / span line):
- `step_in_worker_document.awl` — a `step` in a worker document → CHECK
  "implements a queue's actions and runs no flow" @6.
- `child_in_worker_document.awl` — a `child` declaration → CHECK "a worker
  document runs no flow" @6.
- `const_in_worker_document.awl` — a `const` → CHECK "a worker document has no
  expressions" @6.
- `second_worker_block.awl` — a second `worker` block → CHECK "serves exactly
  one queue" @6.
- `bodyless_action.awl` — an action with no body → CHECK "declares no body" @5.
- `delivery_config.awl` — `timeout`/`retry` on the implementation → CHECK
  "belongs to the action's declaration in the `workflow` document" @4.
- `text_capture_wrong_type.awl` — `run text` returning a record → CHECK "its
  result type must be `String`" @3.
- `json_capture_nil.awl` — `run json` returning `Nil` → CHECK "there is nothing
  for the decoded stdout to become" @3.
- `capture_in_workflow_document.awl` — the family rule in the other direction:
  a `run json` body in a WORKFLOW document, whose bodies the server executes →
  CHECK "declared bodies are executed by the server" @10.
- `header_decl_in_worker_block.awl` — `input` inside a worker block; the one
  PARSE-staged refusal in the family, because a header declaration cannot open
  an action → PARSE "cannot stand inside a worker block" @3.

## harness-section — the `harness` block inside a `worker` block

A `worker` block may declare the LAUNCH of the agent worker that serves its
queue: which harness, how many activities at once, how it reconnects, which
environment variables cross into the agent subprocess, and the per-kind launch
settings. Every setting is the operator's; nothing anywhere invents one, and a
section missing a field of ITS DECLARED KIND's required set is refused naming
the field. The section is launch configuration and not flow meaning, which the
MIR ratchet pins: the three section-carrying fixtures below and the absent
control lower to the same instructions, their goldens differing only where the
documents differ in their own names (module, input record, outcome union).

Valid (parse, check clean, print byte-canonically; the printer emits the fixed
field order, so these files are already canonical):
- `harness-section/valid/acp_harness.awl` — a complete `acp` launch. Carries no
  Norn setting and is COMPLETE, which is the kind-scoping ruling in one file.
- `harness-section/valid/norn_harness.awl` — a complete `norn` launch, with the
  two optional settings (`args`, `binary`) present. Carries no `command`,
  `permission` or `exit_grace` and is COMPLETE. It DOES carry `cwd`: that
  setting belongs to neither protocol in particular — the directory an agent
  stands in decides which tree it reads and edits — so both kinds are held to
  it, and only the kind-SPECIFIC settings stay scoped.
- `harness-section/valid/norn_harness_without_optional_settings.awl` — the same
  launch with both optional settings omitted: an optional setting left out is a
  declaration, not a gap, and the adapter's own `norn`-on-`PATH` default is the
  library's value rather than one invented at the boundary.
- `harness-section/valid/harness_absent_control.awl` — the norn document with
  its whole section removed. It is what turns "the section does not change the
  lowering" from a claim about documents that all carry one into a comparison
  against a document that does not.
- `harness-section/valid/norn_harness_with_a_bound_cwd.awl` — the second `cwd`
  form: `cwd from repo`, where the directory arrives WITH the job rather than
  being written into a document authored before any job exists. Its agent action
  declares `repo` beside the prompt, which is the shape the marker rule admits
  under a binding harness (prompt plus exactly one more `String`, the bound
  directory). A different document from the four above, so it is not part of
  their comparison; its own golden pins where the directory travels — the
  activity input object carries `repo` and `prompt` by name, so the worker reads
  the tree out of the call exactly as it reads the prompt.

Invalid (stage / substring / span line) — one per required field of each kind,
then the value and kind-scoping refusals. Every missing-field refusal anchors on
the `harness` line, because the section is what omitted the setting:
- `missing_kind.awl`, `missing_concurrency.awl`,
  `missing_reconnect_initial_backoff.awl`, `missing_reconnect_max_backoff.awl`,
  `missing_reconnect_max_attempts.awl`, `missing_env_pass.awl` → CHECK
  "declares no `<field>`", anchored on the `harness` line — the
  kind-independent required set.
- `missing_acp_command.awl`, `missing_acp_cwd.awl`,
  `missing_acp_permission.awl`, `missing_acp_exit_grace.awl` → CHECK
  "declares no `<field>`", anchored on the `harness` line — the `acp` kind's
  own required set. There is no
  `norn` counterpart for any of them, which is the point.
- `missing_kind_on_a_norn_shape.awl`, `missing_env_pass_on_norn.awl`,
  `missing_concurrency_on_norn.awl` → the kind-independent set again on a Norn
  section, so the rule is witnessed on both kinds rather than inferred from one.
- `missing_cwd_on_norn.awl` → CHECK "declares no `cwd`", anchored on the
  `harness` line. `cwd` is required of BOTH kinds — see `missing_acp_cwd.awl`
  for the other one — because the only default available is whichever directory
  the worker process happened to be started in, which is the invisible scoping
  the setting exists to end.
- `relative_norn_cwd.awl` → CHECK "must be an ABSOLUTE path", anchored on the
  value. The `{workspace_root}` form did not widen this: a value carrying no
  placeholder has nothing to expand and must stand absolute on its own, so the
  old refusal is witnessed AFTER the new accept-form exists rather than assumed
  to have survived it.
- `norn_cwd_with_a_misplaced_workspace_root.awl` → CHECK "somewhere other than
  its start". The placeholder buys a shipped document one shape — the box's root
  at the front — because no root any box resolves can make a value absolute by
  landing in the middle of it. Accepting it would mean checker-green,
  spawn-red.
- `bound_cwd_naming_no_parameter.awl` → CHECK "declares no such parameter" @26;
  `bound_cwd_on_a_parameter_that_is_not_a_string.awl` → CHECK "A working
  directory is a path, so the parameter is a `String`" @25. The pair is what
  makes `cwd from <parameter>` a checked declaration rather than a hope: the
  binding is settled statically against EVERY action the harness actually serves
  — presence first, then type — and both refusals anchor on the `cwd` line,
  because the binding is what named a parameter the action does not carry. An
  unchecked binding fails at the launcher with no directory at all, which is the
  same silent inheritance a missing `cwd` produces, wearing the clothes of a
  declaration.
- `norn_section_carrying_an_acp_command.awl` → CHECK "`command` configures the
  `acp` harness"; `acp_section_carrying_a_norn_binary.awl` → CHECK "`binary`
  configures the `norn` harness". Both directions, because a setting that
  silently does nothing is how an operator comes to believe a policy is in
  force when it is not.
- `unknown_kind.awl` → CHECK "names no agent harness"; `unknown_permission.awl`
  → CHECK "names no permission policy"; `relative_acp_command.awl` → CHECK
  "must be an ABSOLUTE path"; `zero_concurrency.awl` → CHECK "must be greater
  than zero"; `reconnect_cap_below_the_first_wait.awl` → CHECK "is shorter
  than".
- `env_pass_carrying_a_value.awl` → CHECK "takes variable NAMES": the list says
  WHICH variables cross into the child, and writing a value into a document
  puts a secret in a file that gets committed, diffed and deployed.
- `harness_with_nothing_to_serve.awl` → CHECK "nothing on this queue for an
  agent harness to serve": every action carries a declared body, so the launch
  would start a subprocess no work is ever sent to.

## flow-shape — flow-vocabulary B2: subflow, distribute/sequence, collect, visits

Rev-3 surface (`AWL-FLOW-VOCABULARY.md` rev 3 §1–§3, §5–§6). B4 landed: these fixtures parse,
print canonically, CHECK clean, and LOWER on both backends — every one is pinned in the
covered ratchet (`src/mir/covered.rs`, asserted refusal-free over the whole valid population
by `lowers_cover_and_golden`) and emits through the reference path
(`every_valid_fixture_emits`). The honest "not yet lowered" refusal survives only for shapes
no corpus fixture carries: `after` dependencies on substeps (`src/unlowered.rs`) and a
`max … visits` bound on a substep (the MIR backend's deliberate refusal — the reference
would emit an unbounded self-recursion).

Valid:
- `flow-shape/valid/dev_flow.awl` — the rev-3 §6 worked example, adapted to checkable AWL
  (real types/worker/args for the design's ellipses): subflow `dev_item` with an in-subflow
  bounded review loop-back (`max 3 visits`, value route payload `route out(verdict)`),
  `distribute` region (`wave after plan` — the opener is route-targeted by `fold`, so it arms
  via `after`), subflow-call step, tolerant `collect verdict? -> results` (`[ItemVerdict?]`),
  and the `fold` step rebinding `state` on the visits-bounded wave cycle (the sanctioned
  cycle-threaded rebinding). Byte-canonical.
- `flow-shape/valid/sequence_region_loopback.awl` — `sequence` region with an in-region
  loop-back (`visits` read in a member's guard), strict `collect` with a further statement
  after it, and a pure decision step (`judge`).
- `flow-shape/valid/distribute_child_collect.awl` — parallel fan-out over a single
  child-workflow track: spawn all, await each in item order, strict `collect` (any terminal
  child failure fails the run).
- `flow-shape/valid/distribute_child_tolerant.awl` — the tolerant child arm: a failed
  instance's slot is absent (spawn refusals included), siblings keep their results.
- `flow-shape/valid/distribute_activity_tolerant.awl` — tolerant parallel fan-out over a
  single-activity track: the settled combinator gathers one slot per item, absent where an
  instance failed.
- `flow-shape/valid/sequence_activity_tolerant.awl` — tolerant in-order fan-out: one instance
  at a time in collection order, a failed instance's slot absent.
- `flow-shape/valid/region_close_bounded.awl` — a region close carrying a `max … visits`
  bound: the counter keeps its identity through region synthesis, and the close's literal
  payload lowers after it.
- `flow-shape/valid/region_pure_decision.awl` — a pure-decision member routing to the close
  inside an extracted region.
- `flow-shape/valid/region_nested_local_reuse.awl` — nested regions reusing locals at
  unrelated types; the carried payload is the piped `count` of the collected rows, so the
  expression rides the region's own binding.
- `flow-shape/valid/region_sibling_local_reuse.awl` — sibling regions reusing region-local
  names at unrelated types.
- `flow-shape/valid/subflow_nested.awl` — a subflow invoking a subflow, each compiled once,
  the inner invocation binding the outer's local.
- `flow-shape/valid/subflow_local_names.awl` — subflow-local names independently typed: two
  subflows reuse the same author names at unrelated types, and the host reuses them again.
- `flow-shape/valid/value_route_payload.awl` — the canonical value-payload arm:
  `route done(note)` at the workflow boundary, the expression itself the payload.
- `flow-shape/valid/on_failure_route_tail.awl` — `on failure` plus a body-terminal route: the
  fallible prefix runs under compensation, the route stays a genuine tail, so a routed
  failure outcome never reads as a step failure.
- `flow-shape/valid/on_failure_compensates_to_outcome.awl` — the compensation arm actually
  RUNS (`fail_*` fails by name in the BC-4 differential) and the run ends on the failure
  outcome the compensation routed to.
- `flow-shape/valid/on_failure_compensates_to_step.awl` — compensation hands control to a
  SIBLING STEP, whose refund succeeds and settles the workflow.
- `flow-shape/valid/substep_on_failure_route_tail.awl` — a substep's routed failure outcome
  stays outside its compensation attempt: the route fires a parent arm as a tail, never a
  fallible result.
- `flow-shape/valid/action_param_hygiene.awl` — name hygiene: wrapper temporaries must not
  shadow author action parameters (an author binding named `awl_input_codec`).
- `flow-shape/valid/counter_hygiene.awl` — name hygiene: the allocator skips an exact
  collision with its first visit-counter candidate (`awl_visits_again_188`).
- `flow-shape/valid/layer_hygiene.awl` — name hygiene: an author input literally named
  `awl_layer`; the direct path's dependency-parallel layer is an SSA join with no name
  surface to collide on.
- `flow-shape/valid/temporary_hygiene.awl` — name hygiene: pipe, index, workflow-id, and loop
  temporaries avoid author bindings; a binary-concat payload (`status + workflow.id`) in both
  arms of a decision.

Invalid (all CHECK; one per region rule):
- `distribute_never_collected.awl` — region opened, never closed → CHECK
  "never reaches a `collect`" @10 (the opener's line).
- `collect_without_region.awl` — bare collect, nothing open → CHECK "closes no region" @10.
- `nested_region_not_closed_inside.awl` — outer + inner opener, ONE collect: bracket nesting
  pairs it with the INNER region → CHECK "never reaches a `collect`" @14 (the OUTER opener).
- `route_escapes_region.awl` — member routes past the collect → CHECK
  "leaves the per-item region" @15 (route target).
- `route_enters_region.awl` — route from outside targets a member → CHECK
  "enters the per-item region" @14 (route target).
- `collect_binding_not_definite.awl` — a member decision skips the producer → CHECK
  "not definitely assigned on every success path" @27 (the collected binding).
- `decoy_loop_cycle.awl` — the pre-rev-3 `backward_route_bounded_cycle` shape (cycle bounded
  only by an inner loop) → CHECK "not a cycle bound" @36 (backward route target).
- `distribute_not_alone.awl` — distribute + second statement → CHECK
  "is its step's only line" @10.
- `collect_not_first.awl` — collect as the second statement → CHECK
  "`collect` opens its step" @17.

## BC-4 adversarial — the differential-harness corpus (net-new)

Eight fixtures authored for the BC-4 differential harness. Seven lower AND
emit — `nested_handlers`'s on-failure refusal is retired (the exact-span pin
became `nested_handlers_on_failure_now_lowers` in `src/mir/deferred_tests.rs`
and the fixture joined the covered ratchet) — so they run through the
differential to byte-identical durable trails; one remains below both
backends' floor, its refusal pinned in the same file.

Lowering (both backends accept):
- `dag-fork/valid/empty_fork_collection.awl` — fork over a possibly-empty input
  collection; the differential drives it with `[]` (the zero-item fan-out).
- `dag-fork/valid/runtime_sized_fork.awl` — fork whose fan-out width is only
  known at run time (a plain input list).
- `declarations/valid/timeout_inside_retry.awl` — an action with a per-attempt
  `timeout` nested inside a `retry … backoff …` schedule.
- `step-bodies/valid/unicode_payloads.awl` — UTF-8 string literals (日本語, emoji,
  Ω) plus a concat, onto the wire.
- `declarations/valid/child_spawn_combo.awl` — an awaited child call plus a
  fire-and-forget `spawn` of a second child.
- `header-types/valid/max_arity_record.awl` — a high-arity (20-field) record,
  a wide tuple through the codec.
- `loop-outcomes/valid/nested_handlers.awl` — an `on failure` region on a
  guarded step, compensation ending in a workflow failure outcome. Once the
  direct path's recorded refusal, now lowered on both backends
  (`nested_handlers_on_failure_now_lowers`).

Below both backends' floor (recorded refusal, never failed):
- `header-types/zero_step_workflow.awl` — checks clean but has no steps, so
  BOTH backends refuse (no runnable code; span and message pinned by
  `bc4_zero_step_workflow_refuses_no_start_region`). Lives outside any
  `valid/` sweep directory for that reason.
