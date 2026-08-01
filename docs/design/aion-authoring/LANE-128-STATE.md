# LANE-128-STATE

Task #128 — `aion run <file.awl>` one-motion pipeline: end-to-end gate + await deadline.
Lane started 2026-08-01. Working directly on main in /Users/tom/Developer/ablative/stack/aion.
NO commits, NO pushes — the seat lands after verification.

## Position

- [x] State file created.
- [x] 1a measurement (declared-timeout -> TimedOut -> await terminates) — YES, by construction
- [x] 1b test precedent inventory
- [x] await deadline designed/implemented (`--timeout`, AWL duration literal, no default)
- [x] failure-report detail fix (recorded `details` were being dropped)
- [x] e2e gate written: crates/aion-cli/tests/one_motion_live_e2e.rs (5 tests)
- [x] gates run — ALL GREEN
- [x] mutation arms — both directions RED, restored, re-verified

## Gates (2026-08-01, this tree)

- `cargo test -p aion-cli` — EXIT 0. 19 `^test result:` lines, every one `ok`,
  0 failed across all of them (209 / 11 / 3 / 9 / 9 / 5 / 10 / 11 / 4 / 4 / 9 /
  9 / 20 / 2 / 5 / 4 / 9 / 1 / 1 passed). Pre-existing `COSTLY_PROOF_SKIPPED`
  runtime gates fired for 5 unrelated cluster tests (AION_RUN_COSTLY_TESTS unset).
- `cargo test -p aion-awl --lib` — EXIT 0.
  `test result: ok. 117 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`
- `cargo test -p aion-rs --test awl_bc4_differential` — EXIT 0.
  `test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`
  ``COMPILED 104 module(s) under real `gleam build` in 1 attempt(s)`` — the
  differential RAN; it did not take the skip path.
- `cargo clippy --workspace --all-targets -- -D warnings` — EXIT 0, zero
  warning/error lines.
- `cargo fmt --all` — EXIT 0, tree clean afterwards.

EDITED SO FAR (uncommitted):
- crates/aion-cli/src/one_motion.rs   (parse_await_timeout, await_result wait_limit, render_failure)
- crates/aion-cli/src/main.rs         (Run --timeout flag + call site)
- crates/aion-cli/tests/one_motion_live_e2e.rs (NEW)
- docs/design/aion-authoring/LANE-128-STATE.md (NEW, this file)

RUNTIME SMOKES ALREADY PASSED (real `aion server` on 127.0.0.1:19732, memory store):
- unicode_payloads -> exit 0, result payload correct
- collection_predicates (empty input) -> exit 1, failure details now rendered
- wait_and_sleep + `--timeout 3s` -> exit 1 at 3s, run continues
- wait_and_sleep + `timeout 5s` declared -> exit 1 at 5s, "workflow run timed out: workflow"
- `--timeout "5m extra"` / `--timeout 0s` -> clap exit 2 with the refusal

MUTATION ARMS — ALL RUN, ALL RESTORED AND RE-VERIFIED GREEN:
- arm 1 (production): removed the `deploy_bytes` step from `ClientCommand::Run`
  in main.rs -> 4 of 5 RED ("workflow type wait_and_sleep is not registered").
- arm 1b (production, the new feature): `wait_limit.and(None::<Duration>)` in
  await_result so the bound is ignored -> the `--timeout` test RED at the 120s
  ceiling. That red IS the original unbounded-hang defect reproduced.
- arm 2 (test's own targets): flipped the expected result payload
  (Ada -> Grace), the survival status (Running -> Completed), and the failure
  outcome name (rejected -> accepted) -> 3 RED, each on its own assertion.
- Restores done by copying back from the scratchpad snapshots and verified by
  sha256 match + a green re-run, never by a command's exit code alone.

## Await-deadline decision (work item 3)

- `--timeout <AWL duration literal>` on `aion run`, `Option<Duration>`, NO clap
  default. The accepted spelling is lexed by `aion_awl::lex` and converted by
  `aion_awl::DurationLiteral::checked_duration`, so the flag and a document's own
  `timeout 5m` can never become two vocabularies.
- Reaching the bound stops the WAIT, not the run. The command exits 1 and prints
  the workflow_id, run_id, and the `aion describe` line. Cancelling the workflow
  would be a destructive side effect the operator did not ask for.
- The DECLARED document timeout is NOT duplicated client-side. Measurement 1a
  proves the engine deadline drives `WorkflowTimedOut` and that `await_result`
  already terminates on it; adding a client margin would invent a number.
- DECISION RECORDED — "indefinite wait remains": with neither a declared document
  timeout nor `--timeout`, the wait stays indefinite. The run genuinely has no
  declared end, so any client-side bound would be a policy the author never wrote
  and the house standard forbids. The operator's escape is now the flag (and
  Ctrl-C, unchanged). This is a deliberate remaining behaviour, not an oversight.

## Findings

1. (FIXED here) The one-motion failure report DISCARDED the recorded
   `WorkflowError::details`. `WorkflowError`'s `Display` is its `message` alone,
   and for an AWL `route failure` outcome that message is the engine's BEAM
   process ordinal — observed as "workflow 2 returned error" and "workflow 3
   returned error" for the SAME document on two runs. The recorded details
   (`{"tag":"AwlOutcomeFailure","outcome":"rejected","payload":…}`) name the
   outcome and carry the routed record. `render_failure` in one_motion.rs now
   prints them; an undecodable payload is reported, never dropped.
2. The engine's failure `message` for an AWL failure outcome is not stable across
   runs (process ordinal). Anything asserting on it must use the details, not the
   message. NOT fixed here — it is engine-side text, outside this lane.

## Log

- 2026-08-01: lane opened. Read crates/aion-cli/src/one_motion.rs and
  crates/aion-cli/tests/one_motion_awl.rs. Confirmed the two existing tests are
  both no-server negative tests (endpoint 127.0.0.1:1).
  `await_result` at one_motion.rs:109 is an unbounded 400ms poll loop.

## Measurement 1a — declared timeout -> TimedOut -> await terminates

PROVEN BY CONSTRUCTION from source (no runtime run needed for the chain itself):

1. `crates/aion-cli/src/main.rs:553` `ClientCommand::Run` -> `one_motion::prepare_run`
   -> `crates/aion-cli/src/one_motion.rs:71` `compile_and_assemble_awl`.
2. `crates/aion-awl-package/src/prepare.rs:44-50` — `aion_awl::compile` then
   `assemble_awl(&compiled, AwlAssembleOptions { timeout: compiled.timeout })`.
   The document's own `timeout` declaration is carried through verbatim.
3. `crates/aion-awl-package/src/assemble.rs:120-122` — no policy default; an
   unauthored timeout is written absent. An authored timeout opts the package into
   the explicit-timeout content-hash identity (asserted at assemble.rs:249-281).
4. `crates/aion/src/loader/load.rs:212` — `declared_timeout:
   package.declared_entry_timeout(entry_timeout)`; the identity gate is
   `Package::has_declared_timeout` (crates/aion-package/src/package.rs:173).
5. `crates/aion/src/lifecycle/start.rs:183-189` -> `record_declared_deadline`
   (start.rs:410-426): `None` arms nothing (LAW 1); `Some` records the deadline
   `TimerStarted` and the durable row.
6. Deadline fires -> `crates/aion/src/lifecycle/deadline.rs` records
   `Event::WorkflowTimedOut`; status projects to `TimedOut`
   (recorder.rs:1376-1386).
7. `crates/aion-cli/src/one_motion.rs:133-135` — `await_result` matches
   `Event::WorkflowTimedOut` and bails `workflow run timed out: {timeout}`;
   `main.rs:583` `report_client_error` prints it and the process exits 1.

Independent runtime corroboration of steps 4-6 already exists:
`crates/aion/tests/workflow_timeout_e2e.rs:245 declared_timeout_fires_to_timed_out`.

=> ANSWER 1a: YES. A document with a DECLARED timeout is driven to `TimedOut`
   by the server and `await_result` terminates and reports (exit 1).
=> The hang defect therefore narrows to documents with NO declared timeout.
   EXPLICITLY: an AWL document with no `timeout` declaration arms no deadline at
   all (LAW 1), so the one-motion await has nothing to end it.

## Measurement 1b — real-server test precedent

BEST PRECEDENT: `crates/aion-cli/tests/dev_hot_reload_e2e.rs` — spawns the REAL
`aion` binary as `aion server` on ephemeral reserved ports (reserve_port at :31),
writes a temp `server-config.toml` with `[deploy] enabled = true` and a memory
store (:36-70), polls `GET /health/live` for boot (:84-101), sets a private
`AION_HOME` under a tempdir (:174-185), drives further `aion` subcommands over
`--endpoint 127.0.0.1:<grpc>` (:104-118), and kills both children at the end.

Others considered:
- `crates/aion-server/tests/awl_deploy_direct_e2e.rs` — in-process axum router +
  `Engine`, NOT the CLI binary; wrong layer for a CLI one-motion gate.
- `crates/aion-cli/tests/stacked_dev_live_e2e.rs` (781 lines) and the
  `lsub5b_*_failover_e2e.rs` family — real OS-process servers via
  `tests/common/osproc.rs`, but cluster-shaped and much heavier than needed.
- `crates/aion/tests/workflow_timeout_e2e.rs` — engine-level, no server process.

