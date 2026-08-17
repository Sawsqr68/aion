# The CLI and the server side

## Local verbs — never contact a server

| Command | What it does |
|---|---|
| `aion awl check FILE.awl` | Parse + typecheck. `ok: <file> (N steps)`, or `file:line:col: error: …` on stderr, non-zero exit. |
| `aion awl fmt FILE.awl` | Canonical rewrite in place. The printer IS the formatter; no `--check` mode; a parse error leaves the file untouched. |
| `aion awl schema FILE.awl [--type T] [--queries]` | JSON Schema (draft 2020-12) for the start contract, one type, or the query surface. |
| `aion awl scaffold FILE.awl --out DIR` | Compiling Rust worker crate per `worker` block; handlers file is yours after first generation. |
| `aion awl emit FILE.awl [--target gleam\|beam]` | Executable artifact; optional in the normal loop. |
| `aion awl guide [WORD]` | The language, one sentence per word + checked examples; `--reference` prints the whole embedded reference. |
| `aion awl lsp` | Language server over stdio (extensions under `editors/`). |

## Against a running server

| Command | What it does |
|---|---|
| `aion` (bare) | First-run scaffold + launcher: config, home, console. |
| `aion server` | Runs the server (HTTP + gRPC + WebSocket + worker protocol + ops console). |
| `aion deploy FILE.awl` | Check, compile, assemble, deploy — without starting a run. |
| `aion run FILE.awl --input '{…}' [--timeout DUR]` | Compile, deploy, start, await, in one motion. Input validated locally first. `--timeout` bounds only your wait; the run continues server-side. |
| `aion start <workflow> --input-file f.json [--name NAME]` | Start a deployed workflow. |
| `aion worker awl / shell / agent` | The three no-crate serving paths (see workers.md). |
| `aion list / describe / inspect / signal / query / pause / resume / cancel / reopen / versions` | Operate runs. `describe` output can be large — pipe through a JSON summary. |

## Commissioning workers: the two configuration bites

A stock server has **no port for a worker to dial**. Both halves bite anyone
standing a worker up for the first time:

1. `[outbox] enabled` defaults to **false**, and the worker listener only
   exists inside the outbox gate — with it off, a worker's dial is refused
   forever with nothing server-side saying why.
2. `liminal_listen_address` has **no default at all**. With the outbox on
   and a missing address, boot refuses naming the field.

```toml
[outbox]
enabled = true
liminal_listen_address = "127.0.0.1:50061"
```

`outbox.enabled = true` additionally requires a durable store
(`store.backend = "haematite"`); the in-memory store has no outbox. Then
**read the address the server actually bound** from its log line
(`liminal server listener bound`) rather than assuming the configured one.
On a fresh Aion home this is scaffolded for you: a first boot that finds no
config writes `<AION_HOME>/config.toml` with `[outbox]` already
commissioned, and `scripts/setup.sh` drives the whole one-command path.

## Habits that save an afternoon

- The store is relative to the working directory in default setups — start
  the server from the same directory every time, and verify with
  `aion versions` (non-empty means you found your store).
- Deploys are immutable, content-hashed versions; long-lived runs keep the
  version they started on while new runs get the new deploy. Deploying the
  same document twice with no byte change is the same version.
- Timers, signals, and runs survive server restarts — that is the point.
  Kill the server mid-run and watch `aion describe` after restart before
  believing anything is lost.
- Workers are started manually or by Aion-native supervision. Match queue
  names exactly: the AWL `worker` block name IS the task queue string the
  worker must register.
- Full operator tutorial: `docs/GETTING-STARTED.md`. Operations reference:
  `docs/operations/`.
