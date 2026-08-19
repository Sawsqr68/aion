# Status truth — what the operator sees is what the history says

STATUS: DISPATCHED ON TOM'S WORD (via Waffles 04:48Z/04:49Z 2026-08-16).
Priority front: displaces everything. Waffles owns adjudication and landing;
trimmed authoring ceremony is his responsibility on the record.

## The complaint (the operator's own words, relayed)

Every workflow Tom looks at comes up Failed. He has been asking for days.
The workflow list is the product's face; right now it reads as a wall of
red that neither distinguishes honest failures from infrastructure
casualties nor tells the truth about runs the server lost track of.

## Census evidence already in hand (Vesper, 04:5xZ, live server, banked)

Raw: `gate-logs/status-front/census-raw.json` in the MAIN aion tree
(read it — it is the map of the territory). 142 rows in namespace
`default`: 80 Completed, 56 Failed, 5 Cancelled, 1 Running.

Three defect faces, all with named specimens:

- **F1 — Running shown over dead runs.** Before today's restart, four
  `mm_capture_run` workflows displayed Running for DAYS while their
  histories were dead — two of them with `WorkflowFailed` recorded
  underneath (record: `gate-logs/262-dig/orphan-worker-disposition.md`).
  The projection did not re-derive. (#174-adjacent face.)
- **F2 — Failed shown with NO terminal event in history.** After the
  restart, workflows `e4ad76e8-...` and `70f3b331-...` (mm_capture_run)
  display **Failed** while their histories end at `ActivityScheduled` /
  `ActivityCompleted` — no `WorkflowFailed`, no terminal event at all.
  Something wrote Failed without going through recorded history.
  Load-bearing invariant 4 (CLAUDE.md): status is a PROJECTION of event
  history, never a stored mutable field. One of F1/F2 (or both) violates
  it in effect.
- **F3 — opaque failure reasons.** 53 of the 56 Failed rows carry only
  `"workflow N returned error"` — an internal counter naming nothing
  (#151 verified this class; the real error text and ids are in hand in
  the same response's details payload and unused). Even honest
  fail-loud runs (norn_dev gate reds) are indistinguishable from
  crashes. The other 3 carry raw beamr stacktraces — the opposite
  failure of legibility.

## Requirements

### R1 — In-run census (the run's first artifact)

From inside this run, against the LIVE server at `http://localhost:8080`
(READ-ONLY: `POST /workflows/list` and `POST /workflows/history` with
`{"namespace":"default", ...}` only — you must NOT restart, kill, deploy
to, signal, cancel, or otherwise mutate the live server or its runs):

produce `<worktree>/gate-logs/status-front-run/census.json` and
`CENSUS.md` covering EVERY row: workflow_id, type, displayed status,
status derived from the history's own events, classification
(consistent / F1 / F2), and failure_reason class (opaque-counter /
stacktrace / real-message). The two F2 specimens must appear by id.
This is evidence for the fix AND the demonstration Tom watches.

### R2 — Mechanism at the bytes (MECHANISM.md)

With file:line evidence in this worktree's sources, answer:
(a) Where does the list summary's `status` come from — projection at
read, visibility store, boot reconciliation? Name the code path.
(b) What wrote Failed for the F2 specimens without a terminal event in
history? Find the writer.
(c) Why did Running persist over dead runs for days (F1) — what should
have re-derived it and why didn't it?
(d) Does any stored mutable status field exist anywhere in the chain
(engine, server, store) — the invariant-4 audit.
No guessing: every claim carries file:line or a recorded observation.

### R3 — The fix, red-first

Status served by `/workflows/list` (and `/workflows/describe` if it
shares the path) must be derived from recorded history for every
workflow in every state, including after crash-recovery:

- (a) A run whose history holds a terminal event NEVER displays Running.
  Pin F1 with a failing test first (construct the stale state the way
  the wild produced it — dead run, terminal event in history, stale
  summary — then assert the served status).
- (b) A run displays a terminal status ONLY if its history holds the
  matching terminal event. If recovery concludes a run is dead/
  unresumable, it RECORDS the terminal event through that run's single
  writer (the Recorder path — never a bare EventStore::append, never a
  side-channel status write), and only then projects Failed. Pin F2
  red-first from the literal wild state (history ending mid-flight).
- (c) If a visibility cache/summary store exists, it reconciles from
  history and cannot diverge silently: a stale or divergent cache entry
  is corrected (at read or at reconciliation), pinned by a test that
  plants a divergent entry and asserts correction.

No new stored status field. No compat shim. No `#[allow]`.

### R4 — Honest failure reason at the list surface

The Failed row's `failure_reason` carries the recorded terminal error —
the `WorkflowFailed` event's actual message (e.g. the gate activity's
`the declared command `zsh` exited 1`) — never the bare
`workflow N returned error` counter string. Where the recorded error is
a multi-hundred-line stacktrace, the reason is its first line (the
error proper) with the full text still available from history. Pin with
a test at the server list layer: a workflow failed by a known activity
error lists that error's text. (#151's finding co-lands at this
surface; do not build a separate CLI leg — the CLI reads the same
field.)

### R5 — Verification floor

The supplied gate script (all legs EXIT=0 at your final HEAD), red-first
commit ordering visible in the branch (red commit precedes fix commit
per face), and an honest delivery report: what is complete, what is
not, no claims beyond the evidence.

## Working rules

- Worktree: you are in `.worktrees/status-truth` on branch
  `status-truth`, base `0bed60196` (aion main). Commit here only.
- Conventional commits; each red pin its own commit before its fix.
- The live server on :8080 is production — READ-ONLY as stated in R1.
  Your tests construct their own servers/stores; they never touch :8080.
- Delivery report: write
  `<worktree>/gate-logs/status-front-run/DELIVERY-REPORT.md` — final
  HEAD, per-requirement status, gaps named plainly.
