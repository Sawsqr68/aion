# #122 shape (1): the libSQL connection pool — design

**Ruled by Waffles 2026-08-01 (relayed via Cally). The defect, the proof, and the
ruling verbatim are in `LIBSQL-SHARED-CONNECTION-FINDING.md`; this is the shape.
Nothing is built. No compiler has run on any of it.**

The ruling's own test for this design, in his words:

> "The lock disappears rather than grows: correctness lives in the structure, not
> in 41 methods remembering to take a mutex."

So the design is judged on one question: **can a caller reach a connection without
going through the thing that makes it safe?** Today the answer is yes —
`pub(crate) fn connection(&self) -> &libsql::Connection` hands the shared handle to
anyone, and 36 statement-issuing methods take it without the lock. That accessor
is the hole, and deleting it is the load-bearing part of this change.

## The shape

**One dedicated WRITE connection, plus READ connections acquired per operation.**

```
LibSqlStore
  ├── db: Arc<libsql::Database>          // the factory, already present
  ├── writer: Arc<Mutex<Connection>>     // ONE. not reachable except through .writer().await
  └── readers: ReadPool                  // acquire-per-operation, returns on drop
```

- `async fn writer(&self) -> Result<WriteGuard<'_>, StoreError>` — the only path
  to a write. The guard derefs to `&Connection`; holding it *is* the exclusion.
- `async fn reader(&self) -> Result<ReadGuard, StoreError>` — a connection owned
  for the operation's duration and returned on drop.
- **`connection()` is deleted.** Not deprecated — deleted. There is then no
  expression in the crate that yields a connection without stating which kind of
  access it is.

### Why one write connection and not N

SQLite in WAL mode admits **exactly one writer at a time**. N writer connections
do not increase write throughput; they convert an in-process queue into SQLite
lock contention resolved by the busy handler — which introduces a *failure mode*
(timeout exceeded) that the in-process queue does not have. One write connection
is strictly better here: writes serialise by waiting on a mutex, which cannot
fail, instead of by waiting on a file lock, which can.

It also matches the system: **invariant #3 already says exactly one Recorder per
workflow is the sole append path.** A single write connection is the store-level
expression of the same idea, one layer down.

### Why this is not "the lock, grown"

The current `transaction_lock` is a mutex over `()` sitting *beside* a connection
that everyone can also reach directly. Remembering to take it is a convention, and
36 methods do not.

The proposed writer mutex **owns** the connection. There is no `&Connection`
obtainable except by holding the guard, so "forgetting" is not expressible — the
code that forgets does not compile. That is the difference between a lock you must
remember and a door that is the only way in. Reads never touch that connection at
all, so C2 (dirty reads inside a stranger's transaction) does not merely become
unlikely, it becomes unrepresentable.

**C3 dissolves with no guard at all** (Cally's point, adopted): each operation
holds its connection exclusively for its whole duration, so `sqlite3_changes64`
is per-operation by construction. `record_outbox_failure_delivered` and the
`redrive_outbox_row` guard stop needing to be careful.

**P4b dissolves too**: the six unguarded `BEGIN`s — `put_package`,
`put_package_with_routes`, `register_namespace`, `put_namespace`,
`set_namespace_placement`, `deprecate_namespace` — become writes through the one
door, so a `BEGIN` can never land on a connection already inside one.

## 🔴 The trap that would silently undo this

**Every PRAGMA in `open_connection` is per-CONNECTION state, except one.**

- `busy_timeout` — **per connection.** `sqlite3_busy_timeout(db, ms)`.
- `synchronous` — **per connection.**
- `journal_mode = wal` — persists in the database file, so it survives.

A pool that creates connections with `db.connect()` and does not re-apply
`busy_timeout` and `synchronous` gives every connection but the first the libSQL
default busy timeout of **zero** — reintroducing the exact `SQLITE_BUSY` failure
that `BUSY_TIMEOUT` exists to prevent, and silently dropping the operator's
durability choice on every connection they cannot see.

⇒ **The pool must have exactly one connection factory, and it must be the same
code path `open_connection` uses today.** Not a copy. A second place that
configures a connection is a second place to forget. This gets a test that asserts
the settings on a connection obtained from the pool, not just on the first one —
otherwise it is a green about connection #1.

## Constraint (b): writer contention explicit and loud

After this change, **writer-vs-writer contention inside one store is gone** — one
write connection, serialised by a mutex that cannot time out. What remains is
real and must stay visible:

- **Cross-store.** Production opens a second `LibSqlStore` for the outbox
  dispatcher on the same `store.url` (stated at `connection.rs:12-27`). Two
  stores means two write connections on one file, and they contend in SQLite.
- **External.** Anything else holding the file.

Both are resolved today by `const BUSY_TIMEOUT: Duration = from_secs(10)` —
**a hardcoded value nobody chose**, which is an assumed default under the house
rule. Two changes ride constraint (b):

1. **The timeout becomes configuration**, in the crate's existing idiom:
   `busy_timeout_ms: Option<u64>` on `LibSqlConfig`, `None` meaning *not set* —
   the same "this crate does not assume defaults for operator tunables" contract
   the file already states for `journal_mode`, `synchronous` and
   `sync_interval_seconds`. The current 10 s becomes a value someone picks, and
   the picking is a conversation, not a constant. **Units in the name.**
2. **Exceeding it becomes a NAMED failure**, not `StoreError::Backend(String)`.
   Today an operator cannot distinguish "another writer held the file longer than
   the timeout" from "the disk is gone" — both arrive as an opaque backend
   string. Proposed: a typed variant carrying how long we waited, so the message
   names the cause and the wait. Never a silent retry-forever; the timeout is
   stated, and reaching it is reported.

## Constraint (c): pool sizing, with rationale and units

**`read_pool_size: Option<usize>` — a count of RETAINED connections.**

Following the crate's existing contract exactly (`config.rs:9-11`): absent means
unset, and unset has a stated behaviour rather than an invented number.

- **`None` — no retention.** A fresh read connection per operation, closed on
  drop. Correct, needs no invented number, and cannot grow without bound. Cost is
  one `sqlite3_open` per read against an already-open file.
- **`Some(n)` — retain at most `n` idle read connections for reuse.**

**The rationale, which is the part that matters: `n` caps RETENTION, never
CONCURRENCY.** WAL permits unlimited concurrent readers, so the pool is a *reuse
cache*, not a limiter. If `n` capped concurrency, a reader would block waiting for
a slot — reintroducing a queue exactly where SQLite has none, and inventing a
throughput ceiling nobody asked for. So: **when the pool is empty, a caller opens
a new connection rather than waiting.** Bounded memory, unbounded concurrency.

That property is what lets `None` be honest. With retention-only semantics, `None`
is not "unlimited connections" — it is "no reuse", and the live count is bounded
by concurrent operations either way.

⚠️ **`Some(n)` values are not proposed here.** Sizing touches deployment targets,
and constraint (c) says it is a stated choice to be discussed. `None` is the
shipped behaviour until someone chooses otherwise, because "no reuse" is a
defensible position that requires no number.

## Blast radius

Concentrated, which is the good news. The delegate modules (`append`, `read`,
`outbox`, `timer`, `package`, `namespace`, `visibility`, `schema`) all already
take `&libsql::Connection` and are **unchanged** — they receive a pooled
connection instead of the shared one. The change is:

- `connection.rs` — factory extracted so the pool and the initial open share one
  configuration path.
- `store.rs` — every method acquires before delegating. ~50 sites, mechanical.
  `transaction_lock` is deleted, not extended.
- `config.rs` — two new `Option` tunables, same idiom as the existing three.
- `error.rs` (aion-store) — one named contention variant.

`schema::ensure_schema` runs once at open on one connection; schema is
file-level, so it is unaffected. `sync()` operates on the `Database`, not a
connection — unaffected.

## Acceptance criteria

The first two are Waffles' constraints, written here so they bind whoever
implements this rather than living in two seats' memory.

1. **🔴 The pool is justified by the PROVEN MECHANISM, never by #85's flakes.**
   If the flakes disappear once this lands, that is recorded **consistent, not
   load-bearing**. No commit message, doc, or task update may cite flake
   disappearance as evidence the fix worked.
2. **🔴 The failing interleave GRADUATES to a deliberate pin test.** The
   `outbox_e2e.rs:534-547` shape — a guarded `append` concurrent with an
   unguarded `complete_outbox_row` on one store — becomes an explicit test that
   asserts the property, not a flake that quietly stopped happening. It must not
   be dropped as "the flake went away."
3. **A test proves the pool's connections carry the configured PRAGMAs** —
   obtained from the pool, not the first one opened. Otherwise it is a green
   about connection #1.
4. **`connection()` does not exist.** Grep is the check: no expression yields a
   connection without naming read or write.
5. **A concurrency test with many tasks against ONE store**, replacing the
   coverage `concurrent_writers_on_the_same_file_do_not_lock_each_other_out` only
   appears to give — that one opens two separate connections and proves the
   configuration production does not use inside a store.
6. **The named contention error is reachable and named in a test**, so the
   distinction between "another writer" and "the disk is gone" is proven to
   survive, not just intended.

## What is NOT in scope

- **Shape (2) — one lock over every statement — must not be built as a step.**
  Waffles: a pre-declared contingency, triggering only if a live failure
  *attributed to this mechanism* arrives before the pool lands. "A decision
  inherited, not a dilemma."
- **Shape (3) — locking the unguarded writes only — stays rejected.** It makes
  the symptom rarer and the reasoning wrong.
- **libSQL's fake async.** `Conn::execute` calls the sync FFI on the caller's
  thread, so every store call blocks a tokio worker. Real, separate, and not
  fixed by a pool — it deserves its own item rather than riding this one.
