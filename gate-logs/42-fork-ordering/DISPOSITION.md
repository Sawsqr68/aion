# Steward's disposition: aion#42 — the harness normalizes, the engine keeps its freedom

**Date:** 2026-08-18 · **Steward:** Vesper Lynd
**Rules on:** the lane's REPORT.md (worktree `42-fork-ordering`, three commits
off charter base), verified at the bytes at my seat (cursor.rs doctrine,
fail-fast pin, red receipt, battery-of-record instrument proof).

## The ruling

**The engine is not changed.** Out-of-ordinal terminal recording is
declined-by-design AND measured as load-bearing (fail-fast cancels a running
sibling only because a higher ordinal's failure records first — the pinned
trail at `concurrency_e2e.rs` proves it). Imposing a total order would trade a
real capability for a property no consumer needs: replay binds by ordinal and
the lane's control arm proved inverted histories replay correctly.

**The differential harness gains a canonical order over concurrent activity
terminals, keyed by ordinal** — the shape the 2026-08-06 finding proposed and
the lane's evidence now justifies. The differential's job is to assert what
the design PROMISES: same event set, same ordinal assignment, same per-ordinal
bindings, same command-stream order. Concurrent-terminal position is not a
promised property; comparing it is asserting the enumeration, not the
invariant.

## Conditions (each is a gate, not advice)

1. **Scope of the normalizer.** It may reorder ONLY what the design permits:
   activity terminal events (`acomp`/`afail`/`acancel`) relative to each
   other within a contiguous run not separated by any other event kind.
   Nothing else moves — not `sched` anchors, not signals, not child
   terminals, not workflow lifecycle events. If that definition proves too
   narrow or too wide against the fixtures, STOP and report; do not widen
   silently.
2. **The normalizer must be proven blind only to the permitted freedom.**
   Four red controls, each built from a literal defect shape, each red
   through the NORMALIZED comparison:
   a. a result's bytes bound to the wrong ordinal — red;
   b. a missing or extra event — red;
   c. a terminal recorded before its own `sched` anchor — red;
   d. an event moved across a non-terminal boundary (e.g. past a signal or a
      lifecycle event) — red.
3. **Decision 11 is amended at the design level, not silently in test code.**
   `awl_bc4_differential/report.rs` doc and the BC-4 brief's premise
   ("trails can only diverge on backend behavior",
   `BC-4-DIFFERENTIAL-BRIEF.md:47-51`) are corrected to state the real
   contract: a divergence in PROMISED properties is never normalized away;
   concurrent-terminal position is not one. The 2026-08-06 finding
   (`FINDING-BC4-DIFFERENTIAL-NONDETERMINISM-2026-08-06.md`) is dispositioned
   in place (Filed → Resolved, pointing at #42's lane evidence and this
   ruling). If the brief carries JSON sources, re-render per house rules.
4. **The lane's deliberately-red reproduction is re-cast, not deleted.** It
   becomes the permanent pin of the mechanism: both settle orders occur, the
   `sched` anchors and per-ordinal bindings are identical, and the NORMALIZED
   trails are equal — assert the invariant, not the enumeration. The control
   arm (inverted replay) stays exactly as written.
5. **No fixture excluded, no assertion weakened beyond the scoped
   normalization.** `EXCLUDED_OUT_OF_ORACLE` does not grow.

## Leg 2 (same builder, separate zone): the worker_shell pid-file race

`aion-cli` worker_shell's `cancellation_reaps_the_entire_spawned_process_group`
has failed twice across batteries with `ParseIntError { kind: Empty }` — the
lane diagnosed the mechanism to the line: the test's `wait_for_pid_file`
polls `is_file()` while the spawned shell's `echo $$ >` creates-then-writes,
so the reader can observe an existing empty file. Fix the waiter to wait for
a PARSEABLE pid (non-empty content that parses), never mere existence, and
state in the test why existence is not readiness. Zone: `crates/aion-cli`
worker_shell tests only. Board row to be minted and closed by this leg.

## Collection

Same worktree, same branch, on top of the lane's three commits. The branch
lands only when the recast reproduction and the four normalizer controls are
green, the battery of record is green-with-completion-proof, and the board
rows (#42, the pid-file row) carry their evidence. Standard rules: cite by
subject, REPORT.md appended not rewritten, extraction battery per the
standard, `NORN_BIN` set.
