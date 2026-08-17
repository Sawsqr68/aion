# Resume-adoption remainder — follow-on lane brief (r1, DISPATCH-READY)

- **Status**: r1, dispatch-ready. r0 authored at Vesper's seat on Waffles'
  ruling 07:20Z 2026-08-17; **approved with three amendments 09:51Z (all
  folded below: base named, leg-2 binary-provenance wall, battery
  modernized), dispatch at Vesper's discretion — no further pass needed.**
- **Base**: fresh worktree from landed aion main — the e05d3e3b4 tree
  (12 commits landed by ff, battery run 4 single-run green 5085/5085;
  record commit 36a5ce8ba). Board rows aion#37–#41 are these five legs.
  NOTE for leg authors: the landed base's e2e expectations already carry
  the honest exhausted-wording (`exhausted: spent N of M attempts; last
  refusal: …`) — activity_retry_e2e and advisory_class_e2e were fixed at
  landing (e05d3e3b4). Do not re-derive that fight.
- **Execution mode**: PLAIN BUILDER — **no aion harness anywhere in this
  lane's execution** (Tom's directive 07:15Z: no more development through
  aion). The worktree is the deliverable; collection is manual through
  Waffles' gates; the delivery report is BOTH a REPORT.md file at the
  worktree root AND the closing reply (charter fix, now standard — the first
  lane's reply died with the harness ledger and had to be recovered from the
  norn session).
- **Opening state** (read these before any code):
  1. `gate-logs/resume-adoption/DELIVERY-REPORT-RECOVERED.md` — the first
     builder's honest self-refusal, incl. its explicit-non-claim section and
     disclosed non-goals. Its leg-disposition table is this lane's work list.
  2. `gate-logs/resume-adoption/BRIEF-DRAFT.md` — the approved r1 charter
     this lane completes; all its isolation walls and red-first rules carry
     forward unchanged.
  3. The three review findings (Vesper, ratified 07:20Z), restated below as
     legs 4–5 and the evidence rule.

## What the first lane proved, so this lane doesn't re-prove it

- Exhaustion records name exhaustion (real behavioral red 836ef5a88 →
  b73246ddc).
- Spawn-gate self-occupancy adopts while foreign occupancy refuses, at the
  Norn-harness boundary (real behavioral red 74c249d8c → 85755d737; paired
  fixture keeps #246's red red).
- Same-worker redeliveries single-flight by execution (unit level:
  agent_flights + liminal wiring).
- Paused histories with dangling attempts enter the recovery sweep.
- The 400ms false-error deadline is gone; resume waits the fence and errors
  only if the recovered process dies.

## The five legs (Waffles' board rows, verbatim scope)

**Leg 1 — cross-process re-attach arm.** Worker killed under a live agent;
replacement worker receives the re-dispatch, spawn gate finds the surviving
agent's own (workflow_id, activity_id), and the Norn invocation resumes the
session instead of spawning. Session-ID continuity (pre/post capture) proves
it; assert exactly one session exists after adoption. The first builder's
six-step fixture sketch (report §"Explicit cross-process non-claim") is the
acceptance shape. **If this fixture is genuinely difficult, that difficulty
is a FINDING to be written up (what wall, where, why), not a delay** — the
freeze-window explanation for the first lane's missing e2es is a hypothesis,
and this lane must not assume the work is merely unfinished rather than hard.

**Leg 2 — issue #36 verbatim e2e.** Live builder → pause → server restart →
resume → run completes with ONE agent session, on an isolated in-worktree
server (own AION_HOME, own ports, recorded). This is the charter's original
first acceptance leg and it has never run. **Binary-provenance WALL
(amendment 2, 09:51Z): the e2e's server binary MUST be built from the
worktree tree under test at the tested commit — never `~/.cargo/bin/aion`,
which is a different tree by definition. A leg-2 green against the installed
binary proves nothing about this lane's code. REPORT.md records the binary's
path and the commit it was built from.**

**Leg 3 — supersession-absence proof + adoption event.** History assertion:
while the holder lives, NO `superseded:server-death` record is written for
its attempt on either the Paused or Running path; an explicit adoption event
is recorded instead (append through the run's one Recorder — single-writer
law; never EventStore::append). The first lane's startup change only
enumerates Paused histories into recovery; nothing yet proves the record
stays truthful.

**Leg 4 — Running-path truth fix.** The first lane left Running recovery
re-dispatching, with the worker-side join/adopt absorbing the consequence.
That closes the operational hazard on the observed face but leaves the
engine writing a false record path. Recovery must offer dangling
remote-agent attempts to the adoption path for Running exactly as for
Paused, and leg 3's assertion must pass on both. (Landing-record precision,
ratified: observed-incident face — same live worker — closed WITH evidence
by single-flight join; worker-replacement face closed by construction only,
evidence owed here.)

**Leg 5 — readiness-pin hardening.** The current pin
(`recovered_pending_await_has_no_false_error_deadline`) asserts a constant
helper and sits on no mutation path — a reintroduced literal deadline
bypassing `pending_await_deadline()` survives it. Replace with a test that
fails against the literal prior defect (a deadline that expires while the
recovered process is alive and progressing toward its await), per
the mutation-must-sit-on-the-test's-path law. A decorative test is worse
than none.

## Rules carried forward (unchanged from the r1 charter)

- Red-first, every leg: the red is committed failing BEFORE the fix, red
  output quoted in the commit message. **Reds must be behavioral where the
  defect face permits** — a compile-error red (E0425/E0432) proves the test
  didn't exist, not that the defect was seen; three of the first lane's five
  reds were compile-error reds and are named as such in the record.
- Isolation walls absolute: never the live estate server, ~/.aion,
  aion-data/, live workers, or any sibling worktree with a live agent.
  All e2e legs on an isolated server inside the worktree.
- No lint bypasses, no #[allow], no #[ignore], no test skips.
- Full workspace battery at the FINAL tree — the first lane never ran it
  post-commit. **Shape modernized (amendment 3, 09:51Z, both halves from
  this morning's own incidents):** (a) there is NO fmt gate — Tom struck it
  (his word, 2026-08-17); builders `cargo fmt --all` (autoformat) before
  committing and never format-check. (b) The battery is the extraction
  pipeline: `cargo check` → `cargo clippy -- -D warnings` → `cargo nextest
  run --workspace`, all `--keep-going` / `--no-fail-fast`, JSON
  message-format jq-reduced to one finding per line. Fail-fast hid two red
  files entirely in run 3's predecessor — the battery runs EVERYTHING and
  reports everything, never stopping at the first red binary. (Clippy ×2
  workspace AND per-crate still applies — the preserve_order resolution
  differs between them.)
- Explicit non-claim in REPORT.md is the only permitted fallback for any
  unproven leg; silence is not an option.
- **No allocation instrumentation without a disk budget** (Waffles' ruling
  07:49Z, from a proven incident, not a hypothetical): builders never run
  under MallocStackLogging or any malloc/allocation instrumentation unless
  the charter carries an explicit disk-budget line for its output — the
  2026-08-17 estate server's malloc log wrote ~3GiB/min to an UNLINKED file
  (invisible to du, counted by the filesystem) and bottomed the box at
  924MB, killing an unrelated battery leg as an ENOSPC non-run. Any
  instrumented run states where the log lands, its cap, and who reaps it.

## Sequencing

DONE: battery green (run 4, 5085/5085 single-run) → lane 1 landed
(168baa5f4..e05d3e3b4, pushed) → Waffles' review pass (approved w/ three
amendments, 09:51Z, folded above). REMAINING: dispatch as a plain builder on
a fresh worktree from the landed main, at Vesper's discretion.
