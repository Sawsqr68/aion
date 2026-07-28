# Red-then-green evidence — build/unserved-honesty (R1, Phase 1)

Unserved-queue honesty at the bridge seam: the four-way taxonomy computed
against the landed handshake contract records, two service policies, two
never-conflated clocks, a typed `WorkerUnavailable` failure, and a queryable
unserved-queue state. `docs/design/RUNTIME-OPERATIONS.md` R1, Phase-1 seam
ordering (§8). Built on branch `build/unserved-honesty` off main `28eb1eec`,
2026-07-28/29.

## Red — behavioural: the park says nothing

`gate-logs/unserved-honesty-red.log` (+ `.exit` = 101), captured before the
classifier existed. The exhibit
(`crates/aion-server/tests/unserved_queue_honesty.rs`) parks a real dispatch on
a queue nobody serves, reads the seam's OWN tracing output through a
thread-scoped subscriber, and asserts the desired R1 behaviour:

```
test parked_dispatch_on_an_unserved_queue_states_its_reason_at_warn ... FAILED
a dispatch parked on an unserved queue emitted no reasoned event; captured
instead: [CapturedEvent { level: Level(Info), message: "no connected worker;
waiting for a matching worker to register", fields: {"activity_id":
"activity:0", "activity_type": "greet", "namespace": "default",
"workflow_id": "…"} }, … ×5 ]
```

Five identical INFO lines in two and a half seconds: no state, no reason, no
age, no deadline. "Served slowly" and "served by nobody" are the same event.
That is the incident shape of the committed 3.5-hour hang triage
(`handshake-red-green.md`, defect #3) — the engine-less-registration fix
removed one CAUSE; this exhibit pins the surviving BEHAVIOUR.

## Green — the seam states what it knows

`gate-logs/step3-exhibit.log` (+ `.exit` = 0): the same exhibit, unchanged,
against the classifier.

```
test parked_dispatch_on_an_unserved_queue_states_its_reason_at_warn ... ok
```

The park now emits WARN with `queue_service_reason=NO_LIVE_POLLERS`,
`last_compatible_poller_age_ms`, the policy, the deadline, and the three
census counts. Repeats drop to DEBUG and a changed verdict re-escalates, so a
real signal does not become noise nobody reads.

## The four-way taxonomy, and where each variant really fires

| Reason | Fires from | Policy answer |
| --- | --- | --- |
| `NO_QUEUE_DECLARATION` | no deployed `.v4` contract declares the queue | refuses immediately, terminally, under BOTH policies |
| `NO_LIVE_POLLERS` | no worker connected for `(namespace, task_queue)` | strict refuses on the availability clock; `durable_pending` parks |
| `POLLERS_INCOMPATIBLE` | workers in the pool, none covering the activity type or the node pin | same as above |
| `SATURATED` | a live compatible worker whose intake stays full past the schedule-to-start clock | refuses, retryably |

`SATURATED` is not a placeholder. It cannot come from the selection wait —
selection SUCCEEDS whenever compatible pollers are live — so it is emitted
where saturation actually shows up: the hand-off
(`queue_service::delivery`). Pinned end to end through the real dispatcher
(`a_full_intake_on_a_live_worker_refuses_as_saturated`) and at unit level
(`an_intake_that_stays_full_past_the_clock_is_saturated`), with the two
converse pins that keep the name honest — a closed transport is never reported
as saturation, and an intake that frees up inside the clock is served, not
refused.

## Built against the handshake records, not beside them

The declaration reader (`queue_service::declarations`) answers from
`Engine::declared_task_queues` — a new read-only accessor over the SAME
retained `.v4` contracts that registration admission
(`worker::contracts::validate_worker_contracts`) and start admission
(`EngineError::NoQueueDeclaration`) already read. No second queue-declaration
truth exists in the server.

It is fail-open by construction: no source installed, an empty declaration set,
or an unreadable catalog all answer `Unknown`, and `Unknown` can never produce
a structural refusal. This is the handshake lane's own lesson applied — its
defect #3 was a strict check firing where the catalog legitimately had nothing
to say, and the symptom was this seam waiting forever at INFO. Pinned at the
production seam: a real boot with an empty catalog classifies as
`NO_LIVE_POLLERS`, never `NO_QUEUE_DECLARATION`
(`unserved_queues_surfaces_a_parked_dispatch_and_clears_it`).

## The two policies and the two clocks

* **strict** — the default everywhere. Structural unservability refuses at
  once. A fleet condition refuses when the **service-availability deadline**
  expires, typed with THAT clock and retryable (a worker may yet connect; the
  workflow's own declared retry policy, or its absence, decides what happens
  next — no retry budget is invented here).
* **`durable_pending`** — a written per-queue opt-in. The run is marked
  unserved immediately and visibly, survives the availability deadline, and
  completes when a worker finally arrives. Structural unservability still
  refuses: pending on a queue no deployment declares would be exactly the lie
  the policy exists to avoid.
* The **schedule-to-start timeout** is the other clock and applies under both
  policies: it bounds the hand-off to a worker that already exists.

Both clocks are `Option` and unset by default. An unset clock means unbounded
— which IS today's behaviour, and the only honest default: the server does not
get to invent the deadline an operator never wrote (house law: no assumed
defaults for configurable values). What changed is that the unbounded wait is
now loud, typed, and queryable instead of silent.

`WorkerUnavailable` renders into the engine's own reason vocabulary
(`terminal:` / `retryable:`) with a machine-readable head —
`reason=`, `clock=`, `waited_ms=`, `last_compatible_poller_age_ms=`, the census
counts — and a human tail naming the address to fix. `WorkerUnavailable::parse`
reads the typed head back out of a recorded failure, so a run's history carries
the verdict without string sniffing.

## Acceptance pins

`crates/aion-server/tests/unserved_queue_policies.rs` (real dispatcher, real
connected-worker registry):

* `an_undeclared_queue_refuses_immediately_under_both_policies` — structural,
  terminal, no clock consulted even with a 600s deadline written.
* `strict_refuses_no_live_pollers_when_the_availability_deadline_expires` —
  typed, retryable, `clock=service_availability`, refused no earlier than the
  declared deadline.
* `connected_workers_that_do_not_serve_the_activity_are_incompatible` — the
  distinction the flagship incident could not make.
* `a_full_intake_on_a_live_worker_refuses_as_saturated` —
  `clock=schedule_to_start`, asserted distinct from the availability clock.
* `durable_pending_parks_visibly_and_completes_when_a_worker_arrives` — marked
  unserved immediately, still running past the deadline that would have refused
  a strict dispatch, then served and completed by the arriving worker.
* `the_recorded_failure_reason_is_typed_and_never_reads_as_a_park` — the sharp
  edge: a refusal mistaken for the drain sentinel would be swallowed as a
  "parked for restart recovery" no-op and the run would stall silently again.

Existing contract held: `dispatcher_fails_immediately_when_draining_without_workers`
untouched and passing — the drain gate still runs first, still wins, and now
also wins inside the hand-off retry window
(`a_drain_during_the_retry_window_fails_with_the_drain_reason`).

## Two structural notes

* The exhibit owns its own test binary. It reads the seam's real tracing
  output through a thread-scoped subscriber, and `tracing`'s
  default-dispatcher state is process-global: a sibling test running
  concurrently in the same binary empties the capture. Split into
  `unserved_queue_honesty.rs` (exhibit) and `unserved_queue_policies.rs`
  (typed pins) rather than weakened, or made to depend on `--test-threads=1`.
* Last-compatible-poller age is node-keyed in the registry, so a worker still
  serving the activity on another node never makes a node-pinned address look
  freshly served. Departure records are dropped when the address is served
  again, so the map is bounded by real registrations and never by
  caller-supplied dispatch strings.

## Battery record

All at the committed tree, inside the lane worktree, default target dirs,
output to `gate-logs/` with `.exit` manifests alongside.

* `gate-logs/df-preflight.log` — 170Gi available before the run.
* `gate-logs/fmt.log` — `cargo fmt --all --check`, exit 0.
* `gate-logs/clippy.log` — `cargo clippy --workspace --all-targets -D warnings`,
  exit 0, zero errors.
* `gate-logs/test-workspace.log` — `cargo test --workspace --no-fail-fast`,
  exit 0, **217 suites, 2999 passed, 0 failed**, 6 ignored (all pre-existing:
  the slow OS-process kill-9 gates and the network-dependent scaffold build;
  this lane added none). 33 of the passing tests are this lane's.
* Step logs retained: `unserved-honesty-red.log` (the red), `step2-*`,
  `step3-*`, `step4-*` (per-commit gates).

The battery ran at the code tree of this branch; the only delta between that
tree and the final commit is this evidence file.
