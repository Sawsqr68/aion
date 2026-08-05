# The cancellation contract

**Status: this is what Aion does TODAY, stated so a consumer can build around
it. It is not a proposal, and it is not an endorsement.** ADR-004 has already
decided that it should change; that change is not built. Until it is, this page
is the whole contract.

Verified against the code on 2026-08-05 and against the test that pins it,
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

The route is `POST /workflows/cancel` — **body-addressed, not path-addressed**.
The JSON body carries `namespace`, `workflow_id`, an optional `run_id`, and
`reason` (`crates/aion-server/src/api/http/router.rs`, `cancel_workflow` in
`crates/aion-server/src/api/http/workflows.rs`, `CancelWorkflowRequest` in
`crates/aion-server/src/api/http/clean_dtos.rs`). It reaches `Engine::cancel`
(`crates/aion/src/engine/api.rs`) → `lifecycle::terminate::cancel`
(`crates/aion/src/lifecycle/terminate.rs`).

`cancel` opens by asking the registry for the run's live handle. A run that has
one takes the ordinary path:

1. Refuses if the run already recorded a terminal event
   (`reject_if_recorded_terminal`, checked under the handle's recorder lock so
   the check-then-record is atomic against the exit monitor).
2. Appends exactly one `WorkflowCancelled` event **to that run's own history**,
   through that handle's single recorder. No other workflow's history is written
   to, by this or any later step.
3. Retires that run's declared-timeout deadline (`time::retire_run_deadline`).
4. Kills that run's BEAM process. Runtime link propagation tears down its
   **linked activity processes** — activities only. A process that already
   exited between the record and the kill is logged and tolerated; its exit
   monitor reconciles against the recorded cancellation.
5. Updates visibility, notifies the terminal, deregisters the handle.

A run that holds **no** handle does not get refused outright; it takes the
never-alive branch below.

### Cancelling a run that was never alive

`crates/aion/src/lifecycle/cancel_never_alive.rs` is the second cancel path, for
the run that can never obtain a writer: one whose pinned package version no
longer loads, so startup recovery skipped it, so it never became resident, so
the ordinary path — which opens by taking the run's recorder — had nothing to
take. Before this branch existed such a run stayed `Running` forever with the
one lever it needed denied (#117(c)).

It does not relax the single-writer invariant; it satisfies it by a different
primitive. The registry grants a `TerminalWriterReservation` only when it can
prove under its own lock that the workflow has no other writer of any kind, and
every condition below is measured while that reservation is held:

1. **This engine recorded a residency verdict** — a reason the run could not be
   made resident. The cancellation cites it. No verdict → `NoResidencyVerdict`,
   which is deliberately *not* `WorkflowNotFound`: the run exists and its
   history is readable, and what is missing is the verdict this path must cite.
2. **A writer slot is available**, proven atomically by the registry. Occupied →
   `TerminalWriterUnavailable`.
3. **The run's pinned package does not resolve right now** — re-measured against
   the live catalog through the same resolution startup recovery uses, never
   cited from the boot-time verdict. If it resolves, the branch refuses with
   `RunIsRecoverable`: a redeploy has landed, so the ordinary path must be used.
4. **The run has recorded no terminal**, and the history's latest
   `WorkflowStarted` names the run the caller named (else `InvalidState`).

Ahead of all four, two triage refusals keep the answer honest in both
directions: an empty history is `WorkflowNotFound`, and an already-terminal run
is refused by naming the terminal — which also covers the real race where a
resident run's deadline fires and deregisters between the registry lookup and
this branch.

On success it appends `WorkflowCancelled` through the reservation, refreshes
visibility, and stops. There is nothing to deregister and nobody to notify: the
run never had a handle, so it never had a completion notifier.

Its stated limitation: the probe is package **resolution**, not a spawn. A run
that is unrecoverable because spawning fails — rather than because its version
is absent — resolves, so this path refuses it as recoverable. That is the
conservative direction, and it is deliberate: a real-spawn probe would leave a
resident process behind on a cancellation path.

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
- There is **no usable parent→children index**. A child's history does not
  record its parent (`WorkflowSummary::from_history` leaves `parent` unset and
  says so, `crates/aion-core/src/filter.rs`); the only durable record of the
  relationship is a `ChildWorkflowStarted` event inside the parent's own
  history. `WorkflowFilter` does carry a `parent` predicate, but it is
  reachable from nothing an operator can call: the HTTP list DTO drops a
  `parent` field on the floor (`crates/aion-server/src/api/http/clean_dtos.rs`,
  `WorkflowFilterDto`), there is no proto field for it, and of the three
  backends only libSQL implements it — by scanning every `ChildWorkflowStarted`
  row (`crates/aion-store-libsql/src/read.rs`, `load_parent_links`). On the
  default haematite backend and on the in-memory store the summary's `parent`
  is always `None`, so the predicate matches nothing. **Through the API and the
  CLI you cannot ask the system "what are this workflow's children".** This is
  why a cascade is not a small change: the linkage a cascade would need does
  not exist where a cascade would have to read it.
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
parent→children linkage above has to exist where the engine can read it, on
every backend, rather than as one backend's query-time scan.

Today's behaviour is deliberate **as engineered** — children are unlinked on
purpose, and that is load-bearing for recovery, since it is exactly what lets
children keep running while a parent is down. What was never decided is that
the same mechanism should also govern an operator's explicit cancel. ADR-004 is
that decision. Until it ships, `Abandon` is the contract, and this page is it.
