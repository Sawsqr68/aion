# Aion

**A durable workflow engine for AWL, Gleam, Rust, and the BEAM.**

Aion gives you Temporal-class durable execution: workflows that survive
`kill -9`, replay from event history, sleep durably for months, and resume
exactly where they were. Workflows are authored in **AWL** — Aion's own
workflow language — or in type-safe **Gleam**, compiled to BEAM bytecode, and
executed on [beamr](https://crates.io/crates/beamr) — a Rust implementation of
the BEAM VM — with a Rust persistence and transport layer around it.

```sh
cargo install aion-cli --locked   # installs the `aion` binary
```

**Start here: [Getting started](docs/GETTING-STARTED.md)** — zero to a
completed durable workflow, in AWL, with every file you need inline.
The AWL language itself is in [`docs/AWL.md`](docs/AWL.md).

> Aion is the Greek conception of eternal, unbounded time — distinct from
> Chronos, who is sequential, ticking time. A workflow that sleeps for three
> months and resumes is living in eternal time, not clock time.

## What you get

- **Durable execution** — event-sourced histories with deterministic replay.
  Kill the server mid-run; on restart it replays history and the run resumes
  at the same await, without re-executing completed activities.
- **Two first-class authoring surfaces** — AWL `.awl` documents and typed
  Gleam ([`aion_flow`](gleam/aion_flow/)).
  - **AWL** is the shortest path: one file declares the types, the workers and
    their actions, and the step graph. `aion awl check|fmt|emit|scaffold|schema|lsp`
    cover the local loop (including a language server and Rust worker
    scaffolding); `aion deploy <file.awl>` direct-compiles and deploys a
    document; `aion run <file.awl> --input <json> [--timeout <dur>]` compiles,
    deploys, starts, and awaits it in one motion. No Gleam build, no
    `workflow.toml`, no `aion package` step.
  - **Gleam** is the typed-code path when a workflow needs real code:
    `aion new`, `aion generate`, `aion dev` hot-loading, and
    `aion check --deterministic`.
- **Activities on workers you own** — Rust, Python, and TypeScript worker
  SDKs over a gRPC worker protocol with acks, reconnect, heartbeats, and
  cooperative cancellation.
- **Versioned deploys** — `.aion` packages are content-hash versioned and
  immutable. Deploy at runtime to a live server, route new starts, roll back
  atomically; running workflows keep their pinned version, and
  runtime-deployed packages persist across restarts.
- **One operator surface** — the `aion` CLI runs the server (`aion server`),
  packages workflows, deploys, and operates runs; HTTP/JSON, gRPC, and
  WebSocket event-stream transports, Prometheus metrics, and structured
  audit logs.

## Honest limits

- Clustering support is compiled into the stock binary by default through the
  haematite backend; single-node remains the default topology until
  `[store.cluster]` is configured. Distributed mode uses quorum replication,
  and the supervisor automatically adopts a declared peer's shards after
  debounced link loss. Membership and shard ownership are operator-configured,
  automatic failover requires each peer's `owned_shards`, and `shard_count` is
  fixed when the database is created — there is no online resharding path.
- Activity retries are engine-driven: retryable failures (provider flakes,
  lost workers) are re-dispatched per the declared `RetryPolicy` and cost
  retries, not the run; exhaustion fails the run loudly with the attempt
  trail preserved. Workflow-driven bounded retry loops remain available on
  top (the [order-saga example](docs/examples/order-saga.md) shows the
  pattern).
- The ops console UI is under development; the CLI and HTTP API are the
  operating surfaces.
- Cooperative cancellation covers worker shutdown/drain only. There is no
  cancel frame in the worker protocol, so the server cannot cancel a
  specific in-flight activity; the SDK sets the cancellation flag when a
  worker drains or shuts down (local in-flight activities), not in response
  to a server-initiated per-activity cancel.

## Documentation

| Document | What it covers |
|---|---|
| [Getting started](docs/GETTING-STARTED.md) | Zero to completed workflow on published artifacts |
| [AWL](docs/AWL.md) | The AWL surface: the verbs, the authoring loop, and where the language reference lives |
| [AWL language reference](examples/assistant/resources/AWL-REFERENCE.md) | The complete checked AWL grammar: types, workers, steps, forks, regions, loops, waits, signals, queries |
| [Declared commands](docs/DECLARED-COMMANDS.md) | `run "…"` action bodies — what is guaranteed and what is not (**there is no sandbox**) |
| [Workflow authoring guide](docs/guides/workflows.md) | The Gleam entry contract, determinism rules, timers, signals, queries, children |
| [Activities & workers guide](docs/guides/activities-and-workers.md) | Worker scaffolding, failure classification, retry semantics |
| [Schema codegen guide](docs/guides/codegen.md) | Generate Gleam types + JSON codecs from your schemas; `--check` CI gate; supported subset |
| [Operations guide](docs/guides/operations.md) | Full config reference, deploy/versioning, persistence & recovery, metrics |
| [Errors reference](docs/errors.md) | Every error code and what to do about it |
| [API overview](docs/API.md) | HTTP/JSON, gRPC, WebSocket transports |
| [Packaging reference](docs/packaging.md) | `workflow.toml` and the `.aion` format |
| [Order saga walkthrough](docs/examples/order-saga.md) | The flagship example: retries, timeout races, child workflows, compensation |

## Why Gleam + Rust + BEAM

- **Gleam** — compile-time type safety for workflow definitions. Activity
  inputs, results, signals, and queries are statically typed; you cannot
  wire mismatched types together and ship it.
- **BEAM** (via beamr) — the execution runtime: processes, mailboxes,
  selective receive, supervision, hot code loading. Every workflow is a
  process.
- **Rust** — the durable substrate: the event store, replay machinery,
  network APIs, and the VM itself.

Temporal had to build distributed process management, supervision, and fault
tolerance from scratch. The BEAM provides them natively; Aion adds the
durability layer the BEAM traditionally lacks.

## Architecture

1. **beamr** — the BEAM runtime (external crate).
2. **The Aion engine** (`crates/aion`) — workflow lifecycle, event-sourced
   durability and replay, durable timers, signal routing, queries, child
   workflows. Transport-agnostic.
3. **SDKs and transports** — the Gleam authoring SDK (`gleam/aion_flow`),
   the server (HTTP/gRPC/WebSocket + worker protocol), and worker/client
   SDKs in Rust, Python, and TypeScript.

The whole-system design lives in
[`docs/design/workflow-engine/`](docs/design/workflow-engine/)
([`DESIGN-OVERVIEW.md`](docs/design/workflow-engine/DESIGN-OVERVIEW.md),
[`COMPONENT-ARCHITECTURE.md`](docs/design/workflow-engine/COMPONENT-ARCHITECTURE.md)).

### Components

| Crate / package | Role |
|---|---|
| `aion-core` | Domain model: events, payloads, identifiers, status, errors |
| `aion-store` | The `EventStore` contract + in-memory reference + conformance suite |
| `aion-store-haematite` | Default durable store; single-node or distributed with `[store.cluster]` |
| `aion-store-libsql` | Alternative durable store over libSQL |
| `aion-package` | The `.aion` package format, content-hash versioning |
| `aion-awl` | AWL parser, checker, formatter, schema derivation, and compiler |
| `aion-awl-lsp` | AWL language-server adapter |
| `aion-awl-package` | AWL-native `.aion` package assembly |
| `aion` | The engine: lifecycle, durability/replay, timers, signals, queries |
| `aion-proto` | Shared wire contract (gRPC + serde) |
| `aion-darwin-acl` | Single-snapshot native macOS ACL decoder for the server's path-safety gate |
| `aion-server` | Server library: HTTP/gRPC/WebSocket + worker protocol |
| `aion-worker` | Rust remote-worker SDK (library — scaffold your own binary) |
| `aion-client` | Rust caller SDK |
| `aion-cli` | The `aion` binary: server, packaging, deploy, workflow operations |
| `gleam/aion_flow` | The Gleam authoring SDK ([Hex](https://hex.pm/packages/aion_flow)) |
| `gleam/aion_client` | The Gleam caller SDK |
| `sdks/python/*`, `sdks/typescript/*` | Worker + client SDKs |
| `apps/aion-ops-console` | Ops console UI under development |

## Repository layout

```
crates/            Rust crates (engine, store, package, proto, server, worker, client, cli)
gleam/             Gleam packages (aion_flow authoring SDK, aion_client)
sdks/python/       Python worker + client SDKs
sdks/typescript/   TypeScript worker + client SDKs
apps/              The under-development ops console (React + Vite)
conformance/       Cross-language conformance suites
examples/          Working examples (hello-world first, order-fulfillment flagship)
docs/              User documentation; docs/design/ holds the full design
tools/             Workspace tooling (scaffold.py)
workspace.json     Machine-readable description of every component
```

## Publishing

Rust crates are published leaf-first; the order is derived from
`cargo metadata`, with `aion-darwin-acl` immediately before its dependent
`aion-server`. The canonical order is in [`publish-workspace.sh`](publish-workspace.sh)
and is validated by the alternate
[`scripts/publish-crates.sh`](scripts/publish-crates.sh) (dry-run by default,
`--live` to publish). The Gleam SDK (`aion_flow`) is published to Hex.
See [`CONTRIBUTING.md`](CONTRIBUTING.md) for development workflow.

## License

AGPL-3.0-only. See [`LICENSE`](LICENSE).

Created by [Tom Whiting](https://github.com/tomWhiting). If you build on
Aion or write about it, a link back here is appreciated.
