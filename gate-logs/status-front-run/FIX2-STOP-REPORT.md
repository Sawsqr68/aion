# BRIEF-STATUS-FIX2 stop report

Date: 2026-08-16 UTC
Base and diagnostic HEAD: `e790ada7410afc0c70d6f6ce8448dadd724503dd`
Branch/worktree: `status-truth` / `.worktrees/status-truth`

## Disposition

STOP under R2's explicit fixture-invalidity branch. No production or test source change is proposed at this handoff.

The supplied oracle's final `corrected.len() == 1` assertion reads the unfiltered shared visibility store. `shared_engine()` builds a real engine, whose cold-start path seeds and reconciles the engine-internal schedule coordinator into that same store. The repository contract at `crates/aion-server/src/internal_workflow.rs:3-11` explicitly says internal executions live in the same event and visibility stores and are removed only from user-facing enumeration surfaces. Therefore a raw store containing the fixture plus the coordinator has length two by design.

The HTTP response itself contains exactly one user row and has already corrected the fixture from `Failed` to `Running`. The failure is only the raw-store cardinality assertion after that response.

## R1 — printed key evidence

Committed diagnostic output: `fix2-key-diagnostic.log`.

It prints the two surviving keys:

1. Coordinator: `(00000000-a10a-0000-0000-000000000004, 00000000-a10a-0000-0000-000000000005)`, type `aion.schedule_coordinator`, status `Running`.
2. Fixture: `(00000000-0000-0000-0000-000000000001, 00000000-0000-0000-0000-00000000000a)`, type `fixture`, status `Running`.

The fixture's planted `Failed` row and history projection use the same key. `Recorder::record_workflow_started` preserves `WorkflowStartRecord.run_id` (`crates/aion/src/durability/recorder.rs:127-150`), and the deterministic fixture helpers provide run `...000a` (`crates/aion-server/src/api/http/test_support.rs:189-195`). There is no Recorder-minted identity divergence.

The coordinator is the second writer's row: cold startup calls `seed_schedule_coordinator_history` before `reconcile_visibility` (`crates/aion/src/engine/startup.rs:64-74`). Both surviving rows therefore have authoritative histories.

## Census controls

Committed census: `fix2-writer-census.log`.

Instrument and scope: `git grep` over all tracked `*.rs` files.

- `reconcile_visibility` call-site denominator: **4 production calls**. The separate definition positive control is **1**.
- `.record_visibility(` call-syntax denominator: **20 tracked-Rust occurrences**. The definition positive control is **8**. Of the 20 calls, the production projection calls are Recorder fan-out (`recorder.rs:1030`), targeted lifecycle upsert (`visibility.rs:27`), and reconciliation (`visibility.rs:61`); the other 17 occur in tests or test delegating wrappers.

Relevant passes in this fixture are cold-start reconciliation (coordinator row) and HTTP-list reconciliation (fixture correction). This is not a claim that no other writer exists; the committed search carries the complete stated scope and denominator.

## Check tiers

For the actual general defect described by the brief, the mechanical disposition would be an authoritative-key sweep: reconciliation upserts every history-backed projection and removes each pre-existing visibility key absent from the history-derived key set. That requires a key-specific removal seam on `VisibilityStore` and both durable implementations.

Structural prevention is unavailable without a broader redesign: visibility is intentionally a separate mutable projection, multiple lifecycle/Recorder paths write it after durable history changes, and reconciliation exists to repair divergence after interrupted or stale projection writes.

That mechanical fix is not implemented here because the supplied byte-identical test contains no history-unaccounted row. It would correctly retain both observed keys and remain red. Deleting the coordinator or preventing its projection would violate the explicit shared-store contract and would not fix stale orphan rows.

## Red evidence and timing

- Supplied measurement: Waffles, 2026-08-16 around 06:20Z, solo/uncontended, 0.06s.
- Reproduced at `e790ada74`: `fix2-solo-red.log`, exit 101, one test failed in 0.05s, `left: 2`, `right: 1` at the final raw-store assertion.
- Diagnostic reproduction: `fix2-key-diagnostic.log`, exit 101, one test failed in 0.05s, with both keys printed.

This is not timing-sensitive.

## Gates and non-claims

The full R4 gate battery was not run because the mandatory oracle remains red and R2 directs a stop when the fixture's construction invalidates the byte-identical oracle. No claim of battery green, implementation completion, or landing readiness is made.

No new surface, presentation work, supersession grouping, reason-extraction change, refusal surface, daemon, alternate target directory, live `:8080` mutation, or push was performed.
