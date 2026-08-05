# 📍 SEAT RESUME — 2026-08-05 (AA)

**Supersedes Z.** I am **Vesper Lynd**, session
`5849e0d8-4802-4869-8e0f-9f7fd187e198`. Sign `— Vesper`. Fable is the MODEL,
not my name. Working brief: `DOC-COMB-2026-08-05.md`.

## ✅ TOM'S ORDER IS DELIVERED AND HE HAS BEEN TOLD

The guided path landed at `6fb20072` and `LLM-STARTUP.md` is deleted. Tom was
messaged on Meridian (`dm:c9255b2a-5731-4d17-8124-e3bfa2224186`) with the
plain-language version. **Do not re-announce it.**

## 🔴 THE ONE THING IN FLIGHT: agent C is UNCOMMITTED and HALF-REVIEWED

Four files sit modified in the tree, **not staged, not committed**:

| File | My review state |
|---|---|
| `docs/workers/SHELL-WORKER-MANIFEST.md` | ✅ **diff read in full, approved** |
| `docs/workers/WORKER-AUTHORING.md` | ✅ **diff read in full, approved** |
| `docs/workers/AGENT-WORKERS.md` | ❌ **DIFF UNREAD** (260 lines) |
| `docs/workers/activities-and-workers.md` | ❌ **DIFF UNREAD** (357 lines) |

**Do not commit the two unread ones until you have read their diffs.** That is
the standing rule (`opus-subagent-work-needs-fable-verification`). The two I
read are good and precise.

### What I already verified at my own hand (C is 3/3 on its hardest claims)

- **`timeout_ms` is consumed by nothing.** Grepped `crates/*/src/` — only
  `query_timeout_ms` / `review_timeout_ms` / `drain_timeout_ms`, all unrelated.
  Agent B independently reached the same conclusion. A declared per-activity
  `timeout` does not bound anything.
- **The spawn-failure asymmetry is real, and C's REFUTATION was right.**
  `crates/aion-cli/src/worker_shell.rs:226` → `ActivityFailure::retryable`
  (manifest shell worker). `crates/aion-worker/src/shell/action.rs:239`
  `spawn_failure` → terminal in every arm (declared body). Two different code
  paths; the original finding conflated them.
- 🔴 **OPERATOR BLOCKER, confirmed.** `aion worker shell` gets the endpoint
  **raw**: `main.rs:510` passes `cli.endpoint()` straight through, while the
  client verbs go via `normalize_endpoint` (`main.rs:598, 615`).
  `DEFAULT_ENDPOINT` is the scheme-less `127.0.0.1:50051` (`main.rs:44`), so a
  worker started without `--endpoint http://…` never connects — **silently**,
  because the verb installs no tracing subscriber. C proved it with a paired
  treatment/control against the live server: `connected_workers` **0** vs **1**.
- **All links and anchors in all four of C's files resolve** (scripted, file
  existence + heading-anchor derivation).

### 🔴 STILL TO DO IN THE SAME LANDING — a falsehood in MY OWN file

C caught it and is right. `docs/workers/DECLARED-COMMANDS.md`, in the
`THERE IS NO SANDBOX` list, says:

> no resource limit — no CPU, memory, file-descriptor, or wall-clock cap
> **beyond the activity's own timeout and cancellation.**

Both escape clauses are false for a declared body, and **the file contradicts
itself two sections later**, where I wrote `### Cancellation of a declared body
is NOT wired`. Since `timeout_ms` is read by nothing either, a declared
`run "…"` has **no external bound at all** — which strengthens the doc's own
"bound it in the command" advice. Fix it in the same commit as C's files so the
two accounts cannot diverge again.

## Landed and pushed this session (main), in order

`65abd1f4` · `d555e669` (#140 closed) · `64dcdbfa` · `e939cc43`
(`{workspace_root}`) · `f34e45fd` (CONTRIBUTING) · `43d24388` (CHANGELOG) ·
`b4c9ae14` (Y) · `7fc88d08` (**agent A**, 11/11 verified) · `6fb20072`
(**guided path + LLM-STARTUP deleted**) · `f962665c` (**agent B**, 8/8
verified) · `03870acd` (Z). **All pushed.**

## After C lands

1. **The estate-facing write-up of the vacuous-green law.** Waffles asked for
   it. Memory `a-treatment-that-can-become-its-own-control` has the content;
   the write-up does not exist. Nobody holding it. I twice started and twice
   yielded it to higher-priority agent landings.
2. **Task #141** — wire-with-oracle sweep. LANE, no date. My constraint,
   adopted estate-wide: *the oracle must not read both sides from the caller.*

## Code defects the comb surfaced — NOT doc fixes, unowned

- 🔴 **The scheme-less endpoint blocker above** — `aion worker shell` silently
  never connects. The likely fix is routing the worker verbs through
  `normalize_endpoint` too, plus installing a tracing subscriber so the redial
  driver's warnings are visible. **Needs a ruling, not a unilateral patch.**
- `aion list --status paused` works, but the clap help
  (`crates/aion-cli/src/main.rs:407`) and the parser's own error message
  (`crates/aion-cli/src/payload.rs:43`) both omit `paused`.
- `crates/aion-cli/src/main.rs:213` cites `docs/guides/codegen.md`; no such
  directory. It is `docs/authoring/codegen.md`.
- `crates/aion-server/src/api/http/router.rs:89` says the CORS layer allows
  "GET, POST, and OPTIONS preflight"; line 112 also allows `PUT`.
- `CLAUDE.md`'s crate list omits `aion-integration-acp`, a real workspace
  member (21 `crates/*` members; the list names 20). Tom's file — flagged only.
- `WORKER-AUTHORING.md:63-64` says the document "pins each action to a node";
  node pinning is optional (`ActionContract.node: Option<String>`). C reported
  it rather than editing outside its brief.

## Live state — unchanged

- 🔴 **WORKER REBUILDS FROZEN ESTATE-WIDE** until the #139 live cut lands.
- **The #139 cut is RATIFIED and waits on Tom's word, which Waffles carries.**
  Sequence: `CUT-PLAN-139-LIVE.md`. Do not build, swap, or deploy until then.
- Server started FROM THE REPO ROOT (store is cwd-relative — see the
  `aion-server-must-start-from-repo-root` memory).
- Manifold dark on a liminal recovery brick — Hermes has it, aion untouched.

## Laws in memory (do not re-derive)

`withdrawal-proven-against-the-artifact` ·
`a-treatment-that-can-become-its-own-control` ·
`refusal-direction-widening-never-blocks` · `commit-messages-via-stdin` ·
`assert-the-invariant-not-the-enumeration` ·
`opus-subagent-work-needs-fable-verification`.
