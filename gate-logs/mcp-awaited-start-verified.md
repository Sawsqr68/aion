# The MCP awaited start, driven rather than read

**2026-08-19 07:21Z.** Tom asked, after talking to Waffles, whether the task
system could be used to get updates and be notified when a workflow finishes.

**It is already built and already live on his server.** The finding is not that
it exists — it is that it works end to end on the RUNNING binary, and that one
part of it is not durable.

## Driven against the live server, not read from the branch

The source I read is HEAD of `investigate-workflow`; Tom's server runs the
binary deployed at 06:19Z. *Build venue is not the branch you are standing on* —
so every claim below was taken from the process holding the port, over HTTP.

```
POST /mcp                              → -32602 "params._meta is required"
```

That answer is the protocol layer, not a router miss. **The obvious negative
control does not work here**: `GET /definitely-not-here` returns **200**, because
the ops-console SPA catch-all serves `index.html` for unknown paths. A 404 test
would have proved nothing. The JSON-RPC error body is what discriminates.

`tools/list` on the running binary — **13 tools**:

```
describe_run  read_transcript  read_history  list_runs  query
list_documents  read_document  check_document  save_document  deploy_document
start_run  <-- carries `await_completion`
signal  cancel
```

## The end-to-end proof

Deployed a two-second `blink` workflow, then:

```
tools/call start_run {workflow_type:"blink", await_completion:true}
  → resultType "task"
    taskId 0fa78cf9-…  status "working"  ttlMs 3600000  pollIntervalMs 1000

tasks/get {taskId}
  → status "completed"
    {"workflow_id":"205a6c3f-…","run_id":"e8c1bdcb-…",
     "workflow_type":"blink","status":"Completed","awaited":true}
```

Header discipline is enforced and mirrors the body: `MCP-Protocol-Version`,
`Mcp-Method`, and `Mcp-Name` (which mirrors `params.name` for `tools/call` and
`params.taskId` for `tasks/*`). Each was refused until sent correctly — the gate
is real, not decorative.

## What the code gets right

- **The server decides, not the client.** `awaits_completion` is the server's
  task decision in the extension's sense; a client cannot demand a task-shaped
  response for work that finishes instantly, nor refuse one for work that does
  not.
- **`await_terminal` has no iteration cap and no deadline of its own** — and
  says why: a durable run may legitimately take three months, and a cap there
  would be a lie about when the answer arrives. The only bounds are the task's
  own (`tasks/cancel`, TTL), which keeps exactly one authority over the wait and
  puts it in the operator's configuration.
- **No `tasks/list`,** deliberately, so a server cannot leak one caller's tasks
  to another.
- `ContinuedAsNew` is reported as terminal for THAT run, not smoothed over.

## 🔴 The gap worth fixing: the ticket is not durable

`TaskStore` is `Arc<Mutex<HashMap<String, Entry>>>` — **in memory**. A server
restart drops every task, while the workflows they track carry on perfectly.

In an engine whose entire thesis is that a workflow survives its process, the
handle to that workflow does not. It is recoverable — `start_run` returns the
`workflow_id` before the await begins, so a client that lost its task can call
`describe_run` — but the recovery is the caller's to write, and nothing tells
them they need it.

This is the same shape as the respawner's spawn-attribution lie: a supervisory
layer that looks authoritative and is thinner than the thing it supervises.
*The safety layer is the one nobody audits.*

Secondary, and a design question rather than a defect: **an awaited task reports
only the terminal state.** There is no progress channel — no "step 3 of 10" —
so "get updates" in Tom's question is served by polling `describe_run`, a
separate call. `status_message` and the `input_required` status exist in the
model and are unused by the awaited start.

Default TTL is 3,600,000 ms (one hour); `task_ttl_ms = 0` means unlimited and is
a real operator choice for a months-long run. Not the default, because a task
nobody polls holds an executor for as long as it lives.

## Left on Tom's box

`blink` is deployed in `default` (one completed run,
`205a6c3f-2bd5-4f28-8834-71c101856c38`). Inert — nothing starts it. Told him it
is there and offered to remove it rather than leaving it for him to find.
