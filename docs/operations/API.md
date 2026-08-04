# Aion API overview

Aion exposes the engine through the server (`aion server --config aion.toml`, built on the `aion-server` library crate) plus language-specific client and worker SDKs. This document enumerates the full HTTP, gRPC, and WebSocket surface as mounted by `crates/aion-server/src/api/http/router.rs` and `crates/aion-server/src/run.rs`.

## Implementation status

- **Implemented:** engine crash/restart recovery for active workflow histories, durable timer recovery, HTTP/JSON workflow operations (including live workflow queries), gRPC worker/client APIs, and the WebSocket event-stream route described below.
- **In progress:** ops console UX and cross-language SDK/conformance hardening. Prefer the `aion` CLI, HTTP, gRPC, or the Rust client when you need the most exercised surfaces.

## Authentication and caller metadata

HTTP and WebSocket routes use the same caller extraction path, and the gRPC API reads the same names as request metadata:

- In local development with auth disabled (`[auth] enabled = false`, the default), send `x-aion-subject` to identify the caller. If omitted, the server identifies the caller as `anonymous`.
- Send `x-aion-namespaces` as a comma-separated list of namespace grants. The default namespace mode is `SharedEngine`, in which a request is authorized only when its `namespace` value appears in this list — so the dev curl examples below must include the header. Deployments configured with `[namespace] mode = SingleTenant` instead authorize exactly the configured namespace for every caller, without consulting this header.
- **Trust model:** with auth disabled, both headers are taken at face value. Namespace grants are purely caller-asserted — any caller can self-grant any namespace simply by listing it in `x-aion-namespaces`. This is a development convenience, not tenant isolation. Real tenant isolation requires `[auth] enabled = true` with a JWKS endpoint (`[auth] jwks_url`, refreshed every `jwks_refresh_seconds` seconds): the server then validates the `Authorization: Bearer <token>` header against the JWKS keys and derives the caller identity from validated token claims. (A server binary compiled without the `auth` feature falls back, when `auth.enabled` is true, to comparing the bearer token against the configured `jwks_url` value as a static shared secret — also development-only.) WebSocket upgrades use the same headers as HTTP requests.
- The `aion` CLI follows the same model over gRPC metadata: it asserts exactly its `--namespace` flag value (default `default`) as its single namespace grant and sends `--subject` (default `cli-user`) as the caller.

Most examples use the default `default` namespace.

## Route families and gating

The HTTP router mounts several route families, some of which are dark by default:

| Family | Gate | When off |
|---|---|---|
| Identity, namespaces, workflows, events, schedules, operator routes | Always mounted | — |
| `/deploy/*` and `/awl/deployed*` | `[deploy] enabled = true` | Plain 404 |
| `/authoring/compile` | `[authoring] gleam_path` set | Plain 404 |
| `/awl/*` studio (except `/awl/deployed*`) | Always mounted; document/layout routes additionally need `[authoring] workspace_dir` | 503 `AuthoringWorkspaceUnconfigured` for document routes without a workspace |
| `/dev/*` | `[dev] enabled = true` | Plain 404 |
| `/metrics` | Metrics installed in server state | Not mounted |
| `/health/live`, `/health/ready` | Health endpoints installed | Not mounted |
| Ops console (`/` + static assets + SPA fallback) | Always embedded | — |

> **Probe the body, never the status.** The ops-console single-page app is served with an SPA fallback: any path no API route matches answers `200 text/html` with the console's `index.html`. A probe that reads only the HTTP status will report every endpoint "present" on every build — check the response body (or content type) instead. This is also why `GET /build` exists: it returns build identity JSON so an operator can tell which code a server is actually running.

## Identity and namespaces

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/whoami` | Reflect the resolved caller for runtime capability discovery: `subject`, `auth_enabled`, `deploy_granted`, `all_namespaces`, and the sorted `namespaces` grant list. The ops console fetches this to gate affordances. |
| `GET` | `/build` | Build identity JSON — what code this server is. Authenticated like every data route (a deployment's revision is not a public fact). Probe the body, not the status (see above). |
| `GET` | `/namespaces` | Namespaces visible to the caller, sorted and deduplicated. Unauthorized names are dropped before the response is built (the existence-leak boundary: a caller never learns a namespace it cannot access exists). |
| `POST` | `/namespaces` | Idempotently mint/register a namespace the caller is authorized for. The durable upsert reports whether the record was `created` or already existed. |
| `GET` | `/namespaces/records` | Full namespace registry records (created / last-seen / origin columns), grant-filtered on the same existence-leak boundary, sorted by `created_at` then name. Additive alongside the string-list `GET /namespaces`. |
| `PUT` | `/namespaces/{name}/placement` | Idempotent quorum value-CAS on an already-minted namespace's `placement` field; emits a placement-changed delta on the deploy-scoped cluster socket. A namespace with no registry row is a 404 — this endpoint never mints. |

## Workflow operations

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/workflows` | Visibility list. Query parameters: `namespace` (required), optional `workflow_type`, `status`, `started_after`, `started_before`, `closed_after`, `closed_before`, `search_attributes`, `limit`, `offset`. Returns a JSON array of `WorkflowSummary` values. |
| `GET` | `/workflows/count` | Same query parameters as `/workflows`; returns `{ "count": <number> }`. |
| `POST` | `/workflows/start` | Start a workflow. Body: `namespace`, `workflow_type`, optional `input` (an ordinary JSON value or a `ProtoPayload` envelope with `content_type` and byte-array `bytes`). Returns `workflow_id` + `run_id`. |
| `POST` | `/workflows/signal` | Deliver a named signal. Body: `namespace`, `workflow_id`, optional `run_id`, `signal_name`, optional `payload`. Empty acknowledgement object. |
| `POST` | `/workflows/query` | Synchronous live query into the running workflow. Body: `namespace`, `workflow_id`, optional `run_id`, `query_name`. Result or typed error — see [Query semantics](#query-semantics). |
| `POST` | `/workflows/cancel` | Request cancellation. Body: `namespace`, `workflow_id`, optional `run_id`, `reason`. Empty acknowledgement object. |
| `POST` | `/workflows/reopen` | Reopen a terminal run (run resolved to latest when omitted); returns the reopened `run_id` and its projected `Running` status. The terminal-reopenable precondition is the engine's: a non-reopenable run answers `invalid_state`, an absent workflow `not_found`. |
| `POST` | `/workflows/list` | Raw wire-form visibility list (`ProtoListWorkflowsRequest` with optional encoded `filter`); returns `{"summaries":[...]}` with serde-encoded envelopes. Prefer `GET /workflows` for human-readable output. |
| `POST` | `/workflows/describe` | Workflow summary plus optional history (`include_history`). See [List and describe response shapes](#list-and-describe-response-shapes). |
| `POST` | `/workflows/history` | One ascending projected history window. Body: `namespace`, `workflow_id`, optional `from_seq` (default 0), `limit` (clamped to 1..2000), `payload_limit_bytes` (default 2048 — payloads over the limit are truncated in the projection). Returns `events`, `next_from_seq` (cursor for the next window, `null` at the head), and `head_seq`. |
| `POST` | `/workflows/event` | One full, untruncated history event. Body: `namespace`, `workflow_id`, `seq`. |
| `POST` | `/workflows/intervene` | Route an intervention command at a live activity attempt. `issued_by`/`issued_at` are stamped by the server from the authenticated caller — never accepted from the client — so transcript attribution cannot be forged. Applied / capability-not-supported / stale-target outcomes are all `200 OK` acks; only authorization or routing/lock faults are HTTP errors. |
| `POST` | `/workflows/attempts` | Enumerate a workflow's live intervenable activity attempts with each owning worker's advertised intervention capabilities (the console renders only the primitives in `capabilities.supported`). An empty list is the honest answer for a workflow with no live agent attempt, not an error. |
| `POST` | `/workflows/transcript` | Read the retained agent-observability transcript tail of one `(workflow, activity, attempt)` stream from `from_seq` (default 0), in `store_seq` order. An unknown or pre-retention stream returns `200` with empty `events`, null `next_from_seq`, and `head_seq == 0`. |
| `POST` | `/workflows/transcripts` | Enumerate a workflow's retained transcript streams, ordered by `(activity, attempt)`. A workflow with no retained transcript answers `200 { "streams": [] }`. |
| `GET` | `/workflows/unrecoverable` | Runs this engine process could not make resident (for example, their pinned code version is gone), oldest observation first, with the remedy stated (redeploy the pinned version first; cancel only if the code is gone for good). Grant-filtered on the existence-leak boundary; unattributed runs are visible only to an all-namespaces operator. An empty list is the healthy answer. |
| `GET` | `/events/stream` | WebSocket upgrade; the first client message selects the subscription. See [WebSocket event streaming](#websocket-event-streaming). |

The `Proto*` JSON field names above are the serde names from `crates/aion-proto`. Prefer the CLI or generated/client SDK types for complex encoded `Payload` and `WireEnvelope` fields.

### Start example

```sh
curl -sS -X POST http://127.0.0.1:8080/workflows/start \
  -H 'content-type: application/json' \
  -H 'x-aion-subject: docs-user' \
  -H 'x-aion-namespaces: default' \
  -d '{"namespace":"default","workflow_type":"hello_world","input":{"name":"Ada"}}'
```

A successful start returns the assigned identifiers as nested objects:

```json
{"workflow_id":{"uuid":"<workflow-id>"},"run_id":{"uuid":"<run-id>"}}
```

(`aion start` flattens this to `{"workflow_id":"<workflow-id>","run_id":"<run-id>"}` in its own output.)

For non-JSON binary payloads, use the envelope form with `bytes` as a JSON array of integers. For the hello-world tutorial, the `aion` CLI is the recommended path because it handles payload encoding for you.

### List and describe response shapes

`GET /workflows` returns a plain JSON array of visibility summaries. Each element carries a top-level `status`:

```json
[
  {
    "workflow_id": "<workflow-id>",
    "run_id": "<run-id>",
    "workflow_type": "hello_world",
    "status": "Completed",
    "start_time": "2026-01-01T00:00:00Z",
    "close_time": "2026-01-01T00:00:01Z",
    "search_attributes": { "aion.namespace": { "type": "String", "data": "default" } }
  }
]
```

`POST /workflows/describe` does **not** return a top-level `status`. It returns a `summary` envelope plus a `history` array of event envelopes; the workflow's projected status lives inside the decoded summary at `summary.payload.data.status`:

```json
{
  "summary": {
    "namespace": "default",
    "request_id": null,
    "payload": {
      "content_type": "application/json",
      "data": {
        "workflow_id": "<workflow-id>",
        "workflow_type": "hello_world",
        "status": "Completed",
        "started_at": "2026-01-01T00:00:00Z",
        "ended_at": "2026-01-01T00:00:01Z",
        "parent": null
      }
    }
  },
  "history": [ { "namespace": "default", "request_id": null, "payload": { "content_type": "application/json", "data": { "...": "event" } } } ]
}
```

`POST /workflows/list` returns the raw wire form — `{"summaries":[...]}` with serde-encoded envelopes whose payload `bytes` are JSON byte arrays. Prefer `GET /workflows` or `aion list` for human-readable output.

### Query semantics

`POST /workflows/query` (and the gRPC `WorkflowService.Query`) is a synchronous, deadline-bounded round trip into the live workflow process: the engine delivers the query to the workflow's registered handler and waits for its reply up to the server-configured deadline. The server **requires** `runtime.query_timeout_ms` in its configuration (env: `AION_RUNTIME_QUERY_TIMEOUT_MS`) — startup fails without it, because the query route is always mounted.

Query-semantic failures ride the `QueryResponse.error` oneof on a successful transport call (HTTP 200 / gRPC OK):

| `error.code` | Meaning | Typical cause |
|---|---|---|
| `unknown_query` | The workflow has no registered handler for the requested query name. | Wrong query name, or the workflow has not reached its registration code yet. |
| `query_timeout` | No handler reply arrived before the configured `runtime.query_timeout_ms`. | The workflow is busy between yield points, or parked in a non-yielding (blocking) call. |
| `not_running` | The workflow cannot answer a live query: it is terminal, suspended, or ended before answering (`error_type` distinguishes `QueryNotRunning` from `QueryReplyDropped`). | Querying a completed/failed/cancelled workflow, or racing its completion. |
| `query_failed` | The workflow's query handler ran and reported an application-level failure. | The handler raised or replied with an error. |

Namespace denial, unknown workflow ids, and backend faults remain transport-level errors with the usual status codes (403/404/500), exactly as for every other operation — a query against another tenant's workflow is byte-identical to one against a workflow that never existed.

## Schedules

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/schedules?namespace=...` | List schedules in the namespace (`ProtoListSchedulesResponse`). |
| `POST` | `/schedules` | Create a schedule (`ProtoCreateScheduleRequest`: `namespace`, encoded schedule `config`). Returns `201 Created` with `ProtoCreateScheduleResponse`. |
| `GET` | `/schedules/{id}?namespace=...` | Describe one schedule (`ProtoDescribeScheduleResponse`). |
| `PUT` | `/schedules/{id}` | Update; the path id is used as the target schedule id (`ProtoUpdateScheduleRequest`). |
| `DELETE` | `/schedules/{id}?namespace=...` | Delete the schedule. Empty acknowledgement object. |
| `POST` | `/schedules/{id}/pause?namespace=...` | Pause the schedule (`ProtoPauseScheduleResponse`). |
| `POST` | `/schedules/{id}/resume?namespace=...` | Resume the schedule (`ProtoResumeScheduleResponse`). |

## Operator and cluster routes

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/cluster/command` | Deploy-gated operator command seam (same `deploy_denied` 403 shape as the deploy API). Two commands do real work over HTTP: the read-only `RequestClusterSnapshot`, and `RedriveOutboxRow`, which returns one dead-lettered outbox row to the pending claim path in eligible-only mode (a row whose failure was already judged by its workflow is refused — redriving it would re-execute a possibly non-idempotent activity behind recorded history). Other `ClusterCommand` variants are refused on this transport. |
| `POST` | `/outbox/dead-letters` | List one workflow's dead-lettered outbox rows, ordered by ordinal, as an operator sees them. |
| `GET` | `/queues/unserved` | Addresses nobody is serving, with everything parked on them. Grant-filtered on the same existence-leak boundary as `GET /namespaces`; an empty list is the healthy answer. |
| `POST` | `/workers/{worker_id}/drain` | Fence one worker from assignment and ask its transport to drain gracefully. |
| `POST` | `/workers/{worker_id}/stop` | Drain one worker, then force worker-loss teardown at the configured deadline. An absent worker is a typed 404. |

## Operator deploy API

> **Operator surface, not a caller operation.** Deploy is deliberately outside the caller SDK contract (`CLIENT-CONTRACT.md`): client SDKs SHALL NOT expose it. The wire surface is a separate gRPC `DeployService` (`crates/aion-proto-generated/proto/deploy.proto`) plus the `/deploy/*` HTTP routes below, driven by `aion deploy`/`versions`/`route`/`unload`.

The deploy surface is **dark by default**. It mounts only when commissioned in server config:

```toml
[deploy]
enabled = true                 # default false: routes not mounted (HTTP 404, gRPC Unimplemented)
max_archive_bytes = 16777216   # default 64 MiB when enabled — override to size for your packages
max_inflated_bytes = 67108864  # default 256 MiB when enabled — when set, must be >= max_archive_bytes
```

`max_archive_bytes` (env override `AION_DEPLOY_MAX_ARCHIVE_BYTES`; `AION_DEPLOY_ENABLED` gates the mount) is enforced while reading the upload on both transports; oversized archives are refused with `413` / `InvalidArgument` naming the key.

`max_inflated_bytes` (env override `AION_DEPLOY_MAX_INFLATED_BYTES`) caps the total decompressed size of an uploaded archive's contents: `max_archive_bytes` bounds only the compressed upload, and a DEFLATE bomb under that cap can inflate ~1000:1. Extraction charges every inflated byte (manifest included) against this budget and refuses with the same `413` / `InvalidArgument` class, naming the key. Startup validation rejects a value below `max_archive_bytes` — an inflate ceiling under the upload ceiling would refuse archives the upload ceiling admits even stored uncompressed.

**Authorization** is a deployment-wide `deploy` grant, decided before any handler logic runs:

- JWT path (`[auth] enabled = true` with the `auth` feature): a boolean `deploy` claim in the same bearer token. Absent claim = no grant; existing data-operation tokens keep working unchanged.
- Development paths: the `x-aion-deploy: true` header/metadata entry, the dev analog of the claim (the `aion` CLI sends it automatically). The dev-token fallback checks the shared secret first, then the header.
- Denials are `403` / `PermissionDenied` with the dedicated `deploy_denied` wire code, and the message names the knob that carries the grant (header vs token claim).

The grant is engine-global on purpose: **a package load is engine-global**. Loading registers code into the shared VM and re-points routing for a workflow *type* that is startable from every namespace — there is no namespace field anywhere on the deploy surface, and no per-namespace isolation is implied.

| Method | Path | Body | Behavior |
|---|---|---|---|
| `POST` | `/deploy/packages` | raw `application/octet-stream` `.aion` archive | Load + atomic route flip. Response: `workflow_type`, `content_hash`, `deployed_entry_module`, `entry_function`, `freshly_loaded`, `route_changed`. Idempotent: re-POSTing a resident archive succeeds with `freshly_loaded = false`; `route_changed` reports whether routing moved (re-deploy after rollback). A deploy pipeline may retry blindly. |
| `GET` | `/deploy/versions` | — | The deploy read model: every loaded version with `route_active`, sorted `(type, loaded_at)`. Keeps serving during drain. |
| `POST` | `/deploy/route` | JSON `{"workflow_type", "content_hash"}` | Atomic, idempotent re-point (rollback / roll-forward) to an already-loaded version. |
| `POST` | `/deploy/unload` | JSON `{"workflow_type", "content_hash"}` | Unload a non-routed version after the engine verifies nothing pins it. |

Failure taxonomy (both transports; messages pass the engine's refusal prose through):

| Condition | Wire code | HTTP | gRPC |
|---|---|---|---|
| No deploy grant | `deploy_denied` | 403 | `PermissionDenied` |
| Version route-active or pinned (live run, in-flight start, recoverable run, recorded child) | `version_pinned` | 409 | `FailedPrecondition` |
| Malformed archive / hash mismatch / collision / same-hash-different-manifest | `invalid_input` | 400 | `InvalidArgument` |
| Unknown `(workflow_type, content_hash)` | `not_found` | 404 | `NotFound` |
| Archive exceeds `deploy.max_archive_bytes` | `invalid_input` (names the key) | 413 | `InvalidArgument` |
| Archive contents inflate past `deploy.max_inflated_bytes` | `invalid_input` (names the key) | 413 | `InvalidArgument` |
| Draining / shutting down (mutations only) | `backend` with explicit message | 503 | `Unavailable` |
| Deploy surface disabled | — (not mounted) | 404 | `Unimplemented` |

Every mutation emits one structured audit log line (`operation`, `subject`, `grant_source`, `transport`, `workflow_type`, `content_hash`, `outcome`, and `freshly_loaded`/`route_changed` for loads); denials log at `warn`. Metrics: `aion_deploy_operations_total{operation, outcome}`, `aion_deploy_denied_total{transport}`, and the `aion_loaded_workflow_versions{workflow_type}` gauge.

CLI (`--token` overrides the `AION_TOKEN` environment variable; without either, the development headers apply):

```bash
aion --endpoint 127.0.0.1:50051 --token "$TOKEN" deploy dist/order.aion
aion versions --workflow-type order
aion route order <content-hash>     # rollback / roll-forward
aion unload order <content-hash>
```

See [docs/packaging.md](../authoring/packaging.md) for building the `.aion` archives this API consumes.

## Server-side authoring

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/authoring/compile` | Server-side Gleam compile, type-check, package, and load of authored workflow source. Mounted only when `[authoring] gleam_path` is configured; otherwise every `/authoring/*` path is a plain 404 and the server deploys pre-built `.aion` files only. A type error is a `400` carrying the verbatim gleam diagnostics inline (`error_type: "TypeError"`); drain/shutdown is `503`. |

## AWL studio

The AWL studio surface (`crates/aion-server/src/api/http/awl.rs`) backs the visual authoring console. It is always mounted. Authorization is two-tier: **reads** require an authenticated caller (a `namespace_denied` 403 otherwise), **mutations** (document/layout writes, create, deploy, run binding) additionally require the deployment-wide deploy grant — the same guard as the deploy API. Document and layout routes operate on the configured `[authoring] workspace_dir`; without one they answer `503 AuthoringWorkspaceUnconfigured`.

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/awl/check` | Check AWL source in the workspace; returns diagnostics. |
| `POST` | `/awl/edit` | Apply a structured edit operation to AWL source and return the edited source. |
| `POST` | `/awl/fmt` | Canonically format AWL source; an unformattable document is a `422` with the diagnostic. |
| `POST` | `/awl/scaffold` | Scaffold new AWL source from a template request. |
| `POST` | `/awl/emit` | Compile/emit an AWL document through the run loop (the studio's compile step). |
| `POST` | `/awl/deploy` | Compile and deploy an authored AWL document from the workspace (deploy-gated via the run loop). |
| `GET` | `/awl/revisions/{hash}` | Fetch one archived authoring revision by content hash. |
| `GET` | `/awl/runs/{deployment_id}` | Run status for a studio deployment. |
| `POST` | `/awl/runs/{deployment_id}/binding` | Bind a studio deployment record to its started `workflow_id`/`run_id` (deploy-gated). |
| `POST` | `/awl/workers/availability` | Connected-worker availability for a `namespace` + `task_queue` (namespace-authorized). |
| `GET` | `/awl/documents` | List AWL documents in the authoring workspace. |
| `POST` | `/awl/documents` | Create a workspace document (deploy-gated); `201 Created`, `409 DocumentExists` on collision. |
| `GET` | `/awl/documents/{*path}` | Read one workspace document. |
| `PUT` | `/awl/documents/{*path}` | Write one workspace document (deploy-gated). |
| `GET` | `/awl/layout/{*path}` | Read the caller's canvas layout record for a document (keyed by caller subject). |
| `PUT` | `/awl/layout/{*path}` | Write the caller's canvas layout record (deploy-gated). |
| `GET` | `/awl/deployed` | Every loaded workflow version with the honest state of its archived AWL source. Deploy-guarded and follows `[deploy] enabled` (plain 404 when off) — it reads the same deployed catalog `GET /deploy/versions` serves. |
| `GET` | `/awl/deployed/{workflow_type}/{content_hash}` | The archived AWL document of one deployed version, with its schema files and projection. Same gate as `/awl/deployed`. |

## Dev surface

Mounted only when `[dev] enabled = true`; otherwise every `/dev/*` path is a plain 404 and the engine runs the bare production dispatcher with no activity-mock decorator.

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/dev/runs` | Trigger a run and return the existing-firehose subscription for it. |
| `POST` | `/dev/mocks` | Install an opt-in per-run activity mock. |
| `POST` | `/dev/replay` | Re-drive a failed run through the real engine. |

## Observability

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/metrics` | Prometheus metrics text (`text/plain; version=0.0.4`), when metrics are enabled. |
| `GET` | `/health/live` | Liveness, when health endpoints are installed. Unauthenticated. |
| `GET` | `/health/ready` | Readiness, when health endpoints are installed. Unauthenticated. |

## gRPC services

The gRPC server (same process, `aion server`) mounts three services. Authoritative contracts live in [`crates/aion-proto-generated/proto/`](../../crates/aion-proto-generated/proto/); caller metadata follows the same names as the HTTP headers.

### `WorkflowService` (`workflow.proto`)

| RPC | Purpose |
|---|---|
| `StartWorkflow` | Start a workflow run; returns `workflow_id` + `run_id`. |
| `Signal` | Deliver a named signal with an optional payload. |
| `Query` | Synchronous live query — same semantics and error oneof as `POST /workflows/query`. |
| `Cancel` | Request cancellation with a reason. |
| `Reopen` | Reopen a terminal run; returns the reopened `run_id` and projected `Running` status. |
| `Pause` | Hold new activity dispatch for a live `Running` run. |
| `Resume` | Release the dispatch hold on a `Paused` run. |
| `ListWorkflows` | Visibility list with an optional encoded filter. |
| `CountWorkflows` | Visibility count for the same filter shape. |
| `DescribeWorkflow` | Workflow summary plus optional history. |
| `CreateSchedule` | Create a schedule from an encoded config. |
| `UpdateSchedule` | Update a schedule. |
| `PauseSchedule` | Pause a schedule by id. |
| `ResumeSchedule` | Resume a schedule by id. |
| `DeleteSchedule` | Delete a schedule by id. |
| `ListSchedules` | List schedules in a namespace. |
| `DescribeSchedule` | Describe one schedule by id. |
| `MintNamespace` | Intra-cluster: mint (or gate) already-authorized namespaces on **this** node. A namespace's registry record hashes to its own shard, so a non-owning node forwards the mint here; the receiving node applies its ordinary auto-create policy — a forwarded mint is exactly as privileged as a local one, never a policy bypass. |

### `DeployService` (`deploy.proto`)

Mounted only when `[deploy] enabled = true`; otherwise every RPC answers `Unimplemented`. Same deploy-grant authorization and failure taxonomy as the HTTP deploy routes.

| RPC | Purpose |
|---|---|
| `LoadPackage` | Upload a `.aion` archive: load + atomic route flip (mirror of `POST /deploy/packages`). |
| `ListVersions` | The deploy read model (mirror of `GET /deploy/versions`). |
| `RouteVersion` | Atomic, idempotent re-point to an already-loaded version (mirror of `POST /deploy/route`). |
| `UnloadVersion` | Unload a non-routed, unpinned version (mirror of `POST /deploy/unload`). |

### `WorkerProtocol` (`worker.proto`)

| RPC | Purpose |
|---|---|
| `StreamWorker` | The single bidirectional worker stream. The worker sends `RegisterWorker` first; the server answers with `RegisterAck` (assigned worker id, authorized namespace, heartbeat window) as the guaranteed first frame before dispatching any task. The worker then sends `ActivityResult` and `Heartbeat` frames while the server pushes `ActivityTask` (with a one-based delivery `attempt`), `ResultAck` (only that ack clears a worker's re-report backlog), and `DrainRequest` (finish in-flight work and reconnect without consuming the reconnect drop budget) down the same stream. There is no polling RPC. |

## WebSocket event streaming

Connect to `ws://<server>/events/stream` and then send one JSON text or binary message that selects the subscription. The server ignores ping/pong frames while waiting, but closes the stream with an input error if the subscription is missing or malformed. The socket is one-subscription-per-socket: a client wanting two streams opens two sockets.

Supported subscription messages:

```json
{"per_workflow":{"namespace":"default","workflow_id":{"uuid":"<workflow-id>"}}}
```

```json
{"filtered":{"namespace":"default","workflow_type":"hello_world","status":"Completed"}}
```

```json
{"firehose":{"namespace":"default"}}
```

```json
{"cluster":{"after_seq":0}}
```

```json
{"transcript":{"namespace":"default","workflow_id":{"uuid":"<workflow-id>"},"activity_id":{"uuid":"<activity-id>"},"attempt":1}}
```

The same shapes may also be wrapped as `{ "subscription": { ... } }`. `filtered` accepts optional `workflow_type`, optional `status` (status name or numeric value), and optional `namespace_selector`. `firehose` accepts `namespace` or `namespace_selector`.

### Cluster subscription

The `cluster` arm streams cluster topology/ownership deltas and is **deployment-scoped, not namespace-scoped**: it is authorized by the caller's deploy grant, not a namespace grant (a caller without the grant receives a terminal `namespace_denied` error frame). The server sends one priming `cluster_snapshot` frame (the calm-state baseline) before any live `cluster_event` delta; clients apply only deltas with `cluster_seq > snapshot.as_of_seq`. `after_seq` is the highest `cluster_seq` the client has already applied; `0` requests the full in-flight backlog. Cluster history is non-durable: a long disconnect surfaces `cluster_lagged`, and the client re-requests a snapshot.

### Transcript subscription

The `transcript` arm streams the agent-observability transcript of one `(workflow, activity, attempt)` — two attempts of one activity are distinct streams. It is namespace-scoped exactly like `per_workflow` (authorized by the caller's namespace grant, never the deploy grant). Optional `after_seq` is the highest `store_seq` the client has already applied: the server replays the durable transcript tail with `store_seq > after_seq`, then splices onto the live broadcast with no gap and no duplicate. Absent `after_seq` = a fresh subscriber that must see the full durable transcript from `store_seq == 0`. `POST /workflows/transcript` is the request/response counterpart over plain HTTP.

### Filtered-subscription selector semantics

`filtered` selectors are enforced server-side, before any frame is encoded:

- **`workflow_type`** delivers only events of workflows whose recorded type equals the selector. A workflow's type is its most recent recorded `WorkflowStarted` type (continue-as-new chains record each run's type on that run's `WorkflowStarted`, so the type follows the chain). The type is resolved from durable history via the same per-workflow read that verifies namespace ownership; a workflow whose history records no started run never matches a `workflow_type` selector — absence is not a wildcard.
- **`status`** matches per event kind: each terminal lifecycle event matches exactly its projected status (`WorkflowCompleted` → `Completed`, `WorkflowFailed` → `Failed`, `WorkflowCancelled` → `Cancelled`, `WorkflowTimedOut` → `TimedOut`, `WorkflowContinuedAsNew` → `ContinuedAsNew`), and every non-terminal event — including `WorkflowStarted` — matches `Running`. So `status: "Completed"` is a stream of completion events, and `status: "Running"` is the stream of in-flight activity (activities, timers, signals, starts) without terminal events.
- When both selectors are present they **AND** together: an event is delivered only when its workflow has the selected type *and* the event kind matches the selected status.

Filtered streams remain live-only (see resumption below): selectors choose which live events are delivered, they do not replay history.

The server requires `websocket.event_broadcast_capacity` in its configuration (env: `AION_WEBSOCKET_EVENT_BROADCAST_CAPACITY`) — startup fails without it. It sizes the engine-global live-event broadcast channel; lag is filter-blind, so size it for global event volume across all namespaces.

### Subscription resumption (`resume_from_seq`)

Per-workflow subscriptions accept an optional `resume_from_seq` cursor — the **first** per-workflow sequence number the caller wants (SDKs send `last delivered seq + 1`):

```json
{"per_workflow":{"namespace":"default","workflow_id":{"uuid":"<workflow-id>"},"resume_from_seq":7}}
```

The server replays recorded history events with `seq >= resume_from_seq` in order, then splices into the live stream with no gaps and no duplicates. Sequence numbers start at 1; `resume_from_seq: 1` replays the full history; `head + 1` (one past the recorded head) replays nothing and tails live events only. Absent cursor = live tail from now (previous behaviour). A replayed terminal or `ContinuedAsNew` event closes the stream at that run boundary, exactly like a live one; callers walk continue-as-new chains by resubscribing with the next cursor.

Filtered and firehose subscriptions are **live-only** and accept no cursor: `seq` is per-workflow, so a cross-workflow cursor is structurally unrepresentable. SDKs must surface a non-resumable disconnect on those streams instead of silently reattaching with a gap. (The cluster and transcript arms carry their own `after_seq` cursors, described above.)

Cursor error semantics (the namespace guard verdict always comes first — an unauthorized or foreign workflow probe yields the same `not_found` regardless of cursor, never a cursor error):

| Condition | Error frame |
|---|---|
| Workflow unknown or foreign-owned | `not_found` (identical for both; cursor never inspected) |
| `resume_from_seq: 0` | `invalid_input` (`resume_from_seq must be >= 1`) |
| `resume_from_seq > head + 1` | `invalid_input`, `error_type: "ResumeCursorAheadOfHistory"` |
| Cursor older than earliest retained event (compaction; cannot occur yet) | reserved: `not_found`, `error_type: "HistoryCompacted"` — restart without a cursor |

All stream errors are one terminal `{"error": {"code": ..., "message": ..., "error_type": ...}}` text frame followed by a close frame; any event frames already queued for the connection are delivered before the terminal error frame. A consumer that falls behind the live stream receives a terminal `lagged` error frame; reconnect with `resume_from_seq = last delivered seq + 1` to continue gap-free. Per-workflow streams additionally carry a server-side contiguity tripwire: if a delivered-sequence gap or regression is ever observed (unreachable under normal operation), the stream ends with a terminal `lagged` error frame carrying `error_type: "SequenceContiguityViolation"` instead of silently delivering a gapped stream — recover the same way, by reconnecting with `resume_from_seq = last delivered seq + 1`.

## Recovery and durable timers

Crash/restart recovery and durable timers are implemented in the current build. On server startup the engine replays durable histories for active workflows, reconciles visibility, and recovers timer state. Timer operations record durability events and are re-armed from stored history after restart, so workflow-visible time continues to come from recorded event timestamps rather than wall-clock reads inside workflow code.

## Client SDKs

Use client SDKs when application code needs to call Aion workflows over the server API:

- Rust: [`crates/aion-client/README.md`](../../crates/aion-client/README.md) — gRPC client surface with workflow operations and event subscription.
- Gleam: [`gleam/aion_client/README.md`](../../gleam/aion_client/README.md).
- Python: [`sdks/python/aion-client/README.md`](../../sdks/python/aion-client/README.md).
- TypeScript: [`sdks/typescript/aion-client/README.md`](../../sdks/typescript/aion-client/README.md).

## Worker SDKs

Use worker SDKs to host activities outside the workflow VM and connect them to the server worker protocol:

- Rust: [`crates/aion-worker/README.md`](../../crates/aion-worker/README.md).
- Python: [`sdks/python/aion-worker/README.md`](../../sdks/python/aion-worker/README.md).
- TypeScript: [`sdks/typescript/aion-worker/README.md`](../../sdks/typescript/aion-worker/README.md).

The worker protocol is the single bidirectional gRPC stream described in [`WorkerProtocol`](#workerprotocol-workerproto) above; the authoritative contract is [`crates/aion-proto-generated/proto/worker.proto`](../../crates/aion-proto-generated/proto/worker.proto).

The hello-world quickstart uses the Python worker SDK and registers a `greet` activity.

## Workflow authoring SDK

Gleam workflow code is authored with [`aion_flow`](../../gleam/aion_flow/README.md). It provides typed workflow definitions, activity calls, signals, timers, queries, child workflows, codecs, and a test harness.

## Examples

See [`../examples/`](../../examples/) for working examples. Start with [`../examples/hello-world/README.md`](../../examples/hello-world/README.md) for a complete workflow/package/server/worker run.
