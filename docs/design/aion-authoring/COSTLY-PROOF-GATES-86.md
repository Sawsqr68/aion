# The durability proofs now say when they did not run (#86)

**Built 2026-08-01. Five `#[ignore]`s removed; zero remain in the tree.**

## The rule, and why it is not pedantry

CLAUDE.md bans `#[ignore]` outright: *"`#[ignore]` on tests … are all bypasses,
not fixes. Tests that need a runtime gate it at runtime (read an env var, emit a
`tracing::info!` skip line, return `Ok(())`) — never `#[ignore]`."*

Five tests violated it. Two of them were:

- `osproc_kill9_liminal_failover_completes_exactly_once` — the **cross-node**
  kill-9 failover proof
- `osproc_kill9_parked_timer_resumes_and_completes_exactly_once` — the
  **parked-timer resume** proof

Those are the tests that establish the durability claim. **By default they did
not run, and the headline number could not say so.** The 2026-08-01 green run
reported `3952 passed` with `5 ignored` buried in per-target tallies nobody
aggregates.

## What changed, and what deliberately did not

Each is now runtime-gated on `AION_RUN_COSTLY_TESTS`, and each announces on the
**real stderr handle** which proof did not happen:

```
COSTLY_PROOF_SKIPPED (PROVES NOTHING): a kill -9 of a node in a 3-node cluster
failing over and completing the work EXACTLY ONCE — not established by this run.
Set AION_RUN_COSTLY_TESTS=1 to run it. [crates/…/lsub5b_osproc_kill9_failover_e2e.rs:168]
```

**The default is unchanged on purpose.** They still do not run. Turning a 3-node
kill-9 cluster test on by default would add many minutes to every battery and
trade one problem for a worse one. What changed is that the absence is *stated*.

The announcement names the **proof**, not the mechanism. "Slow: spawns a 3-node
cluster" tells a reader what the test does; it does not tell them which claim is
now unproven.

`announce` writes to `io::stderr()` rather than `println!`, because the harness
captures per-test output and a gated skip always passes — a captured
announcement would be invisible in exactly the case it exists for.

## 🔴 The honest cost of this shape

A runtime-gated skip returns `Ok(())`, so **it counts as a pass**. The total now
includes tests that proved nothing, which is *worse* than `#[ignore]`'s separate
tally.

That trade is only acceptable because the announcement is **checked**, not merely
printed. `gates:costly-announced` in `scripts/battery.sh` asserts every declared
costly suite emits one. Delete that gate and this conversion becomes a downgrade
— which is stated in the module docs so nobody removes it casually.

## 🔴 The inverted control found TWO real defects in my own gate

This is the part worth keeping. The mutation: silence one suite's announcement
while leaving it skipped — the precise failure the gate exists to catch.

**Defect 1 — the pattern matched build noise.** The first gate grepped the bare
token `COSTLY_PROOF_SKIPPED` over combined stdout+stderr. Removing the call site
made the constant dead, so cargo emitted a dead-code warning **quoting the source
line that defines it**:

```
pub const SKIP_TOKEN: &str = "COSTLY_PROOF_SKIPPED";
```

The count was inflated by compiler output *in exactly the case the gate exists to
detect*. It reported `4 call sites, 5 announcements` — the right alarm for
entirely the wrong reason. Fixed by anchoring to
`^COSTLY_PROOF_SKIPPED (PROVES NOTHING):`, which a quoted source line cannot
match because compilers indent them.

**Defect 2 — and this one would have shipped a false pass.** With the pattern
fixed, the gate reported `4 call sites, 4 announcements` and **PASSED**.

The baseline was derived by counting `skip_unless_opted_in(` call sites. So
deleting a gate call deleted its own expectation. The two numbers moved together
and agreed perfectly about nothing.

> **A baseline is only a control if it is INDEPENDENT of the thing it measures.**
> Counting the call sites and counting their announcements is counting the same
> fact twice.

Rewritten to assert **per declared suite**: each named target must emit at least
one announcement. The suite list is an enumeration and has to be — nothing else
in the tree says which suites own a costly proof — but the *answer* is no longer
enumerated, and silencing one suite is now caught and named:

```
COSTLY-ANNOUNCEMENT GATE FAILED: `stacked_dev_live_e2e` announced NOTHING.
1 of 5 costly suite(s) failed to announce.
```

Both arms verified: red with the mutation naming the right suite, green after
restore, and `stacked_dev_live_e2e.rs` sha256-identical before and after
(`3512cec5…`).

## Stale documentation is worse than none

Three module headers told the reader to run `-- --ignored`, which after this
change selects **zero tests**. All corrected to the `AION_RUN_COSTLY_TESTS=1`
invocation. A doc that names a command which silently does nothing is a trap with
a helpful tone.

## The five

| suite | the proof it now names |
|---|---|
| `lsub5b_osproc_failover_e2e` | a real liminal worker connecting to a real server listener over OS processes |
| `lsub5b_osproc_kill9_failover_e2e` | kill -9 of a node in a 3-node cluster, failing over, completing EXACTLY ONCE |
| `parked_timer_osproc_kill9_failover_e2e` | a timer-parked workflow surviving a hard kill, resuming, completing EXACTLY ONCE |
| `stacked_dev_live_e2e` | a change landing through a REAL worker process and returning a review signal |
| `new_scaffold_build` | the dev-pipeline template scaffolding, building, packaging, passing its own tests (needs hex.pm) |

The last one needs the **network**, not just time. Opting in on a disconnected
box is a real failure and is reported as one: the gate governs whether the
attempt happens, never whether its result is believed.
