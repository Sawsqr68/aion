# #247 — Transcript events survive a server outage (spool, don't drop)

STATUS: RATIFIED (Waffles 2026-08-15 07:49:48Z) — mechanism independently
verified at bytes by Waffles; fork CUT (see below); three amendments
folded verbatim. Floors are law. Wave 2 of Tom's 07:46Z go (dev-loop
unblock set: #246 → #247; #254 folded into #246; #245 pulled to design).
Queue: #246 → #247 after Waffles' current landings.

## Defect record

Task #247: worker transcript events are dropped, not spooled, across a
server outage — a recovered run's record has holes where the outage was.

Mechanism verified at bytes (worker side): the command transcript seam
emits through `context.emit_event`; when the seam is closed mid-command
(crates/aion-worker/src/command_transcript.rs:123-127) the remaining
lines are DROPPED and the loss is reported once
(`seam_failure_reported`, one-shot AtomicBool) — the command is not
failed, and nothing is retained for replay. The behavior is honest
(loss is reported, not silent) but lossy: every event emitted while the
server is away is unrecoverable. Specimen: the 2026-08-15 01:23Z server
death — surviving workers kept executing; everything they said during
the outage is missing from the record.

## Proposed fix shape (Layer: worker-side spool + ordered replay)

When the transcript path to the server is unavailable, the worker
appends events to a LOCAL DISK SPOOL instead of dropping them:

- Spool entries carry the event's full stream identity (the #168
  4-axis ActivityStreamKey axes) plus a worker-monotonic sequence, so
  replay preserves order within each stream.
- On reconnect/re-registration, the spool DRAINS FIRST, in order,
  before (or seq-fenced ahead of) new live events for the same stream —
  the server's observability keyspace never sees out-of-order history
  for a stream it accepted.
- Server ingestion of replayed events is IDEMPOTENT (dedup on stream
  identity + seq): a re-delivered spool entry cannot double-record.
- Bounded by DISK, not by an invented constant — no magic buffer size,
  no cap on event count (house law: no arbitrary limits). Disk
  exhaustion surfaces as a loud typed error, not silent drop.
- Residual loss remains possible in exactly one case — the WORKER
  itself dies with a spool it never drained — and that loss must be
  REPORTED in the stream as a typed gap event on next contact (by
  whoever holds the evidence), never inferred from absence.

## Design fork — CUT (Waffles 07:49Z)

v1 ACCEPTS worker-death loss; NO durable cross-incarnation spool — it
is a new persistence surface with ownership questions (a spool file is
a place, not an owner — the pid-reuse/squatter class all over again),
and #246 adoption makes the surviving-worker case the common one.

Gap-event pin (sharper than the proposal's "whoever holds the
evidence", his words verbatim): the AUTHOR is the SERVER (the survivor
that detects the worker death — it holds the dispatch records and the
last accepted seq), and the CONTENT claims only the WINDOW — "stream X:
last accepted event seq S at time T1, worker incarnation died ~T2,
events in between are unrecoverable" — never a count or a claim about
what the spool held. Claims carry their population; the server cannot
know the size of a loss it never received.

## Amendments (Waffles 07:49Z, law)

**Amendment 1 — dedup incarnation-safety, settled at bytes BEFORE
build.** Floor 4's dedup on (stream identity + seq) has a trap: if a
later worker incarnation can ever serve the SAME stream identity, its
worker-monotonic seq restarts and collides with the prior incarnation's
accepted entries — idempotent dedup would then silently EAT distinct
events, converting the cure into a new silent-loss channel. Suspicion:
the #168 4-axis key's attempt axis already forecloses this (new
incarnation ⇒ new attempt ⇒ new stream identity), but that composed
claim needs its own measurement: the builder verifies at the bytes and
pins the answer in the module doc — either the identity provably
changes across incarnations, or the dedup key gains the incarnation
axis. Either way a TEST pins the chosen truth.

**Amendment 2 — disk exhaustion is a TESTED loss class, not just a
typed one.** Floor 3 names it; the verification floor must EXERCISE
it: a fixture that exhausts (or mocks exhaustion of) the spool device
proves the typed error surfaces and nothing is silently dropped. The
one loss class we can manufacture on demand does not get to live as
untested code.

**Amendment 3 — Red A's control arm is load-bearing: keep exactly as
written** (pre-outage events present proves the assertion reads real
data). Red B's mutation check (swap drain-first for concurrent send
must break the green) is the right unfakeable shape — keep.

## Red first (both committed red before any fix)

- **Red A (the hole):** live command streaming transcript events; kill
  the server mid-stream; keep the command emitting; restart the server;
  worker reconnects and the command completes. Assert the recorded
  stream contains the events emitted DURING the outage. Pre-fix this is
  red (the hole exists). Control: pre-outage events present (proves the
  assertion sees real data, not vacuity).
- **Red B (ordering):** same fixture; assert replayed outage events
  land in-order relative to both pre-outage and post-reconnect events
  for the same stream (seq strictly monotonic per stream at the read
  API). Pre-fix red by absence; post-fix green must fail if replay
  races live events (mutation: swap drain-first for concurrent send).

## Floors (proposed for ratification)

1. Protocol skew degrades to today at BOTH ends: old worker / new
   server and new worker / old server each produce exactly today's
   behavior — spool machinery is additive, never load-bearing for
   registration or dispatch.
2. No invented constants: spool bounded by disk only; any operational
   bound is config with no default (all-or-nothing, per house law).
3. No silent loss anywhere in the new arm: every loss class that
   remains (worker death, disk exhaustion, malformed spool entry on
   drain) surfaces as a typed, recorded event — absence is never the
   signal.
4. Idempotent ingestion proven with a re-delivery test (deliver the
   same spool entry twice; the record holds one).
5. Fixture family shared with #246 where the outage/recovery rig
   overlaps — one rig, two briefs' assertions, no divergent copies.
6. Interaction pin with #246 adoption: an ADOPTED survivor drains its
   spool into the new server incarnation under its re-armed generation
   token; the adopted-case red is part of #246's A2 fixture extension,
   not a separate rig.

## Verification floor

- Both reds committed red-first with output quoted in the pin commits.
- cargo clippy --workspace --all-targets -- -D warnings ×2 + fmt.
- Focused: aion-worker suite + the outage/recovery e2e family + the
  #168 observability read-path tests (ordering assertions live there).
- Full battery per the standing lane protocol on a quiet box.
