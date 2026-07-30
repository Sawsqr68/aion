# The cancellation contract

**Status: this is what Aion does TODAY, stated so a consumer can build around
it. It is not a proposal, and it is not an endorsement.** ADR-004 has already
decided that it should change; that change is not built. Until it is, this page
is the whole contract.

Verified against the code on 2026-07-31 and against the test that pins it,
`cancelling_level_two_orphans_level_three_and_fails_level_one_await`
(`crates/aion/tests/nested_workflows_e2e.rs`).

## The short version

**Cancelling a workflow cancels that workflow. Nothing else.**

Its activities die with it. Its child workflows do not. Every child — declared,
implicitly fanned out, or detached — carries on as if nothing happened, runs to
its own terminal, and records that terminal in its own history.

In ADR-004's vocabulary, **every child is `Abandon`, always, and the policy is
not selectable.**

## In detail

### What cancel does

`POST /workflows/{id}/cancel` → `Engine::cancel` → `lifecycle::terminate::cancel`
(`crates/aion/src/lifecycle/terminate.rs:105-144`):

1. Refuses if the run is already terminal.
2. Appends exactly one `WorkflowCancelled` event **to that run's own history**.
   No other workflow's history is written to, by this or any later step.
3. Retires that run's declared-timeout deadline.
4. Kills that run's BEAM process. Runtime link propagation tears down its
   **linked activity processes** — activities only.
5. Updates visibility, notifies the terminal, deregisters the handle.

### What happens to children

Nothing. Concretely, for a cancelled parent `P` with a child `C`:

| | |
|---|---|
| `C`'s history | untouched — no event is appended on `P`'s account |
| `C`'s status | unchanged; it keeps running and completes on its own |
| `C`'s activities, timers, signals | unaffected |
| `C`'s eventual terminal | recorded in `C`'s history and **nowhere else** |
| `P`'s history after `C` finishes | still frozen at `WorkflowCancelled` |

This holds identically for all three kinds of child. There is no distinction in
the engine between a declared `child`, an implicit fan-out child, and a
detached `spawn`: all three compile to the same `workflow.spawn`, and none of
them is process-linked to the parent (`crates/aion/src/child/spawn.rs`).

### What a parent sees when its CHILD is cancelled

The other direction is wired, with one sharp edge worth knowing:

A parent awaiting a child that gets cancelled records
`ChildWorkflowFailed`, with the message `cancelled:<reason>`. **It does not
record a cancellation.** There is no separate cancelled taxonomy on the parent
side — a cancelled child and a failed child arrive at the parent as the same
event kind, distinguishable only by a string prefix. If you are branching on
that, branch on the prefix.

### What is NOT true

- There is **no reaper**. Nothing notices an orphaned child, times it out, or
  cleans it up later. A grandchild parked on a three-month timer outlives its
  cancelled tree by three months.
- There is **no parent→children index**. A child's history does not record its
  parent, and the visibility store cannot filter by parent. The only durable
  record of the relationship is a `ChildWorkflowStarted` event inside the
  parent's own history. **You cannot ask the system "what are this workflow's
  children".** This is why a cascade is not a small change: the linkage a
  cascade would need does not exist yet.
- `Event::ChildWorkflowCancelled` exists in the event enum and is **never
  recorded**. Do not match on it expecting traffic.

## Building around this today

If you need a cancel that stops a tree, you have to own the tree yourself: keep
your own parent→children record outside Aion and cancel each child explicitly.
The per-child cancel works exactly as documented above — it is only the
propagation that is missing, not the mechanism.

If you are surfacing workflow state to people, this is the sharp bit: cancel a
parent and the UI keeps showing children as `Running`, correctly, forever. That
is not a stale view. They really are running.

## Where this is going, and why it is not there yet

**ADR-004** (`docs/design/DECISIONS.md`, 2026-06-13, decided by Tom) rules the
target design: a Temporal-style per-spawn **parent-close policy** —
`RequestCancel` (graceful cascade), `Terminate` (immediate kill), `Abandon`
(today's behaviour, made explicit) — as a **required** argument on child spawn.
Defaulting to any of the three was rejected outright, per ADR-001: the engine
provides the mechanism, the author states the intent.

Its stated consequences are why it is not a weekend's work: propagation must
fire on **all** parent terminals rather than only cancel, must recurse, and
recovery must re-arm pending propagations after a restart. Plus the
parent→children linkage above has to exist first.

Today's behaviour is deliberate **as engineered** — children are unlinked on
purpose, and that is load-bearing for recovery, since it is exactly what lets
children keep running while a parent is down. What was never decided is that
the same mechanism should also govern an operator's explicit cancel. ADR-004 is
that decision. Until it ships, `Abandon` is the contract, and this page is it.
