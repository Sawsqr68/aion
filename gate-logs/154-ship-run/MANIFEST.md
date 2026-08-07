# #154 — ship-and-run availability pre-flight: evidence

Seat: Vesper Lynd, 2026-08-08 (UTC timestamps 2026-08-07T21:1x-21:3xZ).
Fix under review in crates/aion-server/src/awl/run_loop.rs (uncommitted at
capture time; the postfix server banner carries build
`0.11.0+eee1f3510…-dirty` — the tree state, honestly named).

## The operator's failure (read-only probes of Tom's tutorial server, :8099, pid 50822)
- `tom-server-availability-8099.json` — queue `gate`: available:false,
  connected_workers:0, scaffold hint directing the operator to build a worker.
  (Captured post-incident; his server still runs installed 0.11.0, pre-fix, so
  this is the same answer his Ship & Run received.)
- `tom-server-documents-8099.json` — repo_gate.awl authored in his console
  workspace. His deploy is version e7e89ddd… (route_active, loaded
  2026-08-07T01:57:18Z per `aion versions`); the server's run list holds ZERO
  repo_gate runs after that deploy — the panel refused at `start`, before
  startWorkflow.

## Pre-fix reproduction + counterfactual (isolated world, :8098, installed aion 0.11.0)
- `prefix-deploy.json` — deck bytes deploy (hash 9926c2fe…, the tutorial's own).
- `prefix-availability-false.json` — same false answer + scaffold hint.
- `prefix-counterfactual-start.json` / `prefix-counterfactual-completed.txt` —
  the exact start the panel refuses, issued anyway: repo_gate Completed in
  ~50s with zero workers connected. The pre-flight refused a start that
  succeeds; the hint prescribed a worker the document cannot use.

## Post-fix proof (same world, patched debug binary)
- `postfix-availability-true.json` — queue `gate`: available:true,
  connected_workers:0, worker_actions:[],
  server_run_actions:["checks","clone","head","tests"].
- `probe_worker.awl` + `postfix-control-bodyless-false.json` — the control:
  a deployed document whose one action has NO body; its queue `probeq`
  still answers available:false with the unserved hint. Both arms driven live.
- `postfix-start.json` / `postfix-both-runs-completed.txt` — the panel-sequence
  start on the patched server: second repo_gate run Completed 21:33:48Z.

## Gates (at capture, tree = fix + tests + docs)
- `gates-1.log` — cargo test -p aion-server --lib run_loop: 10 passed / 0
  failed (five new tests named); clippy -p aion-server --all-targets -D
  warnings: 0; clippy --workspace --all-targets -D warnings: 0;
  cargo fmt --check: 0.
- `gates-2.log` — availability-filter tests 2/0 (incl. the auth-off router
  test through the changed handler); cargo build -p aion-cli: 0.
- NOT run here: the `#[cfg(feature = "auth")]` availability router test
  (feature-gated, needs a live JWKS endpoint; compile-checked by clippy
  --all-targets only). Full-feature batteries run on Dean's box.

## R2 — the Opus R1 refusal (1 BLOCKER / 2 MAJOR / 4 minor) fixed in substance

Fixes (UTC 2026-08-07T21:5x-22:1xZ, tree at `75a86419b`-dirty): census-based
supply per `(action, node)` dispatch address via `WorkerRegistry::pool_census`
(M2); endpoint-level tokio tests over real compiled packages, a real engine,
and real registrations (B1); `RunLoopError::EngineUnavailable`/`QueueContracts`
now carry typed `#[source]` causes, chain-rendered at the wire (m4);
`ActionContract::worker_owed()` is the ONE place the body rule lives — seven
sites migrated to it (m7); console `requireServedQueue` rename (m5);
CHANGELOG entry (m6); API.md + response docs state the per-queue scope
explicitly, and the multi-queue pre-flight gap is filed as task #158 and
routed to Waffles (M3).

- `154-r2-check.log` — cargo check -p aion-package -p aion-server -p aion-cli
  --all-targets: 0.
- `154-r2-tests.log` — run_loop filter 17 passed / 0 failed (6 endpoint tests
  + 6 pure + hint suite); aion-package full suite 309/0 + awl_worker_build
  1/0 (66s — compiles the generated scaffold, proving the migrated template
  emits code that builds); aion-cli worker_surface filter matched 0 tests
  (stated: no test carries that name; the call sites are covered by the
  shell/agent/scaffold filters below).
- `154-r2-tests-cli.log` — aion-cli worker_shell 9/0, worker_agent 18/0,
  scaffold 22/0 (+ integration bins).
- `154-r2-clippy.log` — clippy -p (three crates) -D warnings: 0; clippy
  --workspace --all-targets -D warnings: 0; fmt --check: red then
  `cargo fmt` (autoformat rule) then 0.
- `154-r2-mut1.log` — THE R1 BLOCKER MUTATION re-run at my own hand: literal
  pre-fix text `available: connected_workers > 0` → suite RED, 4 endpoint
  tests fail (13 passed / 4 failed). The mutation that survived 803/0 in R1
  is now killed.
- `154-r2-mut2.log` — field-swap mutation (worker_actions ↔
  server_run_actions) → RED, 2 endpoint tests fail (15/2). Nothing read the
  new fields in R1; now something does.
- `154-r2-restored.log` — bytes restored by writing (not mv), rebuilt tree
  green 17/0; `grep` confirms neither mutation text remains.
- `availability-r2-gate.json` — live wire, rebuilt binary (banner
  `0.11.0+75a86419b…-dirty`, server pid 85521): queue `gate` available:true,
  connected_workers:0, exact action lists.
- `availability-r2-control.json` — control arm live: `probeq` available:false
  + unserved hint, under the census implementation.
- `start-r2.log` — the previously-refused start, run again on the rebuilt
  binary: repo_gate outcome `passed` end-to-end (clone, head=1577ed9, clippy,
  13 tests) with zero workers connected.
- Node-pinned arm: proven at endpoint level (`a_node_pinned_action_is_not_
  served_from_the_wrong_node` — wrong node refused with connected_workers:1,
  right node serves), against the SAME `register_namespaces` path the gRPC
  transport uses. Not driven over the live wire: no shipped worker surface
  takes a node pin from the CLI, and a bespoke binary would prove the test's
  own plumbing, not the server's. Stated, not hidden.

## R3 — the R2 refusal (1 new MAJOR + 1 new MINOR) fixed

The M2 census made `unserved && connected_workers > 0` reachable and the hint
answered it with "No connected worker" — contradicting the count beside it.
Fix per the reviewer's remedy: the first unserved `(action, node)` address and
its `PoolCensus` now travel from the availability decision into
`unserved_hint`, which distinguishes four worlds: refusal on record
(unchanged); empty pool (unchanged scaffold text); pool connected but action
unadvertised (names the action, says starting another copy cannot help);
advertised from the wrong node (names the action, the pin, and the remedy).
The two endpoint tests for the new census arms now assert the hint TEXT, not
just `available`. MINOR: the `available` field doc, API.md row, and CHANGELOG
now state that only declaration-level node pins are visible — a call-site
`node` override can still park (platform-wide with admission; #158 carries
the family of pre-flight blind spots).

- `154-r3-tests.log` — run_loop filter 19 passed / 0 failed (2 new pure hint
  arm tests; both census-arm endpoint tests now read the message).
- `154-r3-gates.log` — clippy -p 0; clippy --workspace 0; fmt 0; build 0.
- `availability-r3-gate.json` / `availability-r3-control.json` — both arms
  re-driven live on the r3 binary (server pid 83909, :8098): identical
  payloads to r2 (the control's empty-pool hint text is unchanged by design).
- Scope of the live proof: both live probes are 0-worker cases and therefore
  render arm 2. The two NEW hint arms (action-unadvertised, wrong-node) are
  proven at endpoint level — real `ServerState`, real registry, real compiled
  packages — but have not been observed over HTTP; the only untested layer for
  them is JSON serialization of a `String` field that round-trips in the other
  arms. R3 review (2026-08-08): CLEAR — reviewer re-injected the R2 defect
  (both census arms) at their own hand, RED both times, restore green 19/0.
