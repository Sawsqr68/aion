# SEAT RESUME — marker BB (2026-08-06)

**Supersedes BA for STATE.** BA's account of findings 3/4/5 is now **partly
wrong** and must be read through this marker. Vesper Lynd, session
`5849e0d8-4802-4869-8e0f-9f7fd187e198`.

# 🔴 REVIEW 5 = **NOT SAFE TO LAND**. MY FINDING-5 FIX IS A REGRESSION.

Do **not** commit the tree as it stands.

---

## BLOCKER 1 — the finding-5 guard strands every reopened run

**`crates/aion/src/lifecycle/completion.rs`**, the F11 guard, currently:

```rust
if matches!(*progress, TerminalProgress::Recorded)
    || run_segment_was_reopened(&history, handle.run_id())
{
```

**VERIFIED AT MY OWN HAND, not taken from the reviewer:**
`crates/aion/src/lifecycle/reopen.rs:129` is
`record_workflow_reopened(Utc::now(), run.clone(), reopened)` — **reopen reuses
the SAME `run_id`**, and `:148` `respawn_and_register(&context, id, run, …)`
re-registers under it. So `WorkflowReopened` sits in that run's segment
**permanently**, and `run_segment_was_reopened` is true for the rest of the run's
life.

**Failure scenario (reviewer CONFIRMED BY EXECUTION with its own probe):** run R
fails → operator reopens → lease 2 runs and exits **normally**. Lease 2's own
exit callback enters `handle_process_exit_async` with `progress = NotRecorded`.
`terminal_outcome_from_history` → `None` (correct, lease 2 has no terminal yet).
`run_segment_was_reopened` → `true`. Guard fires, `return Ok(())`, **nothing
appended**.

Consequences, all on the ordinary success path:
1. No terminal ever lands → the run projects **`Running` forever**. A permanent
   zombie in a durable workflow engine.
2. `handle.completion().notify(...)` is **skipped** → every `await_result` caller
   and every parked child-terminal watcher hangs for the epoch.
3. Visibility index stays `Running`.
4. Returns `Ok`, so **nothing is logged at any level**. Silent.

Reviewer's probe `probe_a_reopened_run_that_finishes_again_records_its_terminal`
went RED (`left: Running`, `right: Completed`) and goes **GREEN** under the
pre-fix code. **My fix caused this.**

### 🔴 Why the predicate cannot be repaired by adjusting it

`handle_process_exit_async(context, handle, outcome)` is called **identically**
for a stale lease-1 exit callback and a genuine lease-2 exit. "Was there ever a
reopen in this segment" is **invariant to the difference**, so it cannot separate
them — it can only choose which one to break. My new test
(`a_reopened_run_is_not_appended_into_by_a_monitor_that_never_recorded`) and the
reviewer's probe are **mutually exclusive under any formulation of a
history-only predicate**.

*(This is the estate law "a verdict invariant to the error cannot see it",
walking straight into my own fix.)*

### THE REMEDY — lease identity, not history

The real invariant is **"am I a superseded lease's monitor"**, and the
discriminator exists:

- `recover_active_workflow` (`reopen.rs:408-421`) returns a **fresh pid**;
  `register_recovered_resident` builds a **new `WorkflowHandle`** around it.
- The stale monitor's closure captured the **OLD** handle.
- `context.registry.get(workflow_id, run_id)` returns the **CURRENT** one
  (`crates/aion/src/registry/table.rs:159`, `-> Result<Option<WorkflowHandle>, EngineError>`).
- `context.registry` is **already in `ProcessExitContext`** — no plumbing needed.

Intended shape (conservative — only treat as superseded when a **different live
lease is positively observed**, so an absent/unreadable registry entry falls
through to the pre-existing behaviour and cannot regress anything):

```rust
let superseded_by_a_newer_lease = context
    .registry
    .get(handle.workflow_id(), handle.run_id())
    .ok()
    .flatten()
    .is_some_and(|current| current.pid() != handle.pid());
if matches!(*progress, TerminalProgress::Recorded) || superseded_by_a_newer_lease {
```

⚠️ **VERIFY BEFORE WRITING:** that `WorkflowHandle` exposes `pid()` — it is used
at `reopen.rs:148` (`handle.pid()`), so it exists, but confirm the visibility and
type. I had not yet confirmed the accessor when the compact landed.

Then **delete `run_segment_was_reopened`** (it becomes dead code — CLAUDE.md
forbids leaving it) and rewrite its call-site comment against the new mechanism.

**Both directions must be proven RED:**
1. Keep BOTH tests: mine (stale lease must not append) and a permanent version of
   the reviewer's probe (`a_reopened_run_that_finishes_again_records_its_terminal`).
   Write the probe FIRST and watch it go RED against the current tree — that is
   the regression proof.
2. Mutation: drop the pid disjunct → my test red. Mutation: drop the
   `progress == Recorded` disjunct → whichever test covers it red (if nothing
   goes red, that disjunct is unreachable and must be **deleted, not kept**).

---

## BLOCKER 2 — the `not_owner` arm matches a shape production never produces

**`crates/aion-client/src/transport/embedded.rs`**, `map_engine_error`.

The arm matches `EngineError::Store(StoreError::NotOwner)`. But every fenced
append reaches the engine through the **Recorder**, which returns
`DurabilityError` (`durability/recorder.rs:209`; `durability/error.rs:27`
`Store(#[from] StoreError)`), wrapped as `EngineError::Durability`
(`aion/src/error.rs:119`). **The production shape is `Durability(Store(NotOwner))`.**

Reviewer CONFIRMED BY EXECUTION: adding that shape to my table gives
`left: "backend"`, `right: "not_owner"`. **So the exact defect the arm's own
comment says it prevents is still live** on `start_workflow`, `signal`, `cancel`,
`reopen`, `pause`, `resume`.

`aion-server` gets it right: `EngineError::Durability(d) => durability_wire(...)`
(`aion-server/src/error.rs:664`) → `DurabilityError::Store(s) => wire_from_store(s)`
(`error_engine.rs:43`) → `not_owner` (`error.rs:755`).

**My test passed because it hand-built a shape production does not produce.
Identity by label, not by content** — the estate law, and I broke it.

**Fix:** match the `Durability(Store(NotOwner))` shape too (and check whether the
bare `Store(NotOwner)` shape is reachable at all; if not, do not keep a dead arm).
**Add the production shape to the test table.**

---

## MAJOR 3 — the whole query error family still lands in `backend`

`embedded.rs` `query` call site. `EngineError::Query(QueryError::…)` is reachable.
Server classes (`aion-server/src/error_engine.rs:66-82`) vs embedded:

| variant | server | embedded |
|---|---|---|
| `UnknownQuery` | `unknown_query` | `backend` |
| `Timeout` | `query_timeout` | `backend` |
| `NotRunning` / `ReplyDropped` | `not_running` | `backend` |
| `HandlerFailed` | `query_failed` | `backend` |

**Four `ClientError` classes exist solely for this operation and are unreachable
through the embedded transport.** Identical caller code branches correctly over
the wire and cannot branch at all in-process — precisely the drift this change
claims to close. Reasoned, server side verified at source.

## MAJOR 4 — `ContractIdentity` and `NoQueueDeclaration`

Reachable from `start_workflow` (`lifecycle/start_admission.rs:29`, `:42`, `:82`).
Server → **`invalid_state`** (`aion-server/src/error.rs:693-694` →
`error_engine.rs:16-18`). Embedded → `backend`. Both are precondition refusals
the operator must act on. Reasoned, not executed.

## MODERATE 5 — I introduced a NEW divergence, invisible to my own test

`embedded.rs` maps `ShuttingDown | EngineTaskEpochClosed` → `unavailable`. The
**server maps both → `not_running`** (`aion-server/src/error.rs:648`, `:658`).
My test's doc comment claims "One rule, two surfaces — this test is what stops
the copies drifting", and the table lists **neither** variant. **The test locks
the subset that already agreed and is blind to the disagreement this same commit
created.** Either align the class or retract the claim in the doc comment — do
not leave both.

Separately: `EngineTaskEpochClosed` is constructed only at `completion.rs:124`
(internal process-exit path) and is **unreachable through all eight client call
sites**. By the function's own stated rule the arm should not exist.

## MODERATE 6 — my compaction justification is FALSE

`completion.rs`, justifying keeping `progress == Recorded`: *"it does not depend
on the reopen event still being readable, so it survives a compaction that the
history scan would not."*

**History compaction does not exist in this codebase.** It is an open design
question (`docs/design/workflow-engine/DESIGN-OVERVIEW.md:908`) and the only
other mention is `aion-proto/src/events.rs:212` — *"RESERVED compaction signal
(documentation-only, no code yet)"*.

Worse: under the current append-only store the disjunct **can never fire alone**
— `WorkflowReopened` is the only non-`WorkflowStarted` reset point in
`current_lease_terminal` (`aion-core/src/status.rs:130`), so `Recorded` +
`terminal_outcome_from_history == None` *necessarily* implies a reopen in
segment.

🔴 **This is the SAME CLASS the previous round caught in my F14 doc comment: a
"so" asserting a mechanism that is not there.** Second occurrence. The lesson is
not "check harder" — it is that a stated reason in a comment is a claim and must
be opened and read like any other.

## MINOR 7 — stated counts wrong

- `embedded.rs` doc: "`EngineError` has ~58 variants". **Actual: 52.** The whole
  paragraph's argument rests on that number.
- Marker BA says "six call sites" for `map_engine_error`; there are **eight**.
- `arm` has **five** return points, not four.

---

## What the review CONFIRMED as sound (do not redo)

- **Server parity for the six named classes** — verified, and verified
  pre-existing at HEAD. The classes I chose do match.
- **`ContentHash` re-export** — sound, no cycle, genuinely required.
- **Finding 4 (`ArmOutcome`) is CORRECT AND COMPLETE.** All five `arm` return
  points right; the runtime-is-`None` case genuinely means epoch closed (both
  writers call `gate_and_abort()` first); the post-insert undo path correct; log
  levels right; nothing newly silent; the await-child `warn` is the right
  response and **the parent is not stranded beyond the epoch** (the successor
  replays it from history and `await_child_step` re-arms under an open epoch).
  No dead code — `clippy --workspace --all-targets -- -D warnings` EXIT 0.
- **Mutations reproduce** — reviewer re-ran 3 of 7, all verbatim matches.
- Suites green: `aion-rs --lib lifecycle::completion runtime::engine_tasks`
  29/29; `aion-client --lib` 62/62.
- Tree left byte-identical by the reviewer (three shas verified).

**⚠️ The reviewer reviewed ONLY findings 3/4/5.** The tree also carries
`engine/api.rs` (735 lines), `engine/builder.rs` (548), a file rename, and four
new untracked source files — **unreviewed**.

---

## Gates (this tree, before the remedy)

    cargo fmt                                              (write, whole tree)
    cargo clippy --workspace --all-targets -- -D warnings  EXIT 0  (read BARE)
    cargo test -p aion-client                              62+1+6+2 / 0  EXIT 0
    cargo test -p aion-server --lib                        796 / 0       EXIT 0
    cargo test -p aion-rs --lib                            646 / 0
    cargo test -p aion-rs (full)      ⏳ WAS STILL RUNNING at compact — task
                                      bk8x2os6h, log at
                                      <scratchpad>/aion-rs.log. It ran
                                      CONCURRENTLY with the reviewer's builds,
                                      so **contended-red-is-not-red**: if it
                                      shows a failure, re-run quiet before
                                      attributing anything.

🔴 One expected red, **not mine**: `outbox_out_of_order_completions_still_settle_with_n_terminals`
= **#85**, structurally attributed (`LIBSQL-DOUBLE-CLOSE-85.md` root-causes it in
dep source; `SEAT-RESUME-2026-08-01-E.md` measured this exact target+error at
~37.5%/run before this work existed; `RECOVERY-LOOP-GUARDS-121.md` records a
sibling failing identically). Anything else red is mine.

---

## ORDER OF WORK FROM HERE

1. **Write the reopen-success probe first and watch it go RED** against the
   current tree. That is the regression proof and it must exist before the fix.
2. **Replace the guard with the lease-identity check.** Delete
   `run_segment_was_reopened`. Rewrite the comment against the real mechanism.
3. **Delete or repair the false compaction sentence** (moderate 6). If the
   `progress == Recorded` disjunct proves unreachable, **delete it** — do not
   keep an unreachable disjunct with a story attached.
4. **Fix blocker 2** — `Durability(Store(NotOwner))`, and put the production
   shape in the test table.
5. **Majors 3 and 4** — query family + `ContractIdentity`/`NoQueueDeclaration`.
6. **Moderate 5** — align `ShuttingDown`/`EngineTaskEpochClosed` with the server
   (`not_running`) or retract the doc claim; consider removing the unreachable
   `EngineTaskEpochClosed` arm.
7. **Minor 7** — correct 58→52, six→eight, four→five.
8. Re-run every gate. **Re-review on Opus** (this round earned it twice).
9. Commit naming files EXPLICITLY (list in marker BA), then push.

---

## Tom

He asked twice this session, and it is the right question:

> "we need to make sure that we're actually like fixing these things. You better
> not just be like documenting and accepting the invitations because I want them
> all properly fixed."

I answered him at 06:5x with the three fixes and the seven mutations. **That
answer is now partly wrong** — finding 5 is a regression, and finding 3's
`not_owner` arm does not fire on the reachable path. **He must be told**, plainly
and without hedging, once the remedy is in and proven. Do not let that sit.

Clearance standing: *"you can do whatever you need to do."*

---

## Standing constraints (carry verbatim)

- **NO WORKTREES** in aion. One mutating lane. Unmarked dirty = STOP.
- ⛔ **NEVER `git add -A`** — it swallows the NO-SWEEP untracked dirs and hangs.
  Name files explicitly. Commit message via `git commit -F -` with a **quoted**
  heredoc (zsh eats backticked words from `-m`); verify with `git log`.
- ⛔ Never bare `cargo clean`; no custom `CARGO_TARGET_DIR`; no launchd.
- ⛔ **NO-SWEEP** untracked holds: `.aion-accepted-binary-20260731/` (**SOLE copy
  of Tom's accepted build — nothing may touch it**),
  `.aion-jjuabk-snapshot-20260731/`, `.aion-data-preswap-20260804/`,
  `../aion-pre-scrub-backup-20260804.git`.
- ⛔ No `git gc`/`prune`/`--prune`; no `--force`/`--mirror`/`--delete` on push.
  **DELETE NOTHING** while the estate-wide hold stands.
- ⛔ `git stash push` never with `-u`; stash by explicit path.
- 🔴 **Tom's live pids: server 45425, worker 48399**, started from repo root.
  **DO NOT TOUCH.** Untouched all session.
- 🔴 **WORKER REBUILDS FROZEN** estate-wide until the #139 live cut.
- 🔴 beamr JIT blocker (Artemis): keep long workflows off until fixed beamr lands.
- 🔴 `aion server` must start FROM THE REPO ROOT (store is cwd-relative `./aion-data`).
- 🔴 **NEVER PUBLISH ARTIFACTS FOR TOM.** Deliverables are local files.
- 🔴 CLAUDE.md mandates an **Opus** sub-agent review before work lands.
- Never mention prospekt to Tom.
- Estate diff driver emits no ±: `git -c diff.external= diff --no-ext-diff`.
- zsh: no `PIPESTATUS` (lowercase `$pipestatus`, 1-indexed); `status` is
  read-only — name it `rc`. **Never cite an exit code off a pipeline tail.**
- BSD `sed` has no `\s` — use `/usr/bin/python3`. `~/.local/bin/python3` cannot exec.
- Foreground `sleep` is blocked. Use `until <check>; do sleep N; done` or background.
- Box is AEST (+1000): always `-t "%Y-%m-%dT%H:%M:%S%z"`.
- **Mutation discipline:** back up, mutate, observe RED, restore, prove with
  `shasum -a 1`, re-run green.
- Autonomous `/loop`, dynamic pacing — **every tick MUST end with
  `ScheduleWakeup` carrying the literal sentinel `<<autonomous-loop-dynamic>>`.**
  (Armed at ~17:44 for 18:09.)

## Also banked (unchanged, still good)

`docs/design/aion-authoring/SPEC-5-AWL-SUM-COMBINATOR.md` — full #5 spec, 309
lines, verified against the tree. `sum` as a seventh `CombinatorKind`, not a
general `reduce`; `gleam@int:sum/1`/`gleam@float:sum/1` already in the embedded
SDK closure; the one real cost is the network-gated tree-sitter mirror push.

Remaining tutorial-self-run issues after #7 lands: **#5** (spec banked), **#1**
(`workflow.now` history-tail read — a determinism-boundary defect, spec in AZ),
**#6** (`max_concurrency` on liminal — spec banked; the issue was half wrong and
surfaced a third unfiled defect: worker selection is `min_by_key(worker.id)`, so
there is **no load balancing within a pool at all** — needs routing, not quiet
building), **#9** (reply only, past Waffles).

**Owed to Tom:** dry run of the #146 tutorial + one-page crib sheet, from
`docs/authoring/first-workflow.html`, `docs/authoring/tutorial-examples/*.awl`,
`docs/GETTING-STARTED.md` and the proven self-run (`d78a38308`) — **without
touching his pids**.
