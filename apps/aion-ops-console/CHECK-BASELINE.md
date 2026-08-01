# `bun run check` — declared gate + named finding baseline

**Ruling (2026-08-01, joint word Cally Ray + Waffles the Terrible, task #131):**
`bun run check` (biome check .) in `apps/aion-ops-console` **is a gate**. Its
red state on main was 13 real findings, not noise, so the instrument is
enforced rather than renamed into harmlessness.

## Terms (all ratified)

1. **The baseline is this NAMED set** — file + position + finding, exactly as
   the tool reported at capture. A count baseline would let
   one-fixed-one-added pass as zero delta; names cannot.
2. **Legs gate on DELTA-ZERO against this set, not absolute green.** A diff
   must add no findings; a seat does not inherit the 13 by touching the repo.
   Finding the hole must not be a losing move.
3. **The 13 are a named debt item with an owner** (task #131, seat: Vesper).
4. **Trigger, declared with the gate:** any leg touching a file in this set
   clears that file's findings as part of its own gate, with a **backstop at
   the next aion release cut** — whichever arrives first. Complexity findings
   are cleared by refactor only; `// biome-ignore` suppressions are bypasses,
   not fixes (same law as `#[allow]`).

## The baseline set

Captured 2026-08-01 at tree `22e95de9`, `biome check .` via `bun run check`.
Tool summary: **4 errors** (the organizeImports assists) + **9 warnings**
(the cognitive-complexity lints) = 13 findings.

```
src/features/authoring/components/EditorPane.tsx:50:17 lint/complexity/noExcessiveCognitiveComplexity   (16, max 15)
src/features/authoring/components/ShipRunPanel.tsx:55:18 lint/complexity/noExcessiveCognitiveComplexity  (21, max 15)
src/features/authoring/lib/canvas-layout.ts:104:17 lint/complexity/noExcessiveCognitiveComplexity        (16, max 15)
src/features/authoring/lib/facade.ts:127:18 lint/complexity/noExcessiveCognitiveComplexity               (19, max 15)
src/features/transcript/lib/transcriptFoldBuffer.ts:125:11 lint/complexity/noExcessiveCognitiveComplexity (20, max 15)
src/features/workflow-detail/lib/incrementalTimeline.ts:44:10 lint/complexity/noExcessiveCognitiveComplexity (21, max 15)
src/features/workflow-detail/lib/timeline.ts:171:10 lint/complexity/noExcessiveCognitiveComplexity
src/features/workflow-detail/swimlane/laneTree.ts:84:12 lint/complexity/noExcessiveCognitiveComplexity
src/features/workflow-detail/swimlane/timeLayout.ts:190:17 lint/complexity/noExcessiveCognitiveComplexity
src/features/failover/FailoverFallback.tsx:9:1 assist/source/organizeImports  FIXABLE
src/features/failover/hooks/useFanOutProgress.ts:9:1 assist/source/organizeImports  FIXABLE
src/features/workflow-detail/hooks/useLiveWorkflowEvents.ts:8:1 assist/source/organizeImports  FIXABLE
src/features/workflow-detail/lib/timeline.ts:32:1 assist/source/organizeImports  FIXABLE
```

Line/column positions are the capture-time anchors; a finding is identified by
(file, rule) with position as a locator, so unrelated line drift does not
manufacture a delta. `timeline.ts` appears twice — two findings, one file.

## Correction recorded

The original #131 filing said "2 organizeImports, +2 unclear". The named
capture shows **4** organizeImports — the filing's count was a partial
enumeration, which is precisely the failure mode term 1 exists to prevent.

## ✅ DISCHARGED — 2026-08-01, tree `400a7c39` (task #131 closed)

**`bun run check` is ABSOLUTE GREEN: `Checked 359 files. No fixes applied.`,
exit 0, zero findings.** All 13 baseline findings are cleared — facade.ts
earlier under term 4's trigger, the remaining 12 in this lane.

**The gate is henceforth GREEN-NOT-DELTA.** Term 2 (delta-zero against the
named set) is retired with the debt it existed to bound: with the set empty,
delta-zero and absolute-green are the same statement, and the weaker one
invites re-accumulation. Any leg that reddens `bun run check` clears it
before landing. The ruling text above stands as the record of how a red gate
is handled — enforced, named, owned — not renamed into harmlessness.

How each was cleared (complexity scores as the tool reported them):

| File | Finding | Clearing |
| --- | --- | --- |
| `FailoverFallback.tsx` | organizeImports | `biome check --write --only=assist/source/organizeImports` |
| `hooks/useFanOutProgress.ts` | organizeImports | same |
| `hooks/useLiveWorkflowEvents.ts` | organizeImports | same |
| `lib/timeline.ts:32` | organizeImports | same |
| `EditorPane.tsx` | 16 → clear | extracted `EditorHeader` + `EditorStatusBar` presentational components |
| `ShipRunPanel.tsx` | 21 → clear | extracted `checkDeploysGreen`, `requireConnectedWorker`, `parseRunInput`, `projectDeployedRun`, `errorMessage` from the guided pipeline |
| `lib/canvas-layout.ts` | 16 → clear | extracted `placeGraphNodes` + `edgeGutters` from `layoutEmbedded` |
| `lib/transcriptFoldBuffer.ts` | 20 → clear | split `flushPending` into `mergePendingText` + `mergePendingTools` |
| `lib/incrementalTimeline.ts` | 21 → clear | extracted `diffStep` (one merge-walk position) from `sequenceDifference` |
| `lib/timeline.ts:171` | 16 → clear | replaced the 5-deep nested ternary with an `activityStatus` precedence ladder |
| `swimlane/laneTree.ts` | 30 → clear | split `visit` into `visit` + `visitChildLane`; extracted `noticeRow`, `isChildExpanded`, `resolveChildState` |
| `swimlane/timeLayout.ts` | 23 → clear | extracted `timeSpan`, `steppedSpan`, `markerGeometry`, `spanGeometry`, `attemptOffset` from `positionBar` |

**No suppressions and no threshold change.** Every complexity finding was
cleared by refactor, per term 4: no `// biome-ignore`, and
`noExcessiveCognitiveComplexity` remains `warn` at the default max of 15.

Gates at the discharge tree: `bun run check` exit 0 · `bun test` **613 pass /
0 fail** (609 before, +4 from the #134 pin) · `bun run build` exit 0.
