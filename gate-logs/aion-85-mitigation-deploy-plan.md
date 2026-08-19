# aion#85 — mitigation deploy + verification plan (Vesper, 2026-08-19)

Authority: queue line 19 + Waffles's **scoped hold exception** written into the hold
line at 04:03Z (verified at HEAD, docs commit 9526b61). The rest of the box hold
stands; Athena and Hermes remain behind Tom's done-word.

## What is being deployed

`--jit-threshold` wired end to end (aion never named beamr's JIT threshold, so
beamr's default of 1000 governed every deployment aion has ever run):

```
--jit-threshold → [runtime] jit_threshold → EngineBuilder::scheduler_jit_threshold
  → RuntimeConfig::jit_threshold → SchedulerConfig
```

`None` passes through untouched, so an unset knob leaves the boot byte-identical.

**Value for Tom: `4294967295` (u32::MAX).** Verified at the bytes in both 0.18.2 and
the shipped 0.19.2: `saturating_add(1)` then `if new_count < current_threshold()`, so
this defers compilation to 4.29e9 calls **to one MFA** — at the ~5 calls per wait this
engine shows, ~859M waits. **It is not an off-switch and must never be described as
one**; compilation still occurs beyond that horizon. The real repair is Artemis's
PROPAGATE fix in beamr.

Build venue: worktree `scratchpad/wt-fix-77` at **67e0b8d9f** (v0.18.1, beamr 0.19.2,
haematite 0.9.0) — the release line. **Not** `investigate-workflow`, which is 0.17.0 on
beamr 0.18.2 and would have downgraded Tom's server by 36 commits.

## Deploy sequence

1. Build completes → `wt-fix-77/target/release/aion`.
2. Verify the artifact, **not** the claim:
   - `./target/release/aion --version` → expect `aion 0.18.1`
   - `./target/release/aion server --help | grep jit-threshold` → the flag exists
   - `shasum -a 256` the new binary; record it. **Identify by hash, never by version
     string** ([[swap-a-live-binary-by-rename]]).
3. **Tell Tom in the same breath, before the restart** — his live surface, his
   environment (Waffles's shape correction, 04:00Z). Wait if he is mid-test.
4. `lsof` the running binary path first. Stop the server (TERM; it went down in 2 s
   last time). **Swap by rename, never `cp`** — `cp` truncates the running image.
   Keep the previous binary as `aion.0181.prev`.
5. Start from **repo root** (`/Users/tom/Developer/ablative/stack/aion`) — the store is
   CWD-relative `./aion-data` — with `--jit-threshold 4294967295`, stdout to
   `~/.aion/logs/server-stdout-respawn.log`.
6. Verify launch **by first artifact**: `aion versions` non-empty. Never by pid.
7. Start Tom's watcher (`watch_inbox`, inbox/outbox under
   `prototyping/thing-finder/pocs/counting/`, `scans` 1200, detector `fast`).

## Verification — two-sided, and both sides are required

The old failure was **exactly at activity 200**, three independent confirmations on
the real product workflow (latest: wf `baaf0a3a`, Failed 03:55:35Z, 1009 events,
`{bad_generator, <<"activity:200">>}`).

**Side 1 — survival past the death point.** Watch the run cross **activity 300+** and
keep going. At ~3 s per round that is ~15 minutes. Crossing 200 is the claim; 300 is
the margin.

**Side 2 — the compile witness goes quiet, using the same instrument that convicted it.**

```
grep -c "cranelift_jit::backend" ~/.aion/logs/server-stdout-respawn.log
```

Under the old threshold this fired within 0.3–3.3 s before every first death. Under
`u32::MAX` there must be **no new compile events** for the workflow modules across a
run that would previously have died. Absence here is the positive result, and it is
checkable because the same grep produced 17 events on the poisoned server.

🔴 **Side 1 alone is not enough.** A run that merely survives longer could be luck,
load, or a different code path. Side 2 says *the mechanism did not fire*, which is the
thing the configuration claims to control. A green with only one side is the shape of
an instrument that reports a comfortable number without touching what it claims to
touch.

🔴 **Gate the watch on the event count INCREASING**, never on content that already
exists — an earlier check of mine passed instantly on the *previous* record and told
me a working fix had failed.

## Still owed after this lands

- **Proportionality test for Artemis** (the decisive science, and it needs its own
  slot — it cannot share a server instance with Tom's u32::MAX): start with
  `--jit-threshold 100`; both arms should die at ~1/10th the counts — sleep_wall at
  ~100 rounds, the activity arm at ~20 activities. **Two predicted numbers rather than
  one explained number.** If they do *not* scale, that is the more interesting result
  and says my correlation is measuring something adjacent — report either way.
- **Owed row (a): JIT-compiled observability.** `Scheduler::jit_cache()` is already
  public in beamr with `JitCache::lookup`; aion holds the scheduler and never asks
  (`crates/aion/src/runtime/` mentions jit nowhere before this change). Cache
  *observability* exists; only *control* was missing, which is what this row added.
- **Artemis's `Yielded` suspicion is a HYPOTHESIS, not a finding** — they explicitly
  have not measured it and named its discriminator first. Do not promote it.

## Acceptance leg — positive control (Waffles GO 04:13Z, in-scope of the same exception)

Isolated instance: `aion server --config <probe>/probe-config.toml --jit-threshold 100`,
ports 9099/59051/59061, store `/Users/tom/.aion/probe100/data`. Up 04:16Z, zero errors,
Tom's server untouched.

Two obstacles worth recording, both the system behaving correctly:
- With an explicit `--config`, the data root resolved to the **home** default rather than
  the CWD-relative one, and the home store is held by another live writer. Fixed by
  naming `store.data_dir` explicitly — which is the isolation the control wanted anyway.
- The path-safety gate **refused** a `data_dir` under `/private/tmp`: ancestor mode 1777
  grants group/world write, sticky bit not accepted. Correct refusal; moved under the
  owner-only Aion home.

**Predictions, named before the run** (already banked in the queue file so they cannot be
retrofitted): at threshold 100 the sleep arm dies at **~100 rounds** and the activity arm
at **~20 activities** — roughly a tenth of their counts at beamr's default 1000.

🔴 **RUN THE ARMS SEQUENTIALLY, ON A FRESH SERVER EACH.** The profiler counts per
`(module, function, arity)`, and both arms route through the same shared
`gleam_json_ffi` module. Run concurrently, one arm's calls can push the shared module
past the threshold and kill the other arm early — which would look like a confirmed
prediction while actually measuring cross-talk. Sleep arm first; restart the probe
server to clear counters before the activity arm.

The activity arm is served by `probe_worker.awl` — a minimal `scan_inbox` returning a
constant empty list (`aion awl check` clean). The counting notebook serves the same
contract but drags in the vision stack; this arm measures how many activity round-trips
a workflow survives, not what the activity says.

---

## The REAL fix — deploy of Artemis's beamr repair (2026-08-19, ~04:40Z)

The threshold knob above is a **mitigation and stays one**. This section is the
repair, and it deploys at the **DEFAULT** threshold with the flag dropped
entirely: running the fixed engine behind `u32::MAX` would leave the repair
permanently untested in production, which is the same mistake as shipping a
guard nobody audits.

**Artifact:** aion 0.18.1, sha256 `43b1b573a7c8103c…`, built in
`scratchpad/wt-fix-77` (branch `jit-threshold-on-0181`, base 67e0b8d9f = the
release line) against beamr `fix/aion85-nested-run-suspend` @ `85c5f9d6`.

**Patch witnesses — both required, because a version-mismatched `[patch]` drops
SILENTLY and everything still compiles:**
- **Lock:** exactly one `beamr` entry, 0.19.2, carrying **no `source` line** —
  which is how a path dependency appears, and how a registry copy would have
  given itself away.
- **Loud:** a compile-time reference to `Process::take_jit_transfer`, a symbol
  present in no published beamr. It compiled.
- **Version checked BEFORE building** against the `^0.19.2` requirement, since
  a mismatch is exactly the silent-drop case.

`a18bc77` (the tip Waffles relayed) verified **at the repo, not on his name**:
one commit touching `compiler_tests.rs` and the probe test only. Nothing outside
probe/test, so the build at `85c5f9d6` carries the identical fix.

### Two instrument bugs caught before they wrote a verdict

Both were mine, both in the acceptance harness, both would have produced a
confident wrong answer:

1. **The one-second boot.** The first run reported "server up" 1 s after killing
   the previous server. If the old (pre-fix) server had outlived the kill, it
   would have answered the readiness probe identically while the new process
   died on a bound port — grading the **pre-fix binary** and calling it the fix.
   Now the harness asserts the launched pid is still alive, that exactly one
   probe server exists, and that its executable image **hashes** to the fixed
   binary, printing `pid N, image <sha16>` before any arm starts.
   *Identify by hash, never by version string.*
2. **`grep -c` exits 1 on zero matches**, so the `|| echo 0` fallback appended a
   second line; every later integer test on the compile count would have errored
   into the failure branch — **a green arm reported red by its own counter.**

### Acceptance protocol

Three arms, **sequentially, on a fresh server each** (they share
`gleam_json_ffi`; run concurrently, one arm's calls push the shared module past
the threshold and kill another early — a confirmed prediction that actually
measured cross-talk). Threshold 100.

An arm PASSES only on **Completed AND a non-zero compile witness**. Completed
with `compiles=0` is graded **VACUOUS**, not green: it tested a JIT that never
fired. The witness channel is proven live on this exact rig and config (6 events
on the pre-fix probe server), so its silence would be meaningful.

| arm | rounds | calibrated red today (pre-fix) |
|---|---|---|
| `sleep_wall` | 400 | died at 100 |
| `nosleep_wall` | 200 | died at 20 |
| `fork_wall` | 200 | died at 50 |

Then: deploy at DEFAULT with rollback staged (`aion.jitmitigation.prev`), and
run Tom's real `watch_inbox` past **1009** events.

### 🔴 Correction to the record: what `fork_wall` does NOT show

`fork_wall.awl`'s header claims the arm "plausibly drives
`jit_dispatch_closure`". **That inference is dead**, per beamr#28: the runtime
JIT compile path forwards `ModuleCompileMetadata::EMPTY`, whose `lambdas` table
is empty, so **no function containing `make_fun` can ever be JIT-compiled in a
running scheduler** — it is refused as `UnsupportedOperand` and marked
permanently unsupported, so it never re-heats. Every Gleam closure-*minting*
function is therefore silently excluded from the JIT tier.

Consequences:
- The compiled frame in `fork_wall` is some **other** function. The arm never
  established anything about the CallFun site. The threshold-scaling evidence
  for "something compiled" stands; the attribution never existed.
- The nearest cause is a green from a door the tests don't use: beamr's unit
  tests call `compile_module_function` with a real lambda table and pass;
  production calls `compile`, which passes `EMPTY`. **The suite reports the
  passing door as coverage of the failing one.**
- Silver lining for the deploy record: `jit_dispatch_closure` is reachable only
  by a compiled function calling a closure it **received**, never one it mints.
  That bounds the exposure by mechanism rather than by hope.

Artemis subsequently **proved** the closure site with a reworked arm (driver
mints, `outer/2` receives, no `make_fun` in the compiled function), calibrated
red first against unfixed code. Both nested-run sites are now proven at the unit
level. Still owed and named: `DirtyCall` fixed-but-unproven; `Yielded` and
`Exited`-without-exception as untouched hypotheses.

### The honest sentence

A green here means **"the workload survives with compilation occurring"** — not
"both sites are fixed". Artemis's probe proves the mechanism in two constructed
functions; these arms prove a real workload survives. Those are different claims
and the deployment record keeps them apart.

---

## Owed row (a), now with a measured reason and a concrete shape

The deploy plan already carried "JIT-compiled observability" as owed. Tonight
it stopped being a nice-to-have and became the thing standing between a number
and a measurement.

**The gap, established by reading the source rather than inferring it:** beamr
emits **no tracing whatsoever** in the JIT compile path — zero
`tracing::{info,debug,trace}` under `crates/beamr/src/jit/` or `/scheduler/`.
The only compile evidence reaching aion is cranelift's own `defining function
funcidNN` at INFO, whose CLIF body carries no module or function name.

Consequence: `compiles > 0` says *something* heated. It can never say **which**
function did. Every arm tonight that reported a compile witness is therefore
reporting a real count keyed to the wrong thing — the same shape as the false
green Artemis caught in their own probe, where a counter bucketed on a value
the callee could clobber.

**What already exists:** `Scheduler::jit_cache()` is public and returns
`&Arc<JitCache>`; `JitCache::lookup(module, function, arity, generation)` is
public. So a *targeted* question — "is `gleam@list:try_fold/3` compiled?" — is
answerable today if the caller knows the generation. What is missing is
**enumeration**: there is no way to ask what the cache holds.

**The two shapes, cheapest first:**
1. *(beamr, preferred)* an iterator over `JitCache`'s keys, plus a
   `tracing::debug!` naming the MFA on **both** the accept and the refuse
   paths. The refuse half is already filed inside beamr#28 — the compile job
   discards the `JitError`, so an operator cannot learn why the tier declined a
   function. The same silence means they cannot learn what it accepted.
2. *(aion side)* a runtime query that asks the cache about a named MFA, so an
   arm's pass condition can be **that MFA present** rather than **count > 0**.

Until one lands, a lab arm can establish that a workload survives with
compilation occurring — it cannot establish that the compilation was of the
function under test. That limit is stated on every verdict tonight rather than
papered over.

**Not done during the pairing, deliberately.** It needs an aion rebuild, and a
rebuild mid-pairing hands the comparison a second variable. It goes after the
deploy.
