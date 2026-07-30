# `ActivityStarted` means "dispatched", not "started" — a decision for the owner

Status: **DECISION REQUIRED**. Nothing in this document has been implemented.
The observability work that accompanies it (task #87) is landed and deliberately
changes NO durable event semantics.

Context: task #87. Live exhibit and mechanism in
`docs/design/aion-authoring/ASSISTANT-STUCK-FINDING.md`.

---

## 1. What the current semantics are

`Event::ActivityStarted` is recorded by the ENGINE at dispatch time, in the same
atomic append as its `ActivityScheduled`, before any worker has seen the work.
It is not a report from a worker. Its doc comment — "An activity worker started
executing an activity attempt" (`crates/aion-core/src/event.rs`) — is the only
thing in the tree that claims otherwise, and it is wrong.

Confirmed at all four producing seams:

| Seam | File | Evidence |
| --- | --- | --- |
| Single dispatch | `crates/aion/src/runtime/nif_activity_dispatch.rs:247` | `record_started(...)` runs, then `spawn_completion_task(...)` hands the request to the dispatcher. The record precedes the dispatcher call, let alone worker selection. |
| Retry delivery | `crates/aion/src/runtime/nif_activity_retry_dispatch.rs:241` | Comment is explicit: "Record the retry delivery's `ActivityStarted` before it goes on the wire". |
| Fan-out | `crates/aion/src/durability/recorder/fan_out.rs:86-142` | `N×(ActivityScheduled + ActivityStarted)` derived and appended as one batch. |
| In-VM | `crates/aion/src/runtime/nif_activity_in_vm.rs:172` | Same `record_started` helper. |

All four funnel through `NifContext::record_activity_scheduled_started`
(`crates/aion/src/runtime/nif_context.rs:260`), which appends the pair under one
`recorded_at`. Hence the identical timestamps in the live exhibit.

**The brief's characterisation is correct in every respect.** I found nothing to
refute.

Worker selection happens strictly afterwards, in the server, at
`WorkerActivityDispatcher::select_worker_or_wait`
(`crates/aion-server/src/worker/bridge.rs:684`) → `select_worker_or_refuse`
(`crates/aion-server/src/worker/queue_service/wait.rs:103`). Under the default
`strict` policy with no configured `service_availability_deadline`, that wait is
unbounded — correctly so: the server does not invent a deadline the operator
never wrote. So a dispatch to a queue nobody serves parks there indefinitely,
having already recorded `ActivityStarted`.

### The consequence

Two runs with byte-identical history shapes:

- one whose activity a worker is executing right now;
- one whose activity was dispatched to an empty queue on 6 July.

Both project `Running`. The projection is not lying — history genuinely contains
no terminal event, and invariant 4 holds. The gap is that **nothing projects
reachability**: whether anything can still advance the run.

---

## 2. Exactly what breaks if the semantics change

Deferring `ActivityStarted` until a worker leases the work is not a rename. It
moves an event across the durability boundary: today it is a COMMAND-ISSUED
event written by the workflow's replay handoff; afterwards it would be an
ASYNCHRONOUS-ARRIVAL event written by a service. Concretely:

### 2.1 Replay resolution

- `crates/aion/src/durability/resolver.rs` decides `Recorded` vs `ResumeLive`
  for a `Command::RunActivity`. `dangling_retryable_failure_resumes_live_for_redispatch`
  (line 649) encodes the current trail semantics.
- `crates/aion/src/runtime/nif_activity_retry.rs:next_delivery_attempt` derives
  the next attempt by reading the recorded `ActivityStarted` trail
  (`nif_activity_dispatch.rs:244`). If `ActivityStarted` no longer appears at
  dispatch, a crash between dispatch and lease leaves a `ActivityScheduled` with
  no `ActivityStarted`, and the attempt derivation must be re-specified for that
  state. Today that state cannot exist.
- `crates/aion/src/runtime/nif_activity_dispatch_tests/retry_parking.rs:32`
  asserts the durable log "must end at the dangling scheduled/started trail" —
  the kill -9 convergence contract. That assertion is about the pair.

### 2.2 The single-writer invariant (invariant 3)

`ActivityStarted` would have to be appended by the server's selection path, which
does not hold the workflow's `Recorder`. It would need to route through the same
Recorder as the timer/signal/child services do. This is achievable — those
services already do it — but it is a new writer path for an event that has only
ever had one, and `SequenceConflict` is the failure mode if it is got wrong.

### 2.3 The NOI-0 attempt identity

`(workflow, activity, attempt)` is a stable identity across the lifecycle, and
`ActivityStarted.attempt` is one of its two anchors (`event.rs:279-293`). The
attempt is decided at the dispatch seam. If the start is recorded at lease time,
either the attempt travels to the worker and back (a new wire round-trip before
any durable record) or the identity is anchored on `ActivityScheduled` instead.
The intervention router's `AttemptKey`
(`crates/aion-server/src/worker/intervention.rs`) is keyed on this.

### 2.4 The store conformance suite

- `crates/aion-store/src/conformance.rs` and
  `crates/aion-store/src/conformance/outbox_settlement.rs` build histories with
  the pair.
- `crates/aion-store-libsql/src/append/metadata.rs` indexes on `ActivityStarted`.
- Every backend (in-memory, libSQL, haematite) shares the suite.

### 2.5 Every existing history in every store

This is the decisive constraint and it is not fixable by a migration. Under
event sourcing, histories already written are immutable facts. After the change,
`ActivityStarted` in an OLD history means "dispatched" and in a NEW history means
"leased". Any reader that treats it as one thing is wrong about half the corpus.
The house rule forbids compat shims, so there is no sanctioned "old semantics"
flag.

### 2.6 The projections and surfaces that read it

`crates/aion-core/src/status.rs`, `crates/aion-server/src/stream/selector.rs`,
`crates/aion/src/engine/delegated.rs`, `crates/aion/src/lifecycle/continue_as_new.rs`,
plus ~12 integration suites under `crates/aion/tests/` and
`crates/aion-server/tests/`.

---

## 3. Alternatives

**A. Change nothing durable; project reachability at the surfaces.**
What was built for #87. Costs nothing durable, breaks no history, and answers
the operator's actual question. Does not make history self-describing: a reader
holding only an exported history, with no live fleet, still cannot tell the two
cases apart.

**B. Rename the event to what it is.**
`ActivityStarted` → `ActivityDispatched`. Same information, honest name, and the
doc comment stops asserting something false. Still a wire/format change across
every store and every SDK, but it changes no MEANING — so no history becomes
ambiguous, which is the fatal problem with option D.

**C. Add a second event — `ActivityLeased` — recorded when a worker takes the
work.** `ActivityStarted` keeps its current meaning and its current position.
History becomes self-describing: dispatched-with-no-lease is visible in the
record forever, including in an exported history with no live server. Purely
additive, so no existing history changes meaning; old histories simply lack the
new event, which is honestly "unknown" rather than wrong. Costs: a new writer
path at the selection seam (see 2.2) and one extra append per activity attempt.

**D. Defer `ActivityStarted` to lease time.**
The change section 2 describes. Every existing history silently changes meaning.

**E. Bound the wait.**
Orthogonal, and it is a real gap — but it is task #42's (`WorkflowTimedOut`
wired to nothing), not this one, and a bound invented here would refuse work
nobody asked to have refused. Note the assistant's own declared seven-day
timeout was exceeded by 17 days with nothing happening.

---

## 4. Recommendation

**A now (done), then B, and C only if history must be self-describing.
Not D.**

Reasoning:

1. **D is disqualified by section 2.5 alone.** The one thing an event-sourced
   system cannot do is change what an already-written event meant. Every other
   cost in section 2 is engineering; that one is a correctness loss with no
   remedy, and it would land on the exact histories — the long-lived ones — that
   durable execution exists to protect.

2. **The current semantics are DEFENSIBLE; only the name and the silence were
   wrong.** "The engine dispatched this attempt" is a real, useful, durable fact,
   and it is the fact the replay/retry machinery is built on. The defect was
   never that the engine recorded it. It was that the event's name and its doc
   comment claimed a worker had started, and that no surface said otherwise.
   Option A removes the silence; option B removes the false claim. Together they
   fix the whole of what actually hurt the operator.

3. **C is the honest completion, and it is separable.** If you want an exported
   history to answer "did a worker ever take this" without a live server, C is
   the way, and it is purely additive. But it needs the invariant-3 writer
   question settled first, and it should not be bundled with a rename.

4. **On B's cost.** It touches `aion-core`, the proto conversions, the store
   backends, the conformance suite, and the Gleam/Python/TypeScript SDKs. That
   is a wide but mechanical change, and — unlike D — a mistake in it is a
   compile error or a decode error, not a silently wrong answer about a
   three-week-old run.

**If you want the minimum:** keep A, and fix the doc comment on
`Event::ActivityStarted` to say "the engine dispatched this attempt to the task
queue; it does NOT mean a worker has taken it". That is a one-line change with
no wire impact and it stops the tree asserting something false. I have not made
it, because even a doc comment on a durable event's meaning is yours to rule on.

---

## 5. What was built instead (no durable change)

- `crates/aion-server/src/worker/queue_service/reachability.rs` — joins the run's
  own recorded dispatch addresses (`ActivityScheduled`'s stamped `task_queue` and
  `node`) to the live poller census and the SAME `classify()` verdict the
  dispatcher's selection wait uses. No second notion of "is anyone serving this".
- `POST /workflows/describe` now returns `unserved: [...]` — computed regardless
  of `include_history`, because it answers a question the operator does not know
  to ask.
- `GET /queues/unserved` — exposes the live `QueueServiceState` that the
  unserved-honesty lane already built and that reached no surface at all.

Silence is preserved for the healthy case by construction: `classify()` returns
`None` for an address with a live compatible worker, so a served run produces no
entry. That is asserted by a negative control which, on its first run, correctly
FAILED against a fixture whose worker registration had been dropped early.
