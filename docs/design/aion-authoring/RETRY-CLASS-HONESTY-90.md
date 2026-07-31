# #90 — a terminal record that called itself retryable

**Seat:** Vesper Lynd. **Date:** 2026-08-01.
**Status:** fix complete and mutation-proven; workspace battery + push pending.

## The observed defect

Run `f9b062c2-d260-43f9-8374-d3210330f567`, seq 20, in the durable log:

```json
{"kind":"Terminal","message":"retryable:harness reported failure: run/execute failed (code -32603)"}
```

The `kind` field and the message prefix are **two copies of one
classification, disagreeing**, in the one place in the system that can never be
rewritten. An operator reading the terminal failure of a dead workflow was
told, in the durable bytes, that the failure was retryable.

The mechanism is inheritance: the terminal record takes its message verbatim
from the last attempt, and that attempt genuinely *was* `retryable:`-classed.
The statement was true of the attempt and false of the event being written.

## Why the prefix is corrected rather than stripped

The `retryable:` prefix is not decoration. It is the wire vocabulary the retry
executor reads (`is_retryable_reason`) and the server's dispatch taxonomy
writes — the string form of the structured `ActivityErrorKind` the worker
reports. Stripping it would leave an unclassified reason where every sibling
terminal carries a class. So the classification is corrected to `terminal:`,
and the diagnostic tail is preserved verbatim.

## Three findings the sweep produced

The task as filed described one call site and proposed stripping the prefix.
Both turned out to be wrong, and neither was discovered by a second sighting —
they came from sweeping.

### 1. The first wording was itself a false mechanism claim

The first fix wrote the head `"retry budget exhausted; last attempt: "`.
Sweeping the *producers* of a retained activity error shows a
`retryable:`-classed reason reaches a `Terminal` record from **two** producers:

| producer | retry loop? |
|---|---|
| `nif_activity_retry_dispatch::spawn_completion_task` | yes — a budget genuinely was spent |
| `nif_activity_in_vm::spawn_in_vm_completion_watcher` | **no** — zero retry symbols in the file |

An in-VM activity returning a typed `error.Retryable` is delivered straight
through and never retried (this independently corroborates open defect #34,
"retryable-but-not-retried activities"). On that path the head named a
mechanism that never ran — trading #90's false claim for a new one of exactly
the same kind.

The head now reads `"no further attempts; last attempt was retryable-classed: "`,
which states only what the call site can know: a fact about the record being
written, not about the history behind it. Both halves are true on every path.
The function was renamed `exhausted_terminal_reason` →
`reclassified_terminal_reason` because the name carried the same over-claim.

### 2. There were two consumers, and the first fix treated one

`nif_collect_settlement.rs:220` is the same defect in the fan-out settlement
path: same source (`take_activity_error`), same `Terminal` kind, message
verbatim. **Every `distribute` leg settles through it.**

The consumers are exactly two, and the sweep that found them is the point:

- grepping `ActivityErrorKind::Terminal` returns ~40 hits, mostly tests, and
  would have buried it;
- grepping `take_activity_error` — the *source of the bad value* — returns two
  non-test callers, and **both were defects**.

The fix is applied at the consumers and deliberately **not** at any producer.
The consumer is the only place that knows a `Terminal` record is what is being
written, which is exactly the invariant. There are three producers (retry loop,
in-VM watcher, durable outbox); fixing any one would leave the others free to
reintroduce it.

### 3. The defect had a test defending it

`nif_activity_dispatch_tests/await_paths.rs::awaited_terminal_records_the_noted_final_attempt`
asserted the recorded terminal message was `"retryable:reset three"` — verbatim,
prefix included. **The defect was pinned as expected behaviour.** That is why it
survived review.

It went red the moment the fix landed and stayed invisible because the gate
being run was the filtered `--lib nif_activity_retry`, chosen from the module
being edited — so it could only ever see what was already known.

## Replay safety is the load-bearing property

Both consumers now compute the corrected reason **once** and use it for the
durable record *and* the value handed to workflow code.

This is not tidiness. Replay re-derives the failure by reading the recorded
event (`recorded_step` on the await path, `recorded_terminal` on the fan-out
path) rather than by re-running the settlement. A fix that corrected only the
durable record would hand live code one string and replayed code another — a
**replay divergence**, not a cosmetic inconsistency.

The unit-level expression of the same property is that
`reclassified_terminal_reason` is a **fixed point**: feeding a recorded reason
back through it must not change it again. Pinned over every class, not just the
interesting one.

## Mutation proof — five arms, all watched failing

| arm | mutation | expected | observed |
|---|---|---|---|
| A | function → identity | core pin RED, pass-through GREEN | ✅ RED, printing the real production bytes |
| B | rewrite unconditionally | core GREEN, non-vacuity pin RED | ✅ 2 RED (non-vacuity + leading-only) |
| C | second site reverted to raw | fan-out pin RED | ✅ RED on the class assertion |
| D | half-fix: record corrected, return raw | fan-out pin RED | ⚠️ RED — but on assertion 1, **not** the equality one |
| E | mirror half-fix: record raw, return corrected | equality assertion RED | ✅ RED, and left/right IS the divergence in bytes |

**Arm E exists only because arm D fired on the wrong assertion.** D killed the
test through the class check and said nothing about the equality check — it
looked like proof and was not. To prove a specific assertion load-bearing, the
mutation must be the one that leaves every *other* assertion satisfied.

Both pristine files were restored and `cmp`-verified byte-identical after every
arm.

## Method laws earned

- **A defect can have a test defending it.** A green suite is not evidence the
  behaviour is wanted; it may be evidence the behaviour was pinned.
- **A filtered gate hides its own blast radius.** The filter was derived from
  the module being edited, so it could only see what was already known — the
  same shape as a baseline derived from what it audits.
- **A half-fix arm can fire on the wrong assertion.** Mirror the half-fix.
- **A fix's search space is part of the fix — and it runs over the CONSUMERS of
  the bad value, not over sites that merely resemble the one you found.**
- **A wording that names a mechanism is a claim about history.** Prefer a fact
  about the record being written; it is the only thing a call site can know
  without knowing which producer sent it.
