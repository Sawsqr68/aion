# The orphaned run that reads Running forever — shaping note

Filed 2026-08-16 ~17:40Z from Waffles' 17:36Z defect row, verified and
CORRECTED at the artifact before shaping. Specimen: wid
`1f032477-7d7c-4999-b8c8-cf56640b4b8e` run `cb34f788-d1ed-49d6-8758-35b47886ca1b`
(git_status_sweep, display_name "pulse-6") — the run pid 25680 died
executing (death #3, ~16:27:40Z+).

## What the bytes say (aion describe, read 17:37Z)

Status: `Running`, `ended_at: null`, an hour+ and two server generations
after the death. History (11 events, seq order):

| seq | event | time | note |
|---|---|---|---|
| 1-2 | WorkflowStarted + attrs | 16:27:51Z | |
| 3-5 | activity 0 find_repositories sched/start/COMPLETE | 16:27:51-52Z | fine |
| 6-7 | activity 1 collect_status sched + start **attempt 1** | 16:27:53Z | died with pid 25680 |
| 8-9 | collect_status resched + start **attempt 2** | 16:29:57Z | **50s after recovery pid 13416 came up (16:29:07Z)** |
| 10-11 | collect_status resched + start **attempt 3** | 17:11:05Z | **exactly when pid 73945 booted (proof run Completed 17:12:03Z)** |

## Correction to the relayed framing

The relay said "never adopted." **The bytes refute that**: attempts 2 and
3 are re-dispatches whose schedule times match the two successive server
recoveries to the minute. #246 adoption-at-recovery IS reaching this
activity. Two distinct defects remain, and they are not the relayed one:

**Defect A — a dispatched attempt under a LIVE server goes nowhere.**
Attempt 3 was dispatched by the current healthy server at 17:11:05Z.
`collect_status` is a seconds-long shell sweep (attempt-0's sibling
completed in 1.6s). 25+ minutes later: no completion, no failure. This is
the known stranded-dispatch family (#254 retry-to-occupied-worker dies /
#259 parked dispatch never re-driven / #87 gRPC outbox leg waits forever)
wearing its production face — likely the adoption re-dispatch raced
worker/queue re-registration during boot and was dropped or parked. This
is a FIX to hunt, not a design question. NOTE: task_queue is `local`
(declared bodies run at the server), which narrows the suspect list — the
dispatch never left the building.

**Defect B — an attempt that vanishes with its server leaves no record.**
Attempts 1 and 2 each died with their server generation, and history
shows nothing between "started attempt N" and "scheduled attempt N+1".
No ActivityFailed, no reason, no attribution. Status-is-a-projection is
working exactly as designed — the projection is honest, history is not
complete. Consequence (Waffles' point, correct): the run table cannot
distinguish orphaned-by-death from still-thinking, and the console shows
dead work as live.

## Recommended shape (the design fork mostly dissolves)

Waffles posed re-dispatch vs fail-with-attribution as the fork. The bytes
show re-dispatch already happens. The missing piece is attribution AT THE
ATTEMPT LEVEL: when adoption re-dispatches an in-flight activity, it
should first record `ActivityFailed { attempt: N, reason:
server-death/adoption, … }` for the attempt it is superseding — history
becomes honest, the console can say "attempt 2 lost to server death,
attempt 3 running", and the run-level decision (keep retrying vs fail the
run) falls out of the activity's OWN retry policy instead of a new
mechanism. No invented defaults: an action with no declared retry ceiling
keeps its authored semantics.

Then Defect A is its own hunt: why did the 17:11:05Z re-dispatch on queue
`local` under a live server strand? That one is the highest-value thread
— it will fire again on every recovery.

## Ruling (Waffles, 17:38Z)

The supersession record is **APPROVED as design direction**: adoption
records `ActivityFailed { attempt: N, reason: server-death }` for the
attempt it supersedes before re-dispatching; terminality falls out of the
action's own retry policy. Grounds: a no-record attempt death IS a silent
failure (house law already forbids those); no new mechanism, no invented
default — application of settled principles, not a new fork. Rides to Tom
in the morning digest under #264 for ratification; build is NOT gated on
that (reshaping on his word is expected work). Sequencing ratified:
breadcrumb lands first, then the Defect-A hunt — and they compound: a
crash-shaped repro during the hunt gets its run named by the corpse for
free.

## Operator-facing requirement (added on the same ruling)

A `Running` row must carry enough for the operator to tell **resident by
design** (the assistant, a durable long-lived agent) from **incomplete
history** (an orphan whose next event will never arrive). Today the
list/console surface cannot distinguish the two — this is the
console-side half of the defect and belongs on #264's table alongside
the history-side fix. Concretely: the row (or its describe surface)
should expose the age of the newest history event and the state of the
newest attempt, so "attempt 3 started 25 minutes ago on a seconds-class
action" reads as the anomaly it is.

## Defect A — ROOT CAUSE FOUND (17:5xZ, every link verified at an artifact)

The stranded adopted dispatch is a **boot-ordering defect with a silent
fallback arm**, closed end-to-end:

1. **Recovery replay dispatches before the declared-body source exists.**
   `state.rs:276` `build_engine(...)` → `builder.build().await` carries
   `.production_recovery_seam()` and performs recovery replay INSIDE the
   build — re-dispatching every in-flight activity through the
   already-wired dispatcher chain. `state.rs:287`
   `install_engine_backed_seams(...)` fills the `DeclaredBodySource`
   OnceLock **eleven lines later** — after every recovery re-dispatch has
   already left.
2. **The empty-source arm is the one silent arm.**
   `declared_body.rs:126` — `self.inner.get().map_or(
   DeclaredBodyLookup::None, …)`: an uninstalled source answers `None`
   with no log (Unreadable logs ERROR, no-registry-handle logs WARN —
   absence of both in every generation's log is what fingerprints this
   arm). `None` silently delegates to the inner worker dispatcher.
3. **The worker path parks forever.** Queue `local` never has pollers
   (declared bodies ARE its service). `queue_service::wait` parks with
   `NO_LIVE_POLLERS`, `service_availability_deadline_ms: "never"` —
   WARN logged once, then nothing, ever. Nothing re-drives a parked
   dispatch when the body source installs moments later (#259's face).
4. **The false premise is written in the source.** The doc comment on
   `install_declared_bodies` (state.rs:1491-1497): "nothing is deployed
   before the engine exists, so no declared body can be missed" — true
   for fresh deploys, false for recovery: things were deployed before
   the engine DIED.

Evidence per generation (all four server boots):
- gen 1 (pid 25680, log 20260816→17): fresh dispatches log "executing
  declared action body at the server" ×2 — control arm.
- gens 2/3/4 (13416/73945/96436, logs b/c/d): adopted re-dispatch parks
  at boot +~1s with the identical WARN; no declared-body INFO, no
  registry WARN, no breadcrumb (gen 4, note file — dispatch never
  reached the executor); gens 2/3 each surface a bridge ERROR only at
  their own TERM.
- Timing (gen 4): park 17:43:59.561, banner 17:44:00.020, liminal
  listener 17:44:00.290 — the park precedes the server being open.

**Fix shape (primary): recovery must not dispatch before the
dispatcher's collaborators are installed.** Construct the engine, install
the engine-backed seams, THEN run recovery replay — needs the engine
builder to defer recovery out of `build()` (engine-crate seam) or an
equivalent hold-release on the dispatcher. Hardening riders, each its own
verdict: (i) the empty-source `None` arm must be loud (silent-failure
law); (ii) a park with no deadline on an operator-visible dispatch
deserves an event, not only a WARN line (feeds #264's honest surface);
(iii) #259 re-drive remains its own defect — the ordering fix removes
this trigger of it, not the class.

Interim: pulse-6 (wid 1f032477) is left AS IS deliberately — it is the
wild specimen for the fix's e2e and for #264/#265; settle it only after
the fix can watch it settle.

## Relation to the board

- #264 (status-truth front, TOM'S WORD) — this specimen is its wild
  exhibit: list status derived from history is only as honest as history.
- #237 (TimedOut/Failed leak activities) — same family, other direction.
- #254 / #259 / #87 — Defect A's suspect list.
- #265 (stale visibility row) — wanted a wild specimen; check whether
  this run's visibility row matches its history before reusing it.
- The breadcrumb (this lane) makes the NEXT specimen self-attributing:
  the corpse names the in-flight run.

Control worth keeping: the OTHER Running row (assistant 1f81ae95, since
08-11) is legitimately resident — the list surface cannot tell these two
apart today, which is the operator-facing half of the defect.
