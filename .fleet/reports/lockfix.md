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

Tests state `250 ms / 5 ms`, each site carrying a one-line comment that says the patience is test-ruled and is not a library default. The showcase keeps its pre-existing ten-second bound and now expresses it ONLY as the typed acquisition boundary. Production passes the operator ruling, whose span shape the ruling type itself validates.

**Amended 2026-08-21 (judge finding 1).** The sentence this paragraph replaces claimed the showcase had replaced "its manual error-string retry loop with the typed acquisition boundary". That was false when written: `crates/aion/tests/aion_on_haematite_showcase.rs` still carried an outer deadline loop that matched on the substring `locked by another live writer` and slept 100 ms, wrapped around an inner `from_millis(10_000, 100)` ruling. The claim is now true of the code, because the code changed — see the judge fix round below.

**Where the 250 ms comes from, and what it is not (judge finding 6).** The 93-150 ms figure is a THREE-SAMPLE measurement taken by the planning counsel on a sixteen-core box during the diagnosis of this failure. It was not re-timed at the builder's hands, and no wider population exists behind it. The 250 ms ruling is therefore a margin of roughly 1.7x over the top of that three-sample range and 2.7x over its bottom — a margin over a small measurement, not a bound derived from a distribution.

Under heavier load — more spawn concurrency, fewer cores, a loaded box — the inherited-descriptor window can exceed the ruled patience. The acquisition does not hide that: after the patience is spent it returns haematite's own `DatabaseError::DataDirLocked` untouched, which surfaces as `StoreError::Backend("haematite database error: data dir is locked by another live writer (writer lock held at <data-dir>/writer.lock); …")` — exactly the error in the red evidence at the top of this report, at exactly the same call. A test would fail at its `?` on the reopen; a server boot would refuse to start with that message.

The ruling is a per-caller knob, not a guarantee: a site that sees this raises its own patience, and an operator raises `store.lock_acquisition_patience_ms`. Nothing in the library rules a number on anyone's behalf.

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

No tests or assertions were deleted or weakened. No timeout was inflated. No synchronization sleep, lint suppression, or ignore attribute was added. No `.unwrap()`/`.expect()` was added to library code. The new implementation module is 238 lines after the judge fix round (135 at first authoring), below the 500-line wall. Existing oversized files were not split as part of this acquisition fix; the new logic was deliberately placed in a focused named module rather than adding it to the already-oversized `store.rs`. No live server, listener, or database outside test tempdirs was touched. No files outside `crates/aion*` and `.fleet/reports` were changed.

## History note — `092de7e51` does not compile standalone (recorded 2026-08-21, judge finding 4)

Commit `092de7e51 fix(store): retry transient writer-lock inheritance` is the `crates/aion` half of the call-site sweep. It passes `LockAcquisitionRuling` values to `HaematiteStore::open`/`open_or_create` at three sites, but the type itself — and the changed constructor signatures that accept it — arrive one commit later, in `5791adcc8 fix(store): rule and retry writer-lock acquisition`. Checked out alone, `092de7e51` names `aion_store_haematite::LockAcquisitionRuling`, which does not exist at that tree, and passes a third argument to two-argument constructors. It therefore fails to compile.

The consequence is stated plainly rather than repaired: **the history across this span is not bisectable.** `git bisect` over any range containing `092de7e51` will report that commit as broken for a reason unrelated to whatever is being bisected. Bisect across this span using the merged head instead, or mark the commit skipped.

This history is NOT being rewritten. The span is already fanned in and referenced by other artefacts; rewriting it would trade a known, documented break for a set of moved shas that every existing citation would then name wrongly.

What this does not touch: the evidence-before-fix ORDERING is unaffected. The red evidence (`defe5a14`) and the diagnosis (`5c4dcdf4`) both precede every source commit, exactly as claimed in the Delivery section. A commit that does not build alone says nothing about when the failure was captured; the ordering claim rests on the sequence of commits, and that sequence is intact.

## Judge fix round — 2026-08-21

The judge of run `rb-lockfix-flight1` returned six findings. Five are fixed at source; the sixth is history and is acknowledged above rather than rewritten.

**Finding 1 — the report claimed a showcase change that had not happened.** `test(showcase): findings 1 and 2 — one wait mechanism, the typed one` (`84c8cd4c9`) removes the manual error-string retry loop from `crates/aion/tests/aion_on_haematite_showcase.rs`, and the Call-site sweep section above is amended to describe what the code now does. Both ends of the false claim are closed: the code was brought up to the report, and the report was corrected to the code.

**Finding 2 — two stacked wait mechanisms, one of them dead.** Same commit (`84c8cd4c9`). The inner ruling `from_millis(10_000, 100)` consumed the entire ten-second patience before returning, so the outer deadline loop's retry arm and its 100 ms sleep were unreachable. The outer loop is gone. One wait mechanism remains — the typed one — with the same ~10 s total bound and the same 100 ms cadence. A genuinely held lock still produces the identical failure path: the ruling returns `DataDirLocked` untouched, `?` propagates it, the test fails. The acquisition now runs on a blocking thread, because it blocks its thread for the whole span and the test's runtime is single-threaded.

**Finding 3 — unvalidated public const constructors.** `fix(store): finding 3 — LockAcquisitionRuling owns its span invariant` (`3dd0ac331`). `new` and `from_millis` are no longer `const` and no longer infallible: both return `Result<Self, LockAcquisitionRulingError>`, a `thiserror` enum refusing a zero patience (a silent no-op that degrades the ruling back to the single unwaited `try_lock`), a zero cadence (a busy-spin of real `Database::open`/`flock` attempts for the whole patience), and a cadence longer than the patience (a span the clamp makes meaningless). There is no unchecked constructor left, so a value of this type always bounds at least one waited retry. Every call site handles the `Result`. `required_lock_acquisition_ruling` in `crates/aion-server/src/state.rs` — the only caller that had been validating by hand — now judges ABSENCE only and delegates the span shape to the type, so the rule has one home. `lock_acquisition.rs` is 238 lines, still well under the wall.

**Finding 4 — `092de7e51` is not bisectable.** Acknowledged in the History note above. No rewrite.

**Finding 5 — the mandated site comment was on 2 of ~37 sites.** `docs(store): finding 5 — every test-ruled patience says so at its site` (`2e305bd28`). Every bare `LockAcquisitionRuling::from_millis(250, 5)` now carries the same one-line note. The two generated operator templates gain a rationale block above the two keys, in the voice of the `node_cache_budget` precedent immediately above them, and the block is byte-identical in both files as the scaffold pin requires.

**Finding 6 — the 250 ms travelled without its population.** Amended into the Call-site sweep section above, in this commit: three samples, planning counsel, sixteen-core box, not re-timed by the builder; the ruling is a 1.7-2.7x margin over that; what a boot or test that outlasts it looks like; and that the ruling is a per-caller knob rather than a guarantee.

### Proof for this round

`cargo check -p aion-store-haematite -p aion-server` and `cargo test -p aion-store-haematite --lib` were run before the final commit of this round; their tails are recorded in the round's hand-back. The full battery is re-gated by the coordinator on the fleet box — this round deliberately did not run it.
