# #121 — both recovery-loop guards, reached for the first time

**Status: BUILT AND CONTROLLED.** Two guards that no test could reach are now
covered by two tests that have each been watched failing at my own hand.

---

## The finding, restated exactly

`repopulate_active_workflows` (`crates/aion/src/engine/startup.rs`) iterates
`store.list_active()`, re-reads each run's history, and refuses two classes:

```rust
if projected_status.is_terminal() { … continue }          // :197
if projected_status == WorkflowStatus::Paused { continue } // :212, #204
```

Both were covered by **nothing**. Measured, not inferred: `if false && …` on each
guard left `cargo test -p aion-rs --all-targets --no-fail-fast` at EXIT 0, 1022
passed, 0 failed.

The reason is structural and worth naming, because it generalises:

> **A second line of defence behind a total filter reads as covered because the
> OUTCOME is always right.**

Every backend filters `list_active` to projected status `Running` —
`InMemoryStore` at `memory.rs:374-378`, `LibSqlStore` at `read.rs:133`, and the
haematite backend in its own read path. So a terminal or `Paused` run **cannot
reach the loop through any honest store**. The guard never fires; deleting it
changes no observable outcome; every test still passes. The filter is correct and
is not the problem — its correctness is precisely what hides the second layer.

---

## The instrument

`aion_store::testing::StaleActiveListStore` — a delegating double over
`InMemoryStore` that lies in **exactly one place**. `force_active(id)` puts an id
into the `list_active` answer whatever its history projects; everything else,
including the per-run `read_history` the reader defends itself with, delegates to
a real store. The reader then faces a genuinely inconsistent store.

Both forced states are reachable in production, by **different** routes:

| forced state | production route |
|---|---|
| terminal | a backend whose active-set filter is wrong, stale, or racing a concurrent terminal write |
| `Paused` | **no filter bug needed** — `list_active` is a SNAPSHOT, and a run paused between that snapshot and the per-run re-read was legitimately active when listed (#204). The double reproduces that interleaving deterministically, with no race to lose. |

The double carries **five tests of its own**, because an instrument has to be
proven before anything is measured with it. The load-bearing one is the control:
`forcing_nothing_leaves_the_answer_identical_to_the_inner_store`. A double that
lied unconditionally would pass every other test in the file — that one is what
distinguishes it from a double that lies only when asked.

`testing.rs` became `testing/` (`shard_seam_spy.rs` + `stale_active_list.rs`)
rather than growing to ~440 lines with two doubles of different kinds in it.

---

## The controls — the only step that has ever caught a vacuous test here

`startup.rs` sha256 before mutation:
`0759b697afe80fae80a5758c37b251b8eec3923baff4b750b8e334ed5613f3d7`
— **matches the pin recorded in #121**, so this is the same tree that was
measured, not a lookalike.

| mutation | terminal test | paused test |
|---|---|---|
| `if false && projected_status.is_terminal()` | 🔴 **FAILED** | ✅ ok |
| `if false && projected_status == Paused` | ✅ ok | 🔴 **FAILED** |
| restored | ✅ ok | ✅ ok |

Each control kills **exactly its own test** and leaves the other green. That
cross-check is what proves the two tests are testing two different guards rather
than one of them covering both by accident.

Restored sha256 is byte-identical to the pin, and `grep "if false"` returns
nothing.

The failure messages, quoted from the runs:

```
a terminal run must never be made resident even when the store insists it is
active: residency mints a writer for a run whose history is closed

a durably-paused run must NOT be respawned (#204 GATE-2): the dispatch hold
rebuilt from list_paused keeps its outbox rows held, and an operator `resume`
is the only thing that may bring it back
```

Both tests carry a **healthy control run** through the same boot path. Without
it, "not resident" is satisfied by a loop that recovered nothing at all —
including one that never ran.

---

## 🔴 What this block found on the way — a gate that ran by accident

`aion-store`'s `testing` module was `#[cfg(feature = "test-support")]`. Measured:

```
cargo test -p aion-store --lib   →  0 of the new tests listed
cargo test --workspace --lib     →  5 of the new tests listed
```

The doubles' own tests were **invisible to their own crate's battery** and ran
only because a dependent's feature selection happened to unify the feature in.
That is a gate that exists and runs by accident — the #74/#104 class.

This was not reasoned out; it was measured. The answer depends on Cargo's feature
unification, not on anything visible in the source. Fixed at the source:

```rust
#[cfg(any(test, feature = "test-support"))]
pub mod testing;
```

`cargo test -p aion-store` went **58 → 63**. Dependents still opt in explicitly.

**The general shape, for the sweep:** any module gated on a feature that only
dependents enable has tests that its own crate never runs. `-p <crate>` and
`--workspace` resolve different feature sets; they are different runs and both
are required.

---

## Gates

| gate | result |
|---|---|
| `cargo clippy -p aion-store -p aion-rs --all-targets --all-features -- -D warnings` | EXIT 0, 0 warnings |
| `cargo fmt --check` | EXIT 0 |
| `cargo test -p aion-store --all-targets` | EXIT 0, 63 passed / 0 failed |
| `cargo test -p aion-rs --all-targets --no-fail-fast` | **EXIT=101**, 58 verdicts, 1 FAILED — see below |

### 🔴 The dependents gate came back RED, and the red is #85

```
test outbox_happy_path_completes_with_n_terminals_in_order ... FAILED
Error: Backend("SQLite failure: `bad parameter or other API misuse`")
```

58 verdict lines, so the run happened in full. Both #121 tests passed:

```
a_terminal_run_forced_into_the_active_list_is_never_made_resident ... ok
a_paused_run_forced_into_the_active_list_is_never_made_resident ... ok
```

That failure is #85's recorded flake (3) — predicted test, predicted error string —
on a change that is a pure test addition plus one `cfg` widening. It is not a
regression from this block.

**It is recorded as its own tree (T5) and NOT pooled into #85's 3-in-8**, under the
law that produced tonight's denominator correction: T5 is T3 + Block H + these tests
+ the `cfg` change. Pooling it would have *helped* the author by adding a red to a
rate they would rather keep small. The law does not get to apply only when it flatters.

Evidence: `evidence/85-outbox-flake/gate-121-t5.log.gz`.

⚠️ The harness reported **exit code 0** for this run while cargo exited **101** — the
wrapper reports the last statement's status. The `EXIT=` line inside the log is the
authority. Second time tonight.

Formatting was run as its own command block, never alongside a gate — the one
unexplained red of the previous block came from a run where `cargo fmt` and
`cargo test` shared a block.

---

## What this does NOT do

- It does not touch **#85**. Any `outbox_e2e` red in the dependents gate is that
  open flake, at its recorded 3-in-8 rate on the Block G final tree (37.5%), not a
  regression from this change —
  which is a pure test addition plus one `cfg` widening. Name the test and add
  the run to #85's denominator either way.
- It does not sweep for **other** second-lines-of-defence behind total filters.
  That shape is now named and the instrument to test it exists, but the sweep is
  not done. Stated as outstanding rather than implied complete.
