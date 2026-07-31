# 🔴 BLOCK G — #122 connection pool — IN FLIGHT, UNCOMMITTED

**Written at compaction, 2026-08-01. The working tree carries an uncommitted
implementation. Nothing here is on `origin/main` except the docs.**

## State right now

- **Block G is OPEN with Athena.** GO given, naming exactly four invocations:
  `rm -rf target/debug` (done), `cargo clippy -p aion-store-libsql --all-targets`,
  `cargo test -p aion-store-libsql`, `cargo test -p aion-rs --all-targets --no-fail-fast`.
  **Anything beyond those four re-declares.**
- Sampler-6 armed, 100 min, expires **~17:42Z**, band 50. If running at 17:34 she re-arms.
- Disk: reclaimed `target/debug` only (**not** `cargo clean` — that would take
  `target/release`, Tom's #108 server build). free 71 → 89 GiB, release intact at 1.4G.

## 🔴 PINS TAKEN BY ATHENA AT 16:13:59Z — hand BOTH back after the revert

```
crates/aion-store-libsql/src/connection.rs
  sha256 9a9e662f348ec04dbce5135a4fb2eb2d87825869a56071d619bb31a2e94d3363
  14884 bytes

crate-tree digest 2e2f4a19b82364b25d63488f89899d8abb32459d72924cb8338992a6a99b83af
  = sha256 of the sorted per-file sha256s across crates/aion-store-libsql/src/**.rs
```

The second exists because a single-file pin is blind to a stray edit elsewhere by
construction. Both go back to her.

## The one step left mid-air: the INVERTED CONTROL (amendment granted)

**Mutation class declared and approved: one file, one function, reverted.**

1. Break `ConnectionFactory::connect` in `connection.rs` — drop the PRAGMA
   application.
2. `cargo test -p aion-store-libsql pool::tests::a_pooled_connection_carries` →
   must go **RED with its own message**.
3. Restore.
4. Recompute both hashes and hand them to Athena.

Why it matters: the test asserts `synchronous = full` (SQLite **2**, vs the WAL
default **1**) on a **pooled** connection, never the first. That is precisely the
property the pool's existence could silently break. A control never observed
failing is indistinguishable from one that cannot fail.

## What is built (uncommitted, gates green)

```
cargo clippy -p aion-store-libsql --all-targets -- -D warnings   exit 0
cargo test   -p aion-store-libsql                                exit 0, 96 tests
```

- **`connection.rs`** — `ConnectionFactory` is now the ONE place a configured
  connection is produced; `open_connection` is written in terms of it.
  `BUSY_TIMEOUT` renamed **`INHERITED_BUSY_TIMEOUT`** with docs saying the 10 s is
  inherited and unchosen; `LibSqlConfig::busy_timeout_ms` overrides it.
- **`pool.rs` (NEW)** — `ReadPool` + `ReadGuard`. Retention cap, **never**
  concurrency: an empty pool **opens rather than waits**. Poisoning is recovered
  totally via `PoisonError::into_inner` (a `Vec` of handles has no invariant a
  panic can break), so there is **no silent-drop branch**.
- **`store.rs`** — `LibSqlStore { factory, writer: Arc<Mutex<Connection>>, readers }`.
  `transaction_lock` **deleted**. **`connection()` DELETED** — that deletion broke
  43 call sites across four files, which is the accessor proving it was reachable
  from everywhere.
- **`config.rs`** — `busy_timeout_ms: Option<u64>`, `read_pool_size: Option<usize>`,
  both in the crate's existing unset-means-unset idiom. **No values chosen.**
- Call sites: writes → `&*self.writer().await`, reads → `&*self.reader().await?`.
- Test writes rerouted from `reader()` to `writer()` (5 sites) after the bulk
  transform proved too coarse.
- `connection_accessor_reuses_same_database_handle` **renamed** to
  `a_write_is_visible_from_a_different_pooled_connection` — the old name claimed a
  surface the run could no longer touch (#119's defect one level down).

New tests, all passing: `a_pooled_connection_carries_the_configured_pragmas_not_only_the_first`,
`the_pool_retains_at_most_the_configured_count`, `an_absent_read_pool_size_retains_nothing`,
`many_tasks_against_one_store_interleave_writes_and_reads_safely`.

## Still to do after the revert

1. **The dependents gate** — `cargo test -p aion-rs --all-targets --no-fail-fast`,
   launched as background task **`bxctvwjg2`**, log `/tmp/gate-aion-rs.log`
   (writes `EXIT=<code>` as its last line). Already inside the GO.
2. **Commit.** Nothing of the implementation is committed yet.
3. 🔴 **The GRADUATED PIN TEST — Waffles' constraint, not optional.** The
   `crates/aion/tests/outbox_e2e.rs:534-547` interleave (a guarded `append`
   concurrent with an unguarded `complete_outbox_row` on one store) becomes a
   **deliberate** test. It must NOT be allowed to vanish as "the flake went away".
4. `aion-server` may need the two new `LibSqlConfig` fields at its construction
   sites — not yet checked; the dependents gate will say.

## 🔴 CONSTRAINT (a), which binds the commit message

**The pool is justified by the PROVEN MECHANISM, never by #85's flakes.** If the
flakes disappear, that is recorded **consistent, not load-bearing**. No commit
message, doc, or task update may cite flake disappearance as evidence the fix
worked.

## Laws earned in this block

- **Check the COMMAND'S blast radius against the rule, not your INTENTION against
  the rule.** I held "never touch `target/release`" as a rule I was following
  while proposing `cargo clean`, which takes it. That is how a careful person
  deletes something.
- **The rationalisation arrives before the search does.** I wrote a
  `std::process::abort()` into a library `Deref` and justified it as "honest about
  the impossibility". The clean shape took thirty seconds once I looked. If you can
  say why the ugly thing is acceptable before spending two minutes looking for a
  clean one, the articulation is the symptom.
- **Exit 0 from a text transform means "I ran", never "I changed anything."** BSD
  `sed` has no `\b`; my substitution silently did nothing and was caught only by
  grepping after.
- **A published predicate transfers its unstated SCOPE along with its logic.**
  Athena reproduced my count exactly because she inherited the same one-file scope.
