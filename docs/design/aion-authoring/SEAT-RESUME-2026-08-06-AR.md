# SEAT RESUME — marker AR, 2026-08-06

Supersedes AQ. Latest letter wins.

## Where things stand

- **#147 CLOSED.** `origin/main = 9189ba585`. Suite 46/46 verdict lines green,
  lib target 796. Waffles accepted in full and was sent the settled result;
  **R3 stays at his seat, unsent by me.** #94 closed by the same motion.
- **#7 IS IN FLIGHT AND UNCOMMITTED.** Everything below is working-tree state.

## #7 — the brief is the authority

`docs/design/aion-authoring/BRIEF-7-COMPLETION-MONITOR-TRANSIENT-STORE.md`
(202 lines). Read it first; it carries the mechanism, the two findings the triage
did not have, the design decision, and acceptance A1–A5.

**The headline finding: the obvious fix is strictly worse than the defect.**
Process-exit callbacks are drained by ONE thread (`process_exit_callback.rs`
`run_worker`), and `handle_process_exit` is `block_on`-ed on it. A backoff loop
inline would convert a one-run wedge into a node-wide completion wedge. The
precedent in `nif_child_watch` is safe only because it already runs as a spawned
task — copying its loop without its PLACEMENT copies the wrong half.

Second finding: **two callers, not one** — `start.rs:574` AND
`engine/startup.rs:446` both swallow. Cure is subtraction into the shared
function, exactly as #147.

## Working-tree changes (ALL UNCOMMITTED)

| file | change |
|---|---|
| `runtime/nif_child_tasks.rs` → `runtime/engine_tasks.rs` | `git mv`; `ChildTaskRuntime` → `EngineTaskRuntime`; third task kind `completion_retries` keyed `(WorkflowId, RunId)`; `sleep_backoff` promoted here as `pub(crate)` |
| `runtime/mod.rs` | `pub(crate) mod engine_tasks;` |
| `runtime/handle.rs` | `RuntimeHandle` now OWNS `engine_tasks`; `engine_tasks()` accessor |
| `runtime/nif_child_engine.rs` | bridge no longer owns one — borrows `self.runtime.engine_tasks()` |
| `runtime/nif_child_watch.rs` | uses shared `sleep_backoff`; `FlakyAppendStore` extracted out |
| `store_faults.rs` (NEW) | shared `FlakyStore` with `fail_next_appends` AND `fail_next_reads` + `recorded_history` |
| `lib.rs` | `#[cfg(test)] pub(crate) mod store_faults;` |
| `lifecycle/completion.rs` | `TerminalIntent`, `CompletionFailure{Retryable,Invariant}`, `arm_completion_retry`, `retry_process_exit`, `classify_completion_failure`; resume path now DOES the visibility upsert; 5 new tests |

Backoff comes from `RuntimeHandle::signal_delivery()` — the builder's policy.
**No invented default anywhere.**

## 🔴 AMENDMENT 3 — the review's items are closed, and TWO of its claims were WRONG

**Read this first; it supersedes Amendment 2's dispositions.** Amendment 2's
fixes all stand except where corrected below.

### Waffles RULED on M2 (03:27Z) — build the knob, do not invent the number

Verbatim shape: build the distinct builder-supplied completion-retry policy **in
this lane**, unbounded by attempts (option 3 rejected on my grounds), and pin the
call site to the values the code effectively ran TODAY — **1 ms initial, 8 ms
ceiling** — verbatim. *"That is not choosing a default: it is making the
accidental value explicit, attributable, and changeable in one place."* The
defect was the **coupling**; the coupling dies here, the value stays visibly
unchosen. Behaviour is byte-for-byte the old behaviour, **so nothing new needs a
control** — his words, and the reason there is no knob control below. Stop and
land option 1 only if it touches serialization or files outside the lane; it did
neither. He carries the 125-reads/sec question to Tom with my seconds-scale
recommendation attached. **One holder, and it is him.**

### BUILT for that ruling

| file | change |
|---|---|
| `runtime/config.rs` | NEW `CompletionRetryConfig { initial_backoff, max_backoff }`; `RuntimeConfig.completion_retry` + `with_completion_retry`. Its `Default` impl carries the 🔴 "inherited, not chosen" note and the 125-reads/sec figure. |
| `runtime/handle.rs` | `completion_retry` field + `completion_retry()` accessor, documented as deliberately NOT `signal_delivery()`. |
| `engine/builder.rs` | `EngineBuilder::completion_retry(..)` threaded into `runtime_config()`. |
| `lib.rs`, `runtime/mod.rs` | `CompletionRetryConfig` exported. |
| `lifecycle/completion.rs` | `arm_completion_retry` reads `completion_retry()`; `retry_process_exit` takes the new type; test `fast_backoff()` returns it and is wired via `with_completion_retry`. |
| `runtime/engine_tasks.rs` | `sleep_backoff(current, ceiling: Duration)` — takes the CEILING, not a policy struct, so the helper cannot re-couple two callers that no longer share a policy. |
| `runtime/nif_child_spawn_retry.rs` | 🔴 **deleted a byte-identical private `sleep_backoff`** — the promotion had left one rule in two places. Now imports the shared one. |

### 🔴 TWO REVIEW CLAIMS THAT DID NOT SURVIVE MEASUREMENT

**C1 was NOT a durability defect.** Deleting the `completion_retries` abort sweep
from `Drop for EngineTaskRuntime` leaves **every test in the crate green**
(`exit 0`, 7 passed / 0 failed on `runtime::engine_tasks`). The reason is
mechanical: `Drop` then drops the owned tokio `Runtime`, and that drop cancels
every remaining task **whichever map its handle sat in**. So the sweep is not
what stops a retry outliving the epoch — the runtime drop is.

The sweep is KEPT (it makes `Drop` and `shutdown` close the epoch by the same
three steps, which is a real anti-drift property) but **the comment I wrote to
justify it was false** and has been replaced with what was measured. The test
`dropping_the_registry_aborts_completion_retries_too` is kept as a **pin, not a
control**, and its doc now says so in the file. A line with no independent
observable has no independent control; saying that is cheaper than a test that
passes for a reason other than the one it names.

**C2 was real, and its fix had NO test until now.** The first control run said
`UNDECISIVE`: deleting `self.engine_tasks.shutdown()` from
`RuntimeHandle::shutdown` left **629 passed / 0 failed**. New pin
`runtime_shutdown_closes_the_completion_retry_epoch` (in `lifecycle::completion`)
arms a retry, calls the runtime's own shutdown, and asserts the count goes 1 → 0.
With the pin, the same mutation is **DECISIVE (629 passed / 1 failed)**.

⇒ The generalisable rule, twice in one lane: **a fix is not landed until a
mutation that removes it turns something red.** Both C1 and C2 read as "fixed"
from the code and the green suite. Only the mutation separated them.

### CONTROL BOARD (all seven re-run on the FINAL tree — the earlier five were stale)

| control | mutation | verdict |
|---|---|---|
| A1 | `classify_completion_failure` always `Invariant` | DECISIVE — 5 failed |
| A2 | await the retry inline instead of arming | DECISIVE — 3 failed, run time 0.03s → **3.34s** |
| A4 | delete the resume path's visibility upsert | DECISIVE — 1 failed |
| BOUND | key the retry on `RunId::new_v4()` | DECISIVE — 1 failed |
| C3 | every `StoreError` classified transient | DECISIVE — 1 failed |
| C1 | `Drop` forgets `completion_retries` | 🔴 **UNDECISIVE — 0 failed** (see above) |
| C2a | `RuntimeHandle::shutdown` skips the epoch close | DECISIVE — 1 failed (was 0 before the new pin) |

Every file restored **byte-identical** (sha256 compared, recorded per control).
Harness + logs: `<scratchpad>/mutctl/` (`run.py`, `REPORT.txt`, one log per control).

### ALSO CLOSED IN THIS AMENDMENT

- **M6** — A2's doc comment now claims only `armed_completion_retry_count()==1`
  and states plainly that its fixture never touches the real dispatcher thread.
- **M5** — A5 is recorded in the brief as **structural, not executed**, with the
  reason (both call sites need a live beamr process) and what closing it
  empirically would take. No control is claimed because none exists.
- **A6 added to the brief** — the `SequenceConflict` pin.
- Brief gained a **"What these criteria do NOT prove"** section.
- Naming: `child_tasks()` → `engine_tasks()`, `shutdown_child_tasks()` →
  `shutdown_engine_tasks()` across `nif_child*.rs`, `nif_state.rs`, `api.rs` (19
  call sites) — the old names understated an engine-wide executor.
- Stale docs corrected: `engine_tasks::shutdown` described a channel the code
  does not use; the shutdown-thread spawn-failure branch called dropping the
  runtime on the calling thread a "graceful fallback"; `nif_child_watch.rs`
  dangling `FlakyStore` doc line; `runtime/mod.rs` alphabetical run restored.
- `handle_process_exit`'s `# Errors` doc now states that an arming REFUSAL also
  returns `Ok(())`, and why that is right rather than glossed.

### STILL OPEN

1. Full `cargo test -p aion-rs` + clippy on the final tree — **running, receipts
   at `<scratchpad>/suite-7c.log` and `clippy-7d.log`. Read the `*_EXIT=` line;
   never the absence of bad lines.**
2. **SECOND Opus review** — non-negotiable. The first found 3+6; this amendment
   proves the tree changed materially since.
3. Commit, push, § AS BUILT.
4. Then Tom's remaining queue: #9 (reply goes past Waffles BEFORE it posts), #6,
   #5 (must bump `Keyword::ALL`'s `[Self; 57]` in the same change), #1.

## 🔴 AMENDMENT 2 — OPUS REVIEW CAME BACK RED. 3 CRITICAL + 6 MAJOR. NOTHING LANDED.

**Read this first. It supersedes Amendment 1's "ready" tone.** Amendment 1's
measurements were all true and still hold; they were just not sufficient.

I verified C1/C2/C3 against the bytes myself before acting. All three real.

### The three CRITICALs (all now FIXED in the working tree, unverified by test yet)

- **C1 — `Drop for EngineTaskRuntime` never aborted `completion_retries`.** It
  aborted `watches` and `spawn_retries` and stopped. The one task kind that
  appends TERMINAL events was the one the backstop skipped.
  **FIXED**: third `retain` added to `Drop`.
- **C2 — the epoch close never reached the executor `RuntimeHandle` owns.**
  `RuntimeHandle::shutdown()` did not call `engine_tasks.shutdown()`; the only
  production route was `api.rs:849 → nif_state → installed_child_bridge()` —
  i.e. **through the optional bridge my own brief rejected**. Ownership moved;
  the epoch close did not. Compounded by a reference cycle: the spawned future
  held `Arc<RuntimeHandle>` (via `ProcessExitContext`) which owns
  `engine_tasks`, so `Drop` could never run while a retry was armed.
  **FIXED, two parts**: (a) `RuntimeHandle::shutdown()` now calls
  `self.engine_tasks.shutdown()` after `process_exits.close_and_join_all()`;
  (b) new `CompletionRetryContext` holds `Weak<RuntimeHandle>` and upgrades per
  attempt — cycle broken, `Drop` reachable, and a released engine now ends the
  loop with a named warn instead of looping forever.
- **C3 — the classification retried invariant violations forever.** Matching
  whole `EngineError::Store` / `Durability` families swept in
  `StoreError::SequenceConflict` (CLAUDE.md invariant 3 calls this a
  double-writer bug), `NotFound`, `Serialization`,
  `DurabilityError::NonDeterminism`, `HistoryShape`, `SearchAttribute`. My own
  doc comment two lines above claimed the opposite.
  **FIXED**: `completion_failure_is_transient` + `store_error_is_transient`
  enumerate per variant. Only `Backend(_)` and `NotOwner{..}` (which the store's
  own docs mark retryable) are transient. Doc comment rewritten to match.

### The six MAJORs

- **M1 window between the gate check and the spawn** → `arm` returned `true`
  for a task `shutdown` had already swept, so the operator was told the work was
  owned when nothing owned it. **FIXED**: `arm` re-reads `shutting_down` after
  inserting and undoes its own arm.
- **M3 a panicking retry was completely silent** — skipped its removal,
  stranded the map entry, no log, and `armed_completion_retry_count` still
  reported it live. **FIXED**: `CompletionRetrySlot` drop-guard (holds the
  executor `Weak`), released on return, unwind, and abort.
- **M2 backoff semantics** — `SignalDeliveryConfig` is a *bounded* enqueue
  ladder (max 8 ms) being used as an *unbounded* durable-retry ladder: ~125
  reads/s **and 125 warn lines/s per stuck run**, forever, against a store
  already failing. **LOG HALF FIXED** (transition-only logging, the #94 rule —
  a fact is logged when it CHANGES; plus an `attempts` field). **READ-RATE HALF
  IS NOT FIXED AND MUST NOT BE INVENTED**: a real completion-retry policy is a
  VALUE DECISION and CLAUDE.md says discuss before implementing. **Route to
  Waffles.**
- **M4 no test pinned the epoch close for this task kind** — which is exactly
  why C1 survived to review. **FIXED**: `shutdown_gates_new_arms_and_awaits_aborted_tasks`
  extended to arm/assert a completion retry, plus new
  `dropping_the_registry_aborts_completion_retries_too`.
- **M5 A5 has no test and no control.** Neither `start.rs:574` nor
  `engine/startup.rs:446` is executed by ANY test in the crate. Structurally
  satisfied, empirically unproven. **STILL OPEN.**
- **M6 the A2 test's doc claims more than its fixture can observe** — it builds
  two independent engines and calls the async fn directly, so the single
  dispatcher thread is never on the path. What it genuinely pins (and what the
  A2 control killed) is `armed_completion_retry_count()==1` — "the call returned
  while a retry is still armed" — which IS the load-bearing property.
  **STILL OPEN: rewrite the doc comment to claim only that.**

### 🔴 EXACTLY WHERE I STOPPED

`cargo check -p aion-rs --all-targets` → **CHECK_EXIT=0** after all the above.
Then I was mid-way through adding the **C3 regression pin** and got compacted.

**The C3 pin is HALF-BUILT — this is the next keystroke:**
`crates/aion/src/store_faults.rs` needs a fault that returns
`StoreError::SequenceConflict{..}` instead of `Backend(_)` (mirror
`fail_next_reads`/`remaining_read_failures`, see `take_failure` at line 63 and
the read site at line 80). Then a test in `lifecycle::completion::tests` that
injects it and asserts **`handle_process_exit_async` returns `Err` and
`armed_completion_retry_count() == 0`** — a conflict must be abandoned loudly,
never retried.

### THEN, IN ORDER

1. Finish the C3 pin (above).
2. M6: fix the A2 doc comment.
3. M5: decide — either cover a call site or state plainly in the brief that A5
   is structural-only. **Do not claim a control that does not exist.**
4. Re-run all four Amendment-1 mutation controls — **the code changed under
   them, so those results are now STALE.** Pristine copies are at
   `<scratchpad>/pristine/`; backup + hashes at `<scratchpad>/wip7-backup/`.
5. Add controls for the new fixes: delete the `Drop` retain → C1 test red;
   delete `engine_tasks.shutdown()` from `RuntimeHandle::shutdown` → ?; make
   `store_error_is_transient` return true for everything → C3 pin red.
6. Full `cargo test -p aion-rs` with `TEST_EXIT=$?`, clippy, **second Opus
   review** (the first found this much; do not skip it), commit, push, § AS BUILT.
7. Route M2's read-rate value question to Waffles.

**Minor items from the review still to sweep:** `ChildNifBridge::child_tasks()`
and `shutdown_child_tasks()` now understate scope (engine-wide, not child);
`nif_state.rs:311-316` and `api.rs:841-847` docs still enumerate only
"watcher and spawn-recovery"; dangling doc line at `nif_child_watch.rs:965`;
`runtime/mod.rs:27` breaks the alphabetical run; `completion.rs` arming-refused
branch returns `Ok(())` though nothing owns the terminal.

**Already done in this amendment:** thread renamed `aion-engine-tasks-shutdown`,
`# Errors` doc on `handle_process_exit` corrected.

## AMENDMENT 1 — module tests green, all four controls decisive, clippy clean

Written after the re-run. Supersedes the "9/10" section below, which is kept
because its lesson stands.

**Module tests: 10/10.** `lifecycle::completion`, 617 filtered out.

**All four mutation controls run, each decisive:**

| control | mutation | result |
|---|---|---|
| A1 | `classify_completion_failure` returns `Invariant` for everything | 4 failed — both transient tests **and** both arming tests (they depend on classification producing a retry) |
| A2 | `await` the retry inline instead of arming | 2 failed — **only** the two arming tests; transient tests stayed green, correctly (an inline await still lands the terminal, it just wedges the dispatcher). Run time 0.07s → **3.36s** — the wedge showing up in the clock |
| A4 | delete the new `upsert_workflow_visibility` from the resume branch | 1 failed — `a_resumed_completion_appends_nothing_and_still_refreshes_visibility`, on the visibility half **by name**: `the resume path refreshes the index it used to skip: []` |
| bound | key the retry on `RunId::new_v4()` instead of the handle's run | 1 failed — `left: 25, right: 1` |

Each restored byte-identical (`cmp` against saved pristine copies).

A2 is the one worth keeping: it separates *lands the terminal* from *does not
hold the dispatcher*. A control that killed both halves would not have told us
which property the test measures.

**Clippy: EXIT=0, 0 findings** — after fixing six real ones, listed below.

## 🔴 TWO PROCESS LESSONS FROM THIS PASS

### 1. A reaped background process leaves a log that reads exactly like a clean pass

The first clippy run was reaped mid-build. Its log ended at
`Checking aion-core` with **zero** `^(warning|error)` matches. A count of zero
findings in a truncated log is indistinguishable from a clean run.

⇒ **A gate's verdict must come from its own exit code, not from the absence of
bad lines in its output.** Every gate now runs as
`nohup sh -c '<cmd>; echo "EXIT=$?"'` and is read by grepping for the receipt.
The re-run with the receipt returned `CLIPPY_EXIT=101` and six real findings.

This is the same shape as the mis-keyed grep in marker AQ: absence of a match
was read as presence of a property. Pair absence with survival — here, survival
is the exit-code line existing at all.

### 2. Clippy caught a husk I had written to excuse my own leftover

`ChildNifBridge::new` returned `Result` only because it used to construct the
executor. After the promotion it constructs nothing fallible, and I wrote:

> *"Infallible today; the signature is kept fallible because construction sits on
> the engine's start path, where a new dependency that can fail must not force
> every caller to change."*

That is a limitation dressed as a contract — speculative future-proofing, which
this repo forbids outright. `unnecessary_wraps` named it. Fixed by removing the
`Result` and updating all four call sites, not by an `#[allow]`.

⇒ **Read your own justifications as evidence.** A reason that exists to excuse
a leftover reads the same as a reason that documents a decision.

## Clippy findings fixed (six, no suppressions)

- 4 × `needless_borrow` in `completion.rs` (352, 360, 361, 381) — fallout from
  `handle_process_exit_attempt` taking `&context` / `&handle` instead of owned.
- 1 × `unnecessary_wraps` on `ChildNifBridge::new` — the husk above.
- 1 × `field_reassign_with_default` in the test helper `fast_backoff()` —
  rewritten as a struct literal with `..Default::default()`.

## 🔴 SEPARATE FINDING, FILED NOT FIXED — the BC-4 differential gate

`docs/design/aion-authoring/FINDING-BC4-DIFFERENTIAL-NONDETERMINISM-2026-08-06.md`

The full `cargo test -p aion-rs` came back with one red:
`awl_bc4_differential :: covered::covered_set_is_byte_identical`.

**It is red on pristine main, and its number is not a measurement.** Established
by stashing the six tracked #7 paths by name, restoring the rename's delete
side, confirming no tracked modification, and running the target twice:

| run | tree | DIVERGENCES |
|---|---|---|
| pristine 1 | `9189ba585` clean | **51** |
| pristine 2 | `9189ba585` clean | **2** |
| with #7 | working tree | 25 |

Two runs of the same bytes differ 25×. The #7 sample sits inside the pristine
range, so this harness could neither convict nor clear the change. **Reading
"25 vs 51" as an improvement would have been the error the finding warns
against.**

Not in the #7 lane and not on Tom's six-issue list — filed, tracked, not taken.

## HOW THE BASELINE WAS TAKEN SAFELY (repeat this)

`git stash push` with **explicit paths**, never `-u` (the three NO-SWEEP
untracked dirs must never be swept). Before stashing: `tar czf` the changed
files, `git diff HEAD > .patch`, and `shasum -a 256 > sha256.txt`. After
`git stash pop`, `shasum -c` all eight — every one `OK`. The stash leaves the
rename's delete side unstaged; `git add` it to re-pair `R`.

## 🔴 TEST STATE — 9/10, AND THE ONE FAILURE WAS MINE (superseded, lesson stands)

Last run: `9 passed; 1 failed`. The failure was
`a_transient_history_read_failure_still_lands_the_terminal`, reporting
`Backend("transient history read failure injected by FlakyStore")`.

**That was a defect in the ORACLE, not the fix.** `await_terminal` polled through
the same `FlakyStore` it was measuring, so the test's own observation drew from
the injected failure budget and got refused — an instrument failure wearing the
costume of a broken fix.

⇒ **AN ORACLE SUBJECT TO THE TREATMENT CANNOT MEASURE IT.** Fixed by adding
`FlakyStore::recorded_history`, which reads the wrapped store past the injector,
and pointing `await_terminal` at it. **`cargo check --all-targets` passes; the
tests have NOT been re-run since that fix.**

## ON RESUME, IN ORDER

1. **Re-run:** `env -u CARGO_TARGET_DIR cargo test -p aion-rs --lib lifecycle::completion`
   — expect 10/10. Build alone takes ~4.5 min, so run it DETACHED and wait with a
   `until grep -qE "^test result:|^error" LOG; do sleep 5; done` loop. A bare
   2-minute foreground call times out on the BUILD, not a hang.
2. **Controls — none are done yet.** Each acceptance needs a mutation that makes
   it fail:
   - A1: make `classify_completion_failure` return `Invariant` for everything →
     both transient tests must go red.
   - A2: `await` the retry inline instead of arming → the
     `does_not_hold_up_another_workflows_completion` test must go red.
   - A4: delete the new `upsert_workflow_visibility` from the resume branch →
     `a_resumed_completion_appends_nothing_and_still_refreshes_visibility` must
     go red on the visibility half only.
   - Registry bound: key the retry on something caller-supplied → the
     twenty-five-arm test must go red.
   Restore byte-identical after each (`cmp` against a saved copy).
3. Full `cargo test -p aion-rs` + `cargo clippy -p aion-rs --all-targets -- -D warnings`.
4. Opus sub-agent review (CLAUDE.md mandates it), then commit + push.
5. Update the brief with an § AS BUILT recording departures and the oracle lesson.

## Then, Tom's "fix everything" queue

**#9** operator fact in `details` (`RETRY-CLASS-HONESTY-90.md` §5) — **the reply
goes past Waffles BEFORE it posts** · **#6** `max_concurrency` ignored on liminal
+ missing pin · **#5** AWL `sum` (bump `Keyword::ALL`'s `[Self; 57]` in the SAME
change) · **#1** `workflow.now()` history tail vs replay position.

Chanreg measurement lane is released and now unblocked.

## Standing

**Tom's pids 45425 (server) / 48399 (worker): untouched all session.** Worker
rebuilds still frozen estate-wide until the #139 live cut. Three NO-SWEEP
untracked dirs remain untracked.
