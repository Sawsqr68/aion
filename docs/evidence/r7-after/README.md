# R7-b — the byte-identity proof. **The recorded diff is EMPTY.**

Design §7, verbatim: *"If implementation cannot meet byte-identical
emission for a pre-existing parallel-only document, it has changed the
wrong seam and fails this design."* That is a STOP and a report, never a
workaround. It did not fire.

## Clause 2 — `aion awl emit` byte-for-byte identical, all 424 documents

```
$ bash docs/evidence/r7-baseline/capture-awl-observable.sh \
       docs/evidence/r7-after target/release/aion \
       docs/evidence/r7-baseline/preexisting-awl-paths.txt
captured 424 documents into docs/evidence/r7-after/manifest.tsv

$ diff docs/evidence/r7-baseline/manifest.tsv docs/evidence/r7-after/manifest.tsv
$ echo $?
0
```

`emit-identity.diff` in this directory is that diff, recorded: **0 bytes.**

The comparison is over the FULL observable — `(exit_code, stdout, stderr)`
— not stdout alone, because 234 of the 424 are deliberately-invalid checker
fixtures whose *refusal* must also be unchanged.

The two documents design §7 names explicitly, before and after:

| Document | emit exit | stdout sha256 | stderr sha256 |
|---|---|---|---|
| `workflows/fleet-dev/fleet_dev.awl` | 0 | `dcbf2e5cc316…` | `e3b0c44298fc…` (empty) |
| `workflows/fleet-dev/fleet_ops_worker.awl` | 1 | `e3b0c44298fc…` (empty) | `f1a78a8adcb9…` |

Both rows are identical in the baseline and the after-capture.
`fleet_ops_worker.awl` exiting 1 is correct and unchanged: it is a WORKER
document and `emit` lowers workflows. Its refusal is part of the preserved
observable — which is precisely why the comparison includes stderr.

## Clause 3 — the fmt fixpoint, per document, unchanged

Every `fmt_fixpoint` and `fmt_output_sha256` column is identical between
the two manifests (they are in the same empty diff).

**72 of the 424 are NOT fmt-canonical today** and `fleet_dev.awl` is one of
them (movement finding M8 — see `../r7-baseline/README.md`). So design §7
clause 3's "clean" is not an invariant that holds on this tree; the
invariant proven instead is **preservation of each document's exact fmt
image**, byte-hashed.

## Clause 1 — new documents, not changed semantics

The coordinator and its worker are NEW documents under
`workflows/fleet-chain/`. The verifier action is deliberately NOT added to
`fleet_ops_worker.awl`. Design §10 refuses that shape by name
(Resolution C: *"Breaks byte-identical emission and widens the legacy
parallel contract for data used only by the chain"*).

New documents sit OUTSIDE the comparison set, as design §7's final ¶
provides. The set is frozen in `../r7-baseline/preexisting-awl-paths.txt`,
so it cannot drift when this flight adds documents:

```
$ git ls-files '*.awl' | wc -l      # after this flight
426
$ wc -l < docs/evidence/r7-baseline/preexisting-awl-paths.txt
424
```

## Clause 4 — exit 1/3 precedence and the legacy path, behaviorally unchanged

Proven EXECUTABLY, old script versus new script, by
`../r7-legacy-path/legacy-path-identity.sh`. See that directory.

## Clause 5 — a plan without a block, and the coordinator's own refusal

Two halves, both proven:

* the **checker** falls through to the pre-existing `op_emit 0` with
  byte-identical output (the harness above, case "exit 0 — plan and brief
  committed" and "exit 0 — 'fleet-stages-v1' in PROSE only");
* the **coordinator** refuses its own missing contract rather than
  reinterpreting a legacy flight — `fleet_chain.awl` step `check_plan`,
  outcome `legacy`, routes the `no_contract` outcome. The Rust half is
  `CheckOutcome::NoChainBlock`, pinned by
  `r8a_a_plan_without_a_block_takes_the_legacy_path`.
