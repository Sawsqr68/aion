# aion#35 libsql-retirement lane record

Baseline: `00064c41c`. Census-first commit: `dc7d2933e`. Red-first commit: `5179b58d6`.

## Boundary

The four retired inputs (`backend = "libsql"`, `store.url`, `AION_STORE_URL`, and `--store-url`) converge on `ServerConfig::validate`'s single `retired_input` refusal. The refusal names the input, prescribes `backend = "haematite"` plus `store.data_dir`, and states that this build does not read existing libsql database files. There is no fallback or migration.

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

0.16 does not read libsql database files and publishes 21 crates. Consumers with 0.14/0.15 libsql data must retain a 0.15 binary to access it. No published versions are yanked.

## Non-goals

This subtraction does not fix or claim to fix aion#32. It adds no migration tool, compatibility shim, fallback, wire change, SDK change, or haematite tuning constant.
