# #200 landing gate — assistant off queue `default`

Merge under gate: `b5ad0bc47` (branch worktree-agent-ae5cbb2b6b023215a onto main,
16 files, +1173/−36, including the new
`crates/aion-server/tests/assistant_private_queue.rs`).

## Ruled gate: full `cargo test -p aion-cli` on the merge result

`aion-cli-suite.log` — 26 test binaries, 26 top-level `test result:` verdicts,
all `0 failed`, `SUITE_EXIT=0`. Reconciled per the crashed-binary law: the
`running N tests` line count matches the verdict line count, so no binary died
without a verdict.

## Rider: agent-dev worker suite, red then green

The merge touched `examples/agent-dev/worker/src/descriptors_tests.rs`, so the
out-of-workspace agent-dev worker suite ran as part of the gate.

`agent-dev-suite-red.log` — `SUITE_EXIT=101`, zero binaries ran: E0061 in three
integration-test files, each calling the two-argument
`ActivityContext::new(activity_id, attempt)`.

**Pre-existing, not #200's.** `ActivityContext::new` grew to
`(WorkflowId, RunId, ActivityId, u32)` in the #168 lane (RunId in the
observability keyspace, `6976aa6eb`, landed via swing at `6dbaf9409`). The
agent-dev worker is not a workspace member, so no workspace battery compiled
these three files since; this gate is the first run that did.

Fix (rider on this landing): the three `activity_context()` test helpers in
`tests/{workspace_persistence,handlers_shims,provision_body}.rs` now pass
`WorkflowId::new_v4()` / `RunId::new_v4()` ahead of the existing arguments —
the identities are not asserted on; the context only needs a dispatch identity,
matching the canonical caller in `crates/aion-worker/src/context.rs` tests.
`Cargo.lock` relocked 0.13.6→0.13.8 path deps on the same run.

`agent-dev-suite-green.log` — 7 binaries → 7 verdicts (28+8+12+5+6+3+0 passed),
all `0 failed`, `SUITE_EXIT=0`.
