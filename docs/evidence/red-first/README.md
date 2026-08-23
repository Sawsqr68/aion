# Red-first evidence

The standing evidence law is that every requirement lands red-first with
the red committed here. This directory carries both halves of that, and is
explicit about which is which.

## 1. Chronological reds — what the first runs actually produced

| File | What went red |
|---|---|
| `r1-first-run.txt` | The R1 suite's first run. `canonical_form_refusals_are_each_named` failed: the fixture handed every case the leg list `["a","b"]`, so §5.2 clause 5 (`leg_not_placed`) fired before the malformed-`admit` case could. A fixture defect, and a real red — fixed by giving each case its own dispatch leg list so no unrelated clause can mask the refusal under test. |
| `r6-first-run.txt` | The R6 suite's first run, two failures. (a) `crash_and_resume_at_every_one_of_the_seven_boundaries` could not derive a control action at boundary 1, because `chain-next` treated an absent ledger as an error rather than as the empty ledger that boundary 1 IS. (b) `no_pattern_kill_anywhere` matched its own source: the forbidden spellings were written literally in the needle list. |

Both were fixed by changing the code, not the assertion. The second is
worth naming: the kill-scope sweep could only have been made green by
exempting itself, which is the exact bypass the wall exists to prevent, so
the needles are now ASSEMBLED at runtime and the sweep reads code lines
only, never comment prose.

## 2. The mutation matrix — proving every specimen is load-bearing

`mutation-matrix.sh`.

A chronological red proves that one test could fail once. It does not prove
that **each** clause's refusal specimen is testing **that clause**. A
specimen can pass for the wrong reason — an earlier clause firing first, a
grammar error swallowing the case — and stay green forever.

The mutation matrix proves the property directly and reproducibly:
**neutralize exactly one clause, and the specimen keyed to it must go RED.**
Each mutation makes one function return `Ok(())` before it can refuse;
nothing else changes; the source is restored after every run.

```
$ bash docs/evidence/red-first/mutation-matrix.sh
=== mutation matrix: neutralize one clause, the suite must go RED ===
killed    charset clause (stage + leg ids)
killed    clause 2  — stage id nonempty
killed    clause 2  — stage id uniqueness
killed    clause 3  — leg uniqueness
killed    clauses 4+5 — dispatch leg cover
killed    clause 6  — stage has legs
killed    clause 10 — conclusion owner
killed    clause 11 — multi-leg vocabulary
killed    clause 12 — multi-value legs
killed    clause 15 — distinct conclusion paths
killed    clause 7  — predecessors
killed    clause 9  — acyclicity
killed    clause 8  — chain cardinality
killed    clauses 13+14 — admit references
killed    clause 16 — advance

=== the R3 seal: neutralize the exact-object-id gate ===
killed    R3 seal — a ref name is refused before any git process starts

=== the R3 token contract: accept anything as a well-formed token ===
killed    R3 token contract

=== the chain decision function: R5 base law and R6/R8 ordering ===
killed    R5 clause 4 — successor base = accepted SHA
killed    R8 §9.4 — admission evaluated from recorded history
killed    R3/R4 §2.6 — verify BEFORE accept-or-hold

mutations: 20   survivors: 0
MUTATION MATRIX: PASS — every clause is load-bearing; neutralizing any one turns the suite RED
```

**20 mutations, zero survivors.**

## 3. The green run

`full-green-run.txt` — the whole `aion-fleet-plan` suite, 85 tests.

## Fixture-size law

**Boundary-level synthetic inputs throughout — never multi-GB fixtures.**
The largest input any test in this crate constructs is a three-line
`fleet-stages-v1` block; the largest byte specimen handed to the conclusion
verifier is 20 bytes; the largest ledger is twelve JSON records. R3's
fixtures are scratch git repositories holding three files. Nothing here
generates a corpus and nothing grows with time.
