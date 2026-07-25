# Red-then-green evidence — dev/drain-leases @ d8aaa006

Captured 2026-07-25 at the reviewer's hands (Vesper Lynd) using surgical
reverts in the committed worktree, per the bound Phase-1 gate: a regression
pin lands with its captured failing run beside the passing one.

## Red A — behavioral: shutdown drain silently skipped liminal workers

The pre-fix broadcast body (the `let Some(sender) else continue` skip,
restored verbatim from the diff) was substituted back into
`ConnectedWorkerRegistry` and the pin re-run:

```
test shutdown_drain_fences_and_deregisters_liminal_worker ... FAILED
assertion failed: !registry.is_registered(worker_id)?
    at crates/aion-server/tests/worker_lifecycle_admin.rs:141
test result: FAILED. 0 passed; 1 failed; exit 101
```

The worker is still registered after a full drain broadcast — the lie of
omission, caught by the pin. With the fix restored the same test passes.

## Red B — absence: the idle-lease capability did not exist

All of `crates/aion-server/src` and `crates/aion-worker/src` were reverted to
`origin/main` (d84799df) with the new test file kept, and the suite compiled:

```
error[E0599]: no method named `register_connection` found for struct
              `HeartbeatTracker` in the current scope   (x2 call sites)
error[E0599]: no method named `record_connection_activity` ...
4 compile errors; exit 101
```

The lease surface is provably new: against the pre-fix code the pins cannot
even compile — an idle wedged worker had no detection path to test.

## Green — restored tree

```
cargo test -p aion-server --all-features --test worker_lifecycle_admin
test result: ok. 4 passed; 0 failed; exit 0
```

Both reverts were discarded (`git checkout d8aaa006 -- <paths>`); the
committed tree is byte-identical to d8aaa006 before this file was added.
