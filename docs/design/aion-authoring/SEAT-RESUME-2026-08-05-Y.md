# 📍 SEAT RESUME — 2026-08-05 (Y)

**Supersedes X. Read this, then `DOC-COMB-2026-08-05.md` (the working brief,
which now carries an append-only PROGRESS LOG at its end).** I am **Vesper
Lynd**, session `5849e0d8-4802-4869-8e0f-9f7fd187e198`. Sign `— Vesper`.
Fable is the MODEL, not my name.

## 🔴 THREE AGENTS WERE IN FLIGHT AT COMPACTION

If their reports have not arrived, they are still running. **Do not duplicate
their work and do not edit their files until they report.** Every one was told:
verify each finding against code before editing, report refutations rather than
"fixing" something already true, do NOT touch git. **Their output gates through
my read of the final bytes before landing.**

| Agent | Files it owns |
|---|---|
| A (root + authoring) | `README.md`, `docs/GETTING-STARTED.md`, `docs/authoring/authoring.md`, `docs/authoring/workflows.md`, `docs/authoring/aion-authoring-dx.md` |
| B (contracts/demo/examples/ops/vision) | `docs/contracts/CANCELLATION-CONTRACT.md`, `docs/contracts/PAUSE-RESUME-DESIGN.md`, `docs/demo/MULTI-HOST.md`, `docs/examples/order-saga.md`, `docs/operations/API.md`, `docs/operations/operations.md`, `docs/vision/ROADMAP.md` |
| C (workers) | `docs/workers/AGENT-WORKERS.md` + `activities-and-workers.md` (REWRITES), `WORKER-AUTHORING.md` + `SHELL-WORKER-MANIFEST.md` (patches) |

The exact findings each was given are in `DOC-COMB-2026-08-05.md`'s verdict
tables. Agent A was additionally told that `CONTRIBUTING.md` now genuinely
carries the gates, so a pointer to it is TRUE and must not be removed.

## Landed and pushed this session (main)

`65abd1f4` errors.md's three status codes + FAILOVER-DEMO's no-op proof
command · `d555e669` **#140 CLOSED — search view withdrawn, embedded bundle
regenerated** · `64dcdbfa` comb progress log · `e939cc43` **`{workspace_root}`
written into DECLARED-COMMANDS.md** + its two false claims corrected ·
`f34e45fd` **CONTRIBUTING.md rewritten** with the real gates · `43d24388`
CHANGELOG Unreleased section + `RELEASE-0.9.2.md` → `docs/archive/`.

Pushed through `d555e669`; **`64dcdbfa`, `e939cc43`, `f34e45fd`, `43d24388`
may still need a push — check `git status` / `git log origin/main..HEAD`.**

## What remains of Tom's order, in order

1. **Land the three agents' work** after my read. Console gates if any console
   file moves; nothing else needs a build.
2. **THE GUIDED PATH — this is the actual ask and it is NOT started.** Spine:
   `README.md` (trimmed front door) → a new opening in `docs/GETTING-STARTED.md`
   covering setup.sh → console → assistant → the existing AWL Part 1 → Gleam
   Part 2 → `docs/README.md` as the onward map. It waits on agent A finishing
   with those two files. **Fold in all six salvaged `LLM-STARTUP.md` items**
   (listed under "What to salvage" in the comb doc): verify-then-proceed and
   never-restart-as-a-reflex; the prerequisite table with norn's honest
   degradation; the symptom→first-look table; the first-assistant-session
   walkthrough; the ~25-minute cold-install warning and the `~/.cargo/bin` PATH
   fix; the going-deeper reading order.
3. **Merge root `GETTING-STARTED.md` into `README.md`** (it is a 43-line
   duplicate of README's "Start here").
4. **DELETE `LLM-STARTUP.md` LAST**, only once all six items have a home.
5. Tell Tom. He asked; I told him the comb was done and the guide was coming.

## Live state — unchanged

- **Server pid 85467**, installed 0.11.0, started FROM THE REPO ROOT (the store
  is cwd-relative — see the `aion-server-must-start-from-repo-root` memory).
  Worker pid 66189. Assistant `6f43c809` route_active.
- 🔴 **WORKER REBUILDS FROZEN ESTATE-WIDE** until the #139 live cut lands.
- **The #139 cut is RATIFIED and waits on Tom's word, which Waffles carries.**
  Sequence: `CUT-PLAN-139-LIVE.md`. Do not build, swap, or deploy until he
  returns with it.
- Manifold is dark on a liminal recovery brick (third occurrence) — Hermes has
  it, aion untouched.

## Open, nobody else holding

- **Task #141 — the wire-with-oracle sweep.** Opened at Waffles' direction as a
  LANE, no date, queued behind this rebuild and the parked cut. My scoping
  constraint, which he adopted estate-wide: *the oracle must not read both sides
  from the caller* — the absent side has to come from the server's own
  registration, or it is a spelling test wearing a wire test's clothes.
- **The estate-facing write-up of the vacuous-green law.** Memory exists
  (`a-treatment-that-can-become-its-own-control`); the write-up does not.

## Laws in memory (do not re-derive)

`withdrawal-proven-against-the-artifact` (NEW — the source is a claim, the
served bundle is the fact; regenerate and prove against the artifact with an
independent control) · `a-treatment-that-can-become-its-own-control` ·
`refusal-direction-widening-never-blocks` (with the 2026-08-05 refinement) ·
`commit-messages-via-stdin`.
