# aion-cli

The Aion command line. The crate is named `aion-cli`; the installed binary
is **`aion`** — the one user-facing binary for running the server, packaging
workflows, deploying packages, and operating workflow executions over gRPC.

## Install

```sh
cargo install aion-cli --locked
```

## Subcommands

### Authoring AWL (`.awl` documents)

AWL is Aion's workflow language. These run entirely locally and never connect
to a server:

- `aion awl check <FILE.awl>` parses and typechecks a document. Prints
  `ok: <file> (N steps)`, or compiler-style
  `<file>:<line>:<column>: error: <message>` diagnostics on stderr.
- `aion awl fmt <FILE.awl>` rewrites the document in place with the canonical
  printer. The printer *is* the formatter — there is no `--check` mode.
- `aion awl emit <FILE.awl> [--output <PATH>] [--target gleam|beam]` emits an
  executable artifact. `--target gleam` (the default) writes Gleam source to
  `--output` or stdout; `--target beam` compiles direct BEAM bytecode through
  the same seam the server's deploy path uses and **requires** `--output`,
  writing a `<output>.json` sidecar alongside it.
- `aion awl schema <FILE.awl> [--type <Type>] [--queries]` derives JSON Schema
  (draft 2020-12). With neither flag it emits the workflow's start contract;
  `--type` emits a declared type; `--queries` emits the read-only surface.
- `aion awl scaffold <FILE.awl> --out <DIR> [--worker <name>] [--aion-crates <DIR>]`
  generates a compiling Rust worker crate per `worker` block, with every wire
  descriptor taken from the compiled contract.
- `aion awl lsp` runs the AWL language server over stdio for editor
  integration.

Deploying and running an `.awl` document needs no Gleam build, no
`workflow.toml`, and no `aion package` step:

- `aion deploy <FILE.awl>` direct-compiles and deploys the document.
- `aion run <FILE.awl> --input '<json>' [--timeout <30s|5m|3h|2d>]` compiles,
  validates the input locally against the derived start contract, deploys,
  starts, and awaits the run in one motion. `--timeout` has **no default**:
  absent, the wait is bounded only by the document's own `timeout`
  declaration, and a document declaring neither waits indefinitely. Reaching
  the bound stops the *wait*, never the run — the workflow continues on the
  server.

### Server, workers, and Gleam packaging

- `aion server --config aion.toml` runs the Aion server (the `aion-server`
  crate is the library it embeds). `--workflow-package <path>` preloads
  `.aion` archives at boot.
- `aion worker shell --manifest worker.toml --awl <FILE.awl>` serves strict
  manifest-wired host commands on a task queue.
- `aion worker agent <FILE.awl> --liminal-address <HOST:PORT> --identity <NAME>`
  serves an agent harness compiled into this binary on the queue an `.awl`
  document declares — an agent worker without writing a crate. The LAUNCH is
  the document's, in the `worker` block's `harness` section (kind, concurrency,
  reconnect policy, environment pass-through, and the agent's command and
  arguments); the command line carries only where this worker connects and who
  it says it is. `--check` prints the exact action surface, resolved harness,
  and intervention controls, then exits without connecting.
- `aion new <name> [--template <t>] [--worker rust]` scaffolds a workflow
  project — a checked, runnable AWL document by default, or one of five
  buildable Gleam templates.
- `aion dev --gleam-path <path>` watches a Gleam workflow project and
  hot-loads each save into a running server.
- `aion generate [PATH] [--check]` derives codecs, schemas, and worker
  plumbing from a Gleam package's authored types.
- `aion check [PATH] --deterministic` statically flags wall-clock and entropy
  calls reachable from Gleam workflow code.
- `aion input <workflow-type> [PATH]` prints a valid input skeleton derived
  from the workflow's input schema.
- `aion package [PATH] [--out <FILE>] [--build]` packages a Gleam workflow
  project into a `.aion` archive. Local-only; never connects to a server.
- `aion deploy <archive>` deploys a `.aion` archive to a running server
  (requires the server's `[deploy]` surface to be enabled).

### Versions and run operations
- `aion versions [--workflow-type <name>]` lists loaded workflow versions
  with routing flags.
- `aion route <workflow-type> <content-hash>` re-points routing to an
  already-loaded version (rollback / roll-forward).
- `aion unload <workflow-type> <content-hash>` unloads a non-routed,
  unpinned version.
- `aion start <workflow-type> --input '<json>'` starts a workflow execution.
- `aion signal <workflow-id> <signal-name> --payload '<json>' [--run-id <id>]`
  sends a signal.
- `aion query <workflow-id> <query-name> [--run-id <id>]` performs a live
  read-only query.
- `aion cancel <workflow-id> [--reason <text>] [--run-id <id>]` requests
  cancellation.
- `aion pause <workflow-id> [--reason <text>] [--run-id <id>]` durably holds
  new activity dispatch; `aion resume <workflow-id> [--run-id <id>]` releases
  the hold.
- `aion reopen <workflow-id> [--run-id <id>]` reopens a terminal-reopenable
  (Failed or Cancelled) run and re-drives it.
- `aion list [--status <status>]` and
  `aion describe <workflow-id> [--run-id <id>] [--raw]` cover visibility and
  history.
- `aion tail <parent-workflow-id> [--run-id <id>] [--no-follow]` interleaves
  retained and live transcripts for the parent and its direct child legs. It
  prints leg/activity/attempt identity on every event, announces superseded or
  continued-as-new generations, pending child starts, socket/decode failures,
  and per-leg lag recovery instead of going quiet. The CLI requests the most
  recent 2000 retained events per activity attempt; that is a CLI-owned bound,
  not a server guarantee. A sequence-based notice identifies unavailable earlier
  records, which may reflect retention or that request bound. `--http-endpoint`
  selects the HTTP/WebSocket listener; it
  defaults from the server's shipping `DEFAULT_HTTP_ADDRESS` and can also be
  set with `AION_HTTP_ENDPOINT`.
- `aion inspect <workflow-id> [--run-id <id>] [--from <seq> --mock <json>]`
  time-travels over a recorded run's oplog, and with `--from`/`--mock`
  (required together) runs a read-only what-if re-run from a chosen event.

Global flags: `--endpoint` (default `127.0.0.1:50051`), `--namespace`
(default `default`), `--subject` (default `cli-user`), `--token` (overrides
the `AION_TOKEN` environment variable), and `--pretty`.

## Minimal usage

```sh
aion --endpoint 127.0.0.1:50051 \
  start hello_world --input '{"name":"Ada"}'
```

## Error reporting

Every operational failure prints one report to stderr and exits with code 1
(CLI usage mistakes exit 2); stdout stays reserved for the JSON result
document. Failures that carry the client taxonomy render as

```text
error[<class>]: <operation>: <server detail message>
  server error type: <ErrorType>   # when the wire carried one
  hint: <actionable next step>     # for classes with a known remedy
```

where `<class>` is aligned with the wire error codes: `not_found`,
`already_exists`, `query_failed`, `query_timeout`, `unknown_query`,
`not_running`, `cancelled`, `unavailable`, `unauthenticated`,
`namespace_denied`, `invalid_input`, `backend`, and — on the deploy
surface — `deploy_denied` and `version_pinned`. Local failures without a
taxonomy class render their full cause chain on one `error:` line. The full
taxonomy is documented in the repository's
[errors reference](https://github.com/ablative-io/aion/blob/main/docs/operations/errors.md).
