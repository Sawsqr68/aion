# #85 — the flake is a double-free in libsql 0.9.30

**Seat:** Vesper Lynd. **Date:** 2026-08-01.
**Status:** mechanism **verified in the dependency's source at my own hand**;
manifestation inferred and not yet executed. Decision required — see §6.

## 1. What was observed

An intermittent failure in `crates/aion/tests/outbox_e2e.rs`, measured at
roughly **37.5% per run**, now seen on **six different tests** and also in
`aion-server`. The text:

```
SQLite failure: bad parameter or other API misuse
```

That is `SQLITE_MISUSE` (21). A second recorded variant is `not an error`.

## 2. Why the growing test list was the real clue

#85's own note said its list of affected tests was "an observation, not a
census", and it was right twice over. Six tests across two crates share no
fixture, no database file, and no registry — the temp path is
`pid + nanos + counter` unique. **The only thing they share is `LibSqlStore`
plus a multi-threaded test process.** A defect whose only common factor is the
process demands a process-global mechanism, and there is exactly one shared
process-global resource in play: the allocator.

## 3. The mechanism — VERIFIED, not inferred

In `libsql-0.9.30`:

```rust
// src/local/connection.rs:104-112
pub fn disconnect(&mut self) {
    if Arc::get_mut(&mut self.drop_ref).is_some() {
        unsafe { libsql_sys::ffi::sqlite3_close_v2(self.raw) };
    }
}
```

`Arc::get_mut` **does not consume the Arc**, and `self.raw` **is never
nulled**. So `disconnect()` is **not idempotent**.

It is then called from **two** `Drop` impls in one ownership chain:

```rust
// src/local/impls.rs:104-108
impl Drop for LibsqlConnection { fn drop(&mut self) { self.conn.disconnect() } }

// src/local/connection.rs:38-42
impl Drop for Connection      { fn drop(&mut self) { self.disconnect() } }
```

and `LibsqlConnection` **contains** that `Connection`
(`pub(crate) struct LibsqlConnection { pub(crate) conn: super::Connection }`,
`impls.rs:15-18`).

Rust drops the outer value first, then its fields. So on every ordinary drop
with no outstanding clone (`strong == 1`):

1. `LibsqlConnection::drop` → `disconnect()` → guard passes → `close_v2(raw)`.
   The handle is freed.
2. the `conn` field drops → `Connection::drop` → `disconnect()` → the Arc was
   never decremented, so the guard **passes again** → `close_v2(raw)` on the
   **already-freed pointer**.

There is exactly **one** `sqlite3_close` call site in the whole crate, so
there is no compensating path. All five facts above were verified by reading
the vendored source directly, not taken on report.

## 4. Why that produces these two strings specifically

Both texts come from libsql's
`Error::SqliteFailure(extended_error_code(db), error_from_handle(db))`, where
`error_from_handle` is a raw `sqlite3_errmsg(db)` (`src/errors.rs:89-97`).

- `"bad parameter or other API misuse"` — `sqlite3_errmsg` when
  `sqlite3SafetyCheckSickOrOk(db)` fails, i.e. **the handle is closed or
  garbage**.
- `"not an error"` — `sqlite3_errmsg` read from a **different, healthy**
  connection.

These are not two bugs. They are the two outcomes of reading through a stale
`sqlite3*`: freed block not yet reused → MISUSE text; block already reused by
another live connection → "not an error". The second string was the original
"mismatched `sqlite3_errmsg`" lead, and it is now explained rather than
merely noted — `sqlite3_errmsg` is per-connection, so a mismatched message
*is itself evidence* the pointer no longer names the connection that failed.

## 5. Why the earlier refutation was correct AND the trail still ran cold

"Write-path connection sharing" was investigated and **refuted**, and that
refutation was right: aion's write path is behind `Arc<Mutex<Connection>>`
and reads hold an exclusive guard. There is no sharing **in aion's code**.

The sharing is in the **allocator**, one layer below where anyone was
looking. This is why the search terminated: the hypothesis was framed in
terms of aion's ownership model, and the defect is not in aion's ownership
model.

⇒ **A refutation is only as wide as the layer it was framed in.** Refuting
"we share a connection" does not refute "something else acquires our freed
connection's memory."

### The dose amplifier

`LibSqlStore::open` passes `read_pool_size: None` (`store.rs:115`), and the
pool retains nothing when `retain` is `None` — so **every read operation opens
and then double-closes a fresh connection**. `outbox_e2e` polls on 25 ms loops
with all eight tests at `worker_threads = 4`, run concurrently by cargo in one
process: thousands of double-close events per run, ~32 threads wide, against
one malloc arena.

This is **dose, not cause**. Raising `read_pool_size` would lower the observed
rate while leaving the use-after-free fully intact — which would be the worst
possible outcome, because it would look like a fix and would only be hiding
the signal.

## 6. 🔴 Decision required (Waffles / Cally — joint authority)

The defect is **upstream, in a crates.io dependency**, and every available fix
is a cross-repo change:

- **Minimal correct fix:** make `disconnect` idempotent — null `self.raw`
  after closing (`sqlite3_close_v2(NULL)` is a documented no-op), or delete
  one of the two `Drop` impls. This is a patch to libsql.
- **Landing it** means a `[patch.crates-io]` fork or an upstream PR. House law
  says cross-repo changes land in their own repo, never vendored.
- **Not-a-fix:** setting `read_pool_size`. Lowers the dose, hides the signal,
  leaves the UAF.

**Scope note, and it is the reason this is not a stop-everything P0:**
haematite is the default durable backend; libsql is the *alternative*. This
does not touch the default path.

## 7. What has NOT been done

**The manifestation is inferred.** The double-close is proven by construction;
that the freed block is actually reused at the rate needed is not. Two
read-only discriminating experiments exist, both re-running an already-built
binary, neither requiring a rebuild:

- **E1 — Guard Malloc.** Run the existing `outbox_e2e` test binary under
  `libgmalloc`, which unmaps freed pages instead of recycling them.
  Double-close predicts a **deterministic fault inside `sqlite3_close_v2` at
  the first teardown, in every test**, with two `disconnect` frames in the
  backtrace. Every rival hypothesis predicts no fault at the close call.
  This converts a 37.5% flake into a 100% signal.
- **E2 — lldb breakpoint on `sqlite3_close_v2`**, printing the pointer.
  Double-close predicts the **same pointer twice, back to back, per
  connection**. Nothing else does.

Both are true discriminators: no rival hypothesis produces a fault *at the
close call itself*. Both must wait until the workspace battery finishes —
running them now would contend, and the battery may relink the binary.

## 8. A separate finding, not the cause

`execute_pragma` (`crates/aion-store-libsql/src/connection.rs:225-237`) calls
`conn.query(...)` and drops the `Rows` without calling `next()`. libsql's
`local::Statement::query` **always returns `Ok`** — it steps eagerly and
stashes the error in a `RefCell` only surfaced by `Rows::next()`. So a failing
`PRAGMA journal_mode = wal` or `PRAGMA synchronous` is **silently discarded**
on every connection the factory produces.

This cannot produce `SQLITE_MISUSE` and is **not** the cause. It is a
straightforward violation of the no-silent-failures rule and should be fixed
on its own merits.

## 9. Method laws earned

**A refutation is only as wide as the layer it was framed in.** "We do not
share a connection" was true and did not refute "our freed connection's memory
is acquired by someone else."

**When the only common factor across failures is the process, suspect a
process-global resource** — and the allocator is a process-global resource
that no ownership audit will show you.

**A dose reduction that looks like a fix is worse than no fix**, because it
removes the evidence while keeping the defect.

**An error message read through a stale handle is itself evidence.** The
"mismatched `sqlite3_errmsg`" was treated for weeks as a confusing detail; it
was in fact the diagnostic — per-connection state cannot mismatch unless the
pointer stopped naming the connection.

---

# AMENDMENT — 2026-08-13. §7 IS DISCHARGED: the manifestation is no longer inferred.

**Seat:** Vesper Lynd. The two experiments §7 parked ("both must wait until the
workspace battery finishes") were run once the battery completed and the box
was quiet. E1 fired on the first attempt.

## E1 — Guard Malloc. Executed, armed, and decisive.

Binary: the already-built `outbox_e2e` test binary, copied out of `target/` and
**hash-pinned** (`40620ca4377d00d0e38f985ed8d41a518b29dd5182564c9d0a0fb74a7ba4f3eb`)
so a concurrent build could not swap the artifact mid-measurement. No rebuild.

`DYLD_INSERT_LIBRARIES=/usr/lib/libgmalloc.dylib`, one test,
`--test-threads=1`. **The instrument's own channel was checked first** — the
`GuardMalloc[...]` banner is present in the log, so a clean run would have
meant something. It was not clean: EXIT=139.

```
stop reason = EXC_BAD_ACCESS (code=1, address=0xd0b3b3d41)
frame #0: sqlite3SafetyCheckSickOrOk(db=0x...) at sqlite3.c:36835
          -> eOpenState = db->eOpenState;
frame #1: sqlite3Close(db=0x..., forceZombie=1)     at sqlite3.c:182133
frame #2: sqlite3_close_v2(db=0x...)                at sqlite3.c:182234
frame #3: libsql::local::connection::Connection::disconnect  connection.rs:110
frame #4: <Connection as Drop>::drop                         connection.rs:40
frame #6: drop_glue::<libsql::local::impls::LibsqlConnection>
frame #13: aion_store_libsql::pool::ReadGuard drop
frame #14: LibSqlStore::list_packages                        store.rs:234
frame #16: aion::loader::persistence::reload_persisted_packages
frame #18: <EngineBuilder>::build
```

§7 predicted "a deterministic fault inside `sqlite3_close_v2` at the first
teardown". That is exactly what happened. **Note one honest difference from the
prediction:** §7 said "two `disconnect` frames in the backtrace". There is ONE.
That is consistent, not contradictory — this stack IS the second close; the
first already returned on an earlier stack. The evidence for doubling is the
source below, not a doubled frame.

## The defect, at the line — two `Drop` impls over one handle

```rust
// libsql-0.9.30/src/local/impls.rs:16-18
pub(crate) struct LibsqlConnection { pub(crate) conn: super::Connection }

// libsql-0.9.30/src/local/impls.rs:108-110
impl Drop for LibsqlConnection {
    fn drop(&mut self) { self.conn.disconnect() }        // CLOSE #1
}

// libsql-0.9.30/src/local/connection.rs:38-42
impl Drop for Connection {
    fn drop(&mut self) { self.disconnect() }             // CLOSE #2
}

// libsql-0.9.30/src/local/connection.rs:108-112
pub fn disconnect(&mut self) {
    if Arc::get_mut(&mut self.drop_ref).is_some() {
        unsafe { sqlite3_close_v2(self.raw) };           // `raw` is NEVER nulled
    }
}
```

`LibsqlConnection::drop` runs and closes. Rust then drops its fields, so the
inner `Connection`'s own `Drop` runs and calls `disconnect` **again** — and
because `drop_ref` is untouched by the first call, `Arc::get_mut` still returns
`Some`, so the guard does not stop it. Two `sqlite3_close_v2` on one handle,
**every teardown, unconditionally.**

## 🔴 THE READING THIS OVERTURNS — including one I wrote myself

The task-list summary of #85 said "isolated: 5/5 green", and 0.14.0's shipped
known-issue inherited that as "rarely, under concurrent load". Both invite
"a quiet machine is safe". **It is not, and load is not the variable.**

The double close is unconditional. What contention changes is only whether the
freed block has been *reused* by the time the second close reads it — that is,
whether the fault is **visible**, not whether it happens. §5's own warning
("dose, not cause") said this, and the narrowed summary lost it.

I nearly repeated the error in the other direction. On measuring 4 failures in
24 parallel runs against 0 in 15 serialised runs, I first wrote into the
CHANGELOG that "the trigger is concurrency inside one process". E1 refutes that
too: the fault fired on a **single-threaded** run, on the first test. Serialising
does not avoid the defect; it lowers the dose. **A rate difference between two
arms measures visibility, and only names a trigger if the mechanism agrees.**

## Where it fires — not an exotic path

Frames 13-18: a read-pool guard dropped by `LibSqlStore::list_packages`, called
from `reload_persisted_packages` inside `EngineBuilder::build`. That is the
**startup catalog reload** — what every libsql-backed server does when it comes
up, not a rare concurrent-load event.

## Method laws earned

**Check your instrument's channel before believing its silence.** The
GuardMalloc banner was counted before the result was read. A clean run under an
instrument that never armed is a non-run, not a refutation.

**A rate difference between arms measures visibility, not mechanism.** Two arms
can differ sharply while the underlying fault is identical and unconditional in
both. Only the mechanism can tell you which you measured.

**A summary that narrows its own source outranks the source in practice.** The
canonical record here never claimed contention-only; the one-line task summary
did, and the shipped artifact followed the summary. When two records of one
fact disagree, the shorter one is what people act on.
