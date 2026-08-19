# BRIEF-STATUS-FIX2-AMEND — the oracle carries its population

**Status:** GO (Waffles' ruling 06:40Z 2026-08-16, verbatim below; dispatched by Vesper)
**Design of record:** BRIEF.md (R1-R5) + BRIEF-FIX2.md + FIX2-STOP-REPORT.md @ e53758842
**Scope:** ONE test amendment + full gates. Nothing else. No production source change
of any kind — e790ada74's fix was correct for the served surface; the stop report
proved the oracle's final leg counted the wrong population.

## The ruling being executed (Waffles, 06:40Z, after his own verification)

> AUTHORIZED — amend the oracle's final leg only. The raw-store check becomes
> key-scoped: find the fixture's (workflow_id, run_id) row and assert status
> Running, failure_reason None, close_time None. The HTTP assertions stay
> byte-identical — they are the front's actual subject and they pass. One
> requirement on the amendment: the population goes INTO the test — a comment
> at the store-read naming why the raw store legitimately holds more than the
> fixture's row (the coordinator, per internal_workflow.rs's contract), so the
> next reader doesn't re-derive this the hard way. This was a
> claims-carry-their-population defect in the oracle, and the amended oracle
> should carry its population visibly. No new red-first needed: the current
> failure IS the red, and it's committed evidence at e53758842's report.

## Requirements

R1. In `api::http::workflows::tests::http_list_refuses_terminal_status_without_terminal_history`
    (crates/aion-server/src/api/http/workflows.rs), replace ONLY the final
    raw-store leg (currently `corrected.len()==1` + index-0 assertions,
    lines ~866-871 at e53758842's parent): look up the row whose
    (workflow_id, run_id) equals the fixture's key and assert
    `status == WorkflowStatus::Running`, `failure_reason == None`,
    `close_time == None`. The lookup itself must fail the test loudly if the
    fixture's key is absent (no silent-pass on a missing row).
R2. The population comment at the store-read, per the ruling: why the raw
    store legitimately holds more than the fixture's row (the engine's
    schedule coordinator; cite `internal_workflow.rs`'s shared-store
    contract). State the constraint, not the change's history.
R3. HTTP assertions (lines ~859-864) stay BYTE-IDENTICAL. Every other test
    stays byte-identical. No production source change.
R4. No new red commit: the red is the committed evidence at e53758842
    (fix2-solo-red.log + fix2-key-diagnostic.log). Cite it in the commit
    message. One amendment commit (plus fmt only if fmt demands it).
R5. Full gates at final HEAD, SINGLE-INSTANCE (this run's server-run gate leg
    is the one instance; launch nothing else): all supplied gate-script legs
    EXIT=0, including the named test green solo AND inside the full
    aion-server suite.
R6. Delivery addendum appended to gate-logs/status-front-run/DELIVERY-REPORT.md:
    amendment commit hash, what changed, confirmation nothing else moved
    (`git diff --stat` against e53758842 committed in the addendum).

## REQUIRED: Check tiers
Mechanical: the key-scoped lookup is the check. Structural is unavailable —
the store's population is contract-shared with internal executions by design
(internal_workflow.rs); the test cannot make the coordinator unwritable.

## REQUIRED: Census controls
No census commissioned — no absence claim is made by this amendment.

## REQUIRED: Numbers
Counts carry their counter. The red measurements cited are Waffles' (06:20Z
solo 0.06s) and the builder's committed logs at e53758842.

## REQUIRED: Refusal surface
n/a — no new gates.

## REQUIRED: Timing sensitivity
NOT timing-sensitive.

## REQUIRED: Absence claims
"No production source change" is proven by the R6 diff-stat against e53758842.

## Hard constraints (estate law, non-negotiable)
Worktree `.worktrees/status-truth` only; branch `status-truth`. NEVER push —
landing is the dispatcher's re-gate on the FINAL tree. Live server :8080 is
production — READ-ONLY. No new dotfolders, no daemons, no CARGO_TARGET_DIR
override.

## Done means
Amended test green solo and in the full suite; all gate legs EXIT=0
single-instance at final HEAD; R6 addendum shows the diff-stat touching only
workflows.rs (+ DELIVERY-REPORT.md); tree clean for the dispatcher's re-gate.
