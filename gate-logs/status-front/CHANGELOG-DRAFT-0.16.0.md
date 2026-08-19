# DRAFT changelog entry for the 0.16.0 cut — Waffles, 2026-08-16

Status: DRAFT — folds into CHANGELOG.md as the cut commit AFTER the status-truth
landing. Every entry below is derived from the commit chain v0.15.0..main plus the
status-truth branch; claims verified at commit granularity, wording to be checked
against bytes where marked [VERIFY].

## 0.16.0 — 2026-08-16

- **A workflow's displayed status is reconciled from its full recorded history —
  never from a stale cached row.** `POST /workflows/list` now rebuilds each row
  from the run's complete event history before filtering and serving: a finished
  workflow can never be shown Running, and a mid-flight workflow can never be
  shown Failed off a divergent durable visibility row (the stored cache is
  corrected in the same pass). Opaque terminal messages of the form
  `workflow N returned error` are replaced by the first line of the recorded
  structured error detail; full detail stays in event history. Both behaviours
  are pinned by red-first regressions.

- **The fleet document fans out (#240):** parallel legs in isolated worktrees
  with a server-run merge; fan-in merges before it carries. Run 2 went green
  end-to-end on the resumed pipeline.

- **Child runs are visible while they run (#244):** the server derives child
  legs from parent run history and resolves the current run for child
  projection (concurrently and observably); the console folds live child leg
  transcripts into the run page and discovers fanout from live parent events;
  the CLI tails parent and child transcripts together, follows successor leg
  attempts over sockets, reports tail backfill gaps truthfully, replays
  workflow events across the tail-attach window, and scopes replay payloads to
  the selected run. Unknown parent runs are rejected.

- **An agent outcome carries a four-field contract everywhere (#205):** the
  outcome names the conversation that produced it, and the final message is
  carried distinctly from the outcome fields. Worker protocol stubs (Python and
  TypeScript) regenerated to the current contract.

- **A cancelled run tells its workers to stop (#233):** cancellation propagates
  to serving workers at the wire; the last delivery arm (TransportCannotCarry)
  is red-pinned.

- **Sibling agent spawns into a live worktree are refused (#246),** as are
  pending responder collisions on the same worker — both explicit refusals,
  both red-first.

- **A red gate says WHY, in the record (#236).**

## Cut mechanics (not part of the entry)

1. Land status-truth: ff main 0bed60196 → e790ada74, push.
2. Fold this entry into CHANGELOG.md; bump workspace version 0.15.0 → 0.16.0;
   commit to main (the cut commit); tag v0.16.0; push.
3. cargo install from main checkout (default target dir). Do NOT restart the
   server mid-install.
4. Restart server 52844 under the detached posture (detached-relaunch.py
   pattern). Worker 55729 (market-mirror collector arm) is NOT restarted — it
   reconnects on its own backoff; the collector moves on Tom's word about the
   experiment (204-runbook rule stands).
5. Post-swap: server answers on :8080, workflow list shows honest statuses
   (spot-check the two former F2 specimens + one stale-Running candidate),
   version endpoint shows 0.16.0.
6. crates.io publish: NOT part of this cut unless Tom wants it — the running
   server is the deliverable Tom named. [Publish authority delegated; decide at
   cut time.]
