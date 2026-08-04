# Aion documentation

Start here. The reader-facing documentation lives in the folders below; each
folder groups one kind of question. Everything else under `docs/` is the
project's own working record (design corpus, work briefs, evidence logs) and
is listed at the bottom so you know what it is when you trip over it.

## Start

- [GETTING-STARTED.md](GETTING-STARTED.md) — nothing to a completed durable
  workflow using only published artifacts; no repository checkout needed.
  The one-command alternative is `scripts/setup.sh` at the repository root,
  which also deploys the assistant and starts its worker.

## `authoring/` — writing workflows

- [AWL.md](authoring/AWL.md) — AWL, Aion's own workflow language: the whole
  language, document by document. One of the two first-class authoring
  surfaces.
- [workflows.md](authoring/workflows.md) — the other surface: writing
  workflows in typed Gleam with the `aion_flow` SDK.
- [authoring.md](authoring/authoring.md) — Gleam activity authoring: declare
  an activity once, generate the rest.
- [codegen.md](authoring/codegen.md) — `aion generate`: every wire-shaped
  artifact derived from authored Gleam types.
- [packaging.md](authoring/packaging.md) — `aion package`: turning a Gleam
  workflow project into the `.aion` archive the server loads.
- [aion-authoring-dx.md](authoring/aion-authoring-dx.md) — the `aion dev`
  instant loop, `aion new` templates, and the authoring dev routes.

## `workers/` — serving activities

- [activities-and-workers.md](workers/activities-and-workers.md) — the
  concepts: what an activity is, what a worker is, how dispatch and admission
  work.
- [WORKER-AUTHORING.md](workers/WORKER-AUTHORING.md) — writing an activity
  worker as a Rust crate, grounded in the `examples/dev-brief/worker/`
  reference implementation.
- [AGENT-WORKERS.md](workers/AGENT-WORKERS.md) — agent workers: serving
  agent actions with `aion worker agent` (no crate) or the assistant's
  norn-backed example worker, plus session pinning and the one-command setup.
- [DECLARED-COMMANDS.md](workers/DECLARED-COMMANDS.md) — actions that carry
  a body in the AWL document and are run by the server; no worker involved.
- [SHELL-WORKER-MANIFEST.md](workers/SHELL-WORKER-MANIFEST.md) — the
  `worker.toml` field reference for `aion worker shell`.

## `operations/` — running Aion

- [operations.md](operations/operations.md) — running, configuring,
  deploying to, and recovering an Aion server.
- [ops-console.md](operations/ops-console.md) — the web console embedded in
  the server binary: every view, and what an operator can do from each.
- [API.md](operations/API.md) — the API surface: HTTP, gRPC, WebSocket, and
  the client/worker SDKs, with implementation status.
- [errors.md](operations/errors.md) — the one stable error taxonomy across
  HTTP, gRPC, and the CLI.

## `contracts/` — behavior you can build against

- [CANCELLATION-CONTRACT.md](contracts/CANCELLATION-CONTRACT.md) — what
  cancellation does today, stated so a consumer can build around it.
- [PAUSE-RESUME-DESIGN.md](contracts/PAUSE-RESUME-DESIGN.md) — the ratified
  pause/resume design and its rulings.

## `vision/` — where this is going

- [ROADMAP.md](vision/ROADMAP.md) — the canonical forward roadmap.
- [DURABLE-AGENTS-AS-INFRASTRUCTURE.md](vision/DURABLE-AGENTS-AS-INFRASTRUCTURE.md)
  — the north-star pitch the whole stack is building toward.
- [AION-DISTRIBUTION-DESIGN.md](vision/AION-DISTRIBUTION-DESIGN.md) —
  distributed orchestration: fan-out, fan-in, affinity, storage.
- [WORKFLOW-AUTHORING-AGENT-IDEA.md](vision/WORKFLOW-AUTHORING-AGENT-IDEA.md)
  — captured idea: variable-liveness testing and a specialised authoring
  agent.

## SDKs — writing callers and workers in your language

Each SDK documents itself where it lives:

- [gleam/aion_flow](../gleam/aion_flow/README.md) — the typed Gleam workflow
  SDK (the other first-class authoring surface).
- [gleam/aion_client](../gleam/aion_client/README.md) — the Gleam caller SDK.
- [crates/aion-client](../crates/aion-client/README.md) — the Rust caller SDK.
- [crates/aion-worker](../crates/aion-worker/README.md) — the Rust worker SDK.
- [sdks/python](../sdks/python/aion-client/README.md) — Python caller and
  [worker](../sdks/python/aion-worker/README.md) SDKs.
- [sdks/typescript](../sdks/typescript/aion-client/README.md) — TypeScript
  caller and [worker](../sdks/typescript/aion-worker/README.md) SDKs.

## Examples

[examples/README.md](../examples/README.md) indexes all thirty runnable
examples, starter ones first.

## `demo/` — driving it live

- [FAILOVER-DEMO.md](demo/FAILOVER-DEMO.md) — the kill-it-and-watch-it-recover
  demonstration.
- [MULTI-HOST.md](demo/MULTI-HOST.md) — running across more than one machine.

## Working records (not user documentation)

- `design/` — the design corpus: per-cluster design JSON and rendered
  markdown. The whole-system picture is
  `design/workflow-engine/DESIGN-OVERVIEW.md` and
  `COMPONENT-ARCHITECTURE.md`.
- `briefs/` — point-in-time work briefs for individual landings.
- `evidence/` — live-run evidence records backing specific claims.
- `remediation/` — the finding-to-fix remediation lanes, by crate and date.
- `examples/` — narrative walkthroughs of specific examples.
- `design-system/` — the tooling that renders and checks the design corpus.
- `archive/` — superseded designs and reviews, kept because they explain
  decisions; they describe their moment, not the present.
