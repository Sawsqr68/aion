# #244 fix leg 2 — dispatch record

Commissioned by Waffles' pass-2 verdict 2026-08-15 21:30:42Z (his DM):
DO-NOT-LAND, narrow — everything from the B/I review genuinely discharged;
two new findings block: NEW-B1 (blocker, introduced by the B4 cure) and
NEW-2 (small, B5's own surface). Ruling artifact:
gate-logs/244-fix-leg/OPUS-REVIEW-2-20260816.md (read in full at my seat).

## My independent verification before dispatch (2026-08-16, Vesper)

- NEW-B1 at the bytes: `grep -n selected_run_seen live.rs` → exactly 3
  hits (112, 131, 132), all inside the WorkflowStarted arm; lines 145-168
  (parent child-discovery + observe_attempt) run for every replayed event
  with no generation gate. Confirmed.
- NEW-2 at the bytes: tail.rs:360 `let first_store_seq = first_store_seq?;`
  returns None when events is empty regardless of head_seq. Confirmed.
- Control at fresh bytes: workspace version "0.15.0" at the worktree's own
  Cargo.toml; tip 0328e67a2; worktree clean (untracked review dir only).

## Fired

- 2026-08-15 21:32:54Z (recorded_at of seq 1): `aion start norn_dev
  --input-file input.json --name "244-fix-leg-2"` →
  workflow 4b0139d2-05d7-4401-9a66-e82070b75218
  run 63b34977-2a7b-4128-a63e-d4a478de485f (start-response.json).
- Input verified verbatim in the run's own WorkflowStarted event with
  payload_limit_bytes:0 — all five fields byte-equal to staged input.json,
  no extra keys.
- Watcher: bsvtnl79b, 5-min poll of /workflows/history → run-watch.log,
  exits on terminal event type.

## Gate scope rationale

The pass-2 delta is confined to aion-cli (tail/live.rs, tail.rs, tests).
Reviewer's landing requirement 3 names "aion-cli suite + e2e at minimum";
Waffles' full re-gate follows a LAND verdict and covers the world. Gate
legs: fmt, clippy workspace, clippy -p aion-cli (both resolutions of the
touched crate), full aion-cli suite --no-fail-fast, tail_follow_e2e
--nocapture. Every leg's exit captured to its own log, no pipelines.

## Charter deltas from leg 1

- Only NEW-B1 + NEW-2 are in scope; append-only ground now extends
  through 0328e67a2.
- Commit bodies must carry REAL newlines (pass-2 cosmetic note: leg 1's
  bodies had literal \n\n escapes — tooling artifact, not to repeat).
- Requirement 3's "fresh Opus pass on the small delta" stays at Waffles'
  seat (same division as leg 1: I dispatch the build, he dispatches the
  review).
