# SEAT RESUME — marker BF (2026-08-06)

**Supersedes BE for state.** BE remains the record of what round-6 remedy did and
its mutation table (still valid except where noted below). BB/BC/BD are the
finding history.

## HEADLINE

**Review 7 (Opus, read-only) came back NOT SAFE TO LAND: 2 blockers, 7 majors.**
Both blockers were in the layer that had NEVER been reviewed. **B1 is a live
correctness bug, not a comment defect.** I verified both at source myself before
touching anything. All fixes below are APPLIED but **NOT YET COMPILED OR
GATED** — that is the next action.

Review 7 also confirmed a large sound-list (see BE claim table reproduced at the
bottom); **claim 4 of BE was FALSE** and is now corrected.

## THE FULL `aion-rs` BATTERY IS GREEN AND QUIET

`cargo test -p aion-rs`, detached, started 09:09:51Z, log at
`scratchpad/aion-rs-battery.log`. **1105 passed / 0 failed across 59 binaries.**

🔴 This RETIRES the BD-era single failure
(`outbox_happy_path_completes_with_n_terminals_in_order`, the #85 libsql
double-close signature). That run was CONTENDED with a reviewer's builds; it did
not reproduce quiet. Contended-red is not red. **Do not attribute it to #85.**

⚠️ The battery predates every fix below, so it must be re-run before landing.

## B1 — LIVE BUG, CONFIRMED AND FIXED

**`EngineTaskRuntime::arm` takes `task: F` by value and returns without spawning
on BOTH refusal paths** (`AlreadyArmed` at `engine_tasks.rs`, `EpochClosed` at
the top gate and the post-insert undo). The un-polled future was dropped — and
with it the `CompletionRetrySlot` that had been constructed at *future
construction time*, whose `Drop` called `remove_completion_retry` **by key with
no identity check**.

**Failure scenario (confirmed by construction):** run `R` armed retry `T1`, still
spinning. A second exit for the same `(W,run)` arrives — reachable via double
process-exit AND via **reopen, which reuses the run id**. Arm 2 → `AlreadyArmed`
→ future dropped → **T1's entry deleted while T1 runs**. Arm 3 → `Vacant` →
spawns `T2`. Two live writers of one run's terminal. `T1` also escapes
`gate_and_abort` (it is no longer in the map).

`one_run_can_never_hold_more_than_one_completion_retry` arms **25** times and
asserts `== 1`: odd arms insert, even arms delete, so the count alternates
1,0,1,0… and 25 lands on 1. **It passed with ~13 live retries.** A verdict
invariant to the error.

### Fix applied (two parts, both required)

1. **`completion_retry.rs`** — the slot is now claimed on FIRST POLL, inside the
   `async move` body, via `CompletionRetrySlot::claim(weak_tasks, slot_run)`.
   Statements in an async block do not run until polled, so a refused arm claims
   nothing. Uses `tokio::task::try_id()` (not `id()`, which panics off-task);
   `None` is reported via `tracing::error!` and declines to claim — the safe
   direction.
2. **`engine_tasks.rs`** — `remove_completion_retry(&self, run, task:
   tokio::task::Id)` uses `DashMap::remove_if(run, |_, handle| handle.id() ==
   task)`. **This second half is also load-bearing**: the `Occupied` +
   `is_finished` arm deliberately replaces a dead handle, and the outgoing task's
   release can land after that replacement — same eviction race without it.

## B2 — `Drop for Engine` left a WRITING task detached. FIXED

`Drop` did not abort `visibility_reconciliation_task`. Dropping a `JoinHandle`
**detaches**. That task is an unbounded loop holding both stores and calling
`reconcile_visibility`, which **writes**. `Engine::shutdown` aborts it as its
first act. Fix: `Drop` now aborts it too.

The doc comment's claim that the gate "makes a released engine stop writing" was
therefore false. Rewritten, and it now **names the residual it does NOT fix**:
`clear_engine_seams` is deliberately not called from `Drop` (the scheduler has
not stopped, so a NIF could still read a cleared slot — trading a leak for a
use-after-clear is the wrong direction), so **an engine released without
`shutdown` still holds its runtime**. Stated, not hidden.

Two sibling passages tightened to stay true: `api.rs` ("backstops neither: it
gates the epoch and aborts the reconciliation loop, and stops there") and
`handle.rs`.

## MAJORS — ALL APPLIED

- **M1 — BE claim 4 was FALSE.** BE said "`reconcile_terminal_registry` suspends
  rather than removes, so no ordinary terminal path empties the entry today."
  **Five ordinary paths DO remove it**, all verified at my own hand:
  `terminate::complete` (`terminate.rs:64`), `terminate::fail` (`:96`),
  `terminate::cancel` (`:154`), `continue_as_new` (`continue_as_new.rs:150`),
  deadline teardown (`deadline.rs:245`, "deregister LAST"). The **conclusion**
  (do not write the third disjunct) survives and is STRONGER: an empty registry
  entry is not a reopen signature, so the third disjunct is the refuted history
  predicate wearing a registry check as a disguise. Comment rewritten with the
  five sites named and the false generalisation explicitly retracted.
- **M2 — `Default for CompletionRetryConfig`.** "Inherited, not chosen" was false
  against HEAD: **nothing at HEAD retries a completion at all** (verified with
  `git show HEAD:` on `lifecycle/mod.rs`, `completion.rs`, `runtime/config.rs`).
  1 ms/8 ms ARE `SignalDeliveryConfig`'s HEAD backoff values — but that sibling
  pairs them with `max_enqueue_attempts: 8` and **this retry inherited no bound
  at all**. Comment rewritten to say exactly that: ~125 attempts/sec
  indefinitely, single-worker executor, on a store by definition unwell.
  🔴 **STILL OWED: route to Waffles for a ruling on the ladder AND a budget.**
  Unreachable from `aion server` (`completion_retry`/`CompletionRetryConfig`: 0
  hits in `aion-server/src` and `aion-cli/src`).
- **M3 — weak-runtime rationale.** Claimed holding the runtime weakly keeps
  `RuntimeHandle::drop` reachable. False: `ChildNifBridge`/`SignalNifBridge` hold
  `Arc<RuntimeHandle>` unconditionally in the NIF state the handle owns, so that
  path already has a cycle this task plays no part in. Rewritten; the real
  stopper is named (`Drop for Engine`).
- **M4 — the reopen-window test's only assertion could not fail.** The fixture
  leaves the recorder stale by design, so the count can never reach 2. Added the
  decisive `outcome.is_ok()` assertion (the false-`SequenceConflict`-incident
  claim), demoted the count to secondary. Same defect class as review 6's B2,
  surviving in the same file.
- **M5 — borrow-scoping justification.** "A block ends it at a place the compiler
  enforces; `drop(closure)` would only look like it does" describes pre-NLL
  lexical scoping; crate is edition 2024. Rewritten as a readability choice.
- **M6 — injected drain fault impersonated a variant its injection point cannot
  raise.** `begin_shutdown` returned `ProcessExitDrainerShutdownTimedOut`, which
  only `close_and_join_all` can produce. Changed to
  `ProcessExitRegistryPoisoned` (what `lock_lifecycle` actually returns).
  ⚠️ **STILL TO DO:** `handle.rs:~526` and `~545` say the drain failures are
  "all timeout-shaped" — three of four are not. Correct those two sentences.
- **M7 — two false claims in `engine_tasks.rs`.** (a) "a run cannot accumulate
  racing writers" — false before the B1 fix, true after. (b) "Both key components
  … server-derived in full, never off anything a caller supplies" — FIXED:
  `StartWorkflowOptions::workflow_id` is a public caller-supplied field
  (`start.rs:84-86`). Comment rewritten to keep only the bound that is true.

## 🔴 STILL TO DO, IN ORDER — NOTHING BELOW IS DONE

1. **`handle.rs` "all timeout-shaped" ×2** (M6 tail) — not yet applied.
2. **Re-derive `one_run_can_never_hold_more_than_one_completion_retry`.** It must
   assert `== 1` after EVERY arm, not only the 25th, so an even/odd parity can
   never rescue it. **This is mandatory — the current test proved nothing.**
3. Give `dropping_an_engine_without_shutdown_closes_the_completion_retry_epoch`
   (`builder.rs:~846`) a fixture WITH a reconciliation interval, so it can see
   the B2 defect at all. Add an assertion that the reconciliation task is
   cancelled.
4. **Minors from review 7, all unfixed:** `completion.rs` `.ok().flatten()`
   swallowing `RegistryPoisoned` with no log; three unresolvable intra-doc links
   (`api_workflow_ops.rs:31` `SearchAttributeSchema`, `:341`
   `EventStore::list_paused`, `api.rs:77` `EngineTaskRuntime::begin_close`);
   graceful-shutdown cost argued in `completion.rs` not `api.rs`;
   `embedded.rs` new test never calls `engine.shutdown()`; "stale by one" (reopen
   also appends re-armed `TimerStarted`); `respawned_lease` builds
   `Recorder::new` at head 0 (latent trap); `handle.rs:622` says "two" maps,
   `gate_and_abort` sweeps three; four `pub(super)` widenings in
   `builder_assembly.rs` with all callers in-file; `BRIEF-7-…md:69,197` still
   cite `nif_child_tasks.rs`; **`handle.rs` is exactly 500 code lines — at the
   CLAUDE.md ceiling.**
5. `cargo fmt --all`, then compile. **Nothing since the review has been built.**
6. **Re-run the ENTIRE mutation battery** — `scratchpad/mutate.py`. B1 changed
   `arm`'s behaviour, so every recorded result touching the retry registry is
   void. **Add new mutations for B1 and B2**: (a) move the slot claim back to
   construction → must turn the re-derived retry-count test RED; (b) drop the
   identity check in `remove_completion_retry` → must turn something RED;
   (c) drop the `task.abort()` in `Drop for Engine` → must turn the re-fixtured
   drop test RED.
7. Full gate battery again: fmt; clippy `--workspace` AND `-p` (exit code read
   BARE from `$pipestatus[1]`); `aion-client --lib`; `aion-store --lib`;
   `aion-server --lib`; `aion-rs --lib`; full quiet `aion-rs`.
8. **Review 8 on Opus.** Give it BB/BC/BD/BE/BF and review 7's findings.
9. Commit naming files EXPLICITLY, then push.
10. Route **Mo3/M2** to Waffles: the unbounded 1 ms / 8 ms `CompletionRetryConfig`
    default plus the missing attempt budget, unreachable from `aion server`.
11. Tell Tom plainly: reviews 6 AND 7 both failed, review 7 found a real
    double-writer bug, both are fixed.
12. Then #5 (`SPEC-5-AWL-SUM-COMBINATOR.md`), #1, #6, #9.
13. **Owed to Tom:** #146 tutorial dry run + one-page crib sheet, without
    touching his pids.

## REVIEW 7'S SOUND LIST — DO NOT RE-LITIGATE

Verified by the reviewer at its own hand: the api/builder split is genuinely
mechanical (independent line-multiset diff — no dropped `?`, no changed error
handling, no reordering, no dropped logging, no altered lock scope); the
lease-identity discriminator; `Engine::shutdown` ordering; `EngineError` has
exactly 52 variants; `SequenceConflict` unretryable; `check_completion_retry`
shape; no god files (all counted); **zero** `#[allow]`/`#[expect]`/`#[ignore]`/
`#[cfg(any())]`/`todo!`/`unwrap`/`expect`/`panic` in changed files; `mod.rs`
purity; the rename is a true `RM` with no shim and the beamr boundary intact;
**invariant 3** — no new `EventStore::append` site, epoch check inside the lock
and before the branch; **invariant 2** — the one new wall-clock read cannot reach
`workflow.now`; **invariant 4**; the `lib.rs` re-exports; `aion-server/src/
error.rs` additions; the 22-row embedded case table with both controls;
`store_faults.rs` oracle placement.

**BE claims 1,2,3,5,6,7,8,9 all SOUND** — including that `store_error_class` /
`query_error_class` mirror `aion-server` with **no drift** across all 11 rows,
and that the `test-support` dev-dep does not leak into a non-test build.

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
