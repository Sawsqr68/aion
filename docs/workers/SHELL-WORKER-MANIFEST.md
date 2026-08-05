# The shell worker manifest (`worker.toml`)

The built-in shell worker maps declared AWL actions to host commands, with no
Rust crate and no source code of your own:

```sh
aion --endpoint http://127.0.0.1:50051 worker shell \
    --manifest worker.toml --awl my_flow.awl
```

Both `--manifest` and `--awl` are required, and so in practice is
`--endpoint` — see below. The manifest carries **wiring only** — which queue
to serve, and which command each action runs. Everything typed stays in the `.awl`
document: input/output schemas, `timeout`, and `retry` are the document's, the
worker advertises the document's own schemas verbatim, and output shape is
enforced by the workflow decoder exactly as for every other worker. A manifest
cannot widen, narrow, or restate a contract; it can only wire one.

The worker dials the server's gRPC endpoint from the global `--endpoint` flag,
and **that endpoint must carry a scheme**:

```sh
aion --endpoint http://127.0.0.1:50051 worker shell --manifest worker.toml --awl my_flow.awl
```

The global flag's fallback value is the bare `127.0.0.1:50051`, and this verb
passes it to the worker SDK verbatim — the client verbs prepend `http://` to a
scheme-less endpoint (`normalize_endpoint`), and this one does not
(`crates/aion-cli/src/main.rs`). A worker started without `--endpoint`, or
with a scheme-less one, therefore never connects; and because the verb
installs no tracing subscriber, the redial driver's failures go nowhere and
the process just sits there. **Silence from `aion worker shell` is not
success.** Confirm the worker actually arrived — `POST
/awl/workers/availability {"namespace":"…","task_queue":"…","activity_types":[…]}`
answers `connected_workers` — before concluding it is serving.

Ground truth for everything below: `crates/aion-cli/src/worker_shell.rs`.

## Parsing rules

- **Strict TOML.** Unknown keys are rejected at every level — a typo is a
  startup refusal, never a silently ignored field.
- The manifest is read once, at worker startup. Editing it does nothing until
  the worker restarts.
- Exactly one `[worker]` table and at least one `[[action]]` entry are
  required. Action names must be unique.

## `[worker]` — required, exactly one

| Field | Required | Type | Meaning |
|---|---|---|---|
| `name` | required | string, non-empty | Names this worker. The connection identity the server sees is `<name>-shell-worker`. |
| `task_queue` | required | string, non-empty | The queue to serve. Must equal a `worker <name>` block declared in the `.awl` document — the block name **is** the queue. A queue the document does not declare is refused at startup, naming both what was wired and what the document declares. |

## `[[action]]` — required, one entry per wired action

| Field | Required | Type | Meaning |
|---|---|---|---|
| `name` | required | string, non-empty, unique | The declared action this entry serves. It must exist on the wired queue in the document, and must **not** carry a declared `run` body — a bodied action is the server's to run, and wiring it is refused. |
| `command` | required | array of strings, non-empty | The argv to execute: element 0 is the program, the rest are its arguments. Executed directly — **never through a shell** — so shell syntax (`\|`, `>`, `$VAR`) is passed literally, not interpreted. Every element, **including the program**, may contain placeholders (see below). |
| `env` | optional (default: none) | table of string → string | Environment variables set on the spawned process. Values may contain placeholders. These are **added to** the worker's own environment, which the child inherits in full — see the security section. |
| `result` | required | `"text"` or `"json"` | How stdout becomes the activity result. `"text"`: trimmed stdout is returned as a JSON string. `"json"`: trimmed stdout must parse as JSON and is returned as that value; invalid JSON is a **terminal** activity failure. Either way the value must match the document's declared output type — the workflow decoder enforces it. |

There is deliberately no `timeout`, no `retry`, and no `cwd` field. Timeout
and retry belong to the action's declaration in the `.awl`; the command runs
in whatever working directory the worker was started in.

A declared `retry` really does govern re-dispatch — the engine's retry loop
consumes it (see
[activities-and-workers.md](activities-and-workers.md#retry-semantics)). A
declared `timeout` does **not** stop a running command: it travels to the
engine in the dispatch config and nothing reads it today. Bound a command that
must not run forever inside the command, with the program's own timeout.

### Placeholders

Command elements and `env` values may project the activity input:

- `{input}` — the whole input, serialized as one compact JSON string.
- `{input.field}` — one **top-level** field of the input. Field names may use
  letters, digits, and `_` only. The value must be a scalar (string, number,
  or boolean); a null, array, object, or absent field is refused at dispatch
  time as a terminal failure — never guessed at.

A value is substituted as one whole argv element (or one whole environment
value) and never re-enters a parser. Anything else brace-shaped — nested paths,
unknown names, an unterminated `{`, a stray `}` — is refused when the manifest
is parsed. There is no escape syntax: a literal `{` or `}` cannot appear in a
command element.

### Serving is all-or-nothing

The manifest must wire **every** workerless action the document declares on the
queue. A partially wired queue is refused at startup: an admitted worker would
otherwise be dispatched an action it cannot serve. Wiring an action the
document does not declare is likewise refused, by name.

## A complete worked example

For this document:

```awl
workflow my_flow
  input name: String
  outcome greeted: type Greeting, route success

type Greeting { message: String }

worker my_flow
  action greet(name: String) -> String
    timeout 30s, retry 3 every 10s
  action lookup(name: String) -> Greeting
    timeout 30s
```

this manifest serves the whole `my_flow` queue:

```toml
[worker]
name = "my-flow-worker"
task_queue = "my_flow"          # the `worker my_flow` block name IS the queue

[[action]]
name = "greet"
command = ["printf", "Hello, %s!", "{input.name}"]
result = "text"                 # trimmed stdout becomes the JSON string result

[[action]]
name = "lookup"
command = ["/usr/local/bin/lookup-person", "--name", "{input.name}"]
env = { LOOKUP_MODE = "strict" }
result = "json"                 # stdout must be JSON matching `Greeting`
```

## Failure and cancellation semantics

- Each command runs in its **own process group**. Cancelling the activity
  terminates the whole group — `SIGTERM`, a two-second grace period, then
  `SIGKILL` — so a command that spawned children does not leave them running.
  A cancellation is a **terminal** failure.
- A **non-zero exit** is a **retryable** failure carrying the command's own
  stderr; the document's declared `retry` policy decides whether another
  attempt is scheduled. So is a program that could not be spawned (missing
  binary, permissions) — `ProcessGroupError::Spawn` is classified retryable
  here. Note the asymmetry with a declared `run` body, whose spawn failure is
  **terminal** in every arm (`crates/aion-worker/src/shell/action.rs`,
  `spawn_failure`) — do not carry an expectation from
  [`DECLARED-COMMANDS.md`](DECLARED-COMMANDS.md) across to this worker.
- A process-containment failure, and stdout that is not valid JSON under
  `result = "json"`, are **terminal**: retrying cannot change them.
- stdout and stderr are read as UTF-8 (lossily, invalid bytes replaced) and
  trimmed of surrounding whitespace.
- The worker serves up to four activities concurrently and redials the server
  indefinitely on disconnect (backoff 100 ms doubling to a 5 s ceiling).

## THERE IS NO SANDBOX

The commands a manifest wires are **host commands**, exactly as
[`DECLARED-COMMANDS.md`](DECLARED-COMMANDS.md) describes for `run` bodies —
read that document's security section in full, because most of it applies
here unchanged. There is no allowlist, no denylist, no filesystem confinement,
no network restriction, no privilege drop, and no resource limit — no CPU,
memory, file-descriptor, or wall-clock cap. Cancellation is the one thing that
stops a running command from outside it, and a declared `timeout` is not (see
the `[[action]]` note above). A wired command can do anything the
operating-system user who ran `aion worker shell` can do.

The value-substitution guarantee does hold: a parameter value becomes exactly
one argv element and never passes through a shell, so nothing inside a value
can execute. But the shell worker is **more permissive than a declared body**
in three ways, and they are worth stating plainly:

1. **A caller CAN choose which program runs — if you let them.** Unlike a
   declared body, the program element of `command` accepts placeholders:
   `command = ["{input.tool}"]` is legal, and it hands the workflow's caller
   the choice of binary. Keep the program literal unless that is precisely
   what you mean.
2. **The environment is inherited, not cleared.** A declared body's command
   starts with a scrubbed environment; a shell-worker command inherits the
   worker process's environment **in full** — credentials included — plus the
   manifest's `env` entries. `command = ["env"]` prints whatever the worker
   was started with. Start the worker with a clean environment if its commands
   must not see one.
3. **The manifest is not sealed.** A declared body's command text is committed
   into the package's content hash; the manifest is a local file read at
   startup. Whoever can edit `worker.toml` chooses what runs on the next
   restart, with no change to any deployed artifact. Protect it like the
   scripts it is.

What protects you is review of the manifest and control of the host it runs
on, not runtime enforcement. If you need containment, run the worker as a
constrained user, in a container, or on a host that has nothing else on it.
