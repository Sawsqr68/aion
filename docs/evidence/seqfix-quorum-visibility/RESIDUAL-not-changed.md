# Residual, reported and NOT changed: the same premise in `distributed_failover.rs`

## The observation

`crates/aion-store-haematite/tests/distributed_failover.rs:288-298`
(`workflow_history_replicates_to_followers`) carries the identical false premise —
and a strictly harder version of it:

```rust
let node_a = Node::spawn(NODE_A, dir_a.path(), 3, &[NODE_B, NODE_C])?;
...
block_on(node_a.store.append(WriteToken::recorder(), &workflow_id, &history, 0))?;
...
for node in [&node_b, &node_c] {
    let replicated = block_on(node.store.read_history(&workflow_id))?;
    assert_eq!(replicated, history, "follower {} must read the full replicated ...", node.name);
}
```

Send targets are `{B, C}` with `total_nodes = 3`, so `quorum_size(3) == 2` — A plus
**one** peer. This asserts **both** followers instantly, so unlike the showcase it
loses whenever *either* peer lags, not just a named one.

The other two gates in that file are **sound** and were re-read to confirm it:
`workflow_history_survives_owner_failover` and
`lagging_node_without_writes_reads_empty_history` both spawn with
`send_targets = &[NODE_B]`, so C is never a replication target at all — their
`is_empty()` assertions on C are a real guarantee, not a race.

## Why it was NOT changed

It does not fail. Measured on this tree, whole binary, default parallelism:

| condition | result |
|---|---|
| 40 consecutive runs, unloaded | 40 PASS / 0 FAIL |
| 30 consecutive runs, 48 busy processes on 16 cpus | 30 PASS / 0 FAIL |

**70/70 green. No reproduction, so no fix.** This flight's own law is explicit
that a fix must answer a measured red — the round-3 report landed as
*"the cause named from measurement, no fix manufactured"* (60d4b8b6), and the
red-first order requires the failure reproduced on the pre-fix tree before any
change. Editing a green test in a second crate on a reading-level argument alone
would be a change with no red to answer, and it would widen this round's changed
paths past the cause it is meant to fix.

So it is reported here instead, with everything needed to act on it:

* the premise is false **by construction** — `quorum_size(3) == 2`
  (`haematite-0.11.0/src/sync/consistency.rs:223`), proposals are fire-and-forget
  to every send target (`endpoint.rs:704-723`);
* the fix, if a red ever appears, is mechanical and already written: the
  `await_replicated` shape now in `crates/aion/tests/aion_cluster_failover_showcase.rs`;
* the reason it survives where the showcase did not is most likely load shape —
  that binary runs no beamr VM and no Gleam build, and its three adapter reads
  (A, then B, then C) each give the lagging peer more slack than the showcase's
  single immediate read.

This is a live latent flake in a currently-green test. It is the estate's call
whether to pre-empt it or wait for a red to justify the change.
