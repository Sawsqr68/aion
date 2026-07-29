# Worker-connection dead-man switch — silence becomes a named death

Lane `build/dead-man-switch`, off `main` @ `2a31cf7d`.
Toolchain: `rustc 1.97.1 (8bab26f4f 2026-07-14)`, `aarch64-apple-darwin`, LLVM 22.1.6.
Charter: Tom's ruling 2026-07-29 ~09:05Z; plan ratified by Waffles 09:08Z; Hit D
added and its two rulings issued 09:13Z / 09:14Z.

One absent mechanism was hit **four** independent ways in one day on the
remote-gates proof. Every hit below carries the live evidence, the fix, and the
pin shown red on `main` and green on the lane.

---

## The ground, at the bytes

Two liveness domains existed and **neither fed the other**:

| domain | keyed on | advanced by |
| --- | --- | --- |
| per-activity liveness (`HeartbeatTracker` tasks) | `(worker, workflow, activity)` | the worker's progress heartbeat, **only while an activity executes** |
| connection lease (`HeartbeatTracker::connections`) | `worker_id` | `on_channel_publish` — i.e. only frames the worker happens to send |

An **idle** worker publishes on no channel and runs no activity, so it advanced
neither. Confirmed by live probe (relayed 09:xx Z): a worker restarted with
`--progress-interval 10` against a 30 000 ms window was still deregistered at
exactly the 30 s mark — registered 09:37:21, `"idle worker connection lease
expired"` 09:37:51 — **before it ever received a dispatch**. Idle death was
structural at any heartbeat cadence, not a razor margin.

On the worker side, `PushClient::recv_timeout` distinguishes a CLOSED socket
(reader thread ends → `Disconnected` → the existing redial fires) from a merely
DEAD one. A dead-but-open socket has no signal at all: every read returns
`WouldBlock`/`TimedOut` and every write lands in the kernel send buffer and
returns `Ok`.

---

## D1 as built, and the one deviation from the brief

The brief's D1 says *"Worker sends a lightweight ping … server answers."* That
shape is **not implementable in this lane**, and the deviation is reported rather
than improvised:

* the only worker→server frames the SDK exposes are `Frame::Publish` (answered
  with `PublishAck` **only** when the channel falls through to liminal's normal
  fan-out — a reserved aion channel is consumed by the notifier and gets
  `FrameAction::NoResponse`) and `Frame::PushReply`;
* `liminal::protocol::Frame::Ping`/`Pong` **does** exist and the server answers
  it unconditionally (`liminal-server-0.5.0` `src/server/connection/apply.rs`:
  `Frame::Ping { .. } => FrameAction::Respond(Frame::Pong { flags: 0 })`), but
  `liminal-sdk` 0.5.0 exposes no client-side ping and its reader **ignores**
  `Pong`. Reaching it means changing the liminal crates — a different repo, and
  cross-repo changes land in their own repo, never vendored.

So the direction is **inverted**: the SERVER pushes the ping, the WORKER answers
it, over the existing LSUB-0 `Push`/`PushReply` primitive that already carries
dispatches and interventions. One exchange proves **both** legs:

* the **pong's arrival** proves the worker to the server, and explicitly advances
  the connection lease — independent of any activity traffic;
* the **ping's arrival** proves the server to the worker, and its ABSENCE past
  the declared window is the worker's death signal.

Everything D1 asks for holds; only the initiator differs.

### Timings — derived, no knobs (D1's "no new configuration surface")

Both ends read one operator value, `worker.heartbeat_window` (default 30 000 ms):

* **ping cadence** = `sweep_interval(heartbeat_window)` — the *existing*
  quarter-window derivation the expiry sweeper already uses (7 500 ms at the
  default). **Four beats per window**, never equal to it.
* **worker silence window** = the heartbeat window itself, carried on **every
  ping** (`LivenessPing::silence_window_ms`). The worker holds no copy and no
  constant of its own, so the two ends can never drift and there is no second
  knob to misconfigure.
* the ping's reply deadline is the cadence, so at most **one** outstanding probe
  push per connection — it can never crowd out real dispatches against liminal's
  per-connection pending-push cap.

The worker's switch is **UNARMED** until the first ping arrives (at most one
cadence after connect). A worker talking to a server that never pings behaves
exactly as before; a zero window cannot disarm an armed switch.

### Files

| file | role |
| --- | --- |
| `crates/aion-worker/src/runtime/liminal_liveness.rs` | worker wire pair + lock-free `SilenceMonitor` |
| `crates/aion-server/src/worker/liminal_liveness.rs` | server wire pair + `LivenessProbe` |
| `crates/aion-server/src/worker/transport_loss.rs` | Hit D: transport-domain class + re-dispatch budget |
| `crates/aion-worker/src/runtime/liminal.rs` | ping demux, arming, declared link death |
| `crates/aion-worker/src/runtime/liminal_serve.rs` | Hit B worker half: every dial failure logged |
| `crates/aion-server/src/worker/liminal_transport.rs` | Hit B server half: every refusal logged; `push_payload`, `liveness_targets` |
| `crates/aion-server/src/worker/heartbeat.rs` | D2: expiry log gains queue + parked count |
| `crates/aion/src/runtime/nif_activity_retry{,_dispatch}.rs` | Hit D: attempt-neutral re-dispatch |

---

## Hit A — idle expiry, one-sided

**Live:** server log 08:01:46 `"idle worker connection lease expired; worker
deregistered"` ~37 s after the last activity. The worker was never told, kept
believing it was connected, and the next run's dispatch parked forever
(`"dispatch is parked on a queue that is not being served"`, 08:03:13).

**Fix (D1+D2):** pings arrive four times per window on an idle connection and
each answer advances the lease, so the idle expiry is **structurally unable** to
fire on a live worker. An expiry that genuinely fires now names the queue and the
parked-dispatch count (D2), captured from the registry *before* deregistration:

```
idle worker connection lease expired; worker deregistered
  worker_id=… task_queue=gates parked_dispatches=7 heartbeat_window_ms=30000
```

**Pin:** `crates/aion-server/tests/dead_man_switch_e2e.rs` →
`idle_worker_survives_past_the_old_lease_window`. Real liminal listener, real
notifier, real probe, real sweeper, real worker through the production
`serve_with_redial`; connect, then do nothing for 2× the window.

* **RED on `main`** (`gate-logs/dead-man-switch/red/hit-a-and-c-red.log`):
  > `the idle worker was deregistered: its connection lease expired while it was alive and connected — nothing kept the lease alive on an idle connection`
* **GREEN on the lane:** `test idle_worker_survives_past_the_old_lease_window ... ok`

---

## Hit B — silent registration rejection

**Live:** a worker whose contract field-mismatched the deployed `.v4` was refused
on EVERY dial. The server logged nothing on the `Rejected` ack; the worker's
redial loop dropped the error value unlogged and slept. A process sample showed
2674/2675 samples asleep in backoff. Both sides mute.

**Fix (D3):**

* **Server:** every refusal funnels through one `admit_registration` seam whose
  `Err` is logged at WARN with the queue, the worker build identity, the
  namespaces, the advertised activity types, and the **structured** reason —
  the same text that rides back in the ack.
* **Worker:** `serve_with_redial`'s connect closure carries `.inspect_err`, so
  each failed dial logs at WARN with the address, an **attempt counter**, and the
  error — which carries the server's refusal reason verbatim. **Rate-limit
  choice:** none added. The driver's existing bounded exponential backoff
  (`initial_backoff`…`max_backoff`) already paces the loop, and the attempt
  counter supplies the escalation an operator reads severity from; a second
  limiter would only be able to hide lines.

**Pin:** `crates/aion-server/tests/dead_man_switch_refusal_logs.rs` →
`a_refused_registration_is_named_in_both_logs_on_the_first_dial`. Real deployed
AWL contract (`charge(amount: Int)`), real worker registering a
contract-carrying `charge(amount: String)`. Asserts on both sides' captured
tracing output.

* **RED on `main`** (`gate-logs/dead-man-switch/red/hit-b-red.log`):
  > `the server refused every dial and logged NONE of them; captured: …`
  — the captured stream shows the connection accepted and torn down thirty-plus
  times with not one word about why.
* **GREEN on the lane:** `test a_refused_registration_is_named_in_both_logs_on_the_first_dial ... ok`,
  with the server line carrying
  `reason="listener accept failed: WORKER_CONTRACT_MISMATCH: worker build `refusal-pin-worker` queue `payments` refused: package `…` action `charge` field `input_schema.properties.amount.type` …"`
  and the worker's `attempt=1` line carrying the same text.

---

## Hit C — mid-run link death, both blind

**Live:** run `3a894965`. Pushes flowed serially (clone→check→clippy); the link
died at 08:33:41; the worker finished clippy and blocked on the dead socket for
**28 minutes**; the server recorded `ActivityStarted` for nextest/fmt whose
pushes went nowhere; completed leg results sat stranded on the worker.

**Fix (D1):** the worker measures silence against the server-declared window and,
on breach, logs at WARN with the address, the silence, and the window, then
returns `ServeResult::Dropped` — straight into the **existing, proven** redial
loop, which reconnects and re-registers. Server-side the unanswered pings stop
advancing the lease, so the existing expiry sweep reaps the registration and its
leases through the one idempotent deregistration path (deliberately no second
reaper).

**Pin:** `crates/aion-server/tests/dead_man_switch_e2e.rs` →
`a_wedged_link_is_detected_and_redialed`. A **wedgeable TCP relay** stops
carrying bytes in both directions and closes **neither** socket — the faithful
shape, since a relay that closed would prove nothing (a closed socket already
redials today). Candidates are `[relay, server]`, so a detected death has
somewhere honest to go.

* **RED on `main`** (`gate-logs/dead-man-switch/red/hit-a-and-c-red.log`):
  > `the worker never noticed its link was dead: no redial, no re-registration. A wedged connection is indistinguishable from an idle one unless something is expected to arrive on it`
* **GREEN on the lane:** `test a_wedged_link_is_detected_and_redialed ... ok` —
  a second, distinct registration appears and serves a real dispatch.

---

## Hit A′ — busy is not silent (found by building the fix, fixed in-lane)

Building the switch surfaced a fifth face of the same defect, and it is the one
the live stack complained about loudest: **"any leg longer than ~40 s reliably
loses its connection mid-work."**

A plain activity executes **INLINE** on the worker's serve loop. While it runs
the worker cannot dequeue — let alone answer — the server's pings, and on the
outbox path (`heartbeat_window_ms: 0`) the per-dispatch liveness pump does not
arm either. So **nothing** advanced the connection lease during a long leg, and
a worker was deregistered mid-work for the crime of working. Left alone, the new
switch would ALSO have made the worker tear down its own healthy connection the
moment such a handler returned, because the silence clock started when the
dispatch arrived — a self-inflicted regression strictly worse than the bug.

Both halves are closed:

* the silence clock re-arms on frame **completion** as well as arrival — the
  switch measures silence while the worker is LISTENING; time spent executing is
  work, not silence;
* the existing per-dispatch liveness pump now arms from **either** window: the
  dispatch's own (unchanged, the bridge path) **or the CONNECTION's**, learned
  from the ping. The pump publishes from a background task on a shared writer,
  so the lease stays alive whatever the serve loop is doing. No new mechanism —
  the proven one, given the window it was missing.

**Pin:** `a_long_activity_is_not_mistaken_for_a_dead_link` — an activity that
holds the serve loop for 2× the window, then a second dispatch on the SAME
registration.

* **RED without the fix** (`gate-logs/dead-man-switch/red/long-activity-red.log`,
  produced by reverting only the connection-window pump arming):
  > `the worker tore its healthy connection down after a long activity: the dead-man switch counted execution time as silence`
* **GREEN on the lane:** `test a_long_activity_is_not_mistaken_for_a_dead_link ... ok`

> **⚠ Residual — LEDGERED 2026-07-29, closes in its own lane.** The pump is armed
> by the FIRST ping, which arrives within a quarter window of registration. A
> dispatch that both lands inside that first quarter window AND outruns the whole
> window would still lose its lease.
>
> **Preferred close (Waffles, ruling update 11:28Z): the server sends its first
> liveness ping IMMEDIATELY on registration** rather than waiting for the next
> probe tick. That shrinks the unarmed gap from one quarter-window to roughly one
> round trip, adds no new mechanism, and changes no semantics.
>
> **Fallback: off-serve-loop spawn of plain liminal dispatches**, the way agent
> dispatches already are. This was the original direction; it demotes because it
> is a per-worker concurrency-semantics change and needs its own lane.
>
> The reasoning of record: the off-loop direction existed to keep the cure
> in-repo. First-ping-at-registration is *also* in-repo and is strictly less
> mechanism — and when a cheaper cure appears inside the same constraint that
> motivated a ruling, the ruling follows the constraint, not its earlier
> conclusion.
>
> **Recorded, not built.** Nothing in this lane implements either option.

## Hit D — retryable-but-never-retried on worker loss

**Live:** gate runs `dca75b24-bd33-414d-9841-4dc80e6a1f43` and
`e6fc9d83-d455-4c29-97a4-cdc8b59f8337`. In-flight activity, worker connection
lost, server records `ActivityFailed` with message
`retryable:worker WorkerId(2) lost before reporting activity result` but
`kind: Terminal`, and the workflow immediately takes the fault arm and dies.

**Root cause, at the bytes:** `lost_worker_error` produced
`ActivityErrorKind::Retryable`, which `PendingActivities::complete_activity`
collapsed into the string `retryable:…`. But `failure_stand_down` only retries
`Some(policy) if is_retryable_reason(reason) && attempt < policy.max_attempts`.
An activity with the SDK's run-exactly-once default (`"retry": null`) has **no
policy**, so the arm fell straight through to `RetryLoopTerminal::Failed(reason)`
— delivered verbatim, recorded terminal. The failure named itself retryable and
was structurally unable to be retried.

### The two domains (ruling, 09:14Z)

An authored `retry N` prices the probability that the **action** fails — a flaky
test, a racy build, a real red. A transport death is a different failure domain
that never produced a result. Charging it against the action's budget makes
authored semantics weather-dependent — **`retry 2` meaning less on a bad network
day** — precisely the property a gate runner cannot have. **Attempt-neutral is
ratified.**

**Fix:** a new outcome variant `ActivityCompletionOutcome::WorkerLost`
(a distinct domain, not a `Failed` wearing a retryable kind) is classified by
`transport_loss::TransportLossLedger` into one of two prefixes:

* `lost:` — still inside budget. The engine's retry loop re-dispatches the
  **same attempt**, records **nothing**, consumes **no** authored budget.
* `transport-exhausted:` — budget spent. No arm recognises it, so the run
  terminates naming the **transport**.

`lost_worker_error` is **deleted**, not deprecated — the old `retryable:` framing
has no remaining caller.

### The ceiling (ratified adversarial condition, 09:14Z)

Attempt-neutral must not be unbounded: a permanently flapping link — worker
registers, takes the dispatch, dies, forever — must still terminate.
Operationally confirmed the same day: a log-signature watchdog bounced the worker
every ~35 s for 25 minutes and drove one activity past attempt 4 with the action
budget untouched.

**Budget = `TRANSPORT_LOSS_BUDGET_WINDOWS` (4) × `worker.heartbeat_window`**, per
execution site, measured in wall clock from its FIRST transport loss (120 s at
the default window).

**Derivation, not invention.** The transport's whole detect-and-recover cycle —
declare the link dead within one window, redial, re-register, re-dispatch —
completes inside ONE window; four consecutive windows with not one successful
delivery is a flap, not a transient. It scales with the single operator value
that already declares what silence means, so shortening the window shortens the
budget and there is no new knob. A quarter/quadruple relation to that window is
the established derivation in this crate: `sweep_interval` is `window / 4` for
the same reason and is likewise underived by any separate setting.

> **✓ RATIFIED — Waffles, 2026-07-29 10:49Z.** The wall-clock ceiling stands, on
> the condition (met) that the derivation lives next to the constant: the
> rationale above sits in `transport_loss.rs`'s module docs, immediately beside
> `TRANSPORT_LOSS_BUDGET_WINDOWS`, so the number cannot drift away from its
> reason.
>
> The **count**-based alternative (N consecutive losses) was considered and
> **declined**: wall clock composes with the operator's own declared
> `worker.heartbeat_window`, so shortening the window shortens the budget with
> no second thing to configure, whereas a loss count assumes something about how
> fast a link flaps — a fast flap and a slow one would get wildly different real
> budgets from the same integer.

Book-keeping: a resolution OUTSIDE the transport domain (a real result, a real
action failure) retires the budget; a transport-domain resolution deliberately
does not — the running budget is exactly what bounds the flap. An unreadable
ledger reports exhaustion, never a re-dispatch: with no trustworthy budget the
only safe answer is the one that terminates.

**Pins:** `crates/aion/src/runtime/nif_activity_dispatch_tests/retry_worker_loss.rs`.

| test | RED on `main` | GREEN on the lane |
| --- | --- | --- |
| `a_worker_loss_redispatches_even_with_no_authored_retry_policy` | `a lost worker must never terminate the activity; got Failed` | ok |
| `a_worker_loss_consumes_no_authored_budget_and_records_nothing` | `two transport deaths under a 3-attempt policy must still complete` | ok |
| `a_flapping_link_terminates_naming_the_transport_not_the_action` | `the terminal must name the transport domain: lost:worker WorkerId(2) lost before reporting activity result` | ok |
| `a_settled_ordinal_stops_the_worker_loss_redispatch` | `an ordinal already settled elsewhere must stand the loop down` | ok |
| `an_action_failure_still_consumes_its_authored_budget` | passes on both — the regression guard that the transport class did **not** widen what counts as retryable | ok |

Red log: `gate-logs/dead-man-switch/red/hit-d-red.log`
(`test result: FAILED. 1 passed; 4 failed`).

---

## SIGTERM hang — reported out, not attributed

**What was asked:** whether the wedged state holds the shutdown path.

**What the evidence actually shows.** Both captured shutdowns in
`~/.aion/remote-gates-proof/` reached a **clean** drain:

```
server-live.log:236  "activity drain completed cleanly"   09:40:14.357276Z
server3.log:82       "activity drain completed cleanly"   08:20:23.903533Z
```

so whatever hung was **after** the activity drain, not in it. No log tail or
thread sample from the shutdown that actually needed `kill -9` exists in the
evidence directory.

**Static audit of the post-drain exit path** found no unbounded wait:

* the queue-service park consults the drain gate (`accepting()` in
  `queue_service/wait.rs`) and breaks out with `NotAccepting`, so a parked
  dispatch does not wedge the blocking pool;
* `receive_bridge_reply`'s unbounded wait re-arms on `keep_waiting()`, which goes
  false once the expiry sweep retires the task — bounded by window + one poll;
* `ServerListener::drop` → `stop_worker` self-connects to wake the blocked
  `accept` and joins one worker thread.

**Verdict: not attributable to this mechanism on the available evidence, and
therefore not fixed in this lane.** What this lane *does* change is the trigger:
a permanently wedged connection can no longer persist — the worker tears it down
within one window and the server reaps the registration — so the state the hang
was observed in is no longer reachable for longer than the detection window.

**To settle it**, one artefact is needed: a `sample <pid>` (or `lldb` thread
backtrace) of the hung process, or the server log tail from that specific
shutdown. Filed for whoever next reproduces it.

**This lane adds no new exit hazard:** the probe's `spawn_blocking` push is
bounded by the ping deadline (one cadence, 7.5 s at the default window), so a
SIGTERM mid-round costs at most one cadence at runtime drop.

---

## Battery — at the committed tree (`c3709681`)

All logs + `.exit` files under `gate-logs/dead-man-switch/`.

| gate | verdict (the log's own result line) | exit |
| --- | --- | --- |
| `cargo fmt --all --check` | clean | `0` |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean | `0` |
| `cargo test --workspace --no-fail-fast` | **3070 passed, 0 failed, 6 ignored** across 224 suites | `0` |
| `lsub5b_osproc_failover_e2e` (alone, `--ignored`) | `osproc_liminal_listener_accepts_real_worker_connection ... ok` — `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 41.84s` | `0` |
| `lsub5b_osproc_kill9_failover_e2e` (alone, `--ignored`) | `osproc_kill9_liminal_failover_completes_exactly_once ... ok` — `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 38.53s` | `0` |

Both osproc gates ran ALONE, after the workspace battery released the machine,
per their harness law — real 3-OS-process clusters, their own logs, their own
`.exit` files.

**One unexplained diff, now explained:** `spike/liminal-fan-worker/Cargo.lock`
(+521 lines) rode `e7d50822` without a word in its commit message. It is not a
change this lane authored — the lsub5b gates build that spike worker via
`build_worker_binary`, and doing so realigned its stale path-dep pins (`0.8.0` →
the workspace's current `0.10.0`) and refreshed the transitive graph. Running the
gates is what wrote it; committing the tree carried it. Recorded here so nothing
in the lane is unaccounted for.

### Two earlier battery runs, and what each caught

Recorded because each did work a green run cannot show.

**At `00754854`: `3065 passed, 5 failed`.** Every failure was a test still
pinning `retryable:` for worker loss.

An earlier battery at `00754854` came back `3065 passed, 5 failed`. Every failure
was a test still pinning `retryable:` for worker loss — the exact framing Hit D
deletes — and in each case the test's own doc comment stated the intent that
string never delivered ("retryable so the engine can re-dispatch"). All five now
pin the transport-loss class through the shared constant; three were renamed,
because a test called `..._fails_retryable_lost_worker` teaches the wrong
contract to the next reader. Landed in `e7d50822`.

**At `e7d50822`: green (3070/0/6), and still not the final word.** Byte review of
that tree found three things a passing battery cannot see: a ZERO transport-loss
budget on all four embedder `from_parts*` constructors (an embedder-booted server
got one re-dispatchable loss, then transport-exhausted), the deleted `retryable:`
vocabulary still live in two operator-facing WARN lines and a dozen doc comments,
and a stale ratification block in this document. Fixed in `c3709681`; the table
above is the re-run at that tree. Green is a floor, not a verdict.

## Laws

* No `#[allow]`, no `#[ignore]`, no `unwrap`/`expect`/`panic` outside `cfg(test)`.
* No new configuration surface. Both timings and the transport budget derive from
  `worker.heartbeat_window`.
* No backwards compatibility: `lost_worker_error` and the `retryable:`
  worker-loss framing are **deleted**; both sides of the ping wire change
  together with no version negotiation.
* **File sizes.** All NEW logic went into new modules
  (`liminal_liveness.rs` ×2, `transport_loss.rs`) precisely so no touched file
  had to grow. `liminal_transport.rs` (2 369 lines) and `bridge.rs` (2 141) were
  already over the 500-line law before this lane; the additions to them are the
  minimum seam wiring (`push_payload`, `liveness_targets`, the refusal-log split,
  the `WorkerLost` arm). **Extracting either is a pre-existing debt this lane
  does not discharge** — flagged, not smuggled.
