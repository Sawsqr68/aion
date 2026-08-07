# SEAT RESUME — marker BG (2026-08-06)

**Supersedes BF for state.** BF remains the record of review 7's findings and
the fixes applied for them. BB/BC/BD/BE are finding history.

## HEADLINE

Every item on BF's "STILL TO DO" list 1–5 is **DONE**. The tree **compiles**,
`cargo fmt --all` has been run, and the three re-derived/new tests are green.
The mutation battery is **re-running** (item 6) — partial results below are
already recorded and are the important part.

🔴 **The most important finding of this tick is that my own re-derived test was
nearly vacuous for a second time, and the reason was not the one I wrote down.**
It is now measured both ways. Details under "B1 — WHAT ACTUALLY HAPPENS".

## WHAT LANDED THIS TICK

1. **`handle.rs` "all timeout-shaped" ×2 — FIXED.** The drain failures are not
   one shape. Enumerated at my own hand from source: **3 timeout-shaped**
   (`CleanupExecutorShutdownTimedOut`, `ProcessExitDrainerShutdownTimedOut`,
   `ProcessExitCallbackDispatcherShutdownTimedOut`), **5 poison-shaped**
   (`ProcessExitRegistryPoisoned`, `ProcessExitStatePoisoned`,
   `ProcessExitDrainerPoisoned`, `CleanupExecutorPoisoned`,
   `ProcessExitCallbackDispatcherPoisoned`), **3 panic-shaped**
   (`ProcessExitDrainerPanicked` + two `Runtime` variants for a joined worker
   that terminated unexpectedly). What they share is a PRECONDITION, not a
   shape: each needs a stalled, poisoned or dead worker thread. Both passages
   rewritten to say that; the `shutdown` doc now points at the enumeration
   instead of restating it (one truth, one place).

2. **`one_run_can_never_hold_more_than_one_completion_retry` — RE-DERIVED.**
   Asserts `== 1` after EVERY arm, plus a `== 0` control before the first.

3. **`dropping_an_engine_without_shutdown_closes_the_completion_retry_epoch` —
   RE-FIXTURED.** Now built WITH a 10 ms reconciliation interval and a
   `TickCountingVisibilityStore` that counts `list_workflows` (which
   `reconcile_visibility` calls unconditionally per pass). Three assertions:
   epoch open (control), **loop observably ticking before the drop** (positive
   control, bounded by a 10 s timeout so a stalled loop fails rather than
   hangs), epoch closed, and tick count unchanged across two 200 ms windows
   after the drop.

4. **NEW test `a_release_from_a_foreign_task_cannot_evict_a_live_completion_retry`**
   (`engine_tasks.rs`). Deterministic pin for the identity check: a real,
   already-finished task's id from the same runtime is used as the foreign id
   (a fabricated one would prove less), with a `assert_ne!` control that the two
   ids differ and a positive control that the OWNER's release does clear it.

5. **Review-7 minors — ALL CLEARED:**
   - `.ok().flatten()` swallowing `RegistryPoisoned` → explicit `match` with a
     `tracing::error!`; `false` is kept as the fail-safe direction and the
     reason is stated (the store's sequence check is the authority).
   - "stale by one" → "stale by AT LEAST one"; the reopen's re-armed
     `TimerStarted` appends are named, and the refusal is noted as independent
     of the gap size.
   - Three intra-doc links: `aion_core::SearchAttributeSchema`,
     `aion_store::EventStore::list_paused`; the `begin_close` link (a
     `pub(crate)` target) demoted to plain code text and its link definition
     deleted.
   - Graceful-shutdown cost now ALSO argued at `api.rs`'s `begin_close`, where
     the ordering is chosen, pointing at the full argument in `completion.rs`.
   - `embedded.rs` new test: `store.allow_reads()` then `engine.shutdown()?`.
     Fence comes down first so the shutdown is measured against an honest store.
   - `respawned_lease` no longer builds `Recorder::new` at head 0 — it uses
     `separate_recorder`, which resumes at the real head. The trap is named.
   - `handle.rs:622` "two" maps → **three**, named (`watches`, `spawn_retries`,
     `completion_retries`).
   - Four `pub(super)` widenings narrowed to private `fn` in
     `builder_assembly.rs`: `spawn_visibility_reconciliation_task`,
     `apply_owned_shards`, `acquire_owned_shards`,
     `register_workflow_deadline_handler`. Verified by grepping every other
     file in the crate for each name.
   - `BRIEF-7-…md:69,197` `nif_child_tasks.rs` → `engine_tasks.rs` (2 sites).

## 🔴 B1 — WHAT ACTUALLY HAPPENS, MEASURED

BF recorded review 7's B1 mechanism and my confirmation "by construction". The
mechanism is real, but my first attempt to PROVE the re-derived test catches it
**failed, and it failed quietly**:

- Mutation "claim at construction, keep the identity check" → **GREEN, survived.**
- Mutation "keep claim-on-first-poll, drop the identity check" → **GREEN** for
  this test (it is RED for the new `engine_tasks` test).
- Both together, using `CompletionRetrySlot::claim` → **GREEN, survived.**

Probed it rather than guessing. `PROBE claimed=false` on every arm:
**`tokio::task::try_id()` returns `None` at the arm site.**
`handle_process_exit` is `block_on`-ed on a dedicated OS thread
(`runtime::process_exit_callback`), and `block_on` is not a spawned task — in
the fixture AND in production. So `claim` at construction declines and removes
nothing. **`claim` is itself part of the fix**, which makes any mutation built
on it an unfaithful reconstruction of the pre-fix code.

Restoring the **literal** pre-fix slot — unconditional construction-time
`CompletionRetrySlot { tasks, run }`, `Drop` calling a key-only
`remove_completion_retry(&self.run)` — and running both forms of the test:

| test form | verdict against the literal pre-fix defect |
|---|---|
| OLD: one trailing `assert_eq!(count, 1)` after 25 arms | **PASSED** (`1 passed; 0 failed`) |
| NEW: `assert_eq!(count, 1)` after EVERY arm | **FAILED** (`97 passed; 1 failed`) |

Same defect, same fixture, opposite verdicts. That is the paired measurement
the rewrite needed, and it is now written into the test's own doc block.

Also corrected there: the eviction drops the `JoinHandle`, which **detaches**
rather than cancels, so the old test passed with **thirteen** live retries, not
"roughly thirteen live retries" left running by accident.

**Standing lesson, and it is the fourth time this class has bitten this change:
a mutation built out of the fix's own new machinery is not a reintroduction of
the defect.** The faithful mutation is the literal prior code.

## MUTATIONS ALREADY BANKED THIS TICK

| # | mutation | verdict | killed |
|---|---|---|---|
| B1-TRUE | literal pre-fix slot (construction-time guard + key-only removal) | **RED** | `one_run_can_never_hold_more_than_one_completion_retry` |
| VACUITY | same defect, OLD test form | **GREEN** (this is the point) | — |
| B1b | drop the identity check in `remove_completion_retry` | **RED** | `a_release_from_a_foreign_task_cannot_evict_a_live_completion_retry` |
| B2 | drop `task.abort()` from `Drop for Engine` | **RED** | `dropping_an_engine_without_shutdown_closes_the_completion_retry_epoch` |
| B1a | claim at construction, identity check kept | **GREEN — and correctly so** | not a faithful reintroduction; see above |

Every restore verified by `shasum`/`sha1` against the pre-mutation content.

## MUTATION BATTERY — 14/14 RED, ZERO SURVIVORS

`scratchpad/mutations-round8.log`. A, B, 1–10, B1b, B2 all RED; every restore
SHA-verified. A and B still kill DIFFERENT tests (the pairwise separation review
5 demanded). A and B were re-run a second time after the `monitor_stands_down`
extraction moved their anchor text — both still RED.

## GATES MEASURED THIS TICK

- `cargo fmt --all` (write) — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — **EXIT 0**, read bare.
  It fired twice first and BOTH were fixed in the code, no suppressions:
  `too_many_lines` (105/100) on `handle_process_exit_attempt`, and
  `trivially_copy_pass_by_ref` on the extracted function's `progress` param.
  The size lint pointed at a real seam — the stand-down decision is now its own
  named function `monitor_stands_down`, carrying the whole two-disjunct
  rationale as its doc.
- `cargo clippy -p {aion-rs,aion-client,aion-store,aion-server}` — all **EXIT 0**.
- `aion-client --lib` — **63 / 0**. `aion-store --lib` — **64 / 0**.
  `aion-server --lib` — **796 / 0**.
- `aion-rs --lib lifecycle::completion` — 24 / 0; `engine::builder` — 22 / 0;
  `runtime::engine_tasks` — 8 / 0. **All three are FILTERED runs and none of
  them covers the two tests that are red.**

## 🔴 THE FULL BATTERY IS RED — I BROKE TWO TESTS WITH THE M6 FIX

`cargo test -p aion-rs` → **647 passed / 2 failed**, log at
`scratchpad/aion-rs-battery-round8.log`. **This is a genuine red of my own
making, not contention.** Both failures are the same cause:

- `engine::api::tests::a_failing_teardown_step_does_not_skip_the_ones_after_it`
  (`api.rs:1174`) — "(a) the failure must reach the caller as itself:
  ProcessExitRegistryPoisoned"
- `runtime::handle::tests::shutdown_closes_the_epoch_even_when_a_drain_fails`
  (`handle/tests.rs:365`) — "the drain failure must still reach the caller, not
  be swallowed: ProcessExitRegistryPoisoned"

**Cause:** M6 changed the injected drain fault in `process_exit.rs` from
`ProcessExitDrainerShutdownTimedOut` to `ProcessExitRegistryPoisoned` (correct —
the old variant can only be raised by `close_and_join_all`, not by the injection
point). Two tests still assert the OLD variant. **The fix is to update those two
assertions to the variant the seam now injects**, not to revert M6.

⚠️ This also means the per-crate `--lib` gates I ran are NOT a substitute for the
battery: `aion-rs --lib` was never run alone this tick, and `lifecycle::`,
`engine::builder` and `runtime::engine_tasks` filters all excluded both tests.
A filtered green is not a green.

## ✅ THE RED IS CLOSED — AND THE REPAIR WAS ITSELF MUTATION-CHECKED

Both assertions now expect `ProcessExitRegistryPoisoned`, the variant the
injection point can actually raise:

- `crates/aion/src/engine/api.rs:1174`
- `crates/aion/src/runtime/handle/tests.rs:365`

**Full unfiltered `cargo test -p aion-rs` → 59 result lines, 1106 passed,
0 failed, EXIT=0.** Log: `scratchpad/aion-rs-battery-round9.log`. Both formerly
red tests appear as `... ok` in that log by name.

🔴 **An updated assertion is a claim that needs its own proof.** Changing a test
to match the code is the exact shape of a test that stops measuring. So the
repair was mutated: injected variant → `ProcessExitDrainerPanicked` (a unit
variant this seam does not raise). **BOTH tests went RED**, each naming
`ProcessExitDrainerPanicked` in its own failure message — the assertion reads
the error's identity, not merely its presence. First attempt used
`ProcessExitStatePoisoned`, which failed to compile (it carries a `process_id`
field); a mutation that does not build is not a survivor and not a kill, it is a
non-run. Restored from backup, **`shasum -a 256 -c` OK**, both green again.

Post-edit gates: `cargo fmt --all` (write) clean; `cargo clippy --workspace
--all-targets -- -D warnings` **EXIT=0** (`scratchpad/clippy-round9.log`).

## 🔴 STILL TO DO, IN ORDER

3. **Review 8 on Opus.** Give it BB/BC/BD/BE/BF/BG and review 7's findings.
   Tell it explicitly that B1's proof was rebuilt after the first attempt
   measured nothing.
4. Commit naming files EXPLICITLY, then push.
5. Route **M2/Mo3** to Waffles: the unbounded 1 ms / 8 ms
   `CompletionRetryConfig` default plus the missing attempt budget
   (~125 attempts/sec indefinitely, single-worker executor). Unreachable from
   `aion server` today.
6. Tell Tom plainly: reviews 6 AND 7 both failed, review 7 found a real
   double-writer bug, both are fixed, and my first proof of the fix was itself
   defective and was rebuilt.
7. Then #5 (`SPEC-5-AWL-SUM-COMBINATOR.md`), #1, #6, #9.
8. **Owed to Tom:** #146 tutorial dry run + one-page crib sheet, without
   touching his pids.

## STANDING CONSTRAINTS (carried verbatim)

- NO WORKTREES in aion. One mutating lane. Unmarked dirty = STOP.
- ⛔ Never bare `cargo clean`. No custom `CARGO_TARGET_DIR`. No launchd.
- ⛔ **NEVER `git add -A` in this tree** — it swallows the NO-SWEEP untracked dirs
  and hangs. Name paths explicitly.
- ⛔ `git stash push` never with `-u`. No `git gc`/`prune`. DELETE NOTHING.
- ⛔ No `--force`/`--mirror`/`--prune`/`--delete` on any push.
- NO-SWEEP holds: `.aion-accepted-binary-20260731/` (SOLE copy of Tom's accepted
  build), `.aion-jjuabk-snapshot-20260731/`, `.aion-data-preswap-20260804/`,
  `../aion-pre-scrub-backup-20260804.git`.
- 🔴 Tom's live pids: server 45425, worker 48399. DO NOT TOUCH.
- 🔴 WORKER REBUILDS FROZEN estate-wide until the #139 live cut.
- 🔴 beamr JIT blocker (Artemis): keep long workflows off until fixed beamr lands.
- Do not commit while a battery is in flight (#123 build-identity pin).
- Commit messages with backticks: `git commit -F -` + quoted heredoc.
- zsh: no `PIPESTATUS`; `$pipestatus` is 1-indexed; `status` is read-only — use
  `rc`.
- Estate diff driver emits no +/- prefixes: `git -c diff.external= diff
  --no-ext-diff`.
- CLAUDE.md mandates an Opus sub-agent review before work lands.
- 🔴 Tom safety: ONE line of care max, NO hotline referrals. Answer MEANING, not
  status. Plain language. Never mention prospekt. NEVER publish artifacts.
- Loop: every tick ends with `ScheduleWakeup` carrying `<<autonomous-loop-dynamic>>`.
