# Venue conditions during the #83 landing battery

Recorded live, while the run was in flight, so the receipt does not have to
reconstruct them afterwards. [[a-red-must-carry-its-invocation]]

- **Box:** Tom's Mac, **10 cores** (`hw.ncpu`).
- **Battery:** `bash ./scripts/battery.sh`, pid **35942**, started
  **Thu 20 Aug 18:41:32 AEST 2026**, ppid 1 (detached).
  Exactly **one** `battery.sh` instance on the box — verified by enumerating
  `pgrep -f 'scripts/battery.sh'`, not assumed.
- **Load average at 19:07 AEST: 20.06 / 27.58 / 23.71** — i.e. roughly **2×
  oversubscribed** on 10 cores.

## What the load is, measured rather than guessed

- The battery's own leg: `cargo test --workspace --all-features --no-fail-fast`
  (pid 32474 → parent **35942**, confirmed by walking ppid). The test harness
  runs targets in parallel; most of the load is the run measuring itself.
- **Tom's live server `~/.cargo/bin/aion` (pid 67116) at ~67% CPU** — his
  production engine, resident on this same box. Never a venue for anything of
  mine, and a permanent tenant beside any battery run here.
- Ordinary desktop tenancy: WindowServer, Chrome/WebKit, several `claude`
  seats.
- **No `rustc` processes** at the time of sampling — this is test execution,
  not compilation.

⚠️ **PIDs have wrapped on this box** (uptime 1d 12h). A cargo child carried a
*lower* pid (32474) than its own parent battery (35942). Do not read pid
ordering as start ordering here; walk `ppid` instead. I nearly took the low pid
for a second, foreign battery.

## Why this is written down

🔴 **A relayed reassurance about headroom described a DIFFERENT BOX.** I was
told the estate cap had risen to two–three concurrent and that the load was
"~5 of 16 cores, plenty of headroom". This box has **10** cores and was at
**20+**. The figure was honest and was simply not about this venue.
[[a-comparison-has-two-sides-measure-both]] · [[global-gauge-cannot-attribute]]

**Consequence for reading this battery's verdict:** if a timing-sensitive leg
reds, contention is a live candidate and must be ruled out before the code is
blamed. The known one is **aion#54's shard-actor timeout**, whose base rate is
**2/24 on base == 2/24 on branch** — already established as flake, never to be
attributed to a branch. [[aion-54-known-flake-shard-actor-timeout]] ·
[[contention-pushes-assertions-directionally]] · venue before verdict.

A green under this load is a *stronger* result than a green on a quiet box, not
a weaker one. A red under it is not yet a finding.
