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
