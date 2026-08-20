# rb-lockfix flight1 — builder report

## Delivery

Branch: `leg/fix`

Committed evidence and implementation:

- `defe5a14 test(rb-lockfix): capture writer-lock red evidence`
- `5c4dcdf4 test(rb-lockfix): record teardown and lock-holder diagnosis`
- `092de7e5 fix(store): retry transient writer-lock inheritance`
- `5791adcc fix(store): rule and retry writer-lock acquisition`
- `c330b3fa fix(config): state lock ruling in generated configs`

The first source commit above is the `crates/aion` integration-call-site part of the sweep; the second contains the acquisition boundary, server/config wiring, remaining call sites, and regression test.

## R1 — red first

The complete verbatim output is committed at `.fleet/reports/lockfix-red.txt` in `defe5a14`, before any source change.

Full suite result:

```text
test api::http::worker_deployments::tests::http_put_survives_a_second_server_state_over_the_same_store ... FAILED

---- api::http::worker_deployments::tests::http_put_survives_a_second_server_state_over_the_same_store stdout ----
Error: Backend("haematite database error: data dir is locked by another live writer (writer lock held at /tmp/.tmps1eAPp/worker-deployments.db/writer.lock); a second writer would corrupt the shard WALs — use ReadOnlyDatabase to observe")

test result: FAILED. 1155 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.79s
```

Isolation result, committed beside it:

```text
test api::http::worker_deployments::tests::http_put_survives_a_second_server_state_over_the_same_store ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1155 filtered out; finished in 9.24s
```

## R2 — diagnosis at the diff

The exact behavioral change in `git diff 856b2a863..b72a5c35 -- crates/aion` is `crates/aion/src/engine/api.rs:323`, inside `Drop for Engine`:

```diff
- self.runtime.engine_tasks().begin_close();
+ self.runtime.engine_tasks().shutdown();
```

That change adds waiting: `shutdown()` gates, aborts, and joins the owned Tokio runtime, whereas `begin_close()` gates and aborts without the join. Its sign is opposite the original hypothesis that teardown had begun returning earlier.

The complete verbatim E1/E2/E3 output is committed at `.fleet/reports/lockfix-diagnostics.txt` in `5c4dcdf4`. All diagnostic source instrumentation was restored before that commit.

### E1 — engine line does not carry the failure

I temporarily changed `Engine::drop` back to `begin_close()`, rebuilt, and ran the full library suite:

```text
test api::http::worker_deployments::tests::http_put_survives_a_second_server_state_over_the_same_store ... FAILED

test result: FAILED. 1155 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.40s
```

The file was restored immediately. Reverting the line is neither a diagnosis nor a cure, and would reintroduce the descriptor leak fixed by `0f3aff9d`.

### E2 — teardown releases the store it owns

The drop-site probe printed:

```text
PROBE strong_count = 1
PROBE was_last_reference = true
PROBE self scan after drop
```

No `/proc/self/fd` entry pointed at that test's `writer.lock`. The state builds no `Engine`; the last `Arc<HaematiteStore>` was consumed and its synchronous database drop had completed.

### E3 — named holder is a cloned pre-exec child

The full-suite `--nocapture` probe printed:

```text
PROBE holder pid=2928116 fd=542 cmdline="" status=Name:	worker::workspa; State:	R (running); PPid:	2916820; Threads:	1
PROBE failed acquisition: store backend error: haematite database error: data dir is locked by another live writer (writer lock held at /tmp/.tmpvgO94y/worker-deployments.db/writer.lock); a second writer would corrupt the shard WALs — use ReadOnlyDatabase to observe
```

The named holder is a single-threaded, empty-command-line child cloned from the test process during the `worker::workspace_root` spawn burst, before `exec` closes its inherited `CLOEXEC` descriptors. Repeated real attempts succeeded and the instrumented suite finished:

```text
test result: ok. 1156 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.00s
```

### Causal chain

1. The failing test creates a haematite store, performs a PUT, drops the router/state/store, and reopens the same directory.
2. The PUT re-reads and SHA-256 hashes the whole running test binary. With `debug = 0`, that work completes much earlier than the old debuginfo build and places the reopen inside the concurrent workspace-root/process-spawn phase.
3. A spawn clones the parent's descriptor table. Haematite's advisory `flock` belongs to the open file description, so a cloned child temporarily owns the writer-lock description until `exec` closes the `CLOEXEC` descriptor or the child exits.
4. Haematite 0.9.0 performs one nonblocking `try_lock` at `src/db/lock.rs:166`; it never waits. A reopen landing in the clone-to-exec window returns `DatabaseError::DataDirLocked`.
5. The original slow hash merely moved the reopen outside that window. It camouflaged the race; it was not correctness.

The brief's statement that base “waits” is mechanically wrong: the acquire is a bare `try_lock`. The true contract is that the reopen succeeds, and that contract is preserved.

## R3 ruling gate and typed refusal

**Typed refusal — R3 first sentence:** “teardown-complete must not report before the store writer lock is released” cannot be implemented as stated because E1/E2/E3 prove no such premature teardown report exists in the failing test. The test constructs no engine, the final store reference is consumed, this process has no descriptor for the lock, and the named holder is a cloned pre-exec child process. I therefore adopted the confirming branch authorized by the brief: cure transient lock inheritance at the acquisition boundary. This substitute preserves successful reopen after teardown and does not claim to change a teardown ordering that measurement refuted.

## Source fix

`crates/aion-store-haematite/src/lock_acquisition.rs` introduces required `LockAcquisitionRuling` values carrying both patience and retry cadence. There is no library default and no compatibility constructor. `HaematiteStore::open` and the existing-store branch of `open_or_create` now require the ruling.

The acquisition loop:

- retries only `DatabaseError::DataDirLocked`;
- returns every other `DatabaseError` immediately, unchanged;
- returns the same `DataDirLocked` after the ruled patience;
- waits only between real lock attempts and returns on the first successful acquisition.

The cadence is a caller-ruled lock-acquisition wait, not a test synchronization sleep. The regression's transient release is channel-driven and contains no sleep.

Retry safety follows haematite's construction: create takes the writer lock before writing `config.json` (`startup.rs:105-108`) and open takes it before starting shards (`startup.rs:175`). A failed lock attempt has written nothing and started nothing. `adopt_node_cache_budget`, which precedes open, is idempotent by its existing contract.

`create` and `create_with_shard_count` are intentionally unchanged: without an existing `config.json`, there is no prior database writer-lock description for a child to have inherited.

### Operator surface

Production boot requires both:

- `store.lock_acquisition_patience_ms`
- `store.lock_acquisition_retry_cadence_ms`

with matching environment variables. Both must be positive and cadence must not exceed patience. They remain `None` in `StoreConfig::default`; production haematite boot refuses absence instead of inventing a schema default. First-run generated TOML explicitly states `250 ms / 5 ms`, visibly in the operator-owned file. Memory deployments do not need either field.

### Call-site sweep

Every executable external `open`/`open_or_create` caller now passes a ruling. The executable count is 36 across 19 files; the brief's 37th external match is the documentation-only `HaematiteStore::open(<same dir>)` reference in `aion_on_haematite_showcase.rs`. The three implementing-module test calls were also updated.

Tests state `250 ms / 5 ms`, tied to the measured 93-150 ms transient window. The showcase preserves its pre-existing `10 s / 100 ms` policy while replacing its manual error-string retry loop with the typed acquisition boundary. Production passes the validated operator ruling.

## Acquisition-boundary regression

`lock_acquisition::tests::transient_lock_is_retried_but_live_writer_still_fails_unchanged` has both required halves:

1. a second descriptor holds `writer.lock`; a channel releases it only after the first failed real acquisition; reopen succeeds inside patience;
2. a descriptor remains locked beyond patience; the result is still `DatabaseError::DataDirLocked`.

Without the retry arm, committed verbatim at `.fleet/reports/lockfix-regression-red.txt`:

```text
test lock_acquisition::tests::transient_lock_is_retried_but_live_writer_still_fails_unchanged ... FAILED
Error: DataDirLocked { lock_path: "/tmp/.tmpp9V42J/store/writer.lock" }
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 68 filtered out; finished in 0.01s
REGRESSION_WITHOUT_CURE_EXIT=101
```

With the cure restored:

```text
test lock_acquisition::tests::transient_lock_is_retried_but_live_writer_still_fails_unchanged ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 68 filtered out; finished in 0.04s
```

## R4 proof

### Full `aion-server` library suite — first green

```text
test result: ok. 1156 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.41s
```

### Full `aion-server` library suite — second consecutive green

```text
test result: ok. 1156 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.73s
```

### Full workspace green

Command: `cargo test --workspace --no-fail-fast`

Exit status: `0`.

The final target lines were:

```text
Doc-tests aion_worker
running 1 test
test crates/aion-worker/src/lib.rs - (line 9) - compile ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
all doctests ran in 0.84s; merged doctests compilation took 0.84s
```

### Formatting and lint

```text
cargo fmt --all --check
```

Exit status: `0`.

```text
cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized] target(s) in 1m 02s
```

Exit status: `0`.

## Scope and walls

No tests or assertions were deleted or weakened. No timeout was inflated. No synchronization sleep, lint suppression, or ignore attribute was added. No `.unwrap()`/`.expect()` was added to library code. The new implementation module is 135 lines, below the 500-line wall. Existing oversized files were not split as part of this acquisition fix; the new logic was deliberately placed in a focused named module rather than adding it to the already-oversized `store.rs`. No live server, listener, or database outside test tempdirs was touched. No files outside `crates/aion*` and `.fleet/reports` were changed.
