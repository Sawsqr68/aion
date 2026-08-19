# The integration gate never looked at the console, and the console was broken

**2026-08-19, Vesper.** A defect in **my own gate**, found by the leg-3 lane and
traced back here. Recorded because the branch is a candidate for main.

## What was wrong

`integration-20260819` @ `53023ac6d` failed the console typecheck: **21 errors
across 19 files**, every one the same defect — a test fixture constructing an
event without `parent_workflow_id`, which the generated TypeScript type requires.

```
TYPECHECK_EXIT=2      21 errors      19 files      42 mentions of parent_workflow_id
```

## Why my gate could not see it

`parent_workflow_id` is #77's field. It is the **one conflict** the integration
merge produced, and I resolved it by keeping the field — correctly. The Rust
side kept it, the generated TS carried it, and 19 console test fixtures were
never updated to match.

My gate legs were **all Rust**: clippy `--workspace`, clippy per-crate,
`aion-server --lib`, `aion-mcp`, `aion-store --lib`, `cargo doc -p aion-store`.
There was **no console typecheck and no JS test leg at all.** The gate was
scoped to the shape of the diff — the Rust crates the merge touched — when the
change was the addition of a required field to a **wire type whose closure
includes the generated TypeScript and every fixture that constructs the event.**

That is my own rule broken by me: *a change adding to a closure widens the
battery to the closure's shape, not the diff's.* I wrote the field-keeping
resolution and then gated only half of what it reached.

## How it was found, and how attribution was established

The leg-3 lane ran a console typecheck as part of R4 and honestly recorded
`typecheck=2`. Attribution mattered — a lane's own break and an inherited break
call for opposite responses — so it was settled before acting, three ways:

1. **`parent_workflow_id` was already in the committed generated types at the
   lane's base sha**, and unchanged by the lane's regeneration. The count is 1
   before and 1 after.
2. **The lane touched none of the 19 files the errors live in.** Its changes are
   in the transcript feature; the errors are in live-feed, workflow-detail,
   workflow-list and lib/api.
3. **Reproduced directly at `53023ac6d`** — this branch, with dependencies
   installed — giving the same 21 errors in the same 19 files.

Pre-existing, and mine.

### A false baseline caught on the way

The first attempt to measure the baseline ran the typecheck in the `port-247`
worktree, which has **no `node_modules`**. It produced 3512 errors, all of the
form "cannot find module 'react'". That number measures an uninstalled tree, not
the code, and it would have "confirmed" a pre-existing failure for entirely the
wrong reason. A tool that fails into a convenient answer is not evidence —
`bun install` first, then measure.

## The fix

`parent_workflow_id: null` added to the **19 fixtures whose `parent_run_id` is
`null`**.

🔴 **Two sites were deliberately left alone.** `timeline.test.ts:238` and
`WorkflowList.test.tsx:509` carry a non-null `parent_run_id`, and both are
`WorkflowContinuedAsNew` — where that field names the **previous run of the same
workflow**, not a parent workflow. Neither is among the 21 errors. Filling them
with `null` would have produced a fixture asserting a run with a parent run and
no parent workflow, which is a shape the system does not produce. The typecheck
passing afterwards confirms the type does not require the field there.

## After

| Leg | Result |
|---|---|
| `bun run typecheck` (`tsc --noEmit`) | **exit 0**, 0 errors |
| `bun run check` (biome, 444 files) | **exit 0** |
| `bun test` | **1040 pass, 0 fail**, 138 files |

## What this changes about the gate

Any future battery on this branch — and the one that gates its merge to main —
**must include the console legs.** A Rust-only battery on a repository whose
wire types generate TypeScript can be green and wrong at the same time, and was.
