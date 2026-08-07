# SEAT RESUME — marker BA (2026-08-06)

**Supersedes AZ for STATE.** AZ remains the record of review-4's findings and of
the blocker fixes (findings 1 and 2). This marker records findings 3, 4, 5 and 6
being **fixed in code and proven decisive**, and carries the commit checklist.

Vesper Lynd, session `5849e0d8-4802-4869-8e0f-9f7fd187e198`.

---

## Tom's word this session

> "Okay, yeah, you can do whatever you need to, you've got clearance to do
> whatever you need to do." — 06:56:57Z

and, twice, the thing that matters:

> "we need to make sure that we're actually like fixing these things. You better
> not just be like documenting and accepting the invitations because I want them
> all properly fixed."
>
> "So are we actually coming back and fixing these things properly?"

Answered at 3 findings × real code + 7 mutations × 7 reds. **Every one of the
three is a code change, not a comment.** The answer sent to him is in the
transcript; the evidence is below.

---

## What landed this session (uncommitted, in the working tree)

### Finding 3 — the client error map misreported six reachable variants

`crates/aion-client/src/transport/embedded.rs`, `map_engine_error`.

The doc above that function states its own rule: name an arm when "the generic
bucket would mislead a caller about what to DO". Six variants failed that rule
and fell into `ClientError::server` — "not your fault, retry" — when they mean
the opposite.

| variant | was | is |
|---|---|---|
| `StartInputRefused`, `SignalRefused` | `server` | `invalid_argument` |
| `TerminalWriterUnavailable`, `TerminalWriterHeld`, `RunIsRecoverable`, `NoResidencyVerdict` | `server` | `invalid_state` |
| `Store(StoreError::NotOwner)` | `server` | `not_owner` |

None of these classes is invented here — `aion-server/src/error.rs` already
classifies the same variants the same way (`declared_contract_wire` :695-696,
`invalid_state_wire` :640-643, `WireError::not_owner` :755-756). This is the
embedded surface reaching the server's verdict rather than a second opinion, and
the new test is what stops the two copies drifting.

`not_owner` is the costly one: the client's own retry policy treats it as a
**routing** failure — retryable against a *different* owner. `server` erased that
distinction entirely.

**Rider fix.** `aion::ContentHash` is now re-exported from `crates/aion/src/lib.rs`.
`EngineError::StartInputRefused`/`SignalRefused` carry a `ContentHash` in their
**public** signature, so a consumer of `aion` could not construct, destructure or
exhaustively match those variants without taking its own `aion-package`
dependency. Found because the test needed it; it is a real public-API gap.

**Test:** `every_named_engine_error_reaches_its_own_class` — a table with a
**negative control** (`EngineError::Runtime` must still reach `backend`), so a
map that had accidentally started naming *everything* would fail too.

**Mutations — 3 run, 3 red:**

| removed arm | observed |
|---|---|
| `not_owner` | RED — left `"backend"`, right `"not_owner"` |
| `invalid_argument` group | RED — left `"backend"`, right `"invalid_input"` |
| `invalid_state` group | RED — left `"backend"`, right `"invalid_state"` |

Restored, `shasum -a 1` identical to the pre-mutation backup, green.

### Finding 4 — a retry log said "no retry was armed" when one was

`crates/aion/src/runtime/engine_tasks.rs` + `crates/aion/src/lifecycle/completion_retry.rs`
+ `crates/aion/src/runtime/nif_child.rs` + the two child-task wrappers.

`EngineTaskRuntime::arm` returned a bare `bool`. It refuses for **two reasons
that mean opposite things about whether the work has an owner**:

- a live task for this key is already running — the work IS owned;
- the engine-task epoch is closing — nothing owns it and nothing in this process
  will.

One flag cannot carry that, so `arm_completion_retry`'s call site logged the
epoch sentence for both: *"workflow completion could not be recorded and no
retry was armed; the run stays Running until a monitor is re-installed"* — for a
run whose retry was **in flight**. On the `Recorded` branch it told the operator
a `Completed` run "stays Running", which is a false durable fact about the one
thing this machinery exists to get right.

**Now:** `pub(crate) enum ArmOutcome { Armed, AlreadyArmed, EpochClosed }`,
threaded through `arm_watch` / `arm_spawn_retry` / `arm_completion_retry` and
both child-task wrappers. Three call sites now say what actually happened:

- `completion_retry.rs` — `AlreadyArmed` gets its own `debug!` naming the
  in-flight retry as the terminal's owner; `EpochClosed` keeps the two `warn!`
  lines, which are now true whenever they fire.
- `nif_child.rs` spawn recovery — `AlreadyArmed` stays `debug!`, `EpochClosed`
  is promoted to `warn!` (the recorded child stays unstarted until the next
  epoch's sweep).
- `nif_child.rs` await-child — the epoch-closed refusal was being **discarded
  entirely**. It now warns: the parent parks unwatched until startup recovery
  re-installs the watcher.

**Tests sharpened, not merely made to compile.** Every `assert!(arm(...))`
became `assert_eq!(..., ArmOutcome::Armed)`, every `assert!(!arm(...))` became
an assertion on **which** refusal. `arming_is_idempotent_per_key` now demands
`AlreadyArmed`; `shutdown_gates_new_arms_and_awaits_aborted_tasks` demands
`EpochClosed`. Under the `bool` both passed with the two swapped.

**Mutations — 2 run, both red, in both directions:**

| mutation | observed |
|---|---|
| occupied-entry arm returns `EpochClosed` | RED ×2 — `arming_is_idempotent_per_key`, `abort_watch_disarms_a_single_key` |
| shutdown gate returns `AlreadyArmed` | RED ×1 — `shutdown_gates_new_arms_and_awaits_aborted_tasks` |

Restored, sha-verified `9270d998…`, 7/7 green.

**Stated limit, not hidden:** the *log text* itself is not asserted (no tracing
capture in this suite). What is asserted is the type-level distinction the log
reads from, which is the load-bearing part — the site physically could not have
told them apart before.

### Finding 5 — F11's reopen guard covered one branch of two

`crates/aion/src/lifecycle/completion.rs`.

The guard asked `progress == Recorded` — *"did **I** record the terminal"*. That
is this monitor's private recollection. A run can be driven terminal by a path
this monitor never took (an explicit cancel, a `WorkflowDeadlineHandler` timeout
teardown, a predecessor engine's writer) and then reopened, with this monitor
still holding `NotRecorded`. On that branch the append went through — **into a
live reopened run** — which is the exact outcome F11 exists to prevent, on the
branch F11 did not cover. It fails safe and then hands the operator
`SequenceConflict`, which this codebase defines as THE double-writer indicator,
for a benign reopen.

**Now:** `run_segment_was_reopened(&history, run_id)` — a new `fn` that asks
**history**, scoped through `run_segment` exactly as `terminal_outcome_from_history`
is. `WorkflowReopened` in this run's segment is true regardless of who wrote the
terminal it superseded.

The `progress` check is **kept as the second disjunct rather than deleted**: it
is decisive on its own (this monitor's own append returned `Ok`) and does not
depend on the reopen event still being readable, so it survives a compaction the
history scan would not.

**Test:** `a_reopened_run_is_not_appended_into_by_a_monitor_that_never_recorded`.
Terminal and reopen both written through the handle's recorder **before** the
monitor is invoked, so as far as `handle_process_exit_async` is concerned no
attempt of its own ever recorded anything. Observable is the count of
`WorkflowCompleted` events in the run segment: 1 before, 1 after.

**Two fixture controls, both asserted** — the terminal count is exactly 1 going
in, and `run_segment_was_reopened` is verified true against real durable history
before the call. Without the second, the test could reach the guard for the
wrong reason and measure nothing.

**Mutation — 1 run, red:** dropping the `|| run_segment_was_reopened(…)`
disjunct (i.e. restoring the pre-fix guard) turns **exactly one** test red — the
new one — with **21 siblings green**. That is the ideal signature. Restored,
sha-verified `a77254dc…`, 22/22 green.

### Finding 6 (note) — the post-record bookkeeping runs outside the gate

`completion.rs`, above the trailing `upsert_workflow_visibility`. A named comment
rather than a change, and it says why: neither call appends to history so neither
can be the second writer the gate stops; both are idempotent derived-index writes
a successor's recovery redoes. What they are NOT is protected, and a reader of
the gate should not assume otherwise.

---

## Also banked this session

`docs/design/aion-authoring/SPEC-5-AWL-SUM-COMBINATOR.md` — the full #5 spec
(309 lines), verified against the working tree by an agent. Headlines:

- **`sum` as a seventh `CombinatorKind`**, zero-argument, pipe-stage only.
  `[Int]→Int`, `[Float]→Float`, anything else is a check error. Empty list → the
  additive identity, which is the stdlib's own answer, not an invented default.
- **Not a general `reduce`** — it would need a two-argument lambda, arithmetic
  operators, and an accumulator binding form, none of which AWL has. The
  asymmetry is an order of magnitude: `sum` is 2 `RuntimeFn` rows + 1 `CallRt`;
  `reduce` needs a new MIR `Stmt::Add`, a burst, a `verify` arm, and capturing
  closures where every combinator closure today is capture-free.
- `gleam@int:sum/1` and `gleam@float:sum/1` are **already exported** by the
  embedded SDK closure (verified by reading the shipped `.beam` export chunks).
- **The one real cost is the tree-sitter mirror push** (§3.11) — network-gated,
  and `the_zed_pin_names_a_mirror_commit_carrying_this_grammar` is red until it
  happens. Budget for it.
- The `AWL-2-SPEC.md` overclaim was **already fixed** on main (`a32a28846`).

---

## Gates — measured at my own hand, this tree

    cargo fmt                                              (write, whole tree)
    cargo clippy --workspace --all-targets -- -D warnings  EXIT 0   (read BARE)
    cargo test -p aion-client                              62+1+6+2 / 0   EXIT 0
    cargo test -p aion-server --lib                        796 / 0        EXIT 0
    cargo test -p aion-rs                                  ⏳ IN FLIGHT

🔴 If `-p aion-rs` comes back with **one** red and it is
`outbox_out_of_order_completions_still_settle_with_n_terminals`, that is **#85,
not this work** — structural attribution, not statistical: mechanism root-caused
in dep source (`LIBSQL-DOUBLE-CLOSE-85.md`), a prior seat measured this exact
target+error at ~37.5%/battery run **before this work existed**
(`SEAT-RESUME-2026-08-01-E.md`), and a sibling test is recorded failing
identically (`RECOVERY-LOOP-GUARDS-121.md`). Anything else red is mine.

---

## COMMIT CHECKLIST — name files EXPLICITLY

⛔ **NEVER `git add -A` in this tree.** It swallows the NO-SWEEP untracked dirs
and hangs.

Untracked, load-bearing, must be named:

    crates/aion/src/store_faults.rs
    crates/aion/src/lifecycle/completion_retry.rs
    crates/aion/src/engine/builder_assembly.rs
    crates/aion/src/engine/api_workflow_ops.rs
    docs/design/aion-authoring/BRIEF-7-COMPLETION-MONITOR-TRANSIENT-STORE.md
    docs/design/aion-authoring/FINDING-BC4-DIFFERENTIAL-NONDETERMINISM-2026-08-06.md
    docs/design/aion-authoring/SPEC-5-AWL-SUM-COMBINATOR.md
    docs/design/aion-authoring/SEAT-RESUME-2026-08-06-AZ.md
    docs/design/aion-authoring/SEAT-RESUME-2026-08-06-BA.md

Modified (derive the list with `git status --short`, do not recall it).

Commit message via `git commit -F -` with a **quoted** heredoc — zsh eats
backticked words from `-m`. Verify with `git log`.

---

## Standing constraints (carry verbatim)

- **NO WORKTREES** in aion. One mutating lane. Unmarked dirty = STOP.
- ⛔ Never bare `cargo clean`; no custom `CARGO_TARGET_DIR`; no launchd.
- ⛔ **NO-SWEEP** untracked holds: `.aion-accepted-binary-20260731/` (**SOLE copy
  of Tom's accepted build — nothing may touch it**), `.aion-jjuabk-snapshot-20260731/`,
  `.aion-data-preswap-20260804/`, and `../aion-pre-scrub-backup-20260804.git`.
- ⛔ No `git gc`/`prune`/`--prune`; no `--force`/`--mirror`/`--delete` on push.
  **DELETE NOTHING** while the estate-wide hold stands.
- ⛔ `git stash push` never with `-u`; stash by explicit path.
- 🔴 **Tom's live pids: server 45425, worker 48399**, started from repo root.
  **DO NOT TOUCH.** Untouched all session.
- 🔴 **WORKER REBUILDS FROZEN** estate-wide until the #139 live cut.
- 🔴 beamr JIT blocker (Artemis): keep long workflows off until fixed beamr lands.
- 🔴 `aion server` must start FROM THE REPO ROOT — the store is cwd-relative
  `./aion-data`.
- 🔴 **NEVER PUBLISH ARTIFACTS FOR TOM.** Deliverables are local files.
- 🔴 CLAUDE.md mandates an **Opus** sub-agent review before work lands.
- Never mention prospekt to Tom.
- Estate diff driver emits no ±: use `git -c diff.external= diff --no-ext-diff`.
- zsh: no `PIPESTATUS` (lowercase `$pipestatus`, 1-indexed); `status` is
  read-only — name it `rc`. Never cite an exit code off a pipeline tail.
- BSD `sed` has no `\s` — use `/usr/bin/python3`. `~/.local/bin/python3` cannot exec.
- Foreground `sleep` is blocked. Use `until <check>; do sleep N; done` or background.
- Box is AEST (+1000): always `-t "%Y-%m-%dT%H:%M:%S%z"`.
- Autonomous `/loop`, dynamic pacing — **every tick MUST end with
  `ScheduleWakeup` carrying the literal sentinel `<<autonomous-loop-dynamic>>`.**

---

## Still to do

1. **Read the `-p aion-rs` result** and settle it against #85 (above).
2. **Opus sub-agent review** of findings 3/4/5 before landing (CLAUDE.md mandate).
3. **Commit naming files explicitly**, then push.
4. Then the remaining tutorial-self-run issues: **#5** (spec banked, ready to
   build — mirror push is the gate), **#1** (`workflow.now` history-tail read —
   spec banked in AZ, and it is a determinism-boundary defect, not a line),
   **#6** (`max_concurrency` on liminal — spec banked; the issue was half wrong
   and it surfaced a third unfiled defect: worker selection is
   `min_by_key(worker.id)`, so there is **no load balancing within a pool at
   all**. Both need routing, not quiet building), **#9** (operator fact in
   `details` — reply only, past Waffles).
5. **Owed to Tom:** dry run of the #146 tutorial + a one-page crib sheet, derived
   from `docs/authoring/first-workflow.html`, `docs/authoring/tutorial-examples/*.awl`,
   `docs/GETTING-STARTED.md` and the proven self-run (`d78a38308`) — **without
   touching his pids**.
