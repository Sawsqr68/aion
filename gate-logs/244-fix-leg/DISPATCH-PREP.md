# #244 fix leg — dispatch record

Commissioned by Waffles 2026-08-15 18:08:46Z on his DO-NOT-LAND verdict:
Opus landing review (6 blockers, 8 IMPORTANT, 11 lower; B1+B6 verified at
bytes by his own hand). His re-gate + fresh Opus pass rule the landing.

## Fired

- 2026-08-15 18:11:38Z: `aion start norn_dev --input-file input.json
  --name "244-fix-leg"` →
  workflow e0625271-f3a4-44ad-b5a7-512aa929c9fa
  run c72b5950-4b65-4101-98b7-b70e5b317470 (start-response.json).
- Input verified verbatim in the run's own WorkflowStarted event
  (all five fields byte-equal to staged input.json).
- Watcher: bg0ve5h3o, 5-min poll → run-watch.log, exits on terminal.

## Preflight record

- Review artifact verified at bytes before staging: 18,609 bytes at
  .worktrees/244-child-visibility/gate-logs/244-child-visibility/
  OPUS-REVIEW-20260816.md, carries "VERDICT: DO-NOT-LAND" and B1..B6.
  build-input.py refuses to assemble if those sections vanish.
- Worktree: .worktrees/244-child-visibility, branch 244-child-visibility,
  tip b0214c0ef, clean (only the untracked review dir), 11G warm target
  kept deliberately by Waffles for this rebuild.
- Control: workspace.package.version = "0.15.0" read at the worktree's
  own Cargo.toml bytes (not assumed from the 0.15.0 cut).
- Brief: the review VERBATIM (build-input.py reads the artifact at
  assembly time — no paraphrase) + builder charter: every finding
  mandatory (B1-B6, I1-I8, all lower + mod.rs doc omission); red-first
  for each pinnable blocker with red output quoted in commits; history
  append-only (scope/traceability notes discharged via NEW commit
  bodies, never rewrite); mpsc-256 handled under the no-invented-
  defaults law; no lint bypasses; never push/merge; live estate
  untouchable.
- Gate (gate-logs/244-fix-leg/gate.sh, #247-proven shape): js-deps ×2,
  fmt, clippy workspace AND per-crate (both resolutions required),
  focused suites aion-cli / aion-server / aion-client (the three crates
  the slice touches), console check/typecheck/test, full standing
  battery. Every leg's exit captured to its own log, no pipelines.

## Concurrency context at fire

- K21 run 12db1121 mid-gates (server-run, its own worktree); #247 run
  542cee1a mid-step on queue dev — the 244 leg queues behind it if the
  dev worker is single-lane; a queued run is visible on the console,
  which is the standing order's point.
- Disk 43Gi free at fire; the 244 rebuild is incremental on the warm
  11G target. #246's ~61G battery (Waffles' hand) holds until K21/#247
  terminals free their targets — his sequencing, unchanged by this
  dispatch.

## Known register facts carried

- ACP/norn narration can FALSE-FLAG exact-token judges (#241): a
  judge_flagged terminal with a passing verdict text is READ, not
  re-run.
- Non-zero declared-command exit = terminal activity error (fail-loud).
- ENOSPC-before-code applies to any I/O-shaped red in the 17:3x window
  (ratified) — and to any future window a disk event is on record for.
