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

## Where the reference lives

The complete language reference — every construct the parser and checker
accept, with the fixture that proves each one — is
**[`examples/assistant/resources/AWL-REFERENCE.md`](../../examples/assistant/resources/AWL-REFERENCE.md)**.
It is filed there because it is also the reference the bundled assistant is
given, and one copy that cannot drift beats two that can.

Alongside it, and worth reading in the same sitting:

| Document | Why |
|---|---|
| [`AWL-AUTHORING.md`](../../examples/assistant/resources/AWL-AUTHORING.md) | The practical path from an objective to a checked document. |
| [`WORKERS.md`](../../examples/assistant/resources/WORKERS.md) | What a `worker` block declares, and what actually serves it. |
| [`COMMANDS.md`](../../examples/assistant/resources/COMMANDS.md) | Every CLI command in the AWL loop, with its honest failure modes. |
| [`DECLARED-COMMANDS.md`](../workers/DECLARED-COMMANDS.md) | **Read before writing a `run "…"` body. There is no sandbox.** |

## Reaching the guide from wherever you are

You should not have to leave what you are doing to find out what a word
means. Every authoring surface answers, and every one of them answers with the
*same sentence*, because they all render one table — `aion_awl::guide`, which
carries one sentence per word of the language and the reference section that
explains it in full.

| Where you are | How you ask |
|---|---|
| The terminal | `aion awl guide` for every word, `aion awl guide <word>` for one — with its complete checked example documents. `aion awl guide --reference` prints the full language reference, embedded in the binary, so no checkout is needed. `aion awl --help` names each document above by its real path. |
| Your editor | Hover any reserved word. The tooltip carries the sentence, every checked example document, and the reference citation, and it answers even while the document is mid-edit and does not parse. Completion carries the same sentence with the first example on every offered word. |
| The ops console | The authoring view's **Language** panel lists every word with its examples, grouped by reference section, and searchable. On the Workspace and Deployed surfaces the same guide floats as a non-modal pop-over — toggle **Language guide** in the corner to keep definitions and examples beside the editor while you type. Both read a checked-in copy of the same table, so they work with no server reachable. |

That the four renderings agree is not maintained by care. `aion-awl`'s
glossary is an exhaustive match over the keyword inventory, so a word added to
the language fails the build until someone writes what it means;
`crates/aion-awl/tests/guide_reference_gate.rs` refuses any citation the
reference cannot answer; and `crates/aion-cli/tests/console_guide_asset.rs`
refuses any difference between the console's copy and what
`aion awl guide --json` renders today. Regenerate that copy with:

```sh
aion awl guide --json > apps/aion-ops-console/src/features/authoring/assets/awl-guide.json
```

## The verbs

### Local — these never contact a server

| Command | What it does |
|---|---|
| `aion awl check <FILE.awl>` | Parses and typechecks. Prints `ok: <file> (N steps)`, or `<file>:<line>:<column>: error: <message>` on stderr and exits non-zero. |
| `aion awl fmt <FILE.awl>` | Rewrites the file in place with the canonical printer. The printer *is* the formatter — there is no `--check` mode. A parse error leaves the file untouched. |
| `aion awl schema <FILE.awl> [--type <T>] [--queries]` | Derives JSON Schema (draft 2020-12). No flag: the workflow's start contract. `--type`: one declared type. `--queries`: the read-only surface — every declared `query` with the schema of the answer an operator receives. |
| `aion awl scaffold <FILE.awl> --out <DIR> [--worker <name>] [--aion-crates <DIR>]` | Generates a compiling Rust worker crate per `worker` block, at `<out>/<queue>-worker`. Every advertised wire descriptor comes from the compiled contract, not from the handlers' Rust types. `src/handlers.rs` is yours from the moment it exists — re-running never rewrites it. |
| `aion awl emit <FILE.awl> [--output <P>] [--target gleam\|beam]` | Emits an executable artifact. `gleam` (default) writes Gleam source to `--output` or stdout; `beam` compiles direct BEAM bytecode through the same seam the deploy path uses, **requires** `--output`, and writes a `<output>.json` sidecar. Optional in the normal loop. |
| `aion awl guide` | Prints every word of the language with one sentence of meaning and its reference citation, grouped by section. `aion awl guide <word>` prints one, with its complete checked example documents; `--reference` prints the full embedded language reference; `--json` emits the table the ops console renders. An unknown word exits non-zero rather than printing an empty answer. |
| `aion awl lsp` | The AWL language server over stdio, for editor integration. |

### Against a running server

| Command | What it does |
|---|---|
| `aion deploy <FILE.awl>` | Reads, checks, direct-compiles, assembles, and deploys. Requires the server's `[deploy]` surface and the caller's deploy grant. |
| `aion run <FILE.awl> --input '<json>' [--timeout <dur>]` | Compile, deploy, start, and await in one motion. |
| `aion worker awl <FILE.awl>` | Serves the queue a **worker document** declares — a `.awl` file opening with `worker <queue>` instead of `workflow <name>`, whose actions each carry the command that runs them. See [`AWL-WORKER-DOCUMENTS.md`](../workers/AWL-WORKER-DOCUMENTS.md). |
| `aion worker shell --manifest worker.toml --awl <FILE.awl>` | Serves manifest-wired host commands on the declared queue. |
| `aion worker agent <FILE.awl> --liminal-address <addr> --identity <name>` | Serves a compiled-in agent harness on the queue the document declares. Which harness, and every other launch setting, comes from the `worker` block's `harness` section — not from a flag. See [AGENT-WORKERS.md](../workers/AGENT-WORKERS.md). |

`aion list`, `describe`, `inspect`, `signal`, `query`, `pause`, `resume`,
`cancel`, `reopen`, and `versions` operate the resulting runs; see the
[operations guide](../operations/operations.md).

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
**not** prove a worker exists. Four things can serve an action — see
[`WORKERS.md`](../../examples/assistant/resources/WORKERS.md):

1. **A declared body** (`run "…"`) — the server runs the command itself and no
   worker serves that action. Read
   [`DECLARED-COMMANDS.md`](../workers/DECLARED-COMMANDS.md) first: **there is no
   sandbox.**
2. **A worker document** — a second `.awl` file that opens with `worker
   <queue>` and carries a body for every action, served by `aion worker awl`.
   The typed counterpart of the manifest below, and the only serving path
   whose wiring the language checks. See
   [`AWL-WORKER-DOCUMENTS.md`](../workers/AWL-WORKER-DOCUMENTS.md).
3. **`aion worker shell`** — host commands wired by a strict manifest.
4. **A real worker process** — `aion awl scaffold` generates a Rust one, or
   use any worker SDK. `aion worker agent` serves an agent harness without a
   crate at all, launching it from a `harness` section written inside the
   `worker` block — the one thing a `worker` block carries that is
   configuration rather than requirement. See
   [`AGENT-WORKERS.md`](../workers/AGENT-WORKERS.md).

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
[`editors/`](../../editors/) wire it up.
