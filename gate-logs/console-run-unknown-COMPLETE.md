# console "run unknown" — COMPLETE, gated, NOT deployed

Diagnosis: `gate-logs/console-run-unknown-diagnosis.md`. Ratified by Waffles as
plan of record. 🔴 **Still not deployed to Tom's box** — it needs a rebuilt
server binary, and no second engine swap goes on his box on the same night as
the first. It takes a queue line and rides the release line with a proper
battery.

## Gates (all green, this tree)

```
apps/aion-ops-console:  bun run typecheck   clean
                        bun test            1039 pass / 0 fail
                        bun run check       exit 0
crates:                 cargo clippy -p aion-core -p aion-store -p aion-server \
                          --all-targets -- -D warnings        exit 0
                        cargo test -p aion-store               77 pass / 0 fail
                        cargo test -p aion-core \
                          export_ops_console_wire_types        pass, bindings in
                                                               tree already match
```

## The defect

The attempt navigator scanned the LOADED history window for `WorkflowStarted` to
work out which run each attempt belonged to, and set `runId = null` when it
found none — honest about the window, catastrophic in practice. The loaded
history is a TAIL window, so on any workflow longer than one window the start
event is never in it and **every row rendered "run unknown"**. A label that
admits ignorance on every row teaches an operator to ignore the column, which is
worse than either a lie or a silence.

## The fix

The server states the boundaries; the console stops deriving them. Costs **no
extra store read** — the describe handler already reads full history, and the
projection reuses the store's own `run_chain_from_history` rather than
re-scanning for `WorkflowStarted`, so there is one implementation of what a
generation is.

### Rust

- `aion-store/src/store.rs` — `RunSummary` gains `started_seq: u64`. The chain
  carried a *timestamp* but never its boundary sequence, and **sequence, not
  timestamp, is the axis that locates an event's generation**.
- `aion-store/src/run_chain.rs` — populated from `envelope.seq`.
- `aion-store/src/conformance.rs` — 6 fixture sites, values taken from each
  fixture's own event seqs, not invented.
- `aion-core/src/describe.rs` — `RunGeneration { seq, run_id }`;
  `DescribeWorkflowResponse.generations`. Re-exported; registered in
  `generated_types.rs`.
- `aion-server/src/api/http/workflows.rs` — projected from `outcome.history`,
  same shape as the existing `unserved`.

### Console

- `lib/attemptNavigator.ts` — **`deriveGenerations` DELETED**.
  `AttemptNavigatorRow.runId` tightened to `RunId`; the `-` key sentinel gone.
  `generationAt` returns `undefined` ONLY for an empty list — a totality answer,
  not a "cannot say".
- `lib/api/client.ts` — `getWorkflowSummary` → **`getWorkflowHead`**, returning
  `{ summary, generations }`. Renamed because it now answers more than a
  summary. `client-normalize.ts` gains a STRICT `normalizeDescribeGenerations`
  (throws on a missing array or malformed entry; never a silent `[]`).
  `WorkflowHead` was missing from `client.ts`'s import AND from both barrels —
  the WIP had never been typechecked.
- `hooks/useWorkflowSummary.ts` → `useWorkflowHead.ts`.
- The `run unknown` render branch, its `data-testid`, its tooltip and the
  `selectedRow.runId === null` banner: **deleted**, not hidden. The state that
  produced them cannot occur.

### 🔴 The design the WIP sheet had wrong

The sheet said: thread `generations` from `WorkflowDetailView` down to the
navigator. **That is a fusion bug.** `AttemptNavigator` is rendered with
`workflowId={selectedWorkflowId}` — the swimlane's SELECTION SCOPE, which may be
a CHILD workflow. The root's boundaries would label a child's attempts with the
parent's runs: the exact fusion the run axis exists to stop, one layer up.

Reading the head inside the navigator was tried and rejected too — it puts
`useQuery`/`useNamespace` into a subtree whose whole convention is
namespace-as-prop with no providers, and it broke nine provider-free SSR tests.
Accommodating a design choice by rewriting deliberate tests is the wrong
direction.

**Settled design: the boundaries travel WITH the timeline they describe, from
the same loader, so they cannot disagree.**

- `ChildTimelineState` gains `generations` beside `entries` — one state, so a
  reader holding one without the other is unrepresentable.
- `ChildTimelineLoader` reads the CHILD's head for the CHILD's workflow id
  (memoised on the query's data identity — a fresh `[]` per render would make
  the reporting effect re-set parent state forever).
- `SwimlaneSelection` and `SelectionSurface` carry `generations`, resolved from
  the same source as `timeline` in the same breath.
- `LegsSection` gets a `useGenerations` reader on the injectable
  `LiveLegReaders`, for the same reason — a leg is a child with its own history.

`laneTree.test.tsx` asserts it directly: a child-scoped selection resolves to
`CHILD_GENERATIONS` and **`not.toBe(ROOT_GENERATIONS)`**.

### Tests replaced, not deleted

Per *narrowing makes downstream tests vacuous*, every assertion that covered the
old `null` was replaced by one that would have FAILED under the old derivation:

- `attemptNavigator.test.ts` — the tail-window case is now a mid-chain window
  with no `WorkflowStarted` anywhere in it, straddling a boundary. Both sides
  resolve, and resolve DIFFERENTLY, which is what proves the boundary is applied
  rather than one run pinned to every row. A separate case covers the empty list.
- `attemptRowList.test.tsx` — two rows, one coordinate, two runs; the retained
  badge lands on one alone, which is only possible because they carry distinct
  keys rather than a shared unresolved one.
- `client.test.ts` — new coverage that a missing/malformed `generations` is a
  typed error, **with the empty list as a control** so the assertions cannot
  pass by the normalizer rejecting everything.

## 🔴 Found on the way: the branch was committed RED

`bun run typecheck` had **three pre-existing errors** and `bun test` **three
failures** on `investigate-workflow` before I touched it. Attribution settled
against the artefact, not assumed — `git log -S` on the generated bindings:

- `ActivityAdoptionOffered` ← `f89b6af7a` (lane #36)
- `DispatchParked` ← `4584a3797` (lane #266)

Both regenerated the Rust→TS bindings and landed without the console's
exhaustive maps catching up. **The maps did their job — they fail closed. The
process never ran them.** Closed here because a red typecheck makes my own green
unmeaningful:

- `timelineVariants.ts` — `ActivityAdoptionOffered` into the `activity` family.
- `timeline.ts` — into the activity lane, with an EXPLICIT arm: it sets no role
  field, because an offer to re-adopt a dangling attempt is not an outcome and
  falling through to the `cancelled` arm would report a live activity as
  cancelled.
- `cluster-stream-protocol.ts` — a `DispatchParked` validator.
- `timeline.test.ts` — the missing fixture.

### 🔴 …and the guard that was supposed to catch it was vacuous

`timelineVariants.ts` carries a compile-time proof that `KNOWN_EVENT_TYPES`
enumerates the whole generated union. It was annotated
`: readonly KnownEventType[]`, which widens `(typeof KNOWN_EVENT_TYPES)[number]`
straight back to `KnownEventType` — making `MissingFromList` unconditionally
`never`. **The proof could not fail.** It passed while the list was missing a
variant; a runtime test caught the drift instead.

Fixed by subtraction: the annotation is gone, the list is
`as const satisfies readonly KnownEventType[]`.

**Calibrated, because a proof I have not watched go red is a proof I have not
tested.** One entry removed from the list, switch untouched:

```
src/features/workflow-detail/lib/timelineVariants.ts(247,7):
  error TS2322: Type 'true' is not assignable to type 'false'.
```

That is the guard line, and it fired on the LIST rather than the switch — the
exact drift claimed. Negative control already observed: the same guard was
silent on the old annotated form while the list was genuinely short. Restored by
writing the bytes back and re-running both gates green on the restored tree.

Also removed: two hardcoded variant counts (`toHaveLength(32)`, a doc comment
saying "29" while the union held 33). A count is a second copy of a generated
fact; it made a missing-handler defect read as a failure about arithmetic. The
duplicate-check that remains is derived, and the gap is proved at compile time
by the guard that now bites.

## Still owed

- A queue line, so this rides the release line with a battery. **Not tonight.**
- Waffles' both-ends rule: I argued it does not bind, because `rust_embed`
  compiles the console INTO the server binary, so no version skew is possible in
  a deployed aion; dev-mode is the only skew surface. Awaiting his ruling. If he
  wants tolerance anyway it goes in `normalizeDescribeGenerations` ONLY — never
  by restoring the window scan.
