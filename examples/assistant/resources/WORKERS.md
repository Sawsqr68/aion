# WORKERS.md — current worker layers and AWL queue contracts

An AWL workflow orchestrates durable work; workers perform world-touching
actions. Do not blur those responsibilities. A `worker` block declares a task
queue and the typed actions something must serve on it.

Two precise exceptions. First, an action in a WORKFLOW document MAY carry a
declarative `run "…"` body, and then that document is the action's
implementation — the server executes the command itself and no worker serves
it. Second, a `.awl` file may open with `worker <queue>` instead of `workflow
<name>`, and then the whole file is an implementation: a **worker document**,
every action carrying the command that runs it, served by `aion worker awl`.
Both are real capabilities with real consequences; read
`docs/workers/DECLARED-COMMANDS.md` before writing either, and
`docs/workers/AWL-WORKER-DOCUMENTS.md` for the worker-document family.
Everything else in a `worker` block is a typed requirement, not an
implementation.

## What can serve an action

| Layer | Purpose | Current truth |
|---|---|---|
| 0 — declared body | The document implements the action itself with `run "…"` | Shipped. The SERVER runs the command; no worker serves that action at all. Read `docs/workers/DECLARED-COMMANDS.md` first — there is no sandbox. |
| 0a — worker document | A second `.awl` file, opening `worker <queue>`, carrying a body for every action | Shipped as `aion worker awl FILE.awl`. The command runs on the WORKER's host, not the server's. Three body forms: `run "…"` (outcome record), `run text "…"` (stdout as a string), `run json "…"` (stdout decoded into the declared type). |
| 1 — command runner | Map actions to host commands, returning text or JSON | Shipped as `aion worker shell --manifest worker.toml --awl FILE.awl` |
| 1a — agent harness | Hand an action's prompt to an agent harness | Shipped as `aion worker agent FILE.awl` |
| 2 — script worker | One-file Python-style action host | Not built; do not promise a Python worker command |
| 3 — service worker | Substantial long-lived logic and integrations | Shipped through the Rust/Gleam worker surfaces and repository worker examples; `aion awl scaffold` generates a Rust one from the document. Python/TypeScript are not current built-in CLI surfaces |

All layers use the same task-queue dispatch model.

The executable CLI carries **three** `worker` subcommands, `shell`, `agent`
and `awl` — verify with `aion worker --help` rather than assuming either way.
Worker lifecycle management and Python script hosting, as described in
direction papers, remain roadmap unless the current source and `--help` show
otherwise.

## Layer 1a: the agent worker

`aion worker agent FILE.awl` serves the agent harness compiled into the binary
on the queue the document declares — an agent worker without writing a crate.
It is the counterpart to the `agent` marker on an action declaration:

```awl
worker assistants
  action ask(prompt: String) -> String
    agent, timeout 10m
```

The marker says the action's single `String` input is a prompt and its `String`
output is the agent's reply. That is the whole contract an adapter needs, and
it is why the document is the ONLY possible source of the advertised schemas:
an agent activity's output is decided by the agent, so there is no Rust type to
derive one from.

Facts to check before claiming this works in a session:

- The worker dials `--liminal-address` candidates (repeatable, for ordered
  redial), **not** the global gRPC `--endpoint`. The agent-harness seam exists
  only on the liminal worker transport.
- `--harness` selects among the adapters the binary was built with. The default
  build carries `norn` and `acp`, so the flag is REQUIRED; a build carrying
  exactly one derives it, and an unknown name is refused naming what the build
  does carry. A flag configuring a harness this invocation is not serving is
  refused rather than ignored.
- Connection settings (`--identity`, `--concurrency`, the three reconnect
  settings, and at least one `--liminal-address`) each come from a flag or a
  named environment variable, the flag wins, nothing is invented, and every
  missing one is named at once.
- The task queue is derived when the document declares exactly one `worker`
  block, and selected with `--task-queue` when it declares several. Serving is
  all-or-nothing: a queue is served whole or the verb refuses by name. An
  action WITH a `run` body is run by the server, so no worker serves it.
- `--check` prints the exact advertised action surface, the resolved harness,
  and the intervention controls that harness advertises, then exits without
  connecting and without demanding connection settings. Run it before
  reporting that an agent worker cannot serve a document — those descriptors
  are the whole admission decision on a contract-bearing queue.

Declaring `node agent` in AWL does NOT create an agent worker; `node` is a
dispatch label. The `agent` marker on the action declaration is the contract.

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
aion --endpoint HOST:PORT worker shell --manifest worker.toml --awl your_document.awl
```

Manifest facts:

- Unknown TOML fields are rejected.
- `worker.name`, `worker.task_queue`, and at least one action are required.
- `--awl` is required and must be the document the manifest was generated from.
  The manifest carries wiring only, so the document is the only source for the
  schemas the worker advertises — and a worker advertising nothing is refused by
  any queue carrying a deployed contract.
- A queue is served whole or not at all. An action in the manifest that the
  document does not declare, an action the document declares that the manifest
  does not wire, and an action whose body the document carries (the server runs
  those itself) are each refused by name before the worker connects.
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

## Layer 0: the declared command body

An action may implement itself, and then nothing serves it:

```awl
type RunOutcome { exit_code: Int, stdout: String, stderr: String }

worker greeter
  action greet(name: String) -> RunOutcome
    run "echo hello $name"
```

The result type must be a record over a SUBSET of the fixed outcome object
`{ exit_code: Int, stdout: String, stderr: String }`. Only the action's own
parameters interpolate, as `$name` or `${name}`, each substituted as one WHOLE
argv element that never re-enters a parser — so a value can never become code.
The program itself must be written out (`run "$program arg"` is refused), there
is exactly one `run` body per action, and there is no shell: `run "a | b"`
passes `|` and `b` as arguments. An author who wants a shell names one —
`run "sh -c 'a | b'"` — which is visible in review.

**Inside those single quotes everything is literal, the dollar included, so
there is no `$$` escape there.** `$$` is how you write a literal dollar in the
bare and double-quoted regions; inside single quotes write `$1` and `$d`
plainly. A parameter reaches the script by being passed as a word after it and
read positionally: `run "sh -c 'echo \"$1\"' -- $path"`. Writing `$$1` there is
not a syntax error and not a check failure — `sh` reads `$$` as its own process
id, so the command runs, exits 0, and reports success having done the wrong
thing. For a bodied action the exit status is not an oracle; check the effect
the command was written to have.

**`docs/workers/DECLARED-COMMANDS.md` is required reading before you write one.** In
summary: THERE IS NO SANDBOX. No allowlist, no denylist, no filesystem
confinement, no network restriction, no privilege drop, no resource limit. A
declared command can do anything the server's operating-system user can do.
What protects an operator is review, not runtime enforcement — the command is
source they can read before deploying, and it is sealed into the package's
worker contract so it cannot change afterwards without producing a different
package. What IS guaranteed: environment cleared except `PATH`, stdin closed,
cancellation kills the whole process group, a NUL byte in a value refused
rather than truncated, and a non-zero exit retryable by default while a
missing parameter or unspawnable program is terminal.

Never present a declared command as sandboxed, contained, or safe by
construction. Never propose one for work an operator has not agreed can run
with the server's full privileges.

## Layer 0a: the worker document

A `.awl` file may open with `worker <queue>` instead of `workflow <name>`, and
then the whole file is the implementation of that one queue:

```awl
//! Serve the `greeter` queue.
worker greeter
  action greet(name: String) -> Greeting
    run json "greeter-tool --json --name $name"
  action shout(text: String) -> String
    run text "tr a-z A-Z"

type Greeting { message: String }
```

`aion worker awl greeter.awl` serves it. The command runs on the WORKER's
host, not the server's — that is the difference from layer 0, and it is the
reason a worker document exists.

Facts to hold on to:

- **Every action must carry a body.** A bodyless action is a requirement, and
  requirements are declared in a workflow document; the checker refuses one
  here and says so.
- **Delivery keys are refused here.** `timeout`, `retry`, `node`, `advisory`
  and `agent` configure how the CALLER dispatches an action, so they belong to
  the action's declaration in the workflow document. Do not restate them.
- **A worker document runs no flow.** A `step`, `child`, `subflow`, `const`,
  or a second `worker` block is refused by name.
- **Three body forms**, and the capture decides only what a SUCCESSFUL command
  returns: `run "…"` the outcome record (subset return type), `run text "…"`
  trimmed stdout as a `String`, `run json "…"` stdout decoded into the
  declared type (`Nil` refused). The capture selectors are worker-document
  vocabulary: a workflow document, whose bodies the server runs, may declare
  only the bare form.
- **Admission is what keeps the two files honest.** The advertised surface is
  derived through the same seam the deploy path uses, and a worker document
  that has drifted from the workflow calling it is refused at registration,
  by field. `aion worker awl <file> --check` prints that surface without
  connecting.
- **THERE IS NO SANDBOX**, exactly as for layer 0. The environment IS cleared
  except `PATH` (unlike the manifest worker, which inherits everything).

Read `docs/workers/AWL-WORKER-DOCUMENTS.md` before writing one.

## Layer 2: not available

There is no shipped `aion worker python` script host in the current CLI. If the
requested logic needs a one-file script layer, report the gap. Do not fabricate
decorators, commands, or deployment behavior from design docs. A worker
document, a host-native shell manifest, or a declared command may be sufficient
only when a command line plus text/JSON stdout genuinely covers the task.

## Layer 3: service workers

Use a service worker for stateful logic, APIs, agents, rich validation, or
anything beyond command wiring. Current repository proofs are Rust worker
crates (for example `examples/awl-hello/worker`, `examples/dev-brief/worker`,
and `examples/general-worker`) and existing Gleam/Rust SDK surfaces.

Do not hand-copy the contract. Generate it:

```sh
aion awl scaffold FILE.awl --out ./workers
```

That writes one compiling Rust crate per `worker` block, at
`<out>/<queue>-worker`, with every advertised wire descriptor taken from the
contract the compiler returns — the same bytes the server deploys and admits
against. Deriving those schemas from the handlers' Rust types instead would be
a second, independent rendering of the same shapes, which is precisely the
drift the server's contract gate exists to catch. The generated crate opens one
connection per node, registers a descriptor with every handler, serves no
action that carries a `run` body, and fails every un-implemented activity
loudly. `src/handlers.rs` is yours from the moment it exists: re-running
rewrites the generated files and never that one.

When writing a worker by hand instead, copy the queue, action names, and JSON
shapes from the checked AWL declaration exactly.

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
  executable `WorkerCommand` in `crates/aion-cli/src/main.rs` (today: `Shell`,
  `Agent`, and `Awl`).
- Shell manifest and process behavior: `crates/aion-cli/src/worker_shell.rs`.
- Worker documents: `docs/workers/AWL-WORKER-DOCUMENTS.md`, with the family
  rules in `crates/aion-awl/src/checker/family.rs`, the compile seam in
  `crates/aion-awl/src/worker_document.rs`, the verb in
  `crates/aion-cli/src/worker_awl/`, and the accepted/refused corpus under
  `crates/aion-awl/tests/fixtures/rev2/worker-documents/`.
- Agent worker flags, harness resolution, and the `--check` surface:
  `crates/aion-cli/src/worker_agent/` and `crates/aion-cli/src/harness/`.
- Declared command bodies: `docs/workers/DECLARED-COMMANDS.md` (the boundaries, in
  full), with authoring-time checks in `crates/aion-awl/src/checker/bodies.rs`
  and the command scanner in `crates/aion-awl/src/ast/command.rs`.
- The `agent` marker's rules: `crates/aion-awl/src/checker/agents.rs`.
- Queue declaration grammar: checker-valid worker fixtures under
  `crates/aion-awl/tests/fixtures/rev2/declarations/valid/` — including
  `action_run_bodies.awl`, `agent_seam.awl`, and `advisory_side_channel.awl`.
- Host-native law: beamr 0.16.2
  `src/native/system_info_bifs.rs`, where `bif_open_port_2` returns `badarg`.
