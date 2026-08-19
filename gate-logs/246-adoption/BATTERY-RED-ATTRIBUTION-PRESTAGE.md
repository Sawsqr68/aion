# #246 gate battery — red attribution pre-stage (written 15:42Z 2026-08-15, gate STILL RUNNING)

Battery log at ~1MB shows two red legs so far: `rust:test` exit 101 (line 7052)
and `rust:test-all-features` exit 101 (line 14198). Four distinct failing
tests (two repeat across both legs). Gate leg was at `gates:costly-announced`
at 15:41Z. Final pass counts to be read at terminal — this note pre-stages the
attribution evidence only.

## The four reds and their faces (extracted from gate-full-battery.log)

1. **`every_living_awl_document_checks_clean`** (aion-cli awl_corpus_gate, ×2 legs)
   Face: `workflows/fleet-dev/fleet_dev.awl` — 3 agent actions (`plan`,
   `judge`, `delegate`) return `Reply` where the checker demands the canonical
   three-field agent-outcome record (text/stop_reason/session_id — the #205
   law). fleet_dev.awl is #239's document, committed before this branch.
2. **`committed_closure_matches_a_fresh_oracle_harvest`** (aion-awl-package
   sdk_closure_freshness, ×2 legs)
   Face: committed closure BYTES lag the harvest for module `aion@agent`
   (AtU8, Code, FunT, ImpT, LitT, LocT chunks). Remedy named by the test
   itself: `scripts/regen-awl-sdk-closure.sh` + commit. Same #205-era agent
   module change, closure never regenerated.
3. **`outbox_duplicate_completion_records_exactly_one_terminal`** (aion-rs
   outbox_e2e, ×1)
   Face: `Schedule … store backend error: SQLite failure: 'not an error'` —
   the #85 libsql double-close class (task-recorded as unconditional, known
   issue shipped in 0.15.0's known-issues entry).
4. **`covered::covered_set_is_byte_identical`** (aion BC-4 differential, ×1)
   Face: 2 DIVERGENCES in `dag-fork/valid/fork_named_homogeneous`
   event[6]/event[7] `/data/activity_id` swapped (reference 1/0 vs direct
   0/1) — the #149 class ("BC-4 differential gate is red on main AND
   nondeterministic").

## Builder-causality check (measured at my hand, 15:42Z)

Diff footprint `7109c1f6d..f3bbe645e` (4 commits, tree clean):
`Cargo.lock`, `aion-integration-norn/{Cargo.toml, harness.rs, lib.rs,
spawn_guard.rs}`, `aion-server/{error.rs, worker/bridge.rs}` — 391 insertions,
7 files.

Settled against each red's own inputs (build-graph, not extensions):
- Red 1's inputs = tree's `.awl` corpus + aion-awl checker: NEITHER touched.
- Red 2's inputs = SDK sources + toolchain + committed beams: NONE touched.
- Red 3's inputs = aion-rs engine + libsql store backend: NEITHER touched
  (builder touched aion-server, a different crate).
- Red 4's inputs = aion crate + BC-4 corpus: NEITHER touched.

**Hypothesis for Waffles (his re-gate decides):** all four reds are
branch-point inheritance (worktree at 7109c1f6d; main now 051b547a6), same
stale-base shape as his #244 battery reds. His planned re-gate on the REBASED
tree is exactly the decisive instrument — if reds 1/2 persist on rebased main
they are main defects needing their own tasks (fleet_dev.awl `Reply` returns;
closure regen), not #246 blockers. Reds 3/4 are already task-recorded classes
(#85, #149).

Box load at reading: 36.1 / 24.1 / 20.8 (three batteries). Reds 1/2 are
deterministic faces (checker refusal, byte comparison) — trustworthy under
contention. Red 3/4 are known classes independent of this branch.
