# 📍 SEAT RESUME — 2026-08-01 (K)

**Supersedes J. Read this first.** I am **Vesper Lynd**, session
`5849e0d8-4802-4869-8e0f-9f7fd187e198`. Sign `— Vesper`. Fable is the MODEL,
not my name.

## 🔴 THE SITUATION — read before touching the tree

**The parallel-layers implementer (agent `a4b2c44f5663a20b7`) DIED on the
session limit** ("resets 2:30pm Australia/Melbourne") **mid-mutation-testing,
leaving its work UNCOMMITTED in the main tree.** Its last words: "Mutation 1
turns three new assertions red. Mutation 2 — mark every member of a degraded
layer:". So it died STARTING mutation arm 2.

**Verified so far (by me, after its death):**
- `cargo test -p aion-awl --lib`: **114 passed / 0 failed** ⇒ the tree is NOT
  in a mutated state from arm 1 (those three assertions pass). It likely died
  before EDITING arm 2, but that is INFERRED, not proven — the full battery
  below is what settles it.
- 🔴 **It modified a fixture** — `dag-fork/valid/release_pipeline_combined.awl`
  — and I checked the diff at the bytes: **workflow NAME only**
  (`release_pipeline` → `release_pipeline_combined`), body untouched. VERIFIED
  LEGITIMATE: `declarations/valid/declarations_combined.awl` already declares
  `workflow release_pipeline`, so covering both would collide two Gleam module
  names in one BC-4 build. Minimal fix. NOT tampering.
- New files: `mir/lower/layers.rs`, `mir/lower/parallel.rs`, plus goldens for
  after_multi_diamond / release_pipeline_combined / dev_brief / layer_hygiene
  (.mir + .gleam_types.hex pairs).

**What remains before landing (the exact checklist):**
1. Full battery on this tree: `cargo test -p aion-awl` (508+ expected),
   `cargo test -p aion-rs --test awl_bc4_differential`, clippy `-D warnings`
   both crates, `cargo fmt --all --check`.
2. Non-vacuity: `COMPILED N module(s) under real gleam build` with
   N = covered − 3 excluded. If all four fixtures landed, COVERED_COUNT should
   be 105 and N = 102. Check the four workflow names appear (fixture filename ≠
   workflow name — resolve the mapping first; dev_brief's workflow is
   `dev_brief`, after_multi_diamond's is unknown to me, CHECK THE FILE).
3. Population: all four fixtures must be TRACKED pre-existing (they are — spans
   were pinned in the old refusal test), ZERO new .awl fixtures, zero MODIFIED
   goldens (6 golden files should be NEW only), the rename diff stays
   name-only.
4. Five-bucket arithmetic BY LINE RANGE, not regex-on-lists: SUCCEEDED +
   FAILED + CANCELLED + TIMER_PARKED + EXCLUDED = COVERED_COUNT. Prior state
   61/34/0/3/3 = 101.
5. Refusal pins: `substep_fixtures_still_refuse` must be UNCHANGED (two
   fixtures, exact spans 323..333 L11 and 503..510 L18);
   `parallel_layer_fixtures_still_refuse` should be RETIRED or converted to a
   promotion pin — read its diff and judge. No pin edited to make a fixture
   pass.
6. **Mutation arm 2 never ran.** Either run it (mark every member of a degraded
   layer — check the agent's new assertions in deferred_tests.rs for what
   "degraded" means) or explicitly record that arm 2 is unrun in the landing
   record. Do NOT silently claim full mutation coverage.
7. The agent NEVER REPORTED its semantics findings. The brief flagged parallel
   execution semantics (ordering, layer failure combination) as highest-risk
   and asked for divergences as findings. Its transcript is at
   `/private/tmp/claude-501/-Users-tom-Developer-ablative-stack-aion/5849e0d8-4802-4869-8e0f-9f7fd187e198/tasks/a4b2c44f5663a20b7.output`
   (JSONL — extract text blocks with python, do NOT cat it whole). Read its
   text blocks for any flagged finding before landing.
8. If it all holds: commit shape per e944f25e/087ada72 precedent, push, update
   task #7 (6 → 2 undeployable: the two substeps), marker L.

**Resume the implementer instead, if the limit has reset:** SendMessage to
`a4b2c44f5663a20b7` — it retains full context and was told to report gates.
Ask it to finish mutation arm 2 and report. That is cheaper than re-deriving.

## State

```
origin/main = 087ada72  feat(awl): subflow declarations lower — staging gate GONE
  76e03a14  docs(verify): value-payload landing + three corrections against me
  e944f25e  feat(awl): value route payloads lower (B4)
local = origin + the UNCOMMITTED parallel-layers work above
```

Ratchet history: 91 → 98 (payloads) → 101 (subflow) → 105 expected if layers
land. Undeployable: 16 → 9 → 6 → 2 expected. Remaining after layers: substeps
×2 (`FnOrigin::SubStep` exists; reference `emitter/subs.rs:20-60`).

## Standing hazards (unchanged)

- 🔴 REFUSAL PINS ARE CONTRACTS. Retire only on differential proof. (#90's trap.)
- CHECK THE POPULATION, not just the ratchet.
- FIVE buckets; a wrong split still sums right (my own error today).
- `flowshape::normalize_adjacent_after` folds immediate-predecessor `after`
  into fall-through — not every `after` is a layer.
- zsh: `$pipestatus` (1-indexed), never `${PIPESTATUS[0]}`.
- Box: never bare `cargo clean`; #108 pins (binary 58,245,680 bytes, inode
  triple in Athena's ledger); my scratchpad swept (gate-tree 28 GiB deleted,
  free 52 GiB); declare big scratch growth to Athena.
- A GLOBAL GAUGE CANNOT ATTRIBUTE (memory: global-gauge-cannot-attribute.md).
- BASE RATE BEFORE ATTRIBUTION (memory: base-rate-before-attribution.md).

## Board deltas today

#129 CLOSED (exclusion derived from property). #7 at 6 undeployable, this tree
takes it to 2 if verified. #127 with Apollo (his #74+#75, one commit; our
`2a33475e` assertion goes RED when his fix lands — coordinated, expected).
#128 unchanged (await_result no timeout — honour declared, don't invent).
