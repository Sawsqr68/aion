# SEAT RESUME BT — 2026-08-07 — supersedes BS (read BS §1–§9c for history)

🔴 **THIS IS THE CURRENT AUTHORITY.** Latest letter wins.

## STATE IN ONE LINE

Review 16 **REFUSED** the Review-15 fix delta with **two BLOCKING findings**.
F-A is **partly applied** (see §3). F-B is **NOT STARTED**. Do not commit.

---

## §1 — WHERE THE LANE IS

- The whole #7 lane + #146 tutorial is uncommitted on `main`. ~7,400 lines.
- Reviews 6–16 have **all refused**. Twice the findings were defects introduced
  while fixing the previous review's findings. **That is the base rate — assume
  the next defect is in the fix you just wrote.**
- Review 15 (agentId `ad6520ea5b433526c`) explicitly **cleared the engine work**
  and refused on 3 mechanical findings + 1 minor. All four were closed — see
  BS §9. Its brief: `scratchpad/review-15-brief.md`.
- Review 16 (agentId `a651bf4c09aed7edb`) reviewed **only that fix delta** and
  refused. Its brief: `scratchpad/review-16-brief.md`.

## §2 — REVIEW 16'S VERDICT (verbatim substance)

### F-A (BLOCKING, CONFIRMED) — the new append wording is FALSE at the cancel site
`nif_timer_fire.rs:54-62`, raised from `nif_timer_bridge.rs:515-517`.
`record_workflow_event` extracts `torn_down_timer` from **either** arm of
`TimerOutcome` (`nif_timer_bridge.rs:495-499`) and raises `WheelTornDown` for
both, and `into_seam_error` mapped every one to `wheel_torn_down_after_firing`.
For a refused `TimerCancelled` both halves are wrong and the second is inverted:
nothing fired, and *"still live … the owning engine re-arms it"* is the FAILURE,
not the remedy — the run asked for the timer to stop.

Reachable: `Drop for Engine` (`engine/api.rs:163`) sets `shut_down` and leaves
the scheduler + seams alive by design (`api.rs:122-133`). A still-runnable
process calls `aion_flow_ffi:cancel_timer/1` → `nif_timer.rs:193` →
`cancel_live_timer` (`nif_timer.rs:478`) → `TimerService::cancel` →
`cancel_guarded` (`time/timer_service.rs:186-207`, passes `timer_is_live`,
`disarm_timer` has no gate) → `record_workflow_event(TimerCancelled)`. Also
surfaces at `engine/api_workflow_ops.rs:252-258` (`tracing::warn!`) and
`nif_timeout.rs:308`.

**This is the identical defect class Review 15 blocked on, reintroduced at a new
site by the fix for it. The brief asked the question; I shipped as though the
answer were yes.**

### F-B (BLOCKING, CONFIRMED) — the deadline fire path has NO stand-down refusal
The refutation itself **checks out** — `fire_timer_guarded` demuxes
`is_deadline_timer` at `time/timer_service.rs:264-266` **before** the generic
record-then-deliver path, so a deadline never reaches the append boundary, and
`is_wheel_teardown`'s variant bound holds. **But that demux is exactly why the
deadline path is ungated:**

- Deadlines arm on the live wheel: `lifecycle/start.rs:463-482
  arm_declared_deadline` → `TimerService::schedule` → `engine.arm_timer`.
- Wheel task → `fire_wheel_timer` → `fire_timer` → demux → `fire_deadline` →
  `WorkflowDeadlineHandler::on_deadline_elapsed`.
- **`crates/aion/src/lifecycle/deadline.rs` contains NO `shut_down`, epoch or
  residency gate anywhere.** `decide_disposition` re-checks only terminal +
  deadline liveness under the recorder lock, then appends `WorkflowTimedOut`.

Scenario: engine dropped without `shutdown`; a deadline wheel task is already
inside its poll when `shutdown_timer_wheel()` runs — the exact residual argued
for at `nif_timer_bridge.rs:502-514` (`abort` cannot stop a task already in a
poll). `abort` misses it, the pre-arm gate is irrelevant, the append boundary is
never reached because of the demux. It records a durable `WorkflowTimedOut` for
a run a successor may already own = **load-bearing invariant 3 / the #119
breach**, through the very wheel this lane was hardening.

🔴 **This falsifies `crates/aion/src/engine/api.rs:139-141`, which is NEW IN
THIS LANE** (not at HEAD): *"the live timer wheel — `shutdown_timer_wheel()`
below, which gates and drains, **and refuses at its own append boundary**…"*.
For a deadline fire there is no append-boundary refusal. `api.rs:150-158` warns
in its own words that the list *"is only as good as its enumeration — twice
proven"*. **This is the third time.**

### Minors from Review 16 (all still OPEN unless noted)
1. `crates/aion/src/engine_seam.rs:192-197` — `EngineSeamError::TimerWheel`'s
   doc says "could not arm or disarm the timer wheel"; the lane added a third
   use (the append refusal) that is neither.
2. 🔴 **The recorded `build-tutorial.py` hash in BS §9 is WRONG.** BS records
   `e69a87f0…`; the delivered file hashes
   `3d880bf9bf730f38974114972c765f49f6f9b3808a525527aa949ba1de0fbe7b`. Cause:
   the hash was taken BEFORE the F4 docstring edit, which lands in the same
   file. **The substance is verified true by other means** — Review 16
   regenerated the deck to a scratch path and got a file byte-identical to the
   committed `first-workflow.html` (`e3f97508…`), no `zzz` residual, 4 group
   rows, all gates green. **Amend BS §9's hash; do not re-assert the old one.**
   (Law: a withdrawal is proven against the ARTIFACT.)
3. BS §9's closing line "⏳ battery + clippy RE-RUN REQUIRED" is **stale** —
   both ran green (BS §9a). Amend.
4. **F1(c) is cheaply closeable after all.** `gate_keyword_coverage` already has
   `used` and `explained` in scope (`build-tutorial.py:295-308`); `explained -
   used` is ~3 lines. Review 16 computed both sets: each is exactly the same 13
   words, so it passes today. Documenting it as a permanent gap understates it.
5. **F1(d): there are TWO more hand-typed numbers**, both currently TRUE but
   ungated and the same shape as the one that drifted:
   - `build-tutorial.py:687` "Four real commands, run by the server" — there are
     exactly 4 `run` bodies in `repo_gate.awl`.
   - `build-tutorial.py:694` "You will run it five times as you go" — there are
     exactly 5 staged `.awl` documents, and the build's own gate PRINTS
     "all 5 documents" from the derived count while the prose types it.

### What Review 16 CLEARED (traced, not assumed — do not re-do)
- Typed-error conversion preserves behaviour: the blocking future has exactly 3
  fallible `?` sites, all now `TimerAppendError::append` → `Append` →
  `Recorder { reason: error.to_string() }`, same string as the boxed version.
  The 3 error paths outside the future are untouched.
- `is_wheel_teardown`'s variant bound holds (fire path only calls
  `record_workflow_event`/`resolve_workflow`/`deliver_workflow_message`; the
  latter two raise only `Delivery`; `workflow_id_for_process` is reachable only
  from `arm_timer`).
- **F3 split is a real seam and correctly measured**: `nif_timer_bridge.rs` 436,
  `nif_timer_fire.rs` 139. The four moved bodies byte-compared against HEAD —
  three identical, `fire_wheel_timer` differs by exactly the intended 12-line
  early return. Nothing dropped or duplicated. `store: pub(super)` widens only
  to `crate::runtime` and only `nif_timer_fire` reads it.
- **The tests measure the path.** `stand_down_is_not_a_fault` seeding is genuine
  (`WorkflowStarted` seq 1 + `TimerStarted` seq 2 → `timer_is_live` passes,
  `is_deadline_timer("stand-down")` false, reaches `record_workflow_event`); the
  else-guard fires if the path is skipped. M13/M14 both sit on the execution
  path and are orthogonal, as recorded.
- F4 correct. Gate evidence real: `bt-clippy-ws.log` zero diagnostics;
  `bt-workspace.log` 247 verdicts / 4215 passed / 0 failed / `RC_WS=0`; **no
  `.rs` or `.py` under `crates/` or `docs/authoring/` is newer than that log.**
- One doc imprecision to fix while nearby: deleting the `shut_down` check fails
  `a_torn_down_wheel_refuses_to_append` at the **else-guard**, not at "both
  assertions".

---

## §3 — WHAT I HAD ALREADY DONE WHEN THE COMPACTION LANDED

**F-A: PARTLY APPLIED — `crates/aion/src/runtime/nif_timer_fire.rs` ONLY.**
Applied and on disk:
- `wheel_torn_down_after_firing` doc now says it is the FIRE case only and
  points at the cancel constructor.
- **NEW** `wheel_torn_down_before_cancelling(timer_id)` with honest wording:
  `"cancellation of timer `X` was not recorded: … The timer stays live in
  durable history and that engine re-arms it; the cancellation takes effect only
  once the run reissues it there"`.
- `TimerAppendError::WheelTornDown` is now a **struct variant**
  `{ timer_id: TimerId, cancelled: bool }`.
- `into_seam_error` dispatches on `cancelled`.

🔴 **NOT YET DONE — THIS WILL NOT COMPILE UNTIL IT IS:**
`crates/aion/src/runtime/nif_timer_bridge.rs` still constructs the OLD tuple
form. Around `nif_timer_bridge.rs:515`:
```rust
return Err(TimerAppendError::WheelTornDown(torn_down_timer));
```
must become the struct form carrying whether the outcome was a cancel. The
outcome discriminant is available where `torn_down_timer` is computed
(`nif_timer_bridge.rs:495-499`, matching `TimerOutcome::Fired(_) |
TimerOutcome::Cancelled(_, _)`) — compute a `cancelled: bool` alongside it and
move both into the future.

## §4 — THE REMAINING WORK, IN ORDER

1. **Finish F-A** (§3) and add a test: a refused CANCEL must NOT say "fired" and
   must NOT offer the re-arm as reassurance. Mutation: collapse both arms back
   to `wheel_torn_down_after_firing` → that test must go RED.
2. **F-B — close the hole AND correct the claim.** Design settled before the
   compaction:
   - `WorkflowDeadlineHandler` (`lifecycle/deadline.rs:46-74`) gains
     `stand_down: Arc<AtomicBool>`.
   - Gate **inside** `decide_disposition`'s recorder lock (`deadline.rs:114-122`,
     immediately after `let mut recorder = recorder.lock().await;`) — that is
     what makes it decisive rather than check-then-act, same argument as
     `nif_timer_bridge.rs:498-514`.
   - 🔴 **`finalize_timed_out_without_handle` ALSO appends durably and is ALSO
     ungated** (`record_ordinary_timer_retirements`, `upsert_workflow_visibility`,
     `retire_run_deadline`). It has no handle and therefore no recorder lock —
     gate it before the writes and **state honestly in the doc that this one is
     check-then-act, not lock-decisive**, because it is.
   - Add `DeadlineDisposition::StoodDown` rather than reusing `LoseCleanly`
     (whose doc means "a competing terminal won") — this lane is about not
     conflating causes. `StoodDown` → return `Ok(())`, **no teardown**, debug
     line naming the cause.
   - Wiring: bridge is installed at `engine/builder_assembly.rs:50`, handler
     built at `:238` in `register_workflow_deadline_handler` — **the bridge
     exists first**, so add an accessor on the bridge for its `shut_down` Arc and
     pass it into `WorkflowDeadlineHandler::new`.
   - **3 test call sites** need the new arg: `lifecycle/deadline_tests.rs:83`,
     `:250`, `:436`.
   - Correct `engine/api.rs:139-141` either way — after the gate lands it must
     describe what is actually true of the deadline path.
   - Test: a torn-down wheel + an elapsed deadline records NO `WorkflowTimedOut`
     and does NOT tear down; with a POSITIVE CONTROL that it does both when the
     flag is clear. Mutation: delete the gate → RED.
3. **Minors 1–5** from §2.
4. **Amend BS §9**: the wrong hash (minor 2) and the stale line (minor 3).
5. **Re-run gates on the new tree** (battery-re-runs-on-new-tree):
   `cargo clippy --workspace --all-targets -- -D warnings`, then
   `cargo test --workspace --no-fail-fast`, then reconcile.
6. **Review 17** on the new delta. Do not land on my own say-so — see the base
   rate in §1.

## §5 — RECONCILIATION METHOD (both defects already bit me once each)

🔴 **Declared-target regex MUST be:**
`^\s*Running\s+.*?\((target/\S+)\)\s*$`
Requiring one token (`\S+\s+`) drops every `Running unittests src/lib.rs (…)`
line and reported 200 instead of 243 — a *surplus*, which reads safe.

🔴 **Production line count MUST split on** `\n#[cfg(test)]\n#[path` — splitting
on the first `#[cfg(test)]` hits the inline test seam at `nif_timer_bridge.rs:409`
and reported **26** production lines for a 520-line file.

🔴 **The load-bearing battery assertion is ZERO-VERDICT TARGETS: NONE**, i.e.
every declared target emitted ≥1 `test result:` line. A crashed binary emits no
verdict line at all, so a surplus can never rule one out.

Last good battery (pre-F-A/F-B, tree of BS §9a): 223 binaries + 20 doc-tests =
243 declared, 247 verdicts, 243/243 emitted, **4215 passed / 0 failed / 0
ignored**, `RC_WS=0`. Surplus +4 = `aion_server` (4) + `Doc-tests aion` (2).

🔴 **#85 did not fire in either of the last two runs. THAT IS NOT EVIDENCE IT IS
FIXED** — 2/2 contended runs reproduce it, 5/5 isolated are green. Stays open.

## §6 — COMMIT PLAN (only after a review passes)

Three lots, paths named EXPLICITLY, **never `git add -A`**, lists DERIVED FRESH
at commit time:
```
git -c diff.external= diff --no-ext-diff --name-status | sort
git ls-files --others --exclude-standard -- crates docs | sort
```
(the `-- crates docs` scoping is REQUIRED — unscoped walks the no-sweep backup
dirs and returns 8.2 MB)

- **(a) tutorial** — `docs/authoring/build-tutorial.py`, `first-workflow.html`,
  `tutorial-examples/aion.toml`, 3 deletions (`00-skeleton.awl`,
  `first_flow.awl`, `worker.toml`), 5 new `repo_gate*.awl`.
  Message drafted: `scratchpad/commit-msg-a-tutorial.txt`
- **(b) the #7 fix lane** — all `crates/` tracked modifications + untracked
  `aion-store/src/testing/fenced_history.rs`, `aion/src/engine/api_workflow_ops.rs`,
  `aion/src/engine/builder_assembly.rs`, `aion/src/lifecycle/completion_retry.rs`,
  `aion/src/log_capture.rs`, `aion/src/store_faults.rs`,
  **`aion/src/runtime/nif_timer_fire.rs` (NEW)**.
  Message drafted: `scratchpad/commit-msg-b-fixlane.txt` — ⚠️ **must be updated**
  to cover F-A/F-B before use.
- **(c) markers** — `docs/design/aion-authoring/*`, **KEEP EXCLUDED:
  `SPEC-5-AWL-SUM-COMBINATOR.md`**.
  Message drafted: `scratchpad/commit-msg-c-markers.txt`

Backticks in commit messages: `git commit -F <file>` (zsh eats them from `-m`).

## §7 — STANDING CONSTRAINTS (unchanged)

- **NO WORKTREES** in aion; one mutating lane; unmarked dirty = STOP.
- ⛔ never bare `cargo clean`; no custom `CARGO_TARGET_DIR`; no launchd.
- ⛔ **NEVER `git add -A`**; `git stash push` never with `-u`; **DELETE NOTHING**
  (git objects) — no `git gc`/`prune`/`--prune`; no `--force`/`--mirror`/
  `--delete` pushes.
- 🔴 **Tom's live pids: server 45425, worker 48399. DO NOT TOUCH.**
- NO-SWEEP untracked: `.aion-accepted-binary-20260731/`,
  `.aion-jjuabk-snapshot-20260731/`, `.aion-data-preswap-20260804/`,
  `../aion-pre-scrub-backup-20260804.git`.
- ⛔ `.aion-accepted-binary-20260731/aion` is the SOLE copy of Tom's accepted build.
- 🔴 WORKER REBUILDS FROZEN estate-wide until the #139 live cut.
- 🔴 beamr JIT blocker with Artemis — keep long workflows off.
- ⛔ Do not edit `CLAUDE.md`.
- Estate diff driver emits no +/- prefixes: `git -c diff.external= diff --no-ext-diff`.
- `/usr/bin/python3` is 3.9 (no tomllib) — use `/opt/homebrew/bin/python3.13`.
- zsh: `$pipestatus` (1-indexed), not `PIPESTATUS`.

## §8 — WHO IS WAITING

- **Tom** (`dm:c9255b2a-5731-4d17-8124-e3bfa2224186`) — told 00:0xZ the tutorial
  is ready and accurate NOW and doesn't wait on the engine work, and that I'd
  tell him when the rest is in. **He can walk the tutorial today.** Plain
  language, meaning not status, one line of care max, never re-offer hotlines.
- **Waffles** (`dm:896955e1-86dd-4d6a-9c25-f3f978b189a9`) — holding
  BRIEF-ENGINE-CONFIG-SERVER-SURFACE behind this; standing ruling covers the
  time cost of another refusal ("the deadline bends, the bar doesn't"). Owed a
  declaration either way. He banked the comfortable-number law.
- **Cally** (`dm:15a23351-009a-4e69-8d63-45602a805f1c`) — captured
  `.worktrees/delivery-fix` to
  `/Users/tom/Developer/ablative/backups/aion-delivery-fix-worktree-20260806.patch`
  (sha256 `6c104cfaa0f2c12acbd50b9d79c898738f06f01cbf4d097fec670fa24804eb34`).
  My working-tree flag stands until the lane lands; I said I'd prefer he **hand
  it over rather than clear it**.

## §9 — TEARDOWN OWED (mine, not Tom's)

- pid **92685**, `~/tut-clean`, ports 8098/50098 — kill **BY LISTENING PORT**
  (`lsof -tiTCP:8098 -sTCP:LISTEN`), never a pgrep pattern. Then `rm -rf ~/tut-clean`.
- Proving clones: `~/gate-clean-3/-4/-5/-red`, `~/gate-work-3/-4/-5/-red`.
- Remove `scratchpad/shimbin/aion` from PATH (fake `aion` stub).
- ⛔ Tom's tutorial server pid **50822** (`~/my-first-flow`, :8099) is HIS.
