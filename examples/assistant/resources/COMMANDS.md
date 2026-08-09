# COMMANDS.md — AWL check, deploy, run, and observe

The command definitions live in `crates/aion-cli/src/awl.rs`, `main.rs`, and
`one_motion.rs`. Verify the installed binary with `aion --version` and the
relevant `--help`; do not transpose commands from the retired Gleam package
path.

`--endpoint`, `--namespace`, `--subject`, `--token`, and `--pretty` are global
options. The defaults are `127.0.0.1:50051` and namespace `default`, but defaults
are not evidence that they name the operator's server.

## Local authoring commands

These commands do not contact a server.

### Check

```sh
aion awl check FILE.awl
```

The command parses and typechecks declarations, calls, binding flow, step
graphs, routes, outcome exhaustiveness, and schema imports. Success prints:

```text
ok: FILE.awl (N steps)
```

Errors print compiler-style `file:line:column: error: message` diagnostics to
stderr and exit non-zero.

### Format

```sh
aion awl fmt FILE.awl
```

This rewrites the file in place with the canonical printer. There is no
`--check` mode. A parse error leaves the file untouched. Inspect the diff and
run `aion awl check FILE.awl` again.

### The language guide

```sh
aion awl guide
aion awl guide wait
aion awl guide --reference
aion awl guide --json
```

With a word, `guide` prints its meaning, a complete example document that
passes `aion awl check`, and the reference section that explains it. With no
argument it prints every word of the language grouped by reference section,
above the paths of the documents an author needs. `--reference` prints the
full `AWL-REFERENCE.md` embedded in the binary, so no repository checkout is
required; `--json` emits the glossary table. Every sentence comes from the
same table the AWL language server hovers and the ops console's guide overlay
renders, so those surfaces cannot disagree.

### Derive schema

```sh
aion awl schema FILE.awl
aion awl schema FILE.awl --type Report
```

With no `--type`, this emits the workflow start contract. With `--type`, it
emits draft 2020-12 JSON Schema for that declared type. Schema imports resolve
relative to the document directory.

### Emit a compiler artifact

```sh
mkdir -p target/awl
aion awl emit FILE.awl --target beam --output target/awl/workflow.beam
```

Direct BEAM output requires `--output` and also writes a
`target/awl/workflow.beam.json` sidecar. The sidecar records the workflow name,
contracts, action requirements, synthesized entries, and timeout.

The default target is `gleam`:

```sh
aion awl emit FILE.awl --output target/awl/workflow.gleam
```

That is a generated stopgap/toolchain view. Never edit it or replace the `.awl`
source with it. Emission is optional in the normal authoring loop; direct
`.awl` deploy/run compile through the production path themselves.

### Scaffold a worker crate

```sh
aion awl scaffold FILE.awl --out ./workers
```

This generates one compiling Rust worker crate per `worker` block, at
`<out>/<queue>-worker`. Every wire descriptor the generated worker advertises
comes from the contract the compiler returns — the same bytes the server
deploys and admits against — not from the handlers' Rust types, because a
second rendering of the same shapes is exactly the drift the server's contract
gate exists to catch.

`--worker NAME` scaffolds one block instead of every block. `--aion-crates
<DIR>` points the generated crate at a local aion checkout's `crates/`
directory by path; omitted, it depends on the published crates at this binary's
version. `src/handlers.rs` is yours from the moment it exists — re-running
rewrites the generated files and never that one.

The generated worker serves no action that carries a declarative `run` body:
the server executes those itself.

## Deploy a checked AWL document

Deployment contacts a running server and requires its deploy surface and the
caller's deploy grant:

```sh
aion --endpoint HOST:PORT --namespace NAME deploy FILE.awl
```

For a case-sensitive `.awl` path, the CLI reads, checks, direct-compiles, and
assembles the workflow before using the ordinary deploy operation. There is no
Gleam build, `workflow.toml`, `aion package`, or hand-managed `.aion` input in
this path.

The assembled package **carries the document it was compiled from**, under
`awl/document/<filename>` plus one `awl/schema/<relative path>` entry per
`schema("…")` import, so a deployed workflow can be shown the exact source it
was built from. That archived source is provenance ONLY: it is excluded from
the package content hash, so carrying it never re-versions a deployed workflow
or orphans runs pinned to it. Do not describe archived source as part of
package identity. A declared `run` command IS identity-bound, but through the
worker contract, not through the archived document.

The ops console's authoring studio uses the same direct compiler seam. Its
**Ship & Run** panel takes the open document through check, compile, package,
deploy, and start with a JSON object input, then watches the run; its deployed
pane lists deployed versions and shows the authored AWL source the archive
carries alongside the version's committed start input schema and signal
contracts. Prefer the console when working with an operator there. Never
start, stop, or restart a server to make deployment available.

## Compile, deploy, run, and await in one motion

```sh
aion --endpoint HOST:PORT --namespace NAME run FILE.awl --input '{"name":"Ada"}'
```

`run` accepts an `.awl` file and an inline JSON `--input`. It compiles and
assembles locally, parses and validates the input against the derived start
schema before opening the deploy connection, deploys, starts the workflow by
its declared name, and polls that concrete run until a terminal result. Success
includes `workflow_type`, `workflow_id`, `run_id`, and the decoded result.
Workflow failure, cancellation, timeout, or continue-as-new is reported as an
error rather than as success.

### `--timeout` bounds the WAIT, not the run

```sh
aion --endpoint HOST:PORT run FILE.awl --input '{"name":"Ada"}' --timeout 5m
```

The value is an AWL duration literal — `30s`, `5m`, `3h`, `2d` — parsed by
aion-awl's own lexer, so this flag and a document's `timeout` declaration share
one vocabulary.

There is **no default**. Absent the flag, the wait is bounded only by the
document's own `timeout` declaration, which the engine enforces as a real
deadline; a document declaring neither, run without `--timeout`, waits
indefinitely.

Reaching the bound stops the WAIT and never the run. The command exits 1 and
reports the workflow id, run id, and a describe line; **the workflow continues
executing on the server**, and `describe` will show it Running. Never report a
`--timeout` exit as a failed or cancelled workflow — it is a statement about
this command's patience, not about the run. To stop a run, cancel it.

Use `aion deploy FILE.awl` when the operator wants deployment without an
immediate run. Use `aion run FILE.awl --input ...` for the one-motion path. Do
not teach `aion package` + `aion start` as the AWL authoring loop.

## Serve the declared actions

A clean check does not connect a worker. Unless every action carries a
declarative `run` body, something must serve the declared queue before a run
can get past its first call:

```sh
aion --endpoint HOST:PORT worker shell --manifest worker.toml --awl FILE.awl
aion --endpoint HOST:PORT worker awl WORKER_FILE.awl
aion worker agent FILE.awl --harness norn --liminal-address HOST:PORT \
  --identity NAME --concurrency N \
  --reconnect-initial-backoff S --reconnect-max-backoff S \
  --reconnect-max-attempts N
```

`aion worker shell` requires BOTH `--manifest` and `--awl`: the manifest
carries host-command wiring only, so the document is the sole source of the
schemas the worker advertises.

`aion worker awl` serves a WORKER DOCUMENT — a `.awl` file whose opening
declaration is `worker <queue>`, carrying a command body on every action. The
commands run on the worker's host, not the server's. A workflow document is
refused by name, and `--check` prints the exact surface the worker would
advertise, with the command each action runs, without connecting. See
`WORKERS.md` and `docs/workers/AWL-WORKER-DOCUMENTS.md`.

`aion worker agent` serves an agent harness compiled into the binary on the
queue the document declares — an agent worker without writing a crate. Facts
that matter before running it:

- The document is positional and required; it is the only possible source of
  the advertised schemas, because an agent activity's output is decided by the
  agent and there is no Rust type to derive one from.
- It dials `--liminal-address` candidates (repeatable, for ordered redial),
  **not** the global gRPC `--endpoint` — the agent-harness seam exists only on
  the liminal worker transport.
- The default build carries two harnesses, `norn` and `acp`, so `--harness` is
  REQUIRED; a build carrying exactly one derives it. A flag configuring a
  harness this invocation is not serving is refused, never ignored.
- Every connection setting comes from a flag or its named environment variable
  (the flag wins), nothing is invented, and every missing one is reported by
  name at once.
- The queue is derived when the document declares exactly one `worker` block,
  and selected with `--task-queue` when it declares several. A queue is served
  whole or the verb refuses.
- `--check` prints the exact action surface the worker would advertise, the
  resolved harness, and its intervention controls, then exits without
  connecting or demanding connection settings. Run it before reporting that an
  agent worker cannot serve a document.

## Observe and operate a real run

The ops console is the primary author → check → canvas → deploy → run → observe
surface. After a start, use the workflow and run identifiers the actual command
returned; do not guess them.

CLI observation surfaces include:

```sh
aion --endpoint HOST:PORT --namespace NAME list
aion --endpoint HOST:PORT --namespace NAME describe WORKFLOW_ID
aion --endpoint HOST:PORT --namespace NAME inspect WORKFLOW_ID
aion --endpoint HOST:PORT --namespace NAME versions --workflow-type WORKFLOW_NAME
```

`describe` shows the durable history and decoded payloads. `inspect` is the
history/time-travel diagnostic surface. `versions` confirms loaded content and
routing. Check each command's `--help` before using optional flags.

For a declared signal:

```sh
aion --endpoint HOST:PORT --namespace NAME signal WORKFLOW_ID SIGNAL_NAME \
  --payload '{"approved":true}'
```

A signal is not proof that the workflow consumed it; confirm progress in the
console or with `describe`.

For a declared `query`, the read-only surface of a RUNNING instance:

```sh
aion --endpoint HOST:PORT --namespace NAME query WORKFLOW_ID QUERY_NAME
```

The answer is whatever the run's most recent `answer` statement registered, so
a query reads live state and never changes it. A **completed** run no longer
answers live queries and returns `error[not_running]`; that is expected —
inspect a terminal run with `describe`. `aion awl schema FILE.awl --queries`
emits the schema of every declared query's answer without contacting a server.

To hold and release a run:

```sh
aion --endpoint HOST:PORT --namespace NAME pause WORKFLOW_ID --reason TEXT
aion --endpoint HOST:PORT --namespace NAME resume WORKFLOW_ID
aion --endpoint HOST:PORT --namespace NAME cancel WORKFLOW_ID --reason TEXT
aion --endpoint HOST:PORT --namespace NAME reopen WORKFLOW_ID
```

`pause` durably holds new activity dispatch and is non-terminal; `resume`
releases the hold. `reopen` re-drives a terminal-reopenable (Failed or
Cancelled) run. These change a real operator's real run — never issue one
without explicit operator authority.

## Honest failure handling

- A check error means the `.awl` is invalid. Fix the source; do not emit around
  it.
- A clean check does not prove workers exist. A call waits for a worker serving
  the declared task queue and action.
- A deploy error is not a reason to start a local server or change endpoints
  speculatively. Report the endpoint/namespace used and the full error.
- A run accepted by the server is not complete until its durable history shows
  the terminal outcome. `aion run` waits; console-started runs must be observed.
- Never report a command as run unless it actually ran in this session and its
  exit status was checked.
