---
name: aion-authoring
description: Author Aion workflows and workers — the AWL language, the four ways an action gets served, worker documents, agent workers, and the Python and Rust worker SDKs. Use when writing, checking, deploying, or serving any .awl document or Aion worker.
---

# Authoring Aion workflows and workers

Aion is a durable workflow engine: a workflow can crash on step nine of ten
and resume exactly where it was, or sleep for three months and wake in the
same logical state. The mechanism is event-sourcing plus deterministic
replay — workflow code re-executes from the start on recovery, and every
side-effecting call returns its recorded result from history instead of
acting again. You never write recovery code; you write the workflow, and
durability is the engine's job.

Two consequences shape everything you author:

1. **Workflow code must be deterministic.** Side effects live in
   **activities** (AWL: `action`s served by workers or declared commands).
   The workflow's own expressions — guards, maps, filters — must compute the
   same answer on replay.
2. **Every action is a typed wire contract.** Inputs and outputs are JSON
   checked against declared types; drift between a document and its worker is
   a loud decode failure, never silent coercion.

## The five-minute path

```sh
aion new my_flow          # scaffold a checked, runnable AWL document
aion awl check my_flow/my_flow.awl        # parse + typecheck, until clean
aion awl fmt my_flow/my_flow.awl          # canonical formatting (in place)
aion run my_flow/my_flow.awl --input '{"name":"Ada"}'   # compile, deploy, start, await
```

`aion run` validates `--input` against the derived start contract locally
before deploying, reports `workflow_id`, `run_id`, and the decoded result,
and reports failure/cancel/timeout as an error, never as success. Use
`aion deploy FILE.awl` to deploy without running.

## A whole workflow in one file

One `.awl` document declares a whole workflow: its types, the workers and
actions it requires, and the step graph that runs them. No build step, no
manifest, no codegen.

```awl
//! Greet a name, then shout it.
workflow awl_hello
  input name: String
  outcome shouted: type Shouted, route success

type Greeting { greeting: String }
type Shouted  { text: String }

worker awl_hello
  action greet(name: String) -> Greeting
  action shout(text: String) -> Shouted

step greet_and_shout
  name |> greet |> .greeting |> shout |> route shouted
```

Read [resources/awl-workflows.md](resources/awl-workflows.md) for the
language — document anatomy, the full word map, and the worked patterns
(selection, parallelism, loops, signals, queries, failure handling).

## A `worker` block is a requirement, not an implementation

The block's name is a **task queue**; each `action` is a wire contract
something real must serve. A clean `aion awl check` proves the document is
internally consistent — it does **not** prove a worker exists. Four things
can serve an action:

| Path | When | One line |
|---|---|---|
| Declared body: `run "…"` | Server-side commands | The server runs the command itself. **No sandbox — read `docs/workers/DECLARED-COMMANDS.md` first.** |
| Worker document | Host commands, typed | A second `.awl` opening `worker <queue>`; each action carries its command. `run json "…"` parses stdout into your declared type. Served by `aion worker awl`. |
| Shell manifest | Host commands, TOML-wired | `aion worker shell --manifest worker.toml --awl FILE.awl` |
| Worker process / SDK | Real logic, agents | `aion awl scaffold` generates a Rust crate; or the Python/Rust SDKs; or `aion worker agent` for a coding-agent harness with no crate at all. |

Read [resources/workers.md](resources/workers.md) for all four paths, the
routing model (namespace × task_queue × node — **never** by activity type),
the failure taxonomy, and agent workers.

## SDK workers

- **Python** — decorated handlers on dataclasses, served by
  `aion-worker script.py …`. Read [resources/python-sdk.md](resources/python-sdk.md).
- **Rust** — a standalone crate on the `aion-worker` library.
  Read [resources/rust-sdk.md](resources/rust-sdk.md).

Both derive the advertised action schemas from your type annotations, and
the server admits or refuses the worker against the deployed contract —
shape drift is caught at registration, not mid-run.

## The server side

A stock server has **no port for a worker to dial** until the outbox is
commissioned — the two configuration bites and every CLI verb (local
checking, deploy/run, operations) live in
[resources/cli-and-server.md](resources/cli-and-server.md).

## Where the deeper truth lives (in this repository)

The skill summarizes; these are canonical and cannot drift from the code:

- `aion awl guide` — every word of the language, one sentence each;
  `aion awl guide <word>` adds a complete checked example;
  `aion awl guide --reference` prints the full embedded reference.
  The same table drives editor hover and the ops console, so all surfaces agree.
- `examples/assistant/resources/AWL-REFERENCE.md` — the full language reference.
- `examples/assistant/resources/AWL-AUTHORING.md` — objective → checked document.
- `examples/assistant/resources/WORKERS.md` — what actually serves an action.
- `examples/assistant/resources/COMMANDS.md` — the CLI loop with honest failure modes.
- `docs/workers/DECLARED-COMMANDS.md` — **before any `run "…"` body. No sandbox.**
- `docs/workers/AWL-WORKER-DOCUMENTS.md` — worker documents and the three captures.
- `docs/workers/AGENT-WORKERS.md` — agent harnesses, `harness` sections, setup.
- `docs/workers/WORKER-AUTHORING.md` + `examples/dev-brief/worker/` — the
  hand-written Rust worker reference implementation.
- `docs/GETTING-STARTED.md` — the full operator tutorial.
