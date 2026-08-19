# manifold #210 — the attach path drives the ObserverBackpressure recovery instead of bailing

Fault-tolerance lane, Waffles' commission 14:51Z 2026-08-15 (his YES on the
board line his 09:50Z K9 battery verdict opened). Repo: the manifold
worktree named in your dispatch. Landing is Waffles' (merged gates +
scratch-estate walk at his desk); your branch never merges itself.

## The defect, verified at the bytes 14:5xZ

`crates/manifold-node/src/mailbox/member.rs:19` documents it as **"a named
unimplemented arm"**: `ObserverBackpressure` on attach has a documented
recovery — send `ObserverRecoveryHandshake` naming the refused epoch, wait
for `ObserverProgressed`, retry the attach — **which the slice does not
drive**. Instead the arm bails with a typed error at THREE sites:

- `member.rs:219` — enrollment refused with backpressure → `bail!`
- `member.rs:494` — same shape, second exchange site → `bail!`
- `mailbox/driver/fate.rs:90` — record submit refused → `Fate::Refused`

Under load the broker answers backpressure, the node treats it as terminal,
and whatever depended on that attach/enrollment dies. The comment's own
justification ("the observer facility is not in manifold's path and a wrong
guess here deadlocks") is a disclosure, not a fix — and the house law is
that a limitation a fix could close is never merely documented.

## Verification duty ONE — the contract, before any build

Read the liminal protocol contract for `ObserverBackpressure`,
`ObserverRecoveryHandshake`, and `ObserverProgressed` (the
`liminal_protocol` dependency — read the actual dependency source your
lockfile resolves, not memory). Establish and RECORD in the report:

1. who emits `ObserverProgressed`, on which stream, and in reply to what;
2. whether manifold's seat can legally drive the handshake from where these
   three sites sit (the doc comment fears a deadlock — name the exact
   wait-cycle it fears and show why your shape does or does not close it);
3. what bounds the wait — if the protocol carries its own bound, use it;
   if not, the wait budget is a deployment config value with NO default
   (all-or-nothing pattern), surfaced in the report for the value
   discussion; never an invented constant.

**If the contract genuinely cannot be driven from manifold's position, STOP
after duty two: commit the red fixtures, write the blocker report citing
the contract lines, and end. Do not guess. Do not paper.**

## Verification duty TWO — attribution of the K9 reds (honest, separate)

The K9 gate run (02:0xZ, heavily contended box) failed
`cargo test -p manifold-node --test mailbox` 26/142 (attach_client ×7,
face ×8, registry ×6, chair ×1, manifest ×2, session_door ×2; face:
"session ONE is still alive 30s after its kernel was killed"). That log
contains NO backpressure text — those reds are NOT yet attributed to this
arm. Your duties:

1. run the mailbox suite SOLO in your worktree on the base commit, record
   pass/fail counts and box load beside it;
2. state plainly which of the 26 this fix explains (with evidence — e.g.
   the fixture reproduces their face) and which it does not. No claimed
   causality without evidence; "not attributed" is an acceptable answer.

## Red case (red-first, committed failing)

A fixture at the exchange seam that makes the broker answer
`ObserverBackpressure` for an attach/enrollment (fault injection or test
double at the same seam the three sites read). Pin TODAY'S behavior
committed red in the shape "the recovery was not driven — the arm bailed".
Then build until the same fixture shows: handshake sent naming the refused
epoch, `ObserverProgressed` awaited within the bound, attach retried and
accepted. A second fixture arm: backpressure REPEATING past the bound ends
in a typed refusal that still names the epoch, the attempts made, and the
recovery sequence — never a silent loop, never an unbounded wait.

## Floors

1. All three sites move together — member.rs ×2 and fate.rs get the same
   recovery discipline; no site keeps the bail while another recovers.
2. Waits are event-driven off the protocol's own messages, never wall-clock
   polling; tests deterministic (no sleep races — this box carries three
   batteries tonight).
3. Every refusal that survives keeps its current honesty: epoch named,
   recovery sequence named, attempts counted.
4. The module doc comment is updated to describe what IS driven — the
   "named unimplemented arm" paragraph dies with the limitation.

## House laws

Worktree-only, commit-as-you-go, never push, never merge, red-first with
the red output quoted in the commit message, no lint bypasses, no
#[allow]/#[ignore]/test skips, ≤500 LOC per file, gates run to files, box
load recorded beside any red. The live estate (servers, workers, ~/.aion,
the running manifold estate) is untouchable.
