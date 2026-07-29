# The outbox delivery contract cannot span a real activity — scoping

Investigation-mode assignment, 2026-07-30 (Cally's relay of Waffles' ask: effort,
risk, what the fix touches — file-level). Primary exhibit: wedged run
`0756ecd5-9569-4dc9-ba3a-c0ecb043f701` (preserved untouched). Secondary
exhibits: every gate run of 2026-07-29 in the same store.

## Verdict up front

**One defect family.** This morning's dead-letters and tonight's wedge are the
same defect. Yesterday's dead-man-switch lane fixed a *different* defect (the
connection lease) that was stacked on top of this one; landing it is what
isolated this one cleanly. The delivery contract itself has a hard **~30-second
per-activity execution ceiling**, and no genuinely long activity has ever been
delivered successfully through the outbox liminal path — every apparent success
in the rig's history was a leg that happened to finish inside 30 seconds.

This is the single blocker between the estate and off-machine testing.

## The defect at the bytes

The outbox liminal path uses a **reply-when-done contract over a bounded
awaiter**:

- The worker executes a plain activity INLINE on its serve loop and replies the
  `DispatchResponse` — carrying the result — **on the push correlation, only
  when the activity completes** (`aion-worker` `liminal.rs`).
- The server's delivery blocks on that reply for
  `PUSH_REPLY_TIMEOUT = 30s` — a **const**, `liminal_transport.rs:90`, whose doc
  comment ("generous because an activity may legitimately run a while") is
  false for any real leg. `LiminalWorkerDelivery::dispatch`
  (`liminal_transport.rs:611`) maps the timeout to the outbox's normal
  backoff path.
- The outbox dispatcher drives claimed rows **sequentially**
  (`outbox_dispatcher.rs:431` `for row in rows`, `:472` dispatch → `:473`
  `mark_done`), so one slow row also stalls its siblings' retries.
- The retry budget is 5 attempts with millisecond-scale backoff; each attempt
  spends up to 30s in the awaiter, so total patience is **~2.5 minutes against
  an activity that takes tens of minutes**. Then the row dead-letters.
- Each retry **re-pushes the dispatch**, and the worker — which is still
  faithfully executing the first push — will later dequeue and **re-execute the
  same activity** (at-least-once execution with no idempotency fence; history
  records `ActivityStarted attempt 1` exactly once while the worker runs the
  leg multiple times — history and reality disagree).
- A completion can only land on a **live** awaiter. The worker's reply to an
  abandoned correlation vanishes **silently** — there is no log line anywhere
  for "reply arrived for an abandoned correlation." Completed work is
  discarded without a word (a silent-failure violation in its own right).
- After dead-letter: no redrive exists (#61), and the workflow reports
  `Running` forever — the liveness lie.

## Evidence chain: one defect, three faces

**Tonight — the pure isolate (run `0756ecd5`, meridian main, known-green
input).** Worker registered the entire time; **zero lease expiries** (the
dead-man switch held: 49 unanswered-ping WARNs during busy windows, no
deregistration — the landed mechanism did its job). Rows :2/:3/:4 (clippy,
nextest, fmt) each burned five delivery attempts between 12:35:38 and 12:39:09
and dead-lettered; the check leg — 8 seconds on a warm checkout — delivered
fine. Dean's box received duplicate re-pushes (worker log: `run_nextest`
delivery `attempt=3`) and was still grinding doomed duplicates 70 minutes
later. `server-remote.log` 12:35:38–12:39:09; run history seq 1–15.

**Yesterday morning — the same defect with the lease defect stacked on top.**
Runs `3a894965`, `e6fc9d83`, `dca75b24`, `ae0273fd`, `539f2363`: long legs
thrashed the delivery retries (duplicate-execution evidence: `…-attempt4.log`
leg logs on the local worker), then the connection lease expired mid-work
(the now-fixed defect), the worker deregistered, and the rows dead-lettered
against "no worker registered."

**The apparent successes — survivorship, not health.** The fixture runs'
legs take 1–4 seconds. The iridium verdict run (`2c3b6227`, 09:43) completed
all four legs in **13–19 seconds each** — because three earlier (doomed) runs
had already fully built iridium's target on the same checkout; its famous
"3m37s" was `notify_result` alone (09:43:32 → 09:46:47). Every leg that ever
completed fit inside the 30-second awaiter. **`worker.heartbeat_window` is
irrelevant to this defect** — the timeout is a const; reverting the 600s
mitigation did not cause tonight's wedge, it merely let the worker live long
enough to expose the delivery ceiling purely.

## Why the bridge path does not have this bug

The engine-seam bridge dispatcher already owns the correct wait:
`receive_bridge_reply` (`liminal_transport.rs:780`) — **unbounded by
contract**, re-armed each `BRIDGE_REPLY_POLL`, with a `keep_waiting()`
abandonment gate, a **prompt typed wake on real connection death**
(`PushReplyDisconnected`, matched by variant not message), and a dedicated
thread. The module's own words: "one frame format, one push primitive, two
wait policies." The outbox path got the wrong policy.

## Fix options (the fork is Waffles' to rule)

### Option 1 — adopt the bridge wait policy on the outbox path (recommended)

Delivery = push once, then wait unbounded with
`keep_waiting = row still claimed by this dispatcher AND worker still
registered`. Two mechanisms that landed yesterday are the prerequisites that
make this small and correct:

- the **dead-man switch** makes "worker still registered" a truthful liveness
  signal (before it, an unbounded wait gated on registration would wait on a
  lie);
- the **transport-loss class** already handles the real-death wake:
  `PushReplyDisconnected` → `WorkerConnectionLost` → attempt-neutral
  re-dispatch under the 4×window budget.

The retry budget then prices only genuine delivery failures (push refused,
connection lost) — which is what its 5×ms-backoff shape was always sized for.

**What it touches, file-level:**

| file | change | est. lines |
| --- | --- | --- |
| `crates/aion-server/src/worker/liminal_transport.rs` | outbox `dispatch` adopts the bridge-style poll-wait (share `receive_bridge_reply`'s loop; keep the typed Disconnected/Timeout split) | ~40 |
| `crates/aion-server/src/worker/outbox_dispatcher.rs` | per-row delivery moves off the sweep loop — spawn per claimed row (or bounded pool) with an in-flight set so a held row is neither re-claimed nor re-pushed | ~100–150 |
| `crates/aion-server/src/worker/outbox_reconciler.rs` | in-flight rows exempt from stale re-arm (heartbeat the claim stamp during the held wait, or consult the in-flight set) — otherwise the reconciler recreates the duplicate-push bug on a longer fuse | ~30–60 |
| `crates/aion-server/src/worker/outbox_delivery.rs` | loud late-reply: a reply for an abandoned correlation logs at WARN, never vanishes | ~20 |
| tests | red-first pins: (a) a 90-second activity through the outbox liminal path is delivered **exactly once** and completes (red on today's tree — dead-letters at ~2.5 min); (b) duplicate-execution pin (re-push while executing = the bug); (c) dead-letter fires only on genuine delivery failure; (d) wedge-shape regression from exhibit `0756ecd5` | ~400–600 |

No wire change. No worker change. No new configuration surface.

**Design note discovered in scoping:** the worker executes plain activities
serially, so delivering **one row per worker connection at a time** is not a
limitation — it is the natural backpressure that matches the execution model,
and it sidesteps liminal's per-connection pending-push cap entirely (one held
dispatch + at most one probe ping in flight per connection).

**Effort: M** — two to three focused build sessions plus one battery slot.
**Risk: MEDIUM** — the exactly-once bookkeeping (`mark_done`, completion
fences) is touched at its edges but not restructured; the two named races
(re-claim of a held row; reconciler re-arm mid-execution) are exactly what the
in-flight set and the pins exist to kill. Pre-build verification items:
completion-fence coverage on the outbox path, and liminal-server 0.5.0's
pending-push-cap behavior under one held push (expected fine under
one-per-connection; verify, don't assume).

### Option 2 — immediate receipt-ack, completion on a separate channel

The cleanest long-term contract (delivery ≠ execution), but it changes the
wire: the worker replies an ack at receipt and publishes the completion later,
so both sides change in lockstep, the server grows a second completion path,
and every SDK eventually follows. **Effort: L (1–2 weeks). Risk:
MEDIUM-HIGH.** The right eventual shape if Option 1 hits a wall; not the
first move under a freeze-recovering estate.

### Option 3 — duration-aware bounded awaiter (authored timeout + margin)

Smallest textual change, wrong shape: it still holds a delivery slot for the
whole leg, needs the same dispatcher concurrency restructure to be usable, and
requires threading the authored per-action timeout into `OutboxRow` (store
schema change across three backends) — more surface than Option 1 for a worse
contract. Listed for completeness; not recommended.

## What must ride the fix regardless of option (#61, #34 residue)

1. **Redrive** — dead-lettered rows re-enter `Pending` on worker
   re-registration (and/or an operator verb). Dead-letter must mean "awaiting
   judgment," never "abandoned." Store-trait addition → touches
   `aion-store` + all three backends (memory, libsql, haematite): the largest
   single rider, ~1 session.
2. **Terminal honesty** — a workflow with a dead-lettered row must not report
   `Running` in silence: an infrastructure-terminal state or an explicit
   requeue, never nothing. (Which of the two is a ruling.)
3. **Loud late-reply** (folded into Option 1's table above).

## Severity note beyond the gates

Re-push-while-executing means **at-least-once execution without idempotency
fences on the activity path** — for a durable workflow engine aimed at
financial/legal/healthcare work, that is a correctness bug independent of the
gates use-case. Option 1 removes this case entirely. The *deliberate*
at-least-once that remains (attempt-neutral re-dispatch after a real worker
loss) is the documented Temporal-class contract, and should be stated in the
engine docs as such.

## Verification plan (when a slot is granted)

Red pin first (the 90-second activity), then the fixed tree greens it, then
the wake-prompt shakedown re-run — unchanged as the acceptance gate: known-green
input, the rig as the only variable. The wedged run `0756ecd5` stays preserved
as the shape reference until the regression pin exists.

## Addendum — the hot wedged server is a different defect (Apollo's datum, run down)

Apollo observed the wedged rig server at 83% CPU sustained with nothing left to
serve, and hypothesized the dead-letter retry loop was still churning. Sampled
at the bytes (`sample 49862`, 3s, full thread stacks): **refuted.** The outbox
is quiet — `server-remote.log` has no dispatcher line since 12:41Z, consistent
with dead-letter being terminal for the dispatcher — and no thread spins: every
thread in the sample sits in a wait primitive. The burn is distributed
**wake-churn in beamr's scheduler threads** (`beamr-sched-0..3`, `dirty-cpu-*`,
`beamr-readiness-poll` cycling `semaphore_timedwait`/crossbeam timed parks)
while the engine hosts the wedged run's resident workflow process. An idle-ish
server burning most of a core is a real defect, but it is **beamr's idle-cost
question** — already open with Artemis' seat — not this one. Recorded here so
the two are never conflated; sample preserved at the session scratchpad
(`rig-sample.txt`).
