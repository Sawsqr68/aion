# SEAT RESUME — marker BI (2026-08-06)

**Supersedes BH.** BH's PROGRESS LOG is still the record of what was built and
proven this tick; this file carries REVIEW 9 and what remains.

## REVIEW 9 (Opus) — NOT SAFE TO LAND. The FOURTH refusal in a row.

Reviews 6, 7, 8, 9 have all refused this lane. 7 found a real double-writer bug,
8 found a real silent-zombie path, 9 found **no new correctness defect in the
retry loop** — it refuses on documentation plus one real code hole.

### What review 9 independently CONFIRMED sound (record this, it is load-bearing)
- **B1 keying is correct**, and it verified the precondition at its own hand:
  beamr allocates pids with `next_pid.fetch_add(1, Relaxed)` on an `AtomicU64`
  (`beamr-0.16.2/src/scheduler/spawning.rs:436`), so pids are monotonic per
  scheduler and a superseded lease's key can never be reoccupied. **Pid reuse is
  NOT a hazard here.**
- Two armed retries under different pids cannot both write (`monitor_stands_down`
  + stale-recorder sequence refusal in the registry-empty reopen window).
- Map growth bounded; every entry reclaimed by slot guard / `is_finished`
  replace / undo / `gate_and_abort`.
- **B2 genuinely fixed** — it traced all three `Ok(())` exits;
  `Ok(NotRecorded)` is reachable ONLY from stand-down.
- Every loop exit path reachable and correct; `EngineTaskEpochClosed` classifies
  `Invariant`; no hot spin at any validated ladder.
- Zero suppressions, zero `unwrap`/`expect`, all lock poison handled, module
  sizes under the 500-line rule (`handle.rs` at exactly 500 — at the ceiling).

## WORK DONE THIS TICK AGAINST REVIEW 9

- **BL-1 — MEASURED, and the comment WAS FALSE.** `engine_tasks.rs` claimed
  "measured: deleting the `completion_retries` sweep from `gate_and_abort`
  leaves every test in the crate green". I ran it: **RED** at
  `engine_tasks.rs:935` (`shutdown_gates_new_arms_and_awaits_aborted_tasks`,
  651 passed / 1 failed). Retracted BOTH halves in the code, and named the
  deeper defect: the mutation deletes a line from a helper THREE callers share,
  so it cannot attribute to the Drop path at all — a shared gauge cannot
  attribute to one consumer. Restore verified by `shasum -c`.
- **BL-2 — done.** `completion.rs` doc rewritten (it stated the zombie verbatim
  as a design property) and the test renamed
  `one_run_can_never_hold_more_than_one_completion_retry` →
  `one_monitor_lease_can_never_hold_more_than_one_completion_retry`, with the
  sibling `a_reopened_runs_successor_lease_can_arm_its_own_completion_retry`
  named as the thing that stops a maintainer keying by run again.
- **MA-1 — done.** `api.rs` "Drop for Engine backstops neither" was falsified by
  this lane's own M1 fix. Rewritten to name the seam clearing as the half with
  no backstop, and why (`clear_engine_seams` needs both epochs closed).
- **MA-2 — done by SUBTRACTION.** `api.rs`'s second copy of the drain-failure
  characterisation deleted; it now points at `RuntimeHandle::shutdown`. That
  comment had been wrong twice in two revisions.

## STILL OPEN — do these next

1. **MA-3 (real code hole).** `check_completion_retry` is bypassable: every step
   is `pub` — `CompletionRetryConfig::new` (`config.rs:125`, fields `pub`),
   `RuntimeConfig::with_completion_retry` (`config.rs:213`), `RuntimeHandle::new`
   (`handle.rs:143`), `lifecycle::completion::{ProcessExitContext,
   handle_process_exit}`. m2 was closed with a DOC EDIT; a documented hole is not
   a closed one. **Fix: private fields + fallible `try_new -> Result`, so the
   invariant lives on the type and `check_completion_retry` delegates.** That is
   subtraction. Check blast radius across the workspace first.
2. **MA-4.** `handle.rs:610-613` — third stale two-item enumeration of
   `Drop for Engine` (it now does three things). The argument it supports
   (Drop cannot await) survives; the enumeration does not.
3. **MI-1.** `builder.rs:1038-1043` assertion message claims the TASK survives;
   the observable is `pending_timers.len()`. Delete `pending.handle.abort()` in
   `shutdown_timer_wheel` and the test stays green — and **no test anywhere
   covers that abort**. Use the `DropFlag`/`park_forever` idiom from
   `engine_tasks.rs`. (M1 itself IS measured; only the message overclaims.)
4. **MI-2.** O5 test measures no-gaps, not no-truncation. `fail_next_reads(64)`
   yields exactly 64 failing attempts (one history read each), so
   `assert_eq!(attempts.len(), 64)` is free and closes it. The `>= 3` guard
   accepts a truncation at 3.
5. **MI-3.** `engine_tasks.rs:842-846` control fires before the decisive
   assertion under the faithful mutation. Restructure so the control survives.
6. **MI-4.** `completion_retry.rs:636-641` — at-ceiling rate IS the ceiling, so
   `new(1ms, 2ms)` is a valid config emitting ~500 warn/s. A throttle is
   forbidden; STATE the coupling.
7. **MI-5.** `completion_retry.rs:173-174` "Once the terminal event is in history
   the run IS terminal" is false after a reopen. Narrow to "was observed
   durable"; note `Recorded` covers two states.
8. **MI-6.** `completion_retry.rs:100-110` duplicated paragraph — delete one.
9. **MI-7.** `completion_retry.rs:391,414` misindented `monitor_pid` inside two
   macro bodies (rustfmt does not descend into macro args).
10. **MI-8.** `completion.rs` `attempt_number` splits on `'|'`, which a backend
    error string can contain.
11. **MI-9.** `monitor_pid` missing from both `Invariant` arms
    (`completion_retry.rs:658-677`) and the engine-released warn (`:578-583`).
12. **F-i.** The O5 test's "the arguments below are exactly the ones
    `arm_completion_retry` passes" is held by diligence, not mechanism.

## THEN
- Re-run `cargo test -p aion-rs` unfiltered + `cargo clippy --workspace
  --all-targets -- -D warnings` at the FINAL tree.
- `cargo test --workspace` was in flight at compaction → check
  `$SCRATCH/battery-ws.log`; PID was 35691.
- **Review 10 on Opus.** Then commit + push, files named EXPLICITLY (never
  `git add -A` in this tree).

## PRIOR GATE RESULTS (before the review-9 edits — must be re-run)
`cargo test -p aion-rs` **1109 passed / 0 failed EXIT 0**;
`cargo clippy --workspace --all-targets -- -D warnings` **EXIT 0**.
