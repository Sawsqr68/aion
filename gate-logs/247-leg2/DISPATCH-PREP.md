# #247 leg 2 — dispatch record

Fired on Waffles' queue word (22:37Z, the 244-landing message): "#247
rebase leg (target now e8c1b28bf)". Shape per the banked terminal
adjudication: rebase onto current aion main + discharge the four
report-named gaps + quiet re-gate.

## Pre-dispatch findings at my hand (2026-08-16, Vesper)

- WORKTREE WAS DIRTY, adjudication had said clean: 3 modified files, all
  sdks/python/aion-worker proto stubs. Identified before touching:
  battery-regenerated output (the committed stubs at base 7109c1f6d are
  stale against worker.proto — board #252's exact subject; the leg-1
  battery's python leg regenerated them). Not slice work, reproducible —
  discarded via `git checkout -- sdks/python`, tree CLEAN at ca9360f2f.
- Leg-1 delivery report extracted from durable history (seq 11 of run
  542cee1a) to gate-logs/247-transcript-spool/DEV-DELIVERY-REPORT.md —
  it had never been written to a file. The four gaps (report §"Ratified
  requirements not satisfied"): (1) server-authored worker-death gap
  publisher absent; (2) red A/B are FakeSink unit fixtures, not the
  chartered live outage e2e (#246 rig, kill+restart, re-registration,
  adopted-survivor replay under re-armed generation token); (3)
  malformed-entry recording depends on a later live event; (4)
  disk-exhaustion diagnostic discarded if transport stays down.

## Fired

- `aion start norn_dev --input-file input.json --name "247-leg2"` →
  workflow 221b52c3-9e1f-41c0-9871-d892b2bf2b13
  run 8d9b31ef-22c9-4658-ab48-c1c48a95e445 (start-response.json).
- Input verified verbatim in WorkflowStarted (5 fields byte-equal,
  payload_limit_bytes:0, no extras).
- Watcher: bfxd5mbon, 5-min poll → run-watch.log, exits on terminal.

## Charter shape

- Brief = original BRIEF.md verbatim + leg-1 report verbatim + charter.
- PHASE 1: rebase 29863a6ca/6fb7d909a/ca9360f2f from base 7109c1f6d
  onto e8c1b28bf — explicitly authorized for these three never-pushed,
  never-reviewed-at-sha commits only (the ruled cure for the two
  base-inherited reds: fleet_dev Reply-shape corpus + closure
  freshness). Post-rebase suite run recorded before phase 2.
- PHASE 2: the four gaps, red-first where pinnable.
- Control 0.15.0 (unchanged through e8c1b28bf).
- Gate: 247's full 8-leg shape (js×2, fmt, clippy×2, aion-worker,
  aion-server, battery) with the HEAD-stamp run_leg (delta-review
  follow-up) → gate-logs/247-leg2/.

## Context at fire

- aion main e8c1b28bf (244 landed 22:37Z, ff, pushed). Disk 166Gi after
  the 244-worktree reclaim (89G, lsof-gated, artifacts salvaged first).
- Waffles' lanes: K21 fix leg at its builder; his re-gate + walk follow
  its word; #246 sequencing his.
