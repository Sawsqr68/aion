# MCP task durability — BUILD PLAN (resume sheet)

Design brief: `docs/design/MCP-TASK-DURABILITY-DECISION-BRIEF-2026-08-19.md`
(committed 247cfaf9b, cancel note discharged in a later edit).
**RATIFIED by Waffles 2026-08-19T07:31Z. Build approved as briefed.**
Nothing implemented yet at the time of writing — this is the plan, not a record.

> **BUILT 2026-08-19.** The record of what was actually done, including the
> two places the build departed from this plan, is
> `gate-logs/mcp-task-durability-BUILT.md`. Read that first; this file is kept
> for the reasoning behind each decision.
>
> Departures: (1) `status_message` was hoisted out of the `Working` variant
> onto `TaskProjection`, so a settled task can carry prose too; (2) the
> `Finished` state carries the tool name + `ToolOutcome` rather than a rendered
> `Value`, so the MCP layer stays the single renderer and a task's result
> cannot drift from the inline form. `TaskState::Failed` therefore does not
> exist — `failed` is reachable only from a render failure, which is the
> server breaking its own output schema. (3) The projection is RUN-scoped
> rather than chain-scoped; see the BUILT record for why.

## Settled before building (both discharged)

- **Cancel = option (a)**, ruled by Waffles. And his design note is **settled
  STATELESS**, read from the pinned contract itself
  (`modelcontextprotocol/ext-tasks` @ `2c1425d9a288b9b1f489430fe1e00bb392b47e48`,
  `specification/draft/tasks.md`), not from this repo's paraphrase:
  - *"the task's observable status MAY remain `working` … and MAY ultimately
    reach a terminal status other than `cancelled`"*
  - *"Cancellation is cooperative: The request signals intent, and the server
    decides whether and when to honor it."*
  - status set is closed at **five** — `working`, `input_required`, `completed`,
    `failed`, `cancelled`. **No `cancel_requested`/`cancelling` exists**, so the
    extension has nowhere to put a visible requested-state. `TaskStatus` in
    `model.rs` already matches exactly.
  ⇒ acknowledgement alone satisfies the contract. **No stored state, and no
  event written to the run's history.** The second branch of Waffles's note
  (persist in history) does NOT apply.
- **Authorization moves in the SAME change.** Non-negotiable, Waffles: "no
  intermediate state where derived ids exist without the guard."

## Queue

Line 21 claimed and pushed (`ablative-docs` 336ea6d) for the CONSOLE battery.
This MCP work is a separate change and will want its own line when it is ready.

---

## The seam

`aion-mcp` is protocol-only (no `aion-core` dep). Task resolution needs workflow
knowledge, so it goes through the existing host trait `ToolService`
(`crates/aion-mcp/src/tools/service.rs:248`).

### KEEP `tasks/model.rs` entirely
`Task` + `DetailedTask` are the right model. `Task::write_into` already emits
`ttlMs: null` correctly for `None`, and `DetailedTask` already enforces the
per-status payload obligations structurally.

### NEW `crates/aion-mcp/src/tasks/resolve.rs`

```rust
pub struct TaskProjection {
    pub state: TaskState,
    pub created_at: DateTime<Utc>,
    pub last_updated_at: DateTime<Utc>,
}
pub enum TaskState {
    Working { status_message: Option<String> },
    Completed { result: Value },   // a CallToolResult
    Failed { error: Value },       // a JSON-RPC error object
}
impl TaskProjection {
    pub fn into_detailed(self, task_id: String, poll_interval_ms: u64) -> DetailedTask;
    // ttl_ms is ALWAYS None here — there is nothing to expire.
}

pub enum TaskResolveError {
    Malformed { detail: String },   // not a handle this server issues
    Unknown   { task_id: String },  // names no work, or none this caller may see
    Backend   { detail: String },
}
```

`Cancelled` is deliberately absent from `TaskState`: this server never honours a
task cancel, so a task never reaches that status. Document it there.

### EXTEND `ToolService`

```rust
/// Mint the durable handle for a task-shaped call that has just been performed.
fn task_handle(&self, call: &ToolCall, outcome: &ToolOutcome) -> Option<String>;

/// Resolve a handle by reading the durable work it names, under this caller's
/// authorization.
async fn resolve_task(&self, caller: &Self::Caller, task_id: &str)
    -> Result<TaskProjection, TaskResolveError>;
```

### DELETE

- `crates/aion-mcp/src/tasks/store.rs` (the in-memory `HashMap` + expiry)
- `crates/aion-mcp/src/tasks/runner.rs` (the held executor — the TTL's cause)
- `McpServerConfig.task_ttl_ms` + the `task_ttl_ms` ctor param
- `DEFAULT_MCP_TASK_TTL_MS`, `DEFAULT_MCP_AWAIT_POLL_INTERVAL_MS`
  (`crates/aion-server/src/config/defaults.rs:236,240`)
- `mcp.task_ttl_ms`, `mcp.await_poll_interval_ms`
  (`crates/aion-server/src/config/sections.rs` ~538-620; also `load.rs:1012-1040`)
- `await_terminal` + its polling loop (`crates/aion-server/src/mcp/tools/start.rs:159`)
- `ServerHandle.tasks` field + `tasks()` accessor (`server/handle.rs:26,36,48,108`)
- `TaskStore` threading through `dispatch::route` and `tools_methods::execute_call`

`task_poll_interval_ms` STAYS — a client hint, bounds nothing.

## Control flow after the change

**`tools/call` with `task_decision == Task`** (`tools_methods.rs:85-140`):
1. extension gate (unchanged — capability check first)
2. **perform the call INLINE** via `service.call(...)`. The workflow really
   starts and is confirmed before any task exists, so there is never a task
   pointing at a run that failed to start.
3. on `Err(failure)` → return the tool failure **inline, no task**. A task that
   can never resolve is worse than an error.
4. on `Ok(outcome)` → `task_handle(&call, &outcome)`, then **immediately
   `resolve_task`** and build the create-result from that one projection.
   ONE code path answers "what is this task's state", used at creation and at
   every `tasks/get` — no second truth, and no momentary lie for a run that was
   already terminal by the time we minted.
5. `statusMessage` on creation carries the cancel prose (see below).

**`tasks/get`**: `required_task_id` (keeps the extension gate) →
`service.resolve_task(caller, &id).await` → `into_detailed` → envelope.
**Now takes `caller`** — this is the authorization move.

**`tasks/cancel`**: `resolve_task` first (so a bogus id is `-32602`, not a
cheerful ack for nothing), then acknowledge. Nothing else.

**`tasks/update`**: `contains` becomes `resolve_task` (existence +
authorization), then the same acknowledgement it already returns.

### Where the cancel prose lives

`CancelTaskResult` has no message field, so the explanation goes in the
**`statusMessage` set at creation**, which is stateless and visible on every
read:
> "`start_run` is running; cancelling this task does not stop the run — use the
> `cancel` tool for that."
Also reinforce in `instructions()` and `start_run`'s description.

## Host side (`aion-server`)

### Handle format
`run.v1:{workflow_id}:{run_id}:{namespace}`

`splitn(4, ':')` — namespace takes the remainder, so any character in a
namespace is safe. Chosen over base64 deliberately: it is inspectable in logs,
which matters to an operator, and the ids are not secrets (the caller was just
handed them by `start_run`). Opacity was never the security property and must
not be relied on as one — the guard is.

### `resolve_task` implementation
1. parse → `Malformed` on a bad prefix/shape
2. `RunView::read_parts(state, caller, namespace, &workflow_id, Some(&run_id))`
   — this ALREADY routes through `handlers::describe(state.namespace_guard(),
   caller, …)`, so it IS the same guard `describe_run` passes. That is the
   authorization, and it comes for free.
3. project.

### Status mapping — every terminal workflow status → task `completed`

Preserves the runner's existing and correct rule: *a tool-level failure is a
COMPLETED task carrying `isError`, never a `failed` task; `failed` is reserved
for a JSON-RPC error.* The `start_run` contract is "start it and tell me how it
ended" — a run that ended `Failed` is the tool **succeeding** at its job.

| `WorkflowStatus` | task status | notes |
|---|---|---|
| Running, Paused | `working` | |
| Completed / Failed / Cancelled / TimedOut / ContinuedAsNew | `completed` | run's status rides in the structured result |

`failed` therefore arises only from a projection that cannot render — i.e. the
server publishing structured content that violates its own output schema, which
is a server defect. Keep that arm (reuse `call_tool_success_result`'s error
path).

Note `TaskStatus::Cancelled` becomes unreachable. That is honest — this server
never cancels a task — and must be documented rather than left to be noticed.

### Timestamps
- `created_at = summary.started_at`
- `last_updated_at = summary.ended_at.unwrap_or(summary.started_at)`

This is exactly the spec's "last status change": the status changes when the run
starts (→ working) and when it ends (→ completed), and nothing in between
changes it. No history read is needed for it.

### The result payload
Must be byte-identical to what the inline call would have returned, and must
still pass the tool's output-schema validation. Rebuild the `start_run`
structured outcome from the `RunView` and render through
`call_tool_success_result(catalog().find("start_run"), &outcome)`.

## Gates before it can land

- `cargo clippy --workspace --all-targets -- -D warnings` AND
  `-p aion-mcp -p aion-server --all-targets -- -D warnings`
  (⚠ `--workspace` resolves `preserve_order` ON, `-p` off — BOTH runs required)
- `cargo test -p aion-mcp -p aion-server`
- a live drive against a running server repeating
  `gate-logs/mcp-awaited-start-verified.md`'s sequence, PLUS the two new proofs:
  1. **restart survival** — start an awaited task, restart the server, and
     `tasks/get` still resolves. This is the whole point and must be witnessed,
     not asserted.
  2. **authorization** — a caller without the namespace grant gets a refusal
     from `tasks/get` on a valid handle. Without this the change is a leak, and
     an unauthorized-read test that passes because the id was unguessable would
     measure nothing.
- Headers for any live drive: `MCP-Protocol-Version: 2026-07-28`,
  `Mcp-Method: <method>`, `Mcp-Name: <params.name | params.taskId>`, and
  `params._meta` carrying protocolVersion + `clientCapabilities.extensions
  ["io.modelcontextprotocol/tasks"]`.

## 🔴 Not to be deployed to Tom's engine

Same standing rule as the console fix: a rebuilt binary rides the release line
with a battery. `blink` (a 2s probe workflow) is deployed on his server from the
verification run — told him, offered to remove it.
