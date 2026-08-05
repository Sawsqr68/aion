# 📍 SEAT RESUME — 2026-08-05 (Z)

**Supersedes Y.** I am **Vesper Lynd**, session
`5849e0d8-4802-4869-8e0f-9f7fd187e198`. Sign `— Vesper`. Fable is the MODEL,
not my name. The working brief is `DOC-COMB-2026-08-05.md`.

## ✅ TOM'S ORDER IS DELIVERED

The guided path is written and `LLM-STARTUP.md` is deleted. `6fb20072`.

`docs/GETTING-STARTED.md` now opens with a three-way choice — **Part 0** one
command (`scripts/setup.sh`, written out in full: each step, success criteria,
self-verification, first assistant session, symptom→first-look table), **Part
1** AWL by hand from published artifacts, **Part 2** Gleam. All six salvaged
`LLM-STARTUP.md` items have a home. Root `GETTING-STARTED.md` merged into
`README.md` and deleted. `docs/README.md`'s Start entry rewritten. Part 2's
steps renumbered 1-8. Every relative link and internal anchor verified to
resolve by script.

**Two LLM-STARTUP claims were corrected rather than carried:**
- it called the repository **private** — it is public; I fetched
  `scripts/setup.sh` unauthenticated and SHA-256-matched it to the local file,
  so the `curl | bash` form is documented as real;
- it said that without norn "everything else still works" — true of Aion,
  **false of the script**, which hard-fails on the norn check in step 1. The
  guide now separates the two and routes a norn-less reader to Part 1.

## Agents A and B: landed after my read. C still out.

| Agent | Commit | Verified at my hand |
|---|---|---|
| A (root + authoring) | `7fc88d08` | 11/11 spot checks against code |
| B (contracts/demo/ops/vision) | `f962665c` | 8/8 spot checks against code |
| C (workers) | **STILL RUNNING** | `docs/workers/AGENT-WORKERS.md` is dirty in the tree — **that is C's, do not commit it as yours** |

Both A and B refuted one finding each by measurement rather than "fixing" a
true statement. The two biggest catches: `workflows.md` taught that
engine-side retry "is not wired up yet" (it is, #197 — an author would have
hand-rolled a loop the engine was already running), and
`CANCELLATION-CONTRACT.md` documented `POST /workflows/{id}/cancel`, which is
not a route (it is body-addressed `POST /workflows/cancel`).

## Landed and pushed this session (main), in order

`65abd1f4` · `d555e669` (#140 closed) · `64dcdbfa` · `e939cc43`
(`{workspace_root}`) · `f34e45fd` (CONTRIBUTING) · `43d24388` (CHANGELOG) ·
`b4c9ae14` (marker Y) · `7fc88d08` (**agent A**) · `6fb20072` (**the guided
path + LLM-STARTUP deleted**) · `f962665c` (**agent B**). **All pushed.**

## What is left

1. **Land agent C** after my read, when it reports. Its files:
   `docs/workers/AGENT-WORKERS.md` + `activities-and-workers.md` (rewrites),
   `WORKER-AUTHORING.md` + `SHELL-WORKER-MANIFEST.md` (patches).
2. **The estate-facing write-up of the vacuous-green law.** Waffles asked;
   memory `a-treatment-that-can-become-its-own-control` exists, the write-up
   does not. I started this tick and yielded it to agent A's landing — nothing
   written yet, nobody holding it.
3. **Task #141** — the wire-with-oracle sweep. LANE, no date. My constraint,
   adopted estate-wide: *the oracle must not read both sides from the caller.*

## Two code defects surfaced by the comb — NOT doc fixes, unowned

- **`aion list --status paused` works but the tool denies it.** The clap help
  (`crates/aion-cli/src/main.rs:407`) and the parser's own error message
  (`crates/aion-cli/src/payload.rs:43`) both omit `paused` from the accepted
  set. One line each.
- **`crates/aion-cli/src/main.rs:213`** cites `docs/guides/codegen.md`; that
  directory does not exist. The file is `docs/authoring/codegen.md`.
- Stale comment: `crates/aion-server/src/api/http/router.rs:89` says the CORS
  layer allows "GET, POST, and OPTIONS preflight"; line 112 also allows `PUT`.

Also noted: **`CLAUDE.md`'s crate list omits `aion-integration-acp`**, which is
a real workspace member (root `Cargo.toml` — 21 `crates/*` members, the list
names 20). Tom's file; flagged, not edited.

## Live state — unchanged

- 🔴 **WORKER REBUILDS FROZEN ESTATE-WIDE** until the #139 live cut lands.
- **The #139 cut is RATIFIED and waits on Tom's word, which Waffles carries.**
  Sequence: `CUT-PLAN-139-LIVE.md`. Do not build, swap, or deploy until then.
- Server started FROM THE REPO ROOT (the store is cwd-relative — see the
  `aion-server-must-start-from-repo-root` memory).
- Manifold dark on a liminal recovery brick — Hermes has it, aion untouched.

## Laws in memory (do not re-derive)

`withdrawal-proven-against-the-artifact` ·
`a-treatment-that-can-become-its-own-control` ·
`refusal-direction-widening-never-blocks` · `commit-messages-via-stdin` ·
`assert-the-invariant-not-the-enumeration`.
