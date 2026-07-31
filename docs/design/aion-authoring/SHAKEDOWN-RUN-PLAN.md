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

---

## LIVE PROCESS STATE (for resume across a context compaction)

- **Rig server** pid `79663` — `target/release/aion server`, config
  `~/.aion/remote-gates-proof/server.toml`, HTTP 8091 / gRPC 51091 / liminal 51061.
- **Gate worker** pid `81616` — `examples/remote-gates/worker/target/release/remote-gates-worker`,
  identity `vesper-box-shakedown`, workspace `/Users/tom/aion-gates/workspace`.
- **Run** `dfd2117c-b7b1-410c-bd61-3e4c2552e638`, run id `716e29a2`, `Running`,
  four legs started, no leg completed yet.
- **Monitor task** `bt16xil7r` (liveness spam filtered out).
- Logs: `<scratchpad>/shakedown/rig-server.log`, `rig-worker.log`.
- **DO NOT TOUCH** Tom's pid `99844` (8080/50051/50061).

**Standing position, per Cally 10:43Z relaying her recommendation to Waffles
(NOT yet his word):** run every measurable leg — (3) wakes, (4) no new zombie
rows, and (2) *attempted* with the attempt documented either way — then land the
shakedown as **RED-WITH-FINDINGS on this build** rather than certifying around a
condition Finding A may make unreachable. A controlled gap (manual worker kill
after completion, before delivery) is under consideration to make (2) testable
independently of Finding A: feasibility is mine, whether it counts for green is
his. **Certify nothing until Waffles rules.**

Teardown still owed: orphan sweep, workspace sweep + reading to Athena, fixture
and Tom's-server verification, and tell Athena when the rig is down so she can
re-base P (currently VOID — last good value 80 GiB at 10:32:28Z).

---

## 🔴 FINDING A — CORRECTED AND ESCALATED: the liveness probe poisons the connection it monitors

The earlier root-cause entry above is **incomplete, and its emphasis was wrong.**
The blocked serve loop is real, but it is the *trigger*, not the defect. The
defect is what the server does about it. Measured at 10:47Z on run
`dfd2117c`, from `rig-server.log`.

### The two phases, exact

| pings | window | failure reason |
|---|---|---|
| 10–41 (**32**) | 10:39:06 → 10:42:59 | `liveness answer never arrived: no correlated push reply arrived within the timeout` |
| 42–82 (41+, ongoing) | 10:42:59 → now | `liveness ping push failed: … max_pending_pushes_per_connection limit of 32 is reached` |

**Exactly 32 pings time out, and the 33rd attempt is refused by a cap of 32.**
The transition is immediate and arithmetically exact. Zero pings have ever been
answered since the first dispatch; zero recoveries.

### The mechanism, in liminal's own words

`liminal-server-0.5.0` offers two server-push APIs. From `push_to_connection`:

> this no-deadline push reserves a slot that is reclaimed only by (a) the reply
> being consumed or (b) the connection closing. **An elapsed `receive` poll is a
> benign re-arm, never a failure and never a cancellation.** The §5
> `max_pending_pushes_per_connection` cap bounds abandonment; **use
> `push_to_connection_with_deadline` when the reply must have an explicit
> expiry.**

The deadline variant, at expiry, "is removed, and its §5
`max_pending_pushes_per_connection` cap admission is **released**."

`aion-server` calls `push_to_connection` at **all three** of its push sites
(`liminal_transport.rs:623, 644, 674` — dispatch, liveness, intervention).
`push_to_connection_with_deadline` is called **nowhere in the codebase.**

And `liminal_liveness.rs:302` does precisely what the doc warns against:

```rust
let awaiter = delivery.push_payload(payload)         // no deadline
    .map_err(|error| format!("liveness ping push failed: {error}"))?;
let reply = awaiter.receive(deadline)                // one quantum
    .map_err(|error| format!("liveness answer never arrived: {error}"))?;
```

An elapsed quantum is treated as a **failure** and the awaiter is dropped. The
slot is not. The probe then fires again 7.5 s later with a fresh correlation id
and a fresh slot.

### Why this is a P0 rather than a monitoring nuisance

`push_payload`'s own doc comment states the design intent, and it is *correct*:

> used by the connection liveness probe … so the dead-man switch rides the SAME
> server-push leg a dispatch does — a ping that reaches the worker proves the
> exact path a dispatch would take, not a parallel one that could be healthy
> while the real one is not.

Sharing the leg is right. **Sharing the leg while leaking its bounded capacity
is what turns the probe into a weapon.** The ping does not merely fail to
observe the dispatch path — it *consumes the capacity the dispatch path needs*,
at a fixed 7.5 s cadence, until none is left. The instrument built to prove the
connection works is what destroys it.

The bound follows directly: **32 slots ÷ one leaked slot per 7.5 s ≈ 240 s.**
Any worker whose serve loop is busy for about **four minutes** permanently
exhausts its connection's push capacity and can never be sent another dispatch,
intervention, or ping — until the connection closes. `remote_gates.awl` declares
`run_nextest … timeout 45m`. **The declared ceiling exceeds the survivable one
by more than tenfold.**

Two caveats stated honestly. The precise slot accounting against the three
in-flight dispatch slots is not fully reconciled — the exact 32/33 transition,
the API semantics, and the call sites together make "an abandoned ping leaks
exactly one slot" the only reading consistent with the evidence, but I have not
instrumented the registry to prove the arithmetic term by term. And the cap of
32 is liminal's default, not an Aion setting.

### This answers the flag I raised, and the answer is worse than the flag

I flagged to Cally, for Waffles, that Finding A might make green condition (2)
— *completion delivered across a worker gap* — **unreachable by construction**
on this build. It does, and the mechanism is now exact: `notify_result` requires
a **new** push on a connection whose push admission is permanently refused. The
run cannot deliver its result. Not because a worker died — the worker is alive
and doing real work — but because the server poisoned its own channel to it.

### It also unifies 07-29 and tonight

Finding B (the reaper never fires) and Finding A are the same mechanism seen
from two sides. 07-29's *"lease expired mid-cargo"* and tonight's *"lease never
expires at all"* are one broken supervision path failing in opposite directions
across a build. **This makes #58 a correctness fix, not hardening** — which is
how it has now been reclassified.

### Fix shape (for #58, not built off my own bat)

Three parts, and the third is the one that matters:

1. **Spawn plain-activity dispatches** the way agent dispatches already are, so
   the serve loop stays free. Removes the trigger.
2. **Push the liveness ping with a deadline**, so an unanswered ping releases
   its slot. Removes the leak. This is a one-line API change with a documented
   contract behind it.
3. **A ping that cannot be pushed because the cap is reached is not "the worker
   did not answer" — it is "the server can no longer ask."** The current WARN
   asserts a lease claim that is false, and names the worker as the party at
   fault when the server is. This is the discarded-fact pattern again — **the
   sixth instance** after #83, #113, #114, the reload message, and the worker's
   inline assumption. The typed error already carries the distinction; the
   warning collapses it.

**Nothing here is certified. The run continues; conditions (3) and (4) remain
measurable and are still worth measuring.**

---

## 🔴 CORRECTION — two of my own claims above are WRONG, and the real defect is sharper

Found at 10:55Z reading `aion-worker/src/runtime/liminal.rs:998–1030`
(`spawn_liveness_pump`) while scoping the #58 fix. Written before building,
because the fix would have been designed against a false picture.

### WITHDRAWN 1 — "the worker assumes plain activities are short"

I wrote that Finding A's root cause "rests on one parenthesis — *A plain
activity runs inline (short, no live session)* — that assumption is false, and
the workflow document itself says so," and I counted it as the **fifth**
instance of the discarded-fact pattern.

**That is wrong, and I withdraw it.** The worker does not assume plain
activities are short. A fuller comment in the *same file* states the opposite,
names the exact consequence, and reports it observed in production:

> a plain activity executes INLINE on the serve loop, so while it runs the
> worker **cannot dequeue (or answer) the server's pings**, and nothing else
> would advance the connection lease. A leg longer than the window would be
> deregistered mid-work for the crime of working — **observed live on legs over
> ~40 s.** The pump publishes from a background task on a shared writer, so it
> keeps the lease alive whatever the serve loop is doing.

The hazard was known, diagnosed, and **compensated** — by a background liveness
pump publishing `WorkerLivenessBeat` on `WORKER_LIVENESS_CHANNEL` every quarter
window, independent of the serve loop. The parenthesis I quoted is a stale
comment at one site, contradicted by a fuller one a few hundred lines away. It
is not the load-bearing assumption I claimed. **The discarded-fact census drops
back to five instances, not six.**

That paragraph also identifies 07-29's failure exactly — *"deregistered mid-work
for the crime of working, observed on legs over ~40 s"* is the "lease expired
mid-cargo" event — and records that it was **already fixed** by the pump.

### WITHDRAWN 2 — "the reaper is broken / supervision is switched off"

Finding B concluded the dead-man switch does not fire and offered two readings,
treating "the sweep does not reap" as the live one. **The other reading was the
correct one: something *is* refreshing the lease, by design.** The pump is.

So the reaper is **not** broken, and my refusal of green condition (1) rested on
a false premise. **Condition (1) — workers keep their leases, no mid-cargo
expiry orphaning — passes, and passes for a legitimate engineered reason.** I
withdraw the refusal. It was the right instinct applied to the wrong mechanism:
I inferred a broken reaper from an absence of reap events without checking
whether a designed refresher explained the absence.

### WHAT STANDS, UNCHANGED

Everything in the slot-leak finding. Exactly 32 pings time out, ping 42 is
refused at a cap of 32; `aion-server` uses liminal's no-deadline push at all
three sites and the deadline variant nowhere; an abandoned ping never releases
its slot; the resulting push-admission ceiling is ≈240 s against a declared
45-minute leg; condition (2) is unreachable because `notify_result` needs a new
push on a permanently-refused connection. None of that is touched by this
correction — the leak is in the *server's* probe, not the worker's pump.

### AND THE REAL FINDING B, WHICH IS SHARPER THAN THE ONE I WITHDREW

`push_payload`'s doc gives the ping's whole reason for existing:

> the dead-man switch rides the SAME server-push leg a dispatch does — a ping
> that reaches the worker proves the exact path a dispatch would take, **not a
> parallel one that could be healthy while the real one is not.**

**The pump is precisely such a parallel channel, and it feeds the same lease.**
It publishes from a background task whatever the serve loop is doing — including
when the serve loop can receive nothing at all.

So the two signals collide, and the weaker one wins:

- the **ping** says *the server cannot reach this worker* — 73 consecutive
  failures, the exact evidence the switch exists to collect;
- the **pump** says *this worker's process is alive* — and refreshes the lease;
- the lease accepts either, so the ping's negative evidence is **discarded**.

A worker whose dispatch path is one hundred percent dead holds a perfectly
healthy lease indefinitely. **The dead-man switch cannot fire for the failure
mode the ping was built to detect.** That is live right now on `dfd2117c`: the
server has been unable to reach this worker for over fifteen minutes, and the
worker is in perfect standing.

The root cause is that **two different facts are collapsed into one lease**:
*the worker process is alive* (pump, worker→server) and *the server can reach
the worker* (ping, server→worker). Only the second is a dispatch precondition.
The current warning asserts the lease is not being refreshed, which is **false**
whenever an activity is in flight — the pump is refreshing it by design.

### CONSEQUENCE FOR WAFFLES' ACCEPTANCE MUST (b)

His reaper positive control is right and now needs a sharper target. **Killing
the worker process is not a sufficient control** — that closes the connection,
and connection-close reclaims by a different path, so it would go green while
this defect stands. The control that discriminates is a worker that **keeps
pumping while its serve loop cannot answer** — process alive, dispatch path
dead. On this build that worker is never reaped. That is the positive control
worth having, and it is exactly tonight's live condition.

**Method note against myself, twice in one night.** The invalid `Code`-chunk
fixture failed for the wrong reason and I nearly banked it. Here I inferred a
broken mechanism from an absence of events without asking what else could
produce that absence. Same error shape: *a signal's absence is not
self-explanatory.* Both were caught by reading the code that produces the
signal rather than the signal alone.

---

## RUN ENDED 10:56:07Z — KILLED ON DISK PRESSURE. RED-WITH-FINDINGS.

Not a completion and not a failure of the run: a **deliberate stop**, taken on
Athena's off-tick alarm and my own confirming measurement, because the rig was
consuming the volume that carries Tom's unrestartable `99844`.

### Why it was stopped

| time | free | `aion-gates` |
|---|---|---|
| 10:37:56Z | 79.0 GiB | absent |
| 10:54:11Z | 48.8 GiB | 32G |
| 10:54:50Z | 47.5 GiB | 34G |
| **10:55:55Z (mine)** | **45 GiB** | **37G** |

Still climbing at the moment of the kill, no plateau. My pre-run estimate for
the workspace was **10–15 GiB**; it reached **37G in 17 minutes** and
`run_nextest` had a declared 45-minute timeout with ~31 minutes still to run.

**The rig declares no disk bound at all**, and `remote_gates.awl` has no
workspace cap and no between-run sweep. It leaned on a 15-minute monitoring
cadence to be safe — and a monitor is not a bound. That is a rig defect and it
is mine: **an unbounded writer must not use a watcher as its safety mechanism.**
Before this rig runs again it carries a declared workspace bound *and* a
between-run sweep. They answer different failures — the bound stops one run
running away, the sweep stops N runs accumulating.

Kill order mattered: **worker first** (it owned the `cargo`/`rustc` children
doing the writing), then an orphan sweep, then the server. Killing the server
first would have orphaned the writers and left them running. Post-kill
`pgrep rustc|cargo|remote-gates-worker` was empty. Growth stopped, then 37G of
regenerable `checkouts/meridian/target` was deleted after inspection —
`workspace/logs` (8.4MB of gate leg evidence) kept. **Free returned to 78 GiB.**
Tom's `99844` verified alive throughout and after: `etime 23:57:43`, **HTTP 200**.

### 🔴 THE SHUTDOWN CONFIRMED THE SLOT-LEAK PREDICTION AT THE BYTES

This is the strongest evidence of the night and it was free — it fell out of the
teardown. I had predicted that activities **2 (`run_clippy`)** and **4
(`run_fmt_check`)** never reached the worker, because the worker log recorded
only ordinals 0, 1 and 3. On disconnect:

```
outbox dispatch failed … dispatch_key "dfd2117c…:2" … no liminal worker registered
outbox dispatch failed … dispatch_key "dfd2117c…:4" … no liminal worker registered
outbox dispatch exhausted retry budget; dead-lettering row … :2
outbox dispatch exhausted retry budget; dead-lettering row … :4
```

**Exactly ordinals 2 and 4, and no others.** Both had carried
`ActivityScheduled` *and* `ActivityStarted` at 10:39:14.984Z and sat undelivered
for **seventeen minutes** while history said they were running. The two legs the
worker never received are precisely the two the outbox still held.

That is the leak's operator-visible consequence stated exactly: **history
records an activity as Started that the worker was never able to be told
about.** Ordinals 0, 1 and 3 got through before the cap saturated; 2 and 4 did
not, and never could.

### And the must-(b) control, demonstrated by accident

`"deregistered liminal worker on disconnect"` fired at 10:56:07.618Z —
immediately, cleanly, on connection close. This is the empirical half of the
correction sent to Waffles: **connection-close reclaims by its own path and
reclaims fine.** So a process-kill reaper control would have gone green tonight
while the defect stood untouched. The discriminating control remains a worker
that keeps pumping while its serve loop cannot answer.

### Final condition ledger — NOTHING CERTIFIED

| condition | verdict |
|---|---|
| (1) workers keep their leases | **passes** — and legitimately, via the pump. My earlier refusal is withdrawn. |
| (2) completion delivered across a worker gap | **unreachable by construction** on this build. Moves whole into the post-fix re-run. |
| (3) wakes fire | **not reached** — the run never got past the gate legs. |
| (4) no new zombie rows | **not established.** `dfd2117c` was killed mid-flight and its two dead-lettered rows are exactly the state this condition asks about. It is a question for the re-run, not an answer from this one. |

The shakedown lands **RED-WITH-FINDINGS**, which is what it was for. It was
never going to certify anything tonight; it was going to find out whether the
rig survives real load, and it found three reasons it does not — a slot leak
that kills the dispatch channel, a lease that cannot see that channel die, and
a workspace with no bound.
