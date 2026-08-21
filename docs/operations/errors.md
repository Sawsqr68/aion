# Errors reference

Aion uses one stable error taxonomy across HTTP, gRPC, and the CLI. Every
API failure carries a wire **code** (a stable string), a human message, and
— where the server can name a fix — a hint.

## How the CLI renders errors

```
error[<code>]: <context>: <server detail message>
  server error type: <type>
  hint: <actionable advice>
```

For example, querying a completed workflow:

```
error[not_running]: failed to query workflow: workflow <id> is not running
```

The `hint:` line appears only when the error class has actionable guidance.
Local (non-API) failures print as a plain `error: <cause chain>`. Exit
codes: `1` for errors, `2` for CLI usage mistakes.

## The error codes

| Code | HTTP | gRPC | Meaning |
|---|---|---|---|
| `not_found` | 404 | `NotFound` | Workflow, run, or deploy version unknown. Also returned for another tenant's workflow — a foreign probe is byte-identical to a miss. |
| `namespace_denied` | 403 | `PermissionDenied` | Caller not authorized for the requested namespace. |
| `invalid_input` | 400/413 | `InvalidArgument` | Malformed body, identifier, or archive; oversized deploy uploads (413) name the violated config key (`deploy.max_archive_bytes` / `deploy.max_inflated_bytes`). |
| `not_running` | 412 | `FailedPrecondition` | The operation needs a live workflow but the run is terminal (completed/failed/cancelled/timed out) or otherwise unable to answer. 412 rather than 409: the request is refused on a precondition about the run's liveness, not on a conflict with its state. |
| `invalid_state` | 409 | `FailedPrecondition` | A precondition on the target's current state failed: the run exists but is in the wrong state for the operation — reopening a run that is not a terminal `Failed`/`Cancelled`, pausing a run that is not `Running`, resuming one that is not `Paused`. The message names the actual status; inspect it with `aion describe <id>`. Distinct from `not_found` (absent) and `backend` (internal failure). |
| `unknown_query` | 400 | `InvalidArgument` | No query handler registered under that name (wrong name, or the workflow has not reached its registration code yet). |
| `query_timeout` | 408 | `DeadlineExceeded` | No handler reply within the server's `runtime.query_timeout_ms` — the workflow is busy between yield points. |
| `query_failed` | 500 | `Internal` | The query handler ran and reported an application-level failure. |
| `sequence_conflict` | 409 | `Aborted` | A durable write lost an optimistic sequence race. `Aborted` is the gRPC class for a lost CAS, so a generic client treats it as retryable — but in Aion exactly one `Recorder` writes per workflow, so seeing this at all indicates a double-writer bug. Report it rather than retrying around it. |
| `lagged` | 429 | `ResourceExhausted` | An event-stream consumer fell behind the bounded channel and was disconnected. Reconnect; per-workflow streams resume with `resume_from_seq`. |
| `deploy_denied` | 403 | `PermissionDenied` | No deploy grant. Supply the `deploy` token claim, or in development the `x-aion-deploy: true` header (the CLI sends it automatically). |
| `version_pinned` | 409 | `FailedPrecondition` | Deploy unload/route refused: the version is route-active or pinned by a live run, a recoverable run, or an in-flight start. The message names the holder. |
| `not_owner` | 409 | `Aborted` | The targeted shard is owned by a different cluster node; the request was fenced. Not a connectivity fault — the server answered. A retryable routing signal: ownership moves as the cluster fails over, so retry (the routing edge re-resolves the owner) or point `--endpoint` at another node. |
| `backend` | 500/503 | `Internal` | Storage, serialization, or internal runtime failure; also deploy mutations and workflow starts refused during drain (503, with an explicit message). |

That is the whole taxonomy — fourteen codes, no more
(`crates/aion-proto/src/error.rs`).

Two common failures carry **no** wire code at all, because nothing builds a
typed error for them. Match on the status, not on a code string:

- **401 Unauthorized** — missing or invalid credentials. The body is empty:
  the request is rejected before any handler runs, so there is no code,
  message, or hint to read (`api/http/auth.rs`).
- **503 Service Unavailable** from `/health` and `/ready` — the probes answer
  with a bare status.

One surface labels its errors differently: every failure from the authoring
routes (`/awl/*`) carries the code `invalid_input` whatever its HTTP status,
and distinguishes itself in `error_type` instead — `DocumentNotFound` with a
404, `AuthoringWorkspaceUnconfigured` with a 503, and so on
(`api/http/awl.rs`). Branch on `error_type` for those routes.

## Terminal-state behavior (what "done" means to each operation)

Once a run reaches a terminal status — `Completed`, `Failed`, `Cancelled`,
`TimedOut`, or `ContinuedAsNew` — it is history, not a live process:

| Operation on a terminal run | Result |
|---|---|
| `aion query <id> <name>` | `error[not_running]` — **expected**, not a bug. Queries are live reads answered by the running workflow; a terminal run answers nothing. Read its recorded state with `describe`. |
| `aion describe <id>` | Works — describe returns bounded summary and terminal facts and works at any history length. |
| `aion list` | The run appears with its terminal status. |
| `aion signal <id> <name>` | Refused — a terminal run rejects signals; nothing is appended after the terminal event. |
| `aion cancel <id>` | No effect — the run already has its terminal event, and status is a projection of history. |
| Event stream (`/events/stream`) | A replayed or live terminal event closes a per-workflow stream at that run boundary. |

`not_running` can also appear on a race: the workflow completed between
your request and its delivery (the server's `error_type` distinguishes
`QueryNotRunning` from `QueryReplyDropped`).

## Quick diagnosis table

| Symptom | Likely cause | Fix |
|---|---|---|
| `error[not_running]` on query | Run already terminal | `aion describe <id>` instead |
| `error[unknown_query]` | Handler not registered yet, or name typo | Check the name; the workflow registers handlers as it reaches them |
| `error[query_timeout]` | Workflow busy between yield points | Wait; raise `runtime.query_timeout_ms` if legitimate |
| `error[deploy_denied]` | No deploy grant | Token `deploy` claim, or dev header (automatic from the CLI) |
| `error[version_pinned]` on unload | Version routed or in use | `aion route` away first; wait for pinning runs to finish |
| `error[not_found]` on deploy route/unload | `(type, hash)` not loaded | `aion versions` to see what is actually loaded |
| 404 on `/deploy/*` / gRPC `Unimplemented` | Deploy surface dark | Set `[deploy] enabled = true` + both size ceilings |
| Workflow stuck after start | No worker serving the activity type | Start a worker registering that type on the right endpoint/queue |
| Activity failed: `worker ... lost before reporting activity result` | The serving worker's stream ended mid-activity (crash, disconnect, drain timeout) | Retryable — the workflow's retry policy decides re-dispatch; make activities idempotent. Long activities are fine: the engine imposes no activity timeout |
| Server refuses to start, names a config key | Required key missing | Set the named key (or its `AION_*` env override) |
