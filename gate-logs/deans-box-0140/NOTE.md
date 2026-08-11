# aion 0.14.0 release battery — Dean's box, final tree c13b4c75b

Venue ruled by Waffles 2026-08-11 11:45Z (Tom's box carries his manifold
re-gate). Box: deanwhiting@100.68.29.10, rustc 1.97.1 (forward-compat canary),
repo at /Users/deanwhiting/Developer/stack/aion, branch release-0140-battery
hard-reset to `c13b4c75b2f852ecdb8737574d21c7fdea3df69e` — the final release
tree (version bump 65d2f96f1 + BC-4 covered-ratchet fix c13b4c75b).

## Legs and verdicts

| Leg | Argv | Result |
|---|---|---|
| fmt-check | `cargo fmt --check` | EXIT=0 (`fmt-check.log`) |
| clippy-workspace | `cargo clippy --workspace --all-targets -- -D warnings` | EXIT=0 (`clippy-workspace.log`) |
| clippy-percrate | `-p aion-cli -p aion-server -p aion-awl -p aion-worker` | EXIT=0 (`clippy-percrate.log`) |
| test-workspace | `cargo test --workspace` (+ `--no-fail-fast` on run-5) | see the run history below |

## The test-leg run history — every red kept beside the evidence

**Run-1** (earlier tree, pre-ratchet-fix): sole failure was the BC-4 covered
ratchet (107 vs the tree's 111 fixtures) — a REAL release-delta miss, fixed as
`c13b4c75b` after a probe run on this box (`bc4-probe-111.log`: 0 divergences,
0 infra, all four harness-section fixtures byte-identical to completion; the
EXPECTED_SUCCEEDED entries were read off that run, not predicted). Run-1's log
was overwritten by run-2 on relaunch; its verdict is recorded here and in the
commit history.

**Run-2** (`test-workspace.log`) — EXIT=101. 155 running-lines vs 155
`test result:` lines (reconciled, no crashed binaries). Sole failure:
`outbox_e2e::outbox_race_settles_first_completion_and_cancels_losers`,
`SQLite failure: bad parameter or other API misuse`. Fail-fast dropped the
remaining ~90 binaries.

**Run-3** (`test-workspace-run3.log`) — EXIT=101, ENVIRONMENTAL. 177/177
reconciled; 436 failures, ALL `Too many open files (os error 24)` in
`aion-server --lib`: the detached launch shell carried the box's soft fd
limit of 256 (hard: unlimited). Not a code failure. Cured by `ulimit -n 8192`
in the leg script (`testleg.sh`), recorded as `ULIMIT_N=8192` in later headers.

**Run-4** (`test-workspace-run4.log`) — EXIT=101. 155/155 reconciled. Sole
failure: `outbox_e2e::outbox_crash_recovery_rearms_stranded_done_row_and_finishes`
— a DIFFERENT test in the same binary, same API-misuse face.

**Run-5** (`test-workspace-run5.log`) — `--no-fail-fast`, full coverage:
**273 running-lines vs 273 `test result:` lines, every binary in the workspace
ran and reconciled.** Sole failure in the entire run:
`aion-store-libsql --lib pool::tests::many_tasks_against_one_store_interleave_writes_and_reads_safely`,
same API-misuse face. outbox_e2e was green this run. 272 of 273 binaries
green; every test outside the libsql flake green.

## The one red class: #85, not the release delta

Every failing test across all runs failed with the same libsql face
(`bad parameter or other API misuse`, plus one SIGABRT with no verdict line),
always in libsql-backed code, and at a DIFFERENT test each time:

- run-2: `outbox_e2e::race_settles…`
- run-4: `outbox_e2e::crash_recovery_rearms…`
- run-5: `aion-store-libsql pool::…interleave_writes_and_reads_safely`
- isolation series: one SIGABRT (`outbox-isolation.log`), one
  `outbox_tests::staged_row_round_trips_node_affinity` (`libsql-iso-5.log`)

A moving target with a constant signature is the flake shape, not the defect
shape. This is task **#85** — libsql 0.9.30 double-close (use-after-free),
filed 2026-08-06 with the mechanism verified in the dependency source, BOTH
faces documented (API-misuse and SIGSEGV/SIGABRT). It predates this cut.
The 0.14.0 delta (#197 server dispatch, #200 assistant queue, #204 CLI/AWL
harness section, one test-constant change) touches no store-layer or
libsql-adjacent code — settled against its inputs, not asserted.

Observed rate on this box at this tree: 3/3 full workspace runs hit exactly
one #85-signature failure each; isolation series `outbox_e2e` 5/5 green
(after one crashed warm-up run), `aion-store-libsql --lib` 4/5 green with the
one hit above. The flake fires under the binaries' own internal test
concurrency; it is not box contention.

## Attestation position

Legs 1–3 green. Test leg: full-coverage 273/273 reconciliation with a single
known-mechanism dependency flake (#85) as the only red class, each affected
binary isolation-proven green at the same tree. Publish/hold decision on this
classification routed to Waffles (his 11:45Z ruling conditioned publish on
battery green; this is green-except-#85-classified, so the word is his).

## Waffles' word (13:30Z): PUBLISH, with the reachability question settled

Ruled: publish under the classification. His riding condition — settle whether
the double-close is production-reachable or test-only — is settled REACHABLE:
the failing pool test models the server's own shape (many tasks against one
Arc-shared store through the connection pool), and `store.backend = "libsql"`
is a selectable production backend (`config/env.rs:252`). Accordingly the
known-issues entry ships in the 0.14.0 CHANGELOG (both copies — root and
`crates/aion-server/changelog-embed/`). The default haematite backend and the
memory backend do not use libsql and are unaffected. Because the embedded
changelog is a build input to aion-server, that crate was re-verified against
its inputs at the changelog commit (fmt + clippy + full crate tests on Dean's
box); every other crate's inputs are byte-identical to c13b4c75b and their
battery verdicts carry.
