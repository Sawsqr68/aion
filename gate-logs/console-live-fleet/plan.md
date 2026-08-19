# FLEET PLAN — BRIEF-CONSOLE-LIVE

**Planner:** Waffles (planner seat), 2026-08-17.
**Tree:** `/Users/tom/Developer/ablative/stack/aion/.worktrees/console-live`, branch `console-live`.
**Base commit:** `cf4510f2e`.
**Legs:** L1 and L2, parallel, isolated worktrees, own branches, merged mechanically.

---

## 0. THE FINDING THAT RESHAPES THE SPLIT

The brief proposes the natural cut as **R1 (children) vs R2/R3/R4 (describe-live)**. I measured
the tree and that cut is **wrong**, because R1 has no source work left in it. Both halves of the
brief's table reproduce — the *artefact* finding is correct — but the *cause* differs between the
two rows, and only one of them is a build task.

### 0.1 Census, re-derived at this tree (2026-08-17)

Instrument: `/usr/bin/grep -o <literal> <file> | wc -l`, summed over **every** `*.js` chunk in
`crates/aion-server/ops-console-embed/assets/`, not just the one `index.html` names. Bundle path
chosen by reading the reference out of the served `index.html`:

```
$ grep -o 'src="[^"]*\.js"' crates/aion-server/ops-console-embed/index.html
src="/assets/index-CMn6AElm.js"
```

`index-CMn6AElm.js` is 1,024,025 bytes — the brief's number, so this is the same artefact the
brief measured. **I widened the denominator to all nine JS chunks** because vite code-splits and a
single-chunk grep could have produced a false zero. It did not: the zeros hold everywhere.

| endpoint | router anchor | refs, main chunk | refs, ALL 9 chunks |
|---|---|---|---|
| `workflows/list` | — | 1 | **1** |
| `workflows/describe` | `router.rs:378` | 3 | **3** |
| **`workflows/describe-live`** | **`router.rs:379`** | **0** | **0** |
| **`workflows/children`** | **`router.rs:380`** | **0** | **0** |
| `workflows/history` | `router.rs:381` | 6 | **6** |
| `workflows/transcripts` | `router.rs:386` | 5 | **5** |

Positive control (canary), same instrument, same files: `workflow` → **373** in the main chunk,
**678** across all nine. The instrument is live against these bytes.

This is the **BEFORE** capture. It is recorded here so the AFTER has something to be compared to.

### 0.2 R1 is a STALE EMBED, not missing code

The children client, its hook, its section component, and its mount **all already exist in console
source and are wired end to end**:

- `apps/aion-ops-console/src/lib/api/children-read.ts:13` — `CHILDREN_ENDPOINT = '/workflows/children'`
- `apps/aion-ops-console/src/lib/api/children-read.ts:33` — `ChildrenReadClient.listChildren`, full typed row normalisation, `ApiError` on every malformed field
- `apps/aion-ops-console/src/lib/config/clients.ts:46` — `createConfiguredChildrenReader`
- `apps/aion-ops-console/src/features/workflow-detail/hooks/useChildren.ts:29` — the react-query hook
- `apps/aion-ops-console/src/features/workflow-detail/components/LegsSection.tsx` — `LiveLegsSection`
- `apps/aion-ops-console/src/features/workflow-detail/swimlane/WorkflowDetailView.tsx:38` — `useChildren({...})`
- `apps/aion-ops-console/src/features/workflow-detail/swimlane/WorkflowDetailView.tsx:72` — `<LiveLegsSection legs={children.data} namespace={namespace} />`
- reachable from the router: `src/app/routes.tsx:124` renders `WorkflowDetailView` for `/workflows/:id`
- tests exist: `src/lib/api/children-read.test.ts`, `src/features/workflow-detail/hooks/useChildren.test.ts`, `src/features/workflow-detail/__tests__/legs.test.tsx`

Why the bundle shows zero anyway — the dates:

```
last commit touching crates/aion-server/ops-console-embed/ : af65e69f5  2026-08-15 12:27:35 +1000
last commit touching apps/aion-ops-console/src/            : 709754ec4  2026-08-16 05:18:23 +1000
children-read.ts history: f64fd01d9 2026-08-16 | b0214c0ef 2026-08-15 | 89f0e0e85 2026-08-15
```

The committed embed **predates the children work**. R1's entire delivery is therefore *regenerate
the embed*. There is no client to write, no hook to write, no component to write.

### 0.3 R2/R3/R4 is genuinely unbuilt

Zero references to the endpoint path anywhere in console source:

```
$ grep -rn "describe-live|describe_live|DescribeLive" apps/aion-ops-console/src/
src/types/generated/index.ts:1651:export type DescribeLiveResponse = {
```

One hit, and it is the ts-rs **type**, not a call. So: the response vocabulary is already exported
to TypeScript (§3.1), and nothing consumes it. This half is real work.

### 0.4 Consequence for the split

If I cut R1-vs-R2/R3/R4 as the brief suggests:

- L1 would contain **no source edits at all** — only a regenerated embed.
- That embed would be built from a tree **without L2's describe-live UI**, so keeping it at merge
  reintroduces the exact stale-embed defect being fixed.
- And L2 must regenerate too, so both legs write the same ~24 content-hashed asset files.

**So I am not making that cut.** The cut below splits the *describe-live feature* by layer, and
lifts the embed regeneration out of both legs entirely (§2).

---

## 1. THE SPLIT

One feature, two layers, disjoint file sets, a seam I specify verbatim in §4 so that **neither leg
depends on the other leg's output** — both depend on this plan.

| | **L1 — the live-read data layer** | **L2 — the live-step operator surface** |
|---|---|---|
| owns | `src/lib/api/**`, `src/lib/config/**` | `src/features/workflow-detail/**` |
| delivers | typed `DescribeLiveReadClient` + configured factory + endpoint pin | live-step card, liveness disclosure incl. `NoLiveOwner`, capability-gated controls, poll cadence |
| requirements | R2/R3/R4 wire contract; R3's `tail` **plumbing** | R2, R3 (chooses the window number), R4, R5 |
| brief | `.fleet/legs/L1.md` | `.fleet/legs/L2.md` |
| report | `.fleet/reports/L1.md` | `.fleet/reports/L2.md` |

### 1.1 File ownership — EXACT, and disjoint

**L1 owns and may modify only:**
```
apps/aion-ops-console/src/lib/api/describe-live-read.ts        (new)
apps/aion-ops-console/src/lib/api/describe-live-read.test.ts   (new)
apps/aion-ops-console/src/lib/api/index.ts                     (edit: add exports)
apps/aion-ops-console/src/lib/api/client-transport.ts          (edit: pin endpoint + method)
apps/aion-ops-console/src/lib/config/clients.ts                (edit: add factory)
apps/aion-ops-console/src/lib/config/index.ts                  (edit: re-export factory)
.fleet/reports/L1.md                                           (new)
```

**L2 owns and may modify only:**
```
apps/aion-ops-console/src/features/workflow-detail/lib/live-step-port.ts        (new)
apps/aion-ops-console/src/features/workflow-detail/hooks/useLiveStep.ts         (new)
apps/aion-ops-console/src/features/workflow-detail/components/LiveStepCard.tsx  (new)
apps/aion-ops-console/src/features/workflow-detail/__tests__/live-step.test.tsx (new)
apps/aion-ops-console/src/features/workflow-detail/swimlane/WorkflowDetailView.tsx (edit: mount)
apps/aion-ops-console/src/features/workflow-detail/index.ts                     (edit: exports)
.fleet/reports/L2.md                                                            (new)
```

Intersection: **empty**. Neither leg touches `crates/`, `xtask/`, `src/types/generated/`, or any
file the other names. A merge conflict on any of these paths is a finding about this plan, and the
dispatcher should report it as such rather than resolving it by hand.

### 1.2 The API-client-layer question, answered explicitly

The dispatch instruction asks me to say outright whether the design forces both legs into the
console's API client layer. **It does, and I have assigned that layer to exactly ONE leg: L1.**
L2 writes no HTTP code, constructs no client, and imports nothing from `src/lib/api/`. L2 consumes
the shape through a locally-owned port type (§4) that it declares itself from the text in this
plan. That is the same injection pattern the tree already uses —
`useChildren.ts:16` takes `reader?: Pick<ChildrenReadClient, 'listChildren'>`, and
`legs.test.tsx:35` drives `LiveLegsSection` through an injected `readers` prop — so L2 is not
inventing a mechanism, it is following the house one.

---

## 2. THE EMBED IS POST-MERGE, ONCE — AND WHY BOTH LEGS ARE FORBIDDEN TO BUILD IT

### 2.1 How it regenerates

Single sanctioned path, `xtask/src/main.rs:62` (`build-ops-console`), which calls
`build_ops_console_dist` at `xtask/src/main.rs:151`:

```
cargo xtask build-ops-console
```

Three steps, in order (`xtask/src/main.rs:151-200`):
1. `cargo test -p aion-core export_ops_console_wire_types` with `AION_UPDATE_BINDINGS=1` — regenerates `src/types/generated/`.
2. `bun install` then `bun run build` in `apps/aion-ops-console` → `dist/`.
3. `sync_embed(dist → crates/aion-server/ops-console-embed)`; the embed dir is **wiped** except dotfiles and repopulated, so no stale asset lingers (`xtask/src/main.rs:69-72`).

The freshness guard is `cargo xtask verify-ops-console` (`xtask/src/main.rs:91`): rebuilds to a
scratch dir and asserts byte-for-byte equality with the committed bundle.

### 2.2 There is no CI performing it

`.github/` **does not exist in this tree at all** — not an empty `workflows/` directory, no
directory:

```
$ ls -la .github/
ls: .github/: No such file or directory
```

Note for the record: `crates/aion-core/src/generated_types.rs:101`,
`docs/evidence/python-worker-sdk-wire-death.md:322` and
`docs/design/aion-authoring/MEASUREMENT-LAWS-2026-08-05.md:512` all refer to a
`.github/workflows/ops-console-embed.yml` that regenerates and asserts. **That file is not
present here.** Those references are stale documentation, and nothing automated will rebuild this
bundle. Whoever does not run the xtask by hand ships the stale embed.

### 2.3 Why NEITHER leg runs it

Vite emits **content-hashed filenames** (`index-CMn6AElm.js`, `index-C8Kr-xA5.js`, …). Two legs
regenerating in isolated worktrees produce two different hash sets over the same ~24 files plus
two different `index.html` script refs. That is not a conflict git can merge — it is a
whole-directory collision on generated binary-ish output, and any resolution is a coin toss
between two bundles, each of which is missing the other leg's source.

Worse, it is the **defect being fixed**: a bundle built from a partial tree is a stale bundle.
The gate-log at `gate-logs/battery-0150/embed.json:5` records exactly this failure shape already
(a dist/embed diff listing eight `Only in …` asset mismatches).

**Therefore:** both legs are forbidden to run `cargo xtask build-ops-console`, forbidden to touch
`crates/aion-server/ops-console-embed/**`, and forbidden to touch `src/types/generated/**`. Each
leg proves its own work at source level with `bun test` + `bun run typecheck` + `biome check`.
The embed is regenerated **once, by the dispatcher, on the merged tree**, per §5.

A hand-edited asset remains an automatic reject; under this plan no leg has any reason to be
near one.

---

## 3. GROUNDING — the vocabulary both legs must reuse

### 3.1 The response types ALREADY EXIST in TypeScript. Do not hand-write them.

The ts-rs pipeline has already exported the entire `DescribeLiveResponse` tree into the console:

| type | Rust anchor | TS anchor (`apps/aion-ops-console/src/types/generated/index.ts`) |
|---|---|---|
| `DescribeLiveResponse` | `crates/aion-core/src/describe_live.rs:41` | `:1651` |
| `OpenStep` | `:70` | `:1508` |
| `StepState` | `:89` | `:1494` |
| `CurrentStep` | `:115` | `:1606` |
| `LiveAttempt` | `:130` | `:1584` |
| `AttemptLiveness` | `:150` | `:1535` |
| `HeartbeatNote` | `:180` | `:1541` |
| `TranscriptTail` | `:213` | `:1560` |
| `TranscriptStreamHead` | `:238` | `:1621` |
| `InterventionCapabilities` | — | `:1478` |
| `InterventionPrimitive` | — | `:1400` |

`src/types/index.ts:42` re-exports from `./generated`, so both legs import from `@/types`.
**Minting a second vocabulary for any of these is a reject.** Neither leg regenerates them either
(§2.3) — they are already correct at this commit.

### 3.2 The children vocabulary — reuse, do not re-mint

The CLI has a live, working client:
- `crates/aion-cli/src/tail.rs:114` — `struct ChildrenRequest<'a>`
- `crates/aion-cli/src/tail.rs:84` — `struct ChildrenResponse`
- `crates/aion-cli/src/tail.rs:173-182` — the `POST /workflows/children` call
- server side: `crates/aion-server/src/api/http/children.rs:52` / `:63`

The console's `ChildrenReadClient` (`src/lib/api/children-read.ts`) already speaks exactly this
wire shape — snake_case `workflow_id` / `run_id`, `children` array. **This is the reference shape
for L1's new client too**: same transport helpers, same normalisation discipline, same error
class. L1 copies the *pattern*, not the payload.

### 3.3 The request shape L1 must send

`crates/aion-server/src/api/http/describe_live.rs:47-65`:

```rust
pub(crate) struct DescribeLiveRequest {
    pub namespace: String,
    pub workflow_id: String,
    #[serde(default)] pub run_id: Option<String>,
    #[serde(default)] pub tail: Option<u32>,
}
```

The `tail` doc comment at `describe_live.rs:57-64` is load-bearing and both legs must respect it:

> Omitted returns `TranscriptTail::NotRequested` — no tail is read and nothing is claimed about
> the stream. **There is deliberately no default window**: how much transcript a caller wants is
> the caller's decision, and a number invented here would be a cost every caller paid without
> asking.

So: **L1's client must not invent a default `tail`.** Omitted in, omitted on the wire.
**L2 chooses the number**, states it and its reason in `.fleet/reports/L2.md`, and makes it
adjustable (R3).

### 3.4 The existing capability-gating precedent (R4)

Do not build a second gating mechanism. The tree already gates controls on advertised
capabilities:
- `src/features/transcript/components/InterventionControls.tsx:73,96` — takes `AttemptCapabilities`
- `src/features/transcript/hooks/useIntervene.ts:57` — `apiClient.intervene(params, { namespace })`
- `src/features/capabilities/context/CapabilitiesContext.tsx:16-23` — the house rule, stated in its own doc comment: affordances gate on the resolved runtime value, **never** a build-time flag, and a `null` value is treated as UNPRIVILEGED so a slow server cannot momentarily expose an affordance.

L2 applies that same rule to `AttemptLiveness::Live { capabilities }`.

---

## 4. THE SEAM — verbatim, authored here, copied identically into both briefs

Neither leg may alter this text. L1 implements a client that satisfies it. L2 declares it in
`src/features/workflow-detail/lib/live-step-port.ts` and codes against it. Because both copies
come from this plan, neither leg is waiting on the other.

```ts
import type { DescribeLiveResponse, Namespace, RunId, WorkflowId } from '@/types';

/** How much transcript tail to request. Omitted = the server reads none. */
export type LiveStepTail = number | undefined;

/** The one call the live-step surface needs. L1 implements it; L2 consumes it. */
export type LiveStepReader = {
  describeLive(
    namespace: Namespace,
    workflowId: WorkflowId,
    runId: RunId | null,
    tail: LiveStepTail,
    signal?: AbortSignal
  ): Promise<DescribeLiveResponse>;
};
```

L1's `DescribeLiveReadClient.describeLive` must be assignable to `LiveStepReader['describeLive']`
— identical parameter order, identical nullability, identical return.

### 4.1 The single integration line (dispatcher applies at merge)

L2 leaves `useLiveStep`'s reader as a **required injected parameter** and, in
`WorkflowDetailView.tsx`, supplies it from a clearly marked seam:

```ts
// FLEET-SEAM(L1): replaced at merge with createConfiguredLiveStepReader({ namespace })
```

At merge the dispatcher makes exactly this edit in `WorkflowDetailView.tsx` (L2's own file, so no
conflict):

```ts
import { createConfiguredLiveStepReader } from '@/lib/config';
...
const liveStep = useLiveStep({ namespace, workflowId, runId, reader: createConfiguredLiveStepReader({ namespace }) });
```

This is a **pre-specified 2-line edit**, not a conflict resolution. It is the only hand edit the
merge is permitted.

---

## 5. MERGE AND FIELD PROOF — the dispatcher, on the merged tree, once

Nothing in this section belongs to a leg.

1. Merge L1 and L2 branches. Expect **zero** conflicts (§1.1). Any conflict → report as a
   planning finding, do not hand-resolve.
2. Apply the §4.1 integration edit — the only permitted hand edit.
3. `cd apps/aion-ops-console && bun run typecheck && bun test && bunx biome check .`
4. **Regenerate the embed, once:** `cargo xtask build-ops-console` from the repo root
   (no `CARGO_TARGET_DIR` override — estate law). Commit
   `crates/aion-server/ops-console-embed/` and, if the ts-rs step rewrote them,
   `apps/aion-ops-console/src/types/generated/`. Expected: generated types unchanged, since no
   Rust type changed in either leg.
5. `cargo xtask verify-ops-console` — must pass, proving the committed bundle matches its source.
6. **Re-derive the census (§0.1) against the NEW bundle.** Read the bundle path out of
   `crates/aion-server/ops-console-embed/index.html`; do not guess from filenames. Same
   instrument, all `*.js` chunks, and carry the `workflow` canary. Required AFTER state:
   `workflows/children` **> 0** and `workflows/describe-live` **> 0**, with the other four rows
   still non-zero (nothing regressed). Record BEFORE and AFTER side by side.
7. **R6 field proof, in a browser, on a real multi-leg run.** `aion server` must be started
   **from the repo root** — the store is CWD-relative `./aion-data`, and a server started
   elsewhere shows an empty `aion versions`. Screenshots showing, at minimum: (a) a parent run
   listing its child legs; (b) a current step named — activity id / type / queue / state and
   attempt number — **while the run is still running**; (c) the view settling correctly when the
   run reaches terminal, with polling stopped. A passing unit test is explicitly not this proof.
8. Standing gates at final HEAD, single instance:
   `cargo clippy --workspace --all-targets -- -D warnings` **and** the per-crate
   `-p aion-server` run (the `--workspace` vs `-p` feature-resolution difference means both are
   required), plus `cargo fmt --check`. Formatting: run `cargo fmt` over the tree rather than
   checking.

---

## 6. STANDING WALLS (both legs, restated in each brief)

- **Worktree only. NEVER push.** Landing is the dispatcher's re-gate.
- **Commit only inside your own branch**, and only files your leg owns per §1.1.
- **The embed is off limits** — do not run `cargo xtask build-ops-console`, do not touch
  `crates/aion-server/ops-console-embed/**`, do not touch `src/types/generated/**`. A hand-edited
  asset is an automatic reject.
- No new dotfolders, no daemons, no `CARGO_TARGET_DIR` override.
- No arbitrary limits, no assumed defaults. Caller-chosen numbers (poll cadence, transcript
  window) are **stated with their reason** in the leg report, never silently invented.
- No silent failures: every error surfaced to the operator or thrown as a typed `ApiError`.
  A swallowed rejection is a reject.
- No god files (500 LOC excl. tests/comments), `mod.rs`/barrels stay thin.
- **Your report states what is NOT done as plainly as what is.**

---

## 7. WHAT THIS PLAN DOES NOT DO

Stated plainly, so no one reads completion into it:

- **No server work.** Both endpoints are built, mounted, and tested
  (`describe_live.rs:72` with `describe_live_tests.rs`, `children.rs` with `children_tests.rs`).
- **No R1 source work**, because there is none left (§0.2). R1 is discharged by the post-merge
  embed regeneration, and its proof is census row `workflows/children > 0` plus the browser
  screenshot of legs listed. If the AFTER census shows children still at 0, the diagnosis in §0.2
  is wrong and the finding must be reopened — not patched around.
- **No new intervention primitives.** R4 is gating only: L2 shows or withholds controls that
  already exist. Wiring a control the console cannot already perform is out of scope.
- **No transcript streaming rework.** `describe-live` carries a bounded tail; the live socket path
  (`transcript-stream.ts`) is untouched.
- The field proof (R6) and both gate runs are **dispatcher work**, not leg work. No leg may
  declare R6 satisfied.
