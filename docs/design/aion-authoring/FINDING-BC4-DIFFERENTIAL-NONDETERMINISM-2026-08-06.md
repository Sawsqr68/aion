# Finding — the BC-4 differential gate cannot distinguish, and is red on main

> **STATUS: RESOLVED 2026-08-18** (was: Filed, not fixed). Resolved by the
> aion#42 lane and the steward's disposition
> (`gate-logs/42-fork-ordering/DISPOSITION.md`). This finding read the
> mechanism correctly on the first pass — "two concurrent branches landing in
> the opposite order — the trail is being compared without a total order over
> concurrent events" — and the remedy it proposed below ("sort fork branches by
> a stable key, not by arrival") is what shipped, scoped to directly adjacent
> activity terminals in `crates/aion/tests/common/terminal_order.rs`. Its
> instruction "**Do not** widen the `excluded` set to make it pass" was
> honoured: `EXCLUDED_OUT_OF_ORACLE` did not grow. #42 added what this finding
> could not: a load-independent reproduction, proof that the ordinal ASSIGNMENT
> half was intact all along, and proof that replay was never at risk because it
> binds by ordinal and not by position. Its second defect — that
> `cargo test -p aion-rs` stops after the first failing target, so `TEST_EXIT=$?`
> is not optional — is superseded by the estate's three-leg extraction standard,
> which runs `--no-fail-fast` and reports machine-readable finding lines.
> The sampling warning at the foot of this file stands unamended and still binds.

**Found:** 2026-08-06, incidentally, while establishing a baseline for the #7 build.
**Tree:** `9189ba585` (origin/main).
**Target:** `cargo test -p aion-rs --test awl_bc4_differential`,
test `covered::covered_set_is_byte_identical`.

## Two facts, each measured

### 1. The test is RED on pristine main

Not caused by the #7 working-tree changes. Established by stashing the six
tracked #7 paths, restoring the rename's delete side, confirming
`git status --porcelain` showed no tracked modification, and running the target.

Red in **both** pristine samples.

### 2. Its divergence count is not a measurement

Three runs, two of them on **byte-identical pristine code**:

| run | tree | DIVERGENCES | verdict |
|---|---|---|---|
| pristine 1 | `9189ba585`, no local change | **51** | RED |
| pristine 2 | `9189ba585`, no local change | **2** | RED |
| with #7 (a) | #7 working tree | 25 | RED |
| with #7 (b) | #7 working tree, clippy fixes only | **0** | **GREEN** |

Two pristine runs of *the same bytes* differ by more than an order of magnitude.
And the fourth sample is the decisive one: the **same tree** that produced 25
divergences produced **zero** on a later run and the test passed. The
`succeeded` / `failed-path` split also moved (62/37 vs 63/36), so the
instability is not confined to the divergence tally.

⇒ The test is not "red on main". It is **red or green depending on the run**,
which is worse: a green sample proves nothing, so no past green run of this
target ever certified anything either.

⇒ **A count that varies 2..51 across identical inputs cannot certify byte
identity.** The number carries no information about the change under test: the
#7 sample (25) sits inside the pristine range, so this harness could neither
convict nor clear it. An earlier version of this seat would have read "25 vs 51"
as an improvement. It is noise.

This is the estate's own rule one level down: *an indicator that cannot tell a
husk from a debt is a mood.* Here the indicator cannot tell a real divergence
from a rerun.

## Why it matters more than a flaky test normally would

BC-4 exists to prove the AWL direct-bytecode path and the Gleam reference path
produce **byte-identical** normalized trails. That is a determinism claim. A
harness that is itself nondeterministic cannot make a determinism claim — and
worse, a green run of it would be indistinguishable from a lucky one.

The observed divergences point at execution-order sensitivity, not compiler
output:

```
DIVERGENCE dag-fork/valid/after_multi_diamond event[0] : reference="disposition=Failed" direct="disposition=Parked"
DIVERGENCE dag-fork/valid/fork_named_branches event[6] /data/activity_id: reference=0 direct=1
DIVERGENCE dag-fork/valid/fork_named_branches event[6] /data/result/bytes/(length): reference=24 direct=13
```

`activity_id: 0` vs `1` in a **fork** fixture is two concurrent branches landing
in the opposite order — the trail is being compared without a total order over
concurrent events. `Failed` vs `Parked` is a timing race between a failure and a
timer park. Both are the harness comparing a partially-ordered history as if it
were a sequence.

## A second, quieter defect found the same way

`cargo test -p aion-rs` **stops after the first failing target**. The red run
emitted **9** `test result:` verdict lines; the green run emitted **59**.

A settle-check that counts verdict lines, or reads "0 failed" off the ones it
finds, sees 9 green-ish lines and no evidence that 50 targets never executed.
The only thing separating a truncated suite from a complete one is the process
exit code.

⇒ **`TEST_EXIT=$?` is not optional.** Any battery reading a cargo log without
capturing the exit code can report a 9-target run as a suite pass.

## What is NOT established

- Whether the underlying AWL/Gleam paths actually diverge semantically. The
  harness cannot currently answer that, which is the point.
- When this started. `#106` recorded `cargo test -p aion-rs` at 1015 passed /
  0 failed, so it was green at some past tree; the regression window is not
  bisected.
- Whether the 3 `timer-parked` and 3 `excluded` fixtures are related.

## The shape of the fix, not yet taken

Normalize before comparing: impose a total order on concurrent events (sort
fork branches by a stable key, not by arrival), and make the disposition
comparison wait for a settled terminal rather than sampling a transient one. If
the two paths genuinely differ, that must survive normalization — which is what
makes the gate worth having.

**Do not** widen the `excluded` set to make it pass. That converts a broken
oracle into a quiet one.

## Disposition

**RESOLVED 2026-08-18 by aion#42.** ~~Filed, not fixed.~~ The original reason
for parking it stands as written and is left here unedited:

> Filed, not fixed. It is outside the #7 lane and outside Tom's six-issue list;
> taking it silently would be inventing work. It is recorded here so the next
> seat does not re-derive it, and so no one reads a single BC-4 run as evidence
> in either direction.

That park held for twelve days and cost five red batteries before #42 was
opened on the same mechanism. The resolution is in
`gate-logs/42-fork-ordering/` (charter, disposition, REPORT.md, battery) and in
`crates/aion/tests/common/terminal_order.rs` with its red controls.

**Anyone citing a BC-4 divergence count as evidence must state how many samples
they took.** One is never enough.
