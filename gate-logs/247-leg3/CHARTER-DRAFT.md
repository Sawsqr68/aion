# #247 LEG 3 — charter draft (paper-authored 2026-08-16 ~02:2xZ, Vesper)

Status: DRAFT. Build fires only after the read-path leg-1 battery clears
the box (Waffles' dispatch order: one battery at a time). The rebase phase
below needs Waffles' explicit nod at fire time (same doctrine as leg 2's:
never-pushed commits, base-advance is the ruled cure — but leg 3's chain
is 7 commits, not 3, and the authorization must be re-spoken, not assumed).

## Base state going in

- Branch 247-transcript-spool at 568c1f6193d8e6f88dc39aa2a7273f911bae66c5
  (7 commits atop e8c1b28bf; worktree .worktrees/247-transcript-spool,
  target dir reclaimed 01:3xZ — cold rebuild expected).
- aion main is now 0bed60196 (#246 landed): the outage/recovery rig the
  leg-2 builder could not find on e8c1b28bf EXISTS on main now — this is
  the ratified reason leg 3 waited (#246-first, Waffles 00:59Z).
- Gaps 1+2 from the leg-1 report remain the only open requirements; gaps
  3+4 landed in leg 2 (verified at my read, red-first genuine).

## PHASE 1 — REBASE (needs Waffles' nod at fire)

Rebase the 7-commit chain from base e8c1b28bf onto current aion main
(0bed60196 or later): `git rebase --onto <main> e8c1b28bf
247-transcript-spool`. Justification: the chain is unpushed and unreviewed
at these shas; leg 3's entire purpose (extend the #246 rig) requires the
rig in the base. Post-rebase: run the touched-crate suites
(aion-worker --features liminal-transport; aion-server --lib
activity_publisher; aion-server --lib api::http::transcripts) and record
results before phase 2.

## PHASE 2 — GAP 1: server-authored worker-death gap event

The leg-2 builder's investigation (banked in his report, verified
uncommitted): the in-flight liveness record does not retain run_id, so
the death path cannot reconstruct the four-axis ActivityStreamKey.
Required shape:

1. Thread the run axis through dispatch tracking (InFlightActivity /
   LostWorkerReport seams named in the leg-2 report) so the death
   detector can name the exact stream.
2. Wire HeartbeatTracker's lost-worker paths (fail_lost_worker /
   park_lost_worker) to an asynchronous durable transcript-gap publisher
   recording: four-axis stream identity, last accepted event seq, last
   accepted timestamp, death window, and the bounded claim ("events
   between are unrecoverable") — the brief's verbatim event text shape.
3. Transcript support must NOT become load-bearing for dispatch: a
   publisher failure never blocks or fails the dispatch path itself
   (but is never silently dropped either — same no-silent-loss floor
   gaps 3/4 enforced worker-side).
4. Red-first: a test that kills a worker incarnation with accepted
   events outstanding and asserts the typed gap event exists with the
   right identity BEFORE the implementation.

## PHASE 2 — GAP 2: the chartered live outage e2e

Extend the #246 outage/recovery rig (now in crates/aion-server tests on
main) — do NOT build a cold fixture. The chartered sequence, all eight
beats, as ONE e2e:

1. Real streaming command emits a pre-outage transcript event.
2. Real server killed mid-stream.
3. Command continues emitting during the outage (spool active).
4. Real server restarts.
5. Surviving worker re-registers (observed).
6. Survivor adopted/re-armed under the new generation token (the #246
   machinery exercised, not mocked).
7. Outage events drain BEFORE post-reconnect live events.
8. Observability read API returns a strictly monotonic stream.

The FakeSink unit fixtures stay; they are not the proof. Red-first where
pinnable (the e2e lands red against a deliberately-broken drain ordering
or is proven non-vacuous by a mutation control per house law).

## Ground rules (carry-over, unchanged)

Work only in the worktree; commit as you go; never push/merge; live
estate untouched; no lint bypasses (clippy workspace + per-crate, both
resolutions); real newlines in commit bodies; no invented defaults/caps;
full delivery report with red-then-green evidence per gap.

## Gate

247's 8-leg shape re-used (js×2, fmt, clippy×2, aion-worker, aion-server,
full battery), HEAD-stamped run_leg, logs → gate-logs/247-leg3/.
Expected known faces if they recur: #251 (costly-announce), #85 (libsql
all-features intermittent) — state honestly, attribute with evidence.

## On the lane landing (not before)

- Mint the libsql-retirement ticket (standing rule).
- Board #247 closes with the full three-leg record.
