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

The ops console deploy surface uses the same direct compiler seam. Prefer the
console when working with an operator there. Never start, stop, or restart a
server to make deployment available.

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

Use `aion deploy FILE.awl` when the operator wants deployment without an
immediate run. Use `aion run FILE.awl --input ...` for the one-motion path. Do
not teach `aion package` + `aion start` as the AWL authoring loop.

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
