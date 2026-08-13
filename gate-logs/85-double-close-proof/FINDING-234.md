# #234 — SETTLED AS #85, WITH A CORRECTION TO #85 THAT MATTERS

Measured 2026-08-13 ~16:00-16:20 +1000. Binary HASH-PINNED
`40620ca4377d00d0e38f985ed8d41a518b29dd5182564c9d0a0fb74a7ba4f3eb`
(copied out of target/ so a concurrent cargo build could not swap it).
Ledger: `scratchpad/probe-234/ledger.tsv`, one full log per run.

## THE MECHANISM, SETTLED FROM THE COMPILED SOURCE (not from memory)

The error is SQLITE_MISUSE (21). Read the actual bundled sqlite:
`libsql-ffi-0.9.30/bundled/src/sqlite3.c`.

- `sqlite3_open_v2` (184707) calls `openDatabase` (184326–184694) directly.
- `openDatabase` contains **exactly two** MISUSE returns across its whole body:
  1. `if( ppDb==0 ) return SQLITE_MISUSE_BKPT;` — unreachable: libsql passes
     `&mut raw`, never null.
  2. `if( ((1<<(flags&7)) & 0x46)==0 ){ rc = SQLITE_MISUSE_BKPT; }` — requires
     `flags&7 ∉ {1,2,6}`.
- Our flags are `OpenFlags::default()` = READ_WRITE|CREATE → `flags&7 == 6`
  → `1<<6 == 0x40` → `0x40 & 0x46 != 0` → **always passes**.
- libsql is on DEFAULT features (core, replication, remote, sync, tls) — NOT
  `encryption` — so the plain `bundled/src/sqlite3.c` is what compiles, not
  SQLite3MultipleCiphers. The reading above is of the file actually built.

**Therefore the `flags` read at the call site was not the value written at
construction.** No input our code supplies can produce this error. It is a
memory-safety failure inside the dependency — which is exactly what #85 names.

## THE PROBE — TWO ARMS, INTERLEAVED, SAME WEATHER

Interleaved so a drift in box load cannot land on one arm only. Load stayed
2.07–3.77 across all 30 runs — quiet, and quiet for BOTH arms.

| arm | runs | failures |
|---|---|---|
| serial (`--test-threads=1`) | 15 | **0** |
| parallel (default) | 15 | **1** (round 9, load 3.22) |

Combined with the 9 earlier runs: parallel **4 / 24**, serial **0 / 15**.
0/15 against a ~17% rate has ~6% chance of arising by luck — suggestive of a
concurrency trigger, NOT decisive. Fisher on 4/24 vs 0/15 ≈ p 0.14.

## 🔴 TWO CORRECTIONS TO WHAT I WROTE BEFORE THE COMPACT

**1. It is NOT test-specific.** I had recorded this as a failure of
`outbox_failure_fails_fast_and_cancels_unresolved_siblings`. The probe's one
failure was a DIFFERENT test —
`outbox_race_settles_first_completion_and_cancels_losers` — with the same error
code. A defect in the failfast test's own temp-db handling could not do that.
**The "test defect" hypothesis is dead.**

**2. It IS #85, and my "refutes the #85 classification" was too strong.** The
probe's failure reads `SQLite failure: bad parameter or other API misuse` —
#85's recorded wording, verbatim. Same error code, same dependency, same
unreachable-from-our-inputs fault. What my earlier measurement refuted was
#85's *contention-only signature*, not #85's *identity*. Those are different
claims and I conflated them.

⚠️ And I nearly missed this: my probe grepped for the string I had already
seen (`Unable to open connection to local database`) and scored the failing
run `misuse=no`. Had I trusted the ledger column instead of opening the log,
I would have reported a third, unrelated defect. **Same error as the first
census, one week apart: keyed on a remembered wording, not the mechanism.**

## WHAT ACTUALLY CHANGES

#85's record says "isolated: 5/5 green" and 0.14.0 shipped a known-issues entry
on that basis. **#85 reproduces on a QUIET box** — load 2.07–3.77, ~17% of
parallel runs (4/24). 5/5 green against a 17% rate has probability 0.39, so the
old isolated sample never had the power to say what it was read as saying.

So this is not a new defect to file. It is #85, understated.

## OPEN — the fix question, in order

1. **Is there a libsql newer than 0.9.30 that fixes it?** Cheapest possible
   remedy; check BEFORE writing any known-issue text.
   ("Never document a limitation a fix could close.")
2. If not: blast radius is bounded — libsql is the ALTERNATIVE backend;
   haematite is the default. But production opens TWO LibSqlStores on the same
   file (engine + outbox dispatcher), so an operator who chooses libsql is
   exposed to the same concurrency trigger the probe exercised.
3. Serialising the test harness would HIDE this, not fix it. Not on the table.
