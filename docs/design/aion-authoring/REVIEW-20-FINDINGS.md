# Review 20 — REFUSED. Working list.

Opus sub-agent, 2026-08-07, over the whole uncommitted diff. Full text:
`tasks/a87845d7405b1a1da.output` (JSONL — do not cat it into a context).

**Accepted in substance:** Review 19's A and B ARE closed. `ContinuationOutcome` is the
right shape; `outcome_must_end_the_process` is total and correct on every reachable input
(reviewer independently verified `Recorder::append_one` can only fail before a store
append, so `Err` genuinely means nothing durable landed); the live test genuinely reaches
the NIF gate; the `.beam` matches its `.erl` (`beam_lib:cmp/2`, differs only in `Dbgi`);
the test is stable (40× sequential, 36× at 12-way parallel, 0 failures).

## BLOCKING

- [ ] **B1 — `cargo fmt --check` FAILS, EXIT 1**, five diffs, all in
  `nif_continue_as_new_tests.rs` (142, 200, 372, 577, 672). CLAUDE.md names it beside
  clippy. Marker CB §5's "ALL GREEN" table omits the fmt gate entirely — a claim over a
  proper subset of the required gates. Fix: `cargo fmt` (WRITE, per standing preference —
  never format-check), and add the fmt row to the table.
- [ ] **B2 — the `TerminalOnly` operator message names a mechanism the deadline path does
  not use** (`nif_continue_as_new.rs:136-162`). I claimed it was "read off the deadline
  path, not assumed". Three errors:
  1. `finalize_timed_out_without_handle` is NEVER REACHED. `fire_timer_guarded`
     (`time/timer_service.rs:256-258`) returns `Ok(())` before any demux when
     `timer_is_live` is false, and `live_timers_in_active_segment` (`:369-387`) scans
     FORWARD from the last `WorkflowStarted` — so once the successor's `WorkflowStarted`
     lands, the predecessor's deadline is outside the active segment. The real reason the
     row is harmless is the active-segment liveness guard, which my comment never mentions.
  2. "whole-history recovery re-arms it on every engine start" is FALSE past the deadline
     instant — `outstanding_future_timers` (`time/recovery.rs:170`) filters `fire_at > now`.
     What re-produces an overdue row is `recover_due` → `store.expired_timers`, the durable
     timer table.
  3. The stated remedy DOES NOTHING. No store implementation ever deletes a timer row
     (`aion-store/src/memory.rs:423-460`; no `DELETE FROM timers` in libsql or haematite),
     so `expired_timers` keeps returning it; and the event lands in the successor's active
     segment where the id is already not live.
  Fix: rewrite against `live_timers_in_active_segment` + `expired_timers`. Honest
  statement: the row is permanently present in the timer table and permanently inert once
  the successor starts, and NO operator action removes it.
- [ ] **B3 — both operator messages reach NO operator.** `error_result_term` /
  `context_error_term` build a term and nothing else; there is no `tracing::` call in
  `nif_continue_as_new.rs`, `nif_context.rs` or `engine_nifs.rs`; and `cancel_pid` killed
  the calling process two statements earlier, so the term returns into a dead process and
  is discarded. The branch whose stated justification is "the alternative is silence"
  ships silence — CLAUDE.md "no silent failures" head-on. Same for the epoch refusal's
  nine lines of guidance (`record_continuation` `:363-372`).
  Fix: `tracing::error!`/`warn!` with `workflow_id`, `run_id`, deadline timer id, at the
  `TerminalOnly` branch AND at the epoch refusal BEFORE `cancel_pid`; correct both
  comments to say where the text actually goes.

## MAJOR

- [ ] **M4 — assertion message says "read failure"; the armed fault is an APPEND failure**
  (`nif_continue_as_new_tests.rs:654-658` vs `:641`). Survivor of the withdrawn draft, and
  the red comment directly above it says the opposite. My error, today.
- [ ] **M5 — `engine/api.rs:257` still says "refusing cost nothing that was not already
  lost"** — the exact claim `nif_continue_as_new.rs:261-265` exists to retract, and
  `api.rs:211` already contradicts it 46 lines earlier. Finding E was supposed to fix this
  doc comment and left the retracted sentence standing.
- [ ] **M6 — CONTROL 2 claims a one-variable comparison it does not make.** It differs
  from the treatment in FOUR ways (different `RuntimeHandle`, different `Registry`,
  `FlakyStore` vs `InMemoryStore`, injected fault). And CONTROL 1 cannot see the failure
  that made the previous incarnation vacuous — under the broken harness `park_only` LIVED
  and only calling processes died, so a harness where every NIF-calling process dies
  passes CONTROL 1 unchanged. All the attribution rests on CONTROL 2, the one with the
  confounds. Fix: run CONTROL 2 on the treatment's runtime/registry before
  `begin_close()`, or state plainly why cross-runtime is tolerable.
- [ ] **M7 — CONTROL 2 arms its fault AFTER the starting gun.** `register_live_run`'s own
  docstring (`:492-496`) states the rule; `fail_next_appends(1)` is armed at `:641`, after
  `register_live_run` returns at `:639`. The fixture polls every 5 ms; if it wins the race
  the call runs unfaulted and the test fails blaming the instrument. The correct tool is
  already used by the sibling test at `:377`: `fail_appends_after(1, 1)` armed BEFORE
  `register_live_run`.
- [ ] **M8 — CONTROL 2's survival is checked ONCE; the treatment's death gets 10 s.**
  Under the `if true` mutation the witness (budget drained) fires strictly BEFORE
  `cancel_pid` → `cleanup_exited_process` (synchronous). So M7's RED is probabilistic, not
  structural. Fix: `assert!(!settles(|| !is_live(spared)), …)` — same budget both ways.
- [ ] **M9 — README's #107 row does not reproduce.** Recorded 479; reviewer's run of the
  document's own command gives **460**. Worse, **104 of the 460 are git-ignored build
  artefacts** (`crates/aion/tests/fixtures/*/build/packages/gleam_*`), so the number is a
  function of build state, not a property of the repo. Every OTHER row reproduced exactly
  (77, 42, 1, 2, and all four zeros — reviewer checked all ten). Fix: re-run, record the
  artefact contamination, or scope the control to tracked sources.

## MINOR

- [ ] **m10** README worked example (`:98`) uses `"control_symbol": "pub"`; every table row
  uses `pub fn`. A reader following it gets a number appearing nowhere.
- [ ] **m11** `unspent_append_failures` doc overclaims — true of CONTROL 2's harness, false
  as a property of `FlakyStore`, which any appender can drain. State it as a property of
  the setup.
- [ ] **m12** `store_faults.rs` module doc still enumerates two consumers and calls itself
  a retry-path fixture; continue-as-new is a third and is not a retry path.
- [ ] **m13** Source comment cites untracked CB, which defers to untracked CA. Resolves
  only if both land.
- [ ] **m14** Same file gives two counts for one investigation — module doc says "seven
  process shapes", `:434` says "three process shapes".
- [ ] **m15** `nif_continue_as_new_tests.rs:257-259` asserts three predicate mutations and
  their outcomes with no recorded run, while the same file insists elsewhere that "the
  mutation was RUN rather than guessed". Run and record, or say they are reasoned.
- [ ] **m16** `settles`' 10 s budget is undocumented (`:535`), and finding M8 makes it
  load-bearing.

---

## B2 INDEPENDENTLY VERIFIED AT MY OWN HAND (2026-08-07) — the reviewer is RIGHT on all three

Checked rather than taken on trust, because a reviewer can be wrong too. All three sub-claims CONFIRMED:

1. **`fire_timer_guarded` (`time/timer_service.rs:250-258`)** — first statement is
   `if !self.timer_is_live(&workflow_id, &timer_id).await? { return Ok(()); }`, BEFORE the
   `is_deadline_timer` demux. So a non-live deadline never reaches `fire_deadline`,
   never reaches `WorkflowDeadlineHandler`, and **never reaches
   `finalize_timed_out_without_handle`**. And `live_timers_in_active_segment`
   (`:369-387`) computes `segment_start` with
   `rposition(|e| matches!(e, Event::WorkflowStarted { .. }))` and iterates
   `&history[segment_start..]` — i.e. FORWARD FROM THE LAST `WorkflowStarted`. Once the
   successor run's `WorkflowStarted` is appended, the predecessor's
   `TimerStarted(deadline:{pred_run})` is behind the segment start and is not live.
   ⇒ **My comment named a mechanism that is unreachable. The active-segment liveness
   guard is what actually makes the leftover row harmless, and I never mentioned it.**

2. **`outstanding_future_timers` (`time/recovery.rs:168-171`)** ends with
   `.filter(|(_, fire_at)| *fire_at > now)`. ⇒ **"whole-history recovery re-arms it on
   every engine start" is FALSE once the deadline instant has passed.**

3. **No store deletes timer rows.** Grepped `crates/aion-store/src`,
   `crates/aion-store-libsql/src`, `crates/aion-store-haematite/src` for
   `DELETE FROM timers` / `timers.remove` / `.timers…remove` — **zero hits in all three.**
   ⇒ **The remedy my message prescribes ("record `TimerCancelled` for `deadline:{run}`")
   clears nothing.** An operator following it performs a durable append that accomplishes
   nothing and still sees the row.

**The honest replacement message must say:** the predecessor's deadline row is
permanently present in the durable timer table (nothing deletes timer rows), and
permanently INERT from the moment the successor's `WorkflowStarted` lands (the
active-segment liveness guard drops it before any demux) — and there is **no operator
action that removes it**. `recover_due` → `store.expired_timers` is what keeps
re-surfacing an overdue row, not history re-arming.

## ORDER OF WORK (resume here)

M4 → M5 → m10/m11/m12/m14/m16 → M7 → M8 → M6 → **B2 rewrite (verified above)** → B3
(tracing) → m15 → M9 (re-run README #107 row) → **B1 `cargo fmt` LAST, after all edits**
→ re-run both clippy gates + suite to a FILE → Review 21.

⚠️ Marker CB §5's gate table must gain a `cargo fmt` row; "ALL GREEN" there is currently a
claim over a proper subset of CLAUDE.md's named gates.

---

# FIX PASS — ALL 16 WORKED, 2026-08-07 (same window)

Every box above is now done. Nothing was documented-and-accepted; each finding changed
the artifact it named. Where the finding was a wrong claim, the replacement was measured,
not re-asserted.

| finding | what changed |
|---|---|
| B1 | `cargo fmt` (write) over the whole tree; `cargo fmt --check` now **EXIT 0**. The gate table below carries the row. |
| B2 | `terminal_only_account()` — the message now states the row is PERMANENT (no store deletes timer rows; positive control: the deletion grep finds `.remove(`/`DELETE FROM` for packages) and INERT by two mechanisms in sequence (pre-successor: `finalize_timed_out_without_handle` returns for non-TimedOut; post-successor: `fire_timer_guarded`'s `timer_is_live` check drops the fire before any demux), that `recover_due` → `expired_timers` is the re-surfacer (not history re-arming), and that **no operator action removes it**. The old remedy is retracted inside the function's own doc, mechanism by mechanism. |
| B3 | `tracing::error!` (TerminalOnly) and `tracing::warn!` (epoch refusal) emitted **before** `cancel_pid`, carrying `workflow_id`, `run_id`, and (TerminalOnly) the deadline timer id rendered via `crate::time::deadline_timer_id`, not a pasted format. Both comments corrected: the `{error,_}` term is documented as returning into a dead process. One account function feeds both the log and the term, so the two cannot drift. |
| M4 | Assertion message now says "the injected APPEND failure was never spent". |
| M5 | `engine/api.rs` — the retracted sentence replaced with the stated price (deadline stays armed; a long outage times the run out) and an explicit note that the earlier wording was the exact claim the NIF doc retracts. |
| M6 | Live test restructured: ONE runtime, ONE registry, ONE `FlakyStore` for treatment + both controls; the two conditions differ only in why `record_continuation` refuses. The epoch is the one unsharable variable, so the conditions are ordered (control while open, treatment after `begin_close`), with CONTROL 1 spanning both and asserted last. The doc states what CONTROL 1 can and cannot see. |
| M7 | `fail_appends_after(1, 1)` armed **before** `register_live_run` (skipping exactly its one seeding append); the race is gone by construction. |
| M8 | `assert!(!settles(‖ !is_live(spared)))` — the control's survival gets the same 10 s budget as the treatment's death; `SETTLE_BUDGET` is one named constant documenting why both directions share it. |
| M9 | README #107 row re-run at my hand: **460** full-tree (reviewer's number reproduced exactly), of which **104** are git-ignored `fixtures/*/build/packages/gleam_*` artefacts; tracked-only control **356** via `git grep`, recorded as the durable owner. The claim itself (0 hits) is build-state-immune and stated so. |
| m10 | Worked example now uses `"control_symbol": "pub fn"`. |
| m11 | `unspent_append_failures` doc now states attribution as a property of the SETUP (no other appender inside the measured window), not of `FlakyStore`. |
| m12 | Module doc: retry-fixture claim replaced — three consumers, one of which (this gate) is explicitly **not** a retry path. |
| m13 | CA/CB/BZ + this file are committed with the lane (see below), so the citation chain is tracked. |
| m14 | The `:434` block now says SEVEN shapes, names the three that carry the argument, and points at CB §1 for the rest. |
| m15 | The three predicate mutations RUN (literal prior text, pinned baseline `3de2e869…`, green-baseline control first, each restored byte-identical): every-error-ends → RED (unit + live CONTROL 2); wrong-variant → RED (unit + live treatment); Ok-narrowed → RED (unit + TerminalOnly integration). Comment rewritten to claim exactly these runs. Logs: `scratchpad/m15-*.log`. |
| m16 | `SETTLE_BUDGET` constant with the full rationale, replacing the bare `10`. |

## GATE MUTATIONS RE-RUN AGAINST THE RESTRUCTURED TEST

CB §4's runs were against the old test shape, and a mutation result does not survive a
test restructure. All three re-proven against the new bytes (target
`if outcome_must_end_the_process(&recorded) {`, pinned baseline, byte-identical restores):

| mutation | result | failed test |
|---|---|---|
| `if recorded.is_ok()` — the reviewer's exact pre-remediation form | **RED** | the live gate test |
| `if false` | **RED** | the live gate test |
| `if true` | **RED in 0.72 s** — caught STRUCTURALLY at CONTROL 2's same-budget survival check, which is what M8's fix was for | the live gate test |

## GATES AT THE FIX-PASS CHECKPOINT — ALL GREEN, ALL FOUR

| gate | result |
|---|---|
| `cargo fmt --check` | **EXIT 0** (after `cargo fmt` write per standing preference) |
| `cargo clippy -p aion-rs --all-targets -- -D warnings` | **EXIT 0** |
| `cargo clippy --workspace --all-targets -- -D warnings` | **EXIT 0**, 0 warning/error lines |
| `cargo test -p aion-rs` (to a FILE, `scratchpad/r20-suite.log`) | **EXIT 0** — 60 verdict lines, **1130 passed / 0 failed / 0 ignored**, reconciled: 58 test binaries + 1 doc-test runner emitting two blocks |

---

# REVIEW 22 — REFUSED (1B/1M/3m). FIX PASS IN PROGRESS

Full verdict in the R22 task output (`tasks/ac0791a59bc6e9b0f.output`, JSONL — grep, don't cat).
Reviewer left tree byte-identical, both shasums proven, no `.orig` residue.

## R22 FINDINGS AND STATUS — ALL FIXED (development probes done; formal battery pending)
- [x] B1 (BLOCKING) — FIXED, and it grew: the live test now drives ALL FOUR predicate
      input classes through the NIF. `register_live_run` takes a `SeededDeadline`
      parameter (None | Outstanding, seeded via `deadline_timer_id` between the
      started-append and the registry insert). New conditions:
      · TREATMENT A `Ok(TerminalOnly)`: deadline seeded, `fail_appends_after(3,1)`
        (skip started+seed+terminal, fail the retirement); asserts fault SPENT, process
        DIES, `continued(history)`, `!retired_a_timer(history)`; diagnosis-carrying
        failure messages throughout.
      · TREATMENT B `Ok(Complete)`: deadline seeded, NO fault; asserts process DIES,
        `continued && retired_a_timer` (the history IS the witness — a completed
        transition writes, a refused one doesn't).
      · The epoch refusal is now TREATMENT C; test renamed
        `the_nif_gate_ends_a_live_process_for_each_outcome_that_must_end_it_and_spares_the_rest`.
      🔴 TREATMENT B EXISTS BECAUSE THE REVIEWER'S OWN REMEDY WAS INCOMPLETE AND I
      MEASURED IT: the Complete-skip mutation (`&& !matches!(..., Ok(Complete))`)
      SURVIVED the e2e (3/0 in 0.49 s — tests/continue_as_new.rs never drives this NIF)
      and the whole lib. The "e2e owns the Complete arm" belief was refuted at my hand;
      the doc comment states the refutation. Probe results (dev iterations, load ~110):
      TerminalOnly-skip RED (killed by TREATMENT A), Complete-skip RED (killed by
      TREATMENT B), both restored byte-identical, rebuilt tree green (5/0).
- [x] M2 (MAJOR) — all THREE sites rewritten with the true reason: a pre-terminal store
      fault leaves the run genuinely live (spared for the original reason);
      `HistoryShape` means an unsuperseded terminal is ALREADY durable — the run cannot
      be "turned into" a dead one — and it is spared because this seam does not OWN
      that teardown (terminate::cancel → cancel_pid; terminate::complete/fail →
      registry.remove; deadline handler cancels and deregisters LAST; a second
      cancel_pid here would race a teardown in flight and report success as failure).
      Sites: predicate doc, tests CONTROL 1 comment+message, api.rs.
- [x] m3 — lib.rs: "retry proofs" → "durable-refusal proofs — not all of their
      consumers are retry paths".
- [x] m4 — repair-chain bullet now says the handle "is still registered when the
      monitor performs the retirement — reconcile_terminal_registry runs after it and
      only SUSPENDS residency, it does not deregister".
- [x] m5 — "the paragraph 46 lines above" → named ("cost, stated because it is not
      zero" paragraph).

**New pinned baselines (post-fix, post-fmt, fmt-check 0):**
- `nif_continue_as_new.rs` = `8431b5729df2d773a47ba36ecf5aac3723b17a92`
- `nif_continue_as_new_tests.rs` = `938c41e70eefb5746ee32036a3cd3eb69bc552a8`
  (superseded `f71b0b93…` — see battery run 1's blocker below)
- also touched: `engine/api.rs` `5c85ee52…`, `lib.rs` `10d04200…`

**Formal battery:** `scratchpad/r22-battery.sh` — REFUSES an oversubscribed box (load >
ncpu), records load beside every gate and mutation, runs all four gates + suite ×2 +
EIGHT mutations (six prior + TerminalOnly-skip + Complete-skip), ends with the rebuilt
green re-run. Run it when the box is quiet, then dispatch Review 23.

## BATTERY RUN 1 (2026-08-07 ~20:16, pid 4959): REFUSED BY ITS OWN CLIPPY GATE — FIXED
Transcript `scratchpad/r22-battery.transcript`, logs `scratchpad/r22b-*.log`.
Preconditions passed clean (load 7.96/7.81/9.04 vs 10 cores, disk 56.6 GiB).
- 🔴 **clippy -p AND clippy --workspace EXIT 101**: `clippy::too_many_lines` — the
  four-condition live test hit 172/100 lines. My defect: I grew the test across two
  treatments and never re-ran clippy before pinning. **FIXED BY DECOMPOSITION, no
  `#[allow]`**: new `GateHarness` struct (one tokio runtime + beamr runtime + registry +
  FlakyStore, built once, lent to every condition — "one runtime, one registry, one
  store" is now STRUCTURAL, a condition cannot build its own world) with
  `spawn_continuer`/`register`/`history` helpers, and one function per condition:
  `control_two_spared_refusal`, `treatment_a_terminal_only`, `treatment_b_complete`,
  `treatment_c_epoch_refusal`. The `#[test]` is now an orchestrator that spawns/asserts
  CONTROL 1 and calls the four in latch order. Every assert message and load-bearing
  comment preserved verbatim; the spawn-timing rationale moved to `spawn_continuer`'s
  doc; CONTROL 1's "neither result above" corrected to "none of the results above".
  Post-fix dev checks: module 5/0 (10.37 s), `clippy -p` EXIT 0, fmt-check clean.
- Everything else in run 1 was green/red exactly as predicted: fmt EXIT 0; suite ×2
  both 60 verdict lines, 0 failures in any block (the #149 flake did not appear);
  mutation baseline 5/0; ALL EIGHT mutations RED with the predicted killer tests named
  in the failures block and byte-identical restores; final rebuilt-tree green 5/0.
- ⚠️ LOAD CAVEAT on run 1: the box was quiet at the precondition gate but another
  seat's build drove 1-min load to 27–64 from suite run 2 onward (recorded per step in
  the transcript). Per the estate polarity rule: the mutation REDs are structural and
  stand; the GREEN evidence (suites, final green) contains ceiling-shaped survival
  asserts and is pass-biased under load. The re-run on the decomposed tree is therefore
  not just for the new baseline — it must also catch a quiet window end-to-end.

## ⚠️ CONTAMINATION + INCIDENT (2026-08-07 18:15): MY OWN LOAD PROBE ORPHANED
Ten busy-loop shells from my pre-compaction contention probe (r21-load-6/7/8) outlived
their `kill $LOADPIDS` (subshells reparented to pid 1), ~50% CPU each for 1h57m; box hit
load 131/10 cpus. Killed by explicit verified pid at 18:15; load fell to 78 within a
minute. CONSEQUENCE: the R21 gate table's suite runs, the six-mutation session, AND
Review 22's own runs all sat inside the window. Structural results stand (mutation REDs,
the surviving mutation, shasums); green timing evidence is pass-biased for
ceiling-shaped asserts. Estate ruling (Cally 08:15Z): box exclusivity is a gate
precondition — ALL final gates re-run on a measured-quiet box with the load figure
recorded beside each result. Probe teardown law: kill by captured child pid, prove
death, record it.

## QUEUED BEHIND THIS LANE (accepted 2026-08-07 08:49Z)
- 🔴 beamr `loader/decode/etf.rs:120` — accumulator-rooting suspect, MY CLAIM (Artemis
  flagged, untouched). Read against AR-1 ground pack
  (docs/design/beamr/briefs/AR-1-GROUND-PACK.md @ 9587d2f); instrument FP rate 29% —
  refute with the ordering argument written out, or fix with the in-tree
  `with_rooted`/`rooted_push` pattern (root-as-you-go, or root-up-front +
  `_prereserved`). The tell: the terminal allocator roots its args but by then they're
  stale — check ORDERING, not presence. Not in AR-1's patch set; my verdict owed.

## RESUME STATE 2026-08-07 19:10 (post-compaction: START HERE)
- Disk floor CLEARED at my hand: removed `target/debug/incremental` (25.35 GiB, all
  from today's own builds — mtime could not separate stale, so the whole speed-cache
  went; correctness unaffected, mutation cycles now full-crate rebuilds). Free 23.21 →
  **48.59 GiB**. Also closed owed teardown: killed my scratch server pid 92685 (ports
  8098/50098 verified before firing, proven clear after), removed gate-clean-3/4/5/red,
  gate-work-3/4/5/red, tut-clean (+0.5 GiB). `gate-verify-bx` + pid 82997 stay until
  #154. Tom's pids untouched. Waffles took my teardown as the estate template; owe him
  the 48.59 number.
- `scratchpad/r22-battery.sh` is DUAL-GATED (3× load samples ≤ 10 cores AND disk ≥
  28 GiB) — disk now passes; run it when load ≤ 10 (was 19.6 falling at 08:59Z).
- BATTERY RUN 1 done: found+fixed the clippy line-budget blocker (see its section
  above); FT baseline now `938c41e7…`, script pin updated.

## BATTERY RUN 2 (2026-08-07 21:14–21:47 local, pid 52131): COMPLETE, GREEN WITH ONE
## ATTRIBUTED RED. Transcript `scratchpad/r22-battery.transcript`, logs `r22b-*.log`
## (run 1's archived as `r22b-run1-*`).
- Preconditions: load 4.38/4.07/4.04 vs 10 cores, disk 50.05 GiB. Entry collided with
  Athena's frame battery (hers 11:13:46Z, mine 11:14:47Z — both quiet-gates passed
  honestly; "is the box quiet" ≠ "am I the only one entering", now doctrine). Her run
  took ITS OWN harness TERM at 11:16:45Z (her measurement; my script contains no kill);
  the overlap covered only my fmt + clippy-p gates, both binary verdicts, both EXIT 0.
  Her sequencer then HELD BEHIND me for the rest of the run — box uncontended, verified
  at the process table (only cargo process = the battery's own suite; Tom's server at
  normal duty).
- Gates: fmt EXIT 0 · clippy -p EXIT 0 · clippy --workspace EXIT 0 (the run-1 blocker
  is dead, no allow anywhere).
- Suite run 1: EXIT 0, 60 verdict lines, 0 failures, **1130 passed**.
- Suite run 2: EXIT 101 — `outbox_out_of_order_completions_still_settle_with_n_terminals`
  (tests/outbox_e2e.rs, UNTOUCHED by the lane) failed with
  `Backend("SQLite failure: bad parameter or other API misuse")` — **task #85's known
  libsql 0.9.30 double-close defect**, same error face, mechanism verified in dep source
  on 2026-08-06. Tree verified clean (exactly the lane's 5 files + 2 untracked fixtures)
  before attributing. NEW data for #85: this reproduction is `-p aion-rs` on an
  UNCONTENDED box (load 10.25 at suite start, self-decay), so contention is not required.
  Fail-fast cut the run at 39 verdict lines — the count difference is cargo stopping,
  not tests vanishing.
- Suite run 3 — A MANUAL RUN OUTSIDE THE HARNESS, 21 minutes after the battery
  closed (Review 23 m1 caught the record implying otherwise): run at my own hand
  with load checked at entry (9.16, gate ≤ 10 applied by hand; load-after 21.57
  self-decay). EXIT 0, 60 verdict lines, 0 failures, **1130 passed** — the green
  pair is runs 1+3, totals identical. The battery transcript's per-step-load
  property covers runs 1 and 2 only.
- Mutation baseline 5/0. ALL EIGHT mutations RED (EXIT 101) with exactly the predicted
  killer tests named in each failures block; every restore byte-identical to
  `8431b572…`; loads 5.9–16.3 (mutation-tail values self-decay; reds are structural).
  The two R22 arm-skips each killed solely by the live gate test — TerminalOnly-skip
  and Complete-skip both RED in 20.1 s.
- FINAL sha = baseline; rebuilt-tree green 5/0; SESSION CLOSED GREEN at load 5.99.
- Caveat carried honestly: this battery ran under the 1-min-only load gate it started
  with. Athena's refinements (1-min AND 5-min vs cores; 15-min RETRACTED per Doug's
  142-reading zero-pass; mid-run disk = floor+margin not entry band) are banked for
  every FUTURE battery, not edited into a running script.

# REVIEW 23 — REFUSED (1B/0M/4m). FIX PASS IN PROGRESS
Full verdict in the R23 task output (`tasks/a99681274f8aac0ce.output`, JSONL — grep,
don't cat). Reviewer honoured box sequencing (waited out Apollo's haematite battery at
load 58, ran nothing until sole actor), left the tree byte-identical at all five pinned
shas, and INDEPENDENTLY corroborated the lane's core: their own Mutation X (reverting
the central `Err→TerminalOnly` fix) went RED in two tests with TREATMENT A's diagnosis
fields empirically confirming the fail_appends_after(3,1) arithmetic; R22's arm-skip
re-proven RED at their hand; full suite 60 lines 1130/0.

## R23 FINDINGS AND STATUS
- [x] B1 (BLOCKING) — my M2 replacement justification was wrong AGAIN, second attempt
      at that paragraph: named `terminate::complete`/`fail` as writers that "end the
      process" when (1) all six call sites sit inside `#[cfg(test)] mod tests` in
      api.rs (test-only, verified at my hand), (2) they never touch the pid — they
      `registry.remove` only (terminate.rs:64/:96), and (3) the enumeration OMITTED
      `lifecycle::continue_as_new` — the one LIVE writer that records this very
      terminal on the API path, with the successor already running while the
      predecessor is still registered (continue_as_new.rs:93→:150). Worse:
      completion.rs:213-220 already carries the correct FIVE-path enumeration
      (as REMOVERS) — one-truth-in-two-places drift, the exact estate law. FIXED at
      all three sites by SUBTRACTION: the predicate doc now POINTS at completion.rs's
      "Five ordinary terminal paths" paragraph as the single enumeration owner and
      states only the seam-local split — pid-enders (terminate::cancel, deadline
      handler; a second cancel_pid races them) vs deregister-only
      (lifecycle::continue_as_new, test-only complete/fail; a kill usurps them) —
      with the previous version's error named in place. tests:306 "three writers"
      count corrected; api.rs paragraph rewritten to the same split.
- [x] m1 — record wording fixed above: suite-3 marked as a manual post-battery run.
- [x] m2 — run 2's r22b-clippy-p.log was a CACHE REPLAY (72 bytes, "Finished in
      0.90s", no `Checking aion-rs` line) — the workspace gate did the real work and
      the reviewer re-ran clippy -p with real work, EXIT 0. Rule banked: a gate log
      must show the work happened (`Checking <crate>`), not just the exit code; the
      post-fix re-run below dirties the crate so its clippy -p is necessarily real.
- [x] m3 — ragged mid-paragraph doc breaks at api.rs:207/:268 reflowed (fmt does not
      reflow doc comments).
- [x] m4 — `GateHarness` now has a `Drop` backstop: a failing condition can no longer
      leak the beamr runtime for the rest of the binary. `torn_down: Cell<bool>` set
      by the explicit green-path `shutdown()` (which still propagates its error via
      `?`); the Drop runs only if that never happened and REPORTS a teardown failure
      on stderr rather than swallowing it (a Drop can neither propagate nor panic
      mid-unwind).
- Reviewer observation NOT charged to the lane, carried forward: `terminate::complete`
  and `terminate::fail` have NO production caller — a CLAUDE.md zombie-code question,
  pre-existing, to be filed after the lane lands.

## POST-R23-FIX GATES (2026-08-07 22:35–23:22 local; harness transcript
## scratchpad/r23-fix-gates.transcript covers through suite attempt 1; logs
## r23f-*.log): ALL GREEN
- Held behind Waffles' manifold suite + a beamr wasm probe before starting (process
  table, not load, decided entry). fmt --check EXIT 0 · module 5/0 · clippy -p EXIT 0
  WITH REAL WORK (`Checking aion-rs` present — the m2 rule applied) · clippy
  --workspace EXIT 0. Loads 5.0–9.6 throughout the gates.
- Suite attempt 1: EXIT 101 — #85 AGAIN, different test same binary
  (`outbox_happy_path_completes_with_n_terminals_in_order`, tests/outbox_e2e.rs, same
  `bad parameter or other API misuse` face). #85 task updated: 2 hits in 4 quiet-box
  `-p aion-rs` runs today — the outbox e2e binary is materially unreliable on main;
  libsql dep move queued behind this lane's Cargo freeze.
- Suite attempt 2 — A MANUAL RE-RUN OUTSIDE THE HARNESS at 23:22 (the transcript ends
  at attempt 1; same clause the battery's suite-3 earned): load 7.85 at entry checked
  at my own hand, no foreign cargo on the process table. EXIT 0, 60 verdict lines,
  **1130 passed / 0 failed**.

**Pinned baselines after the R23 fix pass (post-fmt, fmt-check 0):**
- `nif_continue_as_new.rs` = `5c36b302c066b7b68620f7b4f7d3ab9c120c258c`
- `nif_continue_as_new_tests.rs` = `c5adbd62bc4967060622ebf7b7f6e652a9d425a0`
- `engine/api.rs` = `10feca1918ba1a35425be4392be92ce50221d2d1`
- (lib.rs and store_faults.rs unchanged this pass: `10d04200…` / `7aae3983…`)

- NEXT: dispatch Review 24 (delta brief: verify the five R23 fixes + that the core is
  unchanged) → after clear: push
  9d259b185 + 88b010961 + 610ae17f7 + lane commit (untracked fixtures + docs),
  declare Waffles, tell Tom plainly (he's confirmed the norn-fleet plan as next) →
  beamr ^0.16.3 both halves (PUBLISH HELD for Tom's own envelope + verify-build
  positive-token gate row) → then the beamr etf.rs:120 rooting claim.

---

# REVIEW 24 — REFUSED (0B/0M/2m). BOTH FIXED, GATES GREEN

Verdict at `tasks/a6f61b568b0350116.output` (brief: `scratchpad/r24-brief.txt`).
"The lane is one clause from landing; nothing structural is wrong." The reviewer
verified at their own hand: all five R23 fixes genuinely fixed (every factual claim
in the three rewritten B1 texts checked against the code — completion.rs's five-path
paragraph, the pid-enders' `cancel_pid`, the deregister-only paths, `terminate::
complete`/`fail` test-only, the continue_as_new terminal→successor-start→remove
ordering); the CORE PROVEN UNCHANGED by byte-diff against R23's preserved backup
(only the predicate doc paragraph differs); the arm-skip mutation RE-PROVEN RED
through the decomposed GateHarness + Drop rework; ZERO bypasses in the diff; the
60-verdict-line suite shape reconciled against binaries on disk. Their own harness
hit a waiter self-match bug mid-run (their `pgrep -f` wait pattern matched its own
command line; they re-armed on `pgrep -x cargo`) — noted here as another instance of
the actuator-inside-its-own-target class.

## R24 FINDINGS AND STATUS — BOTH FIXED
- **minor 1 — FIXED.** The B1 fix's closing clause "so sparing loses nothing" was
  over-wide — third instance of that species in this lane (after "cost nothing that
  was not already lost", R21, and "spared because recoverable", R22). Replaced with
  the reviewer's remedy: sparing costs no durability; what it does NOT close is the
  still-registered continue_as_new window named just above, and that residual is
  accepted because no kill from this seam can be taken safely, not because the
  window is free. Doc-only change to `nif_continue_as_new.rs`.
- **minor 2 — FIXED.** The POST-R23-FIX GATES section implied harness coverage of
  suite attempt 2; the window is now extended to 23:22 and attempt 2 carries the
  explicit MANUAL RE-RUN OUTSIDE THE HARNESS clause (same honesty clause suite-3
  earned under m1).

## POST-R24-FIX GATES (2026-08-07 23:46–23:49 local, detached pid 96934; transcript
## scratchpad/r24-fix-gates.transcript, logs r24f-*.log): ALL GREEN
- Entry: load 10.49, ZERO foreign cargo on the process table (process-table
  exclusion, not load, decided entry). fmt --check EXIT 0 · module 5/0 (10.36s) ·
  clippy -p EXIT 0 WITH REAL WORK (checking-lines 1, the m2 rule applied) · clippy
  --workspace EXIT 0. Loads 9.0–11.0 throughout. No suite re-run: the change is
  doc-comment-only in one file plus this record — the module test is the executable
  surface it touches.

**Pinned baselines after the R24 fix pass (post-fmt, fmt-check 0):**
- `nif_continue_as_new.rs` = `1f13c64d7e73caada8a412411a3c25bec9f45d0e` (doc-only
  delta from `5c36b302…`)
- `nif_continue_as_new_tests.rs` = `c5adbd62c…` unchanged · `engine/api.rs` =
  `10feca19…` unchanged · lib.rs `10d04200…` / store_faults.rs `7aae3983…` unchanged

- NEXT: Review 25 (tiny delta: the one clause + this record's two fixes + gate
  logs/shas). Chain R6–24 all refused; R24 found nothing structural. On CLEAR: the
  push per the sequence above.

---

# REVIEW 25 — REFUSED (0B/0M/2m), BOTH IN THIS RECORD FILE, BOTH FIXED. THE CODE
# HALF DECLARED CLEAR. LANE LANDS.

Verdict in the session task output (brief: `scratchpad/r25-brief.txt`). The reviewer:
"The code half is clear: R24 fix 1 is genuine, minimal, and every factual claim in
it holds. Both remedies are one-line doc edits in an untracked file that sits on no
build graph — no re-gate is required."

## R25 FINDINGS AND STATUS — BOTH FIXED, VERIFIED AGAINST stat AT MY OWN HAND
- **minor 1 — FIXED.** The POST-R24-FIX GATES header said `~23:56` — a time that
  post-dates every gate artifact AND this file's own last write (23:51:46); it was
  the time I wrote the pre-compaction note, not the time the gates ran. Artifact
  mtimes (re-taken at my hand, `-t "%Y-%m-%dT%H:%M:%S%z"`, AEST): r24f-fmt.log
  23:46:55 · r24f-module.log 23:47:20 · r24f-clippy-p.log 23:48:09 ·
  r24f-clippy-ws.log + transcript 23:49:30. Header now reads **23:46–23:49**.
- **minor 2 — FIXED.** The POST-R23-FIX GATES window said 22:36 start, excluding
  the fmt (22:35:21) and module (22:35:58) gates it cites — carried over from the
  pre-R24 text; R24's m2 fix extended only the end. Header now reads **22:35–23:22**.

## WHAT R25 VERIFIED AT ITS OWN HAND (the code half)
- All five pinned shas exact, re-verified unchanged at review end.
- The R24 fix is EXACTLY one doc-only hunk vs the pre-fix baseline; every factual
  claim in the new clause holds (window named just above; terminal durable before
  the window opens per continue_as_new.rs :92-99→:111→:150; the named residual is
  exactly the residual — the seam is unreachable after handle removal via
  resolve_handle_with_birth_wait/UnknownProcess; HistoryShape carries no owner info
  so "cannot tell which owner" is structural).
- The R23 B1 fix has NOT regressed: completion.rs :212-216 five-path enumeration
  present; pid-enders really cancel_pid (terminate.rs:139, deadline.rs:343);
  deregister-only really don't (terminate.rs:64/:96, continue_as_new.rs:150);
  api.rs :204-209 points rather than restates — nothing to drift.
- No new over-wide phrasing in the diff (the only "cost nothing" hits are the
  api.rs retractions of the old claim); bypass scan of added lines zero.
- The record's R23/R24 sections check out against every artifact (suite attempt
  identities, verdict lines, loads, the real `Checking aion-rs` line); the "no
  suite re-run" justification judged HONEST (a `///` paragraph on a private
  `const fn` + an untracked doc file outside every crate — nothing includes it).

## DISPOSITION — WHY THE LANE LANDS WITHOUT A REVIEW 26
R25's two findings were digit corrections in this untracked record, each decidable
by `stat` against the artifacts — no judgment surface. I re-took the mtimes at my
own hand (above) and applied exactly the reviewer's remedies. The code was declared
clear by a reviewer who byte-diffed it; a further review would verify that two
numbers match stat output already reproduced here. Chain: R6–R25, nineteen
consecutive refusals, monotonically narrowing to two timestamp digits.

TAKEN ON TRUST by R25 (recorded honestly): entry loads/pid-liveness claims not in
artifacts; r23f-suite-2.log's EXIT 0 (no .exit file — content consistent: 60 green
verdicts, 0 `^error`); the #85/#149 attribution counts.

OBSERVATION (R25, not a finding; open convention question for Waffles): this
record's evidence — transcripts, gate logs, review verdicts — lives in the
ephemeral session directory and will not survive the seat, while the record itself
is committed. Pre-existing convention of the whole document; raised in the landing
declaration.

## R22 ORDER
1. B1 test condition (+ Complete-skip control mutation) · 2. M2 three sites · 3. m3/m4/m5
· 4. fmt + check · 5. re-pin baselines · 6. gates to files ON A MEASURED-QUIET BOX (load
recorded) · 7. ALL mutations re-run: the six + R22's TerminalOnly-skip (must now be RED)
+ the Complete-skip control · 8. update this file · 9. dispatch Review 23 · 10. after
clear: push, declare, then beamr ^0.16.3 (publish HELD for Tom's own envelope).

---

# REVIEW 21 — REFUSED (3B/2M/4m). FIX PASS COMPLETE (superseded by R22 above)

Full verdict: `tasks/ac5b2614d91048666.output` (JSONL — do NOT cat whole). Reviewer left
tree byte-identical, proven.

## R21-B1 — CLOSED BY MECHANISM, PROVEN AT MY HAND WITH BOTH CONTROLS
The reviewer's 5 "nondeterministic" failures of the live test (all at CONTROL 2's
survival assert, 0.03-0.06 s) were a STALE MUTANT TEST BINARY: my mutation harness
restored sources via `mv`, which carries the backup's OLD mtime — older than the binary
cargo compiled from the mutant — so cargo judged the build fresh and served the `if true`
build while the source read clean. Reproduced deliberately 2026-08-07: source proven at
baseline `3de2e869…` + mv-restore → EXIT 101 in 0.06 s (identical signature); `touch` →
rebuild → EXIT 0. Logs: `scratchpad/b1-mut.log`, `b1-stale.log`, `b1-touched.log`.
The gate at shipping bytes is deterministic-green (reviewer's 40+ runs + saturation +
12-way parallel). Instrument hardened: `scratchpad/mutate.py` now has a `restore` mode
that WRITES bytes (fresh mtime, forces rebuild) and refuses to clobber a backup; drivers
must pin the expected baseline AND end every mutation session with a green re-run of the
rebuilt tree (the dropped final-green-re-run step is what let this escape).

## STATUS OF R21 FINDINGS (all being fixed, none documented-and-accepted)
- [x] B1 — mechanism above; harness fixed; obligation: re-run all six mutations with the
      fixed harness, final green re-run after the last restore.
- [x] B2 — `terminal_only_account` REWRITTEN against the four repair paths, each read at
      my hand: (1) completion.rs terminal-attempt path calls `retire_run_deadline` for
      non-TimedOut terminals BEFORE `start_continuation_replacement`, `?` into
      completion-retry; (2) `time/deadline.rs` retire_run_deadline doc states the
      resume contract; (3) `sweep_uncancelled_terminal_deadlines` (startup_sweeps.rs)
      sweeps ALL rows (10-millennium horizon) at boot/adoption via independent recorder
      with SequenceConflict re-eval; (4) decide_disposition handle path retires on a
      racing fire (handle stays registered until reconcile_terminal_registry AFTER the
      retirement). New message: "No operator action is required" + repair chain +
      the still-real hazard window (recovery re-arms not-yet-due rows — the D5
      resurrection regression in time/recovery.rs:415-496 pins it).
- [x] B3 — the "whole-history recovery does not re-arm it" bullet replaced with the
      scoped truth (re-arms NOT-YET-DUE rows until retirement lands; overdue rows come
      back via recover_due→expired_timers on the owning node).
- [x] M5 — handle-path vs handle-free scoped correctly in the new doc (handle-free =
      cold/recovering engine only).
- [x] M4 — CONTROL 2 failure now panics with diagnosis: terminal-in-history +
      unspent-append-failures + full history; tracing subscriber deliberately NOT
      installed (log_capture is thread-scoped by crate law; NIF emits on beamr scheduler
      thread; global capture is what that module forbids) — documented at the site.
- [x] m8 — TerminalOnly doc: "only account" → earliest witness, repair chain referenced.
- [x] m9 — epoch refusal warn! → error! with cross-crate-consistency comment.
- [x] m6 — README sentence FIXED: 92 vendored packages + 12 compiler-generated Erlang
      sources, decomposition verified at my hand (460/104 = 92+12+0 unaccounted).
- [x] m7 — VERIFIED post-compaction: the message string no longer makes any
      `expired_timers` claim (it says only that recovery re-arms a not-yet-due row,
      which the D5 regression pins unconditionally); the owning-node-scoped
      `expired_timers` statement lives in the doc (nif_continue_as_new.rs:239-241).

## R21 FIX-PASS RECORD — COMPLETE 2026-08-07 (post-compaction window)

All nine findings fixed; the two verification steps below re-run everything with the
fixed harness. One additional code change was forced during the pass and Review 22 must
see it: **M4's diagnosis-carrying `panic!` tripped the workspace `clippy::panic` lint**
(this tree lints test targets too). Rewritten as a lazy-formatted `assert!` — the
history fetch was hoisted (it was already fetched unconditionally three lines later and
now serves both asserts), and `assert!` format args are only evaluated on failure, so
the diagnosis (terminal-in-history + unspent-append-failures + full history) fires
exactly as before. No `#[allow]`, no bypass.

**Pinned baselines (post-fix, post-fmt):**
- `nif_continue_as_new.rs` = `ef9a764b8fd637c5471eba2055c71279f512dca6`
- `nif_continue_as_new_tests.rs` = `c8a65690087d8ccba6c535afb45afb618ad150e9`

**Gates (all to files in the session scratchpad, prefix `r21-gate-`):**
| Gate | Result |
|---|---|
| `cargo fmt --check` | exit 0 |
| `cargo clippy -p aion-rs --all-targets -- -D warnings` | exit 0 (after the panic→assert fix) |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 |
| `cargo test -p aion-rs` run 1 | ⚠️ exit 101 — `covered::covered_set_is_byte_identical` (BC-4 differential), task #149's documented red-and-nondeterministic-on-main; harness untouched by this lane; log kept (`r21-gate-suite-1.log`) |
| `cargo test -p aion-rs` run 2 | exit 0, **60 verdict lines, 1130 passed / 0 failed** |
| `cargo test -p aion-rs` run 3 | exit 0, **60 verdict lines, 1130 passed / 0 failed** |

Run 1's failure is #149's known nondeterminism (dag-fork activity_id ordering +
loop-outcomes trail length), pre-existing on main and outside this lane's blast radius;
tree checked clean of unexpected dirt before attribution (only the four lane files +
two untracked fixtures).

**Six mutations, fixed harness (`scratchpad/r21-six-mutations.sh`, transcript
`r21-six-mutations.transcript`, logs `r21mut-*.log`):** baseline proven green first
(5/0); every restore via `mutate.py restore` (bytes written, fresh mtime); session
ENDED with a green re-run of the rebuilt tree — `SESSION CLOSED GREEN`, 5/0.
| Mutation | Result |
|---|---|
| a `Err(_) => true` | RED — 2 failed (live test + table test) |
| b `EngineTaskEpochClosed` → `HistoryShape` | RED — 2 failed |
| c `Ok(_)` narrowed to `Ok(Complete)` | RED — 2 failed (terminal-only + table test) |
| M5 reviewer's `if recorded.is_ok()` | RED — live test failed |
| M6 `if false` | RED — live test failed |
| M7 `if true` | RED — live test failed |

The old driver scripts (`m15-predicate-mutations.sh`, `gate-mutations-rerun.sh`) are
superseded by `r21-six-mutations.sh` and must not be re-run (stale pinned baseline,
mv-restore).

## ORDER (remaining)
6. Review 22 DISPATCHED 2026-08-07 ~07:40Z on Opus (verdict will land at
   tasks/ac0791a59bc6e9b0f.output). Brief = R21's shape updated to the R21 fixes: B1
   mechanism judgement, the four-repair-path B2 message, the panic→assert M4
   restructure, fresh baselines, six-mutation session, #149 attribution. 🔴 TREE
   BYTE-FROZEN until the verdict — no edits, no Cargo.toml/lock, nothing.
8. After R22 clears → push 9d259b185, 88b010961, 610ae17f7 + the lane commit (incl.
   untracked fixtures + all four docs), declare to Waffles, tell Tom plainly. THEN the
   beamr bump — TARGET `^0.16.3` (Cally RETRACTED 0.17.0 at 07:36Z: for 0.x, cargo
   treats minors as majors, so a lone ^0.17.0 double-links two beamr runtimes and the
   battery goes vacuously green — the family moves together). Both halves: `cargo
   update -p beamr` + `Cargo.toml:94` → "0.16.3". Separate commit, own battery.
   🔴 PUBLISH HELD — Tom's authorisation must arrive in MY envelope (provenance,
   Seth's blocker); Cally's quote does not carry it.
   🔴 PUBLISH GATE ROW (Cally 08:44Z, estate sweep): `publish-workspace.sh:76` honors
   `NO_VERIFY=1` SILENTLY — the success line is byte-identical either way, so a clean
   log proves nothing. Any aion-rs publish must POSITIVELY show the verify build ran:
   run `cargo publish` directly so the verify build's own output is in the record, or
   add a positive token to the script first. (0.11.0's published-binary install-verify
   107/0 was a different-shaped instrument and stands; a `.crate` pull-and-build is the
   stronger check if provenance is ever questioned.)

## ⚠️ INCIDENT DURING THE PASS: A KILLED MUTATION RUN CORRUPTED ITS OWN BASELINE

An interrupted mutation run left mutation A (`Err(_) => true`) live in the tree; the next
run then backed the MUTATED file up over the good backup and reported "restored
byte-identical" against a corrupted baseline. Caught because two mutations came back
NON-RUN (their targets were gone — the found-exactly-once guard is what surfaced it) and
the predicate read wrong at my own hand. Recovered by reconstructing the arm and proving
the shasum returns to the pinned baseline. The instrument is hardened three ways:
`mutate.py` refuses to overwrite an existing backup (exit 3), the driver pins the
EXPECTED baseline hash instead of blessing whatever bytes it finds, and a green-baseline
suite run gates every mutation session. `set -e` is banned from mutation drivers — a red
test run is the expected outcome, and aborting on it is what left the mutation live.
