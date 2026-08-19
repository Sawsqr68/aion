# aion#85 — the real fix deployed to Tom's engine

**2026-08-19 06:19:04Z–06:19:57Z.** 53 seconds of downtime. Both gate inputs
landed first: leg two green, and Artemis's four-row arm statement re-affirmed at
the certified tree.

## The gate, as satisfied

**Input 1 — leg two.** Tom's own `watch_inbox`, unmodified, on the fixed binary,
DEFAULT JIT threshold, 400 scans:

```
status=Completed  events=2803  compiles=23
ActivityScheduled=400  ActivityCompleted=400  ChildWorkflowStarted=399
```

Against leg one on the unfixed binary: **2803 events, 400/400/399, 22 compiles.**
Identical on every count, with compilation demonstrably occurring on both sides
(so neither was a cold walk). **Regression evidence at full strength and no
more** — leg one's green dissolved the pairing's positive control, so no arm of
mine is evidence the fix *works*. That evidence is Artemis's, at the unit level.

**Input 2 — the arm statement at certified `470993d`:** `Waiting`, `DirtyCall`
and `Yielded` each FIXED in both helpers and PROVEN red-then-green by
per-mutation calibration (each mutation restored and md5-verified between arms;
11/11 green restored). Arm C carries its own negative control, so the reds
attribute to the compiled caller rather than to a merely-live JIT.
`Exited(_)`-without-exception stays **honestly unclaimed** with its discriminator
debt outstanding.

## 🔴 The precondition the gate did not name, and why it was checked

Artemis certifies beamr `470993d`. My staged binary is `aion-633b9c2`, named for
the beamr rev it embeds — **a different commit**. She had, in the same hour,
withdrawn a night of figures because her box carried an uncommitted working copy
she had verified file-by-file against a population that was "files I had in
mind" rather than "files the compiler reads". Deploying on the assumption that
two hashes meant the same tree would have been the identical error one seat
along.

Settled by looking:

- the beamr checkout the binary was built from (`/Users/tom/Developer/ablative/beamr-aion85`) is at `633b9c2246bb42119cf84eb99cd9bcb09151e3fc`, **detached HEAD, working tree clean — zero porcelain lines**
- `633b9c2` is a **direct ancestor** of `470993d`; `git log 470993d..633b9c2` is empty, so nothing in mine is absent from hers
- the delta is two commits labelled `test(jit)`. **Settled against ITS INPUTS, not against the commit messages or file extensions** — `git diff --name-only 633b9c2 470993d` returns exactly:
  - `crates/beamr/tests/jit_dirty_call_replay_probe.rs`
  - `crates/beamr/tests/jit_yield_replay_probe.rs`
  and nothing else. No `src`, no `Cargo.toml`, no `Cargo.lock`.
- Cargo integration tests compile as separate binaries and are never linked into
  the `beamr` library that aion consumes.

**Conclusion, as a build-graph claim:** the library source in the deployed binary
is identical to the library source Artemis's certified runs were made on.

## The deploy record

```
06:19:04Z new=14b509c16506da9033b6 old=18f89a62c7a6e388
06:19:04Z respawner HOLD set; waiting one full loop (25s) before touching the port
06:19:29Z serving pid 86322 verified as 18f89a62c7a6e388 — the image about to be replaced
06:19:29Z argv: aion server --jit-threshold 4294967295
06:19:29Z stopping server pid 86322
06:19:29Z port 50051 free
06:19:29Z installed 14b509c16506da90; rollback kept at aion.jitmitigation.prev (18f89a62c7a6e388)
06:19:29Z launched (NO --jit-threshold — default applies)
06:19:57Z UP — pid 67116 serving 50051, image 14b509c16506da90 (the fixed binary, verified as the port holder)
06:19:57Z argv: /Users/tom/.cargo/bin/aion server
  18 loaded versions
06:19:57Z respawner identity updated: pid=67116 started=Wed 19 Aug 16:19:29 2026
06:19:57Z respawner HOLD released
```

Port holder resolved and hashed **before and after** — the eighth law applied to
Tom's box rather than to my lab. The post-swap `argv` line is the witness that
the mitigation flag is *gone* rather than silently inherited.

## Why the flag is gone, and why that is the safer state

The `--jit-threshold 4294967295` mitigation was never an off-switch: at
`u32::MAX` it defers compilation to ~4.29e9 calls to one MFA. And it lived
**only on the running process's argv**. The estate respawner spawns
`os.execv(bin, [bin, "server"])` — no flag — so any death would have brought Tom
back on the **unfixed binary at the default threshold**, silently, which is
exactly the world the deaths happened in. Its log shows three respawns earlier
the same day.

**"Stay on the mitigation" was never the stable state; it was a state whose
stability depended on nothing crashing.** The fixed binary needs no flag, so the
safest configuration and the fixed configuration are the same one — and the
respawner's no-flag spawn form is now correct rather than dangerous.

## The durability proof, which arrived free

Tom's watcher `9a53974f` was mid-flight with **5,919 events** when the server
went down. It resumed on the fixed binary and ran to **natural completion at
6,005 events**. A five-thousand-event workflow carried across a binary swap with
no intervention, on his live surface, on real work — not a demo.

## Verification in flight

Fresh window `d728f2e9-178b-4d81-a73d-115799adafef`, his real inbox, `scans:
1200`, on the fixed engine. Running, **`unserved` empty** (the counting notebook
worker reconnected on its own after the restart). Being driven past **1009
events** — the count his real deaths landed at. With the mitigation gone this is
the first crossing of that number with the JIT live and nothing turned down.

## Rollback

One command, image kept beside the new one:

```
touch ~/.aion/respawner/HOLD
pkill -f 'aion server'
mv ~/.cargo/bin/aion.jitmitigation.prev ~/.cargo/bin/aion    # sha 18f89a62c7a6e388
cd /Users/tom/Developer/ablative/stack/aion && aion server --jit-threshold 4294967295
# then rewrite ~/.aion/respawner/server.pid + server.started, and rm HOLD
```

## Still owed, named rather than closed

- `Exited(_)`-without-exception: no route found, **not proven unreachable**. Discriminator debt is Artemis's and stays open.
- `slice_wall.awl` on the **unfixed** binary — the positive control the pairing lost, and the only artefact that could show the `Yielded` face was reachable from what a user can write. Built and staged; a read of the emitter until it fires.
- `nosleep_wall` re-paired on the private `probe_walls` queue, **both** binaries — the remembered `85c5f9d` figure was measured in a different store state and does not ride free.
- `JitCache::keys()` — enumeration of which functions reachable from an AWL workflow actually tier up.
- Respawner defects (Vesper's finding, logged with Waffles): the flagless spawn form (moot after this deploy, live as a class) and the **spawn-attribution lie** — it records whatever holds the port as its own child, so its "spawned pid=…" lines are not evidence it started anything. Same shape as the corpse-hash, in supervisor's clothing.
- Ops console `run unknown` — diagnosed in `gate-logs/console-run-unknown-diagnosis.md`, queued next.
