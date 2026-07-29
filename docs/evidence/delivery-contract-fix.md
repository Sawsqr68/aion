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

## Phase 2 — the fix (in progress)
