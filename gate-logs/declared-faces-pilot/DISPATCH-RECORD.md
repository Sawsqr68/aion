# PILOT DISPATCH RECORD — declared faces (K6 leg 1) — 2026-08-14

THE FIRST REAL AION-WORKFLOW DEV LEG (Tom's week-goal milestone).
Dispatcher: Vesper. Brief author + re-gate + landing: Waffles.

## The run
- **workflow_id 766e0bfe-d8b4-48ba-b1b5-afd2f8991cca / run_id 30a7f12f-ede8-4e5e-804a-e07d32b86f90**
- Started 2026-08-14 ~12:15Z via `aion start norn_dev --input @scratchpad/pilot-input.json`
  (deployed norn_dev c63f0a7a…, queue `dev`, dev-worker pid 74470, server pid 56424 on 0.15.0).
- Control dispatch RECEIVED by worker 12:15:23Z (ordinal 0, attempt 1).
- Pipeline: control (known-answer: surface/page package name = manifold-surface-page)
  → build (brief, 90m timeout, retry 3) → gates (server-run zsh script).

## The inputs
- repo = /Users/tom/Developer/ablative/apps/manifold/.worktrees/declared-faces
  (branch declared-faces, HEAD f1e7ba0, verified clean pre-dispatch; third
  worktree, disjoint from Waffles' two builder lanes; ALL landings at his desk).
- brief = docs repo d40cece manifold-declared-faces-brief-20260814.md, verbatim,
  with dispatcher preamble restating the hard walls (ports/paths untouchable,
  no crates/ changes → STOP-REPORT, worktree only, never land/merge/push/deploy).
- gate_script = gate-logs/declared-faces-pilot/gate.sh (this dir).

## Preflight (all at my hand before dispatch)
1. Deployed norn_dev source read off /awl/deployed (byte-saved ~/.aion/norn_dev.awl);
   harness: norn, concurrency 1, cwd from repo, args --account default.
2. Worktree had NO node_modules (worktrees don't share them) — `npm ci`
   surface/page EXIT=0 (8 packages). Without this the build leg was doomed.
3. Gate script preflight on the UNTOUCHED tree: first pass build=2 (missing
   deps — caught by the instrument), after npm ci: build=0 loc=0 dist=0,
   composition 1/630 red ("editor takes the focus" — the known load-shaped
   flake class), solo re-run 630/630 EXIT=0. Baseline GREEN. Logs in
   preflight/ beside this file; worktree cleaned of my preflight logs.
4. Flake protocol (Waffles' standing rule) BUILT INTO gate.sh: composition
   red → one solo re-run, both logs kept; real red fails twice.

## Protocol on wedge/death
#82-class evidence protocol: preserve everything, kill nothing by pattern,
report state before remedy. Live pids: server 56424, dev-worker 74470,
assistant-worker 59932, mm-capture 58869 — kill only recorded pids with cause.

## On completion
Send Waffles the outcome (text + stop_reason — pre-#205 two-field on this
binary) → his re-gate on the final worktree tree → his landing, sequenced
with his other two lanes.
