# Lane charter: aion#42 — parallel-fork ordinal assignment diverges under load

**Date:** 2026-08-18 · **Author:** Vesper Lynd (steward) · **Status:** dispatch-ready
**Board row:** ablative-io/aion#42 (five sightings recorded there with receipts)
**Battery standard:** `ablative/docs/GATE-EXTRACTION-STANDARD.md` (cite, don't restate)

## The defect as observed

`awl_bc4_differential::covered::covered_set_is_byte_identical` fails only on
a loaded box, always the same shape: fixture `dag-fork/valid/after_multi_diamond`,
events 9/10, `/data/activity_id` reference=2/direct=1 swapped, with the matching
result-byte swaps. Five sightings across 2026-08-17 batteries; solo re-runs
always pass (receipts on the board row and in `gate-logs/36-follow-on/`,
`gate-logs/follow-on/`). The two engines assign the fork's activity ordinals
in opposite order when scheduling differs.

## The question that must be answered FIRST (built vs expressible)

Do the design and the determinism-boundary invariant (CLAUDE.md invariant 2)
promise that activity ordinals in a parallel fork are assigned
deterministically — independent of completion/scheduling order?

- **If yes** (read the dag-fork design/briefs and the replay contract before
  concluding): a scheduling-dependent ordinal assignment is a REPLAY
  correctness hazard, not a test fragility — a recorded history replayed
  against re-executed workflow code could bind results to the wrong branch.
  The fix is in the engine: ordinal assignment derived from the workflow's
  deterministic structure (e.g. declaration order), never from arrival order.
  The differential test is then correct as written and becomes the gate.
- **If no** — the design genuinely does not promise cross-path ordinal
  identity for parallel branches — then the differential asserts more than
  the contract. Weakening it is NOT the default: per house law, prefer
  establishing the invariant (deterministic assignment) over documenting a
  limitation, and any narrowing of the differential is a re-scope that goes
  back to the steward with the design citation in hand, not a lane decision.

State the answer with citations in the report before writing any fix.

## Reproduction discipline

The failure needs load. A solo green proves nothing about this defect —
five solo greens already exist. The lane must build a reproduction that
fails deterministically or with high probability WITHOUT depending on
ambient box noise: e.g. constrain the runtime's scheduler threads, inject
completion-order inversion at the fork's join, or run the differential under
a controlled parallel load generator inside the lane. Red first: the
reproduction goes red on the current tree before any fix, and the mutation
sits on the fix's path.

## Scope and mechanics

- Worktree lane off current main (`efc71eda4` or later), plain builder, no
  aion harness. No other lane is live; the zone is `crates/aion` (runtime
  fork dispatch + `tests/awl_bc4_differential/`) plus whatever the ordinal
  trail touches — report any spill beyond `crates/aion` before editing it.
- `NORN_BIN` set for every battery so agent gates execute; extraction
  battery per the standard with per-leg stderr kept (completion proof).
- REPORT.md at the worktree root; cite commits by subject; runtime marker
  `.aion-held-execution.json` is gitignored and must stay untracked.
- The five-sighting history and both solo-rerun receipts are the fixture
  library — do not delete or rewrite them; add to them.
