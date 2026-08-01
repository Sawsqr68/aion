# Getting started with Aion

The getting-started guide lives at
[`docs/GETTING-STARTED.md`](docs/GETTING-STARTED.md). It takes you from nothing
to a completed durable workflow using only published artifacts: install the
`aion` CLI, write an **AWL** workflow, serve its activities, run the server,
and deploy, start, signal, and complete a run end to end — then kill the server
and watch the run survive. The stock binary defaults to the haematite store;
libSQL is the alternative durable backend.

The short version, once `aion` is installed and a server is running:

```sh
aion awl check my_flow.awl                        # local: parse + typecheck
aion run my_flow.awl --input '{"name":"Ada"}'     # compile, deploy, start, await
```

- [`docs/AWL.md`](docs/AWL.md) — the AWL surface: every verb, the authoring
  loop, and links to the full language reference.
- [`docs/DECLARED-COMMANDS.md`](docs/DECLARED-COMMANDS.md) — read this before
  writing a `run "…"` action body. **There is no sandbox.**

**Gleam** is the other first-class authoring path, for workflows that need
real typed code: `aion new` scaffolds a project, `aion generate` derives
codecs and schemas from your types, `aion dev` hot-loads on save, and
`aion package` + `aion deploy` ship a `.aion` archive. The second half of
`docs/GETTING-STARTED.md` walks it end to end.

## Working from this repository instead?

If you have a checkout and want to run the bundled examples from source:

- [`examples/hello-world/README.md`](examples/hello-world/README.md) — the
  first run: build, package, serve, and operate the hello-world workflow.
- [`examples/order-fulfillment/README.md`](examples/order-fulfillment/README.md)
  — the flagship saga (retries, signal/timeout race, child workflow,
  compensation), walked through in
  [`docs/examples/order-saga.md`](docs/examples/order-saga.md).
- [`dev-config.toml`](dev-config.toml) — the local development server
  config; run the server from source with
  `cargo run -p aion-cli -- server --config dev-config.toml`.
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — development workflow, lint and test
  gates.
