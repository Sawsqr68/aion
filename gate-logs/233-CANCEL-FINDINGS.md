# #233 — a cancelled run keeps eating the box: located, at the wire

Measured 2026-08-14 at Vesper's hand. Every claim names the file and line it was
read from.

## The symptom

An operator cancels a run. The run reports `Cancelled`. The command it dispatched
— a workspace test suite, a build, an agent round — **keeps running**, keeps
holding the machine, and finishes on its own schedule. The operator, seeing
`Cancelled`, stops watching.

## The mechanism, end to end

### 1. Engine-side cancel kills the workflow's own process, and only that

`crates/aion/src/lifecycle/terminate.rs:115` — `cancel`:

1. records `WorkflowCancelled` through the recorder (`:134`),
2. retires the run's declared-timeout deadline (`:137`),
3. calls `context.runtime.cancel_pid(handle.pid())` (`:139`),
4. upserts visibility, notifies the completion waiter, deregisters (`:149-154`).

Its own doc comment states the theory of the case (`:100-101`):

> Cancels a live workflow run, **relying on runtime link propagation to tear down
> any linked activity children.**

That is true for children *inside the VM*. **A remote worker's activity is not
one.** It is a separate OS process, on a separate machine in the general case,
reached over gRPC. Killing the workflow's process propagates to nothing there.

### 2. The worker SDK's cancellation machinery is complete

None of this is missing:

| piece | where |
|---|---|
| `ActivityCancellationHandle` with `cancel()` | `aion-worker/src/context.rs:42`, `:284` |
| per-activity `CancellationState`, `is_cancelled`, `cancelled().await` | `context.rs:58`, `:202`, `:207` |
| in-flight registry keyed by (workflow, activity) | `runtime/loop_.rs:659` |
| `deliver_cancellation` → fires the handle | `runtime/loop_.rs:669-682` |
| `cancel_all_in_flight` for shutdown/drain | `runtime/loop_.rs:684-693` |
| process-group containment + termination grace | `ContainedChild`, `PROCESS_GROUP_TERMINATION_GRACE`, `run_cancellable_command` (`aion-worker/src/lib.rs:81-82`) |
| the receive-loop arm that consumes a cancel | `runtime/loop_.rs:245-247` |

The loop arm is written and waiting:

```rust
Ok(WorkerSessionEvent::Cancel { workflow_id, activity_id }) => {
    deliver_cancellation(workflow_id, &activity_id, &in_flight);
}
```

### 3. 🔴 The wire cannot carry a cancel — so that arm can never be taken

`crates/aion-proto-generated/proto/worker.proto:40-48`:

```proto
message ServerToWorker {
  oneof message {
    ActivityTask task = 1;
    DrainRequest drain = 2;
    RegisterAck register_ack = 3;
    ResultAck result_ack = 4;
    LivenessPing liveness_ping = 5;
  }
}
```

**Five members. No cancel.** The string `cancel` does not appear anywhere in
`worker.proto` — a case-insensitive search over the whole file returns nothing.

And the decoder matches that exactly. `decode_server_message`
(`aion-worker/src/protocol/session.rs:636-668`) has arms for `Task`, `Drain`,
`ResultAck`, `LivenessPing`, and a `RegisterAck` protocol-violation refusal.
**There is no arm that constructs `WorkerSessionEvent::Cancel`.**

The server's encoder agrees: `encode_server_to_worker`
(`aion-server/src/api/worker_grpc.rs:738-759`) maps a `WorkerMessage` with
exactly three variants — `ActivityTask`, `DrainRequest`, `LivenessPing`.

## The finding

**`WorkerSessionEvent::Cancel` is never constructed anywhere in production.** The
worker's entire cancellation path — handle, registry, delivery, process-group
kill, grace period — is real, correct, and **unreachable from the server**,
because the protocol between them has no message that means "stop".

So `cancel` does precisely what its code says: it records the fact, kills the
workflow's own process, and returns. The dispatched command was never told.

This is the guard-that-fails-quiet shape. Nothing errors. Nothing logs a refusal.
The status is truthfully `Cancelled` — the *workflow* is cancelled. It is the
work that isn't.

## Why the census is decisive despite a public enum

`WorkerSessionEvent` is `pub`, so in principle an external crate could construct
a `Cancel`. That does not weaken the finding: the only thing that feeds the
worker's event stream in production is `decode_server_message`, and its input is
`ServerToWorker`, whose oneof has no cancel member. The claim rests on the **wire
contract**, not on a call-site census.

## What has to move, and it must move together

1. **`worker.proto`** — a `CancelActivity { workflow_id, activity_id }` member on
   the `ServerToWorker` oneof, and the regenerated stubs.
2. **`encode_server_to_worker`** — a `WorkerMessage::CancelActivity` variant and
   its encoding.
3. **`decode_server_message`** — the arm that finally produces
   `WorkerSessionEvent::Cancel`, with the same missing-field refusals
   `decode_result_ack` already models (`session.rs:670-699`).
4. **The server's cancel path** — on `WorkflowCancelled`, find every in-flight
   dispatch for that workflow and push a cancel down each owning worker's
   forwarder. The forwarder is the same one `Drain` and the liveness ping ride.
5. **The engine→server edge** — `terminate::cancel` lives in `aion`, which knows
   nothing of the worker registry. The signal has to cross that boundary the way
   other engine-to-server facts already do, without `aion` learning about
   workers.

### Two things the fix must not get wrong

- **Cancelling is a request, not a guarantee.** A worker may be unreachable, may
  have already finished, or may be running an action that ignores cooperative
  cancellation. The run's record must say what was *asked* and what was
  *observed*, never assert a stop it did not witness. "Silently assumed it
  stopped" is the same defect one layer up.
- **The grace period is the operator's value, not ours.** `PROCESS_GROUP_TERMINATION_GRACE`
  already exists; whether a cancel escalates past it, and after how long, is a
  declared value — not a number invented here.

## Related, and deliberately separate

- **#224** — a dropped dispatch parks its blocking thread until the worker-side
  call ends on its own. Same neighbourhood (nothing tells the far end to stop),
  different trigger.
- **#26** — ruled: cancel does not cascade to child *workflows*, contract stated,
  ADR-004 is the unbuilt target. That is about children in the engine. This is
  about the activity already executing on a worker. Do not conflate them: closing
  #233 does not close #26.
- **#233 is not #223.** #223 armed the activity dispatch *timeout* — a deadline
  the engine enforces on its own side. It does not tell the worker to stop either.

---

# BUILD PLAN (measured 2026-08-14, before compaction — resume from here)

## What ALREADY exists and must NOT be rebuilt

- **`settle_terminal_outbox_rows`** (`aion-server/src/worker/outbox_settle.rs:54`)
  and **`OutboxStore::cancel_outbox_rows_for_workflow`**
  (`aion-store/src/outbox.rs:568`) already settle a terminal workflow's live
  (`Pending`/`Claimed`) rows to `Cancelled` — #253. The Recorder settles at
  terminal-record time; `outbox_settle` is the boot/adoption backstop.
  **This stops FUTURE dispatch and redrive. It does NOT tell a worker already
  executing to stop.** That remains the whole of #233.
- `is_settle_terminal` (`outbox_settle.rs:29`) — the four hard terminals.

## Routing decision — MADE, with the reason

The outbox row does **not** record which worker took the dispatch
(`OutboxRow`, `aion-store/src/outbox.rs:166-227`: dispatch_key, workflow_id,
ordinal, run_id, namespace, task_queue, node, activity_type, input, status,
attempt, visible_after, claimed_at, failure_delivered — **no worker id**).

Two shapes were available:
- (a) record the winning `WorkerId` on claim and route the cancel to it;
- (b) **broadcast to every worker in the row's `(namespace, task_queue, node)`
  pool** and let each ignore a key it does not hold.

**(b) is chosen.** `deliver_cancellation` (`aion-worker/src/runtime/loop_.rs:669`)
is already a no-op when the key is absent from `in_flight`, and the in-flight key
is `(workflow_id, activity_id)` — globally unique, so a broadcast cannot cancel
someone else's work. (b) needs no durable schema change and cannot point at a
stale registration (a worker that reconnects gets a new `WorkerId` mid-flight,
which is exactly the hazard (a) carries). Cost is a few no-op messages inside one
small pool, and the pool is already the correctness boundary.

## The five pieces, in order

1. **`crates/aion-proto-generated/proto/worker.proto`** — add
   `CancelActivity { WorkflowId workflow_id = 1; ActivityId activity_id = 2; }`
   and a sixth member on the `ServerToWorker` oneof (tag `6`). Stubs are
   generated at build time by `build.rs` (protoc vendored, `rerun-if-changed=proto`),
   so **nothing is committed by hand**.
2. **`WorkerMessage`** (`aion-server/src/worker/registry.rs:63`) — a
   `CancelActivity` variant beside `ActivityTask` / `DrainRequest` /
   `LivenessPing`.
3. **`encode_server_to_worker`** (`aion-server/src/api/worker_grpc.rs:738`) — its
   encoding arm.
4. **`decode_server_message`** (`aion-worker/src/protocol/session.rs:636`) — the
   arm that finally constructs `WorkerSessionEvent::Cancel`, with the same
   missing-field refusals `decode_result_ack` (`session.rs:670-699`) already
   models. **This is the line that makes `loop_.rs:245-247` reachable.**
5. **The server's cancel fan-out** — the natural hook is beside the settle, which
   already runs on terminal and already has the workflow id. For each row settled
   by `cancel_outbox_rows_for_workflow` whose prior status was `Claimed`, push a
   `CancelActivity{workflow_id, activity_id: ordinal}` to every worker in that
   row's pool via `WorkerRegistry::workers_for(...)`.
   ⚠️ `cancel_outbox_rows_for_workflow` currently returns only `Vec<String>` of
   dispatch keys — **it does not say which were `Claimed` nor carry the pool**.
   Either widen its return to the settled rows, or read the rows before settling.
   Decide by reading the concrete impls before changing the trait.

## The engine→server edge

`terminate::cancel` is in `aion`, which must not learn about workers. It does not
need to: the settle hook already fires on the terminal record, server-side. Put
the fan-out there, **not** in the engine. No new cross-crate seam.

## Ordering

Fan out the cancel **after** the terminal is durably recorded — never before. A
cancel pushed ahead of the record could stop work for a cancellation that then
fails to persist.

## Two things the fix must not get wrong (restated — these are the review bar)

- **A cancel is a request, not a guarantee.** The record must say what was
  *asked* and what was *observed*; it must never assert a stop it did not
  witness. "Silently assumed it stopped" is this same defect one layer up.
- **The escalation grace is the operator's value.**
  `PROCESS_GROUP_TERMINATION_GRACE` already exists
  (`aion-worker/src/lib.rs:82`). Whether a cancel escalates past it, and after
  how long, is declared — not invented here.

## Battery scope for this change

Closure, not diff: `aion-proto-generated`, `aion-proto`, `aion-worker`,
`aion-server`, plus the outbox conformance suite
(`aion-store/src/conformance/outbox_settlement.rs`) if the trait return widens.
Both clippy resolutions (`--workspace` and `-p aion-server`). The new decode arm
must be seen failing before it is trusted.
