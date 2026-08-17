# Resume adopts the surviving builder — lane brief (r1 — APPROVED)

- **Status**: **r1 APPROVED by Waffles 04:40Z 2026-08-17**, two rulings folded
  (cross-process acceptance leg added; exhaustion-record defect bound to this
  lane). Dispatch-ready the moment his queue clears — DISPATCH WAITS ON HIS
  WORD. Box on ~25-min guard-cycle cadence under ambient leak load; r5's
  builders still finishing. Code anchors byte-verified at his read.
- **Author**: Vesper. **Incident record**: aion issue #36 (Waffles, receipts
  from `aion describe c814d5a5` + docs tracking day file @ d09e723).
- **Blast radius tonight**: r5 (front deliverable) Failed at the harness layer
  while its real work continued in the trees; two paused runs (b75333eb,
  124d9706) carry the identical hazard and are held un-resumable until this
  lands or their agents exit.

## The measured mechanism (event-store receipts, issue #36)

1. 04:26:30Z fleet_dev 7dbd2ce5 paused; 04:26:52 old server TERM'd by the
   memory guard; 04:27:36 fresh server 36150 up.
2. **Seq 6, 04:28:54 — the disownment**: the fresh server failed the run's
   original attempt 1 as `superseded:server-death` — disowning a LIVE builder
   — because the workflow was **Paused at boot, so #246's recovery adoption
   never examined it**. Paused workflows fall through recovery and re-dispatch
   on resume.
3. 04:30:11 resume → fresh dispatch of activity:0 → spawn gate finds the
   surviving builder holding the tree. **#33 worked**: attempts 2–3 failed
   kind:Retryable and retried on backoff (+31s/+60s/+120s).
4. Attempt 4 flipped **kind:Terminal with the message still reading
   "retryable:"** — retry-budget exhaustion against a sibling that outlives
   any plausible budget (90264 still mid-gates 10+ min past the first
   refusal). Run Failed 04:32:54. Both children died identically.
5. **The refusal is SELF-occupancy**: the holder record names (c814d5a5,
   activity:0) — the refused spawn's own workflow and activity. The gate
   (`crates/aion-integration-norn/src/spawn_guard.rs`; marker
   `.aion-held-execution.json` carries workflow_id + activity_id + pid +
   process incarnation) already holds everything needed to tell "stranger's
   tree" from "my own attempt-1 builder".

Live-binary fact: server 36150 = 0.16.0 @ 00064c41c (today's tip) — #246 and
#33 are both in it. The gap is path-shaped, not version-shaped.

**Control observation from the other side (05:0xZ, free)**: with the agents
EXITED, resuming b75333eb and 124d9706 across the same server-death pattern
worked perfectly — dead incarnations cleaned, markers rebound to fresh agents
(29537/30756), sessions continued via resume-if-exists, no collision. The
foreign-vs-own distinction is thereby proven from both sides: live own
builder → refusal kills the run (r5); dead own builder → clean re-claim
works. Only the live-own quadrant is broken, and it is exactly the quadrant
adoption fills.

**Third incident, scope-widening (05:1xZ)**: both resumed runs then died the
IDENTICAL self-occupancy death at the NEXT guard cycle — while **Running at
boot**, not Paused. b75333eb Failed refusing against its own fresh agent
29537 (still alive and working after the run died — the #36 orphan pattern
again); 124d9706 Failed against 55362, a second-generation agent from
re-dispatch churn across cycles. Consequence for Remedy B: "extend #246's
adoption to Paused workflows" is TOO NARROW — recovery adoption evidently
does not reach remote-worker agent activities at all; a Running workflow's
in-flight agent activity is re-dispatched on every server generation. B must
cover the remote-agent-activity class for BOTH Paused and Running paths.
Operational consequence: under the current ~10-min cycle cadence, ANY run
with a live agent activity dies within one generation — no re-runs until
this lane lands.

## The fork, argued: BOTH remedies, gate as backstop

**Remedy B — recovery examines Paused workflows (engine, the truth fix).**
The root falsehood is seq 6: history records `superseded:server-death` for an
attempt whose builder is alive and working. #246's adoption sweep must examine
Paused workflows too — a Paused run's in-flight attempts get the same
adopt-don't-disown treatment as Running ones, and the supersession verdict is
deferred until the holder's liveness is actually checked. B alone closes
tonight's exact hole (resume then finds attempt 1 still owned; nothing
re-dispatches). What B cannot do: catch the OTHER re-dispatch paths into an
own live builder — reconnect redelivery (board #258) and retry-into-held
(#254) reach the same gate without a pause anywhere in the story.

**Remedy A — spawn gate adopts on self-occupancy (worker, the class fix).**
At `SpawnClaim::acquire`, a live holder whose (workflow_id, activity_id)
equals the incoming spec's is not a collision — it is this very execution,
still running. Foreign holder: refusal stays exactly as is (#246's red must
stay red). Own holder: ADOPT —
  - in-process (the worker never died; tonight's case): single-flight join on
    (workflow_id, activity_id) — the re-dispatch awaits the original
    handler's outcome and returns it as its own result;
  - cross-process (worker restarted, agent survived reparented): the norn
    harness already speaks re-attach — session ids templated on
    {workflow_id} with resume-if-exists; resume the session, never spawn.
A alone turns every stray re-dispatch into a join — but leaves seq-6-style
disownment records lying in history whenever a Paused run crosses a restart.

**Why both**: B keeps history truthful at the source (no false supersession,
no disowned live work); A is the structural backstop that makes the WHOLE
class safe — any path, engine bug or not, that re-dispatches into the run's
own live builder converges on the same single execution. B without A leaves
#254/#258 lethal; A without B leaves the record lying. Waffles' parenthetical
("or both with the gate as backstop") is the right answer.

## Third defect: exhaustion record (RIDES THIS LANE — Waffles' ruling 04:40Z)

Budget exhaustion rewrites kind Retryable→Terminal while keeping the
classifier's `retryable:` message — the terminal record misnames its own
cause. Exhaustion must say exhaustion (attempts spent, budget, last refusal),
not wear the retryable classifier's text. Ruled into this lane, not #34's
trio: its acceptance leg is already below, the fix site is adjacent to this
lane's code, and splitting a one-classification change spreads context.

## Fourth defect: false-error resume (RIDER — Waffles' ruling 04:59Z)

`aion resume` on a recovered run answers exit 1 with "recovered workflow
process N did not reach its pending await within 400ms [Runtime]" while the
resume SUCCEEDS (list shows Running, agent spawns). Observed two-for-two
deterministic on b75333eb and 124d9706 at 04:58Z — pin it while it
reproduces. Ruled a rider here (same door, same dispatch, same reviewer
context; sibling of the exhaustion-record leg: an exit code must name what
happened). Accepted fix shape: the CLI either waits out the recovery fence or
reports success-with-recovery-pending on exit 0 — exit 1 is reserved for a
resume that did NOT take.

## Acceptance sketch

- **Issue #36's repro as the red-first e2e, verbatim**: builder live → pause
  → server restart → resume → today Failed within one dispatch cycle; after
  the fix, the run completes with ONE agent session (session id proves it).
- Foreign-sibling control: different workflow's live holder still refused —
  #246's refusal red stays red (coinciding-answers law: the fixture must
  distinguish adopt from refuse-removed).
- Redelivery leg: re-dispatch of a held (workflow_id, activity_id) into the
  same live worker joins — two dispatches, one agent outcome, no second
  norn session (covers #258's worst face).
- **Cross-process re-attach leg (Waffles' ruling 04:40Z — the arm must be
  tested, not just built)**: worker killed under a live agent; replacement
  worker receives the re-dispatch, finds the marker naming the surviving
  agent's own (workflow_id, activity_id), and resumes the norn session
  instead of spawning — session id continuity proves adoption. The harness
  already speaks resume-if-exists, so the fixture should be cheap; if the
  build lane finds it genuinely prohibitive, the fallback is an EXPLICIT
  non-claim in the delivery report plus a board row — silence is not an
  option (an untested arm of a class fix is a husk).
- Paused-at-boot leg: Paused run with in-flight attempt crosses a restart —
  history shows NO supersession record while the holder lives; adoption
  event recorded instead.
- Exhaustion-record leg: forced budget exhaustion names exhaustion, message
  no longer reads `retryable:`.
- False-error-resume leg (rider): scripted resume under load asserts exit 0
  when list shows Running; the slow-recovery path emits a distinguishable
  "resumed, recovery pending" line, never an error.
- Single-writer law respected throughout: adoption events append through the
  run's one Recorder; no direct EventStore::append.

## Sequencing (Waffles' desk)

Builders exit → r5 tree disposition → release cut (clean tree, never the
serving checkout) → this lane's dispatch on his word. My two paused runs
resume only after this fix lands or their agents exit on their own —
whichever first.
