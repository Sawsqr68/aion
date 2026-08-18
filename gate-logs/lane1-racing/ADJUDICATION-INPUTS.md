# lane-1 racing test — landing adjudication inputs (written 2026-08-18, before the verdict is needed)

The question the landing must answer: does the repro loop's outcome, under
its DECLARED deviation, suffice to rule on
`query_racing_completion_yields_payload_or_typed_error_without_events`
(crates/aion/tests/engine_query.rs:693) — the single gate on the lane-1
landing and, downstream, the aion-core close and the family publish?

## The evidence that will exist at adjudication time

1. **Two full-suite estate-Mac data points** (Waffles' battery, 2026-08-18):
   one RED at 09:46:01Z (5089/5090, fast-fail 1.163s at position 3005, on a
   quiet box, assertion text NOT captured — instrument defect, banked) and
   one GREEN at 09:55:25Z (5090/5090 warm rerun). Isolated: 10/10 green at
   loads 15–341. Waffles' load-flake ruling RETRACTED — the discriminator is
   in-suite conditions, not external load.
2. **The aion-workflows loop (v2)**: up to 10 runs of
   `cargo test --workspace --exclude aion-awl`, per-run timeout 2400, on a
   box with co-tenants (norn build / fleet flight — eras annotated in
   ~/lane1-repro/). Deviation: aion-awl excluded because its runtime_codecs
   binary hangs deterministically on this box class (aion#64) — the
   exclusion removes aion-awl's contention contribution from "in-suite
   conditions", and that is the caveat's whole content.

## The judgment's inputs, priced now

- **If the loop catches a red with assertion text**: the caveat is moot —
  the captured text names the fourth outcome and the fix lands red-first in
  the lane. This is the prize regardless of suite shape.
- **If ten reduced-suite runs come back green**: reduced-suite green does
  NOT overturn the Mac full-suite red — the red remains an unexplained
  observation whose assertion text was never captured. Ten caveated greens
  + one uncaptured red + isolated 10/10 green = the race exists and is
  narrow; the honest disposal is then EITHER another bounded full-suite
  loop on a box where the full suite can run (the estate Mac, queued
  through the token file), OR a lane-side hardening of the test's fourth
  outcome into a named, typed admission (making the assertion
  self-documenting on next natural occurrence) with the lane landing
  behind it. What ten caveated greens must NOT become is "the red was
  noise" — a green under a config where the red's conditions cannot fully
  recur is absence-of-evidence, not evidence of absence (the soak law:
  a soak must run in the world the death happened in).
- **The assertion-text capture defect is fixed in the loop** (complete
  per-run logs); any future red is self-documenting.

## Who rules

The lane holder rules on the racing test; Vesper gates the landing; the
adjudication uses exactly the inputs above plus whatever the loop banks.
