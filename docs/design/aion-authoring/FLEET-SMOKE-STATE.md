# Fleet smoke: live state, 2026-07-30 ~05:35Z

The resume point for the fleet-workflow first flight. Supersedes the ~05:10Z
revision of this file; the blocker recorded there has been resolved and TWO
further layers were found underneath it by actually flying the thing.

## What is proven, and what is not

PROVEN on a real server: the AWL document deploys, is visible in the authoring
surface, and its per-node contract is admitted correctly (see #78 below).
NOT YET PROVEN: a single activity has ever executed. No run has been started.
The worker has never completed a registration, so nothing has been served.

## Commits landed (all on main, NOT pushed)

- `e6bf8679` — AWL contract change (gate_item / plan_reviews /
  verify_integration / fix_integration, Lens + GatedItem + ReviewTask types, the
  `unverified` disposition), `tharsis-input.json`, `FLEET-BUILD-DESIGN.md`.
- (worker commit) — the eleven-action worker serving that contract.
- `036f976d` — the integration scope fence compared a tree to itself; the
  document now passes `base_branch` and the worker resolves
  `merge-base(base_branch, HEAD)`.
- `3ac1b3ec` — the previous revision of this file.
- Unrelated, same session: `9a44483c` (deploy errors discarded the server's real
  message), `3c79535f` (the gleam test gate's invisible skips, four crates).

## UNCOMMITTED IN THE TREE — my #78 fix, gates green, awaiting commit

Files: `crates/aion-package/src/compatibility.rs`,
`crates/aion-server/src/worker/contracts.rs`,
`crates/aion-server/src/worker/registry.rs`,
`crates/aion-server/src/api/worker_grpc.rs`,
`crates/aion-server/src/worker/liminal_transport.rs`,
`crates/aion-server/tests/worker_contract_registration.rs`.

Gates at my own seat: aion-package lib 11/11 on the compatibility module;
`aion-server --test worker_contract_registration` 4/4; `aion-server --lib
worker::` 211 passed / 0 failed / 0 ignored. Workspace clippy NOT yet re-run
over these — OWED before commit.

## 🔴 #78 (FIXED, live-proven): admission demanded the WHOLE queue of every connection

`aion_package::contract_diffs` demanded every bodyless action of the queue's
contract from EVERY registering connection, ignoring the node dimension —
although `ActionContract` carries `node` and `ProtoRegisterWorker` carries it on
proto tag 4. The server routes by (namespace × task_queue × node), so a worker
serving several nodes MUST open one connection per node advertising only that
node's actions. Each connection was therefore refused for omitting the OTHER
connections' actions. Every dial refused; the queue unservable; the process
looking merely asleep.

Fix: `contract_diffs` takes the connection's `worker_node` and demands exactly
the REACHABLE set, via `dispatch_can_reach` — the admission-side mirror of the
registry's `worker_matches_node` (an unpinned action reaches every worker; a
pinned one reaches only that node's connections). `validate_worker_contracts`
threads the node through BOTH transports and reuses the registry's own
`optional_node` normalization rather than restating it. The gate keeps its
teeth: any action a dispatch could land on a connection is still demanded of it.

The refusal text was also misleading — `expected "declared"` read as if a
declared body were wanted, and sent me hunting through the unrelated #65 fix. It
now names why the connection owed the action: ``advertised: the action is pinned
to node `shell` `` or "the action is unpinned, so every worker in the pool must
serve it".

LIVE PROOF that the filter works: before the fix, the `planner` connection was
refused for dev_item, fix_integration, fold_phase, gate_item, merge_branches …
(the whole queue, alphabetically). After it, the `planner` connection is refused
for `planner` alone and the `developer` connection for `dev_item` alone — each
connection now judged only on its own node's actions.

## 🔴 THE REMAINING BLOCKER: the example worker advertises NOTHING

All five connections are still refused, now on their own node's actions only,
every diff reading `but worker advertised <missing>`. Cause is entirely inside
`examples/staged-rounds/worker`: it registers all eleven activities through
`ActivityRegistry::register_activity`, which commits a handler but NO wire
descriptor. The SDK says so outright in the doc comment on
`register_activity_with_descriptor` (`crates/aion-worker/src/activity.rs` ~189):
"Registering a handler with no descriptor is what makes a worker invisible to
contract admission — it advertises nothing, and a queue carrying any deployed
contract refuses it."

THE FIX IN FLIGHT (delegated, uncommitted when written): the advertisement must
come from the DECLARATION, never from Rust types. `aion_awl::compile(source,
schema_root)` returns `CompiledWorkflow { contract: PackageContract, .. }` whose
`workers[].actions[]` carry the exact `input_schema`/`output_schema` the server
deployed and admits against. The worker embeds the `.awl` with `include_str!`,
compiles it once at startup, and registers every handler through
`register_activity_with_descriptor` with its declared descriptor — handlers keep
their concrete Rust types, only the advertisement becomes exact, and drift is
structurally impossible. Deriving with `schemars` was considered and REJECTED:
it reintroduces precisely the drift the contract gate exists to catch.

Two startup guards are part of the fix, both loud and named: a registered
handler the document declares no action for, and a declared bodyless action with
no handler (actions carrying a declarative `body` are server-executed and must be
excluded, exactly as `contract_diffs` excludes them).

## The scratch server IS UP — do not touch Tom's pid 23302

- pid **16798**, running `./target/release/aion` REBUILT at 15:29 with the #78 fix
- `AION_HOME=/Users/tom/.aion-fleet-smoke` (700, `hdata/` 700)
- config `<scratchpad>/fleet-smoke/server.toml`, log `<scratchpad>/fleet-smoke/server.log`
  (the pre-fix log is kept as `server.log.pre-nodefix` — it holds the original
  whole-queue refusals, which are the evidence for #78)
- HTTP 127.0.0.1:**8099** (health 200), gRPC **50099**, liminal **50161** — all
  three CONFIRMED listening via lsof
- haematite store at `~/.aion-fleet-smoke/hdata`; deploy enabled
- `staged_rounds` deployed, hash
  `5f0f3bf19ed9b6d240eb87bfcb3fe55a6d2c10988c84b448f2c240e7b53fd5a3`
  (redeploy after restart returned `freshly_loaded: false` — the durable store
  carried it across the restart, which is itself worth knowing)

`<scratchpad>` =
`/private/tmp/claude-501/-Users-tom-Developer-ablative-stack-aion/5849e0d8-4802-4869-8e0f-9f7fd187e198/scratchpad`.

CLEAN UP WHEN DONE: kill 16798, `rm -rf ~/.aion-fleet-smoke` (this also removes
the throwaway smoke repo below). Only Tom's :8080 console is meant to be live.

## Tom's server still cannot attach a worker — HIS CALL, untouched

pid 23302 listens on 127.0.0.1:8080 and 127.0.0.1:50051 ONLY. No outbox liminal
listener, so no worker can connect and no activity can be served there. Needs,
in a config FILE:

```toml
[outbox]
enabled = true
transport = "liminal"
liminal_listen_address = "127.0.0.1:50061"
```

plus a restart. He also has a pending decision about swapping that server, so
this has NOT been done.

Facts established while proving it:

- **There is no `AION_OUTBOX_TRANSPORT` env override.** The set is ENABLED /
  POLL_INTERVAL_MS / BATCH_SIZE / MAX_ATTEMPTS / BACKOFF_BASE_MS /
  BACKOFF_MULTIPLIER / BACKOFF_MAX_MS / RECONCILE_INTERVAL_MS /
  RECONCILE_STALE_AFTER_MS / LIMINAL_LISTEN_ADDRESS. The address can be set by
  env, the transport cannot — so env alone can never enable the listener. Worth
  its own task.
- `outbox.enabled = true` is refused with `store.backend = "memory"`, honestly,
  naming the fix.
- `store.backend = "libsql"` is refused by the RELEASE BINARY: it lacks the
  `libsql-backend` feature. `haematite` works.
- The path-safety gate refuses a `data_dir` under `/private/tmp` (ancestor mode
  1777, "sticky bit is not accepted"). The data dir needs an owner-only chain.
- A project-local `./aion.toml` and the legacy `./aion-data` directory SHADOW
  `AION_HOME`. Anything hermetic must set `store.data_dir` explicitly or it will
  quietly attach to Tom's real state.

## The throwaway smoke target

`/Users/tom/.aion-fleet-smoke/smoke-repo`, git-init'd on `main`, one commit
`815a120`: a tiny `smoke-target` crate with `add()` and a README asking for `sub`
and `mul` with unit tests. Fly THIS before tharsis — the first flight must prove
the wiring, not the plan.

## Next steps, in order

1. Review and land the delegated worker-advertisement fix; rebuild the worker
   release binary.
2. Re-run workspace clippy over the #78 files, then commit #78.
3. Start the worker against the scratch liminal port and CONFIRM REGISTRATION —
   the ready file appears AND the server log shows five connections registered,
   one per node. Do not proceed on the ready file alone; it has appeared before
   while every connection was being refused in a retry loop.
4. Start a run against the smoke repo. Preflight with the exact production argv.
5. Only then the tharsis run:
   `aion --endpoint 127.0.0.1:50099 start staged_rounds --input-file examples/staged-rounds/awl/tharsis-input.json`

Watch terminal state with `aion list`, NOT `aion describe` — describe dies over
4 MiB of history and a whole-project run will exceed it.

Worker start line:

```sh
cd examples/staged-rounds/worker
./target/release/staged-rounds-worker \
  --address 127.0.0.1:50161 \
  --profiles-dir ./profiles \
  --identity fleet-smoke \
  --max-parallel 4 \
  --ready-file <scratchpad>/fleet-smoke/worker.ready
```

`--profiles-dir` is REQUIRED and must hold planner.md, developer.md, reviewer.md
and remediator.md (all four present). `norn` is on PATH at
`/Users/tom/.cargo/bin/norn` (0.1.0) and must be logged in through ChatGPT OAuth
— the worker STRIPS `OPENAI_API_KEY` from the child environment.

## Tom's open asks

1. **The full-flight stress test.** He is out and said explicitly not to wait:
   "I've been waiting three days now so what's another half an hour."
2. **The AWL worker language / worker stuff** (task #30). Note what already
   exists: an `action` may carry a `run "<command>"` body executed by the SERVER
   with no worker at all, but its result type is constrained to a subset of
   `{exit_code, stdout, stderr}` — which is exactly why a rich gate battery needs
   a worker action rather than a declared body. The declared-body exclusion in
   contract admission is the same seam.
3. **SIGNALS — he believes they are not represented in AWL. THEY ARE.**
   `signal <name>: <Type>` in the header and
   `wait <signal> [timeout <duration>] -> <binding>` in a step; with a timeout the
   binding is `T?`, so absence is branchable. What does NOT exist is any
   in-language SENDER — no `send`/`emit` anywhere in the lexer, parser or AST.
   Signals arrive only from outside via
   `aion signal <workflow_id> <signal_name> --payload <json>`. A human approval
   gate is fully expressible today; agent-to-agent or workflow-to-child
   signalling is not. Task #24 is mis-titled as if the whole feature were
   missing and should be narrowed to the sender.

## Known limits to carry

- `aion describe` dies over 4 MiB of history (runbook task #249).
- Shell activities report per-gate-command progress, but a single 10-minute
  `cargo test` is one beat, not a stream.
- Evidence is clipped at 16 KiB with an explicit TRUNCATED marker.
- `verify.diff` is the whole project change and the `fix_integration` prompt
  embeds the entire verify object — bounded by the clip, but worth knowing on a
  whole-project run.
- The direct-deploy path REFUSES subflows, substeps, value route payloads,
  dependency-parallel step layers, and any collection-fork body beyond one
  unbound call. `staged_rounds` stays inside that set; the older `dev_brief.awl`
  does not, which is why it still will not direct-deploy.

## Process lesson banked this session

A compaction can carry FORWARD a fix while STRIPPING the evidence for it, so a
seat can honestly disown numbers it genuinely produced. Tonight I could not
attest to a battery result my own transcript showed my seat sending four hours
earlier (line 62556, 01:56:11Z). The defence is to resolve the artifact, not the
memory — and, for my own summaries, to carry battery results and any
deliberately-left running process forward explicitly. Hence the "what is proven"
section at the top of this file.
