# #85 — the surviving outbox flake: the three red runs

Every red run observed on the **#122-fixed tree**, preserved because the scratchpad
they were captured in does not survive the session. Gzipped; `gunzip -c` to read.

Command, identical for all runs:

```
cargo test -p aion-rs --all-targets --no-fail-fast
```

## The denominator

**3 red in 11.** Each run indexed by its own `test result:` lines, never by a wrapper's
exit code — the series wrapper reported exit 0 for a batch of six that contained two reds,
because it reports the last statement's status rather than cargo's.

A run only counts if it produced **58** `test result:` lines. All eleven did.

| arm | runs | red |
|---|---|---|
| gate (`gate-aion-rs{,-2,-3,-4}`) | 4 | 1 |
| series (`mine-1..6`) | 6 | 2 |
| Block-H dependents (2026-08-01) | 1 | 0 |
| **total on the fixed tree** | **11** | **3** |

A separate two-run clean-HEAD baseline arm (`base-1`, `base-2`) was both-green. **N=2
licenses nothing** — no claim is made, in either direction, about whether #122 changed
the frequency.

## What is in each file

| file | failing test(s) |
|---|---|
| `gate-aion-rs-3.log.gz` | `outbox_crash_recovery_rearms_stranded_done_row_and_finishes` **and** `outbox_failure_fails_fast_and_cancels_unresolved_siblings` — both in one run |
| `mine-3.log.gz` | `outbox_duplicate_completion_records_exactly_one_terminal` |
| `mine-5.log.gz` | `outbox_happy_path_completes_with_n_terminals_in_order` |

Four distinct tests across three runs. Two are on #85's original list by name and two are
not, which settles that #85 recorded what was **observed**, not a census.

## Start here

**`mine-3.log.gz` is the richest artifact.** The error surfaces *inside the workflow* at
`collect_all:durability:store error` — the fan-in settling four activities at once — so the
failing call is on the durability path reached from `collect_all`, not in test scaffolding.

Two distinct error strings appear across the set, and the second is the more informative:

```
Store(Backend("SQLite failure: `bad parameter or other API misuse`"))   SQLITE_MISUSE
Store(Backend("SQLite failure: `not an error`"))                        SQLITE_OK's text
```

`error.rs:7` is a pass-through, so both are libSQL's own `Display` for
`Error::SqliteFailure`. The second means libSQL took a failure path and then formatted a
message saying no error occurred — i.e. the `sqlite3_errmsg` read did not correspond to the
statement that failed. **`sqlite3_errmsg` is connection-level state, exactly like
`sqlite3_changes64`.** That is the strongest surviving lead, and it is a **lead**: untested,
and labelled as such precisely because the last mechanism this plausible was refuted.

## What is already ruled out

- **Connection sharing on the outbox WRITE path.** Every outbox write was already serialised
  by `transaction_lock` at pre-#122 HEAD. The five unguarded `BEGIN IMMEDIATE` entry points
  were all deploy/namespace admin, none on the outbox path. #85's stated cause is refuted by
  arithmetic on the old code, which needed no run at all.
- **Unguarded outbox READS on the writer's handle** — a genuine shared-handle hazard, and the
  one candidate that could have explained this. #122 removed it. The flake survived.
- **Cross-process database collision.** Temp paths carry pid + nanos + a counter ⇒ intra-process.

Full audit: [`../../BLOCK-G-122-POOL-IN-FLIGHT.md`](../../BLOCK-G-122-POOL-IN-FLIGHT.md),
commit `88e3d4a0`. Task #85 stays open; #122 is closed and did not close it.
