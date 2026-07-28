# Failover start-path investigation — "was the start path ever provable?"

Lane `build/failover-start-path`. Charter (Waffles, 2026-07-28): **"was the start
path ever provable" is the question; "what broke" presumes a green that never
existed.**

Toolchain for every specimen and gate run at this seat:

```text
rustc 1.97.1 (8bab26f4f 2026-07-14)
binary: rustc
commit-hash: 8bab26f4f68e0e26f0bb7960be334d5b520ea452
commit-date: 2026-07-14
host: aarch64-apple-darwin
release: 1.97.1
LLVM version: 22.1.6
```

Base: `origin/main` **d93f2342** (this lane rebased mid-flight from 9e6963c2 onto
d93f2342 after the R5 advisory-class merge landed; the rebase was textually clean
and `cargo check --workspace --all-targets` was green on the rebased tree before
any further work — R5's surface and this lane's are disjoint, verified, not
assumed).

---

## Verdict (STATED)

**The start path was never provable by this gate, and the gate could never have
been green as authored.** Two distinct defects, on two different seams:

1. **Harness defect — hardcoded cluster ports collide with a macOS system
   service.** FIXED in this lane (commit 2 of 2). Cause of the boot-death
   specimens.
2. **Production start-path behaviour — an unsteered `start` is refused forever by
   the namespace-registry shard fence, not by anything about the workflow's own
   shard.** NOT FIXED. This is the answer to the charter question, it is a
   production seam, and per the brief this lane STOPS and reports rather than
   touching production code.

Classification asked for by the brief: **production start-path defect** (with a
harness defect layered on top of it that was hiding it). The gate remains RED and
**fleet gate 2 stays unmet.** No assertion was weakened to manufacture a green.

The precise statement of "never provable": whether this gate could EVER pass was
settled at authoring time by whether `shard_for(namespace_key("default"))`
happened to equal node 0's shard. With the default namespace and
`shard_count = 3` it is shard **1**, and node 0 owns shard **0**. It never could.
That is a one-in-three authoring lottery that lost, not a regression.

---

## The specimen record — 7 specimens, 2 machines, 0 greens, the kill never fired

| # | Where | Profile | Result | Shape |
|---|-------|---------|--------|-------|
| 1 | device (Annabel's box) | contended by a live haematite build worker | VOID | setup START_DEADLINE expiry — "collect_four never landed on node 0's shard" |
| 2 | this seat | load ~6.5→12-15, no cargo during test | RED 98s | identical shape to 1 |
| 3 | this seat | warm target, zero compile, load ~6.7 | RED 9s | "node 0 failed to boot: server exited during boot with exit status: 1; output:" — **output EMPTY** |
| 4 | this seat | Act 1 instrumentation, contended (another lane building) | RED 99s | boot death, now READABLE: beamr distribution connect failed |
| 5 | this seat | cluster ports reserved | RED 38s | boots cleanly, then START_DEADLINE expiry with the refusal quoted |
| 6 | this seat | + refusal histogram | RED 38s | 262 attempts, **1 distinct refusal** |
| 7 | this seat | battery leg, post-refactor | RED 44s | 254 attempts, **1 distinct refusal** — identical |

Specimens 1-3 are the brief's record; 4-7 are this lane's. Logs for 4-7 are in
`gate-logs/failover-start-path/`. The `kill -9` this gate exists to exercise has
**never fired on any specimen** — every failure is in SETUP.

Disclosure on the specimen artifacts: the `.exit` capture for specimens 4-6 was
written with `${PIPESTATUS[0]}`, a bash-ism that expands to nothing under this
seat's zsh, so those three files were created EMPTY. They have been deleted
rather than back-filled with a number nobody measured. Their verdicts are read
from the logs' own `test result:` / `Error:` lines, which is the ruled method
regardless. The battery legs and specimen 7 capture `$?` directly and their
`.exit` files are real. (Same class of defect as the one main's cceea122 fixed
for the device battery — worth knowing it recurs in any `cmd | tee` under zsh.)

---

## Act 1 — the harness was blind by construction

### What was wrong

`boot_node` redirects each node's stdout+stderr into `node{index}.log` with
`Stdio::from(file)`. A child spawned that way has **no pipes**: `child.stdout`
and `child.stderr` are `None`. The shared post-mortem, `captured_output` in
`crates/aion-cli/tests/common/osproc.rs`, drained exactly those two handles — so
it returned an empty string every single time in this harness. Specimen 3's

```text
Error: "node 0 failed to boot: server exited during boot with exit status: 1; output:\n"
```

was therefore not evidence that node 0 wrote nothing. Node 0's dying words were
in `node0.log` on disk the entire run. Then the run ended, `tempfile::tempdir()`
dropped, and the tempdir — with the logs — was deleted. The only post-mortem hook
(`dump_logs_if_requested`) was gated on `KILL9_KEEP_LOGS` being set, and it was
additionally unreachable on this path: a boot failure returns from `boot_cluster`
through `?` before the hook's call site is ever reached.

Three independent mechanisms, all discarding the same evidence.

### What changed (commit `79f0f7b6`, shared helper — Rider 2)

- The pipe drain is **deleted**, not renamed. Nothing in `osproc.rs` reads pipes
  any more, so an empty capture can never again be cited as silence.
- `log_tail(path)` reads the log FILE's last 50 lines and says so **explicitly**
  when there is nothing to read (`<no readable log at …>`, `<… exists but is
  empty>`).
- `wait_for_liveness` takes the log path and reports the exit status **and any
  signal** (`describe_exit`), plus whether the server died **BEFORE or AFTER** it
  bound its HTTP port (a `Probe` enum distinguishes a refused connection from an
  answered one).
- `log_redirect(path)` is now the only way the gates spawn a child: stdout+stderr
  to one file. This also removes an undrained-`Stdio::piped()` hazard from the
  connect-half gate, where a chatty `RUST_LOG=info` server could fill the pipe
  buffer and block on write with nobody reading.
- `preserve_on_failure` copies the work dir out to `CARGO_TARGET_TMPDIR` on
  **every** failure and names the destination in the error. Unconditional: an
  env-gated post-mortem is exactly the discarding this act existed to end. Both
  gates now run their scenario in an inner function so every failure — including
  a boot death — flows through it.
- `try_start` returns the **refusal** instead of a bare `None`.

Three fast guards (`/bin/sh`, no cluster, not `#[ignore]`d) pin the harness's
ability to report: `osproc::harness_diagnostics`.

### Demonstration

The guard's own output, showing a boot death now carries status, bind phase, and
the child's real stdout+stderr:

```text
server exited during boot BEFORE binding 127.0.0.1:52911 (exit status 3, no signal);
last 50 lines of /var/folders/…/T/.tmpnek2g4/node0.log:
config parse failed
boom
```

And preservation outliving the tempdir:

```text
preserved this run's files in …/target/tmp/failover-logs/harness-diagnostics-54343-1785259994332 (node0.log)
```

The real thing, on specimen 4 — the same failure specimen 3 could not explain:

```text
Error: "node 2 failed to boot: server exited during boot BEFORE binding 127.0.0.1:53763
 (exit status 1, no signal); last 50 lines of …/node2.log:
 …
 {"level":"ERROR","fields":{"message":"aion-server failed","error":"store backend failed:
  store backend error: haematite database error: distribution endpoint error:
  beamr distribution connection could not be established"}}

preserved this run's files in …/failover-logs/lsub5b-kill9-failover-76688-1785260279273
 (collect_four.aion, node0.log, node0.toml, node1.log, node1.toml, node2.log, node2.toml)"
```

---

## Act 2, finding A — the boot death: port 7000 is macOS AirPlay Receiver

With specimen 4's logs preserved, the three node logs read together:

```text
node0  17:37:54.029  configuration resolved
node1  17:37:54.030  configuration resolved
node2  17:37:54.029  configuration resolved          <- all three, same instant
node1  17:37:54.169  startup banner                  <- up in 0.14s
node0  17:37:59.123  WARN prepare send target unreachable (haematite::sync::endpoint)
node0  17:37:59.137  startup banner                  <- up after 5.1s of retrying
node2  17:37:59.095  ERROR aion-server failed: … beamr distribution connection
                     could not be established        <- dead after 5.07s
```

`BOOT_STAGGER` staggers the **spawn** by 3s, but under load the three processes'
startup latencies converged and all three attempted their distribution connect
within a millisecond of each other anyway. That is worth knowing, but it is not
the cause. The cause is the address they dialed:

```text
$ lsof -nP -iTCP:7000 -sTCP:LISTEN
COMMAND   PID USER   FD   TYPE  NAME
ControlCe 795  tom    9u  IPv4  TCP *:7000 (LISTEN)
ControlCe 795  tom   10u  IPv6  TCP *:7000 (LISTEN)
$ ps -p 795
/System/Library/CoreServices/ControlCenter.app/Contents/MacOS/ControlCenter

$ connect 127.0.0.1:7000
HTTP/1.1 403 Forbidden
Server: AirTunes/935.7.1
```

`127.0.0.1:7000` is **macOS AirPlay Receiver**, on by default. The gate hardcoded
each node's cluster distribution address as `7000 + index` while reserving every
other port with `reserve_port`. A peer dialing node 0's distribution address
before node 0 had bound it did not get a refused connection — it got AirTunes,
which **accepts** and speaks HTTP. beamr's distribution handshake against an HTTP
server fails after ~5s: "beamr distribution connection could not be established",
fatal at boot.

Corroboration from the tree itself: the sibling gate
`parked_timer_osproc_kill9_failover_e2e` — same harness shape, same 3-node
cluster, same `kill -9`, and **green** — uses `7100 + index`. The one structural
difference in the distribution port scheme between the green gate and the
never-green gate is the base port, and the never-green one starts on AirPlay's.

Also measured, since it explains why node 0 sometimes survived this: a plain bind
of `127.0.0.1:7000` fails with `EADDRINUSE`, but a bind **with `SO_REUSEADDR`
succeeds** against ControlCenter's wildcard listener. Whether a dial reaches
node 0 or AirTunes is therefore a race with node 0's own bind.

**Classification: harness defect. FIXED in this lane** — all four ports per node
are now `reserve_port`-allocated, with the reason recorded at the call site so it
cannot be re-hardcoded innocently.

This closes specimen 3 and specimen 4. It does **not** close specimens 1, 2, 5, 6
and 7, which boot fine.

---

## Act 2, finding B — the start: a constant, not a lottery

With the cluster booting cleanly, the start loop's own refusal is readable
(specimen 6, `gate-logs/failover-start-path/specimen6.log`):

```text
Error: "collect_four never landed on node 0's shard within 30s against
 http://127.0.0.1:56274; what it was told: 262 attempts, 1 distinct:

  [x262] `aion start aion_outbox_fixture --input {"fixture":"input"}
         --endpoint http://127.0.0.1:56274` exited exit status: 1; stderr:
  error[unavailable]: failed to start workflow: shard 1 is owned by another node (not owner)
    server error type: NotOwner"
```

**262 attempts over 30 seconds. One distinct refusal. Always shard 1.**

That single number settles the question the brief posed. A retry loop can only
help if the outcome varies. It does not vary, so `START_DEADLINE` buys nothing —
not 30 seconds, not 30 minutes.

### Why shard 1, and why it can never be shard 0

The refusal's text is `StoreError::NotOwner`'s (`crates/aion-store/src/error.rs:33`),
**not** the routing edge's (`routing_resolve.rs:202` renders "workflow shard {n}
is owned by another cluster node"). So the start passed routing and was refused
deeper, by the store's epoch fence. The chain, all at the bytes:

1. The CLI never sets a routing key (`routing_key` appears nowhere in
   `crates/aion-cli/src`), so the start is **unsteered**.
2. `resolve_start` (`crates/aion-server/src/api/grpc/routing_resolve.rs`) takes
   the unsteered arm: `StartResolution::Local(self.start_placement())`, which
   calls `route_start` → `HaematiteStore::remint_for_owned_shard`. On a clustered
   node this **re-mints the workflow id onto a locally-owned shard**, so the
   workflow itself lands on node 0's shard 0. The R-1 remint is working.
3. The start then mints its namespace **on use**:
   `NamespaceMinter::mint_or_gate` → `NamespaceStore::register_namespace` →
   `HaematiteStore::register_namespace_record`.
4. That is a **quorum write** on `keyspace::namespace_key(name)` — a key derived
   from the NAMESPACE, with no relation to the workflow id. When the write is
   fenced it returns `StoreError::NotOwner { shard: database.shard_for(&key) }`
   (`crates/aion-store-haematite/src/store.rs:799` and `:823`).
5. Measured directly against a real `haematite::Database` with `shard_count = 3`:

   ```text
   PROBE namespace default -> shard 1
   PROBE namespace orders  -> shard 2
   ```

   `namespace_key("default")` lands on **shard 1**. Node 0 declares
   `owned_shards = [0]`. Its quorum write is fenced. Every time.

The minter's own doc already states the behaviour:

> A `aion_store::StoreError::NotOwner` from a quorum mint (this node is not the
> namespace shard's owner) propagates unchanged … which surfaces as the typed,
> *retryable* `NotOwner` wire code — never a silent success.

"Retryable" is true only for a **routing-aware caller that re-resolves to a
different node**. The `aion` CLI is not one: it retries the same endpoint and
fails identically forever. And note the shape of the seam — both branches of
`register_namespace_record` are quorum writes, the create-if-absent branch AND
the "exists, bump last_seen" touch. **Pre-creating the namespace does not help.**
On a clustered multi-shard deployment, a node can only accept a `start` for a
namespace whose registry record lives on a shard it owns.

### The brief's four candidates, answered

- *Is node 0 accepting starts and fencing them to other shards (routing / shard
  math)?* — **No.** The workflow's own placement is correct: R-1 re-mints it onto
  node 0's shard 0. The fence is on a **different key** entirely.
- *Is the cluster still forming quorum during the window (timing)?* — **No.** All
  three nodes are up and commissioned before the start loop begins (the loop is
  reached only after `boot_cluster` and the worker's readiness file). The refusal
  is identical on attempt 1 and attempt 262; nothing is converging.
- *Is `try_start` erroring outright (transport)?* — **Yes, and now we can read
  it**: exit status 1 with `error[unavailable] … NotOwner`. Not a transport
  failure; a typed application-level refusal. (The CLI's stock hint — "cannot
  reach the server; check --endpoint and that aion-server is running" — is
  actively misleading for this error code; noted below as adjacent debt.)
- *Is specimen 3's boot death a separate defect?* — **Yes, entirely separate**:
  finding A, the AirPlay port collision, fixed here.

**Classification: production start-path defect at the namespace-mint seam.** Per
the brief, this lane STOPS here and does not touch production code.

---

## Recommended next lane

The disposition is a ruling, not a builder's call, because it is a design
question with at least three defensible answers. The next lane should decide,
with the estate's authority, between:

1. **Route the namespace mint.** Treat the namespace record like any other shard
   write at the edge: resolve `shard_for(namespace_key(ns))`'s owner and forward
   the mint (the R-3 forwarder already exists and is used for steered starts), so
   any node can accept a start for any namespace.
2. **Make the on-use touch non-fencing.** The create-if-absent needs quorum; the
   "bump last_seen" touch on an already-existing record arguably does not, and
   collapsing it to a non-fenced local write would let any node start into an
   existing namespace. Narrower, but leaves first-use still single-homed.
3. **Declare it intended and make the client honest.** If a namespace really is
   single-homed by design, then the CLI must re-resolve on `NotOwner` instead of
   retrying a doomed endpoint, and the gate must be rewritten to start against
   the namespace's owner — which then requires a way to keep the workflow itself
   on the killed node's shard, so the kill still interrupts a real in-flight
   dispatch.

Only after that ruling can this gate be made expressible. Until then it stands
RED with its module doc stating exactly why, so nobody reads it as a proven
capability and nobody "fixes" it by weakening the assertions.

**Fleet gate 2 remains unmet.** The device (no build worker) remains the ruled
venue for the eventual clean specimen, once there is something that can pass.

---

## Battery

Run on the rebased tree at **5dd03128** (base `origin/main` d93f2342), logs and
`.exit` files in `gate-logs/failover-start-path/`. Verdicts below are the logs'
own verdict lines, not a summary:

| Leg | Command | `.exit` | Verdict line |
|-----|---------|---------|--------------|
| fmt | `cargo fmt --all --check` | 0 | (no output — clean) |
| clippy | `cargo clippy --workspace --all-targets -- -D warnings` | 0 | `Finished dev profile … in 49.47s` |
| tests | `cargo test --workspace --no-fail-fast` | 0 | 218 × `test result: ok`, **3020 passed, 0 failed**, zero `test result: FAILED` |
| e2e connect-half | `--test lsub5b_osproc_failover_e2e -- --ignored` | 0 | `test result: ok. 1 passed; 0 failed` |
| e2e kill-9 (specimen 7) | `--test lsub5b_osproc_kill9_failover_e2e -- --ignored` | **101** | `test result: FAILED. 0 passed; 1 failed` |

The `clippy` leg is the repo's own `gates.json` definition. The tests leg is
plain `cargo test`, not nextest, per the estate lead's RAIL 6 amendment (nextest's
per-test process concurrency reds aion's server-booting e2e suites on contention
alone).

**The kill-9 leg is RED and is expected to be RED.** It is red for the production
seam declared above, not for anything this lane could honestly fix, and it is the
lane's finding rather than its failure. Everything else in the battery is green,
including the connect-half sibling gate — which shares `common/osproc.rs`, was
moved off undrained pipes by Act 1, and still passes, so the shared-helper rework
did not damage its consumer.

## Scope wall

Board **#157**'s post-kill quorum bug (adopted-shard `WriteMembership` retaining
the dead owner) lives on the far side of a kill this gate has never reached. It
is cited, not touched. Nothing in this lane went near the post-kill path.

## Adjacent debt — named, deliberately untouched

- **`parked_timer_osproc_kill9_failover_e2e.rs:351`** carries its own local
  duplicate of `wait_for_liveness` with the identical pipe-drain defect (its
  `captured_output` at `:332` will return empty for the same reason, since that
  gate also redirects nodes to log files). It was left alone per the brief.
  *Recommendation: fold that gate onto `common/osproc.rs` — it inlined its
  helpers to avoid `dead_code` on unused shared items, which the shared module
  can now accommodate.*
- **Running this gate dirties a tracked file.** `build_worker_binary` runs
  `cargo build` in `spike/liminal-fan-worker`, whose committed `Cargo.lock` is
  stale (it pins `aion-core` 0.8.0, `beamr` 0.11). Every run re-resolves it and
  leaves ~480 lines of lockfile churn in the working tree. Reverted here, not
  committed. *Recommendation: refresh the spike lock, or build it with
  `--locked`, so a test run stops mutating the repo.*
- **The CLI's error hint is wrong for `NotOwner`.** "cannot reach the server;
  check --endpoint and that aion-server is running" is attached to an error that
  means the server answered and refused. It cost real diagnosis time here.
- **`outbox reconciler is UNCONFIGURED on a clustered boot`** — every node warns
  this on boot in this gate's config (`outbox.reconcile_interval_ms` /
  `reconcile_stale_after_ms` unset), which the warning itself says bounds
  post-kill in-flight recovery only by re-residency replay. Worth setting in this
  gate's config when the gate is next revisited, since post-kill recovery is
  exactly what it means to prove.
