# MCP task durability — BUILT

**2026-08-19.** Plan: `gate-logs/mcp-task-durability-BUILD-PLAN.md`.
Brief: `docs/design/MCP-TASK-DURABILITY-DECISION-BRIEF-2026-08-19.md`.
Ratified by Waffles the Terrible 2026-08-19T07:31Z.

An MCP task is now **the run, read through the task model**. Nothing is stored
to make that true, so nothing expires, nothing is held, and a task survives a
restart because the run does.

## The spec, read again from the pinned source

Tom asked, mid-build, to be certain of the tasks/subscriptions contract. Read
from `modelcontextprotocol/ext-tasks` @ `2c1425d9…`,
`specification/draft/tasks.md` — not from this repo's paraphrase of it.

- **Three methods only**: `tasks/get`, `tasks/update`, `tasks/cancel`. No
  `tasks/list`, and its absence is why one caller cannot enumerate another's.
- **`tasks/get` = `Result & DetailedTask`**, `resultType: "complete"`, and it
  carries the final `result` inline. There is no separate retrieval method, so
  the implementation's single-read shape is correct.
- **Five statuses**, closed: `working`, `input_required`, `completed`,
  `failed`, `cancelled`. **No `cancel_requested`** — which is what settles the
  cancel design as stateless.
- **`ttlMs` is REQUIRED and nullable**; `null` is the spelling of unlimited.
  "Servers are not required to retain tasks indefinitely" — retaining forever
  is stronger than the obligation, and legal.
- **Notifications are a MAY**: `notifications/tasks`, requested through the
  core `subscriptions/listen`. We implement neither and that is conformant —
  but it is also the piece that would answer "tell me progress, not just the
  ending". Not built; named as the real next piece rather than pretended.

## What was deleted

- `crates/aion-mcp/src/tasks/store.rs` (in-memory `HashMap` + expiry)
- `crates/aion-mcp/src/tasks/runner.rs` (the held executor — the TTL's cause)
- `McpServerConfig.task_ttl_ms`; `DEFAULT_MCP_TASK_TTL_MS`;
  `DEFAULT_MCP_AWAIT_POLL_INTERVAL_MS`; `MCP_AWAIT_POLL_INTERVAL_REQUIRED`
- config keys `mcp.task_ttl_ms` and `mcp.await_poll_interval_ms` — and because
  `[mcp]` is `deny_unknown_fields`, a file that still sets one **refuses the
  boot by name** rather than being ignored. Witnessed by
  `the_retired_mcp_task_knobs_are_refused_by_name`.
- `await_terminal` and its polling loop (`mcp/tools/start.rs`)
- `ServerHandle.tasks` + `tasks()`; `TaskStore` threading through
  `dispatch::route` / `execute_call`
- `ToolService::Caller`'s `Clone` bound — it existed only so a detached
  executor could keep the identity alive; there is no executor now
- `aion-mcp`'s `tokio`, `tokio-util`, `tracing`, `uuid` dependencies, which
  only the deleted modules used

## What was added

- `crates/aion-mcp/src/tasks/resolve.rs` — `TaskProjection`, `TaskState`
  (`Working` | `Finished { tool_name, outcome }`), `TaskResolveError`.
  `into_detailed` renders through **`call_tool_success_result`**, the same
  function the inline path uses, so a task's result cannot drift from what the
  inline call would have returned. `ttl_ms` is always `None`.
- `ToolService::task_handle` + `ToolService::resolve_task` (the latter takes
  the caller).
- `crates/aion-server/src/mcp/task_ref.rs` — the handle
  `run.v1:{workflow_id}:{run_id}:{namespace}` (`splitn(4, ':')`, so a
  namespace containing a colon round-trips), and the resolution through
  `RunView::read_parts`, which already routes through
  `handlers::describe(state.namespace_guard(), caller, …)`.

## Two decisions worth naming

**The projection is RUN-scoped, not chain-scoped.** `WorkflowSummary` folds a
whole continue-as-new chain, so a chain-wide read would leave a task `working`
forever for a run that had already handed over. `resolve_task` projects
`view.run_segment()` instead. Witnessed: generation one of the seeded chain
resolves `completed` carrying `status: "ContinuedAsNew"` while generation two
resolves `working`.

**A task-shaped call that FAILS is answered inline, with no task.** A handle
pointing at a run that never started could never resolve, and would hide the
error that produced it.

## Gates

| Gate | Result |
|---|---|
| `cargo clippy -p aion-mcp -p aion-server --all-targets -- -D warnings` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | see below |
| `cargo test -p aion-mcp` | 60 + 22 + 11 pass, 0 fail |
| `cargo test -p aion-server --lib` | 1162 pass, 0 fail |
| `cargo test -p aion-server --test mcp_surface_e2e` | 45 pass, 0 fail |
| `cargo test -p aion-mcp -p aion-server` (everything) | exit 0 |

All exit codes read WITHOUT a pipe.

### One contention flake, recorded rather than swallowed

Running the lib tests and the e2e binary CONCURRENTLY once produced:

```
---- run::lsub_prod_xnode_e2e::production_boot_dispatches_executes_and_records_over_liminal
Error: "worker never registered in-band for the pool"
```

It passes alone (exit 0) and passes in a sequential `--lib` run (1162/1162,
exit 0), both before and after this change, and it touches no MCP code. So it
is a pre-existing contention sensitivity in that test — a real-worker-pool
registration wait whose green depends on how loaded the box is. Not caused
here, not fixed here, and **named** rather than left for someone to hit in CI
and think this change broke it.

## The two proofs, witnessed rather than asserted

**1. Restart survival** — `a_task_survives_a_server_that_never_saw_it`. A
second `Harness` is built over the SAME store: new engine, new `ServerState`,
new MCP layer that has never seen the task. The handle still resolves, with
the same status and the same `createdAt`. The workflow→namespace registry is
re-registered because it is a test double (a real server rebuilds it from its
own durable state); no task state crosses.

**2. Authorization, on a VALID handle** —
`a_valid_handle_is_refused_to_a_caller_who_does_not_hold_the_namespace`, on the
dev-token auth path. The holder reads the handle successfully FIRST (the
positive control: without it a refusal would only show the handle was bad),
then a caller holding a different namespace is refused `-32602` on all three
task methods, with a message that does not distinguish "not yours" from "no
such task".

**The authorization proof was watched go red.** Mutation: substitute
`CallerIdentity::operator(...)` for `caller` in `task_ref::resolve_task` — the
literal pre-change behaviour, where the task methods took no caller at all.

```
test a_valid_handle_is_refused_to_a_caller_who_does_not_hold_the_namespace ... FAILED
test result: FAILED. 44 passed; 1 failed
```

Restored by writing the original bytes back; the rebuilt tree is green
(45 passed). Note `a_handle_with_a_forged_namespace_does_not_reach_the_run`
stayed GREEN under that mutation — it measures a different property (the run
does not live in the namespace the handle claims), and its doc comment now
says so rather than letting it read as a second authorization proof.

## The live drive — DONE, and it was killed

`gate-logs/mcp-task-durability-LIVE-DRIVE.md`. A real server on its own port and
data dir, identified by SHA-256 through `lsof` rather than by version string,
`SIGKILL`ed twice and restarted each time. One task id, three pids, one
unchanged answer — `working` across the first death, `completed` carrying
`Cancelled` across the second. The status was watched CHANGE (via the `cancel`
tool) so that "always working" was excluded, and three malformed/forged/unknown
handles were each refused `-32602`.

## Still owed before this can land

- 🔴 **Not to be deployed to Tom's engine** on its own. A rebuilt binary rides
  the release line with a battery, same standing rule as the console fix.
