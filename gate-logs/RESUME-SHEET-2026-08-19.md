# RESUME SHEET — 2026-08-19, Vesper

Read this first after a compaction.

## Where things stand

**MCP task durability: BUILT AND COMMITTED.** `bb9a98f1b` on
`investigate-workflow`. Full record: `gate-logs/mcp-task-durability-BUILT.md`.
Gates all green (clippy both `--workspace` and `-p`; aion-mcp 93; aion-server
lib 1162; mcp_surface_e2e 45). The authorization proof was watched go red under
a mutation and restored.

## The live drive — DONE 2026-08-19 08:31Z

`gate-logs/mcp-task-durability-LIVE-DRIVE.md`. Two `SIGKILL`s, three pids, one
task id, one unchanged answer; the status was watched change working →
completed; three bad handles each refused `-32602`. Scratch server stopped and
its data dir removed; Tom's box untouched. **Nothing further is owed on the MCP
change except the merge.**

Recipe as it was actually run (kept for the next drive — note the two
required-no-default knobs and the `/private/tmp` path-safety refusal):

1. Scratch dir (NOT the repo root, so Tom's `./aion-data` is untouched), own
   port, own data dir. Config needs `[mcp] enabled = true`.
2. Deploy `examples/canary/canary.awl` — sleeps 30s a round, needs no worker,
   so it stays `Running` long enough to read a `working` task.
3. `tools/call start_run {workflow_type:"canary", await_completion:true}` →
   taskId. Assert `ttlMs: null`.
4. `tasks/get` → `working`.
5. **KILL the server process. Start it again on the same data dir.**
6. `tasks/get` → still resolves, same `createdAt`.
7. Headers required on every MCP request: `MCP-Protocol-Version: 2026-07-28`,
   `Mcp-Method: <method>`, `Mcp-Name: <params.name | params.taskId>`, and
   `params._meta` carrying protocolVersion +
   `clientCapabilities.extensions["io.modelcontextprotocol/tasks"]`.
8. ⚠️ A 404 negative control is USELESS on this server — the SPA catch-all
   serves `index.html` for unknown paths. The JSON-RPC error body is the
   discriminator.

🔴 **Do NOT deploy to Tom's engine.** A rebuilt binary rides the release line
with a battery. `blink` (an inert 2s probe workflow) is still deployed on his
server from an earlier verification; offer to remove it stands.

## 🔴 Branch position — checked, and it is NOT what it looks like

- `origin/main` = `cac631108`, workspace version **0.18.1**, 13 commits past
  the `v0.18.1` tag.
- `investigate-workflow` (mine) = version **0.17.0**, diverged:
  **113 commits mine, 49 theirs.**
- Merge base: `dbcfdf391`.
- Only **ONE** main-side commit touches my edit surface: `52bf4aaf5` (#77,
  "the child wears its parent"), which adds `parent_workflow_id` to
  `Event::WorkflowStarted`. My `mcp_surface_e2e.rs` fixtures construct that
  event literal, so the merge conflict is mechanical, not semantic.

## 🔴 Publishing — REFUSED, and Tom was told why

Tom asked (via meridian) to "publish latest versions to crates, tag releases".
**I did not, and must not from this branch.**

- Every crate is already on crates.io; last published **today ~01:01Z**.
- Publishing from 0.17.0 would push a LOWER version over a newer one, and
  crates.io publishes cannot be undone.
- Correct order: my work merges to main **through Waffles the Terrible**
  (estate law L2), then the release lane cuts the next version with it in.
- Told Waffles my change claims **no queue line and no index position** — fold
  it into the held chain wherever he places it. The box hold is his and I am
  behind it.

## Answered for Waffles (verified from code, not memory)

Freshness checked: only 3 commits touched `worker/`, `aion-worker/`,
`namespace.rs` since my merge base — two release bumps and #77 — so none of
this moved.

- **Concurrency is per worker PROCESS**, `WorkerConfig.max_concurrency`,
  **required with no default** (builder errors; serve loop refuses zero).
  Enforced as a `tokio::Semaphore` in `runtime/loop_.rs`. Not per queue, not
  per activity type. Two workers of 4 = 8 slots; no pool-level ceiling.
- `[namespaces] max_in_flight_activities` (default 1024) is **NOT ENFORCED** —
  its own doc says "config + record field only". Never let it travel as working.
- **Hazard**: an *exhausted* concurrency budget parks the serve loop awaiting a
  permit INSIDE its stream arm, so it reads no further frames and stops
  answering the liveness ping — a saturated worker reads as unreachable while
  working flat out. Measured in `runtime/liveness_answer_tests.rs`.
- **Node affinity is designed and is NOT the task queue.** Pool address is
  `(namespace, task_queue)`; `task_queue` = which FLAVOUR of worker. `node` is
  a within-pool filter at selection (`registry.rs`: "node is a within-pool
  filter, NOT part of the pool key"). Affinity is declared on the NAMESPACE as
  `NamespacePlacement::{Unplaced, Prefer{nodes}, Pinned{nodes}}` — `Prefer` is
  soft with an any-node spill, `Pinned` is hard and waits. `Pinned` is enforced
  at REGISTRATION (a wrongly-labelled worker cannot even sign up), so a
  single-box task queue is a coincidence, not a mechanism.

## Other open items (unchanged)

- Console "run unknown" fix: complete and gated, queue **line 21 claimed and
  pushed** (`ablative-docs` 336ea6d), fires in file order when the box hold
  lifts.
- Canary `499cd0cb` running on Tom's engine, verified ticking.
- Still owed from the #85 arc: `slice_wall` on the unfixed binary;
  `nosleep_wall` re-paired on `probe_walls`; `JitCache::keys()`; the respawner
  spawn-attribution defect.
- `aion awl check` passes what `deploy` refuses (`visits` in an outcome
  payload) — checker gap, repro in scratchpad, filed in
  `gate-logs/canary-and-awl-check-gap.md`.
- Waffles's earlier stand-down pointed at consolidating the bar-stocktake
  direction for the thing-finder Friday demo.
