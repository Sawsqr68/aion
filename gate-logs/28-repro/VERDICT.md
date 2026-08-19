# aion#28 re-measurement on the current tree — non-repro, banked with its scope

**Date:** 2026-08-18 ~05:37Z · **Measured by:** Vesper Lynd (steward hands)
**Binary:** ~/.cargo/bin/aion (R3/0.8.3 landing lineage, d4317268f); the #1
correlation rebuild (1e20aea43) postdates this binary — scope note 3.

## The measurement

Fresh engine (scratch server, fresh v2 store at ~/.aion/repro28-scratch,
ports 18080/15051/15061, outbox transport grpc), workflow `repro28` carrying
the issue's named candidates minus filter/count: custom struct types, 3-way
fork/join of worker-served actions, when/otherwise outcomes, second
dispatching step after the join. Activities served by a real external worker
(`aion worker awl repro28_worker.awl`, gRPC, concurrency 4) — the issue's
failing runs had served activities, so an unserved arm cannot reach the
mechanism.

**Result: 11 consecutive runs (1 control + m1..m10) ALL Completed.** No
run-6 failure, no "await_activity_result: invalid correlation id", normal
dispatch throughout and after. The issue's signature did not appear.

## Scope, stated honestly

1. The original 0.14.1 failing document (with filter/count and its exact
   structure) was unavailable; the issue's OWN synthetics never reproduced
   either — a synthetic non-repro is bounded evidence, not proof of absence.
2. Changed since 0.14.1 in the implicated machinery: #1 rebuilt correlation
   resolution (resolve split, run-scoped position cell, settle_race); #47
   (in flight) corrects settlement terminality. The issue's log lines point
   exactly into the rebuilt zone.
3. The serving binary predates #1's landing; on the next binary swap this
   arm re-runs (recipe below).
4. Observed during setup, separate signature, noted not claimed: one START
   refusal on the LIVE server under full battery load ("timed out waiting
   for shard actor 34") — store-contention shape, not the wedge; watching.

## Also learned (rows/notes)

- Declared-body FAN-OUT (fork) dispatches through the outbox to a worker
  pool; sequential bodies run in-server. With `[outbox] transport =
  "liminal"`, an `aion worker awl` worker (gRPC-only, no --liminal-address)
  can NEVER serve fork rows ("selected worker is not delivered over
  liminal"). An authoring surface that compiles but cannot be served on the
  default estate config — row-worthy paper cut.
- A server started from a cwd without ./aion-data falls back to
  ~/.aion/data, which on this box holds a PRE-BUDGET database → open fails
  "config is missing node_cache_budget". Any fresh `aion server` from an
  arbitrary cwd dies on an old store it never asked for. Row-worthy.
- The path-safety gate refuses /private/tmp stores (mode 1777 ancestor) —
  correct; scratch recipes use ~/.aion/<name>.

## Repro recipe (re-run on next binary)

scratchpad/repro28/{config.toml,repro28.awl,repro28_worker.awl} (awl copies
beside this file); server: `aion server --config config.toml` (cwd = that
dir); worker: `aion worker awl repro28_worker.awl --endpoint
http://127.0.0.1:15051 --identity repro28-worker --concurrency 4
--reconnect-initial-backoff 1 --reconnect-max-backoff 10
--reconnect-max-attempts 50`; then 10× `aion start repro28 --input
'{"tag":"mN"}' --endpoint 127.0.0.1:15051`.
