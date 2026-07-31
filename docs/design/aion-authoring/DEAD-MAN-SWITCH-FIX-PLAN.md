# #58 — the dead-man switch fix, four parts

Authorized by Waffles 2026-07-31 10:53Z (three parts) and 10:58Z (part 4 as a
scope amendment). Tom's silence-default GO already stood on #58. Evidence this
builds on: `SHAKEDOWN-RUN-PLAN.md`, commits `dc739cca` (mechanism), `2dec7bc5`
(my two withdrawals), `9838f1e2` (run killed, prediction confirmed at the bytes).

**The build starts from red on the record**, which is what the standard wants.

## The defect, in one paragraph

The server's liveness probe pushes a ping on the worker's connection using
liminal's **no-deadline** push API. An unanswered ping's reply slot is reclaimed
only by a consumed reply or a connection close — never by the probe giving up.
So every timed-out ping leaks one slot against
`max_pending_pushes_per_connection` (32). Measured live: pings 10–41 time out,
ping 42 is refused, ≈240 s to permanent exhaustion. Meanwhile the worker's
background liveness **pump** refreshes the same connection lease the ping feeds,
so the lease never expires and the switch cannot fire for the one failure it
exists to detect.

## Part 1 — plain-activity dispatches must not block the serve loop

`crates/aion-worker/src/runtime/liminal.rs`, `handle_pushed_frame`.

Agent dispatches are already spawned so a long run cannot block the serve loop.
Plain activities run inline. Spawn them the same way.

Note for whoever reviews: the inline hazard was **already known and
compensated** by `spawn_liveness_pump` — see the withdrawal in
`SHAKEDOWN-RUN-PLAN.md`. This part removes the trigger; it is not the fix, and
must not be mistaken for it.

## Part 2 — the ping must release its slot

`crates/aion-server/src/worker/liminal_transport.rs`.

All three push sites (`push_dispatch` :623, `push_payload` :644,
`push_intervention` :674) call `push_to_connection`. Move to
`push_to_connection_with_deadline`, whose slot expiry **releases the cap
admission**. `push_payload` has exactly one caller (the probe), so it is
replaced rather than added alongside — house rule, no compat shim.

## Part 3 — the error must say what is true

A ping that cannot be *pushed* because the cap is reached is not "the worker did
not answer". It is **"the server can no longer ask."** Today's WARN also asserts
*"its connection lease is NOT being refreshed"*, which is **false** whenever an
activity is in flight, because the pump is refreshing it by design.

`ping_one` returns a typed outcome distinguishing *unaskable* (push refused)
from *unanswered* (pushed, no reply), and the WARN differs per variant and
claims nothing about the lease it cannot verify.

## Part 4 — split the lease's two facts (Waffles, 10:58Z)

Two different facts are collapsed into one lease:

- **the worker process is alive** — proven by the pump, worker→server;
- **the server can reach the worker's dispatch path** — proven only by an
  answered ping, server→worker.

**Only the second is a dispatch precondition.** `liminal_transport.rs:635`
already states the principle — the ping rides the dispatch leg so it proves the
real path, *"not a parallel one that could be healthy while the real one is
not."* The pump feeding the same lease is exactly the parallel channel that
sentence prohibits. **The fix makes the code agree with its own doc.**

Shape (smallest that satisfies the amended control):

- `HeartbeatState` gains `reachability: HashMap<WorkerId, Instant>` — the last
  time the server *proved* it can reach this worker's dispatch path.
- `register_connection` seeds both maps: the handshake proves reachability at t0.
- `record_connection_activity` (any inbound frame, including a pump beat)
  advances **`connections` only**. Process alive. It keeps its true fact.
- New `record_dispatch_reachability` (an **answered ping** only) advances both.
- New `unreachable_workers(now)` — reachability older than the heartbeat window.
- The probe, which holds both tracker and registry, flips dispatch eligibility
  in the registry each round; `select_worker` filters ineligible workers.

**A pump refresh can never hold dispatch eligibility against failing pings.**

## Acceptance (Waffles' two musts, (b) as amended at my correction)

**(a) busy-past-ceiling leg.** A worker held busy well past **240 s** — the old
outage boundary — with pings answering throughout. Green must be demonstrated in
the **declared workload class** (`remote_gates.awl` declares `timeout 45m`), not
under it. *Bounds carry workload class.*

**(b) reaper positive control — AMENDED.** A **pump-alive, serve-loop-dead**
worker must be seen to **lose dispatch eligibility within the window despite the
pump**. A process-kill control does **not** discriminate: connection-close
reclaims by its own path and was observed doing so cleanly at 10:56:07.618Z, so
a kill control would go green while this defect stands untouched.

Plus part 3's honest error text verified at the bytes.

## Rig precondition, mine, before any acceptance run

`remote_gates.awl` declares **no disk bound** and there is **no between-run
sweep**; the workspace reached **37G in 17 minutes** against a 10–15 GiB
estimate and was killed on disk pressure. The re-run carries **both** a declared
workspace bound and a sweep — they answer different failures. **An unbounded
writer must not use a watcher as its safety mechanism.**

## Build constraint while Tom's server lives

`target/release/aion` is the binary the rig ran and `99844` is unrestartable on
this volume. **Debug profile only** (`cargo test`/`clippy` write `target/debug`)
until a release build is deliberately sequenced. `lsof` before replacing any
artifact under a running process.

## 🔴 GATE TRAP HIT WHILE BUILDING THIS — record it

`cargo check -p aion-worker --all-targets` **compiles none of the liminal
transport and exits 0.** `runtime/mod.rs` guards `pub mod liminal;` with
`#[cfg(feature = "liminal-transport")]`, and that feature is not in the crate's
defaults. The primary production worker transport is simply absent from the
compilation unit.

Caught by an inverted control, not by luck: I appended `this is not valid rust
@@@` to `liminal.rs` and the gate still returned **exit 0, zero errors**. With
`--all-features` the same tree returns exit 101 and four real errors.

It is **not a shipped defect** — `aion-cli` carries `liminal-transport` in its
default features and `aion-server`'s dev-dependency on `aion-worker` enables it,
so a workspace build does compile it. It is a **measurement defect**, and a
dangerous one: any per-package gate on this crate is vacuous, and reports green
while the file under test is not parsed.

**Every gate for this work uses `--all-features`.** The general rule, already on
the board in another form: `-p <crate>` and `--workspace` are different runs
with different feature resolution, and a per-package green says nothing about
the feature-gated modules. **A compile gate that cannot fail on a syntax error
is not a compile gate.**

I walked into a trap whose shape I had already written down. The control is what
caught it — the reading did not.
