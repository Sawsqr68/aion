# #58 acceptance — what is proven, and what is not

Both musts were specified as rig-level demonstrations. Both are now **automated
pins with inverted controls**, which is strictly stronger: deterministic,
repeatable, and running in every battery rather than once under a person's eye.
One of the two is proven at a narrower seam than the rig would have covered, and
that narrowing is stated below rather than glossed.

## Must (a) — a busy worker still answers the server's liveness ping ✅

`dead_man_switch_e2e::a_busy_worker_still_answers_the_servers_liveness_ping`

Real liminal listener, real `LivenessProbe`, real `HeartbeatSweeper`, real remote
worker through the production `serve_with_redial` entry point. A plain activity
holds the worker for three windows while the main thread asks the server's own
question — `HeartbeatTracker::is_dispatch_reachable` — at a point more than one
full window after the leg began.

**Non-vacuity is structural**, not asserted by hand: reachability is seeded by
the registration handshake and lives exactly one window, so a seeded value is
already stale when the check runs. Only a ping answered *while the handler was
running* can hold it true. The test asserts that the observation point really is
past a window before it asserts the property.

**Watched failing.** Against the pre-#58 behaviour (plain dispatch executed
inline on the serve loop, restored temporarily), it fails on exactly its own
assertion:

```
a worker busy with a long activity must still be answering pings: the server can
only keep its dispatch eligibility if the serve loop stayed free to answer while
the handler ran
```

**Substituted for the 45-minute declared-class leg**, deliberately. The
mechanism under test is "can a ping be dequeued while a handler runs", and that
does not become more true at 45 minutes than at three windows — it is the same
loop, either free or not. What a 45-minute leg would add is confidence about
*accumulated* effects over a long run, which is a different question and is not
claimed here.

## Must (b) — a pump-alive worker the server cannot reach loses eligibility ⚠️ WIRING ONLY

`registry::tests::a_pump_alive_worker_the_server_cannot_reach_is_not_selected`

Real `HeartbeatTracker` census feeding a real `ConnectedWorkerRegistry`, staged
as Finding B exactly: the worker's pump beats well past the window (every
process-is-alive signal says alive) and no ping is ever answered (the one
server-can-reach-it signal is absent). Asserts marked ineligible AND excluded
from `select_worker`, with the baseline (freshly registered ⇒ selectable) and the
recovery (one answered ping ⇒ selectable again) as controls, so a registry that
selected nobody could not pass.

**Watched failing** by removing the eligibility filter from `select_worker`; it
fails on the SELECTED assertion.

### 🔴 What is NOT covered, and why

**There is no transport-level e2e for (b).** The staging needs a
server→worker-only wedge — the worker's frames still arriving while nothing the
server pushes can land. The relay now has independently wedgeable directions for
that purpose. The test was not built, for a reason that is a finding in itself:

**The worker's own dead-man switch races the server's, on the same window.** Wedge
server→worker and the worker sees silence too, declares link death within one
window, tears down and redials. So the scenario does not hold still: it becomes
"worker redials repeatedly", not "worker stays connected while the server cannot
reach it".

And redial exposes something worth checking on its own:

> **`register_connection` seeds dispatch reachability, so every redial re-seeds
> it.** A worker the server can never push to will therefore cycle: register,
> look reachable for one window, go stale, be excluded, redial, look reachable
> again. It never settles into exclusion.

That may be correct — a fresh connection IS a completed round trip, and refusing
a brand-new worker on the previous connection's evidence would be wrong. But it
means **exclusion is not stable under redial**, and nothing currently bounds the
cycle. Whether that is a defect or the intended shape has NOT been decided, and
it should be decided rather than discovered later. Recorded as the open half of
(b).

## Status

Must (a): discharged, automated, inverted-controlled.
Must (b): wiring discharged, automated, inverted-controlled; transport-level
staging open, with the redial re-seeding question named above as the thing to
settle first.
