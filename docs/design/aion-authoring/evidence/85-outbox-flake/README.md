# #85 — the surviving outbox flake: the three red runs

Every red run observed on the **Block G final tree**, preserved because the scratchpad
they were captured in does not survive the session. Gzipped; `gunzip -c` to read.

Command, identical for all runs:

```
cargo test -p aion-rs --all-targets --no-fail-fast
```

## The denominator

> **3 red in 8 runs** of that command on the **Block G final tree (pool + timer P0)**,
> 2026-07-31 16:27–17:05Z. All 8 verified COMPLETE — 58 `test result:` lines each.
> Four distinct `outbox_e2e` tests; two of the four are #85's recorded *pre-change*
> flakes. Adjacent-tree runs are recorded separately and **not pooled**. Baseline arm
> is N=2, so no comparison is drawn.

| arm | tree | runs | red | pooled? |
|---|---|---|---|---|
| `gate-aion-rs.log` | T1 — pool, pre-inverted-control | 1 | 0 | **no** |
| `gate-aion-rs-2.log` | T2 — pool, **pre timer-P0** | 1 | 0 | **no** |
| `gate-aion-rs-3.log`, `-4.log`, `mine-1..6` | **T3 — Block G final** | **8** | **3** | **yes** |
| `blockh-dependents.log` | T4 — T3 + Block H | 1 | 0 | **no** |
| `gate-121-t5.log.gz` | T5 — T4 + the #121 tests + the `cfg` widening | 1 | **1** 🔴 | **no** |

Each run is indexed by its own `test result:` lines, never by a wrapper's exit code —
the series wrapper reported exit 0 for a batch of six that contained two reds, because
it reports the last statement's status rather than cargo's.

A separate two-run clean-HEAD baseline arm (`base-1`, `base-2`) was both-green. **N=2
licenses nothing** — no claim is made, in either direction, about whether #122 changed
the frequency.

### 🔴 CORRECTED — this file first published **3 in 11**, which pooled four trees

Adjudicated by Athena on tree identity, which is measurable, rather than on either
party's recollection:

- The **timer P0 edit landed at 16:24:15Z**. Attempt 2 *finished* at 16:25:57Z and these
  runs take ~4½–6 minutes, so it started ~16:21 and compiled **pre-fix source**. That is
  independent of the contemporaneous label and agrees with it — two sources, not one.
- **`store.rs` was edited again at 17:22Z**, after every `mine-*` run had finished. Block
  H rewrites ~20 signatures around the connection types the failure lives in. T4 is a
  different experiment.

> **A denominator inherits the scope of the thing you counted in — and "runs" was counted
> across trees.**

The 11 is not nonsense; it is correct for the noun *"every run of this gate that night on
any pool-bearing tree."* That noun exists. It cannot carry the conclusion, because the
timer P0 **is a change to the durable write path** — the exact subject matter — and Block
H rewrites the connection types.

The sharpest part: this file already said T4's green was *one green run, not a fix*. That
caveat was correct **and the arithmetic beside it said otherwise**, because pooling T4
into the total let a post-Block-H run vouch for a pre-Block-H rate.

> **A correct caveat does not survive being added to a wrong total.**

The correction moves the rate from 27% to **37.5%** — i.e. against the author. It was
adjudicated by the party whose own published number it confirms, who tested the version
where they were wrong first.

## What is in each file

| file | tree | failing test(s) |
|---|---|---|
| `gate-aion-rs-3.log.gz` | T3 | `outbox_crash_recovery_rearms_stranded_done_row_and_finishes` **and** `outbox_failure_fails_fast_and_cancels_unresolved_siblings` — both in one run |
| `mine-3.log.gz` | T3 | `outbox_duplicate_completion_records_exactly_one_terminal` |
| `mine-5.log.gz` | T3 | `outbox_happy_path_completes_with_n_terminals_in_order` |
| `gate-121-t5.log.gz` | **T5** | `outbox_happy_path_completes_with_n_terminals_in_order`, `bad parameter or other API misuse` — 58 verdicts, `EXIT=101` |

**`gate-121-t5.log.gz` is recorded separately and NOT pooled**, under the same law that
produced the correction above — even though pooling it would add a red to a denominator
its author would rather keep small. The law does not get to apply only when it flatters.
Its own harness reported **exit code 0** for that run while cargo exited **101**, because
the wrapper reports the last statement's status; the `EXIT=` line in the log is the
authority, and this is the second time that trap has been caught here.

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
