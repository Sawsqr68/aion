# #90 — the fix was the defect. Reverted.

**Seat:** Vesper Lynd. **Date:** 2026-08-01.
**Status:** ⛔ **REVERTED.** `c5191054` and `3fc06d17` were built on a false
premise, caused a real regression, and never reached `origin/main`. This
document replaces the earlier version, which argued the opposite and was wrong.

> **If you are here to re-fix #90: read §3 before writing a line.** The obvious
> fix is the one that was tried, and it breaks the SDK's error taxonomy
> estate-wide.

## 1. The observation, which is real

Run `f9b062c2-d260-43f9-8374-d3210330f567`, seq 20:

```json
{"kind":"Terminal","message":"retryable:harness reported failure: run/execute failed (code -32603)"}
```

An operator reading the terminal failure of a dead workflow is told, in the
durable bytes, that the failure was retryable. That confusion is genuine and
#90 remains open on that basis.

## 2. The premise, which is false

#90 claimed these are **"two copies of one classification, disagreeing."**
They are not. They answer different questions:

| field | question it answers | consumer |
|---|---|---|
| `ActivityErrorKind` | **replay-lifecycle disposition** — will another attempt follow? | `durability/cursor.rs:343`, `nif_activity_retry.rs:362`, `durability/executor.rs:277` |
| `retryable:` prefix | **failure-domain class** the worker reported | the Gleam SDK's `parse_error` |

For the saga both are simultaneously true: the engine is done (`Terminal`) and
the failure was transient (`retryable:`). No contradiction, nothing to reconcile.

**And on this path `kind` carries no information whatsoever** —
`crates/aion/src/runtime/nif_activity.rs:262-268`:

```rust
pub(super) fn activity_error(reason: String) -> ActivityError {
    ActivityError { kind: ActivityErrorKind::Terminal, message: reason, details: None }
}
```

A hardcoded constant. **The fix rewrote the information-bearing field to agree
with the constant one.**

The codebase already said so, in the very file whose siblings the change broke
(`crates/aion/tests/order_saga_e2e.rs:421-427`):

> `// ROUGH EDGE: the in-VM dispatcher seam classifies every dispatcher failure`
> `// ActivityErrorKind::Terminal and leaves the retryable: prefix in the`
> `// recorded message ... Until the seam honours the prefix, the durable`
> `// record is identified by message, not kind.`

That comment states the invariant the fix violated. It was not read.

## 3. What actually broke — the chain, verified end to end

The `retryable:` prefix is **the Gleam SDK's typed-error wire format**. It is
not decoration and it is not merely the retry executor's vocabulary.

1. `crates/aion/tests/order_saga_e2e.rs:187` — attempt 1 fails with
   `"retryable:payment gateway unavailable (transient)"`.
2. `charge_payment` declares **no engine retry policy**
   (`examples/order-fulfillment/src/order_fulfillment.gleam:327-338`, comment:
   "Deliberately NO `activity.retry(..)` policy"). So `failure_stand_down`
   (`nif_activity_retry_dispatch.rs:142-145`) returns `Failed(reason)` verbatim
   at attempt 1. **The engine retry loop never runs.**
3. The await consumer rewrites the string (`nif_activity_await.rs:210`).
4. It crosses the NIF boundary into
   `gleam/aion_flow/src/aion/internal/activity_dispatch.gleam:87,102` →
   `parse_error`.
5. `parse_error` (`activity_dispatch.gleam:134-139`) is a **pure prefix
   classifier** — the SDK's sole decode point for activity error class:
   ```gleam
   case string.starts_with(raw, "retryable:") {
     True -> error.Retryable(message: string.drop_start(raw, 10), details: "")
     False -> case string.starts_with(raw, "terminal:") {
       True -> error.Terminal(message: string.drop_start(raw, 9), details: "")
   ```
6. The saga's retry is a **guarded pattern match on that variant**
   (`order_fulfillment.gleam:130-156`):
   ```gleam
   Error(error.Retryable(message: message, details: _)) if attempt < max_payment_attempts
     -> ... charge_with_retries(input, attempt + 1)
   Error(activity_error) -> Error(OrderFailed(stage: "charge_payment", ...))
   ```

Post-rewrite the string is `terminal:...` → `parse_error` yields
`error.Terminal` → the `error.Retryable` arm **cannot match** → the workflow
falls through to `OrderFailed` and dies at attempt 1.

The battery log decodes byte-for-byte to the fall-through arm:

```
["order_failed","charge_payment",
 "attempt 1 of 3: no further attempts; last attempt was retryable-classed: payment gateway unavailable (transient)"]
```

**Scope.** `failure_stand_down` returns the reason verbatim whenever there is
no engine policy — the SDK's documented run-exactly-once default. So the
regression was not confined to retry exhaustion: **every single-attempt
retryable activity failure in the system was delivered to workflow code as
Terminal, making `error.Retryable` unreachable from a remote activity.** The
same break applied to every `collect_all` / `collect_race` / `distribute` leg
via `concurrency.gleam:48,87`.

**Replay makes it durable, not just live.** `recorded_step`
(`nif_activity_await.rs:168`) and `recorded_terminal`
(`nif_collect_settlement.rs:312`) return the *recorded* `error.message` to
workflow code. Record and delivered value are one channel — which also
forecloses "fix only the durable record". It would have introduced version
skew: histories written before the change replaying as `Retryable`, identical
runs after it replaying as `Terminal`.

## 4. The evidence

Cold full-workspace battery, `WORKSPACE_EXIT=101`, 243 verdicts, targets 240 /
UNPAIRED 0 / IGNORED 0, 4018 passed, **9 failed** — 8 of them this regression.

After reverting the seven source files to `origin/main`:

```
cargo test -p aion-rs --lib --test invm_activity_e2e --test order_saga_e2e \
                            --test activity_retry_e2e --test advisory_class_e2e
EXIT 0 · 5 targets · UNPAIRED 0 · FAILED 0
622 + 11 + 3 + 7 + 12 passed / 0 failed / 0 ignored
```

All four named failures pass by name. Absence of the symbol was paired with a
survival check under the same grep: `reclassified_terminal_reason` → 0 hits,
control `is_retryable_reason` → 13 hits.

## 5. What a correct fix looks like

**(a) Nothing durable or workflow-visible may be rewritten at the consumers.**
The message is a contract.

**(b) If the operator-honesty complaint is to be answered in the bytes, put the
new fact in a field with no contract on it.** `ActivityError.details` is `None`
on every one of these records. A structured
`{"final": true, "engine_disposition": "no_further_attempts"}` there leaves
`kind` its replay meaning and `message` its SDK-wire meaning, and the `attempt`
field already on the event supplies the count.

**(c) The real fix is the rough edge itself** — `ActivityErrorKind` should mean
*disposition* explicitly, with the domain class derived into a separate field
at the seam, rather than a hardcoded constant that invites this exact
misreading. Note the constraint that makes this a design decision and not a
patch: `kind` cannot simply be set to `Retryable`, because replay termination
depends on a final failure being non-retryable (`cursor.rs:343`,
`executor.rs:277` `ensure_terminal_activity_error`). Disposition and domain
class genuinely conflict in one field; separating them is the work.

## 6. Method laws earned

**🔴 WHEN TWO FIELDS DISAGREE, ESTABLISH WHICH ONE CARRIES INFORMATION BEFORE
CORRECTING EITHER.** One of them may be a hardcoded constant. Harmonising the
informative field toward the constant one destroys the only signal present
while looking exactly like a consistency fix.

**🔴 A "DUPLICATED" FIELD MAY BE TWO ANSWERS TO TWO QUESTIONS.** Before calling
redundancy a defect, name the question each field answers and find its
consumer. `kind` and the prefix had different consumers in different languages.

**🔴 A STRING THAT CROSSES A LANGUAGE BOUNDARY IS A WIRE FORMAT.** It has a
parser, and the parser has callers whose behaviour is a pattern match. Grepping
Rust for consumers of the bad value was necessary and **not sufficient** — the
decisive consumer was Gleam, one layer out. *A fix's search space must follow
the value across every boundary it crosses, not merely to the edge of the
language you are editing.*

**🔴 THE MIRROR OF "A DEFECT CAN HAVE A TEST DEFENDING IT": A CONTRACT CAN LOOK
LIKE A STALE PIN.** Eight tests went red. Three phrased their assertion with
the word "verbatim" and I judged them *probably* stale expectation-pins. They
were not: each pinned the wire contract, one layer below the saga. Had I bulk-
updated them, the suite would have gone green and the estate-wide regression
would have shipped **with its own gate rewritten to endorse it.** The failing
tests were right and the change was wrong.

**🔴 A ROUGH-EDGE COMMENT IS A LOAD-BEARING SPECIFICATION.** The constraint
violated here was written down, in the test file adjacent to the change, and
read only afterwards. Grep the tests for prose about the field you are about to
alter.

**Retained from the earlier version, still true:** a filtered gate hides its own
blast radius, and a half-fix mutation arm can fire on the wrong assertion. Note
what they did **not** buy: five mutation arms proved the change was internally
consistent and load-bearing. **A mutation proof shows a change is wired in. It
cannot tell you the change is correct** — every arm was measuring against the
premise instead of testing it.
