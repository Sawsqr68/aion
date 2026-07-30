# Fleet smoke: live state, 2026-07-30 ~05:10Z

Written immediately before a context compaction. This is the resume point.

## Commits landed (all on main, NOT pushed yet)

- `e6bf8679` — the AWL contract change (gate_item / plan_reviews /
  verify_integration / fix_integration, Lens + GatedItem + ReviewTask types, the
  `unverified` disposition), `tharsis-input.json`, `FLEET-BUILD-DESIGN.md`.
- (worker commit) — the eleven-action worker serving that contract.
- `036f976d` — the integration scope fence compared a tree to itself; the
  document now passes `base_branch` into `verify_integration` and the worker
  resolves `merge-base(base_branch, HEAD)`.

Also landed earlier this session, unrelated to the fleet work: `9a44483c`
(deploy errors were discarding the server's real message) and `3c79535f` (the
gleam test gate's invisible skips, four more crates).

## Deployed

`staged_rounds` content hash `5f0f3bf19ed9b6d240eb87bfcb3fe55a6d2c10988c84b448f2c240e7b53fd5a3`,
`route_active: true`, on Tom's server (pid 23302, :8080/:50051).
Earlier hash `deb845e4…` was the pre-base-fix version.

Worker release binary built:
`examples/staged-rounds/worker/target/release/staged-rounds-worker`.

Gates at my own seat, not taken on report: `cargo test` 136 passed / 0 failed /
0 ignored; `cargo clippy --all-targets -- -D warnings` exit 0; no
`#[allow]`/`#[expect]`/`#[ignore]`/`cfg(any())`; no `unwrap`/`expect`/`panic`
in src; every file under 500 code lines.

## 🔴 THE BLOCKER: Tom's server cannot attach a worker

`lsof` on pid 23302 shows it listening on **127.0.0.1:8080 and 127.0.0.1:50051
ONLY**. There is no outbox liminal listener, so no remote worker can connect to
it and no `staged_rounds` activity can ever be served there. The workflow
deploys and is visible; it cannot run.

What it needs, in a config FILE:

```toml
[outbox]
enabled = true
transport = "liminal"
liminal_listen_address = "127.0.0.1:50061"
```

Facts established while proving this:

- **There is no `AION_OUTBOX_TRANSPORT` environment override.** The full set is
  ENABLED / POLL_INTERVAL_MS / BATCH_SIZE / MAX_ATTEMPTS / BACKOFF_BASE_MS /
  BACKOFF_MULTIPLIER / BACKOFF_MAX_MS / RECONCILE_INTERVAL_MS /
  RECONCILE_STALE_AFTER_MS / LIMINAL_LISTEN_ADDRESS. So the address can be set
  by env but the transport cannot — env alone can never enable the listener.
  That is a real ergonomic gap worth its own task.
- `outbox.enabled = true` is REFUSED with `store.backend = "memory"`: "the
  durable outbox dispatcher claims rows from the store's outbox table, which
  the in-memory store does not provide". Honest refusal naming the fix.
- `store.backend = "libsql"` is REFUSED by the RELEASE BINARY: "requires the
  aion-server `libsql-backend` feature". The shipped binary does not have it.
- `store.backend = "haematite"` WORKS.
- The path-safety gate refuses a `data_dir` anywhere under `/private/tmp`
  (ancestor mode 1777, "sticky bit is not accepted"). The data dir must sit
  under an owner-only chain.
- A project-local `./aion.toml` and the legacy `./aion-data` directory SHADOW
  `AION_HOME`: the server logged "using legacy store data directory
  `/Users/tom/Developer/ablative/stack/aion/aion-data` instead of new Aion-home
  default …". Anything hermetic must set `store.data_dir` explicitly, or it
  will quietly attach to Tom's real state.

**Restarting Tom's server is HIS call and has not been done.** He has a pending
decision about swapping that server anyway.

## The scratch server IS UP — use it, do not touch pid 23302

- pid **51281**, started from `/Users/tom/Developer/ablative/stack/aion`
- `AION_HOME=/Users/tom/.aion-fleet-smoke` (created 700, with `hdata/` 700)
- config: `<scratchpad>/fleet-smoke/server.toml`
- log: `<scratchpad>/fleet-smoke/server.log`
- HTTP 127.0.0.1:**8099** (health 200), gRPC 127.0.0.1:**50099**,
  liminal 127.0.0.1:**50161** (CONFIRMED LISTENING via lsof)
- store: haematite at `/Users/tom/.aion-fleet-smoke/hdata`
- deploy surface enabled

Where `<scratchpad>` is
`/private/tmp/claude-501/-Users-tom-Developer-ablative-stack-aion/5849e0d8-4802-4869-8e0f-9f7fd187e198/scratchpad`.

CLEAN UP WHEN DONE: kill 51281 and `rm -rf ~/.aion-fleet-smoke`. Only one
console is meant to be live (Tom's :8080).

## Next steps, in order

1. `./target/release/aion --endpoint 127.0.0.1:50099 deploy examples/staged-rounds/awl/staged_rounds.awl`
2. Start the worker against the scratch liminal port:
   ```sh
   cd examples/staged-rounds/worker
   ./target/release/staged-rounds-worker \
     --address 127.0.0.1:50161 \
     --profiles-dir ./profiles \
     --identity fleet-smoke \
     --max-parallel 4 \
     --ready-file /tmp/claude-501/fleet-worker.ready
   ```
   `--profiles-dir` is REQUIRED and must hold all four of planner.md,
   developer.md, reviewer.md, remediator.md (all present). `norn` is on PATH at
   `/Users/tom/.cargo/bin/norn` (0.1.0) and must be logged in through ChatGPT
   OAuth — the worker STRIPS `OPENAI_API_KEY` from the child environment.
   Confirm the ready file appears and the log shows connections registered per
   node.
3. Start a run. Do NOT start the tharsis run first — use a small throwaway repo
   so the first flight proves the wiring, not the plan. Preflight must use the
   exact production argv.
4. Then the tharsis run:
   `aion --endpoint 127.0.0.1:50099 start staged_rounds --input-file examples/staged-rounds/awl/tharsis-input.json`

Watch terminal state with `aion list`, NOT `aion describe` — describe dies on
histories over 4 MiB and a whole-project run will exceed that.

## Tom's open asks, verbatim intent

1. **The full-flight stress test example** — he is out for ~30 min and
   explicitly said do not stop for him: "I've been waiting three days now so
   what's another half an hour."
2. **The AWL worker language / worker stuff.** Task #30 covers AWL-declared
   workers. Note what already exists: an `action` may carry a `run "<command>"`
   body executed by the SERVER with no worker at all, but its result type is
   constrained to a subset of `{exit_code, stdout, stderr}` — so declared
   commands cannot return rich typed results, which is why the gate battery
   needs a worker action rather than a declared body.
3. **SIGNALS — he believes they are not represented in AWL. THEY ARE.**
   `signal <name>: <Type>` in the workflow header and
   `wait <signal> [timeout <duration>] -> <binding>` in a step; with a timeout
   the binding is `T?` so absence is branchable. What does NOT exist is any
   in-language SENDER — no `send`/`emit` statement anywhere in the lexer,
   parser or AST. Signals arrive only from outside, via
   `aion signal <workflow_id> <signal_name> --payload <json>`. So a human
   approval gate is fully expressible today; agent-to-agent or
   workflow-to-child signalling is not. Task #24 is mis-titled as if the whole
   feature were missing; it should be narrowed to the sender.

## Known limits to carry

- `aion describe` dies over 4 MiB of history (runbook task #249).
- Shell activities now report per-gate-command progress, but a single
  10-minute `cargo test` is one beat, not a stream.
- Evidence is clipped at 16 KiB with an explicit TRUNCATED marker.
- `verify.diff` is now the whole project change and the `fix_integration`
  prompt embeds the entire verify object — bounded by the clip, but worth
  knowing on a whole-project run.
- The direct-deploy path REFUSES subflows, substeps, value route payloads,
  dependency-parallel step layers, and any collection-fork body beyond one
  unbound call. `staged_rounds` stays inside that set; the older
  `dev_brief.awl` does not, which is why it still will not direct-deploy.
