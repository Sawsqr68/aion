# Getting started with Aion

This guide takes you from nothing to a completed durable workflow using only
published artifacts — no repository checkout.

Aion has two first-class authoring surfaces, and this guide covers both:

- **[Part 1 — AWL](#part-1--the-awl-path)**, Aion's workflow language. One
  `.awl` file declares the whole workflow, and `aion run` compiles, deploys,
  starts, and awaits it in one motion. **Start here.**
- **[Part 2 — Gleam](#part-2--the-gleam-path)**, for workflows that need real
  typed code: `aion generate` derives codecs and schemas from your types, and
  `aion package` produces a `.aion` archive.

Either way you will install the CLI, run the server, serve an activity, and
then deploy, start, signal, and complete a run — and kill the server mid-run to
watch it survive.

Working from a repository checkout instead? Use
[`examples/awl-hello/README.md`](../examples/awl-hello/README.md) for AWL or
[`examples/hello-world/README.md`](../examples/hello-world/README.md) for
Gleam.

## Prerequisites

- Rust toolchain and Cargo ([rustup](https://rustup.rs) recommended)
- **Part 2 only:** [Gleam](https://gleam.run/getting-started/installing/) with
  Erlang/OTP on your `PATH`. AWL needs no Gleam toolchain.

## 1. Install the CLI

```sh
cargo install aion-cli --locked
```

The crate is named `aion-cli`; the installed binary is **`aion`**. It is the
one user-facing binary: it authors AWL, packages Gleam workflows, runs the
server (`aion server`), serves workers, and operates runs. Verify:

```sh
aion --version
aion --help
```

The subcommands are `server`, `dev`, `awl`, `worker`, `new`, `generate`,
`check`, `input`, `package`, `deploy`, `run`, `versions`, `route`, `unload`,
`start`, `signal`, `query`, `cancel`, `reopen`, `pause`, `resume`, `list`,
`describe`, and `inspect`.

> **Released versus this repository.** This guide is written against the
> **published** `aion` crate, and every AWL document in Part 1 is checked
> against it. The repository's `main` branch carries AWL surfaces that have not
> been released yet — declared `run "…"` command bodies, the `agent` action
> marker, in-language `send`, `query`/`answer`, `aion awl scaffold`, `aion
> worker agent`, and `aion run --timeout`. They are documented in
> [`docs/AWL.md`](AWL.md) and the [language
> reference](../examples/assistant/resources/AWL-REFERENCE.md); if `aion awl
> check` rejects one, you are on the published binary and need a build from
> source. Check with `aion awl --help` and `aion worker --help`.

> Prefer a generated starting point? `aion new <name>` scaffolds a complete,
> buildable **Gleam** project — workflow, schemas, `workflow.toml`, a dev
> `aion.toml`, and a README with these same steps. Templates are
> `hello-world` (default), `approval-flow`, `saga`, `dev-pipeline`, and
> `agent`; `--worker rust` adds a worker crate, and `dev-pipeline` and `agent`
> require it. There is no `.awl` template — for AWL, write the file, which is
> what Part 1 does.

> There is no separate server binary and no installable worker binary.
> `cargo install aion-server` and `cargo install aion-worker` are not how
> Aion is installed: the server is `aion server`, and workers are either
> `aion worker shell` or ordinary Rust/Python/TypeScript programs you write
> against a worker SDK.

# Part 1 — the AWL path

## Write the workflow

Create `my_flow.awl`. It greets the caller through a `greet` activity, waits
durably for an `approval` signal, and returns a record built from both:

```awl
//! Greet a name, hold for approval, then report who approved.
workflow my_flow
  input name: String
  signal approval: Approval
  outcome greeted: type Greeting, route success

/// The payload an operator sends to release the hold.
type Approval { approver: String }
/// The recorded result of a completed run.
type Greeting { greeting: String, approved_by: String }

worker my_flow
  /// A worker on the `my_flow` queue must serve this.
  action greet(name: String) -> String
    timeout 30s, retry 3 every 10s

step greet_and_hold
  greet(name: name) -> greeting
  wait approval -> approved
  route greeted(greeting: greeting, approved_by: approved.approver)
```

That is the whole workflow. Reading it top to bottom:

- `//!` narrates the workflow and `///` documents the declaration that follows.
  Both reach the derived schemas and the ops console — the prose is part of the
  model, not a comment.
- `input` defines the start contract; `signal` declares a durable signal the
  run can receive; `outcome … route success` declares the terminal payload and
  maps it onto the engine's completed status.
- `worker my_flow` declares a **task queue**, and each `action` is the typed
  wire contract something on that queue must serve. It is a requirement, not an
  implementation. The indented config line sets this action's timeout and retry
  policy — those belong to the workflow, not to the worker.
- `step` holds the flow. `->` binds a call's result. `wait approval -> approved`
  is a durable wait: the run suspends there, for seconds or months, and
  survives restarts. `route` ends the run with the declared outcome.

There is no entry function, no codec, no `workflow.toml`, and no build step.

## Check it

```sh
aion awl check my_flow.awl
```

```text
ok: my_flow.awl (1 step)
```

`check` runs entirely locally — it never contacts a server. It parses and
typechecks declarations, binding flow along the step graph, route targets,
outcome exhaustiveness, and schema imports. An error prints as
`my_flow.awl:<line>:<column>: error: <message>` on stderr and exits non-zero.
Fix the source; never emit around a diagnostic.

`aion awl fmt my_flow.awl` rewrites the file in canonical form. The printer
*is* the formatter, so there is no `--check` mode: inspect the diff and check
again.

## See the contracts it derives

The start contract and every declared type are derivable without a server:

```sh
aion awl schema my_flow.awl
```

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "description": "Greet a name, hold for approval, then report who approved.",
  "properties": {
    "name": {
      "type": "string"
    }
  },
  "required": [
    "name"
  ],
  "type": "object"
}
```

Note the `description`: your `//!` narration became the schema's documentation.
`aion awl schema my_flow.awl --type Greeting` does the same for any declared
type. This is the exact contract `aion run` validates your input against.

## Serve the activity

A clean check proves the document is internally consistent. It does **not**
prove a worker exists — a call waits for a worker serving the declared queue
and action. The quickest worker is the built-in shell worker, which maps
actions to host commands. Create `worker.toml`:

```toml
[worker]
name = "my-flow-worker"
task_queue = "my_flow"      # the `worker my_flow` block name IS the queue

[[action]]
name = "greet"
command = ["printf", "Hello, %s!", "{input.name}"]
result = "text"             # trimmed stdout becomes the JSON string result
```

Facts worth knowing before you rely on it: unknown TOML fields are rejected;
`command` is an argv array and is **not** interpreted by a shell;
`{input.field}` projects one top-level scalar field into one whole argument
(arrays, objects, and absent fields cannot be projected); `result = "json"`
instead requires stdout to be valid JSON; and a non-zero exit is a retryable
activity failure, with AWL's declared `retry` policy deciding whether another
attempt is scheduled.

For anything beyond command wiring, write a real worker against a worker SDK —
[Part 2, step 4](#4-write-the-activity-worker) shows the Rust one, and the
[activities and workers guide](guides/activities-and-workers.md) covers failure
classification, heartbeats, and cancellation.

## Configure and run the server

The server reads a TOML config file. Create `aion.toml` in a fresh working
directory — the settings are explained in full in
[step 5 of Part 2](#5-configure-and-run-the-server):

```toml
[server]
listen_address = "127.0.0.1:8080"
grpc_address = "127.0.0.1:50051"

[runtime]
query_timeout_ms = 10000            # REQUIRED, no default

[websocket]
event_broadcast_capacity = 1024     # REQUIRED, no default

[deploy]
enabled = true                      # REQUIRED for `aion deploy` / `aion run`
```

The store needs no configuration: the stock binary defaults to the haematite
backend, and its data directory defaults to `~/.aion/data`. Set `AION_HOME` to
put a server's whole state somewhere else — that is also how you run two
servers on one machine without them sharing state. See the
[operations guide](guides/operations.md#the-aion-home-aion-and-aion_home).

Start the server (terminal 1):

```sh
aion server --config aion.toml
```

In terminal 2, start the worker:

```sh
aion worker shell --manifest worker.toml
```

## Compile, deploy, start, and await — in one motion

In terminal 3:

```sh
aion run my_flow.awl --input '{"name":"Ada"}'
```

`run` does the whole pipeline: it compiles and assembles the document locally,
validates `--input` against the derived start contract **before** it opens the
deploy connection, deploys, starts the workflow under its declared name, and
waits for the terminal result. Try `--input '{"nmae":"Ada"}'` once to watch it
refuse at your desk rather than half-way through a deploy.

The run executes `greet` on your shell worker, then suspends at
`wait approval` — so this command is now waiting. Leave it.

> Use `aion deploy my_flow.awl` instead when you want the document deployed
> without starting a run, then `aion start my_flow --input '{"name":"Ada"}'`
> later. Do not reach for `aion package` here: that is the Gleam path, and an
> `.awl` document never needs it.

## Prove the durability

In terminal 4, find the run and then kill the server hard:

```sh
aion list
kill -9 <server pid>
```

Restart it — same directory, same config:

```sh
aion server --config aion.toml
```

On startup the server reloads the deployed package from the store, replays the
run's event history, and the run is waiting at `wait approval` again, exactly
where it was. The `greet` activity is **not** re-executed: replay returns its
recorded result from history. That is the whole promise, and it costs you
nothing to write — nothing in `my_flow.awl` mentions durability.

## Approve and complete

```sh
aion signal <workflow-id> approval --payload '{"approver":"ada"}'
aion describe <workflow-id> --pretty
```

`describe` shows `"status": "Completed"` and the full event history: workflow
started, `greet` scheduled and completed, the signal delivered, and the
workflow completed carrying
`{"greeting":"Hello, Ada!","approved_by":"ada"}`.

Sending a signal is not proof the workflow consumed it — confirm with
`describe`. Other operating verbs on a live run are `aion pause` /
`aion resume` (durably hold and release activity dispatch), `aion cancel`,
`aion reopen` (re-drive a Failed or Cancelled run), and `aion inspect`
(time-travel over the recorded oplog). See the
[errors reference](errors.md) for every error code.

## Where to go from here in AWL

`my_flow.awl` used a fraction of the language. AWL also has children and
subflows, `fork`/`join`, multi-step `distribute`/`sequence` regions closed by
`collect`, bounded loops and visit-capped flow cycles, timed waits, `sleep`,
and `on failure` compensation.

- **[`docs/AWL.md`](AWL.md)** — every verb and the authoring loop.
- **[The language reference](../examples/assistant/resources/AWL-REFERENCE.md)**
  — the complete grammar, with the checked fixture that proves each construct.
- **[`docs/DECLARED-COMMANDS.md`](DECLARED-COMMANDS.md)** — read before writing
  a `run "…"` action body. **There is no sandbox.**
- **[Editor support](../editors/)** — `aion awl lsp` drives the Zed and Neovim
  extensions.

# Part 2 — the Gleam path

Gleam is the surface for workflows that need real typed code. The steps below
build the same shape as Part 1 — one activity, one signal, one query — in
Gleam, and package it into a versioned `.aion` archive.

## 2. Write the workflow

Workflows are written in [Gleam](https://gleam.run) with the
[`aion_flow`](https://hex.pm/packages/aion_flow) SDK and compiled to BEAM
bytecode that the server executes durably.

```sh
gleam new my_flow
cd my_flow
gleam add aion_flow gleam_json
```

Replace `src/my_flow.gleam` with this workflow. It greets the caller via a
remote `greet` activity, publishes a `status` query, waits durably for an
`approval` signal, and returns a JSON result:

```gleam
import aion/activity
import aion/codec
import aion/error
import aion/query
import aion/signal
import aion/workflow
import gleam/dynamic.{type Dynamic}
import gleam/dynamic/decode
import gleam/json
import gleam/result

pub type HelloInput {
  HelloInput(name: String)
}

pub type GreetingOutput {
  GreetingOutput(greeting: String)
}

pub type Approval {
  Approval(approver: String)
}

pub type FlowError {
  FlowFailed(message: String)
}

/// Engine entry point — the function named by `workflow.toml`.
///
/// The contract: the runtime delivers the start input as a **raw JSON string**
/// inside a `Dynamic`. Decode the string, parse it with your input codec, run
/// the typed workflow, and **encode the success value back to a JSON string**
/// for the recorded result payload.
pub fn run(raw_input: Dynamic) -> Result(String, FlowError) {
  case decode.run(raw_input, decode.string) {
    Ok(raw_json) ->
      case input_codec().decode(raw_json) {
        Ok(input) -> execute(input)
        Error(codec.DecodeError(reason: reason, path: _)) ->
          Error(FlowFailed("failed to decode workflow input: " <> reason))
      }
    Error(_) -> Error(FlowFailed("workflow input payload was not a string"))
  }
}

fn execute(input: HelloInput) -> Result(String, FlowError) {
  // Activities are the only side-effect boundary. The engine records the
  // dispatch, a connected worker executes it, and replay returns the
  // recorded result instead of running it again.
  use greeting <- result.try(greet(input))
  use _ <- result.try(set_status("awaiting_approval"))
  use approval <- result.try(await_approval())
  use _ <- result.try(set_status("approved"))
  Ok(
    json.to_string(
      json.object([
        #("greeting", json.string(greeting)),
        #("approved_by", json.string(approval.approver)),
      ]),
    ),
  )
}

fn greet(input: HelloInput) -> Result(String, FlowError) {
  case workflow.run(greet_activity(input)) {
    Ok(GreetingOutput(greeting: greeting)) -> Ok(greeting)
    Error(activity_error) ->
      Error(FlowFailed(activity_error_message(activity_error)))
  }
}

fn greet_activity(
  input: HelloInput,
) -> activity.Activity(HelloInput, GreetingOutput) {
  // The final argument is a local implementation used only by the
  // `aion/testing` harness; a deployed workflow always dispatches to a worker.
  activity.new("greet", input, input_codec(), greeting_codec(), local_greet)
}

fn local_greet(input: HelloInput) -> Result(GreetingOutput, error.ActivityError) {
  Ok(GreetingOutput(greeting: "Hello, " <> input.name <> "! (test harness)"))
}

fn await_approval() -> Result(Approval, FlowError) {
  // A durable wait: the run suspends here — for seconds or months — and
  // survives server restarts while it waits.
  case workflow.receive(approval_signal()) {
    Ok(approval) -> Ok(approval)
    Error(_) -> Error(FlowFailed("failed to receive the approval signal"))
  }
}

fn approval_signal() -> signal.SignalRef(Approval) {
  signal.new("approval", approval_codec())
}

fn set_status(stage: String) -> Result(Nil, FlowError) {
  // Re-register the `status` query handler with a fresh closure at each
  // state change; queries are answered at yield points and never touch
  // workflow history.
  case query.handler("status", status_codec(), fn() { stage }) {
    Ok(Nil) -> Ok(Nil)
    Error(_) -> Error(FlowFailed("failed to register the status query"))
  }
}

fn input_codec() -> codec.Codec(HelloInput) {
  codec.json_codec(hello_input_to_json, hello_input_decoder())
}

fn hello_input_to_json(input: HelloInput) -> json.Json {
  json.object([#("name", json.string(input.name))])
}

fn hello_input_decoder() -> decode.Decoder(HelloInput) {
  use name <- decode.field("name", decode.string)
  decode.success(HelloInput(name: name))
}

fn greeting_codec() -> codec.Codec(GreetingOutput) {
  codec.json_codec(greeting_to_json, greeting_decoder())
}

fn greeting_to_json(output: GreetingOutput) -> json.Json {
  json.object([#("greeting", json.string(output.greeting))])
}

fn greeting_decoder() -> decode.Decoder(GreetingOutput) {
  use greeting <- decode.field("greeting", decode.string)
  decode.success(GreetingOutput(greeting: greeting))
}

fn approval_codec() -> codec.Codec(Approval) {
  codec.json_codec(approval_to_json, approval_decoder())
}

fn approval_to_json(approval: Approval) -> json.Json {
  json.object([#("approver", json.string(approval.approver))])
}

fn approval_decoder() -> decode.Decoder(Approval) {
  use approver <- decode.field("approver", decode.string)
  decode.success(Approval(approver: approver))
}

fn status_codec() -> codec.Codec(String) {
  codec.json_codec(json.string, decode.string)
}

fn activity_error_message(activity_error: error.ActivityError) -> String {
  case activity_error {
    error.Retryable(message: message, details: _) -> message
    error.Terminal(message: message, details: _) -> message
    _ -> "greet activity failed"
  }
}
```

The full authoring surface — timers, timeout races, child workflows,
determinism rules — is in the
[workflow authoring guide](guides/workflows.md).

## 3. Describe and build the package

Add `workflow.toml` next to `gleam.toml`:

```toml
# Packaging descriptor read by `aion package`. Reference: docs/packaging.md.
[[workflow]]
entry_module = "my_flow"          # also the workflow type clients start
entry_function = "run"
timeout_seconds = 3600
input_schema = "schemas/input.json"
output_schema = "schemas/output.json"
activities = ["greet"]            # every activity the workflow dispatches
output = "my-flow.aion"
```

Unknown keys in `workflow.toml` are hard errors — typos fail loudly. The two
schema files it references are **generated artifacts**: your Gleam types are
the source of truth (types-first, ADR-014). Declare the boundary types in
`src/my_flow_io.gleam`:

```gleam
/// The workflow's start input.
pub type Input {
  Input(name: String)
}

/// The workflow's recorded result.
pub type Output {
  Output(greeting: String, approved_by: String)
}
```

then generate everything wire-shaped from them:

```sh
aion generate .
```

This writes `src/my_flow_codecs.gleam` — a JSON encoder/decoder pair plus a
typed codec per type, deterministic and marked do-not-edit — and emits
`schemas/input.json` / `schemas/output.json` (the documents `workflow.toml`
references, marked as generated):

```json
{
  "$comment": "Generated by aion generate from src/my_flow_io.gleam — do not edit; run `aion generate`.",
  "type": "object",
  "required": ["name"],
  "additionalProperties": false,
  "properties": {
    "name": { "type": "string" }
  }
}
```

Re-run `aion generate .` after every type change, and add
`aion generate . --check` to CI to fail on drift. The supported type subset
and the full contract are in the
[types-first codegen guide](guides/codegen.md).

Build and package:

```sh
gleam build
aion package .
```

You should see a JSON report naming the archive and its content-hash version:

```json
{
  "packages": [
    {
      "workflow_type": "my_flow",
      "output": "/.../my_flow/my-flow.aion",
      "version": "<sha-256 content hash>",
      "deployed_name": "my_flow$<hash>",
      "modules": 42
    }
  ],
  "excluded": [
    { "module": "gleeunit", "reason": "dev_dependency" },
    { "module": "aion@testing", "reason": "sdk_test_only" }
  ]
}
```

(The real `excluded` list is longer — dev dependencies and the SDK's
test-only modules are excluded from the archive by design; each entry
names its reason.)

(`aion package . --build` compiles and packages in one step. The full
packaging reference is [`docs/packaging.md`](packaging.md).)

## 4. Write the activity worker

Activities run outside the workflow VM, in workers you write against a worker
SDK (Rust here; Python and TypeScript SDKs also exist under `sdks/`). The
Rust SDK is a **library** — `cargo install aion-worker` fails with
"no binaries". Scaffold your own crate:

```sh
cd ..
cargo new my-worker
cd my-worker
cargo add aion-worker@0.5
cargo add tokio --features macros,rt-multi-thread
cargo add serde --features derive
cargo add serde_json
```

Replace `src/main.rs`:

```rust
use std::time::Duration;

use aion_worker::{ActivityContext, HandlerFuture, Worker, WorkerConfig};
use serde::{Deserialize, Serialize};

// Input types need Serialize + DeserializeOwned; output types need Serialize.
#[derive(Serialize, Deserialize)]
struct GreetInput {
    name: String,
}

#[derive(Serialize)]
struct GreetOutput {
    greeting: String,
}

fn greet(input: GreetInput, _context: &ActivityContext) -> HandlerFuture<'_, GreetOutput> {
    Box::pin(async move {
        Ok(GreetOutput {
            greeting: format!("Hello, {}! Welcome to Aion.", input.name),
        })
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Every field is explicit; there are no hidden defaults.
    let config = WorkerConfig::builder()
        .endpoint("http://127.0.0.1:50051")
        .task_queue("default")
        .identity("my-worker-1")
        .max_concurrency(4)
        .reconnect_initial_backoff(Duration::from_millis(100))
        .reconnect_max_backoff(Duration::from_secs(5))
        .reconnect_max_attempts(10)
        .build()?;

    Worker::builder(config)
        .register_activity("greet", greet)?
        .build()?
        .run()
        .await?;

    Ok(())
}
```

The handler's `Ok` value is serialized as the activity result — here a JSON
object the workflow's `greeting_codec` decodes. Failure classification,
retries, heartbeats, and cancellation are covered in the
[activities and workers guide](guides/activities-and-workers.md).

## 5. Configure and run the server

The server reads a TOML config file. Two keys have **no default** and must be
set (`runtime.query_timeout_ms`, `websocket.event_broadcast_capacity`), and
the `[deploy]` section must be enabled — with both size ceilings — for
`aion deploy` to work. Missing keys fail startup with an error naming the key.

Create `aion.toml` in a fresh working directory:

```toml
[server]
listen_address = "127.0.0.1:8080"   # HTTP/JSON API + ops console
grpc_address = "127.0.0.1:50051"    # gRPC API + worker protocol

[store]
backend = "libsql"                  # durable; "memory" loses state on stop
url = "aion.db"                     # embedded libSQL file, created on start

[runtime]
# REQUIRED, no default: reply deadline for workflow queries, in milliseconds.
query_timeout_ms = 10000

[websocket]
# REQUIRED, no default: capacity of the global live-event broadcast channel.
event_broadcast_capacity = 1024

[deploy]
# Runtime package deploy/route/unload. Dark by default; when enabled both
# ceilings default to conservative limits (64 MiB archive / 256 MiB inflated),
# so you can omit them. When set, max_inflated_bytes must be >= max_archive_bytes.
enabled = true
max_archive_bytes = 16777216        # 16 MiB upload ceiling
max_inflated_bytes = 67108864       # 64 MiB decompressed-contents ceiling
```

Every key can also be set by environment variable as
`AION_<SECTION>_<KEY>` (for example `AION_RUNTIME_QUERY_TIMEOUT_MS=10000`).
The complete config reference is in the
[operations guide](guides/operations.md).

Start the server (terminal 1):

```sh
aion server --config aion.toml
```

You should see JSON log lines on stdout, including the HTTP and gRPC listen
addresses. Leave it running. In terminal 2, start the worker:

```sh
cargo run --manifest-path my-worker/Cargo.toml
```

## 6. Deploy and run the workflow

In terminal 3, deploy the archive to the running server:

```sh
aion deploy my_flow/my-flow.aion
```

You should see a JSON response with `workflow_type`, `content_hash`,
`freshly_loaded: true`, and `route_changed: true`. Runtime-deployed packages
**persist in the store**: after a restart, the server reloads them before
recovering workflows, so deployed code survives restarts too.

Start a run (the workflow type is the entry module name, `my_flow`):

```sh
aion start my_flow --input '{"name":"Ada"}'
```

You should see the assigned identifiers:

```json
{"run_id":"<run-id>","workflow_id":"<workflow-id>"}
```

The run executes `greet` on your worker, then suspends waiting for the
`approval` signal. Ask it where it is:

```sh
aion query <workflow-id> status
```

You should see `{"result":"awaiting_approval"}`. Queries are answered live by the
handler the workflow registered — they read state, never change it.

## 7. Prove the durability

While the run is waiting, kill the server hard:

```sh
kill -9 <server pid>
```

Restart it (same directory, same config):

```sh
aion server --config aion.toml
```

On startup the server reloads the deployed package from the store, replays
the run's event history, and the run is waiting for `approval` again —
exactly where it was. The `greet` activity is **not** re-executed; replay
returns its recorded result.

## 8. Approve and complete

```sh
aion signal <workflow-id> approval --payload '{"approver":"ada"}'
aion describe <workflow-id> --pretty
```

`describe` shows the summary with `"status": "Completed"` and the event
history: workflow start, `greet` scheduled and completed, the signal, and
workflow completion carrying the JSON result
`{"greeting":"Hello, Ada! Welcome to Aion.","approved_by":"ada"}`.

A completed run no longer answers live queries — `aion query` against it now
returns `error[not_running]`. That is expected: terminal runs are inspected
with `describe`, not queried. See the
[errors reference](errors.md) for every error code.

## 9. Versions, routing, rollback

Each deployed package version is identified by its content hash, and new
starts route to one version per workflow type:

```sh
aion versions                       # every loaded version, route flags
aion route my_flow <content-hash>   # roll back / forward to a loaded version
aion unload my_flow <content-hash>  # remove a non-routed, unpinned version
```

Running workflows keep executing the version they started on while new
starts use the routed version. The full deploy, versioning, and recovery
model is in the [operations guide](guides/operations.md).

## Where to go next

- [`docs/AWL.md`](AWL.md) — the AWL surface: every verb, the authoring loop,
  and the full language reference.
- [Declared commands](DECLARED-COMMANDS.md) — `run "…"` action bodies: what is
  guaranteed and what is not. **There is no sandbox.**
- [Workflow authoring guide](guides/workflows.md) — the Gleam entry contract,
  determinism rules, timers, signals, queries, child workflows.
- [Activities and workers guide](guides/activities-and-workers.md) — worker
  scaffolding, failure classification, retry semantics as they actually are.
- [Schema codegen guide](guides/codegen.md) — generate Gleam types and JSON
  codecs from your schemas; `--check` in CI; the supported subset.
- [Operations guide](guides/operations.md) — full config reference, deploy
  surface, persistence and recovery, metrics.
- [Errors reference](errors.md) — every error code, with hints.
- [`docs/API.md`](API.md) — HTTP/gRPC/WebSocket transports.
- [`docs/examples/order-saga.md`](examples/order-saga.md) — the flagship
  order-fulfillment saga: retries, timeout races, child workflows, and saga
  compensation in one walkthrough.
- [`examples/stacked-dev/`](../examples/stacked-dev/) — the composition
  showcase: a three-workflow family driving a real dev pipeline (concurrent
  activities, bounded verify-fix loops, a signal-driven review gate, typed
  exhaustion), runnable hermetically with `gleam test` or live against a
  server.
