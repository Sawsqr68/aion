# SEAT RESUME — marker AZ, 2026-08-06

Supersedes **AY** for STATE. Latest letter wins. **Nothing is committed.**

---

## Where #7 stands: REVIEW 4 CAME BACK **NOT SAFE TO LAND**. 2 blockers fixed, 3 findings OPEN.

The fourth Opus review ran the three mutation legs itself and confirmed all
three decisive (sha1-verified restore). It also confirmed clean: the epoch-gate
blast radius is complete, `Engine::shutdown` loses no step, `FaultReporter`'s
bound holds, all four god-file splits are byte-identical moves with nothing
lost, and no `#[allow]`/`#[ignore]`/`unwrap` was introduced. **It then found 5
more things.**

### ✅ FIXED THIS SESSION (blockers 1 and 2)

**Blocker 1 — `completion.rs` denied a refusal window that genuinely exists.**
Verified at my hand: `Engine::shutdown` calls `begin_close()` FIRST
(`engine/api.rs:384`), then `ShutdownGate::close_and_wait` (an **unbounded**
condvar wait, `engine/shutdown_gate.rs:52-62`), and only reaches
`process_exits.begin_shutdown()` later (`runtime/handle.rs:586`). Exit callbacks
are admitted throughout. So runs exiting during graceful teardown ARE refused,
record no terminal, and stay `Running`. The old comment claimed the opposite.
Rewritten to name the window, its unboundedness, and why the trade is still
right. `EngineError::EngineTaskEpochClosed`'s doc also corrected — it said "a
successor engine may already be recovering", which is false during this node's
own shutdown and sends an operator hunting a second node that isn't there.

**Blocker 2 — MY F14 SURVEY WAS WRONG, and this is the lesson of the day.**
I wrote that `workflow.now` "advances only in `Replay::step`". Verified false at
my hand: `nif_determinism.rs:215` reads `NifContext::last_recorded_at`, assigned
at `nif_context.rs:118` as `history.last().map(|e| *e.recorded_at())` — **the
tail**. `Replay::now()`/`DeterminismContext` have ZERO production callers. I
surveyed the designed mechanism and missed that production reimplemented it.
🔴 **One truth in two places had drifted, and I checked the copy nobody uses.**

The bullet is now RETRACTED IN PLACE (not deleted) and re-derived against the
real mechanism. The conclusion does survive, for a different reason I verified:
`reopen.rs` appends `WorkflowReopened` (and re-arm `TimerStarted`) at
`Utc::now()` **before** `respawn_and_register`, so a respawned process's first
context always reads a tail later than the terminal it superseded. Plus:
`HistoryCursor` orders on `seq` not `recorded_at`, and the resolver rejects
workflow terminals as history-shape faults. The doc now says the conclusion is
load-bearing on the tail read and MUST be re-derived when #1 is fixed.

### 🔴 STILL OPEN on #7 — do these before committing

3. **`aion-client/src/transport/embedded.rs` `map_engine_error` fails its own
   stated rule.** The doc says the rule for naming an arm is "would the generic
   bucket mislead a caller about what to DO" — and then misreports these, all
   reachable through the embedded surface, all of which `aion-server/src/error.rs`
   already classifies correctly:
   - `StartInputRefused` / `SignalRefused` → currently `server`; must be
     `ClientError::invalid_argument` (server: `declared_contract_wire`, :695-696)
   - `TerminalWriterUnavailable` / `TerminalWriterHeld` / `RunIsRecoverable` /
     `NoResidencyVerdict` → currently `server`; must be `invalid_state`
     (server: `invalid_state_wire`, :640-643 — with a comment saying anything
     else "would reproduce, one layer out, the exact lie this path exists to
     stop telling")
   - `Store(StoreError::NotOwner)` → currently `server`; `ClientError::not_owner`
     exists (server: :755-756)
   All constructors exist (`not_found, already_exists, query_failed,
   unknown_query, unavailable, unauthenticated, namespace_denied,
   invalid_argument, invalid_state, not_owner, server`). Keep the `_` arm; the
   defect is the arms that should be named, not the wildcard.
4. **`completion_retry.rs:296-320` logs a falsehood for one of two causes.**
   The comment names both causes; the message says "no retry was armed; the run
   stays Running" — but when the cause is "already in flight", a retry IS armed.
   `EngineTaskRuntime::arm` returns a bare `bool` (`runtime/engine_tasks.rs:158-217`)
   so the site cannot tell them apart. Same defect class the whole
   `TerminalProgress`/`operator_summary` apparatus exists to kill, on the
   armed/already-armed axis instead of the recorded/not-recorded one.
   **Fix = make `arm` return which it was, then say the true thing.**
5. **F11's reopen guard covers one branch** (`completion.rs:219-228`): it
   abandons quietly only when `progress == Recorded`. If arming failed before
   `Recorded` was established and a reopen lands during the retry window, the
   next attempt appends into a live reopened run. Fails safe (`SequenceConflict`
   from the stale recorder head) but hands the operator THE double-writer
   indicator for a benign reopen — exactly what F11 was written to prevent, on
   the branch F11 didn't cover.
6. *(note only)* `completion.rs:308-317` and `:339-346`:
   `upsert_workflow_visibility` and `reconcile_terminal_registry` run after the
   recorder lock drops, so the gate at `:175` doesn't cover them. Worth a
   sentence, not a blocker.

---

## Gates last measured (before findings 1/2 doc fixes; RE-RUN BEFORE COMMIT)

    cargo clippy --workspace --all-targets -- -D warnings   EXIT 0 (read BARE)
    cargo clippy -p aion-rs --all-targets -- -D warnings    EXIT 0 (after doc fixes)
    cargo test -p aion-client                               61+1+6+2 / 0 failed
    cargo test -p aion-server --lib                         796 / 0 failed
    cargo test -p aion-rs      39 verdict rows, 985 passed / 1 FAILED

🔴 The 1 red is **#85**, not mine — structural attribution, not statistical:
mechanism root-caused in dep source (`LIBSQL-DOUBLE-CLOSE-85.md`), a prior seat
measured this exact target+error at ~37.5%/battery run BEFORE this work existed
(`SEAT-RESUME-2026-08-01-E.md`), sibling test recorded failing identically
(`RECOVERY-LOOP-GUARDS-121.md`), 4/4 green when the target runs alone, and this
change set touches neither libsql, the store, nor the outbox. The reviewer
independently agreed and flagged one honest caveat: finding 1 does move an epoch
close earlier in `Engine::shutdown` — it judged that unrelated (the epoch close
touches no store handle) and so do I, but it is stated rather than hidden.

---

## 🔴 COMMIT CHECKLIST (unchanged, still binding)

Never `git add -A` here. **FOUR UNTRACKED LOAD-BEARING FILES** — committing
without them lands a NON-COMPILING build:

    crates/aion/src/store_faults.rs                 (lib.rs declares it)
    crates/aion/src/lifecycle/completion_retry.rs
    crates/aion/src/engine/builder_assembly.rs
    crates/aion/src/engine/api_workflow_ops.rs

Plus the modified set (derive it fresh from `git status`, don't recall it).
Message via `git commit -F -` with a QUOTED heredoc. Then push.

---

## The other three issues: TWO FULL SPECS BANKED, one agent still out

Tom asked for parallel Opus agents. Three were dispatched READ-ONLY (the
one-mutating-lane law forbids concurrent editors) to produce apply-ready specs.

### #1 `workflow.now()` — SPEC RECEIVED, CONFIRMED, and BIGGER than "a line"

Defect at `nif_determinism.rs:215-220` (above). **Second consumer, same root
cause:** `nif_timer.rs:340-346` `recorded_now` feeds `sleep`/`start_timer`
deadlines, so on recovery replay `fire_at` is recomputed from the tail and
disagrees with the durable `TimerStarted.fire_at` — silently, because
correlation keys on `TimerId` not `fire_at`. AWL is NOT in the blast radius
(`aion-awl` has no `now`; `tests/emitter.rs:253` asserts emitted code never
contains `workflow.now`).

Fix shape: a `DeterminismContext` held once per run on `WorkflowHandle`, seeded
from the recorded `WorkflowStarted`, advanced only at observation seams
(`resolve_command_observed` vs a look-ahead `resolve_command`). Decisive test is
a **differential over the history suffix**: two legs, identical prefix, replay
leg has extra later events; assert the observed `(before, after)` pair matches.
Cannot pass under the tail read (leg B would read the later tail on both reads).
Full spec incl. 5 mutations, 4 pins to correct (`nif_determinism.rs:489-507`
`now_returns_last_recorded_event_timestamp_millis` is the defect AS ITS NAME,
plus `docs/design/aion-nif-bridge/DESIGN.md:49` D3) and 5 husks — see the agent
transcript. **Agent did not compile anything; type-check before trusting.**

### #6 `max_concurrency` — SPEC RECEIVED, and the issue was WRONG in one direction

Exact drop point: `liminal.rs:479`, `spawn_plain_dispatch_executor()` takes NO
argument and hardcodes a single serial drain thread. `connect_advertising` holds
`config` and reads five other fields but never `max_concurrency`.
**Both directions are broken:** plain liminal dispatch = hardcoded 1 (under),
agent-harness dispatch (`spawn_agent_dispatch`, `liminal.rs:882-918`) =
unbounded `std::thread::spawn` (over). gRPC honours it correctly.

🔴 **The serial executor WAS deliberate** — three sites say so, and the
load-bearing sentence is *"it belongs in worker configuration"*. That deferral
was never discharged: configuration already HAS the knob, it is required with no
default, and `aion worker agent` refuses to start without it under a doc heading
"Nothing is invented" — on a verb that serves ONLY over liminal. So the defect
is real and narrow: **a required setting enforced on one transport, discarded on
the other.** Do not "fix" the serial default — there is no default.

**Two things must be ROUTED, not built silently:**
- Bounding the agent path is a live behaviour change for agent workers
  (`AION_WORKER_CONCURRENCY` feeds straight in). If deferred, `config.rs:133`'s
  doc must be amended to say *plain* activities.
- **A THIRD defect found in passing:** `ConnectedWorkerRegistry::select_worker`
  (`aion-server/src/worker/registry.rs:919-942`) resolves `min_by_key(worker.id)`
  — there is NO load balancing within a pool. Issue #6's "server does not fan a
  fork across idle workers" is live and unaddressed.

Also: the incumbent head-of-line pin `wedge_shape_regression_0756ecd5` is green
under total blocking AND under perfect fan-out (its only throughput-sensitive
quantity is a 400 s deadline; serial finishes in ~95 s). Replacement pin
specified — peak simultaneous occupancy across two **node-distinct** workers each
capped at 1, event-counted not sampled.

### #5 AWL `sum` — AGENT STILL RUNNING at compaction.

### #9 — NOT A BUILD. Reply only, authorised, pointing at our own #90 revert
(`64aa16c99`). **Must go past Waffles before it posts** (outward-facing).

---

## Tom comms

06:04Z "how are we going?" — answered.
06:32Z harder: how much longer, how close to the bottom, *"you told me this was
all good to go a month ago"*, *"we really need to have stuff to show tomorrow
morning."* Answered plainly, premise not argued: what shipped did work, but some
fixes only fixed the front door and my own tests walked in the front door too.
Offered the tutorial walk as the showable thing today + a dry-run crib sheet.
06:38Z asked for parallel ultracode/Opus agents. Did it, and told him WHY only
the hunting parallelises (one editor per repo).
**He has not replied to any of the three. Poll before assuming.**

🔴 **OWED TO TOM:** dry run of the #146 tutorial + a one-page crib sheet (every
command in order, what he should see). Tutorial lives at
`docs/authoring/first-workflow.html`, `docs/authoring/tutorial-examples/*.awl`,
`docs/GETTING-STARTED.md`. Self-run already proven (`d78a38308`). **His server
and worker pids must not be touched — derive the sheet from the doc + the proven
self-run.**

---

## Standing constraints (carry verbatim)

NO WORKTREES in aion · ONE mutating lane · unmarked dirty = STOP · never
`git add -A` · never `git stash -u` · NO `git gc`/`prune`/`--prune` until the
estate deletion hold lifts · **DELETE NOTHING** · no `--force`/`--mirror`/
`--delete` on any push · ⛔ never bare `cargo clean`, no custom
`CARGO_TARGET_DIR`, no launchd · 🔴 **Tom's pids 45425 (server) / 48399 (worker)
— DO NOT TOUCH** (untouched all session) · worker rebuilds FROZEN estate-wide
until the #139 live cut · NO-SWEEP untracked dirs: `.aion-accepted-binary-20260731/`
(SOLE copy of Tom's accepted build), `.aion-jjuabk-snapshot-20260731/`,
`.aion-data-preswap-20260804/` · beamr JIT blocker with Artemis · do not edit
`CLAUDE.md` · never mention prospekt to Tom · **NEVER publish artifacts for Tom**
— deliverables are local files · CLAUDE.md mandates an **Opus** sub-agent review
before work lands · `<<autonomous-loop-dynamic>>` wakeup must be re-armed every
tick (armed 16:41, fires ~17:06).

Box: AEST (+1000) — `-t "%Y-%m-%dT%H:%M:%S%z"`. zsh: no `PIPESTATUS` (lowercase
`$pipestatus`, 1-indexed), `status` is READ-ONLY so name it `rc`, no
word-splitting on unquoted expansion. BSD `sed` has no `\s` — use
`/usr/bin/python3` (NOT `/Users/tom/.local/bin/python3`). Foreground `sleep`
blocked. Estate diff driver emits no ± prefixes —
`git -c diff.external= diff --no-ext-diff`.

🔴 **Never cite an exit code off a pipeline tail.** Run the gate bare, read `$?`.
🔴 **Verify the agent, not just the claim.** Two of today's three specs were
right; my own F14 survey was wrong. Read the line before you believe either.
