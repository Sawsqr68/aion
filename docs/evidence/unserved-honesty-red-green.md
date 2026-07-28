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
  exit 0, **217 suites, 2999 passed, 0 failed**, 6 ignored (all pre-existing;
  this lane added none), named in full below. 33 of the passing tests are this
  lane's.
* Step logs retained: `unserved-honesty-red.log` (the red), `step2-*`,
  `step3-*`, `step4-*` (per-commit gates).

The battery ran at the code tree of this branch; the only delta between that
tree and the final commit is this evidence file. It ran BEFORE the toolchain
pin landed and therefore carries no toolchain stamp — see the review-refusal
record below, which supersedes this record's completeness claim.

### The 6 ignored tests, by name

None is a runtime-gated skip (those never use `#[ignore]` here; they log and
pass). Every one is `#[ignore]`d for cost or for an off-box dependency:

| Test | Suite | Why ignored |
| --- | --- | --- |
| `osproc_kill9_liminal_failover_completes_exactly_once` | `aion-cli/tests/lsub5b_osproc_kill9_failover_e2e.rs` | spawns a 3-node cluster + liminal worker, kill -9 failover |
| `osproc_kill9_parked_timer_resumes_and_completes_exactly_once` | `aion-cli/tests/parked_timer_osproc_kill9_failover_e2e.rs` | 3-node cluster + hard-kill failover |
| `osproc_liminal_listener_accepts_real_worker_connection` | `aion-cli/tests/lsub5b_osproc_failover_e2e.rs` | spawns real aion-server + liminal worker processes |
| `stacked_dev_lands_through_the_real_worker_and_review_signal` | `aion-cli/tests/stacked_dev_live_e2e.rs` | cargo-builds an out-of-workspace worker, boots a server, drives real processes |
| `dev_pipeline_template_builds_packages_and_passes_its_tests` | `aion-cli/tests/new_scaffold_build.rs` | downloads `gleam_json` from hex.pm; fails offline |
| `norn_harness_drives_a_real_run_end_to_end` | `aion-integration-norn/tests/norn_e2e.rs` | spawns the real `norn` binary + a local mock provider |

## Review refusal — a MIXED catalog, and the refusal it manufactured

The domain-owner review refused this lane on one blocker, read-verified at the
bytes. `WorkflowCatalog::declared_task_queues` skipped any entry it could not
decode (`let Ok(contract) = entry.workflow.contract() else { continue }`).
A catalog holding one `.v4` package beside one pre-`.v4` package — precisely
the state the handshake migration creates, old packages retained while new
`.v4` ones deploy — therefore answered with a NON-EMPTY set that silently
omitted whatever the old package served. The empty-set fail-open above never
fired; the omitted queue classified `NotDeclared`, and `NotDeclared` is
terminal: dispatches a live compatible worker could have served were refused
outright, with a confident and false reason. The bare `continue` was the
defect and the concealment at once.

### The fix: an answer that reports its own limits

`declared_task_queues` now returns `DeclaredQueues`
(`crates/aion/src/loader/declared_queues.rs`) — the queues one read found,
plus the stored identities that same read could not decode. Not a bool: the
undecodable identities ARE the completeness fact, and they are what an
operator needs in order to act (re-deploy those packages under `.v4`). The
reader is the site that observes an undecodable entry, so the reader is the
site that warns, naming the identities.

The classifier mapping follows the read's standing:

| Read | Queue | Answer |
| --- | --- | --- |
| covered every entry | found | `Declared` |
| covered every entry | missing | `NotDeclared` — structural refusal, and correct |
| found no queue at all | — | `Unknown` (unchanged: contradicts nothing) |
| partial | found | `Declared` — a positive find is positive |
| partial | missing | `Unknown` + WARN naming the undecodable identities |

The last row is the whole fix. What a partial read did not find, it may simply
have been unable to see.

### Red, then green

Red first: `96fe31ed` lands the answer type and the threading with the DEFECT
UNCHANGED — the reader still skips silently, still reports no undecodable
identities — so the pins fail behaviourally rather than by compile error.
`gate-logs/refix-f1-red.log` (+ `.exit` = 101 at both seams):

```
loader::declared_queues::…::a_mixed_catalog_names_the_entry_it_could_not_decode
  FAILED: a pre-.v4 entry means this read saw only part of the catalog
worker::queue_service::declarations::…::a_queue_absent_from_a_partial_read_is_never_undeclared
  FAILED: left: NotDeclared  right: Unknown
worker::queue_service::declarations::…::a_mixed_catalog_never_structurally_refuses_the_undecodable_queue
  FAILED: left: Some(NoQueueDeclaration)  right: Some(NoQueueDeclaration)
```

That third line is the refused lane's whole point in one assertion: the
terminal structural refusal, produced against a queue nobody contradicted.

Green: `3d4bf77b` — `gate-logs/refix-f1-green.log` (+ `.exit` = 0).

New pins, all at the seams that actually decide:

* `a_mixed_catalog_names_the_entry_it_could_not_decode` — real
  `WorkflowCatalog`, one `.v4` entry declaring `orders` and one pre-`.v4`
  entry: the read finds `orders`, reports itself as not covering every entry,
  and names the pre-`.v4` stored identity.
* `a_read_of_only_v4_entries_covers_every_entry` — the converse; a fully
  readable catalog keeps its standing to refuse.
* `a_queue_absent_from_a_partial_read_is_never_undeclared` /
  `a_positive_find_survives_a_partial_read` — the mapping, both directions.
* `a_complete_read_still_reports_an_absent_queue_as_undeclared` — the
  structural refusal is preserved where it is earned, not weakened.
* `an_undecodable_entry_is_named_at_warn` — the operator half, read back off
  the seam's real tracing output through a thread-scoped subscriber: exactly
  one WARN, naming the stored identity to re-deploy. A silent downgrade to
  `Unknown` would leave a fleet reading as merely unserved, forever. (Green
  only: added after the red commit, so it is proven green, not red-then-green.)
* `a_mixed_catalog_never_structurally_refuses_the_undecodable_queue` — the
  consequence, through `classify`: `POLLERS_INCOMPATIBLE`, a retryable fleet
  condition, never `NO_QUEUE_DECLARATION`.

### Battery record for the refix

Targeted gates at this seat, at the pinned tree (`rust-toolchain.toml`,
channel `1.97.1`), inside the lane worktree, default target dirs, `gate-logs/`
with `.exit` manifests. Toolchain stamped into every log header:

```
rustc 1.97.1 (8bab26f4f 2026-07-14)
host: aarch64-apple-darwin
```

| Leg | Log | Exit | Result |
| --- | --- | --- | --- |
| df preflight | `refix-df-preflight.log` | — | 125Gi available |
| F1 red (aion) | `refix-f1-red.log` | 101 | 2 passed, 1 failed |
| F1 red (aion-server) | `refix-f1-red.log` | 101 | 6 passed, 2 failed |
| F1 green (aion) | `refix-f1-green.log` | 0 | 4 passed |
| F1 green (aion-server) | `refix-f1-green.log` | 0 | 35 passed |
| `aion-rs --lib loader::` | `refix-green-suites.log` | 0 | 27 passed |
| `aion-rs --lib lifecycle::` | `refix-green-suites.log` | 0 | 71 passed |
| `aion-server --lib worker::` | `refix-green-suites.log` | 0 | 188 passed |
| `unserved_queue_policies` | `refix-green-suites.log` | 0 | 6 passed |
| `unserved_queue_honesty` | `refix-green-suites.log` | 0 | 1 passed |
| `cargo fmt --all --check` | `refix-fmt.log` | 0 | clean |
| `cargo clippy --workspace --all-targets -D warnings` | `refix-clippy.log` | 0 | zero warnings |

No `cargo test --workspace` was run at this seat and none is claimed. The full
workspace re-battery on the pinned tree rides the LANDING run on the fast
device, per the landing owner's routing; that run is the one that supersedes
the untoolchain-stamped 2026-07-28 battery above.
