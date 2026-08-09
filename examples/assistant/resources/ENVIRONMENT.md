# ENVIRONMENT.md — AWL preflight and workspace rules

AWL (`.awl`) is the authoring source of truth. Do not create a Gleam project,
`gleam.toml`, `workflow.toml`, generated codec, or `.aion` archive for an AWL
request. The current CLI compiles an AWL document directly for deploy and run.
Generated Gleam from `aion awl emit` is a toolchain view, not authored source.

## Workspace boundary

Your file tools are confined to the session workspace. Shell commands start in
that workspace and may read paths outside it, subject to host permissions.
Keep every file you author inside the workspace. If the workspace is a clone of
the aion repository, preserve unrelated changes and use the repository itself
as the authority. If it is scratch space, nothing is materialized for you:
clone the repository when you need it, and use `aion awl guide` for the
language surface — the binary itself carries the glossary and the full
reference (`aion awl guide --reference`), so no checkout is required for
language questions.

Before editing:

```sh
pwd
git status --short --branch 2>/dev/null || true
command -v aion
aion --version
aion awl --help
aion awl check --help
```

`aion awl check` is local: it parses and typechecks without contacting a
server. If `aion` is unavailable, report that AWL cannot be checked, compiled,
deployed, or run in this session. Do not substitute a Gleam workflow.

Gleam is not required to author or check `.awl` files. Git is useful only when
the requested work belongs in a repository. Do not install missing tools or
start a server as a workaround.

## Ground truth order

Use these sources in order:

1. The `.awl` file being edited and `aion awl check FILE.awl`.
2. `examples/assistant/resources/AWL-REFERENCE.md` in an aion checkout, or
   `aion awl guide --reference`, which prints the same document embedded in
   the binary. `aion awl guide WORD` explains one word of the language with a
   complete example that passes `aion awl check`; with no argument it prints
   every word grouped by reference section.
3. Checked examples such as `examples/awl-hello/awl_hello.awl`,
   `examples/dev-brief/awl/dev_brief.awl`, and
   `examples/remediation-packet/awl/remediation_packet.awl`.
4. Checker-valid fixtures under
   `crates/aion-awl/tests/fixtures/rev2/**/valid/*.awl`.
5. The parser and checker in `crates/aion-awl/src/`, plus the CLI definitions
   in `crates/aion-cli/src/awl.rs`, `main.rs`, `one_motion.rs`,
   `awl_scaffold/` (the `aion awl scaffold` worker generator),
   `worker_shell.rs`, and `worker_agent/` (the two `aion worker`
   subcommands).

The reserved vocabulary itself is enumerated once, in
`crates/aion-awl/src/lexer/tokens.rs`: `Keyword::ALL` is the complete reserved
inventory, and `POSITIONAL_WORDS` names the openers that are recognised in one
grammar position WITHOUT being reserved (`send`, `to`, `query`, `answer`,
`run`, `in`). A word's absence from `Keyword::ALL` is not evidence the
construct does not exist — check `POSITIONAL_WORDS` and the parser before
saying so.

Design papers can explain intent, but accepted grammar plus checked files decide
what can be authored now. Never copy syntax only from a roadmap or draft.

## Server boundary

Checking, formatting, schema derivation, worker scaffolding, emitting, and the
language guide are local. `aion deploy`, `aion run`, and observation commands
contact the configured endpoint.

A server may also expose an MCP endpoint: `POST /mcp`, on the SAME HTTP
listener as the console and the workflow API, mounted only when the server's
config sets `enabled = true` under `[mcp]` — the default is off, and with it
off the path is a plain 404. It inherits the server's existing authentication
and namespace authorization (there is no MCP-specific auth), and it implements
the 2026-07-28 stateless revision: `POST /mcp` only, with `GET` and `DELETE`
answered 405. `docs/operations/operations.md` carries the authoritative
section. Never claim an MCP endpoint is live without the operator's config or
an actual checked response. Do not assume the default endpoint is the operator's
target, do not start or restart a server, and do not claim a deploy or run
succeeded unless the actual command or ops console reported it. When server
access is not explicitly available, finish at a checked `.awl` file and state
that deploy/run was not performed.

## The operator's server state lives in the Aion home

A running server resolves one home directory before it reads any config:
`AION_HOME` when set, otherwise `$HOME/.aion`. An empty `AION_HOME` is a
startup error, and a relative one resolves against the current directory.

Under that home:

| Path | What it is |
|---|---|
| `<home>/config.toml` | user-level server config |
| `<home>/data` | default haematite event-store directory |
| `<home>/authoring` | default authoring/AWL workspace root |

Config discovery takes the first that exists: `--config PATH`, then
`./aion.toml`, then `<home>/config.toml`, then built-in defaults.

Consequences for a session:

- **This is the operator's durable state.** Never delete, move, chmod, or
  rewrite anything under the Aion home, and never set `AION_HOME` for a command
  you run on their behalf unless they asked for it — pointing a server at a
  different home silently detaches it from every run it was holding.
- The server provisions and tightens these roots itself (`0700`). An operator
  does not need to `chmod` them by hand; do not advise it.
- If more than one server is involved, `AION_HOME` is what separates them.
  Confirm which endpoint and which home before reasoning about "the" state.
- A `.aion` directory INSIDE an authoring workspace is a different thing
  entirely (per-document layout sidecars). Do not confuse it with the home.

At the end, report the path of every authored `.awl` file, every local check you
ran, and whether deploy/run was performed or deliberately left to the operator.
