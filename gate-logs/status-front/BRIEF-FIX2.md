# BRIEF-STATUS-FIX2 — the reconciled list must serve exactly one row per execution

**Status:** GO
**Design of record:** gate-logs/status-front/BRIEF.md (R1-R5) + DELIVERY-REPORT.md @ e790ada74
**Scope cut:** ONLY the duplicate-row defect below + battery green. No new surfaces, no
presentation work (supersession grouping is a separate brief), no changes to R4's reason
extraction.

## The defect being closed

At branch HEAD e790ada74, `api::http::workflows::tests::
http_list_refuses_terminal_status_without_terminal_history` fails SOLO and UNCONTENDED
(cargo test -p aion-server, single test, 0.06s, Waffles's hands, 2026-08-16 ~06:20Z):
assertion at crates/aion-server/src/api/http/workflows.rs:869 — `summaries.len()` left=2,
right=1. The fixture plants ONE execution (history via Recorder: WorkflowStarted +
SearchAttributesUpdated, non-terminal) plus ONE divergent visibility row (Failed,
close_time set) and the reconciled list serves TWO rows.

Measured boundary (Waffles, at the bytes): InMemoryStore::record_visibility upserts by
key `(workflow_id, run_id)` (memory.rs:37-43 HashMap insert). Two rows surviving therefore
means the reconciliation's corrected record carries a DIFFERENT key than the fixture's
stale row. `reconcile_visibility` (crates/aion/src/lifecycle/visibility.rs:37) projects
`started_run_id(&history)` and upserts — it NEVER removes or corrects a visibility row
whose key matches no history projection. Diagnose WHICH two keys exist in the failing
fixture and WHY they differ (candidate: Recorder/engine minting a run identity that
differs from the WorkflowStartRecord's run_id; or a second projection writer firing during
the fixture), then fix at the source. The fix must hold for the general shape: after
reconciliation, a visibility row that history cannot account for must not be served as a
workflow that exists.

## Requirements

R1. Name the two keys in the failing fixture with printed evidence (a diagnostic run's
    output committed in the evidence log, not prose). If the cause is a second writer,
    census its call sites before choosing the fix site.
R2. Fix at the mechanism, not the fixture: do NOT edit the failing test's assertions.
    The test is the oracle; it stays byte-identical unless R1 proves the FIXTURE (not the
    assertion) constructs an impossible state — in which case stop and report instead.
R3. Red evidence first: the failing test's current red at your HEAD is already the red —
    cite it; any NEW pin you add (e.g. stale-row-removal) gets its own red commit before
    its fix commit.
R4. Full gates at final HEAD: the supplied gate script legs (fmt, clippy workspace +
    per-crate, aion-server, aion-rs, stores, scripts/battery.sh) ALL EXIT=0, run
    SINGLE-INSTANCE (the server-run gate leg is the one instance; nothing else launches a
    battery against this worktree).
R5. Honest delivery addendum appended to DELIVERY-REPORT.md: what the key divergence was,
    what changed, what did not.

## REQUIRED: Check tiers
The stale-row disposition is mechanical (reconcile pass corrects-or-removes with the
refusal/correction visible in the projection); name why structural (unwritable divergent
row) is unavailable if you don't take it.

## REQUIRED: Census controls
Only if R1 finds a second writer: census its call sites with denominator + positive
control. Otherwise state "no census commissioned."

## REQUIRED: Numbers
Counts carry their counter. The solo-red measurement above is Waffles's, 2026-08-16.

## REQUIRED: Refusal surface
n/a — no new gates beyond the existing test oracles.

## REQUIRED: Timing sensitivity
NOT timing-sensitive. The solo red reproduces in 0.06s uncontended.

## REQUIRED: Absence claims
Any "no other writer exists" claim carries its search (instrument + scope).

## Hard constraints (estate law, non-negotiable)
Worktree `.worktrees/status-truth` only; branch `status-truth`; base now e790ada74.
NEVER push — landing is the dispatcher's re-gate on the FINAL tree. Commit at truthful
handoff points as you go. Live server :8080 is production — READ-ONLY. No new dotfolders,
no daemons, no CARGO_TARGET_DIR override.

## Done means
The named test green solo AND inside the full aion-server suite; all gate legs EXIT=0
single-instance at final HEAD; delivery addendum states what is NOT done as plainly as
what is; tree clean for the dispatcher's independent re-gate.
