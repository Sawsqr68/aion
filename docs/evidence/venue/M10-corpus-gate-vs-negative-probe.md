# M10 — this flight's own negative probe is a `.awl` the corpus gate calls living

Found by the workspace suite, which the gate reaches only once `fmt` and
`clippy` are green (see `M9-preexisting-fmt-red.md` for that chain). Unlike
M9 this red is **this flight's own**, and it is fixed rather than reported.

## The red, verbatim

```
thread 'every_living_awl_document_checks_clean' panicked at crates/aion-cli/tests/awl_corpus_gate.rs:203:5:
1 living document(s) fail `aion awl check` — every living `.awl` in the tree must check clean, or it teaches (or serves) a shape the language refuses:

docs/evidence/probes/q2b_failure_label_negative.awl:
docs/evidence/probes/q2b_failure_label_negative.awl:18:25: error: expected an expression, found keyword `failure`
```

## Provenance: ours, not the venue's

```
$ git log --oneline -1 -- docs/evidence/probes/q2b_failure_label_negative.awl
73ef76ff evidence: R7-a baseline + Act-3 probes, before any source edit

$ git cat-file -e c590712f:docs/evidence/probes/q2b_failure_label_negative.awl
(absent at rig base)
```

Commit `73ef76ff` is a flight commit (rig base `c590712f` is its ancestor).
This flight put the document in the tree; this flight owns the red.

## Cause

The document is a **negative probe**. Its own header states the contract:

```
//! Q2(b) NEGATIVE PROBE: try to observe the child's typed failure label
//! inside `on failure`. If M4 is right this must NOT check.
```

and `docs/evidence/probes/OBSERVED.md` records its exit status as the
*answer*: `| Q2b | Does `on failure` bind the child's typed outcome label? |
q2b_failure_label_negative.awl | **1** | **NO** |`.

So the refusal is the finding. There is no version of this file that both
checks clean and remains the evidence it was recorded as — Q2b's answer IS
that the checker refuses this shape.

The defect is therefore in where the gate draws the line around "living".
`awl_corpus_gate` excludes dated-record areas by class, and its own
doc-comment already names this exact species — "the playtest probes, several
of which exist to be refused" — but it scopes that exclusion to `docs/design`
alone. `docs/evidence/` is the same class of area: dated records of what was
measured on a date. It is the area `.fleet/gate-entry.sh` itself requires
this flight to write into.

**The sibling gate had already ruled this way.** In the same directory,
`crates/aion-cli/tests/retired_doctrine_gate.rs` sweeps the living tree for
retired doctrine and excludes:

```rust
fn area_excluded(relative: &Path) -> bool {
    relative.starts_with("docs/design")
        || relative.starts_with("docs/archive")
        || relative.starts_with("docs/evidence")
        || relative.starts_with("gate-logs")
}
```

with the stated reason "dated records of what was believed or measured on a
date — rewriting them to keep a gate green would falsify the record". Two
gates sweeping one tree disagreed about what "living" means. That
disagreement is the bug.

## Fix

`awl_corpus_gate`'s `area_excluded` now names the same four areas as its
sibling, and its doc-comment states the reason and the alignment.

## What was NOT done, and why

- **The probe was not rewritten to check clean.** That would delete the
  answer to Q2b and falsify `OBSERVED.md` — the precise failure mode the
  gate's own doc-comment warns against.
- **The probe was not deleted.** Same reason.
- **The probe was not moved** (e.g. under a `fixtures/` directory, which the
  gate already excludes). `OBSERVED.md` records verbatim transcripts keyed to
  the path `docs/evidence/probes/q2b_failure_label_negative.awl`; moving the
  file makes a recorded command line irreproducible.
- **No `#[ignore]`, in any spelling.** All three tests in the file still run.
- **No assertion was deleted or weakened.** The sweep, the `CORPUS_FLOOR`
  non-vacuity assertion, the five area ANCHORS, and the negative control
  (`the_gate_refuses_the_retired_three_field_agent_shape`, which proves the
  checking path can still go red) are untouched.

## The exclusion is measured, not assumed

The corpus is derived by a walk, so the fix is only sound if the walk still
finds a real corpus. Re-deriving both walks over this tree:

```
corpus before the fix                    : 60 documents
corpus after excluding docs/evidence     : 55 documents
dropped                                  : exactly the 5 files under docs/evidence/probes/
CORPUS_FLOOR                             : 35   (55 >= 35)
all 5 ANCHORS still present              : yes
```

Nothing but this flight's own probe directory left the corpus. The four
positive probes (Q1, Q2a, Q3, Q4) that leave with it check clean today and
are recorded as exit 0 in `OBSERVED.md`; they are dated evidence of the same
class, not documents a reader is invited to copy or a server serves.
