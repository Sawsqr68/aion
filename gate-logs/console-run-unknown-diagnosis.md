# Ops console: "run unknown" on every attempt row — diagnosis and fix shape

**Reported by Tom, 2026-08-19 06:09Z**, from his live console:

```
activity 945 · attempt 1 · run unknown
```

## What it is

Not a rendering glitch, and not cosmetic. `AttemptNavigator.tsx:209-217` renders
that marker whenever a row's `runId` is `null`, with a tooltip that is *accurate*:

> "No `WorkflowStarted` for this attempt is inside the loaded history window, so
> its generation cannot be named — load earlier history to address its
> transcript."

The run is derived, not read. `lib/attemptNavigator.ts`:

- `deriveGenerations(timeline)` scans **the loaded window** for `WorkflowStarted`
  events and collects `(seq, run_id)` boundaries.
- `generationAt(generations, seq)` returns the last boundary at or before `seq`,
  and `null` when the window holds no start at or before it.

`WorkflowDetailView.tsx` feeds it `live.events`, which comes from
`useWorkflowHistory` — a **paginated tail window** (`hasEarlier`,
`loadEarlier`, `isFetchingEarlier`).

## Why it fires on everything

Measured against Tom's live watcher rather than reasoned about:

```
$ aion describe 9a53974f-0c12-44b5-88c2-4a8de44dc957
history=5919  WorkflowStarted=1  first_type=WorkflowStarted
```

One generation, and its `WorkflowStarted` is at the very head of 5,919 events.
The default view is the tail. **The start event is never in the window, so every
attempt row on this workflow resolves to `null` and every row says "run
unknown".**

The longer a workflow runs — which is the whole point of a durable engine — the
more certain this is. It is not an edge case; it is the normal case for the
product's flagship workload.

## Why this is worse than either a lie or a silence

⭐ **A label that admits ignorance on every row trains the operator to ignore the
column.** It is honest and useless at once, which is worse than either alone: an
accurate warning that fires unconditionally is a silent fallback wearing
honesty's clothes. Tom noticed it as noise, which is exactly the failure.

Related law already held: *never document a limitation a fix could close.* The
tooltip documents one — "load earlier history" — that the server can simply
answer.

## Why the current shape can't be patched in place

`DescribeWorkflowResponse` (`crates/aion-core/src/describe.rs`) returns
`summary` + the **whole** `history`, so the data exists server-side. But:

- `WorkflowSummary` (`crates/aion-core/src/filter.rs:68`) carries **no run at
  all** — verified on the wire: `ended_at parent started_at status workflow_id
  workflow_type`.
- Loading the entire 5,919-event history into the console just to name a run is
  not a fix, it is the cost the pagination exists to avoid.
- Always fetching the head page as well is a workaround that grows with
  continue-as-new chains and leaves the derivation in place to degrade again.

## The fix — subtraction, not another checker

**The server projects the generation boundaries; the console stops deriving
them.**

1. `aion-core` — add the boundary list (`seq` + `run_id`, ascending) to the
   describe projection, computed from authoritative history. It is small even
   for a long continue-as-new chain: one entry per `WorkflowStarted`.
2. `aion-server` — populate it at the handler, from the same authoritative
   history the summary is projected from.
3. `aion-ops-console` — `deriveGenerations`'s window scan is **deleted**;
   `generationAt` resolves against the server's list. `row.runId` can no longer
   be `null`, so the `run unknown` branch, its `data-testid`, and its tooltip are
   deleted too, along with the `selectedRow.runId === null` guard at
   `AttemptNavigator.tsx:144`.

The point of the shape: it removes a truth derived from whatever happened to be
loaded and replaces it with the one the engine already owns. One truth, one
place. The marker cannot reappear because the state that produces it stops
existing — as opposed to being hidden, which would leave the same defect behind a
nicer surface.

**Tests:** `__tests__/attemptRowList.test.tsx:72` asserts `'run unknown'` is
rendered. That assertion goes with the branch. Per *narrowing makes downstream
tests vacuous*, the replacement is not "assert it's absent" — it is a case that
would have shown `null` under the old derivation (a tail window with no start)
and now resolves to the correct run.

## Status

**Diagnosed, not started.** Queued behind the engine deploy deliberately, so the
gate does not open onto a half-finished tree. Tom has the plain-language version.
