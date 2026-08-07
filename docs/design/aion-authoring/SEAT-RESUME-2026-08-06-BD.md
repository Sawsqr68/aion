# SEAT RESUME — marker BD (2026-08-06)

**Supersedes BC for STATE.** Vesper Lynd, session
`5849e0d8-4802-4869-8e0f-9f7fd187e198`.

# 🔴 REVIEW 6 = NOT SAFE TO LAND. WORK IS **MID-EDIT** — THE TREE DOES NOT COMPILE RIGHT NOW.

Do not commit. Do not run gates until the edits below are finished.

---

## The review's verdict, and what I verified MYSELF before acting

Review 6 (Opus) returned **2 blockers, 3 majors, 3 moderates**. The full text is
in the task transcript, but everything load-bearing is reproduced here.

### 🔴 B1 — orphaned doc comment. **FIXED, DONE.**
Deleting `run_segment_was_reopened` left its doc comment behind, where it became
a false description of `terminal_outcome_from_history` plus a dangling `///`.
Verified at my own hand at `completion.rs:533`. Replaced with an accurate
doc naming the run-scoped/reset-aware semantics and distinguishing it from the
whole-history function of the same name in `engine/api.rs`.

### 🔴 B2 — MY COMMENTS AND FIXTURES ARE STILL WRONG ABOUT THE MECHANISM. **MID-FIX.**

**The reviewer is right, and I verified the mechanism at source:**
`crates/aion/src/durability/recorder.rs:39-42` — *"A recorder owns the workflow's
tracked sequence head… **It never reads the store head after construction**; a
sequence conflict is surfaced as a hard durability error because it indicates a
second writer."* And `reopen.rs:127-131` builds a **separate**
`Recorder::resume_at(id, store, history_head)`.

**Therefore:** a stale monitor's append is **NOT silently duplicated — it is
REFUSED** with `SequenceConflict`. My comments said it "would append a SECOND
one". That is false, and it is the THIRD round in which a stated reason in this
change asserted a mechanism that is not there.

**And my fixtures wrote BOTH the terminal and the reopen through
`active.handle.recorder()`** — one recorder, so the monitor is never stale.
That is BB's blocker 2 (a test passing on a shape production never makes)
**re-committed in the file written to remedy it**. The reviewer rebuilt the
fixture in production shape and mutation B2 produced
`Error: Durability(Store(SequenceConflict { expected: 2, found: 3 }))`, not
`left: 2 / right: 1`.

**The guard is still worth keeping** — it converts a spurious `SequenceConflict`
(THE double-writer indicator here; `classify_completion_failure` treats it as
unretryable and it surfaces as `error!` at `startup.rs:447` / `start.rs:573`)
into a clean logged stand-down. That is real value. It is just not the value I
wrote down.

### ⚠️ MID-EDIT STATE — WHAT IS DONE AND WHAT IS NOT

`crates/aion/src/lifecycle/completion.rs`, test module. **Already applied:**
1. B1 doc comment fixed.
2. `a_reopened_run_is_not_appended_into_by_a_monitor_that_never_recorded` —
   both the terminal and the reopen now go through
   `append_through_a_separate_recorder`; assertions rewritten to check BOTH
   `outcome.is_ok()` (no spurious double-writer report) AND
   `completed_count == 1`.
3. `a_recorded_monitor_stands_down_inside_the_reopen_window` — terminal still
   through the monitor's own recorder (correct: it IS the `Recorded` case), the
   reopen moved to `append_through_a_separate_recorder`.

**🔴 NOT YET WRITTEN — the tree will not compile without it. DO THIS FIRST:**

Add to the test module (next to `respawned_lease`):

```rust
    /// Append through a recorder OTHER than the monitor's, built at the current
    /// store head — the way every other writer in this engine does.
    ///
    /// 🔴 This is the whole fidelity of these fixtures. A `Recorder` tracks its
    /// own sequence head and never re-reads the store
    /// (`durability/recorder.rs`), so writing a fixture event through the
    /// monitor's own recorder leaves that recorder perfectly in step — which
    /// production never does. `reopen::reopen` builds its own recorder at the
    /// history head, so the monitor's is left stale by one and its next append
    /// is REFUSED with `SequenceConflict`. A fixture that shares one recorder
    /// cannot see that, and silently tests a machine that does not exist.
    async fn append_through_a_separate_recorder<F>(
        active: &ActiveWorkflow,
        write: F,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        F: FnOnce(
            &mut Recorder,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<(), Box<dyn std::error::Error>>> + '_>,
        >,
    {
        let history = active
            .context
            .store
            .read_history(active.handle.workflow_id())
            .await?;
        let head = history.last().map(Event::seq).unwrap_or_default();
        let mut recorder = Recorder::resume_at(
            active.handle.workflow_id().clone(),
            Arc::clone(&active.context.store),
            head,
        );
        write(&mut recorder).await
    }
```

⚠️ **The closure signature is UNVERIFIED — it has never compiled.** If the
lifetime dance fights back, the simpler shape is two concrete helpers
(`terminal_by_another_writer(&active, payload)` and
`reopen_by_its_own_recorder(&active, &run_id)`) with no closure at all. **Prefer
that** — it is less clever and the call sites already read as two named acts.
`Recorder::resume_at` and `Event::seq` are both real (used at `reopen.rs:126-127`).
Check `Event` and `Recorder` are in scope in the test module (`Recorder` is
imported via `use crate::durability::Recorder;`).

**Then rewrite the guard's comment at `completion.rs:~284-300`.** It currently
claims a retry "would append a SECOND one into a run the operator is in the
middle of reviving". Replace with the truth: the append is refused on sequence;
the cost of not standing down is a **false incident report**, not corruption;
and note that the file already says this correctly fifty lines earlier
(lines ~236-241), so the two passages must agree.

### MAJOR M1 — two named arms are untested and silently deletable. **NOT DONE.**
Reviewer deleted **both** `EngineError::WorkflowNotFound => not_found`
(`embedded.rs:480`) and `EngineError::InvalidState => invalid_state` (`:483`)
and got **62 passed / 0 failed**. Neither is in either table half.
**Fix: add both rows to `admission_and_run_state_cases`.**

### MAJOR M2 — "No client operation can produce that" is FALSE. **NOT DONE.**
The bare `Store(StoreError::NotOwner)` shape IS reachable:
`engine/api_workflow_ops.rs:491-500` (`list_workflows` → `store.query` /
`read_history`, both `-> StoreError`, and `EngineError::Store` is
`#[from] StoreError` at `error.rs:115`) and `start.rs:158`
(`admission::workflow_identity`). Two of the eight call sites.
**Fix: correct the doc claim in `embedded.rs:~913-915`. Keep the arm and the
row — they are right; only the reason is wrong.**

### MAJOR M3 — two call sites bypass the classifier. **NOT DONE.**
`embedded.rs:240` (`describe_workflow`) and `:281` (`subscribe`) do
`.map_err(|error| ClientError::server(error.to_string()))` on a `StoreError`
from `read_history`, erasing `NotOwner` — the one class the client's retry
policy treats as a routing signal. Pre-existing lines, same file, same defect
the remedy exists to close. **Fix: route both through `store_error_class`.**
Parity currently holds for 8 of 10 operations, not 10.

### MODERATE Mo1 — the reopen window is half-covered. **RULED: NAME IT, DO NOT CLOSE IT.**
The `NotRecorded` branch inside the reopen window is uncovered and reachable
(cancel → reopen is AD-013; the killed process's exit callback lands in the
window). Cost: a `SequenceConflict` + one `error!` line for a benign reopen.
Fail-safe.

**I verified the fact that would make a third disjunct safe, and then ruled
against it anyway.** `reconcile_terminal_registry` (`completion.rs:452-463`)
runs AFTER the terminal append and calls `registry.reconcile` +
`replace_residency(Suspended)` — it does **not** remove the entry (this is
exactly what `reopen.rs:109-111` relies on). So a genuine lease's handle IS
still registered during its own exit callback, and
`registry-absent + reopen-in-segment` could not be a live lease.

**Ruled against because:** any path that removes a live run's entry (unload of a
paused/parked run) would make that disjunct fire on a legitimate terminal —
which is blocker 1's failure mode, a run stranded `Running` forever. Trading a
logged, fail-safe false alarm for a possible silent permanent zombie is the
wrong direction. **Write the residual into the comment** — which branch is
uncovered, what it costs, and why a history predicate cannot close it (that is
what caused the regression in the first place). The file's own standard, twenty
lines earlier: *"it is a real window and it is named here rather than denied."*

### MODERATE Mo2 — BC says "19 rows, verified by count". It is **20**. **NOT DONE.**
(10 + 10.) Fourth stated count across BB→BC that does not verify. My counting
regex missed one. **Fix in the new marker, and stop stating counts I have not
counted twice.**

### MODERATE Mo3 — UNREVIEWED LAYER, NEEDS A DECISION, NOT A BUILD.
`crates/aion/src/runtime/config.rs`, `impl Default for CompletionRetryConfig`:
the shipped ladder is 1 ms / 8 ms, **unbounded**, and **unreachable from
`aion server`** — grep of `aion-server/src` and `aion-cli/src` for
`completion_retry` / `CompletionRetryConfig` / `signal_delivery` /
`SignalDeliveryConfig` returns **nothing**. That is ~125 history reads per second
per stuck run, forever, against a store that is by definition unwell, with no
operator knob. CLAUDE.md: *"NO ARBITRARY LIMITS / NO ASSUMED DEFAULTS… Discuss
values before implementing."* **Route this to Waffles as a ruling, do not pick a
number.**

---

## What the review CONFIRMED SOUND — do not redo

- **The lease-identity discriminator is sound.** beamr allocates pids from a
  monotonic `AtomicU64::fetch_add(1, Relaxed)` seeded at 1
  (`beamr-0.16.2/src/scheduler/spawning.rs:436`, `scheduler/mod.rs:1519`), no
  free list, no recycling; the counter resets per `Scheduler` but registry and
  monitor closures die with the engine, so no cross-process comparison happens.
- Reopen genuinely registers a NEW handle with a NEW pid under the SAME key
  (`recover_active_workflow` → fresh pid; `register_recovered_resident`
  `startup.rs:317/331` → `Registry::insert`, replace semantics).
- **The `None`/`Err` fall-through is exactly HEAD behaviour** (`git show
  HEAD:.../completion.rs` — the `else` branch had no guard at all). It cannot
  regress anything.
- `run_segment_was_reopened` is gone (grep-clean).
- **The whole wire composition, every arm, checked by hand — NO
  mis-classification found.** Store family, all six query variants, all six
  `invalid_state` variants, `ShuttingDown`→NotRunning, `Durability(non-Store)`
  →backend. The classes I chose are right.
- **The `EngineTaskEpochClosed` deletion is SAFE** — one construction site
  (`refuse_if_epoch_closed`, `completion.rs:124`), two callers (`:198`, `:478`),
  both internal; unreachable from all eight transport operations.
- `Durability(Store(..))` and bare `Store(..)` are the ONLY nested shapes a store
  refusal arrives in.
- `ScheduleNotFound` / `UnknownVersion` / `ManifestMismatch` /
  `UnenforceableContract` are genuinely NOT reachable through the eight.
- **Counts: `EngineError` has exactly 52 variants, not `#[non_exhaustive]`;
  eight call sites.** All three minor-7 corrections are right.
- Both re-exports genuinely required, no cycle.
- The `Engine::shutdown` ordering claim is TRUE (`api.rs:384/405/418`;
  `shutdown_gate.rs:52-59` condvar, no timeout).
- History compaction genuinely does not exist — the moderate-6 retraction is right.
- **Four of BC's eight mutations re-run independently, all verbatim matches**
  (A, B2, embedded-1, embedded-4).
- Hygiene clean across all changed/new files: no god files (largest non-test
  code count `builder.rs` 443, `embedded.rs` 419), no `#[allow]`/`#[expect]`/
  `#[ignore]`/`todo!`, no `unwrap`/`expect`/`panic` outside tests, no swallowed
  `Result`s.
- Reviewer's gates: `aion-rs --lib lifecycle::completion` 24/0 ·
  `lifecycle::` 97/0 · `aion-client --lib` 62/0 · **`aion-server --lib` 796/0** ·
  `clippy -p aion-rs -p aion-client --all-targets -D warnings` exit 0.
  ⚠️ `-p` resolution, not `--workspace` — **both are required, they differ on
  `preserve_order`**.

---

## Battery result (started 08:06:55Z, ran ~1h20m)

**One failure: `outbox_happy_path_completes_with_n_terminals_in_order`** in
`tests/outbox_e2e.rs`, with
`"SQLite failure: \`bad parameter or other API misuse\`"` — the **#85** libsql
0.9.30 double-close signature. BB records this exact class, and names a sibling
(`outbox_out_of_order_completions_still_settle_with_n_terminals`) failing
identically. **It also ran CONTENDED** with the reviewer's builds.
🔴 **Re-run `cargo test -p aion-rs --test outbox_e2e` on a quiet box before
attributing it either way.** Everything else in the log is `ok`.

---

## ORDER OF WORK FROM HERE

1. **Make it compile** — add the separate-recorder helper (prefer the two
   concrete non-closure helpers).
2. Rewrite the guard comment to the true mechanism (refusal, not duplication)
   and add Mo1's residual, named.
3. M1: two table rows. M2: correct the false doc claim. M3: two call sites
   through `store_error_class`.
4. Re-run ALL mutations — the fixtures changed, so every previously recorded
   result is void. Expect the reopen ones to go red as
   `SequenceConflict`/`is_ok()` now, not as a doubled count.
5. `cargo fmt --all`; `cargo clippy --workspace --all-targets -- -D warnings`
   (read the exit code BARE) **and** the `-p` variant; `aion-client --lib`;
   `aion-server --lib`; `aion-rs --lib`; full `aion-rs` quiet.
6. **Review 7 on Opus.** Give it BB (original findings), BC (first remedy), this
   marker, and the review-6 findings. Tell it the tree still carries the
   unreviewed layer: `engine/api.rs`, `engine/builder.rs`, the
   `nif_child_tasks.rs → engine_tasks.rs` rename, and
   `api_workflow_ops.rs`, `builder_assembly.rs`, `completion_retry.rs`,
   `store_faults.rs` — **none of which has had a line-by-line review by anyone.**
7. Commit naming files EXPLICITLY. Push.
8. Route Mo3 to Waffles as a ruling.
9. Then #5 (spec banked), #1, #6, #9.
10. **Owed to Tom:** #146 tutorial dry run + one-page crib sheet, without
    touching his pids.

---

## Tom

Told at ~07:5xZ what the plan was and that the tutorial is ready NOW regardless.
He replied approving the work and asking that things get merged as they go.
**He has not been told review 6 failed.** He asked twice this session for real
fixes rather than documentation, and he was right both times — the review found
the SAME defect class recommitted inside the fix for it. Tell him plainly, and
do not dress it up: the mapping work is genuinely correct, the comments and the
test fixtures were not.

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
- 🔴 Do not commit while a battery is in flight (the #123 build-identity pin).
- Never mention prospekt to Tom.
- Estate diff driver emits no ±: `git -c diff.external= diff --no-ext-diff`.
- zsh: no `PIPESTATUS` (lowercase `$pipestatus`, 1-indexed); `status` is
  read-only — name it `rc`. **Never cite an exit code off a pipeline tail.**
- BSD `sed` has no `\s` — use `/usr/bin/python3`. `~/.local/bin/python3` cannot exec.
- Foreground `sleep` is blocked. Use `until <check>; do sleep N; done` or
  `nohup … & disown` (plain background bash dies in waves).
- Box is AEST (+1000): always `-t "%Y-%m-%dT%H:%M:%S%z"`.
- **Mutation discipline:** back up, mutate, observe RED, restore, prove with
  `shasum -a 1`, re-run green.
- Autonomous `/loop`, dynamic pacing — **every tick MUST end with
  `ScheduleWakeup` carrying the literal sentinel `<<autonomous-loop-dynamic>>`.**
  (Armed at ~18:22 for 18:47.)

## The lesson worth keeping

Three rounds, three comments whose "because" asserted a mechanism that is not in
this codebase — and this time the false reason was inside the fix written to
stop exactly that. **A stated reason is a claim and must be opened and read like
any other line of code.** Add: a fixture that shares one writer where production
uses two is not a fixture, it is a different machine.
