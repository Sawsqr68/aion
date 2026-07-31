# #118 — main was RED under `--all-features`, and what it was hiding

Status at time of writing: **fixed, gated, not yet committed.** Evidence logs under
`.evidence/118/`.

## What was red

`cargo test -p aion-server --all-features` failed four tests, all with the same
refusal at the HTTP start boundary:

```
409 Conflict  NO_QUEUE_DECLARATION: workflow `aion_outbox_fixture`
version `e2e7a414…` has unscoped activities fixture_activity;
re-deploy from a checked AWL contract
```

- `run::lsub_prod_xnode_e2e::production_boot_dispatches_executes_and_records_over_liminal` (lib, `crates/aion-server/src/run.rs`)
- `run_server_outbox_happy_path_fan_out_completes_once`
- `run_server_outbox_restart_rearms_stranded_rows`
- `run_server_outbox_live_reconciliation_rearms_stranded_claims_once`

The reported scope was ONE test. It was four. Red evidence banked in
`56301734` before any code was touched.

## The refusal was correct. The fixtures were stale.

`PackageContract::from_manifest` deliberately "never invents a queue", so a
manifest's bare activity names land in `unscoped_activities`, and under
queue-routed serving `aion::lifecycle::start_admission` refuses them. That
refusal is EARNED — an unserved queue waits silently forever.

Both fixtures also named an activity, `fixture_activity`, that `collect_four`
has never scheduled: the fixture fans out `fan:0..3`. The sibling harness
(`crates/aion-cli/tests/common/osproc.rs`) had already been corrected, with a
doc comment saying exactly this. The correction was never carried across, and
nothing caught it because of the gate trap (#119): these targets only compile
under `--all-features`.

Fix, in both places: declare the four real ordinals on the queue the worker
polls, via `PackageBuilder::with_contract`, with the action schemas derived from
`activity_descriptor::<String, String>` — the SAME source the worker advertises
from, so the two sides cannot drift and `WORKER_CONTRACT_MISMATCH` compares two
schemas with one origin. The gRPC harness worker had to start advertising typed
contracts too (`activities: Vec::new()` is names-only, which is inadmissible).

## 🔴 What the red was hiding: a P0 I introduced in the #58 fix

With the start refusal cleared, the test got further and then hung: all four
dispatches were received by the worker and **none executed**.

```
11:31:07.978  received dispatch push  activity_type=fan:1 …
11:31:07.979  received dispatch push  activity_type=fan:2 …
11:31:07.979  received dispatch push  activity_type=fan:0 …
11:31:07.979  received dispatch push  activity_type=fan:3 …
  (20 seconds of nothing)
```

**Mechanism.** The #58 fix moved plain dispatches off the serve loop with
`tokio::spawn`. But the serve loop's receive, `PushClient::recv_timeout`, is a
BLOCKING call — the push client is thread-based, not async — and
`serve_with_redial` (`crates/aion-worker/src/runtime/liminal_serve.rs:104`),
**the entry point every real worker uses**, drives that loop with `block_on` on
a **current-thread** runtime. The only task on that runtime blocks in the
receive and never yields, so a `tokio::spawn`ed dispatch is never polled. The
dispatch is received, logged, and silently never executed.

This is not a test artifact. `aion-cli`'s `worker_agent`, `remote-gates`,
`norn-fan-worker`, `agent-dev`, `plan-fanout` and `incident-triage` all go
through `serve_with_redial`. As landed in `2abcdd47`, **every real liminal
worker would have accepted dispatches and executed none of them.**

The agent path was never affected: `spawn_agent_dispatch` already runs on a
dedicated thread with its own runtime, for a different stated reason (a `?Send`
future). The right pattern was, again, already in the file.

**Fix.** One dedicated thread owning its own current-thread runtime, draining an
unbounded queue serially (`spawn_plain_dispatch_executor`). This preserves the
one-at-a-time execution plain dispatches had when they ran inline — not a new
cap, the old one kept — and removes the dependency on the caller's runtime
flavour entirely. The `plain_dispatch_slots` semaphore is gone; serialization is
now structural rather than enforced by a permit.

Also corrected: the `serve_one` comment claiming "a plain activity executes
INLINE on this loop", falsified by the #58 change and not updated then.

## Why no gate caught it — MEASURED, not assumed

`cargo test -p aion-worker` compiles NONE of the liminal transport (#119). Five
other e2es use the same current-thread shape (`lsub1_xnode_dispatch_e2e`,
`lsub3_failover_e2e`, `lsub5_xnode_failover_e2e`,
`noi6_intervention_routing_e2e`, `outbox_delivery_contract_e2e`), and they pass
now. So: did they cover this and not run, or run and not cover it?

**Answered by A/B at the bytes.** The worker fix was reverted (that file alone),
the test binary rebuilt, and one target run under a hard timeout:

| build | result |
|---|---|
| broken (`tokio::spawn`) | `ninety_second_activity_delivers_exactly_once` **HANGS** — killed at 240s, exit 124 |
| fixed (executor thread) | **4 passed / 0 failed in 95.24s** |

Logs: `.evidence/118/broken-build-e2e.log`.

So the coverage was there and it was never executed. **`cargo test` stops at the
first failing target**, so the red lib test truncated the run before any
integration target started. Last session's reading of that battery —
"688 passed / 1 failed, one pre-existing failure, otherwise green" — was wrong in
a way that mattered: it was 688 passed, 1 failed, and **an entire tier of the
suite never run**, one of whose tests would have hung forever on the P0 I had
just introduced. I reported a truncated run as a complete one with a known
exception.

## Three lessons

1. **A red row is a blindfold, not a footnote.** The queue-declaration refusal
   fired at the start boundary, so no run in this suite ever reached dispatch,
   so a P0 in dispatch sat invisible behind it. Waffles' ordering ruling (fix the
   baseline BEFORE acceptance) is right for a stronger reason than the one given:
   not only does a known-red row put an exception clause on every reading, it
   hides every line of code downstream of itself.

2. **A failing target is a truncation, not a row.** "N passed / 1 failed" from
   `cargo test` does NOT mean "everything except that one is green" — it means
   the run stopped there. A count is only a coverage claim if the run reached the
   end. Read the target list, not just the tally.

3. **A hang is not a failure and no deadline catches it.** The broken build did
   not fail this test, it hung it. A battery with no per-test timeout converts a
   correctness defect into an unbounded wait, which reads as "still running"
   rather than "red".
