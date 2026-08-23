# CAUSE — round 5's red: `B must hold exactly the replicated WorkflowStarted`

## The red, verbatim

```
thread 'inflight_workflow_completes_on_survivor_after_owner_death' (3427306)
panicked at crates/aion/tests/aion_cluster_failover_showcase.rs:616:5:
assertion `left == right` failed: B must hold exactly the replicated WorkflowStarted
  left: 0
 right: 1
error: test failed, to rerun pass `-p aion-rs --test aion_cluster_failover_showcase`
GATE-RED: workspace suite
```

`.fleet/gates/red-5.md:3910-3939` (= `RED-LATEST.md`). It is the ONLY failure in
the round-5 gate: fmt clean, clippy clean, every other target green.

## The named cause: a false synchrony premise in the test, not a defect in the engine

ACT 1 of the stretch test appends `WorkflowStarted` on owner A and then, with no
wait, reads survivor **B's** store directly and requires the event to be there:

```rust
// The start replicates to the quorum {B,C} as it is appended on A.
runtime.block_on(a_store.append(WriteToken::recorder(), &workflow_id, &[start], 0))?;
...
let pre = runtime.block_on(b_store.read_history(&workflow_id))?;
assert_eq!(pre.len(), 1, "B must hold exactly the replicated WorkflowStarted");
```

That comment states a guarantee the cluster does not make. `HaematiteStore::append`
routes through `Database::replicate_append`
(`crates/aion-store-haematite/src/store.rs:1570-1600`), which commits on a **strict
majority counting the proposer's own ack**:

* `haematite-0.11.0/src/sync/consistency.rs:223` — `quorum_size(3) == 2`
  (asserted by haematite's own `consistency_tests.rs:21`).
* `consistency.rs:490-560` — `wait_for_cas_quorum` returns the moment
  `acknowledged >= required`, where `acknowledged` starts at the local ack.
* `endpoint.rs:704-723` — the batch proposal is **fire-and-forget** to *every*
  send target; there is no wait for the peers that did not carry the quorum.

So `append` returns when **A plus EXACTLY ONE of {B, C}** has committed. Which one
is a race. The loser applies the identical batch on its next
`respond_to_inbound_writes` drain — the harness's responder thread, polling with a
50 ms timeout. Reading a *named* survivor the instant the append returns therefore
samples a coin-flip that the protocol never promised to win.

## The measurement

`PROBE-quorum-asymmetry.txt` — a temporary probe (reverted; it is not in the tree)
that reads BOTH peers at the assertion point, 30 runs of the whole binary:

| observation at the instant `append` returned | runs |
|---|---|
| `B=1 C=1` — both had applied before the probe's own read | 25 |
| `B=0 C=1` — **C alone carried the quorum; B held nothing** | 4 |
| `B=1 C=0` — **B alone carried it; C lagged** | 1 |

The mirrored case is the proof that neither peer is privileged: the test failed
only because it happened to name the peer that lost. The loser converged 295 us to
5.4 ms later, every time.

## The discrimination: NOT introduced by this flight

Two arms, same repro loop (whole test binary, default parallelism — the gate's own
shape; the two cluster showcases run concurrently in one process):

| arm | tree | binary sha256 (head) | result |
|---|---|---|---|
| A | this flight's HEAD | `c7f98920…` | **1 FAIL / 15** (`ARM-A-flight-tree.txt`) |
| B | base `c590712f`, flight diff ABSENT | `24ca7b11…` | **2 FAIL / 30** (`ARM-B-base-c590712f.txt`) |

The identical failure reproduces with the flight diff absent, so the flight did not
introduce it. That is consistent by construction: the failing assertion executes
**before any engine is built**, so none of the flight's `crates/aion/src/runtime/nif_*.rs`
changes are on its path, and `crates/aion/tests/` and `crates/aion-store-haematite/`
are untouched by `c590712f..HEAD`. In isolation the test is 32/32 green — the
variable is concurrent execution, which is why the race surfaced at the gate and
never under `--exact`.

## The fix: correct the premise, keep every assertion

`await_replicated` (in the same harness, beside the existing `wait_until`) re-reads
the named survivor until it holds the committed history, bounded by the same
`OP_TIMEOUT` that bounds every other cluster operation here, and errors by peer name
and count if it never converges. Both survivor reads in the file now go through it —
the stretch test's ACT 1 and the primary demo's ACT 2, which carried the identical
false premise one event later and was a latent red of the same kind.

Nothing is suppressed, skipped, relaxed or removed:

* Every original assertion still runs on the returned history — `len() == 1`,
  `matches!(.., WorkflowStarted)`, `status == Running`, and in the primary demo the
  full `history_on_b == history_on_a` equality.
* The proof is STRICTLY STRONGER than a bare quorum check: it still demands the
  state reach **B specifically**, and a B that never receives it fails the test.
* What changed is only the false claim that it must have arrived *already*.

Waiting on convergence rather than on an instant is the honest reading of the
guarantee: quorum commit says a majority holds it and every in-quorum replica
converges — it never said which replica was in that majority.
