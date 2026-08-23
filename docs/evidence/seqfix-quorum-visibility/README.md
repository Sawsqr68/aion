# seqfix round 5 — the survivor-visibility race in the cluster failover showcase

Round 5's gate was RED on exactly one target:

```
-p aion-rs --test aion_cluster_failover_showcase
inflight_workflow_completes_on_survivor_after_owner_death
  assertion `left == right` failed: B must hold exactly the replicated WorkflowStarted
    left: 0   right: 1
```

**Cause:** the test asserted a synchrony the cluster never promised. A 3-node
quorum is `quorum_size(3) == 2` — owner A plus **one** of {B, C} — so when
`replicate_append` returns, the peer that did not carry the quorum has not
necessarily applied the batch yet. The test named B and read it instantly.

**Fix:** `await_replicated` — the survivor read waits for the named peer to
converge, bounded by the harness's existing `OP_TIMEOUT`, and fails by peer name
and count if it never does. Every original assertion still runs, on the same
history. Applied to both survivor reads in the file; the primary demo's ACT 2
carried the identical premise and was a latent red of the same kind.

**Not this flight's doing:** the failure reproduces on base `c590712f` with the
flight diff absent (arm B). Full reasoning in `CAUSE.md`.

## The record

| file | what it is |
|---|---|
| `CAUSE.md` | the named cause, cited to haematite's own quorum code, and why the fix is not a relaxation |
| `RAW-arm-a-failing-run.log` | raw failing run, this flight's tree — byte-identical panic to the gate's |
| `RAW-arm-b-base-failing-run.log` | raw failing run at base `c590712f`, flight diff absent |
| `ARM-A-flight-tree.txt` | 1 FAIL / 15 on HEAD, plus the isolation control (32/32 green alone) |
| `ARM-B-base-c590712f.txt` | 2 FAIL / 30 at base — the two-arm discrimination |
| `PROBE-quorum-asymmetry.txt` | temporary both-peers probe (reverted): C alone carried the quorum 4/30, B alone 1/30 |
| `EIGHTY-FIVE-GREENS.log` | post-fix: 60 runs at default parallelism + 25 under 10x CPU load, 0 failures |
| `RESIDUAL-not-changed.md` | the same premise found in `distributed_failover.rs`, measured 70/70 GREEN — reported, deliberately not changed |
| `GATE-RUN-1.log`, `GATE-RUN-2-COLD.log` | the full gate on the final tree, twice; run 2 after `cargo clean` |

## How to re-derive it

The race only shows when the two cluster showcases run **concurrently in one
process** — which `cargo test --workspace` does and `--exact` does not. Run the
whole binary in a loop; do not filter to the single test.

```
cargo build -p aion-rs --test aion_cluster_failover_showcase
for i in $(seq 1 30); do ./target/debug/deps/aion_cluster_failover_showcase || echo "FAIL $i"; done
```
