# Gate record — aion#105, query seam resolves by the CURRENT run

## Verdict — 24 declared, 24 executed, 24 PASS

Battery run 2026-08-20 23:24 → 2026-08-21 00:13 AEST (~49 min) at frozen HEAD
**`6b9cc01a20c81b1521c698f1865a8b20088cc3cb`**, parent
**`a510c72522348f9a52fa589a46bf4eb8b66b4f1f`**, in `.worktrees/105-run-aware-seam`.
Runner: `bash scripts/battery.sh` — the repository's own runner invoked its own
way, not a hand-rolled reproduction. Full log: `battery-20260820.log`.

```
0 of 24 gates failed.
```

HEAD is stamped **9×** in the log and was **unchanged at the finish**.

**Declaration verified**, which the runner correctly refuses to claim for itself
(*"NOT VERIFIED YET … that they are the legs gates.json declares is a separate
claim"*). Ran `scripts/gates/verify-declaration.sh` against this log:

```
declared    : 24 legs
executed    : 24 legs
VERDICT: exact match — all 24 declared legs executed, and nothing else.
```

`rc 0`. Its stated limit is carried here rather than dropped: *names only — this
says every leg RAN, not that any leg TESTS what it is named after.*

## Test totals

628 `test result:` lines, **11,339 passed, 0 failed**. **Zero ignored anywhere** —
no result line carries a non-zero `ignored` count.

### The total reconciles exactly against the previous gate

The `gates-lib-posix` gate recorded **11,325** at a frozen base that predated
PR #106 (aion#94). The delta is fully accounted for:

| source | tests | legs | lines |
|---|---|---|---|
| aion#94's pins (in this tree, absent from that one) | 4 | 2 | 8 |
| aion#105's pins (this branch) | 3 | 2 | 6 |

11,325 + 8 + 6 = **11,339**. The arithmetic closes with nothing left over, which
independently confirms #94's four pins are present and green here — each read off
its own `... ok` line, twice.

## The three #105 pins ran in BOTH test legs

Read off the individual lines by name, not inferred from an aggregate — 6 `ok`
lines = 3 tests × 2 legs (`rust:test`, `rust:test-all-features`):

- `runtime::nif_query_mailbox::tests::a_continued_workflow_resolves_to_its_current_run`
- `runtime::nif_query_mailbox::tests::a_stale_predecessor_handle_without_an_index_entry_is_unknown`
- `runtime::nif_query_mailbox::tests::the_live_run_index_follows_registration_order`

## ⚠️ Two numbers that read wrong at a glance

Both are recorded with their explanation so nobody re-derives them and reaches
the wrong conclusion.

**25 banner lines for 24 legs.** The 25th, at line 17393, is
`=== must-be-claimed gate ===`, printed by `check_must_be_claimed.sh` **inside**
the `estate-guard` leg — a child banner, not a leg. Same class as the
630-vs-628 reconciliation under `gate-logs/94-reconcile/`.

**A `skip` search returns 2 hits, and neither is a skip.** Both are the same
**test name** containing the word — `durability::cursor::tests::
resolve_child_terminal_reports_mismatch_without_skipping_in_strict_replay` — and
both report `... ok`. There are **no runtime-gated skip emissions** in this run.
Counting a word is not counting the thing the word names.

## What was gated

Two files, both under `crates/aion/src/runtime/`:

- `nif_query_mailbox.rs` — `resolve_workflow` consults `live_run_pid` (which run
  is current) instead of scanning `Registry::list` (does any handle exist).
- `nif_query_mailbox_tests.rs` — three pins, new file.

**Mutation check, repeated 20×**, against the **literal prior text** from
`origin/main` (byte-compared to the replaced function, not paraphrased from the
fix). The defect rides `HashMap` iteration under `RandomState`, so one run is a
coin toss rather than evidence:

| test | red |
|---|---|
| `a_continued_workflow_resolves_to_its_current_run` | **7 / 20** |
| `a_stale_predecessor_handle_without_an_index_entry_is_unknown` | **20 / 20** |
| `the_live_run_index_follows_registration_order` | **12 / 20** |
| **suite** | **20 / 20** |

The 7/20 is why the repeat was required: a single-shot mutation had roughly a
**65% chance of coming back green**, which reads exactly like a vacuous test.
Restored by writing saved bytes, sha-verified equal, then five consecutive green
runs — the green's *return* being itself the proof the binary rebuilt rather than
a stale mutant answering.

Full run log committed beside this file as `mutation-20x.log` — all 20
iterations, each with its own `running 3 tests` line and individual
`ok` / `FAILED` lines, so the counts above can be re-derived rather than taken on
trust.

⚠️ That log carries `ITER n RC=` lines which captured the trailing **`grep`'s**
exit status, not `cargo test`'s. They are meaningless and the file says so at its
foot. The verdict comes from the result lines.

## 🔴 This gate does not close #105

#105 is a defect at **eight sites**; this branch fixes **one**. Five of the eight
implement the same trait method, whose signature cannot express a run — four of
those five still scan `Registry::list()` after this lands. The issue stays open
with the census on it. See the PR body for the full table and for the two sites
singled out for a harder look (one of which I measured and **withdrew**).

## Venue and the disk decision

`VENUE-NOTES.md` beside this file records the box, the deps install, and why
63 GiB had to be reclaimed before arming.

**A second reclaim was made mid-run**, when free space hit 19 GiB with a
crate-building leg still ahead: `.worktrees/integration-20260819/target` (13 GiB),
taking free space to 31 GiB. Gated at the moment of action — branch confirmed
merged into `origin/main` with nothing unpushed, and **all three `gate*.pid`
files in that tree confirmed dead**, because a merged branch says nothing about
whether something is still running in it. The nine uncommitted files in that
worktree were verified surviving; only build artifact was removed. Free space at
finish: 26 GiB.

## 🔴 Land by merge, never rebase

This attestation names a **(commit, parent)** pair — `6b9cc01a2` on
`a510c72522348f9a52fa589a46bf4eb8b66b4f1f`. A rebase mints new shas and voids it.

**The record commit sits ON TOP of the gated commit**, so the PR head is not the
commit that was gated. That is stated rather than hidden, and "docs-only" was
**settled, not asserted**: no leg in `gates.json` mentions `gate-logs`, checked by
parsing the declaration, with a positive control on the same search (it finds
`crates` in `rust:preserve-order-pins`, so the search reaches leg bodies and the
zero is a true negative).

## What this gate does not say

It does not say the estate is green. `estate-guard` passed, and its own output
says why that is narrower than it looks: the population it measures is a list of
repositories, and it is silent about the repository being landed in unless that
repository is on the list.
