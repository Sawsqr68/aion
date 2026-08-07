# SEAT RESUME BU — 2026-08-07 — supersedes BT (read BT for Review 16's verdict in full)

🔴 **THIS IS THE CURRENT AUTHORITY.** Latest letter wins.

## STATE IN ONE LINE

**Review 16's two blocking findings (F-A, F-B) are BOTH CLOSED WITH CODE AND
MUTATION-PROVEN.** Four of five minors closed. Clippy running; battery not yet
run; Review 17 not yet dispatched. **Do not commit until both gates are green
and a review passes.**

---

## §1 — F-A: CLOSED, MUTATION-PROVEN

A refused CANCEL no longer reports itself as a refused FIRE.

Applied:
- `crates/aion/src/runtime/nif_timer_fire.rs` — `wheel_torn_down_before_cancelling`
  added; `wheel_torn_down_after_firing` doc scoped to the fire case;
  `TimerAppendError::WheelTornDown` is a struct variant `{ timer_id, cancelled }`;
  `into_seam_error` dispatches on `cancelled`.
- `crates/aion/src/runtime/nif_timer_bridge.rs` — the construction site now
  computes `(torn_down_timer, torn_down_is_cancel)` from the `TimerOutcome` match
  and moves both into the blocking future. **This is what restored compilation.**
- New test `a_refused_cancel_is_not_reported_as_a_refused_fire` (bridge tests):
  positive control records a `TimerCancelled` before teardown, then the refusal
  must name the cause, must NOT contain `"fired"`, must say
  `"cancellation of timer"`, and must say `"reissues it there"`.

**Mutation:** collapse both `into_seam_error` arms onto
`wheel_torn_down_after_firing` (the pre-fix behaviour, discriminant discarded).
Result: **exactly one RED** — the new test — with the fire test still green. The
mutant's own message was the defect verbatim: *"timer `late-cancel` fired, but
its append was refused … the owning engine re-arms it from there"* about a
cancellation. Restored **byte-identical**, `nif_timer_fire.rs` shasum
`8c7f6f1ced6b424692802c9b66b86db21f858624a43d3c6c3cdd340b0f9bfac7`.

## §2 — F-B: CLOSED, TWO GATES, EACH MUTATION-PROVEN SEPARATELY

The deadline path is the one durable writer the wheel's append boundary cannot
reach (`fire_timer_guarded` demuxes `deadline:{run}` to the handler first). It
now stands down.

Applied in `crates/aion/src/lifecycle/deadline.rs`:
- `WorkflowDeadlineHandler` gains `stand_down: Arc<AtomicBool>`.
- New `DeadlineDisposition::StoodDown` — deliberately NOT `LoseCleanly` (that
  means a competing terminal won; this means the run is unsettled and this engine
  is no longer entitled to speak for it). `drive_timed_out` returns `Ok(())` with
  a DEBUG line and NO teardown.
- **Gate 1, lock-decisive:** inside `decide_disposition`, immediately after
  `recorder.lock().await`.
- **Gate 2, honestly weaker:** in `finalize_timed_out_without_handle`,
  immediately before the first write. It has no handle and therefore no recorder
  lock, so it is CHECK-THEN-ACT and the comment says so and says why it cannot be
  strengthened (inventing a second lock over one workflow is what invariant 3
  forbids).

🔴 **THE WIRING IS UNFAKEABLE BY CONSTRUCTION — this is the part to not undo.**
`register_deadline_handler` (`nif_timer_bridge.rs`) now takes a **constructor
closure** `FnOnce(Arc<AtomicBool>) -> Arc<dyn DeadlineHandler>` and hands it the
bridge's own `shut_down`. A signature taking a finished handler plus a flag would
let a caller pass a fresh flag — compiles, passes every test that sets that flag
itself, gates nothing in production. There is now exactly one place the flag can
come from. `builder_assembly.rs` updated accordingly; the earlier
`timer_wheel_stand_down` accessor was written and then **deleted** in favour of
this.

Tests (both in `runtime::nif_timer_bridge::tests`, both using the REAL latch via
`register_deadline_handler`, both with the control running FIRST on the SAME
bridge and handler so only `shutdown_timer_wheel()` differs):
- `a_torn_down_wheel_does_not_time_a_run_out` — registered path.
- `a_torn_down_wheel_does_not_finalize_an_unregistered_timeout` — registry-free
  finalizer.

**Mutations:** deleting gate 1 → only the first test RED (the mutant recorded the
full teardown including `TimerCancelled` for the deadline). Deleting gate 2 →
only the second test RED. **Orthogonal.** Restored byte-identical both times,
`deadline.rs` shasum
`f5168811a87c396f86b15c767b525607191be301882ede2654f8016ffc9a5efa`.

Test-file helpers added: `seed_armed_deadline` (arms THROUGH the handle's
recorder — writing straight to the store staled the recorder and produced a real
`SequenceConflict`, which is the double-writer indicator doing its job),
`seed_unfinished_timeout`, `timed_out`, `cancelled`. `deadline_tests.rs` gained
`clear_stand_down()` for its 3 construction sites, documented as deliberately NOT
where the gate is measured.

**7/7 bridge tests green** after every restore.

## §3 — MINORS

| # | status |
|---|---|
| 1 `engine_seam.rs` TimerWheel doc | ✅ rewritten — names the third use, points at `is_wheel_teardown`, states a refused cancel claims nothing fired |
| 2 BS §9 wrong hash | ✅ amended in place (struck, not re-asserted, with the cause and the law) |
| 3 BS §9 stale "⏳ RE-RUN REQUIRED" line (**BS:339**) | ❌ **STILL OPEN** — both ran green in §9a; amend |
| 4 F1(c) reverse direction | ✅ closed — `gate_keyword_coverage` now also refuses `explained - used` |
| 5 two hand-typed numbers | ✅ closed — `COMMAND_COUNT` (`run` bodies in the final doc), `STAGE_COUNT` (staged `.awl` count), `spelled()`/`NUMBER_WORDS` so prose reads as words |

Also corrected: **`engine/api.rs`** — entry 3 no longer claims the append
boundary covers the whole wheel; it names BOTH refusal sites and says why the
deadline needs its own. The "only as good as its enumeration" note now reads
**three times proven**, with the new lesson stated: *the enumeration was right
and the mechanism named under it was not*, so each entry must be checked by
following the actual route from wake to append.

## §4 — DECK: RE-RENDERED TO SCRATCH, BYTE-IDENTICAL, TOM'S FILE UNTOUCHED

🔴 **Tom is walking the tutorial NOW.** `docs/authoring/first-workflow.html` was
**NOT** rewritten. The deck was rendered to a scratch path via
`scratchpad/render-check.py` (imports the module, patches `OUTPUT`, patches
`relative_to`) and compared: **`e3f9750820cbbb1bd323080e12cda150ab322600c710b60907a3411eb5b3307f`
on both sides.** So the two newly-derived numbers reproduce the prose exactly —
the derivation is proven correct AND the artifact he opens is unchanged. All
build gates green (5 documents, 13 keywords, 4 verbs, 6 flags, 2 payloads).

**If the deck is ever regenerated in place, re-verify that hash first.**

## §5 — TOM (answered, 2026-08-07 ~00:55Z)

He asked where the tutorial was and whether published versions are current.
Verified and told him:
- Deck says `cargo install aion-cli` and **already warns** against
  `cargo install aion`. Confirmed live on crates.io: `aion` = an unrelated
  stranger's **0.2.0 from 2020**; `aion-cli` = **0.11.0, published 2 Aug**. Deck
  is accurate. (I nearly raised this as a defect — it is not one.)
- His `~/.cargo/bin/aion` reports 0.11.0 (dated 3 Aug), matching the slide.
- His `rustc 1.97.1`; deck asks 1.88+; workspace `rust-version = "1.85"`.
- Published 0.11.0 vs main: **~114 commits since 2 Aug**.
  🔴 **`git rev-list --count v0.11.0..HEAD` returns 2460 and is MEANINGLESS** —
  `git merge-base --is-ancestor v0.11.0 HEAD` is FALSE because the history scrub
  rewrote the tree. Never quote a count across that tag. Use `--since=<date>`.

## §6 — WHAT IS LEFT, IN ORDER

1. **Minor 3** — amend BS:339's stale RE-RUN line.
2. **Clippy** — running in background, task `bvxtoo2gc`, log
   `scratchpad/bu-clippy-ws.log` (ends with a `CLIPPY_RC=` line). `cargo fmt
   --all` already run over the whole tree.
3. **Battery** — `cargo test --workspace --no-fail-fast`, then reconcile.
   🔴 Declared-target regex MUST be `^\s*Running\s+.*?\((target/\S+)\)\s*$`.
   🔴 Load-bearing assertion is **ZERO-VERDICT TARGETS: NONE**.
   Previous good run: 243 declared / 247 verdicts / 243 emitted / 4215 passed / 0
   failed. Expect **+3 tests** now (4218) — one from F-A, two from F-B.
   🔴 #85 (libsql) may fire under contention; it is NOT this lane's.
4. **Review 17** on the new delta. **Do not land on my own say-so** — reviews
   6–16 all refused, and twice the finding was a defect introduced by the
   previous fix. Brief should scope to: F-A, F-B (both gates + the constructor
   seam), the api.rs/engine_seam.rs doc corrections, the tutorial derivations.
5. **Commit in three lots** per BT §6, lists DERIVED FRESH, never `git add -A`.
   ⚠️ `scratchpad/commit-msg-b-fixlane.txt` must be rewritten to cover F-A/F-B.
6. **Declare to Waffles** either way; **tell Tom** when the lane is in.

## §9 — LIVE STATUS (amended 2026-08-07 11:05 AEST, post-compaction #13)

- **Minor 3 (BS:339)** — ✅ closed. §3's table row is stale; all five minors are done.
- **Clippy** — ✅ **GREEN, exit 0**, `CLIPPY_RC=0` in `bu-clippy-ws.log`, zero
  diagnostic lines. `cargo fmt --all` run.
- **Tree identity confirmed against this marker** — `nif_timer_fire.rs`
  `8c7f6f1c…`, `deadline.rs` `f5168811…`. The working tree IS the
  mutation-proven state; nothing drifted across the compaction.
- **Battery** — RUNNING, `cargo test --workspace --no-fail-fast`, log
  `scratchpad/bu-workspace.log` (ends `RC_WS=`). Started ~11:03. Box has 78 GiB
  free. Re-invokes on exit.
- **Review 17** — ✅ DISPATCHED on Opus. Brief committed to
  `scratchpad/review-17-brief.md`. Read-only: the agent is barred from cargo
  (battery holds the lock), from any file write, and from mutating git. Brief
  attacks specifically: whether gate 1 is *actually* lock-decisive, whether gate
  2's stated impossibility is true or a husk, whether a THIRD durable write
  route exists on the deadline path that neither gate covers, whether
  `shut_down` is the right flag (window where it is false but the run is
  already lost), and whether the constructor-closure seam is genuinely the only
  install path.
- 🔴 **TEARDOWN DELIBERATELY HELD** until the battery lands. Deleting the
  proving clones now creates disk contention, and #85 (libsql) fires under
  contention — a contended red is not a red, and it would cost a full re-run.
  Do it AFTER `RC_WS=` appears.

## §7 — RESTORE ARTEFACTS (scratchpad)

`fa-restore.rs` (nif_timer_fire), `fb-restore.rs` + `fb2-restore.rs` (deadline),
`render-check.py`, `deck-check.html`, `mutate-fa.py`, `bu-clippy-ws.log`.

## §8 — STANDING CONSTRAINTS

Unchanged — see **BT §7** in full (no worktrees, one mutating lane, never
`git add -A`, no `git gc`/prune, delete nothing, Tom's pids 45425/48399/50822
untouched, worker rebuilds frozen, don't edit CLAUDE.md, `-- crates docs` scoping
on `ls-files --others`). Teardown owed is **BT §9**, still owed.
