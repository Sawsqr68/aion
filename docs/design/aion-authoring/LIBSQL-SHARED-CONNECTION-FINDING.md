# `LibSqlStore` shares ONE raw `sqlite3*` and only guards half its statements

**Found 2026-08-01 by reading, chasing #85's hypothesis. No run, no compile.
Nothing built — the cure is an architectural change to the durable write path
and needs a decision before a line moves.**

## The claim

`LibSqlStore` holds a single libSQL connection. Every method issues statements on
it. **13 methods take `transaction_lock` first; roughly 28 do not** — and the
unguarded set includes writes. Because SQLite transaction state is a property of
the *connection*, a statement issued without the lock while another task holds an
open `BEGIN IMMEDIATE` on that same connection **executes inside that other
task's transaction**.

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

**P4 — the guard is applied to some statements and not others.** Enumerated from
`store.rs` mechanically, not by eye. `transaction_lock` is taken by `append`,
`append_with_outbox`, `rearm_outbox_pending`, `settle_workflow_outbox_rows_cancelled`,
`append_outbox_batch`, the four `claim_outbox_rows*`, `rearm_stale_claimed_outbox_rows`,
`cancel_outbox_rows_for_workflow`, `redrive_outbox_row`.

It is **NOT** taken by these WRITES:

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
before the pool exists. Not started — this is the durable write path, and the
house rule is to agree the shape first.
