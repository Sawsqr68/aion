# AWL — the Aion workflow language

AWL is Aion's own workflow language and one of its two first-class authoring
surfaces (the other is [typed Gleam](authoring.md)). One `.awl` document
declares a whole workflow: its types, the workers and actions it requires, and
the step graph that runs them. Aion compiles it directly — **no Gleam build, no
`workflow.toml`, no `aion generate`, no `aion package` step.**

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

> **Released versus this repository.** This page describes AWL as it stands on
> `main`. Several surfaces below have **not been released** and are not in the
> published `aion` crate (0.10.0): declared `run "…"` command bodies, the
> `agent` action marker, in-language `send`, `query`/`answer`, `aion awl
> scaffold`, `aion worker agent`, and `aion run --timeout`. On the published
> binary, `aion awl check` rejects the first four with a parse error and the
> last three do not appear in `--help`. If you need them, build from this
> checkout. Everything else here works on both.

## Where the reference lives

The complete language reference — every construct the parser and checker
accept, with the fixture that proves each one — is
**[`examples/assistant/resources/AWL-REFERENCE.md`](../examples/assistant/resources/AWL-REFERENCE.md)**.
It is filed there because it is also the reference the bundled assistant is
given, and one copy that cannot drift beats two that can.

Alongside it, and worth reading in the same sitting:

| Document | Why |
|---|---|
| [`AWL-AUTHORING.md`](../examples/assistant/resources/AWL-AUTHORING.md) | The practical path from an objective to a checked document. |
| [`WORKERS.md`](../examples/assistant/resources/WORKERS.md) | What a `worker` block declares, and what actually serves it. |
| [`COMMANDS.md`](../examples/assistant/resources/COMMANDS.md) | Every CLI command in the AWL loop, with its honest failure modes. |
| [`DECLARED-COMMANDS.md`](DECLARED-COMMANDS.md) | **Read before writing a `run "…"` body. There is no sandbox.** |

## The verbs

### Local — these never contact a server

| Command | What it does |
|---|---|
| `aion awl check <FILE.awl>` | Parses and typechecks. Prints `ok: <file> (N steps)`, or `<file>:<line>:<column>: error: <message>` on stderr and exits non-zero. |
| `aion awl fmt <FILE.awl>` | Rewrites the file in place with the canonical printer. The printer *is* the formatter — there is no `--check` mode. A parse error leaves the file untouched. |
| `aion awl schema <FILE.awl> [--type <T>] [--queries]` | Derives JSON Schema (draft 2020-12). No flag: the workflow's start contract. `--type`: one declared type. `--queries`: the read-only surface — every declared `query` with the schema of the answer an operator receives. |
| `aion awl scaffold <FILE.awl> --out <DIR> [--worker <name>] [--aion-crates <DIR>]` | Generates a compiling Rust worker crate per `worker` block, at `<out>/<queue>-worker`. Every advertised wire descriptor comes from the compiled contract, not from the handlers' Rust types. `src/handlers.rs` is yours from the moment it exists — re-running never rewrites it. |
| `aion awl emit <FILE.awl> [--output <P>] [--target gleam\|beam]` | Emits an executable artifact. `gleam` (default) writes Gleam source to `--output` or stdout; `beam` compiles direct BEAM bytecode through the same seam the deploy path uses, **requires** `--output`, and writes a `<output>.json` sidecar. Optional in the normal loop. |
| `aion awl lsp` | The AWL language server over stdio, for editor integration. |

### Against a running server

| Command | What it does |
|---|---|
| `aion deploy <FILE.awl>` | Reads, checks, direct-compiles, assembles, and deploys. Requires the server's `[deploy]` surface and the caller's deploy grant. |
| `aion run <FILE.awl> --input '<json>' [--timeout <dur>]` | Compile, deploy, start, and await in one motion. |
| `aion worker shell --manifest worker.toml --awl <FILE.awl>` | Serves manifest-wired host commands on the declared queue. |
| `aion worker agent <FILE.awl> --harness <name> --liminal-address <addr>` | Serves a compiled-in agent harness on the queue the document declares. |

`aion list`, `describe`, `inspect`, `signal`, `query`, `pause`, `resume`,
`cancel`, `reopen`, and `versions` operate the resulting runs; see the
[operations guide](guides/operations.md).

## The authoring loop

```sh
aion awl check my_flow.awl                      # 1. until it is clean
aion awl fmt my_flow.awl                        # 2. canonicalise, check again
aion run my_flow.awl --input '{"name":"Ada"}'   # 3. compile, deploy, start, await
```

`aion run` validates `--input` locally against the derived start contract
*before* it opens the deploy connection, so a malformed input fails at your
desk rather than half-way through a deploy. Success reports `workflow_type`,
`workflow_id`, `run_id`, and the decoded result; a workflow failure,
cancellation, timeout, or continue-as-new is reported as an error, not as
success.

Use `aion deploy` instead when you want the document deployed but not yet run.

### `--timeout`

`aion run --timeout` takes an AWL duration literal — `30s`, `5m`, `3h`, `2d` —
parsed by the same lexer the language uses, so the flag and a document's own
`timeout` declaration share one vocabulary.

It bounds **the wait, and only the wait.** Reaching it exits 1 with the
workflow id, run id, and a describe line; **the run continues on the server.**
To stop a run, cancel it.

There is no default. Absent the flag, the wait is bounded only by the
document's own `timeout` declaration, which the engine enforces as a real
deadline. A document that declares no `timeout`, run without `--timeout`,
waits indefinitely.

## What a `worker` block means

A `worker` block is a **typed requirement, not an implementation**. Its name is
the task queue; each `action` is a wire contract a real worker on that queue
must serve.

```awl
worker review
  action inspect(repo: String) -> Report
    node reviewer, timeout 10m, retry 3 backoff 5s..1m
```

A clean `aion awl check` proves the document is internally consistent. It does
**not** prove a worker exists. Three things can serve an action — see
[`WORKERS.md`](../examples/assistant/resources/WORKERS.md):

1. **A declared body** (`run "…"`) — the server runs the command itself and no
   worker serves that action. Read
   [`DECLARED-COMMANDS.md`](DECLARED-COMMANDS.md) first: **there is no
   sandbox.**
2. **`aion worker shell`** — host commands wired by a strict manifest.
3. **A real worker process** — `aion awl scaffold` generates a Rust one, or
   use any worker SDK. `aion worker agent` serves an agent harness without a
   crate at all.

## Deployed packages carry their source

A package compiled from a document archives the document with it, under
`awl/document/<filename>` plus `awl/schema/<path>` for each `schema("…")`
import — so a deployed workflow can show you exactly the source it was built
from, and the ops console does, read-only. Archived source is **provenance
only**: it is excluded from the package content hash, so carrying it can never
re-version a deployed workflow. See [`packaging.md`](packaging.md).

## Editors

`aion awl lsp` speaks LSP over stdio: diagnostics from the real checker, plus
completion over the real keyword inventory. The extensions under
[`editors/`](../editors/) wire it up.
