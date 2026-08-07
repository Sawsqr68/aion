# RULING — completion-retry bound and loudness

**Ruled by:** Waffles the Terrible, 2026-08-06T10:25:45Z (+0000)
**Requested by:** Vesper, out of the #7 completion-monitor lane (M2/Mo3)
**Status:** ACCEPTED — binding on the #7 lane. Tom holds override on the 30 s
value; it goes in Waffles' next digest to him, **not** as a blocking ask.

This file exists because the reasoning is the load-bearing part. The numbers
without the reasoning would read as an arbitrary default the next time someone
touches this knob — which is precisely the drift that made the ruling necessary.

---

## What was asked

`CompletionRetryConfig` shipped at 1 ms initial / 8 ms ceiling with **no attempt
budget and no wall-clock bound**, terminated only by success, an unretryable
classification, or the engine-task epoch closing. Roughly **125 attempts per
second, indefinitely**, each doing several history reads, against a store that is
by definition already unwell, on the **single-worker** engine-task executor.

The values were `SignalDeliveryConfig`'s, kept verbatim when the knob was split
out so no intermediate revision altered behaviour — but **the bound was not
borrowed with the numbers**: `SignalDeliveryConfig` pairs its ladder with
`max_enqueue_attempts: 8` and gives up. Nothing at HEAD retries a completion at
all, so measured against the tree this lands on, the ladder is **introduced by
this change**, not inherited.

Verified at my own hand, not recalled: `completion_retry` / `CompletionRetryConfig`
grep to **0 hits** across `crates/aion-server/src` and `crates/aion-cli/src`.
Same for `signal_delivery` / `SignalDeliveryConfig` — **0 hits**. The knob is
reachable only from `EngineBuilder`. **An operator running `aion server` cannot
change it**, so the shipped default *is* the answer and cannot be configured
around.

---

## THE RULING

### 1. Ceiling moves to seconds — exponential, 1 ms initial → **30 s** ceiling

> "The 8 ms ceiling is the real defect in the shipped shape — 125 attempts/sec
> forever, each costing several history reads against a store that is already
> unwell, on the single-worker executor, is a runaway that also **starves
> everything else on that executor**. At 30 s a stuck retry costs two reads a
> minute — idle-cost compliant — and the 1 ms start still recovers instantly
> from a transient."

30 s is **precedent, not invention**: it is the value ruled for DR-001's R4.

### 2. **No attempt budget — deliberate, not omission**

> "A completion is truth trying to land in the record. `SignalDeliveryConfig`'s
> `max_enqueue_attempts: 8` is sound BECAUSE delivery is re-drivable — something
> upstream will try again. **A completion has no re-driver:** if the loop gives
> up, the completion doesn't land late, it *never* lands, and the workflow
> wedges silently — which converts a transient store outage into **permanent
> silent data loss**. That's a worse failure than the runaway the budget would
> prevent, and the runaway is already cured by the ceiling."

The epoch close stays the **only** terminator.

🔴 **The condition on ever adding one:** whoever wants a budget "owes the design
a place to PUT the abandoned completion first; until that place exists, giving up
is a silent fallback and we don't ship those."

### 3. The price of no-budget is **loudness** — part of the ruling, not a suggestion

> "Once the ladder reaches its ceiling, the loop must state itself — **attempt
> count, elapsed time, last error classification** — at **warn** level on a rate
> that doesn't flood (each ceiling-interval attempt is fine; it's one line per
> 30 s). A retry loop we've ruled unbounded must never be invisible."

This is the divergence surface for the knob.

### 4. A server-side config surface is a **follow-on lane, not a rider on #7**

> "Neither this knob nor `SignalDeliveryConfig` being reachable from `aion
> server` means the shipped answer to every such question is the default — worth
> its own brief so it gets a config surface **deliberately rather than as a
> bolt-on**."

Boarded at my seat; Waffles takes it in the next dispatch round. See
`BRIEF-ENGINE-CONFIG-SERVER-SURFACE.md`.

---

## IMPLEMENTATION OBLIGATIONS ON THE #7 LANE

| # | Obligation | Where |
|---|---|---|
| O1 | `CompletionRetryConfig::default()` → `new(1 ms, 30 s)` | `crates/aion/src/runtime/config.rs` |
| O2 | Rewrite the `Default` doc comment: it currently states the gap as *unruled and shipping anyway*. That becomes **false** the moment this lands. It must instead carry the ruling and its reasoning, and cite this file. | same |
| O3 | Ceiling-reached loudness: `warn`-level, one line per ceiling-interval attempt, carrying **attempt count**, **elapsed time**, and **last error classification** | `crates/aion/src/lifecycle/completion_retry.rs` |
| O4 | The no-budget decision recorded **in the code** as ruled-and-deliberate, with the "no re-driver / permanent silent data loss" reason and the somewhere-to-put-it precondition | `config.rs` and/or `completion_retry.rs` |
| O5 | A test that pins O3: at the ceiling the loop **emits**, and the emission carries all three facts | `crates/aion/src/lifecycle/completion.rs` tests — `at_the_ceiling_every_failing_attempt_states_itself` |

🔴 **O5 must be mutation-proven like everything else in this lane** — and the
mutation must be the **literal absence** of the emission, not a variant built out
of the new logging machinery. See the standing law: a mutation constructed from
the fix's own new machinery is not a reintroduction of the defect.

**AMENDMENT 2026-08-07 — O5's location.** As drafted, this row said
`completion_retry.rs` tests. That file has no `#[cfg(test)]` module at all, so
the row named a place the obligation could not be discharged and a reader
checking it would have concluded O5 was unbuilt. The test lives in
`completion.rs`'s test module, alongside the fixtures that drive the retry loop,
and the row above now says so. Amended rather than quietly corrected, because a
ratified obligation is discharged by an amendment naming the change, never by
editing the record to look as though it always said this.

⚠️ O3's rate is **"one line per ceiling-interval attempt"** — that is a
consequence of the ladder already being at 30 s, **not a rate limiter**. Do not
implement a separate throttle; that would be an invented cap and CLAUDE.md
forbids it.

---

## SEQUENCING (why this file exists instead of the code changing right now)

Review 8 (Opus) was in flight over `config.rs`, `completion_retry.rs` and
`completion.rs` when this ruling arrived. Editing those files mid-review would
have made every finding a verdict against a tree that no longer exists —
a contended review measures nothing. **Order:** review 8 reports → apply O1–O5
→ re-run the full unfiltered battery + workspace clippy → mutation-prove O5 →
land.
