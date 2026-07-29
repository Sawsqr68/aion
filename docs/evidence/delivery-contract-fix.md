# Delivery-contract fix wave — lane evidence

Lane: Option 1 + riders, ruling `755737bb`, topic task `83c6dbc6`. Owner:
Vesper Lynd. Execution venue: Dean's box (Tom's routing, 2026-07-29 16:04Z) —
authoring at the owner's seat, every build/check/gate remote. Analysis of
record: `docs/design/DELIVERY-CONTRACT-SCOPING.md` (main `e958ca28`); executor
brief: `docs/briefs/delivery-contract-fix-wave.md` (branch
`brief/delivery-contract-fix-wave`).

## Phase 0 — pre-build verification (COMPLETE)

### 0.1 Completion-fence coverage on the outbox liminal path — VERIFIED PRESENT AND SOUND

The scoping doc's caution ("touched at its edges") is confirmed benign. The
path already fences every dispatch attempt (`liminal_transport.rs`
`RegistryLiminalDispatch::dispatch`):

- `completion_fences.issue(workflow, activity)` BEFORE the push — issue
  **supersedes** any prior generation (`envelope.rs:69`): exactly one current
  token per `(workflow, activity)`.
- The token travels in the wire request (`request_for_row(row, run_id,
  &completion_token)`).
- Every dispatch-error exit **revokes** the token — and `revoke` only removes
  an exact match, never disturbing a newer retry's token (`envelope.rs:120`).
- `accept` consumes only the exact current generation in one mutex critical
  section; stale → typed `StaleGeneration`, absent → `NoCurrentGeneration`
  (`envelope.rs:89`). Two concurrent submissions cannot both become truth.
- Delivery re-enters through `LiminalCompletionSource::deliver` → the shared
  `OutboxDeliveryCallback` with terminal dedup (`record_fan_out_completion`),
  same as gRPC.

Implication for Option 1: with the unbounded wait there is exactly one live
token per activity execution. The attempt-neutral re-dispatch after a real
worker loss revokes-then-reissues — still fenced. The fix does not restructure
this; it removes the only unfenced case (transport re-push of a live
execution).

Also recorded: `mark_done` failure leaves a row `claimed` forever, loudly
logged (`outbox_dispatcher.rs:478`) — pre-existing posture, unchanged by this
lane.

### 0.2 Liminal pending-push cap under a held push — VERIFIED AT SOURCE, NO RUNTIME HARNESS NEEDED

`liminal-server 0.5.0` `config/types.rs:298`: `max_pending_pushes_per_connection`
default **32** (§5), zero rejected at config validation as a typed error,
over-cap admission refused loudly at push time. A held outbox dispatch + one
probe ping = 2 of 32 per connection under the one-row-per-connection design —
the cap never binds. Margin is 16×; a runtime harness would prove nothing the
source and validation tests don't already pin.

Bonus datum for later lanes: 0.5.0 exposes
`push_to_connection_with_deadline` with a distinct TERMINAL
`PushReplyExpired` variant (slot removed, cap admission released) — not used
by Option 1 (the wait is unbounded by contract), noted so nobody rediscovers
it.

## Venue record

- Dean's box: `deanwhiting@100.68.29.10` (Tailscale), 15 cores, 640G free,
  cargo 1.97.1. Repo shipped as 626M git bundle (no credentials on that box),
  cloned at `~/aion-gates/aion`, HEAD = main `e958ca28` (byte-identical to
  the scoping tree). Initial `cargo build --workspace --all-targets` detached
  as pid 25312, log `~/aion-gates/build-initial.log`.
- Seth Crackers' queue law: builds + targeted pin runs = disclosed load, no
  slot; BATTERIES slot through him, one at a time, request format LANE /
  VENUE / SHAPE / DURATION / READY-STATE. Queue empty at 16:08Z; four other
  lanes expected on the box tonight — compile overlap expected and irrelevant
  to pin verdicts; timing-shaped battery reds are suspect and re-run
  clean-box serialized via Seth.

## Phase 1 — red pins: ALL FOUR SHOWN RED (Dean's box, Vesper)

Pin file: `crates/aion-server/tests/outbox_delivery_contract_e2e.rs`
(branch `fix/delivery-contract`). First red run on the UNFIXED tree
(main `e958ca28` + pins only): Dean's box, operator Vesper, nextest run ID
`fae918c3-2b6b-4055-a7e7-4240e76212c7`, log
`~/aion-gates/pins-red-run1.log` on that box (copy committed alongside this
doc). Completion marker present:

```
Summary [ 125.237s] 4 tests run: 0 passed, 4 failed, 0 skipped
  FAIL [   0.573s] dead_letter_is_genuine_and_loud
  FAIL [  60.416s] re_push_while_executing_never_re_executes
  FAIL [  90.659s] ninety_second_activity_delivers_exactly_once
  FAIL [ 125.236s] wedge_shape_regression_0756ecd5
```

Every red is a NAMED-DEFECT ASSERTION, not a timeout:

- **(c) 0.573s** — `SILENT DEAD-LETTER: the row dead-lettered but nothing
  reached the delivery callback — the workflow reports Running forever (the
  liveness lie)`. The guard half held: the genuinely-undeliverable row did
  dead-letter; the loud half is the red.
- **(b) 60.416s** — `DUPLICATE EXECUTION: leg 'minimal-overrun' started 2
  times — the outbox re-pushed a dispatch the worker was already executing`.
- **(a) 90.659s** — `DUPLICATE EXECUTION: leg 'ninety' started 2 times` —
  the 90-second contract pin trips on the duplicate (t≈90s, the moment the
  serial worker finishes leg one and dequeues the doomed re-push), before the
  dead-letter it was also racing toward.
- **(d) 125.236s** — `DUPLICATE EXECUTION: leg 'long-clippy' started 2
  times` — the wedge shape reproduced: fast leg fine, long legs re-pushed
  against a live serial worker, exactly the exhibit's mechanics.

The timings themselves confirm the mechanism: every duplicate lands right
where the 30-second bounded awaiter predicts it. The defect is pinned at the
bytes; fix code may now be written.

## Phase 2 — the fix: FULL DESIGN (verified at the bytes, ready to implement)

Every claim below was verified by reading the named source this session.

### 2.1 `liminal_transport.rs` — the held wait (~40L)

- New `LiminalWorkerDelivery::dispatch_held(request, keep_waiting) ->
  Result<Option<DispatchResponse>, ServerError>`: `push_dispatch()` then
  `receive_bridge_reply(&awaiter, keep_waiting)` — literally SHARES the bridge
  wait loop at `:780`. "Two wait policies" becomes "two callers of one policy."
- `RegistryLiminalDispatch::dispatch` (`:997`): replace the bounded
  `delivery.dispatch(&request)` inside `spawn_blocking` with `dispatch_held`.
  `keep_waiting = !shutdown_flag && in_flight.contains(dispatch_key) &&
  registry.worker_by_id(worker.id())?.is_some()` — ALL SYNC-callable from the
  blocking thread (`worker_by_id` exists, `registry.rs:775`; registry locks are
  sync). `Ok(None)` (abandoned) → revoke fence token, WARN loudly (rider 3
  half 1: "delivery wait abandoned; late reply will be discarded"), return
  WorkerDispatch error so outbox policy re-drives.
- **Cap classification fix**: `push_dispatch`'s error mapping currently maps
  EVERY `push_to_connection` error to `WorkerConnectionLost` ("exactly one
  cause" — FALSE in liminal 0.5.0: `ServerError::ConnectionCapReached { cap:
  "max_pending_pushes_per_connection" }` is a distinct typed refusal,
  supervisor.rs:2416). Map cap-reached to a new typed `worker busy` error;
  everything else stays connection-lost. Without this, stacked held pushes at
  the cap would masquerade as worker death → attempt-consuming immediate
  re-arm churn.
- Rider 3 half 2: WARN at `LiminalCompletionSource::deliver` (`:498`) on fence
  rejection (Stale/NoCurrentGeneration) — a late reply that does arrive is
  logged, never silently swallowed.

### 2.2 `outbox_dispatcher.rs` — per-row delivery + in-flight set (~120L)

- `InFlightSet = Arc<Mutex<HashSet<String>>>` shared dispatcher⇄reconciler⇄
  dispatch impl. RAII guard: insert before spawn, remove when the row's task
  finishes (every path).
- `sweep_once`: skip any claimed row already in-flight (defense vs reconciler
  re-arm races); otherwise insert + `tokio::spawn(self_arc.process_row(row))`.
  `run()` wraps self in `Arc` for task cloning. Concurrency: unbounded per
  claimed row — the worker's serial execution + liminal's 32-push cap bound
  reality; cap refusal is now typed busy (below), so over-cap is honest wait,
  not dead-letter fuel.
- `handle_dispatch_error`: NEW ARM — worker-busy → **attempt-NEUTRAL** re-arm:
  `retry_outbox_row(key, row.attempt /*unchanged*/, now + backoff_base)`,
  WARN. Mirrors the connection-lost arm's shape (which stays
  attempt-consuming per its documented policy).
- Shutdown: dispatcher sets an `Arc<AtomicBool>` when `run` exits; the flag is
  part of `keep_waiting`, so held waits abandon on drain. Rows stay Claimed;
  post-restart the reconciler re-arms them (in-flight set is empty then —
  correct recovery).
- NO new config. NO wire change. NO worker change.

### 2.3 `outbox_reconciler.rs` — in-flight exemption (~40L + store change)

- ⚠️ REFINEMENT (race found during implementation): re-arming individually via
  `retry_outbox_row` after a `list_stale` is UNSAFE — a row that completes
  (`mark_done`) between the list and the re-arm would be flipped Done→Pending
  and re-executed. The re-arm must stay a STATUS-GUARDED bulk store operation.
- Therefore: change the store trait method to
  `rearm_stale_claimed_outbox_rows(older_than, now, batch, excluded:
  &HashSet<String>)` (replace, don't add alongside — no-compat rule; mirrors
  the existing `claim_outbox_rows_excluding` precedent). Memory backend:
  filter; libsql: `AND dispatch_key NOT IN`; haematite: filter in its rearm
  path. Conformance test: an excluded stale claimed row is not re-armed, a
  non-excluded one is.
- Reconciler builder gains `.with_delivery_gate(gate)`; sweep passes
  `gate.snapshot_keys()` (or empty set when absent) as the exclusion. #253
  liveness gate unchanged.

### 2.x Implementation state (updated as edits land)

- ✅ `error.rs`: `WorkerBusy` variant + constructor + `is_worker_busy()` +
  wire arm + trace-fields arm — DONE in worktree.
- `DeliveryGate` (new type, lives in `outbox_dispatcher.rs`): clonable Arc
  handle — `begin(key) -> Option<DeliveryGuard>` (None = already in flight or
  poisoned; guard removes on Drop), `holds(key) -> bool` (poison → false =
  abandon, logged), `drain()`/`is_draining()` (AtomicBool),
  `snapshot_keys() -> HashSet<String>`. Dispatcher CREATES it in `new()` and
  exposes `delivery_gate()` so state.rs/tests hand the SAME gate to
  `RegistryLiminalDispatch` (new 3rd ctor arg — update lsub1/lsub3/lsub5/pins
  construction sites) and the reconciler.
- Dispatcher `run(self)`: wrap self in `Arc::new`, sweep spawns
  `tokio::spawn` per claimed row holding the gate guard; `gate.drain()` on
  run-exit. `sweep_once` becomes `async fn sweep_once(this: &Arc<Self>)`.
- `handle_dispatch_error`: busy arm BEFORE the budget check → attempt-neutral
  `retry_outbox_row(key, row.attempt, now + backoff_base)` + WARN. Dead-letter
  arm: after `fail_outbox_row`, `failure_callback.deliver_failure(...,
  "infrastructure: ...")` via new optional builder
  `.with_delivery_callback(cb)`.
- Transport: `is_connection_cap_error` helper matching liminal
  `ServerError::ConnectionCapReached{..}` BY TYPE; `push_dispatch` +
  `push_payload` map cap→`ServerError::worker_busy`, everything else stays
  connection-lost. `dispatch_held(request, keep_waiting)` = `push_dispatch` +
  `receive_bridge_reply`. `RegistryLiminalDispatch::dispatch`: spawn_blocking
  runs `dispatch_held` with `keep_waiting = !gate.is_draining() &&
  gate.holds(key) && registry.worker_by_id(id).map(|w|w.is_some()).unwrap_or(false)`
  (poison → abandon); `Ok(None)` → revoke token + WARN abandonment + return
  WorkerDispatch error. Fence-rejection WARN at
  `LiminalCompletionSource::deliver` (`accept` error → WARN before `?`).

### 2.4 Dead-letter honesty + redrive (riders 1–2, pin (c))

- Dead-letter arm gains failure delivery: after `fail_outbox_row`, call
  `callback.deliver_failure(wf, activity, run, "infrastructure: delivery to
  worker failed after N attempts: <error>")` → engine
  `deliver_outbox_failure` → recorded activity failure → the workflow's OWN
  retry/failure semantics take over. That IS the explicit
  infrastructure-failed state: never a silent `Running`. Dispatcher gains the
  callback via builder (`.with_delivery_callback`), wired where the server
  builds the dispatcher (state.rs).
- **Redrive semantics resolved this session**: after a SUCCESSFUL failure
  delivery, judgment has passed — auto-redrive would re-execute a
  non-idempotent activity whose failure is recorded (late result would be
  dedup-dropped but side effects would run) → NO auto-redrive of
  failure-delivered rows. Redrive covers rows whose failure delivery FAILED
  (workflow untold, still Running). ⚠️ OPEN ITEM before implementing rider 1:
  read `crates/aion-server/src/worker/outbox_settle.rs` — a settlement seam
  for terminal rows already exists (`list_unsettled_outbox_workflow_ids`) and
  may already carry half of this; do not build a parallel mechanism.
- Store-trait addition for redrive (three backends + conformance) sized ~1
  session; sequence it AFTER the core fix greens the four pins.

### 2.5 Verification sequence

1. Core fix (2.1–2.3) → pins (a), (b), (d) green + (c) guard half.
2. Rider 2 (2.4 failure delivery) → pin (c) fully green.
3. `cargo clippy --workspace --all-targets -- -D warnings` + `cargo fmt` clean
   on Dean's box.
4. Rider 1 (redrive) with its own store-conformance tests.
5. Battery via Seth's slot (LANE/VENUE/SHAPE/DURATION/READY-STATE format).
6. Acceptance: wake-prompt shakedown re-run (needs the rig redeployed from
   the fixed tree — coordinate with Waffles; compute placement per the
   coordinate-only doctrine).

### Execution state (resume anchor)

- Local worktree `.worktrees/delivery-fix`, branch `fix/delivery-contract`
  @ a70e8e5d (pins + evidence + red log, PUSHED to origin).
- Dean's box `~/aion-gates/aion` on branch `fix/delivery-contract` with the
  pin file rsynced (its git state is behind local — sync files, not commits;
  land from the LOCAL branch only).
- Iteration loop: edit locally → `rsync` changed files →
  `ssh … cargo check/nextest` on Dean's box. Nothing compiles on Tom's box.
- Doctrine: progress → LANE TOPIC (task 83c6dbc6, respond); DMs to
  Cally/Waffles = decisions/blockers only. Every evidence line: machine +
  operator ("Dean's box, Vesper").
