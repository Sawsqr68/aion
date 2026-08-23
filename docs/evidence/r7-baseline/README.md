# R7-a — the pre-existing-AWL baseline, captured BEFORE any source edit

Captured at HEAD `572d27da6bd52db5e1437a355b9c230b7d4752d4` with
`target/release/aion` (**`aion 0.20.0`, built from this tree**). The `aion`
on `PATH` is 0.17.0 — stale against this v0.20.0 tree and used for no proof
in this flight.

R7 is the only requirement with a step that is **destroyed by starting the
work**, so this capture is the flight's first recorded act after the census.

## Corpus — pathspec stated

| Scope | Pathspec | Count |
|---|---|---|
| Comparison set (design §7 clause 2, "every pre-existing AWL document in the repository") | `git ls-files '*.awl'` | **424** |
| The design's narrow named core | `git ls-files 'workflows/fleet-dev/*.awl'` | **2** |
| `workflows/**` | `git ls-files 'workflows/*.awl'` | 17 |
| `selfcheck/` AWL documents | `git ls-files 'workflows/fleet-dev/selfcheck/*.awl'` | **0** (only `capture-selfcheck.sh`) |

The exact 424 paths are frozen in `preexisting-awl-paths.txt`; the capture
script reads that list, so the comparison set cannot drift when this flight
adds new documents.

## What is captured, and why

`manifest.tsv`, one row per document:

| Column | Why |
|---|---|
| `emit_exit`, `emit_stdout_sha256`, `emit_stderr_sha256` | The **full observable** of `aion awl emit`. Not stdout alone: **234 of the 424** documents are deliberately invalid checker fixtures whose *refusal* must also be unchanged, and a refusal lives in `(exit, stderr)`. |
| `fmt_exit`, `fmt_fixpoint`, `fmt_output_sha256` | The M1 fmt fixpoint. |

### M1 — `aion awl fmt --check` DOES NOT EXIST on this tree

Design §7 clause 3 and brief R7 both name it. `AwlCommand::Fmt`
(`crates/aion-cli/src/awl.rs`, enum `AwlCommand`, variant `Fmt`) carries one
field, `file: PathBuf`, and its own doc says: *"the printer is the single
formatter, one rendering, so there is no `--check` mode."*

The design's INTENT is a fixpoint proof, discharged with the CLI that
exists: **format a COPY in a scratch dir, then `diff` against the
original.** The tree is never formatted in place — proven by
`git status --porcelain -- '*.awl'` being empty of modifications across
both captures.

### M8 — 72 of the 424 are NOT fmt-canonical today, `fleet_dev.awl` among them

**This is a new movement finding, beyond the plan's M1–M7.** Design §7
clause 3 requires `fmt --check` to "remain **clean** on every pre-existing
document". Measured on this tree, `aion awl fmt` on a copy of
`workflows/fleet-dev/fleet_dev.awl` produces bytes that differ from the
original: it is **NOT-FIXPOINT**. So is `fleet_ops_worker.awl`'s sibling set
— 72 documents in total.

Cleanliness is therefore **not** an invariant that holds today and cannot be
the thing preserved. The invariant this flight proves instead is
**preservation of each document's exact fmt image**, which is why
`fmt_output_sha256` is captured: "exactly as non-canonical after" is a
byte-level claim, not a boolean. Reported to the design's owner.

## Distribution at baseline

| Measure | Value |
|---|---|
| Documents captured | 424 |
| `emit` exit 0 | 190 |
| `emit` exit 1 (deliberately-invalid fixtures; refusal must be preserved) | 234 |
| fmt fixpoint | 352 |
| fmt NOT-FIXPOINT | **72** |

## Fixture-size law

**Boundary-level synthetic inputs only — never multi-GB fixtures.** Every
input to this capture is a document already tracked in the repository; the
largest is `workflows/fleet-dev/fleet_dev.awl` at 434 lines. The captured
manifest is a few tens of KB of hashes. Nothing here generates a corpus.

## How R7-b re-runs it

```sh
bash docs/evidence/r7-baseline/capture-awl-observable.sh \
     docs/evidence/r7-after target/release/aion \
     docs/evidence/r7-baseline/preexisting-awl-paths.txt
diff docs/evidence/r7-baseline/manifest.tsv docs/evidence/r7-after/manifest.tsv
```

**The recorded diff must be EMPTY.** Design §7, verbatim: *"If
implementation cannot meet byte-identical emission for a pre-existing
parallel-only document, it has changed the wrong seam and fails this
design."* That is a STOP and a report, never a workaround.
