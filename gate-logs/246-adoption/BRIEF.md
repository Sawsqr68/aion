# #246 — Recovery adopts a surviving attempt instead of minting its rival

Wave 2 of Tom's general go. Fix-shape proposed by Vesper 01:5xZ, RATIFIED
by Waffles 01:58Z with the floors below pinned verbatim. One brief, TWO
red cases (the layers verify differently). Defect record: task #246 —
live specimen 2026-08-15 01:25Z (#207 attempt 1 pid 59219 survived the
server death with work committed; recovery re-dispatched to the other
worker on the queue, minting pid 57457 into the same worktree; the
fence — envelope.rs:53, process-incarnation-scoped — guarantees the
survivor's completion is refused, so its whole spend is structurally
discarded on every such recovery).

## Layer A — adoption at recovery (the real fix; wire change ratified)

Workers already re-register after a server death (proven 4/4 at the
socket tonight). Extend re-registration with a **held-execution report**:
(workflow_id, activity_id, attempt) for each execution the worker still
holds live. Recovery ADOPTS a reported survivor: the new server
incarnation mints the fresh generation token and delivers it to the
HOLDING worker — re-arm, not re-run. The survivor becomes the current
generation; no duplicate is minted; the fence's incarnation discipline
is preserved because the new incarnation issues the token.

**Floors (Waffles, pinned):**

1. **Protocol law — absence degrades to today at BOTH ends.** Old
   worker against new server, and new worker against old server, must
   each produce exactly today's behavior. The held-execution report is
   additive and never load-bearing for registration itself — version
   skew can never wedge a worker out. Pin both skew directions with
   tests.
2. **The race is closed, not narrowed.** Recovery holds re-dispatch of
   in-flight delegate activities for a bounded adoption window, else
   attempt N+1 mints before the survivor reports and tonight is
   rebuilt. REUSE the existing re-registration grace if one exists —
   measure the code, don't assume; if none exists, the window is a
   config value with NO default (all-or-nothing pattern, as
   [worker_supervision] does it), surfaced to the dispatcher for the
   value discussion before arming — never an invented constant.
3. **Adoption is visible.** An adopted attempt gets its deadline
   re-established at adoption time, and the adoption itself is an
   EVENT in run history — restarts are events, adoptions are too. The
   operator can read "attempt 1 adopted by recovery at T" where they
   already look.

**Red case A:** a recovery fixture — server dies mid-delegate, worker
survives holding the activity, server restarts; pin TODAY'S behavior
first (duplicate dispatch scheduled, survivor's completion refused
StaleGeneration), committed failing in the shape "adoption did not
happen"; then build until the same fixture shows: no re-dispatch,
held-execution reported, token re-delivered, survivor completes
accepted, AdoptionEvent in history.

### Amendment — same-worker collision (Waffles 03:24Z, from #254)

The #254 specimen (2026-08-15 02:56:02Z, run 7649531a): attempt-2's
per-attempt bound fired while its execution was still held by
fleet-dev-worker 59366; the engine minted attempt 3 to the SAME
worker; the engine recorded ActivityFailed kind=Terminal "activity
response channel dropped" 6ms after the worker's dispatch-push
receipt, consuming the final retry and failing the run — while the
worker spawned the attempt-3 agent anyway. Hypothesized mechanism
(verify at the bytes before build): the pending-activity key is
attempt-free (bridge.rs:103 (WorkflowId, ActivityId)), so attempt-3's
responder was dropped on insert collision with held attempt-2 —
retry-after-timeout is structurally DOA whenever the same worker
still holds the prior attempt. Amendment floors, pinned:

4. **No silent responder drop.** The pending-activity key carries the
   attempt, OR the insert refuses/replaces with EXPLICIT semantics —
   in no arm does a responder channel drop silently. Whichever shape
   the bytes favor, the collision becomes a named, typed outcome.
5. **Same-worker red case (Red case A2):** attempt N held live on
   worker W, attempt N+1 dispatched to W; today's "activity response
   channel dropped" Terminal is the committed red. Built until the
   same fixture shows the collision handled under floor 4's explicit
   semantics.
6. **Adoption covers the same-worker case.** A held SAME-worker prior
   attempt is adoptable by the identical rules as the cross-worker
   case — the holder's identity being equal does not change the fence
   discipline (new incarnation mints the token, delivers to the
   holding worker, deadline re-established, adoption is an event).
   The bounded adoption window covers both cases.

### Amendment — reply route (Waffles ratified 04:19Z, from the 59219 exit)

Specimen 2026-08-15 04:13:49Z (dev-worker-0150.log:154): attempt-1's
agent finished its whole charter (4 commits) and the worker's
completion reply died at the TRANSPORT — "failed to reply
DispatchResponse — Broken pipe (os error 32)" — because the reply rode
the pre-outage liminal connection. The server never saw the
completion; NOTHING reached the fence. The fence-refusal arm of the
original #246 analysis therefore remains honestly UNTESTED on the
record: the predicted StaleGeneration refusal was never observed live,
because on this path the spend is discarded one layer earlier.

7. **Adoption re-establishes the reply route, not just the token.**
   A re-registered worker holds a NEW connection; the adopted
   execution's completion must travel that connection, never the
   pre-outage corpse. Layer A's red case A extends: after adoption,
   the survivor's completion is proven DELIVERED and ACCEPTED over
   the post-recovery connection (a token re-delivered to a worker
   whose reply path still points at the dead connection is adoption
   in name only).

## Layer B — spawn-time sibling refusal (containment, every duplicate class)

Before spawning an agent into a declared cwd, the worker writes a
liveness marker in the worktree (workflow_id + activity_id + pid) and
refuses to spawn beside a live sibling.

**Floors (Waffles, pinned):**

1. The marker is verified against a live pid that MATCHES the marker's
   claim — a stale marker from a dead pid is CLEANED and spawn
   proceeds; a worktree is a place, not an owner. (Check liveness by
   signal-0 on the recorded pid AND that the pid is one of ours —
   pid-reuse must not honor a squatter.)
2. The refusal is a TYPED error that reaches the server named (the
   operator sees "spawn refused: live sibling pid N holds this tree"),
   never a silent skip, never an untyped failure.

**Red case B:** a spawn-collision fixture — two dispatches of the same
(workflow, activity) targeting one declared cwd; pin today's behavior
(both spawn) committed failing; build until the second refuses typed
while a stale-marker variant proves cleanup-then-proceed.

## Sequencing and walls

- Layer B may build immediately in Wave 2.
- Layer A builds alongside; its LANDING rides the binary-swap window —
  nothing swaps while fleets fly (standing wall; landed is not
  running).
- Both ends of the wire ship from this repo in one motion (#222
  precedent: one coordinated landing).

## House laws

Worktree-only, commit-as-you-go, never push, red-first with quoted reds,
no lint bypasses, ≤500 LOC per file, clippy ×2 + fmt + battery to files,
box load beside any red. Waffles' re-gate before landing; his desk lands.
