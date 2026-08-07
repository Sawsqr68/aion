# SEAT RESUME — marker BH (2026-08-06)

**Supersedes BG for state.** BG remains the record of the B1 measurement story
and the 14/14 mutation battery. This marker is the authority from here.

## HEADLINE

**REVIEW 8 = NOT SAFE TO LAND.** Third consecutive review to refuse this lane,
and the third to find something real. 2 blockers, 4 majors, 3 minors, 6 false
docs. The battery is green and clippy is EXIT 0 — **that is exactly why the
review mattered**; nothing red pointed at any of this.

Reviewer verified at its own hand: `cargo test -p aion-rs` EXIT 0, 59 result
lines, 1106 passed / 0 failed; `cargo clippy --workspace --all-targets -D
warnings` EXIT 0; suppression sweep over all 17 changed files clean; all files
under the 500-line rule; fault-injection seams properly `#[cfg]`-gated.

🔴 **It also independently re-derived and CONFIRMED the rebuilt B1 proof**,
including the `try_id()` = `None` finding and the reason a `claim`-based mutation
is unfaithful. That part of the lane stands.

---

## BLOCKERS

### B1 — a reopened run's terminal can be permanently orphaned, silently

`lifecycle/completion_retry.rs:396-407` + `runtime/engine_tasks.rs:187-198`

The completion-retry registry is keyed `(WorkflowId, RunId)`. **A reopen reuses
the run id** (`lifecycle/reopen.rs:391` re-registers under `run_id.clone()` and
installs a fresh monitor). So a superseded lease's still-armed retry and the
successor lease's exit **collide on one key**.

1. Run R's terminal is written by another writer; this monitor's `progress`
   stays `NotRecorded`.
2. Monitor's exit fires, inline attempt fails transiently, T1 armed for `(W,R)`.
3. Operator reopens R (admissible — the run is terminal). Lease 2 registers
   under the **same** `(W,R)`.
4. Lease 2 exits, inline attempt fails transiently, `arm` finds `Occupied` +
   `!is_finished()` (T1 still sleeping) → **`AlreadyArmed`. Nothing is armed.**
   Only trace is `tracing::debug!` claiming a retry "owns the terminal".
5. T1 wakes, `monitor_stands_down` sees `current.pid() != handle.pid()`, stands
   down, returns `Ok(())`, slot drop clears the entry.

**Lease 2's terminal is never recorded.** Run projects `Running` forever
(invariant 4 violated — the projection is wrong until node restart), every
`await_result` caller and parked child watcher stranded for the epoch, nothing
logged above `debug!`. `handle_process_exit_async` returned `Ok(())`.

Window = T1's remaining backoff. 8 ms today — **seconds once Waffles' 30 s
ceiling lands**, which makes this worse, not better.

`ArmOutcome::AlreadyArmed`'s doc ("The work IS owned… **Nothing is lost**") is
false for this map: the incumbent can be a *superseded* lease's retry about to
stand down rather than write.

**FIX DIRECTION (mine, from the reviewer's two options): key the map on
`(WorkflowId, RunId, Pid)`.** Reasoning: the map's job is one retry per
**WRITER**, and the writer identity is the lease, not the run. Two armed retries
under different pids is safe — the older stands down by the pid check that
already exists; exactly one writes. And the same lease arming twice still
collides on the same key, so the original double-writer fix is preserved.

### B2 — the retry loop reports a terminal as recorded when nothing was recorded

`lifecycle/completion_retry.rs:565-573`. `handle_process_exit_attempt` returns
`Ok(())` from the stand-down early return (`completion.rs:397-399`) — the path
where the monitor deliberately wrote **nothing**. The `Ok` arm then emits
`"workflow completion recorded after a transient durable failure"`, one line
after `monitor_stands_down` emitted "abandoning… no longer this monitor's to
write". Directly contradictory; an operator grepping the success line concludes
the completion landed.

Violates this lane's own rule at `completion_retry.rs:365` — "🔴 Every line below
reports what was OBSERVED, never what was assumed". **No new mechanism needed:
`progress` is already carried and is `NotRecorded` on that path.**

---

## MAJORS

- **M1 — `Drop for Engine` leaves an armed durable writer running.**
  `engine/api.rs:88-106` aborts the visibility task and gates the epoch but does
  **not** call `shutdown_timer_wheel()`. Live-wheel timer tasks are spawned on
  the host runtime (`nif_timer_bridge.rs:298-312`), record durable `TimerFired`,
  hold `Weak<EngineNifState>` — and this `Drop` deliberately does not clear the
  seams, so the upgrade succeeds. `Engine::shutdown` at `api.rs:464-471` spells
  out the exact failover hazard. **`shutdown_timer_wheel` is non-blocking**
  (`nif_timer_bridge.rs:154-165` = DashMap drain + `abort()`), structurally
  identical to the `visibility_reconciliation_task.abort()` `Drop` already does —
  so the async-context objection does not apply. Invariant 3, on the drop path.
- **M2 — the `remove_completion_retry` identity check is justified by a race
  that cannot occur.** `engine_tasks.rs:278-290`. Verified against vendored
  `tokio-1.53.1/src/runtime/task/harness.rs` (lines 334, 503, 549): tokio drops
  the future — and with it `CompletionRetrySlot`, running the release — strictly
  **before** setting COMPLETE, on every path (ready, cancel, shutdown, panic).
  So `arm` can never see `Occupied` + `is_finished()` with the release
  outstanding. **Keep the check** (cheap, correct, `pub(crate)` API could gain a
  caller) but rewrite the justification to say it is defence in depth.
- **M3 — the drain-failure enumeration is incomplete and its shared-precondition
  claim is false.** `runtime/handle.rs:528-536`. `close_and_join_all`
  `??`-propagates the drainer thread's own `Result` (`process_exit.rs:320`), so
  `ProcessExitEventStreamDisconnected` (`process_exit_drainer.rs:44`) and
  `ProcessExitOutcomeMissingAfterEvent` (`process_exit.rs:379`) both reach
  `shutdown`'s return. Neither is timeout-, poison- or panic-shaped; neither
  needs a stalled/poisoned/dead worker thread. **Eleven variants presented as
  the set is not the set** — the same defect review 7 rejected, re-landed longer.
- **M4 — the sentence review 7 rejected survives verbatim at a second site.**
  `engine/api.rs:1127-1129` still says "every drain failure in here is
  timeout-shaped". BG records this as FIXED — it was fixed in `handle.rs` only.
  `grep -rn "timeout-shaped" crates/` = exactly 2 hits.

## MINORS

- **m1** — `arm`'s undo removes by key, not by handle (`engine_tasks.rs:256-261`);
  can abort a *different* arm's live handle after that arm told the operator the
  work was owned. No durability loss (epoch is closing) but the log is wrong.
  Same `handle.id()` comparison as `remove_completion_retry` fixes it.
- **m2** — `CompletionRetryConfig` validation is reachable only via
  `EngineBuilder::build`; `RuntimeHandle::new` is `pub` and bypasses it. Narrow
  the doc claim, or move the check.
- **m3** — the unbounded/unreachable retry default. **ALREADY RULED** — see
  `RULING-COMPLETION-RETRY-BOUND-2026-08-06.md`. Not an open item.

## FALSE DOCS (F1-F6)

F1 `api.rs:1127-1129` · F2 `handle.rs:528-536` · F3 `engine_tasks.rs:278-286` ·
F4 `api.rs:99-100` (drop's completeness claim) · F5 `engine_tasks.rs:52-54`
(`AlreadyArmed` "Nothing is lost") · F6 `completion_retry.rs:571`.

## TESTS — reviewer's verdicts

VALID: `one_run_can_never_hold_more_than_one_completion_retry` (independently
re-derived), the engine-drop test, `a_failing_teardown_step…`,
`shutdown_closes_the_epoch…`, both `embedded.rs` tests.
**NOT VACUOUS BUT MEASURES AN UNREACHABLE INPUT:**
`a_release_from_a_foreign_task_cannot_evict_a_live_completion_retry` — its doc
claims it pins a production hazard; per M2 it does not. Correct the doc.
**GAP:** the engine-drop test cannot see M1 — no wheel timer armed in the fixture.

---

## 🔴 WORK ORDER

1. **B1** — re-key completion retries by `(WorkflowId, RunId, Pid)`. Mutation:
   restore the `(W,R)` key and require the new reopen test to go RED.
2. **B2** — stand-down must not log "recorded". Mutation: restore the
   unconditional `info!`.
3. **M1** — `shutdown_timer_wheel()` in `Drop`; extend the drop test with an
   armed wheel timer so the gap closes. Mutation: delete the call.
4. **M2/F3, M3/F2, M4/F1, F4, F5, F6** — doc corrections, each verified against
   the cited source line, not against memory.
5. **m1** — undo by handle id. **m2** — narrow the claim.
6. **Waffles' ruling O1-O5** (`RULING-COMPLETION-RETRY-BOUND-2026-08-06.md`):
   30 s ceiling, no budget (deliberate, reasoned in code), ceiling-reached
   `warn!` carrying attempt count + elapsed + last error class, + a
   mutation-proven test. **No separate throttle** — the rate IS the ladder.
7. Full unfiltered battery + workspace clippy at the FINAL tree.
8. **Review 9 on Opus.** Then commit + push.
9. Tell Tom: reviews 6, 7 AND 8 all refused; 7 found a real double-writer bug,
   8 found a real silent-zombie path; my first proof of the 7 fix was itself
   defective and was rebuilt.

## STANDING

All constraints from BG carry unchanged. Tom's pids untouched. Worker rebuilds
frozen. NO WORKTREES. Never `git add -A`. Waffles: nothing further until my
landing report.

---

# PROGRESS LOG (append-only, latest at bottom)

## Done and proven

- **B1 FIXED.** `CompletionRetryKey { workflow_id, run_id, monitor_pid }` replaces
  the `(WorkflowId, RunId)` tuple in `runtime/engine_tasks.rs`. Threaded through
  `arm_completion_retry`, `remove_completion_retry`, `CompletionRetrySlot`, and
  `lifecycle/completion_retry.rs::arm_completion_retry` (which reads
  `handle.pid()`). All log sites carry `monitor_pid`.
  - **NEW TEST** `a_reopened_runs_successor_lease_can_arm_its_own_completion_retry`
    (engine_tasks.rs). Three assertions: `assert_ne!` control that the leases
    differ; same-lease second arm still `AlreadyArmed` (the guarantee the pid must
    not have weakened); different-pid arm is `Armed`; count == 2.
  - **MUTATION (faithful — the literal prior key): delete `monitor_pid` from the
    struct. RED.** Control fired first ("the two leases must differ", left==right).
    Then, with the control temporarily inverted so the decisive assertion was
    REACHED, it also fired: `left: AlreadyArmed, right: Armed`. **Both assertions
    proven load-bearing.** `one_run_can_never_hold_more_than_one_completion_retry`
    stayed GREEN under the same mutation — pairwise separation preserved.
    Restored, `shasum -c` OK on both files, both green.
- **B2 FIXED.** `complete_process_exit` now returns
  `Result<TerminalProgress, CompletionFailure>` (it already had the value and was
  discarding it). The retry loop matches `Ok(Recorded)` vs `Ok(NotRecorded)` and
  no longer says "recorded" on the stand-down path.
- **M1 FIXED.** `Drop for Engine` now calls
  `self.runtime.nif_state().shutdown_timer_wheel()` before `begin_close()`.
  Doc explains why it is safe in a `Drop` (DashMap drain + abort, non-blocking).
- **m1 FIXED.** `arm`'s undo now retracts by HANDLE ID (`remove_if`), not by key.
- **F1** (`api.rs`) and **F2** (`handle.rs`) corrected. F2 now says the drain set
  is **OPEN** and names why: `close_and_join_all` `??`-propagates the drainer
  thread's own `Result`, so `ProcessExitEventStreamDisconnected` and
  `ProcessExitOutcomeMissingAfterEvent` reach the caller. Second shape claim at
  `handle.rs` "Errors" section corrected too. **F4** and **F5** corrected.
  **F6** is B2's fix. **F3** corrected in `remove_completion_retry`'s doc — now
  states it is defence in depth, with the tokio harness line numbers.
- **Waffles' O1/O2/O4 APPLIED:** ceiling `8ms` → **`Duration::from_secs(30)`**;
  `Default` doc rewritten to carry the ruling and its reasoning (no-re-driver
  argument, the somewhere-to-put-it precondition, the config-surface gap).
- **Waffles' O3 APPLIED:** at-ceiling loudness in the Retryable arm —
  `at_ceiling || first_sight`, `should_report` called unconditionally so the
  reporter's bookkeeping stays honest. Carries `attempts`, `elapsed_seconds`,
  `fault`, `at_ceiling`, `monitor_pid`. **No separate throttle** (comment says
  why, and that adding one would be an invented cap).

## STILL OPEN — do these next

1. **O5** — the test pinning the at-ceiling emission, mutation-proven by the
   LITERAL ABSENCE of the emission (not a mutant built from the new machinery).
   A `CaptureLayer` idiom already exists at
   `crates/aion/src/loader/declared_queues_tests.rs:35`. **Extract it to a shared
   `#[cfg(test)]` module rather than copying it** — a second copy is the
   one-truth-in-two-places disease.
2. **M1's test gap** — the engine-drop test cannot see M1: no wheel timer is
   armed in its fixture. Extend it, or add a sibling that arms one.
3. **M2 residual** — `a_release_from_a_foreign_task_cannot_evict_a_live_completion_retry`'s
   doc still claims it pins a production hazard. Per F3 it does not. Correct the
   doc (keep the test).
4. **m2** — narrow the `EngineBuilder::build` "boundary every engine passes
   through" claim: `RuntimeHandle::new` is `pub` and bypasses the check.
5. Full unfiltered `cargo test -p aion-rs` + `cargo clippy --workspace
   --all-targets -- -D warnings` at the FINAL tree.
6. **Review 9 on Opus.** Then commit + push.

## PROGRESS LOG — tick 2026-08-06T11:0xZ

### O5 DONE AND MUTATION-PROVEN

- Extracted the `CaptureLayer`/`EventFields` idiom to **`crates/aion/src/log_capture.rs`**
  (`#[cfg(test)] pub(crate) mod log_capture`), retired the copy in
  `loader/declared_queues_tests.rs`. One place, not two.
- New test `at_the_ceiling_every_failing_attempt_states_itself`
  (`lifecycle/completion.rs`). Flat 1 ms ladder so `at_ceiling` holds from
  attempt 1; `fail_next_reads(64)` → 64 failing attempts measured.
  Asserts the stated attempts are **contiguous 1..=N**, N ≥ 3, and each line
  carries `at_ceiling=true`, `elapsed_seconds=`, `fault=`.
- `CompletionRetryContext::downgrade` widened to `pub(super)` with the reason
  written in: a thread-scoped capture cannot see the executor-thread spawn, so
  the proof calls the loop directly with `arm_completion_retry`'s own arguments.

| mutation | verdict |
|---|---|
| **A — delete the `tracing::warn!` outright (literal absence)** | **RED** — 0 statements; killed at the vacuity control |
| **B — `if at_ceiling \|\| first_sight` → `if first_sight` (the prior condition, new fields KEPT)** | **RED** on the decisive assertion — `left: [1,2,4,8,16,32,64]` vs `right: [1..7]` |

B is the isolating one: the fields were left in place, so only the contiguity
assertion could have killed it. `[1,2,4,8,16,32,64]` over 64 attempts is exactly
the silence O5 closes — at a 30 s ceiling, attempts 33..63 are ~15 minutes of
nothing.

Restore proven by `shasum -c` both times; green re-run after.

### STILL OPEN
1. M1's test gap — `dropping_an_engine_without_shutdown_closes_the_completion_retry_epoch` arms no wheel timer, so it cannot see M1.
2. M2 residual — `a_release_from_a_foreign_task_cannot_evict_a_live_completion_retry`'s doc still claims a production hazard F3 says it does not pin.
3. m2 — narrow the `EngineBuilder::build` "boundary every engine actually passes through" claim (`RuntimeHandle::new` is `pub` and bypasses it).
4. Full unfiltered `cargo test -p aion-rs` + clippy at the FINAL tree, then review 9 on Opus, then commit + push.

## PROGRESS LOG — continued, all four open items CLOSED

### M1's test gap CLOSED
New sibling `dropping_an_engine_without_shutdown_disarms_the_live_timer_wheel`
(`engine/builder.rs`). Arms a real wheel timer through the production path
(`TimerService::schedule` over a `Resident` registry handle, fire_at +3600 s so
nothing fires or self-disarms), reads the count through an
`Arc<EngineNifState>` held ACROSS the drop, control asserts 1 before.
New `#[cfg(test)]` observable: `EngineNifState::armed_wheel_timers` →
`TimerNifBridge::armed_wheel_timers` (`pending_timers.len()`).

| mutation | new test | pre-existing sibling |
|---|---|---|
| delete `self.runtime.nif_state().shutdown_timer_wheel();` from `Drop for Engine` | **RED** (`left: 1, right: 0`) | **GREEN** |

The sibling staying green IS the reviewer's finding, confirmed at my hand.

### M2 residual CLOSED
`a_release_from_a_foreign_task_cannot_evict_a_live_completion_retry`'s doc
retracts the reopen justification (the pid in the key makes those two DIFFERENT
keys) and states the narrower mechanism that survives: one lease arming again
after a retry that ended without recording, `Occupied`+`is_finished` replace,
drop-guard release landing after. Test kept — it still pins a real check.

### m2 CLOSED
`check_completion_retry`'s "boundary every engine actually passes through"
narrowed to "every engine BUILT THROUGH THIS BUILDER", naming
`RuntimeHandle::new` (`pub`, carries its own `completion_retry`) as the bypass.

### A REAL RED THE RULING CAUSED, AND ITS REPAIR
`the_configured_backoff_ladder_governs_when_a_retry_lands` went RED on its own
guard: *"only decisive while the configured first sleep exceeds the ENTIRE
default ladder; if the default moves, move this value."* Waffles' ruling moved
the ceiling 8 ms → 30 s. The guard fired honestly.

It was **two claims collapsed** into one inequality that was cheap while the
ceiling was 8 ms. Separated: (a) the configured sleep must tower over the
default FIRST BACKOFF — the only inherited value a single-refusal fixture can
reach, since `sleep_backoff` sleeps the current value before doubling; (b) the
terminal must still land, which rules out a larger inherited value. Guard
restated against the first backoff. Restating it as "the whole ladder" would
force a >30 s test to decide a property one sleep settles.

🔴 An updated assertion needs its own proof, so the repair was mutated:
`arm_completion_retry`'s `context.runtime.completion_retry()` →
`CompletionRetryConfig::default()` (the literal defect). **RED — and it died on
the DECISIVE assertion at `completion.rs:1916`, not on the repaired guard.**

### GATES AT THE FINAL TREE
- `cargo test -p aion-rs` unfiltered — **1109 passed / 0 failed, EXIT 0**
- `cargo clippy --workspace --all-targets -- -D warnings` — **EXIT 0**
- First battery run had ONE contended red (`outbox_e2e::outbox_failure_fails_
  fast_and_cancels_unresolved_siblings`, 20.13 s under whole-battery load vs
  **0.68 s** for the whole target in isolation — a 30× slowdown, the contention
  signature). 8/8 green in isolation, and 1109/0 on the clean re-run. Nothing in
  this lane touches the outbox fan-out path.
- `cargo test --workspace` running in background → `battery-ws.log`.

### NEXT
Review 9 on Opus against the frozen tree, then commit + push (files named
explicitly — never `git add -A`).
