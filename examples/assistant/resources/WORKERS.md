# WORKERS.md — current worker layers and AWL queue contracts

An AWL workflow orchestrates durable work; workers perform world-touching
actions. Do not blur those responsibilities. The workflow's `worker` block is
a typed requirement, not an implementation and not an AWL-authored worker.

## The three layers, with current status

| Layer | Purpose | Current truth |
|---|---|---|
| 1 — command runner | Map actions to host commands, returning text or JSON | Shipped as `aion worker shell --manifest worker.toml` |
| 2 — script worker | One-file Python-style action host | Not built; do not promise a Python worker command or AWL worker file |
| 3 — service worker | Substantial long-lived logic and integrations | Shipped through the Rust/Gleam worker surfaces and repository worker examples; Python/TypeScript are not current built-in surfaces |

All layers use the same task-queue dispatch model. Agent/Norn workers in the
repository are specialized external service workers; declaring an `agent` node
in AWL does not create one.

The executable CLI is even narrower than the taxonomy: its built-in `aion
worker` subcommand currently has only `shell`. Worker lifecycle, Python script
hosting, ACP batteries, and AWL worker files described in direction papers are
roadmap unless the current source and `aion worker --help` show otherwise.

## What an AWL workflow declares

```awl
worker general
  action run_command(repo: String, argv: [String]) -> CommandResult
    node shell, timeout 10m, retry 3 every 30s
```

- `general` is the task queue.
- `run_command` is the action type a worker on that queue must register.
- Named parameters and the return type are the wire contract checked at calls.
- `node` pins dispatch to a worker node label; it does not select an execution
  language or instantiate a worker.
- Timeout and retry belong to AWL. Worker wiring must not duplicate them as a
  second source of truth.

A clean `aion awl check` proves the workflow is internally typed. It does not
prove a matching worker process is connected. A run that reaches an action
needs a real worker on the declared queue serving that action.

## Layer 1: host-native shell worker

The current shell worker is a Rust host process. Its implementation is
`crates/aion-cli/src/worker_shell.rs` and uses `tokio::process::Command` to
spawn commands. A strict manifest contains wiring only:

```toml
[worker]
name = "greeter"
task_queue = "greeter"

[[action]]
name = "greet"
command = ["printf", "%s", "{input.name}"]
result = "text"
```

Run it only against an explicitly selected server:

```sh
aion --endpoint HOST:PORT worker shell --manifest worker.toml
```

Manifest facts:

- Unknown TOML fields are rejected.
- `worker.name`, `worker.task_queue`, and at least one action are required.
- Action names must be unique and command arrays nonempty.
- `command` is an argv array; it is not interpreted by an implicit shell.
- `{input}` expands to the complete JSON input. `{input.field}` projects one
  top-level string/number/bool field into an argument or environment value.
  Arrays, objects, and absent/null fields cannot be projected as scalars.
- `result = "text"` returns trimmed stdout as a JSON string.
  `result = "json"` requires stdout to be valid JSON.
- A nonzero command exit is a retryable activity failure; invalid result JSON
  is terminal. AWL's declared retry policy decides whether another attempt is
  scheduled.
- The current process sets max concurrency to four. Do not present that
  implementation detail as an author-configurable manifest field.

The console can scaffold this wiring from a checked AWL contract. The types,
timeout, and retry still live in `.awl`; `worker.toml` is host command wiring.

## The shell-worker law: host-native, never beamr ports

Shell workers must remain host-native. beamr 0.16.2 has no port subsystem:
`erlang:open_port/2` is deliberately unsupported and returns `badarg`
unconditionally, `erlang:ports/0` is empty, and `erlang:port_info/1` has no
port metadata. Therefore:

- Do not implement shell actions inside workflow BEAM code.
- Do not emit or recommend Erlang/Gleam `open_port` as a command runner.
- Do not claim direct-compiled AWL can spawn a process.
- Use the host-native `aion worker shell` process or a native service worker.

This is a capability boundary, not a temporary error to catch or retry.

## Layer 2: not available

There is no shipped `aion worker python` script host in the current CLI and no
accepted `worker ...` AWL file kind with action bodies. If the requested logic
needs a one-file script layer, report the gap. Do not fabricate decorators,
commands, deployment behavior, or an AWL command vocabulary from design docs.
A host-native shell manifest may be sufficient only when strict argv/env
projection plus text/JSON stdout genuinely covers the task.

## Layer 3: service workers

Use a service worker for stateful logic, APIs, agents, rich validation, or
anything beyond command wiring. Current repository proofs are Rust worker
crates (for example `examples/awl-hello/worker`, `examples/dev-brief/worker`,
and `examples/general-worker`) and existing Gleam/Rust SDK surfaces. Copy the
queue, action names, and JSON shapes from the checked AWL declaration exactly.

Do not teach the retired Gleam `aion_flow` workflow package as authoring truth.
A Gleam worker is an external action implementation; the orchestration remains
the `.awl` document.

## Diagnosis checklist

When a checked workflow stalls at a call:

1. Read the AWL `worker` name: that is the required task queue.
2. Read the action declaration and exact input/output types.
3. Confirm, in the ops console or actual worker logs, that a worker is
   connected on that queue and registered that action.
4. Confirm the worker's output JSON matches the AWL return type.
5. Inspect durable history before deciding whether the activity is pending,
   retrying, failed, or waiting elsewhere.

Do not start/restart workers or servers without operator authority, and do not
claim a worker exists because its interface is declared.

## Verification sources

- Taxonomy/status: `docs/design/WORKER-AUTHORING-STORY.md`, checked against the
  executable `WorkerCommand` in `crates/aion-cli/src/main.rs`.
- Shell manifest and process behavior: `crates/aion-cli/src/worker_shell.rs`.
- Queue declaration grammar: checker-valid worker fixtures under
  `crates/aion-awl/tests/fixtures/rev2/declarations/valid/`.
- Host-native law: beamr 0.16.2
  `src/native/system_info_bifs.rs`, where `bif_open_port_2` returns `badarg`.
