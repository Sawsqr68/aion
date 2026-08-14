# FIRST fleet_dev DISPATCH — prep record (declared-actions, K8 leg 1)

Dispatcher: Vesper. Package: docs repo c7c055a
manifold-click-actions-fleet-20260814.md (Waffles r1). Document: fleet_dev
44447c793 (two-field Reply world, checks clean on 0.15.0, "ok, 10 steps").

**HELD: nothing fires until Tom's word on the leg list (with him since
~13:35Z).** Worktree (branch declared-actions off manifold main ≥ e58973d)
is cut at Waffles' desk after that word.

## Prepared at this seat (2026-08-14 ~13:4xZ)

- `plan_check.sh` / `report_check.sh` / `gate.sh` — the three server-run
  scripts, adapted from the pilot's proven set (direct exit capture, flake
  protocol in gate.sh, missing legs named on stdout by report_check).
  Worktree path is hardcoded to the expected cut
  (`apps/manifold/.worktrees/declared-actions`) — RE-VERIFY against the real
  path at preflight before dispatch.
- `build-input.py` — builds the full 12-field dispatch input from the
  PACKAGE BYTES at build time (brief stays the single source; re-run after
  any package amendment). Draft build verified: 12 keys, legs L1/L2/L3,
  hard walls present in planner + every leg + judge prompt.
- Controls verified TRUE at manifold e58973d before authoring:
  counsel: BADGE_MEMBERS at surface/page/src/declared-face.ts:41;
  dev: package.json name == manifold-surface-page (the pilot's register).
- Worker seats preflighted with `--check` (both EXIT=0): counsel = acp,
  plan+judge advertised, Cancel; dev = norn, InjectMessage+Cancel.
  /opt/homebrew/bin/npx exists (node 26.3.0).

## Remaining preflight on Tom's word (pilot register, in order)

1. Waffles cuts the worktree; confirm path matches the scripts; verify tree
   clean at expected HEAD.
2. npm ci in surface/page (worktrees don't share node_modules — the pilot's
   lesson) with EXIT captured.
3. Baseline-gate the UNTOUCHED tree via gate.sh preflight run (logs to
   preflight/ beside this file, worktree cleaned of them after).
4. Deploy fleet_dev (from workflows/fleet-dev/fleet_dev.awl) + seat the
   counsel worker (`aion worker agent ~/.aion/authoring/fleet_dev.awl
   --task-queue counsel --identity counsel-worker --liminal-address
   127.0.0.1:50061`) — FIRST LIVE ACP SEATING; watch both registration
   sides. Dev worker 74470 already seated serves queue dev.
5. Rebuild input with the real worktree path; `aion start fleet_dev
   --input @<file>`; watch the two controls land before anything real.

## Protocol on wedge/death

#82-class: preserve everything, kill nothing by pattern, report state
before remedy. Live pids: server 56424, dev-worker 74470, assistant-worker
59932, mm-capture 58869 (+ counsel worker once seated, pid recorded here).
