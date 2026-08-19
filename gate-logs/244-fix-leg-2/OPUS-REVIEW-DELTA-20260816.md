# Delta review — aion #244 fix leg 2 (0328e67a2..e8c1b28bf)

Reviewer: fresh Opus-grade delta pass dispatched by Waffles, 2026-08-16.
Charter: the two pass-2 NEW findings only (see ../244-fix-leg/OPUS-REVIEW-2-20260816.md).

# VERDICT: LAND

## Dispositions

**1. NEW-B1 discharge — DISCHARGED.** At tip `e8c1b28bf`, `crates/aion-cli/src/tail/live.rs:145-147` is exactly `if !selected_run_seen { continue; }`, placed after the `WorkflowStarted` arm (129-143, which continues in every branch — marker logic unbroken) and before both consumers — child discovery at 148-170 and `observe_attempt` at 171. `monitor_source` has no other event consumer. The three pass-2 consequences traced at the bytes: (a) old-generation `ChildWorkflowStarted` hits the gate before `discovered_children.insert`/`spawn_pending_monitor` — no pending monitor, no announce; (b) old-generation `ActivityStarted` never reaches `observe_attempt` — no SUPERSEDED print, no `spawn_transcript`; (c) `current` cannot be poisoned pre-selection — `observe_attempt` (prior >= attempt skip at 271-273) unreachable until `selected_run_seen`. Seq-ordering premise holds and the fixture models it (monotonic seqs 1-6 across the generation boundary).

**2. NEW-B1 red authenticity — AUTHENTIC.** `fa6907670` touches test files only (its tail.rs hunk is the NEW-2 unit test inside `mod tests`). Fixture `replayed_parent_events` (fixtures.rs:68-77) is genuinely two-generation: seq1 prior WorkflowStarted, seq2 prior child "PriorBuilder", seq3 prior ActivityStarted attempt 9, seq4 selected WorkflowStarted, seq5 selected attempt 2, seq6 selected child. Tracing the pre-fix ungated path reproduces the quoted red's five violations in exact fixture order, including the unfakeable socket-boundary check (`prior_child_attached` set only on a per_workflow subscription for PRIOR_CHILD_ID, e2e:433-434) and the poisoned-map suppression (9 >= 2 skip kills the selected attempt-2 socket). Corroborating detail: the sixth assertion ("selected-generation child not followed") is correctly absent from the red quote — pre-fix code follows everything. Fix commit `b48fb022c` touches production only (src/tail.rs + src/tail/live.rs, zero test files).

**3. NEW-2 discharge — DISCHARGED.** Tip tail.rs:360-368: silent `first_store_seq?` gone, replaced by a match whose `None if event_count == 0 && head_seq > 0` arm emits "no retained events; head at durable seq {head_seq}; earlier records were trimmed by retention" — the review's exact wording. Non-empty path unchanged; zero-head path unchanged (silent, correct — nothing was ever recorded). Unit pin `empty_retained_backfill_reports_its_nonzero_durable_head` at tail.rs:515-522, red-committed first with matching quoted red.

**4. Standards — CLEAN.** Zero suppressions over delta added lines; zero unwrap/expect/panic/`let _ =` in added production lines; all three commit bodies carry real newlines (zero literal \n sequences — pass-2's cosmetic note honored). Scope exactly 4 files, all tail-slice; lint commit `e8c1b28bf` is pure motion (moves spawn_line_reader + assert_replay_outcome into the fixture module for clippy::too_many_lines, assertions byte-identical, no suppression). Append-only re-verified: 3 commits, merge-base b0214c0ef intact.

**5. Red gate leg — SUFFICIENT COVERAGE; no suite re-run required before landing.** Log bytes: fmt/clippy-workspace/clippy-aion-cli/tail-e2e all EXIT=0; gate-aion-cli.log EXIT=101, sole failure `worker_shell::tests::cancellation_reaps_the_entire_spawned_process_group` (ParseIntError{Empty}), 285 passed — and tail_follow_e2e passed 2/2 inside that same red run. Blast radius: worker_shell.rs has zero diff in the delta, no cross-references either direction, test pre-exists at 0328e67a2. The race mechanism is visible in the untouched test source (worker_shell.rs:714-731): `wait_for_pid_file` returns on file existence, but `echo $$ > file` truncates before writing, so `read_pid` can parse an empty file — a test-harness race with no production involvement. The reviewer ran the failing test itself at tip bytes — exit 0, 2.03s — a second independent green arm; also independently re-ran the NEW-2 unit pin (ok) and full tail e2e (2 passed, exit 0) at the tip. Provenance note (not a blocker): leg-2 gate logs carry no in-log HEAD stamp; mitigated by log mtimes post-dating the tip commit, clean worktree at tip, and the reviewer's own green re-runs at tip bytes.

## Follow-up recorded off this landing

- `cancellation_reaps` pid-file race: `wait_for_pid_file` should wait for non-empty content, not existence (worker_shell.rs:714-731) — future slice, board line.
- Leg gate logs should stamp HEAD in-log (leg-1's battery did; leg-2's didn't) — dispatch-prep template fix.

## Reviewer's executed commands (exit codes as reported)

git log/diff/merge-base/show/grep reads as cited; suppression + unwrap greps exit 1 (no hits); `cargo test -p aion-cli --bin aion cancellation_reaps...` exit 0; `cargo test -p aion-cli --bin aion empty_retained_backfill...` ok; `cargo test -p aion-cli --test tail_follow_e2e` 2 passed exit 0.
