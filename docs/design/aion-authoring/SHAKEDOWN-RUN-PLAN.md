# The wake-prompt shakedown — run plan and live record

Standing up 2026-07-31 ~10:40Z on Waffles' PROCEED (relayed by Cally 10:12Z),
decoupled from the #117 remedy so neither rushes the other. This file is the
operational record; it is written BEFORE the run so the plan cannot be
retro-fitted to whatever happens.

## Decisions taken at this seat, named because the green report must carry them

| Decision | Value | Why it was mine to take |
|---|---|---|
| **Worker's home** | **THIS BOX** (Tom's machine) | Dean's box is off. Waffles required the home be *named*, not that it be any particular host. |
| Rig store | `~/.aion/remote-gates-proof` | The preserved 07-29 fixture. Unchanged since `2026-07-29T18:20:26+1000`. |
| Ports | HTTP **8091**, gRPC **51091**, liminal **51061** | The rig's own `server.toml` sets 8080/50051/50061 — **exactly Tom's live server's three ports**, despite a comment claiming "non-standard ports on purpose: nothing here touches 8080". All three are overridden. |
| Outbox dispatcher | **ENABLED** | It was disabled for the #117 cancel attempt to avoid incidental writes. Here it is the mechanism under test — the 07-29 failure was a delivery failure, not a work failure. |
| `heartbeat_window` | **ABSENT** | Per Waffles' 07-29 ruling: no explicit override, so the probe runs derived defaults — cadence **7500ms**, declared window **30000ms**. Confirmed absent from the rig `server.toml`; nothing in the tree needed reverting. |

## A redeploy is forced, and it is not optional

The rig's existing `remote_gates` package is minted under the `.v4`
worker-contract identity domain. `ad2fca44` moved identity to `.v5` and this
build carries no encoder for a superseded domain by policy, so **that package
can never load again on a current binary** and every run pinned to it is
unrecoverable (this is [#117](AN-UNRECOVERABLE-RUN-CANNOT-BE-CANCELLED.md)).

So the shakedown deploys `examples/remote-gates/remote_gates.awl` fresh. This is
a re-run of the *exercise*, not a resurrection of the old package.

## Green — by identity, not by count (Waffles' redefinition)

1. **Workers keep their leases under load** — no mid-cargo expiry orphaning.
2. **A completion is DELIVERED ACROSS A WORKER GAP.** Accepted into the
   acceptance terms in my own framing: the 07-29 log shows `activity completed
   10:10:38` → `dispatch failed 10:11:08` → `dead-lettered 10:22:10`. The work
   SUCCEEDED and the delivery died twelve minutes later at retry exhaustion. A
   leg that never faces the gap certifies around the actual defect.
3. **Wakes fire** via the fixed wake-prompt (meridian main `2e35521e1`).
4. **Zero zombie-`Running` rows OTHER THAN the named, shape-pinned exhibit
   `0756ecd5`.** Any NEW `Running` row without a live worker is RED.

All of it on one run, under real battery-scale load. The green report must state
**RE-RUN, not first flight**, and must name the worker's home.

## Budget — sized against 53 GiB, not 80

Athena measured **P = 80 GiB** free at 10:32:28Z on a self-refereeing
zero-census sample (lane census all zero; controls `zsh 17` ≥1 and
`nosuchproc-xyz 0` =0 both green). Her catch, which I accepted and which changed
this plan: **do not size against P.** `stack/aion/target` sits at 14G against
the ≤40 GiB bound I hold, so ~27 GiB of *sanctioned but unspent* allowance is
already inside that 80. Spending it twice — once as headroom, once as my own
allowance — is the composition error one level up.

**Honest floor: 80 − 27 ≈ 53 GiB.**

The dominant term is NOT aion. It is the rig's own workspace: the battery clones
meridian and runs check/clippy/nextest/fmt over it, which builds a full Rust
target tree for a *different* repository. That workspace is a **new tree covered
by no bound anywhere** — Athena's composition finding materialising in the very
next operation after she stated it. It is pinned to an explicit
`--workspace-dir` (the worker refuses to start without one; there is no default)
so it is bounded, attributable and sweepable at close.

## Teardown obligations

- **Orphan sweep** — `ps -eo` for every `aion`/worker process, not a count but an
  identification, with a positive control. Tonight's board standard, minted after
  I left a reader orphaned for twelve minutes.
- Sweep the rig workspace; report the reading to Athena.
- Verify the preserved fixture's `data/` and Tom's server (`99844`, health 200)
  after, from the filesystem and the wire rather than from intent.
- Tell Athena when the rig is down so she can re-base P.

## Live record

| time (Z) | event |
|---|---|
| 10:36 | Preflight: `git 2.47.1`, `cargo 1.97.1`, `cargo-nextest 0.9.133`, `rustfmt 1.9.0-stable`, `clippy 0.1.97`. Every tool the worker shells out to, confirmed **on the gate host** as the runbook requires. |
| 10:37 | Gate worker built, exit 0. |
| 10:38 | Rig server up, pid `79663`. **All three sockets verified bound** (8091/51091/51061), not merely announced. Heartbeat sweeper reports `sweep_interval_ms 7500`, `heartbeat_window_ms 30000` — the derived defaults, read from the product's own log rather than from the flags I passed. |
| 10:38 | `remote_gates` deployed: `5cb875def6e01c4b15f729fd9fe7a143ba1bb8ada44ad61cd723dc64be594711`, `route_changed: true`, `superseded_versions: []`. A **new** identity, distinct from the dead `13958627…`, exactly as the `.v4`→`.v5` move predicts. |
| 10:38 | Worker registered from **this box**, identity `vesper-box-shakedown`, task queue `remote_gates`, workspace `/Users/tom/aion-gates/workspace`, progress cadence 15s. Ready file reads `connected`. |
| 10:39 | Load leg started — workflow **`dfd2117c-b7b1-410c-bd61-3e4c2552e638`**, run `716e29a2`, over `meridian@main`. |

### One mis-step, recorded

The worker binary was first invoked from `target/release/` in the main
workspace and did not exist there. `examples/remote-gates/worker/Cargo.toml`
declares its own `[workspace]`, so it builds to its **own** target directory.
Cost: one failed launch, no state touched. Worth noting because it means the
gate worker's artifacts are a *fourth* tree, outside both the aion target bound
and the rig workspace.

### Baselines held by Athena, taken at 10:34:50Z and 10:37:56Z (zero census)

- `free` **80 GiB** → 79 GiB, `aion/target` **14G**, `~/.aion` **1.2G**
- `/Users/tom/aion-gates` — **absent entirely**, the cleanest possible baseline:
  every byte appearing there is this rig's, with no attribution ambiguity.
- `~/.aion/clones` — 21 entries against Tom's 21 runs. Reassigned to **pid
  99844**: movement there is *his server's activity*, not my growth, which makes
  it a detector for the one process nobody can restart.

Firing conditions are unrelaxed for this run at my own request: `free < 45 GiB`
reaches me off-tick, mid-battery or not, and the kill decision is mine. **A
shakedown that costs Tom his disk is not a green.**

---

## 🔴 TWO FINDINGS, live under load. The first green condition would have been a FALSE GREEN.

### The timeline, exact

| time (Z) | event |
|---|---|
| 10:37:51 | `liminal worker liveness probe started`; heartbeat sweeper started |
| 10:38:41.359 | `registered liminal worker in-band` |
| 10:38:41 → ~10:39:00 | **liveness pings 1–9 answered** (success is silent — no log lines) |
| 10:38:58.401 | worker receives its **first dispatch push** (`clone_repo`) |
| **10:39:06.405** | **ping 10 FAILS** — and every ping since |
| 10:39:14.947 | `clone_repo` **completes, `outcome: succeeded`**, 16 546 ms |
| 10:39:15.147 | worker receives `run_check` push; all four legs dispatched and started |
| 10:41:06 | ping 26 fails. **17 consecutive failures, ~2 minutes, zero recoveries** |

### Finding A — the worker stops answering liveness pings at its first dispatch, permanently

Every ping was answered until the worker received work. None has been answered
since. The failure reason is constant:

```
liveness answer never arrived: push correlation N did not complete:
no correlated push reply arrived within the timeout
```

This is **not** a dead or wedged worker, and that is what makes it a real
finding rather than an environment problem. On the very same connection, at the
very same time, the worker: received dispatch pushes, ran a 16-second clone,
returned a successful completion, received four more pushes, and is right now
running four real `rustc` fan-out legs (workspace at 1.8 GB and climbing). **The
connection carries work perfectly and cannot answer a ping.**

So the liveness channel is starved by activity execution — the push-reply
correlation path stops completing once dispatch handling owns the loop.

This is almost certainly the root cause of the 07-29 incident. That arc was
`activity completed 10:10:38` → `dispatch failed 10:11:08` → dead-lettered with
*no liminal worker registered for the row's pool*. A worker that does real work
but cannot answer liveness is exactly a worker that gets reaped while healthy,
after which the completed work has nowhere to be delivered.

### Finding B — the dead-man switch does NOT fire, and the warning it prints is false

The warning states: *"its connection lease is NOT being refreshed and the expiry
sweep will reap it if the silence persists"*, with `silence_window_ms: 30000`.

The silence has persisted **~2 minutes — four times the declared window** — and:

```
reap / expiry / deregister events in the log:  0
```

The worker is still registered and still being dispatched to. So either the
sweep does not reap, or something *is* refreshing the lease and the warning's
own claim is untrue. Both readings are defects, and this lands directly on
task **#58** (dead-man switch on worker connection).

### Why this matters more than either finding alone

Green condition (1) is *"workers keep their leases — no mid-cargo expiry
orphaning."* Measured literally, it is **passing right now**: the lease was
never lost.

It is passing **because the reaper is broken.** A lease that survives only
because nothing can reap it is not a lease that held; it is a supervision
mechanism that is switched off. Certifying (1) on this evidence would have been
exactly the false green this shakedown exists to prevent — and the reason it is
visible at all is Waffles' redefinition of green by identity rather than by
count.

**I am not certifying condition (1).** The run continues, because conditions
(2)–(4) — completion delivered across a worker gap, wakes firing, no new zombie
rows — are still worth measuring and the legs are doing real work.

### 🔴 ROOT CAUSE OF FINDING A — found at the bytes, stated in the code's own comment

`crates/aion-worker/src/runtime/liminal.rs`, in the pushed-frame handler:

```rust
// An AGENT dispatch may run for a long time. It MUST NOT block the serve loop,
// or a mid-run intervention push (which rides the SAME channel) could never be
// received. So it is SPAWNED: the run drives concurrently and replies its own
// correlated DispatchResponse when it finishes, while the serve loop returns to
// receive interventions. A plain activity runs inline (short, no live session).
if self.is_agent_activity(&request.activity_type) {
    self.spawn_agent_dispatch(correlation_id, request);
    return Ok(());
}
let response = self.execute(&request).await?;   // ← INLINE. Blocks the serve loop.
```

**The hazard is documented, understood, and fixed — for agents only.** The
liveness ping rides that same serve loop (`answer_liveness_ping` is called from
`handle_pushed_frame`), so while a plain activity executes inline, no ping can
be received, let alone answered.

The whole thing rests on one parenthesis: *"A plain activity runs inline (short,
no live session)."* **That assumption is false, and the workflow document itself
says so.** `remote_gates.awl` declares, on plain activities:

```
action run_nextest(path: String) -> LegResult
  node shell, timeout 45m
```

A plain activity with a **45-minute** declared timeout is not short. `clone_repo`
alone ran 16.5 s — more than twice the 7.5 s ping cadence — which is why the very
first ping after the first dispatch failed.

This is the same family as #83, #113, #114 and the reload message: **the fact
that plain activities can be long-running is DECLARED in the document, and the
worker assumes the opposite.** The information exists, one layer away, and the
decision does not consult it.

The fix shape is obvious and small — spawn plain activity dispatches the same
way agent dispatches are already spawned, so the serve loop stays free — but it
belongs to task **#58** and wants its own build with the reap half (Finding B)
settled alongside, since the two interact: making liveness answerable again will
also make the reaper's behaviour observable for the first time.
