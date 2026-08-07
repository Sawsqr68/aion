# SEAT RESUME BW — 2026-08-07 — supersedes BV

🔴 **THIS IS THE CURRENT AUTHORITY.** Latest letter wins. Read BV for §1/§2/§4-6
detail that still stands; this marker carries what changed after it.

## 🔴🔴 TOP PRIORITY ON RESUME — TOM IS LIVE WITH AN AUDIENCE (#146)

At 01:58Z Tom DM'd: he walked the tutorial through **with people watching** and
hit, on the deploy-and-run step:

> `No connected worker serves task queue `gate`. Scaffold and run this worker
> from Workers & Actions, then retry start.`

**I replied** (dm:c9255b2a-5731-4d17-8124-e3bfa2224186) telling him to follow the
on-screen remedy to keep the demo moving, that the contradiction is real and not
his mistake, and that I would come back with the actual cause. **I OWE HIM THAT
ANSWER.** Do this before anything else.

### What I verified before answering
- `docs/authoring/tutorial-examples/repo_gate.awl` declares `worker gate` with
  four actions whose bodies are `run "..."` shell commands (clone / head /
  checks / tests).
- The deck states TWICE that no worker process is needed — `build-tutorial.py`
  line ~758 (“server — no code, no worker process, nothing else to install.”)
  and line ~1007 (“command itself — there is no worker process to write or …”).
- So the refusal asks for exactly the thing the tutorial promises is unnecessary.
  **Genuine contradiction.**

### What I did NOT verify — the open question
Why the server demanded a connected worker instead of running the declared
commands itself. **Two candidates, do not guess between them:**
1. Tom's running server binary predates the server-runs-declared-commands
   feature (task #60, “AWL declares commands, server runs them, no Rust crate”).
   → tutorial is right about main, wrong about his machine; it must state the
   minimum build.
2. A live defect in queue matching / declared-body admission.
   → fix is mine, tutorial stays as written.

Start by identifying WHICH BINARY his server is running vs when #60 landed
(`git log -S` dates a claim in two commands). Tom's tutorial server is **pid
50822, `~/my-first-flow`, :8099 — HIS, do not touch/restart/kill.** Same for
server 45425 / worker 48399.

---

## §A — B3 CLOSED, MUTATION-PROVEN (the last Review 17 blocker)

Verified at my own hand, then a read-only Opus enumeration derived the complete
map from the NIF registration table (a closed set whose size is asserted).

**The reviewer was right that a fifth writer exists, and BOTH of us were partly
wrong about what mattered:**
- The `sleep` ordering the reviewer flagged (durable timer row written before
  `arm_timer` refuses) is **CORRECT BY DESIGN** — an unarmed durable row is
  exactly what recovery re-arms. Not a defect.
- The real defect, which the reviewer did NOT find: **`continue_as_new` records
  `WorkflowContinuedAsNew` — a TERMINAL — through an ungated funnel.** It was
  the ONE terminal a live workflow process could still write after `Engine::drop`.
- 🔴 **The asymmetry is the whole thing:** the successor run that terminal
  obliges was ALREADY epoch-refused at `completion::start_continuation_replacement`.
  So the two halves of one transition disagreed — terminal landed, successor did
  not — leaving a run terminal-`ContinuedAsNew` with no continuation, deadline
  retired, process killed. A severed chain, repairable only by a later engine's
  startup sweep and never if the engine was embedded and released mid-process.

**Fix:** `record_continuation` in `runtime/nif_continue_as_new.rs` — lifted out
of the NIF closure so it is testable without a beamr process; refuses on
`engine_tasks.is_epoch_open()` immediately before the first durable write.
`runtime.runtime.engine_tasks()` was already bound three lines away — zero
plumbing. New `DurabilityError::EngineTaskEpochClosed` (deliberately NOT
`EngineError::EngineTaskEpochClosed`, which carries a documented
single-construction-site invariant `aion-client` depends on).

**Gating cost nothing** that was not already lost: the successor is refused
either way; refusing converts a severed chain into an `{error,_}` and an unmoved
history.

**ONE check, not one per write — and that DIFFERS from the deadline gate
deliberately** (documented at the site): once the terminal lands, the deadline
retirement is no longer optional, so nothing between the two writes may abandon.

**Mutation:** deleted the gate (the literal pre-fix state; script asserts the
literal was found or exits NON-RUN). → **exactly one RED**, the new test; the
sibling stayed green, proving orthogonality. Restored byte-identical:
`nif_continue_as_new.rs` = `10781cac616a3e420a75d69e4c3b43166a809f72a61c2f9c15c964517814f940`
(`shasum -c` OK). 2/2 green after restore. Artefacts: `can-restore-b3.rs`,
`can-b3.sha`, `mutate-b3.py` in scratchpad.

**Classification decided, not defaulted:** `completion_retry.rs` — the new
variant is NOT transient (`begin_close` is a one-way latch, so no number of
attempts reopens it) and gets its own discriminant.

### The `Drop` enumeration is rewritten (engine/api.rs)
Now: four BACKGROUND writers + a fifth FOREGROUND one (the workflow process).
**13 of the 24 registered NIFs perform durable writes** — named individually.
🔴 The enumeration agent's own summary said “11 of 24”, transposing its table;
I counted from the table and from the registration list and it is **13 write /
11 read-only**. Do not re-propagate 11.

Also stated in the doc, all verified:
- `WriteToken` **fences nothing** — zero-sized, public `recorder()` ctor, no
  engine/epoch/lease identity.
- `SequenceConflict` catches only the LOSER of a head race, and a released
  engine is structurally positioned to WIN (its Recorder is already at head).
- **No flag distinguishes “released” from “shutting down”** — `begin_close` sets
  one bit for both `Drop` and `shutdown`. So a gate on that bit at any
  workflow-process write path ALSO fires during ordinary graceful shutdown, in
  the unbounded span between `api.rs` `begin_close()` and `runtime.shutdown()`,
  where the failure is `{error,_}` inside live workflow code. The terminal was
  worth that trade; ordinary progress writes are NOT. **Do not add one of those
  gates without adding a latch that means “released”.**

## §B — N1 / N3 / N4 CLOSED (see BV §3 table, all now ✅)

- **N1** `shutdown_timer_wheel` doc named a shard-relinquishment caller that has
  never existed AND omitted `Drop`, the caller that makes the gate load-bearing.
  Verified: `Engine::shutdown` runs `runtime.shutdown()` (stops the scheduler) at
  `api.rs:584` BEFORE the wheel teardown at `:594`, so a drain would suffice
  there; `Drop` leaves the scheduler alive.
- **N3** `WorkflowDeadlineHandler::new` was reachable from OUTSIDE the crate
  (`pub mod lifecycle` → `pub mod deadline` → `pub fn new`), so the registration
  seam's “one place the flag can come from” was false at the public boundary.
  Now `pub(crate)`; workspace check passing is the PROOF nothing external used it.
- **N4** both tests end `runtime.shutdown()?`.

## §C — WHERE THE GATES STAND (INCOMPLETE — RESUME HERE AFTER TOM)

`cargo fmt --all` run. `cargo clippy --workspace --all-targets -- -D warnings`
**RED with 4 sites, NOT YET FIXED:**
1. `lifecycle/completion_retry.rs:865` — my new match arm (likely
   `match_same_arms`; the arm bodies are all `false`).
2. `runtime/nif_continue_as_new_tests.rs:121` — `expect_used` (`.expect_err`).
3. `runtime/nif_continue_as_new_tests.rs:178` — same.
4. `runtime/nif_timer_bridge_tests.rs:569` — `doc_markdown`, `WorkflowIntent`
   needs backticks. **PRE-EXISTING in the uncommitted B2 work** — the recorded
   clippy-green predates B1/B2, which is exactly why gates re-run on a new tree.

Fix all four WITHOUT bypasses (no `#[allow]`), then:
- full battery `cargo test --workspace --no-fail-fast`, reconcile with
  `scratchpad/reconcile-bu.py` (**ZERO-VERDICT TARGETS: NONE** is the
  load-bearing assertion; a SIGSEGV'd binary drops its verdict line entirely)
- **Review 18** (Opus, read-only). Reviews 6–17 have ALL refused.
- commit + push per the default I stated to Tom (finish, re-run, push).
  He has NOT said “push it”; no override has arrived.

## §D — STANDING CONSTRAINTS

Unchanged — **BT §7** in full, plus BV §6. No worktrees, one mutating lane,
never `git add -A`, no `git gc`/prune, delete nothing, Tom's pids untouched,
worker rebuilds frozen, don't edit CLAUDE.md, `-- crates docs` scoping,
commit messages via `git commit -F -`.
