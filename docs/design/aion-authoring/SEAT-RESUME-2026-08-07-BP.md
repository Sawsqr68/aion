# Seat resume — marker BP, 7 August 2026

Supersedes BO. **Latest letter wins.** BO holds the three green gate results and
their reconciliation, the god-file correction, and the tutorial accuracy pass —
still the detail record. This marker carries what changed since.

`HEAD` = `9189ba585`. Working tree carries the #7 lane **plus** the tutorial
install fix **plus** the in-progress F1 fix — all uncommitted.

## 🔴 REVIEW 13: REFUSE — the eighth consecutive refusal

Full transcript saved to `scratchpad/review-13.jsonl` (1.6 MB). Findings, in
the reviewer's own severity order:

| # | Sev | Where | What |
|---|---|---|---|
| F1 | **BLOCKER** | `nif_timer_bridge.rs:320` vs `:360` | `arm_timer` is a check-then-act: the gate is read, then a registry read + `remove` + `spawn` happen, and only THEN the insert. `shutdown_timer_wheel` can set the flag and snapshot between them, so the entry lands in an already-drained map → a live task records a durable `TimerFired` for a run a successor owns. The comment at `:170-175` claims the opposite. **The correct pattern is `EngineTaskRuntime::arm` (`engine_tasks.rs:304-325`), 200 lines away in the same lane.** |
| F2 | MAJOR | `engine/api.rs:131-141`; writer at `nif_activity_retry_dispatch.rs:88` | `Drop for Engine`'s durable-writer enumeration says three; there are **four**. `spawn_completion_task` discards its `JoinHandle` (detached), sleeps an SDK-declared backoff, and appends `ActivityFailed`/`ActivityStarted`/`ActivityAdvisoryExhausted` with **no epoch check**. Not created by this lane — but the lane added a doc asserting its absence. |
| F3 | MINOR | `aion-client/src/transport/embedded.rs:572` | `ShuttingDown → not_running` gives a false operator hint, by the lane's own F4 argument (which moved `EngineTaskEpochClosed` OFF `not_running` for exactly this reason). Two edits in one lane pointing opposite ways. |
| F4 | MINOR | `aion-store/src/testing/refusing_reads.rs:81-88` vs `:184-189` | `fence()` refuses every read with `NotOwner { shard: 3 }` while `is_current_owner` still answers "yes" for other shards — a state no real backend occupies. |
| F5 | MINOR | `engine/api.rs:505-520`, `runtime/handle.rs:873-891` | The second-error reporting branch **is** defect 2's fix and **no test executes it**: the only injection seam fires `begin_shutdown`, always the first fallible step, so `first_error` is always `None` and the `else` never runs. Surviving mutation named: replace both `else` bodies with `{}`. |
| F6 | MINOR | `engine/api.rs:37`, `:48` | `delegated` and `visibility_reconciliation_task` widened to `pub(super)` but read by no sibling module. Revert to private. |
| F7 | MINOR | `engine/api.rs:467-485` | `Engine::shutdown`'s trade comment omits that continuing past `close_and_wait`'s poison skips the gate's **drain** guarantee. |

**Reviewer explicitly checked and found clean** (worth keeping — it is the
attributable absence): error loss in both shutdown paths; the three-way split
audited item-by-item against HEAD; one-executor-per-node verified exhaustively;
`nif_child_watch`/`nif_child_spawn_retry` (every touched assertion *tightened*);
the completion retry (no double-append, no stale projection); `log_capture`'s
`InterestFloor` reasoning; CLAUDE.md compliance (**zero** `#[allow]`/`#[expect]`/
`#[ignore]`/`unwrap`/`expect`/`panic` across all 34 lane files); and a killing
mutation nameable for every significant new test **except** the two F5 branches.

## Work done this tick

### F1 — FIXED IN CODE (test still owed)

`crates/aion/src/runtime/nif_timer_bridge.rs`:

1. **Post-insert re-read** in `arm_timer`, retracting by **generation identity**
   via `remove_if` (never by key alone), mirroring `EngineTaskRuntime::arm`.
   SeqCst on the one store and both loads totally orders drain vs arm.
2. **Append-boundary gate** in `record_workflow_event`, read **inside the
   recorder lock**. `JoinHandle::abort` does not stop a task already inside a
   poll, so without this the wheel is the only durable writer in the crate with
   no boundary refusal at all. `shut_down` became `Arc<AtomicBool>` so the
   blocking future can hold a clone.
3. `wheel_torn_down()` — the refusal named **once**, used by all three sites.
4. The false comment at `:170-175` rewritten to state the two-sided rule and to
   say plainly that observing the flag unset does not put an entry in the map.
5. `#[cfg(test)] arm_interleave` hook + `set_arm_interleave`/`fire_arm_interleave`,
   fired between the gate and the insert. **Reason it exists:** a test that just
   calls `shutdown_timer_wheel()` then `arm_timer` exercises the pre-arm gate,
   which was already correct, and stays green with the fix deleted. The hook is
   the only way to put a test on the path of the code being fixed.

`cargo check -p aion-rs --all-targets` passes. **NOT YET DONE: the test.**
Plan — in `nif_timer_bridge_tests.rs`, using the `seed_started_workflow` pattern
from `nif_query_tests.rs:171`: install the hook to call `shutdown_timer_wheel()`,
call `arm_timer`, assert `Err` containing "torn down" AND
`armed_wheel_timers() == 0`; **positive control** without the hook asserting the
arm succeeds and the count is 1. Then mutate by deleting the post-insert block
and watch it go red.

### 🆕 Tutorial: the install gap (Tom's question, and it was a real hole)

The deck told the reader to run `aion --version` and **never said how to get
the binary**. Worse, the obvious guess is wrong: **`aion` on crates.io is
somebody else's crate** — a datetime library, 0.2.0, `bin_names: []` — so
`cargo install aion` fails with *"there is nothing to install"* (measured, with
`--root` to a scratch dir; Tom's `~/.cargo/bin/aion` untouched). The name that
works is **`aion-cli`** (0.11.0, `bin_names: ['aion']`).

Fixed in `docs/authoring/build-tutorial.py`: `cli_crate_and_version()` reads the
crate name from `crates/aion-cli/Cargo.toml` and the version from the workspace,
so the slide cannot go stale. **Three guards, all mutation-killed** (RC=1 each,
both manifests restored byte-identical by shasum):
rename the binary · rename the package to `aion` · unhook the version from the
workspace. `_binary()`'s refusal now uses the same derived name.

### 🆕 The NOT-PROVEN console step is now SETTLED

BO recorded "the run appears in the console listing" as unproven because a
guessed endpoint returned 200 with a non-JSON body. That 200 was the **console
SPA fallback** serving `index.html`. The real endpoint is **`POST /workflows/list`
at the root**, and it requires a `namespace`. Measured on the live tutorial
server: `{"namespace":"default"}` → `{"summaries":[]}` — correctly empty before
any run, which is a proper control for the check after Tom's run.
(`/api/v1/...`, `/v1/...`, `/api/...` all → 405: those are console routes.)

## 🔴 LIVE: Tom's tutorial world is UP and he is mid-walk

He asked to be set up to the point where he starts the worker. Standing at
Step 7.

- **Tutorial server pid `50822`**, `~/my-first-flow`, ports **:8099 / :50099**,
  `AION_HOME=~/my-first-flow/aion-home`. Started `nohup`+`disown` (the bg-bash
  reaper kills plain background jobs). Log: `~/my-first-flow/server.log`.
- `first_flow.awl` checked (`ok`, 1 step) and **deployed** — hash
  `629b51413b62804c281dc0dca4e4a39e4b211d757ab68db70f4c0e5a00bdcf39`,
  `freshly_loaded: true`, `route_changed: true`. Also copied into the console's
  authoring root so it appears in **Authoring**.
- `worker.toml` in place. Availability probe **before** his worker:
  `{"available":false,"connected_workers":0}` — the control for the flip to 1.
- ⛔ **Deliberately NOT on his live server.** Verified first that queue `desk`
  is free (his four documents use `default`, `local`, `staged_rounds`), but a
  separate world means the deck's own ports work verbatim and nothing of his is
  at risk. **Tom's pids 45425 / 48399 remain untouched, 1d 16h, :8080 → 200.**

⚠️ **Teardown, when he is done:** target the **listening port**, not a pgrep
pattern — `lsof -tiTCP:8099 -sTCP:LISTEN`. A pattern match on the command line
fails here (relative `--config` after a `cd`) and writes a confident false
success. This bit twice already.

## ON RESUME — DO THIS FIRST

1. **Stay responsive to Tom** — he is mid-walk at Step 7 and expects an answer
   when the worker comes up.
2. **Finish F1's test + its mutation proof** (plan above). Then F2–F7.
3. Any code change means a **new tree** means a **new battery**. BO's green does
   not survive these edits. The tutorial files are docs-only and outside every
   crate build graph (verified: no `include_str!`/`include_bytes!` of `docs/`,
   no manifest reference to `docs/authoring`).
4. **Commit in three separate lots**, paths named **explicitly**, never
   `git add -A`: (a) the tutorial install fix — `docs/authoring/build-tutorial.py`,
   `docs/authoring/tutorial-examples/aion.toml`, `docs/authoring/first-workflow.html`,
   `docs/design/aion-authoring/TUTORIAL-WALK-CRIB-2026-08-07.md`; (b) the #7 lane
   once a review finally passes; (c) markers. Keep
   `docs/design/aion-authoring/SPEC-5-AWL-SUM-COMBINATOR.md` **excluded** (1.1 MB
   of raw agent JSONL under a `.md` path; issue #5's artefact; NOT deleted).
5. Rows still owed: the `~`-in-`data_dir` defect (taken literally, creates a
   directory named `~`); `crates/aion-server/src/error.rs` at 567 production
   lines (563 at HEAD — pre-existing, not this lane's).

## Owed to Tom, in plain words

Reviews 6 through **13** have all refused. That is the reason the #7 lane is not
committed — not a failure to report, the reason. Review 13's blocker is a real
race with a real fix, and the correct pattern was already in the same lane.

## Standing constraints (unchanged)

⛔ Never `git add -A`; name paths. ⛔ Never bare `cargo clean`. ⛔ No worktrees
in aion; one mutating lane. ⛔ Never publish artifacts for Tom — deliverables
are local files he opens. ⛔ Do not edit `CLAUDE.md`. Never mention prospekt.
🔴 Worker rebuilds frozen estate-wide until the #139 cut. 🔴 beamr JIT blocker
with Artemis — keep long workflows off. 🔴 Tom's `~/.aion/config.toml`
`[ops_console] FileSystem` line stays until he installs a binary from
`b20d2da1` or later. Backticked commit messages via `git commit -F -`.
NO-SWEEP untracked: `.aion-accepted-binary-20260731/`,
`.aion-jjuabk-snapshot-20260731/`, `.aion-data-preswap-20260804/`.
⚠️ A store cannot live under `/tmp`. ⚠️ zsh does not word-split unquoted
expansions, and `status` is read-only — name the variable `rc`.

---

# AMENDMENT — the AWL worker, and Tom's live walk (2026-08-07)

## 🔴 TOM WAS RIGHT AND I WAS WRONG — declared action bodies exist

I told him the tutorial's worker needs a `worker.toml` and "you write no code".
He pushed back: *"I thought we had everything set up so we could write workers
using AWL as well."* He was correct. **An AWL action can carry a `run` body and
the server executes it directly — no worker process, no manifest, no
registration.** `docs/workers/DECLARED-COMMANDS.md` documents it in full;
`crates/aion-awl/tests/fixtures/rev2/declarations/valid/action_run_bodies.awl`
is the gated syntax reference.

**Proven live rather than asserted**, on the tutorial server: availability probe
for queue `desk_awl` → `connected_workers: 0`, then the run completed anyway with
a real directory listing. Workflow `34492e88-369f-4534-ac82-025f68520883`,
outcome `done`.

**Rules that matter when writing one:** the result is always
`exit_code`/`stdout`/`stderr` (or a named subset — anything else is refused by
the checker); the body sits under the action after any config line, args
interpolate as `$name`/`${name}`; **it is not a shell** — no pipes, redirects or
`&&`. And **there is no sandbox** of any kind.

## Deck changes made (docs/authoring/first-workflow.html — 26 slides)

- **New gated example** `docs/authoring/tutorial-examples/first_flow_awl.awl`.
- Step 5 is now four slides: "Two ways to have a worker" (a real choice, both
  first class, mixable on one queue) · "The worker, written in the same
  document" (the three rules) · "Read this before you deploy one" (no sandbox,
  in full, not a footnote) · "Or: wire the action to a command from outside"
  (the old `worker.toml` path, now labelled way two).
- **"Start the worker" now opens by telling way-one readers to skip it** —
  otherwise they watch `connected_workers: 0` forever waiting for something
  that was never going to connect.
- Build green: 26 slides, 4 excerpts from 5 of 5 files, 3 documents checked.

## 🔴 TOM'S STANDING INSTRUCTION — DO NOT WRITE HIS FLOW

*"I don't want you to write the flow for me. That's the whole point. I want to
be writing the workflow and the worker."*

His workspace was therefore **cleared**: `~/my-first-flow` has only `aion.toml`,
`aion-data`, `aion-home`, `server.log` and a `reference/` folder holding my
three files (`first_flow.awl`, `first_flow_awl.awl`, `worker.toml`) out of the
way. The console's Authoring list is **empty** — he starts at **New workflow**.
Answer his questions; do not author for him.

## Live state (tutorial world — SEPARATE from his production one)

- Tutorial server **pid 50822**, `~/my-first-flow`, **:8099 / :50099**,
  `AION_HOME=~/my-first-flow/aion-home`, started `nohup`+`disown`.
- Already deployed on it: `first_flow` (hash `629b5141…`) and `first_flow_awl`.
  Both routes live; his own document will not clash unless he reuses a name.
- **Tom's production pids 45425 / 48399 untouched**, :8080 → 200.
- ⚠️ Teardown when he is done: `lsof -tiTCP:8099 -sTCP:LISTEN`, **by port**.

## #7 lane — where F1 actually stands

Code fix complete and compiling. **Tests written but NOT YET PASSING** — the
last compile error was `loaded_version:` in `seed_running_workflow` wanting
`ContentHash`, not `PackageVersion` (see `nif_query_tests.rs`, which imports
`aion_package::ContentHash` and uses a `hash()` helper). Fix that one line in
`crates/aion/src/runtime/nif_timer_bridge_tests.rs`, then:

1. Run `cargo test -p aion-rs --lib runtime::nif_timer_bridge` — expect the two
   new tests green.
2. **Mutation-prove both**: (a) delete the post-insert re-read block in
   `arm_timer` → `a_wheel_teardown_during_arming_leaves_no_armed_timer` must go
   RED; (b) delete the `shut_down` check inside `record_workflow_event`'s
   blocking future → `a_torn_down_wheel_refuses_to_append` must go RED. Restore
   byte-identical by `shasum` and re-run green.
3. Then F2–F7 from the table above.

Files touched for F1 so far: `crates/aion/src/runtime/nif_timer_bridge.rs`,
`crates/aion/src/runtime/nif_timer_bridge_tests.rs`.

## Commit lots (still three, paths named explicitly, NEVER `git add -A`)

(a) **tutorial** — `docs/authoring/build-tutorial.py`,
`docs/authoring/tutorial-examples/aion.toml`,
`docs/authoring/tutorial-examples/first_flow_awl.awl`,
`docs/authoring/first-workflow.html`,
`docs/design/aion-authoring/TUTORIAL-WALK-CRIB-2026-08-07.md`;
(b) the **#7 lane** once a review finally passes;
(c) **markers**. Keep `SPEC-5-AWL-SUM-COMBINATOR.md` excluded.

---

# AMENDMENT 2 — review 13 findings, F1/F3/F4/F6/F7 CLOSED WITH PROOF

Every fix below is mutation-proven: the mutation uses the LITERAL prior text,
turns exactly its own test RED, and the file restores byte-identical by
`shasum`. Two remain: **F2** and **F5**.

| # | State | Proof |
|---|---|---|
| **F1** | ✅ CLOSED | 2 new tests in `nif_timer_bridge_tests.rs`. **M1** delete the post-insert re-read → `a_wheel_teardown_during_arming_leaves_no_armed_timer` RED. **M2** delete the append-boundary gate → `a_torn_down_wheel_refuses_to_append` RED. Each killed exactly its own test, no overlap. Restored, 3/3 green. |
| **F3** | ✅ CLOSED | The real defect was the EMBEDDED surface dropping the `ShuttingDown` discriminator the wire already carries (`not_running_with_type`). Now `ClientError::NotRunning { detail: ErrorDetail::with_type(…, "ShuttingDown") }`, and `render.rs`'s `hint` gained a guard arm keyed on it. **M3** revert to `not_running(...)` → embedded test RED. **M4** delete the render arm → render test RED. `hint` is no longer `const` (it reads a runtime discriminator; sole caller is the runtime path). |
| **F4** | ✅ CLOSED | `is_current_owner` now disclaims EVERY shard while armed, because `fence` is not shard-aware and so models "not serving", not "lost one shard". Shard-aware fencing would need `shard_for_workflow`, which lives in `aion-store-haematite` — the wrong dependency direction. **M5** restore the `shard == REFUSED_SHARD &&` conjunct → RED. |
| **F6** | ✅ CLOSED | `delegated` and `visibility_reconciliation_task` back to private. **The compiler is the census** — any sibling reader would have failed to build; `cargo check --all-targets` is clean. |
| **F7** | ✅ CLOSED | The third accepted cost stated where the choice is made: past `close_and_wait`'s poison the gate's DRAIN guarantee is skipped (not a memory hazard; no new admissions either, since `begin_start`/`begin_operation` share the poisoned state). Doc-only. |

## Still open

- **F2 (MAJOR)** — the fourth durable writer. `spawn_completion_task`
  (`nif_activity_retry_dispatch.rs:88`) detaches its `JoinHandle` and appends
  through `record_retry_event` with no epoch check, surviving both `Drop` and
  `Engine::shutdown`. Proper fix: gate `record_retry_event` on
  `runtime.engine_tasks().is_epoch_open()` — `spawn_completion_task` already
  holds the `Arc<RuntimeHandle>`, so it is a plumb of one field into
  `RetryRecorderSeam`. Then correct `engine/api.rs:131-141`'s enumeration.
- **F5 (MINOR)** — the second-error `else` branches in `engine/api.rs:505-520`
  and `runtime/handle.rs:873-891` are executed by no test, because the only
  injection seam (`force_process_exit_drain_failure`) fires `begin_shutdown`,
  always the FIRST fallible step. Needs a second `#[cfg(test)]` seam so two
  steps fail in one shutdown, plus a `LogCapture` assertion naming the second.

## Files touched since amendment 1

`crates/aion/src/runtime/nif_timer_bridge.rs` · `.../nif_timer_bridge_tests.rs` ·
`crates/aion/src/engine/api.rs` · `crates/aion-client/src/transport/embedded.rs` ·
`crates/aion-cli/src/render.rs` · `crates/aion-store/src/testing/refusing_reads.rs`

⚠️ Every one of these is a **new tree** — BO's workspace battery does not
survive them. A full re-battery is required before any commit.

---

# AMENDMENT 3 — 🔴 ALL SEVEN REVIEW-13 FINDINGS CLOSED, EACH MUTATION-PROVEN

Eleven mutations run. Every one used the LITERAL prior text, turned **exactly
its own test** RED and no other, and every file restored **byte-identical by
`shasum`**. `cargo clippy --workspace --all-targets -- -D warnings` **exit 0**.

| # | Mutation | Result |
|---|---|---|
| F1 | M1 delete the post-insert re-read · M2 delete the append-boundary gate | each RED, own test only |
| F2 | M6 delete the `is_epoch_open` guard in `record_retry_event` | RED |
| F3 | M3 revert the embedded arm to `not_running(...)` · M4 delete the render guard arm | each RED |
| F4 | M5 restore the `shard == REFUSED_SHARD &&` conjunct | RED |
| F5 | M10 silence `keep_shutdown_error`'s report · M11 silence `keep`'s report | each RED |
| F6 | (compiler is the census — private fields, `--all-targets` clean) | n/a |
| F7 | (doc-only) | n/a |
| deck | M7 remove the `run` row · M8 remove the `timeout` row | each RC=1 |

## F2 — the fourth durable writer, closed properly

`RetryRecorderSeam` gained `engine_tasks: Arc<EngineTaskRuntime>` — **a field,
not an argument**, so a future record path cannot be written without one. The
compiler then enumerated all six construction sites and all three match sites;
nothing was found by grep. `record_retry_event` reads `is_epoch_open()` **under
the recorder lock**, the last point at which this process can decline to be a
second writer. New `RetryRecordOutcome::EpochClosed`, deliberately distinct from
`Settled` (nothing was recorded by *anyone*), with its own `info!` at each site.
`Drop for Engine`'s enumeration now names **four** writers and says which two
mechanisms stop them.

**Measurement note:** the reviewer said the completion task "survives both
`Drop` and `Engine::shutdown`". True of the task; the *append* is now gated by
`begin_close()`, which both paths call — so the fix reaches it one indirection
away rather than by aborting it.

## F5 — and a correction to the reviewer

The reviewer said the `else` branches "cannot be reached". For `handle.rs` that
is **wrong**: `close_and_join_all` opens with `begin_shutdown()?`, so one armed
flag fails step 1 *and* step 3. What was true — and what I measured before
believing either of us — is that **nothing asserted on it**: deleting the
`tracing::error!` body left all 659 lib tests green. For `api.rs` the reviewer
was right, and reaching it needed a new seam: `ShutdownGate::force_close_failure`
(returns `RegistryPoisoned`, the only error that function can really raise).

## Clippy findings that were my own new code

Both fixed in the code, never suppressed:
- `too_many_lines` on `dispatch_with_retries` — the two EpochClosed arms were
  identical, so they became `stood_down_on_closed_epoch()`. The lint pointed at
  real duplication.
- `panic!` in my new embedded test — replaced with a returned `Err`, and the
  test now returns `Result`.

## Gate state

- `cargo clippy --workspace --all-targets -- -D warnings` → **exit 0**.
- `cargo test -p aion-rs --lib` → **661 passed / 0 failed** (659 + the two new).
- `cargo test --workspace --no-fail-fast` → **IN FLIGHT**, detached via
  `scratchpad/battery-bp.sh`. Log `scratchpad/bp-workspace.log`; completion
  marker `scratchpad/bp-battery.done`; `RC_WS=` is appended as the LAST line.
  🔴 Reconcile WITHIN that one run: `Running` lines vs declared binaries,
  verdict-line count, and the exit code — **a crashed binary emits no verdict
  line**. Only expected red is **#85** (libsql double-close, contention-
  dependent).

## Deck changes this tick (SEPARATE commit lot)

- The install slide (`cargo install aion-cli`, three guards, mutation-killed).
- The AWL worker: new gated example `first_flow_awl.awl`, four Step-5 slides,
  and the "Start the worker" slide now tells way-one readers to skip it.
- **Keyword coverage**: the table explained 8 of the 10 words the examples use.
  `timeout` and `run` added, and `gate_keyword_coverage()` now DERIVES the
  expectation from the examples so it cannot drift again.
- Keywords **grouped by type** at Tom's suggestion: WHAT IT IS · WHAT IT
  PROMISES · WHO DOES THE WORK · WHAT HAPPENS, IN WHAT ORDER.
- Deck is 26 slides; Tom is starting at **slide 7**.

## ON RESUME

1. Read `RC_WS=` from `scratchpad/bp-workspace.log` once `bp-battery.done`
   exists, and reconcile as above.
2. On green: **Review 14** on Opus over the whole lane (reviews 6–13 all
   refused). Give it this marker and review 13's transcript
   (`scratchpad/review-13.jsonl`).
3. Only then commit, in three lots, paths named **explicitly**.
4. Tom is mid-tutorial and may ask anything — he takes priority over the lane.
