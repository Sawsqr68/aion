# L3 route report — authored policy-refusal fallback

## Result

- **OBSERVED** — AWL now accepts declaration-level `fallback q1, q2`, rejects duplicate entries at check time, keeps `fallback` soft for existing identifiers, prints it after `retry`, and lowers the same authored list through both the reference emitter and direct MIR backend. Bounds: `Keyword::Fallback` through `Keyword::{ALL,as_word,from_word}`; `ConfigLine::fallback` / `FallbackSpec`; `parse_config_line`; `check_fallbacks`; `walk_call` / `check_call_config`; `emit_configured_activity`; `apply_action_config`; `RuntimeFn::ActFallback`.
- **OBSERVED** — the Gleam activity value carries the chain through every record rebuild, exposes `fallback` and `selected_fallback`, and serializes a JSON array or `null`. Bounds: `aion/activity.{Activity,new,retry,timeout,heartbeat,labels,task_queue,node,execution_tier,fallback,selected_fallback}` and `aion/internal/activity_dispatch.config`.
- **OBSERVED** — `ActivityError::is_settled` makes the classification intentionally three-valued; only `Terminal` settles. All seven settlement/terminal guards named by the leg use that predicate, including the two E-3 guards. Bounds: `ActivityError::{is_retryable,is_settled}`; `HistoryCursor::resolve_activity`; `recorded_terminal_event`; `recorded_race_winner`; `ordinal_is_resolved`; `activity_settled`; both `ensure_terminal_activity_error` functions.
- **OBSERVED** — `Event::ActivityFallbackRouted` carries activity, refusal attempt, from/to queues, and original authored index. It is nonterminal/status-invisible/current-step-invisible, appears in activity-family projections, is consumed by the activity cursor, and has regenerated TypeScript bindings. Bounds: `Event::ActivityFallbackRouted`; `Event::envelope`; status/current-step projections; stream selector; cursor/replay/resolver event-kind projections; delegated and continue-as-new projections; ops timeline variant tables.
- **OBSERVED** — the only production construction path is `Recorder::record_activity_fallback_routed`, invoked from `record_retry_event` through `RetryRecord::FallbackRouted`. The recorder-lock guard checks epoch, run segment, settlement, and duplicate `(activity_id, attempt)` hop before append. A repository search found no direct `EventStore::append` production path for this event. Bounds: `Recorder::record_activity_fallback_routed`; `RetryRecorderSeam`; `RetryRecord`; `record_retry_event`.
- **OBSERVED** — the retry loop handles `policy_refused:` ahead of ordinary retryability: declared chains hop first and become terminal when exhausted; no-chain refusals use the act's existing `RetryPolicy`; refusal records retain `PolicyRefused`; terminal delivery keeps the `policy_refused:` prefix; execution attempt and same-provider attempt are distinct (`execution - recorded hops`). Bounds: `is_policy_refused_reason`; `dispatch_with_retries`; `RetryLoopState`; `handle_policy_refusal`; `route_declared_fallback`; `failure_stand_down`.
- **OBSERVED** — queue delivery is history-derived. `fallback_chain_from_config` reads the run-bound dispatch config, `next_hop` skips the current queue while retaining the authored index, recorded hops supply the recovery queue and terminal sequence, and the override is outside `resolve_task_queue` on single-activity and fan-out paths. Bounds: `nif_activity_fallback::{fallback_chain_from_config,next_hop,recorded_hops,recorded_hop_queue,consumed_fallback_position,refused_queue_sequence,trailing_policy_refusal}`; `dispatch_activity_with_context`; `fan_out_items`; `fresh_scheduled_activity`; `fan_out_items_recovered`.
- **OBSERVED** — durable recovery ordering is explicit. A trailing recorded refusal is disposed before a new start; an existing hop stops re-derivation and supplies its recorded queue. Five named crash-boundary tests pass, including the load-bearing recorded-event queue case and terminal recovery without an invented execution. Bounds: `open_this_delivery`; `dispatch_with_retries`; `retry_fallback_recovery::{replay_before_refusal_record_chooses_the_same_authored_hop,replay_between_refusal_and_hop_records_the_same_hop_before_dispatch,replay_between_hop_and_start_reuses_the_event_queue_not_fresh_derivation,replay_between_start_and_delivery_uses_the_recorded_hop_queue,replay_after_last_refusal_finishes_without_inventing_an_execution}`.
- **OBSERVED** — real-engine E2E coverage is committed for declared-chain history audit, no-chain retry/final prefix, and reference/direct lowering parity. In this venue the test compiles but runtime execution is logged-skipped by the repository's existing Gleam-availability gate because `gleam` is absent. Bounds: `fallback_chain_e2e::{declared_chain_is_auditable_and_both_lowerings_agree,no_chain_retries_without_a_hop_and_finishes_policy_refused}`.

## Verification

- **OBSERVED PASS** — `cargo fmt --check`.
- **OBSERVED PASS** — `cargo test -p aion-core --lib`: 118 passed, 0 failed, 0 ignored.
- **OBSERVED PASS** — `cargo test -p aion-rs runtime::nif_activity_dispatch::tests -- --nocapture`: retry/await suite passed, including declared-chain and no-chain unit cases.
- **OBSERVED PASS** — `cargo test -p aion-rs runtime::nif_activity_dispatch::tests::retry_fallback_recovery -- --nocapture`: 5 passed, one per durable crash boundary.
- **OBSERVED PASS** — `cargo test -p aion-awl --test fallback_config`: focused AWL surface cases pass.
- **OBSERVED PASS** — `cargo clippy -p aion-awl -p aion-rs --all-targets -- -D warnings`.
- **OBSERVED PASS** — `cargo clippy --workspace --all-targets -- -D warnings`.
- **OBSERVED PASS** — `cargo test -p aion-rs --test fallback_chain_e2e -- --nocapture`: all 11 compiled tests passed; the two Gleam-dependent runtime proofs emitted explicit `SKIPPED (PROVES NOTHING)` lines because `gleam` is unavailable.
- **OBSERVED PASS** — `tree-sitter generate && tree-sitter test` using pinned CLI 0.26.10 through a temporary Bun-backed `node` shim: 18/18 corpus parses.
- **OBSERVED PASS** — `tree-sitter build --wasm` using the same pinned CLI and shim; the resulting parser was copied to `apps/aion-ops-console/src/features/authoring/assets/awl.wasm`.
- **OBSERVED BLOCKER** — direct `bun test .../highlight-assets.test.ts` could not load `web-tree-sitter` because the ops-console dependencies are not installed in this venue. The WASM generation command itself succeeded.
- **OBSERVED MEASURED RED, REPORT-ONLY** — full `cargo test -p aion-awl` reaches `the_zed_pin_names_a_mirror_commit_carrying_this_grammar` and fails because the external public tree-sitter mirror has not been pushed/re-pinned. The test reports source digest `e48105e80efb2d313604067292d5a53934e7858805c0cec8d4b4e026667bc190` versus recorded `eb61fb429719c7666618a7dc841266119db320ee9a8cf8557d4364dcb993691e`. Updating that external mirror is outside this branch-local leg and was not faked by editing the digest.
- **OBSERVED BLOCKER** — Gleam formatting/type-check and runtime reference-backend E2E execution cannot run because `gleam` is absent; tests use the existing logged runtime gate and are not ignored.

## Process-wall audit

- **OBSERVED** — no new dependency was added.
- **OBSERVED** — no `#[ignore]`, lint suppression, dead-code hiding, or new blocking duration-sized sleep was introduced.
- **OBSERVED** — generated TypeScript was regenerated by the aion-core export test, not hand-edited.
- **OBSERVED** — generated tree-sitter parser files and AWL WASM were produced by the pinned CLI, not hand-edited.
- **OBSERVED** — `resolve_task_queue` and `config_task_queue_selection` are unchanged; fallback remains a post-failure override.
- **OBSERVED** — no numeric retry bound was introduced; the no-chain path consumes `retry_policy_from_config`.
- **OBSERVED** — oversized base files gained only required predicate/event/recorder/retry-seam integration. `recorder.rs` gained the one required recorder method; `cursor.rs` gained the required predicate/hop span/event-kind work; `nif_activity_retry.rs` gained the reason predicate, explicit settlement read, and tests/docs; `nif_collect_settlement.rs` gained the two required predicate decisions. Routing derivation lives in new `nif_activity_fallback.rs`; retry control necessarily changes `nif_activity_retry_dispatch.rs` and was split into bounded helpers to satisfy strict clippy.

## Errata and artifact precedence

- **REASONED** — E-1 was followed: settlement site 4 is `durability/recorder/fan_out.rs::ordinal_is_resolved`, not the nonexistent runtime path.
- **REASONED** — E-2 was followed: the artifact's `ActionConfig` referent is `ast::ConfigLine`.
- **REASONED** — E-3 was followed because the artifact's unqualified terminal-predicate rule governs: both `ensure_terminal_activity_error` guards were converted in addition to the five originally counted sites.
- **OBSERVED** — the known norn `outcome_into_payload`/`run_result_to_outcome` erratum and engine-side `exhausted:`/`parked:`/`lost:` fallthrough remain report-only and were not changed.

## Receipt

The required command output is pasted verbatim below after the final report-bearing commit/amend:

```text
.fleet/reports/l1-design.md
.fleet/reports/l2-classify.md
.fleet/reports/l3-route.md
```

## FIX ROUND — 2026-08-22

### Attribution before mutation

- **MEASURED IN SCOPE** — the immutable pre-leg base was `c0c04142e66184a19a8602774914e6f88e30d624` (`origin/fleet/policyref-l3-base`; the local fleet branch had already advanced through fan-in). In detached worktree `/tmp/policyref-l3-base-measure` with independent `CARGO_TARGET_DIR=/tmp/policyref-l3-base-target`, all five RED targets passed unchanged: `editor_assets` 5/5, `grammar_parser_gate` 6/6, `guide_reference_gate` 7/7, `runtime_surface_guard` 1/1, and `console_guide_asset` 2/2. Therefore none was pre-existing.
- **MEASURED EXTERNAL RESIDUAL** — anonymous `git ls-remote` reports public mirror head `8d4d63e398bf765dd51772b60555b9dae8ebc106`. A read-only shallow clone digests to the recorded `eb61fb429719c7666618a7dc841266119db320ee9a8cf8557d4364dcb993691e`; this L3 tree requires `5c659cfa6ced3154ad5a69173b320591b83886f80972ed44f42b5d7db405f859`. The mirror has not been pushed, so the Zed pin was not falsified locally. Publishing to the shared external repository requires an authorized mirror push and remains report-only.
- **VENUE** — no memory-shaped anomaly was observed during attribution or verification. The known aion#54 shard-actor timeout battery was not run or retried.

### Minimal in-scope fixes

- Added a checker-passing worker-block witness for authored `fallback`, preserving the grammar/parser gate rather than weakening it.
- Added `fallback` to §4's truthful action-config example in the authored AWL reference, synchronized the embedded reference, and regenerated the console guide JSON from `aion awl guide --json`.
- Regenerated the committed 47-module SDK closure with the repository script using Gleam 1.18.1, so `aion@activity:fallback/2` is a real shipped export. Deliberately repinned the closure and fixed-input archive digests with dated provenance; no generated BEAM was hand-edited.
- No `HarnessError` or `ActivityErrorKind` match was touched. No catch-all arm, lint suppression, `#[ignore]`, library `.unwrap()`/`.expect()`, or dependency was added.

### Verification

- **PASS** — `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`.
- **PASS** — doubled gate suites: `aion-integrations` 67/67 twice, `aion-integration-norn` 92/92 twice, and `aion-worker` 143/143 twice.
- **PASS** — `cargo test -p aion-awl --test grammar_parser_gate --test guide_reference_gate`; 6/6 and 7/7.
- **PASS** — `cargo test -p aion-cli --test console_guide_asset`; 2/2.
- **PASS** — full `cargo test -p aion-awl-package --tests -- --nocapture` with Gleam 1.18.1, including `runtime_surface_guard`, closure integrity/archive pins, and `committed_closure_matches_a_fresh_oracle_harvest`.
- **EXACT GATE RESIDUAL** — `.fleet/gate-entry.sh` cleared fmt, clippy, both focused-suite rounds, and every completed workspace target; its only observed failure is `aion-awl --test editor_assets::the_zed_pin_names_a_mirror_commit_carrying_this_grammar`, for the measured unpublished public-mirror digest above. No other failure or memory-shaped anomaly appears in the no-fail-fast log.

### FIX ROUND receipt

```text
.fleet/reports/l1-design.md
.fleet/reports/l2-classify.md
.fleet/reports/l3-route.md
```
