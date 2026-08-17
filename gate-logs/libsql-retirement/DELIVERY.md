# aion#35 libsql-retirement lane record

Baseline: `00064c41c`. Census-first commit: `dc7d2933e`. Red-first commit: `5179b58d6`.

> **Rebased 2026-08-17 onto the R3 lane (`d4317268f`), branch `retire-libsql`.**
> The five commits above were replayed there; their SHAs on the landing branch
> differ. Four things changed in the replay and are NOT described below: the
> fixture constructor now takes R3's required node cache budget; the
> `haematite-backend` feature is removed outright rather than left declared and
> empty; the retired-input wire carries `store.node_cache_budget` again (the
> pre-budget wire struct had silently narrowed the config surface); and the
> operator runbook gained the retired-input refusal table. The battery of
> record for the rebased tree is `gate-logs/35-retire-libsql/`.

## Boundary

The retired inputs (`backend = "libsql"`, `store.url`, `AION_STORE_BACKEND=libsql`, `AION_STORE_URL`, and `--store-url`) converge on `ServerConfig::validate`'s single `retired_input` refusal. The environment backend door was a fifth found in the rebase — it had been reaching `parse_store_backend`'s unknown-variant list, which names no remedy. The refusal names the input, prescribes `backend = "haematite"` plus `store.data_dir`, and states that this build does not read existing libsql database files. There is no fallback or migration.

## Test-file dispositions

Migrated to haematite because their subject requires durable restart, failover, or outbox semantics:

- `crates/aion-server/tests/deploy_restart_e2e.rs`
- `crates/aion-server/tests/lsub3_failover_e2e.rs`
- `crates/aion-server/tests/namespace_restart.rs`
- `crates/aion-server/tests/graceful_shutdown_park_e2e.rs`
- `crates/aion-server/tests/lsub1_xnode_dispatch_e2e.rs`
- `crates/aion-server/tests/outbox_delivery_contract_e2e.rs`
- `crates/aion-server/tests/outbox_transport_e2e.rs`
- `crates/aion-server/tests/recovery_declared_body_e2e.rs`
- `crates/aion-server/tests/run_server_outbox_e2e.rs` and `run_server_outbox_support/helpers.rs`
- `crates/aion/tests/outbox_e2e.rs`
- server unit coverage in `cluster.rs`, `run.rs`, `instrumented_store.rs`, `worker_deployments_tests.rs`, and `outbox_dispatcher.rs`

The deleted crate-owned suites die with their only subject and are not replacements for shared conformance: `conformance.rs`, `outbox_hold.rs`, `persistence.rs`, and `replica_sync.rs` under `crates/aion-store-libsql/tests/`.

`worker_deployment_spawn_fence.rs` removes only the deleted backend source regions; its memory, haematite, wire, HTTP, state, and supervisor fences remain.

## Conformance

The shared conformance contract remains implemented by the in-memory store and by `aion-store-haematite/tests/conformance.rs`. The deleted libsql conformance leg had no unique shared assertion.

## Published-consumer boundary

This build does not read libsql database files and the next cut publishes 19 crates, down from 20. 0.17.0 is the last release that carries the backend (verified: `git ls-tree v0.17.0 crates/` still contains `aion-store-libsql`), so consumers with libsql data must retain a 0.17.0 binary to access it. No published versions are yanked.

## Migration choices discovered by the green battery

- Haematite exposes one live-writer handle per database. Migrated tests now share that handle (or read through `Engine::store`) instead of opening a second writer as libsql permitted.
- Fan-out history can become visible before every routed haematite outbox KV is observable. `aion/tests/outbox_e2e.rs` therefore uses its existing bounded pending-row poll before asserting all four rows; the tests still require every row to exist as `Pending` and still drive the production claim/settle path.
- Dispatcher retry tests assert the durable transition itself rather than elapsed wall-clock time: generic failure records a future `visible_after`, while connection loss is proved immediately claimable. No production backoff constant changed.
- The fixture-only `HaematiteStore::open_or_create` constructor performs its blocking filesystem work on `spawn_blocking`; no runtime store behavior, default, cap, shard count, or tuning value changed.

## Verification record

The battery of record is `gate-logs/35-retire-libsql/`, run on the rebased tree:
the canonical three-leg extraction (check, clippy, nextest) plus named
per-feature clippy legs for `--no-default-features` / `--all-features` on
aion-server and aion-cli, since this lane deletes a cargo feature and the
canonical legs compile only the default set. It carries `findings.jsonl`,
per-leg stderr, the nextest Summary line, the extraction's own red proof
(`instrument-proof.jsonl`), and the refusal's red/green mutation pair
(`red-mutation-refusal.log` / `green-refusal.log`).

## Non-goals

This subtraction does not fix or claim to fix aion#32. It adds no migration tool, compatibility shim, fallback, wire change, SDK change, or haematite tuning constant.
