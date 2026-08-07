# SEAT RESUME BV — 2026-08-07 — supersedes BU

🔴 **THIS IS THE CURRENT AUTHORITY.** Latest letter wins.

## STATE IN ONE LINE

**The lane is COMMITTED (3 commits, NOT pushed) at Tom's explicit ask.** Battery
green and reconciled. Review 17 REFUSED with 4 blocking findings; **3 of 4 are
now closed with code**, B3 remains. Tom is walking the tutorial live.

---

## §1 — BATTERY: GREEN AND RECONCILED

`cargo test --workspace --no-fail-fast`, log `scratchpad/bu-workspace.log`:

- **223 declared targets + 20 doc-test sections = 243 units**
- **247 verdict lines · ZERO-VERDICT TARGETS: NONE · 4218 passed / 0 failed / 0 ignored · RC_WS=0**

The 4 extra verdicts (247 vs 243) were CHASED, not waved through — an unexplained
surplus is exactly where a crashed binary hides behind a nested verdict:
- `aion_server` lib target emits 3 extra, each reading **"796 filtered out"** —
  self-invoking child runs — and still emits its OWN verdict, 797 passed /
  0 filtered out (log line 4742). Nothing hid behind them.
- `Doc-tests aion` splits merged doctests (2) from its `compile_fail` doctest (1).

4215 → 4218 is exactly the three tests added. Reconciler is
`scratchpad/reconcile-bu.py` (walks in order, opens on `Running`, closes on
`test result:`, reports units with ≠1 verdict).

## §2 — COMMITTED AT TOM'S ASK (NOT PUSHED)

Tom asked "Have we pushed the latest to main?" then "I need it all committed".
Answered: 0 unpushed commits, but ~8,400 lines uncommitted. Verified
`cargo check --workspace --all-targets` = 0, ran `cargo fmt --all`, then:

- **bed958b0f** — `docs(146)` tutorial fix
- **2848ddb0a** — `fix(7)` the engine lane
- **5775b58a4** — `docs(seat)` markers/briefs

`crates` + `docs` clean at that point. **DELIBERATELY NOT PUSHED** — told Tom
plainly why (unpassed review), recommended finishing + pushing, offered "say
push it and I will immediately". **Silence-default I stated: push once clean.**
🔴 If he said "push it", that overrides — check the DM.

## §3 — REVIEW 17 (Opus, read-only): REFUSED, 4 BLOCKING

| # | finding | status |
|---|---|---|
| B1 | gate 1 claimed "decisive under the recorder lock" — FALSE | ✅ CLOSED |
| B2 | refused-cancel remedy false for teardown cancels | ✅ CLOSED, mutation-proven |
| B3 | `api.rs` Drop enumeration says FOUR writers; workflow process is a 5th, ungated | ❌ **OPEN** |
| B4 | tutorial "run it five times" — really three | ✅ CLOSED |
| N1 | `nif_timer_bridge.rs:172` names a shard-relinquishment caller that doesn't exist | ✅ CLOSED |
| N2 | gate 2: one read guarding three writes | ✅ CLOSED |
| N3 | `WorkflowDeadlineHandler::new` is pub, takes an arbitrary flag | ✅ CLOSED |
| N4 | 2 new tests lack `runtime.shutdown()?` | ✅ CLOSED |

### N1/N3/N4 — CLOSED 2026-08-07, bridge tests 8/8, `cargo check --workspace --all-targets` RC=0

- **N1** verified: `shutdown_timer_wheel` has exactly TWO production callers,
  `api.rs:179` (`Drop`) and `api.rs:594` (`shutdown`); `relinquish` appears
  nowhere in the crate outside doc prose. The old line named a caller that has
  never existed AND omitted the one that makes the gate load-bearing. Rewritten
  to name both, with the verified asymmetry: `Engine::shutdown` runs
  `runtime.shutdown()` (which stops the scheduler) at `api.rs:584` BEFORE the
  wheel teardown at `api.rs:594`, so there a drain would have sufficed; `Drop`
  leaves the scheduler alive, so there the flag must keep refusing.
- **N3** verified reachable from OUTSIDE the crate: `lib.rs:56 pub mod
  lifecycle` → `lifecycle/mod.rs:12 pub mod deadline` → `pub struct` → `pub fn
  new`. So `register_deadline_handler`'s claim to be "the one place the flag can
  come from" was true in-crate and FALSE at the public boundary — a downstream
  consumer could build a handler around a flag nothing ever sets. Narrowed to
  `pub(crate)`; the workspace check passing is the PROOF no external caller
  existed, not a census.
- **N4** both tests now end `runtime.shutdown()?;` like every sibling.

### B1 — CLOSED (verified at my own hand)
`shutdown_timer_wheel` (`nif_timer_bridge.rs:188`) = atomic store + DashMap drain
+ abort. **Neither it nor either caller (`api.rs:179` Drop, `api.rs:594`
shutdown) takes a recorder lock.** So the lock excludes other recorder writers
and excludes NOTHING about the flag; `read_history().await` sits in the window.
Fixed by BOTH: told the truth in the comments AND narrowed —
- added `fn stood_down(&self)` (names flag+ordering once)
- re-read before `retire_run_deadline` (LoseCleanly repair) and before
  `record_workflow_timed_out`
- gate 2: re-read before each of the three writes; abandoning part-way is safe
  BECAUSE the deadline is retired last
- corrected the "one critical section" doc — stand-down removed from that list
- 🔴 the SAME false "decisive" claim existed in **two more places** in
  `nif_timer_bridge.rs` (the blocking-future comment and the `shut_down` field
  doc). Both corrected. What IS decisive is the arm/insert `SeqCst` composition.

### B2 — CLOSED, MUTATION-PROVEN
`Engine::cancel` (`api_workflow_ops.rs:243-257`) issues
`TimerCancelCause::CancelTeardown` cancels through the same boundary. "the run
reissues it there" is FALSE there — the run is terminal, never re-executes.
- new `RefusedAppend { Fire, Cancel(TimerCancelCause) }` replaces `cancelled: bool`
- `WheelTornDown { timer_id, refused }`; `into_seam_error` matches all 3 arms so a
  new cause won't compile until someone words it
- new `wheel_torn_down_before_teardown_cancel` — remedy **verified not assumed**:
  `fire_timer_guarded` puts the owner's fire through the recorder seam which
  refuses it as `RecordOutcome::RefusedTerminal`, recording nothing
- new test `a_refused_teardown_cancel_does_not_promise_a_reissue` — control first,
  asserts absence of "reissues it there" PAIRED with presence of "will not
  re-execute" and "post-terminal"
- **Mutation:** collapse both Cancel arms onto `wheel_torn_down_before_cancelling`
  → **exactly one RED**, the new test, sibling green. Mutant message was the
  defect verbatim. Restored byte-identical:
  `nif_timer_fire.rs` = `1ec6c04a83d050fba5b4fe65bb4edcb450f91a1c9cc3f56c0a1755713072a29d`
- **8/8 bridge tests green after restore.**

### B4 — CLOSED
Deck said "You will run it five times". Reader runs it **three** (gate-work-3,
-4, -5). `repo_gate-1-skeleton` only shown; `repo_gate-2-contract` only
`aion awl check`. `staged_document_count()` counted FILES; its docstring said
"how many times the reader runs" — the docstring was where the lie lived.
Reworded to "build it up through five complete documents … each one checks clean".
🔴 My FIRST rewording said "checked, then run" — false for docs 1-2, i.e. I
reintroduced the defect while fixing it. Caught and corrected before rebuild.
Also derived `MOTIONS` so "the same five motions" counts its own list.
Deck rebuilt, all gates green. **Tom was told the correct number.**

## §3b — B3 CONFIRMED AT MY OWN HAND (2026-08-07), AND WORSE THAN REPORTED

Verified, each at the source:
- `NifContext::block_on_recorder` (`nif_context.rs:239-253`) — no epoch check,
  no stand-down check.
- `Recorder::append_with`/`append_one` (`durability/recorder.rs:1002-1022`) —
  straight to `store.append`. **No gate inside the Recorder at all.**
- `is_epoch_open` has exactly TWO production call sites in the whole crate:
  `nif_activity_retry_dispatch.rs:498` and `completion.rs:304`. It is NOT in
  the Recorder.
- `sleep` `ResumeLive` (`nif_timer.rs:105-111`) — `record_started` (durable
  `TimerStarted`) then `schedule_sleep_timer`; `TimerService::schedule`
  (`time/timer_service.rs:141-153`) writes `store.schedule_timer` FIRST and
  only then calls `arm_timer`, which is where the refusal lives. So TWO durable
  writes land before the refusal. **On inspection this ordering is CORRECT by
  design** — durable-row-then-arm, and an unarmed durable row is precisely what
  recovery re-arms. That half of the finding is not a defect.
- 🔴 **THE REAL DEFECT, WHICH THE REVIEWER DID NOT FIND:**
  `nif_continue_as_new.rs:66-104` records `WorkflowContinuedAsNew` — a
  **TERMINAL** event — through `block_on_recorder` with no epoch/stand-down
  gate. A terminal is not repairable by recovery. `executor.rs:267-272`
  (`Command::CompleteWorkflow` → `record_workflow_completed`) and
  `resolver.rs:194` (`record_workflow_failed`) are candidate siblings, ROUTES
  NOT YET TRACED.
- `WriteToken` (`aion-store/src/store.rs:11-36`) is a bare capability marker
  with `_private: ()` and NO identity — **it fences nothing**. The only runtime
  defence is `SequenceConflict`.

Enumeration agent dispatched (Opus, read-only) for the complete NIF-surface-
derived route map + gate-placement analysis before any code is written. Do not
write the fix from my partial list.

## §4 — WHAT IS LEFT, IN ORDER

1. **B3** — `engine/api.rs:130-155` `impl Drop for Engine` claims "no durable
   writer this process can still reach keeps writing. There are FOUR…". The
   reviewer says the 5th is the workflow process itself: Drop deliberately leaves
   the beamr scheduler + seams alive, `NifContext::block_on_recorder`
   (`nif_context.rs:239-253`) has NO epoch/stand-down check, and `sleep`
   (`nif_timer.rs:105-111`) does `record_started` (durable `TimerStarted`) then
   `schedule_sleep_timer` (durable timer row) **before** `arm_timer` refuses.
   🔴 **VERIFY THIS MYSELF FIRST** — the reviewer has been right 3/3 so far but
   this is the biggest claim. If true: either name the 5th writer and say nothing
   stops it, or narrow the bolded claim to *background* writers.
2. **N1, N3, N4** — cheap, do them with B3.
3. `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
   **re-run the full battery** (tree changed since the green one).
4. **Review 18.** Reviews 6–17 have ALL refused.
5. **Commit the B1/B2 work** (uncommitted since 2848ddb0a) + push per §2.
6. Teardown owed (BT §9) — still held.

## §5 — RESTORE ARTEFACTS (scratchpad)

`fire-restore-b2.rs` + `fire-b2.sha` (B2), `reconcile-bu.py`, `review-17-brief.md`,
`msg-a/b/c.txt`, plus BU's list.

## §6 — STANDING CONSTRAINTS

Unchanged — **BT §7** in full. Plus: no worktrees, one mutating lane, never
`git add -A`, no `git gc`/prune, delete nothing, Tom's pids untouched, worker
rebuilds frozen, don't edit CLAUDE.md, `-- crates docs` scoping.
