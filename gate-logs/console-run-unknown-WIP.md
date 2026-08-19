# console "run unknown" fix — WORK IN PROGRESS, resume sheet

Diagnosis: `gate-logs/console-run-unknown-diagnosis.md`. Ratified by Waffles as
plan of record. 🔴 **NOT to be deployed to Tom tonight** — it needs a rebuilt
server binary and no second engine swap goes on his box on an ungated build. It
takes a queue line and rides the release line with a proper battery.

## Design (settled)

The server states the generation boundaries; the console stops deriving them.
Costs **no extra store read**: the describe handler already reads full history,
and the projection reuses the store's own `run_chain_from_history` rather than
re-scanning for `WorkflowStarted` (one implementation of what a generation is).

## DONE — Rust side, all three crates compile clean

- `crates/aion-store/src/store.rs` — `RunSummary` gains `started_seq: u64`, the
  sequence of its `WorkflowStarted`. The chain carried a *timestamp* but never
  its boundary sequence, and sequence is the axis that locates an event's
  generation.
- `crates/aion-store/src/run_chain.rs` — populated from `envelope.seq` (already
  destructured there).
- `crates/aion-store/src/conformance.rs` — 6 fixture sites, values taken from
  each fixture's own event seqs (1/3, 1, 1/3/5), not invented.
- `crates/aion-core/src/describe.rs` — new `RunGeneration { seq, run_id }`;
  `DescribeWorkflowResponse.generations: Vec<RunGeneration>`.
- `crates/aion-core/src/lib.rs` — re-export `RunGeneration`.
- `crates/aion-core/src/generated_types.rs` — `push_type::<RunGeneration>`.
  (The bindings pipeline **caught** the omission with an exact remedy — a good
  gate. `AION_UPDATE_BINDINGS=1 cargo test -p aion-core export_ops_console_wire_types`.)
- `crates/aion-server/src/api/http/workflows.rs` — `described.generations`
  projected from `outcome.history`, same shape as the existing `unserved`.
- `crates/aion-server/src/api/http/payload.rs` — construction site gets
  `generations: Vec::new()` (the handler attaches the real value, as with
  `unserved`).
- `apps/aion-ops-console/src/types/generated/index.ts` — REGENERATED, carries
  `RunGeneration` and `generations: Array<RunGeneration>`.

Verified: `cargo check -p aion-store --all-targets`, `-p aion-core --all-targets`,
`-p aion-server` all clean.

## DONE — console, partially

- `lib/attemptNavigator.ts` — **`deriveGenerations` DELETED** along with the
  local `Generation` type. `deriveAttemptNavigator(timeline, generations)` now
  takes the server list. `AttemptNavigatorRow.runId` and `AttemptSlot.runId`
  tightened to `RunId` (no longer nullable); `attemptRowKey` takes `RunId` and
  the `-` sentinel is gone; `generationAt` returns `RunId | undefined` where
  `undefined` is reachable ONLY for an empty list (a workflow with no history,
  hence no rows) — a totality answer, not a "cannot say". `compareRows`'
  null-rank branch removed. Module doc rewritten to record why.
- `lib/api/client-normalize.ts` — new `normalizeDescribeGenerations`, strict
  (throws `ApiError` on a missing array or a malformed entry; no silent `[]`).
- `lib/api/client-types.ts` — new `WorkflowHead { summary, generations }`.
- `lib/api/client.ts` — `getWorkflowSummary` → **`getWorkflowHead`**, returning
  `WorkflowHead`. Renamed because the call now returns more than a summary.

## TODO — the remaining console edits

1. `hooks/useWorkflowSummary.ts` → rename to the head read; `WorkflowSummaryClient`
   becomes `Pick<ApiClient, 'getWorkflowHead'>`; query type `WorkflowHead`.
   Update `features/workflow-detail/index.ts` exports.
2. `swimlane/WorkflowDetailView.tsx:41` — consumer becomes `head.data?.summary`;
   pass `head.data?.generations ?? []` down to `AttemptNavigator`.
3. `swimlane/AttemptNavigator.tsx`:
   - accept a `generations` prop; `deriveAttemptNavigator(timeline, generations)`
   - `runIds` memo: `generations.map((g) => g.run_id)` (was `deriveGenerations(timeline)`)
   - **DELETE** the `row.runId === null` render branch at ~209-217 (the
     `run unknown` span, its `data-testid={`unresolved-run:…`}` and its tooltip)
   - **DELETE** the `selectedRow.runId === null` guard at ~144 and its banner
4. `__tests__/attemptRowList.test.tsx:72` asserts `'run unknown'` renders — that
   assertion goes with the branch. 🔴 Per *narrowing makes downstream tests
   vacuous*, the replacement is NOT "assert it's absent": it is a case that would
   have shown `null` under the old derivation (a tail window containing no
   `WorkflowStarted`) and now resolves to the correct run from the server list.
5. Typecheck: the console has no local `tsc` on PATH via npx — use the repo's own
   script (`npm run typecheck` / `bun run` in `apps/aion-ops-console`), not
   `npx tsc`.
6. Other callers of `deriveAttemptNavigator` / `attemptRowKey` / the old summary
   hook will surface as type errors; fix each rather than widening a type back.

## Note for Waffles' both-ends rule

Waffles required reader-side-first, tolerant of the field's absence. **That rule
does not bind here and I said so rather than diverging silently:** `rust_embed`
compiles the built console from `crates/aion-server/ops-console-embed/` INTO the
server binary (`src/ops_console/assets.rs`), so the console a user loads is
served by the very binary answering its requests. No version skew is possible in
a deployed aion, and CLAUDE.md forbids compat shims during this build. Dev-mode
(vite against a separate server) is the only skew surface. Awaiting his ruling;
if he wants tolerance anyway, make `generations` optional in
`normalizeDescribeGenerations` ONLY — never restore the window scan.
