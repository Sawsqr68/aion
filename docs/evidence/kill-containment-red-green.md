# Red-then-green evidence — dev/kill-containment

Captured by the build lane red-first (test written and run BEFORE the fix)
per the bound Phase-1 gate, verified at the reviewer's hands (Vesper Lynd).
Raw runs preserved in the lane's `gate-logs/` (worktree record); the load-
bearing excerpts are committed here.

## Red — behavioral: cancellation leaked grandchildren

The orphan-scan pin (`cancellation_reaps_the_entire_spawned_process_group`,
`crates/aion-cli/src/worker_shell.rs`) spawns a command whose child spawns a
grandchild, cancels through the real `ActivityContext` cancellation path, and
asserts every PID in the group is gone. Against the pre-fix code (direct-child
`kill_on_drop` only, no process group):

```
failures:
    worker_shell::tests::cancellation_reaps_the_entire_spawned_process_group
test result: FAILED. 0 passed; 1 failed; 153 filtered out; finished in 2.02s
```

The grandchild survived cancellation — the leak proven live.

## Green — the containment fix

Fresh Unix process group per command (`process_group(0)` before exec, shared
adapter `run_cancellable_command` in `aion-worker/src/process.rs`, used by
both `aion worker shell` and the agent-dev harness); cancellation sends
group-wide `SIGTERM`, waits a bounded grace, escalates to `SIGKILL`, reaps
the direct child, and reports cancellation complete only after a
`killpg(signal 0)` → `ESRCH` probe confirms the group is empty. A group that
survives `SIGKILL` yields the typed `CancellationIncomplete` error — `Killed`
is never claimed without proof.

```
test worker_shell::tests::cancellation_reaps_the_entire_spawned_process_group ... ok
test result: ok. 1 passed; 0 failed; 153 filtered out; finished in 0.03s
```

Full battery on the fixed tree: fmt, clippy (`-D warnings`, both crates and
the example harness), `aion-worker` + `aion-cli` + agent-dev suites, and
`cargo test --workspace` — all exit 0 (lane `gate-logs/`).
