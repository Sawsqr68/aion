# BLOCK H — make "a write on a read connection" a COMPILE error

**Status: designed, not built. Fenced out of Block G deliberately.**
Prerequisite: G lands (pool + `schedule_timer` fix).

## The defect this exists to make impossible

Block G routed `schedule_timer` — an `INSERT … ON CONFLICT DO UPDATE` — through
`reader()`. Durable timer scheduling would have executed on a pooled **read**
connection, outside the writer's mutex, concurrent with the writer's
`BEGIN IMMEDIATE`. That is #122's own mechanism, rebuilt inside #122's fix.

Two properties make this class dangerous rather than merely embarrassing:

1. **It compiles.** `ReadGuard` derefs to `libsql::Connection`, which has
   `execute`. Nothing in the type system distinguishes the connection you may
   write on from the one you may not.
2. **It passes.** All 90 crate tests were green with the defect live, because
   ~78 of them are single-threaded `#[tokio::test]` and never contend. The
   battery is blind to this class **by construction**.

It was caught by a habit — asking the blast-radius question during a gate wait.
Habits die with whoever holds them. **The cure has to be mechanical.**

## Design

`ReadGuard` stops implementing `Deref<Target = libsql::Connection>` and instead
hands out a capability type that has no `execute` at all:

```rust
/// A connection restricted to reads. There is no `execute` on this type,
/// so a write cannot be issued through it — the compiler enforces #122's
/// writer/reader split rather than a reviewer.
pub(crate) struct ReadOnly<'a>(&'a libsql::Connection);

impl ReadOnly<'_> {
    pub(crate) async fn query(
        &self,
        sql: &str,
        params: impl libsql::params::IntoParams,
    ) -> Result<libsql::Rows, StoreError>;
}

impl ReadGuard {
    pub(crate) fn read(&self) -> ReadOnly<'_>;
}
```

Every read helper changes signature from `conn: &libsql::Connection` to
`conn: ReadOnly<'_>`. Write helpers are untouched — they keep
`&libsql::Connection` or `&Transaction`.

### No escape hatch, and why that is affordable

A `ReadOnly::from_writer` would reintroduce the judgement call the type exists to
remove. It is not needed: the audit found **no** read helper that is ever called
with the writer connection. Reads performed inside a write transaction already
take `&Transaction`, a different type entirely. If a future caller genuinely
needs one, adding it should be a deliberate, reviewed, greppable act.

### Call-site shape

The guard is currently used as a temporary:

```rust
crate::read::read_history(&*self.reader().await?, workflow_id).await
```

`ReadOnly` borrows the guard, so the guard must be bound — a temporary would be
dropped while still borrowed:

```rust
let guard = self.reader().await?;
crate::read::read_history(guard.read(), workflow_id).await
```

That binding is not noise: it makes the connection's lifetime visible at the
call site, which is exactly the property #122 turned on.

## Honest cost

| what moves | count |
|---|---|
| production `reader()` call sites | 22 |
| test-module `reader()` call sites | 44 |
| read-helper signatures | ~24 |

This is a mechanical change across ~90 sites. It is not small, and pretending
otherwise is how it would get rushed. It is worth it because it converts a
property currently held by my attention into one held by `rustc`.

## 🔴 The inverted control — required, not optional

**A type-level guarantee never watched rejecting anything is the same species as
a test never watched failing.** Landing this on the strength of my own judgement
would be landing the cure on the disease.

The control: hand a `ReadOnly` to something that executes, and confirm the build
fails **with the error that names this design** — not merely that it fails. A
build broken for an unrelated reason is indistinguishable from a working guard
unless the message is read.

```
error[E0599]: no method named `execute` found for struct `ReadOnly` in the current scope
```

The compiler's exact text goes in the commit message. "It failed to build" is
not evidence.

### Standing control vs one-time observation

A one-time manual observation decays: nothing stops a later `Deref` impl from
quietly restoring the hole. Two ways to make the control permanent, in
preference order:

1. **`trybuild`** — a `tests/ui/` case asserting the compile failure and its
   message, re-checked by every `cargo test`. Costs one dev-dependency.
2. **`compile_fail` doctest** — no new dependency, but doctests only run on
   **public** items, and `ReadPool`/`ReadGuard` are `pub(crate)`. This is likely
   unavailable without widening visibility, which would be the wrong trade.

Recommendation: `trybuild`, with the dev-dependency declared and justified. If
that is refused, the fallback is the manual control **plus** an explicit note
that the guarantee is unpoliced — never silently the manual one.

## Acceptance

1. `ReadGuard` no longer implements `Deref`; no path can reach `execute` from a
   read connection.
2. All 22 production sites converted; the write/read split is now a type
   distinction, not a naming convention.
3. The inverted control has been **run**, and the compiler's message is quoted
   verbatim in the commit.
4. Crate gates green, dependents gate green, both read from the artifacts'
   own `test result:` lines rather than a wrapper's status.
5. No `from_writer` escape hatch exists.

## Why this is Block H and not Block G

Absorbing a ~90-site refactor into G would make G's cost figure describe two
things and citable for neither. G had already expanded once (the vacuous PRAGMA
test rewrite), declared in the open. Twice is where a boundary stops meaning
anything — and the uncommitted P0 timer fix is the most losable thing in the
tree until G lands.
