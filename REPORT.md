# aion#1 — `workflow.now()` is the replay position, not the history tail

**Branch:** `1-replay-position-now` (worktree `stack/aion-wt-1-replay-now`, off `d4317268f`)
**Charter:** `gate-logs/1-replay-now/CHARTER.md` (main tree, read-only)

---

## 1. The defect, confirmed at the bytes

`nif_determinism::now_from_context` answered from `NifContext::last_recorded_at`,
which `nif_context.rs:118` assigned as:

```rust
let last_recorded_at = history.last().map(|event| *event.recorded_at());
```

— the **tail of the current run segment**. On a resumed run the tail is the wake
event, so every `now()` a replayed execution makes answers the wake timestamp:
the run's own future at every position before it consumes the sleep.

Confirmed by execution, not by reading. The e2e
(`crates/aion/tests/replay_now_position_e2e.rs`) seeds a store with
`WorkflowStarted@T0`, `TimerStarted@T1`, `TimerFired@T_wake`, lets startup
recovery re-spawn and replay the run through the real
`aion_flow_ffi:now/0` / `aion_flow_ffi:sleep/1` natives, and reads back the span
the workflow computed:

```
left: 0   right: 3600000
```

A workflow that measures its own elapsed time reports **zero** after every
recovery. `random()` was already position-stable (handle-owned draw sequence);
`now()` was the only inconsistent determinism primitive.

A second consumer of the same tail read was found and is part of the defect:
`nif_timer::recorded_now` — the base a timer's `fire_at` is computed from, and
the `recorded_at` a `TimerStarted` is stamped with. Both were therefore
functions of unconsumed asynchronous arrivals.

### The masking the charter named

`crates/aion/tests/replay_behaviour.rs:691`
(`replay_determinism_round_trip_uses_recorded_now`) tests `Replay::now()` — the
`DeterminismContext` path, which is correct and has **zero production callers**.
The production NIF path was untested for position. That is why the defect
survived: two implementations of one rule, and the tests checked the one
production does not use. The new tests all drive the production path.

---

## 2. Red receipts (committed before the fix)

Commit `d1931fb5b` — `test(1): red — workflow.now() answers the history tail,
not the replay position`. Full output in
`gate-logs-local/red-unit-nif-determinism.log` and
`gate-logs-local/red-e2e-replay-now-position.log`.

| Test | Failing assertion (left = actual, right = expected) |
| --- | --- |
| `replayed_sleep_reports_the_recorded_span_not_zero_elapsed` (e2e) | `left: 0` / `right: 3600000` |
| `now_before_any_consumption_is_the_run_start_not_the_history_tail` | `"1700000999456"` / `"1700000000123"` |
| `resumed_run_answers_the_run_start_before_the_sleep_is_consumed` | `"1700003600000"` / `"1700000000000"` |
| `consuming_a_recorded_command_advances_now_to_that_event_only` | `"1700007200000"` / `"1700003600000"` |
| `an_unconsumed_async_arrival_never_moves_now` | `"1700007200000"` / `"1700000000000"` |
| `elapsed_across_a_replayed_sleep_is_the_recorded_span_not_zero` | `0` / `3600000` |
| `live_and_replayed_activity_serve_identical_now_sequences` | `["…000","…000","…000"]` / `["…900","…900","…900"]` |

**Honest note on the red wave.** Four of the seven were written against the API
that existed at red time (`NifContext::resolve_command`), because a test that
does not compile has not run and is not a receipt. In the fix commit those four
lines change API name only — `resolve_command` → `resolve_command_observed` /
`resolve_command_unobserved`, plus one added `observe_recorded_at` call in the
parity test. **No assertion changed between red and green.** The exact diff is
visible as `git diff d1931fb5b..HEAD -- crates/aion/src/runtime/nif_determinism.rs`.

Two pre-existing tests were **re-cast, not deleted** (charter item 6), each
carrying a comment stating the old assertion was the defect:

- `now_returns_last_recorded_event_timestamp_millis` →
  `now_before_any_consumption_is_the_run_start_not_the_history_tail`. The tail
  value is **kept in the fixture** so a regression to the tail read fails here.
- `each_engine_state_resolves_its_own_context_source` — asserted the second
  engine's tail. Re-aimed at each run's own start; the two runs are seeded with
  distinct `WorkflowStarted` timestamps, so the cross-engine isolation property
  it exists for is still what distinguishes a pass from a failure.

---

## 3. The fix

### A. The position cell (`crates/aion/src/registry/handle.rs`)

`WorkflowHandle` gains `workflow_now_millis: Arc<AtomicI64>` (line 147), seeded
to the `WORKFLOW_NOW_UNSET` sentinel, with:

- `advance_workflow_now(DateTime<Utc>)` (line 208) — `fetch_max` on
  `timestamp_millis()`. Monotonic: concurrent NIF calls cannot move the position
  backwards, and a repeated value is a no-op.
- `workflow_now_millis() -> Option<i64>` (line 219).

Run-scoped exactly like the existing ordinal counters: a re-spawned run
(recovery, continue-as-new) gets a fresh handle whose cell starts unset, and its
replayed code re-derives the same positions.

**Why milliseconds.** `workflow.now()` has always served millisecond timestamps,
so the store must already round-trip `recorded_at` to at least that resolution
for replay to reproduce a live run. Truncating in the cell keeps the *live*
advance (from an in-memory `Utc::now()`, nanosecond-precise) and the *replayed*
advance (from the persisted event) **exactly equal** instead of differing below
the workflow-visible resolution. A sub-millisecond mismatch would be invisible
in `now()` but would perturb `recorded_now` → `fire_at`.

**Why `fetch_max` and not a plain store.** Because the cell is a maximum, the
ORDER in which one workflow step applies several consumed timestamps does not
affect the result — only the SET does, and the set is what replay reconstructs.
That property is what makes the fan-out settlement seam provable at all.

### B. The seed / floor (`crates/aion/src/runtime/nif_context.rs:126`)

`NifContext` now caches `run_started_at = history.first()` — this run segment's
`WorkflowStarted`, since `current_run_segment` slices from it.
`NifContext::workflow_now()` (line 244) answers `max(run_started_at, cell)`.

The start is a **floor**, not merely a seed. `recorded_at` is not guaranteed
monotonic with `seq` (a reused terminal exit instant can be stamped earlier than
the event that follows it — `lifecycle/completion_retry`), so the maximum of the
two is taken rather than the cell alone. Empty history keeps the existing loud
`"now: workflow history is empty"` error; nothing substitutes a clock.

An unrepresentable millis value in the cell (unreachable — every value came from
a `DateTime<Utc>::timestamp_millis()`) is logged at `error!` and answered with
the run start. No silent fallback.

### C/D. The two-method resolve split, and the per-seam rule

`NifContext::resolve_command` is **replaced** by two methods
(`nif_context.rs:570`, `:606`), so every existing call site had to state its
choice and the compiler refuses a default:

- `resolve_command_observed` — resolves *and* advances, mirroring
  `Replay::step` exactly (advance on `Recorded { recorded_at }` and on
  `ResumeLive { recorded_at: Some(_) }`).
- `resolve_command_unobserved` — resolves without moving the position.

`NifContext::observe_recorded_at` (line 277) is the live counterpart, called at
the moment a seam records the outcome it is about to return.

**The governing rule (charter §D):** for every seam, the value live `now()`
serves after that seam must equal the value a replayed execution serves at the
same code position. The observable positions are the points where a NIF
*returns to workflow code* — recordings made while the workflow is parked inside
a NIF are not separately observable.

**Deviation from the charter's §C, argued.** The charter specified that
`resolve_command` itself advance. Taken literally that breaks §D at five seams:
`resolve_with_consumed` reports the **terminal's** `recorded_at` for a
dispatch-shaped command, and the dispatch/arm/cancel seams return to workflow
code at a position where the live run has recorded only the *anchor*. §D is the
governing rule and §C the mechanism, so the mechanism was split rather than the
rule bent. Both new names are new, so no existing call site silently kept the
old behaviour.

---

## 4. Per-seam replay-parity audit

Every `NifContext` resolve/record seam reachable from the workflow thread, plus
`nif_collect`'s direct scan.

### Observing seams — the position moves

| # | Seam | What replay does | What live does | Parity argument |
| --- | --- | --- | --- | --- |
| 1 | `nif_activity_await.rs:162` `recorded_resolution_for` (`await_activity_result`) | `Recorded{recorded_at = activity terminal}` → advance | resolves `ResumeLive`, then #2/#3/#4 record the terminal and advance to the same `recorded_at` | The terminal live records **is** the event replay resolves here. Same value, same position. |
| 2 | `nif_activity_await.rs:208` record `ActivityCompleted` | — (never runs on replay) | advance to the `recorded_at` handed the recorder | Returned to workflow code in this same step; #1 replays it. |
| 3 | `nif_activity_await.rs:229` record terminal `ActivityFailed` | — | advance | As #2. |
| 4 | `nif_activity_await.rs:140` scope-expiry terminal `ActivityFailed` | — | advance | The durable timeout failure is the value replay returns verbatim through #1. |
| 5 | `nif_timer.rs:85` `sleep` | `Recorded(TimerFired/TimerCancelled)` at the terminal's `recorded_at` → advance | **identical path**: the timer service records the terminal and wakes the process; the woken pass resolves it here | Both worlds reach the answer through *this* resolution. The first live pass resolves `ResumeLive`, records `TimerStarted`, and parks **without returning to workflow code** — not an observable position. |
| 6 | `nif_signal.rs:334` `receive_signal` | `Recorded(SignalDelivered)` at the arrival's `recorded_at` → advance | **identical path**: the router records `SignalReceived` durably *before* waking; this native never records an arrival | As #5. An arrival no receive has consumed moves nothing (invariant E). |
| 7 | `nif_signal.rs:427` `send_signal` | `Recorded(SignalSent)` → advance | records `SignalSent` at `recorded_at` and advances to it (`:460`) | The event live records is the event replay resolves. |
| 8 | `nif_child.rs:103` `spawn_child` | `Recorded(ChildStarted)` at `ChildWorkflowStarted.recorded_at` → advance | records `ChildWorkflowStarted` and advances to it (`:144`) | As #7. The child id is recorded nondeterminism; the `recorded_at` travels with it. |
| 9 | `nif_child.rs:343` `await_child` | `Recorded(ChildCompleted/Failed)` at the terminal's `recorded_at` → advance | **identical path**: the child watcher records the terminal into the parent's history and wakes it | As #5/#6. |
| 10 | `nif_collect_settlement.rs:362` `observed_terminal` (5 settlement read sites) | reads every member terminal from history → advance | reads the ones already recorded; records the rest (#11/#12) | See the fan-out argument below. |
| 11 | `nif_collect_settlement.rs:222`/`:249` `take_and_record` | — | advance to the `recorded_at` of the terminal it records | Part of the settled answer; #10 replays it. |
| 12 | `nif_collect_settlement.rs:281` `record_cancelled` | — | advance | The cancellation set is part of the settled answer; #10 replays it. |

### Non-observing seams — the position does not move

| # | Seam | Why not |
| --- | --- | --- |
| 13 | `nif_activity_dispatch.rs:338` `dispatch_activity` | **The load-bearing one.** The dispatch hands workflow code a *correlation id*, not the outcome. Recorded resolution here consumes `[Scheduled … terminal]` and reports the **terminal's** `recorded_at` — the live path at this position has only just recorded `ActivityScheduled` and cannot know the completion time. Advancing would make a replayed run serve a timestamp its live original could not have served. Both worlds therefore hold position, and #1 moves it at the await. |
| 14 | `nif_activity_in_vm.rs:157` `dispatch_activity_in_vm` | Identical shape to #13. |
| 15 | `nif_activity.rs::record_started` → `record_activity_scheduled_started` | Records `ActivityScheduled`+`ActivityStarted`. Not advancing is forced by #13: replay never advances to the anchor at that position. |
| 16 | `nif_activity_dispatch.rs::record_superseded_attempt` (#266 Defect B) | Records a **retryable** `ActivityFailed`. Replay skips over it inside the matched range and advances to the terminal at the await (#1), so a live advance here would be live-only. |
| 17 | `nif_activity_dispatch.rs::open_this_delivery` adoption offer (#36) | `ActivityAdoptionOffered` carries no family/key; the cursor never matches it, so replay never advances to it. |
| 18 | `nif_timer.rs:192` `start_timer` | Arms a timer and returns its handle immediately; never waits for the terminal. Recorded resolution can reach `TimerFired`, an event the live path had not recorded when it returned the same handle. Same class as #13. |
| 19 | `nif_timer.rs:218` `cancel_timer` | The live path issues the cancel through the timer service, which arbitrates fire-versus-cancel and owns the terminal's `recorded_at`; this NIF never learns that timestamp and so **cannot** advance to it. Advancing only on the replay side would be a replay-only position change, so neither side advances and the two agree. |
| 20 | `nif_timeout.rs:223` `with_timeout` `arm_scope` | Arming hands workflow code the wrapped closure, not the race outcome. Recorded resolution can reach the deadline's `TimerFired`, which the live arm had not recorded when it ran the same closure. Same class as #13. |
| 21 | `nif_timeout.rs::resume_with_timeout` / `settle_live_scope` | No resolve at all: the outcome is derived by scanning recorded history on **both** sides, and neither side moves a position. Symmetric by construction. |
| 22 | `nif_timer.rs::record_started` (live `TimerStarted` for sleep / scope) | Stamped with `recorded_now`, i.e. the **current** position — advancing to it is a no-op by construction (`fetch_max` of an equal value). |
| 23 | `nif_collect.rs:270` `recorded_terminal` probe + `record_fan_out_dispatch` / `rearm_outbox_pending` / `record_activity_scheduled_started` | The fan-out **dispatch staging** seam, same class as #13/#15: it stages `Scheduled`/`Started` and returns nothing of the outcome. Kept on the read-only `recorded_terminal`, not `observed_terminal`. |
| 24 | `nif_continue_as_new.rs:70` | Records the run's **terminal** `WorkflowContinuedAsNew` and the process ends. Nothing observes a position afterwards; the replacement run gets a fresh handle and a fresh run segment. |
| 25 | `nif_activity_retry_dispatch.rs` retry-loop records (`AttemptFailed` / `AttemptStarted` / `AdvisoryExhausted`) | Run **off the workflow thread**, on the completion task, through the `Recorder` directly — never through a `NifContext`. Structurally cannot move the position. |
| 26 | Timer / signal / child-watcher service appends | Same as #25: asynchronous arrivals, appended by third parties, invisible until a workflow seam consumes them (invariant E). |
| 27 | `nif_query*` | Read-only by construction; the servicing guard already refuses every recording NIF inside a query handler. |

### The fan-out (`collect_*`) settlement — the charter's explicit open question

`nif_collect` deliberately does not use `resolve_command`; it scans the run
segment directly (documented at its head, because the resolution path rejects
`ActivityCancelled`). The charter asked for a decision, not a guess.

**Decision: it is wired, and it must be.** Leaving it unwired is *deterministic*
(neither side advances) but wrong in a way that matters: `recorded_now` — the
base for every subsequent `fire_at` and `TimerStarted` — would freeze across a
fan-out, so a `sleep` after a `collect_all` would compute a deadline already in
the past and fire immediately.

**The parity argument.** A collect settlement is a total function of the
per-ordinal recorded terminal SET. The set is exactly what a settling pass
touches: terminals it reads from its history snapshot (#10) and terminals it
records in this pass (#11/#12). Because the advance is a monotonic **maximum**,
the ORDER does not matter — only the set does. A replayed settling pass reads
all of them from history; a live settling pass reads the earlier ones from its
snapshot and records the rest itself, with the same `recorded_at` values that
end up in history. Same set → same maximum → same `now()`.

Non-settling live passes never return to workflow code, so their partial
advances are not observable positions, and a live pass can only ever have
advanced over a *subset* of what the settling pass sees.

This parity is exactly as strong as the settlement decision's own determinism —
same snapshot, same first-match scan, both sides — and no stronger. It
introduces no new divergence source. See §6.2 for a pre-existing hazard found
while deriving this.

### E. Asynchronous arrivals stay invisible

Pinned structurally (#25, #26 — no service path holds a `NifContext`) and by
test (`an_unconsumed_async_arrival_never_moves_now`,
`consuming_a_recorded_command_advances_now_to_that_event_only`).

### F. `random()` / `random_int()` unchanged

Untouched. `replay_sequence_matches_recorded_history_and_seeded_positions` still
covers the draw sequence.

### The tail read is dead

```
$ grep -rn 'last_recorded_at' crates/ sdks/
crates/aion/src/lifecycle/completion_retry.rs:83:  ///   `NifContext::last_recorded_at`, i.e. `history.last()`, the run
```

One hit, inside the retracted-reasoning history in `completion_retry.rs` (§5),
where naming the dead mechanism is the point. The method no longer exists.
Remaining `history.last()` uses in the crate are sequence-head reads for the
recorder resume position and test assertions — none is workflow-visible time.

---

## 5. `completion_retry.rs` — the re-derivation its own comment demanded

`TerminalIntent::exit_time` carried a red instruction: *"When that is fixed,
re-derive this bullet against the new mechanism — the reasoning above does not
survive the change, and assuming it does is the exact mistake the retracted
bullet made."* Done, and both prior revisions are retracted in place with their
history intact.

The conclusion holds, for a **stronger** reason than either retracted version.
The position advances only to the `recorded_at` of an outcome the workflow has
consumed. A workflow terminal is never such an outcome: the resolver rejects
every workflow-terminal event as a history-shape fault rather than a recorded
command outcome, and no `observe_recorded_at` seam records one. A reused
`exit_time` is therefore not a value the cell can hold on any path, live or
replayed. The old reopen-ordering argument is no longer load-bearing, and the
`fetch_max`-over-a-start-floor construction independently forbids
workflow-visible time from moving backwards.

---

## 6. Open questions for the steward

1. **The dispatch anchor (seam #13) — a deliberate STOP, per the charter.**
   The charter permits an intermediate live advance at `ActivityScheduled`
   "ONLY if replay also advances to that same event at the same position", and
   directs a check of what `resolve_with_consumed` reports for dispatch-shaped
   commands. It reports the **terminal's** `recorded_at`, not the anchor's, so
   the charter's own gate refuses the advance. I did not widen past it.

   The consequence is bounded and, today, unreachable from the SDK: a run that
   dispatches an activity and **never awaits it** holds its position across that
   dispatch. `gleam/aion_flow/src/aion/internal/activity_dispatch.gleam` keeps
   `dispatch/2` and `await/1` private and pairs them in every public combinator,
   so no authored workflow can observe the gap. It is also strictly *better*
   than the previous behaviour, which made the same `fire_at` depend on
   unconsumed asynchronous arrivals.

   Closing it properly would mean teaching `ResolvedCommand::Recorded` to carry
   the **last `ActivityScheduled` in the matched range** alongside the terminal,
   and advancing to that at the dispatch seam. I traced it: it holds for the
   fresh dispatch, the reopen re-dispatch (#222), the retry trail (retries
   record `ActivityStarted`, not `Scheduled`) and the supersession record
   (#266), but the **adoption** branch (#36) records no new `Scheduled` and
   would need its own history lookup to stay in step. Three branches to keep
   synchronised inside the durability layer, for a gap the SDK cannot reach.
   That is a separate change with its own charter, not a silent widening of this
   one.

2. **A pre-existing fan-out hazard, found while deriving the collect argument —
   not introduced here, and not touched.** `recorded_terminal` treats *any*
   `ActivityFailed` for a member ordinal as that member's terminal, without
   checking `error.kind`. A **retryable** failure recorded by the off-thread
   retry loop (`nif_activity_retry_dispatch`) therefore settles a fan-out member
   as `Failed`. If such a failure lands after a live sweep's snapshot but before
   the replay's, live can settle from the runtime map while replay settles from
   the retryable failure — a *settlement* divergence, independent of `now()`.
   My position advance inherits exactly that determinism and no less; it does
   not create it. Flagging it rather than fixing it inside this lane.

3. **`cancel_timer` (seam #19) holds position on both sides.** The workflow does
   observe a terminal there, but the terminal's `recorded_at` is owned by the
   timer service's first-recorded-wins arbitration and is not returned to the
   NIF. Symmetric non-advance is the only reproducible answer available. If the
   steward wants that position to move, the timer service would have to return
   the settled terminal's `recorded_at` to the cancelling NIF.

4. **`Replay` / `DeterminismContext` are still unwired in production.** The
   charter put the driver out of scope and this change mirrors it rather than
   forking it, so `durability/replay.rs` and `durability/determinism.rs` remain
   a second, now-agreeing implementation of the same rule with no production
   caller. "One truth in two places has drifted" — the cure is subtraction, and
   that is its own change.

---

## 7. Files touched

| File | Change |
| --- | --- |
| `crates/aion/src/registry/handle.rs` | position cell (`:147`, `:171`), `advance_workflow_now` (`:208`), `workflow_now_millis` (`:219`), `WORKFLOW_NOW_UNSET` |
| `crates/aion/src/runtime/nif_context.rs` | `run_started_at` replaces `last_recorded_at` (`:126`); `workflow_now` (`:244`); `observe_recorded_at` (`:277`); `resolve_command_observed` (`:570`) / `resolve_command_unobserved` (`:606`) replace `resolve_command`; `position_resolver_for` |
| `crates/aion/src/runtime/nif_determinism.rs` | `now_from_context` reads `workflow_now` (`:226`); six new/re-cast tests |
| `crates/aion/src/runtime/nif_timer.rs` | `recorded_now` reads `workflow_now` (`:372`); sleep observed (`:85`); `start_timer` / `cancel_timer` unobserved (`:192`, `:218`) |
| `crates/aion/src/runtime/nif_timeout.rs` | `arm_scope` unobserved (`:223`) |
| `crates/aion/src/runtime/nif_activity_await.rs` | await observed (`:162`); three live observes (`:140`, `:208`, `:229`) |
| `crates/aion/src/runtime/nif_activity_dispatch.rs` | dispatch unobserved (`:338`) |
| `crates/aion/src/runtime/nif_activity_in_vm.rs` | in-VM dispatch unobserved (`:157`) |
| `crates/aion/src/runtime/nif_signal.rs` | receive/send observed (`:334`, `:427`); live send observe (`:460`) |
| `crates/aion/src/runtime/nif_child.rs` | spawn/await observed (`:103`, `:343`); live spawn observe (`:144`) |
| `crates/aion/src/runtime/nif_collect_settlement.rs` | `recorded_terminal_event` / `terminal_state` split; `observed_terminal` (`:362`) at 5 settlement reads; live observes (`:222`, `:249`, `:281`) |
| `crates/aion/src/lifecycle/completion_retry.rs` | the demanded re-derivation |
| `crates/aion/tests/replay_now_position_e2e.rs` | new recovery-replay e2e |
| `crates/aion/tests/fixtures/aion_fixture_workflow.erl` / `.beam` / `README.md` | `now_span/1` |

---

## 8. Gates

| Gate | Result |
| --- | --- |
| `cargo test -p aion-rs` (63 binaries) | **1187 passed, 0 failed, 0 ignored**, exit 0 — `gate-logs-local/green-aion-rs-full.log` |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 — `gate-logs-local/clippy-workspace.log` |
| `cargo clippy -p aion-rs --all-targets -- -D warnings` | `gate-logs-local/clippy-aion-rs.log` |
| `cargo fmt` (autoformat, never `--check`) | applied to the whole tree |
| Extraction battery (`ablative/docs/GATE-EXTRACTION-STANDARD.md`) | `gate-logs-local/battery/` |

No `#[allow]`, no `#[expect]`, no `#[ignore]`, no `_var` renames, no
`#[cfg(any())]`, no weakened assertion. No `unwrap`/`expect`/`panic` added.
