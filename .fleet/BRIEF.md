# Fleet fix brief — engine teardown writer-lock regression (rb-lockfix)

2026-08-21, Waffles (author). This wave cures the regression that killed
repo-battery-flight1 at gate round 3. Base of this rig = b72a5c35 (the
full leg tree incl. red evidence in .fleet/gates/).

## The finding, measured (one variable, four arms)

Test: api::http::worker_deployments::tests::http_put_survives_a_second_server_state_over_the_same_store
(crate aion-server, lib suite).

- At THIS tree: full lib suite fails 1155/1 on exactly that test, 2/2
  consecutive runs (quiet box, nofile 65536). It fails FAST (~21s suite).
  Error: haematite "data dir is locked by another live writer (writer
  lock held ...)" on the test own tempdir store.
- In ISOLATION at this tree: 3/3 green (~9.4s each).
- At BASE 856b2a863 (identical box/limits/suite): 1156/0, 2/2 runs — and
  the test itself runs 60+ seconds (cargo prints the over-60s notice;
  suite 125s): at base the second server state WAITS for the first
  state teardown to release the writer lock, then proceeds.
- This tree does not touch aion-server or haematite. It DOES touch
  crates/aion/src/engine/api.rs, crates/aion/src/engine/api/api_tests/*,
  crates/aion/src/runtime/engine_tasks.rs (+ tests).

Diagnostic signature, read it before you code: A TEST THAT GETS FASTER
WHEN IT STARTS FAILING is an awaited completion being skipped — the
wait WAS the correctness. The plausible mechanism: this tree makes
engine/server-state teardown return before the store writer lock is
actually released, so a second open over the same store meets a held
lock instead of an eventual acquire. Follow the evidence if it points
elsewhere; do not follow this brief off a cliff.

## Requirements

R1 — RED FIRST. Reproduce at this tree: run the full aion-server lib
suite (nofile raised; the gate script shows how), commit the failing
output verbatim as red evidence in its own commit. The isolation-green
observation is part of the record; note it.

R2 — DIAGNOSE AT SOURCE. Name the exact change in the engine/api or
engine_tasks diff (git diff 856b2a863..b72a5c35 -- crates/aion) that
alters teardown/lock-release ordering, and write the causal chain into
the report. If you conclude the TEST model is wrong instead, prove it
against base behavior (base waits and passes — that is the contract as
currently tested) and carry the proof.

R3 — FIX AT SOURCE. Restore the teardown contract: teardown-complete
must not report before the store writer lock is released. Do NOT revert
the leg wholesale — its battery/workflow work is wanted; cure the
ordering. Walls, absolute: no test deletion, no assertion weakening, no
timeout inflation, no added sleeps as synchronization, no lint
suppressions in any spelling, no ignore attributes, no file over 500
lines of code, mod.rs re-exports only, thiserror for library errors,
never .unwrap()/.expect() in library code, red evidence as separate
commits.

R4 — PROVE. Full aion-server lib suite green TWICE consecutively, plus
one full cargo test --workspace --no-fail-fast green, all at your hands,
result lines committed in the report .fleet/reports/lockfix.md. The
restored behavior may lawfully make the test slow again (~60s+) — slow
and correct beats fast and lying.
