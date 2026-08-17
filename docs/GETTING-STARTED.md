# Getting started with Aion

Aion is a durable workflow engine. A workflow written for it survives crashes,
reboots, and months of sleep, and resumes exactly where it was — the engine
records every step and replays that history instead of re-doing work.

This guide takes you from nothing to a running system, three ways. They are
ordered by how much you want to see happen versus how much you want to
understand while it does.

| | What you get | What it costs |
|---|---|---|
| **[Part 0 — one command](#part-0--the-one-command)** | A running server, the web console, and an AI authoring assistant you can talk to | A repository clone and one script |
| **[Part 1 — AWL by hand](#part-1--the-awl-path)** | The same engine, built up step by step, in Aion's own workflow language | Published artifacts only — no checkout |
| **[Part 2 — Gleam](#part-2--the-gleam-path)** | The typed-code path, for workflows that need real code | A Gleam toolchain as well |

Parts 1 and 2 are the ones that teach you the system. Part 0 is the one that
shows you it exists. If you only have ten minutes, take Part 0; if you are
going to build on this, come back and walk Part 1.

**The two-command start.** With only the Rust toolchain, this is a running
server and the ops console in your browser:

```sh
cargo install aion-cli
aion
```

Bare `aion` starts the server in the background, waits until it answers its
own liveness probe, and tells you in plain lines where everything lives: the
config its first boot wrote (`~/.aion/config.toml` — yours to edit, comments
inside explain every surface it enables), the data directory, the console
URL, the server log (`~/.aion/server.log`), and the process id that stops it.
Run `aion` again and it reports the running server instead of starting a
second one. `aion --foreground` keeps the server in your terminal;
`aion --no-open` skips the browser. What it starts is exactly `aion server` —
the engine Parts 1 and 2 teach you to drive.

Working from a repository checkout and want a bundled example instead? Use
[`examples/awl-hello/README.md`](../examples/awl-hello/README.md) for AWL or
[`examples/hello-world/README.md`](../examples/hello-world/README.md) for
Gleam.

## Two habits worth having before you start

Both of these are the difference between a setup that works and a setup you
merely believe works.

**Verify, then proceed.** Every step below says what success looks like. If
you did not see it, the step did not succeed — diagnose it before moving on.
Do not read a command's silence as its approval. This matters more here than
in most systems, because a status code is not evidence: the console answers
`200 text/html` for any path it does not recognise, so a reachable URL proves
nothing on its own. Read the *body*.

**Never restart the server as a reflex.** If something looks wrong, read the
logs first — `~/.aion/server.log` and, once Part 0 has run,
`~/.aion/assistant-worker.log`. A durable engine's server holds live state.
Restarting is a deliberate act, not a troubleshooting twitch, and it destroys
the evidence you needed to find out what was actually wrong.

## Prerequisites

| Requirement | Check | Success looks like | Needed for |
|---|---|---|---|
| Rust toolchain and Cargo ([rustup](https://rustup.rs)) | `cargo --version` | a version line | everything |
| git | `git --version` | a version line | Part 0 |
| [norn](https://github.com/ablative-io/norn), installed and signed in | `norn --version` | a version line | Part 0 only |
| [Gleam](https://gleam.run/getting-started/installing/) with Erlang/OTP on `PATH` | `gleam --version` | a version line | Part 2 only |

**On norn, stated plainly:** norn is the agent runtime the assistant thinks
with. `scripts/setup.sh` **refuses to start without it** — it is checked in the
script's first step and is a hard failure, not a degradation. That is a
property of the setup script, not of Aion: the server, the console, and both
authoring surfaces need no norn at all, which is why Parts 1 and 2 never
mention it. If you do not have norn, skip Part 0 and start at Part 1; you lose
the assistant and nothing else.

The repository is public, so the clone in Part 0 needs no GitHub account and no
authentication.

# Part 0 — the one command

## Run it

```sh
git clone --depth 1 https://github.com/ablative-io/aion.git
cd aion
bash scripts/setup.sh
```

The script also works standalone, with no clone at all — it will clone the
repository itself, into `./aion` or wherever `AION_CHECKOUT_DIR` points:

```sh
curl -fsSL https://raw.githubusercontent.com/ablative-io/aion/main/scripts/setup.sh | bash
```

It is safe to re-run. Every step detects work already done and skips it: an
already-running server for this Aion home is left alone, a current worker
binary is not rebuilt, and an existing config is never overwritten.

## What it does, in order

It announces each step as it goes.

1. **Checks prerequisites** — `git`, `cargo`, `norn`. Any missing one stops
   the script by name.
2. **Installs the `aion` CLI** if it is not already on `PATH`. **A cold
   install compiles the whole server and takes around 25 minutes.** That is
   expected; let it run. If `aion` is still missing when it finishes, the
   cause is almost always that `~/.cargo/bin` is not on your `PATH` — the
   script says exactly that.
3. **Prepares `~/.aion`**, the server's home, with owner-only permissions
   (`chmod 700`). The server's path-safety gate refuses a store with a group-
   or world-writable ancestor, so doing this up front means it never fires.
4. **Starts the server** and waits up to 120 seconds for it to actually
   answer — proven by the body of `/build`, not by a status code. A first
   start that finds no config anywhere writes `~/.aion/config.toml` itself
   (`chmod 600`) from the server's embedded template, with the two surfaces
   the rest of the script needs already on — `[deploy]`, so `aion deploy`
   has something to talk to, and `[outbox]`, which hosts the listener remote
   workers connect in to. Both ship dark by default; the file says in
   comments what each one opens and what that means, and it is yours to
   edit. An existing config is never touched.
5. **Reads the server's workspace root** out of its own JSON startup banner
   and threads that one resolved value to the worker, so both sides cannot
   disagree about where session workspaces live.
6. **Deploys the assistant workflow** (`crates/aion-server/assistant-embed/assistant.awl`).
   The server also carries that document compiled into itself and installs it at
   boot on a home that holds no version of it (#139 leg C), so an install with no
   repository checkout still has an assistant; the explicit deploy here is the
   script's own, so a checkout that has EDITED the document deploys what it edited.
   Without a checkout, `aion assistant document --output assistant.awl` writes the
   same bytes out of the binary.
7. **Builds and starts the assistant worker**, then waits up to 30 seconds for
   it to register with the server — and fails loudly if it exits first.

## What success looks like

A final block naming the addresses it actually bound:

```
==> Done
    Console:         http://127.0.0.1:8080
    Server build:    {"version":"0.11.0", …}
    Server log:      /Users/you/.aion/server.log
    Worker log:      /Users/you/.aion/assistant-worker.log
```

The port comes from the server's own startup banner. Trust the printed
address, not this example.

If the script fails, it says why and names the log to read. Read the tail of
that log, fix the named problem, and re-run the script.

## Verify it for yourself

Two checks, and they are the ones that would catch a partial setup:

```sh
curl -s http://127.0.0.1:8080/build        # one JSON line containing "version"
grep 'connected and registered' ~/.aion/assistant-worker.log
```

The first proves a real Aion server is answering — remember that the console's
fallback returns `200` for anything, so the JSON body is the evidence and the
status code is not. The second proves the worker is actually serving, rather
than merely running.

Then open the printed console address in a browser. You should see the ops
console: workflows, runs, history, and the authoring surface.

## Your first assistant session

In the console, pick the **assistant** workflow and start a run. It takes
exactly two inputs:

- **objective** — what you want, in your own words. It must not be blank.
- **repo_path** — a git repository for the assistant to work against, as a
  local path or a clone URL. Leaving it empty is fine, and means a scratch
  workspace.

The session is a conversation. Each message you send continues the *same*
assistant, with its full memory, for as long as you keep it open — days or
weeks. The session survives server restarts, because it is a durable workflow
like any other; that is the whole point. Ending it is an explicit action in
the console. There are no round limits.

## When something is wrong — where to look first

| Symptom | First look |
|---|---|
| `aion: command not found` after the install step | `~/.cargo/bin` is not on your `PATH` |
| The script stops at "Waiting for the server to answer" | tail `~/.aion/server.log` — the last error line names the cause |
| The worker "did not register within 30s" | `norn` missing or not signed in; tail `~/.aion/assistant-worker.log` |
| Console loads, but the assistant never answers | the worker log again — is it still running? `pgrep -fl agent-dev-worker` |
| The assistant session starts and then sits there, with a worker connected | the worker is serving a different task queue from the one the deployed document declares. The assistant has its own private queue (it is not `default`); `grep task_queue ~/.aion/assistant-worker.log` and compare with the `worker` line in `crates/aion-server/assistant-embed/assistant.awl`, or with `task_queue` from `aion assistant describe`. Stop the worker and re-run the script |
| "the server's workspace root is unresolvable" | the server booted but could not resolve `<AION_HOME>/clones`; the reason is in the banner and in `~/.aion/server.log` |
| The clone fails | network, not access — the repository is public |

The two logs are the whole diagnostic surface: `~/.aion/server.log` and
`~/.aion/assistant-worker.log`. Read them before changing anything.

---

Everything from here on is the manual path. It uses only published artifacts —
no repository checkout — and builds the same system a piece at a time so you
can see what each piece does. If Part 0 has already run, the CLI is installed
and a server is up; you can skip the next section and go straight to
[Part 1](#part-1--the-awl-path).

## Install the CLI

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

> Prefer a generated starting point? `aion new <name>` scaffolds a complete
> authoring project. The `awl` template is the default and emits a checked,
> runnable `<name>.awl`, a dev `aion.toml`, and a README. Templates are `awl`
> (default), `hello-world`, `approval-flow`, `saga`, `dev-pipeline`, and
> `agent`; the five Gleam templates retain their build manifests, schemas, and
> workflow descriptors. `--worker rust` adds a worker crate where supported,
> and `dev-pipeline` and `agent` require it.

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
[Part 2, step 3](#3-write-the-activity-worker) shows the Rust one, and the
[activities and workers guide](workers/activities-and-workers.md) covers failure
classification, heartbeats, and cancellation.

## Configure and run the server

The server reads a TOML config file. Create `aion.toml` in a fresh working
directory — the settings are explained in full in
[step 4 of Part 2](#4-configure-and-run-the-server):

```toml
[server]
listen_address = "127.0.0.1:8080"
grpc_address = "127.0.0.1:50051"

[runtime]
query_timeout_ms = 10000            # optional; defaults to 10000

[websocket]
event_broadcast_capacity = 1024     # optional; defaults to 256

[deploy]
enabled = true                      # REQUIRED for `aion deploy` / `aion run`
```

Only `[deploy] enabled` earns its place here: the deploy surface is dark by
default, so `aion deploy` and `aion run` have nothing to talk to without it.
Every other key above has a default and can be omitted (see
[`crates/aion-server/src/config/defaults.rs`](../crates/aion-server/src/config/defaults.rs));
they are spelled out so you can see what you are tuning. Set either tuning key
to `0` and startup fails naming it — a zero query deadline or a zero-capacity
broadcast channel is a misconfiguration, not a request for the default.

The store needs no configuration: the stock binary defaults to the haematite
backend, and its data directory defaults to `~/.aion/data`. Set `AION_HOME` to
put a server's whole state somewhere else — that is also how you run two
servers on one machine without them sharing state. See the
[operations guide](operations/operations.md#the-aion-home-aion-and-aion_home).

Start the server (terminal 1):

```sh
aion server --config aion.toml
```

In terminal 2, start the worker:

```sh
aion worker shell --manifest worker.toml --awl my_flow.awl
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
[errors reference](operations/errors.md) for every error code.

## Where to go from here in AWL

`my_flow.awl` used a fraction of the language. AWL also has children and
subflows, `fork`/`join`, multi-step `distribute`/`sequence` regions closed by
`collect`, bounded loops and visit-capped flow cycles, timed waits, `sleep`,
and `on failure` compensation.

- **[`docs/authoring/AWL.md`](authoring/AWL.md)** — every verb and the authoring loop.
- **[The language reference](../examples/assistant/resources/AWL-REFERENCE.md)**
  — the complete grammar, with the checked fixture that proves each construct.
- **[`docs/workers/DECLARED-COMMANDS.md`](workers/DECLARED-COMMANDS.md)** — read before writing
  a `run "…"` action body. **There is no sandbox.**
- **[Editor support](../editors/)** — `aion awl lsp` drives the Zed and Neovim
  extensions.

# Part 2 — the Gleam path

Gleam is the surface for workflows that need real typed code. The steps below
build the same shape as Part 1 — one activity, one signal, one query — in
Gleam, and package it into a versioned `.aion` archive.

## 1. Write the workflow

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
  // The final argument is the runner. With no `activity.execution_tier`
  // decorator the activity dispatches on the remote worker wire and the
  // runner is used only by the `aion/testing` harness — which is this
  // workflow. Decorate with `activity.execution_tier(_, activity.InVm)` and
  // the engine runs this same runner live, in a linked child process of the
  // workflow process, recording byte-identical history.
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
  // `codec.null_codec()` is the arguments codec for a query that takes none:
  // a caller supplying no arguments sends the JSON `null` document.
  case
    query.handler("status", codec.null_codec(), status_codec(), fn(_) { stage })
  {
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
[workflow authoring guide](authoring/workflows.md).

## 2. Describe and build the package

Add `workflow.toml` next to `gleam.toml`:

```toml
# Packaging descriptor read by `aion package`. Reference: docs/authoring/packaging.md.
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
[types-first codegen guide](authoring/codegen.md).

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
packaging reference is [`docs/authoring/packaging.md`](authoring/packaging.md).)

## 3. Write the activity worker

Activities run outside the workflow VM, in workers you write against a worker
SDK (Rust here; Python and TypeScript SDKs also exist under `sdks/`). The
Rust SDK is a **library** — `cargo install aion-worker` fails with
"no binaries". Scaffold your own crate:

```sh
cd ..
cargo new my-worker
cd my-worker
cargo add aion-worker@0.11
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
[activities and workers guide](workers/activities-and-workers.md).

## 4. Configure and run the server

The server reads a TOML config file. Two things in it are load-bearing:
`[deploy] enabled = true`, because the deploy surface is dark by default and
`aion deploy` has nothing to talk to without it; and `store.node_cache_budget`,
which haematite refuses to start without and which has no default anywhere —
the byte ceiling its node caches may hold is a deployment decision. (
`store.data_dir` is optional: omitted, it resolves under the Aion home, as
described above.) The tuning knobs all have
defaults and may be omitted — `runtime.query_timeout_ms` (10000),
`websocket.event_broadcast_capacity` (256), and, once deploy is enabled, both
size ceilings (64 MiB archive / 256 MiB inflated). They are written out here so
you can see what you are tuning. An explicit `0` is a different matter: it
fails startup with an error naming the key, because a zero deadline, a
zero-capacity channel, or a zero upload ceiling is a misconfiguration rather
than a request for the default
([`crates/aion-server/src/config/defaults.rs`](../crates/aion-server/src/config/defaults.rs)).

Create `aion.toml` in a fresh working directory:

```toml
[server]
listen_address = "127.0.0.1:8080"   # HTTP/JSON API + ops console
grpc_address = "127.0.0.1:50051"    # gRPC API + worker protocol

[store]
backend = "haematite"               # durable default; "memory" is development-only
data_dir = "aion-data"              # opened when present, created on first start
                                    # optional; defaults under the Aion home
node_cache_budget = "unlimited"     # REQUIRED, no default: the byte ceiling the
                                    # node caches may hold. Write a bound once you
                                    # know your box, e.g. { bytes = 1073741824 }

[runtime]
# Reply deadline for workflow queries, in milliseconds. Optional; 10000 when omitted.
query_timeout_ms = 10000

[websocket]
# Capacity of the global live-event broadcast channel. Optional; 256 when omitted.
event_broadcast_capacity = 1024

[deploy]
# Runtime package deploy/route/unload. Dark by default; when enabled both
# ceilings default to conservative limits (64 MiB archive / 256 MiB inflated),
# so you can omit them. When set, max_inflated_bytes must be >= max_archive_bytes.
enabled = true
max_archive_bytes = 16777216        # 16 MiB upload ceiling
max_inflated_bytes = 67108864       # 64 MiB decompressed-contents ceiling
```

Some keys can also be set by environment variable as
`AION_<SECTION>_<KEY>` (for example `AION_RUNTIME_QUERY_TIMEOUT_MS=10000`);
the supported variables are an explicit allowlist, spelled out with the
complete config reference in the
[operations guide](operations/operations.md#environment-overrides).

Start the server (terminal 1):

```sh
aion server --config aion.toml
```

You should see JSON log lines on stdout, including the HTTP and gRPC listen
addresses. Leave it running. In terminal 2, start the worker:

```sh
cargo run --manifest-path my-worker/Cargo.toml
```

## 5. Deploy and run the workflow

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

## 6. Prove the durability

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

## 7. Approve and complete

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
[errors reference](operations/errors.md) for every error code.

## 8. Versions, routing, rollback

Each deployed package version is identified by its content hash, and new
starts route to one version per workflow type:

```sh
aion versions                       # every loaded version, route flags
aion route my_flow <content-hash>   # roll back / forward to a loaded version
aion unload my_flow <content-hash>  # remove a non-routed, unpinned version
```

Running workflows keep executing the version they started on while new
starts use the routed version. The full deploy, versioning, and recovery
model is in the [operations guide](operations/operations.md).

## Where to go next

If you want a route rather than a list, read these four in this order. They
take you from the language, through the one capability that can hurt you, to
how the assistant you met in Part 0 is actually wired:

1. [`docs/authoring/AWL.md`](authoring/AWL.md) — AWL end to end: every verb and
   the authoring loop.
2. [Declared commands](workers/DECLARED-COMMANDS.md) — read this **before**
   writing your first `run "…"` action body. **There is no sandbox.**
3. [`examples/assistant/resources/`](../examples/assistant/resources/) — the
   assistant's own operating manual: its environment, authoring method,
   commands, the complete AWL language reference, and workers. What the
   assistant knows, you can read too.
4. [Agent workers](workers/AGENT-WORKERS.md) — how agent-backed workers like
   the assistant's are wired, including session pinning.

The rest, by subject:

- [`docs/authoring/AWL.md`](authoring/AWL.md) — the AWL surface: every verb, the authoring loop,
  and the full language reference.
- [Declared commands](workers/DECLARED-COMMANDS.md) — `run "…"` action bodies: what is
  guaranteed and what is not. **There is no sandbox.**
- [Workflow authoring guide](authoring/workflows.md) — the Gleam entry contract,
  determinism rules, timers, signals, queries, child workflows.
- [Activities and workers guide](workers/activities-and-workers.md) — worker
  scaffolding, failure classification, retry semantics as they actually are.
- [Types-first codegen guide](authoring/codegen.md) — derive JSON codecs and
  emit `schemas/*.json` from your authored Gleam types; `--check` in CI; the
  supported subset.
- [Operations guide](operations/operations.md) — full config reference, deploy
  surface, persistence and recovery, metrics.
- [Errors reference](operations/errors.md) — every error code, with hints.
- [`docs/operations/API.md`](operations/API.md) — HTTP/gRPC/WebSocket transports.
- [`docs/examples/order-saga.md`](examples/order-saga.md) — the flagship
  order-fulfillment saga: retries, timeout races, child workflows, and saga
  compensation in one walkthrough.
- [`examples/stacked-dev/`](../examples/stacked-dev/) — the composition
  showcase: a three-workflow family driving a real dev pipeline (concurrent
  activities, bounded verify-fix loops, a signal-driven review gate, typed
  exhaustion), runnable hermetically with `gleam test` or live against a
  server.
