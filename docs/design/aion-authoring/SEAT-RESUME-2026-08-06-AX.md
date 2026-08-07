# SEAT RESUME — marker AX, 2026-08-06

Supersedes **AW** for STATE. AW holds the third-review findings F1–F15 and why
each mattered; AV holds the closed sweep row. Latest letter wins.

---

## Where #7 stands: 12 of 15 findings BUILT, 3 mutation legs in flight

### Verified green at my own hand on the current tree

    cargo clippy --workspace --all-targets -- -D warnings   → exit 0 (read DIRECTLY, not off a pipeline tail)
    cargo test -p aion-rs --lib                             → 642 passed / 0 failed

🔴 **Do not cite a `${pipestatus[N]}` from a `cmd | grep | head` chain.** I fired
that trap again this session (`CLIPPY_RC=0` was `head`'s status while clippy was
red with 2 real errors). Run the gate bare and read `$?`.

---

## DONE this session (on top of AW's F1/F2/F3/F6–F12)

- **Leftover `self.reported = Some(fault)`** from the FaultReporter rework — the
  tree DID NOT COMPILE at session start. Fixed. Also made the `seen.push`
  conditional on `unseen`: it was unconditional, so every escalation pushed a
  duplicate and falsified the field's own "bounded by the discriminant catalogue"
  doc.
- **F13 god files — ALL FOUR SPLITS DONE.** Non-test code lines:
  - `lifecycle/completion.rs` 568 → **241** (the attempt)
  - `lifecycle/completion_retry.rs` **336** (NEW — everything about repeating it)
  - `engine/builder.rs` 637 → **443** (builder surface)
  - `engine/builder_assembly.rs` **211** (NEW — the steps `build()` runs)
  - `engine/api.rs` 572 → ~**390** (engine lifecycle: new/accessors/shards/shutdown)
  - `engine/api_workflow_ops.rs` **NEW** (start/cancel/continue/reopen/pause/result/list)
  - Sweep of `crates/aion/**`: **no non-test file over 500.** The six remaining
    over-500 hits are all `*_tests.rs`, which the standard excludes.
  - `api.rs` was ALREADY 554 at HEAD and `builder.rs` 596 — pre-existing, but I
    made both worse, so both got split.
- **F14** — `TerminalIntent::exit_time` now carries the ordering consequence AND
  the survey, verified not asserted: `recorded_at` is not a proxy for append
  order (`sequence` is); the determinism clock advances ONLY in `Replay::step`
  on commands resolved against history, and a terminal is not such a command —
  it ends replay — so a reused `exit_time` can never be read back as
  `workflow.now`. `close_time` = `terminal_recorded_at` is the intended
  behaviour. Stated as a survey of today's callers, not a guarantee.
- **F15** — `aion-client/src/transport/embedded.rs` `map_engine_error`:
  `EngineTaskEpochClosed` now maps to `unavailable` alongside `ShuttingDown`
  (it is an availability fact, not an engine fault; `server` would send an
  operator hunting a bug that is not there). The `_` arm is KEPT and its cost is
  written down — 58 variants, not `#[non_exhaustive]`, so the rule for adding an
  arm is "would the generic bucket mislead a caller about what to DO".
- **Two clippy errors found and fixed by the workspace gate**: `drop(keep)` on a
  non-Drop closure (replaced with a real scope block — a block ends the borrow
  where the compiler enforces it, `drop()` only looked like it did), and
  `workflow_not_found` left after the test module.
- **New crate-visible test seam**: `RuntimeHandle::force_process_exit_drain_failure`
  (`#[cfg(test)]`, delegates to the still-private `process_exits`). Needed
  because the F3 property lives at `Engine::shutdown`, one module above where
  `force_shutdown_failure` was reachable.
- **F1 mutation leg WRITTEN** (not yet run):
  `the_resume_arm_refuses_to_append_once_the_epoch_is_closed` in
  `completion.rs` tests. Setup = terminal already durable + outstanding deadline
  (copied from `process_exit_resumes_interrupted_deadline_cancellation`),
  because that is the ONLY shape where the resume arm has a real append to
  refuse — without the outstanding deadline it writes nothing anyway and the
  test would pass with the gate deleted. Carries that control explicitly.

---

## 🔴 STILL TO DO — nothing here is optional

1. **RUN the F1 leg** — `cargo test -p aion-rs --lib the_resume_arm_refuses`.
   Then prove it decisive: move `refuse_if_epoch_closed` back inside the `else`
   arm of `handle_process_exit_attempt` and confirm this test goes RED while
   `an_attempt_refuses_to_append_once_the_epoch_is_closed` stays GREEN. That
   divergence IS the finding. Restore the fix.
2. **F2 mutation leg** — in `engine/builder.rs` tests, beside
   `a_zero_initial_backoff_is_refused_before_anything_is_started`: a zero
   `max_backoff` (and a `max_backoff < initial_backoff`) must be refused by
   `check_completion_retry`, with a control that a sane ladder is accepted.
   Prove decisive by deleting the new `max_backoff` branch.
3. **F3 mutation leg** — in `engine/api.rs` tests: build an engine via
   `engine_with_loaded_workflow`, call
   `engine.runtime().force_process_exit_drain_failure()`, then `engine.shutdown()`.
   Assert (a) the error still reaches the caller, (b) the epoch is closed, and
   (c) **`runtime.nif_state().installed_workflow_catalog()` is `None`** — that
   last one is the decisive observable, because it proves `clear_engine_seams()`
   ran AFTER a failing step. With the old `self.runtime.shutdown()?` the function
   returns early and the catalog is still installed.
4. Re-run on the resulting tree: `cargo fmt` (autoformat, never format-check),
   `clippy --workspace --all-targets -D warnings` (read `$?` BARE),
   `cargo test -p aion-rs`, full mutation board.
5. **COMMIT — naming files EXPLICITLY.** Never `git add -A` in this tree.
   🔴 **`git add crates/aion/src/store_faults.rs`** — it is UNTRACKED, `lib.rs`
   declares it and `completion.rs`/`completion_retry.rs` use it, so committing
   without it lands a NON-COMPILING test build. Also new and untracked:
   `crates/aion/src/lifecycle/completion_retry.rs`,
   `crates/aion/src/engine/builder_assembly.rs`,
   `crates/aion/src/engine/api_workflow_ops.rs`.
   Commit message via `git commit -F -` with a QUOTED heredoc (zsh eats
   backticks from `-m`). Then push.

**Then:** Tom's remaining queue — #9 operator fact in `details`, #6
`max_concurrency` ignored on liminal, #5 AWL `sum` (must bump `Keyword::ALL`'s
`[Self; 57]` in the SAME change), #1 `workflow.now()` history tail vs replay
position. Then walk the tutorial live with Tom (#146), the #139 live cut, the
0.11.1 decision, #142 held on that cut.

---

## Tom comms this session

He asked "how are we going? I think it's about to go last I heard." I answered
at 06:0xZ, plain language, four-part: review came back not-safe-to-land and was
right; three of my earlier fixes were half-fixes that left a side door open on
the same problem; my own tests passed with the holes in because they walked
through the front door; all fixed, clean build, 642 pass; what's left is three
tests that walk the side doors, then it lands; five more issues after. Told him
nothing needs a decision and offered to land-now-test-later if he prefers.
**He has not replied — poll for an answer before assuming silence = proceed.**

---

## Standing constraints (unchanged, carry verbatim)

NO WORKTREES in aion · ONE mutating lane · unmarked dirty = STOP · never
`git add -A` · never `git stash -u` · NO `git gc`/`prune`/`--prune` until the
estate deletion hold lifts · **DELETE NOTHING** · no `--force`/`--mirror`/
`--delete` on any push · ⛔ never bare `cargo clean`, no custom
`CARGO_TARGET_DIR`, no launchd · 🔴 **Tom's pids 45425 (server) / 48399 (worker)
— DO NOT TOUCH** (untouched all session) · worker rebuilds FROZEN estate-wide
until the #139 live cut · NO-SWEEP untracked dirs: `.aion-accepted-binary-20260731/`
(SOLE copy of Tom's accepted build), `.aion-jjuabk-snapshot-20260731/`,
`.aion-data-preswap-20260804/` · beamr JIT blocker with Artemis: keep long
workflows off until the fixed beamr lands · do not edit `CLAUDE.md` · never
mention prospekt to Tom · **NEVER publish artifacts for Tom** — deliverables are
local files · CLAUDE.md mandates an **Opus** sub-agent review before work lands ·
`<<autonomous-loop-dynamic>>` wakeup must be re-armed every tick.

Box: AEST (+1000) — always `-t "%Y-%m-%dT%H:%M:%S%z"`. zsh: no `PIPESTATUS`
(lowercase `$pipestatus`, 1-indexed), `status` is a READ-ONLY alias for `$?` so
name the variable `rc`, no word-splitting on unquoted expansion. BSD `sed` has
no `\s` — use `/usr/bin/python3` (NOT `/Users/tom/.local/bin/python3`, it cannot
exec). Foreground `sleep` is blocked — use `until <check>; do sleep N; done` with
an extended `timeout`, or `run_in_background`. Estate diff driver emits no ±
prefixes — `git -c diff.external= diff --no-ext-diff`.
