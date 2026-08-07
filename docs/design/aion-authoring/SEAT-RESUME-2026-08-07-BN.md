# SEAT RESUME — marker BN (2026-08-07)

Supersedes **BM**. Nothing is committed. Reviews 6–12 have ALL refused.

BM's closures (Blocker 1, Findings 3/4/5, Blocker 2, and review 12's B1–B5)
still stand and are not re-litigated here. This marker records what happened
AFTER them: review 12's F1–F14, and the audit of the surface no review had
reached.

---

## REVIEW 12's F1..F14 — ALL DISCHARGED

Review 12's full text was recovered from the session transcript to
`scratchpad/review12-findings.txt`; the marker summary alone did not carry F11
or F12.

| # | Finding | Disposition |
|---|---|---|
| F1 | `embedded.rs` claims a `not_owner` routing-retry policy that does not exist | **FIXED** — verified at my hand: `stream.rs`'s `is_retryable` is `matches!(error, ClientError::Unavailable { .. })` and nothing else. Both comments (`:570`, `:922`) now say the distinction is one the CALLER acts on, name the real beneficiary (`aion-cli/src/render.rs:175-178`), and `aion-client/src/error.rs`'s `NotOwner` doc was converged with them. |
| F2 | `ShuttingDown → not_running` picked one of two contradictory server answers silently | **FIXED (comment)** — the class stays (it mirrors `wire_from_engine`, which governs the eight operations this transport exposes); the comment now NAMES the two handlers that answer `Unavailable` (`authoring/handlers.rs:404-406`, `api/handlers/deploy.rs:415`) and states that neither serves an operation this transport exposes. |
| F3 | the two new `aion-server` arms had no test | **FIXED** — `engine_task_epoch_closed_maps_to_typed_backend_on_both_halves` in `error_tests.rs`, pinning the wire code, the `error_type` on BOTH halves, and that the message names the run. |
| F4 | `EngineTaskEpochClosed → not_running` contradicts the variant's own doc | **FIXED (code)** — now `backend`. The variant's doc says "in both cases THE RUN STAYS `Running`"; `not_running` renders through `render.rs:157-160` as "the target run is no longer running", which is false twice over. `backend` claims nothing about the run and is what this table already gives every other engine-internal refusal. |
| F5 | `InterestFloor::max_level_hint() == None` is an unstated process-wide side effect | **FIXED** — implemented explicitly with the derivation, including why `Some(LevelFilter::OFF)` is WORSE: between `ensure_interest_floor` and the caller's `set_default` the floor is the only registered dispatcher, so an `OFF` max would open a new silent-loss window. |
| F6 | `log_capture`'s "One copy, deliberately" was false | **FIXED BY SUBTRACTION** — `durability/recorder.rs`'s third capture (`Vec<u8>` writer + `output.contains`) is DELETED, the test migrated to `LogCapture`, and its assertion strengthened to exactly-one-WARN with per-needle `mentions`. 45 lines of duplicate machinery gone. |
| F7 | the O5 failure message still misattributed an empty capture | **FIXED** — the vacuity control now names BOTH causes and prints the discriminator (`total_captured` / `warning_count`), so a lost capture cannot read as a short loop. |
| F8 | a CONTROL comment claimed a control that could not exist | **FIXED BY BUILDING IT** — `subscribe` gained a real success control; `reopen` cannot succeed on a non-terminal run, so its control is that the DISARMED call fails with a class other than `not_owner`. The impossible sentence is gone. |
| F9 | `query_error_class`'s `Engine(_)` arm had no table row | **FIXED** — seventh row added, `EngineSeamError::Delivery` → `backend`, with the reason written in. |
| F10 | `RefusingReadStore` owned the shard it refused | **FIXED** — `is_current_owner` returns `false` for `REFUSED_SHARD` while armed; every other shard still delegates. |
| F11 | `Drop for Engine` new / `Engine::shutdown` rewritten inside a "split" | **AUDITED** — see below. |
| F12 | the O5 proof cannot see the production spawn path | **ACCEPTED** — honestly named in the code already; no change. |
| F13 | the `Ok(Recorded)` arm reported "recorded" on a stand-down | **FIXED** — the message now separates the fact that IS established (the terminal is durable) from the bookkeeping that is not. |
| F14 | the ruling named O5's location as a file with no test module | **FIXED BY AMENDMENT** — the row now names `completion.rs` and the test by name, with an explicit AMENDMENT paragraph; a ratified obligation is not discharged by quiet editing. |

---

## THE UNREACHED SURFACE — AUDITED, AND IT CARRIED TWO REAL DEFECTS

An Opus audit compared every top-level item in `api.rs`/`builder.rs` at HEAD
against the working tree plus `api_workflow_ops.rs` / `builder_assembly.rs`.

**The split itself is clean.** `api_workflow_ops.rs` contains ZERO new code —
15 relocated items and nothing else. `builder_assembly.rs` has exactly one new
item (`claim_owned_shards`, a two-line wrapper preserving order and error
propagation). All 56 test items moved byte-identically. Nothing was deleted.
`mod.rs` holds only declarations. Every file is under 500 code lines.

**The lane is NOT a split, and the framing was hiding three separate pieces of
work:** the relocation, the `completion_retry` builder knob, and `Drop for
Engine` + the `Engine::shutdown` rewrite.

### 🔴 DEFECT 1 — A DRAIN IS NOT A GATE. Invariant 3. FIXED AND MUTATION-PROVEN.

`shutdown_timer_wheel` (`nif_timer_bridge.rs`) removed and aborted every timer
pending at ONE INSTANT, while `arm_timer` had no check of any kind — verified
at my own hand: it spawned unconditionally. `Engine::shutdown` was not exposed
(it stops the scheduler first), but `Drop for Engine` deliberately leaves the
scheduler AND the seams alive, so a workflow process still runnable could reach
`sleep` after the drain and arm a fresh durable `TimerFired` writer. That is the
#119 failover race reached through the door the new `Drop` opened: a released
engine appending for a run a successor has adopted — a second writer.

**Fix:** `shut_down: AtomicBool` on the bridge, set BEFORE the drain, checked at
the top of `arm_timer`, which now refuses with `EngineSeamError::TimerWheel`.
Gate-then-drain composes into one rule no interleaving defeats.

**Proof** — new test `arming_a_wheel_timer_after_the_engine_is_released_is_refused`
(`engine/builder.rs`), two controls (the fixture arms successfully BEFORE the
drop; the count is 1 → 0 across the release):

| tree | result |
|---|---|
| fixed | `engine::builder` **95 passed / 0 failed** (with `runtime::handle`, `lifecycle::completion`, `time::timer`) |
| mutant (the `shut_down` check deleted — literal prior text, no new machinery) | **1 failed / 22 passed IN THE SAME INVOCATION**; failure is the arm returning `Ok(())` |

Restore proven by `shasum -c` (`scratchpad/ntb.sha`, OK).

### 🔴 DEFECT 2 — SECOND SHUTDOWN ERRORS WERE SWALLOWED. FIXED.

`Engine::shutdown`'s accumulate-and-continue kept the first `EngineError` and
DROPPED every later one — not returned, not logged. At HEAD no error was lost,
because `?` meant the later step never ran; the rewrite traded a skipped
teardown for a swallowed failure. Same pattern in `RuntimeHandle::shutdown`.

Both now report: `keep` takes a step label and emits every subsequent failure at
`error` level; `handle.rs` gained `keep_shutdown_error` with the same contract.
The return value carries the first, the log carries the rest, nothing is lost.

### NAMED, NOT FIXED — carried forward honestly

- **`runtime.shutdown()` runs on an undrained gate** when `close_and_wait`
  fails. Consequence: a run wedged `Running` with no process for the life of the
  process; startup recovery is designed to pick it up, and no new operation is
  admitted (`begin_start`/`begin_operation` share the poisoned `state()`).
  Invariant 4 intact. The trade comment argues the timer wheel and seam clearing
  and does not name this.
- **Cancel-safety of `EventStore::append` under `JoinHandle::abort`** —
  unverified for haematite and libSQL.
- **Whether beamr keeps dispatching after `Engine` is dropped** — inferred from
  the absence of any scheduler stop in `Drop`, not verified against beamr.
- `nif_child_watch.rs` (231 changed lines) and the second half of
  `runtime/handle.rs` — skimmed only by the audit.

---

## GATE STATE AT THIS POINT

Run after the F-fixes, before the defect-1/2 fixes:

- `cargo check -p aion-rs -p aion-server -p aion-client -p aion-store --all-targets` → exit 0.
- `cargo test -p aion-rs --lib -- lifecycle::completion durability::recorder loader::declared` → **58 passed / 0 failed**.
- `cargo test -p aion-server --lib` → **797 passed / 0 failed** (+3 child re-invocations).
- `cargo test -p aion-client --all-targets` → **63 + 1 + 6 + 0 passed / 0 failed** — ⚠️ these are the tests review 12 recorded as NEVER RUN by anyone.
- `cargo test -p aion-store --all-targets` → **64 + 1 passed / 0 failed**.

After the defect fixes: `cargo test -p aion-rs --lib -- engine::builder runtime::handle
lifecycle::completion time::timer` → **95 passed / 0 failed**. `cargo fmt --all` run.

## 🔴 ORDER OF WORK ON RESUME

1. **FULL GATES ON THE FINAL TREE, not yet run since the defect fixes:**
   `cargo clippy --workspace --all-targets -- -D warnings`;
   `cargo test -p aion-rs --no-fail-fast`;
   `cargo test --workspace --no-fail-fast` (declared-vs-recorded reconciled
   WITHIN the run — derive `declared-binaries.txt` from the same tree first).
2. **Review 13** on Opus. Give it this marker, BM, and the two defects above.
3. On a PASS only: commit with files named EXPLICITLY (never `git add -A`);
   RE-DERIVE `scratchpad/commit-manifest-7-lane.txt` first, and keep
   `SPEC-5-AWL-SUM-COMBINATOR.md` excluded (1.1 MB of raw JSONL, issue #5's).

## STANDING CONSTRAINTS (unchanged)

- Tom's pids **45425** (server) / **48399** (worker): do not touch. Verified
  alive at both ends of this tick.
- ⛔ Disk **16 GiB** free and falling. No cold builds.
- ⛔ Three untracked `.aion-*` dirs in the repo root are NO-SWEEP holds.
- ⛔ Nothing is deleted under the estate hold.

## OWED TO TOM

Reviews 6–12 have ALL refused. This tick the audit of the one surface no review
had reached found a live single-writer breach in the lane's own new `Drop` —
which is the answer to whether "it's just a split" was ever true. Then: #146
tutorial dry run + crib sheet (without touching his pids), then #5, #1, #6, #9.

---

# 🔴 LIVE GATE STATE — appended 2026-08-07, mid-run

## TWO LINT VIOLATIONS OF MY OWN, CAUGHT BY CLIPPY AND FIXED WITHOUT SUPPRESSION

The first clippy run over the fixed tree came back **RC=101** on code I had just
written this tick:

- `crates/aion/src/durability/recorder.rs` — a `panic!` in the migrated capture
  test (`clippy::panic`). Replaced with `assert_eq!(warnings.len(), 1, ..)` plus
  `warnings.first().ok_or(..)?`.
- `crates/aion/src/engine/builder.rs` — `expect_err` in the new timer-gate test
  (`clippy::expect_used`). Replaced with `let Err(error) = refused else { return
  Err(..) };`.

`cargo fmt --all` re-run after. The in-flight batteries were STOPPED and
restarted from scratch on the corrected tree (a battery does not survive a tree
change). Tom's pids 45425 / 48399 verified alive across the stop.

## GATES ON THE CORRECTED FINAL TREE

| gate | result | log |
|---|---|---|
| `cargo clippy --workspace --all-targets -- -D warnings` | **exit 0, ZERO error lines** | `scratchpad/final-clippy.log` |
| `cargo test -p aion-rs --no-fail-fast` | **exit 0 — 60 verdict lines, 1115 passed / 0 failed** | `scratchpad/final-aion-rs.log` |
| declared test binaries (derived from THIS tree, before the run) | **223** | `scratchpad/final-declared.txt` |
| `cargo test --workspace --no-fail-fast` | ⏳ **IN FLIGHT** | `scratchpad/final-workspace.log`, completion flag `scratchpad/final-gates.done` |

1115 vs BM's 1113 is the two tests added this tick (the epoch-continuation test
was already counted; the new ones are the timer-gate refusal test and the
server-arm test — the latter is in `aion-server`, so the +2 here is the
timer-gate test plus the `aion-rs` half).

## ON RESUME — DO THIS FIRST

1. Wait for `scratchpad/final-gates.done`, then read `RC_WS=` from
   `final-workspace.log`. Reconcile **within that one run**: count `Running`
   lines against the **223** in `final-declared.txt`, count `test result:` lines,
   read the exit code. A crashed binary emits NO verdict line — the line count
   against declared binaries is the check that sees it. The only expected red is
   **#85** (libsql double-close, `outbox_e2e.rs`, pre-dates this lane by 5 days).
2. **Review 13** on Opus — give it BN, BM, and both audit-found defects.
3. On a PASS only: commit, files named EXPLICITLY, manifest re-derived,
   `SPEC-5-AWL-SUM-COMBINATOR.md` still excluded.

⛔ **DISK: 11 GiB free and falling during the workspace battery.** If it aborts
on space, that is NOT a compile failure — re-check `df` before reading any red.
