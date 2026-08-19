# Charter: aion#1 — workflow.now() must be replay-position time, not history tail

**Date:** 2026-08-18 · **Steward:** Vesper Lynd · **Worktree:** `stack/aion-wt-1-replay-now`, branch `1-replay-position-now` off `d4317268f`
**Issue:** #1 `workflow.now() returns full-history tail, not replay-position time — breaks determinism on resume`

## The defect (steward-verified at the bytes, this tree)

- `now_from_context` (`crates/aion/src/runtime/nif_determinism.rs:215-220`) answers from
  `NifContext::last_recorded_at` — set at context construction to `history.last()`
  of the current run segment (`nif_context.rs:118`).
- On a resumed run the tail is the wake event (`TimerFired@T_wake`), so EVERY
  `now()` during replay returns `T_wake`. A workflow that computed
  `started = now()` early and later `elapsed = now() - started` sees `elapsed ≈ 0`
  after any recovery/resume. Determinism invariant #2 (CLAUDE.md) violated.
- The position-stable model already exists and is unit-tested but has ZERO
  production callers: `durability/determinism.rs::DeterminismContext`
  (`advance_to_recorded_at`/`now`) and `durability/replay.rs::Replay::step`
  (advances on `Recorded { recorded_at }` AND on
  `ResumeLive { recorded_at: Some(t) }`).
- `random()` is position-stable (handle-owned `next_deterministic_nif_sequence`);
  `now()` is the only inconsistent determinism primitive.
- The masking: `replay_behaviour.rs:691` tests `Replay::now()` — the unwired
  path — not the NIF path production calls.

## Architecture facts the fix must respect

1. Each NIF call constructs a **fresh** `NifContext` (fresh `Resolver` over the
   full re-read run segment). Cross-call run-scoped state lives ONLY on
   `WorkflowHandle` as shared `Arc` internals (ordinal counters, deterministic
   sequence, signal counts) — `registry/handle.rs:126-166`. A re-spawned run
   (recovery, continue-as-new) gets a fresh handle: counters restart and replayed
   code re-derives them. The now-cell must follow exactly this pattern.
2. ALL command consumption funnels through `NifContext::resolve_command`
   (`nif_context.rs:494`) — activities (await/dispatch/in_vm), timers, signals,
   children, timeouts. `nif_collect.rs` deliberately scans the run segment
   directly (documented at its head) — check whether collect settlement needs
   its own advance and answer it explicitly in the report, don't guess.
3. Live (post-resume-point) consumption records through the `NifContext`
   `record_*` methods with an explicit `recorded_at` (e.g.
   `nif_activity_await.rs::take_runtime_completion` records
   ActivityCompleted/Failed at `chrono::Utc::now()` and returns the value to the
   workflow in the same step). The recorded_at handed to the recorder IS the
   recorded event timestamp workflow-visible time must advance to.

## The fix (design, agreed with the steward — deviations must be argued in the report)

**A. Position cell on the handle.** Add a run-scoped workflow-visible-now cell
to `WorkflowHandle`'s shared internals (e.g. `Arc<AtomicI64>` of recorded_at
millis, 0/sentinel = never advanced — or an equivalent typed cell; pick what
survives clippy strictness and say why). Monotonic advance only (`fetch_max`
semantics): concurrent NIF calls must never move it backwards.

**B. Seed.** When the cell has never advanced, `now()` answers with the current
run segment's `WorkflowStarted.recorded_at` (`history.first()`), NOT
`history.last()`. `NifContext` construction switches its cached field
accordingly. Empty history keeps the existing loud error.

**C. Replay/recorded advance.** `NifContext::resolve_command` switches to
`resolve_with_consumed` internally and advances the cell exactly as
`Replay::step` does: on `Recorded { recorded_at }` and on
`ResumeLive { recorded_at: Some(t) }`. The public `ResolveOutcome` signature to
existing callers stays unchanged.

**D. Live advance.** The `NifContext` `record_*` seams that mark a
workflow-consumed event (activity completed/failed/cancelled, scheduled/started
at dispatch, fan-out completion settlement, and the timer/signal/child
equivalents wherever they record through the context on the workflow thread)
advance the cell to the `recorded_at` they hand the recorder. **Replay-parity
rule — this is the correctness core:** for every seam, the value live `now()`
serves after that seam must equal the value replayed `now()` serves at the same
code position via C. Audit each seam against what the resolver returns for the
corresponding command on replay; document the parity argument per seam in the
report. Where the resolver's recorded resolution advances to a terminal's
recorded_at, the live path must advance at the terminal's recording, and any
intermediate live advance (e.g. at ActivityScheduled) is permitted ONLY if
replay also advances to that same event at the same position (check what
`resolve_with_consumed` reports as consumed recorded_at for the dispatch-shaped
commands).

**E. Asynchronous arrivals stay invisible until consumed.** A signal/timer/child
event appended by the services while the workflow is running must NOT move
`now()` before the workflow consumes it. The tail-read dies with this change;
nothing may reintroduce it.

**F. No behavior change to `random()`/`random_int()`.**

## Tests (red first — commit the red before the fix, house rule)

1. **The defect, literally (unit, nif_determinism tests):** resumed-run-shaped
   history — WorkflowStarted@T0, TimerStarted@T1, TimerFired@T_wake — fresh
   context (fresh handle = new execution's first NIF call), `now_from_context`
   BEFORE any command resolution must return **T0**. Red today (returns T_wake).
2. **Advance-under-consumption:** same history; resolve the sleep command
   (recorded TimerFired) through `resolve_command`, then `now()` returns T_wake.
   Asserts C.
3. **Elapsed≈0 symptom pin (integration, the issue's flagship shape):** a
   workflow that takes `started = now()`, sleeps (recorded fire), then computes
   `elapsed = now() - started` — on a REPLAYED execution elapsed must equal
   T_wake−T0, not ≈0. Place it with the existing replay-behaviour or
   resume e2e suites; drive the PRODUCTION NIF path, not `Replay::now` (that
   masking is part of the defect).
4. **Live parity:** live execution (record through the context seams) and its
   replay serve identical `now()` sequences. Extend
   `replay_sequence_matches_recorded_history_and_seeded_positions` or twin it.
5. **Async-arrival invisibility (E):** append a SignalReceived to history that
   the workflow has not consumed; `now()` unchanged. Red today (tail moves).
6. Existing tests that pin tail behavior (`now_returns_last_recorded_event_timestamp_millis`,
   `each_engine_state_resolves_its_own_context_source`) are re-cast to the new
   contract — with a comment stating the OLD assertion was the defect. Do not
   delete coverage; re-aim it.

## Gates

- Red receipts committed before the fix (test names + failing output in the
  lane's gate-logs or commit messages).
- Extraction battery per `ablative/docs/GATE-EXTRACTION-STANDARD.md` on the
  final tree — `NORN_BIN=/Users/tom/.cargo/bin/norn`, per-leg stderr + Summary
  kept as completion proof. NO fmt-check leg; autoformat instead.
- `cargo clippy --workspace --all-targets -- -D warnings` clean, no `#[allow]`,
  no `#[ignore]`, no weakened assertions.
- BOTH clippy resolutions per house note: `--workspace` AND `-p aion-rs`.
- REPORT.md in the worktree root: per-seam parity table, red receipts, battery
  summary, files touched, anything discovered that this charter got wrong
  (STOP and report rather than silently widening — house rule).

## Out of scope

- `nif_collect`'s direct scan MAY need an advance seam — investigate and either
  wire it with a parity argument or state precisely why settlement cannot be a
  now()-observable position change. Do not skip the question.
- Recovery `as_of` expired-timer decisions (documented as intentionally
  non-workflow-visible in determinism.rs) — untouched.
- The `Replay` driver itself — it is already correct; do not fork its logic,
  mirror it.
