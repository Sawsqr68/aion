# Activities and workers guide

Activities are where side effects live: HTTP calls, database writes,
payments, emails. Workflow code dispatches them through the recorded
boundary (`workflow.run`); **workers** — ordinary programs you write against
a worker SDK — execute them and report results back over the gRPC worker
protocol.

This guide covers the Rust worker SDK (`aion-worker`) in depth, the routing
model, registration admission, the worker-side failure contract, and retry
semantics as they actually are. Python and TypeScript worker SDKs live under
[`sdks/`](../../sdks/) and follow the same protocol.

## Workers are programs you scaffold, not binaries you install

`aion-worker` is a **library crate** — `cargo install aion-worker` fails
with "there is nothing to install ... contains no binaries". That is by
design: your activities are your code, so a worker is your own crate:

```sh
cargo new my-worker
cd my-worker
cargo add aion-worker@0.11
cargo add tokio --features macros,rt-multi-thread
cargo add serde --features derive
cargo add serde_json
cargo add schemars@1
```

`schemars` is what derives the typed schemas a worker advertises at
registration (below). `aion-worker` depends on the same major and re-exports
it as `aion_worker::schemars`, so the two can never disagree about the schema
dialect.

Before writing one at all, check whether something already serves your case:
a declarative `run "…"` body in the document
([DECLARED-COMMANDS.md](DECLARED-COMMANDS.md)), `aion worker shell` with a
manifest ([SHELL-WORKER-MANIFEST.md](SHELL-WORKER-MANIFEST.md)),
`aion worker agent` ([AGENT-WORKERS.md](AGENT-WORKERS.md)), or
`aion awl scaffold`.

## A complete worker

```rust
use std::time::Duration;

use aion_worker::{ActivityContext, HandlerFuture, Worker, WorkerConfig};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema)]
struct GreetInput {
    name: String,
}

#[derive(Serialize, JsonSchema)]
struct GreetOutput {
    greeting: String,
}

fn greet(input: GreetInput, _context: &ActivityContext) -> HandlerFuture<'_, GreetOutput> {
    Box::pin(async move {
        Ok(GreetOutput {
            greeting: format!("Hello, {}!", input.name),
        })
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WorkerConfig::builder()
        .endpoint("http://127.0.0.1:50051")
        .task_queue("greeter")
        .identity("my-worker-1")
        .max_concurrency(4)
        .reconnect_initial_backoff(Duration::from_millis(100))
        .reconnect_max_backoff(Duration::from_secs(5))
        .reconnect_max_attempts(10)
        .build()?;

    Worker::builder(config)
        .register_activity_with_contract("greet", greet)?
        .build()?
        .run()
        .await?;

    Ok(())
}
```

`run()` connects to the server's gRPC address (the server's
`[server] grpc_address`), registers the activity types, and serves until a
non-retryable error or shutdown.

`register_activity_with_contract` rather than `register_activity` is
deliberate: it derives a typed **descriptor** from the Rust types and
advertises it. A worker that advertises names without descriptors is refused
by the server on any queue carrying a deployed contract — see [registration
admission](#registration-admission-the-first-thing-that-goes-wrong).

## What the builder requires, and what it defaults

`WorkerConfigBuilder::build` names the missing field it refuses on
(`crates/aion-worker/src/config.rs`). Not every field is required — three
carry documented defaults and one is genuinely optional, and knowing which is
the difference between a worker that routes where you meant and one that
quietly does not.

| Field | Required | Default when omitted |
|---|---|---|
| `endpoint` | **yes** | — (`MissingEndpoint`) |
| `task_queue` | **yes** | — (`MissingTaskQueue`) |
| `identity` | **yes** | — (`MissingIdentity`) |
| `max_concurrency` | **yes** | — (`MissingMaxConcurrency`) |
| `reconnect_initial_backoff` | **yes** | — (`MissingReconnectInitialBackoff`) |
| `reconnect_max_backoff` | **yes** | — (`MissingReconnectMaxBackoff`) |
| `reconnect_max_attempts` | **yes** | — (`MissingReconnectMaxAttempts`) |
| `namespaces` / `namespace` | no | `["default"]` |
| `subject` | no | `"worker"` |
| `node` | no | this machine's hostname (`$HOSTNAME`, else the OS hostname, else the literal `localhost` — a worker never fails to start over an unresolvable hostname) |
| `transport_credentials` | no | none |

The seven required ones are operator policy the SDK has no business guessing.
The three that default do so because the default is a fact rather than a
choice: `default` is the named default namespace, `worker` is the auth
subject the server expects, and the node IS the machine you are on.

**`node` defaulting to the hostname is the one to watch.** It is a routing
dimension (below), so a worker you never configured a node for still
advertises one — and a dispatch pinned to a different node will not reach it.

## The routing model: namespace × task queue × node

Three independent axes. Conflating them is the most common way a workflow
sits at `ActivityScheduled` forever.

- **Namespace** is the correctness/isolation boundary — a tenant, an
  environment. A worker registers into the SET of namespaces it is authorized
  for (`WorkerConfig::namespaces`, advertised in the `x-aion-namespaces`
  stream metadata and as the registration scope, so authorization and
  registration scope agree). The dispatch's namespace is the one the workflow
  is running in.
- **Task queue** is the pool/flavour selector *within* a namespace. The
  worker-pool address is the pair `(namespace, task_queue)`
  (`crates/aion-server/src/worker/registry.rs`). In AWL the queue is the
  `worker <name>` block's name — the block name IS the queue.
- **Node** is an optional locality affinity a connection advertises. It is a
  **within-pool filter, not part of the pool key**: an unpinned dispatch
  (`node: None`) reaches every worker in the pool; a dispatch pinned to a node
  reaches only a connection advertising that exact node, and waits if none is
  present (`worker_matches_node` in the same file). A connection that
  advertises no node at all is reachable by unpinned dispatches only.

They are independent. A worker's task queue is **not** its namespace, and
setting the two to the same string is a coincidence of the default
configuration (`default` / `default`), not a rule.

### How a dispatch finds a worker

1. The workflow schedules an activity. The **namespace** is the one the
   workflow runs in. The **task queue** is resolved in this order
   (`crates/aion/src/runtime/nif_activity.rs`, `resolve_task_queue`): the
   per-activity selection in the dispatch config, else the workflow-level
   declared default, else the queue the workflow was started on, else the
   named default `"default"`. The **node** is the per-activity `config.node`
   or `None`; there is no workflow-level node default.
2. The server looks up the pool `(namespace, task_queue)` and, within it, the
   index for the exact `activity_type`; then it filters by node and rotates,
   so each pool round-robins independently across its workers
   (`ConnectedWorkerRegistry::workers_for`).
3. If nothing matches, the dispatch **waits** — an activity may legitimately
   wait for a worker to arrive, and on a stock server it waits without bound
   (`[worker.queue_service]` policy `strict`, with no
   `service_availability_deadline` set, invents no clock). Two things cut that
   short: a **structural** unservability — `NO_QUEUE_DECLARATION`, provable
   from the deployed contracts alone and unchangeable by waiting — refuses
   immediately; and an operator-configured availability deadline or
   schedule-to-start timeout refuses with a typed reason when it expires
   (`crates/aion-server/src/worker/queue_service/`).

One binary may hold several connections; each registers ONE node, and a
connection must serve every activity type a dispatch on its node can reach.
That is also exactly what admission demands of it, below.

If a workflow seems stuck right after start, check `aion describe <id>` — an
`ActivityScheduled` event with no completion almost always means no connected
worker serves that activity type in that pool, or the dispatch is pinned to a
node no connection advertises.

## Registration admission: the first thing that goes wrong

A worker does not simply connect. The server validates the connection's
advertised action surface against the deployed contracts on its queue
**before the worker becomes dispatch-visible**
(`crates/aion-server/src/worker/contracts.rs`,
`validate_worker_contracts`). Get this wrong and the worker retries forever
and looks merely asleep.

**What it is held to.** Not every retained package version — only the
REACHABLE ones (`crates/aion/src/engine/admission.rs`). Under content-hash
namespacing a queue retains every version ever deployed to it, so a version
binds a registering worker only when a dispatch could actually be produced
under it:

- it is **route-active** — a caller can start a workflow on it at any moment;
- a **live run** (non-terminal) is pinned to it;
- a **start is in flight** on it.

A retained version that is none of those can never be entered, so demanding
it protects nothing. That narrowing is not theoretical tidiness: one stale,
non-routed package once made a whole task queue permanently unservable.

**What is demanded within a version.** The bodyless actions whose dispatch
can reach this connection's node
(`crates/aion-package/src/compatibility.rs`, `contract_diffs`):

- An action carrying a **declared `run` body is filtered out of the required
  surface entirely.** The server executes it itself, so no worker can
  advertise it and none is asked to. Without that filter, a queue mixing a
  declared body with a bodyless action would be unservable — the worker doing
  its whole job refused for omitting an action that was never its job.
- An action **pinned to a node** is demanded only of the connection
  advertising that node. An **unpinned** action is demanded of every
  connection in the pool, including one carrying no node at all.
- Schemas are compared semantically, not byte-for-byte. Input compatibility
  is contravariant (every value the workflow may send must be accepted by the
  worker) and output covariant (every value the worker may emit must be
  decodable by the workflow), so input widening and optional output-field
  additions pass while narrowing and undeclared output drift are refused.
  `$comment` is ignored, and `$defs`/`$ref` compare against the equivalent
  inline schema.

**There is no partial admission.** Any single disagreement refuses the WHOLE
registration (`contracts.rs`: a non-empty disagreement list becomes
`ContractAdmissionError::Mismatch`). The worker is never admitted for the
actions it got right and refused for the rest — that would leave a worker
selectable for a dispatch it could not serve.

**Names and contracts are two different advertisements.** A registration
carries activity-type NAMES (what the dispatcher selects on) and typed
DESCRIPTORS (what admission compares). Nothing synthesizes one from the
other. `register_activity` inserts a handler with **no descriptor**; only
`register_activity_with_contract` (derived from the Rust types via
`schemars`) and `register_activity_with_descriptor` (an explicit descriptor,
for schemas that come from a compiled AWL contract) make an action
admissible. A worker registered entirely through plain `register_activity` is
selectable and inadmissible at the same time — this is a real bug that
shipped, and it is why the refusal now states both counts.

**What you actually see.** The server logs it at WARN and the worker prints
the identical string back
(`crates/aion-server/src/worker/contracts/refusal.rs`):

```text
WORKER_CONTRACT_MISMATCH: worker build `stale-worker` queue `payments`
refused. checked 1 reachable deployed package version (1 routes new starts);
ignored 1 unreachable version that nothing can dispatch under: `0c578458…`
(workflow `admission_drift`). worker advertised 1 activity-type name and 1
typed contract, and admission compares CONTRACTS. this connection advertises
no node, so it is held only to unpinned actions. 1 disagreement. [1/1]
package `854bace6…`
(workflow `admission_drift`, held because it currently routes new starts)
action `charge` field `input_schema.properties.amount.type` expected
"string" but worker advertised "integer". REMEDY: rebuild and redeploy this
worker from the same source as the deployed package(s) named above, so it
advertises their action shapes; a version that routes new starts cannot be
unloaded — supersede it by deploying a newer version of that workflow, or
re-point the route at another loaded version with POST /deploy/route; the
ignored version(s) can be cleared with POST /deploy/unload
{"workflow_type":"admission_drift","content_hash":"0c578458…"} — that is
hygiene only and will NOT change this refusal
```

Read it in this order:

1. **`advertised N activity-type names and M typed contracts`.** If M is 0,
   or the refusal lists actions "advertised by name with NO contract", the
   fix is the registration path, not the schemas: register through
   `register_activity_with_contract` or `register_activity_with_descriptor`.
2. **The node clause.** It tells you which actions this connection owed.
   An action you believe belongs to another connection appearing here means
   the document's pin and the worker's node disagree.
3. **Each `[i/N]` line.** The exact package, the exact field, expected versus
   advertised. Rebuild the worker from the same source as that package.
4. **`held because …`.** Route-active, a live run, or a start in flight —
   the same three conditions on which an unload refuses. That is why the
   remedy never tells you to unload a version named in a disagreement: you
   would walk straight into a second refusal. Supersede it with a newer
   deploy, or re-point the route.

`aion worker agent --check` prints the exact descriptors a worker would
advertise without connecting, and `aion worker shell` reconciles its manifest
against the document at startup. Both exist so a mismatch is caught at your
desk rather than decoded from the far side of a socket.

## `register_activity` bounds — read these before fighting the compiler

The exact signature (identical on `WorkerBuilder` and `ActivityRegistry`):

```rust
pub fn register_activity<Input, Output, Handler>(
    self,
    activity_type: impl Into<String>,
    handler: Handler,
) -> Result<Self, WorkerError>
where
    Input: Serialize + DeserializeOwned + Send + Sync + 'static,
    Output: Serialize + Send + Sync + 'static,
    Handler: for<'context> Fn(Input, &'context ActivityContext) -> HandlerFuture<'context, Output>
        + Send
        + Sync
        + 'static,
```

`register_activity_with_contract` takes the same arguments and adds
`schemars::JsonSchema` to both `Input` and `Output`.
`register_activity_with_descriptor` takes an `aion_package::ActivityDescriptor`
between the name and the handler and keeps the plain bounds.

In practice:

- **Input types need `#[derive(Serialize, Deserialize)]`** — deriving only
  `Deserialize` does not compile, because the bound is
  `Serialize + DeserializeOwned`.
- **Output types need `#[derive(Serialize)]`.**
- Add `#[derive(JsonSchema)]` to both for the `_with_contract` path.
- The handler returns `HandlerFuture<'_, Output>` =
  `Pin<Box<dyn Future<Output = Result<Output, ActivityFailure>> + Send + '_>>`
  — wrap an `async move` block in `Box::pin(...)` as shown above. Plain
  `async fn` does not satisfy the bound directly.
- Registering the same activity type twice returns
  `WorkerError::Registration`.

The activity type string must match what the workflow dispatches
(`activity.new("greet", ...)`) and what `workflow.toml` declares.

## The failure contract: classify, don't retry

A handler fails by returning `Err(ActivityFailure)`, and the failure carries
an explicit classification:

```rust
use aion_worker::ActivityFailure;

// Transient — the engine may try again under the declared retry policy:
return Err(ActivityFailure::retryable("gateway 503"));

// Permanent — retrying cannot help:
return Err(ActivityFailure::terminal("card declined"));
```

Attach structured detail with `.with_detail(payload)`.

The worker SDK **never retries** a failed activity itself; it reports the
failure with its classification, and the workflow sees it as
`error.Retryable(message:, details:)` or `error.Terminal(message:, details:)`.

### Retry semantics

Retries are **engine-driven from the declared policy**, for remote
activities. The classification the worker reports is what the retry loop keys
on: the wire carries a structured `ActivityErrorKind`, the server seam
collapses it into `retryable:` / `terminal:` prefixes, and the engine's
completion task re-dispatches with an incremented attempt while budget
remains (`crates/aion/src/runtime/nif_activity_retry.rs`,
`nif_activity_dispatch::spawn_completion_task`).

- **No declared policy means exactly one attempt.** An activity with no
  `retry` carries `"retry": null` and runs once. There is no engine-imposed
  default policy, and a *malformed* policy is logged and treated as absent
  rather than coerced into semantics the author never wrote.
- **`max_attempts` is the TOTAL budget** in the Gleam SDK's vocabulary — a
  policy of 3 means at most 2 retries after the first delivery. AWL's
  `retry N` declares N FURTHER attempts and is lowered to `N + 1`, so the two
  surfaces mean what they say and the engine sees one convention.
- **Backoff** is `fixed`, `linear` (`initial + increment × (n-1)`), or
  `exponential` (`initial × multiplier^(n-1)`), each capped at its own `max`.
- **In-VM activities stay single-attempt.** An in-VM retry would have to be
  driven from a seam that holds the runner, and the remote retry loop
  deliberately does not cover that tier.
- **A worker death is attempt-neutral.** A transport-domain loss (the worker
  died before reporting anything, so the activity never ran) is classified
  separately from `retryable:` and consumes **no** authored budget: it
  re-dispatches the SAME attempt. An authored `retry 2` prices the chance the
  ACTION fails, and charging a bad network day against it would make authored
  semantics weather-dependent. The transport carries its own ceiling on that
  neutrality, so the composed system always terminates.
- Tasks delivered to workers carry a one-based delivery `attempt` counter,
  visible as `context.attempt()`.

A workflow-level bounded retry loop — on `Error(error.Retryable(..))`, sleep
a durable backoff and dispatch a fresh recorded attempt — is still a valid
and fully replay-honest pattern, and
[`examples/order-fulfillment/`](../../examples/order-fulfillment/)
implements it. Use it when the *workflow* needs to observe and branch on each
failure; use the declared policy when it only needs the activity to succeed.

Either way, make activities **idempotent** where you can. A crash between an
activity completing and its result being recorded means the activity may be
delivered again.

## Heartbeats, cancellation, timeouts

`ActivityContext` gives long-running handlers their lifeline:

- `context.heartbeat(Some(progress_payload))` reports progress. Heartbeats
  are explicit — the SDK never emits automatic periodic heartbeats, and the
  worker does not enforce heartbeat timeouts.
- `context.is_cancelled()` supports **cooperative** cancellation: the worker
  never aborts a running handler; check the flag at sensible intervals and
  exit gracefully.
- There is no worker-side activity timeout configuration. A per-activity
  `timeout` declared in AWL or attached in the Gleam SDK travels to the
  engine in the dispatch config as `timeout_ms` and is **not consumed** —
  nothing on the Rust side reads it today. The workflow-level timeout
  (`timeout_seconds` in `workflow.toml`) is what bounds the run as a whole.

## The worker protocol, briefly

One bidirectional gRPC stream per worker (contract:
[`crates/aion-proto-generated/proto/worker.proto`](../../crates/aion-proto-generated/proto/worker.proto)):

- The server acknowledges registration with a `RegisterAck` (assigned worker
  id, authorized namespace, heartbeat window) before dispatching tasks — or
  refuses it with `WORKER_CONTRACT_MISMATCH`, above.
- Every consumed result is acknowledged with a `ResultAck`; unacked results
  are re-reported after reconnect, so results are not lost to a dropped
  connection.
- Reconnection uses the configured backoff: delay
  `initial × 2^(failures-1)`, capped at `max_backoff`. A server
  `DrainRequest` asks workers to finish in-flight work and reconnect without
  consuming their drop budget, and the budget also resets once a session
  proves healthy — it served at least one task, or it outlived
  `max_backoff`.
- A worker whose stream ends — process death, disconnect, expired token —
  is considered lost: its in-flight activities fail back to the engine as
  transport-domain lost-worker failures, which re-dispatch the same attempt
  without spending the activity's authored retry budget. The engine imposes
  no activity timeout of its own; an activity may legitimately run for hours,
  bounded only by the workflow's `timeout_seconds` and worker liveness. The
  server's `[worker] heartbeat_window` (default 30 s) is the heartbeat
  cadence advertised in the `RegisterAck`; window-based expiry of
  hung-but-connected workers is roadmap.

## Matching the pieces up

For an activity to execute, the names must line up:

| Place | What |
|---|---|
| Workflow code | `activity.new("greet", ...)` |
| `workflow.toml` | `activities = ["greet"]` |
| Worker | `.register_activity_with_contract("greet", greet)` |

…and the routing must line up too: the worker's `(namespace, task_queue)`
must be the pool the dispatch addresses, and its `node` must match any pin
the document places on the action. See
[the routing model](#the-routing-model-namespace--task-queue--node).

## Python and TypeScript

The same protocol and the same contract (typed handlers,
retryable/terminal classification, explicit heartbeats, cooperative
cancellation) via:

- [`sdks/python/aion-worker`](../../sdks/python/aion-worker/) — used by the
  bundled examples (`examples/hello-world/worker.py`).
- [`sdks/typescript/aion-worker`](../../sdks/typescript/aion-worker/)
