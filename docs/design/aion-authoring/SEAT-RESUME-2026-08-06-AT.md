# SEAT RESUME — marker AT, 2026-08-06

Supersedes AS. AR's AMENDMENT 3 remains the authority on the #7 lane's internals.
Latest letter wins.

## 🔴 STATE

- HEAD `9189ba585` == origin/main. #7 built, gated green, **UNCOMMITTED**.
- **SECOND OPUS REVIEW RETURNED: 1 CRITICAL + 5 MAJOR + 9 MINOR.**
  **#7 DOES NOT COMMIT UNTIL THESE ARE HANDLED.** Full text:
  `<scratchpad>/tasks/a60c43462fd771923.output` (session task file).
- Tom's live pids 45425 / 48399 untouched.

---

# PART 1 — THE SWEEP IS FINISHED. NOTHING WAS LOST.

## The decisive measurement

The 73 NOT-IN-MAIN-BY-CONTENT unreachables were audited against the pre-scrub
backup `/Users/tom/Developer/ablative/stack/aion-pre-scrub-backup-20260804.git`
(2513 commits on main, 2534 reachable from all refs).

**Ruler validated before use** (4 known-answer controls, all as expected):

| control | expectation | result |
|---|---|---|
| backup main tip is-ancestor-of backup main | YES | YES |
| backup `main~500` is-ancestor | YES | YES |
| post-scrub HEAD `9189ba585` present in backup | ABSENT | ABSENT |
| fabricated sha resolves | ABSENT | ABSENT |

**Result: 0 of 73 reachable from ANY ref in the backup. 71 of 73 physically
PRESENT in it as orphans.**

That pairing is what makes the verdict decisive, and it is the whole finding:

- **Presence proves the test is not vacuous.** The backup carries unreachable
  objects (it was a directory-level copy, not a `clone`, which transfers only
  reachable objects). So "absent" would have been a real signal.
- **Unreachability in the backup proves the orphaning predates the scrub.**
  Nothing pointed at these before 2026-08-04 either.

So the scrub did not orphan them. They are the ordinary sediment of rebases,
amends and dropped stashes across the June–August build, carried through
faithfully. Corroborating signature: duplicated subjects, which is exactly what
an amend cycle leaves behind — `chore(aion-server): rename dashboard → ops
console (#156)` ×3, `feat(server): add schedule management endpoints` ×3,
`AD-012 brief: reopen operation` ×2, `feat(aion): split activity NIF` ×2,
`feat(visibility): reconcile crash-consistency gaps` ×2, `fix: accept raw JSON
string in hello-world entry function` ×2.

Breakdown of the 73: **18 stash plumbing** (`WIP on` / `index on` /
`untracked files on` / `On <x>:` / `stash:` — dropped stashes) and
**55 standalone orphans** (amend/rebase intermediates).

**The 2 ABSENT-FROM-BACKUP are dated 2026-08-06 — AFTER the backup was taken on
2026-08-04.** Both are this seat's own `wip-7-completion-retry` stash pair. Their
content is in the working tree right now. Not a gap.

## Full sweep verdict (all four findings, one line each)

1. Dirty tree — **GENUINE**, 13/13 blob-differ from main. It is the #7 work.
2. Seven local June fossils — different branches wearing their origin twins'
   names. 3 husks (0 commits vs main), 5 LIVE origin branches to land.
   All seven names recoverable from the backup.
3. `.worktrees/delivery-fix` — **pure frame-trap**, 7/7 "dirty" files
   byte-identical to main. Disposable.
4. 104 unreachables — **all pinned** `refs/rescue/20260806-<sha>`,
   PINNED=104 FAILED=0. 19 REBASED-LANDED, 12 EMPTY-OR-MERGE,
   **73 audited above = pre-existing sediment, no loss**.

**Answer to Tom's directive: aion lost nothing.** The work at risk was never in
the unreachables — it is the 5 live origin branches, which are intact on origin
and simply unlanded.

## Residual spot-checks (NOT blocking, named for honesty)

Three standalone orphans post-date the June layer and were classified only by
structure, not by reading their content against main:
`c99dc80d` (2026-07-27, `.v4 worker-contract identity`), `d7fc8360` (2026-07-23,
`instruction-level codegen inspection IR-14`), `18cd8e30` (2026-07-06,
`incident-triage prospekt->aion bridge demo`). All three are orphaned-pre-scrub
like the rest, so the verdict stands structurally; a content read would upgrade
it from structural to executed. Pinned under `refs/rescue/` either way.

## Sequencing still binding

- **NO `git gc`, NO `git prune`, NO `--prune` fetch** until estate all-clear.
- Delete nothing (fossils, husks, rescue refs) until Waffles has the report.
- Never `git add -A`; never `git stash -u`. NO-SWEEP dirs stand.

---

# PART 2 — THE SECOND REVIEW. #7 IS NOT READY.

## 🔴 C-1 (CRITICAL) — VERIFIED AT MY OWN HAND

`runtime/handle.rs`: the doc at 527 says *"Currently infallible"* — **false**.
Three `?` sit above the epoch close: `process_exits.begin_shutdown()?` (539),
`cleanup_executor.shutdown()?` (547), `process_exits.close_and_join_all()?`
(550). The epoch close `self.engine_tasks.shutdown()` is at 564, and
`scheduler.shutdown()` / `spawn_heaps.clear()` after it.

Any of those three erroring skips the epoch close entirely. Verified by grep:
**there is no `impl Drop for Engine` and none for `RuntimeHandle`** — the only
backstop is `EngineTaskRuntime::drop` firing on Arc refcount zero.

Failure: degraded store → callback-dispatcher shutdown times out →
`Engine::shutdown()` returns `Err` → retries still armed and appending terminals
→ operator restarts → successor recovers the same histories → **two writers on
one workflow (invariant 3)**. The change is what put terminal-appending tasks
behind those `?`.

**Fix (unambiguous, build it): close the epoch unconditionally** — capture the
first error, always run the epoch close and scheduler shutdown, return the error
after. And delete the false "infallible" doc.

## 🔴 M-1 (MAJOR) — NEEDS A RULING, ROUTED TO WAFFLES

`lifecycle/completion.rs:319-329`: an attempt upgrades the `Weak` to a strong
`Arc<RuntimeHandle>` and holds it across the **entire** attempt — read history,
recorder lock, terminal append, visibility upsert, reconcile. So the `Weak` added
for C2b breaks the *static* cycle but re-creates it *dynamically, per attempt*,
and pins `EngineTaskRuntime::drop` shut for exactly the span of the append it
exists to stop. The doc at 154-166 states the opposite premise.

With C-1 fixed, `shutdown()` always aborts+awaits, so the *shutdown* path is
covered. The residual is **drop-without-shutdown**: an attempt completes a
terminal append after the Engine is released.

This is a design fork with real trade-offs (re-check at the append boundary vs.
narrow the strong span vs. accept and document), so it goes to Waffles rather
than my picking one. Recommendation carried: re-check liveness at the append
boundary, since that is the only point where the hazard is actually realised.

## MAJORs I am building without a ruling (unambiguous)

- **M-2 `NotOwner` retried forever without re-resolving.** The store's own doc
  says *"re-resolve the shard's owner and retry or forward"*; the loop does
  neither, spinning ~125/sec against a shard this node has permanently lost.
  → reclassify non-transient, abandon loudly.
- **M-3 dedup key is a rendered error string + no escalation.** `Backend` embeds
  the inner Display (peer address, elapsed, leader hint, errno), so on a real
  backend the key changes every iteration and #94's flood returns; and when it
  doesn't change, one warn at t=0 then silence forever. → dedup on a structural
  discriminant, and re-log on **attempt-count doubling** (1,2,4,8,16…), which is
  a derived cadence that invents no constant (CLAUDE.md: no arbitrary limits).
- **M-4 operator messages assert unchecked facts.** All three say the terminal is
  *not* recorded, but `complete_process_exit` can fail **after** a successful
  append (deadline retire / visibility upsert / registry reconcile). An operator
  is told "the run stays Running" about a run that is Completed. → report what
  was observed.
- **M-5 the new knob is completely unpinned** and its fixture doc claims it is
  exercised. Deleting `.with_completion_retry(..)` from `builder.rs:639` leaves
  every test green. Waffles' "no behavioural control needed" ruling justifies no
  *behavioural* control — it does not justify a doc asserting wiring that nothing
  holds. → pin the wiring (large initial backoff, assert the retry has NOT landed
  early) or delete the claim.

## MINORs — all to be handled, none deferred

m-1 `EngineTaskRuntime::shutdown` second call returns without awaiting, so the
claimed "returning implies quiescence" is false for a concurrent caller ·
m-2 `CompletionRetryConfig` accepts zero `initial_backoff` → unbounded hot loop
(`0.saturating_mul(2)==0`); validate the boundary · m-3 the classifier's
"enumerated per variant" claim is false for `EngineError` (`_ => false` covers
~50), and `TerminalWriterHeld` is documented-transient yet classified Invariant,
permanently stopping a continue-as-new chain · m-4 the A2 fixture's second engine
shares nothing with the first, so its assertion is green by construction ·
m-5 the epoch-close test observes emptiness, not awaiting (the await is pinned
elsewhere, by `shutdown_gates_new_arms_and_awaits_aborted_tasks`) · m-6 the
resume-path visibility upsert races the timeout teardown's own · m-7 the terminal
timestamp becomes retry-success time, not exit time — `close_time` drifts by the
outage duration · m-8 the doc omits the `Invariant` abandon path · m-9 neither
backoff knob is reachable from the shipped binary, so config.rs's "the number is
the operator's to set" is not true of the product today.

## What the review CONFIRMED (recorded — a clean result is load-bearing)

No `#[allow]`/`#[expect]`/`#[deny]`/`#[ignore]`/`#[cfg(any())]` anywhere in the
diff or the new file · the retry's idempotence precondition is sound
(`Recorder::append_one` marks success only after `Ok`, `next_envelope` takes
`&self`) · the M3 drop-guard is real (slot captured at construction, not first
poll) · the M1 Dekker pattern is sound (DashMap shard RMW drains the store
buffer) · A5 is structurally satisfied · `sleep_backoff` is genuinely one rule in
one place now · `store_faults.rs` is sound as an injector (no oracle can be
refused by the fault it injects) · **and it independently re-derived Amendment
3's C1 UNDECISIVE finding and confirmed it should not be revisited** — while
noting M-1 is the gap C1's investigation walked past.

## 🔴 BUILD PROGRESS (live — update as it moves)

**Waffles ruled 04:05Z:** C-1's shape right ("cleanup runs UNCONDITIONALLY, the
error returns after" — same family as Apollo's trap-handler fix). **M-1: option
1, explicitly — re-check liveness at the append boundary.** M-3's string key
named as "the stringified-typed-error trap"; key on the typed variant.
M-2/M-4/M-5 + nine minors as stated, nothing deferred. Landing lane for the 5
live origin branches opened at my seat, behind my own gates, in my order.
**Retirement of husks + the frame-trap worktree WAITS for the estate-wide
deletion hold to lift.**

DONE (compiles clean, `cargo check -p aion-rs --all-targets` → 0 errors):
- **C-1** `handle.rs`: three `?` replaced by `first_error` capture; epoch close
  + scheduler shutdown now unconditional; the false "Currently infallible" doc
  replaced with what it actually returns. ⚠️ The compiler's
  `unused_assignments` warning caught that I had collected the errors and never
  returned them — an accidental silent-swallow. Fixed with the closing `match`.
- **M-1** three parts: `EngineTaskRuntime::is_epoch_open()`; the append-boundary
  refusal in `handle_process_exit_attempt` returning the new
  `EngineError::EngineTaskEpochClosed`; and **`impl Drop for Engine`** calling
  the new non-blocking `begin_close()`. The Drop is what makes the check
  reachable — a released engine could not otherwise set the flag, because an
  in-flight attempt's own strong handle keeps `EngineTaskRuntime::drop` from
  running. Refcount (`Arc::strong_count == 1`) was considered and REJECTED: two
  concurrent retries each see 2 and both proceed.
- **M-2** `NotOwner` reclassified non-transient, with the reasoning written
  against the store's own doc (re-resolution is the load-bearing half and this
  loop does not have it).
- **M-3** both halves: `failure_discriminant`/`store_discriminant` typed keys
  replace `error.to_string()`, and the loop re-states the fault whenever
  `attempts` reaches the next power of two — a cadence DERIVED from the counter,
  inventing no interval, cap or budget.
- **M-4** new `TerminalProgress` {NotRecorded, Recorded} set at the two durable
  points (own append `Ok`, terminal found already present), threaded through
  `handle_process_exit_attempt` as an out-parameter → `CompletionFailure` →ordering
  all five operator messages. No message asserts an unchecked durable fact now.
- **m-1** the false "idempotent ⇒ quiescent" claim replaced by what a second
  call actually does.
- **m-8** `retry_process_exit`'s doc now names the `Invariant` abandon path.
- **SUBTRACTION:** `gate_and_abort()` is now the one shared prologue for
  `shutdown`, `begin_close` and `Drop` — three copies of the same three
  `retain`s collapsed to one.

ALSO DONE since that list was written — **`cargo check -p aion-rs --all-targets`
→ `CHECK_EXIT=0`, ZERO errors, ZERO warnings** on the whole reworked tree
(`<scratchpad>/check-4.log`):
- **m-2** zero `initial_backoff` refused at `EngineBuilder::build()` with the new
  `EngineError::InvalidCompletionRetry`. Gated THERE, not in
  `CompletionRetryConfig::new`, because the struct's fields are public — a check
  in the constructor could be walked past with a struct literal. Refuses a
  degenerate value; invents no floor.
- **m-3** both halves: the classifier doc now states honestly that it is TWO
  rules (per-variant for `Store`/`Durability`, fail-closed family default for
  `EngineError`), and records the real mechanism — neither store nor durability
  error is `#[non_exhaustive]`, so a new variant fails to compile rather than
  silently taking a default. **`TerminalWriterHeld` reclassified transient**
  (its own doc says "transient by construction"; it was permanently stopping a
  continue-as-new chain) and given its own discriminant name.
- **m-7** `TerminalIntent` is now a struct carrying `exit_time`, captured once
  when the exit is observed and reused by every attempt. The append uses it
  instead of `Utc::now()`, so a terminal retried through a 40-minute outage no
  longer records a `close_time` 40 minutes after the run actually finished.
  Blast radius measured first: 9 sites, all in `completion.rs`.

⚠️ TWO MEASUREMENT LESSONS THIS PASS, both mine:
1. I read "0 errors" off `check-1.log` when the file was **ZERO BYTES** — the
   `| tail -120` buffered everything until cargo finished. *Absence of error
   lines in an empty log is not a pass.* Only `CHECK_EXIT=` from cargo's own
   status counts.
2. `cargo ... | tail; echo "EXIT=$?"` captures **`tail`'s** exit code, not
   cargo's. Redirect to a file and echo `$?` immediately after the command.

✅ ALL FIVE REMAINING ITEMS BUILT — and every fix now carries a decisive
mutation. `cargo test -p aion-rs --lib` → **TEST_EXIT=0, 642 passed / 0 failed**.

- **M-5** pinned in TWO places, because the knob has two links and neither was
  held. `the_completion_retry_ladder_reaches_the_runtime_configuration`
  (builder.rs) asserts on `runtime_config()`, NOT on a builder accessor — an
  accessor test passes with `.with_completion_retry(..)` deleted, because the
  field still holds what the setter put there while the runtime runs on the
  inherited default. And `the_configured_backoff_ladder_governs_when_a_retry_
  lands` (completion.rs) separates 500 ms from the 1 ms/8 ms default far enough
  to tell them apart — every other test in that module supplies 1 ms/5 ms, which
  is indistinguishable from the default at the resolution they observe. The
  fixture doc now says so instead of implying coverage it never had.
- **m-4** the A2 fixture's second engine DELETED, not re-worded, and the test
  renamed to `a_retrying_completion_returns_with_the_retry_still_armed`. A
  healthy exit reaching `Completed` is already pinned elsewhere; what the second
  engine added was the appearance of coverage.
- **m-5** the epoch-close test's message now claims what it observes
  (emptiness), and names `shutdown_gates_new_arms_and_awaits_aborted_tasks` as
  the test that owns the await.
- **m-6** was a REAL DEFECT, not a doc fix. The first cut carved `TimedOut` out
  of the deadline retirement and not out of the visibility upsert, leaving this
  monitor writing one run's index concurrently with the teardown that woke it.
  Fresh reads do not order writes: read-H1 / teardown-writes-H2 / write-H1
  leaves `close_time` stale for good. A `TimedOut` terminal can never be this
  monitor's own earlier attempt (it appends only Completed/Failed), so the
  pre-retry safe-to-skip condition holds unchanged, and the teardown's own
  deadline anchor is the recovery path if it dies first.
- **m-9** `config.rs` no longer says "the number is the operator's to set". The
  grep is recorded WITH its control: zero hits for `completion_retry` /
  `CompletionRetryConfig` / `signal_delivery` in `aion-server` and `aion-cli`,
  while the same trees DO show `EngineBuilder::new()` — so the absence is a
  signal, not a bad path.

## 🔴 MUTATION BOARD — 15 legs, FAILURES: 0, BOARD_EXIT=0

`<scratchpad>/mut7/run.py` + `REPORT.txt`. Question zero answered before any
control ran: **the verdict channel is the exit code plus the report file**, and
every KILLED/SURVIVED line comes from a per-leg `rc` captured at the call site
by `subprocess.run` — never from parsing cargo's prose, never from a pipeline
tail. An anchor that matches zero times, or more than once where one was
expected, is an ERROR and not a pass. Python, so the zsh read-only-`status`
trap does not apply.

Both directions fired on the real script: `control-noop` (anchor replaced by
itself) SURVIVED, `control-positive` KILLED.

**13 real mutations, 13 KILLED**: C-1a return-early-on-drain-failure · C-1b
swallow-the-drain-failure · M-1a delete `Drop for Engine`'s gate · M-1b delete
the append-boundary check · M-2 NotOwner back to transient · M-3a one key for
two faults · M-3b delete the escalation ladder · M-3c delete the change
detection · M-4 never record terminal progress · M-5a builder drops the ladder ·
M-5b runtime ignores the configured ladder · m-2 accept a zero initial backoff ·
m-6 monitor writes a timed-out run's visibility. Tree verified byte-identical
afterwards by SHA-256 against a pre-run snapshot.

🔴 **THE POSITIVE CONTROL FAILED ON THE FIRST RUN, AND IT WAS MY ERROR.** I
paired a `SequenceConflict` discriminant rename with a test that only ever
constructs `Backend` and `NotOwner` — the mutation was OFF THE TEST'S PATH, so
SURVIVED was the correct answer to the question I actually asked. Replaced with
an on-path control that re-introduces a pre-existing defect (retrying the
double-writer indicator) under a test that drives it end to end. The board
caught my mistake rather than the code's, which is what a control is for.

## What had to be BUILT to make the two hardest fixes measurable

Neither C-1 nor M-4 could be measured with what existed, and that is why both
went unpinned long enough to become review findings.

- `ProcessExitRegistry::force_shutdown_failure()` (`#[cfg(test)]`, in-pattern
  with the ten fault flags already on that type). Every real failure on the
  shutdown drain path is lock poison or a drainer that will not stop inside its
  window; neither is producible from a test, so the epoch-close property had no
  way to be observed while three `?` sat above it.
- `FlakyStore::fail_reads_after(skip, count)`, with `fail_next_reads` becoming
  `skip = 0` — ONE budget, not two. One attempt reads history several times and
  those reads sit on opposite sides of the durable append; a budget that can
  only refuse from the first read can only ever produce failures BEFORE the
  terminal lands, which made "this failure happened after the terminal was
  recorded" untestable while looking covered.
- `FaultReporter` extracted from the retry loop. Inline, the re-log cadence's
  only witness was tracing output — a rule whose sole witness is a log line is a
  rule nothing holds. Extracted, it is three unit tests and one place.
- `FlakyStore::fail_next_reads_with_lost_ownership` for M-2's control.

Also corrected: `handle.rs`'s shutdown comment still said "there is no `Drop`
for `Engine`" AFTER M-1 added one. One truth in two places had already drifted
inside a single lane.

## 🔴 CLIPPY — GREEN, after three real findings it caught

`cargo clippy --workspace --all-targets -- -D warnings` → **CLIPPY_WS_EXIT=0**
(`<scratchpad>/clippy-ws2.log`). It took three passes and every failure was a
genuine defect in my work, fixed in the code — no `#[allow]`, no `#[expect]`.

1. **`too_many_lines` on `EngineBuilder::build` (108/100).** The m-2 gate pushed
   an already-long function over. Cured by SUBTRACTION, twice: the config gate
   became `check_completion_retry`, and the two-step shard motion became
   `claim_owned_shards` (declare, then fence — one motion, one name). 101 → clean.
2. **`build` lost its own doc comment** in the first extraction — the new
   function landed between the doc and the item, so `build`'s `# Errors`
   section silently became `check_completion_retry`'s. Three lints fired at
   once on the same line, which is what surfaced it.
3. 🔴 **`aion-server` failed to COMPILE: E0004, two non-exhaustive matches** on
   `EngineError` (`error.rs:503` trace fields, `error.rs:623` wire mapping),
   both missing the new `EngineTaskEpochClosed` and `InvalidCompletionRetry`.
   **This is exactly the mechanism m-3 documents, firing for real**: neither
   error enum is `#[non_exhaustive]`, so a new variant is a compile failure at
   every consumer rather than a silent default. `cargo clippy -p aion-rs` was
   green while this was broken — the workspace run is not optional.
   Classified with reasons written in place: epoch-closed is the `ShuttingDown`
   family (the engine is going away, the request was not wrong);
   invalid-completion-retry is an input class (the embedder's own config).

## 🔴 REMAINING BEFORE COMMIT

1. Full `cargo test -p aion-rs` (not just `--lib`) — RUNNING.
2. **Re-run the mutation board on the FINAL tree.** The green board above was
   measured before the clippy refactor; a battery is only evidence about the
   tree it ran on.
3. THIRD Opus review, then commit + push.

## ORDER OF WORK


1. Build C-1 (unconditional epoch close + honest doc).
2. Build M-2, M-3, M-4, M-5.
3. Build all nine MINORs.
4. Waffles' M-1 ruling → build it.
5. Re-run suite + clippy + the mutation board on the final tree. New fixes get
   new controls — a fix without a decisive mutation is not landed (learned twice
   in this lane).
6. Third review, then commit + push.
7. Then: land the 5 live origin branches; then delete fossils/husks; then report
   to Waffles.
8. Then Tom's remaining queue: #9, #6, #5 (bump `Keyword::ALL`'s `[Self; 57]` in
   the same change), #1.
