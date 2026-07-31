# `LibSqlStore` shares ONE raw `sqlite3*` and only guards half its statements

**Found 2026-08-01 by reading, chasing #85's hypothesis. No run, no compile.
Nothing built — the cure is an architectural change to the durable write path
and needs a decision before a line moves.**

## The claim

`LibSqlStore` holds a single libSQL connection. Every method issues statements on
it. **13 lock sites over 12 distinct methods take `transaction_lock` first; 36
statement-issuing methods do not** — 14 of them writes. Because SQLite
transaction state is a property of the *connection*, a statement issued without
the lock while another task holds an open `BEGIN IMMEDIATE` on that same
connection **executes inside that other task's transaction**.

## Proven by reading — five facts, each with its byte

**P1 — one connection, shared by every clone.**
`crates/aion-store-libsql/src/store.rs:19-24`

```rust
#[derive(Clone)]
pub struct LibSqlStore {
    conn: libsql::Connection,
    db: Arc<libsql::Database>,
    transaction_lock: Arc<Mutex<()>>,
}
```

`libsql::Connection` is `Connection { conn: Arc<dyn Conn + Send + Sync> }`
(libsql-0.9.30 `src/connection.rs:132-135`), and the local implementation behind
that `Arc` is `Connection { raw: *mut ffi::sqlite3, .. }`
(`src/local/connection.rs:27-36`). Cloning the store, or the connection, hands
out another handle to **the same raw SQLite connection**. There is one.

**P2 — a transaction is not a separate object, it is a mode of that connection.**
libsql-0.9.30 `src/transaction.rs:17-42`:

```rust
pub struct Transaction {
    pub(crate) inner: Box<dyn Tx + Send + Sync>,
    pub(crate) conn: Connection,   // <- the SAME connection
    ..
}
impl Deref for Transaction { type Target = Connection; .. }
```

and `transaction_with_behavior` is `Transaction::begin(self.clone(), behavior)`
(`src/local/connection.rs:364-369`) — i.e. `BEGIN IMMEDIATE` issued on that
connection. So while a transaction is open, **every** statement on the store's
connection is inside it, whoever issued it.

**P3 — libsql serialises nothing for us.**
`src/local/impls.rs:22-24`:

```rust
async fn execute(&self, sql: &str, params: Params) -> Result<u64> {
    self.conn.execute(sql, params)      // straight to sync FFI, no lock, no queue
}
```

These `async fn`s never await. They also block the tokio worker thread on FFI,
which is a separate defect worth its own item.

**P4 — the guard is applied to some statements and not others.** Enumerated
mechanically. First reported here as "13 guarded / ~28 unguarded"; the second
number was a guess dressed as a count and is corrected below. **Reproduce it
rather than trusting me** — this predicate walks `store.rs` outside `#[cfg(test)]`,
attributing each `transaction_lock.lock()` to the `fn` it appears in:

```sh
awk '
/^#\[cfg\(test\)\]/ { intest=1 } intest { next }
/^[[:space:]]*(pub |pub\(crate\) )?(async )?fn [a-z_0-9]+/ {
  if (name != "") print (lock?"GUARDED":"unguarded"), name
  match($0, /fn [a-z_0-9]+/); name=substr($0,RSTART+3,RLENGTH-3); lock=0
}
/transaction_lock\.lock\(\)/ { lock=1 }
END { if (name != "") print (lock?"GUARDED":"unguarded"), name }
' crates/aion-store-libsql/src/store.rs
```

**52 `fn` sites. 13 GUARDED, 39 unguarded.** The 13 guarded sites are 12 distinct
methods — `append_with_outbox` is defined twice (inherent impl and trait impl)
and both take the lock. That reconciles with the 15 raw `transaction_lock`
mentions in the file: 13 `.lock()` calls, plus the field declaration and its
constructor initialisation.

Guarded (12): `append`, `append_with_outbox`, `append_outbox_batch`,
`rearm_outbox_pending`, `rearm_stale_claimed_outbox_rows`,
`settle_workflow_outbox_rows_cancelled`, `cancel_outbox_rows_for_workflow`,
`redrive_outbox_row`, and the four `claim_outbox_rows*`.

Of the 39 unguarded, three are not statement paths — `connect` and `open` are
constructors and `connection` is an accessor — leaving **36 statement-issuing
unguarded methods: 14 writes and 22 reads.**

The 14 unguarded WRITES:

```
complete_outbox_row              retry_outbox_row
fail_outbox_row                  record_outbox_failure_delivered
settle_outbox_row_cancelled      schedule_timer
put_package                      put_package_with_routes
put_package_route                delete_package
register_namespace               put_namespace
set_namespace_placement          deprecate_namespace
```

nor by any READ: `read_history`, `read_history_from`, `read_run_chain`,
`list_workflow_ids`, `list_active`, `list_paused`, `query`, `expired_timers`,
`outbox_row_state`, `list_stale_claimed_outbox_rows`,
`list_unsettled_outbox_workflow_ids`, `list_dead_lettered_outbox_rows`, and the
four `count_*`.

**P5 — `execute` reports a CONNECTION-level changed-row count.**
libsql-0.9.30 `src/local/statement.rs:124-135`:

```rust
pub fn execute(&self, params: &Params) -> Result<u64> {
    self.bind(params);
    let err = self.inner.step();
    match err {
        crate::ffi::SQLITE_DONE => Ok(self.conn.changes()),   // sqlite3_changes64(conn)
        ..
```

`sqlite3_changes64` is per **connection**, read *after* the step — not a value
captured from the statement.

## What follows mechanically

**C1 — an unguarded write can be silently rolled back after returning `Ok`.**
`complete_outbox_row` issued while an `append` transaction is open joins that
transaction. If the append rolls back, the outbox row reverts from `done` — but
its caller was already told the settlement succeeded. That is a lost-settlement
bug in the durable delivery path, and it is invisible.

**C2 — unguarded reads can see uncommitted rows.** Same connection, same
transaction, so `read_history` and `list_active` can observe another task's
in-flight writes. `list_active` feeds startup recovery; `read_history` feeds
replay. A dirty read here is not a stale answer, it is an answer about events
that may never exist.

**C3 — `changed > 0` can be another statement's answer.** By P5, if a second
statement finishes on the connection between our `step` and our `changes()`, we
read its count. Two places decide on that boolean:
`record_outbox_failure_delivered` (was the dead-letter judgment marker written?)
and the guarded `UPDATE` inside `redrive_outbox_row`. Both are *status-guarded*
transitions — built precisely so the decision could not be separated from the
act — and the count they trust is not theirs to trust.

## The concrete interleave, in the test that flakes

`crates/aion/tests/outbox_e2e.rs:534-547`, inside
`outbox_happy_path_completes_with_n_terminals_in_order`:

```rust
deliver(&engine, row)?;                              // wakes the workflow ->
                                                     //   its Recorder appends (LOCKED, opens a tx)
store.complete_outbox_row(&row.dispatch_key).await?; // UNLOCKED UPDATE, same connection
wait_for_history(..).await?;                         // UNLOCKED read, same connection
```

The delivery wakes the workflow on another task, which records its terminal
through `append` — the locked path, which opens `BEGIN IMMEDIATE`. The test
immediately issues `complete_outbox_row`, which takes no lock. Both land on one
raw `sqlite3*`. This is exactly the shape P1–P3 say is unsound, in exactly the
test that fails, calling exactly the method that has no guard.

**That is a lead, not a proof.** It has not been demonstrated at runtime, and
`SQLITE_MISUSE` — flake (2)'s "bad parameter or other API misuse" — has not been
traced to a specific interleaving. What is proven is that the invariant the code
depends on does not hold.

## The existing test proves the opposite configuration

`crates/aion-store-libsql/src/connection.rs:223-295`,
`concurrent_writers_on_the_same_file_do_not_lock_each_other_out`, opens **two
separate connections** via two `open_connection` calls and hammers both. It is a
good test and it passes honestly — of a configuration production does **not** use
inside a single store. Nothing exercises many tasks against ONE connection, which
is what every `Arc<LibSqlStore>` actually is. Same class as #119: a green about a
different program.

## Exposure — measured, because severity should not be assumed either

**libSQL is not the default backend, and that bounds this.** The default is
haematite (`crates/aion-server/src/config/sections.rs:646`). libSQL is selected
when — and only when — a store URL is given: both `--store-url`
(`config/load.rs:356-370`) and `AION_STORE_URL` (`config/env.rs:35-48`) coerce an
implicit `Memory` or `Haematite` backend to `LibSql`, on the stated reasoning that
haematite ignores `store.url` so a URL can only mean libSQL. An operator who
writes `backend = "libsql"` lands there directly.

So the exposed population is: every deployment naming a store URL, and the whole
outbox/durability test suite, which runs on libSQL throughout.

**The haematite backend does not have this shape** — checked, not assumed.
`HaematiteStore` (`crates/aion-store-haematite/src/store.rs:194-212`) holds an
`Arc<haematite::EventStore>` and nothing connection-scoped; there is no
transaction mode living on a shared handle, and its only lock (`owned_shards`,
an `RwLock`) guards shard enumeration, not statement issue. Whatever else is true
of it, the defect described here is libSQL-specific.

## The fix is a decision, not a patch

Two coherent answers; a third that only looks like one.

1. **Connection pool — one connection per operation.** What SQLite in WAL mode is
   designed for: independent transactions, real read concurrency, one writer at a
   time enforced by the existing `BUSY_TIMEOUT`. `transaction_lock` disappears
   rather than growing. Correct, and the largest change.
2. **Guard everything with the one lock.** Every statement, reads included, takes
   `transaction_lock`. Sound, small, and serialises the entire store — reads
   would then queue behind writes. Honest, and slow.
3. ~~Add the lock to the unguarded writes only.~~ **Not a fix.** Reads would still
   run inside other transactions (C2), and C3 is untouched. It would make the
   symptom rarer and the reasoning wrong, which is worse than leaving it visible.

Recommendation: **(1)**, with (2) as the interim only if something must ship
before the pool exists.

## 🔴 RULED — shape (1), the connection pool

**Waffles, 2026-08-01, relayed through Cally Ray. Pinned here verbatim because a
ruling that is not in the builder's hands has not been issued — this exact
failure has already cost one ruling between compactions.**

> "the only shape that dissolves the hazard class instead of guarding it — a
> transaction is connection-scoped BY CONSTRUCTION, it cannot leak into a
> stranger's statements because there are no strangers on the line, and changes64
> becomes per-operation the same way. The lock disappears rather than grows:
> correctness lives in the structure, not in 41 methods remembering to take a
> mutex."

**Shape (2) is not a step and must not be built as one.** It is a pre-declared
contingency whose trigger is named now: it activates **only** if a live failure
*attributed to this mechanism* arrives before the pool lands — *"a decision
inherited, not a dilemma."* Shape (3) stays rejected.

**Three constraints ride the ruling, his words:**

- **(a)** *"the pool is justified by the proven mechanism, never by the flakes."*
  If #85's flakes disappear once the pool lands, that is recorded as
  **consistent, not load-bearing**. The failing test's interleave **graduates to
  a deliberate pin test** after the pool rather than dying quietly as a flake.
- **(b)** *"Writer-contention behavior is explicit and loud."* WAL still admits
  one writer. Busy handling gets a stated timeout and a **named failure mode** —
  never a silent retry-forever.
- **(c)** *"Pool parameters declared with rationale and units — no assumed
  defaults; sizing is a stated choice, discussed if it touches deployment
  targets."* This is the house NO ARBITRARY LIMITS rule applied to pool sizing.

Cally's addition, adopted: under a pool the `changes()` hazard (C3) needs no
guard at all — the counter is per-operation by construction. That is the
delete-the-hazard shape rather than the guard-the-hazard shape, and it is one
more argument for (1) over (2).
