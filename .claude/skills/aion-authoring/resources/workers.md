# Serving actions: the four paths, routing, and failure taxonomy

A `worker` block in a workflow document is a typed requirement — its name is
the task queue, each `action` a wire contract. Something real must serve it.
Pick the lightest path that fits; a hand-written worker is the last resort.

## Path 1 — declared bodies (`run "…"`)

The action carries its command in the workflow document; the **server** runs
it, and no worker serves that action. Returns the command's outcome record
`{ exit_code: Int, stdout: String, stderr: String }` (declare the return
type as a record naming any subset of those three).

**There is no sandbox.** The command runs with the server's own authority.
Read `docs/workers/DECLARED-COMMANDS.md` before writing one — it also covers
the `$` parameter-substitution rule (`$name` interpolates a declared
parameter; the value arrives as one whole argument, never through shell
expansion).

Workflow-document bodies never take a capture selector — `run text` and
`run json` belong to worker documents only.

## Path 2 — worker documents (`aion worker awl`)

A second `.awl` file opening `worker <queue>` instead of `workflow <name>`,
carrying a body for every action. The typed counterpart of the shell
manifest, and the only serving path whose wiring the language checks.

```awl
worker greeter
  action greet(name: String) -> Greeting
    run json "greeter-tool --json --name $name"
  action shout(text: String) -> String
    run text "tr a-z A-Z"

type Greeting { message: String }
```

Three captures decide what a **successful** command's result is (failure
classification is identical across all three):

| Body | Result | Declared return type |
|---|---|---|
| `run "…"` | The outcome record | Record naming a subset of `exit_code`/`stdout`/`stderr` |
| `run text "…"` | Trimmed stdout as one string | `String` |
| `run json "…"` | Trimmed stdout **parsed as JSON into your type** | Any declared type except `Nil` |

The checker enforces the declaration; whether the command's actual output
matches the declared type under `run json` is enforced at runtime by the
calling workflow's decoder — a mismatch fails that step loudly. Serve with
`aion worker awl FILE.awl --concurrency N --reconnect-…`. Full document:
`docs/workers/AWL-WORKER-DOCUMENTS.md`.

## Path 3 — the shell manifest (`aion worker shell`)

Host commands wired by a strict TOML manifest:
`aion worker shell --manifest worker.toml --awl FILE.awl`. See
`docs/workers/SHELL-WORKER-MANIFEST.md`.

## Path 4 — a real worker process

- `aion awl scaffold FILE.awl --out DIR` generates a compiling Rust worker
  crate per `worker` block; every advertised schema comes from the compiled
  contract, and `src/handlers.rs` is yours from the moment it exists.
- The **Python** and **Rust** SDKs (see the sibling resources).
- `aion worker agent FILE.awl --liminal-address ADDR --identity NAME` serves
  a compiled-in coding-agent harness with no crate at all — which harness and
  every launch setting come from the `worker` block's `harness` section
  (env allow-list via `env_pass`, working directory via `cwd from <param>`,
  extra argv via `args`, `concurrency`, reconnect backoffs). See
  `docs/workers/AGENT-WORKERS.md` and the `agent` action marker in the AWL
  resource. `~/.aion/norn_dev.awl` and `fleet_dev.awl` are live examples.

## The routing model — get this right first

The server routes a dispatched activity by exactly three dimensions:

    namespace × task_queue × node

**Never by activity type.** One process may hold several connections; each
connection registers ONE node and must serve every activity type its node
owns. Node strings are load-bearing: they must equal the node names the
workflow document declares (`node inspector`). Keep one authoritative table
and assert both sides against it — the reference worker
(`examples/dev-brief/worker/`) does exactly this.

## The failure taxonomy — the single most important discipline

- A command or handler that **cannot run at all** (missing executable, dead
  working directory, unreadable input) is an infrastructure failure →
  **terminal**. Retrying a broken environment cannot help.
- A command that **runs and exits non-zero** is a contract verdict →
  **recorded data returned as a successful result**, never an error. The
  exit status and output ride into durable history so the workflow can
  branch on them.
- Genuinely transient failures (busy volume, flaky socket) → **retryable**;
  the classification rides back to the workflow, which drives retries per
  the action's declared policy. Workers never retry on their own.

Nothing is ever swallowed into a success; nothing real is ever hidden inside
an error.

## Agent workers in one paragraph

An `agent` action runs a real coding agent as a driven session. Session
identity is the superpower: a session id templated on `{workflow_id}` plus
resume-if-exists means the same activity called twice in one workflow
resumes the same conversation — choose session keys deliberately, they
define which calls share memory. Every harness returns the canonical
four-field agent-outcome record (`text`, `final_message`, `stop_reason`,
`session_id`), and the checker demands that shape structurally. No secret is
ever read, stored, printed, or passed by a worker.
