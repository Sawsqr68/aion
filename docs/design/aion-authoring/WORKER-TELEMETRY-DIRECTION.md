# Worker telemetry — direction (Tom's ruling, 2026-08-04)

Tom's words (04:00Z Aug 4, DM): bring telemetry forward and get it working —
agents streaming responses live, checks and test commands showing their
output as it comes — **but** "we don't need to record absolutely
everything... it could be being recorded with the worker database."

## The ruling, unpacked

Two planes, deliberately separate:

1. **The durable plane (Aion's event history)** stays what it is: the
   record replay depends on. Dispatches, attempts, results, transcripts of
   agent rounds — the facts a resumed workflow needs. Nothing changes here.
2. **A live telemetry plane** — new: a worker streams what it is doing
   *right now* (agent tokens as they arrive, a test run's stdout tail,
   progress counts) to whoever is watching, WITHOUT each frame becoming a
   durable event. If nobody is watching, the stream costs nothing durable.
   If the worker wants a local record, that is the **worker's own storage**
   (`liminal`'s worker-side DB is the natural home), not Aion's store.

This is exactly the shape the console already wants: #59 (stream heartbeat
details + leg output tails into the console) is the consumer half of this;
the observability-layer direction doc is the harness-agnostic frame.

## Why the split matters (the determinism boundary)

Replay must never depend on telemetry. A workflow that could read its own
live telemetry would be nondeterministic on replay. Keeping the telemetry
plane worker→observer only — never worker→workflow — preserves invariant
2 for free. Telemetry frames are advisory exhaust, not events.

## Likely build shape (to be designed properly, not committed yet)

- The liminal worker transport already carries heartbeats; the cheapest
  first motion is widening heartbeat detail into an opaque
  progress/telemetry frame the server relays to WebSocket subscribers
  without recording.
- Console: run detail view subscribes when open; frames render live and
  are dropped when the view closes (#59's leg-output tails).
- Worker SDKs and `aion worker agent` get a "report progress" call;
  the norn/ACP harnesses forward the agent's streaming output through it.
- Recording policy is the worker's: local capture in the worker's own
  store if the operator wants it; Aion records only what history already
  records.

## Open questions for the design pass

- Frame schema: opaque bytes + content-type (matching the Payload
  philosophy) vs a minimal typed envelope (progress %, phase, text chunk).
- Backpressure: drop-oldest on a bounded ring per attempt is the obvious
  answer; never block the activity on a slow observer.
- Does the last frame get pinned into the heartbeat record (so an operator
  arriving late sees *something*) without opening the door to "telemetry
  as history"?

Cross-refs: #59 (console consumer), #27 (worker lifecycle — same
transport work), docs/design/aion-observability (direction),
BUNDLE-139-NOTES.md (assistant bundling — the assistant is the first
streaming producer people will watch).
