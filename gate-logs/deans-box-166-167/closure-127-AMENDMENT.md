# Amendment to the #127 closure evidence (2026-08-09)

The closure-run record committed at `7c9a68cb4` claims "~93% power at the
fitted 1/376 rate". Apollo Biscuit asked the positive-control question the
same hour: was a misclassification planted to prove the suite still catches
one? The answer changes what the number means, so this amendment retires it.

## The 93% figure is retired

No positive control was planted — and planting one now would measure
nothing for this defect class. The original misclassification required BOTH
halves of the old world at once:

- the adapter's `stream_head` deriving the head from a range read on the
  append path (removed by #167), and
- haematite reading the event range before the sequence counter
  (fixed upstream by haematite #74, in the pinned 0.8.1).

A reverted adapter running against 0.8.1 stays green through the upstream
fix alone, so the conformance suite at the landed tree cannot catch an
adapter-side revert of this class at ANY n. The 1/376 base rate was fitted
in the old-adapter x old-haematite world; the 1000 runs were executed in
the new-new world. Power computed from one world and spent in another is
not power. The figure should not be quoted.

## What the 1000/1000 does establish

A concurrency soak of the landed pairing: 1000 suite iterations (each
verified to run exactly one real test — a control run at the same tree
prints `running 1 test ... 1 passed`, 1.96s, matching the loop's observed
~2.1s/iteration; the `--exact`-filter vacuity trap was checked, not
assumed) with zero failures of any kind. Useful as a soak; not a
probabilistic guarantee against revert.

## What actually guards each leg (deterministic, not probabilistic)

- **Adapter revert** (`stream_head` back to a scan derivation):
  `stream_head_does_not_depend_on_payload_decodability` goes red
  deterministically — the poisoned-payload fixture makes the scan
  derivation error while the metadata head answers 1, regardless of
  upstream version. This is the positive control for the adapter leg, and
  it is in the tree at `4ed851138`.
- **Upstream order revert**: haematite 0.8.1 carries two in-crate
  instruments measured independent during its own gate (the read_seam race
  test and the read-order recorder — a second injection left one green
  while the other failed alone, per the haematite seat's record).

## Standing of #127

Closure stands, resting on the structural severance plus the deterministic
pins above — not on the retired probabilistic figure. The ratified record
is corrected by this amendment naming the claim and the commit
(`7c9a68cb4`), not by quiet deletion.
