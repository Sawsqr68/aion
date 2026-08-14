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

---

# PLAN CORRECTION (measured 2026-08-14, post-compaction — this supersedes steps 5
# and the routing decision above; both were wrong, and are kept for the record)

## Correction 1 — the settle hook is the BOOT path, not the live one

The plan above put the cancel fan-out "beside the settle, which already runs on
terminal". **It does not run on terminal.** `settle_terminal_outbox_rows` has
exactly two callers — `aion-server/src/run.rs:504` (boot) and
`aion-server/src/cluster.rs:133` (shard adoption). Its own module doc says so
(`outbox_settle.rs:5-11`): *"The Recorder settles a workflow's live rows at
terminal-record time (the live window); this module is the BOOT/ADOPTION
backstop."*

So a fan-out placed there would fire on server boot and never on a live cancel:
by the time it ran, the Recorder (`recorder.rs:438`, inside the `aion` crate)
would already have settled the rows and it would find nothing. **The hook was
placed at the one site that cannot see the event it needs.**

## Correction 2 — the live worker↔activity map EXISTS, so broadcast is superseded

The broadcast-to-pool decision above was reasoned from a true premise about the
DURABLE row (`OutboxRow` carries no worker id) and a false unstated one — that no
live mapping existed anywhere. It does:

`aion-server/src/worker/heartbeat.rs:102` —
```rust
struct TaskKey(WorkerId, WorkflowId, ActivityId);
struct HeartbeatState { tasks: HashMap<TaskKey, TaskLiveness>, ... }
```

`HeartbeatTracker` is the server's live registry of what every connected worker
is executing right now. It is already used for exactly this class of question:
`fail_all_in_flight_workers` (`:719`), `park_all_in_flight_workers` (`:788`),
`remove_worker_tasks` (`:896`), `in_flight_count` (`:591`). Entries are added on
dispatch (`track_task`, `:471`) and removed on completion, disconnect, or
expiry — so a tracked entry means *a live worker holds this activity now*.

**Direct routing wins on every axis the broadcast argument used.** It is precise
(no no-op traffic), it cannot be stale (the tracker drops the entry when the
worker goes), and it needs no durable schema change either. The staleness hazard
that ruled out "record the WorkerId on claim" does not apply here: the tracker's
worker id is not a recorded historical fact, it is present membership.

## Correction 3 — liminal transport has no control channel, and must SAY so

`drain_worker` (`registry.rs:1014-1048`) proves the shape: `WorkerDelivery` is an
enum, and only the `Grpc(WorkerTaskSender)` arm can carry a control message. The
`Liminal` arm logs at error level and deregisters, because *"liminal transport
has no drain control channel"*. A cancel faces the identical wall. It must
therefore report **asked-but-undeliverable** for a liminal worker, named and
logged — never a silent skip. That is this defect's own shape and must not be
reproduced inside its fix.

## The corrected build, in order

1. **`crates/aion-proto-generated/proto/worker.proto`** — `CancelActivity
   { WorkflowId workflow_id = 1; ActivityId activity_id = 2; }` plus a sixth
   `ServerToWorker` oneof member (tag `6`). Stubs generate at build time.
2. **`WorkerMessage::CancelActivity`** (`registry.rs:63`).
3. **`encode_server_to_worker`** (`worker_grpc.rs:738`) — the encoding arm.
4. **`decode_server_message`** (`session.rs:636`) — the arm constructing
   `WorkerSessionEvent::Cancel`, with `decode_result_ack`'s missing-field
   refusals. This is the line that makes `loop_.rs:245-247` reachable.
5. **`HeartbeatTracker::in_flight_for_workflow(&WorkflowId)`** — read the tracked
   entries for one workflow. `TaskLiveness` already carries `worker_id`,
   `activity_id`, and `attempt`.
6. **A new `aion-server/src/worker/activity_cancel.rs`** — asks each tracked
   worker to stop, and returns WHAT WAS ASKED AND WHAT WAS OBSERVED per entry
   (requested / worker gone / transport cannot carry it / send failed). It never
   asserts a stop.
7. **The call site: the server's cancel path**, after the engine's cancel returns
   Ok. All three transports (HTTP `api/http/workflows.rs:83`, gRPC
   `api/grpc/mod.rs:227`, MCP `mcp/tools/control.rs:162`) converge on
   `handlers::workflows::cancel` (`:302`) and all three hold a `ServerState`,
   so there is ONE site.

**Scope stated honestly:** this covers cancel. A run that ends by TIMEOUT or
FAILURE leaks its in-flight activity the same way, and this fix does not close
that — the terminal for those is recorded inside the engine, which must not learn
about workers. That is a separate, named follow-up, not a silent omission.

## Battery scope for this change

Closure, not diff: `aion-proto-generated`, `aion-proto`, `aion-worker`,
`aion-server`, plus the outbox conformance suite
(`aion-store/src/conformance/outbox_settlement.rs`) if the trait return widens.
Both clippy resolutions (`--workspace` and `-p aion-server`). The new decode arm
must be seen failing before it is trusted.

## BUILD RECORD (2026-08-14, post-compact-59)

All seven pieces built. Every load-bearing arm was SEEN RED before being
trusted, and the red-first requirement paid for itself twice.

**Worker-side decode (`aion-worker` session.rs): 4 new tests, 2 mutations.**
- Mutation 1 — missing `activity_id` silently defaults to 0 instead of
  refusing: killed exactly `cancel_without_an_activity_id_is_refused` (the
  accepted event was `ActivityId(0)`, the precise silent-address failure the
  refusal exists to prevent). Other three stayed green — each control kills
  only its own test.
- Mutation 2 — the wire arm never routes to `decode_cancel_activity` (returns
  Drain): killed ALL FOUR. That arm is the whole of #233; the tests sit on it.
- Both restored by writing bytes; suite 9/9 green on the rebuilt tree.
- The stale doc on `WorkerSessionEvent::Cancel` ("the proto does not yet carry
  this frame") corrected — it now names the wire message and decoder.

**Server-side fan-out (`aion-server` activity_cancel.rs): first run was
RED + HUNG, and the red was a real trap.**
- `WorkerRegistration` deregisters on Drop (disconnect cleanup). My helper
  dropped it, so every test's worker was GONE before the body ran: two tests
  failed (`WorkerGone` where `Requested`/`ChannelUnavailable` was expected)
  and one HUNG forever at `recv().await` on a channel no cancel had reached.
- Worse: `a_departed_worker_is_reported_rather_than_skipped` passed GREEN for
  the WRONG reason — its explicit deregister had already been done by the
  drop. A coinciding answer: right verdict, dead treatment.
- Fix: `RegisteredWorker` holds the registration guard for the test's
  lifetime; the departed test now deregisters through the guard explicitly.
  All `recv().await` replaced with `try_recv()` — the fan-out is synchronous,
  so after it returns the message is either queued or absent; a wait can only
  hide the absence as a hang instead of a failure.
- Routing filter mutation — `in_flight_for_workflow` filter → `true` (returns
  every workflow's activities): killed exactly the bystander test (2 asks
  where 1). Restored by writing bytes, 5/5 green in 0.01s.

**Handler threading.** `cargo check` had NOT covered test targets: four
existing handler tests called `cancel` with the old arity. Rather than clone a
40-line RuntimeConfig fixture, `api/http/test_support` was widened to
`pub(crate)` and the handler `TestContext` now carries a `ServerState` built
over the SAME resolver as its guard (one server, not two that happen to
agree). `context()`'s error type widened to `Box<dyn Error>` (all 31 callers
already return that) so the state build refuses instead of panicking.

**Clippy caught my own tests.** Both resolutions were RED on five `panic!`
sites I had just written (the lints cover test code). Fixed by making the
tests return errors — no `#[allow]`. Both resolutions then EXIT=0 (logs:
scratchpad clippy-ws2.log / clippy-p2.log).

**Instrumentation lesson re-learned:** the background wrapper's exit code was
the tail's/echo's, not cargo's — the first "completed exit 0" notification
covered a run that was RED and hung. The EXIT= line written INTO the log file
is the only trustworthy verdict; a `| tail -20` also ate the failures block
until the single-test rerun wrote to a file.

**Remaining before commit:** closure battery (4 crates, all targets) —
running; trait return did NOT widen, so the outbox conformance suite is
untouched by this change. Then commit + push + Opus review.

## Closure battery record (2026-08-14)

**Run 1 (battery-233.log): RED and INCOMPLETE — attributed, kept beside.**
EXIT=101 inside the file (the wrapper notification again said "exit 0" —
second confirmed instance of the wrapper-exit lie). The single failure was
`worker::outbox_dispatcher::tests::failed_dispatch_retries_with_backoff_and_bumps_attempt`
(outbox_dispatcher.rs:997) — 1110 passed / 1 failed in aion-server --lib.
Module untouched by this diff (`git diff --name-only | grep -i outbox` = no
hits). Mechanism: the test asserts nothing is claimable inside a 100ms
backoff fence; at load 34-48 more than 100ms of wall-clock elapsed between
sweep and claim, the row became visible, the absence-assertion went red —
contention pushes this assertion directionally red. Solo re-run on the same
tree: green, 1.27s, EXIT=0 (retry-solo.log). Filed as task #238 (same class
as #207: wall-clock window narrower than scheduling jitter). Because cargo
aborted at that target, run 1 has only 6 verdict lines — most binaries never
ran, so run 1 could not certify the closure regardless of the flake.

**Task #237 filed:** a run ending TimedOut/Failed leaks in-flight activities
identically — #233's fan-out must fire on every terminal transition. Not
closed by this change; must not pass silently under its green.

**Run 2 (battery-233-run2.log): the closure run** — same 4-crate
--all-targets battery, uptime head+foot, EXIT= inside the file. Verdict
recorded below when it lands.

**Run 2 verdict: GREEN, complete.** EXIT=0 inside battery-233-run2.log,
60 verdict lines, 1587 passed / 0 failed, no nonzero-failed lines. Load
12.59 at head, 10.95 at foot (quiet relative to run 1's 34-48). cargo's
own EXIT=0 certifies every target ran to a verdict — a crashed binary
would have propagated nonzero. This is the closure run; run 1 stays
beside it as the contended-red exhibit.
