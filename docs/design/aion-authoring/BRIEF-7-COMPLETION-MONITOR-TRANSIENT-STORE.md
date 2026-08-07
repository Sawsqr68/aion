# Brief #7 — a transient store error wedges a finished run as `Running`

**Filed against:** issue #7, triaged in `ISSUE-TRIAGE-2026-08-06.md` (verdict:
TRUE, NOT STALE — the strongest of the five and undefended).
**Seat:** Vesper. **Tree at design:** `9189ba585`.

## The defect, as read at the tree

`completion.rs:77` reads history with `?`. A transient store error propagates out
of `handle_process_exit_async`, out of `handle_process_exit`, to the monitor
callback, which logs `tracing::error!` and returns. **The terminal event is never
appended.** Because status is a projection (invariant 4), the run then projects
`Running` forever — it has finished, and nothing in the system says so.

`monitor_process` takes `FnOnce` and the exit record is retired unconditionally
after the callback returns (`monitor.rs:341-350`), so there is no re-arm.

**The honest severity**, which the issue overstates: "forever" holds for the life
of the server process. `repopulate_active_workflows` re-spawns anything
projecting `Running` on restart, which installs a fresh monitor. So a single
transient store error wedges the run **until the node restarts**, with no
operator-visible signal in between beyond one `error!` line.

## Two findings the triage did not have

### 1. There are TWO callers, not one

The triage named `start.rs:574`. There is a second, identical swallowing log at
`engine/startup.rs:446` — the recovered-workflow monitor installed by the startup
sweep. Both log and return.

This is the #147 shape exactly: **one rule maintained in two places has already
drifted, or it will.** The cure is the same — the retry belongs INSIDE
`handle_process_exit`, which both callers share, not in either caller. Neither
caller may keep a retry of its own.

### 2. 🔴 THE OBVIOUS FIX IS STRICTLY WORSE THAN THE DEFECT

Process-exit callbacks are not dispatched per-process. They are drained by **one
dedicated thread** — `aion-process-exit-callback`
(`process_exit_callback.rs:49-54`) — pulling from a bounded `sync_channel` and
invoking each callback **serially** (`run_worker`, lines 140-145).

`handle_process_exit` is synchronous and `block_on`s the async body on that
thread.

⇒ A retry-with-backoff loop placed inside `handle_process_exit` — the shape the
precedent uses, and the shape the issue implies — **would hold the single
dispatcher thread for the whole backoff**. Every other workflow's completion on
the node queues behind one unlucky run's flaky store. That converts a
**one-run wedge** into a **whole-node completion wedge**.

That is #6's head-of-line block, one directory over, and it would be introduced
by the fix rather than found in the code.

**The precedent at `nif_child_watch.rs:93` is safe only because `run_watch`
already runs as a spawned task**, not on the dispatcher thread. Copying its loop
without copying its execution context copies the wrong half.

⇒ **A RETRY MUST NOT RUN WHERE THE THING IT RETRIES WAS DISPATCHED FROM.** The
backoff is the point of the retry and the dispatcher is shared, so the two are
incompatible by construction. Read the precedent's *placement*, not just its
control flow.

## Where the retry may run

Not on `ProcessExitContext::tokio_handle`. That is the host runtime, chosen to be
epoch-stable for the *callback*, and a task spawned there is **not aborted and
awaited at engine shutdown**. `engine_tasks.rs:1-15` states the hazard for
exactly this class of task: a task still running after shutdown can double-write
a history that a successor engine instance over the same store also records into
— a second writer, violating invariant 3 (single writer per workflow), whose
symptom is `SequenceConflict`.

The retry appends a terminal. It is precisely that class.

⇒ It must run on an executor whose shutdown **aborts and awaits**, which is what
`ChildTaskRuntime` is. That module is already a general engine-task registry with
two kinds (child-terminal watchers, spawn-recovery); this is a third.

## Classification: what may be retried

There is no `StoreError::is_retryable`. The precedent
(`nif_child_watch.rs:255-317`) splits by hand:

- **Retryable** — history reads, run-segment reads, every recorder append
  (`DurabilityError`). Safe to retry unconditionally because the terminal-check
  makes the whole operation idempotent: a retry that finds its own terminal
  already in history takes the resume path instead of appending twice.
- **Invariant** — a logic error no amount of retrying repairs. Logged at `error!`
  and abandoned.

## The one contract a naive refactor breaks

`completion.rs:94`: a `WorkflowTimedOut` terminal must NOT have its deadline
retired by this monitor — the deadline is the timeout teardown's own resume
anchor and the monitor is woken by that teardown's `cancel_pid`. It is pinned by
`process_exit_does_not_retire_a_timed_out_deadline`. Any restructuring keeps it.

## A gap in the resume path that the retry EXPOSES

The pre-existing-terminal branch (`completion.rs:141-167`) notifies, reconciles
the registry, and returns — it does **not** call `upsert_workflow_visibility`.

That is correct today: if another writer recorded the terminal, that writer did
the visibility upsert. It stops being correct under retry, because the "other
writer" becomes *this same operation's previous attempt*, which may have died
after appending the terminal and before upserting visibility. A retry would then
take the resume path and skip the visibility work permanently — trading a wedged
status for a stale index.

⇒ The resume path must do the visibility upsert too. It is an upsert, so the
redundant case costs one write and changes nothing.

## Test position, before any code

All four tests in the module are **husks with respect to this defect**: every one
takes its store from `active_workflow()`, which is `InMemoryStore`, whose
`read_history` cannot fail transiently — so the `?` at line 77 is never taken.
All four also call `handle_process_exit_async` directly, so the swallowing log
site at `start.rs:574` is executed by **no test in the crate**.

Nothing pins the one-shot behaviour as a contract, so the fix breaks no
deliberate pin.

The fault-injection fixture exists one directory away: `FlakyAppendStore`
(`nif_child_watch.rs:976-1135`) with `fail_next_appends(n)`. It is `#[cfg(test)]`
inside that module — reaching it from the lifecycle tests means promoting it to a
shared test fixture, which is the honest move rather than a second copy (see
finding 1: one rule, two places).

## Acceptance

- **A1.** A transient store failure during process-exit handling ends with the
  terminal recorded and the run projecting its true status — proven with a store
  that fails a bounded number of times, red before the fix.
- **A2.** The process-exit call returns with the retry still outstanding, so it
  cannot hold the dispatcher thread — proven by
  `armed_completion_retry_count() == 1` at the moment the call has returned.
- **A3.** The `WorkflowTimedOut` deadline contract still holds (existing pin).
- **A4.** A retry that finds its own terminal already recorded appends nothing
  and still leaves visibility current.
- **A5.** Both call sites are covered by the one mechanism; neither carries a
  retry of its own. **Structural only — see below.**
- **A6.** A failure no retry can repair is reported to the caller and leaves
  nothing armed — proven with an injected `SequenceConflict`, the store
  contract's double-writer indicator.

Every acceptance criterion **that is empirically tested** gets a control: a
mutation that makes it fail.

### 🔴 What these criteria do NOT prove

Written down because a criterion that quietly means less than it says is worse
than one that is absent.

- **A2 does not observe the real dispatcher.** Its fixture builds two
  independent engines and calls the async function directly, so the single
  process-exit callback thread is never on the path. The second workflow
  completing in that test is therefore **not** evidence about that thread, and
  the test must not be cited as such. What it does measure — the call returning
  while the work is still armed elsewhere — is the load-bearing half, and is
  decisive: awaiting the retry inline kills it.
- **A5 is structural, not executed.** Neither `lifecycle/start.rs` nor
  `engine/startup.rs` installs its monitor in any test in the crate; both
  closures are handed to `runtime.monitor_process`, which needs a live beamr
  process to fire. The criterion is satisfied by construction — both closures
  call the one shared `handle_process_exit`, and neither contains a retry — and
  that is verifiable by reading, **not by running**. There is no control for it,
  and none is claimed. Closing it empirically means an engine-level test that
  starts a real process, kills it, and observes the completion under an injected
  store fault; that is a larger fixture than this lane, and it is the thing to
  build if A5 ever needs to be more than a reading.

---

## Design decision, taken at the tree

### Where the executor lives: promoted, not borrowed

`ChildTaskRuntime` today is constructed and owned by `ChildNifBridge`
(`nif_child_engine.rs:89`), reached through
`EngineNifState::child_bridge: RwLock<Option<Arc<ChildNifBridge>>>` — an
**optional, conditionally-installed** bridge.

The completion monitor is a core lifecycle path. It runs for every workflow on
every node, including nodes where no child workflow is ever spawned. Making the
durability of a terminal event depend on whether an unrelated bridge happens to
be installed would be a silent failure of exactly the kind this repo forbids: on
a node with no child bridge the retry would have nowhere to run, and the fix
would quietly not apply.

Three placements were considered and two rejected:

| placement | why not |
|---|---|
| `ProcessExitContext::tokio_handle` (host runtime) | Not aborted-and-awaited at engine shutdown. A retry still appending a terminal after the epoch closes is a **second writer** against a successor engine over the same store — invariant 3, symptom `SequenceConflict`. This is the precise hazard `engine_tasks.rs:1-15` was written to prevent. |
| reach `ChildNifBridge`'s executor through the optional bridge | Ties a core path's durability to an optional installation, and reads a child-workflow component from the lifecycle layer. |
| **own it on `RuntimeHandle`** ✅ | `RuntimeHandle` already owns every engine-epoch-scoped object (nif state, delivery gates, activity maps) and is already reachable from `ProcessExitContext::runtime`. |

⇒ The executor moves to `RuntimeHandle`. The child bridge takes its `Arc` from
there instead of constructing one, so there is **one** epoch-closed executor on
the node, not two. Two executors with the same shutdown discipline would be the
same rule in two places, which is the thing that has already drifted every time
we have allowed it.

The type is no longer child-specific and is renamed to say what it is. Its module
doc already described a general registry with two task kinds; this adds a third.

### What the retry is keyed by

`(WorkflowId, RunId)` — the run whose terminal has not landed. Server-derived in
full: both come off the `WorkflowHandle` the monitor was installed with, never
off anything a caller supplies. One run cannot allocate more than one retry, and
arming is idempotent against an already-armed key.

### Scope of the change

1. Promote the executor to `RuntimeHandle`; the child bridge borrows it.
2. Add the completion-retry task kind to the registry.
3. Classify `handle_process_exit` failures Retryable/Invariant, following the
   `nif_child_watch` precedent, and arm a retry rather than swallowing.
4. Fix the resume path's missing `upsert_workflow_visibility`.
5. Promote `FlakyAppendStore` from a `#[cfg(test)]` private in
   `nif_child_watch.rs` to a shared test fixture — a second copy would be the
   same defect as the second caller.
6. Tests for A1–A5, each with a mutation control that makes it fail.

No caps, no invented defaults: the backoff comes from the same configured
`SignalDeliveryConfig` the child watcher already uses, not from a new constant.
