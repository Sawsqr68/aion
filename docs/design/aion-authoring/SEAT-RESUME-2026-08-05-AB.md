# 📍 SEAT RESUME — 2026-08-05 (AB)

**Supersedes AA.** I am **Vesper Lynd**, session
`5849e0d8-4802-4869-8e0f-9f7fd187e198`. Sign `— Vesper`. Fable is the MODEL,
not my name.

## ✅ THE BOARD IS CLEAR — both open items closed this tick

### 1. Tom's order: DELIVERED and he has been told

The guided path landed at `6fb20072`, `LLM-STARTUP.md` is deleted, Tom was
messaged on Meridian (`dm:c9255b2a-5731-4d17-8124-e3bfa2224186`).
**Do not re-announce it.**

### 2. The doc comb: CLOSED. All three agents landed after my own verification.

| Agent | Commit | Verified at my hand |
|---|---|---|
| A (root + authoring) | `7fc88d08` | 11/11 spot checks against code |
| B (contracts/demo/ops/vision) | `f962665c` | 8/8 |
| C (workers) | `8441fd79` | **25+**, every one confirming |

Agent C's two large rewrites were read in full before landing (that was the
whole reason it sat overnight). Its biggest catches:

- `activities-and-workers.md` taught that a worker's `task_queue` must be the
  NAMESPACE the workflow runs in. They are independent axes. Replaced with the
  whole routing model — namespace × task queue × node — and node called out as
  a within-pool filter that still defaults to the machine hostname.
- It taught `register_activity`, which inserts a handler with NO descriptor. A
  worker registered entirely that way is selectable and inadmissible at once —
  a bug that shipped. Now teaches `register_activity_with_contract` plus a full
  registration-admission section.
- Retry semantics said engine-side re-dispatch "is not consumed yet". Wrong
  since #197.
- It caught a falsehood in a file **I** wrote (`DECLARED-COMMANDS.md`: "no cap
  beyond the activity's own timeout and cancellation" — both escape clauses are
  empty). Fixed in the same commit, and the cancellation section now states the
  `timeout_ms` fact too, so the two accounts cannot diverge again.

### 3. The vacuous-green write-up: WRITTEN, LANDED, SENT

`docs/design/aion-authoring/MEASUREMENT-LAWS-2026-08-05.md` (`d313806f`).
Both laws with their incidents and guards, plus the argument for why Waffles
was right that the second is the better one: Law 1 is a harness lying to
itself and an internal assertion fixes it; Law 2 is the environment lying to a
correct harness, which no internal check can ever detect — the only instrument
is subtraction. DM'd to Waffles.

## Landed and pushed this session (main), in order

`65abd1f4` · `d555e669` (#140) · `64dcdbfa` · `e939cc43` · `f34e45fd` ·
`43d24388` · `b4c9ae14` (Y) · `7fc88d08` (**agent A**) · `6fb20072` (**guided
path + LLM-STARTUP deleted**) · `f962665c` (**agent B**) · `03870acd` (Z) ·
`468275a8` (AA) · `8441fd79` (**agent C + my own file's fix**) · `d313806f`
(**measurement laws**). **All pushed.** Tree carries only the three NO-SWEEP
untracked directories.

## What is next — nothing is mid-flight

1. **Task #141** — the wire-with-oracle sweep. LANE, no date. My constraint,
   adopted estate-wide: *the oracle must not read both sides from the caller.*
2. The unowned code defects below want a ruling, not a unilateral patch.

## Code defects the comb surfaced — NOT doc fixes, unowned

- 🔴 **`aion worker shell` silently never connects.** It takes the endpoint
  RAW (`crates/aion-cli/src/main.rs:510`) while the client verbs go through
  `normalize_endpoint` (`:598`, `:615`). `DEFAULT_ENDPOINT` is the scheme-less
  `127.0.0.1:50051` (`:44`), so a worker started without `--endpoint http://…`
  never connects — and the verb installs no tracing subscriber, so nothing
  says so. Agent C proved it with a paired treatment/control against the live
  server: `connected_workers` **0** vs **1**. Likely fix: route the worker
  verbs through `normalize_endpoint` too, plus a tracing subscriber so the
  redial driver's warnings are visible.
- `aion list --status paused` works, but the clap help (`main.rs:407`) and the
  parser's error message (`payload.rs:43`) both omit `paused`.
- `crates/aion-cli/src/main.rs:213` cites `docs/guides/codegen.md`; the file is
  `docs/authoring/codegen.md`.
- `crates/aion-server/src/api/http/router.rs:89` says the CORS layer allows
  "GET, POST, and OPTIONS preflight"; line 112 also allows `PUT`.
- `CLAUDE.md`'s crate list omits `aion-integration-acp`, a real workspace
  member (21 `crates/*` members; the list names 20). Tom's file — flagged only.

## Live state — unchanged

- 🔴 **WORKER REBUILDS FROZEN ESTATE-WIDE** until the #139 live cut lands.
- **The #139 cut is RATIFIED and waits on Tom's word, which Waffles carries.**
  Sequence: `CUT-PLAN-139-LIVE.md`. Do not build, swap, or deploy until then.
- Server started FROM THE REPO ROOT (store is cwd-relative — see the
  `aion-server-must-start-from-repo-root` memory).
- Manifold dark on a liminal recovery brick — Hermes has it, aion untouched.
- 🔴 beamr JIT blocker with Artemis: keep long workflows off until the fixed
  beamr lands.

## Laws in memory (do not re-derive)

`withdrawal-proven-against-the-artifact` ·
`a-treatment-that-can-become-its-own-control` ·
`refusal-direction-widening-never-blocks` · `commit-messages-via-stdin` ·
`assert-the-invariant-not-the-enumeration` ·
`opus-subagent-work-needs-fable-verification`.
