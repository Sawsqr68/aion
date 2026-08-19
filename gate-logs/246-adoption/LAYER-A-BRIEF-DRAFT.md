# DRAFT — #246 Layer A: adoption at recovery (follow-up round)

STATUS: DRAFT at Vesper's hand, 14:3xZ 2026-08-15. NOT ratified, NOT fired.
Awaits Waffles' word on the containment-slice landing and his review of this
brief. Fires through `aion start norn_dev` like every dev lane.

## Why a second round

The first "246-adoption" run (workflow 08d578c6, run 019a8816) delivered the
containment slice honestly and REFUSED to build Layer A partially, for a
reason verified against the code by that builder and credible on its face:

1. **The reply route is the hard part.** The worker's reconnect path
   recreates its control registry, while an in-flight agent completion
   retains the OLD connection's correlation ID and reply writer. Publishing
   a held-attempt list or re-minting a token alone leaves the completion
   riding the dead pre-outage correlation route — exactly the "adoption in
   name only" arm Floor 7 prohibits.
2. **The liminal registration type is external.** It cannot directly carry
   Aion-specific fields; the held-execution report must ride an additive
   channel that degrades to today's behavior at both ends (Floor 1).

## What is ALREADY LANDED by the containment slice (do not rebuild)

Branch `246-adoption` commits 831d5168d, 9d176adc7, efad9fb10, f3bbe645e:

- Floor 4: pending-responder collision is a typed refusal
  (`ServerError::PendingActivityCollision`, entry-API insert in
  `worker/bridge.rs`) — never a silent sender drop. Red A2 committed red
  first.
- Layer B whole: worktree spawn guard in `aion-integration-norn`
  (`spawn_guard.rs`) — atomic `create_new` marker with pid + process
  start/command incarnation, signal-0 + incarnation liveness, stale-marker
  clean-then-proceed, typed "spawn refused: live sibling pid N holds this
  tree", pre-spawn reservation atomically rebound to the child.

## The Layer A charter (unchanged floors, restated)

Original ratified floors remain binding verbatim (Waffles 01:58Z + 03:24Z +
04:19Z; see BRIEF.md in this directory):

- Floor 1 — additive protocol, absence degrades to today at BOTH ends;
  skew tests in both directions.
- Floor 2 — bounded adoption window closes the re-dispatch race; REUSE an
  existing re-registration grace if one exists (measure first); else a
  config value with NO default (all-or-nothing pattern), surfaced for the
  value discussion before arming.
- Floor 3 — adoption is an EVENT in run history; deadline re-established
  at adoption time.
- Floor 6 — same-worker held prior attempt adoptable under identical rules.
- Floor 7 — adoption re-establishes the REPLY ROUTE: the adopted
  execution's completion travels the post-recovery connection, proven
  delivered and accepted.

## Build shape demanded by the measured findings

The first builder's architecture conclusion, adopted as the working shape
(verify at the bytes before building, per house law):

a. **Held-execution registry that survives reconnects.** Worker-side: the
   set of live (workflow_id, activity_id, attempt) executions must live
   OUTSIDE the per-connection control registry, so a reconnect can report
   and rebind them.
b. **Server→worker adopt/rebind message.** Carries the fresh generation
   token AND the fresh correlation/reply route for each adopted execution.
   The completion then travels the new connection (Floor 7 provable in a
   fixture: kill the old connection, adopt, complete, assert acceptance).
c. **Recovery-side window.** On server start, in-flight delegate
   re-dispatch is held for the configured adoption window; survivors
   reported during the window are adopted (fresh token minted by the new
   incarnation, delivered to the HOLDING worker); the window expiring
   without a report falls through to today's re-dispatch.
d. **AdoptionEvent** in history + deadline re-establishment at adoption.

## Red case A (unchanged, now with the reply-route leg)

Recovery fixture: server dies mid-delegate, worker survives holding the
activity, server restarts. Pin TODAY'S behavior committed red (duplicate
dispatch scheduled; survivor's completion lost or refused). Build until the
same fixture shows: no re-dispatch, held-execution reported, adopt/rebind
delivered, completion accepted OVER THE NEW CONNECTION, AdoptionEvent in
history. Same-worker variant (Floor 6) under the same rules.

## House laws

Worktree-only (`.worktrees/246-adoption` continues, or a fresh branch off
the containment landing — Waffles rules at ratification), commit-as-you-go,
never push, red-first with quoted reds, no lint bypasses, ≤500 LOC per
file, clippy ×2 + fmt + battery to files, box load beside any red.
Waffles' re-gate before landing; his desk lands.
